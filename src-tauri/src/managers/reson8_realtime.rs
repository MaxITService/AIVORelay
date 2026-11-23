use anyhow::{anyhow, Result};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use log::{info, warn};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::managers::provider_error::parse_provider_error_value;
use crate::managers::remote_stt::{redact_remote_stt_api_key, resolve_reson8_language};
use crate::url_security::{
    REMOTE_STT_PRESET_RESON8, REMOTE_STT_RESON8_BASE_URL, REMOTE_STT_RESON8_DEFAULT_MODEL,
};

pub const RESON8_LIVE_MODEL: &str = REMOTE_STT_RESON8_DEFAULT_MODEL;
pub const RESON8_LIVE_FINALIZE_TIMEOUT_MS: u32 = 8_000;

const RESON8_LIVE_WS_PATH: &str = "/speech-to-text/realtime";
const RESON8_SAMPLE_RATE_HZ: u32 = 16_000;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const AUDIO_QUEUE_CAPACITY: usize = 512;

static FLUSH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub type FinalChunkCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Default)]
struct SessionLease(AtomicBool);

impl SessionLease {
    fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn allows_delivery(&self) -> bool {
        !self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub struct Reson8RealtimeOptions {
    pub model: String,
    pub language: Option<String>,
    pub preset: String,
    pub include_interim: bool,
    pub show_preview: bool,
}

impl Default for Reson8RealtimeOptions {
    fn default() -> Self {
        Self {
            model: RESON8_LIVE_MODEL.to_string(),
            language: None,
            preset: REMOTE_STT_PRESET_RESON8.to_string(),
            include_interim: false,
            show_preview: false,
        }
    }
}

#[derive(Debug)]
enum ControlMessage {
    Finish,
    Cancel,
    Overflow(String),
}

struct SessionEvents {
    on_interim: Box<dyn Fn(&str) + Send + Sync>,
    on_final: Box<dyn Fn(&str, &str, &str) + Send + Sync>,
}

impl SessionEvents {
    fn only_current(self, is_current: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        let current = Arc::new(is_current);
        let interim = Arc::clone(&current);
        Self {
            on_interim: Box::new(move |text| {
                if interim() {
                    (self.on_interim)(text);
                }
            }),
            on_final: Box::new(move |delta, raw, accumulated| {
                if current() {
                    (self.on_final)(delta, raw, accumulated);
                }
            }),
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

pub(crate) struct Reson8FinalizingSession {
    app_handle: AppHandle,
    session: Option<ActiveSession>,
}

#[derive(Clone)]
struct SessionParams {
    binding_id: String,
    operation_id: Option<u64>,
    api_key: String,
    options: Reson8RealtimeOptions,
    on_final_chunk: Option<FinalChunkCallback>,
}

pub struct Reson8RealtimeManager {
    app_handle: AppHandle,
    active_session: Arc<Mutex<Option<ActiveSession>>>,
    // Always lock active_session before pending_audio so cancellation/startup
    // cannot mix two operations' buffered frames.
    pending_audio: Mutex<Option<PendingAudio>>,
    reported_runtime_error: Mutex<Arc<Mutex<Option<String>>>>,
}

impl Reson8RealtimeManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle: app_handle.clone(),
            active_session: Arc::new(Mutex::new(None)),
            pending_audio: Mutex::new(Some(PendingAudio {
                operation_id: None,
                frames: Vec::new(),
                overflow_error: None,
            })),
            reported_runtime_error: Mutex::new(Arc::new(Mutex::new(None))),
        })
    }

    pub fn is_realtime_model(model: &str) -> bool {
        model.trim().eq_ignore_ascii_case(RESON8_LIVE_MODEL)
    }

    fn start_error(&self, message: impl Into<String>) -> anyhow::Error {
        let message = message.into();
        crate::managers::remote_stt::record_external_remote_stt_debug(
            &self.app_handle,
            format!("Reson8 live startup failed: {message}"),
            true,
        );
        anyhow!(message)
    }

    pub fn prepare_session(&self, operation_id: Option<u64>) {
        let old_session = {
            let mut active = self.active_session.lock();
            let mut pending = self.pending_audio.lock();
            *pending = Some(PendingAudio {
                operation_id,
                frames: Vec::new(),
                overflow_error: None,
            });
            active.take()
        };
        if let Some(session) = old_session {
            session.lease.cancel();
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
            hide_preview_if_needed(
                &self.app_handle,
                session.show_preview,
                Some(&session.binding_id),
            );
        }
    }

    pub fn restart_session(&self, reset_preview: impl FnOnce()) -> Result<()> {
        let params = {
            let state = self
                .app_handle
                .state::<crate::session_manager::ManagedSessionState>();
            let owner =
                crate::session_manager::lock_session_state(&state, "restart Reson8 session");
            let mut active = self.active_session.lock();
            let mut pending = self.pending_audio.lock();
            prepare_session_restart(&mut active, &mut pending, |id, binding| {
                id.is_some_and(|id| {
                    matches!(
                        &*owner,
                        crate::session_manager::SessionState::Recording {
                            operation_id,
                            binding_id,
                            ..
                        } if *operation_id == id && binding_id == binding
                    )
                })
            })
        };
        reset_preview();
        if let Some(p) = params {
            self.start_session(
                &p.binding_id,
                p.operation_id,
                &p.api_key,
                p.options,
                p.on_final_chunk,
            )?;
        }
        Ok(())
    }

    pub(crate) async fn transcribe_history_replay(
        &self,
        samples: &[f32],
        api_key: &str,
        mut options: Reson8RealtimeOptions,
        replay: &Arc<super::history_replay::HistoryReplay>,
    ) -> Result<String> {
        options.include_interim = false;
        options.show_preview = false;
        validate_options(api_key, &options)?;
        let ws_request = build_realtime_ws_request(api_key, &options)?;
        let stream = timeout(
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            connect_async(ws_request),
        )
        .await
        .map_err(|_| anyhow!("Timed out while connecting to Reson8 WebSocket"))?
        .map_err(|error| anyhow!(format_handshake_error(error, api_key)))?
        .0;

        let (write, mut read) = stream.split();
        let mut write = super::history_replay::ReplaySink::new(write, replay, RESON8_SAMPLE_RATE_HZ);
        let (audio_tx, audio_rx) = mpsc::channel(1);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        let flush_id = next_flush_id();
        let api_key_for_loop = api_key.trim().to_string();

        let drive = Self::drive_session_loop(
            &mut write,
            &mut read,
            audio_rx,
            control_rx,
            Arc::clone(&final_text),
            None,
            api_key_for_loop,
            flush_id,
            SessionEvents {
                on_interim: Box::new(|_| {}),
                on_final: Box::new(|_, _, _| {}),
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
                        .map_err(|_| anyhow!("Reson8 live session closed"))
                },
                drive,
                final_text,
                RESON8_LIVE_FINALIZE_TIMEOUT_MS,
            )
            .await
    }

    pub fn start_session(
        &self,
        binding_id: &str,
        operation_id: Option<u64>,
        api_key: &str,
        options: Reson8RealtimeOptions,
        on_final_chunk: Option<FinalChunkCallback>,
    ) -> Result<()> {
        validate_options(api_key, &options).map_err(|err| self.start_error(err.to_string()))?;

        let ws_request = build_realtime_ws_request(api_key, &options)
            .map_err(|err| self.start_error(redact_remote_stt_api_key(&err.to_string(), api_key)))?;

        let state = self
            .app_handle
            .state::<crate::session_manager::ManagedSessionState>();
        let owner = operation_id.map(|_| {
            crate::session_manager::lock_session_state(&state, "publish Reson8 session")
        });
        if let Some(id) = operation_id {
            if !owner.as_ref().is_some_and(|owner| {
                matches!(
                    &**owner,
                    crate::session_manager::SessionState::Recording {
                        operation_id,
                        binding_id: binding,
                        ..
                    } if *operation_id == id && binding == binding_id
                )
            }) {
                return Err(self.start_error("Reson8 recording operation is no longer active"));
            }
        }

        let mut active_session_guard = self.active_session.lock();
        let mut pending_audio = self.pending_audio.lock();
        if let Some(pending) = pending_audio.as_ref() {
            if pending.operation_id != operation_id {
                return Err(self.start_error("Reson8 recording operation is no longer active"));
            }
            if let Some(overflow_error) = pending.overflow_error.as_deref() {
                return Err(self.start_error(overflow_error));
            }
        } else if operation_id.is_some() {
            return Err(self.start_error("Reson8 recording operation is no longer active"));
        }

        if active_session_guard.is_some() {
            return Err(self.start_error(
                "Reson8 live session is already active for this profile",
            ));
        }

        let show_preview = options.show_preview;
        crate::managers::remote_stt::record_external_remote_stt_debug(
            &self.app_handle,
            format!(
                "Reson8 live session start model={} language={} include_interim={} binding={}",
                RESON8_LIVE_MODEL,
                resolve_reson8_language(options.language.as_deref())
                    .as_deref()
                    .unwrap_or("auto"),
                options.include_interim,
                binding_id,
            ),
            false,
        );

        let lease = Arc::new(SessionLease::default());
        let restart_params = Some(SessionParams {
            binding_id: binding_id.to_string(),
            operation_id,
            api_key: api_key.to_string(),
            options: options.clone(),
            on_final_chunk: on_final_chunk.clone(),
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
        let session_audio_tx = audio_tx.clone();
        let session_control_tx = control_tx.clone();
        let reported_runtime_error = Arc::new(Mutex::new(None));
        *self.reported_runtime_error.lock() = Arc::clone(&reported_runtime_error);
        let flush_id = next_flush_id();

        let buffered = pending_audio
            .take()
            .map(|pending| pending.frames)
            .unwrap_or_default();
        if buffered.len() > AUDIO_QUEUE_CAPACITY {
            return Err(self.start_error(
                "Reson8 live audio queue overflowed before session startup; aborting to avoid losing speech.",
            ));
        }

        let lease_for_task = Arc::clone(&lease);
        let join_handle = tauri::async_runtime::spawn(async move {
            let session_started = Instant::now();
            let session_result: Result<()> = async {
                let (stream, _) = timeout(
                    Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
                    connect_async(ws_request),
                )
                .await
                .map_err(|_| anyhow!("Timed out while connecting to Reson8 WebSocket"))?
                .map_err(|error| anyhow!(format_handshake_error(error, &api_key_for_task)))?;

                crate::managers::remote_stt::record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!("Reson8 live connected binding={}", binding_id_for_task),
                    false,
                );

                let (mut write, mut read) = stream.split();
                let events = build_runtime_session_events(
                    app_handle_for_task.clone(),
                    binding_id_for_task.clone(),
                    live_sound_session_id,
                    show_preview,
                )
                .only_current({
                    let app = app_handle_for_task.clone();
                    let lease = Arc::clone(&lease_for_task);
                    move || {
                        lease.allows_delivery()
                            && session_callback_is_current(&app, operation_id, live_sound_session_id)
                    }
                });
                Self::drive_session_loop(
                    &mut write,
                    &mut read,
                    audio_rx,
                    control_rx,
                    final_text_for_task,
                    on_final_chunk,
                    api_key_for_task.clone(),
                    flush_id,
                    events,
                )
                .await
            }
            .await;

            if let Err(err) = &session_result {
                let err_str = redact_remote_stt_api_key(&err.to_string(), &api_key_for_task);
                crate::managers::remote_stt::record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!(
                        "Reson8 live runtime failed binding={} elapsed_ms={} error={}",
                        binding_id_for_task,
                        session_started.elapsed().as_millis(),
                        err_str,
                    ),
                    true,
                );
                warn!(
                    "Reson8 live session runtime error (binding='{}'): {}",
                    binding_id_for_task, err_str
                );
                let callback_is_current = lease_for_task.allows_delivery()
                    && session_callback_is_current(
                        &app_handle_for_task,
                        operation_id,
                        live_sound_session_id,
                    );
                if callback_is_current {
                    *reported_runtime_error.lock() = Some(err_str.clone());
                }
                if callback_is_current && live_sound_session_id.is_none() {
                    crate::actions::stop_transcription_after_realtime_error(
                        &app_handle_for_task,
                        &binding_id_for_task,
                        operation_id,
                    );
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
                crate::managers::remote_stt::record_external_remote_stt_debug(
                    &app_handle_for_task,
                    format!(
                        "Reson8 live session ended binding={} elapsed_ms={}",
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
            show_preview,
            audio_tx,
            control_tx,
            final_text,
            join_handle,
            lease,
            restart_params,
        };
        *active_session_guard = Some(active);

        for chunk in buffered {
            if let Err(err) = session_audio_tx.try_send(chunk) {
                if matches!(err, mpsc::error::TrySendError::Full(_)) {
                    let _ = session_control_tx.send(ControlMessage::Overflow(
                        "Reson8 live audio queue overflowed; aborting to avoid losing speech."
                            .to_string(),
                    ));
                }
                break;
            }
        }
        drop(pending_audio);
        drop(active_session_guard);
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

        info!("Started Reson8 live session for binding '{}'", binding_id);
        Ok(())
    }

    pub fn has_active_session(&self) -> bool {
        self.active_session.lock().is_some()
    }

    pub fn take_reported_runtime_error(&self, error: &str) -> bool {
        let reported = Arc::clone(&self.reported_runtime_error.lock());
        let mut reported = reported.lock();
        if reported.as_deref() != Some(error) {
            return false;
        }
        *reported = None;
        true
    }

    pub fn clear_reported_runtime_error(&self) {
        let reported = Arc::clone(&self.reported_runtime_error.lock());
        *reported.lock() = None;
    }

    #[allow(clippy::too_many_arguments)]
    async fn drive_session_loop<S, R>(
        write: &mut S,
        read: &mut R,
        mut audio_rx: mpsc::Receiver<Vec<u8>>,
        mut control_rx: mpsc::UnboundedReceiver<ControlMessage>,
        final_text: Arc<Mutex<String>>,
        on_final_chunk: Option<FinalChunkCallback>,
        api_key: String,
        flush_id: String,
        events: SessionEvents,
    ) -> Result<()>
    where
        S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let mut audio_input_closed = false;
        let mut flush_sent = false;
        let mut accumulated_text = String::new();
        let mut latest_interim = String::new();

        loop {
            if audio_input_closed && !flush_sent {
                let flush_payload = build_flush_request_payload(&flush_id);
                write
                    .send(Message::Text(flush_payload.to_string().into()))
                    .await
                    .map_err(|error| {
                        anyhow!(redact_remote_stt_api_key(
                            &format!("Failed to send Reson8 flush_request: {error}"),
                            &api_key,
                        ))
                    })?;
                write.flush().await.map_err(|error| {
                    anyhow!(redact_remote_stt_api_key(
                        &format!("Failed to flush Reson8 WebSocket stream: {error}"),
                        &api_key,
                    ))
                })?;
                flush_sent = true;
            }

            tokio::select! {
                Some(control) = control_rx.recv() => {
                    match control {
                        ControlMessage::Finish => {}
                        ControlMessage::Cancel => {
                            let _ = write.close().await;
                            return Ok(());
                        }
                        ControlMessage::Overflow(message) => {
                            let _ = write.close().await;
                            return Err(anyhow!(message));
                        }
                    }
                }
                audio_chunk = audio_rx.recv(), if !audio_input_closed => {
                    match audio_chunk {
                        Some(audio_chunk) if !audio_chunk.is_empty() => {
                            write
                                .send(Message::Binary(audio_chunk.into()))
                                .await
                                .map_err(|error| {
                                    anyhow!(redact_remote_stt_api_key(
                                        &format!("Failed to send audio chunk to Reson8: {error}"),
                                        &api_key,
                                    ))
                                })?;
                        }
                        Some(_) => {}
                        None => {
                            audio_input_closed = true;
                        }
                    }
                }
                frame = read.next() => {
                    let frame = match frame {
                        Some(frame) => frame.map_err(|error| {
                            anyhow!(redact_remote_stt_api_key(
                                &format!("Reson8 WebSocket read failed: {error}"),
                                &api_key,
                            ))
                        })?,
                        None => {
                            return Err(anyhow!(
                                "Reson8 WebSocket closed before flush confirmation was received"
                            ));
                        }
                    };

                    match frame {
                        Message::Text(text) => {
                            let payload: Value = serde_json::from_str(text.as_ref()).map_err(|error| {
                                let preview: String = redact_remote_stt_api_key(text.as_ref(), &api_key)
                                    .chars()
                                    .take(200)
                                    .collect();
                                anyhow!("Invalid Reson8 WebSocket payload: {error} (body: {preview})")
                            })?;

                            match parse_server_event(&payload, &flush_id, &api_key)? {
                                ServerEvent::InterimTranscript(raw_text) => {
                                    let trimmed = raw_text.trim();
                                    if !trimmed.is_empty() || !latest_interim.is_empty() {
                                        latest_interim = trimmed.to_string();
                                        (events.on_interim)(&raw_text);
                                    }
                                }
                                ServerEvent::FinalTranscript(raw_text) => {
                                    let had_interim = !latest_interim.is_empty();
                                    latest_interim.clear();
                                    if let Some(delta) =
                                        normalize_reson8_chunk(&accumulated_text, &raw_text)
                                    {
                                        accumulated_text.push_str(&delta);
                                        *final_text.lock() = accumulated_text.clone();
                                        if let Some(callback) = &on_final_chunk {
                                            callback(delta.clone());
                                        }
                                        (events.on_final)(&delta, &raw_text, &accumulated_text);
                                    } else if had_interim {
                                        (events.on_final)("", "", &accumulated_text);
                                    }
                                }
                                ServerEvent::FlushConfirmed => {
                                    if flush_sent {
                                        if !latest_interim.is_empty() {
                                            latest_interim.clear();
                                            (events.on_interim)("");
                                        }
                                        break;
                                    }
                                }
                                ServerEvent::Ignore => {}
                            }
                        }
                        Message::Ping(payload) => {
                            write
                                .send(Message::Pong(payload))
                                .await
                                .map_err(|error| {
                                    anyhow!(redact_remote_stt_api_key(
                                        &format!("Failed to respond to Reson8 ping: {error}"),
                                        &api_key,
                                    ))
                                })?;
                        }
                        Message::Pong(_) | Message::Binary(_) => {}
                        Message::Close(_) => {
                            return Err(anyhow!(
                                "Reson8 WebSocket closed before flush confirmation was received"
                            ));
                        }
                        _ => {}
                    }
                }
            }
        }

        let _ = write.close().await;
        Ok(())
    }

    pub fn push_audio_frame(&self, frame_16khz_mono: Vec<f32>) {
        self.push_audio_frame_for_operation(None, frame_16khz_mono);
    }

    pub fn push_audio_frame_for_operation(
        &self,
        operation_id: Option<u64>,
        frame_16khz_mono: Vec<f32>,
    ) {
        let bytes = frame_16khz_mono_to_pcm_s16le_bytes(&frame_16khz_mono);
        let active_handles = {
            let active = self.active_session.lock();
            active
                .as_ref()
                .filter(|session| session.operation_id == operation_id)
                .map(|session| (session.audio_tx.clone(), session.control_tx.clone()))
        };

        let Some((sender, control_tx)) = active_handles else {
            let mut pending = self.pending_audio.lock();
            let Some(pending) = pending
                .as_mut()
                .filter(|pending| pending.operation_id == operation_id)
            else {
                return;
            };
            if pending.overflow_error.is_some() {
                return;
            }
            if pending.frames.len() >= AUDIO_QUEUE_CAPACITY {
                pending.overflow_error = Some(
                    "Reson8 live audio buffer overflowed before connection completed; aborting to avoid losing speech."
                        .to_string(),
                );
                return;
            }
            pending.frames.push(bytes);
            return;
        };

        if let Err(mpsc::error::TrySendError::Full(_)) = sender.try_send(bytes) {
            warn!("Reson8 live audio queue overflowed; aborting session to avoid dropping speech");
            let _ = control_tx.send(ControlMessage::Overflow(
                "Reson8 live audio queue overflowed; recording aborted to avoid losing speech."
                    .to_string(),
            ));
        }
    }

    pub(crate) fn begin_finish_session_if_matches(
        &self,
        operation_id: Option<u64>,
    ) -> Reson8FinalizingSession {
        let mut active = self.active_session.lock();
        let mut pending = self.pending_audio.lock();
        let session = detach_session_if_matches(&mut active, &mut pending, operation_id);
        Reson8FinalizingSession {
            app_handle: self.app_handle.clone(),
            session,
        }
    }

    pub async fn finish_session(&self, timeout_ms: u32) -> Result<String> {
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

    pub fn cancel_if_matches(&self, operation_id: Option<u64>) {
        let finalization = self.begin_finish_session_if_matches(operation_id);
        if let Some(session) = finalization.session {
            session.lease.cancel();
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
            hide_preview_if_needed(
                &self.app_handle,
                session.show_preview,
                Some(&session.binding_id),
            );
        }
    }

    pub fn cancel(&self) {
        let session = {
            let mut active = self.active_session.lock();
            let session = active.take();
            *self.pending_audio.lock() = None;
            session
        };
        if let Some(session) = session {
            session.lease.cancel();
            let _ = session.control_tx.send(ControlMessage::Cancel);
            session.join_handle.abort();
            hide_preview_if_needed(
                &self.app_handle,
                session.show_preview,
                Some(&session.binding_id),
            );
        }
    }
}

impl Reson8FinalizingSession {
    pub(crate) async fn finish(self, timeout_ms: u32) -> Result<String> {
        let Some(session) = self.session else {
            return Ok(String::new());
        };

        let ActiveSession {
            binding_id,
            operation_id,
            show_preview,
            audio_tx,
            control_tx,
            final_text,
            mut join_handle,
            lease,
            ..
        } = session;
        let hide_preview = || {
            if lease.allows_delivery()
                && session_callback_is_current(&self.app_handle, operation_id, None)
            {
                hide_preview_if_needed(&self.app_handle, show_preview, Some(&binding_id));
            }
        };
        let read_final_text = || -> String { final_text.lock().trim().to_string() };

        let _ = control_tx.send(ControlMessage::Finish);
        drop(audio_tx);

        let wait_ms = timeout_ms.clamp(500, 20_000) as u64;
        let join_result = timeout(Duration::from_millis(wait_ms), &mut join_handle).await;
        match join_result {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => {
                warn!(
                    "Reson8 live session ended with error before flush confirmation (binding='{}'): {}",
                    binding_id, error
                );
                hide_preview();
                return Err(error);
            }
            Ok(Err(error)) => {
                hide_preview();
                return Err(anyhow!("Reson8 live session task failed: {error}"));
            }
            Err(_) => {
                join_handle.abort();
                hide_preview();
                return Err(anyhow!(
                    "Timed out while waiting for Reson8 flush confirmation"
                ));
            }
        }

        let text = read_final_text();
        hide_preview();
        info!(
            "Completed Reson8 live session for binding '{}', output_len={}",
            binding_id,
            text.len()
        );
        Ok(text)
    }
}

enum ServerEvent {
    InterimTranscript(String),
    FinalTranscript(String),
    FlushConfirmed,
    Ignore,
}

fn next_flush_id() -> String {
    let seq = FLUSH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("reson8-flush-{seq}")
}

fn build_flush_request_payload(flush_id: &str) -> Value {
    json!({
        "type": "flush_request",
        "id": flush_id,
    })
}

fn parse_server_event(payload: &Value, expected_flush_id: &str, api_key: &str) -> Result<ServerEvent> {
    let msg_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match msg_type {
        "transcript" => {
            let text = payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if payload.get("is_final").and_then(Value::as_bool) == Some(false) {
                Ok(ServerEvent::InterimTranscript(text.to_string()))
            } else {
                Ok(ServerEvent::FinalTranscript(text.to_string()))
            }
        }
        "flush_confirmation" => {
            let confirmed_id = payload
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if confirmed_id == expected_flush_id {
                Ok(ServerEvent::FlushConfirmed)
            } else {
                Ok(ServerEvent::Ignore)
            }
        }
        "error" => {
            let raw_message =
                parse_provider_error_value(payload, "Reson8 realtime returned an error");
            Err(anyhow!(redact_remote_stt_api_key(&raw_message, api_key)))
        }
        _ => Ok(ServerEvent::Ignore),
    }
}

fn validate_options(api_key: &str, options: &Reson8RealtimeOptions) -> Result<()> {
    if api_key.trim().is_empty() {
        return Err(anyhow!("Reson8 API key is missing"));
    }
    if options.preset != REMOTE_STT_PRESET_RESON8 {
        return Err(anyhow!(
            "Reson8 live transcription requires the canonical Reson8 preset"
        ));
    }
    if !Reson8RealtimeManager::is_realtime_model(&options.model) {
        return Err(anyhow!(
            "Reson8 route only supports model '{}', got '{}'",
            RESON8_LIVE_MODEL,
            options.model.trim()
        ));
    }
    Ok(())
}

fn build_realtime_ws_url(options: &Reson8RealtimeOptions) -> Result<String> {
    let ws_base = REMOTE_STT_RESON8_BASE_URL
        .strip_prefix("https://")
        .map(|rest| format!("wss://{rest}{RESON8_LIVE_WS_PATH}"))
        .ok_or_else(|| anyhow!("Invalid Reson8 base URL"))?;
    let mut url = reqwest::Url::parse(&ws_base)
        .map_err(|error| anyhow!("Invalid Reson8 WebSocket URL: {error}"))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("model", RESON8_LIVE_MODEL);
        query.append_pair("encoding", "pcm_s16le");
        query.append_pair("sample_rate", &RESON8_SAMPLE_RATE_HZ.to_string());
        query.append_pair("channels", "1");
        query.append_pair(
            "include_interim",
            if options.include_interim { "true" } else { "false" },
        );
        if let Some(language) = resolve_reson8_language(options.language.as_deref()) {
            query.append_pair("language", &language);
        }
    }
    Ok(url.to_string())
}

fn build_realtime_ws_request(
    api_key: &str,
    options: &Reson8RealtimeOptions,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>> {
    let ws_url = build_realtime_ws_url(options)?;
    let mut request = ws_url
        .into_client_request()
        .map_err(|error| anyhow!("Failed to create Reson8 WebSocket request: {error}"))?;
    request.headers_mut().insert(
        "Authorization",
        format!("ApiKey {}", api_key.trim())
            .parse()
            .map_err(|error| anyhow!("Invalid Reson8 auth header: {error}"))?,
    );
    Ok(request)
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
            if let Some(header_msg) = header_msg {
                format!("Reson8 handshake rejected ({status}): {header_msg}")
            } else if let Some(body) = response.body() {
                let body_text = String::from_utf8_lossy(body);
                let trimmed = body_text.trim();
                if trimmed.is_empty() {
                    format!("Reson8 handshake rejected ({status})")
                } else {
                    let snippet: String = trimmed.chars().take(300).collect();
                    format!("Reson8 handshake rejected ({status}): {snippet}")
                }
            } else {
                format!("Reson8 handshake rejected ({status})")
            }
        }
        other => format!("Failed to connect to Reson8 WebSocket: {other}"),
    };
    redact_remote_stt_api_key(&raw, api_key)
}

fn detach_session_if_matches(
    active: &mut Option<ActiveSession>,
    pending: &mut Option<PendingAudio>,
    operation_id: Option<u64>,
) -> Option<ActiveSession> {
    let session = if active
        .as_ref()
        .is_some_and(|session| session.operation_id == operation_id)
    {
        active.take()
    } else {
        None
    };
    if pending
        .as_ref()
        .is_some_and(|pending| pending.operation_id == operation_id)
    {
        *pending = None;
    }
    session
}

fn normalize_reson8_chunk(accumulated: &str, raw_chunk: &str) -> Option<String> {
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

fn session_callback_is_current(
    app: &AppHandle,
    operation_id: Option<u64>,
    live_session_id: Option<u64>,
) -> bool {
    live_session_id
        .map(crate::managers::live_sound_transcription::is_session_current)
        .or_else(|| operation_id.map(|id| crate::session_manager::is_operation_current(app, id)))
        .unwrap_or(true)
}

fn prepare_session_restart(
    active: &mut Option<ActiveSession>,
    pending: &mut Option<PendingAudio>,
    is_current: impl FnOnce(Option<u64>, &str) -> bool,
) -> Option<SessionParams> {
    let session = active.as_ref()?;
    let params = session.restart_params.clone()?;
    if params.operation_id != session.operation_id
        || pending
            .as_ref()
            .is_some_and(|pending| pending.operation_id != session.operation_id)
        || !is_current(session.operation_id, &session.binding_id)
    {
        return None;
    }
    let session = active.take()?;
    session.lease.cancel();
    let _ = session.control_tx.send(ControlMessage::Cancel);
    session.join_handle.abort();
    *pending = Some(PendingAudio {
        operation_id: params.operation_id,
        frames: Vec::new(),
        overflow_error: None,
    });
    Some(params)
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

fn build_runtime_session_events(
    app_handle: AppHandle,
    binding_id: String,
    live_sound_session_id: Option<u64>,
    show_preview: bool,
) -> SessionEvents {
    let preview_eligible = show_preview
        && binding_id != crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID
        && binding_id != super::history_replay::HISTORY_REPLAY_BINDING;
    let is_live_sound = binding_id == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID;

    let app_interim = app_handle.clone();
    let app_final = app_handle;

    SessionEvents {
        on_interim: Box::new(move |raw_text| {
            if is_live_sound {
                if let Some(session_id) = live_sound_session_id {
                    crate::managers::live_sound_transcription::set_interim_result_if_session_matches(
                        &app_interim,
                        session_id,
                        raw_text.trim().to_string(),
                        Vec::new(),
                    );
                }
                return;
            }
            if preview_eligible {
                let preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
                let preview_interim =
                    normalize_reson8_chunk(&preview_final_text, raw_text).unwrap_or_default();
                crate::overlay::emit_soniox_live_preview_update(
                    &app_interim,
                    &preview_final_text,
                    &preview_interim,
                );
            }
        }),
        on_final: Box::new(move |delta, raw_text, _accumulated| {
            if is_live_sound {
                if let Some(session_id) = live_sound_session_id {
                    let trimmed = delta.trim();
                    if !trimmed.is_empty() {
                        crate::managers::live_sound_transcription::append_final_result_if_session_matches(
                            &app_final,
                            session_id,
                            trimmed,
                            Vec::new(),
                            true,
                        );
                    }
                    crate::managers::live_sound_transcription::set_interim_result_if_session_matches(
                        &app_final,
                        session_id,
                        String::new(),
                        Vec::new(),
                    );
                }
                return;
            }
            if preview_eligible {
                let preview_final_text = crate::overlay::get_soniox_live_preview_state().final_text;
                let next_preview_final = match normalize_reson8_chunk(&preview_final_text, raw_text)
                {
                    Some(piece) => format!("{preview_final_text}{piece}"),
                    None => preview_final_text,
                };
                crate::overlay::emit_soniox_live_preview_update(
                    &app_final,
                    &next_preview_final,
                    "",
                );
            }
        }),
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
        flush_request_sent: Option<tokio::sync::oneshot::Sender<()>>,
    }

    impl VecSink {
        fn new() -> Self {
            Self {
                sent: Vec::new(),
                closed: false,
                flush_request_sent: None,
            }
        }
    }

    impl Sink<Message> for VecSink {
        type Error = tokio_tungstenite::tungstenite::Error;

        fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            self.sent.push(item);
            Ok(())
        }

        fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if matches!(self.sent.last(), Some(Message::Text(text)) if text.contains("\"flush_request\"")) {
                if let Some(sent) = self.flush_request_sent.take() {
                    sent.send(()).expect("mock peer must wait for flush_request");
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

    #[test]
    fn recognizes_only_rumble_2_and_builds_expected_ws_request() {
        assert!(Reson8RealtimeManager::is_realtime_model("rumble-2"));
        assert!(Reson8RealtimeManager::is_realtime_model(" Rumble-2 "));
        assert!(!Reson8RealtimeManager::is_realtime_model("resonant-1"));
        assert!(!Reson8RealtimeManager::is_realtime_model("rumble-2-flash"));

        let request = build_realtime_ws_request(
            " secret-reson8-key ",
            &Reson8RealtimeOptions {
                model: "rumble-2".to_string(),
                language: Some("en".to_string()),
                preset: REMOTE_STT_PRESET_RESON8.to_string(),
                include_interim: false,
                show_preview: false,
            },
        )
        .unwrap();
        let uri = request.uri().to_string();
        assert_eq!(
            uri,
            "wss://api.reson8.dev/v1/speech-to-text/realtime?model=rumble-2&encoding=pcm_s16le&sample_rate=16000&channels=1&include_interim=false&language=en"
        );
        assert!(!uri.contains("secret-reson8-key"));
        assert_eq!(
            request.headers()["authorization"],
            "ApiKey secret-reson8-key"
        );

        let interim_request = build_realtime_ws_request(
            " secret-reson8-key ",
            &Reson8RealtimeOptions {
                model: "rumble-2".to_string(),
                language: Some("en".to_string()),
                preset: REMOTE_STT_PRESET_RESON8.to_string(),
                include_interim: true,
                show_preview: true,
            },
        )
        .unwrap();
        assert_eq!(
            interim_request.uri().to_string(),
            "wss://api.reson8.dev/v1/speech-to-text/realtime?model=rumble-2&encoding=pcm_s16le&sample_rate=16000&channels=1&include_interim=true&language=en"
        );
    }

    #[test]
    fn realtime_ws_url_never_sends_auto_language() {
        let url = build_realtime_ws_url(&Reson8RealtimeOptions {
            model: "rumble-2".to_string(),
            language: Some("auto".to_string()),
            preset: REMOTE_STT_PRESET_RESON8.to_string(),
            include_interim: false,
            show_preview: false,
        })
        .unwrap();
        assert_eq!(
            url,
            "wss://api.reson8.dev/v1/speech-to-text/realtime?model=rumble-2&encoding=pcm_s16le&sample_rate=16000&channels=1&include_interim=false"
        );
    }

    #[test]
    fn parser_distinguishes_final_and_interim_transcripts_and_flush_confirmations() {
        assert!(matches!(
            parse_server_event(
                &json!({"type": "transcript", "text": "Hello world"}),
                "flush-1",
                "k"
            )
            .unwrap(),
            ServerEvent::FinalTranscript(text) if text == "Hello world"
        ));
        assert!(matches!(
            parse_server_event(
                &json!({"type": "transcript", "text": "Confirmed", "is_final": true}),
                "flush-1",
                "k"
            )
            .unwrap(),
            ServerEvent::FinalTranscript(text) if text == "Confirmed"
        ));
        assert!(matches!(
            parse_server_event(
                &json!({"type": "transcript", "text": "Interim draft", "is_final": false}),
                "flush-1",
                "k"
            )
            .unwrap(),
            ServerEvent::InterimTranscript(text) if text == "Interim draft"
        ));
        assert!(matches!(
            parse_server_event(
                &json!({"type": "flush_confirmation", "id": "other"}),
                "flush-1",
                "k"
            )
            .unwrap(),
            ServerEvent::Ignore
        ));
        assert!(matches!(
            parse_server_event(
                &json!({"type": "flush_confirmation", "id": "flush-1"}),
                "flush-1",
                "k"
            )
            .unwrap(),
            ServerEvent::FlushConfirmed
        ));
    }

    #[test]
    fn chunk_normalization_matches_accumulated_text_without_gluing_or_stripping_spaces() {
        let mut accumulated = String::new();
        let mut emitted = String::new();
        for raw in ["Hello,", "  ", "world.", "  How   are you", "?", "Good!"] {
            if let Some(delta) = normalize_reson8_chunk(&accumulated, raw) {
                accumulated.push_str(&delta);
                emitted.push_str(&delta);
            }
        }
        assert_eq!(accumulated, "Hello, world. How   are you? Good!");
        assert_eq!(emitted, accumulated);
    }

    #[tokio::test]
    async fn drive_session_loop_drains_audio_sends_flush_and_separates_interim_from_final() {
        let (audio_tx, audio_rx) = mpsc::channel(8);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        let emitted_chunks = Arc::new(Mutex::new(Vec::<String>::new()));
        let emitted_chunks_cb = Arc::clone(&emitted_chunks);
        let cb: FinalChunkCallback = Arc::new(move |chunk| {
            emitted_chunks_cb.lock().push(chunk);
        });
        let interim_events = Arc::new(Mutex::new(Vec::<String>::new()));
        let interim_events_cb = Arc::clone(&interim_events);
        let final_events = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let final_events_cb = Arc::clone(&final_events);

        audio_tx.send(vec![1, 2, 3, 4]).await.unwrap();
        drop(audio_tx);
        control_tx.send(ControlMessage::Finish).unwrap();

        let mut sink = VecSink::new();
        let (flush_sent, flush_received) = tokio::sync::oneshot::channel();
        sink.flush_request_sent = Some(flush_sent);
        let mut stream = futures_util::stream::iter(vec![
            Ok(Message::Text(
                r#"{"type":"transcript","text":"draft","is_final":false}"#.into(),
            )),
            Ok(Message::Text(
                r#"{"type":"transcript","text":"Hello"}"#.into(),
            )),
            Ok(Message::Text(
                r#"{"type":"transcript","text":"wor","is_final":false}"#.into(),
            )),
            Ok(Message::Text(
                r#"{"type":"transcript","text":"world.","is_final":true}"#.into(),
            )),
            Ok(Message::Text(
                r#"{"type":"transcript","text":"trailing draft","is_final":false}"#.into(),
            )),
        ])
        .chain(futures_util::stream::once(async move {
            // A real peer cannot acknowledge a request that has not been sent.
            flush_received.await.expect("client must send flush_request");
            Ok(Message::Text(
                r#"{"type":"flush_confirmation","id":"flush-test-1"}"#.into(),
            ))
        }))
        .boxed();

        tokio::time::timeout(
            Duration::from_secs(1),
            Reson8RealtimeManager::drive_session_loop(
                &mut sink,
                &mut stream,
                audio_rx,
                control_rx,
                Arc::clone(&final_text),
                Some(cb),
                "key".to_string(),
                "flush-test-1".to_string(),
                SessionEvents {
                    on_interim: Box::new(move |text| {
                        interim_events_cb.lock().push(text.to_string());
                    }),
                    on_final: Box::new(move |delta, _raw, accumulated| {
                        final_events_cb
                            .lock()
                            .push((delta.to_string(), accumulated.to_string()));
                    }),
                },
            ),
        )
        .await
        .expect("drained audio must complete its flush handshake promptly")
        .unwrap();

        assert!(sink.closed);
        assert!(matches!(sink.sent.first(), Some(Message::Binary(b)) if b.as_ref() == [1, 2, 3, 4]));
        assert!(matches!(
            sink.sent.get(1),
            Some(Message::Text(txt)) if serde_json::from_str::<Value>(txt).unwrap() == json!({"type":"flush_request","id":"flush-test-1"})
        ));
        assert_eq!(*final_text.lock(), "Hello world.");
        assert_eq!(emitted_chunks.lock().concat(), "Hello world.");
        assert_eq!(
            *interim_events.lock(),
            vec![
                "draft".to_string(),
                "wor".to_string(),
                "trailing draft".to_string(),
                String::new(),
            ]
        );
        assert_eq!(
            *final_events.lock(),
            vec![
                ("Hello".to_string(), "Hello".to_string()),
                (" world.".to_string(), "Hello world.".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn drive_session_loop_rejects_clean_close_before_flush_confirmation() {
        let (audio_tx, audio_rx) = mpsc::channel(4);
        let (_control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        drop(audio_tx);

        let mut sink = VecSink::new();
        let mut stream = futures_util::stream::iter(vec![
            Ok(Message::Text(r#"{"type":"transcript","text":"Partial"}"#.into())),
            Ok(Message::Close(None)),
        ]);

        let err = Reson8RealtimeManager::drive_session_loop(
            &mut sink,
            &mut stream,
            audio_rx,
            control_rx,
            final_text,
            None,
            "key".to_string(),
            "flush-test-2".to_string(),
            SessionEvents {
                on_interim: Box::new(|_| {}),
                on_final: Box::new(|_, _, _| {}),
            },
        )
        .await
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("closed before flush confirmation"));
    }

    #[tokio::test]
    async fn drive_session_loop_fails_on_queue_overflow_without_dropping_silently() {
        let (_audio_tx, audio_rx) = mpsc::channel(4);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        control_tx
            .send(ControlMessage::Overflow("overflow test".to_string()))
            .unwrap();

        let mut sink = VecSink::new();
        // Keep the peer connected: EOF would race the queued overflow in select!.
        let mut stream = futures_util::stream::pending();

        let err = tokio::time::timeout(
            Duration::from_secs(1),
            Reson8RealtimeManager::drive_session_loop(
                &mut sink,
                &mut stream,
                audio_rx,
                control_rx,
                final_text,
                None,
                "key".to_string(),
                "flush-test-3".to_string(),
                SessionEvents {
                    on_interim: Box::new(|_| {}),
                    on_final: Box::new(|_, _, _| {}),
                },
            ),
        )
        .await
        .expect("queue overflow must terminate the session promptly")
        .unwrap_err();

        assert!(err.to_string().contains("overflow test"));
        assert!(sink.closed);
        assert!(sink.sent.is_empty());
    }

    #[test]
    fn handshake_error_redacts_api_key_and_includes_x_error_message() {
        let response = tokio_tungstenite::tungstenite::http::Response::builder()
            .status(401)
            .header("X-Error-Message", "Bad key super-secret-key")
            .body(None)
            .unwrap();
        let formatted = format_handshake_error(
            tokio_tungstenite::tungstenite::Error::Http(response),
            "super-secret-key",
        );
        assert!(formatted.contains("401"));
        assert!(formatted.contains("Bad key"));
        assert!(!formatted.contains("super-secret-key"));
    }

    fn synthetic_active_session(operation_id: Option<u64>, initial_text: &str) -> ActiveSession {
        let (audio_tx, _audio_rx) = mpsc::channel(4);
        let (control_tx, _control_rx) = mpsc::unbounded_channel();
        let join_handle = JoinHandle::Tokio(tokio::spawn(std::future::pending::<Result<()>>()));
        ActiveSession {
            binding_id: "test-binding".to_string(),
            operation_id,
            show_preview: false,
            audio_tx,
            control_tx,
            final_text: Arc::new(Mutex::new(initial_text.to_string())),
            join_handle,
            lease: Arc::new(SessionLease::default()),
            restart_params: Some(SessionParams {
                binding_id: "test-binding".to_string(),
                operation_id,
                api_key: "test-key".to_string(),
                options: Reson8RealtimeOptions {
                    include_interim: true,
                    show_preview: true,
                    ..Reson8RealtimeOptions::default()
                },
                on_final_chunk: None,
            }),
        }
    }

    async fn abort_and_join(session: ActiveSession) {
        session.join_handle.abort();
        let _ = session.join_handle.await;
    }

    #[test]
    fn session_events_only_current_stops_emissions_after_lease_cancellation() {
        let lease = Arc::new(SessionLease::default());
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_interim = Arc::clone(&seen);
        let seen_final = Arc::clone(&seen);
        let lease_check = Arc::clone(&lease);
        let events = SessionEvents {
            on_interim: Box::new(move |text| {
                seen_interim.lock().push(format!("interim:{text}"));
            }),
            on_final: Box::new(move |delta, _, _| {
                seen_final.lock().push(format!("final:{delta}"));
            }),
        }
        .only_current(move || lease_check.allows_delivery());

        (events.on_interim)("draft");
        (events.on_final)("Hello", "Hello", "Hello");
        lease.cancel();
        (events.on_interim)("stale draft");
        (events.on_final)("stale", "stale", "Hello stale");

        assert_eq!(*seen.lock(), vec!["interim:draft", "final:Hello"]);
    }

    #[tokio::test]
    async fn prepare_session_restart_invalidates_lease_and_rearms_pending_audio() {
        let mut active = Some(synthetic_active_session(Some(7), "old text"));
        let lease = Arc::clone(&active.as_ref().unwrap().lease);
        let mut pending = None;

        let params = prepare_session_restart(&mut active, &mut pending, |id, binding| {
            id == Some(7) && binding == "test-binding"
        })
        .expect("active recording session should prepare restart");

        assert!(active.is_none());
        assert!(!lease.allows_delivery());
        assert_eq!(params.operation_id, Some(7));
        assert!(params.options.include_interim);
        assert!(params.options.show_preview);
        assert_eq!(pending.as_ref().unwrap().operation_id, Some(7));
    }

    #[tokio::test]
    async fn drive_session_loop_cancel_closes_socket_without_flush_or_emissions() {
        let (audio_tx, audio_rx) = mpsc::channel(4);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let final_text = Arc::new(Mutex::new(String::new()));
        let emitted_callbacks = Arc::new(Mutex::new(Vec::<String>::new()));
        let emitted_callbacks_cb = Arc::clone(&emitted_callbacks);
        let on_final_chunk: FinalChunkCallback = Arc::new(move |chunk| {
            emitted_callbacks_cb.lock().push(chunk);
        });
        let live_events = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let live_events_cb = Arc::clone(&live_events);

        control_tx.send(ControlMessage::Cancel).unwrap();

        let mut sink = VecSink::new();
        // Keep both the audio channel and peer stream pending so Cancel is the only ready branch.
        let mut stream = futures_util::stream::pending();

        tokio::time::timeout(
            Duration::from_secs(1),
            Reson8RealtimeManager::drive_session_loop(
                &mut sink,
                &mut stream,
                audio_rx,
                control_rx,
                Arc::clone(&final_text),
                Some(on_final_chunk),
                "key".to_string(),
                "flush-test-cancel".to_string(),
                SessionEvents {
                    on_interim: Box::new(|_| {}),
                    on_final: Box::new(move |delta, _raw, accumulated| {
                        live_events_cb
                            .lock()
                            .push((delta.to_string(), accumulated.to_string()));
                    }),
                },
            ),
        )
        .await
        .expect("cancel must terminate the session loop promptly")
        .unwrap();

        drop(audio_tx);
        drop(control_tx);

        assert!(sink.closed);
        assert!(sink.sent.is_empty());
        assert!(final_text.lock().is_empty());
        assert!(emitted_callbacks.lock().is_empty());
        assert!(live_events.lock().is_empty());
    }

    #[tokio::test]
    async fn detach_session_if_matches_preserves_newer_active_and_pending_operations() {
        let mut active = Some(synthetic_active_session(Some(2), "new words"));
        let mut pending = Some(PendingAudio {
            operation_id: Some(2),
            frames: vec![vec![7, 8]],
            overflow_error: Some("pending overflow".to_string()),
        });

        assert!(detach_session_if_matches(&mut active, &mut pending, Some(1)).is_none());
        assert!(detach_session_if_matches(&mut active, &mut pending, None).is_none());

        let surviving_active = active.take().expect("newer active session must remain");
        let active_op = surviving_active.operation_id;
        let active_text = surviving_active.final_text.lock().clone();
        abort_and_join(surviving_active).await;

        assert_eq!(active_op, Some(2));
        assert_eq!(active_text, "new words");
        let surviving_pending = pending.as_ref().expect("newer pending audio must remain");
        assert_eq!(surviving_pending.operation_id, Some(2));
        assert_eq!(surviving_pending.frames, vec![vec![7, 8]]);
        assert_eq!(
            surviving_pending.overflow_error.as_deref(),
            Some("pending overflow")
        );

        let mut active = Some(synthetic_active_session(Some(1), "old words"));
        let detached = detach_session_if_matches(&mut active, &mut pending, Some(1))
            .expect("matching active session must detach");
        let detached_op = detached.operation_id;
        abort_and_join(detached).await;

        assert_eq!(detached_op, Some(1));
        assert!(active.is_none());
        let surviving_pending = pending.as_ref().expect("newer pending audio must remain");
        assert_eq!(surviving_pending.operation_id, Some(2));
        assert_eq!(surviving_pending.frames, vec![vec![7, 8]]);
    }

    #[tokio::test]
    async fn detach_session_if_matches_clears_only_matching_active_and_pending_state() {
        for operation_id in [None, Some(1)] {
            let mut active = Some(synthetic_active_session(operation_id, "matched"));
            let mut pending = Some(PendingAudio {
                operation_id,
                frames: vec![vec![1, 2]],
                overflow_error: None,
            });

            let detached = detach_session_if_matches(&mut active, &mut pending, operation_id)
                .expect("matching active session must detach");
            let detached_op = detached.operation_id;
            abort_and_join(detached).await;

            assert_eq!(detached_op, operation_id);
            assert!(active.is_none());
            assert!(pending.is_none());
        }

        let mut active = None;
        let mut pending = Some(PendingAudio {
            operation_id: Some(2),
            frames: vec![vec![3, 4]],
            overflow_error: None,
        });
        assert!(detach_session_if_matches(&mut active, &mut pending, Some(1)).is_none());
        assert_eq!(pending.as_ref().unwrap().operation_id, Some(2));
        assert_eq!(pending.as_ref().unwrap().frames, vec![vec![3, 4]]);

        assert!(detach_session_if_matches(&mut active, &mut pending, Some(2)).is_none());
        assert!(pending.is_none());
    }
}
