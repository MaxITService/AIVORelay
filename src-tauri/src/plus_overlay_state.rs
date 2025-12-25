//! Extended overlay states for Remote STT API
//! Fork-specific file: Provides error categorization and overlay control for transcription errors.
//!
//! This module adds "sending" and "error" states to the overlay,
//! with automatic error categorization (TLS, timeout, network, etc.).

use crate::overlay;
use crate::tray::{change_tray_icon, TrayIconState};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Error categories for overlay display
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum OverlayErrorCategory {
    TlsCertificate,
    TlsHandshake,
    Timeout,
    NetworkError,
    ServerError,
    ParseError,
    Unknown,
}

impl OverlayErrorCategory {
    /// Get the display text for this error category (English only)
    pub fn display_text(&self) -> &'static str {
        match self {
            OverlayErrorCategory::TlsCertificate => "Certificate error",
            OverlayErrorCategory::TlsHandshake => "Connection failed",
            OverlayErrorCategory::Timeout => "Request timed out",
            OverlayErrorCategory::NetworkError => "Network unavailable",
            OverlayErrorCategory::ServerError => "Server error",
            OverlayErrorCategory::ParseError => "Invalid response",
            OverlayErrorCategory::Unknown => "Transcription failed",
        }
    }
}

/// Extended overlay payload with error information
#[derive(Clone, Debug, Serialize)]
pub struct OverlayPayload {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<OverlayErrorCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Categorize an error string into an OverlayErrorCategory
pub fn categorize_error(err_string: &str) -> OverlayErrorCategory {
    let err_lower = err_string.to_lowercase();

    if err_lower.contains("certificate")
        || err_lower.contains("unknownissuer")
        || err_lower.contains("certnotvalidforname")
        || err_lower.contains("expired")
    {
        OverlayErrorCategory::TlsCertificate
    } else if err_lower.contains("tls")
        || err_lower.contains("handshake")
        || err_lower.contains("ssl")
        || err_lower.contains("secure")
    {
        OverlayErrorCategory::TlsHandshake
    } else if err_lower.contains("timeout") || err_lower.contains("timed out") {
        OverlayErrorCategory::Timeout
    } else if err_lower.contains("connect")
        || err_lower.contains("network")
        || err_lower.contains("dns")
        || err_lower.contains("resolve")
        || err_lower.contains("unreachable")
    {
        OverlayErrorCategory::NetworkError
    } else if err_lower.contains("status=")
        || err_lower.contains("server")
        || err_lower.contains("500")
        || err_lower.contains("502")
        || err_lower.contains("503")
        || err_lower.contains("504")
    {
        OverlayErrorCategory::ServerError
    } else if err_lower.contains("parse")
        || err_lower.contains("json")
        || err_lower.contains("deserialize")
        || err_lower.contains("invalid response")
    {
        OverlayErrorCategory::ParseError
    } else {
        OverlayErrorCategory::Unknown
    }
}

/// Show the "sending" overlay state
pub fn show_sending_overlay(app: &AppHandle) {
    let settings = crate::settings::get_settings(app);
    if settings.overlay_position == crate::settings::OverlayPosition::None {
        return;
    }

    overlay::update_overlay_position(app);

    if let Some(overlay_window) = app.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
            };
            let overlay_clone = overlay_window.clone();
            let _ = overlay_clone.clone().run_on_main_thread(move || {
                if let Ok(hwnd) = overlay_clone.hwnd() {
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                    }
                }
            });
        }

        let payload = OverlayPayload {
            state: "sending".to_string(),
            error_category: None,
            error_message: None,
        };
        let _ = overlay_window.emit("show-overlay", payload);
    }
}

/// Show the error overlay state with category and auto-hide after 3 seconds
pub fn show_error_overlay(app: &AppHandle, category: OverlayErrorCategory) {
    let settings = crate::settings::get_settings(app);
    if settings.overlay_position == crate::settings::OverlayPosition::None {
        // Still need to reset tray icon even if overlay is disabled
        change_tray_icon(app, TrayIconState::Idle);
        return;
    }

    if let Some(overlay_window) = app.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
            };
            let overlay_clone = overlay_window.clone();
            let _ = overlay_clone.clone().run_on_main_thread(move || {
                if let Ok(hwnd) = overlay_clone.hwnd() {
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                    }
                }
            });
        }

        let display_text = category.display_text().to_string();
        let payload = OverlayPayload {
            state: "error".to_string(),
            error_category: Some(category),
            error_message: Some(display_text),
        };
        let _ = overlay_window.emit("show-overlay", payload);

        // Auto-hide after 3 seconds
        let window_clone = overlay_window.clone();
        let app_clone = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let _ = window_clone.emit("hide-overlay", ());
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = window_clone.hide();
            change_tray_icon(&app_clone, TrayIconState::Idle);
        });
    } else {
        // If no overlay window, just reset tray icon
        change_tray_icon(app, TrayIconState::Idle);
    }
}

/// Main hook function: handle transcription errors with categorized overlay
/// 
/// This function:
/// 1. Categorizes the error
/// 2. Shows error overlay for 3 seconds
/// 3. Auto-hides overlay and resets tray icon
/// 
/// Note: The existing toast (remote-stt-error event) should still be emitted separately
pub fn handle_transcription_error(app: &AppHandle, err_string: &str) {
    let category = categorize_error(err_string);
    log::debug!(
        "Transcription error categorized as {:?}: {}",
        category,
        err_string
    );
    show_error_overlay(app, category);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_tls_certificate() {
        assert!(matches!(
            categorize_error("invalid peer certificate: UnknownIssuer"),
            OverlayErrorCategory::TlsCertificate
        ));
        assert!(matches!(
            categorize_error("certificate has expired"),
            OverlayErrorCategory::TlsCertificate
        ));
    }

    #[test]
    fn test_categorize_timeout() {
        assert!(matches!(
            categorize_error("request timed out"),
            OverlayErrorCategory::Timeout
        ));
    }

    #[test]
    fn test_categorize_network() {
        assert!(matches!(
            categorize_error("error trying to connect"),
            OverlayErrorCategory::NetworkError
        ));
        assert!(matches!(
            categorize_error("dns resolution failed"),
            OverlayErrorCategory::NetworkError
        ));
    }

    #[test]
    fn test_categorize_server() {
        assert!(matches!(
            categorize_error("status=500"),
            OverlayErrorCategory::ServerError
        ));
    }

    #[test]
    fn test_categorize_parse() {
        assert!(matches!(
            categorize_error("failed to parse JSON"),
            OverlayErrorCategory::ParseError
        ));
    }

    #[test]
    fn test_categorize_unknown() {
        assert!(matches!(
            categorize_error("something weird happened"),
            OverlayErrorCategory::Unknown
        ));
    }
}
