use crate::managers::model::{ModelInfo, ModelManager};
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};

#[tauri::command]
#[specta::specta]
pub async fn get_available_models(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<ModelInfo>, String> {
    Ok(model_manager.get_available_models())
}

#[tauri::command]
#[specta::specta]
pub async fn get_model_info(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<Option<ModelInfo>, String> {
    Ok(model_manager.get_model_info(&model_id))
}

#[tauri::command]
#[specta::specta]
pub async fn download_model(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .download_model(&model_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    // If deleting the active model, unload it and clear the selection.
    let settings = get_settings(&app_handle);
    if settings.selected_model == model_id {
        transcription_manager
            .unload_model()
            .map_err(|e| format!("Failed to unload model: {}", e))?;

        let mut updated_settings = get_settings(&app_handle);
        updated_settings.selected_model = String::new();
        write_settings(&app_handle, updated_settings);
    }

    model_manager
        .delete_model(&model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    // Check if model exists and is available
    let model_info = model_manager
        .get_model_info(&model_id)
        .ok_or_else(|| format!("Model not found: {}", model_id))?;

    if !model_info.is_downloaded {
        return Err(format!("Model not downloaded: {}", model_id));
    }

    // Load the model in the transcription manager
    transcription_manager
        .load_model(&model_id)
        .map_err(|e| e.to_string())?;

    // Update settings
    let mut settings = get_settings(&app_handle);
    settings.selected_model = model_id.clone();
    write_settings(&app_handle, settings);

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

#[tauri::command]
#[specta::specta]
pub async fn get_recommended_first_model(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<String, String> {
    // Deprecated compatibility command: derive recommendation from model metadata.
    // If no model is explicitly marked recommended, keep legacy fallback.
    let models = model_manager.get_available_models();
    if let Some(model) = models.iter().find(|m| m.is_recommended) {
        return Ok(model.id.clone());
    }
    Ok("parakeet-tdt-0.6b-v3".to_string())
}
