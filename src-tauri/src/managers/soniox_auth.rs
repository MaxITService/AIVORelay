use anyhow::{anyhow, Result};
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{header::AUTHORIZATION, HeaderValue, Request},
};

/// Authenticate the upgrade without putting credentials in stream payloads.
pub(super) fn websocket_request(url: &str, api_key: &str) -> Result<Request<()>> {
    if api_key.trim().is_empty() {
        return Err(anyhow!("Soniox API key is missing"));
    }
    let mut authorization = HeaderValue::from_str(&format!("Bearer {}", api_key))
        .map_err(|_| anyhow!("Soniox API key contains invalid HTTP header characters"))?;
    authorization.set_sensitive(true);
    let mut request = url
        .into_client_request()
        .map_err(|_| anyhow!("Failed to build Soniox WebSocket request"))?;
    request.headers_mut().insert(AUTHORIZATION, authorization);
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use tokio_tungstenite::tungstenite::{Error, Message};

    #[test]
    fn websocket_credentials_use_a_sensitive_authorization_header() {
        let request = websocket_request("wss://example.invalid/transcribe", "test-secret").unwrap();
        let authorization = &request.headers()[AUTHORIZATION];

        assert_eq!(authorization.to_str().unwrap(), "Bearer test-secret");
        assert!(authorization.is_sensitive());
        assert!(!format!("{:?}", request).contains("test-secret"));
        assert_eq!(request.uri().to_string(), "wss://example.invalid/transcribe");
    }

    #[test]
    fn invalid_credentials_and_url_errors_do_not_echo_secrets() {
        for key in ["", " \t ", "secret\r\ninjected: value", "secret\0value"] {
            let error = websocket_request("wss://example.invalid/transcribe", key).unwrap_err();
            assert!(error.to_string().starts_with("Soniox API key"));
            assert!(!error.to_string().contains("secret"));
        }
        let error = websocket_request("not a URL containing test-secret", "test-secret").unwrap_err();
        assert_eq!(error.to_string(), "Failed to build Soniox WebSocket request");
    }

    #[tokio::test]
    async fn failed_start_recovers_provider_auth_error_after_unrelated_frames() {
        let mut read = stream::iter(vec![
            Ok(Message::Ping(Vec::new())),
            Ok(Message::Text("not JSON".into())),
            Ok(Message::Text(r#"{"tokens":[]}"#.into())),
            Ok(Message::Text(r#"{"error_code":401,"error_type":"invalid_api_key"}"#.into())),
        ]);

        let error = start_write_error(&mut read, anyhow!("write failed")).await;
        assert_eq!(error.to_string(), "Soniox WebSocket error 401: invalid_api_key");
    }

    #[tokio::test]
    async fn failed_start_prefers_provider_message_over_error_type() {
        let mut read = stream::iter(vec![Ok(Message::Text(
            r#"{"error_code":401,"error_type":"invalid_api_key","error_message":"Invalid credentials"}"#.into(),
        ))]);

        let error = start_write_error(&mut read, anyhow!("write failed")).await;
        assert_eq!(error.to_string(), "Soniox WebSocket error 401: Invalid credentials");
    }

    #[tokio::test]
    async fn failed_start_keeps_write_error_when_server_closes_or_read_fails() {
        for frame in [Ok(Message::Close(None)), Err(Error::ConnectionClosed)] {
            let mut read = stream::iter(vec![frame]);
            let error = start_write_error(&mut read, anyhow!("original write failure")).await;
            assert_eq!(error.to_string(), "original write failure");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn failed_start_recovery_is_bounded_when_server_stays_silent() {
        let mut read = stream::pending::<Result<Message, Error>>();
        let started = tokio::time::Instant::now();

        let error = start_write_error(&mut read, anyhow!("original write failure")).await;
        assert_eq!(error.to_string(), "original write failure");
        assert_eq!(started.elapsed(), std::time::Duration::from_millis(250));
    }

    #[tokio::test]
    async fn failed_start_recovery_is_bounded_for_continuously_ready_frames() {
        let mut read = stream::repeat_with(|| Ok(Message::Ping(Vec::new())));
        let started = tokio::time::Instant::now();

        let error = start_write_error(&mut read, anyhow!("original write failure")).await;
        assert_eq!(error.to_string(), "original write failure");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}

/// On a failed start write, prefer the server's already-sent auth error to a
/// generic closed-socket error. Never delay a successful stream setup.
pub(super) async fn start_write_error<R>(read: &mut R, fallback: anyhow::Error) -> anyhow::Error
where
    R: futures_util::Stream<Item = Result<tokio_tungstenite::tungstenite::Message,
        tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;
    let recovery = async {
        while let Some(Ok(frame)) = read.next().await {
            match frame {
                Message::Text(text) => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text.as_ref()) {
                        if let Some(code) = value.get("error_code").and_then(|code| code.as_u64()) {
                            let message = value.get("error_message")
                                .and_then(|value| value.as_str())
                                .or_else(|| value.get("error_type").and_then(|value| value.as_str()))
                                .unwrap_or("Unknown Soniox WebSocket error");
                            return Some(anyhow!("Soniox WebSocket error {}: {}", code, message));
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
            // A continuously ready stream must still let the timeout run.
            tokio::task::yield_now().await;
        }
        None
    };
    match tokio::time::timeout(std::time::Duration::from_millis(250), recovery).await {
        Ok(Some(error)) => error,
        _ => fallback,
    }
}
