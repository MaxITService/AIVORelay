use crate::file_transcription_diarization::{normalize_raw_speaker_blocks, RawSpeakerBlock};
use serde::Serialize;
use specta::Type;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tauri::async_runtime::JoinHandle;

const EVENT_KEBAB: &str = "live-sound-transcription-state";
const EVENT_SNAKE: &str = "live_sound_transcription_state";

#[derive(Serialize, Clone, Debug, Default, Type)]
pub struct LiveSoundTranscriptSegmentPayload {
    pub speaker_id: Option<u32>,
    pub speaker_label: Option<String>,
    pub text: String,
    pub is_interim: bool,
}

#[derive(Serialize, Clone, Debug, Default, Type)]
pub struct LiveSoundTranscriptionStatePayload {
    pub active: bool,
    pub recording: bool,
    pub processing_llm: bool,
    pub binding_id: Option<String>,
    pub error_message: Option<String>,
    pub final_text: String,
    pub interim_text: String,
    pub segments: Vec<LiveSoundTranscriptSegmentPayload>,
    pub auto_stop_remaining_seconds: Option<u64>,
}

#[derive(Clone, Debug, Default)]
struct LiveSoundTranscriptionRuntime {
    active: bool,
    recording: bool,
    processing_llm: bool,
    binding_id: Option<String>,
    error_message: Option<String>,
    final_text: String,
    interim_text: String,
    final_raw_blocks: Vec<RawSpeakerBlock>,
    interim_raw_blocks: Vec<RawSpeakerBlock>,
    session_id: u64,
    auto_stop_deadline: Option<Instant>,
}

impl LiveSoundTranscriptionRuntime {
    fn push_final_text(&mut self, text: &str, add_separator: bool) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        if self.final_text.trim().is_empty() {
            self.final_text = trimmed.to_string();
            return;
        }

        let ends_with_whitespace = self
            .final_text
            .chars()
            .last()
            .map(char::is_whitespace)
            .unwrap_or(false);
        let starts_with_whitespace = trimmed
            .chars()
            .next()
            .map(char::is_whitespace)
            .unwrap_or(false);
        if add_separator && !ends_with_whitespace && !starts_with_whitespace {
            self.final_text.push(' ');
        }
        self.final_text.push_str(trimmed);
    }

    fn to_payload(&self) -> LiveSoundTranscriptionStatePayload {
        let auto_stop_remaining_seconds = self
            .auto_stop_deadline
            .filter(|_| self.recording)
            .map(|deadline| remaining_seconds(deadline.saturating_duration_since(Instant::now())));

        let mut segments: Vec<LiveSoundTranscriptSegmentPayload> =
            normalize_raw_speaker_blocks(self.final_raw_blocks.clone())
                .into_iter()
                .map(|block| LiveSoundTranscriptSegmentPayload {
                    speaker_id: Some(block.speaker_id),
                    speaker_label: Some(block.default_name),
                    text: block.text,
                    is_interim: false,
                })
                .collect();

        if segments.is_empty() && !self.final_text.trim().is_empty() {
            segments.push(LiveSoundTranscriptSegmentPayload {
                speaker_id: None,
                speaker_label: None,
                text: self.final_text.clone(),
                is_interim: false,
            });
        }

        segments.extend(
            normalize_raw_speaker_blocks(self.interim_raw_blocks.clone())
                .into_iter()
                .map(|block| LiveSoundTranscriptSegmentPayload {
                    speaker_id: Some(block.speaker_id),
                    speaker_label: Some(block.default_name),
                    text: block.text,
                    is_interim: true,
                }),
        );

        if self.interim_text.trim().len() > 0 && self.interim_raw_blocks.is_empty() {
            segments.push(LiveSoundTranscriptSegmentPayload {
                speaker_id: None,
                speaker_label: None,
                text: self.interim_text.clone(),
                is_interim: true,
            });
        }

        LiveSoundTranscriptionStatePayload {
            active: self.active,
            recording: self.recording,
            processing_llm: self.processing_llm,
            binding_id: self.binding_id.clone(),
            error_message: self.error_message.clone(),
            final_text: self.final_text.clone(),
            interim_text: self.interim_text.clone(),
            segments,
            auto_stop_remaining_seconds,
        }
    }
}

static LIVE_SOUND_TRANSCRIPTION_STATE: LazyLock<Mutex<LiveSoundTranscriptionRuntime>> =
    LazyLock::new(|| Mutex::new(LiveSoundTranscriptionRuntime::default()));
static LIVE_SOUND_AUTO_STOP_TASK: LazyLock<Mutex<Option<JoinHandle<()>>>> =
    LazyLock::new(|| Mutex::new(None));
static NEXT_LIVE_SOUND_SESSION_ID: AtomicU64 = AtomicU64::new(1);

fn remaining_seconds(duration: Duration) -> u64 {
    let millis = duration.as_millis();
    if millis == 0 {
        0
    } else {
        ((millis + 999) / 1000) as u64
    }
}

fn emit_state_update(app: &AppHandle, payload: &LiveSoundTranscriptionStatePayload) {
    let _ = app.emit(EVENT_KEBAB, payload.clone());
    let _ = app.emit(EVENT_SNAKE, payload.clone());
}

fn cancel_auto_stop_task() {
    if let Ok(mut guard) = LIVE_SOUND_AUTO_STOP_TASK.lock() {
        if let Some(handle) = guard.take() {
            handle.abort();
        }
    }
}

fn current_auto_stop_remaining(session_id: u64) -> Option<Duration> {
    LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .ok()
        .and_then(|state| {
            if state.session_id != session_id || !state.recording {
                return None;
            }
            state
                .auto_stop_deadline
                .map(|deadline| deadline.saturating_duration_since(Instant::now()))
        })
}

fn spawn_auto_stop_task(app: &AppHandle, session_id: u64) {
    cancel_auto_stop_task();

    let app = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            let Some(remaining) = current_auto_stop_remaining(session_id) else {
                return;
            };

            emit_state_update(&app, &get_state_payload());

            if remaining.is_zero() {
                let app_for_stop = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::actions::stop_live_sound_transcription_session(&app_for_stop);
                });
                return;
            }

            tokio::time::sleep(remaining.min(Duration::from_secs(1))).await;
        }
    });

    if let Ok(mut guard) = LIVE_SOUND_AUTO_STOP_TASK.lock() {
        *guard = Some(handle);
    }
}

fn update_state<F>(app: &AppHandle, updater: F)
where
    F: FnOnce(&mut LiveSoundTranscriptionRuntime),
{
    let payload = {
        let mut guard = match LIVE_SOUND_TRANSCRIPTION_STATE.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        updater(&mut guard);
        guard.to_payload()
    };
    emit_state_update(app, &payload);
}

pub fn get_state_payload() -> LiveSoundTranscriptionStatePayload {
    LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .map(|state| state.to_payload())
        .unwrap_or_default()
}

pub fn current_session_id() -> u64 {
    LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .map(|state| state.session_id)
        .unwrap_or_default()
}

pub fn activate_session(app: &AppHandle, binding_id: String, auto_stop_minutes: u32) {
    cancel_auto_stop_task();

    let session_id = NEXT_LIVE_SOUND_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    let auto_stop_deadline = if auto_stop_minutes > 0 {
        Some(
            Instant::now()
                + Duration::from_secs(u64::from(auto_stop_minutes).saturating_mul(60)),
        )
    } else {
        None
    };
    let should_start_auto_stop = auto_stop_deadline.is_some();

    update_state(app, move |state| {
        state.active = true;
        state.recording = true;
        state.processing_llm = false;
        state.binding_id = Some(binding_id);
        state.error_message = None;
        state.interim_text.clear();
        state.interim_raw_blocks.clear();
        state.session_id = session_id;
        state.auto_stop_deadline = auto_stop_deadline;
    });

    if should_start_auto_stop {
        spawn_auto_stop_task(app, session_id);
    }
}

pub fn finish_session(app: &AppHandle) {
    cancel_auto_stop_task();
    update_state(app, |state| {
        state.active = false;
        state.recording = false;
        state.processing_llm = false;
        state.binding_id = None;
        state.interim_text.clear();
        state.interim_raw_blocks.clear();
        state.auto_stop_deadline = None;
    });
}

pub fn set_recording(app: &AppHandle, recording: bool) {
    if !recording {
        cancel_auto_stop_task();
    }

    update_state(app, move |state| {
        state.recording = recording;
        if recording {
            state.active = true;
        } else {
            state.auto_stop_deadline = None;
        }
    });
}

pub fn set_recording_if_session_matches(app: &AppHandle, session_id: u64, recording: bool) {
    let matches_session = LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .map(|state| state.session_id == session_id)
        .unwrap_or(false);

    if matches_session {
        set_recording(app, recording);
    }
}

pub fn set_processing_llm(app: &AppHandle, processing_llm: bool) {
    update_state(app, move |state| {
        state.processing_llm = processing_llm;
    });
}

pub fn set_error(app: &AppHandle, error_message: Option<String>) {
    update_state(app, move |state| {
        state.error_message = error_message;
    });
}

pub fn clear_transcript(app: &AppHandle) {
    update_state(app, |state| {
        state.final_text.clear();
        state.interim_text.clear();
        state.final_raw_blocks.clear();
        state.interim_raw_blocks.clear();
        state.error_message = None;
    });
}

pub fn current_final_text() -> String {
    LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .map(|state| state.final_text.trim().to_string())
        .unwrap_or_default()
}

pub fn replace_final_text(app: &AppHandle, final_text: String) {
    update_state(app, move |state| {
        state.final_text = final_text.trim().to_string();
        state.interim_text.clear();
        state.final_raw_blocks.clear();
        state.interim_raw_blocks.clear();
        state.error_message = None;
    });
}

pub fn append_final_result(
    app: &AppHandle,
    final_text: &str,
    raw_blocks: Vec<RawSpeakerBlock>,
    add_separator: bool,
) {
    update_state(app, move |state| {
        state.push_final_text(final_text, add_separator);
        if raw_blocks.is_empty() {
            return;
        }
        state.final_raw_blocks.extend(raw_blocks);
    });
}

pub fn set_interim_result(
    app: &AppHandle,
    interim_text: String,
    raw_blocks: Vec<RawSpeakerBlock>,
) {
    update_state(app, move |state| {
        state.interim_text = interim_text.trim().to_string();
        state.interim_raw_blocks = raw_blocks;
    });
}
