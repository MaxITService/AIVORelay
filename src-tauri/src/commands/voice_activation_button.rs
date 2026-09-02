use crate::actions::ACTION_MAP;
use crate::settings::{get_settings, AppSettings};
use crate::ManagedToggleState;
use log::info;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

static PUSH_TO_TALK_PRESS_ACTIVE: AtomicBool = AtomicBool::new(false);

fn active_profile_push_to_talk(settings: &AppSettings) -> bool {
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
    Ok(active_profile_push_to_talk(&get_settings(&app)))
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

    let shortcut_str = "voice_activation_button";

    // Auto marks its recording active on physical keydown. If the Voice
    // Activation Button is pressed during that interval, it is an independent
    // control and must stop the recording instead of attempting a second start.
    {
        let toggle_state_manager = app.state::<ManagedToggleState>();
        let mut states = toggle_state_manager
            .lock()
            .map_err(|_| "Failed to lock toggle state manager".to_string())?;
        if states
            .active_toggles
            .get("transcribe")
            .copied()
            .unwrap_or(false)
        {
            states.active_toggles.insert("transcribe".to_string(), false);
            states.active_presses.remove("transcribe");
            drop(states);
            PUSH_TO_TALK_PRESS_ACTIVE.store(false, Ordering::Release);
            action.stop(&app, "transcribe", shortcut_str);
            return Ok(());
        }
    }

    // Clicking this AivoRelay-owned window makes it the foreground window, so
    // application-aware matching cannot reliably identify the user's target.
    // Use one manually selected profile snapshot for both interaction mode and
    // the recording session; TranscribeAction consumes this pending snapshot.
    let settings = crate::actions::prepare_manual_transcribe_settings(&app, "transcribe");
    let use_push_to_talk = active_profile_push_to_talk(&settings);

    if use_push_to_talk {
        PUSH_TO_TALK_PRESS_ACTIVE.store(true, Ordering::Release);
        action.start(&app, "transcribe", shortcut_str);
        return Ok(());
    }

    PUSH_TO_TALK_PRESS_ACTIVE.store(false, Ordering::Release);

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
        if !should_start {
            states.active_presses.remove("transcribe");
        }
    }

    if should_start {
        action.start(&app, "transcribe", shortcut_str);
    } else {
        let _ = crate::actions::take_pending_transcribe_settings("transcribe");
        action.stop(&app, "transcribe", shortcut_str);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn voice_activation_button_release(app: AppHandle) -> Result<(), String> {
    // Release must follow the mode captured on press even if the user changes
    // the active profile or its Push-to-Talk setting while recording.
    if !PUSH_TO_TALK_PRESS_ACTIVE.swap(false, Ordering::AcqRel) {
        return Ok(());
    }

    let action = ACTION_MAP
        .get("transcribe")
        .ok_or_else(|| "Transcribe action is not available".to_string())?;
    action.stop(&app, "transcribe", "voice_activation_button");
    Ok(())
}
