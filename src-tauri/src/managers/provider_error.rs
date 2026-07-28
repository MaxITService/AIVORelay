use reqwest::StatusCode;
use serde_json::Value;

const MAX_SAFE_ERROR_CHARS: usize = 2_000;

/// Extract a concise provider message for UI/CLI output while leaving callers
/// free to retain status, timing, and body snippets in diagnostic logs.
pub(crate) fn parse_provider_error(body: &[u8], status: StatusCode) -> String {
    let body = String::from_utf8_lossy(body);
    if let Ok(json) = serde_json::from_str::<Value>(&body) {
        return parse_provider_error_value(&json, &format!("HTTP {}", status.as_u16()));
    }

    let plain = safe_text(&body);
    if plain.is_empty() {
        format!("HTTP {}", status.as_u16())
    } else {
        plain
    }
}

/// Extract the standard message/code pair used by HTTP and WebSocket provider
/// errors. Unknown JSON shapes are still returned in a bounded, safe form.
pub(crate) fn parse_provider_error_value(json: &Value, fallback: &str) -> String {
    let provider_code = json
        .pointer("/error/code")
        .or_else(|| json.pointer("/code"))
        .and_then(Value::as_str);

    for pointer in [
        "/error/message",
        "/error_message",
        "/message",
        "/detail",
        "/error",
    ] {
        if let Some(value) = json.pointer(pointer) {
            if let Some(message) = value.as_str() {
                return safe_text(
                    &provider_code
                        .map(|code| format!("{message} (code: {code})"))
                        .unwrap_or_else(|| message.to_string()),
                );
            }
            if !value.is_null() {
                return safe_text(&value.to_string());
            }
        }
    }

    let compact = safe_text(&json.to_string());
    if compact.is_empty() || compact == "{}" {
        safe_text(fallback)
    } else {
        compact
    }
}

pub(crate) fn safe_text(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if output.chars().count() >= MAX_SAFE_ERROR_CHARS {
            output.push('…');
            break;
        }
        if ch == '\n' || ch == '\t' || !ch.is_control() {
            output.push(ch);
        }
    }
    output.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_provider_error, parse_provider_error_value};
    use reqwest::StatusCode;
    use serde_json::json;

    #[test]
    fn extracts_openai_message_and_code() {
        let body = br#"{
            "error": {
                "message": "You exceeded your current quota.",
                "code": "insufficient_quota"
            }
        }"#;

        assert_eq!(
            parse_provider_error(body, StatusCode::TOO_MANY_REQUESTS),
            "You exceeded your current quota. (code: insufficient_quota)"
        );
    }

    #[test]
    fn preserves_plain_provider_errors() {
        assert_eq!(
            parse_provider_error(b"upstream unavailable", StatusCode::BAD_GATEWAY),
            "upstream unavailable"
        );
    }

    #[test]
    fn websocket_errors_use_the_same_shape() {
        let payload = json!({
            "type": "error",
            "error": {
                "message": "Invalid language code.",
                "code": "invalid_value"
            }
        });

        assert_eq!(
            parse_provider_error_value(&payload, "Provider returned an error"),
            "Invalid language code. (code: invalid_value)"
        );
    }
}
