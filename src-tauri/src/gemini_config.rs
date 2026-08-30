use crate::settings::{AppSettings, GeminiTranscriptionMode, TranscriptionProfile};
use serde::{Deserialize, Serialize};
use specta::Type;

pub const GEMINI_VOCABULARY_HARD_MAX: usize = 1_000;
pub const GEMINI_FILE_STANDARD_MAX_SECONDS: f64 = 60.0 * 60.0;
pub const GEMINI_FILE_ANNOTATED_MAX_SECONDS: f64 = 30.0 * 60.0;

pub const GEMINI_SUPPORTED_LOCALES: &[&str] = &[
    "af-ZA", "am-ET", "ar-EG", "hy-AM", "as-IN", "az-AZ", "be-BY", "bn-BD",
    "bn-IN", "bs-BA", "bg-BG", "rup-BG", "my-MM", "yue-Hant-HK", "ca-ES", "ceb",
    "km-KH", "hr-HR", "cs-CZ", "da-DK", "nl-NL", "en-GB", "en-IN", "en-US",
    "et-EE", "fa-IR", "fil-PH", "fi-FI", "fr-FR", "gl-ES", "ka-GE", "de-DE",
    "el-GR", "gu-IN", "ha-NG", "he-IL", "hi-IN", "hu-HU", "is-IS", "id-ID",
    "it-IT", "ja-JP", "jv-ID", "kea-CV", "kn-IN", "kk-KZ", "ko-KR", "ky-KG",
    "lv-LV", "ln-CD", "lt-LT", "mk-MK", "ms-MY", "ml-IN", "mt-MT", "cmn-Hans-CN",
    "mr-IN", "mn-MN", "ne-NP", "nb-NO", "or-IN", "pl-PL", "pt-BR", "pt-PT",
    "pa-IN", "pa-Guru-IN", "ro-RO", "ru-RU", "sr-RS", "sd-Arab-IN", "sk-SK",
    "sl-SI", "es-419", "es-US", "sw-KE", "sv-SE", "tg-TJ", "te-IN", "th-TH",
    "tr-TR", "uk-UA", "uz-UZ", "vi-VN",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiWorkflow {
    Live,
    File { word_timestamps: bool },
    Dictation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GeminiRoute {
    GoogleDirect,
    VercelGateway,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveGeminiConfig {
    pub language_code: Option<String>,
    pub language_warning: Option<String>,
    pub custom_vocabulary: Vec<String>,
    pub mode: GeminiTranscriptionMode,
    pub diarization: bool,
    pub word_timestamps: bool,
    pub max_duration_seconds: u64,
    pub route: GeminiRoute,
}

pub fn is_supported_exact_locale(value: &str) -> bool {
    GEMINI_SUPPORTED_LOCALES.iter().any(|locale| *locale == value)
}

pub fn validate_vocabulary(terms: &[String]) -> Result<Vec<String>, String> {
    if terms.len() > GEMINI_VOCABULARY_HARD_MAX {
        return Err(format!(
            "Gemini Custom Vocabulary accepts at most {} terms; {} were provided.",
            GEMINI_VOCABULARY_HARD_MAX,
            terms.len()
        ));
    }
    let mut normalized = Vec::with_capacity(terms.len());
    for (index, term) in terms.iter().enumerate() {
        let trimmed = term.trim();
        if trimmed.is_empty() {
            return Err(format!(
                "Gemini Custom Vocabulary term {} is empty. Remove the empty entry before saving.",
                index + 1
            ));
        }
        if normalized.iter().any(|existing: &String| existing == trimmed) {
            return Err(format!(
                "Gemini Custom Vocabulary contains the duplicate term '{}' at position {}.",
                trimmed,
                index + 1
            ));
        }
        normalized.push(trimmed.to_string());
    }
    Ok(normalized)
}

pub fn map_os_locale(value: &str) -> Option<&'static str> {
    let normalized = value.trim().replace('_', "-");
    if let Some(exact) = GEMINI_SUPPORTED_LOCALES
        .iter()
        .copied()
        .find(|candidate| candidate.eq_ignore_ascii_case(&normalized))
    {
        return Some(exact);
    }

    let language = normalized.split('-').next()?.to_ascii_lowercase();
    let mut matches = GEMINI_SUPPORTED_LOCALES.iter().copied().filter(|candidate| {
        candidate
            .split('-')
            .next()
            .is_some_and(|part| part.eq_ignore_ascii_case(&language))
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn resolve_language(selection: &str, os_locale: Option<&str>) -> Result<(Option<String>, Option<String>), String> {
    let selection = selection.trim();
    if selection.is_empty() || selection.eq_ignore_ascii_case("auto") {
        return Ok((None, None));
    }
    if selection.eq_ignore_ascii_case("os_input") {
        let mapped = os_locale.and_then(map_os_locale);
        return Ok(match mapped {
            Some(locale) => (Some(locale.to_string()), None),
            None => (
                None,
                Some("The current OS input language could not be mapped unambiguously to a supported Gemini locale, so automatic detection will be used.".to_string()),
            ),
        });
    }
    if !is_supported_exact_locale(selection) {
        return Err(format!(
            "'{}' is not an exact locale supported by Gemini 3.5 Transcribe. Choose Auto Detect, Follow OS Input Language, or a listed BCP-47 locale.",
            selection
        ));
    }
    Ok((Some(selection.to_string()), None))
}

pub fn route_from_preset(preset: &str) -> Result<GeminiRoute, String> {
    match preset.trim() {
        "google" => Ok(GeminiRoute::GoogleDirect),
        "vercel" => Ok(GeminiRoute::VercelGateway),
        other => Err(format!(
            "Gemini 3.5 Transcribe requires the Google Direct or Vercel AI Gateway route; '{}' is configured.",
            other
        )),
    }
}

pub fn validate_compatibility(
    workflow: GeminiWorkflow,
    route: GeminiRoute,
    mode: GeminiTranscriptionMode,
    diarization: bool,
) -> Result<(), String> {
    let timestamps = matches!(workflow, GeminiWorkflow::File { word_timestamps: true });
    if matches!(workflow, GeminiWorkflow::Live) && diarization {
        return Err("Gemini Live does not support speaker diarization.".to_string());
    }
    if matches!(workflow, GeminiWorkflow::Live) && timestamps {
        return Err("Gemini Live does not support word timestamps.".to_string());
    }
    if diarization && route != GeminiRoute::GoogleDirect {
        return Err("Gemini speaker diarization is currently available only through Google Direct. Change the route or disable diarization.".to_string());
    }
    if mode == GeminiTranscriptionMode::Smart && timestamps {
        return Err("Gemini Smart mode cannot produce word timestamps. Select Verbatim for SRT or VTT output.".to_string());
    }
    if mode == GeminiTranscriptionMode::Smart && diarization {
        return Err("Gemini Smart mode cannot use speaker diarization. Select Verbatim or disable diarization.".to_string());
    }
    Ok(())
}

pub fn resolve_effective_config(
    settings: &AppSettings,
    profile: Option<&TranscriptionProfile>,
    workflow: GeminiWorkflow,
    os_locale: Option<&str>,
) -> Result<EffectiveGeminiConfig, String> {
    let route = route_from_preset(&settings.remote_stt.provider_preset)?;
    let language_selection = profile
        .and_then(|profile| profile.gemini_language_code_override.as_deref())
        .unwrap_or(&settings.gemini_language_code);
    let (language_code, language_warning) = resolve_language(language_selection, os_locale)?;
    let vocabulary = profile
        .and_then(|profile| profile.gemini_custom_vocabulary_override.as_ref())
        .unwrap_or(&settings.gemini_custom_vocabulary);
    let custom_vocabulary = validate_vocabulary(vocabulary)?;
    let mode = match workflow {
        GeminiWorkflow::Live => settings.gemini_live_mode,
        GeminiWorkflow::Dictation => settings
            .gemini_dictation_mode
            .unwrap_or(settings.gemini_file_mode),
        GeminiWorkflow::File { .. } => settings.gemini_file_mode,
    };
    let word_timestamps = matches!(workflow, GeminiWorkflow::File { word_timestamps: true });
    let diarization = matches!(workflow, GeminiWorkflow::File { .. }) && settings.gemini_file_diarization;
    validate_compatibility(workflow, route, mode, diarization)?;
    let max_duration_seconds = if word_timestamps || diarization {
        GEMINI_FILE_ANNOTATED_MAX_SECONDS as u64
    } else {
        GEMINI_FILE_STANDARD_MAX_SECONDS as u64
    };
    Ok(EffectiveGeminiConfig {
        language_code,
        language_warning,
        custom_vocabulary,
        mode,
        diarization,
        word_timestamps,
        max_duration_seconds,
        route,
    })
}

pub fn validate_duration(duration_seconds: f64, config: &EffectiveGeminiConfig) -> Result<(), String> {
    if !duration_seconds.is_finite() || duration_seconds < 0.0 {
        return Err("Decoded Gemini audio duration is invalid.".to_string());
    }
    if duration_seconds <= config.max_duration_seconds as f64 {
        return Ok(());
    }
    let reason = if config.word_timestamps && config.diarization {
        "word timestamps and speaker diarization"
    } else if config.word_timestamps {
        "word timestamps required by SRT/VTT output"
    } else if config.diarization {
        "speaker diarization"
    } else {
        "the standard Gemini file limit"
    };
    Err(format!(
        "The decoded audio is {:.2} minutes, exceeding the {:.0}-minute Gemini limit for {}. The file was not sent.",
        duration_seconds / 60.0,
        config.max_duration_seconds as f64 / 60.0,
        reason
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposed_locales_are_exact_and_unique() {
        let mut sorted = GEMINI_SUPPORTED_LOCALES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), GEMINI_SUPPORTED_LOCALES.len());
        assert!(GEMINI_SUPPORTED_LOCALES.iter().all(|locale| is_supported_exact_locale(locale)));
    }

    #[test]
    fn frontend_locale_catalog_matches_backend_exactly() {
        let frontend = include_str!("../../src/lib/gemini/geminiConfig.ts");
        let catalog = frontend
            .split_once("export const GEMINI_LOCALES = [")
            .and_then(|(_, tail)| tail.split_once("] as const;"))
            .map(|(catalog, _)| catalog)
            .expect("frontend Gemini locale catalog markers must remain present");
        let mut frontend_locales = catalog
            .split("],")
            .filter_map(|entry| entry.rsplit_once("\", \"").map(|(_, code)| code))
            .map(|code| {
                code.trim()
                    .trim_end_matches(',')
                    .trim_end_matches(']')
                    .trim_end_matches('"')
                    .to_string()
            })
            .filter(|code| !code.is_empty())
            .collect::<Vec<_>>();
        frontend_locales.sort_unstable();

        let mut backend_locales = GEMINI_SUPPORTED_LOCALES
            .iter()
            .map(|locale| locale.to_string())
            .collect::<Vec<_>>();
        backend_locales.sort_unstable();
        assert_eq!(frontend_locales, backend_locales);
    }

    #[test]
    fn os_mapping_falls_back_when_region_is_ambiguous() {
        assert_eq!(map_os_locale("ru"), Some("ru-RU"));
        assert_eq!(map_os_locale("en"), None);
        assert_eq!(map_os_locale("en-GB"), Some("en-GB"));
    }

    #[test]
    fn duration_boundaries_are_inclusive() {
        let config = EffectiveGeminiConfig {
            language_code: None,
            language_warning: None,
            custom_vocabulary: vec![],
            mode: GeminiTranscriptionMode::Verbatim,
            diarization: true,
            word_timestamps: false,
            max_duration_seconds: GEMINI_FILE_ANNOTATED_MAX_SECONDS as u64,
            route: GeminiRoute::GoogleDirect,
        };
        assert!(validate_duration(1_800.0, &config).is_ok());
        assert!(validate_duration(1_800.001, &config).is_err());
    }

    #[test]
    fn profile_null_inherits_and_empty_vocabulary_replaces_global() {
        let mut settings = crate::settings::get_default_settings();
        settings.remote_stt.provider_preset = "google".to_string();
        settings.gemini_language_code = "ru-RU".to_string();
        settings.gemini_custom_vocabulary = vec!["Global".to_string()];
        let mut profile: TranscriptionProfile = serde_json::from_value(serde_json::json!({
            "id": "profile_test",
            "name": "Test",
            "language": "auto",
            "translate_to_english": false
        })).unwrap();

        let inherited = resolve_effective_config(
            &settings,
            Some(&profile),
            GeminiWorkflow::Live,
            None,
        ).unwrap();
        assert_eq!(inherited.language_code.as_deref(), Some("ru-RU"));
        assert_eq!(inherited.custom_vocabulary, vec!["Global"]);

        profile.gemini_custom_vocabulary_override = Some(vec![]);
        let cleared = resolve_effective_config(
            &settings,
            Some(&profile),
            GeminiWorkflow::Live,
            None,
        ).unwrap();
        assert!(cleared.custom_vocabulary.is_empty());
    }

    #[test]
    fn compatibility_rules_reject_contradictions() {
        assert!(validate_compatibility(
            GeminiWorkflow::File { word_timestamps: true },
            GeminiRoute::GoogleDirect,
            GeminiTranscriptionMode::Smart,
            false,
        ).is_err());
        assert!(validate_compatibility(
            GeminiWorkflow::File { word_timestamps: false },
            GeminiRoute::GoogleDirect,
            GeminiTranscriptionMode::Smart,
            true,
        ).is_err());
        assert!(validate_compatibility(
            GeminiWorkflow::File { word_timestamps: false },
            GeminiRoute::VercelGateway,
            GeminiTranscriptionMode::Verbatim,
            true,
        ).is_err());
    }

    #[test]
    fn vocabulary_limits_are_authoritative() {
        assert!(validate_vocabulary(&vec!["term".to_string(); 1_001]).is_err());
        assert!(validate_vocabulary(&["".to_string()]).is_err());
        assert!(validate_vocabulary(&["Same".to_string(), "Same".to_string()]).is_err());
    }
}
