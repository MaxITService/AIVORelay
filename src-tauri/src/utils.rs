use crate::managers::audio::AudioRecordingManager;
use crate::managers::llm_operation::LlmOperationTracker;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::soniox_stt::SonioxSttManager;
use crate::managers::transcription::TranscriptionManager;
use crate::session_manager;
use crate::ManagedToggleState;
use log::{debug, info, warn};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

// Re-export all utility modules for easy access
// pub use crate::audio_feedback::*;
pub use crate::clipboard::*;
pub use crate::overlay::*;
pub use crate::tray::*;

/// Centralized cancellation function that can be called from anywhere in the app.
/// Handles cancelling both recording and transcription operations and updates UI state.
/// This also cancels any ongoing Processing work (transcription, LLM, etc.).
pub fn cancel_current_operation(app: &AppHandle) {
    info!("Initiating operation cancellation...");

    // Take the active session if any - its Drop will handle cleanup
    // (unregistering cancel shortcut, removing mute, etc.)
    if let Some((session, binding_id)) = session_manager::take_session(app) {
        debug!(
            "Cancellation: took active session for binding '{}'",
            binding_id
        );
        // Session's Drop will handle:
        // - Unregistering cancel shortcut
        // - Removing mute
        // - Hiding overlay
        // - Resetting tray icon
        drop(session);
    } else {
        // No Recording session - maybe we're in Processing state
        // exit_processing will set state to Idle if we were in Processing
        session_manager::exit_processing(app);
        debug!("Cancellation: no active recording session, checked for Processing state");
    }

    // Reset all shortcut toggle states.
    // This is critical for non-push-to-talk mode where shortcuts toggle on/off
    let toggle_state_manager = app.state::<ManagedToggleState>();
    if let Ok(mut states) = toggle_state_manager.lock() {
        states.active_toggles.values_mut().for_each(|v| *v = false);
    } else {
        warn!("Failed to lock toggle state manager during cancellation");
    }

    // Cancel any ongoing recording (belt-and-suspenders, session should have done this)
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    audio_manager.cancel_recording();

    // Cancel any in-flight Remote STT requests
    let remote_stt_manager = app.state::<Arc<RemoteSttManager>>();
    remote_stt_manager.cancel();
    let soniox_stt_manager = app.state::<Arc<SonioxSttManager>>();
    soniox_stt_manager.cancel();

    // Cancel any in-flight LLM requests (AI Replace, etc.)
    let llm_tracker = app.state::<Arc<LlmOperationTracker>>();
    llm_tracker.cancel();

    // Ensure UI is in idle state (redundant if session Drop ran, but safe)
    change_tray_icon(app, crate::tray::TrayIconState::Idle);
    hide_recording_overlay(app);

    // Unload model if immediate unload is enabled
    let tm = app.state::<Arc<TranscriptionManager>>();
    tm.maybe_unload_immediately("cancellation");

    info!("Operation cancellation completed - returned to idle state");
}

/// Check if using the Wayland display server protocol
#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() == "wayland")
            .unwrap_or(false)
}
