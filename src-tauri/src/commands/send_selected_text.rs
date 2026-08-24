use crate::managers::send_selected_text_history::{
    SendSelectedTextHistoryEntry, SendSelectedTextHistoryManager,
};
use crate::send_selected_text::SendSelectedTextOperationResult;
use crate::settings::{
    build_send_selected_text_binding, get_settings, send_selected_text_binding_id,
    write_settings_checked, SendSelectedTextPreset, SendSelectedTextSettings,
};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, State};

static PRESET_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[tauri::command]
#[specta::specta]
pub fn get_send_selected_text_settings(app: AppHandle) -> SendSelectedTextSettings {
    get_settings(&app).send_selected_text
}

#[tauri::command]
#[specta::specta]
pub fn create_send_selected_text_preset(
    app: AppHandle,
    template: Option<SendSelectedTextPreset>,
) -> Result<SendSelectedTextPreset, String> {
    let _settings_guard = lock_settings_mutation()?;
    let mut settings = get_settings(&app);
    let mut preset = template.unwrap_or_default();
    preset.id = new_preset_id();
    normalize_preset(&mut preset)?;
    let binding = build_send_selected_text_binding(&preset.id, &preset.name, String::new());
    settings.bindings.insert(binding.id.clone(), binding);
    settings.send_selected_text.presets.push(preset.clone());
    write_settings_checked(&app, settings)?;
    Ok(preset)
}

#[tauri::command]
#[specta::specta]
pub fn update_send_selected_text_preset(
    app: AppHandle,
    mut preset: SendSelectedTextPreset,
) -> Result<SendSelectedTextPreset, String> {
    let _settings_guard = lock_settings_mutation()?;
    normalize_preset(&mut preset)?;
    let mut settings = get_settings(&app);
    let index = settings
        .send_selected_text
        .presets
        .iter()
        .position(|candidate| candidate.id == preset.id)
        .ok_or_else(|| format!("Send Selected Text preset '{}' was not found", preset.id))?;
    let previous_preset = settings.send_selected_text.presets[index].clone();
    let binding_id = send_selected_text_binding_id(&preset.id);
    let previous_binding = settings
        .bindings
        .get(&binding_id)
        .cloned()
        .unwrap_or_else(|| {
            build_send_selected_text_binding(&preset.id, &previous_preset.name, String::new())
        });

    if !previous_binding.current_binding.trim().is_empty() {
        let _ = crate::shortcut::unregister_shortcut(&app, previous_binding.clone());
    }

    let updated_binding = build_send_selected_text_binding(
        &preset.id,
        &preset.name,
        previous_binding.current_binding.clone(),
    );
    settings.send_selected_text.presets[index] = preset.clone();
    settings
        .bindings
        .insert(binding_id.clone(), updated_binding.clone());

    if preset.enabled && !updated_binding.current_binding.trim().is_empty() {
        if let Err(error) = crate::shortcut::register_shortcut(&app, updated_binding.clone()) {
            settings.send_selected_text.presets[index] = previous_preset.clone();
            settings
                .bindings
                .insert(binding_id, previous_binding.clone());
            if previous_preset.enabled && !previous_binding.current_binding.trim().is_empty() {
                let _ = crate::shortcut::register_shortcut(&app, previous_binding);
            }
            return Err(format!("Failed to enable preset shortcut: {error}"));
        }
    }

    if let Err(error) = write_settings_checked(&app, settings) {
        if preset.enabled && !updated_binding.current_binding.trim().is_empty() {
            let _ = crate::shortcut::unregister_shortcut(&app, updated_binding);
        }
        if previous_preset.enabled && !previous_binding.current_binding.trim().is_empty() {
            let _ = crate::shortcut::register_shortcut(&app, previous_binding);
        }
        return Err(error);
    }
    Ok(preset)
}

#[tauri::command]
#[specta::specta]
pub fn delete_send_selected_text_preset(app: AppHandle, preset_id: String) -> Result<(), String> {
    let _settings_guard = lock_settings_mutation()?;
    let mut settings = get_settings(&app);
    let index = settings
        .send_selected_text
        .presets
        .iter()
        .position(|preset| preset.id == preset_id)
        .ok_or_else(|| format!("Send Selected Text preset '{preset_id}' was not found"))?;
    let removed_preset = settings.send_selected_text.presets[index].clone();
    let binding_id = send_selected_text_binding_id(&preset_id);
    let removed_binding = settings.bindings.remove(&binding_id);
    if let Some(binding) = &removed_binding {
        if !binding.current_binding.trim().is_empty() {
            let _ = crate::shortcut::unregister_shortcut(&app, binding.clone());
        }
    }
    settings.send_selected_text.presets.remove(index);
    if let Err(error) = write_settings_checked(&app, settings) {
        if removed_preset.enabled {
            if let Some(binding) = removed_binding {
                if !binding.current_binding.trim().is_empty() {
                    let _ = crate::shortcut::register_shortcut(&app, binding);
                }
            }
        }
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_send_selected_text_options(
    app: AppHandle,
    history: State<'_, Arc<SendSelectedTextHistoryManager>>,
    history_limit: u32,
    error_overlay_auto_hide_ms: u64,
) -> Result<SendSelectedTextSettings, String> {
    let _settings_guard = lock_settings_mutation()?;
    let mut settings = get_settings(&app);
    settings.send_selected_text.history_limit = history_limit.clamp(1, 5_000);
    settings.send_selected_text.error_overlay_auto_hide_ms =
        error_overlay_auto_hide_ms.clamp(1_000, 100_000);
    let result = settings.send_selected_text.clone();
    write_settings_checked(&app, settings)?;
    history
        .enforce_limit(result.history_limit)
        .map_err(|error| format!("Failed to apply history limit: {error}"))?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn run_send_selected_text_preset(
    app: AppHandle,
    preset_id: String,
    sample_text: Option<String>,
) -> Result<SendSelectedTextOperationResult, String> {
    crate::send_selected_text::run_preset(app, preset_id, sample_text).await
}

#[tauri::command]
#[specta::specta]
pub fn trim_send_selected_text_json(app: AppHandle, preset_id: String) -> Result<usize, String> {
    crate::send_selected_text::trim_json_for_preset(&app, &preset_id)
}

#[tauri::command]
#[specta::specta]
pub fn get_send_selected_text_history(
    history: State<'_, Arc<SendSelectedTextHistoryManager>>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<SendSelectedTextHistoryEntry>, String> {
    history
        .list(limit.unwrap_or(100), offset.unwrap_or(0))
        .map_err(|error| format!("Failed to load Send Selected Text history: {error}"))
}

#[tauri::command]
#[specta::specta]
pub fn delete_send_selected_text_history_entry(
    history: State<'_, Arc<SendSelectedTextHistoryManager>>,
    id: i64,
) -> Result<bool, String> {
    history
        .delete(id)
        .map_err(|error| format!("Failed to delete Send Selected Text history entry: {error}"))
}

#[tauri::command]
#[specta::specta]
pub fn clear_send_selected_text_history(
    history: State<'_, Arc<SendSelectedTextHistoryManager>>,
) -> Result<usize, String> {
    history
        .clear()
        .map_err(|error| format!("Failed to clear Send Selected Text history: {error}"))
}

fn normalize_preset(preset: &mut SendSelectedTextPreset) -> Result<(), String> {
    preset.id = preset.id.trim().to_string();
    preset.name = preset.name.trim().to_string();
    preset.destination_directory = preset.destination_directory.trim().to_string();
    preset.filename_template = preset.filename_template.trim().to_string();
    preset.command_working_directory = preset.command_working_directory.trim().to_string();
    preset.max_chars = preset.max_chars.clamp(1, 2_000_000);
    preset.json_keep_last = preset.json_keep_last.min(100_000);
    if preset.name.is_empty() {
        return Err("Preset name is required.".to_string());
    }
    if !preset.destination_directory.is_empty()
        && !Path::new(&preset.destination_directory).is_absolute()
    {
        return Err("Output folder must be an absolute path.".to_string());
    }
    if preset.filename_template.is_empty() {
        return Err("Filename template is required.".to_string());
    }
    Ok(())
}

fn lock_settings_mutation() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    crate::settings::lock_settings_mutation("updating Send Selected Text settings")
}

fn new_preset_id() -> String {
    let sequence = PRESET_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "preset_{}_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        std::process::id(),
        sequence
    )
}
