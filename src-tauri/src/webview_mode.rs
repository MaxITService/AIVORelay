use std::sync::atomic::{AtomicBool, Ordering};

static WEBVIEWS_DISABLED_FOR_SESSION: AtomicBool = AtomicBool::new(false);

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

pub fn speech_only_shortcut_allowed(binding_id: &str) -> bool {
    binding_id == "transcribe"
        || binding_id == "cancel"
        || binding_id.starts_with("transcribe_")
}

#[cfg(test)]
mod tests {
    use super::speech_only_shortcut_allowed;

    #[test]
    fn speech_only_mode_allows_only_transcription_and_cancel_bindings() {
        assert!(speech_only_shortcut_allowed("transcribe"));
        assert!(speech_only_shortcut_allowed("transcribe_meetings"));
        assert!(speech_only_shortcut_allowed("cancel"));

        for blocked in [
            "ai_replace_selection",
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
