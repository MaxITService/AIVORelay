//! Confidence-gated text-based language identification.
//!
//! This is last-resort evidence for filler-word removal when neither the user
//! nor the transcription provider identifies the output language. Detection
//! fails closed because accepting the wrong language can delete real words.

use whatlang::{Detector, Lang};

const MIN_CONFIDENCE: f64 = 0.9;

fn whatlang_lang_for_model_code(code: &str) -> Option<Lang> {
    let primary = code
        .trim()
        .split(&['-', '_'][..])
        .next()?
        .to_ascii_lowercase();

    if primary == "zh" {
        return Some(Lang::Cmn);
    }

    let language = match primary.len() {
        2 => isolang::Language::from_639_1(&primary)?,
        3 => isolang::Language::from_639_3(&primary)?,
        _ => return None,
    };

    Lang::from_code(language.to_639_3())
}

fn iso639_1_for_whatlang(lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::Cmn => Some("zh"),
        other => isolang::Language::from_639_3(other.code())?.to_639_1(),
    }
}

/// Detect an ISO 639-1 output language with a strict confidence gate.
///
/// When model metadata is available, detection is constrained to languages
/// that the model can produce. Missing metadata permits unconstrained
/// detection; wholly unrepresentable metadata fails closed.
pub fn detect_output_language(text: &str, supported_languages: &[String]) -> Option<String> {
    let allowlist: Vec<Lang> = supported_languages
        .iter()
        .filter_map(|code| whatlang_lang_for_model_code(code))
        .collect();

    let detector = if supported_languages.is_empty() {
        Detector::new()
    } else if allowlist.is_empty() {
        return None;
    } else {
        Detector::with_allowlist(allowlist)
    };

    let info = detector.detect(text)?;
    if !info.is_reliable() || info.confidence() < MIN_CONFIDENCE {
        return None;
    }

    iso639_1_for_whatlang(info.lang()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(codes: &[&str]) -> Vec<String> {
        codes.iter().map(|code| code.to_string()).collect()
    }

    #[test]
    fn detects_portuguese_sentence_containing_um() {
        let detected = detect_output_language(
            "eu vi um carro na rua ontem de manhã quando fui ao mercado",
            &langs(&["en", "pt", "es"]),
        );
        assert_eq!(detected.as_deref(), Some("pt"));
    }

    #[test]
    fn short_ambiguous_text_returns_none() {
        assert_eq!(detect_output_language("um ok", &langs(&["en", "pt"])), None);
    }

    #[test]
    fn normalizes_model_language_codes() {
        assert_eq!(whatlang_lang_for_model_code("pt-BR"), Some(Lang::Por));
        assert_eq!(whatlang_lang_for_model_code("PT_br"), Some(Lang::Por));
        assert_eq!(whatlang_lang_for_model_code("eng"), Some(Lang::Eng));
        assert_eq!(whatlang_lang_for_model_code("zh-Hant"), Some(Lang::Cmn));
    }

    #[test]
    fn unmappable_codes_do_not_disable_other_languages() {
        let detected = detect_output_language(
            "um so the weather forecast said it would probably rain throughout the whole weekend",
            &langs(&["zh", "yue", "en", "ja", "ko"]),
        );
        assert_eq!(detected.as_deref(), Some("en"));
    }

    #[test]
    fn fully_unmappable_metadata_fails_closed() {
        assert_eq!(
            detect_output_language(
                "eu vi um carro na rua ontem de manhã quando fui ao mercado",
                &langs(&["yue"]),
            ),
            None
        );
    }
}
