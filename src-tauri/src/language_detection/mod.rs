//! Policy and runtime support for selecting a spoken language before transcription.

pub mod policy;
mod detector;
pub mod speech;

use log::{info, warn};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use transcribe_cpp::BackendMask;

pub use detector::LanguageDetector;
use policy::{DetectFallback, Plan};

const MIN_SPEECH_SAMPLES: usize = 16_000 / 2;
const UNSURE_RATIO: f32 = 1.5;

fn init_cpu_backends() -> anyhow::Result<()> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        transcribe_cpp::init_logging();
        transcribe_cpp::init_backends_with(None::<&Path>, BackendMask::CPU)
            .map_err(|error| error.to_string())
    })
    .clone()
    .map_err(|error| anyhow::anyhow!("Failed to initialize language detection CPU backend: {error}"))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum LanguageDetectionMode {
    #[default]
    WhenNeeded,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageSource {
    Selected,
    Model,
    Detected,
    Fallback,
}

impl LanguageSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Model => "model",
            Self::Detected => "detected",
            Self::Fallback => "fallback",
        }
    }
}

/// Some native engines ignore language hints entirely; let them decide instead
/// of running the detector and reporting a choice they cannot honor.
pub fn engine_accepts_language(engine: &crate::managers::model::EngineType) -> bool {
    use crate::managers::model::EngineType;
    matches!(engine, EngineType::TranscribeCpp | EngineType::Whisper | EngineType::SenseVoice | EngineType::Canary | EngineType::Cohere)
}

/// The language one transcription runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageDecision {
    /// The model's code, or `None` for the model's own detection.
    pub language: Option<String>,
    pub source: LanguageSource,
}

impl LanguageDecision {
    /// As a language intent: the code, or `"auto"`.
    pub fn intent(&self) -> &str {
        self.language.as_deref().unwrap_or("auto")
    }
}

/// A detector run: `(label, p)` ranked by `p`, over the allowed labels.
pub struct Identified {
    pub candidates: Vec<(String, f32)>,
    pub allowed_mass: f32,
    pub elapsed: Duration,
}

pub enum IdentifyError {
    /// The detector isn't on disk.
    Missing,
    Failed(anyhow::Error),
}

impl Plan {
    /// Whether [`decide`](Self::decide) needs the audio's speech.
    pub fn detects(&self) -> bool {
        matches!(self, Plan::Detect { .. })
    }

    /// The language, given the transcription's `speech` (VAD'd, at most the
    /// detection window), or `None` if the detector isn't on disk. `identify`
    /// runs the detector over `allowed`.
    pub fn decide(
        &self,
        speech: Option<&[f32]>,
        identify: impl FnOnce(&[f32], &[String]) -> Result<Identified, IdentifyError>,
    ) -> LanguageDecision {
        let (languages, allowed, fallback) = match self {
            Plan::Fixed { language, source } => {
                return LanguageDecision {
                    language: Some(language.clone()),
                    source: *source,
                }
            }
            Plan::Native => {
                return LanguageDecision {
                    language: None,
                    source: LanguageSource::Model,
                }
            }
            Plan::Detect {
                languages,
                allowed,
                fallback,
            } => (languages, allowed, *fallback),
        };

        let fallback_decision = LanguageDecision {
            language: match fallback {
                DetectFallback::FirstLanguage => languages.first().cloned(),
                DetectFallback::Native => None,
            },
            source: LanguageSource::Fallback,
        };
        let transcribing_as = match &fallback_decision.language {
            Some(language) => format!("transcribing as {language}, the first preferred language"),
            None => "using the model's own detection".to_string(),
        };
        let Some(speech) = speech else {
            info!("Language detection: the detector isn't downloaded; {transcribing_as}");
            return fallback_decision;
        };
        let seconds = speech.len() as f32 / 16_000.0;

        if speech.len() < MIN_SPEECH_SAMPLES {
            info!(
                "Language detection: skipped, {seconds:.2} s of speech is under the 0.5 s minimum; {transcribing_as}"
            );
            return fallback_decision;
        }
        let identified = match identify(speech, allowed) {
            Ok(identified) => identified,
            Err(IdentifyError::Missing) => {
                info!("Language detection: the detector isn't downloaded; {transcribing_as}");
                return fallback_decision;
            }
            Err(IdentifyError::Failed(e)) => {
                warn!("Language detection failed: {e:#}; {transcribing_as}");
                return fallback_decision;
            }
        };

        let top = identified.candidates.first();
        let language = top
            .filter(|(_, p)| p.is_finite())
            .and_then(|(label, _)| allowed.iter().position(|a| a == label))
            .and_then(|i| languages.get(i))
            .cloned();
        let scores = identified
            .candidates
            .iter()
            .map(|(label, p)| format!("{label} {:.1}%", p * 100.0))
            .collect::<Vec<_>>()
            .join(", ");
        let uncertain = match (top, identified.candidates.get(1)) {
            (Some((_, first)), Some((_, second))) if *first < UNSURE_RATIO * second => {
                ", uncertain"
            }
            _ => "",
        };
        info!(
            "Language detection: {} ({scores}; {seconds:.2} s of speech, allowed mass {:.1}%{uncertain}, {} ms)",
            language.as_deref().unwrap_or("none"),
            identified.allowed_mass * 100.0,
            identified.elapsed.as_millis()
        );
        match language {
            Some(language) => LanguageDecision {
                language: Some(language),
                source: LanguageSource::Detected,
            },
            None => {
                warn!("Language detection returned no usable language; {transcribing_as}");
                fallback_decision
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detector_is_only_used_with_engines_that_accept_language_hints() {
        use crate::managers::model::EngineType;
        assert!(!engine_accepts_language(&EngineType::Parakeet));
        assert!(!engine_accepts_language(&EngineType::Moonshine));
        assert!(engine_accepts_language(&EngineType::Canary));
        assert!(engine_accepts_language(&EngineType::TranscribeCpp));
    }

    fn codes(list: &[&str]) -> Vec<String> {
        list.iter().map(|c| c.to_string()).collect()
    }

    fn detect(fallback: DetectFallback) -> Plan {
        Plan::Detect {
            languages: codes(&["he", "en"]),
            allowed: codes(&["iw", "en"]),
            fallback,
        }
    }

    fn ranked(candidates: &[(&str, f32)]) -> Result<Identified, IdentifyError> {
        Ok(Identified {
            candidates: candidates
                .iter()
                .map(|(label, p)| (label.to_string(), *p))
                .collect(),
            allowed_mass: 0.9,
            elapsed: Duration::ZERO,
        })
    }

    static SPEECH: [f32; 16_000] = [0.1; 16_000];

    #[test]
    fn detects_in_the_models_own_code() {
        let decision = detect(DetectFallback::FirstLanguage).decide(Some(&SPEECH), |_, allowed| {
            assert_eq!(allowed, codes(&["iw", "en"]));
            ranked(&[("iw", 0.9), ("en", 0.1)])
        });
        assert_eq!(decision.language.as_deref(), Some("he"));
        assert_eq!(decision.source, LanguageSource::Detected);
    }

    #[test]
    fn falls_back_when_detection_cant_decide() {
        let first = detect(DetectFallback::FirstLanguage);
        let fallback = |language: Option<&str>| LanguageDecision {
            language: language.map(String::from),
            source: LanguageSource::Fallback,
        };
        let short = &SPEECH[..MIN_SPEECH_SAMPLES - 1];
        assert_eq!(
            first.decide(Some(short), |_, _| panic!("too little speech to run")),
            fallback(Some("he"))
        );
        assert_eq!(
            first.decide(None, |_, _| panic!("no detector to run")),
            fallback(Some("he"))
        );
        let missing = first.decide(Some(&SPEECH), |_, _| Err(IdentifyError::Missing));
        assert_eq!(missing, fallback(Some("he")));
        // Always mode falls back to the model's own detection.
        let failed = detect(DetectFallback::Native).decide(Some(&SPEECH), |_, _| {
            Err(IdentifyError::Failed(anyhow::anyhow!("boom")))
        });
        assert_eq!(failed, fallback(None));
    }

    #[test]
    fn unusable_detector_output_falls_back() {
        let fallback = LanguageDecision {
            language: Some("he".to_string()),
            source: LanguageSource::Fallback,
        };
        let outputs: [&[(&str, f32)]; 4] = [
            &[],
            &[("iw", f32::NAN), ("en", 0.1)],
            &[("en", f32::INFINITY)],
            // A label outside the allowed set must not pick a language by accident.
            &[("fr", 0.9), ("iw", 0.1)],
        ];
        for output in outputs {
            let decision = detect(DetectFallback::FirstLanguage).decide(Some(&SPEECH), |_, _| ranked(output));
            assert_eq!(decision, fallback, "{output:?}");
        }
    }

    #[test]
    fn half_a_second_of_speech_is_enough_to_detect() {
        let decision = detect(DetectFallback::Native).decide(Some(&SPEECH[..MIN_SPEECH_SAMPLES]), |speech, _| {
            assert_eq!(speech.len(), MIN_SPEECH_SAMPLES);
            ranked(&[("en", 0.6), ("iw", 0.4)])
        });
        assert_eq!(decision.language.as_deref(), Some("en"));
        assert_eq!(decision.source, LanguageSource::Detected);
    }

    #[test]
    fn fixed_and_native_plans_never_run_the_detector() {
        let fixed = Plan::Fixed {
            language: "de".to_string(),
            source: LanguageSource::Selected,
        };
        assert!(!fixed.detects());
        let decision = fixed.decide(Some(&SPEECH), |_, _| panic!("a fixed language needs no detection"));
        assert_eq!(decision.language.as_deref(), Some("de"));
        assert_eq!(decision.source, LanguageSource::Selected);

        assert!(!Plan::Native.detects());
        let decision = Plan::Native.decide(Some(&SPEECH), |_, _| panic!("the model detects on its own"));
        assert_eq!(decision.intent(), "auto");
        assert_eq!(decision.source, LanguageSource::Model);
        assert!(detect(DetectFallback::Native).detects());
    }

    #[test]
    fn old_settings_without_detector_fields_keep_previous_behavior() {
        let mut value = serde_json::to_value(crate::settings::get_default_settings()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("spoken_languages");
        object.remove("language_detection");
        object.remove("language_detection_window_secs");
        let settings: crate::settings::AppSettings = serde_json::from_value(value).unwrap();
        assert!(settings.spoken_languages.is_empty());
        assert_eq!(settings.language_detection, LanguageDetectionMode::WhenNeeded);
        assert_eq!(settings.language_detection_window_secs, 5);
    }
}
