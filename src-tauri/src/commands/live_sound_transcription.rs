use crate::actions;
use crate::managers::preview_output_mode::PreviewOutputModeStatePayload;
use tauri::AppHandle;

fn ensure_live_sound_session_scope() -> Result<(), String> {
    let state = crate::managers::preview_output_mode::get_state_payload();
    if state.active && state.binding_id.as_deref() != Some(crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID) {
        return Err(
            "Another preview-owned transcription session is active. Stop it before using Live Sound Transcription."
                .to_string(),
        );
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn live_sound_transcription_start(app: AppHandle) -> Result<(), String> {
    ensure_live_sound_session_scope()?;
    actions::start_live_sound_transcription_session(&app)
}

#[tauri::command]
#[specta::specta]
pub fn live_sound_transcription_stop(app: AppHandle) -> Result<(), String> {
    ensure_live_sound_session_scope()?;
    actions::stop_live_sound_transcription_session(&app)
}

#[tauri::command]
#[specta::specta]
pub fn live_sound_transcription_clear(app: AppHandle) -> Result<(), String> {
    ensure_live_sound_session_scope()?;
    actions::preview_clear_action(app)
}

#[tauri::command]
#[specta::specta]
pub async fn live_sound_transcription_process(app: AppHandle) -> Result<(), String> {
    ensure_live_sound_session_scope()?;
    actions::preview_llm_process_action(app).await
}

#[tauri::command]
#[specta::specta]
pub fn live_sound_transcription_close(app: AppHandle) -> Result<(), String> {
    ensure_live_sound_session_scope()?;
    actions::preview_close_action(app)
}

#[tauri::command]
#[specta::specta]
pub fn get_live_sound_transcription_state() -> PreviewOutputModeStatePayload {
    crate::managers::preview_output_mode::get_state_payload()
}
