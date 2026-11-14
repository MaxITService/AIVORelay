//! Reson8 Rumble-2 language catalog and request-language normalization.
//!
//! The catalog mirrors <https://docs.reson8.dev/speech-to-text/features/languages/>.
//! Recognition identities used by the settings UI (`no`, `tl`, `jw`) are mapped
//! to the concrete codes Rumble-2 accepts (`nb`, `fil`, `jv`). Unsupported
//! choices are omitted so Reson8 falls back to its own auto-detection.

use std::collections::HashSet;

pub(crate) const RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES: [&str; 56] = [
    "af", "ar", "az", "be", "bg", "bn", "bs", "ca", "ceb", "cs", "da", "de", "el", "en", "es",
    "et", "eu", "fa", "fi", "fil", "fr", "fy", "gl", "hi", "hr", "hu", "id", "is", "it", "ja",
    "jv", "ko", "lt", "lv", "mk", "mr", "ms", "mt", "nb", "nl", "pl", "pt", "ro", "ru", "sk", "sl",
    "sq", "sr", "sv", "ta", "th", "tr", "uk", "vi", "yue", "zh",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Reson8LanguageHintListNormalization {
    pub normalized: Vec<String>,
    pub rejected: Vec<String>,
}

/// Canonical Rumble-2 code for a user or OS language identity.
fn canonicalize_language_code(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_lowercase().replace('_', "-");
    if normalized.is_empty() {
        return None;
    }
    if normalized == "zh-hans" || normalized == "zh-hant" {
        return Some("zh".to_string());
    }

    let primary = normalized.split('-').next().unwrap_or("").trim();
    if primary.is_empty() {
        return None;
    }

    let code = match primary {
        "no" => "nb",
        "tl" => "fil",
        "jw" => "jv",
        other => other,
    };
    Some(code.to_string())
}

pub(crate) fn is_reson8_supported_language(code: &str) -> bool {
    canonicalize_language_code(code)
        .is_some_and(|code| RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES.contains(&code.as_str()))
}

pub(crate) fn normalize_reson8_language_hints<I, S>(hints: I) -> Reson8LanguageHintListNormalization
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    let mut rejected = Vec::new();

    for hint in hints {
        let raw = hint.as_ref().trim();
        if raw.is_empty() || raw.eq_ignore_ascii_case("auto") {
            continue;
        }

        match canonicalize_language_code(raw) {
            Some(code) if RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES.contains(&code.as_str()) => {
                if seen.insert(code.clone()) {
                    normalized.push(code);
                }
            }
            _ => rejected.push(raw.to_string()),
        }
    }

    Reson8LanguageHintListNormalization {
        normalized,
        rejected,
    }
}

/// Resolve settings/OS language input into a Reson8 `language` query value.
/// Returns `None` for auto, empty, unavailable OS input, or when no requested
/// language is supported, which lets Reson8 auto-detect.
pub(crate) fn resolve_requested_language_for_reson8(language: Option<&str>) -> Option<String> {
    let raw = language?.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("auto") {
        return None;
    }

    let resolved = if raw.eq_ignore_ascii_case("os_input") {
        crate::input_source::get_language_from_input_source()?
    } else {
        raw.to_string()
    };
    let normalized = normalize_reson8_language_hints(resolved.split(','));
    if normalized.normalized.is_empty() {
        None
    } else {
        Some(normalized.normalized.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn frontend_reson8_language_catalog_matches_backend_exactly() {
        let frontend_source = include_str!("../../src/lib/constants/reson8Languages.ts");
        let catalog_source = frontend_source
            .split_once("export const RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES = new Set<string>([")
            .expect("frontend Reson8 language catalog declaration must exist")
            .1
            .split_once("]);")
            .expect("frontend Reson8 language catalog must have a closing delimiter")
            .0;
        let frontend_codes = catalog_source
            .lines()
            .filter_map(|line| {
                let entry = line.trim().strip_suffix(',')?;
                entry.strip_prefix('"')?.strip_suffix('"')
            })
            .collect::<BTreeSet<_>>();
        let backend_codes = RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();

        assert_eq!(frontend_codes.len(), 56);
        assert_eq!(frontend_codes, backend_codes);
    }

    #[test]
    fn supported_catalog_uses_provider_codes_and_excludes_unsupported_languages() {
        for code in ["nb", "fil", "jv", "ceb", "fy", "zh", "yue"] {
            assert!(is_reson8_supported_language(code), "{code}");
        }
        for code in ["no", "tl", "jw"] {
            assert!(
                is_reson8_supported_language(code),
                "tray identities must match: {code}"
            );
        }
        for code in ["he", "iw", "la", "haw", "xx"] {
            assert!(!is_reson8_supported_language(code), "{code}");
        }
    }

    #[test]
    fn aliases_resolve_to_concrete_rumble_2_codes() {
        assert_eq!(
            resolve_requested_language_for_reson8(Some("no")),
            Some("nb".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("nb-NO")),
            Some("nb".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("tl")),
            Some("fil".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("fil-PH")),
            Some("fil".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("jw")),
            Some("jv".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("jv-ID")),
            Some("jv".to_string())
        );
    }

    #[test]
    fn case_region_and_chinese_variants_normalize_to_supported_codes() {
        assert_eq!(
            resolve_requested_language_for_reson8(Some(" EN-us ")),
            Some("en".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("pt_BR")),
            Some("pt".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("zh-Hans")),
            Some("zh".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("ZH_hant")),
            Some("zh".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("yue-Hant")),
            Some("yue".to_string())
        );
    }

    #[test]
    fn unsupported_or_auto_values_do_not_leak_a_language_hint() {
        assert_eq!(resolve_requested_language_for_reson8(None), None);
        assert_eq!(resolve_requested_language_for_reson8(Some("")), None);
        assert_eq!(resolve_requested_language_for_reson8(Some("   ")), None);
        assert_eq!(resolve_requested_language_for_reson8(Some("auto")), None);
        assert_eq!(resolve_requested_language_for_reson8(Some(" AUTO ")), None);
        assert_eq!(resolve_requested_language_for_reson8(Some("he")), None);
        assert_eq!(resolve_requested_language_for_reson8(Some("iw-IL")), None);
        assert_eq!(resolve_requested_language_for_reson8(Some("xx-YY")), None);
    }

    #[test]
    fn comma_lists_deduplicate_aliases_and_drop_unsupported_entries() {
        let normalized = normalize_reson8_language_hints([
            "en-US", " EN ", "en_GB", "zh-Hans", "ZH_HANT", "no", "nb", "tl", "fil", "jw", "jv",
            "he", "auto", "xx",
        ]);

        assert_eq!(
            normalized.normalized,
            vec!["en", "zh", "nb", "fil", "jv"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            normalized.rejected,
            vec!["he".to_string(), "xx".to_string()]
        );
    }

    #[test]
    fn resolved_lists_join_supported_codes_and_skip_auto_entries() {
        assert_eq!(
            resolve_requested_language_for_reson8(Some(" en-US, auto, he, fr-CA ")),
            Some("en,fr".to_string())
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some(" , auto, , ")),
            None
        );
        assert_eq!(
            resolve_requested_language_for_reson8(Some("no,tl,jw")),
            Some("nb,fil,jv".to_string())
        );
    }
}
