use std::collections::HashSet;
use std::sync::LazyLock;

static SONIOX_SUPPORTED_LANGUAGE_CODES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "af", "sq", "ar", "az", "eu", "be", "bn", "bs", "bg", "ca", "zh", "hr", "cs", "da",
        "nl", "en", "et", "fi", "fr", "gl", "de", "el", "gu", "he", "hi", "hu", "id", "it",
        "ja", "kn", "kk", "ko", "lv", "lt", "mk", "ms", "ml", "mr", "no", "fa", "pl", "pt",
        "pa", "ro", "ru", "sr", "sk", "sl", "es", "sw", "sv", "tl", "ta", "te", "th", "tr",
        "uk", "ur", "vi", "cy",
    ]
    .into_iter()
    .collect()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SonioxLanguageResolutionStatus {
    Supported,
    AutoOrEmpty,
    OsInputUnavailable,
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct SonioxLanguageResolution {
    pub hint: Option<String>,
    pub status: SonioxLanguageResolutionStatus,
    pub normalized: Option<String>,
    pub original: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SonioxHintListNormalization {
    pub normalized: Vec<String>,
    pub rejected: Vec<String>,
}

fn canonicalize_language_code(raw: &str) -> Option<String> {
    let mut normalized = raw.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }

    normalized = normalized.replace('_', "-");
    if normalized == "zh-hans" || normalized == "zh-hant" {
        return Some("zh".to_string());
    }

    let primary = normalized.split('-').next().unwrap_or("").trim();
    if primary.is_empty() {
        return None;
    }

    Some(primary.to_string())
}

pub fn is_soniox_supported_language(code: &str) -> bool {
    let normalized = code.trim().to_lowercase();
    SONIOX_SUPPORTED_LANGUAGE_CODES.contains(normalized.as_str())
}

pub fn normalize_soniox_hint_list<I, S>(hints: I) -> SonioxHintListNormalization
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    let mut rejected = Vec::new();

    for hint in hints {
        let raw = hint.as_ref().trim();
        if raw.is_empty() {
            continue;
        }

        let canonical = canonicalize_language_code(raw);
        match canonical {
            Some(code) if is_soniox_supported_language(&code) => {
                if seen.insert(code.clone()) {
                    normalized.push(code);
                }
            }
            _ => rejected.push(raw.to_string()),
        }
    }

    SonioxHintListNormalization { normalized, rejected }
}

pub fn resolve_requested_language_for_soniox(language: Option<&str>) -> SonioxLanguageResolution {
    let original = language
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let Some(raw) = original.clone() else {
        return SonioxLanguageResolution {
            hint: None,
            status: SonioxLanguageResolutionStatus::AutoOrEmpty,
            normalized: None,
            original,
        };
    };

    if raw.eq_ignore_ascii_case("auto") {
        return SonioxLanguageResolution {
            hint: None,
            status: SonioxLanguageResolutionStatus::AutoOrEmpty,
            normalized: None,
            original: Some(raw),
        };
    }

    let resolved_raw = if raw.eq_ignore_ascii_case("os_input") {
        match crate::input_source::get_language_from_input_source() {
            Some(os_language) if !os_language.trim().is_empty() => os_language,
            _ => {
                return SonioxLanguageResolution {
                    hint: None,
                    status: SonioxLanguageResolutionStatus::OsInputUnavailable,
                    normalized: None,
                    original: Some(raw),
                };
            }
        }
    } else {
        raw.clone()
    };

    let normalized = canonicalize_language_code(&resolved_raw);
    let Some(code) = normalized.clone() else {
        return SonioxLanguageResolution {
            hint: None,
            status: SonioxLanguageResolutionStatus::Unsupported,
            normalized,
            original: Some(raw),
        };
    };

    if !is_soniox_supported_language(&code) {
        return SonioxLanguageResolution {
            hint: None,
            status: SonioxLanguageResolutionStatus::Unsupported,
            normalized: Some(code),
            original: Some(raw),
        };
    }

    SonioxLanguageResolution {
        hint: Some(code.clone()),
        status: SonioxLanguageResolutionStatus::Supported,
        normalized: Some(code),
        original: Some(raw),
    }
}
