use crate::audio_toolkit::audio::list_input_devices;
use crate::managers::audio::AudioRecordingManager;
use crate::settings::{get_settings, write_settings};
use log::warn;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const AUDIO_INPUT_STATE_CHANGED_EVENT: &str = "audio-input-state-changed";

pub struct ManagedManualMicrophoneSelection(pub Mutex<Option<String>>);

impl ManagedManualMicrophoneSelection {
    pub fn new(initial_selection: Option<String>) -> Self {
        Self(Mutex::new(initial_selection))
    }
}

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
    device_names: &[String],
    pattern: &str,
) -> Option<String> {
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

pub fn remember_manual_microphone_selection(app: &AppHandle, selection: Option<String>) {
    let Some(last_manual_selection) = app.try_state::<ManagedManualMicrophoneSelection>() else {
        return;
    };

    let lock_result = last_manual_selection.0.lock();
    match lock_result {
        Ok(mut last_manual_selection) => {
            *last_manual_selection = selection;
        }
        Err(err) => {
            warn!(
                "Failed to lock manual microphone selection state while updating fallback: {}",
                err
            );
        }
    }
}

fn last_manual_microphone_selection(app: &AppHandle) -> Option<String> {
    let Some(last_manual_selection) = app.try_state::<ManagedManualMicrophoneSelection>() else {
        return get_settings(app).selected_microphone;
    };

    let selection = match last_manual_selection.0.lock() {
        Ok(last_manual_selection) => last_manual_selection.clone(),
        Err(err) => {
            warn!(
                "Failed to lock manual microphone selection state while reading fallback: {}",
                err
            );
            get_settings(app).selected_microphone
        }
    };

    selection
}

pub fn emit_audio_input_state_changed(app: &AppHandle) {
    crate::tray::refresh_tray_menu(app, None);
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
    let manual_fallback = last_manual_microphone_selection(app).filter(|name| {
        device_names.iter().any(|device| device == name)
            && current_selected.as_deref() != Some(name.as_str())
    });

    let target_selected = if auto_enabled && !mask.is_empty() {
        if current_exists {
            current_selected.clone()
        } else if let Some(matched_name) = select_matching_microphone(&device_names, &mask) {
            Some(matched_name)
        } else if let Some(manual_fallback) = manual_fallback {
            Some(manual_fallback)
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

pub fn reconcile_selected_microphone_before_recording(app: &AppHandle) -> Result<(), String> {
    let selection_changed = reconcile_selected_microphone(app, true)?;

    if selection_changed {
        emit_audio_input_state_changed(app);
    }

    Ok(())
}
