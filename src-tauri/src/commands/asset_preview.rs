use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use super::file_transcription::SUPPORTED_EXTENSIONS;

const TRANSCRIBE_FILE_PREVIEW_DIR: &str = "transcribe-file-preview";

fn preview_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let cache_dir = crate::portable::app_cache_dir(app)
        .map_err(|e| format!("Failed to get app cache directory: {}", e))?;

    Ok(cache_dir.join(TRANSCRIBE_FILE_PREVIEW_DIR))
}

fn sanitize_file_stem(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("audio-preview");

    let sanitized: String = stem
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect();

    if sanitized.is_empty() {
        "audio-preview".to_string()
    } else {
        sanitized
    }
}

fn build_preview_file_name(source_path: &Path) -> String {
    let file_stem = sanitize_file_stem(source_path);
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    match source_path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => {
            format!("{file_stem}-{unique_suffix}.{extension}")
        }
        _ => format!("{file_stem}-{unique_suffix}"),
    }
}

/// Removes preview copies left behind by earlier app runs. Names end in
/// `-<unix nanos>`; only files stamped before this process started are removed,
/// so a preview prepared in this run is never touched. The stamp is used instead
/// of mtime because hard links share the source's mtime.
pub fn spawn_stale_preview_cleanup(app: &AppHandle) {
    let started_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let Ok(dir) = preview_dir(app) else {
        return;
    };
    std::thread::spawn(move || {
        let Ok(entries) = fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            let stamp = path
                .file_stem()
                .and_then(|value| value.to_str())
                .and_then(|stem| stem.rsplit_once('-'))
                .and_then(|(_, digits)| digits.parse::<u128>().ok());
            match stamp {
                Some(stamp) if stamp < started_nanos => match fs::remove_file(&path) {
                    Ok(()) => log::debug!("Removed stale transcribe-file preview {}", path.display()),
                    Err(error) => log::warn!(
                        "Failed to remove stale transcribe-file preview {}: {}",
                        path.display(),
                        error
                    ),
                },
                Some(_) => {}
                None => log::debug!(
                    "Skipping unrecognized file in transcribe-file preview cache: {}",
                    path.display()
                ),
            }
        }
    });
}

#[tauri::command]
#[specta::specta]
pub async fn prepare_transcribe_file_asset(
    app: AppHandle,
    source_path: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || prepare_transcribe_file_asset_blocking(app, source_path))
        .await
        .map_err(|error| format!("Preview staging task failed: {error}"))?
}

fn prepare_transcribe_file_asset_blocking(
    app: AppHandle,
    source_path: String,
) -> Result<String, String> {
    let source_path = PathBuf::from(source_path);
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "Unsupported audio format: .{}. Supported formats: {}",
            extension,
            SUPPORTED_EXTENSIONS.join(", ")
        ));
    }

    if !source_path.is_file() {
        return Err("Selected file does not exist".to_string());
    }

    let preview_dir = preview_dir(&app)?;
    fs::create_dir_all(&preview_dir)
        .map_err(|e| format!("Failed to create preview cache directory: {}", e))?;

    let staged_path = preview_dir.join(build_preview_file_name(&source_path));

    match fs::hard_link(&source_path, &staged_path) {
        Ok(_) => {}
        Err(_) => {
            fs::copy(&source_path, &staged_path)
                .map_err(|e| format!("Failed to stage preview file: {}", e))?;
        }
    }

    Ok(staged_path.to_string_lossy().to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_transcribe_file_asset(app: AppHandle, staged_path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || delete_transcribe_file_asset_blocking(app, staged_path))
        .await
        .map_err(|error| format!("Preview cleanup task failed: {error}"))?
}

fn delete_transcribe_file_asset_blocking(app: AppHandle, staged_path: String) -> Result<(), String> {
    if staged_path.trim().is_empty() {
        return Ok(());
    }

    let staged_path = PathBuf::from(staged_path);
    let preview_dir = preview_dir(&app)?;

    if staged_path.parent() != Some(preview_dir.as_path()) {
        return Err("Refusing to delete file outside preview cache".to_string());
    }

    if staged_path.exists() {
        fs::remove_file(&staged_path)
            .map_err(|e| format!("Failed to delete preview file: {}", e))?;
    }

    Ok(())
}
