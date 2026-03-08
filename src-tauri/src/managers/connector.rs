//! Connector Manager - HTTP server for Chrome extension communication
//!
//! This module provides an HTTP server that allows the AivoRelay Chrome extension
//! to poll for messages. It tracks the connection status based on polling activity.
//!
//! Supports long-polling: extension can send `wait=N` query parameter to hold
//! the connection open for up to N seconds waiting for new messages.
//!
//! OWASP hardening notes for this local service:
//! - Minimize attack surface: bind only to 127.0.0.1 and keep the route set small.
//! - Secure by default: reject malformed security settings instead of widening access.
//! - Fail securely: keep rollback paths so a bad change does not silently leave the
//!   service in a weaker or inconsistent state.
//! - Defense in depth: use authenticated encryption, randomized nonces, auth backoff,
//!   and no-store headers on sensitive responses.
//! - Keep the protocol narrow: only expose the headers, methods, and payload shapes
//!   that the extension actually needs.

use crate::settings::{default_connector_password, get_settings, write_settings};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::TcpListener;
use tokio::sync::{Notify, RwLock};
use tower_http::cors::{Any, CorsLayer};

// Crypto and utils
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

/// Default server port (same as test-server.ps1)
const DEFAULT_PORT: u16 = 38243;
/// Timeout in milliseconds - if no poll for this duration, consider disconnected
/// Must be longer than MAX_WAIT_SECONDS to account for long-polling
const POLL_TIMEOUT_MS: i64 = 35_000;
/// Keepalive interval in milliseconds
const KEEPALIVE_INTERVAL_MS: i64 = 15_000;
/// Maximum messages to keep in queue
const MAX_MESSAGES: usize = 100;
/// How long to keep blobs available for download (5 minutes)
const BLOB_EXPIRY_MS: i64 = 300_000;
/// Maximum long-poll wait time in seconds
const MAX_WAIT_SECONDS: u32 = 30;
/// Default long-poll wait (0 = immediate response for backward compat)
const DEFAULT_WAIT_SECONDS: u32 = 0;
/// Cooldown for auth-failure toasts in ms
const AUTH_FAILURE_TOAST_COOLDOWN_MS: i64 = 5000;

/// Extension connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionStatus {
    /// Extension is actively polling
    Online,
    /// Extension has not polled recently
    Offline,
    /// Server is starting up, status unknown
    Unknown,
}

/// Status info returned to frontend
#[derive(Debug, Clone, Serialize, Type)]
pub struct ConnectorStatus {
    pub status: ExtensionStatus,
    /// Last time extension polled (Unix timestamp in ms), 0 if never
    pub last_poll_at: i64,
    /// Server is running
    pub server_running: bool,
    /// Port server is listening on
    pub port: u16,
    /// Last server error (e.g., port binding failure), None if no error
    pub server_error: Option<String>,
}

/// A message in the queue to be sent to extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub text: String,
    pub ts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<BundleAttachment>>,
}

/// Attachment info for bundle messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAttachment {
    #[serde(rename = "attId")]
    pub att_id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    pub fetch: BundleFetch,
}

/// Fetch info for attachments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFetch {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

/// A blob stored for serving to extension
#[derive(Debug, Clone)]
pub struct PendingBlob {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub expires_at: i64,
}

/// Configuration sent to extension
#[derive(Debug, Clone, Serialize)]
struct ExtensionConfig {
    /// URL to auto-open when no tab is bound (empty string = disabled)
    #[serde(rename = "autoOpenTabUrl")]
    auto_open_tab_url: Option<String>,
}

/// Response format for GET /messages
#[derive(Debug, Clone, Serialize)]
struct MessagesResponse {
    cursor: i64,
    messages: Vec<QueuedMessage>,
    config: ExtensionConfig,
    /// New password if auto-generated (extension should save this)
    #[serde(rename = "passwordUpdate", skip_serializing_if = "Option::is_none")]
    password_update: Option<String>,
}

/// POST body from extension (ack or message)
#[derive(Debug, Clone, Deserialize)]
struct PostBody {
    #[serde(rename = "type", default)]
    msg_type: Option<String>,
}

/// Query params for GET /messages
#[derive(Debug, Deserialize)]
struct MessagesQuery {
    since: Option<i64>,
    wait: Option<u32>,
}

/// Event payload for connector-message-queued
#[derive(Debug, Clone, Serialize, Type)]
pub struct MessageQueuedEvent {
    pub id: String,
    pub text: String,
    pub timestamp: i64,
}

/// Event payload for connector-message-delivered
#[derive(Debug, Clone, Serialize, Type)]
pub struct MessageDeliveredEvent {
    pub id: String,
}

/// Event payload for connector-message-cancelled
#[derive(Debug, Clone, Serialize, Type)]
pub struct MessageCancelledEvent {
    pub id: String,
}

/// Internal state shared between handlers
struct ConnectorState {
    /// Queue of messages waiting to be picked up by extension
    messages: VecDeque<QueuedMessage>,
    /// Timestamp of last keepalive sent
    last_keepalive: i64,
    /// Blobs stored for extension to download (attId -> blob data)
    blobs: HashMap<String, PendingBlob>,
    /// Set of message IDs that have been delivered (for deduplication)
    delivered_ids: HashSet<String>,
}

/// Represents our cached symmetric encryption key
#[derive(Clone)]
struct CachedCrypto {
    cipher: Aes256Gcm,
}

impl CachedCrypto {
    /// Expensive operation: derives a 256-bit AES key from the user's password using PBKDF2 with SHA256.
    pub fn new(password: &str) -> Self {
        let mut key = [0u8; 32];
        let salt = b"AivoRelayLocalSecretSalt";
        pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, 100_000, &mut key);
        let aes_key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key);
        let cipher = Aes256Gcm::new(aes_key);
        Self { cipher }
    }

    /// Encrypts a JSON plaintext payload.
    pub fn encrypt_payload(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce_bytes = [0u8; 12];
        getrandom::getrandom(&mut nonce_bytes)
            .map_err(|e| format!("Failed to generate encryption nonce: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("Encryption failure: {:?}", e))?;
        let mut final_payload = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        final_payload.extend_from_slice(&nonce_bytes);
        final_payload.extend_from_slice(&ciphertext);
        Ok(final_payload)
    }
}

#[derive(Clone, Default)]
struct CryptoState {
    current: Option<CachedCrypto>,
    pending: Option<CachedCrypto>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthMatch {
    CurrentPassword,
    PendingPassword,
}

#[derive(Clone, Debug)]
enum CorsPolicy {
    Any,
    Exact(String),
}

/// Shared state for axum handlers
#[derive(Clone)]
struct AppState {
    app_handle: AppHandle,
    state: Arc<Mutex<ConnectorState>>,
    last_poll_at: Arc<AtomicI64>,
    /// Stored for consistency with ConnectorManager; handlers get port from settings
    #[allow(dead_code)]
    port: Arc<RwLock<u16>>,
    /// Notify waiters when a new message is queued
    message_notify: Arc<Notify>,
    /// Cached symmetric keys derived from the current and pending passwords
    crypto: Arc<RwLock<CryptoState>>,
    /// Tracks the timestamp (ms) of the last emitted auth-failed event
    last_auth_failure_emitted_at: Arc<AtomicI64>,
}

pub struct ConnectorManager {
    app_handle: AppHandle,
    /// Timestamp of last poll from extension (atomic for lock-free access)
    last_poll_at: Arc<AtomicI64>,
    /// Whether server is running
    server_running: Arc<AtomicBool>,
    /// Port server is listening on
    port: Arc<RwLock<u16>>,
    /// Shared state for message queue
    state: Arc<Mutex<ConnectorState>>,
    /// Flag to stop the server
    stop_flag: Arc<AtomicBool>,
    /// Notify waiters when a new message is queued
    message_notify: Arc<Notify>,
    /// Last server error (e.g., port binding failure)
    server_error: Arc<RwLock<Option<String>>>,
    /// Cached symmetric keys derived from the current and pending passwords
    crypto: Arc<RwLock<CryptoState>>,
    /// Tracks the timestamp (ms) of the last emitted auth-failed event
    last_auth_failure_emitted_at: Arc<AtomicI64>,
}

impl ConnectorManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let mut settings = get_settings(app_handle);
        maybe_migrate_legacy_connector_password(app_handle, &settings);
        settings = get_settings(app_handle);

        let port = if settings.connector_port > 0 {
            settings.connector_port
        } else {
            DEFAULT_PORT
        };

        let manager = Self {
            app_handle: app_handle.clone(),
            last_poll_at: Arc::new(AtomicI64::new(0)),
            server_running: Arc::new(AtomicBool::new(false)),
            port: Arc::new(RwLock::new(port)),
            state: Arc::new(Mutex::new(ConnectorState {
                messages: VecDeque::new(),
                last_keepalive: 0,
                blobs: HashMap::new(),
                delivered_ids: HashSet::new(),
            })),
            stop_flag: Arc::new(AtomicBool::new(false)),
            message_notify: Arc::new(Notify::new()),
            server_error: Arc::new(RwLock::new(None)),
            crypto: Arc::new(RwLock::new(build_crypto_state(
                &settings.connector_password,
                settings.connector_pending_password.as_deref(),
            ))),
            last_auth_failure_emitted_at: Arc::new(AtomicI64::new(0)),
        };

        Ok(manager)
    }

    /// Start the HTTP server in a background task.
    pub fn start_server(&self) -> Result<(), String> {
        let settings = get_settings(&self.app_handle);
        if !settings.connector_enabled {
            debug!("Connector server is disabled in settings, skipping start.");
            return Ok(());
        }

        if self.server_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let cors_policy = parse_cors_policy(&settings.connector_cors).map_err(|err| {
            *self.server_error.blocking_write() = Some(err.clone());
            let _ = self.app_handle.emit("connector-server-error", err.clone());
            err
        })?;

        let port = {
            let port_guard = self.port.blocking_read();
            *port_guard
        };

        if port < 1024 {
            return Err(format!(
                "Port {} is not allowed. Please use a port number of 1024 or higher.",
                port
            ));
        }

        let addr = format!("127.0.0.1:{}", port);
        let std_listener = match std::net::TcpListener::bind(&addr) {
            Ok(listener) => listener,
            Err(e) => {
                let error_msg = format!("Failed to bind to port {}: {}", port, e);
                error!("Connector server: {}", error_msg);
                *self.server_error.blocking_write() = Some(error_msg.clone());
                let _ = self
                    .app_handle
                    .emit("connector-server-error", error_msg.clone());
                return Err(error_msg);
            }
        };

        if let Err(e) = std_listener.set_nonblocking(true) {
            let error_msg = format!("Failed to configure listener for port {}: {}", port, e);
            error!("Connector server: {}", error_msg);
            *self.server_error.blocking_write() = Some(error_msg.clone());
            let _ = self
                .app_handle
                .emit("connector-server-error", error_msg.clone());
            return Err(error_msg);
        }

        *self.server_error.blocking_write() = None;
        self.server_running.store(true, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);
        self.last_poll_at.store(0, Ordering::SeqCst);

        let app_state = AppState {
            app_handle: self.app_handle.clone(),
            state: self.state.clone(),
            last_poll_at: Arc::clone(&self.last_poll_at),
            port: self.port.clone(),
            message_notify: self.message_notify.clone(),
            crypto: self.crypto.clone(),
            last_auth_failure_emitted_at: self.last_auth_failure_emitted_at.clone(),
        };

        // Use a per-instance stop flag so older restart tasks cannot outlive a fresh server.
        let local_stop_flag = Arc::new(AtomicBool::new(false));
        let global_stop_flag = self.stop_flag.clone();

        let server_running = self.server_running.clone();
        let app_handle = self.app_handle.clone();
        let last_poll_at = self.last_poll_at.clone();
        let state = self.state.clone();
        let server_error = self.server_error.clone();

        tauri::async_runtime::spawn(async move {
            let listener = match TcpListener::from_std(std_listener) {
                Ok(listener) => listener,
                Err(e) => {
                    let error_msg = format!(
                        "Failed to initialize async listener for port {}: {}",
                        port, e
                    );
                    error!("Connector server: {}", error_msg);
                    *server_error.write().await = Some(error_msg.clone());
                    server_running.store(false, Ordering::SeqCst);
                    let _ = app_handle.emit("connector-server-error", error_msg);
                    return;
                }
            };

            info!("Connector server starting on port {}", port);
            let _ = app_handle.emit("extension-status-changed", ExtensionStatus::Unknown);

            let mut cors = CorsLayer::new()
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

            match cors_policy {
                CorsPolicy::Any => {
                    cors = cors.allow_origin(Any);
                }
                CorsPolicy::Exact(origin) => {
                    let origin = HeaderValue::from_str(&origin)
                        .expect("validated origin must be convertible to a header value");
                    cors = cors.allow_origin(origin);
                }
            }

            let router = Router::new()
                .route("/messages", get(handle_get_messages))
                .route("/messages", post(handle_post_messages))
                .route("/blob/{att_id}", get(handle_get_blob))
                .layer(cors)
                .with_state(app_state.clone());

            info!("Connector server listening on {}", addr);

            let status_local_stop_flag = local_stop_flag.clone();
            let status_global_stop_flag = global_stop_flag.clone();
            let status_app_handle = app_handle.clone();
            let status_last_poll = last_poll_at.clone();
            tokio::spawn(async move {
                let mut was_online = false;

                loop {
                    if status_local_stop_flag.load(Ordering::SeqCst)
                        || status_global_stop_flag.load(Ordering::SeqCst)
                    {
                        break;
                    }

                    let now = now_ms();
                    let last_poll = status_last_poll.load(Ordering::SeqCst);

                    if last_poll > 0 {
                        let is_online = (now - last_poll) < POLL_TIMEOUT_MS;
                        if is_online != was_online {
                            let status = if is_online {
                                ExtensionStatus::Online
                            } else {
                                ExtensionStatus::Offline
                            };
                            info!("Extension status changed: {:?}", status);
                            let _ = status_app_handle.emit("extension-status-changed", status);
                            was_online = is_online;
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            });

            let keepalive_local_stop_flag = local_stop_flag.clone();
            let keepalive_global_stop_flag = global_stop_flag.clone();
            let keepalive_state = state.clone();
            tokio::spawn(async move {
                loop {
                    if keepalive_local_stop_flag.load(Ordering::SeqCst)
                        || keepalive_global_stop_flag.load(Ordering::SeqCst)
                    {
                        break;
                    }

                    let now = now_ms();
                    {
                        let mut state_guard = keepalive_state.lock().unwrap();

                        if now - state_guard.last_keepalive > KEEPALIVE_INTERVAL_MS {
                            state_guard.last_keepalive = now;
                            state_guard.messages.push_back(QueuedMessage {
                                id: uuid_simple(),
                                msg_type: "keepalive".to_string(),
                                text: "keepalive".to_string(),
                                ts: now,
                                attachments: None,
                            });

                            while state_guard.messages.len() > MAX_MESSAGES {
                                state_guard.messages.pop_front();
                            }
                        }

                        state_guard.blobs.retain(|_, blob| blob.expires_at > now);
                    }

                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            });

            let graceful_local_stop_flag = local_stop_flag.clone();
            let graceful_global_stop_flag = global_stop_flag.clone();
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    loop {
                        if graceful_local_stop_flag.load(Ordering::SeqCst)
                            || graceful_global_stop_flag.load(Ordering::SeqCst)
                        {
                            break;
                        }

                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                })
                .await
                .unwrap_or_else(|e| {
                    error!("Server error: {}", e);
                });

            local_stop_flag.store(true, Ordering::SeqCst);
            server_running.store(false, Ordering::SeqCst);
            info!("Connector server stopped");
            let _ = app_handle.emit("extension-status-changed", ExtensionStatus::Unknown);
        });

        Ok(())
    }

    /// Stop the HTTP server.
    pub fn stop_server(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Restart the server using the current settings.
    pub fn restart_server(&self) -> Result<(), String> {
        if self.server_running.load(Ordering::SeqCst) {
            self.stop_server();

            let start = std::time::Instant::now();
            while self.server_running.load(Ordering::SeqCst) {
                if start.elapsed() > Duration::from_secs(2) {
                    return Err("Timeout waiting for server to stop".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        self.last_poll_at.store(0, Ordering::SeqCst);
        self.start_server()
    }

    /// Update the port and restart the server if it is running or previously failed.
    pub fn restart_on_port(&self, new_port: u16) -> Result<(), String> {
        let previous_port = *self.port.blocking_read();
        let was_running = self.server_running.load(Ordering::SeqCst);
        let had_previous_error = self.server_error.blocking_read().is_some();

        {
            let mut port_guard = self.port.blocking_write();
            *port_guard = new_port;
        }

        if self.server_running.load(Ordering::SeqCst) || had_previous_error {
            if let Err(restart_err) = self.restart_server() {
                *self.port.blocking_write() = previous_port;

                if was_running && !self.server_running.load(Ordering::SeqCst) {
                    if let Err(rollback_err) = self.start_server() {
                        error!(
                            "Failed to rollback connector port from {} to {}: {}",
                            new_port, previous_port, rollback_err
                        );
                        return Err(format!(
                            "{} (rollback to port {} failed: {})",
                            restart_err, previous_port, rollback_err
                        ));
                    }
                }

                return Err(restart_err);
            }
        }

        Ok(())
    }

    /// Queue a message to be sent to the extension.
    pub fn queue_message(&self, text: &str) -> Result<String, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("Message is empty".to_string());
        }

        let msg_id = uuid_simple();
        let ts = now_ms();

        {
            let mut state = self.state.lock().unwrap();
            state.messages.push_back(QueuedMessage {
                id: msg_id.clone(),
                msg_type: "text".to_string(),
                text: trimmed.to_string(),
                ts,
                attachments: None,
            });

            while state.messages.len() > MAX_MESSAGES {
                state.messages.pop_front();
            }
        }

        self.message_notify.notify_waiters();

        let _ = self.app_handle.emit(
            "connector-message-queued",
            MessageQueuedEvent {
                id: msg_id.clone(),
                text: trimmed.to_string(),
                timestamp: ts,
            },
        );

        Ok(msg_id)
    }

    /// Queue a bundle message with an image attachment.
    pub fn queue_bundle_message(&self, text: &str, image_path: &PathBuf) -> Result<String, String> {
        let data =
            std::fs::read(image_path).map_err(|e| format!("Failed to read image file: {}", e))?;

        let extension = image_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();
        let mime_type = match extension.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            _ => "image/png",
        };

        let filename = image_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        let file_size = data.len() as u64;
        let att_id = uuid_simple();
        let msg_id = uuid_simple();
        let now = now_ms();
        let expires_at = now + BLOB_EXPIRY_MS;

        let port = match self.port.try_read() {
            Ok(guard) => *guard,
            Err(_) => DEFAULT_PORT,
        };
        let fetch_url = format!("http://127.0.0.1:{}/blob/{}", port, att_id);

        let attachment = BundleAttachment {
            att_id: att_id.clone(),
            kind: "image".to_string(),
            filename,
            mime: Some(mime_type.to_string()),
            size: Some(file_size),
            fetch: BundleFetch {
                url: fetch_url,
                method: Some("GET".to_string()),
                headers: None,
                expires_at: Some(expires_at),
            },
        };

        {
            let mut state = self.state.lock().unwrap();
            state.blobs.insert(
                att_id,
                PendingBlob {
                    data,
                    mime_type: mime_type.to_string(),
                    expires_at,
                },
            );
            state.messages.push_back(QueuedMessage {
                id: msg_id.clone(),
                msg_type: "bundle".to_string(),
                text: text.trim().to_string(),
                ts: now,
                attachments: Some(vec![attachment]),
            });

            while state.messages.len() > MAX_MESSAGES {
                state.messages.pop_front();
            }

            state.blobs.retain(|_, blob| blob.expires_at > now);
        }

        self.message_notify.notify_waiters();

        let _ = self.app_handle.emit(
            "connector-message-queued",
            MessageQueuedEvent {
                id: msg_id.clone(),
                text: text.trim().to_string(),
                timestamp: now,
            },
        );

        debug!(
            "Queued bundle message with image attachment ({} bytes)",
            file_size
        );
        Ok(msg_id)
    }

    /// Queue a bundle message with image bytes directly.
    pub fn queue_bundle_message_bytes(
        &self,
        text: &str,
        data: Vec<u8>,
        mime_type: &str,
    ) -> Result<String, String> {
        let file_size = data.len() as u64;
        let att_id = uuid_simple();
        let msg_id = uuid_simple();
        let now = now_ms();
        let expires_at = now + BLOB_EXPIRY_MS;

        let port = match self.port.try_read() {
            Ok(guard) => *guard,
            Err(_) => DEFAULT_PORT,
        };
        let fetch_url = format!("http://127.0.0.1:{}/blob/{}", port, att_id);

        let attachment = BundleAttachment {
            att_id: att_id.clone(),
            kind: "image".to_string(),
            filename: Some(format!(
                "screenshot.{}",
                mime_type.split('/').nth(1).unwrap_or("png")
            )),
            mime: Some(mime_type.to_string()),
            size: Some(file_size),
            fetch: BundleFetch {
                url: fetch_url,
                method: Some("GET".to_string()),
                headers: None,
                expires_at: Some(expires_at),
            },
        };

        {
            let mut state = self.state.lock().unwrap();
            state.blobs.insert(
                att_id,
                PendingBlob {
                    data,
                    mime_type: mime_type.to_string(),
                    expires_at,
                },
            );
            state.messages.push_back(QueuedMessage {
                id: msg_id.clone(),
                msg_type: "bundle".to_string(),
                text: text.trim().to_string(),
                ts: now,
                attachments: Some(vec![attachment]),
            });

            while state.messages.len() > MAX_MESSAGES {
                state.messages.pop_front();
            }

            state.blobs.retain(|_, blob| blob.expires_at > now);
        }

        self.message_notify.notify_waiters();

        let _ = self.app_handle.emit(
            "connector-message-queued",
            MessageQueuedEvent {
                id: msg_id.clone(),
                text: text.trim().to_string(),
                timestamp: now,
            },
        );

        debug!(
            "Queued bundle message with image bytes ({} bytes, {})",
            file_size, mime_type
        );
        Ok(msg_id)
    }

    /// Cancel a queued message if it has not been delivered yet.
    pub fn cancel_queued_message(&self, message_id: &str) -> Result<bool, String> {
        let mut state = self.state.lock().unwrap();

        if state.delivered_ids.contains(message_id) {
            return Ok(false);
        }

        let original_len = state.messages.len();
        state.messages.retain(|m| m.id != message_id);

        if state.messages.len() < original_len {
            drop(state);

            let _ = self.app_handle.emit(
                "connector-message-cancelled",
                MessageCancelledEvent {
                    id: message_id.to_string(),
                },
            );

            info!("Cancelled queued message: {}", message_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get current connection status.
    pub fn get_status(&self) -> ConnectorStatus {
        let last_poll = self.last_poll_at.load(Ordering::SeqCst);
        let now = now_ms();
        let server_running = self.server_running.load(Ordering::SeqCst);
        let port = match self.port.try_read() {
            Ok(guard) => *guard,
            Err(_) => DEFAULT_PORT,
        };
        let server_error = match self.server_error.try_read() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };

        let status = if !server_running || last_poll == 0 {
            ExtensionStatus::Unknown
        } else if (now - last_poll) < POLL_TIMEOUT_MS {
            ExtensionStatus::Online
        } else {
            ExtensionStatus::Offline
        };

        ConnectorStatus {
            status,
            last_poll_at: last_poll,
            server_running,
            port,
            server_error,
        }
    }

    /// Check if the extension is currently online.
    pub fn is_online(&self) -> bool {
        let last_poll = self.last_poll_at.load(Ordering::SeqCst);
        if last_poll == 0 {
            return false;
        }
        (now_ms() - last_poll) < POLL_TIMEOUT_MS
    }

    /// Refresh the cached crypto state after password settings change.
    pub fn refresh_crypto_state(&self, current_password: &str, pending_password: Option<&str>) {
        let mut crypto_guard = self.crypto.blocking_write();
        *crypto_guard = build_crypto_state(current_password, pending_password);
    }
}

// ============================================================================
// Axum Handlers
// ============================================================================

async fn handle_get_messages(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
    query: Query<MessagesQuery>,
) -> Response {
    let settings = get_settings(&app_state.app_handle);

    // Auth check
    let auth_match = if let Some(auth_match) = validate_auth_header(
        &headers,
        &settings.connector_password,
        settings.connector_pending_password.as_deref(),
    ) {
        auth_match
    } else {
        apply_random_auth_delay().await;
        maybe_emit_auth_failure_toast(&app_state);
        return unauthorized_response();
    };

    let now = now_ms();
    let old_poll = app_state.last_poll_at.swap(now, Ordering::SeqCst);

    if old_poll == 0 || (now - old_poll) >= POLL_TIMEOUT_MS {
        info!("Extension connected (polling started)");
        let _ = app_state
            .app_handle
            .emit("extension-status-changed", ExtensionStatus::Online);
    }

    let cursor = query.since.unwrap_or(0);
    let wait_seconds = query
        .wait
        .unwrap_or(DEFAULT_WAIT_SECONDS)
        .min(MAX_WAIT_SECONDS);

    let (messages, delivered_ids) = if wait_seconds > 0 {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(wait_seconds as u64);

        loop {
            let (msgs, ids) = get_pending_messages(&app_state.state, cursor);
            if !msgs.is_empty() {
                break (msgs, ids);
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break (Vec::new(), Vec::new());
            }

            tokio::select! {
                _ = app_state.message_notify.notified() => {
                    continue;
                }
                _ = tokio::time::sleep(remaining) => {
                    break (Vec::new(), Vec::new());
                }
            }
        }
    } else {
        get_pending_messages(&app_state.state, cursor)
    };

    if !delivered_ids.is_empty() {
        let mut state_guard = app_state.state.lock().unwrap();
        for id in &delivered_ids {
            state_guard.delivered_ids.insert(id.clone());
            let _ = app_state
                .app_handle
                .emit("connector-message-delivered", MessageDeliveredEvent { id: id.clone() });
        }

        let current_ids: HashSet<_> = state_guard.messages.iter().map(|m| m.id.clone()).collect();
        state_guard
            .delivered_ids
            .retain(|id| current_ids.contains(id));
    }

    let password_update = maybe_generate_new_password(&app_state.app_handle);
    let auto_open_url =
        if settings.connector_auto_open_enabled && !settings.connector_auto_open_url.is_empty() {
            Some(settings.connector_auto_open_url.clone())
        } else {
            None
        };

    let next_cursor = messages.last().map(|m| m.ts).unwrap_or(cursor);
    let response_body = MessagesResponse {
        cursor: next_cursor,
        messages,
        config: ExtensionConfig {
            auto_open_tab_url: auto_open_url,
        },
        password_update,
    };

    if settings.connector_encryption_enabled {
        let plain_json = match serde_json::to_vec(&response_body) {
            Ok(payload) => payload,
            Err(e) => {
                error!("Failed to serialize connector response for encryption: {}", e);
                return json_response(response_body);
            }
        };
        let crypto_guard = app_state.crypto.read().await;
        let selected_crypto = match auth_match {
            AuthMatch::CurrentPassword => crypto_guard.current.as_ref(),
            AuthMatch::PendingPassword => crypto_guard
                .pending
                .as_ref()
                .or(crypto_guard.current.as_ref()),
        };

        if let Some(crypto) = selected_crypto {
            match crypto.encrypt_payload(&plain_json) {
                Ok(encrypted_bytes) => {
                    let b64_payload = STANDARD.encode(&encrypted_bytes);
                    return apply_security_headers(
                        Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body(Body::from(b64_payload))
                        .unwrap(),
                    );
                }
                Err(e) => error!("Failed to encrypt response: {}", e),
            }
        } else {
            warn!("Connector encryption is enabled but no crypto key is available");
        }
    }

    json_response(response_body)
}

async fn handle_post_messages(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let settings = get_settings(&app_state.app_handle);
    if validate_auth_header(
        &headers,
        &settings.connector_password,
        settings.connector_pending_password.as_deref(),
    )
    .is_none()
    {
        apply_random_auth_delay().await;
        maybe_emit_auth_failure_toast(&app_state);
        return unauthorized_response();
    }

    if let Ok(post_body) = serde_json::from_str::<PostBody>(&body) {
        if post_body.msg_type.as_deref() == Some("password_ack") {
            info!("Extension acknowledged password - committing...");
            commit_pending_password(&app_state.app_handle);
        }
    }

    app_state.last_poll_at.store(now_ms(), Ordering::SeqCst);
    json_response(serde_json::json!({"ok": true}))
}

async fn handle_get_blob(
    State(app_state): State<AppState>,
    Path(att_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    let settings = get_settings(&app_state.app_handle);
    if validate_auth_header(
        &headers,
        &settings.connector_password,
        settings.connector_pending_password.as_deref(),
    )
    .is_none()
    {
        apply_random_auth_delay().await;
        maybe_emit_auth_failure_toast(&app_state);
        return unauthorized_response();
    }

    let blob_data = {
        let mut state_guard = app_state.state.lock().unwrap();
        let now = now_ms();
        state_guard.blobs.retain(|_, b| b.expires_at > now);
        state_guard.blobs.get(&att_id).cloned()
    };

    match blob_data {
        Some(blob) => apply_security_headers(
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, blob.mime_type)
                .body(Body::from(blob.data))
                .unwrap(),
        ),
        None => apply_security_headers((StatusCode::NOT_FOUND, "Blob not found").into_response()),
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn get_pending_messages(
    state: &Arc<Mutex<ConnectorState>>,
    cursor: i64,
) -> (Vec<QueuedMessage>, Vec<String>) {
    let state_guard = state.lock().unwrap();
    let filtered: Vec<_> = state_guard
        .messages
        .iter()
        .filter(|m| {
            if m.ts > cursor {
                return true;
            }
            if m.ts < cursor {
                return false;
            }
            !state_guard.delivered_ids.contains(&m.id)
        })
        .cloned()
        .collect();

    let ids: Vec<_> = filtered.iter().map(|m| m.id.clone()).collect();
    (filtered, ids)
}

async fn apply_random_auth_delay() {
    let mut buffer = [0u8; 1];
    let delay = if getrandom::getrandom(&mut buffer).is_ok() {
        30 + (buffer[0] % 31) as u64
    } else {
        45
    };
    tokio::time::sleep(Duration::from_millis(delay)).await;
}

fn maybe_emit_auth_failure_toast(app_state: &AppState) {
    let now = now_ms();
    loop {
        let last_emitted = app_state
            .last_auth_failure_emitted_at
            .load(Ordering::SeqCst);
        if now - last_emitted <= AUTH_FAILURE_TOAST_COOLDOWN_MS {
            return;
        }

        if app_state
            .last_auth_failure_emitted_at
            .compare_exchange(last_emitted, now, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let _ = app_state.app_handle.emit(
                "connector-auth-failed",
                serde_json::json!({ "message": "Failed connection attempt: Incorrect password" }),
            );
            return;
        }
    }
}

fn unauthorized_response() -> Response {
    apply_security_headers(
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("WWW-Authenticate", "Bearer")
            .body(Body::from("Unauthorized"))
            .unwrap(),
    )
}

fn validate_auth_header(
    headers: &axum::http::HeaderMap,
    expected: &str,
    pending: Option<&str>,
) -> Option<AuthMatch> {
    if expected.is_empty() {
        return None;
    }
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                if constant_time_eq(token.as_bytes(), expected.as_bytes()) {
                    return Some(AuthMatch::CurrentPassword);
                }
                if let Some(p) = pending {
                    if constant_time_eq(token.as_bytes(), p.as_bytes()) {
                        return Some(AuthMatch::PendingPassword);
                    }
                }
            }
        }
    }
    None
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut res = if a.len() == b.len() { 0u8 } else { 1u8 };
    let n = std::cmp::min(a.len(), b.len());
    for i in 0..n {
        res |= a[i] ^ b[i];
    }
    res == 0
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
fn uuid_simple() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:032x}", ts)
}

fn generate_secure_password() -> Option<String> {
    let mut bytes = [0u8; 16];
    if let Err(err) = getrandom::getrandom(&mut bytes) {
        error!(
            "Failed to generate connector password from OS CSPRNG: {}",
            err
        );
        return None;
    }

    let mut result = String::with_capacity(32);
    for byte in bytes {
        result.push_str(&format!("{:02x}", byte));
    }
    Some(result)
}

fn maybe_generate_new_password(app: &AppHandle) -> Option<String> {
    let settings = get_settings(app);
    if let Some(ref pending) = settings.connector_pending_password {
        debug!("Returning existing pending password for extension to acknowledge");
        return Some(pending.clone());
    }

    if settings.connector_password == default_connector_password() {
        let Some(new_password) = generate_secure_password() else {
            warn!("Skipping connector password rotation: secure RNG unavailable");
            return None;
        };

        info!(
            "Generating new secure connector password (default password detected) - awaiting acknowledgement"
        );

        let mut new_settings = settings.clone();
        new_settings.connector_pending_password = Some(new_password.clone());
        new_settings.connector_password_user_set = false;
        write_settings(app, new_settings);

        let connector_manager = app.state::<Arc<ConnectorManager>>();
        connector_manager.refresh_crypto_state(&settings.connector_password, Some(&new_password));

        return Some(new_password);
    }

    None
}

fn commit_pending_password(app: &AppHandle) {
    let settings = get_settings(app);
    if let Some(ref pending) = settings.connector_pending_password {
        info!("Extension acknowledged password - committing new password");

        let mut new_settings = settings.clone();
        new_settings.connector_password = pending.clone();
        new_settings.connector_pending_password = None;
        write_settings(app, new_settings);

        let connector_manager = app.state::<Arc<ConnectorManager>>();
        connector_manager.refresh_crypto_state(&pending, None);
    } else {
        debug!("Received password_ack but no pending password to commit");
    }
}

fn maybe_migrate_legacy_connector_password(
    app: &AppHandle,
    settings: &crate::settings::AppSettings,
) {
    if settings.connector_password_user_set || settings.connector_pending_password.is_some() {
        return;
    }

    let default_password = default_connector_password();
    if settings.connector_password.is_empty() || settings.connector_password == default_password {
        return;
    }

    if !is_probably_autogenerated_password(&settings.connector_password) {
        return;
    }

    info!(
        "Detected legacy auto-generated connector password; migrating to two-phase commit handshake"
    );

    let mut new_settings = settings.clone();
    new_settings.connector_pending_password = Some(settings.connector_password.clone());
    new_settings.connector_password = default_password;
    write_settings(app, new_settings);
}

fn is_probably_autogenerated_password(password: &str) -> bool {
    password.len() == 32
        && password
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn build_crypto_state(current_password: &str, pending_password: Option<&str>) -> CryptoState {
    CryptoState {
        current: if current_password.is_empty() {
            None
        } else {
            Some(CachedCrypto::new(current_password))
        },
        pending: pending_password.and_then(|pending| {
            if pending.is_empty() {
                None
            } else {
                Some(CachedCrypto::new(pending))
            }
        }),
    }
}

pub(crate) fn normalize_connector_cors_setting(raw_value: &str) -> Result<String, String> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() || trimmed == "*" || trimmed.eq_ignore_ascii_case("<any>") {
        return Ok(String::new());
    }

    let uri: Uri = trimmed.parse().map_err(|_| {
        format!(
            "Invalid CORS origin '{}': expected an exact origin like https://chatgpt.com",
            trimmed
        )
    })?;
    let scheme = uri
        .scheme_str()
        .ok_or_else(|| format!("Invalid CORS origin '{}': missing URL scheme", trimmed))?;
    if scheme != "http" && scheme != "https" {
        return Err(format!(
            "Invalid CORS origin '{}': only http:// or https:// origins are allowed",
            trimmed
        ));
    }

    let authority = uri
        .authority()
        .map(|value| value.as_str())
        .ok_or_else(|| format!("Invalid CORS origin '{}': missing host", trimmed))?;
    let path = uri.path();
    if (path != "/" && !path.is_empty()) || uri.query().is_some() {
        return Err(format!(
            "Invalid CORS origin '{}': use only scheme, host, and optional port",
            trimmed
        ));
    }

    Ok(format!("{}://{}", scheme, authority))
}

fn parse_cors_policy(raw_value: &str) -> Result<CorsPolicy, String> {
    let normalized = normalize_connector_cors_setting(raw_value)?;
    if normalized.is_empty() {
        Ok(CorsPolicy::Any)
    } else {
        Ok(CorsPolicy::Exact(normalized))
    }
}

fn apply_security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn json_response<T: Serialize>(payload: T) -> Response {
    apply_security_headers(Json(payload).into_response())
}
