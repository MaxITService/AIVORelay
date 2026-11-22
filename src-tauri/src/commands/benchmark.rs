//! STT Benchmark commands: capture raw 16 kHz microphone clips on disk and run
//! repeated transcription comparisons across profiles, providers, microphone
//! input boost levels, RNNoise noise cancellation, and VAD sensitivity levels
//! using the same transcription + post-processing pipeline as dictation/history.

use crate::actions::{
    perform_history_live_transcription, perform_transcription_for_profile,
    process_transcription_output, settings_for_history_transcription, should_use_live_streaming,
    TranscriptionOutcome,
};
use crate::audio_toolkit::audio::NoiseSuppressor;
use crate::audio_toolkit::constants::WHISPER_SAMPLE_RATE;
use crate::audio_toolkit::vad::{VadFrame, VoiceActivityDetector};
use crate::audio_toolkit::{
    list_input_devices, read_wav_samples, save_wav_file, AudioCaptureSource, AudioRecorder,
};
use crate::language_detection::LanguageDetectionMode;
use crate::managers::audio::create_smoothed_detector;
use crate::managers::history_replay::{HistoryReplayGuard, HISTORY_REPLAY_CANCELLED};
use crate::portable::app_data_dir;
use crate::session_manager::{ManagedSessionState, SessionState};
use crate::settings::{
    apply_stt_model_selection, get_settings, normalize_soniox_terms, resolve_stt_prompt,
    sanitize_microphone_input_boost_db, AppSettings, GeminiTranscriptionMode, SttModelSelection,
    TranscriptionProvider, VadBackend,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const RNNOISE_FRAME_SAMPLES: usize = (WHISPER_SAMPLE_RATE as usize * 30) / 1000;
const MIN_CLIP_SAMPLES: usize = (WHISPER_SAMPLE_RATE as usize) / 10; // 100 ms
const READY_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const BENCHMARK_CANCELLED_MESSAGE: &str = "Benchmark run cancelled";
const BENCHMARK_DIR_NAME: &str = "benchmark";
const BENCHMARK_STATE_FILE: &str = "state.json";

struct ActiveBenchmarkRecording {
    recorder: AudioRecorder,
    device_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedBenchmarkState {
    #[serde(default = "default_next_clip_number")]
    next_clip_number: u32,
    #[serde(default)]
    clips: Vec<BenchmarkClipInfo>,
    #[serde(default)]
    results: Vec<BenchmarkRunResult>,
}

fn default_next_clip_number() -> u32 {
    1
}

static ACTIVE_RECORDING: LazyLock<Mutex<Option<ActiveBenchmarkRecording>>> =
    LazyLock::new(|| Mutex::new(None));
static STATE_CACHE: LazyLock<Mutex<Option<PersistedBenchmarkState>>> =
    LazyLock::new(|| Mutex::new(None));
static IS_RUNNING: AtomicBool = AtomicBool::new(false);
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
static ACTIVE_REPLAY_ID: AtomicI64 = AtomicI64::new(0);
static NEXT_REPLAY_ID: AtomicI64 = AtomicI64::new(-1000);
static NEXT_SEQ_ID: AtomicU64 = AtomicU64::new(1);

struct BenchmarkRunGuard;

impl BenchmarkRunGuard {
    fn acquire() -> Result<Self, String> {
        if IS_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("A benchmark run is already in progress.".to_string());
        }
        CANCEL_REQUESTED.store(false, Ordering::SeqCst);
        ACTIVE_REPLAY_ID.store(0, Ordering::SeqCst);
        Ok(Self)
    }
}

impl Drop for BenchmarkRunGuard {
    fn drop(&mut self) {
        ACTIVE_REPLAY_ID.store(0, Ordering::SeqCst);
        IS_RUNNING.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkClipInfo {
    pub id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub sample_count: u32,
    pub device_name: String,
    pub recorded_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkStateSnapshot {
    pub clips: Vec<BenchmarkClipInfo>,
    pub results: Vec<BenchmarkRunResult>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkRunRequest {
    pub clip_id: String,
    pub profile_id: Option<String>,
    pub model_selection_override: Option<SttModelSelection>,
    pub boost_db_override: Option<f32>,
    pub noise_cancellation_override: Option<bool>,
    pub filter_silence_override: Option<bool>,
    pub vad_threshold_override: Option<f32>,
    pub vad_backend_override: Option<VadBackend>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkRunResult {
    pub id: String,
    #[serde(default)]
    pub clip_id: String,
    #[serde(default)]
    pub clip_name: String,
    pub timestamp_ms: i64,
    pub label: String,
    pub profile_name: String,
    pub provider_label: String,
    pub model_label: String,
    pub is_live: bool,
    pub text: String,
    #[serde(default)]
    pub raw_text: Option<String>,
    #[serde(default)]
    pub llm_post_processed: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub clip_duration_seconds: f64,
    pub processed_duration_seconds: f64,
    pub settings_text: String,
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn benchmark_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_data_dir(app)
        .map_err(|err| format!("Failed to resolve app data directory: {err}"))?
        .join(BENCHMARK_DIR_NAME);
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to create benchmark directory: {err}"))?;
    Ok(dir)
}

fn is_safe_clip_id(clip_id: &str) -> bool {
    !clip_id.is_empty()
        && clip_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn clip_wav_path(dir: &Path, clip_id: &str) -> Result<PathBuf, String> {
    if !is_safe_clip_id(clip_id) {
        return Err("Invalid benchmark clip ID.".to_string());
    }
    Ok(dir.join(format!("{clip_id}.wav")))
}

fn load_state_from_disk(dir: &Path) -> PersistedBenchmarkState {
    let path = dir.join(BENCHMARK_STATE_FILE);
    let Ok(raw) = fs::read_to_string(&path) else {
        return PersistedBenchmarkState {
            next_clip_number: 1,
            clips: Vec::new(),
            results: Vec::new(),
        };
    };
    let mut state: PersistedBenchmarkState = serde_json::from_str(&raw).unwrap_or_else(|err| {
        log::warn!("Failed to parse benchmark state file, starting fresh: {err}");
        PersistedBenchmarkState {
            next_clip_number: 1,
            clips: Vec::new(),
            results: Vec::new(),
        }
    });
    // Drop clips whose WAV file was deleted externally on disk.
    state.clips.retain(|clip| {
        clip_wav_path(dir, &clip.id)
            .map(|wav| wav.is_file())
            .unwrap_or(false)
    });
    if state.next_clip_number == 0 {
        state.next_clip_number = (state.clips.len() as u32).saturating_add(1);
    }
    state
}

fn save_state_to_disk(dir: &Path, state: &PersistedBenchmarkState) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|err| format!("Failed to serialize benchmark state: {err}"))?;
    let state_path = dir.join(BENCHMARK_STATE_FILE);
    let tmp_path = dir.join(format!("{BENCHMARK_STATE_FILE}.tmp"));
    fs::write(&tmp_path, serialized)
        .map_err(|err| format!("Failed to write benchmark state temp file: {err}"))?;
    if let Err(rename_err) = fs::rename(&tmp_path, &state_path) {
        // Fallback on Windows when target exists
        let _ = fs::remove_file(&state_path);
        fs::rename(&tmp_path, &state_path).map_err(|err| {
            let _ = fs::remove_file(&tmp_path);
            format!("Failed to save benchmark state ({rename_err}): {err}")
        })?;
    }
    Ok(())
}

fn with_state_mut<T>(
    app: &AppHandle,
    persist: bool,
    f: impl FnOnce(&Path, &mut PersistedBenchmarkState) -> Result<T, String>,
) -> Result<T, String> {
    let dir = benchmark_dir(app)?;
    let mut guard = STATE_CACHE
        .lock()
        .map_err(|_| "Benchmark state lock poisoned".to_string())?;
    if guard.is_none() {
        *guard = Some(load_state_from_disk(&dir));
    }
    let state = guard.as_mut().expect("benchmark state initialized");
    let result = f(&dir, state)?;
    if persist {
        save_state_to_disk(&dir, state)?;
    }
    Ok(result)
}

fn resolve_input_device(selected_name: Option<&str>) -> (Option<cpal::Device>, String) {
    let requested = selected_name
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("default"));

    if let Some(name) = requested {
        if let Ok(devices) = list_input_devices() {
            if let Some(found) = devices.into_iter().find(|d| d.name == name) {
                return (Some(found.device), found.name);
            }
        }
    }

    (None, "Default".to_string())
}

fn apply_input_boost(samples: &[f32], boost_db: f32) -> Vec<f32> {
    let sanitized = sanitize_microphone_input_boost_db(boost_db);
    if sanitized <= 0.0 {
        return samples.to_vec();
    }
    let gain = 10f32.powf(sanitized / 20.0);
    samples
        .iter()
        .map(|sample| (*sample * gain).clamp(-1.0, 1.0))
        .collect()
}

fn apply_rnnoise(samples: &[f32]) -> Result<Vec<f32>, String> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let mut suppressor = NoiseSuppressor::new_16khz()?;
    let mut output = Vec::with_capacity(samples.len());
    let mut padded = [0.0f32; RNNOISE_FRAME_SAMPLES];

    for chunk in samples.chunks(RNNOISE_FRAME_SAMPLES) {
        if chunk.len() == RNNOISE_FRAME_SAMPLES {
            output.extend_from_slice(&suppressor.process_16khz_frame(chunk));
        } else {
            padded.fill(0.0);
            padded[..chunk.len()].copy_from_slice(chunk);
            let processed = suppressor.process_16khz_frame(&padded);
            output.extend_from_slice(&processed[..chunk.len()]);
        }
    }

    Ok(output)
}

fn apply_vad_filter(
    app: &AppHandle,
    samples: &[f32],
    settings: &AppSettings,
    backend: VadBackend,
) -> Result<Vec<f32>, String> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let mut detector = create_smoothed_detector(app, settings, backend)
        .map_err(|err| format!("Failed to initialize VAD: {err}"))?;
    let frame_samples = detector.frame_samples();
    let mut filtered = Vec::with_capacity(samples.len());
    let mut padded = vec![0.0f32; frame_samples];

    for chunk in samples.chunks(frame_samples) {
        let frame_slice: &[f32] = if chunk.len() == frame_samples {
            chunk
        } else {
            padded.fill(0.0);
            padded[..chunk.len()].copy_from_slice(chunk);
            &padded
        };
        match detector
            .push_frame(frame_slice)
            .unwrap_or(VadFrame::Speech(frame_slice))
        {
            VadFrame::Speech(speech) => filtered.extend_from_slice(speech),
            VadFrame::Noise => {}
        }
    }

    Ok(filtered)
}

fn provider_and_model_labels(settings: &AppSettings) -> (String, String) {
    match settings.transcription_provider {
        TranscriptionProvider::Local => ("Local".to_string(), settings.selected_model.clone()),
        TranscriptionProvider::RemoteSoniox => {
            ("Soniox".to_string(), settings.soniox_model.clone())
        }
        TranscriptionProvider::RemoteDeepgram => {
            ("Deepgram".to_string(), settings.deepgram_model.clone())
        }
        TranscriptionProvider::RemoteOpenAiCompatible => {
            let preset = settings.remote_stt.provider_preset.trim();
            let provider = match preset {
                "openai" => "OpenAI",
                "groq" => "Groq",
                "google" => "Google",
                "vercel" => "Vercel",
                "reson8" => "Reson8",
                "elevenlabs" => "ElevenLabs",
                "custom" | "" => "Custom OpenAI-Compatible",
                other => other,
            };
            (provider.to_string(), settings.remote_stt.model_id.clone())
        }
    }
}

fn vad_backend_label(backend: VadBackend) -> &'static str {
    match backend {
        VadBackend::Silero => "Silero",
        VadBackend::Earshot => "Earshot",
    }
}

fn gemini_mode_label(mode: GeminiTranscriptionMode) -> &'static str {
    match mode {
        GeminiTranscriptionMode::Smart => "Smart",
        GeminiTranscriptionMode::Verbatim => "Verbatim",
    }
}

struct SettingsSnapshotArgs<'a> {
    settings: &'a AppSettings,
    clip_name: &'a str,
    run_label: &'a str,
    profile_name: &'a str,
    profile_id: &'a str,
    provider_label: &'a str,
    model_label: &'a str,
    is_live: bool,
    language: &'a str,
    translate_to_english: bool,
    device_name: &'a str,
    clip_duration_seconds: f64,
    processed_duration_seconds: f64,
    duration_ms: u64,
    boost_db: f32,
    noise_cancellation_enabled: bool,
    filter_silence_enabled: bool,
    vad_backend: VadBackend,
    vad_threshold: f32,
    stt_prompt: Option<&'a str>,
    llm_post_processed: bool,
    llm_post_process_requested: bool,
}

fn format_settings_snapshot(args: &SettingsSnapshotArgs<'_>) -> String {
    let settings = args.settings;
    let profile = (args.profile_id != "default")
        .then(|| settings.transcription_profile(args.profile_id))
        .flatten();

    let mut out = String::new();
    let _ = writeln!(out, "Clip: {}", args.clip_name);
    let _ = writeln!(out, "Run: {}", args.run_label);
    let _ = writeln!(out, "Profile: {} ({})", args.profile_name, args.profile_id);
    let _ = writeln!(out, "Provider: {}", args.provider_label);
    let _ = writeln!(
        out,
        "Model: {}",
        if args.model_label.is_empty() {
            "(none)"
        } else {
            args.model_label
        }
    );
    let _ = writeln!(
        out,
        "Mode: {}",
        if args.is_live {
            "Live streaming"
        } else {
            "Batch"
        }
    );
    let _ = writeln!(out, "Language: {}", args.language);

    if !settings.spoken_languages.is_empty() {
        let mode_label = match settings.language_detection {
            LanguageDetectionMode::WhenNeeded => "when needed",
            LanguageDetectionMode::Always => "always",
        };
        let _ = writeln!(
            out,
            "Spoken languages: {} (detection: {})",
            settings.spoken_languages.join(", "),
            mode_label
        );
    }

    let _ = writeln!(
        out,
        "Translate to English: {}",
        if args.translate_to_english {
            "On"
        } else {
            "Off"
        }
    );
    let _ = writeln!(out, "Microphone: {}", args.device_name);
    if args.is_live || !args.filter_silence_enabled {
        let _ = writeln!(out, "Clip duration: {:.2} s", args.clip_duration_seconds);
    } else {
        let _ = writeln!(
            out,
            "Clip duration: {:.2} s (after Filter Silence: {:.2} s)",
            args.clip_duration_seconds, args.processed_duration_seconds
        );
    }
    let _ = writeln!(
        out,
        "Processing time: {:.2} s",
        args.duration_ms as f64 / 1000.0
    );
    if args.boost_db <= 0.0 {
        let _ = writeln!(out, "Microphone Input Boost: Off (0 dB)");
    } else {
        let _ = writeln!(out, "Microphone Input Boost: +{:.0} dB", args.boost_db);
    }
    let _ = writeln!(
        out,
        "Noise Cancellation (RNNoise): {}",
        if args.noise_cancellation_enabled {
            "On"
        } else {
            "Off"
        }
    );
    if args.is_live {
        let _ = writeln!(out, "Filter Silence (VAD): Bypassed in live mode");
    } else if args.filter_silence_enabled {
        let _ = writeln!(
            out,
            "Filter Silence (VAD): On ({}, sensitivity {:.1})",
            vad_backend_label(args.vad_backend),
            args.vad_threshold
        );
    } else {
        let _ = writeln!(out, "Filter Silence (VAD): Off");
    }

    if settings.custom_words_enabled {
        let _ = writeln!(
            out,
            "Custom Words: On ({} words)",
            settings.custom_words.len()
        );
    } else {
        let _ = writeln!(out, "Custom Words: Off");
    }

    // Provider-specific relevant settings (never any secrets/API keys).
    match settings.transcription_provider {
        TranscriptionProvider::RemoteSoniox => {
            let strict = profile
                .and_then(|p| p.soniox_language_hints_strict)
                .unwrap_or(settings.soniox_language_hints_strict);
            if !settings.soniox_language_hints.is_empty() {
                let _ = writeln!(
                    out,
                    "Soniox language hints: {} (strict: {})",
                    settings.soniox_language_hints.join(", "),
                    if strict { "On" } else { "Off" }
                );
            }
            let terms = profile
                .map(|p| normalize_soniox_terms(&p.soniox_context_terms))
                .unwrap_or_else(|| normalize_soniox_terms(&settings.soniox_context_terms));
            if !terms.is_empty() {
                let _ = writeln!(out, "Soniox context terms: {}", terms.len());
            }
        }
        TranscriptionProvider::RemoteDeepgram => {
            let _ = writeln!(
                out,
                "Deepgram: smart_format={}, interim_results={}",
                if settings.deepgram_smart_format {
                    "On"
                } else {
                    "Off"
                },
                if settings.deepgram_interim_results {
                    "On"
                } else {
                    "Off"
                }
            );
        }
        TranscriptionProvider::RemoteOpenAiCompatible => {
            let preset = settings.remote_stt.provider_preset.trim();
            if preset == "google" || preset == "vercel" {
                let mode = if args.is_live {
                    settings.gemini_live_mode
                } else {
                    settings
                        .gemini_dictation_mode
                        .unwrap_or(settings.gemini_file_mode)
                };
                let lang = profile
                    .and_then(|p| p.gemini_language_code_override.as_deref())
                    .unwrap_or(settings.gemini_language_code.as_str())
                    .trim();
                let vocab = profile
                    .and_then(|p| p.gemini_custom_vocabulary_override.as_ref())
                    .unwrap_or(&settings.gemini_custom_vocabulary);
                let _ = writeln!(
                    out,
                    "Gemini: mode={}, language={}, vocabulary={} terms",
                    gemini_mode_label(mode),
                    if lang.is_empty() { "auto" } else { lang },
                    vocab.len()
                );
            } else if preset == "openai"
                && settings
                    .remote_stt
                    .model_id
                    .to_ascii_lowercase()
                    .contains("realtime")
            {
                let _ = writeln!(
                    out,
                    "OpenAI Realtime: delay={}, flatten={}",
                    settings.openai_realtime_whisper_delay.as_str(),
                    if settings.openai_realtime_whisper_flatten_enabled {
                        "On"
                    } else {
                        "Off"
                    }
                );
            }
        }
        TranscriptionProvider::Local => {}
    }

    // LLM Post-Processing status from the common pipeline
    let llm_enabled_for_profile = profile
        .map(|p| p.llm_post_process_enabled)
        .unwrap_or(settings.post_process_enabled);
    if args.llm_post_processed || args.llm_post_process_requested || llm_enabled_for_profile {
        let provider_name = settings
            .post_process_providers
            .iter()
            .find(|p| p.id == settings.post_process_provider_id)
            .map(|p| p.label.as_str())
            .unwrap_or(settings.post_process_provider_id.as_str());
        let llm_model = profile
            .and_then(|p| p.llm_model_override.as_deref())
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .or_else(|| {
                settings
                    .post_process_models
                    .get(&settings.post_process_provider_id)
                    .map(String::as_str)
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
            })
            .unwrap_or("(default)");
        let status = if args.llm_post_processed {
            "Applied"
        } else if args.llm_post_process_requested {
            "Enabled (unchanged/skipped)"
        } else {
            "Enabled"
        };
        let _ = writeln!(
            out,
            "LLM Post-Processing: {} ({} · {})",
            status, provider_name, llm_model
        );
    } else {
        let _ = writeln!(out, "LLM Post-Processing: Off");
    }

    let prompt_display = args
        .stt_prompt
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
        .unwrap_or("(none)");
    let _ = write!(out, "STT Prompt: {}", prompt_display);
    out
}

fn ensure_dictation_idle(app: &AppHandle) -> Result<(), String> {
    let session_state = app.state::<ManagedSessionState>();
    let guard = session_state
        .lock()
        .map_err(|_| "Failed to inspect session state".to_string())?;
    if !matches!(*guard, SessionState::Idle) {
        return Err(
            "Finish the active dictation recording or transcription before recording a benchmark clip."
                .to_string(),
        );
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn benchmark_start_recording(app: AppHandle) -> Result<(), String> {
    ensure_dictation_idle(&app)?;
    if IS_RUNNING.load(Ordering::SeqCst) {
        return Err("Cannot record while a benchmark run is in progress.".to_string());
    }
    {
        let guard = ACTIVE_RECORDING
            .lock()
            .map_err(|_| "Benchmark recorder lock poisoned".to_string())?;
        if guard.is_some() {
            return Err("Benchmark recording is already active.".to_string());
        }
    }

    let settings = get_settings(&app);
    let selected_microphone = settings.selected_microphone.clone();
    let selected_channel = settings.selected_channel;
    #[cfg(target_os = "macos")]
    let airpods_mode = settings.airpods_mode;

    let started = tauri::async_runtime::spawn_blocking(
        move || -> Result<ActiveBenchmarkRecording, String> {
            let (device, device_name) = resolve_input_device(selected_microphone.as_deref());
            let mut recorder = AudioRecorder::new()
                .map_err(|err| format!("Failed to create benchmark recorder: {err}"))?
                .with_selected_channel(selected_channel)
                .with_microphone_input_boost_db(0.0)
                .with_microphone_noise_cancellation_enabled(false);
            #[cfg(target_os = "macos")]
            recorder.set_airpods_mode(airpods_mode);

            recorder
                .open_with_source(device, AudioCaptureSource::Microphone)
                .map_err(|err| format!("Failed to open microphone: {err}"))?;
            let ready_rx = recorder.start().map_err(|err| {
                let _ = recorder.close();
                format!("Failed to start benchmark recording: {err}")
            })?;
            let _ = ready_rx.recv_timeout(READY_WAIT_TIMEOUT);

            Ok(ActiveBenchmarkRecording {
                recorder,
                device_name,
            })
        },
    )
    .await
    .map_err(|err| format!("Benchmark recorder task failed: {err}"))??;

    let mut guard = ACTIVE_RECORDING
        .lock()
        .map_err(|_| "Benchmark recorder lock poisoned".to_string())?;
    if guard.is_some() {
        let mut extra = started.recorder;
        let _ = extra.stop();
        let _ = extra.close();
        return Err("Benchmark recording is already active.".to_string());
    }
    *guard = Some(started);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn benchmark_stop_recording(app: AppHandle) -> Result<BenchmarkClipInfo, String> {
    let active = {
        let mut guard = ACTIVE_RECORDING
            .lock()
            .map_err(|_| "Benchmark recorder lock poisoned".to_string())?;
        guard
            .take()
            .ok_or_else(|| "No active benchmark recording.".to_string())?
    };

    let (samples, device_name) = tauri::async_runtime::spawn_blocking(
        move || -> Result<(Vec<f32>, String), String> {
            let mut recorder = active.recorder;
            let report = recorder
                .stop_with_report()
                .map_err(|err| format!("Failed to stop benchmark recording: {err}"))?;
            let _ = recorder.close();
            Ok((report.samples, active.device_name))
        },
    )
    .await
    .map_err(|err| format!("Benchmark stop task failed: {err}"))??;

    if samples.len() < MIN_CLIP_SAMPLES {
        return Err(
            "Recorded clip is too short or empty. Speak a bit longer and check your microphone."
                .to_string(),
        );
    }

    let now_ms = now_unix_ms();
    let seq = NEXT_SEQ_ID.fetch_add(1, Ordering::Relaxed);
    let clip_id = format!("clip-{now_ms}-{seq}");
    let duration_seconds = samples.len() as f64 / WHISPER_SAMPLE_RATE as f64;
    let sample_count = samples.len().min(u32::MAX as usize) as u32;

    let dir = benchmark_dir(&app)?;
    let wav_path = clip_wav_path(&dir, &clip_id)?;
    tauri::async_runtime::spawn_blocking(move || save_wav_file(&wav_path, &samples))
        .await
        .map_err(|err| format!("Failed to save benchmark clip WAV: {err}"))?
        .map_err(|err| format!("Failed to write benchmark clip WAV: {err}"))?;

    with_state_mut(&app, true, |_dir, state| {
        let number = state.next_clip_number.max(1);
        state.next_clip_number = number.saturating_add(1);
        let clip = BenchmarkClipInfo {
            id: clip_id,
            name: format!("Clip #{number}"),
            duration_seconds,
            sample_count,
            device_name,
            recorded_at_ms: now_ms,
        };
        state.clips.insert(0, clip.clone());
        Ok(clip)
    })
}

#[tauri::command]
#[specta::specta]
pub fn benchmark_get_state(app: AppHandle) -> Result<BenchmarkStateSnapshot, String> {
    with_state_mut(&app, false, |_dir, state| {
        Ok(BenchmarkStateSnapshot {
            clips: state.clips.clone(),
            results: state.results.clone(),
        })
    })
}

#[tauri::command]
#[specta::specta]
pub fn benchmark_get_clip_wav(app: AppHandle, clip_id: String) -> Result<Vec<u8>, String> {
    let dir = benchmark_dir(&app)?;
    let wav_path = clip_wav_path(&dir, &clip_id)?;
    fs::read(&wav_path).map_err(|err| format!("Failed to read benchmark clip WAV: {err}"))
}

#[tauri::command]
#[specta::specta]
pub fn benchmark_delete_clip(app: AppHandle, clip_id: String) -> Result<(), String> {
    if IS_RUNNING.load(Ordering::SeqCst) {
        return Err("Cannot delete a clip while a benchmark run is in progress.".to_string());
    }
    with_state_mut(&app, true, |dir, state| {
        let before = state.clips.len();
        state.clips.retain(|clip| clip.id != clip_id);
        if state.clips.len() != before {
            if let Ok(wav_path) = clip_wav_path(dir, &clip_id) {
                let _ = fs::remove_file(wav_path);
            }
        }
        // Keep existing benchmark results intact so the user can delete them manually.
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
pub fn benchmark_delete_result(app: AppHandle, result_id: String) -> Result<(), String> {
    with_state_mut(&app, true, |_dir, state| {
        state.results.retain(|item| item.id != result_id);
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
pub fn benchmark_clear_results(app: AppHandle, clip_id: Option<String>) -> Result<(), String> {
    with_state_mut(&app, true, |_dir, state| {
        match clip_id.as_deref() {
            Some(target_clip_id) => state
                .results
                .retain(|item| item.clip_id != target_clip_id),
            None => state.results.clear(),
        }
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
pub fn benchmark_cancel_run() {
    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    let replay_id = ACTIVE_REPLAY_ID.load(Ordering::SeqCst);
    if replay_id != 0 {
        crate::managers::history_replay::cancel(replay_id);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn benchmark_run_single(
    app: AppHandle,
    request: BenchmarkRunRequest,
) -> Result<BenchmarkRunResult, String> {
    {
        let guard = ACTIVE_RECORDING
            .lock()
            .map_err(|_| "Benchmark recorder lock poisoned".to_string())?;
        if guard.is_some() {
            return Err("Stop recording before running a benchmark.".to_string());
        }
    }

    let _run_guard = BenchmarkRunGuard::acquire()?;

    let (clip_info, wav_path) = with_state_mut(&app, false, |dir, state| {
        let info = state
            .clips
            .iter()
            .find(|c| c.id == request.clip_id)
            .cloned()
            .ok_or_else(|| "Selected benchmark clip was not found.".to_string())?;
        let wav = clip_wav_path(dir, &info.id)?;
        Ok((info, wav))
    })?;

    let raw_samples = tauri::async_runtime::spawn_blocking(move || read_wav_samples(&wav_path))
        .await
        .map_err(|err| format!("Failed to load benchmark clip task: {err}"))?
        .map_err(|err| format!("Failed to load benchmark clip WAV: {err}"))?;

    if raw_samples.is_empty() {
        return Err("Selected benchmark clip contains no audio samples.".to_string());
    }

    let mut effective_settings = get_settings(&app);
    if let Some(profile_id) = request
        .profile_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if profile_id != "default" && effective_settings.transcription_profile(profile_id).is_none()
        {
            return Err(format!(
                "Transcription profile '{}' was not found.",
                profile_id
            ));
        }
        effective_settings.active_profile_id = profile_id.to_string();
    }

    let mut effective_settings = settings_for_history_transcription(effective_settings);
    if let Some(selection) = request.model_selection_override.as_ref() {
        apply_stt_model_selection(&mut effective_settings, selection)?;
    }

    let effective_boost_db = sanitize_microphone_input_boost_db(
        request.boost_db_override.unwrap_or_else(|| {
            let device_for_lookup = if clip_info.device_name == "Default" {
                None
            } else {
                Some(clip_info.device_name.as_str())
            };
            effective_settings.microphone_input_boost_db_for_device(device_for_lookup)
        }),
    );
    let effective_noise_cancellation = request
        .noise_cancellation_override
        .unwrap_or(effective_settings.microphone_noise_cancellation_enabled);
    let effective_filter_silence = request
        .filter_silence_override
        .unwrap_or(effective_settings.filter_silence);
    let effective_vad_backend = request
        .vad_backend_override
        .unwrap_or(effective_settings.vad_backend);
    let effective_vad_threshold = request
        .vad_threshold_override
        .unwrap_or(effective_settings.vad_threshold)
        .clamp(0.1, 0.9);

    effective_settings.microphone_noise_cancellation_enabled = effective_noise_cancellation;
    effective_settings.filter_silence = effective_filter_silence;
    effective_settings.vad_backend = effective_vad_backend;
    effective_settings.vad_threshold = effective_vad_threshold;

    let is_live = should_use_live_streaming(&effective_settings);
    let profile_id = effective_settings.active_profile_id.clone();
    let profile_name = if profile_id == "default" {
        "Default".to_string()
    } else {
        effective_settings
            .transcription_profile(&profile_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| profile_id.clone())
    };
    let (provider_label, model_label) = provider_and_model_labels(&effective_settings);
    let language = effective_settings
        .transcription_profile(&profile_id)
        .map(|p| p.language.clone())
        .unwrap_or_else(|| effective_settings.selected_language.clone());
    let translate_to_english = effective_settings
        .transcription_profile(&profile_id)
        .map(|p| p.translate_to_english)
        .unwrap_or(effective_settings.translate_to_english);
    let stt_prompt = resolve_stt_prompt(
        effective_settings.transcription_profile(&profile_id),
        &effective_settings.transcription_prompts,
        &model_label,
    );

    let run_label = request
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{} · {}", provider_label, model_label));

    let preprocess_app = app.clone();
    let preprocess_settings = effective_settings.clone();
    let processed_samples =
        tauri::async_runtime::spawn_blocking(move || -> Result<Vec<f32>, String> {
            let boosted = apply_input_boost(&raw_samples, effective_boost_db);
            let denoised = if effective_noise_cancellation {
                apply_rnnoise(&boosted)?
            } else {
                boosted
            };
            if !is_live && effective_filter_silence {
                apply_vad_filter(
                    &preprocess_app,
                    &denoised,
                    &preprocess_settings,
                    effective_vad_backend,
                )
            } else {
                Ok(denoised)
            }
        })
        .await
        .map_err(|err| format!("Audio preprocessing failed: {err}"))??;

    if CANCEL_REQUESTED.load(Ordering::SeqCst) {
        return Err(BENCHMARK_CANCELLED_MESSAGE.to_string());
    }

    let processed_duration_seconds = processed_samples.len() as f64 / WHISPER_SAMPLE_RATE as f64;
    let started_at = Instant::now();

    let (stt_text, error) = if processed_samples.is_empty() {
        (String::new(), None)
    } else if is_live {
        let replay_id = NEXT_REPLAY_ID.fetch_sub(1, Ordering::SeqCst);
        let guard = HistoryReplayGuard::start(&app, replay_id, processed_samples.len())
            .map_err(|err| err.to_string())?;
        ACTIVE_REPLAY_ID.store(replay_id, Ordering::SeqCst);
        if CANCEL_REQUESTED.load(Ordering::SeqCst) {
            crate::managers::history_replay::cancel(replay_id);
        }
        let outcome = tokio::select! {
            biased;
            _ = guard.replay.cancelled() => Err(HISTORY_REPLAY_CANCELLED.to_string()),
            result = perform_history_live_transcription(
                &app,
                &processed_samples,
                &effective_settings,
                &guard.replay,
            ) => result,
        };
        match outcome {
            Ok(text) => (text, None),
            Err(err)
                if err == HISTORY_REPLAY_CANCELLED || CANCEL_REQUESTED.load(Ordering::SeqCst) =>
            {
                return Err(BENCHMARK_CANCELLED_MESSAGE.to_string());
            }
            Err(err) => (String::new(), Some(err)),
        }
    } else {
        match perform_transcription_for_profile(
            &app,
            processed_samples,
            None,
            Some(profile_id.clone()),
            &effective_settings,
        )
        .await
        {
            TranscriptionOutcome::Success(text, _) => (text, None),
            TranscriptionOutcome::Cancelled => {
                return Err(BENCHMARK_CANCELLED_MESSAGE.to_string());
            }
            TranscriptionOutcome::Error { message, .. } => (String::new(), Some(message)),
        }
    };

    if CANCEL_REQUESTED.load(Ordering::SeqCst) {
        return Err(BENCHMARK_CANCELLED_MESSAGE.to_string());
    }

    // Run through the exact same common post-processing pipeline (text replacements + profile LLM)
    // as normal dictation and History re-transcription.
    let (final_text, raw_text, llm_post_processed, llm_post_process_requested) =
        if error.is_none() && !stt_text.trim().is_empty() {
            let processed = process_transcription_output(
                &app,
                &effective_settings,
                &stt_text,
                Some(profile_id.as_str()),
                "Benchmark",
                false,
            )
            .await
            .ok_or_else(|| BENCHMARK_CANCELLED_MESSAGE.to_string())?;

            if CANCEL_REQUESTED.load(Ordering::SeqCst) {
                return Err(BENCHMARK_CANCELLED_MESSAGE.to_string());
            }

            let llm_applied = processed.post_processed_text.is_some();
            let raw_opt = if llm_applied || processed.final_text != stt_text {
                Some(stt_text)
            } else {
                None
            };
            (
                processed.final_text,
                raw_opt,
                llm_applied,
                processed.post_process_requested,
            )
        } else {
            (stt_text, None, false, false)
        };

    let duration_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let settings_text = format_settings_snapshot(&SettingsSnapshotArgs {
        settings: &effective_settings,
        clip_name: &clip_info.name,
        run_label: &run_label,
        profile_name: &profile_name,
        profile_id: &profile_id,
        provider_label: &provider_label,
        model_label: &model_label,
        is_live,
        language: &language,
        translate_to_english,
        device_name: &clip_info.device_name,
        clip_duration_seconds: clip_info.duration_seconds,
        processed_duration_seconds,
        duration_ms,
        boost_db: effective_boost_db,
        noise_cancellation_enabled: effective_noise_cancellation,
        filter_silence_enabled: effective_filter_silence,
        vad_backend: effective_vad_backend,
        vad_threshold: effective_vad_threshold,
        stt_prompt: stt_prompt.as_deref(),
        llm_post_processed,
        llm_post_process_requested,
    });

    let now_ms = now_unix_ms();
    let result_seq = NEXT_SEQ_ID.fetch_add(1, Ordering::Relaxed);
    let result = BenchmarkRunResult {
        id: format!("bench-{now_ms}-{result_seq}"),
        clip_id: clip_info.id,
        clip_name: clip_info.name,
        timestamp_ms: now_ms,
        label: run_label,
        profile_name,
        provider_label,
        model_label,
        is_live,
        text: final_text,
        raw_text,
        llm_post_processed,
        error,
        duration_ms,
        clip_duration_seconds: clip_info.duration_seconds,
        processed_duration_seconds,
        settings_text,
    };

    with_state_mut(&app, true, |_dir, state| {
        state.results.insert(0, result.clone());
        Ok(())
    })?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_toolkit::constants::MAX_MICROPHONE_INPUT_BOOST_DB;
    use crate::settings::{get_default_settings, OpenAiRealtimeWhisperDelay};

    static NEXT_TEST_DIR_ID: AtomicU64 = AtomicU64::new(1);

    struct TempTestDir(PathBuf);

    impl TempTestDir {
        fn new(prefix: &str) -> Self {
            let unique = NEXT_TEST_DIR_ID.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "aivorelay-{prefix}-{}-{}-{unique}",
                std::process::id(),
                now_unix_ms()
            ));
            fs::create_dir_all(&dir).expect("create isolated benchmark temp dir");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempTestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample_clip(id: &str, name: &str) -> BenchmarkClipInfo {
        BenchmarkClipInfo {
            id: id.to_string(),
            name: name.to_string(),
            duration_seconds: 1.5,
            sample_count: 24_000,
            device_name: "Test Mic".to_string(),
            recorded_at_ms: 1_700_000_000_000,
        }
    }

    fn sample_result(id: &str, clip_id: &str, clip_name: &str) -> BenchmarkRunResult {
        BenchmarkRunResult {
            id: id.to_string(),
            clip_id: clip_id.to_string(),
            clip_name: clip_name.to_string(),
            timestamp_ms: 1_700_000_001_000,
            label: "Current".to_string(),
            profile_name: "Default".to_string(),
            provider_label: "Local".to_string(),
            model_label: "whisper-large-v3-turbo".to_string(),
            is_live: false,
            text: "Hello world".to_string(),
            raw_text: None,
            llm_post_processed: false,
            error: None,
            duration_ms: 350,
            clip_duration_seconds: 1.5,
            processed_duration_seconds: 1.5,
            settings_text: "Clip: Clip #1".to_string(),
        }
    }

    #[test]
    fn boost_zero_leaves_samples_unchanged_and_clamps_high_values() {
        let input = vec![0.1, -0.25, 0.5];
        assert_eq!(apply_input_boost(&input, 0.0), input);

        let boosted = apply_input_boost(&[0.5, -0.5], 100.0);
        let expected_gain = 10f32.powf(MAX_MICROPHONE_INPUT_BOOST_DB / 20.0);
        assert_eq!(boosted[0], (0.5 * expected_gain).clamp(-1.0, 1.0));
        assert_eq!(boosted[1], (-0.5 * expected_gain).clamp(-1.0, 1.0));
    }

    #[test]
    fn rnnoise_preserves_sample_count_including_partial_tail() {
        let samples = vec![0.05f32; RNNOISE_FRAME_SAMPLES + 123];
        let output = apply_rnnoise(&samples).expect("RNNoise should succeed");
        assert_eq!(output.len(), samples.len());
    }

    #[test]
    fn settings_snapshot_formats_clear_plain_text_without_secrets() {
        let mut settings = get_default_settings();
        settings.custom_words_enabled = true;
        settings.custom_words = vec!["AivoRelay".to_string()];
        settings.post_process_enabled = true;

        let text = format_settings_snapshot(&SettingsSnapshotArgs {
            settings: &settings,
            clip_name: "Clip #1",
            run_label: "Boost +6 dB",
            profile_name: "Default",
            profile_id: "default",
            provider_label: "Local",
            model_label: "whisper-large-v3-turbo",
            is_live: false,
            language: "ru",
            translate_to_english: false,
            device_name: "USB Mic",
            clip_duration_seconds: 4.5,
            processed_duration_seconds: 3.9,
            duration_ms: 820,
            boost_db: 6.0,
            noise_cancellation_enabled: true,
            filter_silence_enabled: true,
            vad_backend: VadBackend::Silero,
            vad_threshold: 0.5,
            stt_prompt: None,
            llm_post_processed: true,
            llm_post_process_requested: true,
        });
        assert!(text.contains("Clip: Clip #1"));
        assert!(text.contains("Run: Boost +6 dB"));
        assert!(text.contains("Microphone Input Boost: +6 dB"));
        assert!(text.contains("Noise Cancellation (RNNoise): On"));
        assert!(text.contains("Filter Silence (VAD): On (Silero, sensitivity 0.5)"));
        assert!(text.contains("Clip duration: 4.50 s (after Filter Silence: 3.90 s)"));
        assert!(text.contains("Custom Words: On (1 words)"));
        assert!(text.contains("LLM Post-Processing: Applied"));
    }

    #[test]
    fn clip_id_validation_rejects_path_traversal() {
        assert!(is_safe_clip_id("clip-1700000-1"));
        assert!(!is_safe_clip_id("../secret"));
        assert!(!is_safe_clip_id("clip/sub"));
        assert!(!is_safe_clip_id(""));
    }

    #[test]
    fn state_persistence_roundtrip_and_drops_missing_wav_clips() {
        let temp_dir = TempTestDir::new("bench-roundtrip");
        let keep_wav = clip_wav_path(temp_dir.path(), "clip-keep").expect("valid clip id");
        save_wav_file(&keep_wav, &[0.0f32; 320]).expect("write wav for existing clip");

        let state = PersistedBenchmarkState {
            next_clip_number: 7,
            clips: vec![
                sample_clip("clip-keep", "Clip #5"),
                sample_clip("clip-missing", "Clip #6"),
            ],
            results: vec![
                sample_result("res-1", "clip-keep", "Clip #5"),
                sample_result("res-2", "clip-missing", "Clip #6"),
            ],
        };
        save_state_to_disk(temp_dir.path(), &state).expect("persist benchmark state");

        let reloaded = load_state_from_disk(temp_dir.path());
        assert_eq!(reloaded.next_clip_number, 7);
        assert_eq!(reloaded.clips.len(), 1);
        assert_eq!(reloaded.clips[0].id, "clip-keep");
        assert_eq!(reloaded.results.len(), 2);
        assert_eq!(reloaded.results[0].id, "res-1");
        assert_eq!(reloaded.results[1].id, "res-2");
        assert_eq!(reloaded.results[1].clip_id, "clip-missing");
    }

    #[test]
    fn corrupt_state_json_starts_fresh() {
        let temp_dir = TempTestDir::new("bench-corrupt");
        fs::write(
            temp_dir.path().join(BENCHMARK_STATE_FILE),
            "{not-valid-json",
        )
        .expect("write corrupt state.json");

        let reloaded = load_state_from_disk(temp_dir.path());
        assert!(reloaded.clips.is_empty());
        assert!(reloaded.results.is_empty());
        assert_eq!(reloaded.next_clip_number, 1);
    }

    #[test]
    fn settings_snapshot_covers_provider_details_and_llm_states() {
        let fake_api_key = "sk-benchmark-fake-secret-99999";
        let fake_connector_password = "benchmark-fake-connector-password-77777";

        let make_base_settings = || {
            let mut settings = get_default_settings();
            let provider_id = settings.post_process_provider_id.clone();
            settings
                .post_process_api_keys
                .insert(provider_id, fake_api_key.to_string());
            settings
                .post_process_api_keys
                .insert("openai".to_string(), fake_api_key.to_string());
            settings.connector_password = fake_connector_password.into();
            settings
        };

        let assert_no_secrets = |snapshot: &str| {
            assert!(!snapshot.contains(fake_api_key));
            assert!(!snapshot.contains(fake_connector_password));
        };

        // 1. Soniox (live mode, language_hints, strict, normalized context_terms, bypassed VAD, LLM Off)
        let mut soniox_settings = make_base_settings();
        soniox_settings.transcription_provider = TranscriptionProvider::RemoteSoniox;
        soniox_settings.soniox_language_hints = vec!["en".to_string(), "uk".to_string()];
        soniox_settings.soniox_language_hints_strict = true;
        soniox_settings.soniox_context_terms = vec![
            " AivoRelay ".to_string(),
            "".to_string(),
            "   ".to_string(),
            "AivoRelay".to_string(),
            "Soniox".to_string(),
        ];
        soniox_settings.post_process_enabled = false;
        let soniox_snapshot = format_settings_snapshot(&SettingsSnapshotArgs {
            settings: &soniox_settings,
            clip_name: "Clip #1",
            run_label: "Soniox Live",
            profile_name: "Default",
            profile_id: "default",
            provider_label: "Soniox",
            model_label: "stt-rt-v6",
            is_live: true,
            language: "auto",
            translate_to_english: false,
            device_name: "Default",
            clip_duration_seconds: 3.0,
            processed_duration_seconds: 3.0,
            duration_ms: 400,
            boost_db: 0.0,
            noise_cancellation_enabled: false,
            filter_silence_enabled: true,
            vad_backend: VadBackend::Silero,
            vad_threshold: 0.5,
            stt_prompt: None,
            llm_post_processed: false,
            llm_post_process_requested: false,
        });
        assert!(soniox_snapshot.contains("Mode: Live streaming"));
        assert!(soniox_snapshot.contains("Filter Silence (VAD): Bypassed in live mode"));
        assert!(soniox_snapshot.contains("Soniox language hints: en, uk (strict: On)"));
        assert!(soniox_snapshot.contains("Soniox context terms: 2"));
        assert!(soniox_snapshot.contains("LLM Post-Processing: Off"));
        assert_no_secrets(&soniox_snapshot);

        // 2. Deepgram (smart_format, interim_results, LLM Enabled unchanged/skipped)
        let mut deepgram_settings = make_base_settings();
        deepgram_settings.transcription_provider = TranscriptionProvider::RemoteDeepgram;
        deepgram_settings.deepgram_smart_format = true;
        deepgram_settings.deepgram_interim_results = false;
        deepgram_settings.post_process_enabled = true;
        let deepgram_snapshot = format_settings_snapshot(&SettingsSnapshotArgs {
            settings: &deepgram_settings,
            clip_name: "Clip #2",
            run_label: "Deepgram Batch",
            profile_name: "Default",
            profile_id: "default",
            provider_label: "Deepgram",
            model_label: "nova-3",
            is_live: false,
            language: "en",
            translate_to_english: false,
            device_name: "USB Mic",
            clip_duration_seconds: 2.5,
            processed_duration_seconds: 2.5,
            duration_ms: 310,
            boost_db: 0.0,
            noise_cancellation_enabled: false,
            filter_silence_enabled: false,
            vad_backend: VadBackend::Silero,
            vad_threshold: 0.5,
            stt_prompt: None,
            llm_post_processed: false,
            llm_post_process_requested: true,
        });
        assert!(deepgram_snapshot.contains("Deepgram: smart_format=On, interim_results=Off"));
        assert!(deepgram_snapshot.contains("LLM Post-Processing: Enabled (unchanged/skipped)"));
        assert_no_secrets(&deepgram_snapshot);

        // 3. Google Gemini (batch)
        let mut google_gemini_settings = make_base_settings();
        google_gemini_settings.transcription_provider =
            TranscriptionProvider::RemoteOpenAiCompatible;
        google_gemini_settings.remote_stt.provider_preset = "google".to_string();
        google_gemini_settings.remote_stt.model_id = "gemini-3.5-transcribe".to_string();
        google_gemini_settings.gemini_dictation_mode = Some(GeminiTranscriptionMode::Verbatim);
        google_gemini_settings.gemini_file_mode = GeminiTranscriptionMode::Smart;
        google_gemini_settings.gemini_language_code = "en-US".to_string();
        google_gemini_settings.gemini_custom_vocabulary =
            vec!["AivoRelay".to_string(), "Tauri".to_string()];
        google_gemini_settings.post_process_enabled = true;
        let google_gemini_snapshot = format_settings_snapshot(&SettingsSnapshotArgs {
            settings: &google_gemini_settings,
            clip_name: "Clip #3",
            run_label: "Google Gemini Batch",
            profile_name: "Default",
            profile_id: "default",
            provider_label: "Google",
            model_label: "gemini-3.5-transcribe",
            is_live: false,
            language: "en",
            translate_to_english: false,
            device_name: "USB Mic",
            clip_duration_seconds: 4.0,
            processed_duration_seconds: 3.6,
            duration_ms: 620,
            boost_db: 3.0,
            noise_cancellation_enabled: true,
            filter_silence_enabled: true,
            vad_backend: VadBackend::Earshot,
            vad_threshold: 0.4,
            stt_prompt: Some("Technical dictation"),
            llm_post_processed: true,
            llm_post_process_requested: true,
        });
        assert!(google_gemini_snapshot.contains("Mode: Batch"));
        assert!(
            google_gemini_snapshot
                .contains("Gemini: mode=Verbatim, language=en-US, vocabulary=2 terms")
        );
        assert!(google_gemini_snapshot.contains("LLM Post-Processing: Applied"));
        assert_no_secrets(&google_gemini_snapshot);

        // 4. Vercel Gemini (live)
        let mut vercel_gemini_settings = make_base_settings();
        vercel_gemini_settings.transcription_provider =
            TranscriptionProvider::RemoteOpenAiCompatible;
        vercel_gemini_settings.remote_stt.provider_preset = "vercel".to_string();
        vercel_gemini_settings.remote_stt.model_id =
            "google/gemini-3.5-transcribe-live".to_string();
        vercel_gemini_settings.gemini_live_mode = GeminiTranscriptionMode::Smart;
        vercel_gemini_settings.gemini_language_code = String::new();
        vercel_gemini_settings.gemini_custom_vocabulary = vec!["Handy".to_string()];
        vercel_gemini_settings.post_process_enabled = true;
        let vercel_gemini_snapshot = format_settings_snapshot(&SettingsSnapshotArgs {
            settings: &vercel_gemini_settings,
            clip_name: "Clip #4",
            run_label: "Vercel Gemini Live",
            profile_name: "Default",
            profile_id: "default",
            provider_label: "Vercel",
            model_label: "google/gemini-3.5-transcribe-live",
            is_live: true,
            language: "auto",
            translate_to_english: false,
            device_name: "USB Mic",
            clip_duration_seconds: 5.0,
            processed_duration_seconds: 5.0,
            duration_ms: 910,
            boost_db: 0.0,
            noise_cancellation_enabled: false,
            filter_silence_enabled: true,
            vad_backend: VadBackend::Silero,
            vad_threshold: 0.5,
            stt_prompt: None,
            llm_post_processed: true,
            llm_post_process_requested: true,
        });
        assert!(vercel_gemini_snapshot.contains("Mode: Live streaming"));
        assert!(vercel_gemini_snapshot.contains("Filter Silence (VAD): Bypassed in live mode"));
        assert!(
            vercel_gemini_snapshot
                .contains("Gemini: mode=Smart, language=auto, vocabulary=1 terms")
        );
        assert!(vercel_gemini_snapshot.contains("LLM Post-Processing: Applied"));
        assert_no_secrets(&vercel_gemini_snapshot);

        // 5. OpenAI Realtime (delay, flatten)
        let mut openai_realtime_settings = make_base_settings();
        openai_realtime_settings.transcription_provider =
            TranscriptionProvider::RemoteOpenAiCompatible;
        openai_realtime_settings.remote_stt.provider_preset = "openai".to_string();
        openai_realtime_settings.remote_stt.model_id = "gpt-realtime-whisper".to_string();
        openai_realtime_settings.openai_realtime_whisper_delay = OpenAiRealtimeWhisperDelay::High;
        openai_realtime_settings.openai_realtime_whisper_flatten_enabled = true;
        openai_realtime_settings.post_process_enabled = false;
        let openai_realtime_snapshot = format_settings_snapshot(&SettingsSnapshotArgs {
            settings: &openai_realtime_settings,
            clip_name: "Clip #5",
            run_label: "OpenAI Realtime",
            profile_name: "Default",
            profile_id: "default",
            provider_label: "OpenAI",
            model_label: "gpt-realtime-whisper",
            is_live: false,
            language: "en",
            translate_to_english: false,
            device_name: "USB Mic",
            clip_duration_seconds: 3.5,
            processed_duration_seconds: 3.5,
            duration_ms: 500,
            boost_db: 0.0,
            noise_cancellation_enabled: false,
            filter_silence_enabled: false,
            vad_backend: VadBackend::Silero,
            vad_threshold: 0.5,
            stt_prompt: None,
            llm_post_processed: false,
            llm_post_process_requested: false,
        });
        assert!(openai_realtime_snapshot.contains("OpenAI Realtime: delay=high, flatten=On"));
        assert!(openai_realtime_snapshot.contains("LLM Post-Processing: Off"));
        assert_no_secrets(&openai_realtime_snapshot);
    }
}
