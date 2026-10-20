use crate::managers::local_tts::LocalTtsKind;
use crate::managers::model::ModelManager;
use crate::managers::model_storage::{reject_link, storage_bytes, LocalModelStorageEntry};
use crate::managers::tts::TtsManager;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(serde::Serialize, specta::Type)]
pub struct LocalModelStorageSnapshot {
    pub entries: Vec<LocalModelStorageEntry>,
    pub total_bytes: Option<u64>,
    pub disk_free_bytes: Option<u64>,
}

#[tauri::command]
#[specta::specta]
pub async fn get_local_model_storage(app: AppHandle) -> Result<LocalModelStorageSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let models = app.state::<Arc<ModelManager>>();
        let tts = app.state::<Arc<TtsManager>>();
        let mut entries = models.local_model_storage();
        for (kind, id, name) in [
            (LocalTtsKind::Qwen, "qwen", "Qwen3-TTS"),
            (LocalTtsKind::Kokoro, "kokoro", "Kokoro"),
        ] {
            let status = tts.local_tts_status(kind);
            let root = Path::new(&status.install_root);
            let measurement = storage_bytes(root);
            let mut blocked = if status.installing {
                Some("Wait for the TTS installation to finish, or cancel it first".into())
            } else {
                root.parent().map(reject_link).transpose().and_then(|_| reject_link(root))
                    .err().map(|error| error.to_string())
            };
            if let Err(error) = &measurement {
                blocked = Some(error.to_string());
            }
            let bytes_known = measurement.is_ok();
            let bytes = measurement.unwrap_or(0);
            if bytes > 0 || status.installed || status.installing || blocked.is_some() {
                entries.push(LocalModelStorageEntry {
                    id: id.into(), name: name.into(), kind: "tts".into(),
                    bytes, bytes_known, shared_cache_bytes: 0,
                    blocked_reason: blocked,
                });
            }
        }
        entries.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.cmp(&b.name)));
        let total_bytes = entries.iter().all(|entry| entry.bytes_known).then(||
            entries.iter().fold(0_u64, |sum, entry| sum.saturating_add(entry.bytes)));
        let disk_free_bytes = crate::portable::app_data_dir(&app).ok()
            .and_then(|path| fs2::available_space(path).ok());
        Ok(LocalModelStorageSnapshot { entries, total_bytes, disk_free_bytes })
    }).await.map_err(|error| format!("Model storage scan failed: {error}"))?
}

#[tauri::command]
#[specta::specta]
pub async fn cleanup_local_model(app: AppHandle, kind: String, model_id: String, selected_stt_model_ids: Vec<String>) -> Result<(), String> {
    match kind.as_str() {
        "stt" => tauri::async_runtime::spawn_blocking(move || {
            let selected = selected_stt_model_ids.into_iter().collect();
            super::models::delete_model_for_cleanup(&app, &model_id, &selected)
        }).await.map_err(|error| format!("Model cleanup task failed: {error}"))?,
        "tts" => {
            let kind = match model_id.as_str() {
                "qwen" => LocalTtsKind::Qwen,
                "kokoro" => LocalTtsKind::Kokoro,
                _ => return Err("Unknown local TTS model".into()),
            };
            app.state::<Arc<TtsManager>>().delete_local_tts(kind).await
                .map_err(|error| error.to_string())
        }
        _ => Err("Unknown local model type".into()),
    }
}
