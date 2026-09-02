//! Tauri commands for Connector Manager
//!
//! Commands to control and query the connector server status.

use crate::managers::connector::{active_pending_password, ConnectorManager, ConnectorStatus};
use crate::settings::{get_settings, write_settings_checked};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rsa::pkcs8::EncodePublicKey;
use rsa::rand_core::OsRng;
use rsa::RsaPrivateKey;
use serde::Serialize;
use sha2::{Digest, Sha256};
use specta::Type;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use zip::ZipArchive;

const BUNDLED_EXTENSION_RESOURCE_CANDIDATES: &[&str] = &[
    "resources/browser-connector/aivorelay-extension.zip",
    "browser-connector/aivorelay-extension.zip",
];
const EXPORTED_EXTENSION_FOLDER_NAME: &str = "AivoRelay Connector";
const CHROME_EXTENSION_ORIGIN_PREFIX: &str = "chrome-extension://";
const EXTENSION_PASSWORD_FILES: &[&str] = &["popup.js", "sw-config.js"];
const EXTENSION_SETTINGS_FILES: &[&str] = &["popup.js", "sw-config.js"];
const STAGING_FOLDER_NAME: &str = "connector-export-staging";

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BundledExtensionExportResult {
    pub export_path: String,
    pub extension_id: String,
    pub configured_origin: String,
    pub generated_password: String,
    pub reused_existing_id: bool,
    pub replaced_existing_export: bool,
}

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

fn copy_directory_recursive(source_dir: &Path, destination_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(destination_dir)
        .map_err(|e| format!("Failed to create export directory: {}", e))?;

    for entry in
        fs::read_dir(source_dir).map_err(|e| format!("Failed to read staging directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read staging entry: {}", e))?;
        let source_path = entry.path();
        let destination_path = destination_dir.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Failed to inspect staging entry type: {}", e))?;

        if file_type.is_dir() {
            copy_directory_recursive(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create export directory: {}", e))?;
            }
            fs::copy(&source_path, &destination_path)
                .map_err(|e| format!("Failed to copy exported extension file: {}", e))?;
        }
    }

    Ok(())
}

fn resolve_bundled_extension_zip(app: &AppHandle) -> Result<PathBuf, String> {
    // In dev mode, prefer the source resource over the stale build-cache copy
    // in target/debug/resources/. Tauri dev rebuilds on code changes but does
    // NOT re-copy resource files, so the target copy can be arbitrarily stale.
    let dev_candidates: &[&str] = &[
        // CWD is repo root (e.g. launched from workspace root)
        "src-tauri/resources/browser-connector/aivorelay-extension.zip",
        // CWD is src-tauri/ (normal cargo tauri dev)
        "resources/browser-connector/aivorelay-extension.zip",
    ];
    if let Ok(cwd) = env::current_dir() {
        for relative in dev_candidates {
            let candidate = cwd.join(relative);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    for relative_path in BUNDLED_EXTENSION_RESOURCE_CANDIDATES {
        if let Ok(resolved) = app.path().resolve(relative_path, BaseDirectory::Resource) {
            if resolved.exists() {
                return Ok(resolved);
            }
        }
    }

    Err("Failed to locate bundled extension zip in app resources or the dev workspace".to_string())
}

fn generate_secure_password() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| format!("Failed to generate connector password: {}", e))?;

    let mut password = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        password.push_str(&format!("{:02x}", byte));
    }
    Ok(password)
}

fn generate_random_hex_token(byte_len: usize) -> Result<String, String> {
    let mut bytes = vec![0u8; byte_len];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| format!("Failed to generate random token: {}", e))?;

    let mut token = String::with_capacity(byte_len * 2);
    for byte in bytes {
        token.push_str(&format!("{:02x}", byte));
    }
    Ok(token)
}

fn generate_extension_manifest_key() -> Result<(String, String), String> {
    let private_key = RsaPrivateKey::new(&mut OsRng, 2048)
        .map_err(|e| format!("Failed to generate extension keypair: {}", e))?;
    let public_key_der = private_key
        .to_public_key()
        .to_public_key_der()
        .map_err(|e| format!("Failed to encode extension public key: {}", e))?;
    let public_key_bytes = public_key_der.as_bytes();
    let manifest_key = STANDARD.encode(public_key_bytes);

    let digest = Sha256::digest(public_key_bytes);
    let mut extension_id = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        extension_id.push((b'a' + (byte >> 4)) as char);
        extension_id.push((b'a' + (byte & 0x0f)) as char);
    }

    Ok((manifest_key, extension_id))
}

fn resolve_export_dir(destination_root: &Path) -> PathBuf {
    if destination_root
        .file_name()
        .map(|name| name == EXPORTED_EXTENSION_FOLDER_NAME)
        .unwrap_or(false)
    {
        destination_root.to_path_buf()
    } else {
        destination_root.join(EXPORTED_EXTENSION_FOLDER_NAME)
    }
}

fn normalize_metadata_path_string(raw: &str) -> String {
    let normalized = raw.trim().replace('/', "\\");
    let trimmed = normalized.trim_end_matches('\\');
    #[cfg(target_os = "windows")]
    {
        trimmed.to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        trimmed.to_string()
    }
}

fn normalize_metadata_path(path: &Path) -> String {
    normalize_metadata_path_string(&path.to_string_lossy())
}

fn stored_export_matches(settings: &crate::settings::AppSettings, export_dir: &Path) -> bool {
    if settings.connector_last_export_dir.trim().is_empty() {
        return false;
    }

    normalize_metadata_path_string(&settings.connector_last_export_dir)
        == normalize_metadata_path(export_dir)
}

fn patch_exported_manifest(export_dir: &Path, manifest_key: &str) -> Result<(), String> {
    let manifest_path = export_dir.join("manifest.json");
    let manifest_contents = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read exported manifest.json: {}", e))?;
    let mut manifest_json: serde_json::Value = serde_json::from_str(&manifest_contents)
        .map_err(|e| format!("Failed to parse exported manifest.json: {}", e))?;
    let manifest_object = manifest_json
        .as_object_mut()
        .ok_or_else(|| "Exported manifest.json does not contain a JSON object".to_string())?;
    manifest_object.insert(
        "key".to_string(),
        serde_json::Value::String(manifest_key.to_string()),
    );
    let updated_manifest = serde_json::to_string_pretty(&manifest_json)
        .map_err(|e| format!("Failed to serialize exported manifest.json: {}", e))?;
    fs::write(&manifest_path, format!("{}\n", updated_manifest))
        .map_err(|e| format!("Failed to write exported manifest.json: {}", e))
}
fn patch_exported_provisioned_password(
    export_dir: &Path,
    generated_password: &str,
) -> Result<(), String> {
    let escaped_password = serde_json::to_string(generated_password)
        .map_err(|e| format!("Failed to escape generated connector password: {}", e))?;
    let replacement_line = format!(
        "const PROVISIONED_CONNECTOR_PASSWORD = {};",
        escaped_password
    );
    let search_pattern = "const PROVISIONED_CONNECTOR_PASSWORD = \"\";";

    for relative_path in EXTENSION_PASSWORD_FILES {
        let file_path = export_dir.join(relative_path);
        let original = fs::read_to_string(&file_path).map_err(|e| {
            format!(
                "Failed to read exported extension file '{}': {}",
                relative_path, e
            )
        })?;
        let updated = original.replacen(search_pattern, &replacement_line, 1);
        if updated == original {
            return Err(format!(
                "Failed to provision password in exported extension file '{}'",
                relative_path
            ));
        }
        fs::write(&file_path, updated).map_err(|e| {
            format!(
                "Failed to write exported extension file '{}': {}",
                relative_path, e
            )
        })?;
    }

    Ok(())
}

fn patch_exported_default_port(export_dir: &Path, port: u16) -> Result<(), String> {
    let replacement = format!("port: {}", port);

    for relative_path in EXTENSION_SETTINGS_FILES {
        let file_path = export_dir.join(relative_path);
        let original = fs::read_to_string(&file_path).map_err(|e| {
            format!(
                "Failed to read exported extension settings file '{}': {}",
                relative_path, e
            )
        })?;
        if original.contains(&replacement) {
            continue;
        }
        let updated = original.replacen("port: 38243", &replacement, 1);
        if updated == original {
            return Err(format!(
                "Failed to update default port in exported extension file '{}'",
                relative_path
            ));
        }
        fs::write(&file_path, updated).map_err(|e| {
            format!(
                "Failed to write exported extension settings file '{}': {}",
                relative_path, e
            )
        })?;
    }

    Ok(())
}

fn apply_exported_extension_pairing(
    app: &AppHandle,
    export_dir: &Path,
    extension_id: &str,
    manifest_key: &str,
    connector_password: &str,
) -> Result<(), String> {
    let mut settings = get_settings(app);
    settings.connector_allow_any_cors = false;
    settings.connector_cors = format!("{}{}", CHROME_EXTENSION_ORIGIN_PREFIX, extension_id);
    settings.connector_password = connector_password.to_string().into();
    settings.connector_password_user_set = false;
    settings.connector_pending_password = None.into();
    settings.connector_pending_password_issued_at_ms = 0;
    settings.connector_last_export_dir = export_dir.to_string_lossy().to_string();
    settings.connector_last_export_extension_id = extension_id.to_string();
    settings.connector_last_export_manifest_key = manifest_key.to_string();
    write_settings_checked(app, settings.clone())?;

    if let Some(connector_manager) = app.try_state::<Arc<ConnectorManager>>() {
        connector_manager.reload_runtime_config_async();
        connector_manager.refresh_crypto_state(&settings.connector_password, None);
        connector_manager.clear_sessions();
    }

    Ok(())
}

fn restore_exported_extension_backup(
    export_dir: &Path,
    backup_dir: Option<&Path>,
) -> Result<(), String> {
    if export_dir.exists() {
        fs::remove_dir_all(export_dir)
            .map_err(|e| format!("Failed to remove partial exported extension folder: {}", e))?;
    }

    if let Some(backup_dir) = backup_dir {
        fs::rename(backup_dir, export_dir).map_err(|e| {
            format!(
                "Failed to restore previous exported extension folder: {}",
                e
            )
        })?;
    }

    Ok(())
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
    generate_new_id: bool,
) -> Result<BundledExtensionExportResult, String> {
    let trimmed_destination = destination_dir.trim();
    if trimmed_destination.is_empty() {
        return Err("Destination folder is required".to_string());
    }

    let destination_root = PathBuf::from(trimmed_destination);
    if !destination_root.exists() {
        fs::create_dir_all(&destination_root)
            .map_err(|e| format!("Failed to create selected destination folder: {}", e))?;
    }
    if !destination_root.is_dir() {
        return Err("Selected destination path is not a folder".to_string());
    }

    let current_settings = get_settings(&app);
    let bundled_zip_path = resolve_bundled_extension_zip(&app)?;
    log::info!(
        "Exporting bundled extension from zip: {}",
        bundled_zip_path.display()
    );
    let zip_file = File::open(&bundled_zip_path)
        .map_err(|e| format!("Failed to open bundled extension zip: {}", e))?;
    let mut archive = ZipArchive::new(zip_file)
        .map_err(|e| format!("Failed to read bundled extension zip: {}", e))?;
    let app_data_dir = crate::portable::app_data_dir(&app).map_err(|e| {
        format!(
            "Failed to resolve app data directory for export staging: {}",
            e
        )
    })?;
    let staging_root = app_data_dir
        .join(STAGING_FOLDER_NAME)
        .join(generate_random_hex_token(8)?);
    fs::create_dir_all(&staging_root)
        .map_err(|e| format!("Failed to create staging export folder: {}", e))?;

    unzip_to_directory(&mut archive, &staging_root)?;

    let export_dir = resolve_export_dir(&destination_root);
    let export_dir_string = export_dir.to_string_lossy().to_string();

    let replaced_existing_export = export_dir.exists();
    let known_export_for_path = !generate_new_id
        && stored_export_matches(&current_settings, &export_dir)
        && !current_settings
            .connector_last_export_manifest_key
            .trim()
            .is_empty()
        && !current_settings
            .connector_last_export_extension_id
            .trim()
            .is_empty();

    let (manifest_key, extension_id, connector_password, reused_existing_id) =
        if known_export_for_path {
            let effective_password = active_pending_password(&app, &current_settings)
                .unwrap_or_else(|| current_settings.connector_password.to_string());
            (
                current_settings.connector_last_export_manifest_key.clone(),
                current_settings.connector_last_export_extension_id.clone(),
                effective_password,
                true,
            )
        } else {
            let (new_key, new_id) = generate_extension_manifest_key()?;
            let new_password = generate_secure_password()?;
            (new_key, new_id, new_password, false)
        };

    patch_exported_manifest(&staging_root, &manifest_key)?;
    patch_exported_provisioned_password(&staging_root, &connector_password)?;
    patch_exported_default_port(&staging_root, current_settings.connector_port)?;

    let backup_dir = if export_dir.exists() {
        let backup_name = format!(
            "{}-backup-{}",
            EXPORTED_EXTENSION_FOLDER_NAME,
            generate_random_hex_token(6)?
        );
        let backup_dir = export_dir
            .parent()
            .unwrap_or(&destination_root)
            .join(backup_name);
        fs::rename(&export_dir, &backup_dir).map_err(|e| {
            format!(
                "Failed to move existing exported extension folder out of the way: {}",
                e
            )
        })?;
        Some(backup_dir)
    } else {
        None
    };

    let copy_result = copy_directory_recursive(&staging_root, &export_dir);
    if let Err(err) = copy_result {
        let _ = fs::remove_dir_all(&staging_root);
        if let Err(restore_err) =
            restore_exported_extension_backup(&export_dir, backup_dir.as_deref())
        {
            return Err(format!("{} (restore failed: {})", err, restore_err));
        }
        return Err(err);
    }

    if let Err(err) = apply_exported_extension_pairing(
        &app,
        &export_dir,
        &extension_id,
        &manifest_key,
        &connector_password,
    ) {
        let _ = fs::remove_dir_all(&staging_root);
        if let Err(restore_err) =
            restore_exported_extension_backup(&export_dir, backup_dir.as_deref())
        {
            return Err(format!("{} (restore failed: {})", err, restore_err));
        }
        return Err(err);
    }

    if let Some(backup_dir) = backup_dir.as_ref() {
        if let Err(error) = fs::remove_dir_all(backup_dir) {
            log::warn!(
                "Export succeeded but failed to remove the backup extension folder '{}': {}",
                backup_dir.display(),
                error
            );
        }
    }

    let _ = fs::remove_dir_all(&staging_root);

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

    Ok(BundledExtensionExportResult {
        export_path: export_dir_string,
        extension_id: extension_id.clone(),
        configured_origin: format!("{}{}", CHROME_EXTENSION_ORIGIN_PREFIX, extension_id),
        generated_password: connector_password,
        reused_existing_id,
        replaced_existing_export,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use std::sync::atomic::{AtomicU64, Ordering};
    use zip::write::FileOptions;
    use zip::ZipWriter;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "aivorelay-connector-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn archive_with_files(files: &[(&str, &[u8])]) -> ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in files {
            writer.start_file(*name, FileOptions::default()).unwrap();
            writer.write_all(contents).unwrap();
        }
        ZipArchive::new(writer.finish().unwrap()).unwrap()
    }

    #[test]
    fn export_directory_resolution_is_idempotent_for_the_named_folder() {
        let root = Path::new("Q:/Exports");
        let expected = root.join(EXPORTED_EXTENSION_FOLDER_NAME);

        assert_eq!(resolve_export_dir(root), expected);
        assert_eq!(resolve_export_dir(&expected), expected);
    }

    #[test]
    fn stored_path_normalization_handles_whitespace_separators_and_trailing_slashes() {
        let actual = normalize_metadata_path_string("  Q:/Exports/AivoRelay Connector///  ");
        let expected = if cfg!(target_os = "windows") {
            r"q:\exports\aivorelay connector"
        } else {
            r"Q:\Exports\AivoRelay Connector"
        };

        assert_eq!(actual, expected);
        assert_eq!(
            normalize_metadata_path_string(r"Q:\Exports"),
            normalize_metadata_path_string("Q:/Exports/")
        );
    }

    #[test]
    fn stored_export_match_requires_a_nonempty_equivalent_path() {
        let directory = TestDirectory::new("stored-export");
        let export = directory.path().join(EXPORTED_EXTENSION_FOLDER_NAME);
        let mut settings = crate::settings::get_default_settings();

        settings.connector_last_export_dir = String::new();
        assert!(!stored_export_matches(&settings, &export));

        settings.connector_last_export_dir =
            format!("{}/", export.to_string_lossy().replace('\\', "/"));
        assert!(stored_export_matches(&settings, &export));

        settings.connector_last_export_dir = directory
            .path()
            .join("another")
            .to_string_lossy()
            .to_string();
        assert!(!stored_export_matches(&settings, &export));
    }

    #[test]
    fn manifest_patch_preserves_existing_fields_and_writes_a_stable_key() {
        let directory = TestDirectory::new("manifest");
        let manifest_path = directory.path().join("manifest.json");
        fs::write(
            &manifest_path,
            r#"{"manifest_version":3,"name":"AivoRelay","permissions":["storage"]}"#,
        )
        .unwrap();

        patch_exported_manifest(directory.path(), "public-key-base64").unwrap();

        let updated = fs::read_to_string(manifest_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(parsed["manifest_version"], 3);
        assert_eq!(parsed["name"], "AivoRelay");
        assert_eq!(parsed["permissions"], serde_json::json!(["storage"]));
        assert_eq!(parsed["key"], "public-key-base64");
        assert!(updated.ends_with('\n'));
    }

    #[test]
    fn manifest_patch_rejects_non_object_json_without_rewriting_it() {
        let directory = TestDirectory::new("manifest-array");
        let manifest_path = directory.path().join("manifest.json");
        fs::write(&manifest_path, "[1,2,3]\n").unwrap();

        let error = patch_exported_manifest(directory.path(), "key").unwrap_err();

        assert!(error.contains("does not contain a JSON object"));
        assert_eq!(fs::read_to_string(manifest_path).unwrap(), "[1,2,3]\n");
    }

    #[test]
    fn provisioned_password_is_json_escaped_in_every_extension_entrypoint() {
        let directory = TestDirectory::new("password");
        let marker = "const PROVISIONED_CONNECTOR_PASSWORD = \"\";";
        for filename in EXTENSION_PASSWORD_FILES {
            fs::write(
                directory.path().join(filename),
                format!("before\n{marker}\nafter\n"),
            )
            .unwrap();
        }
        let password = "quote\" backslash\\ newline\nvalue";

        patch_exported_provisioned_password(directory.path(), password).unwrap();

        let expected = format!(
            "const PROVISIONED_CONNECTOR_PASSWORD = {};",
            serde_json::to_string(password).unwrap()
        );
        for filename in EXTENSION_PASSWORD_FILES {
            let updated = fs::read_to_string(directory.path().join(filename)).unwrap();
            assert!(updated.contains(&expected), "{filename}");
            assert!(!updated.contains(marker), "{filename}");
        }
    }

    #[test]
    fn password_provisioning_fails_when_the_expected_marker_is_absent() {
        let directory = TestDirectory::new("password-marker");
        for filename in EXTENSION_PASSWORD_FILES {
            fs::write(directory.path().join(filename), "const unrelated = true;\n").unwrap();
        }

        let error = patch_exported_provisioned_password(directory.path(), "password").unwrap_err();

        assert!(error.contains("Failed to provision password"));
        for filename in EXTENSION_PASSWORD_FILES {
            assert_eq!(
                fs::read_to_string(directory.path().join(filename)).unwrap(),
                "const unrelated = true;\n"
            );
        }
    }

    #[test]
    fn default_port_patch_updates_every_entrypoint_and_is_idempotent() {
        let directory = TestDirectory::new("port");
        for filename in EXTENSION_SETTINGS_FILES {
            fs::write(
                directory.path().join(filename),
                "const settings = { host: '127.0.0.1', port: 38243 };\n",
            )
            .unwrap();
        }

        patch_exported_default_port(directory.path(), 45678).unwrap();
        patch_exported_default_port(directory.path(), 45678).unwrap();

        for filename in EXTENSION_SETTINGS_FILES {
            let updated = fs::read_to_string(directory.path().join(filename)).unwrap();
            assert!(updated.contains("port: 45678"), "{filename}");
            assert!(!updated.contains("port: 38243"), "{filename}");
        }
    }

    #[test]
    fn default_port_patch_fails_closed_when_the_expected_setting_is_absent() {
        let directory = TestDirectory::new("port-marker");
        for filename in EXTENSION_SETTINGS_FILES {
            fs::write(directory.path().join(filename), "const settings = {};\n").unwrap();
        }

        let error = patch_exported_default_port(directory.path(), 45678).unwrap_err();

        assert!(error.contains("Failed to update default port"));
        for filename in EXTENSION_SETTINGS_FILES {
            assert_eq!(
                fs::read_to_string(directory.path().join(filename)).unwrap(),
                "const settings = {};\n"
            );
        }
    }

    #[test]
    fn recursive_copy_preserves_nested_files_and_contents() {
        let directory = TestDirectory::new("copy");
        let source = directory.path().join("source");
        let destination = directory.path().join("destination");
        fs::create_dir_all(source.join("nested/deeper")).unwrap();
        fs::write(source.join("root.txt"), b"root").unwrap();
        fs::write(source.join("nested/deeper/file.bin"), [0, 1, 2, 255]).unwrap();

        copy_directory_recursive(&source, &destination).unwrap();

        assert_eq!(
            fs::read(destination.join("root.txt")).unwrap().as_slice(),
            &b"root"[..]
        );
        assert_eq!(
            fs::read(destination.join("nested/deeper/file.bin"))
                .unwrap()
                .as_slice(),
            &[0, 1, 2, 255]
        );
    }

    #[test]
    fn zip_extraction_preserves_nested_files_inside_the_destination() {
        let directory = TestDirectory::new("unzip");
        let destination = directory.path().join("destination");
        let mut archive = archive_with_files(&[
            ("manifest.json", br#"{"manifest_version":3}"#),
            ("assets/icon.txt", b"icon"),
        ]);

        unzip_to_directory(&mut archive, &destination).unwrap();

        assert_eq!(
            fs::read_to_string(destination.join("manifest.json")).unwrap(),
            r#"{"manifest_version":3}"#
        );
        assert_eq!(
            fs::read(destination.join("assets/icon.txt"))
                .unwrap()
                .as_slice(),
            &b"icon"[..]
        );
    }

    #[test]
    fn zip_extraction_rejects_parent_directory_traversal() {
        let directory = TestDirectory::new("zip-slip");
        let destination = directory.path().join("destination");
        let mut archive = archive_with_files(&[("../outside.txt", b"escape")]);

        let error = unzip_to_directory(&mut archive, &destination).unwrap_err();

        assert!(error.contains("Unsafe bundled extension entry path"));
        assert!(!directory.path().join("outside.txt").exists());
    }
}
