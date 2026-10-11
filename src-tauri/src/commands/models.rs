use crate::managers::model::{ModelInfo, ModelManager};
use crate::managers::transcription::{LoadingGuard, ModelStateEvent, TranscriptionManager};
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout, TranscriptionProvider};
use crate::tray;
use serde::Serialize;
use specta::Type;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

const MODEL_DELETION_DELAY: Duration = Duration::from_secs(5);

#[derive(Clone, Serialize, Type)]
pub struct ModelDeletionJob {
    pub model_id: String,
    pub job_id: String,
    pub status: String,
    pub deadline_unix_ms: u64,
    pub remaining_ms: u64,
}

#[derive(Clone, Serialize, Type)]
pub struct ModelDeletionChange {
    pub model_id: String,
    pub job_id: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Type)]
pub struct ModelDeletionSnapshot {
    pub revision: u64,
    pub jobs: Vec<ModelDeletionJob>,
    pub change: Option<ModelDeletionChange>,
}

#[derive(Serialize, Type)]
pub struct ModelDeletionCancellation {
    pub outcome: String,
    pub snapshot: ModelDeletionSnapshot,
}

struct ScheduledModelDeletion {
    job: ModelDeletionJob,
    deadline: Instant,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Default)]
struct ModelDeletionInner {
    revision: u64,
    next_job_id: u64,
    jobs: BTreeMap<String, ScheduledModelDeletion>,
    change: Option<ModelDeletionChange>,
}

#[derive(Default)]
pub struct ModelDeletionState {
    inner: Mutex<ModelDeletionInner>,
}

impl ModelDeletionInner {
    fn snapshot(&self) -> ModelDeletionSnapshot {
        let now = Instant::now();
        ModelDeletionSnapshot {
            revision: self.revision,
            jobs: self.jobs.values().map(|scheduled| {
                let mut job = scheduled.job.clone();
                job.remaining_ms = if job.status == "pending" {
                    scheduled.deadline.saturating_duration_since(now).as_millis() as u64
                } else {
                    0
                };
                job
            }).collect(),
            change: self.change.clone(),
        }
    }

    fn changed(
        &mut self,
        app: &AppHandle,
        model_id: &str,
        job_id: &str,
        status: &str,
        error: Option<String>,
    ) -> ModelDeletionSnapshot {
        self.revision += 1;
        self.change = Some(ModelDeletionChange {
            model_id: model_id.to_string(),
            job_id: job_id.to_string(),
            status: status.to_string(),
            error,
        });
        let snapshot = self.snapshot();
        // Transition and publication share this short lock; no lock crosses an await.
        let _ = app.emit("model-deletion-state", snapshot.clone());
        snapshot
    }

    fn cancel_for_use(&mut self, app: &AppHandle, model_id: &str) -> Result<(), String> {
        let Some(scheduled) = self.jobs.get(model_id) else {
            return Ok(());
        };
        if scheduled.job.status == "deleting" {
            return Err("Model deletion has already started".to_string());
        }
        let job_id = scheduled.job.job_id.clone();
        if let Some(mut scheduled) = self.jobs.remove(model_id) {
            if let Some(cancel) = scheduled.cancel.take() {
                let _ = cancel.send(());
            }
        }
        self.changed(app, model_id, &job_id, "cancelled", None);
        Ok(())
    }
}

/// Selection and the timer's deleting claim use the same short critical section.
/// The caller retains the returned loading claim until model/settings work finishes.
fn claim_model_selection(app: &AppHandle, model_id: &str) -> Result<LoadingGuard, String> {
    let transcription_manager = app.state::<Arc<TranscriptionManager>>();
    if let Some(state) = app.try_state::<Arc<ModelDeletionState>>() {
        let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
        inner.cancel_for_use(app, model_id)?;
        return transcription_manager.try_start_loading()
            .ok_or_else(|| "Model load already in progress".to_string());
    }
    transcription_manager.try_start_loading()
        .ok_or_else(|| "Model load already in progress".to_string())
}

pub(crate) fn cancel_pending_model_deletion_for_use(
    app: &AppHandle,
    model_id: &str,
) -> Result<(), String> {
    let Some(state) = app.try_state::<Arc<ModelDeletionState>>() else {
        return Ok(());
    };
    let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
    inner.cancel_for_use(app, model_id)
}

pub(crate) fn with_model_deletion_cancelled_for_selection<T>(
    app: &AppHandle,
    model_id: &str,
    select: impl FnOnce() -> T,
) -> Result<T, String> {
    let Some(state) = app.try_state::<Arc<ModelDeletionState>>() else {
        return Ok(select());
    };
    let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
    inner.cancel_for_use(app, model_id)?;
    Ok(select())
}

#[tauri::command]
#[specta::specta]
pub fn get_model_deletion_snapshot(
    deletion_state: State<'_, Arc<ModelDeletionState>>,
) -> ModelDeletionSnapshot {
    let inner = deletion_state.inner.lock().unwrap_or_else(|error| error.into_inner());
    inner.snapshot()
}

#[tauri::command]
#[specta::specta]
pub fn cancel_model_deletion(
    app_handle: AppHandle,
    deletion_state: State<'_, Arc<ModelDeletionState>>,
    model_id: String,
    job_id: String,
) -> ModelDeletionCancellation {
    let mut inner = deletion_state.inner.lock().unwrap_or_else(|error| error.into_inner());
    let outcome = match inner.jobs.get(&model_id) {
        Some(scheduled) if scheduled.job.job_id == job_id => {
            if scheduled.job.status == "deleting" {
                "deleting"
            } else {
                // This token still owns the pending entry while the mutex is held.
                let _ = inner.cancel_for_use(&app_handle, &model_id);
                "cancelled"
            }
        }
        _ => "not_found",
    };
    ModelDeletionCancellation { outcome: outcome.to_string(), snapshot: inner.snapshot() }
}

#[tauri::command]
#[specta::specta]
pub fn schedule_model_deletion(
    app_handle: AppHandle,
    deletion_state: State<'_, Arc<ModelDeletionState>>,
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<ModelDeletionSnapshot, String> {
    let state = deletion_state.inner().clone();
    let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(scheduled) = inner.jobs.get(&model_id) {
        return if scheduled.job.status == "pending" {
            Ok(inner.snapshot())
        } else {
            Err("Model deletion has already started".to_string())
        };
    }
    let info = model_manager.get_model_info(&model_id)
        .ok_or_else(|| format!("Model not found: {model_id}"))?;
    if info.is_downloading {
        return Err("Cannot delete a model while its download is in progress".to_string());
    }
    if !info.is_downloaded {
        return Err("Model is not downloaded".to_string());
    }
    inner.next_job_id += 1;
    let job_id = inner.next_job_id.to_string();
    let deadline = Instant::now() + MODEL_DELETION_DELAY;
    let (cancel, mut cancellation) = tokio::sync::oneshot::channel();
    inner.jobs.insert(model_id.clone(), ScheduledModelDeletion {
        job: ModelDeletionJob {
            model_id: model_id.clone(),
            job_id: job_id.clone(),
            status: "pending".to_string(),
            deadline_unix_ms: unix_ms_now() + MODEL_DELETION_DELAY.as_millis() as u64,
            remaining_ms: MODEL_DELETION_DELAY.as_millis() as u64,
        },
        deadline,
        cancel: Some(cancel),
    });
    let snapshot = inner.changed(&app_handle, &model_id, &job_id, "pending", None);
    drop(inner);

    tauri::async_runtime::spawn(async move {
        tokio::select! {
            _ = &mut cancellation => return,
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {}
        }
        let loading_guard = loop {
            let loading_guard = {
                let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
                let Some(scheduled) = inner.jobs.get(&model_id) else { return; };
                if scheduled.job.job_id != job_id || scheduled.job.status != "pending" {
                    return;
                }
                let transcription_manager = app_handle.state::<Arc<TranscriptionManager>>();
                let guard = transcription_manager.try_start_loading();
                if guard.is_some() {
                    let scheduled = inner.jobs.get_mut(&model_id).unwrap();
                    scheduled.job.status = "deleting".to_string();
                    scheduled.cancel = None;
                    inner.changed(&app_handle, &model_id, &job_id, "deleting", None);
                }
                guard
            };
            if let Some(guard) = loading_guard {
                break guard;
            }
            tokio::select! {
                _ = &mut cancellation => return,
                _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        };
        let app = app_handle.clone();
        let delete_id = model_id.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            delete_model_with_loading_claim(&app, &delete_id, loading_guard)
        }).await.map_err(|error| format!("Model deletion task failed: {error}"))
            .and_then(|result| result);

        let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
        if inner.jobs.get(&model_id).map(|scheduled| scheduled.job.job_id.as_str())
            != Some(job_id.as_str()) {
            return;
        }
        inner.jobs.remove(&model_id);
        match result {
            Ok(()) => { inner.changed(&app_handle, &model_id, &job_id, "completed", None); }
            Err(error) => { inner.changed(&app_handle, &model_id, &job_id, "failed", Some(error)); }
        }
    });
    Ok(snapshot)
}

#[tauri::command]
#[specta::specta]
pub async fn get_available_models(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<ModelInfo>, String> {
    Ok(model_manager.get_available_models())
}

#[tauri::command]
#[specta::specta]
pub fn take_missing_model_selection_notice(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Option<(String, Option<String>)> {
    model_manager.take_missing_selection_notice()
}

#[tauri::command]
#[specta::specta]
pub async fn rescan_local_models(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<(), String> {
    let model_manager = model_manager.inner().clone();
    tokio::task::spawn_blocking(move || model_manager.rescan_local_models())
        .await
        .map_err(|e| format!("rescan task panicked: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn download_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    let result = model_manager
        .download_model(&model_id)
        .await
        .map_err(|e| e.to_string());

    if let Err(ref error) = result {
        let _ = app_handle.emit(
            "model-download-failed",
            serde_json::json!({ "model_id": &model_id, "error": error }),
        );
    }

    result
}

/// Ensures a selectable local model is installed before a workflow uses it.
/// A model already being downloaded elsewhere is awaited instead of starting
/// a competing download of the same files.
pub(crate) async fn ensure_local_model_downloaded_for_use(
    app: &AppHandle,
    model_id: &str,
) -> Result<(), String> {
    let model_manager = app.state::<Arc<ModelManager>>();
    let mut waited_for_existing_download = false;

    loop {
        let model_info = model_manager
            .get_model_info(model_id)
            .ok_or_else(|| format!("Model not found: {model_id}"))?;
        if model_info.is_downloaded {
            return Ok(());
        }
        if model_info.is_downloading {
            waited_for_existing_download = true;
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        }
        if waited_for_existing_download {
            return Err(format!("Model download did not complete: {model_id}"));
        }

        let _ = app.emit(
            "model-download-on-demand",
            serde_json::json!({
                "model_id": model_id,
                "model_name": model_info.name,
            }),
        );
        let result = model_manager
            .download_model(model_id)
            .await
            .map_err(|error| error.to_string());
        if let Err(ref error) = result {
            let _ = app.emit(
                "model-download-failed",
                serde_json::json!({ "model_id": model_id, "error": error }),
            );
        }
        result?;
        return model_manager
            .get_model_info(model_id)
            .filter(|model| model.is_downloaded)
            .map(|_| ())
            .ok_or_else(|| format!("Model download did not complete: {model_id}"));
    }
}

#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    app_handle: AppHandle,
    _model_manager: State<'_, Arc<ModelManager>>,
    _transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    cancel_pending_model_deletion_for_use(&app_handle, &model_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        // Immediate callers cancel an older timer before claiming the same loading lane.
        let loading_guard = claim_model_selection(&app_handle, &model_id)?;
        delete_model_with_loading_claim(&app_handle, &model_id, loading_guard)
    }).await.map_err(|error| format!("Model deletion task failed: {error}"))?
}

fn delete_model_with_loading_claim(
    app_handle: &AppHandle,
    model_id: &str,
    _loading_guard: LoadingGuard,
) -> Result<(), String> {
    let model_manager = app_handle.state::<Arc<ModelManager>>();
    let transcription_manager = app_handle.state::<Arc<TranscriptionManager>>();
    let was_selected = get_settings(app_handle).selected_model == model_id;

    // A custom profile/file override can have a loaded model different from selected_model.
    // Always use the existing safe unload before touching its files, retaining the lane
    // through any rollback so no competing load or switch can replace restoration state.
    let was_loaded = transcription_manager.unload_model_for_deletion(model_id)
        .map_err(|e| format!("Failed to unload model: {}", e))?;

    if was_selected {
        let mut updated_settings = get_settings(app_handle);
        if updated_settings.selected_model != model_id {
            return Err("Active model changed while deletion was starting".to_string());
        }
        updated_settings.selected_model = String::new();
        write_settings(app_handle, updated_settings);
    }

    if let Err(delete_error) = model_manager.delete_model(model_id) {
        let current_settings = get_settings(app_handle);
        if !was_selected || current_settings.selected_model.is_empty() {
            if was_loaded {
                // This operation owns the deleting claim; rollback must bypass the
                // public load path's rejection of other callers during deletion.
                let reload = transcription_manager.restore_model_after_failed_deletion(model_id);
                if let Err(reload_error) = reload {
                    tray::refresh_tray_menu(app_handle, None);
                    return Err(format!(
                        "Failed to delete model: {delete_error}. The model also failed to reload: {reload_error}"
                    ));
                }
            }

            if was_selected {
                let mut restored_settings = get_settings(app_handle);
                if restored_settings.selected_model.is_empty() {
                    restored_settings.selected_model = model_id.to_string();
                    write_settings(app_handle, restored_settings);
                }
            }
        }

        tray::refresh_tray_menu(app_handle, None);
        return Err(delete_error.to_string());
    }

    tray::refresh_tray_menu(app_handle, None);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_model(
    app_handle: AppHandle,
    _model_manager: State<'_, Arc<ModelManager>>,
    _transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    // Cancel on receipt, before a busy blocking pool can postpone the selection worker.
    cancel_pending_model_deletion_for_use(&app_handle, &model_id)?;
    tauri::async_runtime::spawn_blocking(move || switch_active_model(&app_handle, &model_id))
        .await
        .map_err(|error| format!("Failed to switch model: {}", error))?
}

pub fn switch_active_model(app: &AppHandle, model_id: &str) -> Result<(), String> {
    let model_manager = app.state::<Arc<ModelManager>>();
    let transcription_manager = app.state::<Arc<TranscriptionManager>>();

    let _loading_guard = claim_model_selection(app, model_id)?;

    let model_info = model_manager
        .get_model_info(model_id)
        .ok_or_else(|| format!("Model not found: {}", model_id))?;

    let settings = get_settings(app);
    let unload_timeout = settings.model_unload_timeout;
    let previous_model_id = settings.selected_model.clone();
    let previous_provider = settings.transcription_provider;
    let previous_language = settings.selected_language.clone();

    let mut updated_settings = settings;
    updated_settings.selected_model = model_id.to_string();
    updated_settings.transcription_provider = TranscriptionProvider::Local;

    // Reset unsupported language selections so backend engines like Canary
    // never receive stale language codes from the previously active model.
    if updated_settings.selected_language != "auto"
        && !model_info.supported_languages.is_empty()
        && !model_info
            .supported_languages
            .contains(&updated_settings.selected_language)
    {
        log::info!(
            "Resetting language from '{}' to 'auto' (not supported by {})",
            updated_settings.selected_language,
            model_id
        );
        updated_settings.selected_language = "auto".to_string();
    }

    if !model_info.is_downloaded {
        write_settings(app, updated_settings);
        let _ = app.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "selection_changed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );
        tray::refresh_tray_menu(app, None);
        return Ok(());
    }

    if unload_timeout == ModelUnloadTimeout::Immediately {
        if transcription_manager.is_model_loaded() {
            transcription_manager
                .unload_model()
                .map_err(|e| format!("Failed to unload previous model: {}", e))?;
        }

        write_settings(app, updated_settings);

        let _ = app.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "selection_changed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );
        tray::refresh_tray_menu(app, None);
        return Ok(());
    }

    write_settings(app, updated_settings);

    if let Err(err) = transcription_manager.load_model(model_id) {
        let mut reverted_settings = get_settings(app);
        reverted_settings.selected_model = previous_model_id;
        reverted_settings.transcription_provider = previous_provider;
        reverted_settings.selected_language = previous_language;
        write_settings(app, reverted_settings);
        tray::refresh_tray_menu(app, None);
        return Err(err.to_string());
    }

    tray::refresh_tray_menu(app, None);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_current_model(app_handle: AppHandle) -> Result<String, String> {
    let settings = get_settings(&app_handle);
    Ok(settings.selected_model)
}

#[tauri::command]
#[specta::specta]
pub async fn get_transcription_model_status(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<Option<String>, String> {
    Ok(transcription_manager.get_current_model())
}

#[tauri::command]
#[specta::specta]
pub async fn has_any_models_available(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<bool, String> {
    let models = model_manager.get_available_models();
    Ok(models.iter().any(|m| m.is_downloaded))
}

#[derive(Serialize, Type)]
pub struct GpuVramStatus {
    pub is_supported: bool,
    pub adapter_name: Option<String>,
    pub used_bytes: u64,
    pub budget_bytes: u64,
    pub system_used_bytes: u64,
    pub system_free_bytes: u64,
    pub total_vram_bytes: u64,
    pub updated_at_unix_ms: u64,
    pub error: Option<String>,
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(target_os = "windows")]
fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|&ch| ch == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end]).trim().to_string()
}

#[cfg(target_os = "windows")]
fn nt_success(status: windows::Win32::Foundation::NTSTATUS) -> bool {
    status.0 >= 0
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct ActiveGpuVramSnapshot {
    adapter_name: String,
    adapter_luid: windows::Win32::Foundation::LUID,
    process_used_bytes: u64,
    process_budget_bytes: u64,
    total_vram_bytes: u64,
}

#[cfg(target_os = "windows")]
fn query_system_gpu_usage_bytes(adapter_luid: windows::Win32::Foundation::LUID) -> Option<u64> {
    use windows::Wdk::Graphics::Direct3D::{
        D3DKMTCloseAdapter, D3DKMTOpenAdapterFromLuid, D3DKMTQueryVideoMemoryInfo,
        D3DKMT_CLOSEADAPTER, D3DKMT_MEMORY_SEGMENT_GROUP_LOCAL, D3DKMT_OPENADAPTERFROMLUID,
        D3DKMT_QUERYVIDEOMEMORYINFO,
    };
    use windows::Win32::Foundation::HANDLE;

    unsafe {
        let mut open = D3DKMT_OPENADAPTERFROMLUID {
            AdapterLuid: adapter_luid,
            ..Default::default()
        };
        let open_status = D3DKMTOpenAdapterFromLuid(&mut open);
        if !nt_success(open_status) || open.hAdapter == 0 {
            return None;
        }

        let mut query = D3DKMT_QUERYVIDEOMEMORYINFO {
            hProcess: HANDLE(std::ptr::null_mut()),
            hAdapter: open.hAdapter,
            MemorySegmentGroup: D3DKMT_MEMORY_SEGMENT_GROUP_LOCAL,
            PhysicalAdapterIndex: 0,
            ..Default::default()
        };
        let query_status = D3DKMTQueryVideoMemoryInfo(&mut query);

        let _ = D3DKMTCloseAdapter(&D3DKMT_CLOSEADAPTER {
            hAdapter: open.hAdapter,
        });

        if !nt_success(query_status) {
            return None;
        }

        Some(query.CurrentUsage)
    }
}

#[cfg(target_os = "windows")]
fn query_active_gpu_vram() -> Result<ActiveGpuVramSnapshot, String> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIAdapter3, IDXGIFactory6,
        DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
        DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
    };

    unsafe {
        let factory: IDXGIFactory6 = CreateDXGIFactory1::<IDXGIFactory6>()
            .map_err(|e| format!("Failed to create DXGI factory: {e}"))?;

        let mut best: Option<ActiveGpuVramSnapshot> = None;
        let mut adapter_index = 0u32;

        loop {
            let adapter: IDXGIAdapter1 = match factory
                .EnumAdapterByGpuPreference(adapter_index, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE)
            {
                Ok(adapter) => adapter,
                Err(_) => break,
            };
            adapter_index += 1;

            let desc = match adapter.GetDesc1() {
                Ok(desc) => desc,
                Err(_) => continue,
            };

            if (desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32)) != 0 {
                continue;
            }

            let adapter3: IDXGIAdapter3 = match adapter.cast::<IDXGIAdapter3>() {
                Ok(adapter3) => adapter3,
                Err(_) => continue,
            };

            let mut memory_info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            if adapter3
                .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut memory_info)
                .is_err()
            {
                continue;
            }

            let budget_bytes = if memory_info.Budget > 0 {
                memory_info.Budget
            } else {
                desc.DedicatedVideoMemory as u64
            };
            let used_bytes = memory_info.CurrentUsage;
            let total_vram_bytes = if desc.DedicatedVideoMemory > 0 {
                desc.DedicatedVideoMemory as u64
            } else {
                budget_bytes
            };
            let adapter_name = wide_to_string(&desc.Description);
            let adapter_name = if adapter_name.is_empty() {
                format!("GPU {}", adapter_index)
            } else {
                adapter_name
            };

            // Prefer the adapter with the largest total VRAM to avoid
            // startup mis-detection where the UI renderer uses a small iGPU budget.
            // If totals are equal, fall back to process usage/budget.
            let should_replace = match best {
                None => true,
                Some(ref best_snapshot) => {
                    total_vram_bytes > best_snapshot.total_vram_bytes
                        || (total_vram_bytes == best_snapshot.total_vram_bytes
                            && (used_bytes > best_snapshot.process_used_bytes
                                || (used_bytes == best_snapshot.process_used_bytes
                                    && budget_bytes > best_snapshot.process_budget_bytes)))
                }
            };

            if should_replace {
                best = Some(ActiveGpuVramSnapshot {
                    adapter_name,
                    adapter_luid: desc.AdapterLuid,
                    process_used_bytes: used_bytes,
                    process_budget_bytes: budget_bytes,
                    total_vram_bytes,
                });
            }
        }

        best.ok_or_else(|| "No active hardware GPU adapter detected".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_active_gpu_vram_status() -> Result<GpuVramStatus, String> {
    let updated_at_unix_ms = unix_ms_now();

    #[cfg(target_os = "windows")]
    {
        match query_active_gpu_vram() {
            Ok(snapshot) => {
                let system_used_bytes = query_system_gpu_usage_bytes(snapshot.adapter_luid)
                    .unwrap_or(snapshot.process_used_bytes);
                let system_free_bytes = snapshot.total_vram_bytes.saturating_sub(system_used_bytes);

                Ok(GpuVramStatus {
                    is_supported: true,
                    adapter_name: Some(snapshot.adapter_name),
                    used_bytes: snapshot.process_used_bytes,
                    budget_bytes: snapshot.process_budget_bytes,
                    system_used_bytes,
                    system_free_bytes,
                    total_vram_bytes: snapshot.total_vram_bytes,
                    updated_at_unix_ms,
                    error: None,
                })
            }
            Err(error) => Ok(GpuVramStatus {
                is_supported: false,
                adapter_name: None,
                used_bytes: 0,
                budget_bytes: 0,
                system_used_bytes: 0,
                system_free_bytes: 0,
                total_vram_bytes: 0,
                updated_at_unix_ms,
                error: Some(error),
            }),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(GpuVramStatus {
            is_supported: false,
            adapter_name: None,
            used_bytes: 0,
            budget_bytes: 0,
            system_used_bytes: 0,
            system_free_bytes: 0,
            total_vram_bytes: 0,
            updated_at_unix_ms,
            error: Some("VRAM meter is only available on Windows".to_string()),
        })
    }
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_download(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .cancel_download(&model_id)
        .map_err(|e| e.to_string())
}
