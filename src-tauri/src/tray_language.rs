//! Profile-scoped tray language choices. Settings remain the source of truth.
use crate::language_detection::{engine_accepts_language, LanguageDetectionMode};
use crate::managers::model::{canonical_language_code, ModelManager};
use crate::settings::{self, AppSettings, TranscriptionProvider};
use crate::tray_i18n::{get_tray_translations, TrayStrings};
use std::sync::Arc;
use tauri::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};

const PREFIX: &str = "profile_language:";
const SPOKEN_AUTO: &str = "auto_spoken";
const CONFIGURE: &str = "configure";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LanguageMenu {
    profile_id: String,
    profile_name: String,
    selected: String,
    preferred: Vec<String>,
    languages: Vec<String>,
    can_use_preferences: bool,
    native_detects: bool,
    accepts_language: bool,
    gemini_locale: bool,
}

fn identity(code: &str) -> String {
    canonical_language_code(&code.trim().to_ascii_lowercase()).to_string()
}

fn unique(codes: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut result = Vec::new();
    for code in codes {
        let code = identity(&code);
        if !result.contains(&code) {
            result.push(code);
        }
    }
    result
}

fn language_name(code: &str) -> String {
    let code = identity(code);
    match code.as_str() {
        "el" => "Greek".to_string(),
        "ht" => "Haitian Creole".to_string(),
        "pa" => "Punjabi".to_string(),
        "ps" => "Pashto".to_string(),
        "yue" => "Cantonese".to_string(),
        _ => isolang::Language::from_639_1(&code)
            .or_else(|| isolang::Language::from_639_3(&code))
            .map(|language| language.to_name().to_string())
            .unwrap_or(code),
    }
}

pub(crate) fn snapshot(app: &AppHandle, settings: &AppSettings) -> LanguageMenu {
    let profile = if settings.active_profile_id == "default" { None }
        else { settings.transcription_profile(&settings.active_profile_id) };
    let mut effective = settings.clone();
    if let Some(profile) = profile {
        effective.selected_language = profile.language.clone();
        effective.spoken_languages = profile.spoken_languages.clone();
        effective.language_detection = profile.language_detection;
        if let Some(selection) = &profile.stt_model_selection_override {
            if let Err(error) = settings::apply_stt_model_selection(&mut effective, selection) {
                log::warn!("Ignoring invalid tray profile model override: {error}");
            }
        }
    }
    let local = effective.transcription_provider == TranscriptionProvider::Local;
    let gemini_locale = effective.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible
        && effective.remote_stt.model_id.to_ascii_lowercase().contains("gemini-3.5-transcribe");
    if gemini_locale {
        effective.selected_language = profile.and_then(|profile| profile.gemini_language_code_override.clone())
            .unwrap_or_else(|| settings.gemini_language_code.clone());
    }
    let model = local.then(|| app.state::<Arc<ModelManager>>()
        .get_model_info(&effective.selected_model)).flatten();
    let model_languages = unique(model.as_ref().map(|model| model.supported_languages.clone()).unwrap_or_default());
    let accepts_language = !local || model.as_ref()
        .map(|model| engine_accepts_language(&model.engine_type)).unwrap_or(true);
    let native_detects = model.as_ref().map(|model| model.supports_language_detection).unwrap_or(!local);
    let streaming_auto = native_detects && local
        && crate::actions::should_use_native_local_streaming(app, &effective, profile, "transcribe");
    let can_use_preferences = local && accepts_language && model_languages.len() > 1 && !streaming_auto;
    let selected = if effective.selected_language == "auto" && can_use_preferences
        && (effective.language_detection == LanguageDetectionMode::Always
            || (!native_detects && !effective.spoken_languages.is_empty())) {
        SPOKEN_AUTO.to_string()
    } else if gemini_locale || matches!(effective.selected_language.as_str(), "auto" | "os_input") {
        effective.selected_language.clone()
    } else {
        identity(&effective.selected_language)
    };
    let mut languages = if gemini_locale {
        crate::gemini_config::GEMINI_SUPPORTED_LOCALES.iter().map(|code| code.to_string()).collect()
    } else if local && !model_languages.is_empty() { model_languages }
        else { unique(PICKER_LANGUAGES.iter().map(|code| code.to_string())) };
    if effective.transcription_provider == TranscriptionProvider::RemoteSoniox {
        languages.retain(|code| crate::language_resolver::is_soniox_supported_language(code));
    }
    languages.sort_by_cached_key(|code| language_name(code));
    LanguageMenu {
        profile_id: profile.map(|profile| profile.id.clone()).unwrap_or_else(|| "default".to_string()),
        profile_name: profile.map(|profile| profile.name.clone()).unwrap_or_default(),
        selected,
        preferred: unique(effective.spoken_languages),
        languages,
        can_use_preferences,
        native_detects,
        accepts_language,
        gemini_locale,
    }
}

fn text<'a>(translated: &'a str, english: &'a str) -> &'a str {
    if translated.is_empty() { english } else { translated }
}

fn menu_id(profile_id: &str, value: &str) -> String {
    format!("{PREFIX}{}", serde_json::to_string(&(profile_id, value)).expect("string pair"))
}

impl LanguageMenu {
    fn language_label(&self, code: &str) -> String {
        if self.gemini_locale { format!("{} ({code})", language_name(code)) }
        else { language_name(code) }
    }

    fn label(&self, strings: &TrayStrings) -> String {
        let current = match self.selected.as_str() {
            "auto" if !self.native_detects => text(&strings.language_model_default, "Auto — model default language").to_string(),
            "auto" => text(&strings.language_model_auto, "Auto — STT model").to_string(),
            SPOKEN_AUTO => text(&strings.language_spoken_auto, "Auto — AivoRelay detector").to_string(),
            "os_input" => text(&strings.language_os_input, "Follow OS input language").to_string(),
            code => self.language_label(code),
        };
        let profile = if self.profile_name.is_empty() {
            text(&strings.language_default_profile, "Default")
        } else { &self.profile_name };
        format!("{} ({profile}): {current}", text(&strings.language, "Language"))
    }

    fn append_choice(&self, app: &AppHandle, menu: &Submenu<tauri::Wry>, code: &str, label: &str, enabled: bool)
        -> Result<(), Box<dyn std::error::Error>> {
        menu.append(&CheckMenuItem::with_id(app, menu_id(&self.profile_id, code), label,
            enabled, self.selected == code, None::<&str>)?)?;
        Ok(())
    }

    pub(crate) fn build(&self, app: &AppHandle, locale: &str, webviews_disabled: bool)
        -> Result<Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
        let strings = get_tray_translations(Some(locale.to_string()));
        let menu = Submenu::with_id(app, "profile_language_submenu", self.label(&strings), true)?;
        self.append_choice(app, &menu, "auto",
            if self.native_detects { text(&strings.language_model_auto, "Auto — STT model") }
            else { text(&strings.language_model_default, "Auto — model default language") }, self.accepts_language)?;
        self.append_choice(app, &menu, SPOKEN_AUTO,
            text(&strings.language_spoken_auto, "Auto — AivoRelay detector"), self.can_use_preferences)?;
        self.append_choice(app, &menu, "os_input",
            text(&strings.language_os_input, "Follow OS input language"), self.accepts_language)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        let mut leading = self.preferred.iter().filter(|code| self.languages.contains(code)).cloned().collect::<Vec<_>>();
        if !matches!(self.selected.as_str(), "auto" | SPOKEN_AUTO | "os_input") && !leading.contains(&self.selected) {
            leading.insert(0, self.selected.clone());
        }
        for code in &leading {
            self.append_choice(app, &menu, code, &self.language_label(code), self.accepts_language && self.languages.contains(code))?;
        }
        let others = Submenu::with_id(app, "profile_language_others",
            text(&strings.language_other, "Other languages"), self.accepts_language)?;
        for code in self.languages.iter().filter(|code| !leading.contains(code)) {
            self.append_choice(app, &others, code, &self.language_label(code), self.accepts_language)?;
        }
        if !others.items()?.is_empty() { menu.append(&others)?; }
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(app, menu_id(&self.profile_id, CONFIGURE),
            text(&strings.language_configure, "Configure this profile's languages…"), !webviews_disabled, None::<&str>)?)?;
        Ok(menu)
    }
}

/// Edit only the profile represented by the menu; a stale menu never edits a new profile.
fn apply_choice(settings: &mut AppSettings, menu: &LanguageMenu, value: &str) -> Result<bool, String> {
    if settings.active_profile_id != menu.profile_id { return Ok(false); }
    if value == menu.selected { return Ok(false); }
    if value == SPOKEN_AUTO {
        if !menu.can_use_preferences { return Err("Selected model cannot use the language detector".to_string()); }
    } else if !menu.accepts_language || (!matches!(value, "auto" | "os_input") && !menu.languages.iter().any(|code| code == value)) {
        return Err("Language is unavailable for this model".to_string());
    }
    if menu.gemini_locale {
        let language = crate::gemini_config::validate_language_selection(value)?;
        if menu.profile_id == "default" {
            settings.gemini_language_code = language;
        } else {
            let profile = settings.transcription_profiles.iter_mut().find(|profile| profile.id == menu.profile_id)
                .ok_or_else(|| "Profile no longer exists".to_string())?;
            profile.gemini_language_code_override = Some(language);
        }
        return Ok(true);
    }
    let (language, preferred, mode) = if menu.profile_id == "default" {
        (&mut settings.selected_language, &mut settings.spoken_languages, &mut settings.language_detection)
    } else {
        let profile = settings.transcription_profiles.iter_mut().find(|profile| profile.id == menu.profile_id)
            .ok_or_else(|| "Profile no longer exists".to_string())?;
        (&mut profile.language, &mut profile.spoken_languages, &mut profile.language_detection)
    };
    if value == SPOKEN_AUTO {
        *language = "auto".to_string();
        *mode = LanguageDetectionMode::Always;
    } else {
        *language = value.to_string();
        if value == "auto" {
            preferred.clear();
            *mode = LanguageDetectionMode::WhenNeeded;
        }
    }
    Ok(true)
}

pub(crate) fn handle_menu_event(app: &AppHandle, id: &str) -> bool {
    let Some(payload) = id.strip_prefix(PREFIX) else { return false; };
    let Ok((profile_id, value)) = serde_json::from_str::<(String, String)>(payload) else { return true; };
    let app = app.clone();
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let guard = settings::lock_settings_mutation("changing profile language from tray")?;
            let mut settings = settings::get_settings(&app);
            if settings.active_profile_id != profile_id { return Ok(()); }
            if value == CONFIGURE {
                let gemini = snapshot(&app, &settings).gemini_locale;
                drop(guard);
                crate::show_main_window(&app);
                let _ = app.emit("open-profile-languages", serde_json::json!({ "profileId": profile_id, "gemini": gemini }));
                return Ok(());
            }
            let menu = snapshot(&app, &settings);
            if apply_choice(&mut settings, &menu, &value)? {
                settings::write_settings_checked(&app, settings)?;
                drop(guard);
                let _ = app.emit("profile-language-changed", &profile_id);
            }
            Ok(())
        })();
        if let Err(error) = result {
            log::error!("Failed to change profile language from tray: {error}");
            let _ = app.emit("profile-language-change-failed", error);
        }
        // Native check items toggle even when the chosen value was already selected.
        crate::tray::refresh_tray_menu_after_language_choice(&app);
    });
    true
}

// Mirrors the main language picker's codes (aliases are deduplicated by identity).
const PICKER_LANGUAGES: &[&str] = &[
    "ar", "af", "am", "as", "az", "ba", "be", "bn", "bo", "bs", "br", "bg",
    "ca", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "eu",
    "fil", "fo", "ga", "gl", "gu", "ha", "haw", "he", "hi", "ht", "hu", "hy",
    "id", "is", "it", "ja", "jw", "ka", "kk", "km", "kn", "ko", "la", "lb",
    "ln", "lo", "lv", "lt", "mg", "mk", "ms", "ml", "mi", "mn", "mr", "mt",
    "my", "nb", "ne", "nn", "no", "oc", "fa", "pa", "pl", "ps", "pt", "ro",
    "ru", "sa", "sd", "si", "sn", "so", "sq", "sr", "su", "sw", "zh", "sk",
    "sl", "es", "sv", "ta", "te", "tg", "th", "tk", "tl", "tt", "yue", "tr",
    "uk", "ur", "uz", "vi", "cy", "yi", "yo",
];


#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{get_default_settings, TranscriptionProfile};

    fn profile(id: &str) -> TranscriptionProfile {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "language": "en", "translate_to_english": false,
            "spoken_languages": ["en", "ru"], "language_detection": "always"
        })).unwrap()
    }

    fn menu(profile_id: &str) -> LanguageMenu {
        LanguageMenu {
            profile_id: profile_id.to_string(), profile_name: profile_id.to_string(),
            selected: "en".to_string(), preferred: vec!["en".to_string(), "ru".to_string()],
            languages: vec!["en".to_string(), "ru".to_string()], can_use_preferences: true,
            native_detects: true, accepts_language: true, gemini_locale: false,
        }
    }

    #[test]
    fn tray_language_edits_only_the_active_custom_profile() {
        let mut settings = get_default_settings();
        settings.selected_language = "de".to_string();
        settings.active_profile_id = "a".to_string();
        settings.transcription_profiles = vec![profile("a"), profile("b")];
        assert!(apply_choice(&mut settings, &menu("a"), "ru").unwrap());
        assert_eq!(settings.selected_language, "de");
        assert_eq!(settings.transcription_profiles[0].language, "ru");
        assert_eq!(settings.transcription_profiles[1].language, "en");
        assert_eq!(settings.transcription_profiles[0].spoken_languages, ["en", "ru"]);
        assert!(!apply_choice(&mut settings, &menu("b"), "ru").unwrap());
        assert_eq!(settings.transcription_profiles[1].language, "en");
    }

    #[test]
    fn auto_modes_match_the_settings_picker_and_reselect_keeps_preferences() {
        let mut settings = get_default_settings();
        settings.active_profile_id = "default".to_string();
        settings.selected_language = "en".to_string();
        settings.spoken_languages = vec!["en".to_string(), "ru".to_string()];
        let mut menu = menu("default");
        assert!(apply_choice(&mut settings, &menu, SPOKEN_AUTO).unwrap());
        assert_eq!(settings.selected_language, "auto");
        assert_eq!(settings.language_detection, LanguageDetectionMode::Always);
        assert_eq!(settings.spoken_languages.len(), 2);
        menu.selected = SPOKEN_AUTO.to_string();
        assert!(!apply_choice(&mut settings, &menu, SPOKEN_AUTO).unwrap());
        assert!(apply_choice(&mut settings, &menu, "auto").unwrap());
        assert!(settings.spoken_languages.is_empty());
        assert_eq!(settings.language_detection, LanguageDetectionMode::WhenNeeded);
        settings.spoken_languages = vec!["ru".to_string()];
        menu.selected = "auto".to_string();
        assert!(!apply_choice(&mut settings, &menu, "auto").unwrap());
        assert_eq!(settings.spoken_languages, ["ru"]);
    }

    #[test]
    fn unavailable_languages_and_inactive_detector_are_rejected_without_changes() {
        let mut settings = get_default_settings();
        settings.active_profile_id = "default".to_string();
        settings.selected_language = "en".to_string();
        let mut menu = menu("default");
        menu.can_use_preferences = false;
        for code in ["de", SPOKEN_AUTO] {
            assert!(apply_choice(&mut settings, &menu, code).is_err());
            assert_eq!(settings.selected_language, "en");
        }
    }

    #[test]
    fn gemini_uses_exact_profile_locale_without_modifying_global_or_other_language_settings() {
        let mut settings = get_default_settings();
        settings.active_profile_id = "a".to_string();
        settings.gemini_language_code = "en-US".to_string();
        settings.transcription_profiles = vec![profile("a"), profile("b")];
        let mut menu = menu("a");
        menu.gemini_locale = true;
        menu.can_use_preferences = false;
        menu.languages = vec!["ru-RU".to_string()];
        assert!(apply_choice(&mut settings, &menu, "ru-RU").unwrap());
        assert_eq!(settings.transcription_profiles[0].gemini_language_code_override.as_deref(), Some("ru-RU"));
        assert_eq!(settings.gemini_language_code, "en-US");
        assert_eq!(settings.transcription_profiles[0].language, "en");
        assert_eq!(settings.transcription_profiles[1].gemini_language_code_override, None);
        assert!(apply_choice(&mut settings, &menu, "ru").is_err());
    }

    #[test]
    fn canonical_names_and_menu_ids_keep_aliases_and_profile_ids_unambiguous() {
        assert_eq!(unique(vec!["iw".into(), "he".into(), "JW".into(), "jv".into()]), ["he", "jv"]);
        assert_eq!(language_name("en-US"), "English");
        assert_eq!(language_name("jw"), "Javanese");
        let id = menu_id("profile:[a]\"", "ru-RU");
        let parsed: (String, String) = serde_json::from_str(id.strip_prefix(PREFIX).unwrap()).unwrap();
        assert_eq!(parsed, ("profile:[a]\"".to_string(), "ru-RU".to_string()));
    }
}
