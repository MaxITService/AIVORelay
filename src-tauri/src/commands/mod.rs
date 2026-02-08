pub mod audio;
pub mod connector;
pub mod file_transcription;
pub mod history;
pub mod key_listener;
pub mod models;
pub mod region_capture;
pub mod remote_stt;
pub mod transcription;
pub mod voice_activation_button;
pub mod voice_command;

use crate::settings::{get_settings, write_settings, AppSettings, LlmFeature, LogLevel};
use crate::utils::cancel_current_operation;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
#[specta::specta]
pub fn cancel_operation(app: AppHandle) {
    cancel_current_operation(&app);
}

#[tauri::command]
#[specta::specta]
pub fn get_app_dir_path(app: AppHandle) -> Result<String, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    Ok(app_data_dir.to_string_lossy().to_string())
}

#[tauri::command]
#[specta::specta]
pub fn get_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    Ok(get_settings(&app))
}

#[tauri::command]
#[specta::specta]
pub fn get_default_settings() -> Result<AppSettings, String> {
    Ok(crate::settings::get_default_settings())
}

#[tauri::command]
#[specta::specta]
pub fn get_log_dir_path(app: AppHandle) -> Result<String, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to get log directory: {}", e))?;

    Ok(log_dir.to_string_lossy().to_string())
}

#[specta::specta]
#[tauri::command]
pub fn set_log_level(app: AppHandle, level: LogLevel) -> Result<(), String> {
    let tauri_log_level: tauri_plugin_log::LogLevel = level.into();
    let log_level: log::Level = tauri_log_level.into();
    // Update the file log level atomic so the filter picks up the new level
    crate::FILE_LOG_LEVEL.store(
        log_level.to_level_filter() as u8,
        std::sync::atomic::Ordering::Relaxed,
    );

    let mut settings = get_settings(&app);
    settings.log_level = level;
    write_settings(&app, settings);

    Ok(())
}

#[specta::specta]
#[tauri::command]
pub fn open_recordings_folder(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    let recordings_dir = app_data_dir.join("recordings");

    let path = recordings_dir.to_string_lossy().as_ref().to_string();
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|e| format!("Failed to open recordings folder: {}", e))?;

    Ok(())
}

#[specta::specta]
#[tauri::command]
pub fn open_log_dir(app: AppHandle) -> Result<(), String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to get log directory: {}", e))?;

    let path = log_dir.to_string_lossy().as_ref().to_string();
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|e| format!("Failed to open log directory: {}", e))?;

    Ok(())
}

#[specta::specta]
#[tauri::command]
pub fn open_app_data_dir(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    let path = app_data_dir.to_string_lossy().as_ref().to_string();
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|e| format!("Failed to open app data directory: {}", e))?;

    Ok(())
}

/// Check if Apple Intelligence is available on this device.
/// Called by the frontend when the user selects Apple Intelligence provider.
#[specta::specta]
#[tauri::command]
pub fn check_apple_intelligence_available() -> bool {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        crate::apple_intelligence::check_apple_intelligence_availability()
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        false
    }
}

/// Returns whether a feature-specific LLM API key is stored in secure storage.
/// On non-Windows platforms this always returns false.
#[specta::specta]
#[tauri::command]
pub fn llm_has_stored_api_key(
    app: AppHandle,
    feature: LlmFeature,
    provider_id: String,
) -> Result<bool, String> {
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        return Ok(false);
    }

    #[cfg(target_os = "windows")]
    {
        let settings = get_settings(&app);
        let has_key = match feature {
            LlmFeature::PostProcessing => {
                !crate::secure_keys::get_post_process_api_key(provider_id)
                    .trim()
                    .is_empty()
            }
            LlmFeature::AiReplace => {
                let use_post_process_key =
                    settings.ai_replace_provider_id.as_deref() != Some(provider_id);

                if use_post_process_key {
                    !crate::secure_keys::get_post_process_api_key(provider_id)
                        .trim()
                        .is_empty()
                } else {
                    !crate::secure_keys::get_ai_replace_api_key(provider_id)
                        .trim()
                        .is_empty()
                }
            }
            LlmFeature::VoiceCommand => {
                let use_post_process_key =
                    settings.voice_command_provider_id.as_deref() != Some(provider_id);

                if use_post_process_key {
                    !crate::secure_keys::get_post_process_api_key(provider_id)
                        .trim()
                        .is_empty()
                } else {
                    crate::secure_keys::get_voice_command_api_key(provider_id).is_some()
                }
            }
        };
        Ok(has_key)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, feature);
        Ok(false)
    }
}
