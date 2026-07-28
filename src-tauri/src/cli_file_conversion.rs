//! First-class CLI file conversion.
//!
//! `--convert-file` mirrors the app's two managed file operations:
//! text/Markdown -> TTS audio, and common audio -> text/Markdown. Provider
//! credentials and conversion behavior continue to come from saved settings.

use crate::cli::CliArgs;
use crate::commands::file_transcription;
use crate::managers::deepgram_stt::DeepgramSttManager;
use crate::managers::model::ModelManager;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::soniox_stt::SonioxSttManager;
use crate::managers::transcription::TranscriptionManager;
use crate::managers::tts::{TtsManager, TtsPhase};
use crate::managers::tts_history::{
    metadata_from_settings, TtsHistoryManager, TtsHistorySourceKind,
};
use crate::settings::{
    get_settings, TranscriptionProvider, TtsOutputFormat, TtsProvider, TtsSettings,
};
use crate::subtitle::OutputFormat;
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const TEXT_EXTENSIONS: &[&str] = &["txt", "md"];
const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "ogg", "flac", "webm"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliFileConversionKind {
    TextToAudio,
    AudioToText,
}

#[derive(Debug)]
struct ConversionPlan {
    kind: CliFileConversionKind,
    input: PathBuf,
}

#[derive(Debug)]
struct CliFailure {
    exit_code: i32,
    message: String,
}

impl CliFailure {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            exit_code: 2,
            message: message.into(),
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            exit_code: 1,
            message: message.into(),
        }
    }
}

/// True when the new symmetric file conversion operation was requested.
///
/// Keep this separate from the legacy `--transcribe-file` benchmark check in
/// `lib.rs`; the two commands intentionally have different initialization and
/// output contracts.
pub fn is_file_conversion_requested(args: &CliArgs) -> bool {
    args.convert_file.is_some()
}

/// Initializes only the managed state needed by the requested conversion.
///
/// This can safely run on the headless worker before `run_file_conversion`.
/// It avoids starting the normal app UI, tray, shortcuts, connector, watcher,
/// and microphone stack.
pub fn initialize_file_conversion_managers(
    app: &AppHandle,
    kind: CliFileConversionKind,
) -> Result<(), String> {
    match kind {
        CliFileConversionKind::TextToAudio => {
            if app.try_state::<Arc<TtsManager>>().is_none() {
                let manager = TtsManager::new(app)
                    .map_err(|error| format!("Failed to initialize Text-to-Speech: {error}"))?;
                app.manage(manager);
            }
            if get_settings(app).tts.history_enabled
                && app.try_state::<Arc<TtsHistoryManager>>().is_none()
            {
                let history = Arc::new(
                    TtsHistoryManager::new(app)
                        .map_err(|error| format!("Failed to initialize TTS History: {error}"))?,
                );
                app.manage(history);
            }
        }
        CliFileConversionKind::AudioToText => {
            let settings = get_settings(app);
            match settings.transcription_provider {
                TranscriptionProvider::Local => {
                    if app.try_state::<Arc<TranscriptionManager>>().is_none() {
                        crate::managers::transcription::init_transcribe_backend();
                        crate::managers::transcription::apply_accelerator_settings(app);
                        let model_manager =
                            Arc::new(ModelManager::new(app).map_err(|error| {
                                format!("Failed to initialize models: {error}")
                            })?);
                        let transcription_manager = Arc::new(
                            TranscriptionManager::new(app, model_manager.clone()).map_err(
                                |error| {
                                    format!(
                                        "Failed to initialize local file transcription: {error}"
                                    )
                                },
                            )?,
                        );
                        app.manage(model_manager);
                        app.manage(transcription_manager);
                    }
                }
                TranscriptionProvider::RemoteOpenAiCompatible => {
                    if app.try_state::<Arc<RemoteSttManager>>().is_none() {
                        let manager = Arc::new(RemoteSttManager::new(app).map_err(|error| {
                            format!("Failed to initialize remote file transcription: {error}")
                        })?);
                        app.manage(manager);
                    }
                }
                TranscriptionProvider::RemoteSoniox => {
                    if app.try_state::<Arc<SonioxSttManager>>().is_none() {
                        let manager = Arc::new(SonioxSttManager::new(app).map_err(|error| {
                            format!("Failed to initialize Soniox file transcription: {error}")
                        })?);
                        app.manage(manager);
                    }
                }
                TranscriptionProvider::RemoteDeepgram => {
                    if app.try_state::<Arc<DeepgramSttManager>>().is_none() {
                        let manager = Arc::new(DeepgramSttManager::new(app).map_err(|error| {
                            format!("Failed to initialize Deepgram file transcription: {error}")
                        })?);
                        app.manage(manager);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Runs one CLI conversion and returns a process exit code.
///
/// Human progress goes to stderr, leaving stdout available for the final human
/// result or one machine-readable JSON object.
pub fn run_file_conversion(app: &AppHandle, args: &CliArgs) -> i32 {
    let result = run_file_conversion_inner(app, args);
    match result {
        Ok(metadata) => {
            if args.json {
                println!("{}", metadata);
            }
            0
        }
        Err(failure) => {
            if args.json {
                println!(
                    "{}",
                    json!({
                        "ok": false,
                        "operation": "file_conversion",
                        "error": failure.message,
                        "exit_code": failure.exit_code,
                    })
                );
            } else {
                eprintln!("error: {}", failure.message);
            }
            failure.exit_code
        }
    }
}

fn run_file_conversion_inner(app: &AppHandle, args: &CliArgs) -> Result<Value, CliFailure> {
    let plan = build_plan(args)?;
    validate_direction_specific_args(args, plan.kind)?;
    initialize_file_conversion_managers(app, plan.kind).map_err(CliFailure::runtime)?;

    tauri::async_runtime::block_on(async {
        match plan.kind {
            CliFileConversionKind::TextToAudio => {
                convert_text_to_audio(app, args, plan.input).await
            }
            CliFileConversionKind::AudioToText => {
                convert_audio_to_text(app, args, plan.input).await
            }
        }
    })
}

fn build_plan(args: &CliArgs) -> Result<ConversionPlan, CliFailure> {
    let input = args
        .convert_file
        .as_ref()
        .ok_or_else(|| CliFailure::usage("--convert-file requires an input path"))?;
    let input = absolute_path(input).map_err(CliFailure::usage)?;
    if !input.is_file() {
        return Err(CliFailure::usage(format!(
            "Input file does not exist: {}",
            input.display()
        )));
    }

    let extension = extension(&input).ok_or_else(|| {
        CliFailure::usage(format!(
            "Input file has no supported extension: {}",
            input.display()
        ))
    })?;
    let kind = if TEXT_EXTENSIONS.contains(&extension.as_str()) {
        CliFileConversionKind::TextToAudio
    } else if AUDIO_EXTENSIONS.contains(&extension.as_str()) {
        CliFileConversionKind::AudioToText
    } else {
        return Err(CliFailure::usage(format!(
            "Unsupported input format .{extension}; use TXT/MD or {}",
            AUDIO_EXTENSIONS.join("/")
        )));
    };

    Ok(ConversionPlan { kind, input })
}

fn validate_direction_specific_args(
    args: &CliArgs,
    kind: CliFileConversionKind,
) -> Result<(), CliFailure> {
    let has_tts_instruction_override = args.tts_prompt.is_some()
        || args.tts_instructions.is_some()
        || args.tts_instructions_file.is_some();
    if kind == CliFileConversionKind::AudioToText && has_tts_instruction_override {
        return Err(CliFailure::usage(
            "--tts-prompt, --tts-instructions, and --tts-instructions-file apply only to TXT/MD to audio conversion",
        ));
    }
    Ok(())
}

async fn convert_text_to_audio(
    app: &AppHandle,
    args: &CliArgs,
    input: PathBuf,
) -> Result<Value, CliFailure> {
    let mut settings = get_settings(app).tts;
    let (output, format) =
        resolve_tts_output(args.output.as_deref(), &input, settings.output_format)?;
    refuse_existing_output(&output)?;
    settings.output_format = format;
    let instruction_source = apply_tts_instruction_override(args, &mut settings)?;
    let history_source = if settings.history_enabled {
        Some(
            app.state::<Arc<TtsManager>>()
                .read_original_text_file(&input)
                .map_err(|error| CliFailure::runtime(error.to_string()))?,
        )
    } else {
        None
    };
    let history_source_kind = if extension(&input).as_deref() == Some("md") {
        TtsHistorySourceKind::Markdown
    } else {
        TtsHistorySourceKind::Text
    };

    if settings.provider != TtsProvider::OpenAi
        && (args.tts_prompt.is_some()
            || args.tts_instructions.is_some()
            || args.tts_instructions_file.is_some())
    {
        return Err(CliFailure::usage(format!(
            "TTS instruction prompts require the saved OpenAI provider; current provider is {}",
            settings.provider.as_str()
        )));
    }
    if settings.provider == TtsProvider::OpenAi
        && (args.tts_prompt.is_some()
            || args.tts_instructions.is_some()
            || args.tts_instructions_file.is_some())
        && !settings.openai_instructions.trim().is_empty()
        && !TtsManager::openai_model_supports_instructions(&settings.openai_model)
    {
        return Err(CliFailure::usage(format!(
            "OpenAI voice instructions require a gpt-4o-mini-tts model; selected model is '{}'",
            settings.openai_model.trim()
        )));
    }

    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    if !args.json {
        eprintln!(
            "TTS: converting {} with {}…",
            input.display(),
            settings.provider.as_str()
        );
    }

    let started = Instant::now();
    let mut operation = Box::pin(manager.convert_text_file(&input, &output, &settings));
    let mut interval = tokio::time::interval(Duration::from_millis(200));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_status = None;
    let result = loop {
        tokio::select! {
            result = &mut operation => break result,
            _ = interval.tick(), if !args.json => {
                let state = manager.current_state();
                let marker = (
                    state.phase,
                    state.completed_chunks,
                    state.total_chunks,
                    state.current_attempt,
                    state.message.clone(),
                );
                if last_status.as_ref() != Some(&marker) {
                    print_tts_progress(&state);
                    last_status = Some(marker);
                }
            }
        }
    }
    .map_err(|error| CliFailure::runtime(error.to_string()))?;

    let output_bytes = fs::metadata(&result.output_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let elapsed_ms = started.elapsed().as_millis();
    let (history_saved, history_entry_id, history_error) = if let Some(source_text) = history_source
    {
        match app
            .try_state::<Arc<TtsHistoryManager>>()
            .ok_or_else(|| anyhow::anyhow!("TTS History manager is unavailable"))
            .and_then(|history| {
                history.save_success(
                    metadata_from_settings(
                        &settings,
                        source_text,
                        history_source_kind,
                        format!(
                            "cli-{}-{}",
                            chrono::Utc::now().timestamp_millis(),
                            result.operation_id
                        ),
                        Some(result.output_path.clone()),
                    ),
                    &result.output_path,
                )
            }) {
            Ok(Some(entry)) => (true, Some(entry.id), None),
            Ok(None) => (false, None, None),
            Err(error) => {
                if !args.json {
                    eprintln!(
                        "warning: TTS output was created, but History capture failed: {error}"
                    );
                }
                (false, None, Some(error.to_string()))
            }
        }
    } else {
        (false, None, None)
    };
    if !args.json {
        eprintln!(
            "TTS: completed {} chunk(s) in {} ms ({} recovered from checkpoint).",
            result.chunk_count, elapsed_ms, result.resumed_chunks
        );
        println!("Created {}", result.output_path.display());
    }

    Ok(json!({
        "ok": true,
        "operation": "text_to_audio",
        "input": input,
        "output": result.output_path,
        "provider": settings.provider,
        "output_format": result.output_format,
        "mp3_bitrate_kbps": result.mp3_bitrate_kbps,
        "source_characters": result.source_character_count,
        "processed_characters": result.processed_character_count,
        "chunks": result.chunk_count,
        "resumed_chunks": result.resumed_chunks,
        "output_bytes": output_bytes,
        "elapsed_ms": elapsed_ms,
        "tts_instruction_source": instruction_source,
        "history_saved": history_saved,
        "history_entry_id": history_entry_id,
        "history_error": history_error,
    }))
}

async fn convert_audio_to_text(
    app: &AppHandle,
    args: &CliArgs,
    input: PathBuf,
) -> Result<Value, CliFailure> {
    let output = resolve_transcription_output(args.output.as_deref(), &input)?;
    refuse_existing_output(&output)?;
    let settings = get_settings(app);
    let provider = transcription_provider_name(settings.transcription_provider);

    if !args.json {
        eprintln!(
            "Transcription: converting {} with {}…",
            input.display(),
            provider
        );
    }

    let started = Instant::now();
    let result = file_transcription::transcribe_audio_file(
        app.clone(),
        input.to_string_lossy().into_owned(),
        None,
        false,
        Some(OutputFormat::Text),
        None,
        None,
        None,
        None,
    )
    .await
    .map_err(CliFailure::runtime)?;

    if !args.json {
        eprintln!("Transcription: writing {} atomically…", output.display());
    }
    write_text_atomic(&output, &result.text).map_err(CliFailure::runtime)?;

    let elapsed_ms = started.elapsed().as_millis();
    let characters = result.text.chars().count();
    let words = result.text.split_whitespace().count();
    let output_bytes = fs::metadata(&output)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if !args.json {
        eprintln!(
            "Transcription: completed {} character(s) in {} ms.",
            characters, elapsed_ms
        );
        println!("Created {}", output.display());
        let preview = text_preview(&result.text, 500);
        if !preview.is_empty() {
            println!("\n{}", preview);
        }
    }

    Ok(json!({
        "ok": true,
        "operation": "audio_to_text",
        "input": input,
        "output": output,
        "provider": provider,
        "characters": characters,
        "words": words,
        "output_bytes": output_bytes,
        "elapsed_ms": elapsed_ms,
        "info_message": result.info_message,
    }))
}

fn resolve_tts_output(
    explicit: Option<&Path>,
    input: &Path,
    saved_format: TtsOutputFormat,
) -> Result<(PathBuf, TtsOutputFormat), CliFailure> {
    if let Some(output) = explicit {
        let output = absolute_path(output).map_err(CliFailure::usage)?;
        let extension = extension(&output).ok_or_else(|| {
            CliFailure::usage("--output must end in .mp3 or .wav for text-to-audio conversion")
        })?;
        let format = match extension.as_str() {
            "mp3" => TtsOutputFormat::Mp3,
            "wav" => TtsOutputFormat::Wav,
            _ => {
                return Err(CliFailure::usage(
                    "--output must end in .mp3 or .wav for text-to-audio conversion",
                ))
            }
        };
        Ok((output, format))
    } else {
        let extension = match saved_format {
            TtsOutputFormat::Mp3 => "mp3",
            TtsOutputFormat::Wav => "wav",
        };
        Ok((input.with_extension(extension), saved_format))
    }
}

fn resolve_transcription_output(
    explicit: Option<&Path>,
    input: &Path,
) -> Result<PathBuf, CliFailure> {
    if let Some(output) = explicit {
        let output = absolute_path(output).map_err(CliFailure::usage)?;
        let extension = extension(&output).ok_or_else(|| {
            CliFailure::usage("--output must end in .txt or .md for audio transcription")
        })?;
        if !TEXT_EXTENSIONS.contains(&extension.as_str()) {
            return Err(CliFailure::usage(
                "--output must end in .txt or .md for audio transcription",
            ));
        }
        Ok(output)
    } else {
        // Markdown is a first-class transcript artifact. It remains plain,
        // readable Markdown when diarization is not enabled.
        Ok(input.with_extension("md"))
    }
}

fn apply_tts_instruction_override(
    args: &CliArgs,
    settings: &mut TtsSettings,
) -> Result<String, CliFailure> {
    if settings.provider != TtsProvider::OpenAi {
        return Ok("not_applicable".to_string());
    }

    if let Some(path) = args.tts_instructions_file.as_deref() {
        settings.openai_instructions = read_utf8_bom(path)?;
        settings.selected_prompt_id.clear();
        return Ok("instructions_file".to_string());
    }
    if let Some(instructions) = args.tts_instructions.as_ref() {
        settings.openai_instructions = instructions.clone();
        settings.selected_prompt_id.clear();
        return Ok("inline".to_string());
    }
    if let Some(name) = args.tts_prompt.as_deref() {
        let matches = settings
            .prompt_presets
            .iter()
            .filter(|preset| preset.name.eq_ignore_ascii_case(name.trim()))
            .collect::<Vec<_>>();
        let preset = match matches.as_slice() {
            [] => {
                let available = settings
                    .prompt_presets
                    .iter()
                    .map(|preset| preset.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let suffix = if available.is_empty() {
                    "No TTS prompt presets are saved.".to_string()
                } else {
                    format!("Available presets: {available}")
                };
                return Err(CliFailure::usage(format!(
                    "Unknown TTS prompt preset '{}'. {suffix}",
                    name.trim()
                )));
            }
            [preset] => *preset,
            _ => {
                return Err(CliFailure::usage(format!(
                    "More than one TTS prompt preset is named '{}'; rename one in Text to Speech settings",
                    name.trim()
                )))
            }
        };
        settings.openai_instructions = preset.instructions.clone();
        settings.selected_prompt_id = preset.id.clone();
        return Ok(format!("preset:{}", preset.name));
    }

    if !settings.selected_prompt_id.trim().is_empty() {
        if let Some(preset) = settings
            .prompt_presets
            .iter()
            .find(|preset| preset.id == settings.selected_prompt_id)
        {
            settings.openai_instructions = preset.instructions.clone();
            return Ok(format!("saved_preset:{}", preset.name));
        }
    }
    Ok("saved_instructions".to_string())
}

fn read_utf8_bom(path: &Path) -> Result<String, CliFailure> {
    let path = absolute_path(path).map_err(CliFailure::usage)?;
    let mut bytes = Vec::new();
    File::open(&path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| {
            CliFailure::usage(format!(
                "Failed to read TTS instructions file {}: {error}",
                path.display()
            ))
        })?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        CliFailure::usage(format!(
            "TTS instructions file must be UTF-8: {}",
            path.display()
        ))
    })
}

fn print_tts_progress(state: &crate::managers::tts::TtsState) {
    match state.phase {
        TtsPhase::Preparing => {
            if let Some(message) = state.message.as_deref() {
                eprintln!("TTS: {message}");
            } else {
                eprintln!("TTS: preparing {} chunk(s)…", state.total_chunks);
            }
        }
        TtsPhase::Synthesizing => eprintln!(
            "TTS: synthesized {}/{} chunk(s)…",
            state.completed_chunks, state.total_chunks
        ),
        TtsPhase::Retrying => {
            let detail = state
                .message
                .as_deref()
                .unwrap_or("provider request failed");
            eprintln!(
                "TTS: retry {} for chunk {}/{} — {}",
                state.current_attempt,
                state.completed_chunks.saturating_add(1),
                state.total_chunks,
                detail
            );
        }
        TtsPhase::Ready => eprintln!("TTS: audio chunks ready…"),
        TtsPhase::Completed => eprintln!("TTS: finalizing output…"),
        TtsPhase::Cancelled => eprintln!("TTS: cancelled."),
        TtsPhase::Error => {
            if let Some(message) = state.message.as_deref() {
                eprintln!("TTS: provider error — {message}");
            }
        }
        TtsPhase::Idle => {}
    }
}

fn refuse_existing_output(output: &Path) -> Result<(), CliFailure> {
    if output.exists() {
        Err(CliFailure::usage(format!(
            "Output file already exists: {}. Choose another path with --output.",
            output.display()
        )))
    } else if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            Err(CliFailure::usage(format!(
                "Output directory does not exist: {}",
                parent.display()
            )))
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

fn write_text_atomic(output: &Path, text: &str) -> Result<(), String> {
    refuse_existing_output(output).map_err(|error| error.message)?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("transcript");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let partial = parent.join(format!(
        ".{file_name}.{}.{}.partial",
        std::process::id(),
        nonce
    ));

    let write_result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .map_err(|error| {
                format!(
                    "Failed to create partial transcript {}: {error}",
                    partial.display()
                )
            })?;
        file.write_all(text.as_bytes()).map_err(|error| {
            format!(
                "Failed to write partial transcript {}: {error}",
                partial.display()
            )
        })?;
        file.flush().map_err(|error| {
            format!(
                "Failed to flush partial transcript {}: {error}",
                partial.display()
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "Failed to sync partial transcript {}: {error}",
                partial.display()
            )
        })?;
        drop(file);

        if output.exists() {
            return Err(format!(
                "Output file appeared while transcription was running: {}",
                output.display()
            ));
        }
        crate::no_clobber::publish_new_file(&partial, output)
            .map_err(|error| format!("Failed to publish transcript {}: {error}", output.display()))
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    write_result
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| format!("Failed to resolve current directory: {error}"))
    }
}

fn transcription_provider_name(provider: TranscriptionProvider) -> &'static str {
    match provider {
        TranscriptionProvider::Local => "local",
        TranscriptionProvider::RemoteOpenAiCompatible => "OpenAI-compatible",
        TranscriptionProvider::RemoteSoniox => "Soniox",
        TranscriptionProvider::RemoteDeepgram => "Deepgram",
    }
}

fn text_preview(text: &str, maximum_characters: usize) -> String {
    let mut preview = text.chars().take(maximum_characters).collect::<String>();
    if text.chars().count() > maximum_characters {
        preview.push('…');
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::TtsPromptPreset;

    #[test]
    fn output_extension_selects_tts_format() {
        let input = std::env::temp_dir().join("aivorelay-cli-input.md");
        let mp3 = std::env::temp_dir().join("aivorelay-cli-output.mp3");
        let wav = std::env::temp_dir().join("aivorelay-cli-output.wav");

        assert_eq!(
            resolve_tts_output(Some(&mp3), &input, TtsOutputFormat::Wav)
                .expect("MP3 output should resolve")
                .1,
            TtsOutputFormat::Mp3
        );
        assert_eq!(
            resolve_tts_output(Some(&wav), &input, TtsOutputFormat::Mp3)
                .expect("WAV output should resolve")
                .1,
            TtsOutputFormat::Wav
        );
    }

    #[test]
    fn inline_instructions_override_named_preset() {
        let mut settings = TtsSettings::default();
        settings.provider = TtsProvider::OpenAi;
        settings.prompt_presets.push(TtsPromptPreset {
            id: "calm".to_string(),
            name: "Calm narrator".to_string(),
            instructions: "Use the saved prompt.".to_string(),
        });
        let args = CliArgs {
            convert_file: Some(PathBuf::from("chapter.md")),
            tts_prompt: Some("Calm narrator".to_string()),
            tts_instructions: Some("Use the inline prompt.".to_string()),
            ..CliArgs::default()
        };

        let source = apply_tts_instruction_override(&args, &mut settings)
            .expect("instructions should resolve");
        assert_eq!(source, "inline");
        assert_eq!(settings.openai_instructions, "Use the inline prompt.");
    }

    #[test]
    fn instructions_file_handles_utf8_bom_and_has_highest_precedence() {
        let path = std::env::temp_dir().join(format!(
            "aivorelay-tts-instructions-{}-{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::write(&path, b"\xEF\xBB\xBFRead from the file.")
            .expect("temporary instructions should be writable");

        let mut settings = TtsSettings::default();
        settings.provider = TtsProvider::OpenAi;
        let args = CliArgs {
            convert_file: Some(PathBuf::from("chapter.md")),
            tts_instructions: Some("Inline".to_string()),
            tts_instructions_file: Some(path.clone()),
            ..CliArgs::default()
        };
        let source = apply_tts_instruction_override(&args, &mut settings)
            .expect("instructions file should resolve");
        let _ = fs::remove_file(path);

        assert_eq!(source, "instructions_file");
        assert_eq!(settings.openai_instructions, "Read from the file.");
    }

    #[test]
    fn text_preview_is_unicode_safe() {
        assert_eq!(text_preview("Привет 🌍", 8), "Привет 🌍");
        assert_eq!(text_preview("日本語テスト", 3), "日本語…");
    }
}
