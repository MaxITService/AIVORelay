use crate::settings::AppSettings;
use serde::Serialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 63155;
const DEFAULT_PATH: &str = "/messages";
const DEFAULT_TIMEOUT_MS: u64 = 3000;

#[derive(Serialize)]
struct ConnectorPayload<'a> {
    text: &'a str,
    ts: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return DEFAULT_PATH.to_string();
    }
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    }
}

fn build_url(settings: &AppSettings) -> Result<reqwest::Url, String> {
    let host = settings.connector_host.trim();
    let host = if host.is_empty() { DEFAULT_HOST } else { host };

    let port = if settings.connector_port == 0 {
        DEFAULT_PORT
    } else {
        settings.connector_port
    };

    let base = format!("http://{}:{}/", host, port);
    let mut url =
        reqwest::Url::parse(&base).map_err(|e| format!("Invalid connector URL: {}", e))?;
    url.set_path(&normalize_path(&settings.connector_path));
    Ok(url)
}

pub async fn send_message(settings: &AppSettings, text: &str) -> Result<(), String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Connector message is empty".to_string());
    }

    let url = build_url(settings)?;
    let payload = ConnectorPayload { text, ts: now_ms() };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .build()
        .map_err(|e| format!("Failed to create connector client: {}", e))?;

    let response = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Connector request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let body = body.trim();
        let detail = if body.is_empty() {
            "No response body".to_string()
        } else {
            body.to_string()
        };
        return Err(format!("Connector HTTP {}: {}", status, detail));
    }

    Ok(())
}
