//! The bundled, offline model catalog.
//!
//! `catalog.json` is a verbatim copy of the `@handy-computer/models` package's
//! catalog (the version pinned in `package.json`; `bun run check:catalog`
//! keeps the two identical). It lists every model transcribe.cpp publishes,
//! with pinned revisions, per-quant hashes and benchmark results. It is
//! compiled into the binary so the app ships a complete model list with zero
//! network access.
//!
//! Every supported transcription model is offered for download and mirrored
//! at the package's default quant, independently of the recommended badge.

use std::collections::{BTreeSet, HashMap};

use once_cell::sync::Lazy;
use serde::Deserialize;

/// Catalog schema this loader understands. The package bumps it on any
/// breaking change; additive fields are ignored.
const SCHEMA: u32 = 2;

/// Base URLs tried in order when the Hugging Face download fails. The full
/// file URL is `{mirror}/{repo_id}/{revision}/{filename}` — the same three
/// values that form the HF resolve URL, so a mirror is a plain static host.
const MIRRORS: &[&str] = &["https://blob.handy.computer"];

/// The small "Recommended" badge / onboarding subset, keyed by catalog id.
const RECOMMENDED: &[&str] = &[
    "parakeet-unified-en-0.6b",
    "nemotron-3.5-asr-streaming-0.6b",
    "canary-180m-flash",
    "cohere-transcribe-03-2026",
    "whisper-medium",
];

/// AIVO exposes Whisper translation; keep it when upstream cards omit the flag.
const TRANSLATION_OVERRIDES: &[&str] = &["whisper-large-v3-turbo"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogRoot {
    schema: u32,
    /// The transcribe.cpp release the catalog was built from.
    #[cfg_attr(not(test), allow(dead_code))]
    transcribe_cpp: String,
    models: Vec<RawModel>,
    /// Benchmark results keyed by catalog id.
    #[serde(default)]
    results: HashMap<String, ModelResults>,
}

/// One model as written in `catalog.json`. Only the fields the app needs are
/// declared; serde ignores the rest (license, parameters, …).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawModel {
    /// Stable package id: the HF repo name without `-gguf`.
    id: String,
    name: String,
    /// Editorial copy; empty for models that aren't curated.
    description: String,
    /// HF repo id, e.g. `handy-computer/whisper-small-gguf`.
    repository: String,
    /// Commit the files and hashes are pinned to. Both HF acquisition and
    /// mirror keys use it, so downloaded bytes provably match the hashes.
    revision: String,
    /// The loader architecture (`general.architecture`).
    #[cfg_attr(not(test), allow(dead_code))]
    family: String,
    languages: Vec<String>,
    capabilities: RawCaps,
    /// Key of the default file in `files`.
    quant: String,
    /// `asr`, `langid`, `vad` or `diarize`; only `asr` models transcribe.
    role: String,
    /// Oldest transcribe.cpp release that can load these files.
    #[cfg_attr(not(test), allow(dead_code))]
    min_engine: Option<String>,
    /// Every published file, keyed by quant.
    files: HashMap<String, RawFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCaps {
    streaming: bool,
    translate: bool,
    language_detection: bool,
    #[serde(default)]
    timestamps: bool,
}

#[derive(Deserialize)]
struct RawFile {
    filename: String,
    size: u64,
    sha256: String,
}

#[derive(Deserialize, Default)]
struct ModelResults {
    /// Error percent per language identity, measured at the default quant.
    #[serde(default)]
    accuracy: HashMap<String, AccuracyCell>,
    #[serde(default)]
    performance: Performance,
}

#[derive(Deserialize)]
struct AccuracyCell {
    error: f64,
}

#[derive(Deserialize, Default)]
struct Performance {
    /// Real-time factor on the benchmark GPU (Vulkan).
    accelerated: Option<f64>,
    cpu: Option<f64>,
}

impl RawModel {
    fn default_file(&self) -> Option<&RawFile> {
        self.files.get(&self.quant)
    }

    /// The app uses a flat models directory for mirror downloads, so only
    /// transcription models with root-level default files can be offered.
    fn is_offered(&self) -> bool {
        self.role == "asr"
            && self
                .default_file()
                .is_some_and(|file| !file.filename.contains('/'))
    }

    fn supports_translation(&self) -> bool {
        self.capabilities.translate || TRANSLATION_OVERRIDES.contains(&self.id.as_str())
    }
}

/// A downloadable transcription model, normalised from the raw catalog entry.
#[derive(Debug, Clone)]
pub struct CatalogModel {
    /// HF repo id, e.g. `handy-computer/whisper-small-gguf`.
    pub repo_id: String,
    /// Pinned HF commit the files and hashes belong to.
    pub revision: String,
    pub name: String,
    pub description: String,
    pub languages: Vec<String>,
    pub capabilities: CatalogCaps,
    /// 0.0–1.0, like `ModelInfo`.
    pub speed_score: f32,
    /// 0.0–1.0, like `ModelInfo`.
    pub accuracy_score: f32,
    /// Every published quant, largest first.
    pub files: Vec<CatalogFile>,
    pub default_quant: String,
    pub recommended: bool,
}

#[derive(Debug, Clone)]
pub struct CatalogCaps {
    pub streaming: bool,
    pub translate: bool,
    pub lang_detect: bool,
}

#[derive(Debug, Clone)]
pub struct CatalogFile {
    pub filename: String,
    pub quant: String,
    pub size_bytes: u64,
    pub sha256: String,
}

impl CatalogModel {
    pub fn default_file(&self) -> Option<&CatalogFile> {
        self.files
            .iter()
            .find(|file| file.quant == self.default_quant)
    }
}

// ───────────────────────── scores ─────────────────────────
// The list's 0–100 bars, from the package's benchmarks with the formulas the
// previous generator used.

/// speed = 100·(1 − e^(−rtf/8)), growing toward 100.
const SPEED_SCALE: f64 = 8.0;
/// accuracy = 100·e^(−error/15), decaying from 100.
const ACCURACY_SCALE: f64 = 15.0;
/// No benchmark speed (too heavy to measure): assume slow.
const SPEED_FLOOR: f64 = 10.0;

fn speed_score(results: Option<&ModelResults>) -> f32 {
    let rtf = results.and_then(|r| r.performance.accelerated.or(r.performance.cpu));
    let score = match rtf {
        Some(rtf) if rtf > 0.0 => 100.0 * (1.0 - (-rtf / SPEED_SCALE).exp()),
        _ => SPEED_FLOOR,
    };
    score.round() as f32
}

/// Accuracy on English when measured, else on the model's first language.
fn accuracy_score(model: &RawModel, results: Option<&ModelResults>) -> f32 {
    let Some(results) = results else {
        return 0.0;
    };
    let headline = std::iter::once("en".to_string())
        .chain(model.languages.first().map(|l| language_identity(l)))
        .find_map(|language| results.accuracy.get(&language));
    headline
        .map(|cell| (100.0 * (-cell.error / ACCURACY_SCALE).exp()).round() as f32)
        .unwrap_or(0.0)
}

/// The package's language identity (its `languageIdentity`): base code, with
/// the aliases its benchmark keys use.
fn language_identity(language: &str) -> String {
    let base = language
        .split(['-', '_'])
        .next()
        .unwrap_or(language)
        .to_ascii_lowercase();
    match base.as_str() {
        "tl" => "fil".to_string(),
        "no" => "nb".to_string(),
        "iw" => "he".to_string(),
        "jw" => "jv".to_string(),
        _ => base,
    }
}

// ───────────────────────── descriptions ─────────────────────────

/// A factual one-liner for models the package has no editorial copy for.
fn generated_description(model: &RawModel) -> String {
    let mut features = Vec::new();
    if model.supports_translation() {
        features.push("translation");
    }
    if model.capabilities.language_detection {
        features.push("auto language detection");
    }
    if model.capabilities.streaming {
        features.push("streaming");
    }
    if model.capabilities.timestamps {
        features.push("timestamps");
    }

    let languages: BTreeSet<String> = model
        .languages
        .iter()
        .map(|l| language_identity(l))
        .collect();
    let base = if languages.len() > 1 {
        format!("{}-language speech-to-text", languages.len())
    } else {
        let name = match languages.iter().next().map(String::as_str) {
            None | Some("en") => "English",
            Some("ar") => "Arabic",
            Some("ja") => "Japanese",
            Some("ko") => "Korean",
            Some("ru") => "Russian",
            Some("uk") => "Ukrainian",
            Some("vi") => "Vietnamese",
            Some("zh") => "Chinese",
            Some(other) => other,
        };
        format!("{name} speech-to-text")
    };

    if features.is_empty() {
        format!("{base}.")
    } else {
        format!("{base} with {}.", features.join(", "))
    }
}

impl From<&RawModel> for CatalogModel {
    fn from(m: &RawModel) -> Self {
        let results = ROOT.results.get(&m.id);
        let mut files: Vec<CatalogFile> = m
            .files
            .iter()
            .map(|(quant, file)| CatalogFile {
                filename: file.filename.clone(),
                quant: quant.clone(),
                size_bytes: file.size,
                sha256: file.sha256.clone(),
            })
            .collect();
        // Largest first, so the list reads F32 → Q4 like the HF repo.
        files.sort_by_key(|file| std::cmp::Reverse(file.size_bytes));

        CatalogModel {
            repo_id: m.repository.clone(),
            revision: m.revision.clone(),
            name: m.name.clone(),
            description: if m.description.is_empty() {
                generated_description(m)
            } else {
                m.description.clone()
            },
            languages: m.languages.clone(),
            capabilities: CatalogCaps {
                streaming: m.capabilities.streaming,
                translate: m.supports_translation(),
                lang_detect: m.capabilities.language_detection,
            },
            speed_score: speed_score(results) / 100.0,
            accuracy_score: accuracy_score(m, results) / 100.0,
            files,
            default_quant: m.quant.clone(),
            recommended: RECOMMENDED.contains(&m.id.as_str()),
        }
    }
}

static ROOT: Lazy<CatalogRoot> = Lazy::new(|| {
    let root: CatalogRoot = serde_json::from_str(include_str!("catalog.json"))
        .expect("bundled catalog.json is valid JSON matching the catalog schema");
    assert_eq!(
        root.schema, SCHEMA,
        "bundled catalog.json has an unsupported schema"
    );
    root
});

/// Transcription models the app can download. Language ID, VAD and
/// diarization models are published alongside them but produce no text.
fn offered_models() -> impl Iterator<Item = &'static RawModel> {
    ROOT.models.iter().filter(|m| m.is_offered())
}

pub static CATALOG: Lazy<Vec<CatalogModel>> =
    Lazy::new(|| offered_models().map(CatalogModel::from).collect());

/// The offered model and quant a file belongs to, matched across every listed
/// quant. `repo_id`, when given, must match exactly: a foreign repo reusing a
/// catalog filename stays foreign, and on a case-sensitive filesystem only the
/// cache folder's own casing finds the file again. Without it the filename
/// alone decides, as in the flat models dir, where filenames are unique.
pub fn file_in_catalog(
    filename: &str,
    repo_id: Option<&str>,
) -> Option<(&'static CatalogModel, &'static CatalogFile)> {
    CATALOG
        .iter()
        .filter(|model| repo_id.is_none_or(|repo_id| model.repo_id == repo_id))
        .find_map(|model| {
            model
                .files
                .iter()
                .find(|file| file.filename == filename)
                .map(|file| (model, file))
        })
}

/// A mirror copy of a catalog model's file, with the expected content hash for
/// end-to-end verification. Mirrors are untrusted bit-pipes: the sha256 here
/// (from the catalog compiled into the binary) is the trust anchor.
pub struct MirrorFile {
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

/// Ordered mirror URLs for an offered catalog model's file — any listed quant,
/// not just the default — or empty when the model isn't offered from the
/// catalog. `model_id` is the registry id (`"{repo_id}/{filename}"`). (The
/// mirror only hosts default quants; a miss there just 404s and the caller
/// reports it, so listing every quant here costs nothing.)
pub fn mirror_fallbacks(model_id: &str) -> Vec<MirrorFile> {
    let Some((model, file)) = offered_models().find_map(|model| {
        model
            .files
            .values()
            .find(|file| format!("{}/{}", model.repository, file.filename) == model_id)
            .map(|file| (model, file))
    }) else {
        return Vec::new();
    };

    MIRRORS
        .iter()
        .map(|base| MirrorFile {
            url: format!(
                "{}/{}/{}/{}",
                base.trim_end_matches('/'),
                model.repository,
                model.revision,
                file.filename
            ),
            sha256: file.sha256.clone(),
            size_bytes: file.size,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::model_capabilities::KNOWN_ARCHES;

    fn registry_id(model: &CatalogModel) -> String {
        let file = model.default_file().expect("default file");
        format!("{}/{}", model.repo_id, file.filename)
    }

    #[test]
    fn catalog_parses_and_offers_models() {
        assert!(!CATALOG.is_empty(), "bundled catalog should offer models");
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<String> = CATALOG.iter().map(registry_id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "catalog registry ids must be unique");
    }

    #[test]
    fn only_transcription_models_are_loaded() {
        // Language ID (ecapa_tdnn), VAD (silero_vad) and diarization
        // (sortformer) models are published but produce no transcription text.
        for arch in ["ecapa_tdnn", "silero_vad", "sortformer"] {
            assert!(
                offered_models().all(|m| m.family != arch),
                "{arch} must not be in the transcription catalog"
            );
        }
    }

    #[test]
    fn all_flat_asr_models_are_offered_at_the_package_default() {
        for m in ROOT.models.iter().filter(|m| {
            m.role == "asr" && m.default_file().is_some_and(|f| !f.filename.contains('/'))
        }) {
            let id = format!("{}/{}", m.repository, m.default_file().unwrap().filename);
            let model = CATALOG
                .iter()
                .find(|model| registry_id(model) == id)
                .expect("ASR model is offered");
            assert_eq!(model.default_quant, m.quant);
            assert_eq!(m.quant, "Q8_0");
        }
    }

    #[test]
    fn catalog_pins_revisions_and_hashes() {
        // HF acquisition and mirror keys rely on immutable commits, and
        // downloads are verified against these hashes.
        let is_hex = |value: &str, len: usize| {
            value.len() == len
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        for m in CATALOG.iter() {
            assert!(is_hex(&m.revision, 40), "{}: bad revision", m.repo_id);
            for file in &m.files {
                assert!(is_hex(&file.sha256, 64), "{}: bad sha256", file.filename);
            }
        }
    }

    #[test]
    fn scores_are_normalised_0_to_1() {
        for m in CATALOG.iter() {
            assert!((0.0..=1.0).contains(&m.speed_score), "{} speed", m.repo_id);
            assert!((0.0..=1.0).contains(&m.accuracy_score), "{} acc", m.repo_id);
        }
    }

    #[test]
    fn every_description_is_filled() {
        for m in CATALOG.iter() {
            assert!(!m.description.is_empty(), "{}: no description", m.repo_id);
        }
    }

    #[test]
    fn recommended_models_are_offered() {
        for id in RECOMMENDED {
            let model = ROOT
                .models
                .iter()
                .find(|m| m.id == *id)
                .unwrap_or_else(|| panic!("recommended model {id} is not in the catalog"));
            assert!(model.is_offered(), "recommended model {id} is not offered");
        }
        assert_eq!(
            CATALOG.iter().filter(|m| m.recommended).count(),
            RECOMMENDED.len()
        );
    }

    #[test]
    fn whisper_turbo_keeps_translation() {
        for id in TRANSLATION_OVERRIDES {
            let model = ROOT
                .models
                .iter()
                .find(|m| m.id == *id)
                .unwrap_or_else(|| panic!("translation override {id} is not in the catalog"));
            let repo_id = model.repository.as_str();
            assert!(CATALOG
                .iter()
                .any(|m| m.repo_id == repo_id && m.capabilities.translate));
        }
    }

    #[test]
    fn file_in_catalog_matches_every_quant_of_the_exact_repo() {
        let model = CATALOG
            .iter()
            .find(|m| m.files.len() > 1)
            .expect("catalog offers multi-quant models");
        for file in &model.files {
            for repo_id in [Some(model.repo_id.as_str()), None] {
                let (found, quant) = file_in_catalog(&file.filename, repo_id).unwrap();
                assert_eq!(found.repo_id, model.repo_id);
                assert_eq!(quant.quant, file.quant);
            }
        }
        let filename = &model.files[0].filename;
        let other_case = model.repo_id.to_uppercase();
        assert!(file_in_catalog(filename, Some(&other_case)).is_none());
    }

    #[test]
    fn offered_filenames_are_unique() {
        // Mirror downloads share one flat models dir, where a file is matched
        // to its catalog model by filename alone.
        let mut owners = HashMap::new();
        for model in CATALOG.iter() {
            for file in &model.files {
                let other = owners.insert(&file.filename, &model.repo_id);
                assert_eq!(
                    other, None,
                    "{} is also in {}",
                    file.filename, model.repo_id
                );
            }
        }
    }

    #[test]
    fn every_offered_model_has_mirror_fallbacks_with_hashes() {
        // The mirror fallback is the safety net for HF outages and blocked
        // networks; an offered entry without one silently loses that net.
        for m in CATALOG.iter() {
            let id = registry_id(m);
            let mirrors = mirror_fallbacks(&id);
            assert!(!mirrors.is_empty(), "{id}: no mirror fallbacks");
            for mirror in &mirrors {
                assert_eq!(mirror.sha256.len(), 64, "{id}: mirror entry lacks a sha256");
                assert!(mirror.size_bytes > 0, "{id}: mirror entry lacks a size");
                assert!(
                    mirror.url.starts_with("https://"),
                    "{id}: bad url {}",
                    mirror.url
                );
            }
        }
    }

    #[test]
    fn catalog_matches_the_engine_the_app_ships() {
        // Every model must be loadable by the transcribe-cpp crate we link.
        let cargo = include_str!("../../Cargo.toml");
        let engine = cargo
            .lines()
            .find_map(|line| {
                line.trim_start()
                    .strip_prefix("transcribe-cpp = { version = \"")
                    .and_then(|rest| rest.split('"').next())
            })
            .expect("Cargo.toml pins a transcribe-cpp version");
        let parse = |v: &str| -> Vec<u32> {
            v.trim_start_matches(['=', '^', '~', 'v'])
                .split(['.', '-', '+'])
                .take(3)
                .map(|p| p.parse().unwrap_or(0))
                .collect()
        };
        assert!(
            parse(&ROOT.transcribe_cpp) <= parse(engine),
            "catalog built for transcribe.cpp {} but the app links {}",
            ROOT.transcribe_cpp,
            engine
        );
        for m in offered_models() {
            if let Some(min) = &m.min_engine {
                assert!(
                    parse(min) <= parse(engine),
                    "{} needs transcribe.cpp {min}, the app links {engine}",
                    m.id
                );
            }
        }
    }

    #[test]
    fn catalog_architectures_are_known_to_capability_probe() {
        let missing: BTreeSet<&str> = offered_models()
            .map(|m| m.family.as_str())
            .filter(|arch| !KNOWN_ARCHES.contains(arch))
            .collect();

        assert!(
            missing.is_empty(),
            "catalog architecture(s) missing from KNOWN_ARCHES: {:?}",
            missing
        );
    }

    #[test]
    fn generated_descriptions_read_naturally() {
        let find = |id: &str| ROOT.models.iter().find(|m| m.id == id).unwrap();
        let gigaam = find("gigaam-v3-ctc");
        assert!(generated_description(gigaam).starts_with("Russian speech-to-text"));
        let whisper = find("whisper-tiny");
        assert!(generated_description(whisper).contains("-language speech-to-text with"));
    }
}
