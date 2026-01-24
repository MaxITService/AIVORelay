use crate::subtitle::SubtitleSegment;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Parameters for configuring Whisper inference behavior.
#[derive(Debug, Clone)]
pub struct LocalWhisperInferenceParams {
    pub language: Option<String>,
    pub translate: bool,
    pub initial_prompt: Option<String>,
}

impl Default for LocalWhisperInferenceParams {
    fn default() -> Self {
        Self {
            language: None,
            translate: false,
            initial_prompt: None,
        }
    }
}

pub struct LocalWhisperResult {
    pub text: String,
    pub segments: Option<Vec<SubtitleSegment>>,
}

pub struct LocalWhisperEngine {
    state: Option<whisper_rs::WhisperState>,
    context: Option<whisper_rs::WhisperContext>,
}

impl LocalWhisperEngine {
    pub fn new() -> Self {
        #[cfg(feature = "cuda")]
        log::info!("LocalWhisperEngine initialized with CUDA acceleration");
        #[cfg(feature = "vulkan")]
        log::info!("LocalWhisperEngine initialized with Vulkan acceleration");
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        log::info!("LocalWhisperEngine initialized with Metal acceleration");
        #[cfg(not(any(
            feature = "cuda",
            feature = "vulkan",
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        log::info!("LocalWhisperEngine initialized with CPU");

        Self {
            state: None,
            context: None,
        }
    }

    pub fn load_model(&mut self, model_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let context = WhisperContext::new_with_params(
            model_path.to_str().ok_or("Invalid path")?,
            WhisperContextParameters::default(),
        )?;

        let state = context.create_state()?;

        self.context = Some(context);
        self.state = Some(state);

        Ok(())
    }

    pub fn unload_model(&mut self) {
        self.state = None;
        self.context = None;
    }

    pub fn transcribe_samples(
        &mut self,
        samples: Vec<f32>,
        params: Option<LocalWhisperInferenceParams>,
    ) -> Result<LocalWhisperResult, Box<dyn std::error::Error>> {
        let state = self
            .state
            .as_mut()
            .ok_or("Model not loaded. Call load_model() first.")?;

        let whisper_params = params.unwrap_or_default();

        let mut full_params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 3,
            patience: -1.0,
        });
        full_params.set_language(whisper_params.language.as_deref());
        full_params.set_translate(whisper_params.translate);
        full_params.set_print_special(false);
        full_params.set_print_progress(false);
        full_params.set_print_realtime(false);
        full_params.set_print_timestamps(false);
        full_params.set_suppress_blank(true);
        full_params.set_suppress_non_speech_tokens(true);
        full_params.set_no_speech_thold(0.2);

        if let Some(ref prompt) = whisper_params.initial_prompt {
            full_params.set_initial_prompt(prompt);
        }

        state.full(full_params, &samples)?;

        let num_segments = state
            .full_n_segments()
            .expect("failed to get number of segments");

        let mut segments = Vec::new();
        let mut full_text = String::new();

        for i in 0..num_segments {
            let text = state.full_get_segment_text(i)?;
            let start = state.full_get_segment_t0(i)? as f32 / 100.0;
            let end = state.full_get_segment_t1(i)? as f32 / 100.0;

            segments.push(SubtitleSegment {
                start,
                end,
                text: text.clone(),
            });
            full_text.push_str(&text);
        }

        Ok(LocalWhisperResult {
            text: full_text.trim().to_string(),
            segments: Some(segments),
        })
    }
}
