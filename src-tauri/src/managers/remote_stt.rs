use crate::audio_toolkit::encode_wav_bytes;
use crate::settings::{RemoteSttDebugMode, RemoteSttSettings};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

const REMOTE_STT_SERVICE: &str = "com.pais.handy";
const REMOTE_STT_USER: &str = "remote_stt_api_key";

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
}

#[derive(Default)]
struct DebugBuffer {
    lines: VecDeque<String>,
    cap_normal: usize,
    cap_verbose: usize,
}

impl DebugBuffer {
    fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            cap_normal: 50,
            cap_verbose: 300,
        }
    }

    fn push_line(&mut self, line: String, mode: RemoteSttDebugMode) {
        let cap = match mode {
            RemoteSttDebugMode::Verbose => self.cap_verbose,
            RemoteSttDebugMode::Normal => self.cap_normal,
        };

        self.lines.push_back(line);
        while self.lines.len() > cap {
            self.lines.pop_front();
        }
    }
}

pub struct RemoteSttManager {
    client: reqwest::Client,
    debug: Mutex<DebugBuffer>,
    app_handle: AppHandle,
}

impl RemoteSttManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

        Ok(Self {
            client,
            debug: Mutex::new(DebugBuffer::new()),
            app_handle: app_handle.clone(),
        })
    }

    pub fn get_debug_dump(&self) -> Vec<String> {
        let buffer = self.debug.lock().unwrap();
        buffer.lines.iter().cloned().collect()
    }

    pub fn clear_debug(&self) {
        let mut buffer = self.debug.lock().unwrap();
        buffer.lines.clear();
    }

    fn record_line(&self, settings: &RemoteSttSettings, line: String, is_error: bool) {
        if !settings.debug_capture {
            return;
        }

        if settings.debug_mode == RemoteSttDebugMode::Normal && !is_error {
            return;
        }

        {
            let mut buffer = self.debug.lock().unwrap();
            buffer.push_line(line.clone(), settings.debug_mode);
        }

        let _ = self.app_handle.emit("remote-stt-debug-line", line);
    }

    fn record_info(&self, settings: &RemoteSttSettings, line: String) {
        self.record_line(settings, line, false);
    }

    fn record_error(&self, settings: &RemoteSttSettings, line: String) {
        self.record_line(settings, line, true);
    }

    pub async fn transcribe(
        &self,
        settings: &RemoteSttSettings,
        audio_samples: &[f32],
    ) -> Result<String> {
        if audio_samples.is_empty() {
            return Ok(String::new());
        }

        let base_url = settings.base_url.trim().trim_end_matches('/');
        if base_url.is_empty() {
            let message = "Remote STT base URL is empty".to_string();
            self.record_error(settings, message.clone());
            return Err(anyhow!(message));
        }

        if settings.model_id.trim().is_empty() {
            let message = "Remote STT model ID is empty".to_string();
            self.record_error(settings, message.clone());
            return Err(anyhow!(message));
        }

        let api_key = get_remote_stt_api_key().map_err(|e| {
            let message = format!("Remote STT API key unavailable: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        let wav_bytes = encode_wav_bytes(audio_samples).map_err(|e| {
            let message = format!("Failed to encode WAV: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        let file_size = wav_bytes.len();
        let url = format!("{}/audio/transcriptions", base_url);

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "Remote STT request base_url={} model={} bytes={}",
                    base_url, settings.model_id, file_size
                ),
            );
        }

        let form = reqwest::multipart::Form::new()
            .text("model", settings.model_id.clone())
            .text("response_format", "json".to_string())
            .part(
                "file",
                reqwest::multipart::Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| anyhow!("Failed to build multipart file: {}", e))?,
            );

        let start = Instant::now();
        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                let message = format!("Remote STT request failed: {}", e);
                self.record_error(settings, message.clone());
                anyhow!(message)
            })?;

        let status = response.status();
        let body = response.bytes().await.map_err(|e| {
            let message = format!("Remote STT response read failed: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;
        let elapsed_ms = start.elapsed().as_millis();

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "Remote STT response status={} elapsed_ms={}",
                    status, elapsed_ms
                ),
            );
        }

        if !status.is_success() {
            let snippet = String::from_utf8_lossy(&body);
            let snippet = snippet.chars().take(500).collect::<String>();
            let message = format!(
                "Remote STT failed: status={} elapsed_ms={} body_snippet={}",
                status, elapsed_ms, snippet
            );
            self.record_error(settings, message.clone());
            return Err(anyhow!(message));
        }

        let parsed: TranscriptionResponse = serde_json::from_slice(&body).map_err(|e| {
            let message = format!("Remote STT response parse failed: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!("Remote STT success output_len={}", parsed.text.len()),
            );
        }

        Ok(parsed.text)
    }
}

#[cfg(target_os = "windows")]
pub fn set_remote_stt_api_key(key: &str) -> Result<()> {
    let entry = keyring::Entry::new(REMOTE_STT_SERVICE, REMOTE_STT_USER)?;
    entry
        .set_password(key)
        .map_err(|e| anyhow!("Failed to store API key: {}", e))
}

#[cfg(target_os = "windows")]
pub fn get_remote_stt_api_key() -> Result<String> {
    let entry = keyring::Entry::new(REMOTE_STT_SERVICE, REMOTE_STT_USER)?;
    entry
        .get_password()
        .map_err(|e| anyhow!("Failed to read API key: {}", e))
}

#[cfg(target_os = "windows")]
pub fn clear_remote_stt_api_key() -> Result<()> {
    let entry = keyring::Entry::new(REMOTE_STT_SERVICE, REMOTE_STT_USER)?;
    entry
        .delete_password()
        .map_err(|e| anyhow!("Failed to delete API key: {}", e))
}

#[cfg(target_os = "windows")]
pub fn has_remote_stt_api_key() -> bool {
    get_remote_stt_api_key()
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
pub fn set_remote_stt_api_key(_key: &str) -> Result<()> {
    Err(anyhow!("Remote STT is only available on Windows"))
}

#[cfg(not(target_os = "windows"))]
pub fn get_remote_stt_api_key() -> Result<String> {
    Err(anyhow!("Remote STT is only available on Windows"))
}

#[cfg(not(target_os = "windows"))]
pub fn clear_remote_stt_api_key() -> Result<()> {
    Err(anyhow!("Remote STT is only available on Windows"))
}

#[cfg(not(target_os = "windows"))]
pub fn has_remote_stt_api_key() -> bool {
    false
}
