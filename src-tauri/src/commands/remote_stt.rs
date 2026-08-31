use crate::managers::remote_stt::{
    clear_remote_stt_api_key, has_remote_stt_api_key, set_remote_stt_api_key, supports_translation,
    RemoteSttManager,
};
use crate::settings::{
    apply_stt_model_selection, get_settings, SttModelSelection, TranscriptionProvider,
};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SttModelSelectionReadiness {
    pub selection: SttModelSelection,
    pub ready: bool,
    pub reason: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn stt_model_selections_readiness(
    app: AppHandle,
    selections: Vec<SttModelSelection>,
) -> Vec<SttModelSelectionReadiness> {
    let settings = get_settings(&app);
    selections
        .into_iter()
        .map(|selection| {
            let mut candidate = settings.clone();
            let configuration_error = apply_stt_model_selection(&mut candidate, &selection).err();
            let ready = if configuration_error.is_some() {
                false
            } else {
                match selection.provider {
                    TranscriptionProvider::Local => true,
                    TranscriptionProvider::RemoteSoniox => {
                        crate::secure_keys::has_soniox_api_key()
                    }
                    TranscriptionProvider::RemoteDeepgram => {
                        crate::secure_keys::has_deepgram_api_key()
                    }
                    TranscriptionProvider::RemoteOpenAiCompatible => {
                        has_remote_stt_api_key(&candidate.remote_stt)
                    }
                }
            };
            let reason = configuration_error.or_else(|| {
                (!ready).then(|| "API key is not configured in Models.".to_string())
            });
            SttModelSelectionReadiness {
                selection,
                ready,
                reason,
            }
        })
        .collect()
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_has_api_key(app: AppHandle) -> Result<bool, String> {
    let settings = get_settings(&app);
    Ok(has_remote_stt_api_key(&settings.remote_stt))
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_set_api_key(app: AppHandle, api_key: String) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    let settings = get_settings(&app);
    set_remote_stt_api_key(&settings.remote_stt, api_key.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_clear_api_key(app: AppHandle) -> Result<(), String> {
    let settings = get_settings(&app);
    clear_remote_stt_api_key(&settings.remote_stt).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn soniox_has_api_key() -> Result<bool, String> {
    Ok(crate::secure_keys::has_soniox_api_key())
}

#[tauri::command]
#[specta::specta]
pub fn soniox_set_api_key(api_key: String) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    crate::secure_keys::set_soniox_api_key(api_key.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn soniox_clear_api_key() -> Result<(), String> {
    crate::secure_keys::clear_soniox_api_key().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn deepgram_has_api_key() -> Result<bool, String> {
    Ok(crate::secure_keys::has_deepgram_api_key())
}

#[tauri::command]
#[specta::specta]
pub fn deepgram_set_api_key(api_key: String) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    crate::secure_keys::set_deepgram_api_key(api_key.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn deepgram_clear_api_key() -> Result<(), String> {
    crate::secure_keys::clear_deepgram_api_key().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_get_debug_dump(
    remote_manager: State<'_, Arc<RemoteSttManager>>,
) -> Result<Vec<String>, String> {
    Ok(remote_manager.get_debug_dump())
}

#[tauri::command]
#[specta::specta]
pub fn remote_stt_clear_debug(
    remote_manager: State<'_, Arc<RemoteSttManager>>,
) -> Result<(), String> {
    remote_manager.clear_debug();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn remote_stt_test_connection(
    app: AppHandle,
    base_url: String,
    remote_manager: State<'_, Arc<RemoteSttManager>>,
) -> Result<(), String> {
    let settings = get_settings(&app);
    remote_manager
        .test_connection(&settings.remote_stt, &base_url)
        .await
        .map_err(|e| e.to_string())
}

/// Returns whether the currently selected Remote STT model supports translation to English.
/// Uses the OpenAI-compatible /audio/translations endpoint.
/// Known support: Groq whisper-large-v3, OpenAI whisper-1/gpt-realtime-2.1/gpt-realtime-2/gpt-realtime-translate.
/// NOT supported: whisper-large-v3-turbo and OpenAI's transcription-only models.
#[tauri::command]
#[specta::specta]
pub fn remote_stt_supports_translation(app: AppHandle) -> bool {
    let settings = get_settings(&app);
    supports_translation(&settings.remote_stt.model_id)
}

/// Returns whether the specified OpenAI-compatible STT model supports
/// translation to English. Model capability rules remain backend-owned.
#[tauri::command]
#[specta::specta]
pub fn remote_stt_model_supports_translation(model_id: String) -> bool {
    supports_translation(&model_id)
}
