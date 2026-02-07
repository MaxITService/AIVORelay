//! Error overlay handling for Remote STT API
//! Fork-specific file: Provides error categorization and overlay control for transcription errors.
//!
//! This module handles error states with automatic categorization (TLS, timeout, network, etc.).
//! Note: The "sending" state is handled by overlay.rs for consistency with other overlay states.

use crate::overlay;
use crate::tray::{change_tray_icon, TrayIconState};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager};

static OVERLAY_GENERATION: AtomicU64 = AtomicU64::new(0);

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
    ExtensionOffline,
    MicrophoneUnavailable,
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
            OverlayErrorCategory::ExtensionOffline => "Extension offline",
            OverlayErrorCategory::MicrophoneUnavailable => "Mic unavailable",
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

fn show_error_overlay_internal(
    app: &AppHandle,
    category: OverlayErrorCategory,
    error_message: Option<String>,
) {
    let settings = crate::settings::get_settings(app);
    if settings.overlay_position == crate::settings::OverlayPosition::None {
        // Still need to reset tray icon even if overlay is disabled
        change_tray_icon(app, TrayIconState::Idle);
        return;
    }

    overlay::update_overlay_position(app);

    if let Some(overlay_window) = app.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        overlay::force_overlay_topmost(&overlay_window);

        let resolved_error_message =
            error_message.unwrap_or_else(|| category.display_text().to_string());
        let payload = OverlayPayload {
            state: "error".to_string(),
            error_category: Some(category),
            error_message: Some(resolved_error_message),
        };
        let _ = overlay_window.emit("show-overlay", payload);

        // Generation counter to prevent hiding overlay of new session
        let current_gen = OVERLAY_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

        // Auto-hide after 3 seconds
        let window_clone = overlay_window.clone();
        let app_clone = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            // Only hide if generation hasn't changed (no new overlay shown)
            if OVERLAY_GENERATION.load(Ordering::SeqCst) == current_gen {
                let _ = window_clone.emit("hide-overlay", ());
                std::thread::sleep(std::time::Duration::from_millis(300));
                let _ = window_clone.hide();
                change_tray_icon(&app_clone, TrayIconState::Idle);
            }
        });
    } else {
        // If no overlay window, just reset tray icon
        change_tray_icon(app, TrayIconState::Idle);
    }
}

/// Show the error overlay state with category and auto-hide after 3 seconds.
/// Uses the category display text as overlay message.
pub fn show_error_overlay(app: &AppHandle, category: OverlayErrorCategory) {
    show_error_overlay_internal(app, category, None);
}

/// Show the error overlay with a specific message and auto-hide after 3 seconds.
pub fn show_error_overlay_with_message(
    app: &AppHandle,
    category: OverlayErrorCategory,
    message: impl Into<String>,
) {
    show_error_overlay_internal(app, category, Some(message.into()));
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

/// Show error overlay for microphone unavailability.
/// This is called when the microphone cannot be opened (e.g., device busy, permissions, etc.)
pub fn show_mic_error_overlay(app: &AppHandle) {
    log::warn!("Showing microphone error overlay");
    show_error_overlay(app, OverlayErrorCategory::MicrophoneUnavailable);
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
