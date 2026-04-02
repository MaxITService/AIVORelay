use crate::audio_toolkit::{apply_custom_words, filter_transcription_output};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::model::{EngineType, ModelManager};
use crate::settings::{
    get_settings, ModelUnloadTimeout, OrtAcceleratorSetting, WhisperAcceleratorSetting,
};
use anyhow::Result;
use log::{debug, error, info, warn};
use serde::Serialize;
use specta::Type;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter, Manager};
use transcribe_rs::{
    onnx::{
        canary::CanaryModel,
        cohere::CohereModel,
        gigaam::GigaAMModel,
        moonshine::{MoonshineModel, MoonshineVariant, StreamingModel},
        parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity},
        sense_voice::{SenseVoiceModel, SenseVoiceParams},
        Quantization,
    },
    whisper_cpp::{WhisperEngine, WhisperInferenceParams},
    SpeechModel, TranscribeOptions,
};

#[derive(Clone, Debug, Serialize)]
pub struct ModelStateEvent {
    pub event_type: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetModel),
    Moonshine(MoonshineModel),
    MoonshineStreaming(StreamingModel),
    SenseVoice(SenseVoiceModel),
    GigaAM(GigaAMModel),
    Canary(CanaryModel),
    Cohere(CohereModel),
}

pub struct LoadingGuard {
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl Drop for LoadingGuard {
    fn drop(&mut self) {
        let mut is_loading = self.is_loading.lock().unwrap();
        *is_loading = false;
        self.loading_condvar.notify_all();
    }
}

fn detect_cohere_quantization(model_id: &str, model_path: &Path) -> Quantization {
    let has_any = |candidates: &[&str]| {
        [model_path.to_path_buf(), model_path.join("onnx")]
            .into_iter()
            .any(|base| candidates.iter().any(|name| base.join(name).exists()))
    };

    if model_id.contains("fp32")
        || (has_any(&["cohere-encoder.onnx", "encoder_model.onnx"])
            && has_any(&["cohere-decoder.onnx", "decoder_model_merged.onnx"]))
    {
        return Quantization::FP32;
    }

    if model_id.contains("fp16")
        || (has_any(&["cohere-encoder.fp16.onnx", "encoder_model_fp16.onnx"])
            && has_any(&["cohere-decoder.fp16.onnx", "decoder_model_merged_fp16.onnx"]))
    {
        return Quantization::FP16;
    }

    if model_id.contains("int4")
        || (has_any(&["cohere-encoder.int4.onnx", "encoder_model.int4.onnx"])
            && has_any(&["cohere-decoder.int4.onnx", "decoder_model_merged.int4.onnx"]))
    {
        return Quantization::Int4;
    }

    Quantization::Int8
}

fn build_whisper_initial_prompt(
    base_prompt: Option<String>,
    custom_words: &[String],
    include_custom_words: bool,
) -> Option<String> {
    let custom_words_prompt = if include_custom_words && !custom_words.is_empty() {
        Some(custom_words.join(", "))
    } else {
        None
    };

    match (base_prompt, custom_words_prompt) {
        (Some(prompt), Some(words)) => Some(format!("{}\n{}", prompt, words)),
        (Some(prompt), None) => Some(prompt),
        (None, Some(words)) => Some(words),
        (None, None) => None,
    }
}

#[derive(Clone)]
pub struct TranscriptionManager {
    engine: Arc<Mutex<Option<LoadedEngine>>>,
    model_manager: Arc<ModelManager>,
    app_handle: AppHandle,
    current_model_id: Arc<Mutex<Option<String>>>,
    last_activity: Arc<AtomicU64>,
    shutdown_signal: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl TranscriptionManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Result<Self> {
        let manager = Self {
            engine: Arc::new(Mutex::new(None)),
            model_manager,
            app_handle: app_handle.clone(),
            current_model_id: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(AtomicU64::new(Self::now_ms())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
            is_loading: Arc::new(Mutex::new(false)),
            loading_condvar: Arc::new(Condvar::new()),
        };

        // Start the idle watcher
        {
            let app_handle_cloned = app_handle.clone();
            let manager_cloned = manager.clone();
            let shutdown_signal = manager.shutdown_signal.clone();
            let handle = thread::spawn(move || {
                debug!("Idle watcher thread started");
                while !shutdown_signal.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(10)); // Check every 10 seconds

                    // Check shutdown signal again after sleep
                    if shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    let settings = get_settings(&app_handle_cloned);
                    let timeout = settings.model_unload_timeout;

                    // Immediate unloading is handled after transcription completes.
                    // The idle watcher should never unload the model mid-recording.
                    if timeout == ModelUnloadTimeout::Immediately {
                        continue;
                    }

                    let is_recording = app_handle_cloned
                        .try_state::<Arc<AudioRecordingManager>>()
                        .map_or(false, |manager| manager.is_recording());
                    if is_recording {
                        manager_cloned.touch_activity();
                        continue;
                    }

                    if let Some(limit_seconds) = timeout.to_seconds() {
                        let last = manager_cloned.last_activity.load(Ordering::Relaxed);
                        let now_ms = Self::now_ms();
                        let idle_ms = now_ms.saturating_sub(last);
                        let limit_ms = limit_seconds * 1000;

                        if idle_ms > limit_ms {
                            // idle -> unload
                            if manager_cloned.is_model_loaded() {
                                let unload_start = std::time::Instant::now();
                                info!(
                                    "Model idle for {}s (limit: {}s), unloading",
                                    idle_ms / 1000,
                                    limit_seconds
                                );

                                match manager_cloned.unload_model() {
                                    Ok(()) => {
                                        let unload_duration = unload_start.elapsed();
                                        info!(
                                            "Model unloaded due to inactivity (took {}ms)",
                                            unload_duration.as_millis()
                                        );
                                    }
                                    Err(e) => {
                                        error!("Failed to unload idle model: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                debug!("Idle watcher thread shutting down gracefully");
            });
            *manager.watcher_handle.lock().unwrap() = Some(handle);
        }

        Ok(manager)
    }

    /// Lock the engine mutex, recovering from poison if a previous transcription panicked.
    fn lock_engine(&self) -> MutexGuard<'_, Option<LoadedEngine>> {
        self.engine.lock().unwrap_or_else(|poisoned| {
            warn!("Engine mutex was poisoned by a previous panic, recovering");
            poisoned.into_inner()
        })
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn touch_activity(&self) {
        self.last_activity.store(Self::now_ms(), Ordering::Relaxed);
    }

    pub fn is_model_loaded(&self) -> bool {
        let engine = self.lock_engine();
        engine.is_some()
    }

    pub fn try_start_loading(&self) -> Option<LoadingGuard> {
        let mut is_loading = self.is_loading.lock().unwrap();
        if *is_loading {
            return None;
        }
        *is_loading = true;
        Some(LoadingGuard {
            is_loading: self.is_loading.clone(),
            loading_condvar: self.loading_condvar.clone(),
        })
    }

    pub fn unload_model(&self) -> Result<()> {
        let unload_start = std::time::Instant::now();
        debug!("Starting to unload model");

        {
            let mut engine = self.lock_engine();
            *engine = None;
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = None;
        }

        // Emit unloaded event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "unloaded".to_string(),
                model_id: None,
                model_name: None,
                error: None,
            },
        );

        let unload_duration = unload_start.elapsed();
        debug!(
            "Model unloaded manually (took {}ms)",
            unload_duration.as_millis()
        );
        Ok(())
    }

    /// Unloads the model immediately if the setting is enabled and the model is loaded
    pub fn maybe_unload_immediately(&self, context: &str) {
        let settings = get_settings(&self.app_handle);
        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately
            && self.is_model_loaded()
        {
            info!("Immediately unloading model after {}", context);
            if let Err(e) = self.unload_model() {
                warn!("Failed to immediately unload model: {}", e);
            }
        }
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        let load_start = std::time::Instant::now();
        debug!("Starting to load model: {}", model_id);

        // Emit loading started event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_started".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: None,
                error: None,
            },
        );

        let model_info = self
            .model_manager
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        let emit_loading_failed = |error_msg: &str| {
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
        };

        if !model_info.is_downloaded {
            let error_msg = "Model not downloaded";
            emit_loading_failed(error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }

        let model_path = self.model_manager.get_model_path(model_id)?;

        // Create appropriate engine based on model type

        let loaded_engine = match model_info.engine_type {
            EngineType::Whisper => {
                let engine = WhisperEngine::load(&model_path).map_err(|e| {
                    let error_msg = format!("Failed to load whisper model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Whisper(engine)
            }
            EngineType::Parakeet => {
                let engine =
                    ParakeetModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                        let error_msg =
                            format!("Failed to load parakeet model {}: {}", model_id, e);
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Parakeet(engine)
            }
            EngineType::Moonshine => {
                let engine = MoonshineModel::load(
                    &model_path,
                    MoonshineVariant::Base,
                    &Quantization::default(),
                )
                .map_err(|e| {
                    let error_msg = format!("Failed to load moonshine model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Moonshine(engine)
            }
            EngineType::MoonshineStreaming => {
                let engine = StreamingModel::load(&model_path, 0, &Quantization::default())
                    .map_err(|e| {
                        let error_msg = format!(
                            "Failed to load moonshine streaming model {}: {}",
                            model_id, e
                        );
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::MoonshineStreaming(engine)
            }
            EngineType::SenseVoice => {
                let engine =
                    SenseVoiceModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                        let error_msg =
                            format!("Failed to load SenseVoice model {}: {}", model_id, e);
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::SenseVoice(engine)
            }
            EngineType::GigaAM => {
                let engine = GigaAMModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                    let error_msg = format!("Failed to load gigaam model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::GigaAM(engine)
            }
            EngineType::Canary => {
                let engine = CanaryModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                    let error_msg = format!("Failed to load canary model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Canary(engine)
            }
            EngineType::Cohere => {
                let quantization = detect_cohere_quantization(model_id, &model_path);
                info!(
                    "Loading Cohere model {} using {:?} quantization",
                    model_id, quantization
                );
                let engine = CohereModel::load(&model_path, &quantization).map_err(|e| {
                    let error_msg = format!("Failed to load cohere model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Cohere(engine)
            }
        };

        // Update the current engine and model ID
        {
            let mut engine = self.lock_engine();
            *engine = Some(loaded_engine);
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = Some(model_id.to_string());
        }

        self.touch_activity();

        // Emit loading completed event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_completed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );

        let load_duration = load_start.elapsed();
        debug!(
            "Successfully loaded transcription model: {} (took {}ms)",
            model_id,
            load_duration.as_millis()
        );
        Ok(())
    }

    /// Kicks off the model loading in a background thread if it's not already loaded
    pub fn initiate_model_load(&self) {
        if self.is_model_loaded() {
            return;
        }

        let Some(loading_guard) = self.try_start_loading() else {
            return;
        };
        let self_clone = self.clone();
        thread::spawn(move || {
            let _loading_guard = loading_guard;
            let settings = get_settings(&self_clone.app_handle);
            if let Err(e) = self_clone.load_model(&settings.selected_model) {
                error!("Failed to load model: {}", e);
            }
        });
    }

    pub fn get_current_model(&self) -> Option<String> {
        let current_model = self.current_model_id.lock().unwrap();
        current_model.clone()
    }

    pub fn transcribe(&self, audio: Vec<f32>, apply_custom_words_enabled: bool) -> Result<String> {
        #[cfg(debug_assertions)]
        if std::env::var("HANDY_FORCE_TRANSCRIPTION_FAILURE").is_ok() {
            return Err(anyhow::anyhow!(
                "Simulated transcription failure (HANDY_FORCE_TRANSCRIPTION_FAILURE)"
            ));
        }

        // Update last activity timestamp
        self.touch_activity();

        let st = std::time::Instant::now();

        debug!("Audio vector length: {}", audio.len());

        if audio.is_empty() {
            debug!("Empty audio vector");
            self.maybe_unload_immediately("empty audio");
            return Ok(String::new());
        }

        // Check if model is loaded, if not try to load it
        {
            // If the model is loading, wait for it to complete.
            let mut is_loading = self.is_loading.lock().unwrap();
            while *is_loading {
                is_loading = self.loading_condvar.wait(is_loading).unwrap();
            }

            let engine_guard = self.lock_engine();
            if engine_guard.is_none() {
                return Err(anyhow::anyhow!("Model is not loaded for transcription."));
            }
        }

        // Get current settings for configuration
        let settings = get_settings(&self.app_handle);

        // Perform transcription with the appropriate engine.
        // We use catch_unwind to prevent engine panics from poisoning the mutex,
        // which would make the app hang indefinitely on subsequent operations.
        let result = {
            let mut engine_guard = self.lock_engine();

            // Take the engine out so we own it during transcription.
            // If the engine panics, we simply don't put it back (effectively unloading it)
            // instead of poisoning the mutex.
            let mut engine = match engine_guard.take() {
                Some(e) => e,
                None => {
                    return Err(anyhow::anyhow!(
                        "Model failed to load after auto-load attempt. Please check your model settings."
                    ));
                }
            };

            // Release the lock before transcribing — no mutex held during the engine call
            drop(engine_guard);

            let transcribe_result = catch_unwind(AssertUnwindSafe(
                || -> Result<transcribe_rs::TranscriptionResult> {
                    match &mut engine {
                        LoadedEngine::Whisper(whisper_engine) => {
                            let whisper_language = if settings.selected_language == "auto" {
                                None
                            } else if settings.selected_language == "os_input" {
                                crate::input_source::get_language_from_input_source()
                            } else {
                                let normalized = if settings.selected_language == "zh-Hans"
                                    || settings.selected_language == "zh-Hant"
                                {
                                    "zh".to_string()
                                } else {
                                    settings.selected_language.clone()
                                };
                                Some(normalized)
                            };

                            let params = WhisperInferenceParams {
                                language: whisper_language,
                                translate: settings.translate_to_english,
                                initial_prompt: build_whisper_initial_prompt(
                                    {
                                        // Get the prompt for current model from the per-model HashMap
                                        let current_model_id =
                                            self.current_model_id.lock().unwrap();
                                        current_model_id
                                            .as_ref()
                                            .and_then(|id| settings.transcription_prompts.get(id))
                                            .filter(|p| !p.trim().is_empty())
                                            .cloned()
                                    },
                                    &settings.custom_words,
                                    apply_custom_words_enabled,
                                ),
                                ..Default::default()
                            };

                            whisper_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))
                        }
                        LoadedEngine::Parakeet(parakeet_engine) => {
                            let params = ParakeetParams {
                                timestamp_granularity: Some(TimestampGranularity::Segment),
                                ..Default::default()
                            };
                            parakeet_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| {
                                    anyhow::anyhow!("Parakeet transcription failed: {}", e)
                                })
                        }
                        LoadedEngine::Moonshine(moonshine_engine) => moonshine_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| anyhow::anyhow!("Moonshine transcription failed: {}", e)),
                        LoadedEngine::MoonshineStreaming(streaming_engine) => streaming_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| {
                                anyhow::anyhow!("Moonshine streaming transcription failed: {}", e)
                            }),
                        LoadedEngine::SenseVoice(sense_voice_engine) => {
                            let language = match settings.selected_language.as_str() {
                                "zh" | "zh-Hans" | "zh-Hant" => Some("zh".to_string()),
                                "en" => Some("en".to_string()),
                                "ja" => Some("ja".to_string()),
                                "ko" => Some("ko".to_string()),
                                "yue" => Some("yue".to_string()),
                                _ => None,
                            };
                            let params = SenseVoiceParams {
                                language,
                                use_itn: Some(true),
                            };
                            sense_voice_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| {
                                    anyhow::anyhow!("SenseVoice transcription failed: {}", e)
                                })
                        }
                        LoadedEngine::GigaAM(gigaam_engine) => gigaam_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| anyhow::anyhow!("GigaAM transcription failed: {}", e)),
                        LoadedEngine::Canary(canary_engine) => {
                            let language = if settings.selected_language == "auto" {
                                None
                            } else {
                                Some(settings.selected_language.clone())
                            };
                            let options = TranscribeOptions {
                                language,
                                translate: settings.translate_to_english,
                                ..Default::default()
                            };
                            canary_engine
                                .transcribe(&audio, &options)
                                .map_err(|e| anyhow::anyhow!("Canary transcription failed: {}", e))
                        }
                        LoadedEngine::Cohere(cohere_engine) => {
                            let language = if settings.selected_language == "auto" {
                                None
                            } else if settings.selected_language == "zh-Hans"
                                || settings.selected_language == "zh-Hant"
                            {
                                Some("zh".to_string())
                            } else {
                                Some(settings.selected_language.clone())
                            };
                            let options = TranscribeOptions {
                                language,
                                ..Default::default()
                            };
                            cohere_engine
                                .transcribe(&audio, &options)
                                .map_err(|e| anyhow::anyhow!("Cohere transcription failed: {}", e))
                        }
                    }
                },
            ));

            match transcribe_result {
                Ok(inner_result) => {
                    // Success or normal error — put the engine back
                    let mut engine_guard = self.lock_engine();
                    *engine_guard = Some(engine);
                    inner_result?
                }
                Err(panic_payload) => {
                    // Engine panicked — do NOT put it back (it's in an unknown state).
                    // The engine is dropped here, effectively unloading it.
                    let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".to_string()
                    };
                    error!(
                        "Transcription engine panicked: {}. Model has been unloaded.",
                        panic_msg
                    );

                    // Clear the model ID so it will be reloaded on next attempt
                    {
                        let mut current_model = self
                            .current_model_id
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        *current_model = None;
                    }

                    let _ = self.app_handle.emit(
                        "model-state-changed",
                        ModelStateEvent {
                            event_type: "unloaded".to_string(),
                            model_id: None,
                            model_name: None,
                            error: Some(format!("Engine panicked: {}", panic_msg)),
                        },
                    );

                    return Err(anyhow::anyhow!(
                        "Transcription engine panicked: {}. The model has been unloaded and will reload on next attempt.",
                        panic_msg
                    ));
                }
            }
        };

        let should_apply_custom_words =
            apply_custom_words_enabled && !settings.custom_words.is_empty();

        // Apply word correction if custom words are enabled and configured
        let corrected_result = if should_apply_custom_words {
            apply_custom_words(
                &result.text,
                &settings.custom_words,
                settings.word_correction_threshold,
                settings.custom_words_ngram_enabled,
            )
        } else {
            result.text
        };

        // Filter out filler words and hallucinations (if enabled)
        let filtered_result = if settings.filler_word_filter_enabled {
            filter_transcription_output(
                &corrected_result,
                &settings.selected_language,
                &settings.custom_filler_words,
            )
        } else {
            corrected_result
        };

        let et = std::time::Instant::now();
        let translation_note = if settings.translate_to_english {
            " (translated)"
        } else {
            ""
        };
        info!(
            "Transcription completed in {}ms{}",
            (et - st).as_millis(),
            translation_note
        );

        let final_result = filtered_result;

        if final_result.is_empty() {
            info!("Transcription result is empty");
        } else {
            info!("Transcription result: {}", final_result);
        }

        self.maybe_unload_immediately("transcription");

        Ok(final_result)
    }

    /// Transcribe audio with optional language/translation/prompt overrides.
    /// Used by transcription profiles to override global settings.
    pub fn transcribe_with_overrides(
        &self,
        audio: Vec<f32>,
        language_override: Option<&str>,
        translate_override: Option<bool>,
        prompt_override: Option<String>,
        apply_custom_words_enabled: bool,
    ) -> Result<String> {
        // Update last activity timestamp
        self.touch_activity();

        let st = std::time::Instant::now();

        debug!("Audio vector length: {} (with overrides)", audio.len());

        if audio.len() == 0 {
            debug!("Empty audio vector");
            return Ok(String::new());
        }

        // Check if model is loaded
        {
            let mut is_loading = self.is_loading.lock().unwrap();
            while *is_loading {
                is_loading = self.loading_condvar.wait(is_loading).unwrap();
            }

            let engine_guard = self.engine.lock().unwrap();
            if engine_guard.is_none() {
                return Err(anyhow::anyhow!("Model is not loaded for transcription."));
            }
        }

        let settings = get_settings(&self.app_handle);

        // Apply overrides
        let selected_language = language_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| settings.selected_language.clone());
        let translate_to_english = translate_override.unwrap_or(settings.translate_to_english);

        let result = {
            let mut engine_guard = self.engine.lock().unwrap();
            let engine = engine_guard.as_mut().ok_or_else(|| {
                anyhow::anyhow!("Model failed to load. Please check your model settings.")
            })?;

            match engine {
                LoadedEngine::Whisper(whisper_engine) => {
                    let whisper_language = if selected_language == "auto" {
                        None
                    } else if selected_language == "os_input" {
                        // Resolve OS input source to language, fall back to auto-detect
                        crate::input_source::get_language_from_input_source()
                    } else {
                        let normalized =
                            if selected_language == "zh-Hans" || selected_language == "zh-Hant" {
                                "zh".to_string()
                            } else {
                                selected_language.clone()
                            };
                        Some(normalized)
                    };

                    let params = WhisperInferenceParams {
                        language: whisper_language,
                        translate: translate_to_english,
                        initial_prompt: build_whisper_initial_prompt(
                            // Priority: 1) profile override, 2) global per-model prompt
                            prompt_override
                                .filter(|p| !p.trim().is_empty())
                                .or_else(|| {
                                    let current_model_id = self.current_model_id.lock().unwrap();
                                    current_model_id
                                        .as_ref()
                                        .and_then(|id| settings.transcription_prompts.get(id))
                                        .filter(|p| !p.trim().is_empty())
                                        .cloned()
                                }),
                            &settings.custom_words,
                            apply_custom_words_enabled,
                        ),
                        ..Default::default()
                    };

                    whisper_engine
                        .transcribe_with(&audio, &params)
                        .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))?
                }
                LoadedEngine::Parakeet(parakeet_engine) => {
                    let params = ParakeetParams {
                        timestamp_granularity: Some(TimestampGranularity::Segment),
                        ..Default::default()
                    };

                    parakeet_engine
                        .transcribe_with(&audio, &params)
                        .map_err(|e| anyhow::anyhow!("Parakeet transcription failed: {}", e))?
                }
                LoadedEngine::Moonshine(moonshine_engine) => moonshine_engine
                    .transcribe(&audio, &TranscribeOptions::default())
                    .map_err(|e| anyhow::anyhow!("Moonshine transcription failed: {}", e))?,
                LoadedEngine::MoonshineStreaming(streaming_engine) => streaming_engine
                    .transcribe(&audio, &TranscribeOptions::default())
                    .map_err(|e| {
                        anyhow::anyhow!("Moonshine streaming transcription failed: {}", e)
                    })?,
                LoadedEngine::SenseVoice(sense_voice_engine) => {
                    let language = match selected_language.as_str() {
                        "zh" | "zh-Hans" | "zh-Hant" => Some("zh".to_string()),
                        "en" => Some("en".to_string()),
                        "ja" => Some("ja".to_string()),
                        "ko" => Some("ko".to_string()),
                        "yue" => Some("yue".to_string()),
                        _ => None,
                    };
                    let params = SenseVoiceParams {
                        language,
                        use_itn: Some(true),
                    };
                    sense_voice_engine
                        .transcribe_with(&audio, &params)
                        .map_err(|e| anyhow::anyhow!("SenseVoice transcription failed: {}", e))?
                }
                LoadedEngine::GigaAM(gigaam_engine) => gigaam_engine
                    .transcribe(&audio, &TranscribeOptions::default())
                    .map_err(|e| anyhow::anyhow!("GigaAM transcription failed: {}", e))?,
                LoadedEngine::Canary(canary_engine) => {
                    let language = if selected_language == "auto" {
                        None
                    } else {
                        Some(selected_language.clone())
                    };
                    let options = TranscribeOptions {
                        language,
                        translate: translate_to_english,
                        ..Default::default()
                    };
                    canary_engine
                        .transcribe(&audio, &options)
                        .map_err(|e| anyhow::anyhow!("Canary transcription failed: {}", e))?
                }
                LoadedEngine::Cohere(cohere_engine) => {
                    let language = if selected_language == "auto" {
                        None
                    } else if selected_language == "zh-Hans" || selected_language == "zh-Hant" {
                        Some("zh".to_string())
                    } else {
                        Some(selected_language.clone())
                    };
                    let options = TranscribeOptions {
                        language,
                        ..Default::default()
                    };
                    cohere_engine
                        .transcribe(&audio, &options)
                        .map_err(|e| anyhow::anyhow!("Cohere transcription failed: {}", e))?
                }
            }
        };

        let should_apply_custom_words =
            apply_custom_words_enabled && !settings.custom_words.is_empty();

        let corrected_result = if should_apply_custom_words {
            apply_custom_words(
                &result.text,
                &settings.custom_words,
                settings.word_correction_threshold,
                settings.custom_words_ngram_enabled,
            )
        } else {
            result.text
        };

        // Filter out filler words and hallucinations (if enabled)
        let filtered_result = if settings.filler_word_filter_enabled {
            filter_transcription_output(
                &corrected_result,
                &selected_language,
                &settings.custom_filler_words,
            )
        } else {
            corrected_result
        };

        let et = std::time::Instant::now();
        let translation_note = if translate_to_english {
            " (translated)"
        } else {
            ""
        };
        info!(
            "Transcription with overrides (lang={}) completed in {}ms{}",
            selected_language,
            (et - st).as_millis(),
            translation_note
        );

        let final_result = filtered_result;

        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately {
            info!("Immediately unloading model after transcription");
            if let Err(e) = self.unload_model() {
                error!("Failed to immediately unload model: {}", e);
            }
        }

        Ok(final_result)
    }

    /// Transcribe audio and return full result including segments with timestamps.
    /// This is used for file transcription where subtitle formats (SRT/VTT) are needed.
    pub fn transcribe_with_segments(
        &self,
        audio: Vec<f32>,
        language_override: Option<&str>,
        translate_override: Option<bool>,
        prompt_override: Option<String>,
        apply_custom_words_enabled: bool,
    ) -> Result<(String, Option<Vec<crate::subtitle::SubtitleSegment>>)> {
        // Update last activity timestamp
        self.touch_activity();

        let st = std::time::Instant::now();

        debug!("Audio vector length: {} (with segments)", audio.len());

        if audio.len() == 0 {
            debug!("Empty audio vector");
            return Ok((String::new(), None));
        }

        // Check if model is loaded
        {
            let mut is_loading = self.is_loading.lock().unwrap();
            while *is_loading {
                is_loading = self.loading_condvar.wait(is_loading).unwrap();
            }

            let engine_guard = self.engine.lock().unwrap();
            if engine_guard.is_none() {
                return Err(anyhow::anyhow!("Model is not loaded for transcription."));
            }
        }

        let settings = get_settings(&self.app_handle);

        // Apply overrides
        let selected_language = language_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| settings.selected_language.clone());
        let translate_to_english = translate_override.unwrap_or(settings.translate_to_english);

        let result = {
            let mut engine_guard = self.engine.lock().unwrap();
            let engine = engine_guard.as_mut().ok_or_else(|| {
                anyhow::anyhow!("Model failed to load. Please check your model settings.")
            })?;

            match engine {
                LoadedEngine::Whisper(whisper_engine) => {
                    let whisper_language = if selected_language == "auto" {
                        None
                    } else if selected_language == "os_input" {
                        // Resolve OS input source to language, fall back to auto-detect
                        crate::input_source::get_language_from_input_source()
                    } else {
                        let normalized =
                            if selected_language == "zh-Hans" || selected_language == "zh-Hant" {
                                "zh".to_string()
                            } else {
                                selected_language.clone()
                            };
                        Some(normalized)
                    };

                    let params = WhisperInferenceParams {
                        language: whisper_language,
                        translate: translate_to_english,
                        initial_prompt: build_whisper_initial_prompt(
                            prompt_override
                                .filter(|p| !p.trim().is_empty())
                                .or_else(|| {
                                    let current_model_id = self.current_model_id.lock().unwrap();
                                    current_model_id
                                        .as_ref()
                                        .and_then(|id| settings.transcription_prompts.get(id))
                                        .filter(|p| !p.trim().is_empty())
                                        .cloned()
                                }),
                            &settings.custom_words,
                            apply_custom_words_enabled,
                        ),
                        ..Default::default()
                    };

                    whisper_engine
                        .transcribe_with(&audio, &params)
                        .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))?
                }
                LoadedEngine::Parakeet(parakeet_engine) => {
                    let params = ParakeetParams {
                        timestamp_granularity: Some(TimestampGranularity::Segment),
                        ..Default::default()
                    };

                    parakeet_engine
                        .transcribe_with(&audio, &params)
                        .map_err(|e| anyhow::anyhow!("Parakeet transcription failed: {}", e))?
                }
                LoadedEngine::Moonshine(moonshine_engine) => moonshine_engine
                    .transcribe(&audio, &TranscribeOptions::default())
                    .map_err(|e| anyhow::anyhow!("Moonshine transcription failed: {}", e))?,
                LoadedEngine::MoonshineStreaming(streaming_engine) => streaming_engine
                    .transcribe(&audio, &TranscribeOptions::default())
                    .map_err(|e| {
                        anyhow::anyhow!("Moonshine streaming transcription failed: {}", e)
                    })?,
                LoadedEngine::SenseVoice(sense_voice_engine) => {
                    let language = match selected_language.as_str() {
                        "zh" | "zh-Hans" | "zh-Hant" => Some("zh".to_string()),
                        "en" => Some("en".to_string()),
                        "ja" => Some("ja".to_string()),
                        "ko" => Some("ko".to_string()),
                        "yue" => Some("yue".to_string()),
                        _ => None,
                    };
                    let params = SenseVoiceParams {
                        language,
                        use_itn: Some(true),
                    };
                    sense_voice_engine
                        .transcribe_with(&audio, &params)
                        .map_err(|e| anyhow::anyhow!("SenseVoice transcription failed: {}", e))?
                }
                LoadedEngine::GigaAM(gigaam_engine) => gigaam_engine
                    .transcribe(&audio, &TranscribeOptions::default())
                    .map_err(|e| anyhow::anyhow!("GigaAM transcription failed: {}", e))?,
                LoadedEngine::Canary(canary_engine) => {
                    let language = if selected_language == "auto" {
                        None
                    } else {
                        Some(selected_language.clone())
                    };
                    let options = TranscribeOptions {
                        language,
                        translate: translate_to_english,
                        ..Default::default()
                    };
                    canary_engine
                        .transcribe(&audio, &options)
                        .map_err(|e| anyhow::anyhow!("Canary transcription failed: {}", e))?
                }
                LoadedEngine::Cohere(cohere_engine) => {
                    let language = if selected_language == "auto" {
                        None
                    } else if selected_language == "zh-Hans" || selected_language == "zh-Hant" {
                        Some("zh".to_string())
                    } else {
                        Some(selected_language.clone())
                    };
                    let options = TranscribeOptions {
                        language,
                        ..Default::default()
                    };
                    cohere_engine
                        .transcribe(&audio, &options)
                        .map_err(|e| anyhow::anyhow!("Cohere transcription failed: {}", e))?
                }
            }
        };

        let should_apply_custom_words =
            apply_custom_words_enabled && !settings.custom_words.is_empty();

        // Convert transcribe_rs segments to our SubtitleSegment format
        let segments: Option<Vec<crate::subtitle::SubtitleSegment>> = result.segments.map(|segs| {
            segs.into_iter()
                .map(|seg| {
                    let text = if should_apply_custom_words {
                        apply_custom_words(
                            &seg.text,
                            &settings.custom_words,
                            settings.word_correction_threshold,
                            settings.custom_words_ngram_enabled,
                        )
                    } else {
                        seg.text
                    };
                    crate::subtitle::SubtitleSegment {
                        start: seg.start,
                        end: seg.end,
                        text,
                    }
                })
                .collect()
        });

        let corrected_result = if should_apply_custom_words {
            apply_custom_words(
                &result.text,
                &settings.custom_words,
                settings.word_correction_threshold,
                settings.custom_words_ngram_enabled,
            )
        } else {
            result.text
        };

        // Filter out filler words and hallucinations (if enabled)
        let filtered_result = if settings.filler_word_filter_enabled {
            filter_transcription_output(
                &corrected_result,
                &selected_language,
                &settings.custom_filler_words,
            )
        } else {
            corrected_result
        };

        let et = std::time::Instant::now();
        let translation_note = if translate_to_english {
            " (translated)"
        } else {
            ""
        };
        info!(
            "Transcription with segments (lang={}) completed in {}ms{}",
            selected_language,
            (et - st).as_millis(),
            translation_note
        );

        let final_result = filtered_result;

        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately {
            info!("Immediately unloading model after transcription");
            if let Err(e) = self.unload_model() {
                error!("Failed to immediately unload model: {}", e);
            }
        }

        Ok((final_result, segments))
    }
}

#[derive(Serialize, Clone, Debug, Type)]
pub struct AvailableAccelerators {
    pub whisper: Vec<String>,
    pub ort: Vec<String>,
    pub gpu_devices: Vec<GpuDeviceOption>,
}

#[derive(Serialize, Clone, Debug, Type)]
pub struct GpuDeviceOption {
    pub id: i32,
    pub name: String,
    pub total_vram_mb: usize,
}

static GPU_DEVICES: OnceLock<Vec<GpuDeviceOption>> = OnceLock::new();

fn cached_gpu_devices() -> &'static [GpuDeviceOption] {
    use transcribe_rs::whisper_cpp::gpu::list_gpu_devices;

    GPU_DEVICES.get_or_init(|| {
        // ggml's Vulkan backend uses FMA3 instructions internally.
        // On older CPUs without FMA3 (e.g. Sandy Bridge Xeons) this causes
        // a SIGILL crash that cannot be caught. Skip enumeration entirely
        // on those CPUs - GPU-accelerated whisper won't work there anyway.
        #[cfg(target_arch = "x86_64")]
        if !std::arch::is_x86_feature_detected!("fma") {
            warn!("CPU lacks FMA3 support - skipping GPU device enumeration");
            return Vec::new();
        }

        list_gpu_devices()
            .into_iter()
            .map(|device| GpuDeviceOption {
                id: device.id,
                name: device.name,
                total_vram_mb: device.total_vram / (1024 * 1024),
            })
            .collect()
    })
}

pub fn apply_accelerator_settings(app: &tauri::AppHandle) {
    use transcribe_rs::accel;

    let settings = get_settings(app);

    let whisper_pref = match settings.whisper_accelerator {
        WhisperAcceleratorSetting::Auto => accel::WhisperAccelerator::Auto,
        WhisperAcceleratorSetting::Cpu => accel::WhisperAccelerator::CpuOnly,
        WhisperAcceleratorSetting::Gpu => accel::WhisperAccelerator::Gpu,
    };
    accel::set_whisper_accelerator(whisper_pref);
    accel::set_whisper_gpu_device(settings.whisper_gpu_device);
    info!(
        "Whisper accelerator set to: {}, gpu_device: {}",
        whisper_pref,
        if settings.whisper_gpu_device == accel::GPU_DEVICE_AUTO {
            "auto".to_string()
        } else {
            settings.whisper_gpu_device.to_string()
        }
    );

    let ort_pref = match settings.ort_accelerator {
        OrtAcceleratorSetting::Auto => accel::OrtAccelerator::Auto,
        OrtAcceleratorSetting::Cpu => accel::OrtAccelerator::CpuOnly,
        OrtAcceleratorSetting::Cuda => accel::OrtAccelerator::Cuda,
        OrtAcceleratorSetting::DirectMl => accel::OrtAccelerator::DirectMl,
        OrtAcceleratorSetting::Rocm => accel::OrtAccelerator::Rocm,
    };
    accel::set_ort_accelerator(ort_pref);
    info!("ORT accelerator set to: {}", ort_pref);
}

pub fn get_available_accelerators() -> AvailableAccelerators {
    use transcribe_rs::accel::OrtAccelerator;

    let ort_options = OrtAccelerator::available()
        .into_iter()
        .map(|accelerator| accelerator.to_string())
        .collect();

    AvailableAccelerators {
        whisper: vec!["auto".to_string(), "cpu".to_string(), "gpu".to_string()],
        ort: ort_options,
        gpu_devices: cached_gpu_devices().to_vec(),
    }
}

impl Drop for TranscriptionManager {
    fn drop(&mut self) {
        if Arc::strong_count(&self.engine) > 1 {
            return;
        }

        // Signal the watcher thread to shutdown
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // Wait for the thread to finish gracefully
        if let Some(handle) = self.watcher_handle.lock().unwrap().take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join idle watcher thread: {:?}", e);
            } else {
                debug!("Idle watcher thread joined successfully");
            }
        }
    }
}
