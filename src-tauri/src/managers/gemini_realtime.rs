use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use log::{info, warn};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::url_security::{REMOTE_STT_PRESET_GOOGLE, REMOTE_STT_PRESET_VERCEL};
use crate::settings::GeminiTranscriptionMode;
use super::gemini_output_window::GeminiOutputWindow;

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
pub const GEMINI_LIVE_SAFE_FINALIZE_SECS: u64 = 9 * 60 + 50;
const GOOGLE_FINAL_TRANSCRIPT_GRACE_MS: u64 = 3_000;
const AUDIO_QUEUE_CAPACITY: usize = 256;
const PENDING_AUDIO_OVERFLOW_ERROR: &str =
    "Gemini 3.5 Transcribe Live audio buffer overflowed before connection completed; aborting to avoid losing speech.";
const PENDING_STARTUP_OVERFLOW_ERROR: &str =
    "Gemini 3.5 Transcribe Live audio queue overflowed before session startup; aborting to avoid losing speech.";
const ACTIVE_AUDIO_OVERFLOW_ERROR: &str =
    "Gemini 3.5 Transcribe Live audio queue overflowed; recording aborted to avoid losing speech.";
const GOOGLE_SETUP_TIMEOUT_SECS: u64 = 5;
const GEMINI_LIVE_TIME_LIMIT_COMPLETED_EVENT: &str = "gemini-live-time-limit-completed";

pub type FinalChunkCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;
pub(crate) type FinalChunkCallbackFactory =
    Arc<dyn Fn(Arc<GeminiOutputWindow>) -> FinalChunkCallback + Send + Sync + 'static>;

fn bind_session_chunk_callback(
    factory: Option<&FinalChunkCallbackFactory>,
    window: &Arc<GeminiOutputWindow>,
) -> Option<FinalChunkCallback> {
    let callback = factory?(Arc::clone(window));
    let window = Arc::clone(window);
    Some(Arc::new(move |chunk| {
        if !window.is_cancelled() { callback(chunk); }
    }))
}

#[derive(Clone, Debug)]
pub struct GeminiRealtimeOptions {
    pub model: String,
    pub language: Option<String>,
    pub preset: String,
    pub custom_vocabulary: Vec<String>,
    pub mode: GeminiTranscriptionMode,
    pub validation_error: Option<String>,
}

impl Default for GeminiRealtimeOptions {
    fn default() -> Self {
        Self {
            model: GEMINI_LIVE_DEFAULT_MODEL.to_string(),
            language: None,
            preset: REMOTE_STT_PRESET_VERCEL.to_string(),
            custom_vocabulary: Vec::new(),
            mode: GeminiTranscriptionMode::Smart,
            validation_error: None,
        }
    }
}

#[derive(Debug)]
enum ControlMessage {
    Finish,
    Cancel,
    Overflow(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeminiLiveTransport {
    GoogleDirect,
    VercelGateway,
}

impl GeminiLiveTransport {
    fn debug_label(self) -> &'static str {
        match self {
            Self::GoogleDirect => "google_direct",
            Self::VercelGateway => "vercel_ai_gateway",
        }
    }
}

fn initial_session_limit(
    transport: GeminiLiveTransport,
    ready_session_limit: Duration,
) -> Duration {
    if transport == GeminiLiveTransport::VercelGateway {
        ready_session_limit
    } else {
        Duration::from_secs(GOOGLE_SETUP_TIMEOUT_SECS)
    }
}

fn live_audio_done_payload(transport: GeminiLiveTransport) -> Value {
    match transport {
        GeminiLiveTransport::GoogleDirect => {
            json!({ "realtimeInput": { "audioStreamEnd": true } })
        }
        GeminiLiveTransport::VercelGateway => {
            json!({ "type": "transcription-stream.audio-done" })
        }
    }
}

fn time_limit_completion_payload(
    binding_id: &str,
    transport: GeminiLiveTransport,
    options: &GeminiRealtimeOptions,
    provider_confirmed_finalization: bool,
) -> Value {
    let partial = !provider_confirmed_finalization;
    let route = match transport {
        GeminiLiveTransport::GoogleDirect => "google_direct",
        GeminiLiveTransport::VercelGateway => "vercel_ai_gateway",
    };
    let mode = match options.mode {
        GeminiTranscriptionMode::Smart => "smart",
        GeminiTranscriptionMode::Verbatim => "verbatim",
    };
    json!({
        "bindingId": binding_id,
        "partial": partial,
        "provider": "google",
        "model": options.model,
        "route": route,
        "profileBindingId": binding_id,
        "mode": mode,
        "language": options.language.as_deref().unwrap_or("auto"),
        "vocabularyTermCount": options.custom_vocabulary.len(),
        "endedBy": "provider_time_limit",
        "fullyFinalized": !partial,
        "completionVariant": if partial {
            "partial"
        } else {
            "complete"
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeminiTimeLimitCompletion {
    Complete,
    Partial,
    Failed(String),
}

impl GeminiTimeLimitCompletion {
    fn from_provider_confirmation(provider_confirmed_finalization: bool) -> Self {
        if provider_confirmed_finalization {
            Self::Complete
        } else {
            Self::Partial
        }
    }
}

fn record_time_limit_failure(
    completion: &Arc<Mutex<Option<GeminiTimeLimitCompletion>>>,
    error: &str,
) -> bool {
    let mut completion = completion.lock();
    if completion.is_none() {
        return false;
    }
    *completion = Some(GeminiTimeLimitCompletion::Failed(error.to_string()));
    true
}

fn planned_provider_error_message(
    transport: GeminiLiveTransport,
    payload: &Value,
) -> Option<String> {
    match transport {
        GeminiLiveTransport::VercelGateway
            if payload.get("type").and_then(Value::as_str) == Some("error") =>
        {
            Some(format!(
                "Vercel AI Gateway transcription error after the time-limit end signal: {}",
                gateway_stream_error_message(payload)
            ))
        }
        GeminiLiveTransport::GoogleDirect => payload.get("error").map(|error| {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Unknown Gemini 3.5 Transcribe Live error");
            format!(
                "Gemini 3.5 Transcribe Live returned error after the time-limit end signal: {}",
                message
            )
        }),
        _ => None,
    }
}

fn planned_read_error_message(error: impl std::fmt::Display) -> String {
    format!(
        "Gemini 3.5 Transcribe Live WebSocket read failed after the time-limit end signal: {}",
        error
    )
}

struct CompletedDirectSegment {
    text: String,
    was_streamed: bool,
}

// Keep desktop side effects outside the transport loop so the same loop can be
// exercised with scripted WebSocket frames and a deterministic clock.
struct SessionEvents {
    live_text: Box<dyn Fn(&str) + Send + Sync>,
    setup_complete: Box<dyn Fn() + Send + Sync>,
    time_limit_stop: Box<dyn Fn() + Send + Sync>,
    time_limit_completed: Box<dyn Fn(Value) + Send + Sync>,
    words_pending: Box<dyn Fn(bool) + Send + Sync>,
}

impl SessionEvents {
    fn only_current(self, is_current: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        let is_current = Arc::new(is_current);
        let text_owner = Arc::clone(&is_current);
        let stop_owner = Arc::clone(&is_current);
        Self {
            live_text: Box::new(move |text| {
                if text_owner() { (self.live_text)(text); }
            }),
            setup_complete: self.setup_complete,
            time_limit_stop: Box::new(move || {
                if stop_owner() { (self.time_limit_stop)(); }
            }),
            time_limit_completed: Box::new(move |payload| {
                if is_current() { (self.time_limit_completed)(payload); }
            }),
            words_pending: self.words_pending,
        }
    }
}

struct ActiveSession {
    binding_id: String,
    operation_id: Option<u64>,
    audio_tx: mpsc::Sender<Vec<u8>>,
    control_tx: mpsc::UnboundedSender<ControlMessage>,
    final_text: Arc<Mutex<String>>,
    join_handle: JoinHandle<Result<()>>,
    output_window: Arc<GeminiOutputWindow>,
    restart_params: Option<SessionParams>,
}

struct PendingAudio {
    operation_id: Option<u64>,
    frames: Vec<Vec<u8>>,
    output_window: Arc<GeminiOutputWindow>,
    overflow_error: Option<String>,
}

pub(crate) struct GeminiFinalizingSession {
    app_handle: AppHandle,
    active_session: Arc<Mutex<Option<ActiveSession>>>,
    session: Option<ActiveSession>,
}

#[derive(Clone)]
struct SessionParams {
    binding_id: String,
    operation_id: Option<u64>,
    api_key: String,
    options: GeminiRealtimeOptions,
    chunk_callback_factory: Option<FinalChunkCallbackFactory>,
}

#[derive(Default)]
struct RuntimeErrorReport {
    operation_id: Option<u64>,
    live_sound_session_id: Option<u64>,
    error: Option<String>,
}

impl RuntimeErrorReport {
    fn matches(&self, operation_id: Option<u64>, live_sound_session_id: Option<u64>, error: Option<&str>) -> bool {
        self.operation_id == operation_id && self.live_sound_session_id == live_sound_session_id
            && self.error.as_deref().is_some_and(|reported| error.is_none_or(|error| error == reported))
    }
}

fn report_runtime_error_before_stop(
    report: &Mutex<RuntimeErrorReport>, error: &str, is_current: impl FnOnce() -> bool,
    emit: impl FnOnce(), stop: impl FnOnce(),
) {
    if !is_current() {
        return;
    }
    {
        let mut report = report.lock();
        if report.error.is_some() {
            return;
        }
        report.error = Some(error.to_string());
    }
    // A fast Stop can complete Processing synchronously. Publish feedback before
    // that transition, and retain the marker until this session is replaced.
    emit();
    stop();
}

pub struct GeminiRealtimeManager {
    app_handle: AppHandle,
    active_session: Arc<Mutex<Option<ActiveSession>>>,
    // Always lock active_session before pending_audio, including while publishing
    // a transport, so cancellation cannot mix two operations' buffered frames.
    pending_audio: Mutex<Option<PendingAudio>>,
    time_limit_completion: Mutex<Arc<Mutex<Option<GeminiTimeLimitCompletion>>>>,
    reported_runtime_error: Mutex<Arc<Mutex<RuntimeErrorReport>>>,
}

impl GeminiRealtimeManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle: app_handle.clone(),
            active_session: Arc::new(Mutex::new(None)),
            pending_audio: Mutex::new(Some(PendingAudio {
                operation_id: None,
                frames: Vec::new(),
                output_window: Arc::new(GeminiOutputWindow::default()),
                overflow_error: None,
            })),
            time_limit_completion: Mutex::new(Arc::new(Mutex::new(None))),
            reported_runtime_error: Mutex::new(Arc::new(Mutex::new(RuntimeErrorReport::default()))),
        })
    }

    pub fn is_realtime_model(model: &str) -> bool {
        let model = model.trim();
        model.eq_ignore_ascii_case(GEMINI_LIVE_DEFAULT_MODEL)
            || model.eq_ignore_ascii_case(GEMINI_LIVE_GOOGLE_DEFAULT_MODEL)
    }

    pub(crate) async fn transcribe_history_replay(
        &self,
        samples: &[f32],
        api_key: &str,
        options: GeminiRealtimeOptions,
        replay: &Arc<super::history_replay::HistoryReplay>,
    ) -> Result<String> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("Gemini 3.5 Transcribe Live API key is missing"));
        }
        if let Some(error) = options.validation_error.as_deref() {
            return Err(anyhow!(error.to_string()));
        }
        let transport = match options.preset.as_str() {
            REMOTE_STT_PRESET_GOOGLE => GeminiLiveTransport::GoogleDirect,
            REMOTE_STT_PRESET_VERCEL => GeminiLiveTransport::VercelGateway,
            _ => return Err(anyhow!("Gemini Live requires the Vercel or Google connection route")),
        };
        let expected_model = match transport {
            GeminiLiveTransport::GoogleDirect => GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
            GeminiLiveTransport::VercelGateway => GEMINI_LIVE_DEFAULT_MODEL,
        };
        if !options.model.trim().eq_ignore_ascii_case(expected_model) {
            return Err(anyhow!("This Gemini Live route requires model '{}'", expected_model));
        }
        crate::gemini_config::validate_vocabulary(&options.custom_vocabulary)
            .map_err(|error| anyhow!(error))?;
        if options.language.as_deref().is_some_and(|language| {
            !crate::gemini_config::is_supported_exact_locale(language)
        }) {
            return Err(anyhow!("Gemini Live requires an exact supported language locale"));
        }
        if samples.len() > GEMINI_LIVE_SAFE_FINALIZE_SECS as usize * 16_000 {
            return Err(anyhow!("This recording exceeds the Gemini Live session limit of {} seconds", GEMINI_LIVE_SAFE_FINALIZE_SECS));
        }
        let stream = connect_live_socket(transport, &options.model, api_key, |request| async move {
            connect_async(request).await.map(|(stream, _)| stream)
        }).await?;
        let (write, mut read) = stream.split();
        let mut write = super::history_replay::ReplaySink::new(write, replay, 16_000);
        send_live_setup(&mut write, transport, &options).await?;
        let (audio_tx, audio_rx) = mpsc::channel(1);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        let drive = Self::drive_session_loop(
            &mut write, &mut read, audio_rx, control_rx, Arc::clone(&final_text),
            super::history_replay::HISTORY_REPLAY_BINDING.to_string(),
            Some(Arc::new(|_| {})), transport, options,
            Duration::from_secs(GEMINI_LIVE_SAFE_FINALIZE_SECS),
            Arc::new(Mutex::new(None)),
            SessionEvents {
                live_text: Box::new(|_| {}),
                setup_complete: Box::new(|| {}),
                time_limit_stop: Box::new(|| {}),
                time_limit_completed: Box::new(|_| {}),
                words_pending: Box::new(|_| {}),
            },
        );
        replay.run_audio(
            samples, audio_tx, frame_16khz_mono_to_pcm_s16le_bytes,
            || control_tx.send(ControlMessage::Finish).map_err(|_| anyhow!("Gemini Live session closed")),
            drive, final_text, GEMINI_LIVE_FINALIZE_TIMEOUT_MS,
        ).await
    }

    fn start_error(&self, message: impl Into<String>) -> anyhow::Error {
        let message = message.into();
        crate::managers::remote_stt::record_external_remote_stt_debug(
            &self.app_handle,
            format!("Gemini Live startup failed: {message}"),
            true,
        );
        anyhow!(message)
    }

    pub(crate) fn restart_session(&self, reset_preview: impl FnOnce()) -> Result<()> {
        let params = {
            // Match stop's lock order: recording state, active session, pending audio.
            // Restart is meaningful only while recording, never during finalization.
            let state = self.app_handle.state::<crate::session_manager::ManagedSessionState>();
            let state_guard = crate::session_manager::lock_session_state(&state, "restart Gemini session");
            let mut active = self.active_session.lock();
            let mut pending = self.pending_audio.lock();
            prepare_session_restart(&mut active, &mut pending, |operation_id| {
                operation_id.is_none_or(|id| matches!(
                    &*state_guard,
                    crate::session_manager::SessionState::Recording { operation_id, .. }
                        if *operation_id == id
                ))
            })
        };
        // Old callbacks are invalidated before clearing, and the replacement
        // cannot emit until after the clear. Never hold manager locks for UI work.
        reset_preview();
        if let Some(p) = params {
            // A newer prepare may win after unlocking. start_session checks its
            // pending operation before publishing, so it cannot replace that owner.
            self.start_session_with_chunk_factory(&p.binding_id, p.operation_id, &p.api_key, p.options, p.chunk_callback_factory)?;
        }
        Ok(())
    }

    pub fn start_session(
        &self,
        binding_id: &str,
        operation_id: Option<u64>,
        api_key: &str,
        options: GeminiRealtimeOptions,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()> {
        let factory = on_final_chunk.map(|callback| {
            Arc::new(move |_: Arc<GeminiOutputWindow>| Arc::clone(&callback)) as FinalChunkCallbackFactory
        });
        self.start_session_with_chunk_factory(binding_id, operation_id, api_key, options, factory)
    }

    pub(crate) fn start_session_with_chunk_factory(
        &self,
        binding_id: &str,
        operation_id: Option<u64>,
        api_key: &str,
        options: GeminiRealtimeOptions,
        chunk_callback_factory: Option<FinalChunkCallbackFactory>,
    ) -> Result<()> {
        if api_key.trim().is_empty() {
            return Err(self.start_error("Gemini 3.5 Transcribe Live API key is missing"));
        }
        if let Some(error) = options.validation_error.as_deref() {
            return Err(self.start_error(error));
        }

        let transport = match options.preset.as_str() {
            REMOTE_STT_PRESET_GOOGLE => GeminiLiveTransport::GoogleDirect,
            REMOTE_STT_PRESET_VERCEL => GeminiLiveTransport::VercelGateway,
            _ => {
                return Err(self.start_error(
                    "Gemini 3.5 Transcribe Live requires the Vercel or Google connection route",
                ));
            }
        };
        let expected_model = match transport {
            GeminiLiveTransport::GoogleDirect => GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
            GeminiLiveTransport::VercelGateway => GEMINI_LIVE_DEFAULT_MODEL,
        };
        if !options.model.trim().eq_ignore_ascii_case(expected_model) {
            return Err(self.start_error(format!(
                "This Gemini 3.5 Transcribe Live route requires model '{}', but settings contain '{}'",
                expected_model,
                options.model.trim()
            )));
        }
        if let Err(error) = crate::gemini_config::validate_vocabulary(&options.custom_vocabulary) {
            return Err(self.start_error(error));
        }
        if let Some(language) = options.language.as_deref() {
            if !crate::gemini_config::is_supported_exact_locale(language) {
                return Err(self.start_error(format!(
                    "'{}' is not an exact locale supported by Gemini 3.5 Transcribe Live",
                    language
                )));
            }
        }

        // Publish under the recording owner's lock, in the same state->active->pending
        // order as restart/stop. Stop may win the gap between preparing a restart
        // and calling start_session; a matching pending buffer alone is insufficient.
        let recording_state = self.app_handle.state::<crate::session_manager::ManagedSessionState>();
        let owner_guard = operation_id.map(|_| {
            crate::session_manager::lock_session_state(&recording_state, "publish Gemini session")
        });
        if let Some(expected_id) = operation_id {
            if !owner_guard.as_ref().is_some_and(|state| matches!(
                &**state,
                crate::session_manager::SessionState::Recording { binding_id: owner_binding, operation_id, .. }
                    if owner_binding == binding_id && *operation_id == expected_id
            )) {
                return Err(self.start_error("Gemini recording operation is no longer active"));
            }
        }
        let mut active_session_guard = self.active_session.lock();
        let mut pending_audio = self.pending_audio.lock();
        let pending = take_ready_pending_audio(
            &active_session_guard,
            &mut pending_audio,
            operation_id,
        )
        .map_err(|error| self.start_error(error))?;
        let buffered = pending.frames;
        let output_window = pending.output_window;
        let on_final_chunk = bind_session_chunk_callback(chunk_callback_factory.as_ref(), &output_window);

        let mode = match options.mode {
            GeminiTranscriptionMode::Smart => "smart",
            GeminiTranscriptionMode::Verbatim => "verbatim",
        };
        crate::managers::remote_stt::record_external_remote_stt_debug(
            &self.app_handle,
            format!(
                "Gemini Live session start route={} model={} language={} mode={} vocabulary_terms={} binding={}",
                transport.debug_label(),
                options.model,
                options.language.as_deref().unwrap_or("auto"),
                mode,
                options.custom_vocabulary.len(),
                binding_id,
            ),
            false,
        );

        let restart_params = Some(SessionParams {
            binding_id: binding_id.to_string(),
            operation_id,
            api_key: api_key.to_string(),
            options: options.clone(),
            chunk_callback_factory,
        });

        let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(AUDIO_QUEUE_CAPACITY);
        if transport == GeminiLiveTransport::VercelGateway {
            // The gateway usually sends the first stable text only after Stop.
            output_window.set_words_pending(true);
        }
        let output_window_for_task = Arc::clone(&output_window);
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
        let session_control_tx = control_tx.clone();
        let time_limit_completion = Arc::new(Mutex::new(None));
        *self.time_limit_completion.lock() = Arc::clone(&time_limit_completion);
        let reported_runtime_error = Arc::new(Mutex::new(RuntimeErrorReport {
            operation_id, live_sound_session_id, error: None,
        }));
        *self.reported_runtime_error.lock() = Arc::clone(&reported_runtime_error);

        let join_handle = tauri::async_runtime::spawn(async move {
            let session_started = Instant::now();
            let session_result: Result<()> = async {
                let stream = connect_live_socket(
                    transport,
                    &options.model,
                    &api_key_for_task,
                    |request| async move {
                        connect_async(request).await.map(|(stream, _)| stream)
                    },
                ).await?;

                crate::managers::remote_stt::record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!(
                        "Gemini Live connected route={} binding={}",
                        transport.debug_label(),
                        binding_id_for_task,
                    ),
                    false,
                );

                let (mut write, mut read) = stream.split();

                send_live_setup(&mut write, transport, &options).await?;

                crate::managers::remote_stt::record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!(
                        "Gemini Live setup sent route={} binding={}",
                        transport.debug_label(),
                        binding_id_for_task,
                    ),
                    false,
                );

                Self::run_session_loop(
                    &mut write,
                    &mut read,
                    audio_rx,
                    control_rx,
                    final_text_for_task,
                    app_handle_for_task.clone(),
                    binding_id_for_task.clone(),
                    live_sound_session_id,
                    operation_id,
                    on_final_chunk,
                    transport,
                    options.clone(),
                    Duration::from_secs(GEMINI_LIVE_SAFE_FINALIZE_SECS),
                    Arc::clone(&time_limit_completion),
                    Arc::clone(&output_window_for_task),
                )
                .await
            }
            .await;

            if let Err(err) = &session_result {
                let err_str = err.to_string();
                let debug_error = crate::managers::remote_stt::redact_remote_stt_api_key(
                    &err_str,
                    &api_key_for_task,
                );
                crate::managers::remote_stt::record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!(
                        "Gemini Live runtime failed route={} binding={} elapsed_ms={} error={}",
                        transport.debug_label(),
                        binding_id_for_task,
                        session_started.elapsed().as_millis(),
                        debug_error,
                    ),
                    true,
                );
                record_time_limit_failure(&time_limit_completion, &err_str);
                warn!(
                    "Gemini 3.5 Transcribe Live session runtime error (binding='{}'): {}",
                    binding_id_for_task, err_str
                );
                report_runtime_error_before_stop(
                    &reported_runtime_error, &err_str,
                    || !output_window_for_task.is_cancelled()
                        && session_callback_is_current(&app_handle_for_task, operation_id, live_sound_session_id),
                    || {
                        let _ = app_handle_for_task.emit("remote-stt-error", err_str.clone());
                        if live_sound_session_id.is_none() {
                            crate::plus_overlay_state::handle_transcription_error(&app_handle_for_task, &err_str);
                        }
                        if !output_window_for_task.is_cancelled()
                            && session_callback_is_current(&app_handle_for_task, operation_id, live_sound_session_id)
                            && crate::managers::preview_output_mode::is_active_for_binding(&binding_id_for_task)
                        {
                            crate::managers::preview_output_mode::set_error(&app_handle_for_task, Some(err_str.clone()));
                        }
                    },
                    || {
                        if !output_window_for_task.is_cancelled() && live_sound_session_id.is_none() {
                            crate::actions::stop_transcription_after_realtime_error(
                                &app_handle_for_task, &binding_id_for_task, operation_id,
                            );
                        }
                    },
                );
                if let Some(session_id) = live_sound_session_id {
                    if !output_window_for_task.is_cancelled()
                        && crate::managers::live_sound_transcription::is_session_current(session_id) {
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
                        crate::managers::live_sound_audio::stop_if_session_matches(&app_handle_for_task, session_id);
                    }
                }
            } else {
                crate::managers::remote_stt::record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!(
                        "Gemini Live session ended route={} binding={} elapsed_ms={}",
                        transport.debug_label(),
                        binding_id_for_task,
                        session_started.elapsed().as_millis(),
                    ),
                    false,
                );
            }

            session_result
        });

        let active = ActiveSession {
            binding_id: binding_id.to_string(),
            operation_id,
            audio_tx,
            control_tx,
            final_text,
            join_handle,
            output_window,
            restart_params,
        };
        *active_session_guard = Some(active);

        // Flush this operation's pre-start audio before live callbacks can send
        // newer frames or another operation can replace the pending buffer.
        flush_pending_frames_to_active(buffered, &session_audio_tx, &session_control_tx);
        drop(pending_audio);
        drop(active_session_guard);
        // Window operations may dispatch to the UI thread; never hold session state there.
        drop(owner_guard);

        if binding_id != crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID
            && session_callback_is_current(&self.app_handle, operation_id, None)
        {
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

    pub fn runtime_error_was_reported(
        &self, operation_id: Option<u64>, live_sound_session_id: Option<u64>, error: Option<&str>,
    ) -> bool {
        let reported = Arc::clone(&self.reported_runtime_error.lock());
        let matches = reported.lock().matches(operation_id, live_sound_session_id, error);
        matches
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

    fn build_google_setup_payload(model: &str, options: &GeminiRealtimeOptions) -> Value {
        let mut input_audio_transcription = serde_json::Map::new();
        if let Some(value) = options.language.as_deref() {
            input_audio_transcription.insert("languageCodes".to_string(), json!([value]));
        }
        if !options.custom_vocabulary.is_empty() {
            input_audio_transcription.insert("customVocabulary".to_string(), json!(options.custom_vocabulary));
        }
        input_audio_transcription.insert(
            "mode".to_string(),
            json!(match options.mode {
                GeminiTranscriptionMode::Smart => "SMART",
                GeminiTranscriptionMode::Verbatim => "VERBATIM",
            }),
        );
        json!({
            "setup": {
                "model": Self::normalize_gemini_model(model),
                "generationConfig": {
                    "responseModalities": ["TEXT"]
                },
                "inputAudioTranscription": Value::Object(input_audio_transcription)
            }
        })
    }

    fn build_vercel_start_payload(options: &GeminiRealtimeOptions) -> Value {
        let mut payload = json!({
            "type": "transcription-stream.start",
            "inputAudioFormat": {
                "type": "audio/pcm",
                "rate": 16000
            }
        });
        let mut google_options = serde_json::Map::new();
        if let Some(language) = options.language.as_deref() {
            google_options.insert("languageCodes".to_string(), json!([language]));
        }
        if !options.custom_vocabulary.is_empty() {
            google_options.insert("customVocabulary".to_string(), json!(options.custom_vocabulary));
        }
        google_options.insert(
            "mode".to_string(),
            json!(match options.mode {
                GeminiTranscriptionMode::Smart => "SMART",
                GeminiTranscriptionMode::Verbatim => "VERBATIM",
            }),
        );
        payload["providerOptions"] = json!({ "google": Value::Object(google_options) });
        // Google's generationComplete marks a speech boundary even when the
        // gateway keeps the same synthetic segment ID until the session ends.
        payload["includeRawChunks"] = json!(true);
        payload
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_session_loop<S, R>(
        write: &mut S,
        read: &mut R,
        audio_rx: mpsc::Receiver<Vec<u8>>,
        control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        final_text: Arc<Mutex<String>>,
        app_handle: AppHandle,
        binding_id: String,
        live_sound_session_id: Option<u64>,
        operation_id: Option<u64>,
        on_final_chunk: Option<FinalChunkCallback>,
        transport: GeminiLiveTransport,
        completion_options: GeminiRealtimeOptions,
        time_limit_duration: Duration,
        time_limit_completion: Arc<Mutex<Option<GeminiTimeLimitCompletion>>>,
        output_window: Arc<GeminiOutputWindow>,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let app_for_text = app_handle.clone();
        let binding_for_text = binding_id.clone();
        let app_for_setup = app_handle.clone();
        let binding_for_setup = binding_id.clone();
        let app_for_stop = app_handle.clone();
        let binding_for_stop = binding_id.clone();
        let app_for_owner = app_handle.clone();
        let window_for_owner = Arc::clone(&output_window);
        let window_for_stop = Arc::clone(&output_window);
        let events = SessionEvents {
            live_text: Box::new(move |text| {
                emit_live_text(&app_for_text, &binding_for_text, live_sound_session_id, text);
            }),
            setup_complete: Box::new(move || {
                crate::managers::remote_stt::record_external_remote_stt_debug(
                    &app_for_setup,
                    format!("Gemini Live setup confirmed route={} binding={}",
                        transport.debug_label(), binding_for_setup),
                    false,
                );
            }),
            time_limit_stop: Box::new(move || {
                let app_for_stop = app_for_stop.clone();
                let binding_for_stop = binding_for_stop.clone();
                let window_for_stop = Arc::clone(&window_for_stop);
                tauri::async_runtime::spawn(async move {
                    if window_for_stop.is_cancelled() || !session_callback_is_current(&app_for_stop, operation_id, live_sound_session_id) {
                        return;
                    }
                    if let Some(session_id) = live_sound_session_id {
                        crate::managers::live_sound_audio::stop_if_session_matches(&app_for_stop, session_id);
                    } else {
                        crate::actions::stop_transcription_at_realtime_limit(
                            &app_for_stop, &binding_for_stop, operation_id,
                        );
                    }
                });
            }),
            time_limit_completed: Box::new(move |payload| {
                let _ = app_handle.emit(GEMINI_LIVE_TIME_LIMIT_COMPLETED_EVENT, payload);
            }),
            words_pending: Box::new(move |pending| output_window.set_words_pending(pending)),
        }.only_current(move || {
            !window_for_owner.is_cancelled()
                && session_callback_is_current(&app_for_owner, operation_id, live_sound_session_id)
        });
        Self::drive_session_loop(
            write, read, audio_rx, control_rx, final_text, binding_id,
            on_final_chunk, transport, completion_options, time_limit_duration,
            time_limit_completion, events,
        ).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn drive_session_loop<S, R>(
        write: &mut S,
        read: &mut R,
        mut audio_rx: mpsc::Receiver<Vec<u8>>,
        mut control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        final_text: Arc<Mutex<String>>,
        binding_id: String,
        on_final_chunk: Option<FinalChunkCallback>,
        transport: GeminiLiveTransport,
        completion_options: GeminiRealtimeOptions,
        time_limit_duration: Duration,
        time_limit_completion: Arc<Mutex<Option<GeminiTimeLimitCompletion>>>,
        events: SessionEvents,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let mut audio_input_closed = false;
        let mut end_signal_sent = false;
        let mut google_setup_complete = transport == GeminiLiveTransport::VercelGateway;
        let mut accumulated_text = String::new();
        #[cfg(test)]
        let mut sent_pcm_bytes = 0usize;
        let mut gateway_has_raw_transcription = false;
        let mut gateway_segment_has_delta = false;
        let mut gateway_output_ends_with_whitespace = false;
        let mut gateway_segment_start = 0;
        let mut gateway_turn_boundary = false;
        let mut gateway_boundary_after_delta = false;
        let mut direct_segment_text = String::new();
        let mut latest_interim = String::new();
        let mut time_limit_finalizing = false;
        let mut provider_confirmed_finalization = false;
        let mut time_limit_failure = None;
        let session_limit = tokio::time::sleep(initial_session_limit(
            transport,
            time_limit_duration,
        ));
        tokio::pin!(session_limit);
        let google_finish_grace =
            tokio::time::sleep(Duration::from_secs(GEMINI_LIVE_SAFE_FINALIZE_SECS));
        tokio::pin!(google_finish_grace);

        loop {
            if let Some(control) = take_queued_control(&mut control_rx) {
                match control {
                    ControlMessage::Finish => {}
                    ControlMessage::Cancel => {
                        let _ = write.close().await;
                        return Ok(());
                    }
                    ControlMessage::Overflow(message) => {
                        record_time_limit_failure(&time_limit_completion, &message);
                        let _ = write.close().await;
                        return Err(anyhow!(message));
                    }
                }
            }
            tokio::select! {
                _ = &mut session_limit, if !time_limit_finalizing => {
                    if let Some(control) = take_queued_control(&mut control_rx) {
                        match control {
                            ControlMessage::Finish => {}
                            ControlMessage::Cancel => {
                                let _ = write.close().await;
                                return Ok(());
                            }
                            ControlMessage::Overflow(message) => {
                                record_time_limit_failure(&time_limit_completion, &message);
                                let _ = write.close().await;
                                return Err(anyhow!(message));
                            }
                        }
                    }
                    if transport == GeminiLiveTransport::GoogleDirect && !google_setup_complete {
                        return Err(anyhow!(
                            "Gemini 3.5 Transcribe Live did not confirm setup within {} seconds",
                            GOOGLE_SETUP_TIMEOUT_SECS
                        ));
                    }
                    time_limit_finalizing = true;
                    audio_input_closed = true;
                    let end_payload = live_audio_done_payload(transport);
                    write.send(Message::Text(end_payload.to_string().into())).await
                        .map_err(|e| anyhow!("Failed to finalize Gemini Live at the safe time limit: {}", e))?;
                    end_signal_sent = true;
                    *time_limit_completion.lock() = Some(GeminiTimeLimitCompletion::Partial);
                    (events.time_limit_stop)();
                    if transport == GeminiLiveTransport::GoogleDirect {
                        google_finish_grace.as_mut().reset(
                            tokio::time::Instant::now() + Duration::from_millis(GOOGLE_FINAL_TRANSCRIPT_GRACE_MS),
                        );
                    }
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
                        ControlMessage::Overflow(message) => {
                            record_time_limit_failure(&time_limit_completion, &message);
                            let _ = write.close().await;
                            return Err(anyhow!(message));
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
                            #[cfg(test)]
                            { sent_pcm_bytes += audio_chunk.len(); }
                        }
                        Some(_) => {}
                        None => {
                            if let Some(control) = take_queued_control(&mut control_rx) {
                                match control {
                                    ControlMessage::Finish => {}
                                    ControlMessage::Cancel => {
                                        let _ = write.close().await;
                                        return Ok(());
                                    }
                                    ControlMessage::Overflow(message) => {
                                        record_time_limit_failure(&time_limit_completion, &message);
                                        let _ = write.close().await;
                                        return Err(anyhow!(message));
                                    }
                                }
                            }
                            audio_input_closed = true;
                            let end_payload = live_audio_done_payload(transport);
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
                        Some(Err(error)) if time_limit_finalizing => {
                            time_limit_failure = Some(anyhow!(planned_read_error_message(error)));
                            break;
                        }
                        Some(frame) => frame.map_err(|e| anyhow!("Gemini 3.5 Transcribe Live WebSocket read failed: {}", e))?,
                        None => {
                            if time_limit_finalizing {
                                break;
                            }
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
                            if time_limit_finalizing {
                                break;
                            }
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

                    #[cfg(test)]
                    if std::env::var_os("AIVORELAY_STT_REPLAY_TRACE").is_some()
                    {
                        // Explicit paid-replay diagnostics: only transcript fields,
                        // never headers, credentials, or arbitrary provider errors.
                        println!("GEMINI_RX {}", json!({
                            "type": payload.get("type"),
                            "id": payload.get("id"),
                            "text": payload.get("text"),
                            "delta": payload.get("delta"),
                            "segments": payload.get("segments"),
                            "warnings": payload.get("warnings"),
                            "serverContent": payload.get("serverContent").or_else(|| payload.pointer("/rawValue/serverContent")),
                            "fields": payload.as_object().map(|object| object.keys().collect::<Vec<_>>()),
                        }));
                    }

                    if time_limit_finalizing {
                        if let Some(message) = planned_provider_error_message(transport, &payload) {
                            time_limit_failure = Some(anyhow!(message));
                            break;
                        }
                    }

                    if transport == GeminiLiveTransport::VercelGateway {
                        let part_type = payload.get("type").and_then(Value::as_str).unwrap_or("");
                        match part_type {
                            "transcript-delta" => {
                                if let Some(delta) = payload.get("delta").and_then(Value::as_str) {
                                    if delta.is_empty() {
                                        if std::mem::take(&mut gateway_boundary_after_delta) {
                                            gateway_turn_boundary = true;
                                        }
                                        continue;
                                    }
                                    if !gateway_segment_has_delta {
                                        gateway_segment_start = accumulated_text.len();
                                    }
                                    let delta = if gateway_has_raw_transcription {
                                        // Raw events identify turns; punctuation inside a
                                        // turn may be part of an abbreviation or token.
                                        delta.to_string()
                                    } else {
                                        normalize_gateway_delta_boundary(&accumulated_text, delta)
                                    };
                                    let delta = if gateway_turn_boundary {
                                        gateway_turn_boundary = delta.chars().all(gateway_attached_punctuation);
                                        gateway_turn_delta(&accumulated_text, &delta)
                                    } else {
                                        delta
                                    };
                                    accumulated_text.push_str(&delta);
                                    gateway_output_ends_with_whitespace = delta.chars().last().is_some_and(char::is_whitespace);
                                    gateway_segment_has_delta = true;
                                    if std::mem::take(&mut gateway_boundary_after_delta) {
                                        gateway_turn_boundary = true;
                                    }
                                    store_gateway_authoritative_text(
                                        &accumulated_text,
                                        &final_text,
                                    );
                                    if let Some(callback) = &on_final_chunk {
                                        callback(delta);
                                    }
                                    (events.live_text)(accumulated_text.trim());
                                    (events.words_pending)(false);
                                }
                            }
                            "transcript-partial" => {
                                if let Some(partial) = payload.get("text").and_then(Value::as_str) {
                                    if !partial.trim().is_empty() {
                                        (events.words_pending)(true);
                                    }
                                    let display = preserve_gateway_partial_fallback(
                                        &accumulated_text,
                                        partial,
                                        &final_text,
                                    );
                                    (events.live_text)(display.trim());
                                }
                            }
                            "raw" => {
                                if let Some(content) = payload.pointer("/rawValue/serverContent") {
                                    if content.pointer("/inputTranscription/text").and_then(Value::as_str).is_some() {
                                        gateway_has_raw_transcription = true;
                                    }
                                    let completed = ["generationComplete", "turnComplete"].iter()
                                        .any(|key| content.get(key).and_then(Value::as_bool) == Some(true))
                                        || content.pointer("/inputTranscription/finished")
                                            .and_then(Value::as_bool) == Some(true);
                                    if completed {
                                        if content.pointer("/inputTranscription/text")
                                            .and_then(Value::as_str).is_some_and(|text| !text.trim().is_empty())
                                        {
                                            // The mapped delta for this same raw frame follows it.
                                            gateway_boundary_after_delta = true;
                                        } else {
                                            gateway_turn_boundary = true;
                                        }
                                    }
                                }
                            }
                            "transcript-final" => {
                                if let Some(text) = payload.get("text").and_then(Value::as_str).filter(|text| !text.trim().is_empty()) {
                                    if gateway_segment_has_delta {
                                        reconcile_gateway_authoritative_text(
                                            &mut accumulated_text, gateway_segment_start, text,
                                            on_final_chunk.as_ref(), &mut gateway_output_ends_with_whitespace,
                                        );
                                    } else {
                                        let previous_len = accumulated_text.len();
                                        accumulated_text =
                                            join_transcript_text(&accumulated_text, text);
                                        if let Some(callback) = &on_final_chunk {
                                            let delta = &accumulated_text[previous_len..];
                                            if !delta.is_empty() {
                                                gateway_output_ends_with_whitespace = delta.chars().last().is_some_and(char::is_whitespace);
                                                callback(delta.to_string());
                                            }
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
                                    if let Some(callback) = &on_final_chunk {
                                        if !gateway_output_ends_with_whitespace {
                                            callback(" ".to_string());
                                        }
                                    }
                                    gateway_output_ends_with_whitespace = true;
                                }
                                store_gateway_authoritative_text(
                                    &accumulated_text,
                                    &final_text,
                                );
                                gateway_segment_has_delta = false;
                                gateway_boundary_after_delta = false;
                                gateway_turn_boundary = true;
                                (events.live_text)(accumulated_text.trim());
                                (events.words_pending)(false);
                            }
                            "finish" => {
                                if let Some(text) = payload.get("text").and_then(Value::as_str) {
                                    reconcile_gateway_authoritative_text(
                                        &mut accumulated_text, 0, text, on_final_chunk.as_ref(),
                                        &mut gateway_output_ends_with_whitespace,
                                    );
                                    store_gateway_authoritative_text(
                                        &accumulated_text,
                                        &final_text,
                                    );
                                    (events.live_text)(accumulated_text.trim());
                                }
                                provider_confirmed_finalization = true;
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

                    if !google_setup_complete
                        && (payload.get("setupComplete").is_some()
                            || payload.get("setup_complete").is_some())
                    {
                        google_setup_complete = true;
                        (events.setup_complete)();
                        session_limit.as_mut().reset(
                            tokio::time::Instant::now() + time_limit_duration,
                        );
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
                                (events.live_text)(display_text.trim());
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
                                (events.live_text)(display_text.trim());
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
                                provider_confirmed_finalization = true;
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
                                (events.live_text)(display_text.trim());
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

        if let Some(control) = take_queued_control(&mut control_rx) {
            match control {
                ControlMessage::Finish => {}
                ControlMessage::Cancel => {
                    let _ = write.close().await;
                    return Ok(());
                }
                ControlMessage::Overflow(message) => {
                    record_time_limit_failure(&time_limit_completion, &message);
                    let _ = write.close().await;
                    return Err(anyhow!(message));
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

        if let Some(error) = time_limit_failure {
            record_time_limit_failure(&time_limit_completion, &error.to_string());
            let _ = write.close().await;
            return Err(error);
        }

        if time_limit_finalizing {
            let partial = !provider_confirmed_finalization;
            *time_limit_completion.lock() = Some(
                GeminiTimeLimitCompletion::from_provider_confirmation(
                    provider_confirmed_finalization,
                ),
            );
            let completion_payload = time_limit_completion_payload(
                &binding_id,
                transport,
                &completion_options,
                provider_confirmed_finalization,
            );
            info!(
                "Gemini Live time-limit completion binding={} model={} route={} mode={} language={} vocabulary_terms={} partial={}",
                binding_id,
                completion_options.model,
                completion_payload["route"].as_str().unwrap_or("unknown"),
                completion_payload["mode"].as_str().unwrap_or("unknown"),
                completion_options.language.as_deref().unwrap_or("auto"),
                completion_options.custom_vocabulary.len(),
                partial,
            );
            (events.time_limit_completed)(completion_payload);
        }

        #[cfg(test)]
        if std::env::var_os("AIVORELAY_STT_REPLAY_TRACE").is_some() {
            println!("GEMINI_AUDIO_SENT bytes={sent_pcm_bytes} samples={}", sent_pcm_bytes / 2);
        }
        let _ = write.close().await;
        Ok(())
    }

    pub(crate) fn take_time_limit_completion(&self) -> Option<GeminiTimeLimitCompletion> {
        let completion = Arc::clone(&self.time_limit_completion.lock());
        let result = completion.lock().take();
        result
    }

    pub fn push_audio_frame(&self, frame_16khz_mono: Vec<f32>) {
        self.push_audio_frame_for_operation(None, frame_16khz_mono);
    }

    pub fn push_audio_frame_for_operation(
        &self,
        operation_id: Option<u64>,
        frame_16khz_mono: Vec<f32>,
    ) {
        if frame_16khz_mono.is_empty() {
            return;
        }
        let bytes = frame_16khz_mono_to_pcm_s16le_bytes(&frame_16khz_mono);
        push_pcm_audio_for_operation(
            &self.active_session,
            &self.pending_audio,
            operation_id,
            bytes,
        );
    }

    /// Detaches the active transport synchronously so a new recording can open
    /// its own Gemini session while this one finishes in the background.
    pub(crate) fn begin_finish_session_if_matches(
        &self,
        operation_id: Option<u64>,
    ) -> GeminiFinalizingSession {
        let mut active = self.active_session.lock();
        let mut pending = self.pending_audio.lock();
        let session = detach_session_if_matches(&mut active, &mut pending, operation_id);
        GeminiFinalizingSession {
            app_handle: self.app_handle.clone(),
            active_session: Arc::clone(&self.active_session),
            session,
        }
    }

    pub async fn finish_session(&self, timeout_ms: u32) -> Result<String> {
        // Dedicated Live Monitor managers do not use recording operation IDs.
        self.finish_session_if_matches(None, timeout_ms).await
    }

    pub async fn finish_session_if_matches(
        &self,
        operation_id: Option<u64>,
        timeout_ms: u32,
    ) -> Result<String> {
        self.begin_finish_session_if_matches(operation_id)
            .finish(timeout_ms)
            .await
    }

    pub fn prepare_session(&self, operation_id: Option<u64>) {
        let mut active = self.active_session.lock();
        if let Some(session) = active.take() {
            session.output_window.cancel();
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
        }
        let mut pending = self.pending_audio.lock();
        if let Some(previous) = pending.as_ref() { previous.output_window.cancel(); }
        *pending = Some(PendingAudio {
            operation_id,
            frames: Vec::new(),
            output_window: Arc::new(GeminiOutputWindow::default()),
            overflow_error: None,
        });
    }

    pub(crate) fn output_window(&self, operation_id: u64) -> Option<Arc<GeminiOutputWindow>> {
        let active = self.active_session.lock();
        if let Some(session) = active.as_ref().filter(|s| s.operation_id == Some(operation_id)) {
            return Some(Arc::clone(&session.output_window));
        }
        self.pending_audio.lock().as_ref()
            .filter(|pending| pending.operation_id == Some(operation_id))
            .map(|pending| Arc::clone(&pending.output_window))
    }

    pub fn cancel_if_matches(&self, operation_id: Option<u64>) {
        let session = {
            let mut active = self.active_session.lock();
            let mut pending = self.pending_audio.lock();
            if let Some(previous) = pending.as_ref().filter(|p| p.operation_id == operation_id) {
                previous.output_window.cancel();
            }
            if let Some(previous) = active.as_ref().filter(|s| s.operation_id == operation_id) {
                previous.output_window.cancel();
            }
            detach_session_if_matches(&mut active, &mut pending, operation_id)
        };
        if let Some(session) = session {
            session.output_window.cancel();
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
        }
        if operation_id.is_some_and(|id| {
            crate::session_manager::is_operation_current(&self.app_handle, id)
        }) && !crate::managers::preview_output_mode::is_active()
            && self.active_session.lock().is_none()
        {
            crate::overlay::end_soniox_live_preview_session();
            crate::overlay::hide_soniox_live_preview_window(&self.app_handle);
        }
    }

    pub fn cancel(&self) {
        let mut active = self.active_session.lock();
        let session = active.take();
        if let Some(session) = session {
            session.output_window.cancel();
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
        }

        if let Some(pending) = self.pending_audio.lock().take() {
            pending.output_window.cancel();
        }
        drop(active);

        if crate::managers::preview_output_mode::is_active() {
            return;
        }
        crate::overlay::end_soniox_live_preview_session();
        crate::overlay::hide_soniox_live_preview_window(&self.app_handle);
    }
}

impl GeminiFinalizingSession {
    pub(crate) async fn finish_early(self, deadline: Instant) -> Result<String> {
        let Some(session) = self.session else { return Ok(String::new()); };
        let operation_id = session.operation_id;
        let is_live_monitor = session.binding_id == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID;
        let result = finish_early_session(session, deadline).await;
        if !is_live_monitor && preview_cleanup_has_owner(&self.app_handle, operation_id)
            && self.active_session.lock().is_none()
            && !crate::managers::preview_output_mode::is_active()
        {
            crate::overlay::end_soniox_live_preview_session();
            crate::overlay::hide_soniox_live_preview_window(&self.app_handle);
        }
        result
    }

    pub(crate) async fn finish(self, timeout_ms: u32) -> Result<String> {
        let GeminiFinalizingSession {
            app_handle,
            active_session,
            session,
        } = self;
        let operation_id = session.as_ref().and_then(|session| session.operation_id);
        let hide_preview = |binding_id: Option<&str>| {
            if binding_id == Some(crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID) {
                return;
            }
            if crate::managers::preview_output_mode::is_active() {
                return;
            }
            if !preview_cleanup_has_owner(&app_handle, operation_id)
                || active_session.lock().is_some()
            {
                return;
            }
            crate::overlay::end_soniox_live_preview_session();
            crate::overlay::hide_soniox_live_preview_window(&app_handle);
        };

        let Some(session) = session else {
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
                if !partial.is_empty() {
                    return Ok(partial);
                }
                return Err(error);
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
}

async fn connect_live_socket<C, F, T>(
    transport: GeminiLiveTransport,
    model: &str,
    api_key: &str,
    connect: C,
) -> Result<T>
where
    C: FnOnce(tokio_tungstenite::tungstenite::http::Request<()>) -> F,
    F: std::future::Future<Output = Result<T, tokio_tungstenite::tungstenite::Error>>,
{
    let request = build_live_websocket_request(transport, model, api_key)?;
    timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS), connect(request))
        .await
        .map_err(|_| anyhow!("Timed out while connecting to Gemini 3.5 Transcribe Live"))?
        .map_err(|e| anyhow!("Failed to connect to Gemini 3.5 Transcribe Live: {}", e))
}

async fn send_live_setup<S>(
    write: &mut S,
    transport: GeminiLiveTransport,
    options: &GeminiRealtimeOptions,
) -> Result<()>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let payload = match transport {
        GeminiLiveTransport::GoogleDirect => {
            GeminiRealtimeManager::build_google_setup_payload(&options.model, options)
        }
        GeminiLiveTransport::VercelGateway => {
            GeminiRealtimeManager::build_vercel_start_payload(options)
        }
    };
    write.send(Message::Text(payload.to_string().into())).await
        .map_err(|e| anyhow!("Failed to send Gemini 3.5 Transcribe Live setup message: {}", e))
}

fn take_queued_control(
    control_rx: &mut mpsc::UnboundedReceiver<ControlMessage>,
) -> Option<ControlMessage> {
    while let Ok(control) = control_rx.try_recv() {
        match control {
            ControlMessage::Finish => {}
            other @ (ControlMessage::Cancel | ControlMessage::Overflow(_)) => {
                return Some(other);
            }
        }
    }
    None
}

fn push_pcm_audio_for_operation(
    active_session: &Mutex<Option<ActiveSession>>,
    pending_audio: &Mutex<Option<PendingAudio>>,
    operation_id: Option<u64>,
    bytes: Vec<u8>,
) {
    if bytes.is_empty() {
        return;
    }
    let active_handles = {
        let active = active_session.lock();
        if let Some(session) = active.as_ref().filter(|s| s.operation_id == operation_id) {
            Some((session.audio_tx.clone(), session.control_tx.clone()))
        } else {
            let mut pending = pending_audio.lock();
            if let Some(pending) = pending
                .as_mut()
                .filter(|pending| pending.operation_id == operation_id)
            {
                if pending.overflow_error.is_none() {
                    if pending.frames.len() >= AUDIO_QUEUE_CAPACITY {
                        pending.overflow_error = Some(PENDING_AUDIO_OVERFLOW_ERROR.to_string());
                    } else {
                        pending.frames.push(bytes);
                    }
                }
            }
            return;
        }
    };

    if let Some((sender, control_tx)) = active_handles {
        if let Err(mpsc::error::TrySendError::Full(_)) = sender.try_send(bytes) {
            warn!("Gemini 3.5 Transcribe Live audio queue overflowed; aborting session to avoid dropping speech");
            let _ = control_tx.send(ControlMessage::Overflow(
                ACTIVE_AUDIO_OVERFLOW_ERROR.to_string(),
            ));
        }
    }
}

fn take_ready_pending_audio(
    active_session: &Option<ActiveSession>,
    pending_audio: &mut Option<PendingAudio>,
    operation_id: Option<u64>,
) -> std::result::Result<PendingAudio, String> {
    let Some(pending) = pending_audio.as_ref() else {
        return Err("Gemini recording operation is no longer active".to_string());
    };
    if pending.operation_id != operation_id {
        return Err("Gemini recording operation is no longer active".to_string());
    }
    if let Some(overflow_error) = pending.overflow_error.as_deref() {
        return Err(overflow_error.to_string());
    }
    if pending.frames.len() > AUDIO_QUEUE_CAPACITY {
        return Err(PENDING_STARTUP_OVERFLOW_ERROR.to_string());
    }
    if active_session.is_some() {
        return Err(
            "Gemini 3.5 Transcribe Live session is already active for this profile".to_string(),
        );
    }
    Ok(pending_audio.take().unwrap())
}

fn flush_pending_frames_to_active(
    buffered: Vec<Vec<u8>>,
    audio_tx: &mpsc::Sender<Vec<u8>>,
    control_tx: &mpsc::UnboundedSender<ControlMessage>,
) {
    for chunk in buffered {
        if chunk.is_empty() {
            continue;
        }
        if let Err(err) = audio_tx.try_send(chunk) {
            if matches!(err, mpsc::error::TrySendError::Full(_)) {
                warn!(
                    "Gemini 3.5 Transcribe Live pre-start audio queue overflowed; aborting session to avoid dropping speech"
                );
                let _ = control_tx.send(ControlMessage::Overflow(
                    ACTIVE_AUDIO_OVERFLOW_ERROR.to_string(),
                ));
            }
            break;
        }
    }
}

fn session_callback_is_current(
    app: &AppHandle,
    operation_id: Option<u64>,
    live_sound_session_id: Option<u64>,
) -> bool {
    live_sound_session_id
        .map(crate::managers::live_sound_transcription::is_session_current)
        .or_else(|| operation_id.map(|id| crate::session_manager::is_operation_current(app, id)))
        .unwrap_or(true)
}

fn preview_cleanup_has_owner(app: &AppHandle, operation_id: Option<u64>) -> bool {
    operation_id.is_none_or(|id| {
        operation_owns_preview(
            crate::session_manager::is_operation_current(app, id),
            crate::session_manager::is_idle(app),
            crate::session_manager::is_latest_operation(id),
        )
    })
}

fn operation_owns_preview(current: bool, idle: bool, latest: bool) -> bool {
    current || (idle && latest)
}

fn prepare_session_restart(
    active: &mut Option<ActiveSession>,
    pending: &mut Option<PendingAudio>,
    is_current: impl FnOnce(Option<u64>) -> bool,
) -> Option<SessionParams> {
    let session = active.as_ref()?;
    let params = session.restart_params.clone()?;
    if params.operation_id != session.operation_id
        || pending.as_ref().is_some_and(|pending| pending.operation_id != session.operation_id)
        || !is_current(session.operation_id)
    {
        return None;
    }
    let session = active.take().unwrap();
    session.output_window.cancel();
    let _ = session.control_tx.send(ControlMessage::Cancel);
    session.join_handle.abort();
    *pending = Some(PendingAudio {
        operation_id: params.operation_id,
        frames: Vec::new(),
        output_window: Arc::new(GeminiOutputWindow::default()),
        overflow_error: None,
    });
    Some(params)
}

fn detach_session_if_matches(
    active: &mut Option<ActiveSession>,
    pending: &mut Option<PendingAudio>,
    operation_id: Option<u64>,
) -> Option<ActiveSession> {
    let session = if active.as_ref().is_some_and(|session| session.operation_id == operation_id) {
        active.take()
    } else {
        None
    };
    if pending.as_ref().is_some_and(|pending| pending.operation_id == operation_id) {
        *pending = None;
    }
    session
}

async fn finish_early_session(session: ActiveSession, deadline: Instant) -> Result<String> {
    let ActiveSession { audio_tx, control_tx, final_text, mut join_handle, output_window, .. } = session;
    let _ = control_tx.send(ControlMessage::Finish);
    drop(audio_tx);
    let deadline = tokio::time::Instant::from_std(deadline);
    // Past the deadline, wait only while Gemini still owes stable text for shown words.
    let mut result = loop {
        let closes_at = output_window.closes_at().map_or(deadline, tokio::time::Instant::from_std);
        if tokio::time::Instant::now() >= closes_at {
            break None;
        }
        tokio::select! {
            result = &mut join_handle => break Some(result),
            _ = tokio::time::sleep_until(closes_at) => {}
            _ = output_window.settled() => {}
        }
    };
    if result.is_none() {
        join_handle.abort();
        // Wait for callbacks already running before scheduling the final UI write.
        let joined = join_handle.await;
        // abort() cannot interrupt synchronous error feedback already executing.
        // Preserve that result instead of reporting a second, empty transcript.
        if !matches!(&joined, Err(tauri::Error::JoinError(error)) if error.is_cancelled()) {
            result = Some(joined);
        }
    } else if !matches!(&result, Some(Ok(Err(_))) | Some(Err(_))) {
        // Keep the insertion time for success, but surface failures immediately.
        tokio::time::sleep_until(deadline).await;
    }
    let text = final_text.lock().trim().to_string();
    info!(
        "Gemini early finalization completed (partial_text={}, waited_past_deadline_ms={})",
        !text.is_empty(),
        tokio::time::Instant::now().saturating_duration_since(deadline).as_millis(),
    );
    if text.is_empty() {
        match result {
            Some(Ok(Err(error))) => return Err(error),
            Some(Err(error)) => return Err(anyhow!("Gemini early finalization task failed: {}", error)),
            _ => {}
        }
    }
    Ok(text)
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

fn normalize_gateway_delta_boundary(committed: &str, delta: &str) -> String {
    let Some(previous) = committed.chars().last() else {
        return delta.to_string();
    };
    let Some(next) = delta.chars().next() else {
        return String::new();
    };
    if previous.is_whitespace() || next.is_whitespace() {
        return delta.to_string();
    }

    let sentence_finished = matches!(previous, '.' | '!' | '?' | '…' | '。' | '！' | '？');
    let sentence_started = next.is_alphabetic() || matches!(next, '"' | '“' | '‘' | '«' | '(');
    if sentence_finished && sentence_started {
        format!(" {}", delta)
    } else {
        delta.to_string()
    }
}

fn preserve_gateway_partial_fallback(
    accumulated_text: &str,
    partial: &str,
    final_text: &Arc<Mutex<String>>,
) -> String {
    let display = join_transcript_text(accumulated_text, partial);
    *final_text.lock() = display.clone();
    display
}

fn gateway_attached_punctuation(ch: char) -> bool {
    matches!(ch, '.' | ',' | '!' | '?' | ';' | ':' | '\'' | '’' | ')' | ']' | '}' | '”' | '»' | '。' | '，' | '！' | '？')
}

fn gateway_turn_delta(committed: &str, delta: &str) -> String {
    let attached = delta.chars().next().is_some_and(gateway_attached_punctuation);
    if attached {
        delta.to_string()
    } else {
        join_transcript_text(committed, delta)[committed.len()..].to_string()
    }
}

/// The gateway can omit spaces at confirmed speech boundaries. Preserve
/// streamed separators and any additional word spaces in the final result.
fn gateway_finish_text(accumulated_text: &str, finish_text: &str) -> String {
    let words_only = |text: &str| text.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    if !words_only(finish_text).starts_with(&words_only(accumulated_text)) {
        return finish_text.trim_end().to_string();
    }
    let mut streamed = accumulated_text.chars().peekable();
    let mut result = String::new();
    for ch in finish_text.chars() {
        if !ch.is_whitespace() {
            let mut had_separator = false;
            while streamed.peek().is_some_and(|ch| ch.is_whitespace()) {
                had_separator = true;
                streamed.next();
            }
            if had_separator && result.chars().last().is_some_and(|last| !last.is_whitespace())
            {
                result.push(' ');
            }
            if let Some(streamed_ch) = streamed.next() {
                debug_assert_eq!(streamed_ch, ch);
            }
        }
        result.push(ch);
    }
    result.trim_end().to_string()
}

/// Find only text beyond the streamed character prefix. Whitespace can differ
/// between streamed and final forms; inserted words must still match exactly.
fn gateway_unstreamed_suffix<'a>(streamed: &str, authoritative: &'a str) -> Option<&'a str> {
    let mut final_chars = authoritative.char_indices().filter(|(_, ch)| !ch.is_whitespace());
    let mut consumed = 0;
    for ch in streamed.chars().filter(|ch| !ch.is_whitespace()) {
        let (offset, final_ch) = final_chars.next()?;
        if ch != final_ch {
            return None;
        }
        consumed = offset + final_ch.len_utf8();
    }
    Some(&authoritative[consumed..])
}

fn reconcile_gateway_authoritative_text(
    accumulated_text: &mut String,
    segment_start: usize,
    text: &str,
    on_final_chunk: Option<&FinalChunkCallback>,
    output_ends_with_whitespace: &mut bool,
) {
    if text.trim().is_empty() {
        return;
    }
    let streamed = &accumulated_text[segment_start..];
    let authoritative = gateway_finish_text(streamed, text);
    // Only a matching character prefix can be appended safely. A revision must
    // update History/preview without duplicating or rewriting pasted words.
    if let Some(suffix) = gateway_unstreamed_suffix(streamed, &authoritative) {
        let suffix = if streamed.chars().last().is_some_and(char::is_whitespace) {
            suffix.trim_start_matches(char::is_whitespace)
        } else {
            suffix
        };
        if !suffix.is_empty() {
            *output_ends_with_whitespace = suffix.chars().last().is_some_and(char::is_whitespace);
            if let Some(callback) = on_final_chunk {
                callback(suffix.to_string());
            }
        }
    } else if authoritative.trim_end() != streamed.trim_end() {
        warn!("Gemini revised already streamed text; preserving its authoritative final transcript");
    }
    accumulated_text.truncate(segment_start);
    accumulated_text.push_str(&authoritative);
}

fn store_gateway_authoritative_text(accumulated_text: &str, final_text: &Arc<Mutex<String>>) {
    *final_text.lock() = accumulated_text.to_string();
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
        let options = GeminiRealtimeOptions {
            language: Some("ru-RU".to_string()),
            custom_vocabulary: vec!["Gemini".to_string(), "BigQuery".to_string()],
            mode: GeminiTranscriptionMode::Verbatim,
            ..Default::default()
        };
        assert_eq!(
            GeminiRealtimeManager::build_google_setup_payload(
                GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
                &options,
            ),
            json!({
                "setup": {
                    "model": "models/gemini-3.5-transcribe-live",
                    "generationConfig": {
                        "responseModalities": ["TEXT"]
                    },
                    "inputAudioTranscription": {
                        "languageCodes": ["ru-RU"],
                        "customVocabulary": ["Gemini", "BigQuery"],
                        "mode": "VERBATIM"
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
                &GeminiRealtimeOptions::default(),
            ),
            json!({
                    "setup": {
                        "model": "models/gemini-3.5-transcribe-live",
                        "generationConfig": {
                            "responseModalities": ["TEXT"]
                        },
                        "inputAudioTranscription": {
                            "mode": "SMART"
                    }
                }
            })
        );
    }

    #[test]
    fn vercel_start_frame_uses_streaming_transcription_envelope() {
        let options = GeminiRealtimeOptions {
            language: Some("ru-RU".to_string()),
            custom_vocabulary: vec!["Gemini".to_string()],
            mode: GeminiTranscriptionMode::Verbatim,
            ..Default::default()
        };
        assert_eq!(
            GeminiRealtimeManager::build_vercel_start_payload(&options),
            json!({
                "type": "transcription-stream.start",
                "includeRawChunks": true,
                "inputAudioFormat": {
                    "type": "audio/pcm",
                    "rate": 16000
                },
                "providerOptions": {
                    "google": {
                        "languageCodes": ["ru-RU"],
                        "customVocabulary": ["Gemini"],
                        "mode": "VERBATIM"
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

    #[test]
    fn vercel_partial_is_preserved_until_authoritative_text_replaces_it() {
        let final_text = Arc::new(Mutex::new("committed".to_string()));

        let display = preserve_gateway_partial_fallback(
            "committed",
            "latest hypothesis",
            &final_text,
        );
        assert_eq!(display, "committed latest hypothesis");
        assert_eq!(*final_text.lock(), "committed latest hypothesis");

        let display = preserve_gateway_partial_fallback(
            "committed",
            "newest hypothesis",
            &final_text,
        );
        assert_eq!(display, "committed newest hypothesis");
        assert_eq!(*final_text.lock(), "committed newest hypothesis");

        store_gateway_authoritative_text("committed authoritative delta", &final_text);
        assert_eq!(*final_text.lock(), "committed authoritative delta");
    }

    #[test]
    fn vercel_final_without_text_discards_a_stale_partial_fallback() {
        let accumulated_text = "committed authoritative text".to_string();
        let final_text = Arc::new(Mutex::new(String::new()));
        preserve_gateway_partial_fallback(
            &accumulated_text,
            "stale hypothesis",
            &final_text,
        );

        store_gateway_authoritative_text(&accumulated_text, &final_text);

        assert_eq!(*final_text.lock(), "committed authoritative text");
    }

    #[test]
    fn deadline_waits_for_google_setup_acknowledgement() {
        let short_test_limit = Duration::from_millis(25);
        assert_eq!(
            initial_session_limit(GeminiLiveTransport::VercelGateway, short_test_limit),
            short_test_limit
        );
        assert_eq!(
            initial_session_limit(GeminiLiveTransport::GoogleDirect, short_test_limit),
            Duration::from_secs(GOOGLE_SETUP_TIMEOUT_SECS)
        );
    }

    #[test]
    fn time_limit_uses_each_transport_normal_finalization_signal() {
        assert_eq!(
            live_audio_done_payload(GeminiLiveTransport::GoogleDirect),
            json!({ "realtimeInput": { "audioStreamEnd": true } })
        );
        assert_eq!(
            live_audio_done_payload(GeminiLiveTransport::VercelGateway),
            json!({ "type": "transcription-stream.audio-done" })
        );
    }

    #[test]
    fn time_limit_completion_is_informational_and_classifies_full_or_partial() {
        let options = GeminiRealtimeOptions {
            model: GEMINI_LIVE_DEFAULT_MODEL.to_string(),
            language: Some("ru-RU".to_string()),
            custom_vocabulary: vec!["private term".to_string()],
            mode: GeminiTranscriptionMode::Verbatim,
            ..Default::default()
        };
        let complete = time_limit_completion_payload(
            "transcribe_profile",
            GeminiLiveTransport::VercelGateway,
            &options,
            true,
        );
        assert_eq!(GEMINI_LIVE_TIME_LIMIT_COMPLETED_EVENT, "gemini-live-time-limit-completed");
        assert_eq!(complete["partial"], false);
        assert_eq!(complete["fullyFinalized"], true);
        assert_eq!(complete["endedBy"], "provider_time_limit");
        assert_eq!(complete["provider"], "google");
        assert_eq!(complete["route"], "vercel_ai_gateway");
        assert_eq!(complete["vocabularyTermCount"], 1);
        assert_eq!(complete["completionVariant"], "complete");
        assert!(complete.get("error").is_none());
        assert!(!complete.to_string().contains("private term"));

        let partial = time_limit_completion_payload(
            "transcribe_profile",
            GeminiLiveTransport::GoogleDirect,
            &options,
            false,
        );
        assert_eq!(partial["partial"], true);
        assert_eq!(partial["fullyFinalized"], false);
        assert_eq!(partial["completionVariant"], "partial");
        assert!(partial.get("message").is_none());
    }

    #[test]
    fn failed_planned_finalization_cannot_be_classified_as_completion() {
        let errors = [
            planned_provider_error_message(
                GeminiLiveTransport::GoogleDirect,
                &json!({ "error": { "message": "Google failed" } }),
            )
            .unwrap(),
            planned_provider_error_message(
                GeminiLiveTransport::VercelGateway,
                &json!({ "type": "error", "error": { "message": "Vercel failed" } }),
            )
            .unwrap(),
            planned_read_error_message("socket reset"),
        ];

        for error in errors {
            let completion = Arc::new(Mutex::new(Some(GeminiTimeLimitCompletion::Partial)));
            assert!(record_time_limit_failure(&completion, &error));
            assert_eq!(
                completion.lock().as_ref(),
                Some(&GeminiTimeLimitCompletion::Failed(error))
            );
            assert!(matches!(
                completion.lock().as_ref(),
                Some(GeminiTimeLimitCompletion::Failed(_))
            ));
        }

        let manual_stop = Arc::new(Mutex::new(None));
        assert!(!record_time_limit_failure(&manual_stop, "manual read failure"));
        assert_eq!(*manual_stop.lock(), None);
        assert_eq!(
            planned_provider_error_message(
                GeminiLiveTransport::GoogleDirect,
                &json!({ "serverContent": {} }),
            ),
            None
        );
    }

    #[test]
    fn direct_error_flush_preserves_accumulated_and_interim_text() {
        let mut accumulated = "stable words".to_string();
        let mut segment = String::new();
        let mut interim = "latest partial".to_string();
        let final_text = Arc::new(Mutex::new(accumulated.clone()));

        let completed = complete_direct_segment(
            &mut accumulated,
            &mut segment,
            &mut interim,
            &final_text,
        )
        .unwrap();

        assert_eq!(completed.text, "latest partial");
        assert_eq!(accumulated, "stable words latest partial");
        assert_eq!(*final_text.lock(), "stable words latest partial");
    }

    #[test]
    fn finalized_direct_text_does_not_duplicate_its_interim_hypothesis() {
        let mut accumulated = "first segment".to_string();
        let mut segment = "final words".to_string();
        let mut interim = "final words".to_string();
        let final_text = Arc::new(Mutex::new(String::new()));

        let completed = complete_direct_segment(
            &mut accumulated,
            &mut segment,
            &mut interim,
            &final_text,
        )
        .unwrap();

        assert_eq!(accumulated, "first segment final words");
        assert_eq!(*final_text.lock(), "first segment final words");
        assert!(completed.was_streamed);
    }
}

#[cfg(test)]
#[path = "gemini_realtime_network_tests.rs"]
mod network_tests;
