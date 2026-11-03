use crate::language_detection::LanguageDetectionMode;
use crate::settings::{get_settings, write_settings_checked};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, State};
use crate::managers::model::ModelManager;

#[derive(Serialize, Type)]
pub struct LanguageDetectionConfig {
    pub spoken_languages: Vec<String>,
    pub mode: LanguageDetectionMode,
    pub window_secs: u32,
}

#[tauri::command]
#[specta::specta]
pub fn language_detector_downloaded(manager: State<'_, Arc<ModelManager>>) -> bool {
    manager.language_detector_path().is_some()
}

#[tauri::command]
#[specta::specta]
pub fn get_local_language_detection_status(
    app: AppHandle,
    manager: State<'_, Arc<ModelManager>>,
) -> Option<crate::language_detection::policy::LanguageStatus> {
    let settings = get_settings(&app);
    if settings.spoken_languages.is_empty() || settings.selected_language != "auto" {
        return None;
    }
    let model = manager.get_model_info(&settings.selected_model)?;
    let detector = crate::catalog::LANGUAGE_DETECTOR.as_ref()?;
    if !crate::language_detection::engine_accepts_language(&model.engine_type) {
        return Some(crate::language_detection::policy::LanguageStatus::Auto);
    }
    let languages = crate::language_detection::policy::ModelLanguages {
        languages: &model.supported_languages,
        detects: model.supports_language_detection,
        // Dictation can also use native streaming, which is handled separately.
        streaming: false,
    };
    Some(crate::language_detection::policy::status(
        &settings.selected_language, &settings.spoken_languages, &languages,
        settings.language_detection, &detector.labels, manager.language_detector_path().is_some(),
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn download_language_detector(manager: State<'_, Arc<ModelManager>>) -> Result<(), String> {
    manager.download_language_detector().await.map_err(|error| format!("{error:#}"))
}

#[tauri::command]
#[specta::specta]
pub fn cancel_language_detector_download(manager: State<'_, Arc<ModelManager>>) -> Result<(), String> {
    manager.cancel_language_detector_download();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_language_detection_config(app: AppHandle) -> LanguageDetectionConfig {
    let settings = get_settings(&app);
    LanguageDetectionConfig {
        spoken_languages: settings.spoken_languages,
        mode: settings.language_detection,
        window_secs: settings.language_detection_window_secs.clamp(2, 30),
    }
}

#[tauri::command]
#[specta::specta]
pub fn set_language_detection_config(
    app: AppHandle,
    spoken_languages: Vec<String>,
    mode: LanguageDetectionMode,
    window_secs: u32,
) -> Result<(), String> {
    let _guard = crate::settings::lock_settings_mutation("changing spoken languages")?;
    if !(2..=30).contains(&window_secs) {
        return Err("Language detection window must be between 2 and 30 seconds".to_string());
    }
    if spoken_languages.len() > 107 || spoken_languages.iter().any(|language| {
        language.is_empty() || language.len() > 35
            || !language.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    }) {
        return Err("Invalid spoken language list".to_string());
    }
    let mut unique = Vec::new();
    for language in spoken_languages {
        let language = language.to_ascii_lowercase();
        if !unique.iter().any(|existing: &String| {
            crate::managers::model::canonical_language_code(existing)
                == crate::managers::model::canonical_language_code(&language)
        }) {
            unique.push(language);
        }
    }
    let mut settings = get_settings(&app);
    settings.spoken_languages = unique;
    settings.language_detection = mode;
    settings.language_detection_window_secs = window_secs;
    write_settings_checked(&app, settings)?;
    drop(_guard);
    crate::tray::refresh_tray_menu(&app, None);
    Ok(())
}
