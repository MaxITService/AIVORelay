//! Connector Manager - HTTP server for Chrome extension communication
//!
//! This module provides an HTTP server that allows the AivoRelay Chrome extension
//! to poll for messages. It tracks the connection status based on polling activity.
//!
//! Supports long-polling: extension can send `wait=N` query parameter to hold
//! the connection open for up to N seconds waiting for new messages.

use crate::settings::{default_connector_password, get_settings, write_settings};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::{Notify, RwLock};
use tower_http::cors::{Any, CorsLayer};

/// Default server port (same as test-server.ps1)
const DEFAULT_PORT: u16 = 63155;
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
    #[serde(default)]
    text: Option<String>,
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
}

impl ConnectorManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let settings = get_settings(app_handle);
        maybe_migrate_legacy_connector_password(app_handle, &settings);

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
        };

        Ok(manager)
    }

    /// Start the HTTP server in a background task
    pub fn start_server(&self) -> Result<(), String> {
        if self.server_running.load(Ordering::SeqCst) {
            return Ok(()); // Already running
        }

        let port = {
            let port_guard = self.port.blocking_read();
            *port_guard
        };

        // Validate port range
        if port < 1024 {
            return Err(format!(
                "Port {} is not allowed. Please use a port number of 1024 or higher.",
                port
            ));
        }

        self.server_running.store(true, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);

        let app_state = AppState {
            app_handle: self.app_handle.clone(),
            state: self.state.clone(),
            last_poll_at: Arc::clone(&self.last_poll_at),
            port: self.port.clone(),
            message_notify: self.message_notify.clone(),
        };

        let stop_flag = self.stop_flag.clone();
        let server_running = self.server_running.clone();
        let app_handle = self.app_handle.clone();
        let last_poll_at = self.last_poll_at.clone();
        let state = self.state.clone();
        let server_error = self.server_error.clone();

        tauri::async_runtime::spawn(async move {
            info!("Connector server starting on port {}", port);

            // Emit initial status
            let _ = app_handle.emit("extension-status-changed", ExtensionStatus::Unknown);

            // Build router with CORS
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

            let router = Router::new()
                .route("/messages", get(handle_get_messages))
                .route("/messages", post(handle_post_messages))
                .route("/blob/{att_id}", get(handle_get_blob))
                .layer(cors)
                .with_state(app_state.clone());

            let addr = format!("127.0.0.1:{}", port);
            let listener = match TcpListener::bind(&addr).await {
                Ok(l) => {
                    // Clear any previous error on successful bind
                    {
                        let mut err_guard = server_error.write().await;
                        *err_guard = None;
                    }
                    l
                }
                Err(e) => {
                    let error_msg = format!("Failed to bind to port {}: {}", port, e);
                    error!("Connector server: {}", error_msg);

                    // Store the error for status display
                    {
                        let mut err_guard = server_error.write().await;
                        *err_guard = Some(error_msg.clone());
                    }

                    // Emit error event so UI can display it
                    let _ = app_handle.emit("connector-server-error", error_msg);

                    server_running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            info!("Connector server listening on {}", addr);

            // Spawn status check task
            let status_stop_flag = stop_flag.clone();
            let status_app_handle = app_handle.clone();
            let status_last_poll = last_poll_at.clone();
            tokio::spawn(async move {
                let mut was_online = false;
                loop {
                    if status_stop_flag.load(Ordering::SeqCst) {
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

            // Spawn keepalive task
            let keepalive_stop_flag = stop_flag.clone();
            let keepalive_state = state.clone();
            tokio::spawn(async move {
                loop {
                    if keepalive_stop_flag.load(Ordering::SeqCst) {
                        break;
                    }

                    let now = now_ms();
                    {
                        let mut state_guard = keepalive_state.lock().unwrap();
                        if now - state_guard.last_keepalive > KEEPALIVE_INTERVAL_MS {
                            state_guard.last_keepalive = now;

                            let keepalive = QueuedMessage {
                                id: uuid_simple(),
                                msg_type: "keepalive".to_string(),
                                text: "keepalive".to_string(),
                                ts: now,
                                attachments: None,
                            };

                            state_guard.messages.push_back(keepalive);

                            // Trim old messages
                            while state_guard.messages.len() > MAX_MESSAGES {
                                state_guard.messages.pop_front();
                            }
                        }
                    }

                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            });

            // Serve requests using axum's built-in serve function
            // We use a graceful shutdown triggered by the stop flag
            let graceful_stop_flag = stop_flag.clone();
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    loop {
                        if graceful_stop_flag.load(Ordering::SeqCst) {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                })
                .await
                .unwrap_or_else(|e| {
                    error!("Server error: {}", e);
                });

            server_running.store(false, Ordering::SeqCst);
            info!("Connector server stopped");
        });

        Ok(())
    }

    /// Stop the HTTP server
    pub fn stop_server(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Update the port and restart the server if it's running
    pub fn restart_on_port(&self, new_port: u16) -> Result<(), String> {
        // Update the stored port
        {
            let mut port = self.port.blocking_write();
            *port = new_port;
        }

        // If server is running, restart it on the new port
        if self.server_running.load(Ordering::SeqCst) {
            info!("Restarting connector server on new port {}", new_port);
            self.stop_server();

            // Wait for server to stop (with timeout)
            let start = std::time::Instant::now();
            while self.server_running.load(Ordering::SeqCst) {
                if start.elapsed() > Duration::from_secs(2) {
                    return Err("Timeout waiting for server to stop".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            // Reset last poll so status goes to Unknown
            self.last_poll_at.store(0, Ordering::SeqCst);

            // Start on new port
            self.start_server()?;
        }

        Ok(())
    }

    /// Queue a message to be sent to the extension
    pub fn queue_message(&self, text: &str) -> Result<String, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("Message is empty".to_string());
        }

        let msg_id = uuid_simple();
        let ts = now_ms();

        let msg = QueuedMessage {
            id: msg_id.clone(),
            msg_type: "text".to_string(),
            text: trimmed.to_string(),
            ts,
            attachments: None,
        };

        {
            let mut state = self.state.lock().unwrap();
            state.messages.push_back(msg);

            // Trim old messages
            while state.messages.len() > MAX_MESSAGES {
                state.messages.pop_front();
            }
        }

        // Wake any long-polling requests
        self.message_notify.notify_waiters();

        // Emit queued event
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

    /// Queue a bundle message with an image attachment
    pub fn queue_bundle_message(&self, text: &str, image_path: &PathBuf) -> Result<String, String> {
        // Read the image file
        let data =
            std::fs::read(image_path).map_err(|e| format!("Failed to read image file: {}", e))?;

        // Determine MIME type from extension
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

        // Get port for fetch URL - use try_read to avoid blocking in async context
        let port = match self.port.try_read() {
            Ok(guard) => *guard,
            Err(_) => DEFAULT_PORT, // Fallback if lock is held
        };
        let fetch_url = format!("http://127.0.0.1:{}/blob/{}", port, att_id);

        // Create the attachment
        let attachment = BundleAttachment {
            att_id: att_id.clone(),
            kind: "image".to_string(),
            filename,
            mime: Some(mime_type.to_string()),
            size: Some(file_size),
            fetch: BundleFetch {
                url: fetch_url,
                method: Some("GET".to_string()),
                headers: None, // Extension provides auth header automatically
                expires_at: Some(expires_at),
            },
        };

        // Store the blob
        let pending_blob = PendingBlob {
            data,
            mime_type: mime_type.to_string(),
            expires_at,
        };

        // Create the bundle message
        let msg = QueuedMessage {
            id: msg_id.clone(),
            msg_type: "bundle".to_string(),
            text: text.trim().to_string(),
            ts: now,
            attachments: Some(vec![attachment]),
        };

        {
            let mut state = self.state.lock().unwrap();

            // Store the blob for later retrieval
            state.blobs.insert(att_id, pending_blob);

            // Queue the message
            state.messages.push_back(msg);

            // Trim old messages
            while state.messages.len() > MAX_MESSAGES {
                state.messages.pop_front();
            }

            // Clean up expired blobs
            let now = now_ms();
            state.blobs.retain(|_, blob| blob.expires_at > now);
        }

        // Wake any long-polling requests
        self.message_notify.notify_waiters();

        // Emit queued event
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

    /// Queue a bundle message with image bytes directly (no file read)
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

        // Get port for fetch URL
        let port = match self.port.try_read() {
            Ok(guard) => *guard,
            Err(_) => DEFAULT_PORT,
        };
        let fetch_url = format!("http://127.0.0.1:{}/blob/{}", port, att_id);

        // Create the attachment
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

        // Store the blob
        let pending_blob = PendingBlob {
            data,
            mime_type: mime_type.to_string(),
            expires_at,
        };

        // Create the bundle message
        let msg = QueuedMessage {
            id: msg_id.clone(),
            msg_type: "bundle".to_string(),
            text: text.trim().to_string(),
            ts: now,
            attachments: Some(vec![attachment]),
        };

        {
            let mut state = self.state.lock().unwrap();

            // Store the blob for later retrieval
            state.blobs.insert(att_id, pending_blob);

            // Queue the message
            state.messages.push_back(msg);

            // Trim old messages
            while state.messages.len() > MAX_MESSAGES {
                state.messages.pop_front();
            }

            // Clean up expired blobs
            let now = now_ms();
            state.blobs.retain(|_, blob| blob.expires_at > now);
        }

        // Wake any long-polling requests
        self.message_notify.notify_waiters();

        // Emit queued event
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

    /// Cancel a queued message if it hasn't been delivered yet
    pub fn cancel_queued_message(&self, message_id: &str) -> Result<bool, String> {
        let mut state = self.state.lock().unwrap();

        // Check if message exists and hasn't been delivered
        if state.delivered_ids.contains(message_id) {
            return Ok(false); // Already delivered
        }

        // Find and remove the message
        let original_len = state.messages.len();
        state.messages.retain(|m| m.id != message_id);

        if state.messages.len() < original_len {
            // Message was removed - emit cancelled event
            drop(state); // Release lock before emitting

            let _ = self.app_handle.emit(
                "connector-message-cancelled",
                MessageCancelledEvent {
                    id: message_id.to_string(),
                },
            );

            info!("Cancelled queued message: {}", message_id);
            Ok(true)
        } else {
            Ok(false) // Message not found
        }
    }

    /// Get current connection status
    pub fn get_status(&self) -> ConnectorStatus {
        let last_poll = self.last_poll_at.load(Ordering::SeqCst);
        let now = now_ms();
        let server_running = self.server_running.load(Ordering::SeqCst);
        let port = match self.port.try_read() {
            Ok(guard) => *guard,
            Err(_) => DEFAULT_PORT,
        };

        let status = if !server_running {
            ExtensionStatus::Unknown
        } else if last_poll == 0 {
            ExtensionStatus::Unknown
        } else if (now - last_poll) < POLL_TIMEOUT_MS {
            ExtensionStatus::Online
        } else {
            ExtensionStatus::Offline
        };

        let server_error = match self.server_error.try_read() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };

        ConnectorStatus {
            status,
            last_poll_at: last_poll,
            server_running,
            port,
            server_error,
        }
    }

    /// Check if extension is currently online
    pub fn is_online(&self) -> bool {
        let last_poll = self.last_poll_at.load(Ordering::SeqCst);
        if last_poll == 0 {
            return false;
        }
        (now_ms() - last_poll) < POLL_TIMEOUT_MS
    }
}

// ============================================================================
// Axum Handlers
// ============================================================================

/// GET /messages - Long-polling endpoint for extension
async fn handle_get_messages(
    State(app_state): State<AppState>,
    Query(params): Query<MessagesQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Auth check
    let settings = get_settings(&app_state.app_handle);
    if !validate_auth_header(
        &headers,
        &settings.connector_password,
        settings.connector_pending_password.as_deref(),
    ) {
        return unauthorized_response();
    }

    // Update last poll time
    let now = now_ms();
    let old_poll = app_state.last_poll_at.swap(now, Ordering::SeqCst);

    // If this is first poll or we were offline, emit online status
    if old_poll == 0 || (now - old_poll) >= POLL_TIMEOUT_MS {
        info!("Extension connected (polling started)");
        let _ = app_state
            .app_handle
            .emit("extension-status-changed", ExtensionStatus::Online);
    }

    let cursor = params.since.unwrap_or(0);
    let wait_seconds = params
        .wait
        .unwrap_or(DEFAULT_WAIT_SECONDS)
        .min(MAX_WAIT_SECONDS);

    // Try to get messages, with optional long-poll wait
    let (messages, delivered_ids) = if wait_seconds > 0 {
        // Long-poll mode: wait for messages or timeout
        let deadline = tokio::time::Instant::now() + Duration::from_secs(wait_seconds as u64);

        loop {
            // Check for messages
            let (msgs, ids) = get_pending_messages(&app_state.state, cursor);
            if !msgs.is_empty() {
                break (msgs, ids);
            }

            // Calculate remaining time
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break (Vec::new(), Vec::new());
            }

            // Wait for notification or timeout
            tokio::select! {
                _ = app_state.message_notify.notified() => {
                    // New message arrived, check again
                    continue;
                }
                _ = tokio::time::sleep(remaining) => {
                    // Timeout reached
                    break (Vec::new(), Vec::new());
                }
            }
        }
    } else {
        // Immediate mode (backward compatible)
        get_pending_messages(&app_state.state, cursor)
    };

    // Mark messages as delivered
    if !delivered_ids.is_empty() {
        let mut state_guard = app_state.state.lock().unwrap();
        for id in &delivered_ids {
            state_guard.delivered_ids.insert(id.clone());

            // Emit delivered event
            let _ = app_state.app_handle.emit(
                "connector-message-delivered",
                MessageDeliveredEvent { id: id.clone() },
            );
        }

        // Clean up old delivered IDs
        let current_ids: HashSet<_> = state_guard.messages.iter().map(|m| m.id.clone()).collect();
        state_guard
            .delivered_ids
            .retain(|id| current_ids.contains(id));
    }

    // Check if we need to generate a new password
    let password_update = maybe_generate_new_password(&app_state.app_handle);

    // Get config from settings
    let settings = get_settings(&app_state.app_handle);
    let auto_open_url =
        if settings.connector_auto_open_enabled && !settings.connector_auto_open_url.is_empty() {
            Some(settings.connector_auto_open_url.clone())
        } else {
            None
        };

    // Set cursor to ts+1 so next poll with >= won't re-fetch same messages
    let next_cursor = messages.last().map(|m| m.ts + 1).unwrap_or(cursor);

    let response_body = MessagesResponse {
        cursor: next_cursor,
        messages,
        config: ExtensionConfig {
            auto_open_tab_url: auto_open_url,
        },
        password_update,
    };

    Json(response_body).into_response()
}

/// POST /messages - Receive acks and messages from extension
async fn handle_post_messages(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    // Auth check
    let settings = get_settings(&app_state.app_handle);
    if !validate_auth_header(
        &headers,
        &settings.connector_password,
        settings.connector_pending_password.as_deref(),
    ) {
        return unauthorized_response();
    }

    debug!("POST /messages body: {}", body);
    if let Ok(post_body) = serde_json::from_str::<PostBody>(&body) {
        debug!("Parsed POST body, msg_type={:?}", post_body.msg_type);
        if post_body.msg_type.as_deref() == Some("keepalive_ack") {
            debug!("Received keepalive ack from extension");
        } else if post_body.msg_type.as_deref() == Some("password_ack") {
            info!("Received password_ack from extension, committing password...");
            commit_pending_password(&app_state.app_handle);
        } else if let Some(text) = post_body.text {
            debug!("Received message from extension: {}", text);
        }
    } else {
        debug!("Failed to parse POST body as JSON");
    }

    // Update last poll time on POST too
    app_state.last_poll_at.store(now_ms(), Ordering::SeqCst);

    Json(serde_json::json!({"ok": true})).into_response()
}

/// GET /blob/{att_id} - Serve blob data for attachments
async fn handle_get_blob(
    State(app_state): State<AppState>,
    Path(att_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Auth check
    let settings = get_settings(&app_state.app_handle);
    if !validate_auth_header(
        &headers,
        &settings.connector_password,
        settings.connector_pending_password.as_deref(),
    ) {
        return unauthorized_response();
    }

    let blob_data = {
        let mut state_guard = app_state.state.lock().unwrap();
        let now = now_ms();

        // Clean up expired blobs
        state_guard.blobs.retain(|_, blob| blob.expires_at > now);

        // Get the requested blob
        state_guard.blobs.get(&att_id).cloned()
    };

    match blob_data {
        Some(blob) => {
            debug!(
                "Serving blob {} ({} bytes, {})",
                att_id,
                blob.data.len(),
                blob.mime_type
            );

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, blob.mime_type)
                .body(Body::from(blob.data))
                .unwrap()
        }
        None => {
            debug!("Blob not found or expired: {}", att_id);
            (StatusCode::NOT_FOUND, "Blob not found").into_response()
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get messages from queue that are at or newer than cursor
fn get_pending_messages(
    state: &Arc<Mutex<ConnectorState>>,
    cursor: i64,
) -> (Vec<QueuedMessage>, Vec<String>) {
    let state_guard = state.lock().unwrap();
    // Use >= to match original behavior - extension sends since=<last_cursor>
    // where cursor from previous response points to next message position
    let filtered: Vec<_> = state_guard
        .messages
        .iter()
        .filter(|m| m.ts >= cursor)
        .cloned()
        .collect();

    let ids: Vec<_> = filtered.iter().map(|m| m.id.clone()).collect();
    (filtered, ids)
}

/// Create unauthorized response
fn unauthorized_response() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("WWW-Authenticate", "Bearer")
        .body(Body::from("Unauthorized"))
        .unwrap()
}

/// Validate Authorization header against expected password
fn validate_auth_header(
    headers: &axum::http::HeaderMap,
    expected_password: &str,
    pending_password: Option<&str>,
) -> bool {
    if expected_password.is_empty() {
        return false;
    }

    if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
        if let Ok(value) = auth_header.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                // Accept current password
                if constant_time_eq(token.as_bytes(), expected_password.as_bytes()) {
                    return true;
                }
                // Also accept pending password during transition
                if let Some(pending) = pending_password {
                    if constant_time_eq(token.as_bytes(), pending.as_bytes()) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Migrate legacy connector password state (pre two-phase-commit) into a recoverable state.
fn maybe_migrate_legacy_connector_password(
    app_handle: &AppHandle,
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
    write_settings(app_handle, new_settings);
}

fn is_probably_autogenerated_password(password: &str) -> bool {
    password.len() == 32
        && password
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Get current Unix timestamp in milliseconds
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Generate a simple UUID (hex string without dashes)
fn uuid_simple() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:032x}", ts)
}

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Generate a secure random password (32 hex characters)
fn generate_secure_password() -> String {
    let ts_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let pid = std::process::id();
    let thread_id = format!("{:?}", std::thread::current().id());

    let seed = format!(
        "{}{}{}{}",
        ts_nanos,
        pid,
        thread_id,
        ts_nanos.wrapping_mul(0x517cc1b727220a95)
    );

    let mut result = String::with_capacity(32);
    let bytes = seed.as_bytes();
    let mut acc: u64 = 0;
    for (i, &b) in bytes.iter().enumerate() {
        acc = acc.wrapping_add((b as u64).wrapping_mul((i as u64).wrapping_add(1)));
        acc = acc.wrapping_mul(0x517cc1b727220a95);
    }

    for i in 0..4 {
        let chunk = acc
            .wrapping_mul((i + 1) as u64)
            .wrapping_add(ts_nanos as u64);
        result.push_str(&format!("{:08x}", chunk as u32));
    }

    result
}

/// Check if we should generate a new password and do so if needed.
fn maybe_generate_new_password(app_handle: &AppHandle) -> Option<String> {
    let settings = get_settings(app_handle);

    if let Some(ref pending) = settings.connector_pending_password {
        debug!("Returning existing pending password for extension to acknowledge");
        return Some(pending.clone());
    }

    let is_default = settings.connector_password == default_connector_password();
    debug!(
        "Password check: is_default={}, user_set={}, current_len={}",
        is_default,
        settings.connector_password_user_set,
        settings.connector_password.len()
    );

    if is_default {
        let new_password = generate_secure_password();
        info!("Generating new secure connector password (default password detected) - awaiting acknowledgement");

        let mut new_settings = settings.clone();
        new_settings.connector_pending_password = Some(new_password.clone());
        new_settings.connector_password_user_set = false;
        write_settings(app_handle, new_settings);

        Some(new_password)
    } else {
        debug!("Password is not default, skipping auto-generation");
        None
    }
}

/// Commit the pending password after extension acknowledges receipt.
fn commit_pending_password(app_handle: &AppHandle) {
    let settings = get_settings(app_handle);

    if let Some(ref pending) = settings.connector_pending_password {
        info!("Extension acknowledged password - committing new password");

        let mut new_settings = settings.clone();
        new_settings.connector_password = pending.clone();
        new_settings.connector_pending_password = None;
        write_settings(app_handle, new_settings);
    } else {
        debug!("Received password_ack but no pending password to commit");
    }
}
