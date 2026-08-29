use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use log::{debug, info, warn};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::url_security::{REMOTE_STT_PRESET_GOOGLE, REMOTE_STT_PRESET_VERCEL};

pub const GEMINI_LIVE_DEFAULT_MODEL: &str = "google/gemini-3.5-transcribe-live";
pub const GEMINI_LIVE_GOOGLE_DEFAULT_MODEL: &str = "gemini-3.5-transcribe-live";
pub const GEMINI_LIVE_FINALIZE_TIMEOUT_MS: u32 = 8_000;

const GOOGLE_GEMINI_LIVE_WS_BASE_URL: &str =
    "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
const VERCEL_GEMINI_LIVE_WS_BASE_URL: &str =
    "wss://ai-gateway.vercel.sh/v4/ai/transcription-model";
const VERCEL_TRANSCRIPTION_SUBPROTOCOL: &str = "ai-gateway-transcription.v1";
const VERCEL_AUTH_SUBPROTOCOL_PREFIX: &str = "ai-gateway-auth.";
const VERCEL_GATEWAY_PROTOCOL_VERSION: &str = "0.0.1";
const VERCEL_TRANSCRIPTION_SPECIFICATION_VERSION: &str = "4";
const VERCEL_MAX_AUDIO_FRAME_BYTES: usize = 64 * 1024;

const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const GEMINI_LIVE_MAX_SESSION_SECS: u64 = 10 * 60;
const GOOGLE_FINAL_TRANSCRIPT_GRACE_MS: u64 = 3_000;
const AUDIO_QUEUE_CAPACITY: usize = 256;

pub type FinalChunkCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub struct GeminiRealtimeOptions {
    pub model: String,
    pub language: Option<String>,
    pub preset: String,
    pub translate_to_english: bool,
}

impl Default for GeminiRealtimeOptions {
    fn default() -> Self {
        Self {
            model: GEMINI_LIVE_DEFAULT_MODEL.to_string(),
            language: None,
            preset: REMOTE_STT_PRESET_VERCEL.to_string(),
            translate_to_english: false,
        }
    }
}

#[derive(Debug)]
enum ControlMessage {
    Finish,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeminiLiveTransport {
    GoogleDirect,
    VercelGateway,
}

struct CompletedDirectSegment {
    text: String,
    was_streamed: bool,
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
    options: GeminiRealtimeOptions,
    on_final_chunk: Option<FinalChunkCallback>,
}

pub struct GeminiRealtimeManager {
    app_handle: AppHandle,
    active_session: Mutex<Option<ActiveSession>>,
    session_params: Mutex<Option<SessionParams>>,
    pending_audio: Mutex<Vec<Vec<u8>>>,
}

impl GeminiRealtimeManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle: app_handle.clone(),
            active_session: Mutex::new(None),
            session_params: Mutex::new(None),
            pending_audio: Mutex::new(Vec::new()),
        })
    }

    pub fn is_realtime_model(model: &str) -> bool {
        let model = model.trim();
        model.eq_ignore_ascii_case(GEMINI_LIVE_DEFAULT_MODEL)
            || model.eq_ignore_ascii_case(GEMINI_LIVE_GOOGLE_DEFAULT_MODEL)
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
        options: GeminiRealtimeOptions,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("Gemini 3.5 Transcribe Live API key is missing"));
        }

        let transport = match options.preset.as_str() {
            REMOTE_STT_PRESET_GOOGLE => GeminiLiveTransport::GoogleDirect,
            REMOTE_STT_PRESET_VERCEL => GeminiLiveTransport::VercelGateway,
            _ => {
                return Err(anyhow!(
                    "Gemini 3.5 Transcribe Live requires the Vercel or Google connection route"
                ));
            }
        };
        let expected_model = match transport {
            GeminiLiveTransport::GoogleDirect => GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
            GeminiLiveTransport::VercelGateway => GEMINI_LIVE_DEFAULT_MODEL,
        };
        if !options.model.trim().eq_ignore_ascii_case(expected_model) {
            return Err(anyhow!(
                "This Gemini 3.5 Transcribe Live route requires model '{}', but settings contain '{}'",
                expected_model,
                options.model.trim()
            ));
        }
        if options.translate_to_english {
            return Err(anyhow!(
                "Gemini 3.5 Transcribe Live does not support AivoRelay's Translate to English option. Disable it in the active profile or global settings."
            ));
        }

        let mut active_session_guard = self.active_session.lock();
        if active_session_guard.is_some() {
            return Err(anyhow!(
                "Gemini 3.5 Transcribe Live session is already active for this profile"
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
        let live_sound_session_id = (binding_id
            == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID)
            .then(crate::managers::live_sound_transcription::current_session_id);
        let api_key_for_task = api_key.trim().to_string();
        let session_audio_tx = audio_tx.clone();

        let join_handle = tauri::async_runtime::spawn(async move {
            let session_result: Result<()> = async {
                let request = build_live_websocket_request(
                    transport,
                    &options.model,
                    &api_key_for_task,
                )?;

                let (stream, _) = timeout(
                    Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
                    connect_async(request),
                )
                .await
                .map_err(|_| anyhow!("Timed out while connecting to Gemini 3.5 Transcribe Live"))?
                .map_err(|e| anyhow!("Failed to connect to Gemini 3.5 Transcribe Live: {}", e))?;

                let (mut write, mut read) = stream.split();

                let language = Self::resolve_language(options.language.as_deref());
                let setup_payload = match transport {
                    GeminiLiveTransport::GoogleDirect => {
                        Self::build_google_setup_payload(&options.model, language.as_deref())
                    }
                    GeminiLiveTransport::VercelGateway => {
                        Self::build_vercel_start_payload(language.as_deref())
                    }
                };
                write
                    .send(Message::Text(setup_payload.to_string().into()))
                    .await
                    .map_err(|e| anyhow!("Failed to send Gemini 3.5 Transcribe Live setup message: {}", e))?;

                Self::run_session_loop(
                    &mut write,
                    &mut read,
                    audio_rx,
                    control_rx,
                    final_text_for_task,
                    app_handle_for_task.clone(),
                    binding_id_for_task.clone(),
                    live_sound_session_id,
                    on_final_chunk,
                    transport,
                )
                .await
            }
            .await;

            if let Err(err) = &session_result {
                let err_str = err.to_string();
                warn!(
                    "Gemini 3.5 Transcribe Live session runtime error (binding='{}'): {}",
                    binding_id_for_task, err_str
                );
                if live_sound_session_id.is_none() {
                    crate::actions::stop_transcription_after_realtime_error(
                        &app_handle_for_task,
                        &binding_id_for_task,
                    );
                }
                let callback_is_current = live_sound_session_id
                    .map(crate::managers::live_sound_transcription::is_session_current)
                    .unwrap_or(true);
                if callback_is_current {
                    let _ = app_handle_for_task.emit("remote-stt-error", err_str.clone());
                    if live_sound_session_id.is_none() {
                        crate::plus_overlay_state::handle_transcription_error(
                            &app_handle_for_task,
                            &err_str,
                        );
                    }
                }

                if crate::managers::preview_output_mode::is_active_for_binding(&binding_id_for_task)
                {
                    crate::managers::preview_output_mode::set_error(
                        &app_handle_for_task,
                        Some(err_str.clone()),
                    );
                }
                if let Some(session_id) = live_sound_session_id {
                    if crate::managers::live_sound_transcription::is_session_current(session_id) {
                        crate::managers::live_sound_transcription::set_recording_if_session_matches(
                            &app_handle_for_task,
                            session_id,
                            false,
                        );
                        crate::managers::live_sound_transcription::set_error_if_session_matches(
                            &app_handle_for_task,
                            session_id,
                            Some(err_str.clone()),
                        );
                        crate::managers::live_sound_audio::stop(&app_handle_for_task);
                    }
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

        // Flush short buffered audio captured while websocket was connecting
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
            "Started Gemini 3.5 Transcribe Live session for binding '{}'",
            binding_id
        );
        Ok(())
    }

    pub fn has_active_session(&self) -> bool {
        self.active_session.lock().is_some()
    }

    fn normalize_gemini_model(model: &str) -> String {
        let trimmed = model.trim();
        let stripped = trimmed.strip_prefix("google/").unwrap_or(trimmed);
        if stripped.starts_with("models/") {
            stripped.to_string()
        } else {
            format!("models/{}", stripped)
        }
    }

    fn resolve_language(language: Option<&str>) -> Option<String> {
        let mut language = language?.trim().to_string();
        if language.eq_ignore_ascii_case("os_input") {
            language = crate::input_source::get_language_from_input_source()?;
        }
        if language.is_empty() || language.eq_ignore_ascii_case("auto") {
            return None;
        }
        crate::managers::remote_stt::normalize_gemini_language_code(&language)
            .map(str::to_string)
    }

    fn build_google_setup_payload(model: &str, language: Option<&str>) -> Value {
        let input_audio_transcription = language
            .map(|value| json!({ "languageCodes": [value] }))
            .unwrap_or_else(|| json!({}));
        // The current Google SDK omits generationConfig because it suppresses
        // finalized inputTranscription events on this dedicated model.
        json!({
            "setup": {
                "model": Self::normalize_gemini_model(model),
                "inputAudioTranscription": input_audio_transcription
            }
        })
    }

    fn build_vercel_start_payload(language: Option<&str>) -> Value {
        let mut payload = json!({
            "type": "transcription-stream.start",
            "inputAudioFormat": {
                "type": "audio/pcm",
                "rate": 16000
            }
        });
        if let Some(language) = language {
            payload["providerOptions"] = json!({
                "google": {
                    "languageCodes": [language]
                }
            });
        }
        payload
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
        live_sound_session_id: Option<u64>,
        on_final_chunk: Option<FinalChunkCallback>,
        transport: GeminiLiveTransport,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let mut audio_input_closed = false;
        let mut end_signal_sent = false;
        let mut google_setup_complete = transport == GeminiLiveTransport::VercelGateway;
        let mut accumulated_text = String::new();
        let mut gateway_segment_has_delta = false;
        let mut direct_segment_text = String::new();
        let mut latest_interim = String::new();
        let session_limit = tokio::time::sleep(Duration::from_secs(GEMINI_LIVE_MAX_SESSION_SECS));
        tokio::pin!(session_limit);
        let google_finish_grace =
            tokio::time::sleep(Duration::from_secs(GEMINI_LIVE_MAX_SESSION_SECS));
        tokio::pin!(google_finish_grace);

        loop {
            tokio::select! {
                _ = &mut session_limit => {
                    return Err(anyhow!(
                        "Gemini 3.5 Transcribe Live reached its 10-minute session limit. Start a new recording to continue."
                    ));
                }
                _ = &mut google_finish_grace,
                    if transport == GeminiLiveTransport::GoogleDirect && end_signal_sent =>
                {
                    break;
                }
                Some(control) = control_rx.recv() => {
                    match control {
                        ControlMessage::Finish => {}
                        ControlMessage::Cancel => {
                            let _ = write.close().await;
                            return Ok(());
                        }
                    }
                }
                audio_chunk = audio_rx.recv(), if google_setup_complete && !audio_input_closed => {
                    match audio_chunk {
                        Some(audio_chunk) if !audio_chunk.is_empty() => {
                            match transport {
                                GeminiLiveTransport::GoogleDirect => {
                                    let media_payload = json!({
                                        "realtimeInput": {
                                            "audio": {
                                                "mimeType": "audio/pcm;rate=16000",
                                                "data": BASE64_STANDARD.encode(&audio_chunk)
                                            }
                                        }
                                    });
                                    write.send(Message::Text(media_payload.to_string().into())).await
                                        .map_err(|e| anyhow!("Failed to send audio chunk to Gemini 3.5 Transcribe Live: {}", e))?;
                                }
                                GeminiLiveTransport::VercelGateway => {
                                    for frame in audio_chunk.chunks(VERCEL_MAX_AUDIO_FRAME_BYTES) {
                                        write.send(Message::Binary(frame.to_vec().into())).await
                                            .map_err(|e| anyhow!("Failed to send audio chunk to Vercel AI Gateway: {}", e))?;
                                    }
                                }
                            }
                        }
                        Some(_) => {}
                        None => {
                            audio_input_closed = true;
                            let end_payload = match transport {
                                GeminiLiveTransport::GoogleDirect => {
                                    json!({ "realtimeInput": { "audioStreamEnd": true } })
                                }
                                GeminiLiveTransport::VercelGateway => {
                                    json!({ "type": "transcription-stream.audio-done" })
                                }
                            };
                            write.send(Message::Text(end_payload.to_string().into())).await
                                .map_err(|e| anyhow!("Failed to finish Gemini 3.5 Transcribe Live audio input: {}", e))?;
                            end_signal_sent = true;
                            if transport == GeminiLiveTransport::GoogleDirect {
                                google_finish_grace.as_mut().reset(
                                    tokio::time::Instant::now()
                                        + Duration::from_millis(GOOGLE_FINAL_TRANSCRIPT_GRACE_MS),
                                );
                            }
                        }
                    }
                }
                frame = read.next() => {
                    let frame = match frame {
                        Some(frame) => frame.map_err(|e| anyhow!("Gemini 3.5 Transcribe Live WebSocket read failed: {}", e))?,
                        None => {
                            if transport == GeminiLiveTransport::VercelGateway {
                                return Err(anyhow!(
                                    "Vercel AI Gateway transcription stream closed before a finish message was received"
                                ));
                            }
                            if end_signal_sent {
                                break;
                            }
                            return Err(anyhow!("Gemini 3.5 Transcribe Live WebSocket closed before transcription completed"));
                        }
                    };

                    let text = match frame {
                        Message::Text(text) => text,
                        Message::Ping(payload) => {
                            write.send(Message::Pong(payload)).await
                                .map_err(|e| anyhow!("Failed to answer Gemini 3.5 Transcribe Live ping: {}", e))?;
                            continue;
                        }
                        Message::Close(_) => {
                            if transport == GeminiLiveTransport::VercelGateway {
                                return Err(anyhow!(
                                    "Vercel AI Gateway transcription stream closed before a finish message was received"
                                ));
                            }
                            if end_signal_sent {
                                break;
                            }
                            return Err(anyhow!("Gemini 3.5 Transcribe Live WebSocket closed before transcription completed"));
                        }
                        _ => continue,
                    };
                    let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                        let preview: String = text.chars().take(200).collect();
                        anyhow!("Invalid Gemini 3.5 Transcribe Live payload: {} (body: {})", e, preview)
                    })?;

                    if transport == GeminiLiveTransport::VercelGateway {
                        let part_type = payload.get("type").and_then(Value::as_str).unwrap_or("");
                        match part_type {
                            "transcript-delta" => {
                                if let Some(delta) = payload.get("delta").and_then(Value::as_str) {
                                    accumulated_text.push_str(delta);
                                    gateway_segment_has_delta = true;
                                    *final_text.lock() = accumulated_text.clone();
                                    if let Some(callback) = &on_final_chunk {
                                        callback(delta.to_string());
                                    }
                                    emit_live_text(
                                        &app_handle,
                                        &binding_id,
                                        live_sound_session_id,
                                        accumulated_text.trim(),
                                    );
                                }
                            }
                            "transcript-partial" => {
                                if let Some(partial) = payload.get("text").and_then(Value::as_str) {
                                    let display = join_transcript_text(&accumulated_text, partial);
                                    emit_live_text(
                                        &app_handle,
                                        &binding_id,
                                        live_sound_session_id,
                                        display.trim(),
                                    );
                                }
                            }
                            "transcript-final" => {
                                if !gateway_segment_has_delta {
                                    if let Some(text) = payload.get("text").and_then(Value::as_str) {
                                        accumulated_text =
                                            join_transcript_text(&accumulated_text, text);
                                        *final_text.lock() = accumulated_text.clone();
                                        if let Some(callback) = &on_final_chunk {
                                            callback(text.to_string());
                                        }
                                    }
                                }
                                if !accumulated_text.is_empty()
                                    && !accumulated_text
                                        .chars()
                                        .last()
                                        .is_some_and(char::is_whitespace)
                                {
                                    accumulated_text.push(' ');
                                    *final_text.lock() = accumulated_text.clone();
                                    if let Some(callback) = &on_final_chunk {
                                        callback(" ".to_string());
                                    }
                                }
                                gateway_segment_has_delta = false;
                            }
                            "finish" => {
                                if let Some(text) = payload.get("text").and_then(Value::as_str) {
                                    accumulated_text = text.to_string();
                                    *final_text.lock() = accumulated_text.clone();
                                    emit_live_text(
                                        &app_handle,
                                        &binding_id,
                                        live_sound_session_id,
                                        accumulated_text.trim(),
                                    );
                                }
                                break;
                            }
                            "error" => {
                                let message = gateway_stream_error_message(&payload);
                                return Err(anyhow!("Vercel AI Gateway transcription error: {}", message));
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if payload.get("setupComplete").is_some()
                        || payload.get("setup_complete").is_some()
                    {
                        google_setup_complete = true;
                    }

                    // Check for error payloads
                    if let Some(err_obj) = payload.get("error") {
                        let message = err_obj
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown Gemini 3.5 Transcribe Live error");
                        return Err(anyhow!("Gemini 3.5 Transcribe Live returned error: {}", message));
                    }

                    // Extract server content / text parts
                    let server_content = payload.get("serverContent").or_else(|| payload.get("server_content"));
                    if let Some(sc) = server_content {
                        let turn_complete = sc
                            .get("turnComplete")
                            .or_else(|| sc.get("turn_complete"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        // 1. Check for Gemini 3.5 dedicated inputTranscription / interimInputTranscription
                        let interim_obj = sc
                            .get("interimInputTranscription")
                            .or_else(|| sc.get("interim_input_transcription"));
                        let final_transcription_obj = sc
                            .get("inputTranscription")
                            .or_else(|| sc.get("input_transcription"));

                        if let Some(interim_val) = interim_obj {
                            let interim_text = interim_val
                                .get("text")
                                .and_then(|t| t.as_str())
                                .or_else(|| interim_val.as_str())
                                .unwrap_or("");
                            if !interim_text.is_empty() {
                                if end_signal_sent {
                                    google_finish_grace.as_mut().reset(
                                        tokio::time::Instant::now()
                                            + Duration::from_millis(
                                                GOOGLE_FINAL_TRANSCRIPT_GRACE_MS,
                                            ),
                                    );
                                }
                                latest_interim = interim_text.to_string();
                                let display_text = join_transcript_text(
                                    &accumulated_text,
                                    interim_text,
                                );
                                emit_live_text(
                                    &app_handle,
                                    &binding_id,
                                    live_sound_session_id,
                                    display_text.trim(),
                                );
                            }
                        }

                        if let Some(final_val) = final_transcription_obj {
                            let fragment = final_val
                                .get("text")
                                .and_then(|t| t.as_str())
                                .or_else(|| final_val.as_str())
                                .unwrap_or("");
                            if !fragment.is_empty() {
                                if end_signal_sent {
                                    google_finish_grace.as_mut().reset(
                                        tokio::time::Instant::now()
                                            + Duration::from_millis(
                                                GOOGLE_FINAL_TRANSCRIPT_GRACE_MS,
                                            ),
                                    );
                                }
                                latest_interim.clear();
                                direct_segment_text.push_str(fragment);
                                if let Some(cb) = &on_final_chunk {
                                    cb(fragment.to_string());
                                }
                                let display_text = join_transcript_text(
                                    &accumulated_text,
                                    &direct_segment_text,
                                );
                                *final_text.lock() = display_text.clone();
                                emit_live_text(
                                    &app_handle,
                                    &binding_id,
                                    live_sound_session_id,
                                    display_text.trim(),
                                );
                            }
                            let segment_finished = final_val
                                .get("finished")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            if segment_finished {
                                let completed = complete_direct_segment(
                                    &mut accumulated_text,
                                    &mut direct_segment_text,
                                    &mut latest_interim,
                                    &final_text,
                                );
                                emit_direct_segment_completion(
                                    on_final_chunk.as_ref(),
                                    completed,
                                );
                            }
                        }

                        if turn_complete {
                            let completed = complete_direct_segment(
                                &mut accumulated_text,
                                &mut direct_segment_text,
                                &mut latest_interim,
                                &final_text,
                            );
                            emit_direct_segment_completion(on_final_chunk.as_ref(), completed);
                        }

                        if end_signal_sent {
                            let interaction_status = sc
                                .get("interactionStatus")
                                .or_else(|| sc.get("interaction_status"))
                                .and_then(Value::as_str);
                            let processing_complete =
                                matches!(interaction_status, Some("IDLE" | "REQUIRES_ACTION"))
                                    || (turn_complete && interaction_status.is_none());
                            if processing_complete {
                                let completed = complete_direct_segment(
                                    &mut accumulated_text,
                                    &mut direct_segment_text,
                                    &mut latest_interim,
                                    &final_text,
                                );
                                emit_direct_segment_completion(
                                    on_final_chunk.as_ref(),
                                    completed,
                                );
                                break;
                            }
                        }
                    }

                    if server_content.is_none() {
                        let top_level_transcription = payload
                            .get("inputTranscription")
                            .or_else(|| payload.get("input_transcription"));
                        if let Some(transcription) = top_level_transcription {
                            let fragment = transcription
                                .get("text")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            if !fragment.is_empty() {
                                if end_signal_sent {
                                    google_finish_grace.as_mut().reset(
                                        tokio::time::Instant::now()
                                            + Duration::from_millis(
                                                GOOGLE_FINAL_TRANSCRIPT_GRACE_MS,
                                            ),
                                    );
                                }
                                latest_interim.clear();
                                direct_segment_text.push_str(fragment);
                                if let Some(callback) = &on_final_chunk {
                                    callback(fragment.to_string());
                                }
                                let display_text = join_transcript_text(
                                    &accumulated_text,
                                    &direct_segment_text,
                                );
                                *final_text.lock() = display_text.clone();
                                emit_live_text(
                                    &app_handle,
                                    &binding_id,
                                    live_sound_session_id,
                                    display_text.trim(),
                                );
                            }
                            if transcription
                                .get("finished")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                            {
                                let completed = complete_direct_segment(
                                    &mut accumulated_text,
                                    &mut direct_segment_text,
                                    &mut latest_interim,
                                    &final_text,
                                );
                                emit_direct_segment_completion(
                                    on_final_chunk.as_ref(),
                                    completed,
                                );
                            }
                        }
                    }
                }
            }
        }

        if transport == GeminiLiveTransport::GoogleDirect {
            let completed = complete_direct_segment(
                &mut accumulated_text,
                &mut direct_segment_text,
                &mut latest_interim,
                &final_text,
            );
            emit_direct_segment_completion(on_final_chunk.as_ref(), completed);
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

        let bytes = frame_16khz_mono_to_pcm_s16le_bytes(&frame_16khz_mono);
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
                    debug!(
                        "Gemini 3.5 Transcribe Live audio queue is full; dropping one audio chunk"
                    );
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

        let wait_ms = timeout_ms.clamp(500, 20_000) as u64;
        let join_result = timeout(Duration::from_millis(wait_ms), &mut join_handle).await;
        match join_result {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => {
                let partial = read_final_text();
                hide_preview(Some(&binding_id));
                warn!(
                    "Gemini 3.5 Transcribe Live ended with a reported runtime error (binding='{}', partial_output={}): {}",
                    binding_id,
                    !partial.is_empty(),
                    error
                );
                return Ok(partial);
            }
            Ok(Err(error)) => {
                let partial = read_final_text();
                hide_preview(Some(&binding_id));
                if !partial.is_empty() {
                    warn!(
                        "Gemini 3.5 Transcribe Live task ended after partial output (binding='{}'): {}",
                        binding_id, error
                    );
                    return Ok(partial);
                }
                return Err(anyhow!(
                    "Gemini 3.5 Transcribe Live session task failed: {}",
                    error
                ));
            }
            Err(_) => {
                join_handle.abort();
                let partial = read_final_text();
                hide_preview(Some(&binding_id));
                if !partial.is_empty() {
                    warn!(
                        "Gemini 3.5 Transcribe Live finalization timed out after partial output (binding='{}', wait={}ms)",
                        binding_id, wait_ms
                    );
                    return Ok(partial);
                }
                return Err(anyhow!(
                    "Timed out while waiting for Gemini 3.5 Transcribe Live transcription to finish"
                ));
            }
        }

        hide_preview(Some(&binding_id));
        Ok(read_final_text())
    }

    pub fn cancel(&self) {
        let session = self.active_session.lock().take();
        if let Some(session) = session {
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
        }

        self.pending_audio.lock().clear();

        if crate::managers::preview_output_mode::is_active() {
            return;
        }
        crate::overlay::end_soniox_live_preview_session();
        crate::overlay::hide_soniox_live_preview_window(&self.app_handle);
    }
}

fn build_live_websocket_request(
    transport: GeminiLiveTransport,
    model: &str,
    api_key: &str,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>> {
    let ws_url = match transport {
        GeminiLiveTransport::GoogleDirect => {
            let mut url = reqwest::Url::parse(GOOGLE_GEMINI_LIVE_WS_BASE_URL)
                .map_err(|error| anyhow!("Invalid Google Gemini 3.5 Transcribe Live WebSocket URL: {}", error))?;
            url.query_pairs_mut().append_pair("key", api_key);
            url.to_string()
        }
        GeminiLiveTransport::VercelGateway => {
            let mut url = reqwest::Url::parse(VERCEL_GEMINI_LIVE_WS_BASE_URL)
                .map_err(|error| anyhow!("Invalid Vercel transcription WebSocket URL: {}", error))?;
            url.query_pairs_mut().append_pair("ai-model-id", model);
            url.to_string()
        }
    };
    let mut request = ws_url
        .into_client_request()
        .map_err(|error| anyhow!("Failed to create Gemini 3.5 Transcribe Live WebSocket request: {}", error))?;

    if transport == GeminiLiveTransport::VercelGateway {
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {}", api_key)
                .parse()
                .map_err(|error| anyhow!("Invalid Vercel auth header: {}", error))?,
        );
        request.headers_mut().insert(
            "ai-gateway-protocol-version",
            VERCEL_GATEWAY_PROTOCOL_VERSION
                .parse()
                .map_err(|error| anyhow!("Invalid Vercel protocol header: {}", error))?,
        );
        request.headers_mut().insert(
            "ai-gateway-auth-method",
            "api-key"
                .parse()
                .map_err(|error| anyhow!("Invalid Vercel auth method header: {}", error))?,
        );
        request.headers_mut().insert(
            "ai-transcription-model-specification-version",
            VERCEL_TRANSCRIPTION_SPECIFICATION_VERSION.parse().map_err(|error| {
                anyhow!("Invalid Vercel transcription protocol header: {}", error)
            })?,
        );
        request.headers_mut().insert(
            "ai-model-id",
            model
                .parse()
                .map_err(|error| anyhow!("Invalid Vercel model header: {}", error))?,
        );
        request.headers_mut().insert(
            "sec-websocket-protocol",
            format!(
                "{}, {}{}",
                VERCEL_TRANSCRIPTION_SUBPROTOCOL, VERCEL_AUTH_SUBPROTOCOL_PREFIX, api_key
            )
            .parse()
            .map_err(|error| anyhow!("Invalid Vercel WebSocket auth protocol: {}", error))?,
        );
    }

    Ok(request)
}

fn join_transcript_text(committed: &str, next: &str) -> String {
    if committed.is_empty() {
        return next.to_string();
    }
    if next.is_empty() {
        return committed.to_string();
    }
    if committed.chars().last().is_some_and(char::is_whitespace)
        || next.chars().next().is_some_and(char::is_whitespace)
    {
        format!("{}{}", committed, next)
    } else {
        format!("{} {}", committed, next)
    }
}

fn complete_direct_segment(
    accumulated_text: &mut String,
    segment_text: &mut String,
    latest_interim: &mut String,
    final_text: &Arc<Mutex<String>>,
) -> Option<CompletedDirectSegment> {
    let was_streamed = !segment_text.is_empty();
    if segment_text.is_empty() && !latest_interim.is_empty() {
        segment_text.push_str(latest_interim);
    }
    let completed_text = segment_text.trim().to_string();
    if !completed_text.is_empty() {
        *accumulated_text = join_transcript_text(accumulated_text, &completed_text);
        *final_text.lock() = accumulated_text.clone();
    }
    segment_text.clear();
    latest_interim.clear();
    (!completed_text.is_empty()).then_some(CompletedDirectSegment {
        text: completed_text,
        was_streamed,
    })
}

fn emit_direct_segment_completion(
    callback: Option<&FinalChunkCallback>,
    completed: Option<CompletedDirectSegment>,
) {
    let (Some(callback), Some(completed)) = (callback, completed) else {
        return;
    };
    if !completed.was_streamed {
        callback(completed.text);
    }
    callback(" ".to_string());
}

fn emit_live_text(
    app_handle: &AppHandle,
    binding_id: &str,
    live_sound_session_id: Option<u64>,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    if binding_id == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
        if let Some(session_id) = live_sound_session_id {
            crate::managers::live_sound_transcription::set_interim_result_if_session_matches(
                app_handle,
                session_id,
                text.to_string(),
                Vec::new(),
            );
        }
        return;
    }

    let _ = app_handle.emit("live-transcript-update", text);
    let _ = app_handle.emit("partial-transcription", text);
    let preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
    crate::overlay::emit_soniox_live_preview_update(app_handle, &preview_final_text, text);
}

fn gateway_stream_error_message(payload: &Value) -> String {
    let error = payload.get("error");
    error
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
        .or_else(|| error.and_then(Value::as_str))
        .or_else(|| payload.get("message").and_then(Value::as_str))
        .unwrap_or("Unknown streaming transcription error")
        .to_string()
}

fn frame_16khz_mono_to_pcm_s16le_bytes(frame: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(frame.len() * 2);
    for &sample in frame {
        let clamped = sample.clamp(-1.0, 1.0);
        let scaled = if clamped < 0.0 {
            clamped * 32768.0
        } else {
            clamped * 32767.0
        };
        let sample_i16 = scaled.round() as i16;
        bytes.extend_from_slice(&sample_i16.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_the_two_supported_live_model_ids() {
        assert!(GeminiRealtimeManager::is_realtime_model(
            GEMINI_LIVE_DEFAULT_MODEL
        ));
        assert!(GeminiRealtimeManager::is_realtime_model(
            GEMINI_LIVE_GOOGLE_DEFAULT_MODEL
        ));
        assert!(!GeminiRealtimeManager::is_realtime_model(
            "google/gemini-3.5-flash"
        ));
        assert!(!GeminiRealtimeManager::is_realtime_model(
            "google/gemini-3.5-transcribe"
        ));
    }

    #[test]
    fn google_setup_uses_live_transcription_camel_case_contract() {
        assert_eq!(
            GeminiRealtimeManager::build_google_setup_payload(
                GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
                Some("ru-RU"),
            ),
            json!({
                "setup": {
                    "model": "models/gemini-3.5-transcribe-live",
                    "inputAudioTranscription": {
                        "languageCodes": ["ru-RU"]
                    }
                }
            })
        );
    }

    #[test]
    fn google_setup_omits_language_codes_for_auto_detection() {
        assert_eq!(
            GeminiRealtimeManager::build_google_setup_payload(
                GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
                None,
            ),
            json!({
                "setup": {
                    "model": "models/gemini-3.5-transcribe-live",
                    "inputAudioTranscription": {}
                }
            })
        );
    }

    #[test]
    fn vercel_start_frame_uses_streaming_transcription_envelope() {
        assert_eq!(
            GeminiRealtimeManager::build_vercel_start_payload(Some("ru-RU")),
            json!({
                "type": "transcription-stream.start",
                "inputAudioFormat": {
                    "type": "audio/pcm",
                    "rate": 16000
                },
                "providerOptions": {
                    "google": {
                        "languageCodes": ["ru-RU"]
                    }
                }
            })
        );
    }

    #[test]
    fn vercel_websocket_handshake_matches_gateway_contract() {
        let request = build_live_websocket_request(
            GeminiLiveTransport::VercelGateway,
            GEMINI_LIVE_DEFAULT_MODEL,
            "test-key",
        )
        .unwrap();

        assert_eq!(
            request.uri().to_string(),
            "wss://ai-gateway.vercel.sh/v4/ai/transcription-model?ai-model-id=google%2Fgemini-3.5-transcribe-live"
        );
        assert_eq!(request.headers()["authorization"], "Bearer test-key");
        assert_eq!(
            request.headers()["ai-gateway-protocol-version"],
            VERCEL_GATEWAY_PROTOCOL_VERSION
        );
        assert_eq!(request.headers()["ai-gateway-auth-method"], "api-key");
        assert_eq!(
            request.headers()["ai-transcription-model-specification-version"],
            VERCEL_TRANSCRIPTION_SPECIFICATION_VERSION
        );
        assert_eq!(request.headers()["ai-model-id"], GEMINI_LIVE_DEFAULT_MODEL);
        assert_eq!(
            request.headers()["sec-websocket-protocol"],
            "ai-gateway-transcription.v1, ai-gateway-auth.test-key"
        );
    }

    #[test]
    fn direct_google_websocket_uses_query_key_without_bearer_header() {
        let request = build_live_websocket_request(
            GeminiLiveTransport::GoogleDirect,
            GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
            "test-key",
        )
        .unwrap();

        assert_eq!(
            request.uri().to_string(),
            "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent?key=test-key"
        );
        assert!(request.headers().get("authorization").is_none());
    }

    #[test]
    fn direct_interim_fallback_is_returned_for_streaming_output() {
        let mut accumulated = String::new();
        let mut segment = String::new();
        let mut interim = "fallback words".to_string();
        let final_text = Arc::new(Mutex::new(String::new()));

        let completed = complete_direct_segment(
            &mut accumulated,
            &mut segment,
            &mut interim,
            &final_text,
        )
        .unwrap();

        assert_eq!(completed.text, "fallback words");
        assert!(!completed.was_streamed);
        assert_eq!(accumulated, "fallback words");
        assert_eq!(*final_text.lock(), "fallback words");
    }
}
