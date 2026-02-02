use tauri::{AppHandle, Manager};

use crate::managers::key_listener::{KeyListenerState, ModifierState};

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

/// Check if the key listener is running
#[tauri::command]
#[specta::specta]
pub async fn key_listener_is_running(app_handle: AppHandle) -> Result<bool, String> {
    let key_listener_state = app_handle.try_state::<KeyListenerState>().ok_or_else(|| {
        "Key listener state not found. App may not be initialized properly.".to_string()
    })?;

    Ok(key_listener_state.manager.is_running().await)
}

/// Get current modifier state (Ctrl, Shift, Alt, Win)
#[tauri::command]
#[specta::specta]
pub async fn key_listener_get_modifiers(app_handle: AppHandle) -> Result<ModifierState, String> {
    let key_listener_state = app_handle.try_state::<KeyListenerState>().ok_or_else(|| {
        "Key listener state not found. App may not be initialized properly.".to_string()
    })?;

    Ok(key_listener_state.manager.get_modifier_state().await)
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

/// Check if a shortcut is registered with rdev
#[tauri::command]
#[specta::specta]
pub async fn key_listener_is_shortcut_registered(
    app_handle: AppHandle,
    id: String,
) -> Result<bool, String> {
    let key_listener_state = app_handle.try_state::<KeyListenerState>().ok_or_else(|| {
        "Key listener state not found. App may not be initialized properly.".to_string()
    })?;

    Ok(key_listener_state.manager.is_shortcut_registered(&id).await)
}

/// Get list of all registered rdev shortcuts
#[tauri::command]
#[specta::specta]
pub async fn key_listener_get_registered_shortcuts(
    app_handle: AppHandle,
) -> Result<Vec<String>, String> {
    let key_listener_state = app_handle.try_state::<KeyListenerState>().ok_or_else(|| {
        "Key listener state not found. App may not be initialized properly.".to_string()
    })?;

    Ok(key_listener_state.manager.get_registered_shortcuts().await)
}
