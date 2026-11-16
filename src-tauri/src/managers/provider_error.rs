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
        .or_else(|| json.pointer("/detail/code"))
        .or_else(|| json.pointer("/detail/status"))
        .or_else(|| json.pointer("/code"))
        .or_else(|| {
            json.pointer("/message_type")
                .filter(|value| value.as_str() != Some("error"))
        })
        .and_then(Value::as_str);

    for pointer in [
        "/error/message",
        "/detail/message",
        "/error_message",
        "/message",
        "/detail",
        "/error",
        "/title",
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

pub(crate) fn api_key_redaction_marker(api_key: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(api_key.trim().as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("[redacted key, SHA-256: {}]", &digest[..16])
}

/// Redacts a literal API key (with its deterministic SHA-256 marker), ElevenLabs
/// single-use tokens (`sutkn_*`), and `xi-api-key` header/query values from
/// user-facing or logged error strings.
pub(crate) fn redact_secrets(value: &str, api_key: Option<&str>) -> String {
    if value.is_empty() {
        return String::new();
    }

    let mut redacted = value.to_string();
    if let Some(raw_key) = api_key {
        let trimmed = raw_key.trim();
        if !trimmed.is_empty() {
            let marker = api_key_redaction_marker(trimmed);
            if raw_key != trimmed && !raw_key.is_empty() {
                redacted = redacted.replace(raw_key, &marker);
            }
            redacted = redacted.replace(trimmed, &marker);
        }
    }

    // Redact any ElevenLabs single-use token (`sutkn_<token>`) that may appear in URLs or payloads.
    redacted = redact_prefixed_token(&redacted, "sutkn_", "sutkn_[REDACTED]");
    // Redact any residual `xi-api-key` header or query parameter values not already replaced above.
    redacted = redact_xi_api_key_patterns(&redacted);
    redacted
}

fn redact_prefixed_token(input: &str, prefix: &str, replacement: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find(prefix) {
        output.push_str(&rest[..idx]);
        let after_prefix = &rest[idx + prefix.len()..];
        let token_len = after_prefix
            .bytes()
            .take_while(|byte| {
                byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'-' | b'.')
            })
            .count();
        if token_len > 0 && !after_prefix.starts_with("[REDACTED]") {
            output.push_str(replacement);
            rest = &after_prefix[token_len..];
        } else {
            output.push_str(prefix);
            rest = after_prefix;
        }
    }
    output.push_str(rest);
    output
}

fn redact_xi_api_key_patterns(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let needle = "xi-api-key";
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(rel_idx) = lower[cursor..].find(needle) {
        let key_start = cursor + rel_idx;
        let after_name = key_start + needle.len();
        output.push_str(&input[cursor..after_name]);

        let remainder = &input[after_name..];
        let mut chars = remainder.char_indices().peekable();
        let mut sep_end = 0;
        let mut saw_delimiter = false;

        while let Some(&(idx, ch)) = chars.peek() {
            if ch.is_whitespace() || ch == '"' || ch == '\'' {
                sep_end = idx + ch.len_utf8();
                chars.next();
            } else if !saw_delimiter && (ch == ':' || ch == '=') {
                saw_delimiter = true;
                sep_end = idx + ch.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        if !saw_delimiter {
            cursor = after_name;
            continue;
        }

        output.push_str(&remainder[..sep_end]);
        let value_part = &remainder[sep_end..];
        if value_part.starts_with("[redacted key,") || value_part.starts_with("[REDACTED]") {
            cursor = after_name + sep_end;
            continue;
        }

        let secret_len = value_part
            .bytes()
            .take_while(|byte| {
                !byte.is_ascii_whitespace()
                    && !matches!(
                        *byte,
                        b'"' | b'\'' | b',' | b';' | b'&' | b')' | b']' | b'}'
                    )
            })
            .count();
        if secret_len > 0 {
            output.push_str("[REDACTED]");
            cursor = after_name + sep_end + secret_len;
        } else {
            cursor = after_name + sep_end;
        }
    }

    output.push_str(&input[cursor..]);
    output
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
    use super::{parse_provider_error, parse_provider_error_value, redact_secrets};
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
    fn extracts_elevenlabs_detail_object_and_realtime_error_frames() {
        let detail_body = br#"{
            "detail": {
                "status": "invalid_api_key",
                "message": "Invalid API key provided."
            }
        }"#;
        assert_eq!(
            parse_provider_error(detail_body, StatusCode::UNAUTHORIZED),
            "Invalid API key provided. (code: invalid_api_key)"
        );

        let ws_error = json!({
            "message_type": "commit_throttled",
            "error": "Commit throttled due to insufficient audio."
        });
        assert_eq!(
            parse_provider_error_value(&ws_error, "ElevenLabs Realtime error"),
            "Commit throttled due to insufficient audio. (code: commit_throttled)"
        );
    }

    #[test]
    fn redacts_literal_api_keys_single_use_tokens_and_xi_api_key_headers() {
        let key = "xi-secret-12345";
        let raw = format!(
            "Failed with key {key}, header xi-api-key: another-secret-token and token=sutkn_abc123XYZ_99"
        );
        let safe = redact_secrets(&raw, Some(key));
        assert!(!safe.contains(key));
        assert!(!safe.contains("another-secret-token"));
        assert!(!safe.contains("sutkn_abc123XYZ_99"));
        assert!(safe.contains("[redacted key, SHA-256:"));
        assert!(safe.contains("xi-api-key: [REDACTED]"));
        assert!(safe.contains("sutkn_[REDACTED]"));
    }

    #[test]
    fn preserves_plain_provider_errors() {
        assert_eq!(
            parse_provider_error(b"upstream unavailable", StatusCode::BAD_GATEWAY),
            "upstream unavailable"
        );
    }

    #[test]
    fn empty_provider_responses_preserve_the_http_status() {
        assert_eq!(
            parse_provider_error(b" \r\n\t ", StatusCode::UNAUTHORIZED),
            "HTTP 401"
        );
        assert_eq!(
            parse_provider_error(b"{}", StatusCode::SERVICE_UNAVAILABLE),
            "HTTP 503"
        );
    }

    #[test]
    fn nested_provider_details_take_precedence_over_generic_gateway_errors() {
        let payload = json!({
            "error": { "message": "Model is unavailable", "code": "model_not_found" },
            "message": "Gateway request failed",
            "code": "gateway_error"
        });

        assert_eq!(
            parse_provider_error_value(&payload, "HTTP 502"),
            "Model is unavailable (code: model_not_found)"
        );
    }

    #[test]
    fn provider_messages_strip_control_characters_but_keep_readable_layout() {
        let payload = json!({ "message": " \u{0000}Retry\u{0007}\n\tafter 30 seconds\r " });

        assert_eq!(
            parse_provider_error_value(&payload, "HTTP 429"),
            "Retry\n\tafter 30 seconds"
        );
    }

    #[test]
    fn unicode_provider_errors_are_bounded_without_splitting_characters() {
        let message = "界".repeat(2_000);
        assert_eq!(
            parse_provider_error_value(&json!({ "message": &message }), "HTTP 500"),
            message
        );

        let oversized = format!("{}界", message);
        assert_eq!(
            parse_provider_error_value(&json!({ "message": oversized }), "HTTP 500"),
            format!("{}…", message)
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
