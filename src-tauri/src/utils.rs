use crate::managers::audio::AudioRecordingManager;
use crate::managers::deepgram_realtime::DeepgramRealtimeManager;
use crate::managers::deepgram_stt::DeepgramSttManager;
use crate::managers::gemini_realtime::GeminiRealtimeManager;
use crate::managers::llm_operation::LlmOperationTracker;
use crate::managers::openai_realtime_whisper::OpenAiRealtimeWhisperManager;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::reson8_realtime::Reson8RealtimeManager;
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

/// Preserve diagnostic text in development builds. Release builds show it only
/// after the user explicitly enables transcription-text logging.
/// Secrets such as API keys are controlled separately and must never rely on this.
pub fn redact_text(text: &str, allow_in_release: bool) -> &str {
    if cfg!(debug_assertions) || allow_in_release {
        text
    } else {
        "[REDACTED]"
    }
}

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

/// What Windows said about the integrity level of the focused window's process.
#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ForegroundIntegrity {
    Level(u32),
    /// Windows refused the query. It does that for a process that is elevated
    /// under another account, which AivoRelay can't send input to either.
    Denied,
    /// No focused window, or the query failed for another reason.
    Unknown,
}

/// Fail-open: when either level is unknown, AivoRelay pastes as usual.
#[cfg(any(test, target_os = "windows"))]
fn integrity_blocks_input(foreground: ForegroundIntegrity, ours: Option<u32>) -> bool {
    match foreground {
        ForegroundIntegrity::Level(theirs) => ours.is_some_and(|ours| theirs > ours),
        ForegroundIntegrity::Denied => true,
        ForegroundIntegrity::Unknown => false,
    }
}

/// Whether the focused window drops the keystrokes AivoRelay sends it.
///
/// Windows blocks input sent to a process at a higher integrity level, such as
/// an app started with "Run as administrator", and `SendInput` isn't documented
/// to report it. Always `false` on other platforms. See issue #434.
pub fn foreground_window_is_elevated() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Threading::GetCurrentProcess;

        // SAFETY: the pseudo-handle from GetCurrentProcess is always valid and
        // needs no closing.
        let ours = unsafe { process_integrity_level(GetCurrentProcess()) }.ok();
        let foreground = foreground_integrity();
        log::debug!("Integrity levels: foreground {foreground:?}, AivoRelay {ours:?}");
        integrity_blocks_input(foreground, ours)
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(target_os = "windows")]
fn foreground_integrity() -> ForegroundIntegrity {
    use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    // SAFETY: every handle is checked before use, and the process handle is
    // closed before returning.
    let level = unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return ForegroundIntegrity::Unknown;
        }
        let mut process_id = 0u32;
        if GetWindowThreadProcessId(window, Some(&mut process_id)) == 0 {
            return ForegroundIntegrity::Unknown;
        }
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).and_then(|process| {
            let level = process_integrity_level(process);
            let _ = CloseHandle(process);
            level
        })
    };

    match level {
        Ok(level) => ForegroundIntegrity::Level(level),
        Err(e) if e.code() == E_ACCESSDENIED => ForegroundIntegrity::Denied,
        Err(e) => {
            log::debug!("Couldn't read the focused window's integrity level: {e}");
            ForegroundIntegrity::Unknown
        }
    }
}

/// The integrity level of `process`, e.g. `0x2000` for a normal app and
/// `0x3000` for one run as administrator.
///
/// # Safety
/// `process` must be a valid handle with `PROCESS_QUERY_LIMITED_INFORMATION`.
#[cfg(target_os = "windows")]
unsafe fn process_integrity_level(
    process: windows::Win32::Foundation::HANDLE,
) -> windows::core::Result<u32> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
        TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::OpenProcessToken;

    let mut token = HANDLE::default();
    OpenProcessToken(process, TOKEN_QUERY, &mut token)?;

    // A TOKEN_MANDATORY_LABEL followed by the SID it points to; a SID is at
    // most 68 bytes. u64 elements keep the buffer aligned for the struct.
    let mut buffer = [0u64; 16];
    let mut written = 0u32;
    let queried = GetTokenInformation(
        token,
        TokenIntegrityLevel,
        Some(buffer.as_mut_ptr().cast()),
        std::mem::size_of_val(&buffer) as u32,
        &mut written,
    );
    let _ = CloseHandle(token);
    queried?;

    // The level is the SID's last sub-authority.
    let sid = (*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()).Label.Sid;
    let count = u32::from(*GetSidSubAuthorityCount(sid));
    Ok(*GetSidSubAuthority(sid, count.saturating_sub(1)))
}

/// Centralized cancellation function that can be called from anywhere in the app.
/// Handles cancelling both recording and transcription operations and updates UI state.
/// This also cancels any ongoing Processing work (transcription, LLM, etc.).
pub fn cancel_current_operation(app: &AppHandle) {
    let Some(guard) = session_manager::try_begin_operation_control(app, None) else {
        return;
    };
    cancel_current_operation_guarded(app, &guard);
}

pub(crate) fn cancel_current_operation_guarded(
    app: &AppHandle,
    _guard: &session_manager::OperationControlGuard,
) {
    info!("Initiating operation cancellation...");
    // A release still waiting out its X11 auto-repeat grace belongs to the
    // recording being cancelled and must not swallow the next press.
    crate::shortcut::clear_pending_push_to_talk_releases();
    crate::live_output_buffer::cancel(app);
    // Armed recordings end without a result; their cancel key (if any) is pressed.
    crate::early_open::cancel_all();
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
    states.active_presses.clear();
    drop(states);

    crate::actions::clear_forced_post_processing();

    // Cancel any ongoing recording (belt-and-suspenders, session should have done this)
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    audio_manager.cancel_recording();

    // Cancel any in-flight Remote STT requests
    let remote_stt_manager = app.state::<Arc<RemoteSttManager>>();
    remote_stt_manager.cancel();
    let openai_realtime_whisper_manager = app.state::<Arc<OpenAiRealtimeWhisperManager>>();
    openai_realtime_whisper_manager.cancel();
    let gemini_realtime_manager = app.state::<Arc<GeminiRealtimeManager>>();
    gemini_realtime_manager.cancel();
    let reson8_realtime_manager = app.state::<Arc<Reson8RealtimeManager>>();
    reson8_realtime_manager.cancel();
    let soniox_live_manager = app.state::<Arc<SonioxRealtimeManager>>();
    soniox_live_manager.cancel();
    let soniox_stt_manager = app.state::<Arc<SonioxSttManager>>();
    soniox_stt_manager.cancel();
    let deepgram_live_manager = app.state::<Arc<DeepgramRealtimeManager>>();
    deepgram_live_manager.cancel();
    let deepgram_stt_manager = app.state::<Arc<DeepgramSttManager>>();
    deepgram_stt_manager.cancel();
    audio_manager.clear_stream_frame_callback();
    // Skip the cancelled dictation's queued chunks now; a new recording may start
    // before the restore below runs.
    crate::clipboard::cancel_current_dictation_output();
    let app_for_restore = app.clone();
    if let Err(e) = crate::output_dispatch::run(move || {
        if let Err(e) = crate::clipboard::end_streaming_paste_session(&app_for_restore) {
            warn!(
                "Failed to end streaming clipboard session during cancellation: {}",
                e
            );
        }
    }) {
        warn!("Failed to queue clipboard restore during cancellation: {}", e);
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
    tm.cancel_transcription();
    tm.cancel_file_transcription();
    crate::commands::file_transcription::cancel_file_transcription();
    tm.maybe_unload_immediately("cancellation");
    crate::platform_shortcuts::notify_cancel(app);

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

/// Check if running on KDE Plasma desktop environment
#[cfg(target_os = "linux")]
pub fn is_kde_plasma() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_uppercase().contains("KDE"))
        .unwrap_or(false)
        || std::env::var("KDE_SESSION_VERSION").is_ok()
}

/// Check if running on KDE Plasma with Wayland
#[cfg(target_os = "linux")]
pub fn is_kde_wayland() -> bool {
    is_wayland() && is_kde_plasma()
}

/// Check if running on GNOME desktop environment
#[cfg(target_os = "linux")]
pub fn is_gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_uppercase().contains("GNOME"))
        .unwrap_or(false)
}

/// Check if running on GNOME with Wayland
#[cfg(target_os = "linux")]
pub fn is_gnome_wayland() -> bool {
    is_wayland() && is_gnome()
}

/// Returns true when the environment variable is set to a truthy value
/// (e.g. "1", "true", "yes", "on").
/// "0", "false", "no", "off" and empty string are treated as falsy (case-insensitive).
/// Returns false when the variable is not set.
#[cfg(target_os = "linux")]
pub fn env_flag_enabled(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        ),
        Err(_) => false,
    }
}

#[cfg(target_os = "linux")]
static GAMESCOPE_WSI_DISABLED_BY_AIVORELAY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Only set `DISABLE_GAMESCOPE_WSI` when the user hasn't set it (even to empty).
#[cfg(target_os = "linux")]
fn should_disable_gamescope_wsi(existing: Option<&std::ffi::OsStr>) -> bool {
    existing.is_none()
}

/// Disable the Gamescope WSI layer, which aborts during Vulkan device creation
/// on SteamOS (#2264). AivoRelay only uses Vulkan for compute, so it isn't needed.
#[cfg(target_os = "linux")]
pub fn disable_gamescope_wsi_layer() {
    let existing = std::env::var_os("DISABLE_GAMESCOPE_WSI");
    if should_disable_gamescope_wsi(existing.as_deref()) {
        std::env::set_var("DISABLE_GAMESCOPE_WSI", "1");
        GAMESCOPE_WSI_DISABLED_BY_AIVORELAY.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// True when [`disable_gamescope_wsi_layer`] set the variable itself.
#[cfg(target_os = "linux")]
pub fn gamescope_wsi_disabled_by_aivorelay() -> bool {
    GAMESCOPE_WSI_DISABLED_BY_AIVORELAY.load(std::sync::atomic::Ordering::Relaxed)
}

/// The `GDK_BACKEND` value requested through `AIVORELAY_GDK_BACKEND`, if any.
/// Empty or whitespace-only values count as unset.
#[cfg(target_os = "linux")]
fn requested_gdk_backend(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// Copy `AIVORELAY_GDK_BACKEND` into `GDK_BACKEND` when it is set.
///
/// The AppImage's `apprun-hooks/linuxdeploy-plugin-gtk.sh` unconditionally
/// runs `export GDK_BACKEND=x11` (a workaround for tauri-apps/tauri#8541) after
/// the user's environment, so `GDK_BACKEND=wayland ./AivoRelay.AppImage` never
/// reaches AivoRelay. The hook cannot overwrite a variable it does not know about,
/// so `AIVORELAY_GDK_BACKEND=wayland ./AivoRelay.AppImage` is the opt-in to native
/// Wayland (cjpais/Handy#1729, #2034). Must run before GTK initializes.
#[cfg(target_os = "linux")]
pub fn apply_aivorelay_gdk_backend() {
    let requested = std::env::var("AIVORELAY_GDK_BACKEND").ok();
    if let Some(backend) = requested_gdk_backend(requested.as_deref()) {
        std::env::set_var("GDK_BACKEND", backend);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_text_matches_build_privacy_mode() {
        let diagnostic = "private transcription";

        if cfg!(debug_assertions) {
            assert_eq!(redact_text(diagnostic, false), diagnostic);
        } else {
            assert_eq!(redact_text(diagnostic, false), "[REDACTED]");
        }
        assert_eq!(redact_text(diagnostic, true), diagnostic);
    }

    #[test]
    fn arm64_native_machine_is_the_only_match() {
        assert!(native_machine_is_arm64(Some(IMAGE_FILE_MACHINE_ARM64)));
        assert!(!native_machine_is_arm64(Some(0x8664))); // AMD64
        assert!(!native_machine_is_arm64(Some(0x014c))); // I386
        assert!(!native_machine_is_arm64(None));
    }
}
