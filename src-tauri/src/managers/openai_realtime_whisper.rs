use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use log::{debug, info, warn};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::{interval, timeout, MissedTickBehavior};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub const OPENAI_REALTIME_WHISPER_MODEL: &str = "gpt-realtime-whisper";

// Server-side transcription WebSocket sessions are opened through the GA
// Realtime endpoint with transcription intent. The similarly named
// /realtime/transcription_sessions REST endpoint creates ephemeral tokens and
// rejects direct WebSocket upgrades with 403.
const OPENAI_REALTIME_TRANSCRIPTION_WS_URL: &str =
    "wss://api.openai.com/v1/realtime?intent=transcription";
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_FLATTENED_READ_TIMEOUT_SECS: u64 = 60;
const OPENAI_REALTIME_WHISPER_AUDIO_CHUNK_BYTES: usize = 48_000;
const AUDIO_QUEUE_CAPACITY: usize = 256;

pub type FinalChunkCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub struct OpenAiRealtimeWhisperOptions {
    pub language: Option<String>,
    pub delay: crate::settings::OpenAiRealtimeWhisperDelay,
}

impl Default for OpenAiRealtimeWhisperOptions {
    fn default() -> Self {
        Self {
            language: None,
            delay: crate::settings::OpenAiRealtimeWhisperDelay::Low,
        }
    }
}

#[derive(Debug)]
enum ControlMessage {
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
    options: OpenAiRealtimeWhisperOptions,
    on_final_chunk: Option<FinalChunkCallback>,
}

pub struct OpenAiRealtimeWhisperManager {
    app_handle: AppHandle,
    active_session: Mutex<Option<ActiveSession>>,
    session_params: Mutex<Option<SessionParams>>,
    pending_audio: Mutex<Vec<Vec<u8>>>,
}

impl OpenAiRealtimeWhisperManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle: app_handle.clone(),
            active_session: Mutex::new(None),
            session_params: Mutex::new(None),
            pending_audio: Mutex::new(Vec::new()),
        })
    }

    pub fn is_realtime_model(model: &str) -> bool {
        model
            .trim()
            .eq_ignore_ascii_case(OPENAI_REALTIME_WHISPER_MODEL)
    }

    pub fn restart_session(&self) -> Result<()> {
        if !self.has_active_session() {
            return Ok(());
        }

        let params = self.session_params.lock().clone();

        if let Some(p) = params {
            self.cancel();
            self.start_session(&p.binding_id, &p.api_key, p.options, p.on_final_chunk)?;
        }
        Ok(())
    }

    pub fn start_session(
        &self,
        binding_id: &str,
        api_key: &str,
        options: OpenAiRealtimeWhisperOptions,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("OpenAI API key is missing"));
        }

        let mut active_session_guard = self.active_session.lock();
        if active_session_guard.is_some() {
            return Err(anyhow!(
                "OpenAI Realtime Whisper session is already active for this profile"
            ));
        }

        {
            let mut params_guard = self.session_params.lock();
            *params_guard = Some(SessionParams {
                binding_id: binding_id.to_string(),
                api_key: api_key.to_string(),
                options: options.clone(),
                on_final_chunk: on_final_chunk.clone(),
            });
        }

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
                let mut request = Self::build_realtime_ws_url()
                    .into_client_request()
                    .map_err(|e| {
                        anyhow!("Failed to create OpenAI Realtime Whisper request: {}", e)
                    })?;
                request.headers_mut().insert(
                    "Authorization",
                    format!("Bearer {}", api_key_for_task)
                        .parse()
                        .map_err(|e| anyhow!("Invalid OpenAI auth header: {}", e))?,
                );
                request.headers_mut().insert(
                    "OpenAI-Safety-Identifier",
                    "aivorelay-remote-stt"
                        .parse()
                        .map_err(|e| anyhow!("Invalid OpenAI safety identifier header: {}", e))?,
                );

                let (stream, _) = timeout(
                    Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
                    connect_async(request),
                )
                .await
                .map_err(|_| anyhow!("Timed out while connecting to OpenAI Realtime Whisper"))?
                .map_err(|e| anyhow!("Failed to connect to OpenAI Realtime Whisper: {}", e))?;

                let (mut write, mut read) = stream.split();
                write
                    .send(Message::Text(
                        Self::build_session_update_payload(&options)
                            .to_string()
                            .into(),
                    ))
                    .await
                    .map_err(|e| {
                        anyhow!("Failed to send OpenAI transcription session update: {}", e)
                    })?;

                Self::run_session_loop(
                    &mut write,
                    &mut read,
                    audio_rx,
                    control_rx,
                    final_text_for_task,
                    app_handle_for_task.clone(),
                    binding_id_for_task.clone(),
                    Self::live_commit_interval_ms_for_delay(options.delay),
                    on_final_chunk,
                )
                .await
            }
            .await;

            if let Err(err) = &session_result {
                let err_str = err.to_string();
                warn!(
                    "OpenAI Realtime Whisper session runtime error (binding='{}'): {}",
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

        let buffered = {
            let mut guard = self.pending_audio.lock();
            std::mem::take(&mut *guard)
        };
        for chunk in buffered {
            if session_audio_tx.try_send(chunk).is_err() {
                break;
            }
        }

        if binding_id != crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
            let preserve_existing_preview =
                crate::managers::preview_output_mode::is_active_for_binding(binding_id);
            crate::overlay::begin_soniox_live_preview_session();
            if !preserve_existing_preview {
                crate::overlay::reset_soniox_live_preview(&self.app_handle);
            }
            crate::overlay::show_soniox_live_preview_window(&self.app_handle);
        }

        info!(
            "Started OpenAI Realtime Whisper session for binding '{}'",
            binding_id
        );
        Ok(())
    }

    pub fn has_active_session(&self) -> bool {
        self.active_session.lock().is_some()
    }

    pub async fn transcribe_flattened(
        &self,
        audio_samples: &[f32],
        api_key: &str,
        options: OpenAiRealtimeWhisperOptions,
    ) -> Result<String> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("OpenAI API key is missing"));
        }

        let pcm_bytes = resample_16khz_f32_to_24khz_pcm16(audio_samples);
        let mut request = Self::build_realtime_ws_url()
            .into_client_request()
            .map_err(|e| anyhow!("Failed to create OpenAI Realtime Whisper request: {}", e))?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", api_key.trim())
                .parse()
                .map_err(|e| anyhow!("Invalid OpenAI auth header: {}", e))?,
        );
        request.headers_mut().insert(
            "OpenAI-Safety-Identifier",
            "aivorelay-remote-stt"
                .parse()
                .map_err(|e| anyhow!("Invalid OpenAI safety identifier header: {}", e))?,
        );

        let (stream, _) = timeout(
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            connect_async(request),
        )
        .await
        .map_err(|_| anyhow!("Timed out while connecting to OpenAI Realtime Whisper"))?
        .map_err(|e| anyhow!("Failed to connect to OpenAI Realtime Whisper: {}", e))?;
        let (mut write, mut read) = stream.split();

        write
            .send(Message::Text(
                Self::build_session_update_payload(&options)
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|e| anyhow!("Failed to send OpenAI transcription session update: {}", e))?;

        for chunk in pcm_bytes.chunks(OPENAI_REALTIME_WHISPER_AUDIO_CHUNK_BYTES) {
            let append = json!({
                "type": "input_audio_buffer.append",
                "audio": BASE64_STANDARD.encode(chunk),
            });
            write
                .send(Message::Text(append.to_string().into()))
                .await
                .map_err(|e| {
                    anyhow!("Failed to send OpenAI Realtime Whisper audio chunk: {}", e)
                })?;
        }

        write
            .send(Message::Text(
                json!({ "type": "input_audio_buffer.commit" })
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|e| anyhow!("Failed to commit OpenAI transcription audio buffer: {}", e))?;
        write
            .flush()
            .await
            .map_err(|e| anyhow!("Failed to flush OpenAI transcription stream: {}", e))?;

        let mut deltas = String::new();
        let mut transcripts = Vec::new();
        let mut completed_item_ids: HashSet<String> = HashSet::new();

        loop {
            let frame = timeout(
                Duration::from_secs(DEFAULT_FLATTENED_READ_TIMEOUT_SECS),
                read.next(),
            )
            .await
            .map_err(|_| anyhow!("OpenAI Realtime Whisper WebSocket read timed out"))?;
            let Some(frame) = frame else {
                break;
            };
            let frame = frame
                .map_err(|e| anyhow!("OpenAI Realtime Whisper WebSocket read failed: {}", e))?;
            let Message::Text(text) = frame else {
                continue;
            };
            let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                let preview: String = text.chars().take(200).collect();
                anyhow!(
                    "Invalid OpenAI Realtime Whisper payload: {} (body: {})",
                    e,
                    preview
                )
            })?;
            let msg_type = payload
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if msg_type == "error" {
                let message = payload
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("OpenAI Realtime Whisper returned an error");
                return Err(anyhow!("{}", message));
            }

            match msg_type {
                "conversation.item.input_audio_transcription.delta" => {
                    if let Some(delta) = payload.get("delta").and_then(|v| v.as_str()) {
                        deltas.push_str(delta);
                    }
                }
                "conversation.item.input_audio_transcription.completed" => {
                    let item_id = payload
                        .get("item_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if !item_id.is_empty() && !completed_item_ids.insert(item_id) {
                        continue;
                    }

                    let transcript = payload
                        .get("transcript")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    if !transcript.is_empty() {
                        transcripts.push(transcript);
                    }
                    break;
                }
                _ => {}
            }
        }

        let _ = write.close().await;
        if transcripts.is_empty() {
            Ok(deltas.trim().to_string())
        } else {
            Ok(transcripts.join(" ").trim().to_string())
        }
    }

    fn build_session_update_payload(options: &OpenAiRealtimeWhisperOptions) -> Value {
        let mut transcription = json!({
            "model": OPENAI_REALTIME_WHISPER_MODEL,
            "delay": options.delay.as_str(),
        });

        if let Some(language) = Self::normalize_language(options.language.clone()) {
            transcription["language"] = json!(language);
        }

        json!({
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": {
                    "input": {
                        "format": {
                            "type": "audio/pcm",
                            "rate": 24000
                        },
                        "transcription": transcription,
                        "turn_detection": Value::Null
                    }
                }
            }
        })
    }

    fn build_realtime_ws_url() -> String {
        OPENAI_REALTIME_TRANSCRIPTION_WS_URL.to_string()
    }

    fn live_commit_interval_ms_for_delay(
        delay: crate::settings::OpenAiRealtimeWhisperDelay,
    ) -> u64 {
        match delay {
            crate::settings::OpenAiRealtimeWhisperDelay::Minimal => 1_500,
            crate::settings::OpenAiRealtimeWhisperDelay::Low => 3_000,
            crate::settings::OpenAiRealtimeWhisperDelay::Medium => 5_000,
            crate::settings::OpenAiRealtimeWhisperDelay::High => 7_000,
            crate::settings::OpenAiRealtimeWhisperDelay::XHigh => 10_000,
        }
    }

    async fn commit_input_audio_buffer<S>(write: &mut S) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        write
            .send(Message::Text(
                json!({ "type": "input_audio_buffer.commit" })
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|e| anyhow!("Failed to commit OpenAI transcription audio buffer: {}", e))?;
        write
            .flush()
            .await
            .map_err(|e| anyhow!("Failed to flush OpenAI transcription stream: {}", e))?;
        Ok(())
    }

    fn normalize_language(language: Option<String>) -> Option<String> {
        let mut lang = language.unwrap_or_default().trim().to_string();
        if lang.is_empty() || lang.eq_ignore_ascii_case("auto") {
            return None;
        }

        if lang.eq_ignore_ascii_case("os_input") {
            lang = crate::input_source::get_language_from_input_source()?;
        }

        if lang == "zh-Hans" || lang == "zh-Hant" {
            return Some("zh".to_string());
        }

        Some(lang)
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_session_loop<S, R>(
        write: &mut S,
        read: &mut R,
        mut audio_rx: mpsc::Receiver<Vec<u8>>,
        mut control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        final_text: Arc<Mutex<String>>,
        app_handle: AppHandle,
        binding_id: String,
        live_commit_interval_ms: u64,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let mut finish_requested = false;
        let mut audio_input_closed = false;
        let mut uncommitted_audio_bytes = 0usize;
        let mut pending_commits = 0usize;
        let mut completed_item_ids: HashSet<String> = HashSet::new();
        let mut interim_text = String::new();
        let mut live_commit_tick = interval(Duration::from_millis(live_commit_interval_ms));
        live_commit_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            if finish_requested && audio_input_closed {
                if uncommitted_audio_bytes > 0 {
                    Self::commit_input_audio_buffer(write).await?;
                    uncommitted_audio_bytes = 0;
                    pending_commits += 1;
                } else if pending_commits == 0 {
                    break;
                }
            }

            tokio::select! {
                Some(control) = control_rx.recv() => {
                    match control {
                        ControlMessage::Finish => {
                            finish_requested = true;
                        }
                        ControlMessage::Cancel => {
                            let _ = write.close().await;
                            return Ok(());
                        }
                    }
                }
                audio_chunk = audio_rx.recv(), if !audio_input_closed => {
                    let Some(audio_chunk) = audio_chunk else {
                        audio_input_closed = true;
                        continue;
                    };
                    if !audio_chunk.is_empty() {
                        let audio_chunk_len = audio_chunk.len();
                        let append = json!({
                            "type": "input_audio_buffer.append",
                            "audio": BASE64_STANDARD.encode(audio_chunk),
                        });
                        write.send(Message::Text(append.to_string().into())).await
                            .map_err(|e| anyhow!("Failed to send audio chunk to OpenAI Realtime Whisper: {}", e))?;
                        uncommitted_audio_bytes = uncommitted_audio_bytes.saturating_add(audio_chunk_len);
                    }
                }
                _ = live_commit_tick.tick(), if !finish_requested => {
                    if uncommitted_audio_bytes > 0 {
                        Self::commit_input_audio_buffer(write).await?;
                        uncommitted_audio_bytes = 0;
                        pending_commits += 1;
                    }
                }
                frame = read.next() => {
                    let frame = match frame {
                        Some(frame) => frame.map_err(|e| anyhow!("OpenAI Realtime Whisper WebSocket read failed: {}", e))?,
                        None => {
                            if completed_item_ids.is_empty() {
                                return Err(anyhow!("OpenAI Realtime Whisper WebSocket closed before transcription completed"));
                            }
                            break;
                        }
                    };

                    let Message::Text(text) = frame else {
                        continue;
                    };
                    let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                        let preview: String = text.chars().take(200).collect();
                        anyhow!(
                            "Invalid OpenAI Realtime Whisper payload: {} (body: {})",
                            e,
                            preview
                        )
                    })?;
                    let msg_type = payload
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();

                    if msg_type == "error" {
                        let message = payload
                            .get("error")
                            .and_then(|error| error.get("message"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("OpenAI Realtime Whisper returned an error");
                        return Err(anyhow!("{}", message));
                    }

                    match msg_type {
                        "session.updated" => {}
                        "input_audio_buffer.committed" => {}
                        "conversation.item.input_audio_transcription.delta" => {
                            let delta = payload
                                .get("delta")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            if delta.is_empty() {
                                continue;
                            }

                            interim_text.push_str(delta);
                            let mut preview_interim = interim_text.clone();
                            if on_final_chunk.is_none() {
                                preview_interim = crate::text_replacement_decapitalize::preview_decapitalize_next_chunk_realtime(&preview_interim);
                            }

                            if binding_id == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
                                crate::managers::live_sound_transcription::set_interim_result(
                                    &app_handle,
                                    preview_interim,
                                    Vec::new(),
                                );
                            } else {
                                let preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
                                crate::overlay::emit_soniox_live_preview_update(
                                    &app_handle,
                                    &preview_final_text,
                                    &preview_interim,
                                );
                            }
                        }
                        "conversation.item.input_audio_transcription.completed" => {
                            let item_id = payload
                                .get("item_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            if !item_id.is_empty() && !completed_item_ids.insert(item_id) {
                                continue;
                            }
                            pending_commits = pending_commits.saturating_sub(1);

                            let mut transcript = payload
                                .get("transcript")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .trim()
                                .to_string();
                            interim_text.clear();
                            if transcript.is_empty() {
                                if finish_requested && audio_input_closed && pending_commits == 0 && uncommitted_audio_bytes == 0 {
                                    break;
                                }
                                continue;
                            }
                            if on_final_chunk.is_none() {
                                transcript = crate::text_replacement_decapitalize::maybe_decapitalize_next_chunk_realtime(&transcript);
                            }

                            {
                                let mut guard = final_text.lock();
                                if !guard.is_empty() {
                                    guard.push(' ');
                                }
                                guard.push_str(&transcript);
                            }

                            if let Some(cb) = &on_final_chunk {
                                cb(format!("{} ", transcript));
                            }

                            if binding_id == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
                                crate::managers::live_sound_transcription::append_final_result(
                                    &app_handle,
                                    &transcript,
                                    Vec::new(),
                                    true,
                                );
                                crate::managers::live_sound_transcription::set_interim_result(
                                    &app_handle,
                                    String::new(),
                                    Vec::new(),
                                );
                            } else {
                                let preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
                                let next_preview_final = if preview_final_text.trim().is_empty() {
                                    transcript.clone()
                                } else {
                                    format!("{} {}", preview_final_text.trim_end(), transcript)
                                };
                                crate::overlay::emit_soniox_live_preview_update(
                                    &app_handle,
                                    &next_preview_final,
                                    "",
                                );
                            }

                            if finish_requested && audio_input_closed && pending_commits == 0 && uncommitted_audio_bytes == 0 {
                                break;
                            }
                        }
                        "conversation.item.input_audio_transcription.failed" => {
                            pending_commits = pending_commits.saturating_sub(1);
                            let message = payload
                                .get("error")
                                .and_then(|error| error.get("message"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("OpenAI Realtime Whisper transcription failed");
                            if finish_requested && audio_input_closed && pending_commits == 0 {
                                let partial = final_text.lock().trim().to_string();
                                if !partial.is_empty() {
                                    warn!(
                                        "OpenAI Realtime Whisper transcription item failed after partial output (binding='{}'): {}",
                                        binding_id, message
                                    );
                                    break;
                                }
                            }
                            return Err(anyhow!("{}", message));
                        }
                        _ => {
                            debug!("Ignoring OpenAI Realtime Whisper event '{}'", msg_type);
                        }
                    }
                }
            }
        }

        let _ = write.close().await;
        Ok(())
    }

    pub fn push_audio_frame(&self, frame_16khz_mono: Vec<f32>) {
        let sender = self
            .active_session
            .lock()
            .as_ref()
            .map(|session| session.audio_tx.clone());

        let bytes = resample_16khz_f32_to_24khz_pcm16(&frame_16khz_mono);
        let Some(sender) = sender else {
            let mut pending = self.pending_audio.lock();
            if pending.len() > AUDIO_QUEUE_CAPACITY {
                let _ = pending.remove(0);
            }
            pending.push(bytes);
            return;
        };

        if let Err(e) = sender.try_send(bytes) {
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    debug!("OpenAI Realtime Whisper audio queue is full; dropping one audio chunk");
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
            audio_tx,
            control_tx,
            final_text,
            mut join_handle,
            ..
        } = session;
        let read_final_text = || -> String { final_text.lock().trim().to_string() };

        let _ = control_tx.send(ControlMessage::Finish);
        drop(audio_tx);

        let wait_ms = timeout_ms.clamp(100, 20000) as u64;
        let join_result = timeout(Duration::from_millis(wait_ms), &mut join_handle).await;

        match join_result {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(e))) => {
                let partial = read_final_text();
                if !partial.is_empty() {
                    warn!(
                        "OpenAI Realtime Whisper session ended with error after partial output (binding='{}'): {}",
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
                        "OpenAI Realtime Whisper session join failed after partial output (binding='{}'): {}",
                        binding_id, e
                    );
                    hide_preview(Some(&binding_id));
                    return Ok(partial);
                }
                hide_preview(Some(&binding_id));
                return Err(anyhow!(
                    "OpenAI Realtime Whisper session join failed: {}",
                    e
                ));
            }
            Err(_) => {
                join_handle.abort();
                let partial = read_final_text();
                if !partial.is_empty() {
                    warn!(
                        "OpenAI Realtime Whisper session timed out after partial output (binding='{}', wait={}ms)",
                        binding_id, wait_ms
                    );
                    hide_preview(Some(&binding_id));
                    return Ok(partial);
                }
                hide_preview(Some(&binding_id));
                return Err(anyhow!(
                    "Timed out while waiting for OpenAI Realtime Whisper completion"
                ));
            }
        }

        let text = read_final_text();
        info!(
            "Completed OpenAI Realtime Whisper session for binding '{}', output_len={}",
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
                "Cancelled active OpenAI Realtime Whisper session for binding '{}'",
                session.binding_id
            );
        }
        self.pending_audio.lock().clear();
        hide_preview_if_needed(cancelled_binding_id.as_deref());
    }
}

fn resample_16khz_f32_to_24khz_pcm16(samples: &[f32]) -> Vec<u8> {
    if samples.is_empty() {
        return Vec::new();
    }

    let output_len = ((samples.len() as f64) * 24_000.0 / 16_000.0).round() as usize;
    let mut out = Vec::with_capacity(output_len * 2);

    for i in 0..output_len {
        let src_pos = (i as f64) * 16_000.0 / 24_000.0;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let s0 = samples.get(idx).copied().unwrap_or(0.0);
        let s1 = samples.get(idx + 1).copied().unwrap_or(s0);
        let sample = s0 + (s1 - s0) * frac;
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_update_uses_transcription_contract() {
        let payload = OpenAiRealtimeWhisperManager::build_session_update_payload(
            &OpenAiRealtimeWhisperOptions {
                language: Some("en".to_string()),
                delay: crate::settings::OpenAiRealtimeWhisperDelay::Medium,
            },
        );

        assert_eq!(payload["type"], "session.update");
        assert_eq!(payload["session"]["type"], "transcription");
        assert_eq!(
            payload["session"]["audio"]["input"]["format"]["type"],
            "audio/pcm"
        );
        assert_eq!(
            payload["session"]["audio"]["input"]["format"]["rate"],
            24000
        );
        assert_eq!(
            payload["session"]["audio"]["input"]["transcription"]["model"],
            OPENAI_REALTIME_WHISPER_MODEL
        );
        assert_eq!(
            payload["session"]["audio"]["input"]["transcription"]["delay"],
            "medium"
        );
        assert_eq!(
            payload["session"]["audio"]["input"]["transcription"]["language"],
            "en"
        );
        assert!(payload["session"]["audio"]["input"]["turn_detection"].is_null());
        assert!(payload.get("response").is_none());
    }

    #[test]
    fn websocket_url_uses_transcription_intent_not_model_query() {
        let url = OpenAiRealtimeWhisperManager::build_realtime_ws_url();

        assert!(url.contains("/v1/realtime"));
        assert!(url.contains("intent=transcription"));
        assert!(!url.contains("/v1/realtime/transcription_sessions"));
        assert!(!url.contains("model="));
        assert!(!url.contains("model=gpt-realtime-2"));
    }

    #[test]
    fn live_commit_interval_tracks_delay_setting() {
        assert_eq!(
            OpenAiRealtimeWhisperManager::live_commit_interval_ms_for_delay(
                crate::settings::OpenAiRealtimeWhisperDelay::Minimal
            ),
            1_500
        );
        assert_eq!(
            OpenAiRealtimeWhisperManager::live_commit_interval_ms_for_delay(
                crate::settings::OpenAiRealtimeWhisperDelay::Low
            ),
            3_000
        );
        assert_eq!(
            OpenAiRealtimeWhisperManager::live_commit_interval_ms_for_delay(
                crate::settings::OpenAiRealtimeWhisperDelay::XHigh
            ),
            10_000
        );
    }

    #[test]
    fn model_match_is_exact() {
        assert!(OpenAiRealtimeWhisperManager::is_realtime_model(
            "gpt-realtime-whisper"
        ));
        assert!(!OpenAiRealtimeWhisperManager::is_realtime_model(
            "gpt-realtime-2"
        ));
    }
}
