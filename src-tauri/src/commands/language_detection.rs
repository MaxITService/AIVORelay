use crate::language_detection::LanguageDetectionMode;
use crate::settings::{get_settings, write_settings_checked};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
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

#[derive(Serialize, Type)]
pub struct LanguageDetectorDownloadStatus {
    pub downloaded: bool,
    pub progress: Option<crate::managers::model::DownloadProgress>,
}

#[tauri::command]
#[specta::specta]
pub fn get_language_detector_download_status(manager: State<'_, Arc<ModelManager>>) -> LanguageDetectorDownloadStatus {
    LanguageDetectorDownloadStatus {
        downloaded: manager.language_detector_path().is_some(),
        progress: manager.language_detector_download_progress(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn open_model_downloads(app: AppHandle, detector: bool) {
    crate::show_main_window(&app);
    let _ = app.emit("open-model-downloads", serde_json::json!({ "detector": detector }));
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
    let unique = normalized_spoken_languages(spoken_languages, window_secs)?;
    let mut settings = get_settings(&app);
    settings.spoken_languages = unique;
    settings.language_detection = mode;
    settings.language_detection_window_secs = window_secs;
    write_settings_checked(&app, settings)?;
    drop(_guard);
    crate::tray::refresh_tray_menu(&app, None);
    Ok(())
}

/// Validates the detector settings and drops case and alias duplicates.
pub(crate) fn normalized_spoken_languages(spoken_languages: Vec<String>, window_secs: u32) -> Result<Vec<String>, String> {
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
    Ok(unique)
}

#[cfg(test)]
mod tests {
    use super::normalized_spoken_languages;

    fn languages(list: &[&str]) -> Vec<String> {
        list.iter().map(|code| code.to_string()).collect()
    }

    #[test]
    fn detection_window_is_limited_to_two_to_thirty_seconds() {
        for (window, valid) in [(0, false), (1, false), (2, true), (30, true), (31, false)] {
            assert_eq!(normalized_spoken_languages(languages(&["en"]), window).is_ok(), valid, "{window} s");
        }
    }

    #[test]
    fn malformed_language_lists_are_rejected() {
        let too_long = "a".repeat(36);
        for invalid in ["", "e n", "en/us", "fr;", "\u{e9}", too_long.as_str()] {
            assert!(normalized_spoken_languages(languages(&["en", invalid]), 5).is_err(), "{invalid:?}");
        }
        assert!(normalized_spoken_languages(languages(&["a".repeat(35).as_str(), "zh-Hant", "pt_BR"]), 5).is_ok());
        let many = |count: usize| (0..count).map(|index| format!("x{index}")).collect::<Vec<_>>();
        assert_eq!(normalized_spoken_languages(many(107), 5).unwrap().len(), 107);
        assert!(normalized_spoken_languages(many(108), 5).is_err());
    }

    #[test]
    fn case_and_alias_duplicates_keep_the_first_spelling() {
        let input = languages(&["EN", "en", "nb", "no", "iw", "he", "fil", "tl", "jw", "jv", "pt-BR", "pt", "zh", "yue"]);
        assert_eq!(
            normalized_spoken_languages(input, 5).unwrap(),
            languages(&["en", "nb", "iw", "fil", "jw", "pt-br", "zh", "yue"])
        );
        assert!(normalized_spoken_languages(Vec::new(), 5).unwrap().is_empty());
    }
}
