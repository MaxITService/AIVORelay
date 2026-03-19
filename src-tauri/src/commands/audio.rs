use crate::audio_feedback;
use crate::audio_toolkit::audio::{list_input_devices, list_output_devices};
use crate::managers::audio::{AudioRecordingManager, MicrophoneMode};
use crate::managers::microphone_auto_switch;
use crate::settings::{
    get_settings, microphone_input_boost_device_key, sanitize_microphone_input_boost_db,
    write_settings, LiveSoundCaptureSource,
};
use log::warn;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::process::Command;

#[cfg(target_os = "windows")]
use winreg::{
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
    RegKey, HKEY,
};

#[derive(Serialize, Type)]
pub struct CustomSounds {
    start: bool,
    stop: bool,
}

fn custom_sound_exists(app: &AppHandle, sound_type: &str) -> bool {
    crate::portable::resolve_app_data(app, &format!("custom_{}.wav", sound_type))
        .map_or(false, |path| path.exists())
}

#[tauri::command]
#[specta::specta]
pub fn check_custom_sounds(app: AppHandle) -> CustomSounds {
    CustomSounds {
        start: custom_sound_exists(&app, "start"),
        stop: custom_sound_exists(&app, "stop"),
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AudioDevice {
    pub index: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAccess {
    Allowed,
    Denied,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct WindowsMicrophonePermissionStatus {
    pub supported: bool,
    pub overall_access: PermissionAccess,
    pub device_access: PermissionAccess,
    pub app_access: PermissionAccess,
    pub desktop_app_access: PermissionAccess,
}

#[cfg(target_os = "windows")]
fn read_registry_permission_access(root_hkey: HKEY, path: &str) -> PermissionAccess {
    let root = RegKey::predef(root_hkey);
    let Ok(key) = root.open_subkey(path) else {
        return PermissionAccess::Unknown;
    };

    let Ok(value) = key.get_value::<String, _>("Value") else {
        return PermissionAccess::Unknown;
    };

    match value.to_ascii_lowercase().as_str() {
        "allow" => PermissionAccess::Allowed,
        "deny" => PermissionAccess::Denied,
        _ => PermissionAccess::Unknown,
    }
}

#[cfg(target_os = "windows")]
fn get_windows_microphone_permission_status_impl() -> WindowsMicrophonePermissionStatus {
    const MICROPHONE_PATH: &str =
        "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone";
    const DESKTOP_APPS_PATH: &str =
        "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\\NonPackaged";

    let device_access = read_registry_permission_access(HKEY_LOCAL_MACHINE, MICROPHONE_PATH);
    let app_access = read_registry_permission_access(HKEY_CURRENT_USER, MICROPHONE_PATH);
    let desktop_app_access = read_registry_permission_access(HKEY_CURRENT_USER, DESKTOP_APPS_PATH);

    let overall_access = if [device_access, app_access, desktop_app_access]
        .into_iter()
        .any(|access| access == PermissionAccess::Denied)
    {
        PermissionAccess::Denied
    } else if [device_access, app_access, desktop_app_access]
        .into_iter()
        .all(|access| access == PermissionAccess::Allowed)
    {
        PermissionAccess::Allowed
    } else {
        PermissionAccess::Unknown
    };

    WindowsMicrophonePermissionStatus {
        supported: true,
        overall_access,
        device_access,
        app_access,
        desktop_app_access,
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_windows_microphone_permission_status() -> WindowsMicrophonePermissionStatus {
    #[cfg(target_os = "windows")]
    {
        get_windows_microphone_permission_status_impl()
    }

    #[cfg(not(target_os = "windows"))]
    {
        WindowsMicrophonePermissionStatus {
            supported: false,
            overall_access: PermissionAccess::Unknown,
            device_access: PermissionAccess::Unknown,
            app_access: PermissionAccess::Unknown,
            desktop_app_access: PermissionAccess::Unknown,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn open_microphone_privacy_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:privacy-microphone"])
            .spawn()
            .map_err(|e| format!("Failed to open Windows microphone privacy settings: {}", e))?;
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Opening microphone privacy settings is only supported on Windows".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub fn update_microphone_mode(app: AppHandle, always_on: bool) -> Result<(), String> {
    // Update settings
    let mut settings = get_settings(&app);
    settings.always_on_microphone = always_on;
    write_settings(&app, settings);

    // Update the audio manager mode
    let rm = app.state::<Arc<AudioRecordingManager>>();
    let new_mode = if always_on {
        MicrophoneMode::AlwaysOn
    } else {
        MicrophoneMode::OnDemand
    };

    rm.update_mode(new_mode)
        .map_err(|e| format!("Failed to update microphone mode: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn get_available_microphones() -> Result<Vec<AudioDevice>, String> {
    let devices =
        list_input_devices().map_err(|e| format!("Failed to list audio devices: {}", e))?;

    let mut result = vec![AudioDevice {
        index: "default".to_string(),
        name: "Default".to_string(),
        is_default: true,
    }];

    result.extend(devices.into_iter().map(|d| AudioDevice {
        index: d.index,
        name: d.name,
        is_default: false, // The explicit default is handled separately
    }));

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn set_selected_microphone(app: AppHandle, device_name: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    let selected_microphone = if device_name == "default" {
        None
    } else {
        Some(device_name.clone())
    };
    let changed = settings.selected_microphone != selected_microphone;
    settings.selected_microphone = selected_microphone.clone();
    if device_name != "default" {
        settings.last_manual_microphone = selected_microphone.clone();
    }
    write_settings(&app, settings);
    microphone_auto_switch::remember_manual_microphone_selection(&app, selected_microphone.clone());

    // Update the audio manager to use the new device
    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.update_selected_device()
        .map_err(|e| format!("Failed to update selected device: {}", e))?;

    if changed {
        let display_name = if device_name == "default" {
            "Default"
        } else {
            device_name.as_str()
        };
        crate::overlay::show_microphone_switch_overlay(&app, display_name);
        microphone_auto_switch::emit_audio_input_state_changed(&app);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_selected_microphone_auto_switch_enabled_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_microphone_auto_switch_enabled = enabled;
    write_settings(&app, settings);

    microphone_auto_switch::emit_audio_input_state_changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_selected_microphone_name_pattern_setting(
    app: AppHandle,
    pattern: String,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_microphone_name_pattern = pattern.trim().to_string();
    write_settings(&app, settings);

    microphone_auto_switch::emit_audio_input_state_changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_available_output_devices() -> Result<Vec<AudioDevice>, String> {
    let devices =
        list_output_devices().map_err(|e| format!("Failed to list output devices: {}", e))?;

    let mut result = vec![AudioDevice {
        index: "default".to_string(),
        name: "Default".to_string(),
        is_default: true,
    }];

    result.extend(devices.into_iter().map(|d| AudioDevice {
        index: d.index,
        name: d.name,
        is_default: false, // The explicit default is handled separately
    }));

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn set_selected_output_device(app: AppHandle, device_name: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_output_device = if device_name == "default" {
        None
    } else {
        Some(device_name)
    };
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_live_sound_capture_source_setting(
    app: AppHandle,
    source: LiveSoundCaptureSource,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.live_sound_capture_source = source;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_live_sound_speaker_diarization_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.live_sound_enable_speaker_diarization = enabled;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn play_test_sound(app: AppHandle, sound_type: String) {
    let sound = match sound_type.as_str() {
        "start" => audio_feedback::SoundType::Start,
        "stop" => audio_feedback::SoundType::Stop,
        _ => {
            warn!("Unknown sound type: {}", sound_type);
            return;
        }
    };
    audio_feedback::play_test_sound(&app, sound);
}

#[tauri::command]
#[specta::specta]
pub fn set_clamshell_microphone(app: AppHandle, device_name: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.clamshell_microphone = if device_name == "default" {
        None
    } else {
        Some(device_name)
    };
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_live_sound_microphone(app: AppHandle, device_name: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.live_sound_microphone = if device_name == "default" {
        None
    } else {
        Some(device_name)
    };
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn is_recording(app: AppHandle) -> bool {
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    audio_manager.is_recording()
}

#[tauri::command]
#[specta::specta]
pub fn change_vad_threshold_setting(app: AppHandle, threshold: f32) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.vad_threshold = threshold;
    write_settings(&app, settings);

    // Update the audio manager immediately
    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.update_vad_threshold(threshold);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_microphone_input_boost_db_setting(app: AppHandle, db: f32) -> Result<(), String> {
    let sanitized = sanitize_microphone_input_boost_db(db);

    let mut settings = get_settings(&app);
    settings.microphone_input_boost_db = sanitized;
    write_settings(&app, settings);

    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.refresh_microphone_input_boost_from_settings();
    crate::managers::live_sound_audio::refresh_microphone_input_boost_from_settings(&app);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_microphone_input_boost_for_device_setting(
    app: AppHandle,
    device_name: String,
    db: f32,
) -> Result<(), String> {
    let sanitized = sanitize_microphone_input_boost_db(db);
    let key = microphone_input_boost_device_key(Some(&device_name));

    let mut settings = get_settings(&app);
    if sanitized <= 0.0 {
        settings.microphone_input_boost_db_by_device.remove(&key);
    } else {
        settings
            .microphone_input_boost_db_by_device
            .insert(key, sanitized);
    }
    settings.microphone_input_boost_db = 0.0;
    write_settings(&app, settings);

    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.refresh_microphone_input_boost_from_settings();
    crate::managers::live_sound_audio::refresh_microphone_input_boost_from_settings(&app);

    Ok(())
}
