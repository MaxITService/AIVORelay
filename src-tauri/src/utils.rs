use crate::managers::audio::AudioRecordingManager;
use crate::managers::deepgram_realtime::DeepgramRealtimeManager;
use crate::managers::deepgram_stt::DeepgramSttManager;
use crate::managers::llm_operation::LlmOperationTracker;
use crate::managers::openai_realtime_whisper::OpenAiRealtimeWhisperManager;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::soniox_realtime::SonioxRealtimeManager;
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

#[cfg(any(test, all(target_os = "windows", target_arch = "x86_64")))]
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xaa64;

#[cfg(any(test, all(target_os = "windows", target_arch = "x86_64")))]
fn native_machine_is_arm64(native_machine: Option<u16>) -> bool {
    native_machine == Some(IMAGE_FILE_MACHINE_ARM64)
}

/// Whether this is the x64 Windows build running under emulation on Windows ARM64.
///
/// Only that exact process/host pairing disables GGML GPU paths. Detection is
/// deliberately fail-open: a native x64 host, an older Windows version without
/// `IsWow64Process2`, or any API error preserves the existing behavior.
pub fn is_windows_x64_emulated_on_arm64() -> bool {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        use std::sync::OnceLock;

        static DETECTED: OnceLock<bool> = OnceLock::new();
        *DETECTED.get_or_init(|| native_machine_is_arm64(native_windows_machine()))
    }

    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    {
        false
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn native_windows_machine() -> Option<u16> {
    use windows::core::{s, w, BOOL};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Threading::GetCurrentProcess;

    type IsWow64Process2 = unsafe extern "system" fn(HANDLE, *mut u16, *mut u16) -> BOOL;

    // Resolve dynamically so merely starting AivoRelay never raises the
    // minimum Windows version.
    unsafe {
        let kernel32 = GetModuleHandleW(w!("kernel32.dll")).ok()?;
        let address = GetProcAddress(kernel32, s!("IsWow64Process2"))?;
        // SAFETY: GetProcAddress returned the documented IsWow64Process2 symbol.
        let is_wow64_process2: IsWow64Process2 = std::mem::transmute(address);
        let mut process_machine = 0u16;
        let mut native_machine = 0u16;
        is_wow64_process2(
            GetCurrentProcess(),
            &mut process_machine,
            &mut native_machine,
        )
        .as_bool()
        .then_some(native_machine)
    }
}

/// Centralized cancellation function that can be called from anywhere in the app.
/// Handles cancelling both recording and transcription operations and updates UI state.
/// This also cancels any ongoing Processing work (transcription, LLM, etc.).
pub fn cancel_current_operation(app: &AppHandle) {
    info!("Initiating operation cancellation...");
    crate::recording_auto_stop::cancel_auto_stop_timer(app);

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
    let mut states = match toggle_state_manager.lock() {
        Ok(states) => states,
        Err(poisoned) => {
            warn!("Toggle state lock poisoned during cancellation; recovering");
            poisoned.into_inner()
        }
    };
    states.active_toggles.values_mut().for_each(|v| *v = false);

    // Cancel any ongoing recording (belt-and-suspenders, session should have done this)
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    audio_manager.cancel_recording();

    // Cancel any in-flight Remote STT requests
    let remote_stt_manager = app.state::<Arc<RemoteSttManager>>();
    remote_stt_manager.cancel();
    let openai_realtime_whisper_manager = app.state::<Arc<OpenAiRealtimeWhisperManager>>();
    openai_realtime_whisper_manager.cancel();
    let soniox_live_manager = app.state::<Arc<SonioxRealtimeManager>>();
    soniox_live_manager.cancel();
    let soniox_stt_manager = app.state::<Arc<SonioxSttManager>>();
    soniox_stt_manager.cancel();
    let deepgram_live_manager = app.state::<Arc<DeepgramRealtimeManager>>();
    deepgram_live_manager.cancel();
    let deepgram_stt_manager = app.state::<Arc<DeepgramSttManager>>();
    deepgram_stt_manager.cancel();
    audio_manager.clear_stream_frame_callback();
    if let Err(e) = crate::clipboard::end_streaming_paste_session(app) {
        warn!(
            "Failed to end streaming clipboard session during cancellation: {}",
            e
        );
    }

    // Cancel any in-flight LLM requests (AI Replace, etc.)
    let llm_tracker = app.state::<Arc<LlmOperationTracker>>();
    llm_tracker.cancel();

    // Ensure UI is in idle state (redundant if session Drop ran, but safe)
    change_tray_icon(app, crate::tray::TrayIconState::Idle);
    hide_recording_overlay(app);
    if crate::managers::preview_output_mode::is_active() {
        crate::managers::preview_output_mode::deactivate_session(app);
        crate::overlay::end_live_preview_session();
        crate::overlay::reset_live_preview(app);
        crate::overlay::hide_live_preview_window(app);
    }

    // Unload model if immediate unload is enabled
    let tm = app.state::<Arc<TranscriptionManager>>();
    tm.cancel_stream();
    tm.cancel_file_transcription();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm64_native_machine_is_the_only_match() {
        assert!(native_machine_is_arm64(Some(IMAGE_FILE_MACHINE_ARM64)));
        assert!(!native_machine_is_arm64(Some(0x8664))); // AMD64
        assert!(!native_machine_is_arm64(Some(0x014c))); // I386
        assert!(!native_machine_is_arm64(None));
    }
}
