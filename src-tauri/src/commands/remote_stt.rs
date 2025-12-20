use crate::managers::remote_stt::{
    clear_remote_stt_api_key, has_remote_stt_api_key, set_remote_stt_api_key, RemoteSttManager,
};
use std::sync::Arc;
use tauri::State;

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
