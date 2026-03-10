use crate::file_transcription_diarization::RawSpeakerBlock;
use crate::settings::SonioxContext;
use anyhow::{anyhow, Result};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::{timeout, MissedTickBehavior};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const SONIOX_WS_URL: &str = "wss://stt-rt.soniox.com/transcribe-websocket";
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const AUDIO_QUEUE_CAPACITY: usize = 256;
const DEFAULT_KEEPALIVE_INTERVAL_SECONDS: u32 = 10;
const MIN_KEEPALIVE_INTERVAL_SECONDS: u32 = 5;
const MAX_KEEPALIVE_INTERVAL_SECONDS: u32 = 20;

pub type FinalChunkCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub struct SonioxRealtimeOptions {
    pub language_hints: Vec<String>,
    pub language_hints_strict: bool,
    pub enable_speaker_diarization: bool,
    pub enable_language_identification: bool,
    pub enable_endpoint_detection: bool,
    pub max_endpoint_delay_ms: u32,
    pub keepalive_interval_seconds: u32,
    pub context: Option<SonioxContext>,
}

impl Default for SonioxRealtimeOptions {
    fn default() -> Self {
        Self {
            language_hints: vec!["en".to_string()],
            language_hints_strict: false,
            enable_speaker_diarization: true,
            enable_language_identification: true,
            enable_endpoint_detection: true,
            max_endpoint_delay_ms: 2000,
            keepalive_interval_seconds: DEFAULT_KEEPALIVE_INTERVAL_SECONDS,
            context: None,
        }
    }
}

#[derive(Serialize)]
struct SonioxStartRequest {
    api_key: String,
    model: String,
    audio_format: String,
    sample_rate: u32,
    num_channels: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_hints: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<SonioxContext>,
    language_hints_strict: bool,
    enable_speaker_diarization: bool,
    enable_language_identification: bool,
    enable_endpoint_detection: bool,
    max_endpoint_delay_ms: u32,
}

#[derive(Clone, Deserialize, Debug, Default)]
struct SonioxToken {
    text: String,
    #[serde(default)]
    is_final: bool,
    #[serde(default)]
    speaker: Option<Value>,
}

#[derive(Deserialize, Debug, Default)]
struct SonioxResponse {
    #[serde(default)]
    tokens: Vec<SonioxToken>,
    #[serde(default)]
    finished: bool,
    #[serde(default)]
    error_code: Option<u16>,
    #[serde(default)]
    error_message: Option<String>,
}

fn parse_soniox_speaker_key(value: &Value) -> Option<String> {
    if let Some(speaker) = value.as_str() {
        let trimmed = speaker.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_f64() {
        if number.is_finite() {
            return Some(number.to_string());
        }
    }
    None
}

fn is_soniox_control_token(text: &str) -> bool {
    matches!(text.trim(), "<fin>" | "<end>")
}

fn build_soniox_raw_speaker_blocks(tokens: &[SonioxToken], is_final: bool) -> Vec<RawSpeakerBlock> {
    let mut blocks: Vec<RawSpeakerBlock> = Vec::new();
    let mut current_speaker: Option<String> = None;
    let mut current_text = String::new();
    let mut pending_prefix = String::new();

    for token in tokens {
        if token.is_final != is_final
            || token.text.is_empty()
            || is_soniox_control_token(&token.text)
        {
            continue;
        }

        let speaker = token
            .speaker
            .as_ref()
            .and_then(parse_soniox_speaker_key)
            .or_else(|| current_speaker.clone());

        let Some(speaker_key) = speaker else {
            pending_prefix.push_str(&token.text);
            continue;
        };

        if current_speaker.as_deref() != Some(speaker_key.as_str())
            && !current_text.trim().is_empty()
        {
            blocks.push(RawSpeakerBlock {
                speaker_key: current_speaker.clone().unwrap_or_default(),
                default_name: None,
                text: current_text.trim().to_string(),
            });
            current_text.clear();
        }

        if current_speaker.as_deref() != Some(speaker_key.as_str()) {
            current_speaker = Some(speaker_key.clone());
            if !pending_prefix.is_empty() {
                current_text.push_str(&pending_prefix);
                pending_prefix.clear();
            }
        }

        current_text.push_str(&token.text);
    }

    if let Some(speaker_key) = current_speaker {
        let text = current_text.trim();
        if !text.is_empty() {
            blocks.push(RawSpeakerBlock {
                speaker_key,
                default_name: None,
                text: text.to_string(),
            });
        }
    }

    blocks
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

#[derive(Clone)]
struct SessionParams {
    binding_id: String,
    api_key: String,
    model: String,
    options: SonioxRealtimeOptions,
    on_final_chunk: Option<FinalChunkCallback>,
}

pub struct SonioxRealtimeManager {
    app_handle: AppHandle,
    active_session: Mutex<Option<ActiveSession>>,
    session_params: Mutex<Option<SessionParams>>,
    pending_audio: Mutex<Vec<Vec<u8>>>,
}

impl SonioxRealtimeManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle: app_handle.clone(),
            active_session: Mutex::new(None),
            session_params: Mutex::new(None),
            pending_audio: Mutex::new(Vec::new()),
        })
    }

    pub fn restart_session(&self) -> Result<()> {
        if !self.has_active_session() {
            return Ok(());
        }

        let params = self.session_params.lock().clone();

        if let Some(p) = params {
            self.cancel();
            self.start_session(
                &p.binding_id,
                &p.api_key,
                &p.model,
                p.options,
                p.on_final_chunk,
            )?;
        }
        Ok(())
    }

    pub fn is_realtime_model(model: &str) -> bool {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return true;
        }
        trimmed.starts_with("stt-rt")
    }

    fn normalize_model_for_realtime(model: &str) -> String {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return "stt-rt-v4".to_string();
        }
        if let Some(version) = trimmed.strip_prefix("stt-async-v") {
            return format!("stt-rt-v{}", version);
        }
        trimmed.to_string()
    }

    fn normalize_language_hints(language_hints: Vec<String>) -> Option<Vec<String>> {
        let normalized_hints = crate::language_resolver::normalize_soniox_hint_list(language_hints);
        if !normalized_hints.rejected.is_empty() {
            warn!(
                "Ignoring unsupported Soniox live language hints: {}",
                normalized_hints.rejected.join(", ")
            );
        }

        if normalized_hints.normalized.is_empty() {
            None
        } else {
            Some(normalized_hints.normalized)
        }
    }

    pub fn start_session(
        &self,
        binding_id: &str,
        api_key: &str,
        model: &str,
        options: SonioxRealtimeOptions,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("Soniox API key is missing"));
        }

        let model = Self::normalize_model_for_realtime(model);
        if !Self::is_realtime_model(&model) {
            return Err(anyhow!(
                "Soniox live mode requires a real-time model (stt-rt-*)"
            ));
        }

        let mut active_session_guard = self.active_session.lock();
        if active_session_guard.is_some() {
            return Err(anyhow!(
                "Soniox live session is already active for this profile"
            ));
        }

        {
            let mut params_guard = self.session_params.lock();
            *params_guard = Some(SessionParams {
                binding_id: binding_id.to_string(),
                api_key: api_key.to_string(),
                model: model.to_string(),
                options: options.clone(),
                on_final_chunk: on_final_chunk.clone(),
            });
        }

        let SonioxRealtimeOptions {
            language_hints,
            language_hints_strict,
            enable_speaker_diarization,
            enable_language_identification,
            enable_endpoint_detection,
            max_endpoint_delay_ms,
            keepalive_interval_seconds,
            context,
        } = options;

        let mut keepalive_interval_seconds = keepalive_interval_seconds;
        keepalive_interval_seconds = keepalive_interval_seconds.clamp(
            MIN_KEEPALIVE_INTERVAL_SECONDS,
            MAX_KEEPALIVE_INTERVAL_SECONDS,
        );

        let start_request = SonioxStartRequest {
            api_key: api_key.to_string(),
            model,
            audio_format: "pcm_s16le".to_string(),
            sample_rate: 16_000,
            num_channels: 1,
            language_hints: Self::normalize_language_hints(language_hints),
            context,
            language_hints_strict,
            enable_speaker_diarization,
            enable_language_identification,
            enable_endpoint_detection,
            max_endpoint_delay_ms: max_endpoint_delay_ms.clamp(500, 3000),
        };

        let start_payload = serde_json::to_string(&start_request)
            .map_err(|e| anyhow!("Failed to build Soniox start payload: {}", e))?;

        let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(AUDIO_QUEUE_CAPACITY);
        let (control_tx, control_rx) = mpsc::unbounded_channel::<ControlMessage>();
        let final_text = Arc::new(Mutex::new(String::new()));
        let final_text_for_task = Arc::clone(&final_text);
        let start_payload_for_task = start_payload;
        let app_handle_for_task = self.app_handle.clone();
        let binding_id_for_task = binding_id.to_string();
        let session_audio_tx = audio_tx.clone();

        let join_handle = tauri::async_runtime::spawn(async move {
            let session_result: Result<()> = async {
                let (stream, _) = timeout(
                    Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
                    connect_async(SONIOX_WS_URL),
                )
                .await
                .map_err(|_| anyhow!("Timed out while connecting to Soniox WebSocket"))?
                .map_err(|e| anyhow!("Failed to connect to Soniox WebSocket: {}", e))?;

                let (mut write, mut read) = stream.split();
                write
                    .send(Message::Text(start_payload_for_task.into()))
                    .await
                    .map_err(|e| anyhow!("Failed to send Soniox start request: {}", e))?;

                Self::run_session_loop(
                    &mut write,
                    &mut read,
                    audio_rx,
                    control_rx,
                    final_text_for_task,
                    keepalive_interval_seconds,
                    app_handle_for_task.clone(),
                    binding_id_for_task.clone(),
                    on_final_chunk,
                )
                .await
            }
            .await;

            if let Err(err) = &session_result {
                let err_str = err.to_string();
                warn!(
                    "Soniox live session runtime error (binding='{}'): {}",
                    binding_id_for_task, err_str
                );
                let _ = app_handle_for_task.emit("remote-stt-error", err_str.clone());
                crate::plus_overlay_state::handle_transcription_error(
                    &app_handle_for_task,
                    &err_str,
                );

                if crate::managers::preview_output_mode::is_active_for_binding(&binding_id_for_task)
                {
                    crate::managers::preview_output_mode::set_error(
                        &app_handle_for_task,
                        Some(err_str.clone()),
                    );
                }
                if binding_id_for_task == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
                    crate::managers::live_sound_transcription::set_recording(
                        &app_handle_for_task,
                        false,
                    );
                    crate::managers::live_sound_transcription::set_error(
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

        // Flush short buffered audio captured while websocket was connecting.
        let buffered = {
            let mut guard = self.pending_audio.lock();
            std::mem::take(&mut *guard)
        };
        for chunk in buffered {
            if session_audio_tx.try_send(chunk).is_err() {
                break;
            }
        }

        // Live Sound has its own UI — skip shared overlay preview state entirely
        // to avoid interfering with regular transcription's preview.
        if binding_id != crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
            let preserve_existing_preview =
                crate::managers::preview_output_mode::is_active_for_binding(binding_id);
            crate::overlay::begin_soniox_live_preview_session();
            if !preserve_existing_preview {
                crate::overlay::reset_soniox_live_preview(&self.app_handle);
            }
            crate::overlay::show_soniox_live_preview_window(&self.app_handle);
        }

        info!("Started Soniox live session for binding '{}'", binding_id);
        Ok(())
    }

    pub fn has_active_session(&self) -> bool {
        self.active_session.lock().is_some()
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
        binding_id: String,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let keepalive_payload = Message::Text(r#"{"type":"keepalive"}"#.to_string().into());
        let finalize_payload = Message::Text(r#"{"type":"finalize"}"#.to_string().into());
        let mut last_audio_or_control = Instant::now();
        let mut keepalive_tick =
            tokio::time::interval(Duration::from_secs(keepalive_interval_seconds as u64));
        keepalive_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut finished = false;
        loop {
            tokio::select! {
                Some(control) = control_rx.recv() => {
                    match control {
                        ControlMessage::Finalize => {
                            write.send(finalize_payload.clone()).await
                                .map_err(|e| anyhow!("Failed to send Soniox finalize control message: {}", e))?;
                            last_audio_or_control = Instant::now();
                        }
                        ControlMessage::Finish => {
                            // Empty frame gracefully closes the stream.
                            write.send(Message::Binary(Vec::new().into())).await
                                .map_err(|e| anyhow!("Failed to finalize Soniox audio stream: {}", e))?;
                            write.flush().await
                                .map_err(|e| anyhow!("Failed to flush Soniox WebSocket stream: {}", e))?;
                        }
                        ControlMessage::Cancel => {
                            let _ = write.close().await;
                            return Ok(());
                        }
                    }
                }
                Some(audio_chunk) = audio_rx.recv() => {
                    write.send(Message::Binary(audio_chunk.into())).await
                        .map_err(|e| anyhow!("Failed to send audio chunk to Soniox: {}", e))?;
                    last_audio_or_control = Instant::now();
                }
                frame = read.next() => {
                    let frame = match frame {
                        Some(frame) => frame.map_err(|e| anyhow!("Soniox WebSocket read failed: {}", e))?,
                        None => {
                            if finished {
                                break;
                            }
                            return Err(anyhow!("Soniox WebSocket closed before completion"));
                        }
                    };

                    match frame {
                        Message::Text(text) => {
                            let payload: SonioxResponse = serde_json::from_str(text.as_ref()).map_err(|e| {
                                let preview: String = text.chars().take(200).collect();
                                anyhow!("Invalid Soniox WebSocket payload: {} (body: {})", e, preview)
                            })?;

                            if let Some(code) = payload.error_code {
                                let message = payload.error_message.unwrap_or_else(|| "Unknown Soniox WebSocket error".to_string());
                                return Err(anyhow!("Soniox WebSocket error {}: {}", code, message));
                            }

                            let is_finished_payload = payload.finished;
                            let mut chunk_text = String::new();
                            let mut interim_text = String::new();
                            let mut final_token_count = 0usize;
                            let mut non_final_token_count = 0usize;
                            for token in &payload.tokens {
                                if token.text.is_empty()
                                    || token.text == "<fin>"
                                    || token.text == "<end>"
                                {
                                    continue;
                                }
                                if token.is_final {
                                    chunk_text.push_str(&token.text);
                                    final_token_count += 1;
                                } else {
                                    interim_text.push_str(&token.text);
                                    non_final_token_count += 1;
                                }
                            }

                            if !chunk_text.is_empty() && on_final_chunk.is_none() {
                                chunk_text = crate::text_replacement_decapitalize::maybe_decapitalize_next_chunk_realtime(&chunk_text);
                            }

                            if !chunk_text.is_empty() {
                                {
                                    let mut guard = final_text.lock();
                                    guard.push_str(&chunk_text);
                                }
                                if let Some(cb) = &on_final_chunk {
                                    cb(chunk_text.clone());
                                }
                            }

                            if is_finished_payload {
                                interim_text.clear();
                            }

                            if !interim_text.is_empty() && on_final_chunk.is_none() {
                                interim_text = crate::text_replacement_decapitalize::preview_decapitalize_next_chunk_realtime(&interim_text);
                            }

                            if !chunk_text.is_empty() || !interim_text.is_empty() || is_finished_payload {
                                if binding_id == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
                                    let final_blocks = if chunk_text.is_empty() {
                                        Vec::new()
                                    } else {
                                        build_soniox_raw_speaker_blocks(&payload.tokens, true)
                                    };
                                    let interim_blocks = if interim_text.is_empty() {
                                        Vec::new()
                                    } else {
                                        build_soniox_raw_speaker_blocks(&payload.tokens, false)
                                    };

                                    if !chunk_text.is_empty() {
                                        crate::managers::live_sound_transcription::append_final_result(
                                            &app_handle,
                                            &chunk_text,
                                            final_blocks,
                                            false,
                                        );
                                    }
                                    crate::managers::live_sound_transcription::set_interim_result(
                                        &app_handle,
                                        interim_text.clone(),
                                        interim_blocks,
                                    );
                                }

                                if binding_id != crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
                                    let mut preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
                                    if !chunk_text.is_empty() {
                                        preview_final_text.push_str(&chunk_text);
                                    }
                                    debug!(
                                        "Live preview update: final_tokens={}, non_final_tokens={}, final_chars={}, interim_chars={}, finished={}",
                                        final_token_count,
                                        non_final_token_count,
                                        chunk_text.len(),
                                        interim_text.len(),
                                        is_finished_payload
                                    );
                                    crate::overlay::emit_soniox_live_preview_update(
                                        &app_handle,
                                        &preview_final_text,
                                        &interim_text,
                                    );
                                }
                            }

                            if is_finished_payload {
                                finished = true;
                                break;
                            }
                        }
                        Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => {}
                        Message::Close(_) => {
                            if !finished {
                                return Err(anyhow!("Soniox WebSocket closed before completion"));
                            }
                            break;
                        }
                        _ => {}
                    }
                }
                _ = keepalive_tick.tick() => {
                    if Instant::now().duration_since(last_audio_or_control)
                        >= Duration::from_secs(keepalive_interval_seconds as u64)
                    {
                        write.send(keepalive_payload.clone()).await
                            .map_err(|e| anyhow!("Failed to send Soniox keepalive control message: {}", e))?;
                        last_audio_or_control = Instant::now();
                    }
                }
            }
        }

        if !finished {
            return Err(anyhow!(
                "Soniox WebSocket transcription did not report completion"
            ));
        }

        Ok(())
    }

    pub fn push_audio_frame(&self, frame_16khz_mono: Vec<f32>) {
        let sender = self.active_session.lock().as_ref().map(|session| session.audio_tx.clone());

        let Some(sender) = sender else {
            let mut pending = self.pending_audio.lock();
            if pending.len() > AUDIO_QUEUE_CAPACITY {
                let _ = pending.remove(0);
            }
            pending.push(frame_16khz_mono_to_pcm_s16le_bytes(&frame_16khz_mono));
            return;
        };

        let bytes = frame_16khz_mono_to_pcm_s16le_bytes(&frame_16khz_mono);
        if let Err(e) = sender.try_send(bytes) {
            // Dropping occasional chunks is preferable to blocking audio callback threads.
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    debug!("Soniox live audio queue is full; dropping one audio chunk");
                }
                mpsc::error::TrySendError::Closed(_) => {}
            }
        }
    }

    pub async fn finish_session(&self, timeout_ms: u32) -> Result<String> {
        let hide_preview = |binding_id: Option<&str>| {
            if binding_id == Some(crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID) {
                return;
            }
            if crate::managers::preview_output_mode::is_active() {
                return;
            }
            crate::overlay::end_soniox_live_preview_session();
            crate::overlay::hide_soniox_live_preview_window(&self.app_handle);
        };

        let session = self.active_session.lock().take();

        let Some(session) = session else {
            hide_preview(None);
            return Ok(String::new());
        };
        let ActiveSession {
            binding_id,
            control_tx,
            final_text,
            mut join_handle,
            ..
        } = session;
        let read_final_text = || -> String {
            final_text.lock().trim().to_string()
        };

        // Manual finalization first, then graceful stream end.
        let _ = control_tx.send(ControlMessage::Finalize);
        let _ = control_tx.send(ControlMessage::Finish);

        // Bound stop/finalization wait to a short, predictable window so
        // stop action can return to idle promptly even on unstable networks.
        let wait_ms = timeout_ms.clamp(100, 20000) as u64;
        let join_result = timeout(Duration::from_millis(wait_ms), &mut join_handle).await;

        match join_result {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(e))) => {
                let partial = read_final_text();
                if !partial.is_empty() {
                    warn!(
                        "Soniox live session ended with error after partial output (binding='{}'): {}",
                        binding_id, e
                    );
                    hide_preview(Some(&binding_id));
                    return Ok(partial);
                }
                hide_preview(Some(&binding_id));
                return Err(e);
            }
            Ok(Err(e)) => {
                let partial = read_final_text();
                if !partial.is_empty() {
                    warn!(
                        "Soniox live session join failed after partial output (binding='{}'): {}",
                        binding_id, e
                    );
                    hide_preview(Some(&binding_id));
                    return Ok(partial);
                }
                hide_preview(Some(&binding_id));
                return Err(anyhow!("Soniox live session join failed: {}", e));
            }
            Err(_) => {
                join_handle.abort();
                let partial = read_final_text();
                if !partial.is_empty() {
                    warn!(
                        "Soniox live session timed out after partial output (binding='{}', wait={}ms)",
                        binding_id, wait_ms
                    );
                    hide_preview(Some(&binding_id));
                    return Ok(partial);
                }
                hide_preview(Some(&binding_id));
                return Err(anyhow!(
                    "Timed out while waiting for Soniox live session completion"
                ));
            }
        }

        let text = read_final_text();

        info!(
            "Completed Soniox live session for binding '{}', output_len={}",
            binding_id,
            text.len()
        );
        hide_preview(Some(&binding_id));
        Ok(text)
    }

    pub fn cancel(&self) {
        let hide_preview_if_needed = |binding_id: Option<&str>| {
            if binding_id == Some(crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID) {
                return;
            }
            if !crate::managers::preview_output_mode::is_active() {
                crate::overlay::end_soniox_live_preview_session();
                crate::overlay::hide_soniox_live_preview_window(&self.app_handle);
            }
        };

        let active = self.active_session.lock().take();

        let cancelled_binding_id = active.as_ref().map(|session| session.binding_id.clone());

        if let Some(session) = active {
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
            warn!(
                "Cancelled active Soniox live session for binding '{}'",
                session.binding_id
            );
        }
        self.pending_audio.lock().clear();
        hide_preview_if_needed(cancelled_binding_id.as_deref());
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
