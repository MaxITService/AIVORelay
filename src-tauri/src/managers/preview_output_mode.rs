use serde::Serialize;
use specta::Type;
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const PREVIEW_WINDOW_LABEL: &str = "soniox_live_preview";
const EVENT_KEBAB: &str = "preview-output-mode-state";
const EVENT_SNAKE: &str = "preview_output_mode_state";

#[derive(Serialize, Clone, Debug, Default, Type)]
pub struct PreviewOutputModeStatePayload {
    pub active: bool,
    pub recording: bool,
    pub processing_llm: bool,
    pub flush_visible: bool,
    pub is_realtime: bool,
    pub binding_id: Option<String>,
    pub profile_id: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct PreviewOutputModeRuntime {
    active: bool,
    recording: bool,
    processing_llm: bool,
    is_realtime: bool,
    binding_id: Option<String>,
    profile_id: Option<String>,
    recording_prefix_text: String,
    error_message: Option<String>,
}

impl PreviewOutputModeRuntime {
    fn to_payload(&self) -> PreviewOutputModeStatePayload {
        PreviewOutputModeStatePayload {
            active: self.active,
            recording: self.recording,
            processing_llm: self.processing_llm,
            flush_visible: self.active && !self.is_realtime,
            is_realtime: self.is_realtime,
            binding_id: self.binding_id.clone(),
            profile_id: self.profile_id.clone(),
            error_message: self.error_message.clone(),
        }
    }
}

static PREVIEW_OUTPUT_MODE_STATE: LazyLock<Mutex<PreviewOutputModeRuntime>> =
    LazyLock::new(|| Mutex::new(PreviewOutputModeRuntime::default()));

fn emit_state_update(app: &AppHandle, payload: &PreviewOutputModeStatePayload) {
    let _ = app.emit(EVENT_KEBAB, payload.clone());
    let _ = app.emit(EVENT_SNAKE, payload.clone());
    if let Some(window) = app.get_webview_window(PREVIEW_WINDOW_LABEL) {
        let _ = window.emit(EVENT_KEBAB, payload.clone());
        let _ = window.emit(EVENT_SNAKE, payload.clone());
    }
}

fn update_state<F>(app: &AppHandle, updater: F)
where
    F: FnOnce(&mut PreviewOutputModeRuntime),
{
    let payload = {
        let mut guard = match PREVIEW_OUTPUT_MODE_STATE.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        updater(&mut guard);
        guard.to_payload()
    };
    emit_state_update(app, &payload);
}

pub fn activate_session(
    app: &AppHandle,
    binding_id: String,
    profile_id: Option<String>,
    is_realtime: bool,
    recording_prefix_text: String,
) {
    update_state(app, move |state| {
        state.active = true;
        state.recording = true;
        state.processing_llm = false;
        state.is_realtime = is_realtime;
        state.binding_id = Some(binding_id);
        state.profile_id = profile_id;
        state.recording_prefix_text = recording_prefix_text;
        state.error_message = None;
    });
}

pub fn deactivate_session(app: &AppHandle) {
    update_state(app, |state| {
        *state = PreviewOutputModeRuntime::default();
    });
}

pub fn set_recording(app: &AppHandle, recording: bool) {
    update_state(app, move |state| {
        if state.active {
            state.recording = recording;
        }
    });
}

pub fn set_recording_prefix_text(app: &AppHandle, recording_prefix_text: String) {
    update_state(app, move |state| {
        if state.active {
            state.recording_prefix_text = recording_prefix_text;
        }
    });
}

pub fn set_processing_llm(app: &AppHandle, processing_llm: bool) {
    update_state(app, move |state| {
        if state.active {
            state.processing_llm = processing_llm;
        }
    });
}

pub fn set_error(app: &AppHandle, error_message: Option<String>) {
    update_state(app, move |state| {
        if state.active {
            state.error_message = error_message;
        }
    });
}

pub fn get_state_payload() -> PreviewOutputModeStatePayload {
    PREVIEW_OUTPUT_MODE_STATE
        .lock()
        .map(|state| state.to_payload())
        .unwrap_or_default()
}

pub fn is_active() -> bool {
    PREVIEW_OUTPUT_MODE_STATE
        .lock()
        .map(|state| state.active)
        .unwrap_or(false)
}

pub fn is_active_for_binding(binding_id: &str) -> bool {
    PREVIEW_OUTPUT_MODE_STATE
        .lock()
        .map(|state| {
            state.active && state.binding_id.as_deref().map(|id| id == binding_id).unwrap_or(false)
        })
        .unwrap_or(false)
}

pub fn current_profile_id() -> Option<String> {
    PREVIEW_OUTPUT_MODE_STATE
        .lock()
        .ok()
        .and_then(|state| state.profile_id.clone())
}

pub fn recording_prefix_text() -> String {
    PREVIEW_OUTPUT_MODE_STATE
        .lock()
        .map(|state| state.recording_prefix_text.clone())
        .unwrap_or_default()
}
