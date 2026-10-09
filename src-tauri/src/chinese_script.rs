//! Simplified / Traditional script handling for Chinese transcription output.
//!
//! Recognition language (Mandarin vs Cantonese) and output script are separate
//! choices: the language picks what the model listens for, and the
//! [`ChineseScript`] setting picks how the resulting text is written. Conversion
//! is keyed off the language the output is known to be in, so mixed-language
//! users on auto-detect only get Chinese output rewritten.

use crate::settings::ChineseScript;
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::error;
use std::sync::OnceLock;

/// The Chinese language a transcription was produced in. Picks the regional
/// conversion tables: Taiwan for Mandarin, Hong Kong for Cantonese.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChineseVariety {
    Mandarin,
    Cantonese,
}

impl ChineseVariety {
    /// Maps an output language code (`zh`, `zh-CN`, `yue`, …) to a Chinese
    /// variety. Any other language returns `None` and is never converted.
    pub fn from_language(language: &str) -> Option<Self> {
        let base = language.split(&['-', '_'][..]).next()?.to_ascii_lowercase();
        match base.as_str() {
            "zh" => Some(Self::Mandarin),
            "yue" => Some(Self::Cantonese),
            _ => None,
        }
    }
}

/// The script a locale writes Chinese in, or `None` for non-Chinese locales.
///
/// `zh` defaults to Simplified unless tagged Hant or regioned to Taiwan, Hong
/// Kong or Macau; Cantonese (`yue`) defaults to Traditional unless tagged Hans.
pub fn chinese_script_for_locale(locale: &str) -> Option<ChineseScript> {
    let normalized = locale.to_lowercase().replace('_', "-");
    let subtags: Vec<_> = normalized.split('-').collect();
    let is_hant = subtags.contains(&"hant");
    let is_hans = subtags.contains(&"hans");
    let is_traditional_region = ["tw", "hk", "mo"]
        .iter()
        .any(|region| subtags.contains(region));

    let traditional = match subtags.first().copied() {
        Some("zh") => is_hant || (!is_hans && is_traditional_region),
        Some("yue") => !is_hans,
        _ => return None,
    };
    Some(if traditional {
        ChineseScript::Traditional
    } else {
        ChineseScript::Simplified
    })
}

/// The OpenCC converter for a variety/script pair, built on first use and
/// reused after. Building one parses its dictionaries, which is too slow to
/// repeat on every live-preview update. A failed build is cached as `None`.
fn converter(variety: ChineseVariety, script: ChineseScript) -> Option<&'static OpenCC> {
    static MANDARIN_SIMPLIFIED: OnceLock<Option<OpenCC>> = OnceLock::new();
    static MANDARIN_TRADITIONAL: OnceLock<Option<OpenCC>> = OnceLock::new();
    static CANTONESE_SIMPLIFIED: OnceLock<Option<OpenCC>> = OnceLock::new();
    static CANTONESE_TRADITIONAL: OnceLock<Option<OpenCC>> = OnceLock::new();

    let (cell, config) = match (variety, script) {
        (_, ChineseScript::AsTranscribed) => return None,
        (ChineseVariety::Mandarin, ChineseScript::Simplified) => {
            (&MANDARIN_SIMPLIFIED, BuiltinConfig::Tw2sp)
        }
        (ChineseVariety::Mandarin, ChineseScript::Traditional) => {
            (&MANDARIN_TRADITIONAL, BuiltinConfig::S2twp)
        }
        (ChineseVariety::Cantonese, ChineseScript::Simplified) => {
            (&CANTONESE_SIMPLIFIED, BuiltinConfig::Hk2s)
        }
        (ChineseVariety::Cantonese, ChineseScript::Traditional) => {
            (&CANTONESE_TRADITIONAL, BuiltinConfig::S2hk)
        }
    };

    cell.get_or_init(|| match OpenCC::from_config(config) {
        Ok(converter) => Some(converter),
        Err(e) => {
            error!(
                "Failed to initialize OpenCC converter: {}. Keeping the original script.",
                e
            );
            None
        }
    })
    .as_ref()
}

/// Rewrites `text` into `script`. Fails open: if OpenCC can't be initialized
/// the text is returned unchanged.
pub fn convert_chinese_script(
    text: &str,
    variety: ChineseVariety,
    script: ChineseScript,
) -> String {
    match converter(variety, script) {
        Some(converter) => converter.convert(text),
        None => text.to_string(),
    }
}

/// Resolve a profile override, retaining script intent in old history snapshots.
pub fn resolve_chinese_script(
    settings: &crate::settings::AppSettings,
    profile: Option<&crate::settings::TranscriptionProfile>,
) -> ChineseScript {
    if let Some(profile) = profile {
        return profile.chinese_script
            .or_else(|| legacy_script(&profile.language))
            .unwrap_or(settings.chinese_script);
    }
    legacy_script(&settings.selected_language).unwrap_or(settings.chinese_script)
}

pub fn legacy_script(language: &str) -> Option<ChineseScript> {
    match language.to_ascii_lowercase().as_str() {
        "zh-hans" | "zh_hans" => Some(ChineseScript::Simplified),
        "zh-hant" | "zh_hant" => Some(ChineseScript::Traditional),
        _ => None,
    }
}

/// Only rewrite output whose language is known. Never infer Chinese from Han
/// characters alone: Japanese also uses them, and translation changes language.
pub fn convert_for_output(
    text: &str,
    settings: &crate::settings::AppSettings,
    profile: Option<&crate::settings::TranscriptionProfile>,
    output_language: Option<&str>,
    translated_to_english: bool,
) -> String {
    if translated_to_english {
        return text.to_string();
    }
    match output_language.and_then(ChineseVariety::from_language) {
        Some(variety) => convert_chinese_script(text, variety, resolve_chinese_script(settings, profile)),
        None => text.to_string(),
    }
}

/// Use the fork's existing language evidence, detecting only unknown output
/// with its confidence gate. Multilingual and translated output stay unchanged.
pub fn convert_with_evidence(
    text: &str,
    settings: &crate::settings::AppSettings,
    evidence: &crate::audio_toolkit::OutputLanguageEvidence,
    supported_languages: &[String],
) -> String {
    convert_with_profile_evidence(text, settings, None, evidence, supported_languages)
}

pub fn convert_with_profile_evidence(
    text: &str,
    settings: &crate::settings::AppSettings,
    profile: Option<&crate::settings::TranscriptionProfile>,
    evidence: &crate::audio_toolkit::OutputLanguageEvidence,
    supported_languages: &[String],
) -> String {
    use crate::audio_toolkit::OutputLanguageEvidence;
    if resolve_chinese_script(settings, profile) == ChineseScript::AsTranscribed {
        return text.to_string();
    }
    let detected;
    let language = match evidence {
        OutputLanguageEvidence::UserSelected(language)
        | OutputLanguageEvidence::ModelConstrained(language)
        | OutputLanguageEvidence::ModelDetected(language)
        | OutputLanguageEvidence::TextDetected(language) => Some(language.as_str()),
        OutputLanguageEvidence::Unknown => {
            detected = crate::audio_toolkit::detect_output_language(text, supported_languages);
            detected.as_deref()
        }
        OutputLanguageEvidence::Multilingual | OutputLanguageEvidence::TranslatedToEnglish => None,
    };
    convert_for_output(text, settings, profile, language, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_toolkit::OutputLanguageEvidence;
    use crate::settings::{get_default_settings, TranscriptionProfile};

    fn profile(language: &str, script: Option<ChineseScript>) -> TranscriptionProfile {
        serde_json::from_value(serde_json::json!({
            "id": "test", "name": "Test", "language": language,
            "translate_to_english": false, "llm_post_process_enabled": false,
            "chinese_script": script
        })).unwrap()
    }

    #[test]
    fn chinese_locale_defaults_honor_explicit_script_before_region() {
        for locale in ["zh", "zh-CN", "zh_SG", "zh-Hans-TW", "yue-Hans-HK"] {
            assert_eq!(chinese_script_for_locale(locale), Some(ChineseScript::Simplified), "{locale}");
        }
        for locale in ["zh-TW", "zh_HK", "zh-MO", "zh-Hant-CN", "yue", "yue-HK"] {
            assert_eq!(chinese_script_for_locale(locale), Some(ChineseScript::Traditional), "{locale}");
        }
        for locale in ["en-US", "ja-JP", "", "auto"] {
            assert_eq!(chinese_script_for_locale(locale), None);
        }
    }

    #[test]
    fn profile_script_inherits_only_when_no_explicit_or_legacy_intent_exists() {
        let mut settings = get_default_settings();
        settings.selected_language = "auto".into();
        settings.chinese_script = ChineseScript::Traditional;
        assert_eq!(resolve_chinese_script(&settings, Some(&profile("zh", None))), ChineseScript::Traditional);
        assert_eq!(resolve_chinese_script(&settings, Some(&profile("zh-Hans", None))), ChineseScript::Simplified);
        assert_eq!(resolve_chinese_script(&settings, Some(&profile("zh-Hant", Some(ChineseScript::AsTranscribed)))), ChineseScript::AsTranscribed);
        settings.selected_language = "zh-Hans".into();
        assert_eq!(resolve_chinese_script(&settings, None), ChineseScript::Simplified);
    }

    #[test]
    fn converters_rewrite_both_chinese_varieties_and_are_reused() {
        for variety in [ChineseVariety::Mandarin, ChineseVariety::Cantonese] {
            assert_eq!(convert_chinese_script("学习汉语", variety, ChineseScript::Traditional), "學習漢語");
            assert_eq!(convert_chinese_script("學習漢語", variety, ChineseScript::Simplified), "学习汉语");
            assert_eq!(convert_chinese_script("學習汉语", variety, ChineseScript::AsTranscribed), "學習汉语");
            assert!(std::ptr::eq(
                converter(variety, ChineseScript::Traditional).unwrap(),
                converter(variety, ChineseScript::Traditional).unwrap(),
            ));
        }
    }

    #[test]
    fn conversion_requires_chinese_output_evidence_and_skips_translation_and_multilingual() {
        let mut settings = get_default_settings();
        settings.selected_language = "auto".into();
        settings.chinese_script = ChineseScript::Traditional;
        for evidence in [
            OutputLanguageEvidence::UserSelected("zh".into()),
            OutputLanguageEvidence::ModelConstrained("zh-CN".into()),
            OutputLanguageEvidence::ModelDetected("yue".into()),
            OutputLanguageEvidence::TextDetected("zh".into()),
        ] {
            assert_eq!(convert_with_evidence("学习汉语", &settings, &evidence, &[]), "學習漢語");
        }
        for evidence in [
            OutputLanguageEvidence::UserSelected("ja".into()),
            OutputLanguageEvidence::ModelDetected("en".into()),
            OutputLanguageEvidence::Multilingual,
            OutputLanguageEvidence::TranslatedToEnglish,
            OutputLanguageEvidence::Unknown,
        ] {
            // Four Han characters must not become Chinese evidence merely
            // because they resemble the selected script's source alphabet.
            assert_eq!(convert_with_evidence("学习汉语", &settings, &evidence, &[]), "学习汉语");
        }
        assert_eq!(convert_for_output("学习汉语", &settings, None, Some("zh"), true), "学习汉语");
        settings.chinese_script = ChineseScript::AsTranscribed;
        assert_eq!(convert_with_evidence("学习汉语", &settings, &OutputLanguageEvidence::ModelDetected("zh".into()), &[]), "学习汉语");
    }

    #[test]
    fn unknown_chinese_output_respects_the_detection_gate_and_model_metadata() {
        let mut settings = get_default_settings();
        settings.selected_language = "auto".into();
        settings.chinese_script = ChineseScript::Traditional;
        let text = "这是中文句子";
        assert_eq!(convert_with_evidence(text, &settings, &OutputLanguageEvidence::Unknown, &["zh".into()]), "這是中文句子");
        assert_eq!(convert_with_evidence(text, &settings, &OutputLanguageEvidence::Unknown, &["yue".into()]), text);
        let japanese = "私は日本語を勉強しています。毎日新しい言葉を覚えます。";
        assert_eq!(convert_with_evidence(japanese, &settings, &OutputLanguageEvidence::Unknown, &["ja".into()]), japanese);
    }
}
