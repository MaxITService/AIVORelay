use crate::actions::ACTION_MAP;
use crate::settings::get_settings;
use crate::ManagedToggleState;
use log::info;
use tauri::{AppHandle, Manager};

fn active_profile_push_to_talk(app: &AppHandle) -> bool {
    let settings = get_settings(app);
    if settings.active_profile_id == "default" {
        settings.push_to_talk
    } else {
        settings
            .transcription_profile(&settings.active_profile_id)
            .map(|p| p.push_to_talk)
            .unwrap_or(settings.push_to_talk)
    }
}

#[tauri::command]
#[specta::specta]
pub async fn spawn_voice_activation_button_window(app: AppHandle) -> Result<(), String> {
    info!("spawn_voice_activation_button_window invoked");
    crate::overlay::show_voice_activation_button_window(&app)
}

#[tauri::command]
#[specta::specta]
pub fn voice_activation_button_get_push_to_talk(app: AppHandle) -> Result<bool, String> {
    Ok(active_profile_push_to_talk(&app))
}

#[tauri::command]
#[specta::specta]
pub fn voice_activation_button_get_show_aot_toggle(app: AppHandle) -> Result<bool, String> {
    Ok(get_settings(&app).voice_button_show_aot_toggle)
}

#[tauri::command]
#[specta::specta]
pub fn voice_activation_button_get_single_click_close(app: AppHandle) -> Result<bool, String> {
    Ok(get_settings(&app).voice_button_single_click_close)
}

#[tauri::command]
#[specta::specta]
pub fn voice_activation_button_press(app: AppHandle) -> Result<(), String> {
    let action = ACTION_MAP
        .get("transcribe")
        .ok_or_else(|| "Transcribe action is not available".to_string())?;

    let use_push_to_talk = active_profile_push_to_talk(&app);
    let shortcut_str = "voice_activation_button";

    if use_push_to_talk {
        action.start(&app, "transcribe", shortcut_str);
        return Ok(());
    }

    let should_start: bool;
    {
        let toggle_state_manager = app.state::<ManagedToggleState>();
        let mut states = toggle_state_manager
            .lock()
            .map_err(|_| "Failed to lock toggle state manager".to_string())?;
        let is_currently_active = states
            .active_toggles
            .entry("transcribe".to_string())
            .or_insert(false);
        should_start = !*is_currently_active;
        *is_currently_active = should_start;
    }

    if should_start {
        action.start(&app, "transcribe", shortcut_str);
    } else {
        action.stop(&app, "transcribe", shortcut_str);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn voice_activation_button_release(app: AppHandle) -> Result<(), String> {
    let use_push_to_talk = active_profile_push_to_talk(&app);
    if !use_push_to_talk {
        return Ok(());
    }

    let action = ACTION_MAP
        .get("transcribe")
        .ok_or_else(|| "Transcribe action is not available".to_string())?;
    action.stop(&app, "transcribe", "voice_activation_button");
    Ok(())
}
