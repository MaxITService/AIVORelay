//! ElevenLabs Scribe v3 (`scribe_v3_live` and `scribe_v3`) language normalization and allowlist.
//!
//! Source of truth: ElevenLabs Speech to Text official documentation
//! - <https://elevenlabs.io/docs/overview/capabilities/speech-to-text#supported-languages>
//! - <https://elevenlabs.io/docs/eleven-api/guides/how-to/speech-to-text/realtime/language-options>
//! - <https://elevenlabs.io/docs/eleven-api/guides/how-to/speech-to-text/batch/number-and-language-options>
//!
//! ElevenLabs Scribe accepts ISO 639-1 or ISO 639-3 codes in `language_code`.
//! AivoRelay canonicalizes supported languages to their two-letter ISO 639-1 code when one exists
//! and to their three-letter ISO 639-3 code otherwise (`ast`, `ceb`, `fil`, `kea`, `luo`, `nso`,
//! `umb`, `yue`). Unsupported languages resolve to `None` (`Unsupported`) so requests omit
//! `language_code` and rely on Scribe auto-detection rather than sending an unsupported code.

use std::collections::HashSet;
use std::sync::LazyLock;

/// Canonical language codes officially documented as supported by ElevenLabs Scribe v3 / Scribe v3 Live.
pub const ELEVENLABS_SCRIBE_SUPPORTED_LANGUAGE_CODES: &[&str] = &[
    "af",  // Afrikaans (afr)
    "am",  // Amharic (amh)
    "ar",  // Arabic (ara)
    "as",  // Assamese (asm)
    "ast", // Asturian (ast)
    "az",  // Azerbaijani (aze)
    "be",  // Belarusian (bel)
    "bg",  // Bulgarian (bul)
    "bn",  // Bengali (ben)
    "bs",  // Bosnian (bos)
    "ca",  // Catalan (cat)
    "ceb", // Cebuano (ceb)
    "cs",  // Czech (ces)
    "cy",  // Welsh (cym)
    "da",  // Danish (dan)
    "de",  // German (deu)
    "el",  // Greek (ell)
    "en",  // English (eng)
    "es",  // Spanish (spa)
    "et",  // Estonian (est)
    "fa",  // Persian (fas)
    "ff",  // Fulah (ful)
    "fi",  // Finnish (fin)
    "fil", // Filipino / Tagalog (fil)
    "fr",  // French (fra)
    "ga",  // Irish (gle)
    "gl",  // Galician (glg)
    "gu",  // Gujarati (guj)
    "ha",  // Hausa (hau)
    "he",  // Hebrew (heb)
    "hi",  // Hindi (hin)
    "hr",  // Croatian (hrv)
    "hu",  // Hungarian (hun)
    "hy",  // Armenian (hye)
    "id",  // Indonesian (ind)
    "ig",  // Igbo (ibo)
    "is",  // Icelandic (isl)
    "it",  // Italian (ita)
    "ja",  // Japanese (jpn)
    "jv",  // Javanese (jav)
    "ka",  // Georgian (kat)
    "kea", // Kabuverdianu (kea)
    "kk",  // Kazakh (kaz)
    "km",  // Khmer (khm)
    "kn",  // Kannada (kan)
    "ko",  // Korean (kor)
    "ku",  // Kurdish (kur)
    "ky",  // Kyrgyz (kir)
    "lb",  // Luxembourgish (ltz)
    "lg",  // Ganda (lug)
    "ln",  // Lingala (lin)
    "lo",  // Lao (lao)
    "lt",  // Lithuanian (lit)
    "luo", // Luo (luo)
    "lv",  // Latvian (lav)
    "mi",  // Māori (mri)
    "mk",  // Macedonian (mkd)
    "ml",  // Malayalam (mal)
    "mn",  // Mongolian (mon)
    "mr",  // Marathi (mar)
    "ms",  // Malay (msa)
    "mt",  // Maltese (mlt)
    "my",  // Burmese (mya)
    "ne",  // Nepali (nep)
    "nl",  // Dutch (nld)
    "no",  // Norwegian (nor)
    "nso", // Northern Sotho (nso)
    "ny",  // Chichewa (nya)
    "oc",  // Occitan (oci)
    "or",  // Odia (ori)
    "pa",  // Punjabi (pan)
    "pl",  // Polish (pol)
    "ps",  // Pashto (pus)
    "pt",  // Portuguese (por)
    "ro",  // Romanian (ron)
    "ru",  // Russian (rus)
    "sd",  // Sindhi (snd)
    "si",  // Sinhala (sin)
    "sk",  // Slovak (slk)
    "sl",  // Slovenian (slv)
    "sn",  // Shona (sna)
    "so",  // Somali (som)
    "sr",  // Serbian (srp)
    "sv",  // Swedish (swe)
    "sw",  // Swahili (swa)
    "ta",  // Tamil (tam)
    "te",  // Telugu (tel)
    "tg",  // Tajik (tgk)
    "th",  // Thai (tha)
    "tr",  // Turkish (tur)
    "uk",  // Ukrainian (ukr)
    "umb", // Umbundu (umb)
    "ur",  // Urdu (urd)
    "uz",  // Uzbek (uzb)
    "vi",  // Vietnamese (vie)
    "wo",  // Wolof (wol)
    "xh",  // Xhosa (xho)
    "yo",  // Yoruba (yor)
    "yue", // Cantonese (yue)
    "zh",  // Mandarin Chinese (zho / cmn)
    "zu",  // Zulu (zul)
];

static ELEVENLABS_SUPPORTED_LANGUAGE_SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ELEVENLABS_SCRIBE_SUPPORTED_LANGUAGE_CODES
        .iter()
        .copied()
        .collect()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElevenLabsLanguageResolutionStatus {
    Supported,
    AutoOrEmpty,
    OsInputUnavailable,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevenLabsLanguageResolution {
    pub code: Option<String>,
    pub status: ElevenLabsLanguageResolutionStatus,
    pub normalized: Option<String>,
    pub original: Option<String>,
}

fn map_alias_or_iso639_3_to_canonical(primary: &str) -> &str {
    match primary {
        // Legacy / AivoRelay / Norwegian / Filipino / Javanese / Hebrew / Chinese aliases
        "nb" | "nn" | "nob" | "nno" | "nor" => "no",
        "tl" | "tgl" => "fil",
        "jw" | "jav" => "jv",
        "iw" | "heb" => "he",
        "in" | "ind" => "id",
        "ji" | "yid" => "yi",
        "zho" | "cmn" | "chi" => "zh",
        // Official ElevenLabs ISO 639-3 codes -> canonical ISO 639-1 codes
        "afr" => "af",
        "amh" => "am",
        "ara" => "ar",
        "asm" => "as",
        "aze" => "az",
        "bel" => "be",
        "ben" => "bn",
        "bos" => "bs",
        "bul" => "bg",
        "cat" => "ca",
        "ces" | "cze" => "cs",
        "cym" | "wel" => "cy",
        "dan" => "da",
        "deu" | "ger" => "de",
        "ell" | "gre" => "el",
        "eng" => "en",
        "est" => "et",
        "fas" | "per" => "fa",
        "fin" => "fi",
        "fra" | "fre" => "fr",
        "ful" => "ff",
        "gle" => "ga",
        "glg" => "gl",
        "guj" => "gu",
        "hau" => "ha",
        "hin" => "hi",
        "hrv" => "hr",
        "hun" => "hu",
        "hye" | "arm" => "hy",
        "ibo" => "ig",
        "isl" | "ice" => "is",
        "ita" => "it",
        "jpn" => "ja",
        "kan" => "kn",
        "kat" | "geo" => "ka",
        "kaz" => "kk",
        "khm" => "km",
        "kir" => "ky",
        "kor" => "ko",
        "kur" => "ku",
        "lao" => "lo",
        "lav" => "lv",
        "lin" => "ln",
        "lit" => "lt",
        "ltz" => "lb",
        "lug" => "lg",
        "mal" => "ml",
        "mar" => "mr",
        "mkd" | "mac" => "mk",
        "mlt" => "mt",
        "mon" => "mn",
        "mri" | "mao" => "mi",
        "msa" | "may" => "ms",
        "mya" | "bur" => "my",
        "nep" => "ne",
        "nld" | "dut" => "nl",
        "nya" => "ny",
        "oci" => "oc",
        "ori" => "or",
        "pan" => "pa",
        "pol" => "pl",
        "por" => "pt",
        "pus" => "ps",
        "ron" | "rum" => "ro",
        "rus" => "ru",
        "sin" => "si",
        "slk" | "slo" => "sk",
        "slv" => "sl",
        "sna" => "sn",
        "snd" => "sd",
        "som" => "so",
        "spa" => "es",
        "srp" => "sr",
        "swa" => "sw",
        "swe" => "sv",
        "tam" => "ta",
        "tel" => "te",
        "tgk" => "tg",
        "tha" => "th",
        "tur" => "tr",
        "ukr" => "uk",
        "urd" => "ur",
        "uzb" => "uz",
        "vie" => "vi",
        "wol" => "wo",
        "xho" => "xh",
        "yor" => "yo",
        "zul" => "zu",
        other => other,
    }
}

/// Normalizes a raw language tag or alias into its canonical code.
///
/// Returns `None` for empty inputs, `"auto"`, `"os_input"`, or malformed tags without a primary
/// subtag. Call [`is_supported_elevenlabs_language`] or [`resolve_requested_language_for_elevenlabs`]
/// to verify that the normalized code is in the ElevenLabs Scribe allowlist.
pub fn normalize_elevenlabs_language(raw: &str) -> Option<String> {
    let mut normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized == "auto" || normalized == "os_input" {
        return None;
    }

    normalized = normalized.replace('_', "-");
    if normalized == "zh-hans" || normalized == "zh-hant" {
        return Some("zh".to_string());
    }
    if normalized == "zh-yue" || normalized == "yue-hant" || normalized == "yue-hans" {
        return Some("yue".to_string());
    }

    let primary = normalized.split('-').next().unwrap_or_default().trim();
    if primary.is_empty() {
        return None;
    }

    let canonical = map_alias_or_iso639_3_to_canonical(primary);
    Some(canonical.to_string())
}

/// Returns `true` when `raw` normalizes to an officially supported ElevenLabs Scribe language code.
pub fn is_supported_elevenlabs_language(raw: &str) -> bool {
    normalize_elevenlabs_language(raw)
        .is_some_and(|code| ELEVENLABS_SUPPORTED_LANGUAGE_SET.contains(code.as_str()))
}

fn resolve_requested_language_for_elevenlabs_with_os_input(
    language: Option<&str>,
    os_input_resolver: impl FnOnce() -> Option<String>,
) -> ElevenLabsLanguageResolution {
    let original = language
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let Some(raw) = original.clone() else {
        return ElevenLabsLanguageResolution {
            code: None,
            status: ElevenLabsLanguageResolutionStatus::AutoOrEmpty,
            normalized: None,
            original,
        };
    };

    if raw.eq_ignore_ascii_case("auto") {
        return ElevenLabsLanguageResolution {
            code: None,
            status: ElevenLabsLanguageResolutionStatus::AutoOrEmpty,
            normalized: None,
            original: Some(raw),
        };
    }

    let resolved_raw = if raw.eq_ignore_ascii_case("os_input") {
        match os_input_resolver() {
            Some(os_language) if !os_language.trim().is_empty() => os_language,
            _ => {
                return ElevenLabsLanguageResolution {
                    code: None,
                    status: ElevenLabsLanguageResolutionStatus::OsInputUnavailable,
                    normalized: None,
                    original: Some(raw),
                };
            }
        }
    } else {
        raw.clone()
    };

    // If a comma-separated list of languages is supplied (e.g. "en,la"), pick the first
    // supported language if any, or report the first normalized code as Unsupported.
    if resolved_raw.contains(',') {
        let parts: Vec<&str> = resolved_raw
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();
        if parts.is_empty() {
            return ElevenLabsLanguageResolution {
                code: None,
                status: ElevenLabsLanguageResolutionStatus::AutoOrEmpty,
                normalized: None,
                original: Some(raw),
            };
        }
        let mut first_normalized = None;
        for part in parts {
            if part.eq_ignore_ascii_case("auto") {
                return ElevenLabsLanguageResolution {
                    code: None,
                    status: ElevenLabsLanguageResolutionStatus::AutoOrEmpty,
                    normalized: None,
                    original: Some(raw),
                };
            }
            if let Some(code) = normalize_elevenlabs_language(part) {
                if ELEVENLABS_SUPPORTED_LANGUAGE_SET.contains(code.as_str()) {
                    return ElevenLabsLanguageResolution {
                        code: Some(code.clone()),
                        status: ElevenLabsLanguageResolutionStatus::Supported,
                        normalized: Some(code),
                        original: Some(raw),
                    };
                }
                if first_normalized.is_none() {
                    first_normalized = Some(code);
                }
            }
        }
        return ElevenLabsLanguageResolution {
            code: None,
            status: ElevenLabsLanguageResolutionStatus::Unsupported,
            normalized: first_normalized,
            original: Some(raw),
        };
    }

    let normalized = normalize_elevenlabs_language(&resolved_raw);
    let Some(code) = normalized.clone() else {
        return ElevenLabsLanguageResolution {
            code: None,
            status: ElevenLabsLanguageResolutionStatus::Unsupported,
            normalized,
            original: Some(raw),
        };
    };

    if !ELEVENLABS_SUPPORTED_LANGUAGE_SET.contains(code.as_str()) {
        return ElevenLabsLanguageResolution {
            code: None,
            status: ElevenLabsLanguageResolutionStatus::Unsupported,
            normalized: Some(code),
            original: Some(raw),
        };
    }

    ElevenLabsLanguageResolution {
        code: Some(code.clone()),
        status: ElevenLabsLanguageResolutionStatus::Supported,
        normalized: Some(code),
        original: Some(raw),
    }
}

/// Resolves a user- or profile-configured language (`None`, `"auto"`, `"os_input"`, BCP-47 tag,
/// or ISO 639-1/639-3 code) for ElevenLabs Scribe (`scribe_v3_live` and `scribe_v3`).
pub fn resolve_requested_language_for_elevenlabs(
    language: Option<&str>,
) -> ElevenLabsLanguageResolution {
    resolve_requested_language_for_elevenlabs_with_os_input(language, || {
        crate::input_source::get_language_from_input_source()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_elevenlabs_catalog_is_sorted_unique_and_non_empty() {
        assert_eq!(ELEVENLABS_SCRIBE_SUPPORTED_LANGUAGE_CODES.len(), 101);
        let mut sorted = ELEVENLABS_SCRIBE_SUPPORTED_LANGUAGE_CODES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, ELEVENLABS_SCRIBE_SUPPORTED_LANGUAGE_CODES);
    }

    #[test]
    fn normalizes_aliases_bcp47_locales_and_iso639_3_codes() {
        assert_eq!(normalize_elevenlabs_language("en-US"), Some("en".to_string()));
        assert_eq!(normalize_elevenlabs_language("pt_BR"), Some("pt".to_string()));
        assert_eq!(normalize_elevenlabs_language("zh-Hans"), Some("zh".to_string()));
        assert_eq!(normalize_elevenlabs_language("zh_hant"), Some("zh".to_string()));
        assert_eq!(normalize_elevenlabs_language("no"), Some("no".to_string()));
        assert_eq!(normalize_elevenlabs_language("nb"), Some("no".to_string()));
        assert_eq!(normalize_elevenlabs_language("nn-NO"), Some("no".to_string()));
        assert_eq!(normalize_elevenlabs_language("tl"), Some("fil".to_string()));
        assert_eq!(normalize_elevenlabs_language("fil"), Some("fil".to_string()));
        assert_eq!(normalize_elevenlabs_language("jw"), Some("jv".to_string()));
        assert_eq!(normalize_elevenlabs_language("jv"), Some("jv".to_string()));
        assert_eq!(normalize_elevenlabs_language("iw"), Some("he".to_string()));
        assert_eq!(normalize_elevenlabs_language("he"), Some("he".to_string()));
        assert_eq!(normalize_elevenlabs_language("yue"), Some("yue".to_string()));
        assert_eq!(normalize_elevenlabs_language("eng"), Some("en".to_string()));
        assert_eq!(normalize_elevenlabs_language("zho"), Some("zh".to_string()));
        assert_eq!(normalize_elevenlabs_language("cmn"), Some("zh".to_string()));
        assert_eq!(normalize_elevenlabs_language("auto"), None);
        assert_eq!(normalize_elevenlabs_language("os_input"), None);
        assert_eq!(normalize_elevenlabs_language("   "), None);
        assert_eq!(normalize_elevenlabs_language("-Latn"), None);
    }

    #[test]
    fn allowlist_accepts_documented_languages_and_rejects_unsupported_ones() {
        for supported in [
            "en", "ru", "zh-Hans", "zh-Hant", "no", "nb", "nn", "tl", "fil", "jw", "jv", "iw",
            "he", "yue", "ceb", "ast", "kea", "luo", "nso", "umb", "yo", "zu", "eng", "de-DE",
        ] {
            assert!(
                is_supported_elevenlabs_language(supported),
                "expected {supported} to be supported"
            );
        }

        for unsupported in [
            "la", "eu", "sq", "ht", "haw", "fo", "br", "ba", "bo", "mg", "sa", "su", "tk", "tt",
            "yi", "xx",
        ] {
            assert!(
                !is_supported_elevenlabs_language(unsupported),
                "expected {unsupported} to be unsupported"
            );
        }
    }

    #[test]
    fn resolves_none_blank_and_auto_to_auto_or_empty() {
        for input in [None, Some(""), Some("   ")] {
            let res = resolve_requested_language_for_elevenlabs_with_os_input(input, || {
                unreachable!("OS input should not be queried")
            });
            assert_eq!(res.status, ElevenLabsLanguageResolutionStatus::AutoOrEmpty);
            assert_eq!(res.code, None);
            assert_eq!(res.normalized, None);
            assert_eq!(res.original, None);
        }

        let auto = resolve_requested_language_for_elevenlabs_with_os_input(Some(" Auto "), || {
            unreachable!("OS input should not be queried")
        });
        assert_eq!(auto.status, ElevenLabsLanguageResolutionStatus::AutoOrEmpty);
        assert_eq!(auto.code, None);
        assert_eq!(auto.normalized, None);
        assert_eq!(auto.original, Some("Auto".to_string()));
    }

    #[test]
    fn resolves_supported_and_unsupported_languages_honestly() {
        let supported = resolve_requested_language_for_elevenlabs_with_os_input(
            Some("zh-Hant"),
            || unreachable!(),
        );
        assert_eq!(
            supported.status,
            ElevenLabsLanguageResolutionStatus::Supported
        );
        assert_eq!(supported.code.as_deref(), Some("zh"));
        assert_eq!(supported.normalized.as_deref(), Some("zh"));
        assert_eq!(supported.original.as_deref(), Some("zh-Hant"));

        let unsupported =
            resolve_requested_language_for_elevenlabs_with_os_input(Some("la"), || unreachable!());
        assert_eq!(
            unsupported.status,
            ElevenLabsLanguageResolutionStatus::Unsupported
        );
        assert_eq!(unsupported.code, None);
        assert_eq!(unsupported.normalized.as_deref(), Some("la"));
        assert_eq!(unsupported.original.as_deref(), Some("la"));
    }

    #[test]
    fn resolves_os_input_through_injected_detector() {
        let available = resolve_requested_language_for_elevenlabs_with_os_input(
            Some("os_input"),
            || Some("nb-NO".to_string()),
        );
        assert_eq!(
            available.status,
            ElevenLabsLanguageResolutionStatus::Supported
        );
        assert_eq!(available.code.as_deref(), Some("no"));
        assert_eq!(available.normalized.as_deref(), Some("no"));
        assert_eq!(available.original.as_deref(), Some("os_input"));

        let unavailable =
            resolve_requested_language_for_elevenlabs_with_os_input(Some("os_input"), || None);
        assert_eq!(
            unavailable.status,
            ElevenLabsLanguageResolutionStatus::OsInputUnavailable
        );
        assert_eq!(unavailable.code, None);
        assert_eq!(unavailable.normalized, None);
        assert_eq!(unavailable.original.as_deref(), Some("os_input"));

        let unsupported_os = resolve_requested_language_for_elevenlabs_with_os_input(
            Some("os_input"),
            || Some("eu-ES".to_string()),
        );
        assert_eq!(
            unsupported_os.status,
            ElevenLabsLanguageResolutionStatus::Unsupported
        );
        assert_eq!(unsupported_os.code, None);
        assert_eq!(unsupported_os.normalized.as_deref(), Some("eu"));
        assert_eq!(unsupported_os.original.as_deref(), Some("os_input"));
    }


}
