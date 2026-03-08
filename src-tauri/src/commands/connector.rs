//! Tauri commands for Connector Manager
//!
//! Commands to control and query the connector server status.

use crate::managers::connector::{ConnectorManager, ConnectorStatus};
use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use zip::ZipArchive;

const BUNDLED_EXTENSION_RESOURCE: &str = "browser-connector/aivorelay-extension.zip";
const EXPORTED_EXTENSION_FOLDER_NAME: &str = "AivoRelay Connector";

fn unzip_to_directory<R>(archive: &mut ZipArchive<R>, destination_dir: &Path) -> Result<(), String>
where
    R: Read + Seek,
{
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read bundled extension entry: {}", e))?;
        let enclosed_name = entry
            .enclosed_name()
            .map(|path| path.to_path_buf())
            .ok_or_else(|| format!("Unsafe bundled extension entry path: {}", entry.name()))?;
        let output_path = destination_dir.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|e| format!("Failed to create export directory: {}", e))?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create export directory: {}", e))?;
        }

        let mut output_file = File::create(&output_path)
            .map_err(|e| format!("Failed to create exported extension file: {}", e))?;
        io::copy(&mut entry, &mut output_file)
            .map_err(|e| format!("Failed to write exported extension file: {}", e))?;
    }

    Ok(())
}

fn resolve_bundled_extension_zip(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve(BUNDLED_EXTENSION_RESOURCE, BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve bundled extension resource: {}", e))
}

/// Get current connector/extension status
#[tauri::command]
#[specta::specta]
pub fn connector_get_status(manager: State<Arc<ConnectorManager>>) -> ConnectorStatus {
    manager.get_status()
}

/// Check if extension is currently online
#[tauri::command]
#[specta::specta]
pub fn connector_is_online(manager: State<Arc<ConnectorManager>>) -> bool {
    manager.is_online()
}

/// Start the connector server
#[tauri::command]
#[specta::specta]
pub fn connector_start_server(manager: State<Arc<ConnectorManager>>) -> Result<(), String> {
    manager.start_server()
}

/// Stop the connector server
#[tauri::command]
#[specta::specta]
pub fn connector_stop_server(manager: State<Arc<ConnectorManager>>) {
    manager.stop_server()
}

/// Queue a message to be sent to the extension
/// Returns the message ID on success
#[tauri::command]
#[specta::specta]
pub fn connector_queue_message(
    manager: State<Arc<ConnectorManager>>,
    text: String,
) -> Result<String, String> {
    manager.queue_message(&text)
}

/// Cancel a queued message if it hasn't been delivered yet
/// Returns true if message was cancelled, false if not found or already delivered
#[tauri::command]
#[specta::specta]
pub fn connector_cancel_message(
    manager: State<Arc<ConnectorManager>>,
    message_id: String,
) -> Result<bool, String> {
    manager.cancel_queued_message(&message_id)
}

/// Export the bundled browser connector extension zip into a folder selected by the user.
#[tauri::command]
#[specta::specta]
pub fn connector_export_bundled_extension(
    app: AppHandle,
    destination_dir: String,
) -> Result<String, String> {
    let trimmed_destination = destination_dir.trim();
    if trimmed_destination.is_empty() {
        return Err("Destination folder is required".to_string());
    }

    let destination_root = PathBuf::from(trimmed_destination);
    if !destination_root.exists() {
        return Err("Selected destination folder does not exist".to_string());
    }
    if !destination_root.is_dir() {
        return Err("Selected destination path is not a folder".to_string());
    }

    let bundled_zip_path = resolve_bundled_extension_zip(&app)?;
    let zip_file = File::open(&bundled_zip_path)
        .map_err(|e| format!("Failed to open bundled extension zip: {}", e))?;
    let mut archive = ZipArchive::new(zip_file)
        .map_err(|e| format!("Failed to read bundled extension zip: {}", e))?;

    let export_dir = destination_root.join(EXPORTED_EXTENSION_FOLDER_NAME);
    if export_dir.exists() {
        fs::remove_dir_all(&export_dir)
            .map_err(|e| format!("Failed to replace existing exported extension folder: {}", e))?;
    }
    fs::create_dir_all(&export_dir)
        .map_err(|e| format!("Failed to create export folder: {}", e))?;

    unzip_to_directory(&mut archive, &export_dir)?;

    let export_dir_string = export_dir.to_string_lossy().to_string();
    if let Err(error) = app
        .opener()
        .open_path(export_dir_string.clone(), None::<String>)
    {
        log::warn!(
            "Bundled extension exported to '{}' but failed to open the folder automatically: {}",
            export_dir_string,
            error
        );
    }

    Ok(export_dir_string)
}
