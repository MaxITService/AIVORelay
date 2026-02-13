//! Recording Session Management
//!
//! This module provides RAII-based session management for recording operations.
//! It ensures that resources (cancel shortcut, mute, overlay) are properly cleaned up
//! regardless of how the recording ends (success, cancel, error, or double-stop).
//!
//! The key insight is that `RecordingSession` is a guard that:
//! - Registers the cancel shortcut on creation
//! - Unregisters it exactly once on Drop
//! - Tracks what resources were acquired to only release what was actually acquired

use crate::managers::audio::AudioRecordingManager;
use crate::settings::AppSettings;
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::hide_recording_overlay;
use log::debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

/// Represents the current state of the recording system.
/// This is the single source of truth for whether we're recording or processing.
#[derive(Debug)]
pub enum SessionState {
    /// No recording or processing in progress
    Idle,
    /// Recording is active with the given session
    Recording {
        session: Arc<RecordingSession>,
        binding_id: String,
        /// The profile ID that was active when recording started.
        /// This is used to ensure transcription uses the correct profile
        /// even if the user switches profiles mid-recording.
        captured_profile_id: Option<String>,
        /// Full settings snapshot captured when recording started.
        /// Stop/transcription logic must use this snapshot to avoid routing
        /// drift when the user changes settings mid-recording.
        captured_settings: AppSettings,
    },
    /// Recording finished, now processing (transcription, LLM, etc.)
    /// New recordings are blocked during this state, only cancellation is allowed.
    Processing { binding_id: String },
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::Idle
    }
}

/// Managed state type for the session
pub type ManagedSessionState = Mutex<SessionState>;

/// A recording session guard that ensures proper cleanup via RAII.
///
/// When this struct is dropped, it will:
/// 1. Unregister the cancel shortcut (if it was registered)
/// 2. Remove mute (if it was applied)
/// 3. Hide the recording overlay
///
/// All cleanup operations are idempotent - safe to call even if the resource
/// wasn't acquired or was already released.
pub struct RecordingSession {
    app: AppHandle,
    cancel_shortcut_registered: AtomicBool,
    mute_applied: AtomicBool,
    /// Track if Drop cleanup has already run (for explicit finish() calls)
    cleaned_up: AtomicBool,
}

impl std::fmt::Debug for RecordingSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordingSession")
            .field(
                "cancel_shortcut_registered",
                &self.cancel_shortcut_registered.load(Ordering::SeqCst),
            )
            .field("mute_applied", &self.mute_applied.load(Ordering::SeqCst))
            .field("cleaned_up", &self.cleaned_up.load(Ordering::SeqCst))
            .finish()
    }
}

impl RecordingSession {
    /// Creates a new recording session with pre-set resource tracking.
    ///
    /// This is used by actions.rs when it manages the recording flow itself.
    /// The `will_register_cancel` and `will_apply_mute` flags indicate what
    /// resources the caller intends to acquire, so the session knows what to clean up.
    pub fn new_with_resources(
        app: &AppHandle,
        _will_register_cancel: bool,
        will_apply_mute: bool,
    ) -> Self {
        Self {
            app: app.clone(),
            cancel_shortcut_registered: AtomicBool::new(false), // Will be set when actually registered
            mute_applied: AtomicBool::new(will_apply_mute),
            cleaned_up: AtomicBool::new(false),
        }
    }

    /// Registers the cancel shortcut for this session.
    /// Safe to call multiple times - only registers once.
    pub fn register_cancel_shortcut(&self) {
        if !self.cancel_shortcut_registered.swap(true, Ordering::SeqCst) {
            debug!("RecordingSession: Registering cancel shortcut");
            shortcut::register_cancel_shortcut(&self.app);
        }
    }

    /// Explicitly finish the session and perform cleanup.
    /// This is called when transitioning from Recording to Processing state.
    /// After this, Drop becomes a no-op.
    pub fn finish(&self) {
        if self.cleaned_up.swap(true, Ordering::SeqCst) {
            debug!("RecordingSession::finish called but already cleaned up");
            return;
        }
        self.do_cleanup();
    }

    /// Internal cleanup logic, shared by finish() and Drop.
    fn do_cleanup(&self) {
        debug!("RecordingSession: Performing cleanup");

        // Unregister cancel shortcut if we registered it
        if self
            .cancel_shortcut_registered
            .swap(false, Ordering::SeqCst)
        {
            debug!("RecordingSession: Unregistering cancel shortcut");
            shortcut::unregister_cancel_shortcut(&self.app);
        }

        // Remove mute if we applied it
        if self.mute_applied.swap(false, Ordering::SeqCst) {
            debug!("RecordingSession: Removing mute");
            let rm = self.app.state::<Arc<AudioRecordingManager>>();
            rm.remove_mute();
        }
    }
}

impl Drop for RecordingSession {
    fn drop(&mut self) {
        if self.cleaned_up.load(Ordering::SeqCst) {
            return; // Already cleaned up via finish()
        }
        debug!("RecordingSession: Drop triggered, performing cleanup");
        self.do_cleanup();
        // Also hide overlay and reset tray on unexpected drop (e.g., cancel)
        hide_recording_overlay(&self.app);
        change_tray_icon(&self.app, TrayIconState::Idle);
    }
}

// ============================================================================
// Session State Management Functions
// ============================================================================

/// Takes the current session out of managed state, returning to Idle.
///
/// Returns the session and binding_id if there was an active recording,
/// or None if we were already Idle or Processing.
///
/// The returned session's Drop will handle cleanup if not explicitly finish()'d.
pub fn take_session(app: &AppHandle) -> Option<(Arc<RecordingSession>, String)> {
    let state = app.state::<ManagedSessionState>();
    let mut state_guard = state.lock().expect("Failed to lock session state");

    match std::mem::replace(&mut *state_guard, SessionState::Idle) {
        SessionState::Recording {
            session,
            binding_id,
            captured_profile_id: _,
            captured_settings: _,
        } => {
            debug!("take_session: Took session for {}", binding_id);
            Some((session, binding_id))
        }
        SessionState::Idle => {
            debug!("take_session: No active session to take");
            None
        }
        SessionState::Processing { binding_id } => {
            debug!(
                "take_session: Was in Processing state for {}, returning to Idle",
                binding_id
            );
            None
        }
    }
}

/// Takes the session only if the binding_id matches.
///
/// This prevents one action's stop from stealing another action's session.
pub fn take_session_if_matches(
    app: &AppHandle,
    expected_binding_id: &str,
) -> Option<Arc<RecordingSession>> {
    let state = app.state::<ManagedSessionState>();
    let mut state_guard = state.lock().expect("Failed to lock session state");

    match &*state_guard {
        SessionState::Recording { binding_id, .. } if binding_id == expected_binding_id => {
            // Matches, take it
            if let SessionState::Recording { session, .. } =
                std::mem::replace(&mut *state_guard, SessionState::Idle)
            {
                debug!(
                    "take_session_if_matches: Took session for {}",
                    expected_binding_id
                );
                return Some(session);
            }
        }
        SessionState::Recording { binding_id, .. } => {
            debug!(
                "take_session_if_matches: Binding mismatch (expected {}, got {})",
                expected_binding_id, binding_id
            );
        }
        SessionState::Processing { binding_id } => {
            debug!(
                "take_session_if_matches: In Processing state for {}",
                binding_id
            );
        }
        SessionState::Idle => {
            debug!("take_session_if_matches: No active session");
        }
    }
    None
}

/// Exits the Processing state, returning to Idle.
/// Call this when async processing completes (success or error).
pub fn exit_processing(app: &AppHandle) {
    let state = app.state::<ManagedSessionState>();
    let mut state_guard = state.lock().expect("Failed to lock session state");

    if let SessionState::Processing { binding_id } = &*state_guard {
        debug!("exit_processing: Exiting Processing for {}", binding_id);
        *state_guard = SessionState::Idle;
    } else {
        debug!("exit_processing: Not in Processing state, ignoring");
    }
}
