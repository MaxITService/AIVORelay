use crate::actions;
use crate::managers::live_sound_transcription::LiveSoundTranscriptionStatePayload;
use crate::settings::{
    apply_stt_model_selection, get_settings, write_settings, LiveSoundTranscriptionProvider,
    SttModelSelection, TranscriptionProvider,
};
use tauri::AppHandle;

const SONIOX_ENDPOINT_DELAY_MIN_MS: u32 = 500;
const SONIOX_ENDPOINT_DELAY_MAX_MS: u32 = 3000;
const DEEPGRAM_ENDPOINTING_MIN_MS: u32 = 10;
const DEEPGRAM_ENDPOINTING_MAX_MS: u32 = 5000;

fn validate_live_sound_model_selection(selection: &SttModelSelection) -> Result<(), String> {
    let supported = match selection.provider {
        TranscriptionProvider::RemoteSoniox => selection.model_id == "stt-rt-v5",
        TranscriptionProvider::RemoteDeepgram => selection.model_id == "nova-3",
        TranscriptionProvider::RemoteOpenAiCompatible => matches!(
            (selection.provider_preset.as_str(), selection.model_id.as_str()),
            ("vercel", "google/gemini-3.5-transcribe-live")
                | ("google", "gemini-3.5-transcribe-live")
        ),
        TranscriptionProvider::Local => false,
    };

    supported
        .then_some(())
        .ok_or_else(|| "This STT model is not supported by Live Monitor.".to_string())
}

fn validate_optional_range(
    value: Option<u32>,
    min: u32,
    max: u32,
    setting_name: &str,
) -> Result<(), String> {
    if value.is_some_and(|value| !(min..=max).contains(&value)) {
        return Err(format!("{setting_name} must be between {min} and {max} ms"));
    }
    Ok(())
}

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
    settings.live_sound_model_selection = None;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_live_sound_model_selection(
    app: AppHandle,
    selection: SttModelSelection,
) -> Result<(), String> {
    validate_live_sound_model_selection(&selection)?;
    let Some(provider) =
        LiveSoundTranscriptionProvider::from_transcription_provider(selection.provider)
    else {
        return Err("Live Monitor does not support local STT models.".to_string());
    };

    let mut settings = get_settings(&app);
    let mut candidate = settings.clone();
    apply_stt_model_selection(&mut candidate, &selection)?;
    settings.live_sound_transcription_provider = provider;
    settings.live_sound_model_selection = Some(selection);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_live_sound_soniox_endpoint_detection(
    app: AppHandle,
    value: Option<bool>,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.live_sound_soniox_endpoint_detection = value;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_live_sound_soniox_max_endpoint_delay_ms(
    app: AppHandle,
    value: Option<u32>,
) -> Result<(), String> {
    validate_optional_range(
        value,
        SONIOX_ENDPOINT_DELAY_MIN_MS,
        SONIOX_ENDPOINT_DELAY_MAX_MS,
        "Soniox endpoint delay",
    )?;
    let mut settings = get_settings(&app);
    settings.live_sound_soniox_max_endpoint_delay_ms = value;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_live_sound_deepgram_endpointing_enabled(
    app: AppHandle,
    value: Option<bool>,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.live_sound_deepgram_endpointing_enabled = value;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_live_sound_deepgram_endpointing_ms(
    app: AppHandle,
    value: Option<u32>,
) -> Result<(), String> {
    validate_optional_range(
        value,
        DEEPGRAM_ENDPOINTING_MIN_MS,
        DEEPGRAM_ENDPOINTING_MAX_MS,
        "Deepgram endpointing",
    )?;
    let mut settings = get_settings(&app);
    settings.live_sound_deepgram_endpointing_ms = value;
    write_settings(&app, settings);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_optional_range;

    #[test]
    fn optional_endpoint_range_accepts_clear_and_boundaries() {
        assert!(validate_optional_range(None, 10, 5000, "Endpointing").is_ok());
        assert!(validate_optional_range(Some(10), 10, 5000, "Endpointing").is_ok());
        assert!(validate_optional_range(Some(5000), 10, 5000, "Endpointing").is_ok());
    }

    #[test]
    fn optional_endpoint_range_rejects_values_outside_boundaries() {
        assert!(validate_optional_range(Some(9), 10, 5000, "Endpointing").is_err());
        assert!(validate_optional_range(Some(5001), 10, 5000, "Endpointing").is_err());
    }
}
