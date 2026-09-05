use std::sync::atomic::{AtomicBool, Ordering};

static WEBVIEWS_DISABLED_FOR_SESSION: AtomicBool = AtomicBool::new(false);
const RECOVERY_NOTICE_MARKER: &str = "speech-only-ui-restored.notice";

/// Clipboard restoration on Windows requires an HWND owned by this process.
/// A hidden native window supplies it without starting a browser or renderer.
#[cfg(target_os = "windows")]
pub struct ClipboardOwnerWindow(isize);

#[cfg(target_os = "windows")]
impl ClipboardOwnerWindow {
    pub fn hwnd(&self) -> windows::Win32::Foundation::HWND {
        windows::Win32::Foundation::HWND(self.0 as *mut std::ffi::c_void)
    }
}

#[cfg(target_os = "windows")]
fn create_clipboard_owner_hwnd() -> windows::core::Result<windows::Win32::Foundation::HWND> {
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
    };

    // STATIC is a built-in Windows class; HWND_MESSAGE creates an invisible,
    // non-activating message-only window with no taskbar entry or WebView.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("STATIC"), w!("AivoRelay Clipboard"),
            WINDOW_STYLE::default(), 0, 0, 0, 0, Some(HWND_MESSAGE), None, None, None,
        )
    }
}

#[cfg(target_os = "windows")]
pub fn create_clipboard_owner_window(app: &tauri::AppHandle) -> windows::core::Result<()> {
    use tauri::Manager;

    if app.try_state::<ClipboardOwnerWindow>().is_none() {
        // Called once on the UI thread at startup. Windows owns the window for
        // this process's lifetime and destroys it when the process exits.
        let hwnd = create_clipboard_owner_hwnd()?;
        app.manage(ClipboardOwnerWindow(hwnd.0 as isize));
    }
    Ok(())
}

/// Freezes the persisted WebView preference for the lifetime of this process.
/// Changing the setting from the UI intentionally takes effect after restart.
pub fn initialize(disabled: bool) {
    WEBVIEWS_DISABLED_FOR_SESSION.store(disabled, Ordering::Release);
}

pub fn webviews_disabled() -> bool {
    WEBVIEWS_DISABLED_FOR_SESSION.load(Ordering::Acquire)
}

pub fn ensure_webviews_enabled(feature: &str) -> Result<(), String> {
    if webviews_disabled() {
        Err(format!(
            "{feature} is unavailable while 'Never launch WebView' mode is active"
        ))
    } else {
        Ok(())
    }
}

pub fn mark_recovery_notice(app: &tauri::AppHandle) -> Result<(), String> {
    let path = crate::portable::resolve_app_data(app, RECOVERY_NOTICE_MARKER)
        .map_err(|error| format!("Failed to resolve speech-only recovery marker: {error}"))?;
    std::fs::write(path, b"tray-recovery")
        .map_err(|error| format!("Failed to save speech-only recovery marker: {error}"))
}

pub fn consume_recovery_notice(app: &tauri::AppHandle) -> Result<bool, String> {
    let path = crate::portable::resolve_app_data(app, RECOVERY_NOTICE_MARKER)
        .map_err(|error| format!("Failed to resolve speech-only recovery marker: {error}"))?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "Failed to consume speech-only recovery marker: {error}"
        )),
    }
}

pub fn speech_only_shortcut_allowed(binding_id: &str) -> bool {
    binding_id == "transcribe"
        || binding_id == "ai_replace_selection"
        || binding_id == "cancel"
        || binding_id.starts_with("transcribe_")
}

#[cfg(test)]
mod tests {
    use super::speech_only_shortcut_allowed;

    #[cfg(target_os = "windows")]
    #[test]
    fn clipboard_owner_exists_without_a_webview_or_visible_window() {
        use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, IsWindow, IsWindowVisible};

        let hwnd = super::create_clipboard_owner_hwnd().unwrap();
        let valid = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
        unsafe { DestroyWindow(hwnd) }.unwrap();
        assert!(valid);
        assert!(!visible);
    }

    #[test]
    fn speech_only_mode_allows_transcription_ai_replace_and_cancel_bindings() {
        assert!(speech_only_shortcut_allowed("transcribe"));
        assert!(speech_only_shortcut_allowed("transcribe_meetings"));
        assert!(speech_only_shortcut_allowed("ai_replace_selection"));
        assert!(speech_only_shortcut_allowed("cancel"));

        for blocked in [
            "cycle_profile",
            "live_sound_transcription",
            "read_clipboard",
            "send_screenshot_to_extension",
            "send_to_extension",
            "spawn_button",
            "voice_command",
        ] {
            assert!(!speech_only_shortcut_allowed(blocked), "{blocked}");
        }
    }
}
