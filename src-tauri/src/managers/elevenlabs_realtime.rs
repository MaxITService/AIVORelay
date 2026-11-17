use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::elevenlabs_language::resolve_requested_language_for_elevenlabs;
use crate::managers::provider_error::parse_provider_error_value;
use crate::managers::remote_stt::{redact_remote_stt_api_key, record_external_remote_stt_debug};
use crate::url_security::{validate_remote_stt_endpoint_url, REMOTE_STT_ELEVENLABS_BASE_URL};

pub const ELEVENLABS_SCRIBE_V3_LIVE_MODEL: &str = "scribe_v3_live";
pub const ELEVENLABS_LIVE_FINALIZE_TIMEOUT_MS: u32 = 8_000;

const ELEVENLABS_LOOP_FINALIZE_TIMEOUT_MS: u64 =
    ELEVENLABS_LIVE_FINALIZE_TIMEOUT_MS.saturating_sub(500) as u64;
const ELEVENLABS_REALTIME_WS_SUFFIX: &str = "/speech-to-text/realtime";
const ELEVENLABS_SAMPLE_RATE_HZ: u32 = 16_000;
const ELEVENLABS_PCM_BYTES_PER_SAMPLE: usize = 2;
const ELEVENLABS_PCM_BYTES_PER_MS: usize =
    (ELEVENLABS_SAMPLE_RATE_HZ as usize * ELEVENLABS_PCM_BYTES_PER_SAMPLE) / 1_000;
const ELEVENLABS_AUDIO_FORMAT: &str = "pcm_16000";
const ELEVENLABS_COMMIT_STRATEGY: &str = "manual";
const SOFT_COMMIT_MIN_AUDIO_MS: usize = 2_200;
const SOFT_COMMIT_MIN_PCM_BYTES: usize = SOFT_COMMIT_MIN_AUDIO_MS * ELEVENLABS_PCM_BYTES_PER_MS;
const TURN_COMMIT_THRESHOLD: f64 = 0.9;
const HARD_COMMIT_AUDIO_MS: usize = 22_000;
const HARD_COMMIT_PCM_BYTES: usize = HARD_COMMIT_AUDIO_MS * ELEVENLABS_PCM_BYTES_PER_MS;
const STREAMING_COMMIT_ACK_TIMEOUT_MS: u64 = ELEVENLABS_LOOP_FINALIZE_TIMEOUT_MS;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const SOCKET_CLOSE_TIMEOUT: Duration = Duration::from_millis(200);
const MAX_AUDIO_MESSAGE_PCM_BYTES: usize = ELEVENLABS_PCM_BYTES_PER_MS * 1_000;
const AUDIO_QUEUE_CAPACITY: usize = 512;
const CANCELLED_OPERATIONS_CAPACITY: usize = 64;

const MAX_KEYTERM_LIBRARY_TERMS: usize = 10_000;
const MAX_KEYTERM_CHARS: usize = 50;
const MAX_KEYTERM_LIBRARY_JSON_BYTES: usize = 1_900_000;

static XI_API_KEY_HEADER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(xi-api-key\s*[:=]\s*)([^\s,;]+)").expect("valid regex"));
static BEARER_TOKEN_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(authorization\s*[:=]\s*(?:bearer|apikey)\s+|bearer\s+)([^\s,;]+)").expect("valid regex"));
static ELEVENLABS_TOKEN_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:sutkn|sk)_[A-Za-z0-9_-]+\b").expect("valid regex"));
static QUERY_TOKEN_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([?&](?:token|api_key|key)=)([^&\s]+)").expect("valid regex"));

pub type FinalChunkCallback = Arc<dyn Fn(String) + Send + Sync>;
pub(crate) type FinalChunkCallbackFactory =
    Arc<dyn Fn(Arc<SessionLease>) -> FinalChunkCallback + Send + Sync>;

/// Clear can replace a transport without changing the recording ID.
#[derive(Default)]
pub(crate) struct SessionLease(AtomicBool);

impl SessionLease {
    fn cancel(&self) { self.0.store(true, Ordering::Release); }
    pub(crate) fn allows_delivery(&self) -> bool { !self.0.load(Ordering::Acquire) }
}

fn bind_session_chunk_callback(
    factory: Option<&FinalChunkCallbackFactory>, lease: &Arc<SessionLease>,
) -> Option<FinalChunkCallback> {
    let callback = factory?(Arc::clone(lease));
    let lease = Arc::clone(lease);
    Some(Arc::new(move |chunk| {
        if lease.allows_delivery() { callback(chunk); }
    }))
}
pub type ElevenLabsLivePacketCallback = Arc<dyn Fn(ElevenLabsLivePacket) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct ElevenLabsRealtimeOptions {
    pub base_url: String,
    pub model_id: String,
    pub language_code: Option<String>,
    pub diarize: bool,
    pub keyterm_library: Vec<String>,
    pub show_preview: bool,
    /// Sends `no_verbatim=true`: drops filler words, false starts and non-speech sounds.
    pub no_verbatim: bool,
    /// Sends `enable_logging=false` (zero retention). ElevenLabs accepts it for Enterprise plans only.
    pub zero_retention: bool,
}

impl Default for ElevenLabsRealtimeOptions {
    fn default() -> Self {
        Self {
            base_url: REMOTE_STT_ELEVENLABS_BASE_URL.to_string(),
            model_id: ELEVENLABS_SCRIBE_V3_LIVE_MODEL.to_string(),
            language_code: None,
            diarize: false,
            keyterm_library: Vec::new(),
            show_preview: false,
            no_verbatim: false,
            zero_retention: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElevenLabsLiveWord {
    pub text: String,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub speaker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElevenLabsLivePacket {
    Partial {
        text: String,
        words: Vec<ElevenLabsLiveWord>,
    },
    Committed {
        text: String,
        segment_index: Option<u64>,
    },
    Diarized {
        segment_index: Option<u64>,
        text: String,
        words: Vec<ElevenLabsLiveWord>,
    },
    SpeakersMerged {
        from_speaker_id: String,
        to_speaker_id: String,
    },
}

#[derive(Debug)]
enum ControlMessage {
    Finish,
    Cancel,
    Overflow(String),
}

struct SessionEvents {
    on_partial: Box<dyn Fn(&str, Vec<ElevenLabsLiveWord>) + Send + Sync>,
    on_committed: Box<dyn Fn(&str, &str, &str, Option<u64>) + Send + Sync>,
    on_diarized: Box<dyn Fn(Option<u64>, &str, Vec<ElevenLabsLiveWord>) + Send + Sync>,
    on_speakers_merged: Box<dyn Fn(&str, &str) + Send + Sync>,
}

impl SessionEvents {
    fn only_current(self, is_current: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        let current = Arc::new(is_current);
        let partial = Arc::clone(&current);
        let committed = Arc::clone(&current);
        let diarized = Arc::clone(&current);
        Self {
            on_partial: Box::new(move |text, words| { if partial() { (self.on_partial)(text, words); } }),
            on_committed: Box::new(move |delta, text, all, idx| { if committed() { (self.on_committed)(delta, text, all, idx); } }),
            on_diarized: Box::new(move |idx, text, words| { if diarized() { (self.on_diarized)(idx, text, words); } }),
            on_speakers_merged: Box::new(move |from, to| { if current() { (self.on_speakers_merged)(from, to); } }),
        }
    }
}

struct ActiveSession {
    binding_id: String,
    operation_id: Option<u64>,
    show_preview: bool,
    audio_tx: mpsc::Sender<Vec<u8>>,
    control_tx: mpsc::UnboundedSender<ControlMessage>,
    final_text: Arc<Mutex<String>>,
    join_handle: JoinHandle<Result<()>>,
    lease: Arc<SessionLease>,
    restart_params: Option<SessionParams>,
}

struct PendingAudio {
    operation_id: Option<u64>,
    frames: Vec<Vec<u8>>,
    overflow_error: Option<String>,
}

#[derive(Default)]
struct SessionCoordinator {
    active_session: Option<ActiveSession>,
    pending_audio: Option<PendingAudio>,
    cancelled_operations: VecDeque<u64>,
}

pub(crate) struct ElevenLabsFinalizingSession {
    app_handle: AppHandle,
    session: Option<ActiveSession>,
}

#[derive(Clone)]
struct SessionParams {
    binding_id: String,
    operation_id: Option<u64>,
    api_key: String,
    options: ElevenLabsRealtimeOptions,
    chunk_callback_factory: Option<FinalChunkCallbackFactory>,
    on_packet: Option<ElevenLabsLivePacketCallback>,
}

pub struct ElevenLabsRealtimeManager {
    app_handle: AppHandle,
    coordinator: Arc<Mutex<SessionCoordinator>>,
    reported_runtime_error: Mutex<Arc<Mutex<Option<String>>>>,
}

impl ElevenLabsRealtimeManager {
    pub fn new(app: &AppHandle) -> Self {
        Self {
            app_handle: app.clone(),
            coordinator: Arc::new(Mutex::new(SessionCoordinator::default())),
            reported_runtime_error: Mutex::new(Arc::new(Mutex::new(None))),
        }
    }

    pub fn is_realtime_model(model_id: &str) -> bool {
        model_id
            .trim()
            .eq_ignore_ascii_case(ELEVENLABS_SCRIBE_V3_LIVE_MODEL)
    }

    fn start_error(&self, message: impl Into<String>) -> anyhow::Error {
        let message = message.into();
        record_external_remote_stt_debug(
            &self.app_handle,
            format!("ElevenLabs live startup failed: {message}"),
            true,
        );
        anyhow!(message)
    }

    pub fn prepare_session(&self, operation_id: Option<u64>) {
        let old_session = {
            let mut coordinator = self.coordinator.lock();
            if let Some(op_id) = operation_id {
                coordinator.cancelled_operations.retain(|&id| id != op_id);
            }
            coordinator.pending_audio = Some(PendingAudio {
                operation_id,
                frames: Vec::new(),
                overflow_error: None,
            });
            coordinator.active_session.take()
        };
        if let Some(session) = old_session {
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.lease.cancel();
            session.join_handle.abort();
            hide_preview_if_needed(&self.app_handle, session.show_preview, Some(&session.binding_id));
        }
    }

    pub fn restart_session(&self, reset_preview: impl FnOnce()) -> Result<()> {
        let params = {
            // Match start/stop's recording-state -> coordinator lock order.
            let state = self.app_handle.state::<crate::session_manager::ManagedSessionState>();
            let owner = crate::session_manager::lock_session_state(&state, "restart ElevenLabs session");
            let mut coordinator = self.coordinator.lock();
            prepare_session_restart(&mut coordinator, |id, binding| {
                id.is_some_and(|id| matches!(&*owner,
                    crate::session_manager::SessionState::Recording { operation_id, binding_id, .. }
                        if *operation_id == id && binding_id == binding))
            })
        };
        reset_preview();
        if let Some(p) = params {
            self.start_session_with_chunk_factory(
                &p.binding_id, p.operation_id, &p.api_key, p.options,
                p.chunk_callback_factory, p.on_packet,
            )?;
        }
        Ok(())
    }

    pub fn has_active_session(&self) -> bool {
        self.coordinator.lock().active_session.is_some()
    }

    pub fn clear_reported_runtime_error(&self) {
        let reported = Arc::clone(&self.reported_runtime_error.lock());
        *reported.lock() = None;
    }

    pub fn take_reported_runtime_error(&self, candidate: &str) -> bool {
        let reported = Arc::clone(&self.reported_runtime_error.lock());
        let mut reported = reported.lock();
        if reported.as_deref() != Some(candidate) {
            return false;
        }
        *reported = None;
        true
    }

    pub fn start_session(
        &self,
        binding_id: &str,
        operation_id: Option<u64>,
        api_key: &str,
        options: ElevenLabsRealtimeOptions,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()> {
        self.start_session_with_packet_callback(
            binding_id,
            operation_id,
            api_key,
            options,
            on_final_chunk,
            None,
        )
    }

    pub fn start_session_with_packet_callback(
        &self,
        binding_id: &str,
        operation_id: Option<u64>,
        api_key: &str,
        options: ElevenLabsRealtimeOptions,
        on_final_chunk: Option<FinalChunkCallback>,
        on_packet: Option<ElevenLabsLivePacketCallback>,
    ) -> Result<()> {
        let factory = on_final_chunk.map(|callback| {
            Arc::new(move |_lease| Arc::clone(&callback)) as FinalChunkCallbackFactory
        });
        self.start_session_with_chunk_factory(
            binding_id, operation_id, api_key, options, factory, on_packet,
        )
    }

    pub(crate) fn start_session_with_chunk_factory(
        &self, binding_id: &str, operation_id: Option<u64>, api_key: &str,
        options: ElevenLabsRealtimeOptions,
        chunk_callback_factory: Option<FinalChunkCallbackFactory>,
        on_packet: Option<ElevenLabsLivePacketCallback>,
    ) -> Result<()> {
        validate_options(api_key, &options)
            .map_err(|err| self.start_error(redact_elevenlabs_secrets(&err.to_string(), api_key)))?;

        let ws_request = build_elevenlabs_realtime_ws_request(api_key, &options)
            .map_err(|err| self.start_error(redact_elevenlabs_secrets(&err.to_string(), api_key)))?;

        let state = self.app_handle.state::<crate::session_manager::ManagedSessionState>();
        let owner = operation_id.map(|_| crate::session_manager::lock_session_state(&state, "publish ElevenLabs session"));
        if let Some(id) = operation_id {
            if !owner.as_ref().is_some_and(|owner| matches!(&**owner,
                crate::session_manager::SessionState::Recording { operation_id, binding_id: binding, .. }
                    if *operation_id == id && binding == binding_id))
            {
                return Err(self.start_error("ElevenLabs recording operation is no longer active"));
            }
        }
        let mut coordinator = self.coordinator.lock();
        if let Some(op_id) = operation_id {
            if coordinator.cancelled_operations.contains(&op_id) {
                return Err(self.start_error("ElevenLabs recording operation was cancelled"));
            }
        }
        if coordinator.active_session.is_some() {
            return Err(self.start_error(
                "ElevenLabs live session is already active for this profile",
            ));
        }

        let buffered = if let Some(pending) = coordinator.pending_audio.as_ref() {
            if pending.operation_id != operation_id {
                return Err(self.start_error("ElevenLabs recording operation is no longer active"));
            }
            if let Some(overflow_error) = pending.overflow_error.as_deref() {
                return Err(self.start_error(overflow_error));
            }
            coordinator
                .pending_audio
                .take()
                .map(|p| p.frames)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if buffered.len() > AUDIO_QUEUE_CAPACITY {
            return Err(self.start_error(
                "ElevenLabs live audio queue overflowed before session startup; aborting to avoid losing speech.",
            ));
        }

        let language_resolution =
            resolve_requested_language_for_elevenlabs(options.language_code.as_deref());
        record_external_remote_stt_debug(
            &self.app_handle,
            format!(
                "ElevenLabs live session start model={} language={} diarize={} keyterms={} binding={}",
                ELEVENLABS_SCRIBE_V3_LIVE_MODEL,
                language_resolution.code.as_deref().unwrap_or("auto"),
                options.diarize,
                options.keyterm_library.len(),
                binding_id,
            ),
            false,
        );

        let lease = Arc::new(SessionLease::default());
        let on_final_chunk = bind_session_chunk_callback(chunk_callback_factory.as_ref(), &lease);
        let restart_params = Some(SessionParams {
            binding_id: binding_id.to_string(), operation_id, api_key: api_key.to_string(),
            options: options.clone(), chunk_callback_factory, on_packet: on_packet.clone(),
        });

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
        let show_preview = options.show_preview;
        let diarize = options.diarize;
        let sanitized_keyterms = sanitize_keyterm_library(&options.keyterm_library);
        let session_audio_tx = audio_tx.clone();
        let session_control_tx = control_tx.clone();
        let reported_runtime_error = Arc::new(Mutex::new(None));
        *self.reported_runtime_error.lock() = Arc::clone(&reported_runtime_error);

        let lease_for_task = Arc::clone(&lease);
        let join_handle = tauri::async_runtime::spawn(async move {
            let session_started = Instant::now();
            let session_result: Result<()> = async {
                let (stream, _) = timeout(
                    Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
                    connect_async(ws_request),
                )
                .await
                .map_err(|_| anyhow!("Timed out while connecting to ElevenLabs WebSocket"))?
                .map_err(|error| anyhow!(format_handshake_error(error, &api_key_for_task)))?;

                record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!("ElevenLabs live connected binding={}", binding_id_for_task),
                    false,
                );

                let (mut write, mut read) = stream.split();
                let events = build_runtime_session_events(
                    app_handle_for_task.clone(),
                    binding_id_for_task.clone(),
                    show_preview,
                    on_packet,
                ).only_current({
                    let app = app_handle_for_task.clone();
                    let lease = Arc::clone(&lease_for_task);
                    move || lease.allows_delivery() && session_callback_is_current(&app, operation_id, live_sound_session_id)
                });

                Self::drive_session_loop(
                    &mut write,
                    &mut read,
                    audio_rx,
                    control_rx,
                    final_text_for_task,
                    on_final_chunk,
                    api_key_for_task.clone(),
                    diarize,
                    sanitized_keyterms,
                    events,
                )
                .await
            }
            .await;

            if let Err(err) = &session_result {
                let err_str = redact_elevenlabs_secrets(&err.to_string(), &api_key_for_task);
                record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!(
                        "ElevenLabs live runtime failed binding={} elapsed_ms={} error={}",
                        binding_id_for_task,
                        session_started.elapsed().as_millis(),
                        err_str,
                    ),
                    true,
                );
                warn!(
                    "ElevenLabs live session runtime error (binding='{}'): {}",
                    binding_id_for_task, err_str
                );
                let callback_is_current = lease_for_task.allows_delivery()
                    && session_callback_is_current(&app_handle_for_task, operation_id, live_sound_session_id);
                if callback_is_current {
                    *reported_runtime_error.lock() = Some(err_str.clone());
                }
                if callback_is_current {
                    let _ = app_handle_for_task.emit("remote-stt-error", err_str.clone());
                    if live_sound_session_id.is_none() {
                        crate::plus_overlay_state::handle_transcription_error(
                            &app_handle_for_task,
                            &err_str,
                        );
                    }
                }
                if callback_is_current
                    && crate::managers::preview_output_mode::is_active_for_binding(&binding_id_for_task)
                {
                    crate::managers::preview_output_mode::set_error(
                        &app_handle_for_task,
                        Some(err_str.clone()),
                    );
                }
                if callback_is_current && live_sound_session_id.is_none() {
                    crate::actions::stop_transcription_after_realtime_error(
                        &app_handle_for_task,
                        &binding_id_for_task,
                        operation_id,
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
                        crate::managers::live_sound_audio::stop_if_session_matches(
                            &app_handle_for_task, session_id,
                        );
                    }
                }
            } else {
                record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!(
                        "ElevenLabs live session ended binding={} elapsed_ms={}",
                        binding_id_for_task,
                        session_started.elapsed().as_millis(),
                    ),
                    false,
                );
            }

            session_result
        });

        coordinator.active_session = Some(ActiveSession {
            binding_id: binding_id.to_string(),
            operation_id,
            show_preview,
            audio_tx,
            control_tx,
            final_text,
            join_handle,
            lease,
            restart_params,
        });

        for chunk in buffered {
            if let Err(err) = session_audio_tx.try_send(chunk) {
                if matches!(err, mpsc::error::TrySendError::Full(_)) {
                    let _ = session_control_tx.send(ControlMessage::Overflow(
                        "ElevenLabs live audio queue overflowed; aborting to avoid losing speech."
                            .to_string(),
                    ));
                }
                break;
            }
        }
        drop(coordinator);
        drop(owner);

        if show_preview
            && binding_id != crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID
            && binding_id != super::history_replay::HISTORY_REPLAY_BINDING
            && session_callback_is_current(&self.app_handle, operation_id, live_sound_session_id)
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
            "Started ElevenLabs live session for binding '{}'",
            binding_id
        );
        Ok(())
    }

    pub fn push_audio_frame(&self, frame: Vec<f32>) {
        self.push_audio_frame_for_operation(None, frame);
    }

    pub fn push_audio_frame_for_operation(
        &self,
        operation_id: Option<u64>,
        frame_16khz_mono: Vec<f32>,
    ) {
        let bytes = frame_16khz_mono_to_pcm_s16le_bytes(&frame_16khz_mono);
        push_pcm_bytes_to_coordinator(&self.coordinator, operation_id, bytes);
    }

    pub(crate) fn begin_finish_session_if_matches(
        &self,
        expected_operation_id: Option<u64>,
    ) -> ElevenLabsFinalizingSession {
        let mut coordinator = self.coordinator.lock();
        let session = detach_session_if_matches(&mut coordinator, expected_operation_id);
        ElevenLabsFinalizingSession {
            app_handle: self.app_handle.clone(),
            session,
        }
    }

    pub async fn finish_session(&self, timeout_ms: u32) -> Result<String> {
        self.finish_session_if_matches(None, timeout_ms).await
    }

    pub async fn finish_session_if_matches(
        &self,
        expected_operation_id: Option<u64>,
        timeout_ms: u32,
    ) -> Result<String> {
        self.begin_finish_session_if_matches(expected_operation_id)
            .finish(timeout_ms)
            .await
    }

    pub fn cancel_if_matches(&self, expected_operation_id: Option<u64>) {
        let session = {
            let mut coordinator = self.coordinator.lock();
            if let Some(op_id) = expected_operation_id {
                record_cancelled_operation(&mut coordinator.cancelled_operations, op_id);
            }
            detach_session_if_matches(&mut coordinator, expected_operation_id)
        };
        if let Some(session) = session {
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.lease.cancel();
            session.join_handle.abort();
            hide_preview_if_needed(
                &self.app_handle,
                session.show_preview,
                Some(&session.binding_id),
            );
            warn!(
                "Cancelled ElevenLabs live session for binding '{}' (op={:?})",
                session.binding_id, session.operation_id
            );
        }
    }

    pub fn cancel(&self) {
        let (session, pending_op_id) = {
            let mut coordinator = self.coordinator.lock();
            let pending_op_id = coordinator
                .pending_audio
                .take()
                .and_then(|pending| pending.operation_id);
            let active = coordinator.active_session.take();
            if let Some(op_id) = active
                .as_ref()
                .and_then(|session| session.operation_id)
                .or(pending_op_id)
            {
                record_cancelled_operation(&mut coordinator.cancelled_operations, op_id);
            }
            (active, pending_op_id)
        };
        let _ = pending_op_id;
        if let Some(session) = session {
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.lease.cancel();
            session.join_handle.abort();
            hide_preview_if_needed(
                &self.app_handle,
                session.show_preview,
                Some(&session.binding_id),
            );
            warn!(
                "Cancelled active ElevenLabs live session for binding '{}'",
                session.binding_id
            );
        }
    }

    pub async fn transcribe_history_replay(
        &self,
        samples: &[f32],
        api_key: &str,
        options: ElevenLabsRealtimeOptions,
        replay: &Arc<crate::managers::history_replay::HistoryReplay>,
    ) -> Result<String> {
        validate_options(api_key, &options)
            .map_err(|err| anyhow!(redact_elevenlabs_secrets(&err.to_string(), api_key)))?;
        let ws_request = build_elevenlabs_realtime_ws_request(api_key, &options)
            .map_err(|err| anyhow!(redact_elevenlabs_secrets(&err.to_string(), api_key)))?;
        let stream = timeout(
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            connect_async(ws_request),
        )
        .await
        .map_err(|_| anyhow!("Timed out while connecting to ElevenLabs WebSocket"))?
        .map_err(|error| anyhow!(format_handshake_error(error, api_key)))?
        .0;

        let (write, mut read) = stream.split();
        let mut write =
            super::history_replay::ReplaySink::new(write, replay, ELEVENLABS_SAMPLE_RATE_HZ);
        let (audio_tx, audio_rx) = mpsc::channel(1);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        let api_key_for_loop = api_key.trim().to_string();
        let sanitized_keyterms = sanitize_keyterm_library(&options.keyterm_library);

        let drive = Self::drive_session_loop(
            &mut write,
            &mut read,
            audio_rx,
            control_rx,
            Arc::clone(&final_text),
            None,
            api_key_for_loop,
            options.diarize,
            sanitized_keyterms,
            SessionEvents {
                on_partial: Box::new(|_, _| {}),
                on_committed: Box::new(|_, _, _, _| {}),
                on_diarized: Box::new(|_, _, _| {}),
                on_speakers_merged: Box::new(|_, _| {}),
            },
        );

        replay
            .run_audio(
                samples,
                audio_tx,
                frame_16khz_mono_to_pcm_s16le_bytes,
                || {
                    control_tx
                        .send(ControlMessage::Finish)
                        .map_err(|_| anyhow!("ElevenLabs live session closed"))
                },
                drive,
                final_text,
                ELEVENLABS_LIVE_FINALIZE_TIMEOUT_MS,
            )
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn drive_session_loop<S, R>(
        write: &mut S,
        read: &mut R,
        audio_rx: mpsc::Receiver<Vec<u8>>,
        control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        final_text: Arc<Mutex<String>>,
        on_final_chunk: Option<FinalChunkCallback>,
        api_key: String,
        diarize: bool,
        keyterm_library: Vec<String>,
        events: SessionEvents,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        Self::drive_session_loop_with_timeouts(
            write,
            read,
            audio_rx,
            control_rx,
            final_text,
            on_final_chunk,
            api_key,
            diarize,
            keyterm_library,
            events,
            Duration::from_millis(STREAMING_COMMIT_ACK_TIMEOUT_MS),
            Duration::from_millis(ELEVENLABS_LOOP_FINALIZE_TIMEOUT_MS),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn drive_session_loop_with_timeouts<S, R>(
        write: &mut S,
        read: &mut R,
        mut audio_rx: mpsc::Receiver<Vec<u8>>,
        mut control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        final_text: Arc<Mutex<String>>,
        on_final_chunk: Option<FinalChunkCallback>,
        api_key: String,
        diarize: bool,
        keyterm_library: Vec<String>,
        events: SessionEvents,
        streaming_ack_timeout: Duration,
        finalize_timeout: Duration,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let mut audio_input_closed = false;
        let mut control_closed = false;
        let mut sent_first_chunk = false;
        let mut state = ManualSessionState::new(diarize);
        let mut finalize_sleep: Option<Pin<Box<tokio::time::Sleep>>> = None;
        let mut streaming_ack_sleep: Option<Pin<Box<tokio::time::Sleep>>> = None;

        loop {
            if !control_closed {
                loop {
                    match control_rx.try_recv() {
                        Ok(ControlMessage::Finish) => {
                            arm_finalize_sleep(&mut finalize_sleep, finalize_timeout);
                        }
                        Ok(ControlMessage::Cancel) => {
                            close_socket(write).await;
                            return Ok(());
                        }
                        Ok(ControlMessage::Overflow(message)) => {
                            close_socket(write).await;
                            return Err(anyhow!(message));
                        }
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            control_closed = true;
                            break;
                        }
                    }
                }
            }

            if state.should_send_intermediate_commit(audio_input_closed) {
                Self::send_commit_chunk(
                    write,
                    &api_key,
                    &mut state,
                    &mut streaming_ack_sleep,
                    streaming_ack_timeout,
                    &finalize_sleep,
                    "intermediate",
                )
                .await?;
            }

            if audio_input_closed
                && state.pending_audio_remainder.is_none()
                && state.pending_commits == 0
            {
                if state.is_tail_settled(audio_input_closed) {
                    close_socket(write).await;
                    return Ok(());
                }
                if !state.tail_commit_sent {
                    state.tail_commit_sent = true;
                    if state.active_segment_pcm_bytes > 0 {
                        arm_finalize_sleep(&mut finalize_sleep, finalize_timeout);
                        Self::send_commit_chunk(
                            write,
                            &api_key,
                            &mut state,
                            &mut streaming_ack_sleep,
                            streaming_ack_timeout,
                            &finalize_sleep,
                            "final",
                        )
                        .await?;
                    }
                }
            }

            tokio::select! {
                biased;
                control = control_rx.recv(), if !control_closed => {
                    match control {
                        Some(ControlMessage::Finish) => {
                            arm_finalize_sleep(&mut finalize_sleep, finalize_timeout);
                        }
                        Some(ControlMessage::Cancel) => {
                            close_socket(write).await;
                            return Ok(());
                        }
                        Some(ControlMessage::Overflow(message)) => {
                            close_socket(write).await;
                            return Err(anyhow!(message));
                        }
                        None => {
                            control_closed = true;
                        }
                    }
                }
                _ = wait_for_sleep(finalize_sleep.as_mut()), if finalize_sleep.is_some() => {
                    close_socket(write).await;
                    return Err(anyhow!(state.finalize_timeout_reason(audio_input_closed)));
                }
                _ = wait_for_sleep(streaming_ack_sleep.as_mut()),
                    if streaming_ack_sleep.is_some() && state.pending_commits > 0 =>
                {
                    close_socket(write).await;
                    return Err(anyhow!(
                        "Timed out while waiting for ElevenLabs commit acknowledgement"
                    ));
                }
                frame = read.next() => {
                    match Self::handle_incoming_frame(
                        frame,
                        write,
                        &api_key,
                        &mut state,
                        &final_text,
                        &on_final_chunk,
                        &events,
                        audio_input_closed,
                    )
                    .await
                    {
                        Ok(true) => {
                            if state.pending_commits == 0 {
                                streaming_ack_sleep = None;
                            }
                        }
                        Ok(false) => break,
                        Err(err) => {
                            close_socket(write).await;
                            return Err(err);
                        }
                    }
                }
                _ = std::future::ready(()),
                    if state.pending_commits == 0 && state.pending_audio_remainder.is_some() =>
                {
                    if let Some(chunk) = state.pending_audio_remainder.take() {
                        Self::send_bounded_audio_chunk(
                            write,
                            &api_key,
                            &mut state,
                            &mut sent_first_chunk,
                            &keyterm_library,
                            chunk,
                        )
                        .await?;
                    }
                }
                audio_chunk = audio_rx.recv(),
                    if !audio_input_closed
                        && state.pending_commits == 0
                        && state.pending_audio_remainder.is_none() =>
                {
                    match audio_chunk {
                        Some(chunk) if !chunk.is_empty() => {
                            Self::send_bounded_audio_chunk(
                                write,
                                &api_key,
                                &mut state,
                                &mut sent_first_chunk,
                                &keyterm_library,
                                chunk,
                            )
                            .await?;
                        }
                        Some(_) => {}
                        None => {
                            audio_input_closed = true;
                            arm_finalize_sleep(&mut finalize_sleep, finalize_timeout);
                        }
                    }
                }
            }
        }

        close_socket(write).await;
        Ok(())
    }

    async fn send_bounded_audio_chunk<S>(
        write: &mut S,
        api_key: &str,
        state: &mut ManualSessionState,
        sent_first_chunk: &mut bool,
        keyterm_library: &[String],
        chunk: Vec<u8>,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        if chunk.is_empty() {
            return Ok(());
        }

        let remaining_cap = HARD_COMMIT_PCM_BYTES.saturating_sub(state.active_segment_pcm_bytes);
        if remaining_cap == 0 {
            state.pending_audio_remainder = Some(chunk);
            return Ok(());
        }

        let slice_len = chunk.len().min(remaining_cap).min(MAX_AUDIO_MESSAGE_PCM_BYTES);
        if slice_len < chunk.len() {
            state.pending_audio_remainder = Some(chunk[slice_len..].to_vec());
        }
        let slice = &chunk[..slice_len];

        let chunk_terms = if !*sent_first_chunk && !keyterm_library.is_empty() {
            Some(keyterm_library)
        } else {
            None
        };
        let payload = build_input_audio_chunk_payload(
            slice,
            false,
            ELEVENLABS_SAMPLE_RATE_HZ,
            chunk_terms,
        );
        *sent_first_chunk = true;
        state.active_segment_pcm_bytes += slice.len();

        if let Err(error) = send_socket_frame(write, Message::Text(payload.to_string().into())).await
        {
            close_socket(write).await;
            return Err(anyhow!(redact_elevenlabs_secrets(
                &format!("Failed to send audio chunk to ElevenLabs: {error}"),
                api_key,
            )));
        }
        Ok(())
    }

    async fn send_commit_chunk<S>(
        write: &mut S,
        api_key: &str,
        state: &mut ManualSessionState,
        streaming_ack_sleep: &mut Option<Pin<Box<tokio::time::Sleep>>>,
        streaming_ack_timeout: Duration,
        finalize_sleep: &Option<Pin<Box<tokio::time::Sleep>>>,
        phase: &str,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        debug_assert_eq!(state.pending_commits, 0);
        debug_assert!(state.active_segment_pcm_bytes > 0);

        let commit_payload = build_tail_commit_chunk_payload();
        if let Err(error) = send_socket_frame(write, Message::Text(commit_payload.to_string().into())).await
        {
            close_socket(write).await;
            return Err(anyhow!(redact_elevenlabs_secrets(
                &format!("Failed to send ElevenLabs {phase} commit chunk: {error}"),
                api_key,
            )));
        }
        state.pending_commits += 1;
        state.active_segment_pcm_bytes = 0;
        state.has_nonempty_speech_partial = false;
        state.latest_turn_probability = None;

        if finalize_sleep.is_none() {
            *streaming_ack_sleep = Some(Box::pin(tokio::time::sleep(streaming_ack_timeout)));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_incoming_frame<S>(
        frame: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
        write: &mut S,
        api_key: &str,
        state: &mut ManualSessionState,
        final_text: &Arc<Mutex<String>>,
        on_final_chunk: &Option<FinalChunkCallback>,
        events: &SessionEvents,
        audio_input_closed: bool,
    ) -> Result<bool>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        let frame = match frame {
            Some(frame) => frame.map_err(|error| {
                anyhow!(redact_elevenlabs_secrets(
                    &format!("ElevenLabs WebSocket read failed: {error}"),
                    api_key,
                ))
            })?,
            None => {
                if state.is_tail_settled(audio_input_closed) {
                    return Ok(false);
                }
                return Err(anyhow!(state.premature_close_reason(audio_input_closed)));
            }
        };

        match frame {
            Message::Text(text) => {
                let payload: Value = serde_json::from_str(text.as_ref()).map_err(|error| {
                    let preview: String = redact_elevenlabs_secrets(text.as_ref(), api_key)
                        .chars()
                        .take(200)
                        .collect();
                    anyhow!("Invalid ElevenLabs WebSocket payload: {error} (body: {preview})")
                })?;

                match parse_server_event(&payload, api_key)? {
                    ServerEvent::SessionStarted {
                        session_id,
                        commit_strategy,
                    } => {
                        // Absent config.commit_strategy assumes requested manual mode;
                        // explicit conflicting strategy fails immediately.
                        if let Some(strategy) = commit_strategy {
                            if !strategy.eq_ignore_ascii_case(ELEVENLABS_COMMIT_STRATEGY) {
                                return Err(anyhow!(
                                    "ElevenLabs negotiated conflicting commit_strategy '{}'; expected '{}'",
                                    strategy,
                                    ELEVENLABS_COMMIT_STRATEGY
                                ));
                            }
                        }
                        if let Some(id) = session_id {
                            debug!("ElevenLabs live session started (session_id={id})");
                        } else {
                            debug!("ElevenLabs live session started");
                        }
                    }
                    ServerEvent::PartialTranscript {
                        text,
                        words,
                        turn_probability,
                    } => {
                        let trimmed = text.trim();
                        let is_nonempty_speech = !trimmed.is_empty() || !words.is_empty();
                        if !is_nonempty_speech && state.latest_provisional.is_empty() {
                            if state.pending_commits == 0 {
                                state.has_nonempty_speech_partial = false;
                                state.latest_turn_probability = None;
                            }
                            return Ok(true);
                        }
                        state.latest_provisional = trimmed.to_string();
                        if state.pending_commits == 0 && state.active_segment_pcm_bytes > 0 {
                            state.has_nonempty_speech_partial = is_nonempty_speech;
                            state.latest_turn_probability = if is_nonempty_speech {
                                turn_probability
                            } else {
                                None
                            };
                        } else {
                            state.has_nonempty_speech_partial = false;
                            state.latest_turn_probability = None;
                        }
                        (events.on_partial)(&text, words);
                    }
                    ServerEvent::CommittedTranscript {
                        text,
                        segment_index,
                    } => {
                        if let Some(idx) = segment_index {
                            if !state.seen_committed_segments.insert(idx)
                                || state
                                    .max_committed_segment_index
                                    .is_some_and(|max| idx < max)
                            {
                                return Ok(true);
                            }
                        }

                        if state.pending_commits == 0 {
                            return Err(anyhow!(
                                "ElevenLabs protocol error: received unexpected unsolicited committed_transcript while no commit was pending under manual commit strategy"
                            ));
                        }

                        if state.diarize && segment_index.is_none() {
                            // Official protocol always emits segment_index when diarize=true;
                            // do not acknowledge a pending commit or emit text on an unindexed commit.
                            return Ok(true);
                        }

                        state.pending_commits -= 1;
                        state.latest_provisional.clear();
                        state.has_nonempty_speech_partial = false;
                        state.latest_turn_probability = None;

                        if let Some(idx) = segment_index {
                            state.max_committed_segment_index = Some(
                                state
                                    .max_committed_segment_index
                                    .map_or(idx, |max| max.max(idx)),
                            );
                            if state.diarize && !state.seen_diarized_segments.contains(&idx) {
                                state.pending_diarized_segments.insert(idx);
                            }
                        }

                        if let Some(delta) =
                            normalize_elevenlabs_chunk(&state.accumulated_text, &text)
                        {
                            state.accumulated_text.push_str(&delta);
                            *final_text.lock() = state.accumulated_text.clone();
                            if let Some(callback) = on_final_chunk {
                                callback(delta.clone());
                            }
                            (events.on_committed)(
                                &delta,
                                text.trim(),
                                &state.accumulated_text,
                                segment_index,
                            );
                        } else {
                            (events.on_committed)(
                                "",
                                text.trim(),
                                &state.accumulated_text,
                                segment_index,
                            );
                        }
                    }
                    ServerEvent::DiarizedTranscript {
                        segment_index,
                        text,
                        words,
                    } => {
                        if let Some(idx) = segment_index {
                            if !state.seen_diarized_segments.insert(idx) {
                                return Ok(true);
                            }
                            state.pending_diarized_segments.remove(&idx);
                        }
                        // Never emit diarized_transcript into final_text or on_final_chunk:
                        // committed_transcript is already the single authoritative text source.
                        (events.on_diarized)(segment_index, &text, words);
                    }
                    ServerEvent::SpeakersMerged {
                        from_speaker_id,
                        to_speaker_id,
                    } => {
                        (events.on_speakers_merged)(&from_speaker_id, &to_speaker_id);
                    }
                    ServerEvent::Warning(warning_message) => {
                        warn!(
                            "ElevenLabs live session non-fatal warning: {}",
                            redact_elevenlabs_secrets(&warning_message, api_key)
                        );
                    }
                    ServerEvent::Ignore => {}
                }
            }
            Message::Ping(payload) => {
                send_socket_frame(write, Message::Pong(payload)).await.map_err(|error| {
                    anyhow!(redact_elevenlabs_secrets(
                        &format!("Failed to respond to ElevenLabs ping: {error}"),
                        api_key,
                    ))
                })?;
            }
            Message::Pong(_) | Message::Binary(_) => {}
            Message::Close(_) => {
                if state.is_tail_settled(audio_input_closed) {
                    return Ok(false);
                }
                return Err(anyhow!(state.premature_close_reason(audio_input_closed)));
            }
            _ => {}
        }

        Ok(true)
    }
}

struct ManualSessionState {
    diarize: bool,
    active_segment_pcm_bytes: usize,
    pending_audio_remainder: Option<Vec<u8>>,
    pending_commits: usize,
    tail_commit_sent: bool,
    accumulated_text: String,
    latest_provisional: String,
    has_nonempty_speech_partial: bool,
    latest_turn_probability: Option<f64>,
    seen_committed_segments: HashSet<u64>,
    max_committed_segment_index: Option<u64>,
    seen_diarized_segments: HashSet<u64>,
    pending_diarized_segments: BTreeSet<u64>,
}

impl ManualSessionState {
    fn new(diarize: bool) -> Self {
        Self {
            diarize,
            active_segment_pcm_bytes: 0,
            pending_audio_remainder: None,
            pending_commits: 0,
            tail_commit_sent: false,
            accumulated_text: String::new(),
            latest_provisional: String::new(),
            has_nonempty_speech_partial: false,
            latest_turn_probability: None,
            seen_committed_segments: HashSet::new(),
            max_committed_segment_index: None,
            seen_diarized_segments: HashSet::new(),
            pending_diarized_segments: BTreeSet::new(),
        }
    }

    fn should_send_intermediate_commit(&self, audio_input_closed: bool) -> bool {
        if self.pending_commits > 0 {
            return false;
        }
        if self.active_segment_pcm_bytes >= HARD_COMMIT_PCM_BYTES {
            return true;
        }
        !audio_input_closed
            && self.active_segment_pcm_bytes >= SOFT_COMMIT_MIN_PCM_BYTES
            && self.has_nonempty_speech_partial
            && self
                .latest_turn_probability
                .is_some_and(|prob| prob >= TURN_COMMIT_THRESHOLD)
    }

    fn is_tail_settled(&self, audio_input_closed: bool) -> bool {
        if !audio_input_closed
            || self.pending_audio_remainder.is_some()
            || self.pending_commits > 0
            || self.active_segment_pcm_bytes > 0
            || !self.latest_provisional.is_empty()
        {
            return false;
        }
        if self.diarize && !self.pending_diarized_segments.is_empty() {
            return false;
        }
        true
    }

    fn premature_close_reason(&self, audio_input_closed: bool) -> &'static str {
        if !audio_input_closed || self.pending_audio_remainder.is_some() {
            "ElevenLabs WebSocket closed before all recording audio was sent"
        } else if self.active_segment_pcm_bytes > 0 {
            "ElevenLabs WebSocket closed before final commit was sent"
        } else if self.pending_commits > 0 {
            "ElevenLabs WebSocket closed before final commit acknowledgement was received"
        } else if !self.latest_provisional.is_empty() {
            "ElevenLabs WebSocket closed with uncommitted interim transcript remaining"
        } else if self.diarize && !self.pending_diarized_segments.is_empty() {
            "ElevenLabs WebSocket closed before matching diarized final transcript was received"
        } else {
            "ElevenLabs WebSocket closed before tail finalization settled"
        }
    }

    fn finalize_timeout_reason(&self, audio_input_closed: bool) -> &'static str {
        if self.pending_commits > 0 {
            "Timed out while waiting for ElevenLabs final commit acknowledgement"
        } else if !audio_input_closed || self.pending_audio_remainder.is_some() {
            "Timed out while draining ElevenLabs final audio queue"
        } else if self.active_segment_pcm_bytes > 0 {
            "Timed out before ElevenLabs final commit could be sent"
        } else if !self.latest_provisional.is_empty() {
            "Timed out while waiting for ElevenLabs to commit remaining interim transcript"
        } else if self.diarize && !self.pending_diarized_segments.is_empty() {
            "Timed out while waiting for ElevenLabs matching diarized final transcript"
        } else {
            "Timed out while finalizing ElevenLabs live transcription"
        }
    }
}

// SinkExt::send flushes the frame. Bound writes and close handshakes as well as
// reads, so a stalled network cannot keep a streaming task alive indefinitely.
async fn send_socket_frame<S>(write: &mut S, message: Message) -> Result<()>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    timeout(SOCKET_WRITE_TIMEOUT, write.send(message))
        .await
        .map_err(|_| anyhow!("Timed out writing to ElevenLabs WebSocket"))??;
    Ok(())
}

async fn close_socket<S>(write: &mut S)
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let _ = timeout(SOCKET_CLOSE_TIMEOUT, write.close()).await;
}

fn arm_finalize_sleep(
    finalize_sleep: &mut Option<Pin<Box<tokio::time::Sleep>>>,
    finalize_timeout: Duration,
) {
    if finalize_sleep.is_none() {
        *finalize_sleep = Some(Box::pin(tokio::time::sleep(finalize_timeout)));
    }
}

async fn wait_for_sleep(sleep: Option<&mut Pin<Box<tokio::time::Sleep>>>) {
    match sleep {
        Some(sleep) => sleep.as_mut().await,
        None => std::future::pending().await,
    }
}

impl ElevenLabsFinalizingSession {
    pub(crate) async fn finish(self, timeout_ms: u32) -> Result<String> {
        let Some(session) = self.session else {
            return Ok(String::new());
        };

        let ActiveSession {
            binding_id,
            show_preview,
            audio_tx,
            control_tx,
            final_text,
            mut join_handle,
            operation_id,
            lease,
            ..
        } = session;
        let hide_preview = || {
            if lease.allows_delivery() && session_callback_is_current(&self.app_handle, operation_id, None) {
                hide_preview_if_needed(&self.app_handle, show_preview, Some(&binding_id));
            }
        };
        let read_final_text = || -> String { final_text.lock().trim().to_string() };

        let _ = control_tx.send(ControlMessage::Finish);
        drop(audio_tx);

        let wait_ms = timeout_ms.clamp(500, 20_000);
        let join_result = timeout(Duration::from_millis(u64::from(wait_ms)), &mut join_handle).await;
        match join_result {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => {
                warn!(
                    "ElevenLabs live session ended with error before final acknowledgement (binding='{}'): {}",
                    binding_id, error
                );
                hide_preview();
                return Err(error);
            }
            Ok(Err(error)) => {
                hide_preview();
                return Err(anyhow!("ElevenLabs live session task failed: {error}"));
            }
            Err(_) => {
                join_handle.abort();
                hide_preview();
                return Err(anyhow!(
                    "Timed out while waiting for ElevenLabs final commit acknowledgement"
                ));
            }
        }

        let text = read_final_text();
        hide_preview();
        info!(
            "Completed ElevenLabs live session for binding '{}', output_len={}",
            binding_id,
            text.len()
        );
        Ok(text)
    }
}

fn build_runtime_session_events(
    app_handle: AppHandle,
    binding_id: String,
    show_preview: bool,
    on_packet: Option<ElevenLabsLivePacketCallback>,
) -> SessionEvents {
    let preview_eligible = show_preview
        && binding_id != crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID
        && binding_id != super::history_replay::HISTORY_REPLAY_BINDING;

    let app_partial = app_handle.clone();
    let packet_partial = on_packet.clone();
    let app_committed = app_handle;
    let packet_committed = on_packet.clone();
    let packet_diarized = on_packet.clone();
    let packet_merged = on_packet;

    SessionEvents {
        on_partial: Box::new(move |text, words| {
            if preview_eligible {
                let preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
                crate::overlay::emit_soniox_live_preview_update(
                    &app_partial,
                    &preview_final_text,
                    text.trim(),
                );
            }
            if let Some(cb) = &packet_partial {
                cb(ElevenLabsLivePacket::Partial {
                    text: text.to_string(),
                    words,
                });
            }
        }),
        on_committed: Box::new(move |_delta, committed_trimmed, _accumulated, segment_index| {
            if preview_eligible && !committed_trimmed.is_empty() {
                let preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
                if let Some(piece) = normalize_elevenlabs_chunk(&preview_final_text, committed_trimmed) {
                    let next_preview_final = format!("{preview_final_text}{piece}");
                    crate::overlay::emit_soniox_live_preview_update(
                        &app_committed,
                        &next_preview_final,
                        "",
                    );
                }
            }
            if let Some(cb) = &packet_committed {
                cb(ElevenLabsLivePacket::Committed {
                    text: committed_trimmed.to_string(),
                    segment_index,
                });
            }
        }),
        on_diarized: Box::new(move |segment_index, text, words| {
            if let Some(cb) = &packet_diarized {
                cb(ElevenLabsLivePacket::Diarized {
                    segment_index,
                    text: text.to_string(),
                    words,
                });
            }
        }),
        on_speakers_merged: Box::new(move |from_speaker_id, to_speaker_id| {
            if let Some(cb) = &packet_merged {
                cb(ElevenLabsLivePacket::SpeakersMerged {
                    from_speaker_id: from_speaker_id.to_string(),
                    to_speaker_id: to_speaker_id.to_string(),
                });
            }
        }),
    }
}

fn hide_preview_if_needed(app_handle: &AppHandle, show_preview: bool, binding_id: Option<&str>) {
    if !show_preview {
        return;
    }
    if binding_id == Some(crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID)
        || binding_id == Some(super::history_replay::HISTORY_REPLAY_BINDING)
    {
        return;
    }
    if crate::managers::preview_output_mode::is_active() {
        return;
    }
    crate::overlay::end_soniox_live_preview_session();
    crate::overlay::hide_soniox_live_preview_window(app_handle);
}

#[derive(Debug, PartialEq)]
enum ServerEvent {
    SessionStarted {
        session_id: Option<String>,
        commit_strategy: Option<String>,
    },
    PartialTranscript {
        text: String,
        words: Vec<ElevenLabsLiveWord>,
        turn_probability: Option<f64>,
    },
    CommittedTranscript {
        text: String,
        segment_index: Option<u64>,
    },
    DiarizedTranscript {
        segment_index: Option<u64>,
        text: String,
        words: Vec<ElevenLabsLiveWord>,
    },
    SpeakersMerged {
        from_speaker_id: String,
        to_speaker_id: String,
    },
    Warning(String),
    Ignore,
}

fn parse_server_event(payload: &Value, api_key: &str) -> Result<ServerEvent> {
    let msg_type = payload
        .get("message_type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();

    match msg_type {
        "session_started" => {
            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);
            let commit_strategy = payload
                .get("config")
                .and_then(|config| config.get("commit_strategy"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_ascii_lowercase);
            Ok(ServerEvent::SessionStarted {
                session_id,
                commit_strategy,
            })
        }
        "partial_transcript" => {
            let text = payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let words = parse_live_words(payload.get("words"));
            let turn_probability = payload
                .get("turn_probability")
                .and_then(Value::as_f64)
                .filter(|prob| prob.is_finite() && (0.0..=1.0).contains(prob));
            Ok(ServerEvent::PartialTranscript {
                text,
                words,
                turn_probability,
            })
        }
        "committed_transcript" => {
            let text = payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let segment_index = parse_optional_u64(payload.get("segment_index"));
            Ok(ServerEvent::CommittedTranscript {
                text,
                segment_index,
            })
        }
        "diarized_transcript" => {
            let segment_index = parse_optional_u64(payload.get("segment_index"));
            let text = payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let words = parse_live_words(payload.get("words"));
            Ok(ServerEvent::DiarizedTranscript {
                segment_index,
                text,
                words,
            })
        }
        "speakers_merged" => {
            let from_speaker_id = payload
                .get("from_speaker_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            let to_speaker_id = payload
                .get("to_speaker_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            if from_speaker_id.is_empty() || to_speaker_id.is_empty() {
                return Ok(ServerEvent::Ignore);
            }
            Ok(ServerEvent::SpeakersMerged {
                from_speaker_id,
                to_speaker_id,
            })
        }
        // Authoritative final text already arrives via `committed_transcript`.
        // Ignore follow-up representations so timestamped/annotated copies are
        // never double-emitted as new dictated text.
        "committed_transcript_with_timestamps"
        | "committed_transcript_entities"
        | "redacted_transcript"
        | "edited_transcript" => Ok(ServerEvent::Ignore),
        "warning" => {
            let message = payload
                .get("warning")
                .and_then(Value::as_str)
                .or_else(|| payload.get("message").and_then(Value::as_str))
                .or_else(|| payload.get("error").and_then(Value::as_str))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("ElevenLabs realtime warning")
                .to_string();
            Ok(ServerEvent::Warning(redact_elevenlabs_secrets(
                &message, api_key,
            )))
        }
        "invalid_request"
        | "auth_error"
        | "unaccepted_terms"
        | "quota_exceeded"
        | "rate_limited"
        | "commit_throttled"
        | "queue_overflow"
        | "resource_exhausted"
        | "session_time_limit_exceeded"
        | "chunk_size_exceeded"
        | "input_error"
        | "insufficient_audio_activity"
        | "transcriber_error"
        | "error" => {
            let fallback = format!("ElevenLabs realtime returned '{msg_type}'");
            let detail = payload
                .get("error")
                .and_then(Value::as_str)
                .or_else(|| payload.get("message").and_then(Value::as_str))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|raw| format!("ElevenLabs realtime ({msg_type}): {raw}"))
                .unwrap_or_else(|| parse_provider_error_value(payload, &fallback));
            Err(anyhow!(redact_elevenlabs_secrets(&detail, api_key)))
        }
        other => {
            debug!("Ignoring ElevenLabs realtime event '{}'", other);
            Ok(ServerEvent::Ignore)
        }
    }
}

fn parse_optional_u64(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    if let Some(num) = value.as_u64() {
        return Some(num);
    }
    if let Some(num) = value.as_i64() {
        return (num >= 0).then_some(num as u64);
    }
    value
        .as_str()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

fn parse_time_ms(word: &Value, ms_key: &str, secs_key: &str) -> Option<u64> {
    if let Some(ms) = parse_optional_u64(word.get(ms_key)) {
        return Some(ms);
    }
    let secs = word.get(secs_key)?.as_f64()?;
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }
    Some((secs * 1000.0).round() as u64)
}

fn parse_live_words(value: Option<&Value>) -> Vec<ElevenLabsLiveWord> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut words = Vec::with_capacity(items.len());
    for item in items {
        let word_type = item.get("type").and_then(Value::as_str).map(str::trim);
        if matches!(word_type, Some("spacing")) {
            continue;
        }
        let Some(text) = item.get("text").and_then(Value::as_str) else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let speaker_id = item
            .get("speaker_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
        words.push(ElevenLabsLiveWord {
            text: trimmed.to_string(),
            start_ms: parse_time_ms(item, "start_ms", "start"),
            end_ms: parse_time_ms(item, "end_ms", "end"),
            speaker_id,
        });
    }
    words
}

fn validate_options(api_key: &str, options: &ElevenLabsRealtimeOptions) -> Result<()> {
    if api_key.trim().is_empty() {
        return Err(anyhow!("ElevenLabs API key is missing"));
    }
    let model = options.model_id.trim();
    if !model.is_empty() && !ElevenLabsRealtimeManager::is_realtime_model(model) {
        return Err(anyhow!(
            "ElevenLabs realtime route only supports model '{}', got '{}'",
            ELEVENLABS_SCRIBE_V3_LIVE_MODEL,
            model
        ));
    }
    let _ = build_elevenlabs_realtime_ws_url(options)?;
    Ok(())
}

fn canonical_elevenlabs_ws_path() -> String {
    format!("/v1{ELEVENLABS_REALTIME_WS_SUFFIX}")
}

fn build_elevenlabs_realtime_ws_url(options: &ElevenLabsRealtimeOptions) -> Result<String> {
    let raw_base = options.base_url.trim();
    let effective_base = if raw_base.is_empty() {
        REMOTE_STT_ELEVENLABS_BASE_URL
    } else {
        raw_base
    };

    let canonical_base = reqwest::Url::parse(REMOTE_STT_ELEVENLABS_BASE_URL)
        .map_err(|error| anyhow!("Invalid canonical ElevenLabs base URL: {error}"))?;
    let canonical_host = canonical_base
        .host_str()
        .ok_or_else(|| anyhow!("Canonical ElevenLabs base URL must include a host"))?;

    let mut url = reqwest::Url::parse(effective_base)
        .map_err(|error| anyhow!("Invalid ElevenLabs base URL: {error}"))?;

    if !url.username().is_empty() || url.password().is_some() {
        return Err(anyhow!(
            "ElevenLabs base URL must not contain embedded credentials"
        ));
    }

    match url.scheme() {
        "https" | "wss" => {}
        scheme => {
            return Err(anyhow!(
                "ElevenLabs realtime base URL must use HTTPS or WSS, got '{}://'",
                scheme
            ));
        }
    }

    let mut validation_url = url.clone();
    if validation_url.scheme() == "wss" {
        validation_url
            .set_scheme("https")
            .map_err(|_| anyhow!("Failed to validate ElevenLabs URL scheme"))?;
    }
    validate_remote_stt_endpoint_url(validation_url.as_str(), false)
        .map_err(|error| anyhow!("{error}"))?;

    let Some(host) = url.host_str() else {
        return Err(anyhow!("ElevenLabs base URL must include a valid host"));
    };
    if !host.eq_ignore_ascii_case(canonical_host) {
        return Err(anyhow!(
            "ElevenLabs realtime base URL must use canonical host '{}', got '{}'",
            canonical_host,
            host
        ));
    }
    if url.port().is_some()
        || url.port_or_known_default() != canonical_base.port_or_known_default()
    {
        return Err(anyhow!(
            "ElevenLabs realtime base URL must use the canonical HTTPS/WSS default port"
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(anyhow!(
            "ElevenLabs base URL must not contain query parameters or fragments"
        ));
    }

    let ws_path = canonical_elevenlabs_ws_path();
    let normalized_path = url.path().trim_end_matches('/');
    if normalized_path.is_empty() || normalized_path == "/v1" || normalized_path == ws_path {
        url.set_path(&ws_path);
    } else {
        return Err(anyhow!(
            "ElevenLabs realtime base URL must use canonical path '/v1' or '{}', got '{}'",
            ws_path,
            url.path()
        ));
    }

    if url.scheme() == "https" {
        url.set_scheme("wss")
            .map_err(|_| anyhow!("Failed to convert ElevenLabs URL scheme to wss"))?;
    }

    let language_resolution =
        resolve_requested_language_for_elevenlabs(options.language_code.as_deref());

    {
        let mut query = url.query_pairs_mut();
        query.append_pair("model_id", ELEVENLABS_SCRIBE_V3_LIVE_MODEL);
        query.append_pair("audio_format", ELEVENLABS_AUDIO_FORMAT);
        query.append_pair("commit_strategy", ELEVENLABS_COMMIT_STRATEGY);

        if let Some(language) = language_resolution.code.as_deref() {
            query.append_pair("language_code", language);
        } else {
            query.append_pair("include_language_detection", "true");
        }

        if options.diarize {
            // ElevenLabs rejects `diarize=true` combined with `filter_background_audio=true`.
            query.append_pair("diarize", "true");
        }

        if options.no_verbatim {
            query.append_pair("no_verbatim", "true");
        }

        if options.zero_retention {
            query.append_pair("enable_logging", "false");
        }
    }

    Ok(url.to_string())
}

fn build_elevenlabs_realtime_ws_request(
    api_key: &str,
    options: &ElevenLabsRealtimeOptions,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>> {
    let ws_url = build_elevenlabs_realtime_ws_url(options)?;
    let mut request = ws_url
        .into_client_request()
        .map_err(|error| anyhow!("Failed to create ElevenLabs WebSocket request: {error}"))?;

    let canonical_base = reqwest::Url::parse(REMOTE_STT_ELEVENLABS_BASE_URL)
        .map_err(|error| anyhow!("Invalid canonical ElevenLabs base URL: {error}"))?;
    let canonical_host = canonical_base
        .host_str()
        .ok_or_else(|| anyhow!("Canonical ElevenLabs base URL must include a host"))?;
    let expected_path = canonical_elevenlabs_ws_path();
    let uri = request.uri();
    let authority_matches = uri
        .authority()
        .is_some_and(|authority| authority.as_str().eq_ignore_ascii_case(canonical_host));
    if uri.scheme_str() != Some("wss") || !authority_matches || uri.path() != expected_path {
        return Err(anyhow!(
            "Refusing to attach ElevenLabs credentials to non-canonical WebSocket target"
        ));
    }

    request.headers_mut().insert(
        "xi-api-key",
        api_key
            .trim()
            .parse()
            .map_err(|error| anyhow!("Invalid ElevenLabs auth header: {error}"))?,
    );
    Ok(request)
}

fn sanitize_keyterm_library(terms: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut total_bytes: usize = 2; // "[]"

    for raw in terms {
        if out.len() >= MAX_KEYTERM_LIBRARY_TERMS {
            break;
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.chars().count() > MAX_KEYTERM_CHARS {
            continue;
        }
        if !seen.insert(trimmed.to_string()) {
            continue;
        }
        let estimated_entry_bytes = trimmed.len() + 3; // quotes + comma
        if total_bytes + estimated_entry_bytes > MAX_KEYTERM_LIBRARY_JSON_BYTES {
            break;
        }
        total_bytes += estimated_entry_bytes;
        out.push(trimmed.to_string());
    }

    out
}

fn build_input_audio_chunk_payload(
    pcm_bytes: &[u8],
    commit: bool,
    sample_rate: u32,
    first_chunk_keyterm_library: Option<&[String]>,
) -> Value {
    let encoded = if pcm_bytes.is_empty() {
        String::new()
    } else {
        BASE64_STANDARD.encode(pcm_bytes)
    };
    let mut payload = json!({
        "message_type": "input_audio_chunk",
        "audio_base_64": encoded,
        "commit": commit,
        "sample_rate": sample_rate,
    });
    if let Some(terms) = first_chunk_keyterm_library.filter(|t| !t.is_empty()) {
        payload["keyterm_library"] = json!(terms);
    }
    payload
}

fn build_tail_commit_chunk_payload() -> Value {
    build_input_audio_chunk_payload(&[], true, ELEVENLABS_SAMPLE_RATE_HZ, None)
}

fn format_handshake_error(
    error: tokio_tungstenite::tungstenite::Error,
    api_key: &str,
) -> String {
    let raw = match &error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            let status = response.status();
            let header_msg = response
                .headers()
                .get("x-error-message")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if status.is_redirection() {
                format!("ElevenLabs WebSocket redirects are disallowed ({status})")
            } else if let Some(header_msg) = header_msg {
                format!("ElevenLabs handshake rejected ({status}): {header_msg}")
            } else if let Some(body) = response.body() {
                let body_text = String::from_utf8_lossy(body);
                let trimmed = body_text.trim();
                if trimmed.is_empty() {
                    format!("ElevenLabs handshake rejected ({status})")
                } else {
                    let snippet: String = trimmed.chars().take(300).collect();
                    format!("ElevenLabs handshake rejected ({status}): {snippet}")
                }
            } else {
                format!("ElevenLabs handshake rejected ({status})")
            }
        }
        other => format!("Failed to connect to ElevenLabs WebSocket: {other}"),
    };
    redact_elevenlabs_secrets(&raw, api_key)
}

fn redact_elevenlabs_secrets(value: &str, api_key: &str) -> String {
    let redacted = redact_remote_stt_api_key(value, api_key);
    let redacted = XI_API_KEY_HEADER_REGEX.replace_all(&redacted, "$1[redacted]");
    let redacted = BEARER_TOKEN_REGEX.replace_all(&redacted, "$1[redacted]");
    let redacted = ELEVENLABS_TOKEN_REGEX.replace_all(&redacted, "[redacted-token]");
    QUERY_TOKEN_REGEX
        .replace_all(&redacted, "$1[redacted]")
        .into_owned()
}

fn push_pcm_bytes_to_coordinator(
    coordinator_lock: &Mutex<SessionCoordinator>,
    operation_id: Option<u64>,
    bytes: Vec<u8>,
) {
    if bytes.is_empty() {
        return;
    }

    let active_handles = {
        let mut coordinator = coordinator_lock.lock();
        if let Some(op_id) = operation_id {
            if coordinator.cancelled_operations.contains(&op_id) {
                return;
            }
        }
        if let Some(session) = coordinator
            .active_session
            .as_ref()
            .filter(|session| session.operation_id == operation_id)
        {
            Some((session.audio_tx.clone(), session.control_tx.clone()))
        } else {
            if coordinator.pending_audio.is_none()
                && operation_id.is_none()
                && coordinator.active_session.is_none()
            {
                coordinator.pending_audio = Some(PendingAudio {
                    operation_id: None,
                    frames: Vec::new(),
                    overflow_error: None,
                });
            }
            if let Some(pending) = coordinator
                .pending_audio
                .as_mut()
                .filter(|pending| pending.operation_id == operation_id)
            {
                if pending.overflow_error.is_none() {
                    if pending.frames.len() >= AUDIO_QUEUE_CAPACITY {
                        pending.overflow_error = Some(
                            "ElevenLabs live audio buffer overflowed before connection completed; aborting to avoid losing speech."
                                .to_string(),
                        );
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
            warn!("ElevenLabs live audio queue overflowed; aborting session to avoid dropping speech");
            let _ = control_tx.send(ControlMessage::Overflow(
                "ElevenLabs live audio queue overflowed; recording aborted to avoid losing speech."
                    .to_string(),
            ));
        }
    }
}

fn session_callback_is_current(app: &AppHandle, operation_id: Option<u64>, live_session_id: Option<u64>) -> bool {
    live_session_id.map(crate::managers::live_sound_transcription::is_session_current)
        .or_else(|| operation_id.map(|id| crate::session_manager::is_operation_current(app, id)))
        .unwrap_or(true)
}

fn prepare_session_restart(
    coordinator: &mut SessionCoordinator,
    is_current: impl FnOnce(Option<u64>, &str) -> bool,
) -> Option<SessionParams> {
    let session = coordinator.active_session.as_ref()?;
    let params = session.restart_params.clone()?;
    if params.operation_id != session.operation_id
        || coordinator.pending_audio.as_ref().is_some_and(|pending| pending.operation_id != session.operation_id)
        || !is_current(session.operation_id, &session.binding_id)
    { return None; }
    let session = coordinator.active_session.take()?;
    session.lease.cancel();
    let _ = session.control_tx.send(ControlMessage::Cancel);
    session.join_handle.abort();
    coordinator.pending_audio = Some(PendingAudio {
        operation_id: params.operation_id, frames: Vec::new(), overflow_error: None,
    });
    Some(params)
}

fn detach_session_if_matches(
    coordinator: &mut SessionCoordinator,
    expected_operation_id: Option<u64>,
) -> Option<ActiveSession> {
    let session = if coordinator
        .active_session
        .as_ref()
        .is_some_and(|session| session.operation_id == expected_operation_id)
    {
        coordinator.active_session.take()
    } else {
        None
    };
    if coordinator
        .pending_audio
        .as_ref()
        .is_some_and(|pending| pending.operation_id == expected_operation_id)
    {
        coordinator.pending_audio = None;
    }
    session
}

fn record_cancelled_operation(queue: &mut VecDeque<u64>, operation_id: u64) {
    if queue.contains(&operation_id) {
        return;
    }
    if queue.len() >= CANCELLED_OPERATIONS_CAPACITY {
        let _ = queue.pop_front();
    }
    queue.push_back(operation_id);
}

fn normalize_elevenlabs_chunk(accumulated: &str, raw_chunk: &str) -> Option<String> {
    let trimmed = raw_chunk.trim();
    if trimmed.is_empty() {
        return None;
    }
    if accumulated.is_empty() {
        return Some(trimmed.to_string());
    }

    let raw_had_leading_space = raw_chunk
        .chars()
        .next()
        .is_some_and(char::is_whitespace);
    let next_attaches_without_space = trimmed.chars().next().is_some_and(|ch| {
        matches!(
            ch,
            '.' | ',' | '!' | '?' | ';' | ':' | ')' | ']' | '}' | '\'' | '’' | '%' | '…'
        )
    });
    let prev_attaches_without_space = accumulated.chars().last().is_some_and(|ch| {
        matches!(ch, '(' | '[' | '{' | '-' | '/' | '$' | '“' | '‘' | '«')
    });

    if raw_had_leading_space
        || (!next_attaches_without_space && !prev_attaches_without_space)
    {
        Some(format!(" {trimmed}"))
    } else {
        Some(trimmed.to_string())
    }
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
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct VecSink {
        sent: Vec<Message>,
        closed: bool,
        tail_commit_sent: Option<tokio::sync::oneshot::Sender<()>>,
        on_send: Option<Box<dyn FnMut(&Message) + Send>>,
    }

    impl VecSink {
        fn new() -> Self {
            Self {
                sent: Vec::new(),
                closed: false,
                tail_commit_sent: None,
                on_send: None,
            }
        }
    }

    impl Sink<Message> for VecSink {
        type Error = tokio_tungstenite::tungstenite::Error;

        fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            if let Some(callback) = self.on_send.as_mut() {
                callback(&item);
            }
            self.sent.push(item);
            Ok(())
        }

        fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if let Some(Message::Text(text)) = self.sent.last() {
                if let Ok(payload) = serde_json::from_str::<Value>(text.as_ref()) {
                    if payload.get("commit").and_then(Value::as_bool) == Some(true) {
                        if let Some(tx) = self.tail_commit_sent.take() {
                            let _ = tx.send(());
                        }
                    }
                }
            }
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            self.closed = true;
            Poll::Ready(Ok(()))
        }
    }

    fn active_fixture(operation_id: u64) -> ActiveSession {
        let (audio_tx, _) = mpsc::channel(4);
        let (control_tx, _) = mpsc::unbounded_channel();
        ActiveSession {
            binding_id: "transcribe".into(), operation_id: Some(operation_id), show_preview: false,
            audio_tx, control_tx, final_text: Arc::new(Mutex::new(String::new())),
            join_handle: JoinHandle::Tokio(tokio::spawn(std::future::pending::<Result<()>>())),
            lease: Arc::new(SessionLease::default()),
            restart_params: Some(SessionParams {
                binding_id: "transcribe".into(), operation_id: Some(operation_id), api_key: "dummy".into(),
                options: ElevenLabsRealtimeOptions::default(), chunk_callback_factory: None, on_packet: None,
            }),
        }
    }

    #[tokio::test]
    async fn clear_invalidates_queued_callbacks_but_normal_finish_keeps_final_words() {
        let queued = Arc::new(Mutex::new(Vec::<Box<dyn FnOnce() + Send>>::new()));
        let delivered = Arc::new(Mutex::new(String::new()));
        let factory: FinalChunkCallbackFactory = Arc::new({
            let queued = Arc::clone(&queued);
            let delivered = Arc::clone(&delivered);
            move |lease| {
                let queued = Arc::clone(&queued);
                let delivered = Arc::clone(&delivered);
                Arc::new(move |chunk| {
                    let lease = Arc::clone(&lease);
                    let delivered = Arc::clone(&delivered);
                    queued.lock().push(Box::new(move || {
                        if lease.allows_delivery() { delivered.lock().push_str(&chunk); }
                    }));
                })
            }
        });
        let old = active_fixture(7);
        let old_lease = Arc::clone(&old.lease);
        let old_callback = bind_session_chunk_callback(Some(&factory), &old.lease).unwrap();
        old_callback("queued before Clear".into());
        let mut coordinator = SessionCoordinator { active_session: Some(old), ..Default::default() };
        let params = prepare_session_restart(&mut coordinator, |id, binding| id == Some(7) && binding == "transcribe").unwrap();
        assert_eq!(params.operation_id, Some(7));
        assert!(!old_lease.allows_delivery());
        old_callback("arrived after Clear".into());
        let replacement = active_fixture(7);
        let new_callback = bind_session_chunk_callback(Some(&factory), &replacement.lease).unwrap();
        coordinator.active_session = Some(replacement);
        // Normal finish detaches, but its committed tail still belongs to this transport.
        let detached = detach_session_if_matches(&mut coordinator, Some(7)).unwrap();
        assert!(detached.lease.allows_delivery());
        new_callback("replacement tail".into());
        for work in std::mem::take(&mut *queued.lock()) { work(); }
        assert_eq!(*delivered.lock(), "replacement tail");
        detached.join_handle.abort();
    }

    #[tokio::test]
    async fn clear_cannot_restart_stopped_or_replaced_recordings() {
        let mut coordinator = SessionCoordinator {
            active_session: Some(active_fixture(7)), ..Default::default()
        };
        // Stop has changed the recording state, even though the transport is still attached.
        assert!(prepare_session_restart(&mut coordinator, |_, _| false).is_none());
        assert!(coordinator.active_session.as_ref().unwrap().lease.allows_delivery());
        // A successor's prepared audio must not be overwritten by an old Clear.
        coordinator.pending_audio = Some(PendingAudio {
            operation_id: Some(8), frames: vec![vec![1, 2]], overflow_error: None,
        });
        assert!(prepare_session_restart(&mut coordinator, |_, _| true).is_none());
        assert_eq!(coordinator.pending_audio.as_ref().unwrap().frames, [vec![1, 2]]);
        coordinator.active_session.take().unwrap().join_handle.abort();
    }

    #[test]
    fn replaced_transport_cannot_update_preview_or_speakers() {
        let lease = Arc::new(SessionLease::default());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let partial = Arc::clone(&count);
        let committed = Arc::clone(&count);
        let diarized = Arc::clone(&count);
        let merged = Arc::clone(&count);
        let owner = Arc::clone(&lease);
        let events = SessionEvents {
            on_partial: Box::new(move |_, _| { partial.fetch_add(1, Ordering::Relaxed); }),
            on_committed: Box::new(move |_, _, _, _| { committed.fetch_add(1, Ordering::Relaxed); }),
            on_diarized: Box::new(move |_, _, _| { diarized.fetch_add(1, Ordering::Relaxed); }),
            on_speakers_merged: Box::new(move |_, _| { merged.fetch_add(1, Ordering::Relaxed); }),
        }.only_current(move || owner.allows_delivery());
        (events.on_partial)("before Clear", vec![]);
        lease.cancel();
        (events.on_partial)("stale", vec![]);
        (events.on_committed)("stale", "stale", "stale", Some(1));
        (events.on_diarized)(Some(1), "stale", vec![]);
        (events.on_speakers_merged)("speaker_1", "speaker_2");
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn timed_out_scribe_chunks_flush_in_order_without_consuming_a_cancelled_trigger() {
        use crate::stream_chunk_processor::StreamChunkProcessor;
        use crate::text_replacement_decapitalize as decapitalize;
        use std::sync::mpsc as sync_mpsc;
        let _guard = decapitalize::TEST_MUTEX.lock().unwrap();
        for cancelled in [false, true] {
            let _ = decapitalize::maybe_decapitalize_next_chunk_standard("Reset");
            let mut settings = crate::settings::get_default_settings();
            settings.custom_words_enabled = true;
            settings.custom_words = vec!["AivoRelay".into()];
            settings.soniox_realtime_fuzzy_correction_enabled = true;
            settings.soniox_realtime_keep_safety_buffer_enabled = true;
            settings.output_whitespace_leading_mode = crate::settings::OutputWhitespaceMode::Preserve;
            let processor = Arc::new(std::sync::Mutex::new(StreamChunkProcessor::from_settings(&settings)));
            let lease = Arc::new(SessionLease::default());
            let inserted = Arc::new(Mutex::new(String::new()));
            let (release, released) = sync_mpsc::channel::<()>();
            let (started, has_started) = sync_mpsc::channel();
            crate::output_dispatch::run(move || {
                started.send(()).unwrap();
                let _ = released.recv_timeout(Duration::from_secs(5));
            }).unwrap();
            has_started.recv_timeout(Duration::from_secs(5)).unwrap();
            let factory: FinalChunkCallbackFactory = Arc::new({
                let processor = Arc::clone(&processor);
                let inserted = Arc::clone(&inserted);
                move |lease| {
                    let processor = Arc::clone(&processor);
                    let inserted = Arc::clone(&inserted);
                    Arc::new(move |chunk| {
                        let processor = Arc::clone(&processor);
                        let inserted = Arc::clone(&inserted);
                        let lease = Arc::clone(&lease);
                        assert!(crate::output_dispatch::run_sync(10, move || {
                            if lease.allows_delivery() {
                                inserted.lock().push_str(&processor.lock().unwrap().push_chunk(&chunk));
                            }
                        }).is_err(), "caller times out while the output task remains queued");
                    })
                }
            });
            let callback = bind_session_chunk_callback(Some(&factory), &lease).unwrap();
            callback("Hello buffered words ".into());
            callback("and the final tail".into());
            let (done, was_done) = sync_mpsc::channel();
            let tail = Arc::clone(&processor);
            let tail_lease = Arc::clone(&lease);
            let tail_output = Arc::clone(&inserted);
            crate::output_dispatch::run(move || {
                if tail_lease.allows_delivery() {
                    tail_output.lock().push_str(&tail.lock().unwrap().flush());
                }
                done.send(()).unwrap();
            }).unwrap();
            decapitalize::mark_edit_key_pressed(60_000, false);
            if cancelled { lease.cancel(); }
            release.send(()).unwrap();
            was_done.recv_timeout(Duration::from_secs(5)).unwrap();
            if cancelled {
                assert!(inserted.lock().is_empty());
                assert!(decapitalize::is_any_trigger_armed_now());
                assert!(processor.lock().unwrap().flush().is_empty());
            } else {
                assert_eq!(*inserted.lock(), "hello buffered words and the final tail");
                assert!(!decapitalize::is_any_trigger_armed_now());
            }
            let _ = decapitalize::maybe_decapitalize_next_chunk_standard("Reset");
        }
    }

    #[test]
    fn recognizes_only_scribe_v3_live_and_builds_expected_ws_request() {
        assert!(ElevenLabsRealtimeManager::is_realtime_model("scribe_v3_live"));
        assert!(ElevenLabsRealtimeManager::is_realtime_model(" SCRIBE_V3_LIVE "));
        assert!(!ElevenLabsRealtimeManager::is_realtime_model("scribe_v3"));
        assert!(!ElevenLabsRealtimeManager::is_realtime_model("scribe_v2_realtime"));
        assert!(!ElevenLabsRealtimeManager::is_realtime_model(""));

        let request = build_elevenlabs_realtime_ws_request(
            " secret-xi-api-key ",
            &ElevenLabsRealtimeOptions {
                base_url: "https://api.elevenlabs.io/v1".to_string(),
                model_id: ELEVENLABS_SCRIBE_V3_LIVE_MODEL.to_string(),
                language_code: Some("en-US".to_string()),
                diarize: true,
                keyterm_library: vec!["AIVORelay".to_string()],
                show_preview: true,
                no_verbatim: true,
                zero_retention: true,
            },
        )
        .unwrap();

        let uri = request.uri().to_string();
        assert!(uri.starts_with("wss://api.elevenlabs.io/v1/speech-to-text/realtime?"));
        assert!(uri.contains("model_id=scribe_v3_live"));
        assert!(uri.contains("audio_format=pcm_16000"));
        assert!(uri.contains("commit_strategy=manual"));
        assert!(!uri.contains("vad_silence_threshold_secs"));
        assert!(!uri.contains("keepalive_interval_ms"));
        assert!(uri.contains("language_code=en"));
        assert!(uri.contains("diarize=true"));
        assert!(!uri.contains("filter_background_audio"));
        assert!(!uri.contains("include_language_detection"));
        assert!(uri.contains("no_verbatim=true"));
        assert!(uri.contains("enable_logging=false"));
        assert!(!uri.contains("secret-xi-api-key"));
        assert_eq!(request.headers()["xi-api-key"], "secret-xi-api-key");

        for canonical_variant in [
            "",
            "https://api.elevenlabs.io",
            "https://api.elevenlabs.io/v1/",
            "wss://api.elevenlabs.io/v1/speech-to-text/realtime",
            "https://API.ELEVENLABS.IO/v1",
        ] {
            let auto_url = build_elevenlabs_realtime_ws_url(&ElevenLabsRealtimeOptions {
                base_url: canonical_variant.to_string(),
                model_id: ELEVENLABS_SCRIBE_V3_LIVE_MODEL.to_string(),
                language_code: Some("auto".to_string()),
                diarize: false,
                ..Default::default()
            })
            .unwrap();
            assert!(
                auto_url.starts_with("wss://api.elevenlabs.io/v1/speech-to-text/realtime?"),
                "unexpected ws url for {canonical_variant}: {auto_url}"
            );
            assert!(auto_url.contains("include_language_detection=true"));
            assert!(!auto_url.contains("language_code="));
            assert!(!auto_url.contains("diarize=true"));
            assert!(!auto_url.contains("no_verbatim"));
            assert!(!auto_url.contains("enable_logging"));
        }
    }

    #[test]
    fn rejects_insecure_arbitrary_hosts_credentials_paths_and_redirects_before_sending_api_key() {
        for bad_url in [
            "http://api.elevenlabs.io/v1",
            "ws://api.elevenlabs.io/v1",
            "http://localhost:8080/v1",
            "http://127.0.0.1/v1",
            "https://user:pass@api.elevenlabs.io/v1",
            "wss://user@api.elevenlabs.io/v1",
            "ftp://api.elevenlabs.io/v1",
            "https://api.eu.residency.elevenlabs.io",
            "https://evil.example.com/v1",
            "wss://evil.example.com/v1/speech-to-text/realtime",
            "https://api.elevenlabs.io.evil.com/v1",
            "https://api.elevenlabs.io:8443/v1",
            "https://api.elevenlabs.io/proxy/v1",
            "https://api.elevenlabs.io/v1?token=leak",
            "https://api.elevenlabs.io/v1#frag",
        ] {
            let url_err = build_elevenlabs_realtime_ws_url(&ElevenLabsRealtimeOptions {
                base_url: bad_url.to_string(),
                ..Default::default()
            })
            .unwrap_err();
            let req_err = build_elevenlabs_realtime_ws_request(
                "secret-xi-api-key",
                &ElevenLabsRealtimeOptions {
                    base_url: bad_url.to_string(),
                    ..Default::default()
                },
            )
            .unwrap_err();
            assert!(!url_err.to_string().is_empty(), "expected url error for {bad_url}");
            assert!(
                !req_err.to_string().contains("secret-xi-api-key"),
                "leaked key for {bad_url}: {req_err}"
            );
        }

        let redirect_response = tokio_tungstenite::tungstenite::http::Response::builder()
            .status(tokio_tungstenite::tungstenite::http::StatusCode::FOUND)
            .header(
                "location",
                "https://evil.example.com/steal?token=sutkn_secret123",
            )
            .body(None::<Vec<u8>>)
            .unwrap();
        let redirect_msg = format_handshake_error(
            tokio_tungstenite::tungstenite::Error::Http(redirect_response),
            "secret-xi-api-key",
        );
        assert!(redirect_msg.contains("redirects are disallowed"));
        assert!(!redirect_msg.contains("evil.example.com"));
        assert!(!redirect_msg.contains("sutkn_secret123"));
        assert!(!redirect_msg.contains("secret-xi-api-key"));
    }

    #[test]
    fn serializes_input_audio_chunk_with_first_chunk_only_keyterms_and_tail_commit() {
        let too_long = "x".repeat(MAX_KEYTERM_CHARS + 1);
        let terms = sanitize_keyterm_library(&[
            "  AIVORelay  ".to_string(),
            "".to_string(),
            "AIVORelay".to_string(),
            too_long,
            "Scribe v3".to_string(),
        ]);
        assert_eq!(terms, vec!["AIVORelay", "Scribe v3"]);

        let first = build_input_audio_chunk_payload(&[1, 2, 3, 4], false, 16_000, Some(&terms));
        assert_eq!(first["message_type"], "input_audio_chunk");
        assert_eq!(first["audio_base_64"], BASE64_STANDARD.encode([1, 2, 3, 4]));
        assert_eq!(first["commit"], false);
        assert_eq!(first["sample_rate"], 16_000);
        assert_eq!(first["keyterm_library"], json!(["AIVORelay", "Scribe v3"]));

        let second = build_input_audio_chunk_payload(&[5, 6], false, 16_000, None);
        assert!(second.get("keyterm_library").is_none());

        let tail = build_tail_commit_chunk_payload();
        assert_eq!(tail["message_type"], "input_audio_chunk");
        assert_eq!(tail["audio_base_64"], "");
        assert_eq!(tail["commit"], true);
        assert_eq!(tail["sample_rate"], 16_000);
        assert!(tail.get("keyterm_library").is_none());
    }

    #[test]
    fn parses_optional_and_unknown_fields_and_ignores_characters_schema_divergence() {
        let partial = parse_server_event(
            &json!({
                "message_type": "partial_transcript",
                "text": "Hello",
                "turn_probability": 0.42,
                "future_field": { "nested": true }
            }),
            "k",
        )
        .unwrap();
        assert_eq!(
            partial,
            ServerEvent::PartialTranscript {
                text: "Hello".to_string(),
                words: Vec::new(),
                turn_probability: Some(0.42),
            }
        );

        let timestamps = parse_server_event(
            &json!({
                "message_type": "committed_transcript_with_timestamps",
                "text": "Hello",
                "language_code": "en",
                "words": [{
                    "text": "Hello",
                    "start": 0.1,
                    "end": 0.5,
                    "type": "word",
                    "characters": ["H", "e", "l", "l", "o"]
                }]
            }),
            "k",
        )
        .unwrap();
        assert_eq!(timestamps, ServerEvent::Ignore);

        let diarized = parse_server_event(
            &json!({
                "message_type": "diarized_transcript",
                "segment_index": 2,
                "text": "Hello world.",
                "extra_top_level": 123,
                "words": [
                    {
                        "text": "Hello",
                        "start": 0.1,
                        "end": 0.4,
                        "type": "word",
                        "speaker_id": "speaker_0",
                        "characters": [{ "text": "H", "start": 0.1, "end": 0.2 }]
                    },
                    {
                        "text": " ",
                        "start": 0.4,
                        "end": 0.45,
                        "type": "spacing"
                    },
                    {
                        "text": "world.",
                        "start": 0.45,
                        "end": 0.8,
                        "type": "word",
                        "speaker_id": null,
                        "unknown_word_field": "kept-compatible"
                    }
                ]
            }),
            "k",
        )
        .unwrap();
        assert_eq!(
            diarized,
            ServerEvent::DiarizedTranscript {
                segment_index: Some(2),
                text: "Hello world.".to_string(),
                words: vec![
                    ElevenLabsLiveWord {
                        text: "Hello".to_string(),
                        start_ms: Some(100),
                        end_ms: Some(400),
                        speaker_id: Some("speaker_0".to_string()),
                    },
                    ElevenLabsLiveWord {
                        text: "world.".to_string(),
                        start_ms: Some(450),
                        end_ms: Some(800),
                        speaker_id: None,
                    },
                ],
            }
        );

        let warning = parse_server_event(
            &json!({
                "message_type": "warning",
                "warning": "Using default retention with xi-api-key: raw_key_123"
            }),
            "raw_key_123",
        )
        .unwrap();
        match warning {
            ServerEvent::Warning(msg) => {
                assert!(!msg.contains("raw_key_123"));
            }
            other => panic!("expected warning event, got {other:?}"),
        }

        let err = parse_server_event(
            &json!({
                "message_type": "auth_error",
                "error": "Rejected xi-api-key: my-secret-key and sutkn_ephemeral99"
            }),
            "my-secret-key",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("auth_error"));
        assert!(!err.contains("my-secret-key"));
        assert!(!err.contains("sutkn_ephemeral99"));
    }

    #[tokio::test]
    async fn session_loop_continues_past_nonfatal_warning_and_unknown_event() {
        let (audio_tx, audio_rx) = mpsc::channel(4);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));

        audio_tx.send(vec![1, 2]).await.unwrap();
        drop(audio_tx);
        control_tx.send(ControlMessage::Finish).unwrap();

        let mut sink = VecSink::new();
        let (tail_sent_tx, tail_sent_rx) = tokio::sync::oneshot::channel();
        sink.tail_commit_sent = Some(tail_sent_tx);

        let mut stream = futures_util::stream::iter(vec![
            Ok(Message::Text(
                r#"{"message_type":"warning","warning":"Non-fatal retention notice","extra":1}"#
                    .into(),
            )),
            Ok(Message::Text(
                r#"{"message_type":"future_event_v4","payload":[1,2,3]}"#.into(),
            )),
        ])
        .chain(futures_util::stream::once(async move {
            tail_sent_rx.await.unwrap();
            Ok(Message::Text(
                r#"{"message_type":"committed_transcript","text":"Recovered."}"#.into(),
            ))
        }))
        .boxed();

        ElevenLabsRealtimeManager::drive_session_loop(
            &mut sink,
            &mut stream,
            audio_rx,
            control_rx,
            Arc::clone(&final_text),
            None,
            "k".to_string(),
            false,
            Vec::new(),
            SessionEvents {
                on_partial: Box::new(|_, _| {}),
                on_committed: Box::new(|_, _, _, _| {}),
                on_diarized: Box::new(|_, _, _| {}),
                on_speakers_merged: Box::new(|_, _| {}),
            },
        )
        .await
        .unwrap();

        assert_eq!(*final_text.lock(), "Recovered.");
    }

    #[tokio::test]
    async fn session_loop_cancellation_aborts_cleanly_without_promoting_text() {
        let (_audio_tx, audio_rx) = mpsc::channel(4);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        let emitted_chunks = Arc::new(Mutex::new(Vec::<String>::new()));
        let emitted_chunks_cb = Arc::clone(&emitted_chunks);
        let cb: FinalChunkCallback = Arc::new(move |chunk| {
            emitted_chunks_cb.lock().push(chunk);
        });

        control_tx.send(ControlMessage::Cancel).unwrap();
        let mut sink = VecSink::new();
        let mut stream = futures_util::stream::pending();

        ElevenLabsRealtimeManager::drive_session_loop(
            &mut sink,
            &mut stream,
            audio_rx,
            control_rx,
            Arc::clone(&final_text),
            Some(cb),
            "k".to_string(),
            false,
            Vec::new(),
            SessionEvents {
                on_partial: Box::new(|_, _| {}),
                on_committed: Box::new(|_, _, _, _| {}),
                on_diarized: Box::new(|_, _, _| {}),
                on_speakers_merged: Box::new(|_, _| {}),
            },
        )
        .await
        .unwrap();

        assert!(sink.closed);
        assert!(final_text.lock().is_empty());
        assert!(emitted_chunks.lock().is_empty());
    }

    #[tokio::test]
    async fn operation_isolation_prevents_stale_cancel_or_finish_from_affecting_newer_session() {
        let mut coordinator = SessionCoordinator::default();
        let (audio_tx, _audio_rx) = mpsc::channel(4);
        let (control_tx, _control_rx) = mpsc::unbounded_channel();
        let handle = tauri::async_runtime::JoinHandle::Tokio(tokio::spawn(async { Ok(()) }));

        coordinator.pending_audio = Some(PendingAudio {
            operation_id: Some(2),
            frames: vec![vec![9, 9]],
            overflow_error: None,
        });
        coordinator.active_session = Some(ActiveSession {
            binding_id: "transcribe".to_string(),
            operation_id: Some(2),
            show_preview: false,
            audio_tx,
            control_tx,
            final_text: Arc::new(Mutex::new(String::new())),
            join_handle: handle,
            lease: Arc::new(SessionLease::default()),
            restart_params: None,
        });

        // Stale operation ID 1 must not detach active or pending state of operation 2.
        assert!(detach_session_if_matches(&mut coordinator, Some(1)).is_none());
        assert!(coordinator.active_session.is_some());
        assert!(coordinator.pending_audio.is_some());

        // Matching operation ID 2 detaches cleanly.
        assert!(detach_session_if_matches(&mut coordinator, Some(2)).is_some());
        assert!(coordinator.active_session.is_none());
        assert!(coordinator.pending_audio.is_none());

        // Cancelled operation ID tracking rejects stale late startup while capping history.
        for op in 10..80 {
            record_cancelled_operation(&mut coordinator.cancelled_operations, op);
        }
        assert!(coordinator.cancelled_operations.len() <= CANCELLED_OPERATIONS_CAPACITY);
        assert!(coordinator.cancelled_operations.contains(&79));
    }

    #[tokio::test]
    async fn session_loop_fails_on_queue_overflow_without_swallowing_error() {
        let (_audio_tx, audio_rx) = mpsc::channel(4);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        control_tx
            .send(ControlMessage::Overflow("overflow test".to_string()))
            .unwrap();

        let mut sink = VecSink::new();
        let mut stream = futures_util::stream::pending();

        let err = tokio::time::timeout(
            Duration::from_secs(1),
            ElevenLabsRealtimeManager::drive_session_loop(
                &mut sink,
                &mut stream,
                audio_rx,
                control_rx,
                final_text,
                None,
                "key".to_string(),
                false,
                Vec::new(),
                SessionEvents {
                    on_partial: Box::new(|_, _| {}),
                    on_committed: Box::new(|_, _, _, _| {}),
                    on_diarized: Box::new(|_, _, _| {}),
                    on_speakers_merged: Box::new(|_, _| {}),
                },
            ),
        )
        .await
        .expect("queue overflow must terminate the session promptly")
        .unwrap_err();

        assert!(err.to_string().contains("overflow test"));
        assert!(sink.closed);
    }

    #[test]
    fn chunk_normalization_avoids_gluing_words_or_breaking_punctuation() {
        let mut accumulated = String::new();
        let mut emitted = String::new();
        for raw in ["Hello,", "  ", "world.", "How are you", "?", "Good!"] {
            if let Some(delta) = normalize_elevenlabs_chunk(&accumulated, raw) {
                accumulated.push_str(&delta);
                emitted.push_str(&delta);
            }
        }
        assert_eq!(accumulated, "Hello, world. How are you? Good!");
        assert_eq!(emitted, accumulated);
    }

    #[test]
    fn pending_audio_enforces_bound_and_ignores_stale_or_cancelled_operations() {
        let coordinator = Mutex::new(SessionCoordinator::default());
        {
            let mut guard = coordinator.lock();
            guard.pending_audio = Some(PendingAudio {
                operation_id: Some(10),
                frames: Vec::new(),
                overflow_error: None,
            });
            record_cancelled_operation(&mut guard.cancelled_operations, 9);
        }

        // Cancelled operation 9 and stale operation 8 must not enqueue into operation 10.
        push_pcm_bytes_to_coordinator(&coordinator, Some(9), vec![1, 2]);
        push_pcm_bytes_to_coordinator(&coordinator, Some(8), vec![3, 4]);
        assert_eq!(
            coordinator.lock().pending_audio.as_ref().unwrap().frames.len(),
            0
        );

        for _ in 0..AUDIO_QUEUE_CAPACITY {
            push_pcm_bytes_to_coordinator(&coordinator, Some(10), vec![5, 6]);
        }
        {
            let guard = coordinator.lock();
            let pending = guard.pending_audio.as_ref().unwrap();
            assert_eq!(pending.frames.len(), AUDIO_QUEUE_CAPACITY);
            assert!(pending.overflow_error.is_none());
        }

        // 513th frame must trip overflow_error rather than dropping speech silently.
        push_pcm_bytes_to_coordinator(&coordinator, Some(10), vec![7, 8]);
        {
            let guard = coordinator.lock();
            let pending = guard.pending_audio.as_ref().unwrap();
            assert_eq!(pending.frames.len(), AUDIO_QUEUE_CAPACITY);
            assert!(pending.overflow_error.is_some());
        }
    }


    fn scripted_stream() -> (
        mpsc::UnboundedSender<Result<Message, tokio_tungstenite::tungstenite::Error>>,
        Pin<Box<dyn Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Send>>,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        let stream = futures_util::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        }).boxed();
        (tx, stream)
    }

    fn noop_session_events() -> SessionEvents {
        SessionEvents {
            on_partial: Box::new(|_, _| {}),
            on_committed: Box::new(|_, _, _, _| {}),
            on_diarized: Box::new(|_, _, _| {}),
            on_speakers_merged: Box::new(|_, _| {}),
        }
    }

    fn server_frame(value: Value) -> Result<Message, tokio_tungstenite::tungstenite::Error> {
        Ok(Message::Text(value.to_string().into()))
    }

    // The peer responds to actual client commits, and deliberately keeps the
    // socket open. No fabricated early acknowledgements or wall-clock waits.
    #[tokio::test]
    async fn manual_commits_preserve_over_ninety_seconds_of_audio_and_identical_phrases() {
        let (audio_tx, audio_rx) = mpsc::channel(128);
        let (_control_tx, control_rx) = mpsc::unbounded_channel();
        let input: Vec<u8> = (0..94 * MAX_AUDIO_MESSAGE_PCM_BYTES)
            .map(|i| (i % 251) as u8).collect();
        // Include input frames crossing the 22-second boundary.
        for chunk in input.chunks(MAX_AUDIO_MESSAGE_PCM_BYTES * 3) {
            audio_tx.send(chunk.to_vec()).await.unwrap();
        }
        drop(audio_tx);
        let (server_tx, mut stream) = scripted_stream();
        server_tx.send(server_frame(json!({
            "message_type":"session_started", "config":{"commit_strategy":"manual"}
        }))).unwrap();
        let peer_tx = server_tx.clone();
        let mut sink = VecSink::new();
        sink.on_send = Some(Box::new(move |message| {
            if let Message::Text(text) = message {
                let value: Value = serde_json::from_str(text).unwrap();
                if value["commit"] == true {
                    peer_tx.send(server_frame(json!({
                        "message_type":"committed_transcript", "text":"Again."
                    }))).unwrap();
                }
            }
        }));
        let text = Arc::new(Mutex::new(String::new()));
        let chunks = Arc::new(Mutex::new(Vec::<String>::new()));
        let chunks_cb = chunks.clone();
        tokio::time::timeout(Duration::from_secs(3),
            ElevenLabsRealtimeManager::drive_session_loop(
                &mut sink, &mut stream, audio_rx, control_rx, text.clone(),
                Some(Arc::new(move |chunk| chunks_cb.lock().push(chunk))),
                "secret".into(), false, vec!["AivoRelay".into()], noop_session_events(),
            )).await.unwrap().unwrap();
        let mut output = Vec::new();
        let mut segment_bytes = 0;
        let mut commits = 0;
        let mut terms = 0;
        for message in &sink.sent {
            if let Message::Text(raw) = message {
                let value: Value = serde_json::from_str(raw).unwrap();
                let pcm = BASE64_STANDARD.decode(value["audio_base_64"].as_str().unwrap()).unwrap();
                if value["commit"] == true {
                    assert!(segment_bytes > 0 && segment_bytes <= HARD_COMMIT_PCM_BYTES);
                    commits += 1;
                    segment_bytes = 0;
                } else {
                    assert!(pcm.len() <= MAX_AUDIO_MESSAGE_PCM_BYTES);
                    segment_bytes += pcm.len();
                    output.extend(pcm);
                    terms += usize::from(value.get("keyterm_library").is_some());
                }
            }
        }
        assert_eq!(output, input, "no dropped, duplicated or generated PCM");
        assert_eq!(segment_bytes, 0);
        assert_eq!(commits, 5);
        assert_eq!(terms, 1);
        assert_eq!(*text.lock(), "Again. Again. Again. Again. Again.");
        assert_eq!(chunks.lock().concat(), *text.lock());
        assert!(sink.closed);
        drop(server_tx);
    }

    #[tokio::test]
    async fn stop_waits_for_inflight_commit_before_sending_queued_tail_audio() {
        let (audio_tx, audio_rx) = mpsc::channel(4);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        audio_tx.send(vec![1; HARD_COMMIT_PCM_BYTES]).await.unwrap();
        audio_tx.send(vec![2; 320]).await.unwrap();
        drop(audio_tx);
        let (server_tx, mut stream) = scripted_stream();
        let (sent_tx, mut sent_rx) = mpsc::unbounded_channel();
        let mut sink = VecSink::new();
        sink.on_send = Some(Box::new(move |message| { sent_tx.send(message.clone()).unwrap(); }));
        let peer = tokio::spawn(async move {
            let mut audio_bytes = 0;
            let mut commits = 0;
            while let Some(message) = sent_rx.recv().await {
                let Message::Text(raw) = message else { continue };
                let value: Value = serde_json::from_str(&raw).unwrap();
                if value["commit"] == true {
                    commits += 1;
                    if commits == 1 {
                        assert_eq!(audio_bytes, HARD_COMMIT_PCM_BYTES);
                        control_tx.send(ControlMessage::Finish).unwrap();
                        // Give the client a chance to incorrectly send the tail
                        // or a second commit while the first acknowledgement waits.
                        tokio::task::yield_now().await;
                        tokio::task::yield_now().await;
                        assert!(matches!(sent_rx.try_recv(), Err(mpsc::error::TryRecvError::Empty)));
                    }
                    server_tx.send(server_frame(json!({
                        "message_type":"committed_transcript", "text":format!("Part{commits}.")
                    }))).unwrap();
                    if commits == 2 { break; }
                } else {
                    audio_bytes += BASE64_STANDARD.decode(value["audio_base_64"].as_str().unwrap()).unwrap().len();
                }
            }
            assert_eq!(commits, 2);
            assert_eq!(audio_bytes, HARD_COMMIT_PCM_BYTES + 320);
            // Keep server open until the client has accepted the acknowledgement.
            std::future::pending::<()>().await;
        });
        let text = Arc::new(Mutex::new(String::new()));
        tokio::time::timeout(Duration::from_secs(2),
            ElevenLabsRealtimeManager::drive_session_loop(
                &mut sink, &mut stream, audio_rx, control_rx, text.clone(), None,
                "secret".into(), false, Vec::new(), noop_session_events(),
            )).await.unwrap().unwrap();
        assert_eq!(*text.lock(), "Part1. Part2.");
        peer.abort();
        let error = peer.await.unwrap_err();
        assert!(error.is_cancelled(), "peer assertions must succeed");
    }

    #[tokio::test]
    async fn diarized_commit_deduplicates_text_and_waits_for_matching_speaker_packet() {
        let (audio_tx, audio_rx) = mpsc::channel(1);
        let (_control_tx, control_rx) = mpsc::unbounded_channel();
        audio_tx.send(vec![1, 2]).await.unwrap();
        drop(audio_tx);
        let (server_tx, mut stream) = scripted_stream();
        let peer_tx = server_tx.clone();
        let mut sink = VecSink::new();
        sink.on_send = Some(Box::new(move |message| {
            if let Message::Text(raw) = message {
                let value: Value = serde_json::from_str(raw).unwrap();
                if value["commit"] == true {
                    for event in [
                        json!({"message_type":"committed_transcript","segment_index":4,"text":"Hello."}),
                        json!({"message_type":"committed_transcript","segment_index":4,"text":"Hello."}),
                        json!({"message_type":"diarized_transcript","segment_index":3,"text":"Old.","words":[]}),
                        json!({"message_type":"diarized_transcript","segment_index":4,"text":"Hello.","words":[]}),
                    ] { peer_tx.send(server_frame(event)).unwrap(); }
                }
            }
        }));
        let text = Arc::new(Mutex::new(String::new()));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_cb = seen.clone();
        let mut events = noop_session_events();
        events.on_diarized = Box::new(move |index, _, _| seen_cb.lock().push(index));
        tokio::time::timeout(Duration::from_secs(1),
            ElevenLabsRealtimeManager::drive_session_loop(
                &mut sink, &mut stream, audio_rx, control_rx, text.clone(), None,
                "secret".into(), true, Vec::new(), events,
            )).await.unwrap().unwrap();
        assert_eq!(*text.lock(), "Hello.");
        assert_eq!(*seen.lock(), vec![Some(3), Some(4)]);
        drop(server_tx);
    }

    #[tokio::test]
    async fn empty_recording_closes_without_commit_or_fake_audio() {
        let (audio_tx, audio_rx) = mpsc::channel(1);
        drop(audio_tx);
        let (_control_tx, control_rx) = mpsc::unbounded_channel();
        let mut sink = VecSink::new();
        let mut stream = futures_util::stream::pending();
        tokio::time::timeout(Duration::from_secs(1),
            ElevenLabsRealtimeManager::drive_session_loop(
                &mut sink, &mut stream, audio_rx, control_rx, Arc::new(Mutex::new(String::new())),
                None, "secret".into(), false, Vec::new(), noop_session_events(),
            )).await.unwrap().unwrap();
        assert!(sink.sent.is_empty());
        assert!(sink.closed);
    }

    #[tokio::test]
    async fn finalization_reports_missing_ack_missing_diarization_and_provider_errors() {
        for (diarize, response, expected) in [
            (false, json!({"message_type":"partial_transcript","text":"Unfinished"}), "commit acknowledgement"),
            (true, json!({"message_type":"committed_transcript","text":"Hello","segment_index":0}), "matching diarized"),
            (false, json!({"message_type":"commit_throttled","error":"Too soon"}), "commit_throttled"),
            (false, json!({"message_type":"auth_error","error":"Rejected secret"}), "auth_error"),
        ] {
            let (audio_tx, audio_rx) = mpsc::channel(1);
            audio_tx.send(vec![1, 2]).await.unwrap();
            drop(audio_tx);
            let (_control_tx, control_rx) = mpsc::unbounded_channel();
            let (server_tx, mut stream) = scripted_stream();
            let peer_tx = server_tx.clone();
            let mut sink = VecSink::new();
            sink.on_send = Some(Box::new(move |message| {
                if let Message::Text(raw) = message {
                    let value: Value = serde_json::from_str(raw).unwrap();
                    if value["commit"] == true {
                        peer_tx.send(server_frame(response.clone())).unwrap();
                    }
                }
            }));
            let text = Arc::new(Mutex::new(String::new()));
            let error = tokio::time::timeout(Duration::from_secs(1),
                ElevenLabsRealtimeManager::drive_session_loop_with_timeouts(
                    &mut sink, &mut stream, audio_rx, control_rx, text.clone(), None,
                    "secret".into(), diarize, Vec::new(), noop_session_events(),
                    Duration::from_millis(100), Duration::from_millis(100),
                )).await.unwrap().unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("secret"));
            if !diarize { assert!(text.lock().is_empty(), "never promote provisional text"); }
            assert!(sink.closed);
            drop(server_tx);
        }
    }

    #[tokio::test]
    async fn cancel_or_overflow_while_awaiting_commit_never_sends_queued_tail() {
        for overflow in [false, true] {
            let (audio_tx, audio_rx) = mpsc::channel(2);
            audio_tx.send(vec![1; HARD_COMMIT_PCM_BYTES + 320]).await.unwrap();
            drop(audio_tx);
            let (control_tx, control_rx) = mpsc::unbounded_channel();
            let mut sink = VecSink::new();
            sink.on_send = Some(Box::new(move |message| {
                if let Message::Text(raw) = message {
                    let value: Value = serde_json::from_str(raw).unwrap();
                    if value["commit"] == true {
                        control_tx.send(if overflow {
                            ControlMessage::Overflow("queue full".into())
                        } else { ControlMessage::Cancel }).unwrap();
                    }
                }
            }));
            let mut stream = futures_util::stream::pending();
            let text = Arc::new(Mutex::new(String::new()));
            let result = tokio::time::timeout(Duration::from_secs(1),
                ElevenLabsRealtimeManager::drive_session_loop(
                    &mut sink, &mut stream, audio_rx, control_rx, text.clone(), None,
                    "secret".into(), false, Vec::new(), noop_session_events(),
                )).await.unwrap();
            assert_eq!(result.is_err(), overflow);
            assert!(text.lock().is_empty());
            let pcm_bytes: usize = sink.sent.iter().filter_map(|message| {
                let Message::Text(raw) = message else { return None };
                let value: Value = serde_json::from_str(raw).unwrap();
                Some(BASE64_STANDARD.decode(value["audio_base_64"].as_str().unwrap()).unwrap().len())
            }).sum();
            assert_eq!(pcm_bytes, HARD_COMMIT_PCM_BYTES);
            assert!(sink.closed);
        }
    }

    #[test]
    fn turn_commit_requires_enough_real_audio_and_a_completed_turn() {
        let mut state = ManualSessionState::new(false);
        state.active_segment_pcm_bytes = SOFT_COMMIT_MIN_PCM_BYTES - 2;
        state.has_nonempty_speech_partial = true;
        state.latest_turn_probability = Some(0.99);
        assert!(!state.should_send_intermediate_commit(false));
        state.active_segment_pcm_bytes += 2;
        assert!(state.should_send_intermediate_commit(false));
        state.latest_turn_probability = Some(0.5);
        assert!(!state.should_send_intermediate_commit(false));
        state.active_segment_pcm_bytes = HARD_COMMIT_PCM_BYTES;
        assert!(state.should_send_intermediate_commit(false));
        state.pending_commits = 1;
        assert!(!state.should_send_intermediate_commit(false));
    }

    #[tokio::test]
    async fn rejects_conflicting_strategy_unsolicited_commit_and_premature_close() {
        for frame in [
            server_frame(json!({"message_type":"session_started","config":{"commit_strategy":"vad"}})),
            server_frame(json!({"message_type":"committed_transcript","text":"Unrequested"})),
            Ok(Message::Close(None)),
        ] {
            let mut sink = VecSink::new();
            let mut state = ManualSessionState::new(false);
            let text = Arc::new(Mutex::new(String::new()));
            assert!(ElevenLabsRealtimeManager::handle_incoming_frame(
                Some(frame), &mut sink, "secret", &mut state, &text,
                &None, &noop_session_events(), false,
            ).await.is_err());
            assert!(text.lock().is_empty());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_socket_write_is_bounded() {
        struct StalledSink;
        impl Sink<Message> for StalledSink {
            type Error = tokio_tungstenite::tungstenite::Error;
            fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
            fn start_send(self: Pin<&mut Self>, _: Message) -> Result<(), Self::Error> {
                unreachable!("socket is never ready")
            }
            fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
            fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
        }
        let mut sink = StalledSink;
        let error = send_socket_frame(&mut sink, Message::Text("audio".into())).await.unwrap_err();
        assert!(error.to_string().contains("Timed out writing"));
        // A failed connection must not block cleanup either.
        close_socket(&mut sink).await;
    }
}
