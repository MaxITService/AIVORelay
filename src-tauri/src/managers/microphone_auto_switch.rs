use crate::audio_toolkit::audio::list_input_devices;
use crate::managers::audio::AudioRecordingManager;
use crate::settings::{get_settings, write_settings};
use log::{debug, warn};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const AUDIO_INPUT_STATE_CHANGED_EVENT: &str = "audio-input-state-changed";
const WATCH_INTERVAL: Duration = Duration::from_secs(2);

fn load_input_device_names() -> Result<Vec<String>, String> {
    let mut names = list_input_devices()
        .map_err(|err| format!("Failed to list audio devices: {}", err))?
        .into_iter()
        .map(|device| device.name)
        .collect::<Vec<_>>();
    names.sort_unstable();
    Ok(names)
}

fn wildcard_match(pattern: &str, candidate: &str) -> bool {
    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    let candidate_chars = candidate.chars().collect::<Vec<_>>();

    let mut pattern_index = 0usize;
    let mut candidate_index = 0usize;
    let mut star_index: Option<usize> = None;
    let mut match_index = 0usize;

    while candidate_index < candidate_chars.len() {
        if pattern_index < pattern_chars.len()
            && (pattern_chars[pattern_index] == '?'
                || pattern_chars[pattern_index] == candidate_chars[candidate_index])
        {
            pattern_index += 1;
            candidate_index += 1;
        } else if pattern_index < pattern_chars.len() && pattern_chars[pattern_index] == '*' {
            star_index = Some(pattern_index);
            match_index = candidate_index;
            pattern_index += 1;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            match_index += 1;
            candidate_index = match_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern_chars.len() && pattern_chars[pattern_index] == '*' {
        pattern_index += 1;
    }

    pattern_index == pattern_chars.len()
}

fn matches_name_mask(device_name: &str, pattern: &str) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return false;
    }

    let normalized_pattern = trimmed.to_lowercase();
    let normalized_name = device_name.to_lowercase();

    if normalized_pattern.contains('*') || normalized_pattern.contains('?') {
        wildcard_match(&normalized_pattern, &normalized_name)
    } else {
        normalized_name.contains(&normalized_pattern)
    }
}

fn select_matching_microphone(
    current_selected: Option<&str>,
    device_names: &[String],
    pattern: &str,
) -> Option<String> {
    if let Some(current_name) = current_selected {
        if device_names.iter().any(|name| name == current_name)
            && matches_name_mask(current_name, pattern)
        {
            return Some(current_name.to_string());
        }
    }

    device_names
        .iter()
        .find(|name| matches_name_mask(name, pattern))
        .cloned()
}

fn refresh_active_microphone_stream(app: &AppHandle) {
    let recording_manager = app.state::<Arc<AudioRecordingManager>>();
    if let Err(err) = recording_manager.update_selected_device() {
        warn!("Failed to refresh active microphone stream after device change: {}", err);
    }
}

pub fn emit_audio_input_state_changed(app: &AppHandle) {
    let _ = app.emit(AUDIO_INPUT_STATE_CHANGED_EVENT, ());
}

pub fn reconcile_selected_microphone(app: &AppHandle, show_overlay: bool) -> Result<bool, String> {
    let device_names = load_input_device_names()?;
    let mut settings = get_settings(app);
    let current_selected = settings.selected_microphone.clone();
    let auto_enabled = settings.selected_microphone_auto_switch_enabled;
    let mask = settings.selected_microphone_name_pattern.trim().to_string();
    let current_exists = current_selected
        .as_ref()
        .map(|name| device_names.iter().any(|device| device == name))
        .unwrap_or(true);

    let target_selected = if auto_enabled && !mask.is_empty() {
        if let Some(matched_name) =
            select_matching_microphone(current_selected.as_deref(), &device_names, &mask)
        {
            Some(matched_name)
        } else if current_exists {
            current_selected.clone()
        } else {
            None
        }
    } else {
        current_selected.clone()
    };

    if target_selected == current_selected {
        return Ok(false);
    }

    settings.selected_microphone = target_selected.clone();
    write_settings(app, settings);
    refresh_active_microphone_stream(app);

    if show_overlay {
        crate::overlay::show_microphone_switch_overlay(
            app,
            target_selected.as_deref().unwrap_or("Default"),
        );
    }

    Ok(true)
}

pub fn start_microphone_auto_switch_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_signature = match load_input_device_names() {
            Ok(device_names) => {
                if let Err(err) = reconcile_selected_microphone(&app, false) {
                    warn!("Initial microphone auto-switch reconcile failed: {}", err);
                }
                device_names.join("\n")
            }
            Err(err) => {
                warn!("Initial microphone device scan failed: {}", err);
                String::new()
            }
        };

        loop {
            std::thread::sleep(WATCH_INTERVAL);

            let device_names = match load_input_device_names() {
                Ok(device_names) => device_names,
                Err(err) => {
                    warn!("Microphone device scan failed: {}", err);
                    continue;
                }
            };

            let signature = device_names.join("\n");
            if signature == last_signature {
                continue;
            }

            last_signature = signature;
            let selection_changed = match reconcile_selected_microphone(&app, true) {
                Ok(changed) => changed,
                Err(err) => {
                    warn!("Microphone auto-switch reconcile failed: {}", err);
                    false
                }
            };

            if !selection_changed {
                refresh_active_microphone_stream(&app);
            }

            emit_audio_input_state_changed(&app);
            debug!("Audio input device state changed");
        }
    });
}
