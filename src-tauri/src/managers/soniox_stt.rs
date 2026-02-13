use crate::audio_toolkit::encode_wav_bytes;
use crate::settings::SonioxContext;
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use log::{debug, info, warn};
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const SONIOX_API_URL: &str = "https://api.soniox.com/v1";
const SONIOX_WS_URL: &str = "wss://stt-rt.soniox.com/transcribe-websocket";
const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY_MS: u64 = 500;
const MAX_RETRY_DELAY_MS: u64 = 5000;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 60;
const AUDIO_CHUNK_SIZE_BYTES: usize = 32 * 1024;
const MIN_TIMEOUT_SECONDS: u32 = 5;

#[derive(Serialize)]
struct SonioxStartRequest {
    api_key: String,
    model: String,
    audio_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_hints: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<SonioxContext>,
    // This fallback WS path keeps endpoint detection enabled to ensure the
    // server emits proper completion/finalization signals for full-clip uploads.
    enable_endpoint_detection: bool,
}

#[derive(Deserialize, Debug, Default)]
struct SonioxToken {
    text: String,
    #[serde(default)]
    is_final: bool,
}

#[derive(Deserialize, Debug, Default)]
struct SonioxResponse {
    #[serde(default)]
    tokens: Vec<SonioxToken>,
    #[serde(default)]
    finished: bool,
    #[serde(default)]
    error_code: Option<u16>,
    #[serde(default)]
    error_message: Option<String>,
    #[serde(default, alias = "final_audio_proc_ms")]
    audio_final_proc_ms: Option<u64>,
    #[serde(default, alias = "total_audio_proc_ms")]
    audio_total_proc_ms: Option<u64>,
}

#[derive(Serialize)]
struct CreateTranscriptionRequest {
    file_id: String,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_hints: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<SonioxContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_speaker_diarization: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_language_identification: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct SonioxAsyncTranscriptionOptions {
    pub language_hints: Option<Vec<String>>,
    pub context: Option<SonioxContext>,
    pub enable_speaker_diarization: Option<bool>,
    pub enable_language_identification: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct FileUploadResponse {
    id: String,
}

#[derive(Deserialize, Debug)]
struct CreateTranscriptionResponse {
    id: String,
}

#[derive(Deserialize, Debug)]
struct TranscriptionStatusResponse {
    status: String,
}

#[derive(Deserialize, Debug)]
struct TranscriptResponse {
    text: String,
}

pub struct SonioxSttManager {
    http_client: reqwest::Client,
    /// Monotonically increasing operation ID; when cancel() is called, all
    /// operations started before that point should abort.
    current_operation_id: AtomicU64,
    /// The operation ID at the time cancel() was last called.
    cancelled_before_id: AtomicU64,
}

impl SonioxSttManager {
    pub fn new(_app_handle: &AppHandle) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| anyhow!("Failed to build Soniox HTTP client: {}", e))?;

        Ok(Self {
            http_client,
            current_operation_id: AtomicU64::new(0),
            cancelled_before_id: AtomicU64::new(0),
        })
    }

    /// Returns a new operation ID for tracking cancellation.
    pub fn start_operation(&self) -> u64 {
        self.current_operation_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Marks all operations started before now as cancelled.
    pub fn cancel(&self) {
        let current = self.current_operation_id.load(Ordering::SeqCst);
        self.cancelled_before_id.store(current + 1, Ordering::SeqCst);
        info!("SonioxSttManager: cancelled all operations up to id {}", current + 1);
    }

    /// Returns true if the given operation ID has been cancelled.
    pub fn is_cancelled(&self, operation_id: u64) -> bool {
        operation_id < self.cancelled_before_id.load(Ordering::SeqCst)
    }

    fn ensure_not_cancelled(&self, operation_id: Option<u64>) -> Result<()> {
        if let Some(op_id) = operation_id {
            if self.is_cancelled(op_id) {
                return Err(anyhow!("Transcription cancelled"));
            }
        }
        Ok(())
    }

    async fn with_retry<F, Fut, T>(
        &self,
        operation_name: &str,
        operation_id: Option<u64>,
        mut operation: F,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);

        for attempt in 0..MAX_RETRIES {
            self.ensure_not_cancelled(operation_id)?;

            match operation().await {
                Ok(result) => return Ok(result),
                Err(err)
                    if attempt < MAX_RETRIES - 1
                        && Self::should_retry(operation_name, &err) =>
                {
                    warn!(
                        "{} attempt {}/{} failed: {}. Retrying in {:?}",
                        operation_name,
                        attempt + 1,
                        MAX_RETRIES,
                        err,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, Duration::from_millis(MAX_RETRY_DELAY_MS));
                }
                Err(err) => {
                    return Err(anyhow!(
                        "{} failed after {} attempts: {}",
                        operation_name,
                        MAX_RETRIES,
                        err
                    ));
                }
            }
        }

        unreachable!()
    }

    fn should_retry(operation_name: &str, err: &anyhow::Error) -> bool {
        // For non-live Soniox WS transcription, server-side 408 usually means
        // the request already timed out in a non-recoverable way for this clip.
        // Retrying the same payload tends to extend "sending" UX without benefit.
        if operation_name == "Soniox WebSocket transcription" {
            let message = err.to_string();
            if message.contains("Soniox WebSocket error 408") {
                return false;
            }
        }
        true
    }

    fn normalized_language_hints(language: Option<&str>) -> Option<Vec<String>> {
        let resolution = crate::language_resolver::resolve_requested_language_for_soniox(language);
        match resolution.status {
            crate::language_resolver::SonioxLanguageResolutionStatus::Supported => {
                if let Some(normalized) = &resolution.normalized {
                    debug!(
                        "Soniox request language resolved: '{}' -> '{}'",
                        resolution.original.as_deref().unwrap_or(""),
                        normalized
                    );
                }
            }
            crate::language_resolver::SonioxLanguageResolutionStatus::AutoOrEmpty => {}
            crate::language_resolver::SonioxLanguageResolutionStatus::OsInputUnavailable => {
                warn!(
                    "Soniox language fallback: OS input language could not be resolved, using auto-detect"
                );
            }
            crate::language_resolver::SonioxLanguageResolutionStatus::Unsupported => {
                warn!(
                    "Soniox language fallback: unsupported language '{}' (normalized='{}'), using auto-detect",
                    resolution.original.as_deref().unwrap_or(""),
                    resolution.normalized.as_deref().unwrap_or("")
                );
            }
        }

        resolution.hint.map(|hint| vec![hint])
    }

    fn normalize_model_for_realtime(model: &str) -> String {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return "stt-rt-v4".to_string();
        }

        if let Some(version) = trimmed.strip_prefix("stt-async-v") {
            return format!("stt-rt-v{}", version);
        }

        trimmed.to_string()
    }

    fn normalize_model_for_async(model: &str) -> String {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return "stt-async-v4".to_string();
        }

        if let Some(version) = trimmed.strip_prefix("stt-rt-v") {
            return format!("stt-async-v{}", version);
        }

        trimmed.to_string()
    }

    fn ensure_within_timeout(start: Instant, timeout_seconds: u32) -> Result<()> {
        let timeout = Duration::from_secs(timeout_seconds.max(MIN_TIMEOUT_SECONDS) as u64);
        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Transcription timed out after {} seconds",
                timeout_seconds
            ));
        }
        Ok(())
    }

    fn remaining_timeout(start: Instant, timeout_seconds: u32) -> Result<Duration> {
        let total = Duration::from_secs(timeout_seconds.max(MIN_TIMEOUT_SECONDS) as u64);
        let elapsed = start.elapsed();
        if elapsed >= total {
            return Err(anyhow!(
                "Transcription timed out after {} seconds",
                timeout_seconds
            ));
        }
        Ok(total - elapsed)
    }

    async fn transcribe_once_ws(
        &self,
        operation_id: Option<u64>,
        api_key: &str,
        model: &str,
        timeout_seconds: u32,
        wav_data: &[u8],
        language_hints: Option<Vec<String>>,
        context: Option<SonioxContext>,
    ) -> Result<String> {
        let started = Instant::now();
        self.ensure_not_cancelled(operation_id)?;
        Self::ensure_within_timeout(started, timeout_seconds)?;

        let (stream, _) = timeout(
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            connect_async(SONIOX_WS_URL),
        )
        .await
        .map_err(|_| anyhow!("Timed out while connecting to Soniox WebSocket"))?
        .map_err(|e| anyhow!("Failed to connect to Soniox WebSocket: {}", e))?;

        let (mut write, mut read) = stream.split();

        let start_request = SonioxStartRequest {
            api_key: api_key.to_string(),
            model: model.to_string(),
            audio_format: "auto".to_string(),
            language_hints,
            context,
            // This manager's WS mode is a non-live full-clip upload fallback.
            // We currently keep this fixed instead of exposing the full live
            // tuning surface from soniox_realtime.rs.
            enable_endpoint_detection: true,
        };

        let start_payload = serde_json::to_string(&start_request)
            .map_err(|e| anyhow!("Failed to build Soniox start payload: {}", e))?;

        write
            .send(Message::Text(start_payload.into()))
            .await
            .map_err(|e| anyhow!("Failed to send Soniox start request: {}", e))?;

        for chunk in wav_data.chunks(AUDIO_CHUNK_SIZE_BYTES) {
            self.ensure_not_cancelled(operation_id)?;
            Self::ensure_within_timeout(started, timeout_seconds)?;

            write
                .send(Message::Binary(chunk.to_vec().into()))
                .await
                .map_err(|e| anyhow!("Failed to send audio chunk to Soniox: {}", e))?;
        }

        // Ask Soniox to finalize any pending tail audio before stream close.
        // This improves end-of-utterance completeness for non-live fallback flows
        // (AI Replace / Connector / Screenshot voice text), which do not use the
        // dedicated live session manager.
        write
            .send(Message::Text(r#"{"type":"finalize"}"#.to_string().into()))
            .await
            .map_err(|e| anyhow!("Failed to send Soniox finalize control message: {}", e))?;

        // Empty binary message signals end-of-audio for Soniox WebSocket API.
        write
            .send(Message::Binary(Vec::new().into()))
            .await
            .map_err(|e| anyhow!("Failed to finalize Soniox audio stream: {}", e))?;

        write
            .flush()
            .await
            .map_err(|e| anyhow!("Failed to flush Soniox WebSocket stream: {}", e))?;

        let mut final_tokens: Vec<String> = Vec::new();
        let mut finished = false;

        loop {
            self.ensure_not_cancelled(operation_id)?;
            Self::ensure_within_timeout(started, timeout_seconds)?;

            let wait = Self::remaining_timeout(started, timeout_seconds)?;
            let frame = timeout(wait, read.next())
                .await
                .map_err(|_| anyhow!("Soniox WebSocket read timed out"))?;
            let Some(frame) = frame else {
                break;
            };
            let frame = frame.map_err(|e| anyhow!("Soniox WebSocket read failed: {}", e))?;

            match frame {
                Message::Text(text) => {
                    let payload: SonioxResponse = serde_json::from_str(text.as_ref()).map_err(|e| {
                        let preview: String = text.chars().take(200).collect();
                        anyhow!(
                            "Invalid Soniox WebSocket payload: {} (body: {})",
                            e,
                            preview
                        )
                    })?;

                    if let Some(code) = payload.error_code {
                        let message = payload
                            .error_message
                            .unwrap_or_else(|| "Unknown Soniox WebSocket error".to_string());
                        return Err(anyhow!("Soniox WebSocket error {}: {}", code, message));
                    }

                    for token in payload.tokens.into_iter().filter(|token| token.is_final) {
                        if !token.text.is_empty() && token.text != "<fin>" && token.text != "<end>" {
                            final_tokens.push(token.text);
                        }
                    }

                    if payload.finished {
                        if let Some(ms) = payload.audio_final_proc_ms {
                            debug!("Soniox final audio processing: {}ms", ms);
                        }
                        if let Some(ms) = payload.audio_total_proc_ms {
                            debug!("Soniox total audio processing: {}ms", ms);
                        }
                        finished = true;
                        break;
                    }
                }
                Message::Binary(_) => {
                    // Server binary frames are currently ignored.
                }
                Message::Ping(_) | Message::Pong(_) => {
                    // Managed by tungstenite internals.
                }
                Message::Close(frame) => {
                    if !finished {
                        if let Some(frame) = frame {
                            return Err(anyhow!(
                                "Soniox WebSocket closed before completion (code: {}, reason: {})",
                                frame.code,
                                frame.reason
                            ));
                        }
                        return Err(anyhow!("Soniox WebSocket closed before completion"));
                    }
                    break;
                }
                _ => {
                    // Ignore unsupported frame variants.
                }
            }
        }

        if !finished {
            return Err(anyhow!(
                "Soniox WebSocket transcription did not report completion"
            ));
        }

        Ok(final_tokens.concat())
    }

    async fn transcribe_once_ws_with_callback<F>(
        &self,
        operation_id: Option<u64>,
        api_key: &str,
        model: &str,
        timeout_seconds: u32,
        wav_data: &[u8],
        language_hints: Option<Vec<String>>,
        context: Option<SonioxContext>,
        on_final_chunk: &mut F,
    ) -> Result<String>
    where
        F: FnMut(&str) -> Result<()>,
    {
        let started = Instant::now();
        self.ensure_not_cancelled(operation_id)?;
        Self::ensure_within_timeout(started, timeout_seconds)?;

        let (stream, _) = timeout(
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            connect_async(SONIOX_WS_URL),
        )
        .await
        .map_err(|_| anyhow!("Timed out while connecting to Soniox WebSocket"))?
        .map_err(|e| anyhow!("Failed to connect to Soniox WebSocket: {}", e))?;

        let (mut write, mut read) = stream.split();

        let start_request = SonioxStartRequest {
            api_key: api_key.to_string(),
            model: model.to_string(),
            audio_format: "auto".to_string(),
            language_hints,
            context,
            enable_endpoint_detection: true,
        };

        let start_payload = serde_json::to_string(&start_request)
            .map_err(|e| anyhow!("Failed to build Soniox start payload: {}", e))?;

        write
            .send(Message::Text(start_payload.into()))
            .await
            .map_err(|e| anyhow!("Failed to send Soniox start request: {}", e))?;

        for chunk in wav_data.chunks(AUDIO_CHUNK_SIZE_BYTES) {
            self.ensure_not_cancelled(operation_id)?;
            Self::ensure_within_timeout(started, timeout_seconds)?;

            write
                .send(Message::Binary(chunk.to_vec().into()))
                .await
                .map_err(|e| anyhow!("Failed to send audio chunk to Soniox: {}", e))?;
        }

        // Ask Soniox to finalize pending tail audio before closing the stream.
        write
            .send(Message::Text(r#"{"type":"finalize"}"#.to_string().into()))
            .await
            .map_err(|e| anyhow!("Failed to send Soniox finalize control message: {}", e))?;

        write
            .send(Message::Binary(Vec::new().into()))
            .await
            .map_err(|e| anyhow!("Failed to finalize Soniox audio stream: {}", e))?;

        write
            .flush()
            .await
            .map_err(|e| anyhow!("Failed to flush Soniox WebSocket stream: {}", e))?;

        let mut final_tokens: Vec<String> = Vec::new();
        let mut finished = false;

        loop {
            self.ensure_not_cancelled(operation_id)?;
            Self::ensure_within_timeout(started, timeout_seconds)?;

            let wait = Self::remaining_timeout(started, timeout_seconds)?;
            let frame = timeout(wait, read.next())
                .await
                .map_err(|_| anyhow!("Soniox WebSocket read timed out"))?;
            let Some(frame) = frame else {
                break;
            };
            let frame = frame.map_err(|e| anyhow!("Soniox WebSocket read failed: {}", e))?;

            match frame {
                Message::Text(text) => {
                    let payload: SonioxResponse = serde_json::from_str(text.as_ref()).map_err(|e| {
                        let preview: String = text.chars().take(200).collect();
                        anyhow!(
                            "Invalid Soniox WebSocket payload: {} (body: {})",
                            e,
                            preview
                        )
                    })?;

                    if let Some(code) = payload.error_code {
                        let message = payload
                            .error_message
                            .unwrap_or_else(|| "Unknown Soniox WebSocket error".to_string());
                        return Err(anyhow!("Soniox WebSocket error {}: {}", code, message));
                    }

                    let mut chunk_text = String::new();
                    for token in payload.tokens.into_iter().filter(|token| token.is_final) {
                        if token.text.is_empty() || token.text == "<fin>" || token.text == "<end>" {
                            continue;
                        }
                        chunk_text.push_str(&token.text);
                        final_tokens.push(token.text);
                    }

                    if !chunk_text.is_empty() {
                        on_final_chunk(&chunk_text)?;
                    }

                    if payload.finished {
                        if let Some(ms) = payload.audio_final_proc_ms {
                            debug!("Soniox final audio processing: {}ms", ms);
                        }
                        if let Some(ms) = payload.audio_total_proc_ms {
                            debug!("Soniox total audio processing: {}ms", ms);
                        }
                        finished = true;
                        break;
                    }
                }
                Message::Binary(_) => {}
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(frame) => {
                    if !finished {
                        if let Some(frame) = frame {
                            return Err(anyhow!(
                                "Soniox WebSocket closed before completion (code: {}, reason: {})",
                                frame.code,
                                frame.reason
                            ));
                        }
                        return Err(anyhow!("Soniox WebSocket closed before completion"));
                    }
                    break;
                }
                _ => {}
            }
        }

        if !finished {
            return Err(anyhow!(
                "Soniox WebSocket transcription did not report completion"
            ));
        }

        Ok(final_tokens.concat())
    }

    async fn upload_file_impl(&self, api_key: &str, wav_data: &[u8]) -> Result<String> {
        let part = multipart::Part::bytes(wav_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let form = multipart::Form::new().part("file", part);
        let response = self
            .http_client
            .post(format!("{}/files", SONIOX_API_URL))
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to upload file: {}", body));
        }

        let payload: FileUploadResponse = response.json().await?;
        debug!("Soniox file uploaded: {}", payload.id);
        Ok(payload.id)
    }

    async fn create_transcription_impl(
        &self,
        api_key: &str,
        file_id: &str,
        model: &str,
        language_hints: Option<Vec<String>>,
        context: Option<SonioxContext>,
        enable_speaker_diarization: Option<bool>,
        enable_language_identification: Option<bool>,
    ) -> Result<String> {
        let request = CreateTranscriptionRequest {
            file_id: file_id.to_string(),
            model: model.to_string(),
            language_hints,
            context,
            enable_speaker_diarization,
            enable_language_identification,
        };

        let response = self
            .http_client
            .post(format!("{}/transcriptions", SONIOX_API_URL))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to create transcription: {}", body));
        }

        let payload: CreateTranscriptionResponse = response.json().await?;
        debug!("Soniox transcription created: {}", payload.id);
        Ok(payload.id)
    }

    async fn wait_for_completion(
        &self,
        api_key: &str,
        transcription_id: &str,
        timeout_seconds: u32,
        operation_id: Option<u64>,
    ) -> Result<()> {
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_seconds.max(MIN_TIMEOUT_SECONDS) as u64);
        let mut poll_interval = Duration::from_millis(500);
        let max_poll_interval = Duration::from_secs(5);

        while start.elapsed() < timeout {
            self.ensure_not_cancelled(operation_id)?;

            let response = self
                .http_client
                .get(format!("{}/transcriptions/{}", SONIOX_API_URL, transcription_id))
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await?;

            if !response.status().is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(anyhow!("Failed to check transcription status: {}", body));
            }

            let payload: TranscriptionStatusResponse = response.json().await?;
            match payload.status.as_str() {
                "completed" => return Ok(()),
                "error" => return Err(anyhow!("Transcription failed")),
                _ => {
                    tokio::time::sleep(poll_interval).await;
                    poll_interval = std::cmp::min(poll_interval * 2, max_poll_interval);
                }
            }
        }

        Err(anyhow!(
            "Transcription timed out after {} seconds",
            timeout_seconds
        ))
    }

    async fn get_transcript_impl(&self, api_key: &str, transcription_id: &str) -> Result<String> {
        let response = self
            .http_client
            .get(format!(
                "{}/transcriptions/{}/transcript",
                SONIOX_API_URL, transcription_id
            ))
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to get transcript: {}", body));
        }

        let raw_text = response.text().await?;
        let payload: TranscriptResponse = serde_json::from_str(&raw_text).map_err(|e| {
            anyhow!(
                "Failed to parse transcript response: {} - body: {}",
                e,
                &raw_text[..raw_text.len().min(200)]
            )
        })?;

        Ok(payload.text)
    }

    async fn delete_file(&self, api_key: &str, file_id: &str) -> Result<()> {
        let response = self
            .http_client
            .delete(format!("{}/files/{}", SONIOX_API_URL, file_id))
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to delete file {}: {}",
                file_id,
                response.status()
            ));
        }

        Ok(())
    }

    async fn delete_file_with_retry(&self, api_key: &str, file_id: &str) {
        const CLEANUP_RETRIES: u32 = 3;
        const CLEANUP_DELAY: Duration = Duration::from_secs(1);

        for attempt in 0..CLEANUP_RETRIES {
            match self.delete_file(api_key, file_id).await {
                Ok(()) => {
                    debug!("Soniox cleanup: deleted uploaded file {}", file_id);
                    return;
                }
                Err(err) => {
                    warn!(
                        "Soniox cleanup failed for {} (attempt {}/{}): {}",
                        file_id,
                        attempt + 1,
                        CLEANUP_RETRIES,
                        err
                    );
                    if attempt < CLEANUP_RETRIES - 1 {
                        tokio::time::sleep(CLEANUP_DELAY).await;
                    }
                }
            }
        }
    }

    // Non-live shortcut fallback path: collect full audio locally, then
    // transcribe via Soniox WebSocket in one request/response session.
    pub async fn transcribe(
        &self,
        operation_id: Option<u64>,
        api_key: &str,
        model: &str,
        timeout_seconds: u32,
        audio_samples: &[f32],
        language: Option<&str>,
        context: Option<SonioxContext>,
    ) -> Result<String> {
        if audio_samples.is_empty() {
            return Ok(String::new());
        }
        if api_key.trim().is_empty() {
            return Err(anyhow!("Soniox API key is missing"));
        }

        self.ensure_not_cancelled(operation_id)?;

        let model = Self::normalize_model_for_realtime(model);
        let wav_data = encode_wav_bytes(audio_samples)?;
        let language_hints = Self::normalized_language_hints(language);
        let started_at = Instant::now();

        let text = self
            .with_retry("Soniox WebSocket transcription", operation_id, || async {
                self.transcribe_once_ws(
                    operation_id,
                    api_key,
                    &model,
                    timeout_seconds,
                    &wav_data,
                    language_hints.clone(),
                    context.clone(),
                )
                .await
            })
            .await?;
        info!(
            "Soniox WebSocket transcription completed in {}ms, output_len={}",
            started_at.elapsed().as_millis(),
            text.len()
        );

        Ok(text)
    }

    // Live transcription path with streaming callback for final chunks.
    // NOTE: This path intentionally avoids retry after output has started, to prevent duplicate insertions.
    pub async fn transcribe_with_streaming_callback<F>(
        &self,
        operation_id: Option<u64>,
        api_key: &str,
        model: &str,
        timeout_seconds: u32,
        audio_samples: &[f32],
        language: Option<&str>,
        context: Option<SonioxContext>,
        mut on_final_chunk: F,
    ) -> Result<String>
    where
        F: FnMut(&str) -> Result<()>,
    {
        if audio_samples.is_empty() {
            return Ok(String::new());
        }
        if api_key.trim().is_empty() {
            return Err(anyhow!("Soniox API key is missing"));
        }

        self.ensure_not_cancelled(operation_id)?;

        let model = Self::normalize_model_for_realtime(model);
        let wav_data = encode_wav_bytes(audio_samples)?;
        let language_hints = Self::normalized_language_hints(language);
        let started_at = Instant::now();

        let text = self
            .transcribe_once_ws_with_callback(
                operation_id,
                api_key,
                &model,
                timeout_seconds,
                &wav_data,
                language_hints,
                context,
                &mut on_final_chunk,
            )
            .await?;
        info!(
            "Soniox WebSocket streaming transcription completed in {}ms, output_len={}",
            started_at.elapsed().as_millis(),
            text.len()
        );

        Ok(text)
    }

    // File transcription path: Soniox async REST API.
    // This path uses the async transcriptions contract (language hints +
    // diarization/language-identification flags) and intentionally remains
    // separate from WS/live-only controls.
    pub async fn transcribe_file_async(
        &self,
        operation_id: Option<u64>,
        api_key: &str,
        model: &str,
        timeout_seconds: u32,
        audio_samples: &[f32],
        language: Option<&str>,
        options: SonioxAsyncTranscriptionOptions,
    ) -> Result<String> {
        if audio_samples.is_empty() {
            return Ok(String::new());
        }
        if api_key.trim().is_empty() {
            return Err(anyhow!("Soniox API key is missing"));
        }

        self.ensure_not_cancelled(operation_id)?;

        let model = Self::normalize_model_for_async(model);
        let wav_data = encode_wav_bytes(audio_samples)?;
        let SonioxAsyncTranscriptionOptions {
            language_hints: explicit_language_hints,
            context,
            enable_speaker_diarization,
            enable_language_identification,
        } = options;
        let language_hints =
            explicit_language_hints.or_else(|| Self::normalized_language_hints(language));
        let started_at = Instant::now();

        let file_id = self
            .with_retry("Soniox file upload", operation_id, || async {
                self.upload_file_impl(api_key, &wav_data).await
            })
            .await?;

        let result: Result<String> = async {
            let transcription_id = self
                .with_retry("Soniox create transcription", operation_id, || async {
                    self.create_transcription_impl(
                        api_key,
                        &file_id,
                        &model,
                        language_hints.clone(),
                        context.clone(),
                        enable_speaker_diarization,
                        enable_language_identification,
                    )
                    .await
                })
                .await?;

            self.wait_for_completion(api_key, &transcription_id, timeout_seconds, operation_id)
                .await?;

            self.with_retry("Soniox get transcript", operation_id, || async {
                self.get_transcript_impl(api_key, &transcription_id).await
            })
            .await
        }
        .await;

        self.delete_file_with_retry(api_key, &file_id).await;

        if let Ok(ref text) = result {
            info!(
                "Soniox async REST transcription completed in {}ms, output_len={}",
                started_at.elapsed().as_millis(),
                text.len()
            );
        }

        result
    }
}
