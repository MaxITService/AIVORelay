//! File transcription commands - transcribe audio files to text
//!
//! Supports common audio formats: wav, mp3, m4a, ogg, flac, webm
//! Uses the same transcription infrastructure as live recording.

use crate::audio_toolkit::apply_custom_words;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, TranscriptionProvider};
use crate::subtitle::{
    get_format_extension, segments_to_srt, segments_to_vtt, OutputFormat, SubtitleSegment,
};
use log::{debug, error, info};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Result of a file transcription operation
#[derive(Serialize, Type)]
pub struct FileTranscriptionResult {
    /// The transcribed text (or formatted SRT/VTT content)
    pub text: String,
    /// Path where the file was saved (if save_to_file was true)
    pub saved_file_path: Option<String>,
    /// The segments with timestamps (only populated for SRT/VTT formats)
    pub segments: Option<Vec<SubtitleSegment>>,
}

/// Supported audio file extensions
const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "ogg", "flac", "webm"];

/// Get the list of supported audio file extensions
#[tauri::command]
#[specta::specta]
pub fn get_supported_audio_extensions() -> Vec<String> {
    SUPPORTED_EXTENSIONS.iter().map(|s| s.to_string()).collect()
}

/// Transcribe an audio file to text
///
/// # Arguments
/// * `file_path` - Path to the audio file
/// * `profile_id` - Optional transcription profile ID (uses active profile if not specified)
/// * `save_to_file` - If true, saves the transcription to a file in Documents folder
/// * `output_format` - Output format: "text" (default), "srt", or "vtt"
/// * `custom_words_enabled_override` - Optional override for applying custom words
///
/// # Returns
/// FileTranscriptionResult with the transcribed text and optional saved file path
#[tauri::command]
#[specta::specta]
pub async fn transcribe_audio_file(
    app: AppHandle,
    file_path: String,
    profile_id: Option<String>,
    save_to_file: bool,
    output_format: Option<OutputFormat>,
    model_override: Option<String>,
    custom_words_enabled_override: Option<bool>,
) -> Result<FileTranscriptionResult, String> {
    let path = PathBuf::from(&file_path);
    let format = output_format.unwrap_or_default();

    // Validate file exists
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    // Validate extension
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "Unsupported audio format: .{}. Supported formats: {}",
            extension,
            SUPPORTED_EXTENSIONS.join(", ")
        ));
    }

    info!(
        "Transcribing audio file: {} (format: {:?})",
        file_path, format
    );

    // Read and decode the audio file to PCM samples
    let samples = decode_audio_file(&path).map_err(|e| {
        error!("Failed to decode audio file: {}", e);
        format!("Failed to decode audio file: {}", e)
    })?;

    if samples.is_empty() {
        return Err("Audio file contains no audio data".to_string());
    }

    debug!("Decoded {} samples from audio file", samples.len());

    // Get settings and determine profile to use
    let settings = get_settings(&app);
    let profile_id = profile_id.unwrap_or_else(|| settings.active_profile_id.clone());
    let profile = settings.transcription_profile(&profile_id);
    let should_unload_override_model = model_override.is_some()
        && settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible;

    let apply_custom_words_enabled =
        custom_words_enabled_override.unwrap_or(settings.custom_words_enabled);
    let should_apply_custom_words = apply_custom_words_enabled && !settings.custom_words.is_empty();

    // Perform transcription - get segments for subtitle formats
    let needs_segments = matches!(format, OutputFormat::Srt | OutputFormat::Vtt);

    // If model_override is provided, we must use the local manager path with that model.
    // Otherwise, check if we should use remote.
    let use_remote = model_override.is_none()
        && settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible;

    let (transcription_text, segments) = if use_remote {
        // Remote STT - currently doesn't support segments
        let remote_manager = app.state::<Arc<RemoteSttManager>>();

        // Determine translate_to_english: use profile setting if available, otherwise global setting
        let translate_to_english = profile
            .as_ref()
            .map(|p| p.translate_to_english)
            .unwrap_or(settings.translate_to_english);

        // Determine language: use profile setting if available, otherwise global setting
        let language = profile
            .as_ref()
            .map(|p| p.language.clone())
            .unwrap_or_else(|| settings.selected_language.clone());

        let prompt = crate::settings::resolve_stt_prompt(
            profile,
            &settings.transcription_prompts,
            &settings.remote_stt.model_id,
        );

        let text = remote_manager
            .transcribe(
                &settings.remote_stt,
                &samples,
                prompt,
                Some(language),
                translate_to_english,
            )
            .await
            .map_err(|e| format!("Remote transcription failed: {}", e))?;

        // Apply custom word corrections
        let corrected = if should_apply_custom_words {
            apply_custom_words(
                &text,
                &settings.custom_words,
                settings.word_correction_threshold,
            )
        } else {
            text
        };

        // Apply filler word filter (if enabled)
        let corrected = if settings.filler_word_filter_enabled {
            crate::audio_toolkit::filter_transcription_output(&corrected)
        } else {
            corrected
        };

        // For remote STT without segment support, create a single segment
        // spanning the estimated duration if subtitle format is requested
        let segs = if needs_segments {
            // Estimate duration: ~150 words per minute average
            let word_count = corrected.split_whitespace().count();
            let estimated_duration = (word_count as f32 / 150.0) * 60.0;
            Some(vec![SubtitleSegment {
                start: 0.0,
                end: estimated_duration.max(1.0),
                text: corrected.clone(),
            }])
        } else {
            None
        };

        (corrected, segs)
    } else {
        // Local transcription with segment support
        let tm = app.state::<Arc<TranscriptionManager>>();

        // If override is provided, load that model first
        if let Some(model_id) = &model_override {
            info!("Using override model: {}", model_id);
            // We need to ensure this model is loaded.
            // Note: The TM currently holds one loaded model. Switching it here might affect global state,
            // but file transcription is a distinct action.
            // However, load_model is async-ish in the background or blocking?
            // `load_model` in TM is synchronous (blocking) but `initiate_model_load` is async.
            // We need it loaded NOW.

            // First check if it's already the current one
            let current = tm.get_current_model();
            if current.as_deref() != Some(model_id) {
                tm.load_model(model_id)
                    .map_err(|e| format!("Failed to load override model: {}", e))?;
            }
        } else {
            // Ensure default model is loaded before transcription
            tm.initiate_model_load();
        }

        let result = if needs_segments {
            // Use the new method that returns segments
            if let Some(p) = &profile {
                tm.transcribe_with_segments(
                    samples,
                    Some(&p.language),
                    Some(p.translate_to_english),
                    crate::settings::resolve_stt_prompt(
                        Some(p),
                        &settings.transcription_prompts,
                        &settings.selected_model,
                    ),
                    apply_custom_words_enabled,
                )
                .map_err(|e| format!("Local transcription failed: {}", e))
            } else {
                tm.transcribe_with_segments(samples, None, None, None, apply_custom_words_enabled)
                    .map_err(|e| format!("Local transcription failed: {}", e))
            }
        } else {
            // Use the standard method for plain text
            let text_result = if let Some(p) = &profile {
                tm.transcribe_with_overrides(
                    samples,
                    Some(&p.language),
                    Some(p.translate_to_english),
                    crate::settings::resolve_stt_prompt(
                        Some(p),
                        &settings.transcription_prompts,
                        &settings.selected_model,
                    ),
                    apply_custom_words_enabled,
                )
                .map_err(|e| format!("Local transcription failed: {}", e))
            } else {
                tm.transcribe(samples, apply_custom_words_enabled)
                    .map_err(|e| format!("Local transcription failed: {}", e))
            };
            text_result.map(|text| (text, None))
        };

        if should_unload_override_model {
            info!("Unloading override model after file transcription");
            if let Err(e) = tm.unload_model() {
                error!("Failed to unload override model: {}", e);
            }
        }

        result?
    };

    // Format the output based on requested format
    let output_text = match format {
        OutputFormat::Text => transcription_text.clone(),
        OutputFormat::Srt => {
            if let Some(ref segs) = segments {
                segments_to_srt(segs)
            } else {
                // Fallback: create single segment
                segments_to_srt(&[SubtitleSegment {
                    start: 0.0,
                    end: 10.0,
                    text: transcription_text.clone(),
                }])
            }
        }
        OutputFormat::Vtt => {
            if let Some(ref segs) = segments {
                segments_to_vtt(segs)
            } else {
                // Fallback: create single segment
                segments_to_vtt(&[SubtitleSegment {
                    start: 0.0,
                    end: 10.0,
                    text: transcription_text.clone(),
                }])
            }
        }
    };

    info!(
        "Transcription completed: {} characters (format: {:?})",
        output_text.len(),
        format
    );

    // Save to file if requested
    let saved_file_path = if save_to_file {
        let output_path = get_output_file_path(&path, format)?;
        std::fs::write(&output_path, &output_text)
            .map_err(|e| format!("Failed to save transcription: {}", e))?;
        info!("Saved transcription to: {}", output_path.display());
        Some(output_path.to_string_lossy().to_string())
    } else {
        None
    };

    Ok(FileTranscriptionResult {
        text: output_text,
        saved_file_path,
        segments,
    })
}

/// Decode an audio file to f32 PCM samples at 16kHz
fn decode_audio_file(path: &PathBuf) -> Result<Vec<f32>, String> {
    use rodio::Source;
    use std::fs::File;
    use std::io::BufReader; // Import trait for sample_rate() and channels()

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // For WAV files, use hound for direct reading
    if extension == "wav" {
        return decode_wav_file(path);
    }

    // For other formats, use rodio's decoder
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);

    let source =
        rodio::Decoder::new(reader).map_err(|e| format!("Failed to decode audio: {}", e))?;

    // Get source sample rate and channels
    let sample_rate = source.sample_rate();
    let channels = source.channels() as usize;

    debug!("Audio file: {} Hz, {} channels", sample_rate, channels);

    // Collect all samples as f32 (rodio decoder outputs f32)
    let samples: Vec<f32> = source.collect();

    // Convert to mono if stereo
    let mono_samples: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if necessary
    let target_sample_rate = 16000;
    let resampled = if sample_rate != target_sample_rate {
        resample_audio(&mono_samples, sample_rate, target_sample_rate)?
    } else {
        mono_samples
    };

    Ok(resampled)
}

/// Decode a WAV file directly using hound
fn decode_wav_file(path: &PathBuf) -> Result<Vec<f32>, String> {
    let reader =
        hound::WavReader::open(path).map_err(|e| format!("Failed to open WAV file: {}", e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    debug!(
        "WAV file: {} Hz, {} channels, {} bits",
        sample_rate, channels, spec.bits_per_sample
    );

    // Read samples based on format
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            // Use i64 for the shift to avoid overflow with 32-bit samples
            let max_val = (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(Result::ok)
            .collect(),
    };

    // Convert to mono if stereo
    let mono_samples: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if necessary
    let target_sample_rate = 16000;
    let resampled = if sample_rate != target_sample_rate {
        resample_audio(&mono_samples, sample_rate, target_sample_rate)?
    } else {
        mono_samples
    };

    Ok(resampled)
}

/// Resample audio from one sample rate to another
fn resample_audio(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, String> {
    use rubato::{FftFixedIn, Resampler};

    // Use a reasonable chunk size
    let chunk_size = 1024.min(samples.len());
    if chunk_size == 0 {
        return Ok(Vec::new());
    }

    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        chunk_size,
        1, // sub_chunks
        1, // channels
    )
    .map_err(|e| format!("Failed to create resampler: {}", e))?;

    let mut output = Vec::new();

    // Process in chunks
    for chunk in samples.chunks(chunk_size) {
        // Pad last chunk if needed
        let mut input_chunk = chunk.to_vec();
        if input_chunk.len() < chunk_size {
            input_chunk.resize(chunk_size, 0.0);
        }

        let result = resampler
            .process(&[input_chunk], None)
            .map_err(|e| format!("Failed to resample audio: {}", e))?;

        if let Some(out_chunk) = result.first() {
            output.extend_from_slice(out_chunk);
        }
    }

    Ok(output)
}

/// Get the output file path for saving transcription
/// Saves to Documents folder with same name as audio file but appropriate extension
fn get_output_file_path(audio_path: &PathBuf, format: OutputFormat) -> Result<PathBuf, String> {
    // Get Documents folder
    let documents_dir =
        dirs::document_dir().ok_or_else(|| "Could not find Documents folder".to_string())?;

    // Create output filename from audio filename
    let stem = audio_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("transcription");

    let ext = get_format_extension(format);
    let output_path = documents_dir.join(format!("{}.{}", stem, ext));

    Ok(output_path)
}
