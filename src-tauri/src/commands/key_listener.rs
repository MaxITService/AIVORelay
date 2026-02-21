use tauri::{AppHandle, Manager};

use crate::managers::key_listener::KeyListenerState;

/// Start the key listener
#[tauri::command]
#[specta::specta]
pub async fn key_listener_start(app_handle: AppHandle) -> Result<(), String> {
    let key_listener_state = app_handle.try_state::<KeyListenerState>().ok_or_else(|| {
        "Key listener state not found. App may not be initialized properly.".to_string()
    })?;

    key_listener_state.manager.start().await
}

/// Stop the key listener
#[tauri::command]
#[specta::specta]
pub async fn key_listener_stop(app_handle: AppHandle) -> Result<(), String> {
    let key_listener_state = app_handle.try_state::<KeyListenerState>().ok_or_else(|| {
        "Key listener state not found. App may not be initialized properly.".to_string()
    })?;

    key_listener_state.manager.stop().await
}

/// Register a shortcut with the rdev key listener
/// This allows shortcuts that tauri-plugin-global-shortcut doesn't support (like Caps Lock)
#[tauri::command]
#[specta::specta]
pub async fn key_listener_register_shortcut(
    app_handle: AppHandle,
    id: String,
    binding: String,
) -> Result<(), String> {
    let key_listener_state = app_handle.try_state::<KeyListenerState>().ok_or_else(|| {
        "Key listener state not found. App may not be initialized properly.".to_string()
    })?;

    key_listener_state
        .manager
        .register_shortcut(id, binding)
        .await
}

/// Unregister a shortcut from the rdev key listener
#[tauri::command]
#[specta::specta]
pub async fn key_listener_unregister_shortcut(
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let key_listener_state = app_handle.try_state::<KeyListenerState>().ok_or_else(|| {
        "Key listener state not found. App may not be initialized properly.".to_string()
    })?;

    key_listener_state.manager.unregister_shortcut(&id).await
}

