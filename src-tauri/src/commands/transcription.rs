use crate::managers::transcription::{
    apply_accelerator_settings, get_available_accelerators as collect_available_accelerators,
    AvailableAccelerators,
    TranscriptionManager,
};
use crate::settings::{
    get_settings, write_settings, ModelUnloadTimeout, OrtAcceleratorSetting,
    WhisperAcceleratorSetting,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
#[specta::specta]
pub fn set_model_unload_timeout(app: AppHandle, timeout: ModelUnloadTimeout) {
    let mut settings = get_settings(&app);
    settings.model_unload_timeout = timeout;
    write_settings(&app, settings);
}

#[tauri::command]
#[specta::specta]
pub fn unload_model_manually(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<(), String> {
    transcription_manager
        .unload_model()
        .map_err(|e| format!("Failed to unload model: {}", e))
}

fn apply_and_reload_accelerator(app: &AppHandle) {
    apply_accelerator_settings(app);

    let transcription_manager = app.state::<Arc<TranscriptionManager>>();
    if transcription_manager.is_model_loaded() {
        if let Err(err) = transcription_manager.unload_model() {
            log::warn!("Failed to unload model after accelerator change: {}", err);
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn change_whisper_accelerator_setting(
    app: AppHandle,
    accelerator: WhisperAcceleratorSetting,
) {
    let mut settings = get_settings(&app);
    settings.whisper_accelerator = accelerator;
    write_settings(&app, settings);
    apply_and_reload_accelerator(&app);
}

#[tauri::command]
#[specta::specta]
pub fn change_ort_accelerator_setting(app: AppHandle, accelerator: OrtAcceleratorSetting) {
    let mut settings = get_settings(&app);
    settings.ort_accelerator = accelerator;
    write_settings(&app, settings);
    apply_and_reload_accelerator(&app);
}

#[tauri::command]
#[specta::specta]
pub fn get_available_accelerators() -> AvailableAccelerators {
    collect_available_accelerators()
}
