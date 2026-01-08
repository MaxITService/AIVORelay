use crate::managers::remote_stt::{
    clear_remote_stt_api_key, has_remote_stt_api_key, set_remote_stt_api_key, RemoteSttManager,
};
use crate::settings::get_settings;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
#[specta::specta]
pub fn remote_stt_has_api_key() -> Result<bool, String> {
    Ok(has_remote_stt_api_key())
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_set_api_key(api_key: String) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    set_remote_stt_api_key(&api_key).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_clear_api_key() -> Result<(), String> {
    clear_remote_stt_api_key().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_get_debug_dump(
    remote_manager: State<'_, Arc<RemoteSttManager>>,
) -> Result<Vec<String>, String> {
    Ok(remote_manager.get_debug_dump())
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_clear_debug(
    remote_manager: State<'_, Arc<RemoteSttManager>>,
) -> Result<(), String> {
    remote_manager.clear_debug();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn remote_stt_test_connection(
    app: AppHandle,
    base_url: String,
    remote_manager: State<'_, Arc<RemoteSttManager>>,
) -> Result<(), String> {
    let settings = get_settings(&app);
    remote_manager
        .test_connection(&settings.remote_stt, &base_url)
        .await
        .map_err(|e| e.to_string())
}

/// Returns the character limit for the system prompt based on the currently selected Remote STT model.
/// Returns None if the model is unknown (no enforced limit).
#[tauri::command]
#[specta::specta]
pub fn remote_stt_get_prompt_limit(app: AppHandle) -> Option<usize> {
    let settings = get_settings(&app);
    crate::managers::remote_stt::get_model_prompt_limit(&settings.remote_stt.model_id)
}
