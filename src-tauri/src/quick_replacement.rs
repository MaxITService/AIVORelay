use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub const BINDING_ID: &str = "quick_text_replacement";
static OPENING: AtomicBool = AtomicBool::new(false);
static READING_SELECTION: AtomicBool = AtomicBool::new(false);

struct SelectionReadGuard;

impl Drop for SelectionReadGuard {
    fn drop(&mut self) {
        READING_SELECTION.store(false, Ordering::SeqCst);
    }
}

pub fn open(app: &AppHandle, initial_from: String, selection_too_long: bool) {
    if crate::webview_mode::webviews_disabled() {
        return;
    }
    crate::show_main_window(app);
    if let Err(error) = app.emit_to(
        "main",
        "open-quick-text-replacement",
        serde_json::json!({
            "initialFrom": initial_from,
            "selectionTooLong": selection_too_long,
        }),
    ) {
        log::warn!("Failed to open quick text replacement: {error}");
    }
}

pub struct QuickReplacementAction;

impl crate::actions::ShortcutAction for QuickReplacementAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        if crate::webview_mode::webviews_disabled()
            || OPENING.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Selection is optional. An unresponsive accessibility provider must
            // not prevent manual entry or block the shortcut dispatch thread.
            // Timing out does not stop a blocking UIA call. Keep at most one
            // such call alive, while allowing later requests to open manually.
            if READING_SELECTION.swap(true, Ordering::SeqCst) {
                open(&app, String::new(), false);
                OPENING.store(false, Ordering::SeqCst);
                return;
            }
            let selection = tokio::time::timeout(
                Duration::from_millis(750),
                tauri::async_runtime::spawn_blocking(
                    || {
                        let _guard = SelectionReadGuard;
                        crate::selection::read_selected_text_without_copying()
                    },
                ),
            )
            .await;
            let (initial_from, selection_too_long) = match selection {
                Ok(Ok(Ok(text))) if text.chars().nth(4096).is_some() => (String::new(), true),
                Ok(Ok(Ok(text))) => (text, false),
                _ => (String::new(), false),
            };
            open(&app, initial_from, selection_too_long);
            OPENING.store(false, Ordering::SeqCst);
        });
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}

    fn is_instant(&self) -> bool {
        true
    }

    fn instant_fire_on_release(&self) -> bool {
        true
    }
}
