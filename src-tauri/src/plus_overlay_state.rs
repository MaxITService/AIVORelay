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

/// Invalidate pending error auto-hide timers.
/// Call this when showing any non-error overlay state.
pub fn invalidate_error_overlay_auto_hide() {
    OVERLAY_GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// Error categories for overlay display
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum OverlayErrorCategory {
    Auth,
    RateLimited,
    Billing,
    BadRequest,
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
            OverlayErrorCategory::Auth => "Authentication failed",
            OverlayErrorCategory::RateLimited => "Rate limit exceeded",
            OverlayErrorCategory::Billing => "Billing required",
            OverlayErrorCategory::BadRequest => "Invalid request",
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayErrorProvider {
    Soniox,
    OpenAiCompatible,
    Local,
    Extension,
    Unknown,
}

impl OverlayErrorProvider {
    fn code_prefix(&self) -> &'static str {
        match self {
            OverlayErrorProvider::Soniox => "SONIOX",
            OverlayErrorProvider::OpenAiCompatible => "OPENAI",
            OverlayErrorProvider::Local => "LOCAL",
            OverlayErrorProvider::Extension => "EXT",
            OverlayErrorProvider::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayErrorTransport {
    Ws,
    Http,
    Local,
    Unknown,
}

impl OverlayErrorTransport {
    fn code_suffix(&self) -> &'static str {
        match self {
            OverlayErrorTransport::Ws => "WS",
            OverlayErrorTransport::Http => "HTTP",
            OverlayErrorTransport::Local => "LOCAL",
            OverlayErrorTransport::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayErrorPhase {
    Connect,
    Start,
    Stream,
    Finalize,
    Process,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OverlayCanonicalErrorCode {
    EAuth,
    EBadRequest,
    EBilling,
    ERateLimit,
    ETimeout,
    ENetwork,
    EServer,
    EParse,
    EExtensionOffline,
    EMicUnavailable,
    EUnknown,
}

impl OverlayCanonicalErrorCode {
    fn display_text(&self) -> &'static str {
        match self {
            OverlayCanonicalErrorCode::EAuth => "Authentication failed",
            OverlayCanonicalErrorCode::EBadRequest => "Invalid request",
            OverlayCanonicalErrorCode::EBilling => "Billing required",
            OverlayCanonicalErrorCode::ERateLimit => "Rate limit exceeded",
            OverlayCanonicalErrorCode::ETimeout => "Request timed out",
            OverlayCanonicalErrorCode::ENetwork => "Network unavailable",
            OverlayCanonicalErrorCode::EServer => "Server error",
            OverlayCanonicalErrorCode::EParse => "Invalid response",
            OverlayCanonicalErrorCode::EExtensionOffline => "Extension offline",
            OverlayCanonicalErrorCode::EMicUnavailable => "Mic unavailable",
            OverlayCanonicalErrorCode::EUnknown => "Transcription failed",
        }
    }

    fn retryable(&self) -> bool {
        match self {
            OverlayCanonicalErrorCode::EAuth
            | OverlayCanonicalErrorCode::EBadRequest
            | OverlayCanonicalErrorCode::EBilling
            | OverlayCanonicalErrorCode::EMicUnavailable => false,
            OverlayCanonicalErrorCode::ERateLimit
            | OverlayCanonicalErrorCode::ETimeout
            | OverlayCanonicalErrorCode::ENetwork
            | OverlayCanonicalErrorCode::EServer
            | OverlayCanonicalErrorCode::EParse
            | OverlayCanonicalErrorCode::EExtensionOffline
            | OverlayCanonicalErrorCode::EUnknown => true,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct OverlayErrorEnvelope {
    pub provider: OverlayErrorProvider,
    pub transport: OverlayErrorTransport,
    pub canonical_code: OverlayCanonicalErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    pub phase: OverlayErrorPhase,
    pub user_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_message: Option<String>,
    pub retryable: bool,
    pub display_code: String,
}

/// Extended overlay payload with error information
#[derive(Clone, Debug, Serialize)]
pub struct OverlayPayload {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<OverlayErrorCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_envelope: Option<OverlayErrorEnvelope>,
}

fn detect_provider(err_lower: &str) -> OverlayErrorProvider {
    if err_lower.contains("waiting for soniox live session completion")
        || err_lower.contains("soniox live session timed out")
        || err_lower.contains("soniox live session join failed")
    {
        OverlayErrorProvider::Local
    } else if err_lower.contains("soniox") {
        OverlayErrorProvider::Soniox
    } else if err_lower.contains("remote stt")
        || err_lower.contains("openai")
        || err_lower.contains("/audio/transcriptions")
        || err_lower.contains("/audio/translations")
    {
        OverlayErrorProvider::OpenAiCompatible
    } else if err_lower.contains("extension offline") || err_lower.contains("connector") {
        OverlayErrorProvider::Extension
    } else if err_lower.contains("microphone")
        || err_lower.contains("mic ")
        || err_lower.contains("recorder")
    {
        OverlayErrorProvider::Local
    } else {
        OverlayErrorProvider::Unknown
    }
}

fn detect_transport(err_lower: &str) -> OverlayErrorTransport {
    if err_lower.contains("waiting for soniox live session completion")
        || err_lower.contains("soniox live session timed out")
        || err_lower.contains("soniox live session join failed")
    {
        return OverlayErrorTransport::Local;
    }

    if err_lower.contains("websocket") || err_lower.contains(" ws ") || err_lower.contains("wss://")
    {
        OverlayErrorTransport::Ws
    } else if err_lower.contains("status=")
        || err_lower.contains("http")
        || err_lower.contains("request failed")
        || err_lower.contains("response")
    {
        OverlayErrorTransport::Http
    } else if err_lower.contains("local")
        || err_lower.contains("microphone")
        || err_lower.contains("recorder")
    {
        OverlayErrorTransport::Local
    } else {
        OverlayErrorTransport::Unknown
    }
}

fn detect_phase(err_lower: &str) -> OverlayErrorPhase {
    if err_lower.contains("connecting") || err_lower.contains("connect to") {
        OverlayErrorPhase::Connect
    } else if err_lower.contains("start request") {
        OverlayErrorPhase::Start
    } else if err_lower.contains("audio chunk")
        || err_lower.contains("stream")
        || err_lower.contains("keepalive")
        || err_lower.contains("read failed")
    {
        OverlayErrorPhase::Stream
    } else if err_lower.contains("finalize")
        || err_lower.contains("completion")
        || err_lower.contains("finished")
        || err_lower.contains("tail")
    {
        OverlayErrorPhase::Finalize
    } else if err_lower.contains("parse")
        || err_lower.contains("json")
        || err_lower.contains("deserialize")
    {
        OverlayErrorPhase::Process
    } else {
        OverlayErrorPhase::Unknown
    }
}

fn extract_3_digit_status_code(err_string: &str) -> Option<u16> {
    let lower = err_string.to_lowercase();
    let markers = ["status=", "error ", "http ", "code "];

    for marker in markers {
        if let Some(idx) = lower.find(marker) {
            let after = &err_string[idx + marker.len()..];
            let mut digits = String::new();

            for ch in after.chars() {
                if ch.is_ascii_digit() {
                    digits.push(ch);
                    if digits.len() == 3 {
                        break;
                    }
                } else if !digits.is_empty() {
                    break;
                }
            }

            if digits.len() == 3 {
                if let Ok(parsed) = digits.parse::<u16>() {
                    if (100..=599).contains(&parsed) {
                        return Some(parsed);
                    }
                }
            }
        }
    }

    None
}

fn detect_provider_code(err_lower: &str) -> Option<String> {
    if err_lower.contains("rate_limit_exceeded") {
        return Some("rate_limit_exceeded".to_string());
    }
    if err_lower.contains("invalid_request_error") {
        return Some("invalid_request_error".to_string());
    }
    if err_lower.contains("insufficient_quota") {
        return Some("insufficient_quota".to_string());
    }
    if err_lower.contains("invalid_api_key") || err_lower.contains("invalid api key") {
        return Some("invalid_api_key".to_string());
    }
    None
}

fn detect_canonical_code(
    err_lower: &str,
    status_code: Option<u16>,
    provider: &OverlayErrorProvider,
) -> OverlayCanonicalErrorCode {
    if let Some(status) = status_code {
        return match status {
            400 | 404 | 413 | 422 => OverlayCanonicalErrorCode::EBadRequest,
            401 | 403 => OverlayCanonicalErrorCode::EAuth,
            402 => OverlayCanonicalErrorCode::EBilling,
            408 => OverlayCanonicalErrorCode::ETimeout,
            429 => OverlayCanonicalErrorCode::ERateLimit,
            500..=599 => OverlayCanonicalErrorCode::EServer,
            _ => OverlayCanonicalErrorCode::EUnknown,
        };
    }

    if err_lower.contains("extension offline") {
        return OverlayCanonicalErrorCode::EExtensionOffline;
    }
    if err_lower.contains("mic unavailable")
        || err_lower.contains("microphone unavailable")
        || err_lower.contains("no input device")
    {
        return OverlayCanonicalErrorCode::EMicUnavailable;
    }
    if err_lower.contains("invalid api key")
        || err_lower.contains("missing api key")
        || err_lower.contains("unauthorized")
        || err_lower.contains("authentication")
    {
        return OverlayCanonicalErrorCode::EAuth;
    }
    if err_lower.contains("invalid request")
        || err_lower.contains("bad request")
        || err_lower.contains("invalid_request_error")
    {
        return OverlayCanonicalErrorCode::EBadRequest;
    }
    if err_lower.contains("rate limit")
        || err_lower.contains("too many requests")
        || err_lower.contains("concurrent requests")
    {
        return OverlayCanonicalErrorCode::ERateLimit;
    }
    if err_lower.contains("budget exhausted")
        || err_lower.contains("balance exhausted")
        || err_lower.contains("payment required")
    {
        return OverlayCanonicalErrorCode::EBilling;
    }
    if err_lower.contains("timeout")
        || err_lower.contains("timed out")
        || err_lower.contains("input too slow")
    {
        return OverlayCanonicalErrorCode::ETimeout;
    }
    if err_lower.contains("certificate")
        || err_lower.contains("unknownissuer")
        || err_lower.contains("certnotvalidforname")
        || err_lower.contains("expired")
    {
        return OverlayCanonicalErrorCode::ENetwork;
    }
    if err_lower.contains("tls")
        || err_lower.contains("handshake")
        || err_lower.contains("ssl")
        || err_lower.contains("connect")
        || err_lower.contains("network")
        || err_lower.contains("dns")
        || err_lower.contains("resolve")
        || err_lower.contains("unreachable")
    {
        return OverlayCanonicalErrorCode::ENetwork;
    }
    if err_lower.contains("parse")
        || err_lower.contains("json")
        || err_lower.contains("deserialize")
        || err_lower.contains("invalid response")
    {
        return OverlayCanonicalErrorCode::EParse;
    }
    if err_lower.contains("server")
        || err_lower.contains("cannot continue request")
        || err_lower.contains("internal error")
    {
        return OverlayCanonicalErrorCode::EServer;
    }
    if matches!(provider, OverlayErrorProvider::Extension) {
        return OverlayCanonicalErrorCode::EExtensionOffline;
    }

    OverlayCanonicalErrorCode::EUnknown
}

fn canonical_to_category(code: &OverlayCanonicalErrorCode) -> OverlayErrorCategory {
    match code {
        OverlayCanonicalErrorCode::EAuth => OverlayErrorCategory::Auth,
        OverlayCanonicalErrorCode::EBadRequest => OverlayErrorCategory::BadRequest,
        OverlayCanonicalErrorCode::EBilling => OverlayErrorCategory::Billing,
        OverlayCanonicalErrorCode::ERateLimit => OverlayErrorCategory::RateLimited,
        OverlayCanonicalErrorCode::ETimeout => OverlayErrorCategory::Timeout,
        OverlayCanonicalErrorCode::ENetwork => OverlayErrorCategory::NetworkError,
        OverlayCanonicalErrorCode::EServer => OverlayErrorCategory::ServerError,
        OverlayCanonicalErrorCode::EParse => OverlayErrorCategory::ParseError,
        OverlayCanonicalErrorCode::EExtensionOffline => OverlayErrorCategory::ExtensionOffline,
        OverlayCanonicalErrorCode::EMicUnavailable => OverlayErrorCategory::MicrophoneUnavailable,
        OverlayCanonicalErrorCode::EUnknown => OverlayErrorCategory::Unknown,
    }
}

fn detect_specific_category(
    err_lower: &str,
    canonical_code: &OverlayCanonicalErrorCode,
) -> OverlayErrorCategory {
    if err_lower.contains("certificate")
        || err_lower.contains("unknownissuer")
        || err_lower.contains("certnotvalidforname")
        || err_lower.contains("expired")
    {
        return OverlayErrorCategory::TlsCertificate;
    }
    if err_lower.contains("tls")
        || err_lower.contains("handshake")
        || err_lower.contains("ssl")
        || err_lower.contains("secure")
    {
        return OverlayErrorCategory::TlsHandshake;
    }
    canonical_to_category(canonical_code)
}

fn build_display_code(
    provider: &OverlayErrorProvider,
    transport: &OverlayErrorTransport,
    canonical_code: &OverlayCanonicalErrorCode,
    status_code: Option<u16>,
    provider_code: Option<&str>,
) -> String {
    if let Some(code) = status_code {
        return format!(
            "{} {} {}",
            provider.code_prefix(),
            transport.code_suffix(),
            code
        );
    }

    if let Some(raw_code) = provider_code {
        let trimmed = raw_code.trim();
        if !trimmed.is_empty() {
            let short_code = match trimmed {
                "rate_limit_exceeded" => "RATE_LIMIT",
                "invalid_request_error" => "BAD_REQUEST",
                "insufficient_quota" => "BILLING",
                "invalid_api_key" => "AUTH",
                _ => trimmed,
            };
            return format!("{} {}", provider.code_prefix(), short_code.to_uppercase());
        }
    }

    let canonical_label = match canonical_code {
        OverlayCanonicalErrorCode::EAuth => "E_AUTH",
        OverlayCanonicalErrorCode::EBadRequest => "E_BADREQ",
        OverlayCanonicalErrorCode::EBilling => "E_BILL",
        OverlayCanonicalErrorCode::ERateLimit => "E_RATE",
        OverlayCanonicalErrorCode::ETimeout => "E_TIMEOUT",
        OverlayCanonicalErrorCode::ENetwork => "E_NET",
        OverlayCanonicalErrorCode::EServer => "E_SERVER",
        OverlayCanonicalErrorCode::EParse => "E_PARSE",
        OverlayCanonicalErrorCode::EExtensionOffline => "E_EXT",
        OverlayCanonicalErrorCode::EMicUnavailable => "E_MIC",
        OverlayCanonicalErrorCode::EUnknown => "E_UNKNOWN",
    };
    format!("{} {}", provider.code_prefix(), canonical_label)
}

fn sanitize_technical_message(message: &str) -> Option<String> {
    let normalized = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }

    const MAX_CHARS: usize = 86;
    if normalized.chars().count() <= MAX_CHARS {
        return Some(normalized);
    }

    let mut out = String::with_capacity(MAX_CHARS + 3);
    for ch in normalized.chars().take(MAX_CHARS) {
        out.push(ch);
    }
    out.push_str("...");
    Some(out)
}

fn build_error_envelope_from_string(err_string: &str) -> OverlayErrorEnvelope {
    let err_lower = err_string.to_lowercase();
    let provider = detect_provider(&err_lower);
    let transport = detect_transport(&err_lower);
    let status_code = extract_3_digit_status_code(err_string);
    let provider_code = detect_provider_code(&err_lower);
    let canonical_code = detect_canonical_code(&err_lower, status_code, &provider);
    let phase = detect_phase(&err_lower);
    let user_message = canonical_code.display_text().to_string();
    let display_code = build_display_code(
        &provider,
        &transport,
        &canonical_code,
        status_code,
        provider_code.as_deref(),
    );

    OverlayErrorEnvelope {
        provider,
        transport,
        canonical_code: canonical_code.clone(),
        status_code,
        provider_code,
        phase,
        user_message,
        technical_message: sanitize_technical_message(err_string),
        retryable: canonical_code.retryable(),
        display_code,
    }
}

fn build_default_envelope_from_category(
    category: &OverlayErrorCategory,
    error_message: &str,
) -> OverlayErrorEnvelope {
    let canonical_code = match category {
        OverlayErrorCategory::Auth => OverlayCanonicalErrorCode::EAuth,
        OverlayErrorCategory::RateLimited => OverlayCanonicalErrorCode::ERateLimit,
        OverlayErrorCategory::Billing => OverlayCanonicalErrorCode::EBilling,
        OverlayErrorCategory::BadRequest => OverlayCanonicalErrorCode::EBadRequest,
        OverlayErrorCategory::TlsCertificate
        | OverlayErrorCategory::TlsHandshake
        | OverlayErrorCategory::NetworkError => OverlayCanonicalErrorCode::ENetwork,
        OverlayErrorCategory::Timeout => OverlayCanonicalErrorCode::ETimeout,
        OverlayErrorCategory::ServerError => OverlayCanonicalErrorCode::EServer,
        OverlayErrorCategory::ParseError => OverlayCanonicalErrorCode::EParse,
        OverlayErrorCategory::ExtensionOffline => OverlayCanonicalErrorCode::EExtensionOffline,
        OverlayErrorCategory::MicrophoneUnavailable => OverlayCanonicalErrorCode::EMicUnavailable,
        OverlayErrorCategory::Unknown => OverlayCanonicalErrorCode::EUnknown,
    };

    let provider = match category {
        OverlayErrorCategory::ExtensionOffline => OverlayErrorProvider::Extension,
        OverlayErrorCategory::MicrophoneUnavailable => OverlayErrorProvider::Local,
        _ => OverlayErrorProvider::Unknown,
    };
    let transport = if matches!(provider, OverlayErrorProvider::Local) {
        OverlayErrorTransport::Local
    } else {
        OverlayErrorTransport::Unknown
    };

    OverlayErrorEnvelope {
        provider: provider.clone(),
        transport: transport.clone(),
        canonical_code: canonical_code.clone(),
        status_code: None,
        provider_code: None,
        phase: OverlayErrorPhase::Unknown,
        user_message: error_message.to_string(),
        technical_message: sanitize_technical_message(error_message),
        retryable: canonical_code.retryable(),
        display_code: build_display_code(&provider, &transport, &canonical_code, None, None),
    }
}

/// Categorize an error string into an OverlayErrorCategory
pub fn categorize_error(err_string: &str) -> OverlayErrorCategory {
    let envelope = build_error_envelope_from_string(err_string);
    let err_lower = err_string.to_lowercase();
    detect_specific_category(&err_lower, &envelope.canonical_code)
}

fn show_error_overlay_internal(
    app: &AppHandle,
    category: OverlayErrorCategory,
    error_message: Option<String>,
    error_envelope: Option<OverlayErrorEnvelope>,
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
        let resolved_error_envelope = error_envelope.unwrap_or_else(|| {
            build_default_envelope_from_category(&category, &resolved_error_message)
        });
        let payload = OverlayPayload {
            state: "error".to_string(),
            error_category: Some(category),
            error_message: Some(resolved_error_message),
            error_envelope: Some(resolved_error_envelope),
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
    show_error_overlay_internal(app, category, None, None);
}

/// Show the error overlay with a specific message and auto-hide after 3 seconds.
pub fn show_error_overlay_with_message(
    app: &AppHandle,
    category: OverlayErrorCategory,
    message: impl Into<String>,
) {
    show_error_overlay_internal(app, category, Some(message.into()), None);
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
    let mut envelope = build_error_envelope_from_string(err_string);
    let err_lower = err_string.to_lowercase();
    let category = detect_specific_category(&err_lower, &envelope.canonical_code);
    envelope.user_message = category.display_text().to_string();
    log::debug!(
        "Transcription error categorized as {:?}: {}",
        category,
        err_string
    );
    show_error_overlay_internal(
        app,
        category,
        Some(envelope.user_message.clone()),
        Some(envelope),
    );
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

    #[test]
    fn test_categorize_auth() {
        assert!(matches!(
            categorize_error("Remote STT failed: status=401 elapsed_ms=123 body_snippet=Unauthorized"),
            OverlayErrorCategory::Auth
        ));
    }

    #[test]
    fn test_categorize_rate_limited() {
        assert!(matches!(
            categorize_error("Remote STT failed: status=429 elapsed_ms=123 body_snippet=Rate limit exceeded"),
            OverlayErrorCategory::RateLimited
        ));
    }
}
