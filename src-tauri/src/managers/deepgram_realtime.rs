use anyhow::{anyhow, Result};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use log::{debug, info, warn};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::{timeout, MissedTickBehavior};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const DEEPGRAM_WS_BASE_URL: &str = "wss://api.deepgram.com/v1/listen";
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const AUDIO_QUEUE_CAPACITY: usize = 256;
const DEFAULT_KEEPALIVE_INTERVAL_SECONDS: u32 = 5;
const MIN_KEEPALIVE_INTERVAL_SECONDS: u32 = 3;
const MAX_KEEPALIVE_INTERVAL_SECONDS: u32 = 5;
const DEFAULT_ENDPOINTING_MS: u32 = 400;
const MIN_ENDPOINTING_MS: u32 = 50;
const MAX_ENDPOINTING_MS: u32 = 5000;

pub type FinalChunkCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub struct DeepgramRealtimeOptions {
    pub language: Option<String>,
    pub smart_format: bool,
    pub interim_results: bool,
    pub endpointing_enabled: bool,
    pub endpointing_ms: u32,
    pub keepalive_interval_seconds: u32,
}

impl Default for DeepgramRealtimeOptions {
    fn default() -> Self {
        Self {
            language: Some("en".to_string()),
            smart_format: true,
            interim_results: true,
            endpointing_enabled: true,
            endpointing_ms: DEFAULT_ENDPOINTING_MS,
            keepalive_interval_seconds: DEFAULT_KEEPALIVE_INTERVAL_SECONDS,
        }
    }
}

#[derive(Debug)]
enum ControlMessage {
    Finalize,
    Finish,
    Cancel,
}

struct ActiveSession {
    binding_id: String,
    audio_tx: mpsc::Sender<Vec<u8>>,
    control_tx: mpsc::UnboundedSender<ControlMessage>,
    final_text: Arc<Mutex<String>>,
    join_handle: JoinHandle<Result<()>>,
}

pub struct DeepgramRealtimeManager {
    app_handle: AppHandle,
    active_session: Mutex<Option<ActiveSession>>,
    pending_audio: Mutex<Vec<Vec<u8>>>,
}

impl DeepgramRealtimeManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle: app_handle.clone(),
            active_session: Mutex::new(None),
            pending_audio: Mutex::new(Vec::new()),
        })
    }

    pub fn clear_committed_transcript(&self) {
        let final_text = {
            let Ok(active_session) = self.active_session.lock() else {
                return;
            };
            let Some(session) = active_session.as_ref() else {
                return;
            };
            Arc::clone(&session.final_text)
        };
        let Ok(mut final_text_guard) = final_text.lock() else {
            return;
        };
        final_text_guard.clear();
    }

    pub fn is_realtime_model(model: &str) -> bool {
        let trimmed = model.trim();
        !trimmed.is_empty()
    }

    fn normalize_model(model: &str) -> String {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return "nova-3".to_string();
        }
        trimmed.to_string()
    }

    fn default_language_for_model(model: &str) -> &'static str {
        if model.trim().eq_ignore_ascii_case("nova-3-medical") {
            "en"
        } else {
            "multi"
        }
    }

    fn normalize_language(language: Option<String>, model: &str) -> Option<String> {
        let mut lang = language.unwrap_or_default().trim().to_string();
        if lang.is_empty() || lang.eq_ignore_ascii_case("auto") {
            return Some(Self::default_language_for_model(model).to_string());
        }

        if lang.eq_ignore_ascii_case("os_input") {
            if let Some(resolved) = crate::input_source::get_language_from_input_source() {
                lang = resolved;
            } else {
                return Some(Self::default_language_for_model(model).to_string());
            }
        }

        if lang == "zh-Hans" || lang == "zh-Hant" {
            return Some("zh".to_string());
        }

        Some(lang)
    }

    fn build_ws_url(model: &str, options: &DeepgramRealtimeOptions) -> Result<String> {
        let mut url = reqwest::Url::parse(DEEPGRAM_WS_BASE_URL)
            .map_err(|e| anyhow!("Invalid Deepgram WebSocket URL: {}", e))?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("model", model);
            qp.append_pair("encoding", "linear16");
            qp.append_pair("sample_rate", "16000");
            qp.append_pair("channels", "1");
            qp.append_pair(
                "smart_format",
                if options.smart_format { "true" } else { "false" },
            );
            qp.append_pair(
                "interim_results",
                if options.interim_results {
                    "true"
                } else {
                    "false"
                },
            );

            if options.endpointing_enabled {
                qp.append_pair(
                    "endpointing",
                    &options
                        .endpointing_ms
                        .clamp(MIN_ENDPOINTING_MS, MAX_ENDPOINTING_MS)
                        .to_string(),
                );
            } else {
                qp.append_pair("endpointing", "false");
            }

            if let Some(language) = Self::normalize_language(options.language.clone(), model) {
                qp.append_pair("language", &language);
            }
        }
        Ok(url.to_string())
    }

    pub fn start_session(
        &self,
        binding_id: &str,
        api_key: &str,
        model: &str,
        options: DeepgramRealtimeOptions,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("Deepgram API key is missing"));
        }

        let model = Self::normalize_model(model);
        if !Self::is_realtime_model(&model) {
            return Err(anyhow!("Deepgram live mode requires a model"));
        }

        let mut active_session_guard = self
            .active_session
            .lock()
            .map_err(|_| anyhow!("Failed to lock Deepgram live session state"))?;
        if active_session_guard.is_some() {
            return Err(anyhow!(
                "Deepgram live session is already active for this profile"
            ));
        }

        let keepalive_interval_seconds = options
            .keepalive_interval_seconds
            .clamp(MIN_KEEPALIVE_INTERVAL_SECONDS, MAX_KEEPALIVE_INTERVAL_SECONDS);
        let ws_url = Self::build_ws_url(&model, &options)?;
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(AUDIO_QUEUE_CAPACITY);
        let (control_tx, control_rx) = mpsc::unbounded_channel::<ControlMessage>();
        let final_text = Arc::new(Mutex::new(String::new()));
        let final_text_for_task = Arc::clone(&final_text);
        let app_handle_for_task = self.app_handle.clone();
        let binding_id_for_task = binding_id.to_string();
        let api_key_for_task = api_key.trim().to_string();
        let session_audio_tx = audio_tx.clone();

        let join_handle = tauri::async_runtime::spawn(async move {
            let session_result: Result<()> = async {
                let mut request = ws_url
                    .into_client_request()
                    .map_err(|e| anyhow!("Failed to create Deepgram request: {}", e))?;
                request.headers_mut().insert(
                    "Authorization",
                    format!("Token {}", api_key_for_task)
                        .parse()
                        .map_err(|e| anyhow!("Invalid Deepgram auth header: {}", e))?,
                );

                let (stream, _) = timeout(
                    Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
                    connect_async(request),
                )
                .await
                .map_err(|_| anyhow!("Timed out while connecting to Deepgram WebSocket"))?
                .map_err(|e| anyhow!("Failed to connect to Deepgram WebSocket: {}", e))?;

                let (mut write, mut read) = stream.split();
                Self::run_session_loop(
                    &mut write,
                    &mut read,
                    audio_rx,
                    control_rx,
                    final_text_for_task,
                    keepalive_interval_seconds,
                    app_handle_for_task.clone(),
                    on_final_chunk,
                )
                .await
            }
            .await;

            if let Err(err) = &session_result {
                let err_str = err.to_string();
                warn!(
                    "Deepgram live session runtime error (binding='{}'): {}",
                    binding_id_for_task, err_str
                );
                let _ = app_handle_for_task.emit("remote-stt-error", err_str.clone());
                crate::plus_overlay_state::handle_transcription_error(
                    &app_handle_for_task,
                    &err_str,
                );

                if crate::managers::preview_output_mode::is_active_for_binding(
                    &binding_id_for_task,
                ) {
                    crate::managers::preview_output_mode::set_error(
                        &app_handle_for_task,
                        Some(err_str.clone()),
                    );
                }
            }

            session_result
        });

        let active = ActiveSession {
            binding_id: binding_id.to_string(),
            audio_tx,
            control_tx,
            final_text,
            join_handle,
        };
        *active_session_guard = Some(active);
        drop(active_session_guard);

        let buffered = if let Ok(mut guard) = self.pending_audio.lock() {
            std::mem::take(&mut *guard)
        } else {
            Vec::new()
        };
        for chunk in buffered {
            if session_audio_tx.try_send(chunk).is_err() {
                break;
            }
        }

        let preserve_existing_preview =
            crate::managers::preview_output_mode::is_active_for_binding(binding_id);
        crate::overlay::begin_soniox_live_preview_session();
        if !preserve_existing_preview {
            crate::overlay::reset_soniox_live_preview(&self.app_handle);
        }
        crate::overlay::show_soniox_live_preview_window(&self.app_handle);

        info!("Started Deepgram live session for binding '{}'", binding_id);
        Ok(())
    }

    pub fn has_active_session(&self) -> bool {
        self.active_session
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_session_loop<S, R>(
        write: &mut S,
        read: &mut R,
        mut audio_rx: mpsc::Receiver<Vec<u8>>,
        mut control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        final_text: Arc<Mutex<String>>,
        keepalive_interval_seconds: u32,
        app_handle: AppHandle,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let keepalive_payload = Message::Text(r#"{"type":"KeepAlive"}"#.to_string().into());
        let finalize_payload = Message::Text(r#"{"type":"Finalize"}"#.to_string().into());
        let close_stream_payload = Message::Text(r#"{"type":"CloseStream"}"#.to_string().into());
        let mut last_audio_or_control = Instant::now();
        let mut keepalive_tick =
            tokio::time::interval(Duration::from_secs(keepalive_interval_seconds as u64));
        keepalive_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut finalize_requested = false;
        let mut finish_requested = false;
        let mut finalize_sent = false;
        let mut close_stream_sent = false;
        let mut audio_input_closed = false;

        loop {
            if audio_input_closed && !close_stream_sent {
                if finalize_requested && !finalize_sent {
                    write.send(finalize_payload.clone()).await.map_err(|e| {
                        anyhow!("Failed to send Deepgram finalize control message: {}", e)
                    })?;
                    finalize_sent = true;
                    last_audio_or_control = Instant::now();
                }

                if finish_requested {
                    write.send(close_stream_payload.clone()).await.map_err(|e| {
                        anyhow!("Failed to send Deepgram close stream message: {}", e)
                    })?;
                    write.flush().await.map_err(|e| {
                        anyhow!("Failed to flush Deepgram WebSocket stream: {}", e)
                    })?;
                    close_stream_sent = true;
                    last_audio_or_control = Instant::now();
                }
            }

            tokio::select! {
                Some(control) = control_rx.recv() => {
                    match control {
                        ControlMessage::Finalize => {
                            finalize_requested = true;
                            last_audio_or_control = Instant::now();
                        }
                        ControlMessage::Finish => {
                            finish_requested = true;
                            last_audio_or_control = Instant::now();
                        }
                        ControlMessage::Cancel => {
                            let _ = write.close().await;
                            return Ok(());
                        }
                    }
                }
                audio_chunk = audio_rx.recv() => {
                    let Some(audio_chunk) = audio_chunk else {
                        audio_input_closed = true;
                        continue;
                    };
                    if !audio_chunk.is_empty() {
                        write.send(Message::Binary(audio_chunk.into())).await
                            .map_err(|e| anyhow!("Failed to send audio chunk to Deepgram: {}", e))?;
                        last_audio_or_control = Instant::now();
                    }
                }
                frame = read.next() => {
                    let frame = match frame {
                        Some(frame) => frame.map_err(|e| anyhow!("Deepgram WebSocket read failed: {}", e))?,
                        None => return Err(anyhow!("Deepgram WebSocket closed before completion")),
                    };

                    match frame {
                        Message::Text(text) => {
                            let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                                let preview: String = text.chars().take(200).collect();
                                anyhow!("Invalid Deepgram WebSocket payload: {} (body: {})", e, preview)
                            })?;

                            let msg_type = payload
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();

                            if msg_type == "Metadata" {
                                break;
                            }

                            if msg_type != "Results" {
                                continue;
                            }

                            let transcript = payload
                                .get("channel")
                                .and_then(|ch| ch.get("alternatives"))
                                .and_then(|alts| alts.as_array())
                                .and_then(|alts| alts.first())
                                .and_then(|alt| alt.get("transcript"))
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            let is_final = payload
                                .get("is_final")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            // Deepgram utterance-boundary hint.
                            // We parse/accept it now, but current live behavior intentionally
                            // remains driven by is_final chunks; future updates may use this
                            // flag to trigger phrase-end actions.
                            let _speech_final = payload
                                .get("speech_final")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let from_finalize = payload
                                .get("from_finalize")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            let mut chunk_text = String::new();
                            if is_final {
                                let trimmed = transcript.trim();
                                if !trimmed.is_empty() {
                                    chunk_text = trimmed.to_string();
                                }
                            }

                            if !chunk_text.is_empty() && on_final_chunk.is_none() {
                                chunk_text = crate::text_replacement_decapitalize::maybe_decapitalize_next_chunk_realtime(&chunk_text);
                            }

                            if !chunk_text.is_empty() {
                                if let Ok(mut guard) = final_text.lock() {
                                    if !guard.is_empty() {
                                        guard.push(' ');
                                    }
                                    guard.push_str(&chunk_text);
                                }
                                if let Some(cb) = &on_final_chunk {
                                    cb(format!("{} ", chunk_text));
                                }
                            }

                            let mut interim_text = if is_final { String::new() } else { transcript };
                            if !interim_text.is_empty() && on_final_chunk.is_none() {
                                interim_text = crate::text_replacement_decapitalize::preview_decapitalize_next_chunk_realtime(&interim_text);
                            }
                            if !chunk_text.is_empty() || !interim_text.is_empty() || from_finalize {
                                let mut preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
                                if !chunk_text.is_empty() {
                                    if !preview_final_text.is_empty() {
                                        preview_final_text.push(' ');
                                    }
                                    preview_final_text.push_str(&chunk_text);
                                }
                                debug!(
                                    "Deepgram live preview update: final_chars={}, interim_chars={}, from_finalize={}",
                                    chunk_text.len(),
                                    interim_text.len(),
                                    from_finalize
                                );
                                crate::overlay::emit_soniox_live_preview_update(
                                    &app_handle,
                                    &preview_final_text,
                                    &interim_text,
                                );
                            }
                        }
                        Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => {}
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                _ = keepalive_tick.tick() => {
                    if close_stream_sent {
                        continue;
                    }
                    if Instant::now().duration_since(last_audio_or_control)
                        >= Duration::from_secs(keepalive_interval_seconds as u64)
                    {
                        write.send(keepalive_payload.clone()).await
                            .map_err(|e| anyhow!("Failed to send Deepgram keepalive control message: {}", e))?;
                        last_audio_or_control = Instant::now();
                    }
                }
            }
        }

        Ok(())
    }

    pub fn push_audio_frame(&self, frame_16khz_mono: Vec<f32>) {
        let sender = if let Ok(guard) = self.active_session.lock() {
            guard.as_ref().map(|session| session.audio_tx.clone())
        } else {
            None
        };

        let Some(sender) = sender else {
            if let Ok(mut pending) = self.pending_audio.lock() {
                if pending.len() > AUDIO_QUEUE_CAPACITY {
                    let _ = pending.remove(0);
                }
                pending.push(frame_16khz_mono_to_pcm_s16le_bytes(&frame_16khz_mono));
            }
            return;
        };

        let bytes = frame_16khz_mono_to_pcm_s16le_bytes(&frame_16khz_mono);
        if let Err(e) = sender.try_send(bytes) {
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    debug!("Deepgram live audio queue is full; dropping one audio chunk");
                }
                mpsc::error::TrySendError::Closed(_) => {}
            }
        }
    }

    pub async fn finish_session(&self, timeout_ms: u32) -> Result<String> {
        let hide_preview = || {
            if crate::managers::preview_output_mode::is_active() {
                return;
            }
            crate::overlay::end_soniox_live_preview_session();
            crate::overlay::hide_soniox_live_preview_window(&self.app_handle);
        };

        let mut session = {
            let mut guard = match self.active_session.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    hide_preview();
                    return Err(anyhow!("Failed to lock Deepgram live session state"));
                }
            };
            guard.take()
        };

        let Some(session) = session.take() else {
            hide_preview();
            return Ok(String::new());
        };
        let ActiveSession {
            binding_id,
            audio_tx,
            control_tx,
            final_text,
            mut join_handle,
            ..
        } = session;

        let read_final_text = || -> String {
            final_text
                .lock()
                .map(|guard| guard.trim().to_string())
                .unwrap_or_default()
        };

        let _ = control_tx.send(ControlMessage::Finalize);
        let _ = control_tx.send(ControlMessage::Finish);
        // Drop the session sender first so the receiver loop can observe channel closure
        // and proceed with Finalize/CloseStream dispatch.
        drop(audio_tx);

        let wait_ms = timeout_ms.clamp(100, 20000) as u64;
        let join_result = timeout(Duration::from_millis(wait_ms), &mut join_handle).await;

        match join_result {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(e))) => {
                let partial = read_final_text();
                if !partial.is_empty() {
                    warn!(
                        "Deepgram live session ended with error after partial output (binding='{}'): {}",
                        binding_id, e
                    );
                    hide_preview();
                    return Ok(partial);
                }
                hide_preview();
                return Err(e);
            }
            Ok(Err(e)) => {
                let partial = read_final_text();
                if !partial.is_empty() {
                    warn!(
                        "Deepgram live session join failed after partial output (binding='{}'): {}",
                        binding_id, e
                    );
                    hide_preview();
                    return Ok(partial);
                }
                hide_preview();
                return Err(anyhow!("Deepgram live session join failed: {}", e));
            }
            Err(_) => {
                join_handle.abort();
                let partial = read_final_text();
                if !partial.is_empty() {
                    warn!(
                        "Deepgram live session timed out after partial output (binding='{}', wait={}ms)",
                        binding_id, wait_ms
                    );
                    hide_preview();
                    return Ok(partial);
                }
                hide_preview();
                return Err(anyhow!(
                    "Timed out while waiting for Deepgram live session completion"
                ));
            }
        }

        let text = read_final_text();
        info!(
            "Completed Deepgram live session for binding '{}', output_len={}",
            binding_id,
            text.len()
        );
        hide_preview();
        Ok(text)
    }

    pub fn cancel(&self) {
        let hide_preview_if_needed = || {
            if !crate::managers::preview_output_mode::is_active() {
                crate::overlay::end_soniox_live_preview_session();
                crate::overlay::hide_soniox_live_preview_window(&self.app_handle);
            }
        };

        let active = {
            let mut guard = match self.active_session.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    hide_preview_if_needed();
                    return;
                }
            };
            guard.take()
        };

        if let Some(session) = active {
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
            warn!(
                "Cancelled active Deepgram live session for binding '{}'",
                session.binding_id
            );
        }
        if let Ok(mut pending) = self.pending_audio.lock() {
            pending.clear();
        }
        hide_preview_if_needed();
    }
}

fn frame_16khz_mono_to_pcm_s16le_bytes(frame: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(frame.len() * 2);
    for sample in frame {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}
