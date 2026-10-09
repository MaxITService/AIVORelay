//! Loading feedback tied to the recording's captured provider and model.

use crate::session_manager::{ManagedSessionState, SessionState};
use crate::settings::{AppSettings, TranscriptionProvider};
use serde::Serialize;
use specta::Type;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Listener, Manager};

#[derive(Clone)]
struct RecordingContext {
    operation_id: u64,
    provider: String,
    model_id: Option<String>,
}

#[derive(Clone, Default)]
struct LoadingState {
    recording: Option<RecordingContext>,
    loading: Option<(String, Instant)>,
}

impl LoadingState {
    fn handle_model_event(&mut self, kind: &str, model_id: Option<&str>, now: Instant) -> bool {
        match (kind, model_id) {
            ("loading_started", Some(model_id)) => self.loading = Some((model_id.to_string(), now)),
            ("loading_completed" | "loading_failed", Some(model_id)) => {
                if self.loading.as_ref().is_some_and(|(loading_id, _)| loading_id == model_id) {
                    self.loading = None;
                }
            }
            _ => return false,
        }
        true
    }

    fn snapshot(&self, current_operation_id: Option<u64>, preview_active: bool, now: Instant) -> RecordingModelLoadingState {
        let Some(recording) = self.recording.as_ref().filter(|recording| Some(recording.operation_id) == current_operation_id) else {
            return RecordingModelLoadingState::default();
        };
        let elapsed = self.loading.as_ref().and_then(|(model_id, started)| {
            (recording.model_id.as_ref() == Some(model_id)).then(|| now.saturating_duration_since(*started))
        });
        RecordingModelLoadingState {
            recording_session_id: Some(recording.operation_id),
            provider: Some(recording.provider.clone()),
            model_id: recording.model_id.clone(),
            loading: elapsed.is_some(),
            loading_elapsed_ms: elapsed.map_or(0, |elapsed| elapsed.as_millis() as u64),
            preview_active,
        }
    }
}

static STATE: LazyLock<Mutex<LoadingState>> = LazyLock::new(|| Mutex::new(LoadingState::default()));

#[derive(Clone, Default, Serialize, Type)]
pub(crate) struct RecordingModelLoadingState {
    recording_session_id: Option<u64>,
    provider: Option<String>,
    model_id: Option<String>,
    loading: bool,
    loading_elapsed_ms: u64,
    preview_active: bool,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_recording_model_loading_state(app: AppHandle) -> RecordingModelLoadingState {
    snapshot(&app)
}

fn snapshot(app: &AppHandle) -> RecordingModelLoadingState {
    let state = {
        let state = STATE.lock().unwrap_or_else(|error| error.into_inner());
        state.clone()
    };
    let Some(session) = app.try_state::<ManagedSessionState>() else {
        return RecordingModelLoadingState::default();
    };
    let current = crate::session_manager::lock_session_state(&session, "model loading snapshot");
    let operation_id = match &*current {
        SessionState::Recording { operation_id, .. } | SessionState::Processing { operation_id, .. } => Some(*operation_id),
        _ => None,
    };
    drop(current);
    state.snapshot(operation_id, crate::managers::preview_output_mode::is_active(), Instant::now())
}

fn emit(app: &AppHandle) {
    let _ = app.emit("recording-model-loading-state", snapshot(app));
}

fn schedule_slow_notice(app: &AppHandle) {
    let delay = Duration::from_millis(2000u64.saturating_sub(snapshot(app).loading_elapsed_ms));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(delay).await;
        let state = snapshot(&app);
        if !state.loading || state.loading_elapsed_ms < 2000 || !state.preview_active {
            return;
        }
        let Some(operation_id) = state.recording_session_id else { return; };
        let Some(session) = app.try_state::<ManagedSessionState>() else { return; };
        let current = crate::session_manager::lock_session_state(&session, "slow model loading notice");
        let recording = matches!(&*current, SessionState::Recording { operation_id: current_id, .. }
            if *current_id == operation_id);
        drop(current);
        if recording {
            emit(&app);
            crate::overlay::show_live_preview_window(&app);
        }
    });
}

pub(crate) fn begin_recording(app: &AppHandle, operation_id: u64, settings: &AppSettings) {
    let local = settings.transcription_provider == TranscriptionProvider::Local;
    let provider = serde_json::to_value(settings.transcription_provider)
        .ok().and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_default();
    STATE.lock().unwrap_or_else(|error| error.into_inner()).recording = Some(RecordingContext {
        operation_id,
        provider,
        model_id: local.then(|| settings.selected_model.clone()),
    });
    emit(app);
    schedule_slow_notice(app);
}

pub(crate) fn notify_session_changed(app: &AppHandle) {
    emit(app);
}

pub(crate) fn install(app: &AppHandle) {
    let handle = app.clone();
    app.listen("model-state-changed", move |event| {
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) else { return; };
        let Some(kind) = payload.get("event_type").and_then(|value| value.as_str()) else { return; };
        let model_id = payload.get("model_id").and_then(|value| value.as_str());
        {
            let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
            if !state.handle_model_event(kind, model_id, Instant::now()) { return; }
        }
        emit(&handle);
        if kind == "loading_started" {
            schedule_slow_notice(&handle);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording(provider: &str, model_id: Option<&str>) -> LoadingState {
        LoadingState {
            recording: Some(RecordingContext { operation_id: 7, provider: provider.into(), model_id: model_id.map(str::to_owned) }),
            loading: None,
        }
    }

    #[test]
    fn loading_snapshot_matches_the_captured_model_and_session() {
        let now = Instant::now();
        let mut state = recording("local", Some("captured"));
        state.handle_model_event("loading_started", Some("other"), now);
        assert!(!state.snapshot(Some(7), true, now).loading);
        state.handle_model_event("loading_started", Some("captured"), now);
        let snapshot = state.snapshot(Some(7), true, now + Duration::from_millis(2300));
        assert!(snapshot.loading);
        assert_eq!(snapshot.loading_elapsed_ms, 2300);
        assert_eq!(snapshot.provider.as_deref(), Some("local"));
        assert_eq!(snapshot.model_id.as_deref(), Some("captured"));
        assert!(snapshot.preview_active);
        for current in [None, Some(8)] {
            let stopped = state.snapshot(current, true, now);
            assert!(!stopped.loading);
            assert_eq!(stopped.recording_session_id, None);
            assert!(!stopped.preview_active);
        }
    }

    #[test]
    fn remote_recording_never_inherits_background_local_loading() {
        let now = Instant::now();
        let mut state = recording("remote_soniox", None);
        state.handle_model_event("loading_started", Some("local-model"), now);
        let snapshot = state.snapshot(Some(7), true, now);
        assert!(!snapshot.loading);
        assert_eq!(snapshot.provider.as_deref(), Some("remote_soniox"));
        assert_eq!(snapshot.recording_session_id, Some(7));
        assert_eq!(snapshot.model_id, None);
    }

    #[test]
    fn loading_terminal_events_clear_only_the_matching_load() {
        let now = Instant::now();
        let mut state = recording("local", Some("captured"));
        for terminal in ["loading_completed", "loading_failed"] {
            state.handle_model_event("loading_started", Some("captured"), now);
            state.handle_model_event("loading_failed", Some("other"), now);
            assert!(state.snapshot(Some(7), false, now).loading);
            state.handle_model_event(terminal, Some("captured"), now);
            assert!(!state.snapshot(Some(7), false, now).loading);
        }
        assert!(!state.handle_model_event("loading_started", None, now));
        assert!(!state.handle_model_event("unrecognized", Some("captured"), now));
    }

    #[test]
    fn late_unload_events_do_not_finish_an_in_flight_load() {
        let now = Instant::now();
        let mut state = recording("local", Some("captured"));
        state.handle_model_event("loading_started", Some("captured"), now);
        // Unload producers release the previous engine before emitting their
        // event, so an old unload can arrive after this new loading_started.
        for unloaded_model in [None, Some("other"), Some("captured")] {
            assert!(!state.handle_model_event("unloaded", unloaded_model, now));
            let snapshot = state.snapshot(Some(7), false, now + Duration::from_millis(2300));
            assert!(snapshot.loading);
            assert_eq!(snapshot.loading_elapsed_ms, 2300);
        }
        state.handle_model_event("loading_failed", Some("captured"), now);
        assert!(!state.snapshot(Some(7), false, now).loading);
    }
}
