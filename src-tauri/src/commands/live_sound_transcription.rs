use crate::actions;
use crate::managers::live_sound_transcription::LiveSoundTranscriptionStatePayload;
use crate::settings::{get_settings, write_settings, LiveSoundTranscriptionProvider};
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn live_sound_transcription_start(app: AppHandle) -> Result<(), String> {
    actions::start_live_sound_transcription_session(&app)
}

#[tauri::command]
#[specta::specta]
pub fn live_sound_transcription_stop(app: AppHandle) -> Result<(), String> {
    actions::stop_live_sound_transcription_session(&app)
}

#[tauri::command]
#[specta::specta]
pub fn live_sound_transcription_clear(app: AppHandle) -> Result<(), String> {
    crate::managers::live_sound_transcription::clear_transcript(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn live_sound_transcription_process(app: AppHandle) -> Result<(), String> {
    actions::process_live_sound_transcription_text(app).await
}

#[tauri::command]
#[specta::specta]
pub fn live_sound_transcription_close(app: AppHandle) -> Result<(), String> {
    if crate::actions::is_live_sound_recording(&app) {
        actions::stop_live_sound_transcription_session(&app)?;
    }
    crate::managers::live_sound_transcription::finish_session(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_live_sound_transcription_state() -> LiveSoundTranscriptionStatePayload {
    crate::managers::live_sound_transcription::get_state_payload()
}

#[tauri::command]
#[specta::specta]
pub fn set_live_sound_auto_stop_minutes(app: AppHandle, minutes: u32) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.live_sound_auto_stop_minutes = minutes;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn save_live_sound_transcript(path: String, content: String) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("Transcript is empty.".to_string());
    }
    std::fs::write(&path, content).map_err(|e| format!("Failed to save transcript: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn change_live_sound_transcription_provider(
    app: AppHandle,
    provider: LiveSoundTranscriptionProvider,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.live_sound_transcription_provider = provider;
    write_settings(&app, settings);
    Ok(())
}
