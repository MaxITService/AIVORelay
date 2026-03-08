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
    response::Response,
    routing::{get, post},
    Json, Router,
};
use log::{debug, error, info, warn};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use p256::ecdh::EphemeralSecret;
use p256::elliptic_curve::rand_core::OsRng;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::PublicKey;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
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
use sha2::{Digest, Sha256};

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
const INITIAL_AUTH_BACKOFF_MS: i64 = 150;
const MAX_AUTH_BACKOFF_MS: i64 = 2000;
const SESSION_TTL_MS: i64 = 120_000;
const SESSION_CLOCK_SKEW_MS: i64 = 15_000;
const PENDING_PASSWORD_TTL_MS: i64 = 120_000;
/// Protocol version for the authenticated ephemeral session path.
///
/// Version 3 keeps the connector password as a bootstrap secret only. The client
/// and server authenticate the handshake with HMAC-SHA256, then derive per-session
/// AES-256-GCM and HMAC keys from an ephemeral P-256 ECDH exchange via HKDF-SHA256.
///
/// Protocol v3 request flow:
/// 1. POST /session with:
///    - Origin: <exact configured origin>
///    - X-AivoRelay-Protocol-Version: 3
///    - JSON body containing the client ephemeral public key, nonce, timestamp, and HMAC proof
/// 2. The server returns its own ephemeral public key plus a signed response proof.
/// 3. Every subsequent /messages and /blob request must include:
///    - X-AivoRelay-Protocol-Version: 3
///    - X-AivoRelay-Session-Id: <session id>
///    - X-AivoRelay-Sequence: <strictly increasing client sequence>
///    - X-AivoRelay-Timestamp: <unix ms within the allowed skew window>
///    - X-AivoRelay-Request-Mac: <base64 HMAC of request metadata/body hash>
/// 4. The server returns per-session response sequence headers plus
///    X-AivoRelay-Response-Mac to support client-side replay detection and integrity checks.
const CONNECTOR_PROTOCOL_VERSION: u8 = 3;
const CONNECTOR_PASSWORD_AUTH_CONTEXT: &[u8] =
    b"AivoRelay Connector Protocol v3 password auth key";
const CONNECTOR_SESSION_ENC_CONTEXT: &[u8] =
    b"AivoRelay Connector Protocol v3 session AES-256-GCM key";
const CONNECTOR_SESSION_MAC_CONTEXT: &[u8] =
    b"AivoRelay Connector Protocol v3 session HMAC-SHA256 key";
const HEADER_PROTOCOL_VERSION: &str = "x-aivorelay-protocol-version";
const HEADER_SESSION_ID: &str = "x-aivorelay-session-id";
const HEADER_CLIENT_SEQUENCE: &str = "x-aivorelay-sequence";
const HEADER_CLIENT_TIMESTAMP: &str = "x-aivorelay-timestamp";
const HEADER_SERVER_SEQUENCE: &str = "x-aivorelay-server-sequence";
const HEADER_SESSION_EXPIRES_AT: &str = "x-aivorelay-session-expires-at";
const HEADER_REQUEST_MAC: &str = "x-aivorelay-request-mac";
const HEADER_RESPONSE_MAC: &str = "x-aivorelay-response-mac";
const HEADER_PAYLOAD_ENCRYPTED: &str = "x-aivorelay-payload-encrypted";
const HEADER_EXTENSION_ID: &str = "x-aivorelay-extension-id";
type HmacSha256 = Hmac<Sha256>;

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

#[derive(Clone)]
struct ConnectorSession {
    origin: String,
    crypto: SessionCrypto,
    next_client_sequence: u64,
    next_server_sequence: u64,
    expires_at: i64,
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
    #[serde(rename = "protocolVersion")]
    protocol_version: u8,
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

/// POST body for authenticated session creation.
#[derive(Debug, Clone, Deserialize)]
struct SessionCreateRequest {
    #[serde(rename = "clientPublicKey")]
    client_public_key: String,
    #[serde(rename = "clientNonce")]
    client_nonce: String,
    timestamp: i64,
    #[serde(rename = "clientProof")]
    client_proof: String,
}

/// Response format for POST /session
#[derive(Debug, Clone, Serialize)]
struct SessionResponse {
    #[serde(rename = "protocolVersion")]
    protocol_version: u8,
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "expiresAt")]
    expires_at: i64,
    #[serde(rename = "nextClientSequence")]
    next_client_sequence: u64,
    #[serde(rename = "nextServerSequence")]
    next_server_sequence: u64,
    #[serde(rename = "encryptionEnabled")]
    encryption_enabled: bool,
    #[serde(rename = "serverPublicKey")]
    server_public_key: String,
    #[serde(rename = "serverNonce")]
    server_nonce: String,
    #[serde(rename = "serverProof")]
    server_proof: String,
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

/// Per-session symmetric encryption and authentication material.
#[derive(Clone)]
struct SessionCrypto {
    cipher: Aes256Gcm,
    mac_key: [u8; 32],
}

impl SessionCrypto {
    pub fn new(enc_key: [u8; 32], mac_key: [u8; 32]) -> Self {
        let aes_key = aes_gcm::Key::<Aes256Gcm>::from_slice(&enc_key);
        let cipher = Aes256Gcm::new(aes_key);
        Self { cipher, mac_key }
    }

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

#[derive(Clone)]
struct ValidatedSession {
    id: String,
    crypto: SessionCrypto,
    server_sequence: u64,
    expires_at: i64,
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
    /// Tracks the timestamp (ms) of the last emitted auth-failed event
    last_auth_failure_emitted_at: Arc<AtomicI64>,
    /// Active connector sessions for protocol v3 replay protection
    sessions: Arc<Mutex<HashMap<String, ConnectorSession>>>,
    /// Count of consecutive auth failures, used for backoff
    auth_failure_count: Arc<AtomicU32>,
    /// Earliest time when a new auth failure response may be returned
    auth_backoff_until_ms: Arc<AtomicI64>,
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
    /// Tracks the timestamp (ms) of the last emitted auth-failed event
    last_auth_failure_emitted_at: Arc<AtomicI64>,
    /// Active connector sessions for protocol v3 replay protection
    sessions: Arc<Mutex<HashMap<String, ConnectorSession>>>,
    /// Count of consecutive auth failures, used for backoff
    auth_failure_count: Arc<AtomicU32>,
    /// Earliest time when a new auth failure response may be returned
    auth_backoff_until_ms: Arc<AtomicI64>,
}

impl ConnectorManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let mut settings = get_settings(app_handle);
        maybe_migrate_legacy_connector_password(app_handle, &settings);
        settings = get_settings(app_handle);
        ensure_pending_password_metadata(app_handle, &settings);
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
            last_auth_failure_emitted_at: Arc::new(AtomicI64::new(0)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            auth_failure_count: Arc::new(AtomicU32::new(0)),
            auth_backoff_until_ms: Arc::new(AtomicI64::new(0)),
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

        let (cors_policy, startup_warning) = match parse_cors_policy(
            &settings.connector_cors,
            settings.connector_allow_any_cors,
        ) {
            Ok(policy) => (Some(policy), None),
            Err(err) => {
                warn!(
                    "Connector server starting without an active CORS allowlist until settings are fixed: {}",
                    err
                );
                (None, Some(err))
            }
        };

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

        let bound_addr = match std_listener.local_addr() {
            Ok(addr) => addr,
            Err(e) => {
                let error_msg = format!("Failed to inspect listener on port {}: {}", port, e);
                error!("Connector server: {}", error_msg);
                *self.server_error.blocking_write() = Some(error_msg.clone());
                let _ = self
                    .app_handle
                    .emit("connector-server-error", error_msg.clone());
                return Err(error_msg);
            }
        };
        if bound_addr.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
            let error_msg = format!(
                "Refusing to start connector on non-loopback address {}",
                bound_addr
            );
            error!("Connector server: {}", error_msg);
            *self.server_error.blocking_write() = Some(error_msg.clone());
            let _ = self
                .app_handle
                .emit("connector-server-error", error_msg.clone());
            return Err(error_msg);
        }

        *self.server_error.blocking_write() = startup_warning.clone();
        if let Some(warning) = startup_warning.as_ref() {
            let _ = self
                .app_handle
                .emit("connector-server-error", warning.clone());
        }
        self.clear_sessions();
        self.auth_failure_count.store(0, Ordering::SeqCst);
        self.auth_backoff_until_ms.store(0, Ordering::SeqCst);
        self.server_running.store(true, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);
        self.last_poll_at.store(0, Ordering::SeqCst);

        let app_state = AppState {
            app_handle: self.app_handle.clone(),
            state: self.state.clone(),
            last_poll_at: Arc::clone(&self.last_poll_at),
            port: self.port.clone(),
            message_notify: self.message_notify.clone(),
            last_auth_failure_emitted_at: self.last_auth_failure_emitted_at.clone(),
            sessions: self.sessions.clone(),
            auth_failure_count: self.auth_failure_count.clone(),
            auth_backoff_until_ms: self.auth_backoff_until_ms.clone(),
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
            let graceful_shutdown = move || {
                let graceful_local_stop_flag = graceful_local_stop_flag.clone();
                let graceful_global_stop_flag = graceful_global_stop_flag.clone();
                async move {
                    loop {
                        if graceful_local_stop_flag.load(Ordering::SeqCst)
                            || graceful_global_stop_flag.load(Ordering::SeqCst)
                        {
                            break;
                        }

                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            };

            let base_router = || {
                Router::new()
                    .route("/session", post(handle_create_session))
                    .route("/messages", get(handle_get_messages))
                    .route("/messages", post(handle_post_messages))
                    .route("/blob/{att_id}", get(handle_get_blob))
                    .with_state(app_state.clone())
            };

            let serve_result = match cors_policy {
                Some(CorsPolicy::Any) => {
                    let cors = CorsLayer::new()
                        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                        .allow_headers([
                            header::AUTHORIZATION,
                            header::CONTENT_TYPE,
                            header::ORIGIN,
                            header::HeaderName::from_static(HEADER_PROTOCOL_VERSION),
                            header::HeaderName::from_static(HEADER_SESSION_ID),
                            header::HeaderName::from_static(HEADER_CLIENT_SEQUENCE),
                            header::HeaderName::from_static(HEADER_CLIENT_TIMESTAMP),
                            header::HeaderName::from_static(HEADER_REQUEST_MAC),
                            header::HeaderName::from_static(HEADER_EXTENSION_ID),
                        ])
                        .expose_headers([
                            header::HeaderName::from_static(HEADER_PROTOCOL_VERSION),
                            header::HeaderName::from_static(HEADER_SESSION_ID),
                            header::HeaderName::from_static(HEADER_SERVER_SEQUENCE),
                            header::HeaderName::from_static(HEADER_SESSION_EXPIRES_AT),
                        ])
                        .allow_origin(Any);

                    axum::serve(listener, base_router().layer(cors))
                        .with_graceful_shutdown(graceful_shutdown())
                        .await
                }
                Some(CorsPolicy::Exact(origin)) => {
                    let origin = HeaderValue::from_str(&origin)
                        .expect("validated origin must be convertible to a header value");

                    let cors = CorsLayer::new()
                        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                        .allow_headers([
                            header::AUTHORIZATION,
                            header::CONTENT_TYPE,
                            header::ORIGIN,
                            header::HeaderName::from_static(HEADER_PROTOCOL_VERSION),
                            header::HeaderName::from_static(HEADER_SESSION_ID),
                            header::HeaderName::from_static(HEADER_CLIENT_SEQUENCE),
                            header::HeaderName::from_static(HEADER_CLIENT_TIMESTAMP),
                            header::HeaderName::from_static(HEADER_REQUEST_MAC),
                            header::HeaderName::from_static(HEADER_EXTENSION_ID),
                        ])
                        .expose_headers([
                            header::HeaderName::from_static(HEADER_PROTOCOL_VERSION),
                            header::HeaderName::from_static(HEADER_SESSION_ID),
                            header::HeaderName::from_static(HEADER_SERVER_SEQUENCE),
                            header::HeaderName::from_static(HEADER_SESSION_EXPIRES_AT),
                        ])
                        .allow_origin(origin);

                    axum::serve(listener, base_router().layer(cors))
                        .with_graceful_shutdown(graceful_shutdown())
                        .await
                }
                None => {
                    axum::serve(listener, base_router())
                        .with_graceful_shutdown(graceful_shutdown())
                        .await
                }
            };

            serve_result.unwrap_or_else(|e| {
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

    /// Password changes invalidate existing sessions, since bootstrap auth changes.
    pub fn refresh_crypto_state(&self, _current_password: &str, _pending_password: Option<&str>) {
        self.clear_sessions();
    }

    pub fn clear_sessions(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.clear();
    }
}

// ============================================================================
// Axum Handlers
// ============================================================================

async fn handle_get_messages(
    State(app_state): State<AppState>,
    uri: Uri,
    headers: axum::http::HeaderMap,
    query: Query<MessagesQuery>,
) -> Response {
    let settings = get_settings(&app_state.app_handle);
    let route_label = request_route_label(&uri);
    let validated_session =
        match validate_session_request(&app_state, &headers, &settings, &route_label, &[]).await {
        Ok(session) => session,
        Err(response) => return response,
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
        protocol_version: CONNECTOR_PROTOCOL_VERSION,
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
                return json_session_response(response_body, &validated_session, &route_label);
            }
        };
        match validated_session.crypto.encrypt_payload(&plain_json) {
            Ok(encrypted_bytes) => {
                let b64_payload = STANDARD.encode(&encrypted_bytes);
                return session_bytes_response(
                    StatusCode::OK,
                    "text/plain",
                    b64_payload.into_bytes(),
                    &validated_session,
                    &route_label,
                    true,
                );
            }
            Err(e) => error!("Failed to encrypt response: {}", e),
        }
    }

    json_session_response(response_body, &validated_session, &route_label)
}

async fn handle_create_session(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<SessionCreateRequest>,
) -> Response {
    let settings = get_settings(&app_state.app_handle);
    if let Err(response) = validate_protocol_header(&headers) {
        return response;
    }

    let cors_policy = match parse_cors_policy(
        &settings.connector_cors,
        settings.connector_allow_any_cors,
    ) {
        Ok(policy) => policy,
        Err(err) => return error_response(StatusCode::FORBIDDEN, &err),
    };
    let request_origin = match validate_origin_header(&headers, &cors_policy) {
        Ok(origin) => origin,
        Err(response) => return response,
    };
    let session_origin = match cors_policy {
        CorsPolicy::Any => String::new(),
        CorsPolicy::Exact(_) => request_origin,
    };
    let auth_match = match authenticate_session_handshake(&app_state, &settings, &request).await {
        Ok(auth_match) => auth_match,
        Err(response) => return response,
    };

    let client_public_key_bytes =
        match decode_base64_field(&request.client_public_key, "client public key") {
            Ok(bytes) => bytes,
            Err(response) => return response,
        };
    let client_public_key = match PublicKey::from_sec1_bytes(&client_public_key_bytes) {
        Ok(key) => key,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Malformed client public key",
            )
        }
    };
    let client_nonce_bytes = match decode_base64_field(&request.client_nonce, "client nonce") {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };

    let session_id = match generate_random_hex(16) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate connector session id",
            )
        }
    };

    let now = now_ms();
    let expires_at = now + SESSION_TTL_MS;
    let server_nonce_bytes = match generate_random_bytes(16) {
        Some(bytes) => bytes,
        None => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate connector session nonce",
            )
        }
    };
    let server_secret = EphemeralSecret::random(&mut OsRng);
    let server_public_key = PublicKey::from(&server_secret);
    let server_public_key_bytes = server_public_key.to_encoded_point(false).as_bytes().to_vec();
    let server_public_key_b64 = STANDARD.encode(&server_public_key_bytes);
    let shared_secret = server_secret.diffie_hellman(&client_public_key);
    let auth_key = derive_password_auth_key(match auth_match {
        AuthMatch::CurrentPassword => &settings.connector_password,
        AuthMatch::PendingPassword => {
            settings.connector_pending_password.as_deref().unwrap_or(&settings.connector_password)
        }
    });
    let transcript_hash = build_handshake_transcript_hash(
        &session_id,
        &client_public_key_bytes,
        &server_public_key_bytes,
        &client_nonce_bytes,
        &server_nonce_bytes,
    );
    let session_crypto = match derive_session_crypto(
        shared_secret.raw_secret_bytes().as_slice(),
        &auth_key,
        &transcript_hash,
    ) {
        Ok(crypto) => crypto,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &err),
    };
    let server_nonce_b64 = STANDARD.encode(&server_nonce_bytes);
    let server_proof = match sign_handshake_server_proof(
        &auth_key,
        &session_id,
        expires_at,
        settings.connector_encryption_enabled,
        &request.client_public_key,
        &server_public_key_b64,
        &request.client_nonce,
        &server_nonce_b64,
    ) {
        Ok(proof) => proof,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &err),
    };

    {
        let mut sessions = app_state.sessions.lock().unwrap();
        clear_expired_sessions(&mut sessions, now);
        sessions.insert(
            session_id.clone(),
            ConnectorSession {
                origin: session_origin,
                crypto: session_crypto.clone(),
                next_client_sequence: 1,
                next_server_sequence: 2,
                expires_at,
            },
        );
    }

    let validated_session = ValidatedSession {
        id: session_id.clone(),
        crypto: session_crypto,
        server_sequence: 1,
        expires_at,
    };
    let response_body = SessionResponse {
        protocol_version: CONNECTOR_PROTOCOL_VERSION,
        session_id,
        expires_at,
        next_client_sequence: 1,
        next_server_sequence: 1,
        encryption_enabled: settings.connector_encryption_enabled,
        server_public_key: server_public_key_b64,
        server_nonce: server_nonce_b64,
        server_proof,
    };

    json_session_response(response_body, &validated_session, "/session")
}

async fn handle_post_messages(
    State(app_state): State<AppState>,
    uri: Uri,
    headers: axum::http::HeaderMap,
    body: String,
) -> Response {
    let settings = get_settings(&app_state.app_handle);
    let route_label = request_route_label(&uri);
    let validated_session = match validate_session_request(
        &app_state,
        &headers,
        &settings,
        &route_label,
        body.as_bytes(),
    )
    .await
    {
        Ok(session) => session,
        Err(response) => return response,
    };

    if let Ok(post_body) = serde_json::from_str::<PostBody>(&body) {
        if post_body.msg_type.as_deref() == Some("password_ack") {
            info!("Extension acknowledged password - committing...");
            commit_pending_password(&app_state.app_handle);
        }
    }

    app_state.last_poll_at.store(now_ms(), Ordering::SeqCst);
    json_session_response(
        serde_json::json!({
            "ok": true,
            "protocolVersion": CONNECTOR_PROTOCOL_VERSION
        }),
        &validated_session,
        &route_label,
    )
}

async fn handle_get_blob(
    State(app_state): State<AppState>,
    Path(att_id): Path<String>,
    uri: Uri,
    headers: axum::http::HeaderMap,
) -> Response {
    let settings = get_settings(&app_state.app_handle);
    let route_label = request_route_label(&uri);
    let validated_session =
        match validate_session_request(&app_state, &headers, &settings, &route_label, &[]).await {
            Ok(session) => session,
            Err(response) => return response,
        };

    let blob_data = {
        let mut state_guard = app_state.state.lock().unwrap();
        let now = now_ms();
        state_guard.blobs.retain(|_, b| b.expires_at > now);
        state_guard.blobs.get(&att_id).cloned()
    };

    match blob_data {
        Some(blob) if settings.connector_encryption_enabled => {
            match validated_session.crypto.encrypt_payload(&blob.data) {
                Ok(encrypted_bytes) => session_bytes_response(
                    StatusCode::OK,
                    "application/octet-stream",
                    encrypted_bytes,
                    &validated_session,
                    &route_label,
                    true,
                ),
                Err(err) => {
                    error!("Failed to encrypt connector blob response: {}", err);
                    session_bytes_response(
                        StatusCode::OK,
                        &blob.mime_type,
                        blob.data,
                        &validated_session,
                        &route_label,
                        false,
                    )
                }
            }
        }
        Some(blob) => session_bytes_response(
            StatusCode::OK,
            &blob.mime_type,
            blob.data,
            &validated_session,
            &route_label,
            false,
        ),
        None => session_bytes_response(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            b"Blob not found".to_vec(),
            &validated_session,
            &route_label,
            false,
        ),
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

fn register_auth_failure(app_state: &AppState) -> i64 {
    let failure_count = app_state.auth_failure_count.fetch_add(1, Ordering::SeqCst) + 1;
    let exponent = failure_count.saturating_sub(1).min(4);
    let delay_ms =
        (INITIAL_AUTH_BACKOFF_MS * (1_i64 << exponent)).min(MAX_AUTH_BACKOFF_MS);
    let until = now_ms() + delay_ms;
    app_state.auth_backoff_until_ms.store(until, Ordering::SeqCst);
    delay_ms
}

fn reset_auth_backoff(app_state: &AppState) {
    app_state.auth_failure_count.store(0, Ordering::SeqCst);
    app_state.auth_backoff_until_ms.store(0, Ordering::SeqCst);
}

async fn apply_auth_failure_delay(app_state: &AppState, fallback_ms: i64) {
    let mut buffer = [0u8; 1];
    let jitter_ms = if getrandom::getrandom(&mut buffer).is_ok() {
        (buffer[0] % 31) as i64
    } else {
        15
    };
    let until = app_state.auth_backoff_until_ms.load(Ordering::SeqCst);
    let delay_ms = (until - now_ms()).max(fallback_ms).max(0) + jitter_ms;
    tokio::time::sleep(Duration::from_millis(delay_ms as u64)).await;
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
    let response = Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("WWW-Authenticate", "Bearer")
        .body(Body::from("Unauthorized"))
        .unwrap();
    apply_security_headers(response)
}

fn error_response(status: StatusCode, message: &str) -> Response {
    apply_security_headers(
        Response::builder()
            .status(status)
            .body(Body::from(message.to_string()))
            .unwrap(),
    )
}

fn validate_protocol_header(headers: &axum::http::HeaderMap) -> Result<(), Response> {
    let Some(value) = headers.get(header::HeaderName::from_static(HEADER_PROTOCOL_VERSION)) else {
        return Err(error_response(
            StatusCode::PRECONDITION_REQUIRED,
            "Missing protocol version header",
        ));
    };
    let Ok(raw_value) = value.to_str() else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Malformed protocol version header",
        ));
    };
    let Ok(protocol_version) = raw_value.parse::<u8>() else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Malformed protocol version header",
        ));
    };
    if protocol_version != CONNECTOR_PROTOCOL_VERSION {
        return Err(error_response(
            StatusCode::PRECONDITION_FAILED,
            "Unsupported connector protocol version",
        ));
    }
    Ok(())
}

fn validate_origin_header(
    headers: &axum::http::HeaderMap,
    cors_policy: &CorsPolicy,
) -> Result<String, Response> {
    let origin = match headers.get(header::ORIGIN) {
        Some(origin_value) => origin_value.to_str().map_err(|_| {
            error_response(StatusCode::BAD_REQUEST, "Malformed Origin header")
        })?,
        None => "",
    };

    match cors_policy {
        CorsPolicy::Any => Ok(origin.to_string()),
        CorsPolicy::Exact(expected_origin) => {
            if origin.is_empty() && expected_origin.starts_with("chrome-extension://") {
                let extension_id = required_string_header(
                    headers,
                    HEADER_EXTENSION_ID,
                    "extension id",
                )?;
                let expected_extension_id = expected_origin
                    .trim_start_matches("chrome-extension://");
                if extension_id != expected_extension_id {
                    return Err(error_response(
                        StatusCode::FORBIDDEN,
                        "Extension id is not allowed for the connector",
                    ));
                }
                return Ok(expected_origin.to_string());
            }
            if origin.is_empty() {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "Missing Origin header",
                ));
            }
            if origin != expected_origin {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "Origin is not allowed for the connector",
                ));
            }
            Ok(origin.to_string())
        }
    }
}

async fn authenticate_session_handshake(
    app_state: &AppState,
    settings: &crate::settings::AppSettings,
    request: &SessionCreateRequest,
) -> Result<AuthMatch, Response> {
    let now = now_ms();
    if request.timestamp < now - SESSION_CLOCK_SKEW_MS
        || request.timestamp > now + SESSION_CLOCK_SKEW_MS
    {
        return Err(error_response(
            StatusCode::PRECONDITION_FAILED,
            "Handshake timestamp is outside the allowed window",
        ));
    }

    let pending_password = active_pending_password(&app_state.app_handle, settings);
    let auth_match = if verify_handshake_client_proof(&settings.connector_password, request) {
        Some(AuthMatch::CurrentPassword)
    } else if pending_password
        .as_deref()
        .is_some_and(|password| verify_handshake_client_proof(password, request))
    {
        Some(AuthMatch::PendingPassword)
    } else {
        None
    };

    if let Some(auth_match) = auth_match {
        reset_auth_backoff(app_state);
        return Ok(auth_match);
    }

    let delay_ms = register_auth_failure(app_state);
    maybe_emit_auth_failure_toast(app_state);
    apply_auth_failure_delay(app_state, delay_ms).await;
    Err(unauthorized_response())
}

async fn validate_session_request(
    app_state: &AppState,
    headers: &axum::http::HeaderMap,
    settings: &crate::settings::AppSettings,
    route_label: &str,
    body: &[u8],
) -> Result<ValidatedSession, Response> {
    validate_protocol_header(headers)?;
    let cors_policy = match parse_cors_policy(
        &settings.connector_cors,
        settings.connector_allow_any_cors,
    ) {
        Ok(policy) => policy,
        Err(err) => return Err(error_response(StatusCode::FORBIDDEN, &err)),
    };
    let origin = validate_origin_header(headers, &cors_policy)?;
    let session_id = required_string_header(headers, HEADER_SESSION_ID, "session id")?;
    let client_sequence = required_u64_header(headers, HEADER_CLIENT_SEQUENCE, "sequence")?;
    let client_timestamp = required_i64_header(headers, HEADER_CLIENT_TIMESTAMP, "timestamp")?;
    let request_mac = required_string_header(headers, HEADER_REQUEST_MAC, "request mac")?;
    let now = now_ms();
    if client_timestamp < now - SESSION_CLOCK_SKEW_MS
        || client_timestamp > now + SESSION_CLOCK_SKEW_MS
    {
        return Err(error_response(
            StatusCode::PRECONDITION_FAILED,
            "Request timestamp is outside the allowed window",
        ));
    }

    let mut sessions = app_state.sessions.lock().unwrap();
    clear_expired_sessions(&mut sessions, now);
    let Some(session) = sessions.get_mut(&session_id) else {
        return Err(error_response(
            StatusCode::PRECONDITION_REQUIRED,
            "Missing or expired connector session",
        ));
    };
    if matches!(cors_policy, CorsPolicy::Exact(_)) && session.origin != origin {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "Session origin does not match request origin",
        ));
    }
    if !verify_request_mac(
        &session.crypto.mac_key,
        route_label,
        client_sequence,
        client_timestamp,
        body,
        &request_mac,
    ) {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "Session request authentication failed",
        ));
    }
    if client_sequence != session.next_client_sequence {
        return Err(error_response(
            StatusCode::CONFLICT,
            "Unexpected request sequence number",
        ));
    }

    session.next_client_sequence += 1;
    let server_sequence = session.next_server_sequence;
    session.next_server_sequence += 1;
    session.expires_at = now + SESSION_TTL_MS;

    Ok(ValidatedSession {
        id: session_id,
        crypto: session.crypto.clone(),
        server_sequence,
        expires_at: session.expires_at,
    })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut res = if a.len() == b.len() { 0u8 } else { 1u8 };
    let n = std::cmp::min(a.len(), b.len());
    for i in 0..n {
        res |= a[i] ^ b[i];
    }
    res == 0
}

pub(crate) fn now_ms() -> i64 {
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

fn generate_random_bytes(byte_len: usize) -> Option<Vec<u8>> {
    let mut bytes = vec![0u8; byte_len];
    if let Err(err) = getrandom::getrandom(&mut bytes) {
        error!("Failed to generate secure random bytes from OS CSPRNG: {}", err);
        return None;
    }
    Some(bytes)
}

fn generate_random_hex(byte_len: usize) -> Option<String> {
    let bytes = generate_random_bytes(byte_len)?;

    let mut result = String::with_capacity(byte_len * 2);
    for byte in bytes {
        result.push_str(&format!("{:02x}", byte));
    }
    Some(result)
}

fn generate_secure_password() -> Option<String> {
    generate_random_hex(32)
}

fn maybe_generate_new_password(app: &AppHandle) -> Option<String> {
    let settings = get_settings(app);
    if let Some(pending) = active_pending_password(app, &settings) {
        debug!("Returning existing pending password for extension to acknowledge");
        return Some(pending);
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
        new_settings.connector_pending_password_issued_at_ms = now_ms();
        new_settings.connector_password_user_set = false;
        write_settings(app, new_settings);

        let connector_manager = app.state::<Arc<ConnectorManager>>();
        connector_manager.refresh_crypto_state(&settings.connector_password, Some(&new_password));
        connector_manager.clear_sessions();

        return Some(new_password);
    }

    None
}

fn commit_pending_password(app: &AppHandle) {
    let settings = get_settings(app);
    if let Some(pending) = active_pending_password(app, &settings) {
        info!("Extension acknowledged password - committing new password");

        let mut new_settings = settings.clone();
        new_settings.connector_password = pending.clone();
        new_settings.connector_pending_password = None;
        new_settings.connector_pending_password_issued_at_ms = 0;
        write_settings(app, new_settings);

        let connector_manager = app.state::<Arc<ConnectorManager>>();
        connector_manager.refresh_crypto_state(&pending, None);
        connector_manager.clear_sessions();
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
    new_settings.connector_pending_password_issued_at_ms = now_ms();
    new_settings.connector_password = default_password;
    write_settings(app, new_settings);
}

fn ensure_pending_password_metadata(app: &AppHandle, settings: &crate::settings::AppSettings) {
    if settings.connector_pending_password.is_none() || settings.connector_pending_password_issued_at_ms > 0 {
        return;
    }

    let mut new_settings = settings.clone();
    new_settings.connector_pending_password_issued_at_ms = now_ms();
    write_settings(app, new_settings);
}

fn active_pending_password(
    app: &AppHandle,
    settings: &crate::settings::AppSettings,
) -> Option<String> {
    let pending = settings.connector_pending_password.as_ref()?;
    if settings.connector_pending_password_issued_at_ms <= 0 {
        return Some(pending.clone());
    }

    if now_ms() - settings.connector_pending_password_issued_at_ms > PENDING_PASSWORD_TTL_MS {
        info!("Connector pending password expired before acknowledgement; clearing it");
        clear_pending_password(app, settings);
        return None;
    }

    Some(pending.clone())
}

fn clear_pending_password(app: &AppHandle, settings: &crate::settings::AppSettings) {
    if settings.connector_pending_password.is_none() {
        return;
    }

    let mut new_settings = settings.clone();
    new_settings.connector_pending_password = None;
    new_settings.connector_pending_password_issued_at_ms = 0;
    write_settings(app, new_settings);

    if let Some(connector_manager) = app.try_state::<Arc<ConnectorManager>>() {
        connector_manager.refresh_crypto_state(&settings.connector_password, None);
        connector_manager.clear_sessions();
    }
}

fn is_probably_autogenerated_password(password: &str) -> bool {
    matches!(password.len(), 32 | 64)
        && password
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn derive_password_auth_key(password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CONNECTOR_PASSWORD_AUTH_CONTEXT);
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

fn build_handshake_client_proof_payload(request: &SessionCreateRequest) -> Vec<u8> {
    format!(
        "aivorelay-v3-client-hello\n{}\n{}\n{}\n{}\n{}",
        CONNECTOR_PROTOCOL_VERSION,
        request.timestamp,
        request.client_nonce.trim(),
        request.client_public_key.trim(),
        "session"
    )
    .into_bytes()
}

fn build_handshake_server_proof_payload(
    session_id: &str,
    expires_at: i64,
    encryption_enabled: bool,
    client_public_key: &str,
    server_public_key: &str,
    client_nonce: &str,
    server_nonce: &str,
) -> Vec<u8> {
    format!(
        "aivorelay-v3-server-hello\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        CONNECTOR_PROTOCOL_VERSION,
        session_id,
        expires_at,
        if encryption_enabled { 1 } else { 0 },
        client_nonce.trim(),
        server_nonce.trim(),
        client_public_key.trim(),
        server_public_key.trim()
    )
    .into_bytes()
}

fn compute_hmac_bytes(key: &[u8], payload: &[u8]) -> Result<Vec<u8>, String> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .map_err(|e| format!("Failed to initialize HMAC state: {}", e))?;
    mac.update(payload);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn verify_handshake_client_proof(password: &str, request: &SessionCreateRequest) -> bool {
    let proof_bytes = match STANDARD.decode(request.client_proof.trim()) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let auth_key = derive_password_auth_key(password);
    let payload = build_handshake_client_proof_payload(request);
    match compute_hmac_bytes(&auth_key, &payload) {
        Ok(expected) => constant_time_eq(&proof_bytes, &expected),
        Err(_) => false,
    }
}

fn sign_handshake_server_proof(
    auth_key: &[u8; 32],
    session_id: &str,
    expires_at: i64,
    encryption_enabled: bool,
    client_public_key: &str,
    server_public_key: &str,
    client_nonce: &str,
    server_nonce: &str,
) -> Result<String, String> {
    let payload = build_handshake_server_proof_payload(
        session_id,
        expires_at,
        encryption_enabled,
        client_public_key,
        server_public_key,
        client_nonce,
        server_nonce,
    );
    Ok(STANDARD.encode(compute_hmac_bytes(auth_key, &payload)?))
}

fn build_handshake_transcript_hash(
    session_id: &str,
    client_public_key: &[u8],
    server_public_key: &[u8],
    client_nonce: &[u8],
    server_nonce: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"aivorelay-v3-transcript");
    hasher.update(session_id.as_bytes());
    hasher.update(client_public_key);
    hasher.update(server_public_key);
    hasher.update(client_nonce);
    hasher.update(server_nonce);
    let digest = hasher.finalize();
    let mut result = [0u8; 32];
    result.copy_from_slice(&digest);
    result
}

fn derive_session_key(
    shared_secret: &[u8],
    auth_key: &[u8; 32],
    context: &[u8],
    transcript_hash: &[u8; 32],
) -> Result<[u8; 32], String> {
    let hk = Hkdf::<Sha256>::new(Some(auth_key), shared_secret);
    let mut info = Vec::with_capacity(context.len() + transcript_hash.len());
    info.extend_from_slice(context);
    info.extend_from_slice(transcript_hash);
    let mut output = [0u8; 32];
    hk.expand(&info, &mut output)
        .map_err(|_| "Failed to derive connector session key".to_string())?;
    Ok(output)
}

fn derive_session_crypto(
    shared_secret: &[u8],
    auth_key: &[u8; 32],
    transcript_hash: &[u8; 32],
) -> Result<SessionCrypto, String> {
    let enc_key = derive_session_key(
        shared_secret,
        auth_key,
        CONNECTOR_SESSION_ENC_CONTEXT,
        transcript_hash,
    )?;
    let mac_key = derive_session_key(
        shared_secret,
        auth_key,
        CONNECTOR_SESSION_MAC_CONTEXT,
        transcript_hash,
    )?;
    Ok(SessionCrypto::new(enc_key, mac_key))
}

fn decode_base64_field(raw_value: &str, label: &str) -> Result<Vec<u8>, Response> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            &format!("Missing {}", label),
        ));
    }
    STANDARD
        .decode(trimmed)
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, &format!("Malformed {}", label)))
}

fn request_route_label(uri: &Uri) -> String {
    uri.path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| uri.path().to_string())
}

fn hash_body_base64(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    STANDARD.encode(digest)
}

fn build_request_mac_payload(route_label: &str, sequence: u64, timestamp: i64, body: &[u8]) -> Vec<u8> {
    format!(
        "aivorelay-v3-request\n{}\n{}\n{}\n{}",
        route_label,
        sequence,
        timestamp,
        hash_body_base64(body)
    )
    .into_bytes()
}

fn build_response_mac_payload(
    route_label: &str,
    status: StatusCode,
    server_sequence: u64,
    expires_at: i64,
    encrypted: bool,
    body: &[u8],
) -> Vec<u8> {
    format!(
        "aivorelay-v3-response\n{}\n{}\n{}\n{}\n{}\n{}",
        route_label,
        status.as_u16(),
        server_sequence,
        expires_at,
        if encrypted { 1 } else { 0 },
        hash_body_base64(body)
    )
    .into_bytes()
}

fn verify_request_mac(
    mac_key: &[u8; 32],
    route_label: &str,
    sequence: u64,
    timestamp: i64,
    body: &[u8],
    provided_mac: &str,
) -> bool {
    let provided = match STANDARD.decode(provided_mac.trim()) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let payload = build_request_mac_payload(route_label, sequence, timestamp, body);
    match compute_hmac_bytes(mac_key, &payload) {
        Ok(expected) => constant_time_eq(&provided, &expected),
        Err(_) => false,
    }
}

fn required_string_header(
    headers: &axum::http::HeaderMap,
    header_name: &'static str,
    label: &str,
) -> Result<String, Response> {
    let Some(value) = headers.get(header::HeaderName::from_static(header_name)) else {
        return Err(error_response(
            StatusCode::PRECONDITION_REQUIRED,
            &format!("Missing {}", label),
        ));
    };
    let Ok(raw) = value.to_str() else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            &format!("Malformed {}", label),
        ));
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '0') {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            &format!("Malformed {}", label),
        ));
    }
    Ok(trimmed.to_string())
}

fn required_u64_header(
    headers: &axum::http::HeaderMap,
    header_name: &'static str,
    label: &str,
) -> Result<u64, Response> {
    let raw = required_string_header(headers, header_name, label)?;
    raw.parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, &format!("Malformed {}", label)))
}

fn required_i64_header(
    headers: &axum::http::HeaderMap,
    header_name: &'static str,
    label: &str,
) -> Result<i64, Response> {
    let raw = required_string_header(headers, header_name, label)?;
    raw.parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, &format!("Malformed {}", label)))
}

fn clear_expired_sessions(sessions: &mut HashMap<String, ConnectorSession>, now: i64) {
    sessions.retain(|_, session| session.expires_at > now);
}

pub(crate) fn normalize_connector_cors_setting(raw_value: &str) -> Result<String, String> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() || trimmed == "*" || trimmed.eq_ignore_ascii_case("<any>") {
        return Err(
            "Connector CORS must be an exact origin. Wildcards and empty values are not allowed."
                .to_string(),
        );
    }

    if let Some(extension_id) = trimmed.strip_prefix("chrome-extension://") {
        if extension_id.is_empty()
            || extension_id.contains('/')
            || extension_id.contains('?')
            || extension_id.contains('#')
            || !extension_id
                .bytes()
                .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-'))
        {
            return Err(format!(
                "Invalid CORS origin '{}': expected chrome-extension://<extension-id>",
                trimmed
            ));
        }
        return Ok(format!("chrome-extension://{}", extension_id.to_ascii_lowercase()));
    }

    let uri: Uri = trimmed.parse().map_err(|_| {
        format!(
            "Invalid CORS origin '{}': expected an exact origin like https://chatgpt.com or chrome-extension://<id>",
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

fn parse_cors_policy(raw_value: &str, allow_any: bool) -> Result<CorsPolicy, String> {
    if allow_any {
        return Ok(CorsPolicy::Any);
    }
    let normalized = normalize_connector_cors_setting(raw_value)?;
    Ok(CorsPolicy::Exact(normalized))
}

fn apply_security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::VARY,
        HeaderValue::from_static("Origin"),
    );
    headers.insert(
        header::HeaderName::from_static(HEADER_PROTOCOL_VERSION),
        HeaderValue::from_static("3"),
    );
    headers.insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn session_bytes_response(
    status: StatusCode,
    content_type: &str,
    body: Vec<u8>,
    session: &ValidatedSession,
    route_label: &str,
    encrypted: bool,
) -> Response {
    let mut response = apply_security_headers(
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body.clone()))
            .unwrap(),
    );

    let headers = response.headers_mut();
    if let Ok(session_id) = HeaderValue::from_str(&session.id) {
        headers.insert(header::HeaderName::from_static(HEADER_SESSION_ID), session_id);
    }
    if let Ok(server_sequence) = HeaderValue::from_str(&session.server_sequence.to_string()) {
        headers.insert(
            header::HeaderName::from_static(HEADER_SERVER_SEQUENCE),
            server_sequence,
        );
    }
    if let Ok(expires_at) = HeaderValue::from_str(&session.expires_at.to_string()) {
        headers.insert(
            header::HeaderName::from_static(HEADER_SESSION_EXPIRES_AT),
            expires_at,
        );
    }
    headers.insert(
        header::HeaderName::from_static(HEADER_PAYLOAD_ENCRYPTED),
        if encrypted {
            HeaderValue::from_static("1")
        } else {
            HeaderValue::from_static("0")
        },
    );
    let response_mac = compute_hmac_bytes(
        &session.crypto.mac_key,
        &build_response_mac_payload(
            route_label,
            status,
            session.server_sequence,
            session.expires_at,
            encrypted,
            &body,
        ),
    );
    if let Ok(response_mac) = response_mac {
        if let Ok(response_mac_value) = HeaderValue::from_str(&STANDARD.encode(response_mac)) {
            headers.insert(
                header::HeaderName::from_static(HEADER_RESPONSE_MAC),
                response_mac_value,
            );
        }
    }
    response
}

fn json_session_response<T: Serialize>(
    payload: T,
    session: &ValidatedSession,
    route_label: &str,
) -> Response {
    match serde_json::to_vec(&payload) {
        Ok(bytes) => session_bytes_response(
            StatusCode::OK,
            "application/json",
            bytes,
            session,
            route_label,
            false,
        ),
        Err(err) => {
            error!("Failed to serialize connector session JSON response: {}", err);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize connector response",
            )
        }
    }
}
