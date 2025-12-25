//! Connector Manager - HTTP server for Chrome extension communication
//!
//! This module provides an HTTP server that allows the Handy Chrome extension
//! to poll for messages. It tracks the connection status based on polling activity.

use crate::settings::get_settings;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tiny_http::{Header, Method, Request, Response, Server};

/// Default server port (same as test-server.ps1)
const DEFAULT_PORT: u16 = 63155;
/// Timeout in milliseconds - if no poll for this duration, consider disconnected
const POLL_TIMEOUT_MS: i64 = 10_000;
/// Keepalive interval in milliseconds
const KEEPALIVE_INTERVAL_MS: i64 = 15_000;
/// Maximum messages to keep in queue
const MAX_MESSAGES: usize = 100;
/// How long to keep blobs available for download (5 minutes)
const BLOB_EXPIRY_MS: i64 = 300_000;

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
}

/// POST body from extension (ack or message)
#[derive(Debug, Clone, Deserialize)]
struct PostBody {
    #[serde(default)]
    text: Option<String>,
    #[serde(rename = "type", default)]
    msg_type: Option<String>,
}

/// Internal state shared between threads
struct ConnectorState {
    /// Queue of messages waiting to be picked up by extension
    messages: VecDeque<QueuedMessage>,
    /// Timestamp of last keepalive sent
    last_keepalive: i64,
    /// Blobs stored for extension to download (attId -> blob data)
    blobs: HashMap<String, PendingBlob>,
}

pub struct ConnectorManager {
    app_handle: AppHandle,
    /// Timestamp of last poll from extension (atomic for lock-free access)
    last_poll_at: AtomicI64,
    /// Whether server is running
    server_running: AtomicBool,
    /// Port server is listening on
    port: RwLock<u16>,
    /// Shared state for message queue
    state: Arc<Mutex<ConnectorState>>,
    /// Flag to stop the server
    stop_flag: Arc<AtomicBool>,
}

impl ConnectorManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let settings = get_settings(app_handle);
        let port = if settings.connector_port > 0 {
            settings.connector_port
        } else {
            DEFAULT_PORT
        };

        let manager = Self {
            app_handle: app_handle.clone(),
            last_poll_at: AtomicI64::new(0),
            server_running: AtomicBool::new(false),
            port: RwLock::new(port),
            state: Arc::new(Mutex::new(ConnectorState {
                messages: VecDeque::new(),
                last_keepalive: 0,
                blobs: HashMap::new(),
            })),
            stop_flag: Arc::new(AtomicBool::new(false)),
        };

        Ok(manager)
    }

    /// Start the HTTP server in a background thread
    pub fn start_server(&self) -> Result<(), String> {
        if self.server_running.load(Ordering::SeqCst) {
            return Ok(()); // Already running
        }

        let port = *self.port.read().unwrap();
        let server = Self::try_bind_server(port)?;

        self.server_running.store(true, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);

        let app_handle = self.app_handle.clone();
        let state = self.state.clone();
        let stop_flag = self.stop_flag.clone();
        let last_poll_at = unsafe {
            // Safe: we're cloning the atomic's address, manager outlives the thread
            &*(&self.last_poll_at as *const AtomicI64)
        };
        let server_running = unsafe { &*(&self.server_running as *const AtomicBool) };

        // Clone the raw pointers for the thread
        let last_poll_ptr = last_poll_at as *const AtomicI64 as usize;
        let server_running_ptr = server_running as *const AtomicBool as usize;

        thread::spawn(move || {
            let last_poll_at = unsafe { &*(last_poll_ptr as *const AtomicI64) };
            let server_running = unsafe { &*(server_running_ptr as *const AtomicBool) };

            info!("Connector server started on port {}", port);

            // Emit initial status
            let _ = app_handle.emit("extension-status-changed", ExtensionStatus::Unknown);

            let mut was_online = false;

            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                // Handle incoming requests with timeout
                match server.recv_timeout(Duration::from_millis(500)) {
                    Ok(Some(request)) => {
                        Self::handle_request(
                            request,
                            &state,
                            last_poll_at,
                            &app_handle,
                            &mut was_online,
                        );
                    }
                    Ok(None) => {
                        // Timeout, check status and keepalive
                    }
                    Err(e) => {
                        error!("Server error: {}", e);
                        break;
                    }
                }

                // Check connection status
                let now_ms = now_ms();
                let last_poll = last_poll_at.load(Ordering::SeqCst);

                if last_poll > 0 {
                    let is_online = (now_ms - last_poll) < POLL_TIMEOUT_MS;

                    if is_online != was_online {
                        let status = if is_online {
                            ExtensionStatus::Online
                        } else {
                            ExtensionStatus::Offline
                        };
                        info!("Extension status changed: {:?}", status);
                        let _ = app_handle.emit("extension-status-changed", status);
                        was_online = is_online;
                    }
                }

                // Send keepalive if needed
                Self::maybe_send_keepalive(&state, now_ms);
            }

            server_running.store(false, Ordering::SeqCst);
            info!("Connector server stopped");
        });

        Ok(())
    }

    /// Try to bind to the specified port or nearby ports
    fn try_bind_server(start_port: u16) -> Result<Server, String> {
        for offset in 0..20 {
            let port = start_port + offset;
            let addr = format!("127.0.0.1:{}", port);

            match Server::http(&addr) {
                Ok(server) => {
                    if offset > 0 {
                        info!(
                            "Port {} in use, bound to port {} instead",
                            start_port, port
                        );
                    }
                    return Ok(server);
                }
                Err(e) => {
                    debug!("Failed to bind to {}: {}", addr, e);
                }
            }
        }
        Err(format!(
            "Could not bind to any port in range {}-{}",
            start_port,
            start_port + 19
        ))
    }

    /// Handle an incoming HTTP request
    fn handle_request(
        mut request: Request,
        state: &Arc<Mutex<ConnectorState>>,
        last_poll_at: &AtomicI64,
        app_handle: &AppHandle,
        was_online: &mut bool,
    ) {
        let path = request.url().split('?').next().unwrap_or("/").to_string();
        let method = request.method().clone();

        // Add CORS headers to all responses
        let cors_headers = vec![
            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
            Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Authorization, Content-Type"[..]).unwrap(),
            Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS"[..])
                .unwrap(),
            Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap(),
        ];

        match (&method, path.as_str()) {
            (Method::Options, _) => {
                // CORS preflight - no auth needed
                let mut response = Response::empty(204);
                for header in cors_headers {
                    response.add_header(header);
                }
                let _ = request.respond(response);
            }

            (Method::Get, "/messages") => {
                // Auth check for messages endpoint
                let settings = get_settings(app_handle);
                if !validate_auth(&request, &settings.connector_password) {
                    Self::respond_unauthorized(request, cors_headers);
                    return;
                }

                // Extension is polling for messages
                let now = now_ms();
                let old_poll = last_poll_at.swap(now, Ordering::SeqCst);

                // If this is first poll or we were offline, emit online status
                if old_poll == 0 || (now - old_poll) >= POLL_TIMEOUT_MS {
                    info!("Extension connected (polling started)");
                    let _ = app_handle.emit("extension-status-changed", ExtensionStatus::Online);
                    *was_online = true;
                }

                // Parse cursor from query string
                let cursor = request
                    .url()
                    .split('?')
                    .nth(1)
                    .and_then(|query| {
                        query
                            .split('&')
                            .find(|p| p.starts_with("since="))
                            .and_then(|p| p.strip_prefix("since="))
                            .and_then(|v| v.parse::<i64>().ok())
                    })
                    .unwrap_or(0);

                // Get messages newer than cursor
                let (messages, next_cursor) = {
                    let state_guard = state.lock().unwrap();
                    let filtered: Vec<_> = state_guard
                        .messages
                        .iter()
                        .filter(|m| m.ts > cursor)
                        .cloned()
                        .collect();
                    let next = filtered.last().map(|m| m.ts).unwrap_or(cursor);
                    (filtered, next)
                };

                // Get config from settings
                let settings = get_settings(app_handle);
                let auto_open_url = if settings.connector_auto_open_enabled
                    && !settings.connector_auto_open_url.is_empty()
                {
                    Some(settings.connector_auto_open_url.clone())
                } else {
                    None
                };

                let response_body = MessagesResponse {
                    cursor: next_cursor,
                    messages,
                    config: ExtensionConfig {
                        auto_open_tab_url: auto_open_url,
                    },
                };

                let json = serde_json::to_string(&response_body).unwrap_or_default();
                let mut response = Response::from_string(json);
                response.add_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                );
                for header in cors_headers {
                    response.add_header(header);
                }
                let _ = request.respond(response);
            }

            (Method::Post, "/messages") => {
                // Auth check for messages endpoint
                let settings = get_settings(app_handle);
                if !validate_auth(&request, &settings.connector_password) {
                    Self::respond_unauthorized(request, cors_headers);
                    return;
                }

                // Extension sending status/ack
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);

                if let Ok(post_body) = serde_json::from_str::<PostBody>(&body) {
                    if post_body.msg_type.as_deref() == Some("keepalive_ack") {
                        debug!("Received keepalive ack from extension");
                    } else if let Some(text) = post_body.text {
                        debug!("Received message from extension: {}", text);
                    }
                }

                // Update last poll time on POST too
                last_poll_at.store(now_ms(), Ordering::SeqCst);

                let json = r#"{"ok":true}"#;
                let mut response = Response::from_string(json);
                response.add_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                );
                for header in cors_headers {
                    response.add_header(header);
                }
                let _ = request.respond(response);
            }

            (Method::Get, blob_path) if blob_path.starts_with("/blob/") => {
                // Auth check for blob endpoint
                let settings = get_settings(app_handle);
                if !validate_auth(&request, &settings.connector_password) {
                    Self::respond_unauthorized(request, cors_headers);
                    return;
                }

                // Serve blob data for attachments
                let att_id = blob_path.strip_prefix("/blob/").unwrap_or("");
                
                let blob_data = {
                    let mut state_guard = state.lock().unwrap();
                    let now = now_ms();
                    
                    // Clean up expired blobs
                    state_guard.blobs.retain(|_, blob| blob.expires_at > now);
                    
                    // Get the requested blob
                    state_guard.blobs.get(att_id).cloned()
                };
                
                match blob_data {
                    Some(blob) => {
                        debug!("Serving blob {} ({} bytes, {})", att_id, blob.data.len(), blob.mime_type);
                        let mut response = Response::from_data(blob.data);
                        response.add_header(
                            Header::from_bytes(
                                &b"Content-Type"[..],
                                blob.mime_type.as_bytes()
                            ).unwrap(),
                        );
                        for header in cors_headers {
                            response.add_header(header);
                        }
                        let _ = request.respond(response);
                    }
                    None => {
                        debug!("Blob not found or expired: {}", att_id);
                        let mut response = Response::from_string("Blob not found").with_status_code(404);
                        for header in cors_headers {
                            response.add_header(header);
                        }
                        let _ = request.respond(response);
                    }
                }
            }

            _ => {
                // 404 for unknown paths
                let mut response = Response::from_string("Not Found").with_status_code(404);
                for header in cors_headers {
                    response.add_header(header);
                }
                let _ = request.respond(response);
            }
        }
    }

    /// Send keepalive message if enough time has passed
    fn maybe_send_keepalive(state: &Arc<Mutex<ConnectorState>>, now_ms: i64) {
        let mut state_guard = state.lock().unwrap();

        if now_ms - state_guard.last_keepalive > KEEPALIVE_INTERVAL_MS {
            state_guard.last_keepalive = now_ms;

            let keepalive = QueuedMessage {
                id: uuid_simple(),
                msg_type: "keepalive".to_string(),
                text: "keepalive".to_string(),
                ts: now_ms,
                attachments: None,
            };

            state_guard.messages.push_back(keepalive);

            // Trim old messages
            while state_guard.messages.len() > MAX_MESSAGES {
                state_guard.messages.pop_front();
            }
        }
    }

    /// Stop the HTTP server
    pub fn stop_server(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Queue a message to be sent to the extension
    pub fn queue_message(&self, text: &str) -> Result<(), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("Message is empty".to_string());
        }

        let msg = QueuedMessage {
            id: uuid_simple(),
            msg_type: "text".to_string(),
            text: trimmed.to_string(),
            ts: now_ms(),
            attachments: None,
        };

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.messages.push_back(msg);

        // Trim old messages
        while state.messages.len() > MAX_MESSAGES {
            state.messages.pop_front();
        }

        Ok(())
    }

    /// Queue a bundle message with an image attachment
    pub fn queue_bundle_message(&self, text: &str, image_path: &PathBuf) -> Result<(), String> {
        // Read the image file
        let data = std::fs::read(image_path)
            .map_err(|e| format!("Failed to read image file: {}", e))?;
        
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
        let now = now_ms();
        let expires_at = now + BLOB_EXPIRY_MS;
        
        // Get the port for constructing the URL
        let port = *self.port.read().unwrap();
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
                headers: Some(HashMap::new()),
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
            id: uuid_simple(),
            msg_type: "bundle".to_string(),
            text: text.trim().to_string(),
            ts: now,
            attachments: Some(vec![attachment]),
        };
        
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        
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
        
        debug!("Queued bundle message with image attachment ({} bytes)", file_size);
        Ok(())
    }

    /// Get current connection status
    pub fn get_status(&self) -> ConnectorStatus {
        let last_poll = self.last_poll_at.load(Ordering::SeqCst);
        let now = now_ms();
        let server_running = self.server_running.load(Ordering::SeqCst);
        let port = *self.port.read().unwrap();

        let status = if !server_running {
            ExtensionStatus::Unknown
        } else if last_poll == 0 {
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

    /// Send 401 Unauthorized response
    fn respond_unauthorized(request: Request, cors_headers: Vec<Header>) {
        let mut response = Response::from_string("Unauthorized").with_status_code(401);
        response.add_header(
            Header::from_bytes(&b"WWW-Authenticate"[..], &b"Bearer"[..]).unwrap(),
        );
        for header in cors_headers {
            response.add_header(header);
        }
        let _ = request.respond(response);
    }
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
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:032x}", ts)
}

/// Validate Authorization header against expected password
fn validate_auth(request: &Request, expected_password: &str) -> bool {
    // If no password configured, reject all requests
    if expected_password.is_empty() {
        return false;
    }
    
    // Check Authorization: Bearer <password>
    for header in request.headers() {
        if header.field.equiv("Authorization") {
            let value = header.value.as_str();
            if let Some(token) = value.strip_prefix("Bearer ") {
                return constant_time_eq(token.as_bytes(), expected_password.as_bytes());
            }
        }
    }
    false
}

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
