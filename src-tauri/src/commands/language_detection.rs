use crate::language_detection::LanguageDetectionMode;
use crate::settings::{get_settings, write_settings_checked};
use serde::Serialize;
use specta::Type;
use tauri::AppHandle;

#[derive(Serialize, Type)]
pub struct LanguageDetectionConfig {
    pub spoken_languages: Vec<String>,
    pub mode: LanguageDetectionMode,
}

#[tauri::command]
#[specta::specta]
pub fn get_language_detection_config(app: AppHandle) -> LanguageDetectionConfig {
    let settings = get_settings(&app);
    LanguageDetectionConfig {
        spoken_languages: settings.spoken_languages,
        mode: settings.language_detection,
    }
}

#[tauri::command]
#[specta::specta]
pub fn set_language_detection_config(
    app: AppHandle,
    spoken_languages: Vec<String>,
    mode: LanguageDetectionMode,
) -> Result<(), String> {
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
    write_settings_checked(&app, settings)
}
