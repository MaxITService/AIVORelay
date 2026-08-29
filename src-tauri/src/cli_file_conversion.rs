//! First-class CLI file conversion.
//!
//! `--convert-file` mirrors the app's two managed file operations:
//! text/Markdown -> TTS audio, and common audio -> text/Markdown. Provider
//! credentials and conversion behavior continue to come from saved settings.

use crate::cli::{
    CliArgs, CliElevenLabsTextNormalization, CliTtsKeySource, CliTtsOutputFormat,
    CliTtsProvider,
};
use crate::commands::file_transcription;
use crate::managers::deepgram_stt::DeepgramSttManager;
use crate::managers::model::ModelManager;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::soniox_stt::SonioxSttManager;
use crate::managers::transcription::TranscriptionManager;
use crate::managers::tts::{
    TtsManager, TtsPhase, OPENAI_TTS_INSTRUCTIONS_MAX_CHARS, SUPPORTED_MP3_BITRATES,
};
use crate::managers::tts_history::{
    metadata_from_settings, TtsHistoryManager, TtsHistoryScope, TtsHistorySourceKind,
};
use crate::settings::{
    get_settings, ElevenLabsTextNormalization, LLMPrompt, TextReplacement,
    TranscriptionProvider, TtsKeySource, TtsOutputFormat, TtsProvider, TtsSettings,
    DEFAULT_TTS_MURF_GEN2_VOICE, DEFAULT_TTS_MURF_VOICE,
};
use crate::subtitle::OutputFormat;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const TEXT_EXTENSIONS: &[&str] = &["txt", "md"];
const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "ogg", "flac", "webm"];
const TTS_LLM_INSTRUCTIONS_MAX_CHARS: usize = 32_768;
const UTF8_BOM_BYTES: usize = 3;
const UTF8_MAX_BYTES_PER_CHAR: usize = 4;
const HEADLESS_CONTROL_FILE_PREFIX: &str = "aivorelay-headless-conversion-";
const HEADLESS_CONTROL_FILE_SUFFIX: &str = ".ctl";
static HEADLESS_CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

pub struct HeadlessCancelListener {
    registry_path: PathBuf,
    shutdown: Arc<AtomicBool>,
}

impl Drop for HeadlessCancelListener {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = fs::remove_file(&self.registry_path);
    }
}

fn headless_control_dir() -> PathBuf {
    std::env::temp_dir().join("aivorelay-headless-control")
}

fn cancel_active_headless_conversion(app: &AppHandle) {
    if let Some(manager) = app.try_state::<Arc<TtsManager>>() {
        manager.cancel_active_batch();
        let operation_id = manager.current_state().operation_id;
        if operation_id != 0 {
            manager.cancel_operation(operation_id);
        }
    }
    if let Some(manager) = app.try_state::<Arc<TranscriptionManager>>() {
        manager.cancel_file_transcription();
    }
    if let Some(manager) = app.try_state::<Arc<RemoteSttManager>>() {
        manager.cancel();
    }
    if let Some(manager) = app.try_state::<Arc<SonioxSttManager>>() {
        manager.cancel();
    }
    if let Some(manager) = app.try_state::<Arc<DeepgramSttManager>>() {
        manager.cancel();
    }
}

pub fn start_headless_cancel_listener(
    app: &AppHandle,
) -> Result<HeadlessCancelListener, String> {
    HEADLESS_CANCEL_REQUESTED.store(false, Ordering::SeqCst);
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("Failed to open headless cancellation endpoint: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure headless cancellation endpoint: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Failed to read headless cancellation endpoint: {error}"))?
        .port();
    let token = format!(
        "{}-{}-{}",
        std::process::id(),
        port,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let control_dir = headless_control_dir();
    fs::create_dir_all(&control_dir).map_err(|error| {
        format!("Failed to create headless cancellation directory: {error}")
    })?;
    let registry_path = control_dir.join(format!(
        "{HEADLESS_CONTROL_FILE_PREFIX}{}-{port}{HEADLESS_CONTROL_FILE_SUFFIX}",
        std::process::id()
    ));
    fs::write(&registry_path, format!("{port}\n{token}\n")).map_err(|error| {
        format!("Failed to publish headless cancellation endpoint: {error}")
    })?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = Arc::clone(&shutdown);
    let app = app.clone();
    std::thread::spawn(move || {
        while !thread_shutdown.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                    let mut request = String::new();
                    let _ = (&mut stream).take(512).read_to_string(&mut request);
                    if request.trim() == token {
                        HEADLESS_CANCEL_REQUESTED.store(true, Ordering::SeqCst);
                        cancel_active_headless_conversion(&app);
                        let _ = stream.write_all(b"ok\n");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    });

    Ok(HeadlessCancelListener {
        registry_path,
        shutdown,
    })
}

pub fn request_headless_conversion_cancel() -> bool {
    let Ok(entries) = fs::read_dir(headless_control_dir()) else {
        return false;
    };
    let mut cancelled = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.starts_with(HEADLESS_CONTROL_FILE_PREFIX)
            || !file_name.ends_with(HEADLESS_CONTROL_FILE_SUFFIX)
        {
            continue;
        }
        let Ok(control) = fs::read_to_string(&path) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        let mut lines = control.lines();
        let Some(port) = lines.next().and_then(|value| value.parse::<u16>().ok()) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        let Some(token) = lines.next().filter(|value| !value.is_empty()) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(400)) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
        if stream.write_all(format!("{token}\n").as_bytes()).is_err() {
            continue;
        }
        let _ = stream.shutdown(Shutdown::Write);
        let mut response = String::new();
        if stream.read_to_string(&mut response).is_ok() && response.trim() == "ok" {
            cancelled = true;
        }
    }
    cancelled
}

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

    fn cancelled() -> Self {
        Self {
            exit_code: 130,
            message: "File conversion was cancelled".to_string(),
        }
    }
}

fn ensure_headless_conversion_not_cancelled() -> Result<(), CliFailure> {
    if HEADLESS_CANCEL_REQUESTED.load(Ordering::SeqCst) {
        Err(CliFailure::cancelled())
    } else {
        Ok(())
    }
}

/// True when the new symmetric file conversion operation was requested.
///
/// Keep this separate from the legacy `--transcribe-file` benchmark check in
/// `lib.rs`; the two commands intentionally have different initialization and
/// output contracts.
pub fn is_file_conversion_requested(args: &CliArgs) -> bool {
    !args.convert_file.is_empty()
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
    if HEADLESS_CANCEL_REQUESTED.load(Ordering::SeqCst) {
        if args.json {
            println!(
                "{}",
                json!({
                    "ok": false,
                    "operation": "file_conversion",
                    "error": "File conversion was cancelled",
                })
            );
        } else {
            eprintln!("File conversion was cancelled.");
        }
        return 130;
    }
    if args.convert_file.len() > 1 {
        return run_multi_tts_file_conversion(app, args);
    }
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

fn run_multi_tts_file_conversion(app: &AppHandle, args: &CliArgs) -> i32 {
    let result = prepare_multi_tts_inputs(args).and_then(|inputs| {
        initialize_file_conversion_managers(app, CliFileConversionKind::TextToAudio)
            .map_err(CliFailure::runtime)?;
        ensure_headless_conversion_not_cancelled()?;
        tauri::async_runtime::block_on(convert_multiple_text_files(app, args, inputs))
    });
    match result {
        Ok((metadata, failed)) => {
            if args.json {
                println!("{}", metadata);
            }
            if failed == 0 {
                0
            } else {
                1
            }
        }
        Err(failure) => {
            if args.json {
                println!(
                    "{}",
                    json!({
                        "ok": false,
                        "operation": "text_to_audio_batch",
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

fn prepare_multi_tts_inputs(args: &CliArgs) -> Result<Vec<PathBuf>, CliFailure> {
    let mut inputs = Vec::with_capacity(args.convert_file.len());
    for input in &args.convert_file {
        let input = absolute_path(input).map_err(CliFailure::usage)?;
        if !input.is_file() {
            return Err(CliFailure::usage(format!(
                "Input file does not exist: {}",
                input.display()
            )));
        }
        let input_extension = extension(&input).ok_or_else(|| {
            CliFailure::usage(format!(
                "Input file has no supported extension: {}",
                input.display()
            ))
        })?;
        if !TEXT_EXTENSIONS.contains(&input_extension.as_str()) {
            return Err(CliFailure::usage(format!(
                "Multiple --convert-file inputs support TXT/MD to audio only; unsupported input: {}",
                input.display()
            )));
        }
        inputs.push(input);
    }
    Ok(inputs)
}

async fn convert_multiple_text_files(
    app: &AppHandle,
    args: &CliArgs,
    inputs: Vec<PathBuf>,
) -> Result<(Value, usize), CliFailure> {
    let mut planning_settings = get_settings(app)
        .tts
        .effective_for_scope(crate::settings::TtsOperationScope::File);
    apply_tts_provider_override(args, &mut planning_settings)?;
    apply_tts_conversion_overrides(args, &mut planning_settings)?;
    let output_extension = output_format_name(planning_settings.output_format);
    let output_directory = args
        .output
        .as_deref()
        .map(absolute_path)
        .transpose()
        .map_err(CliFailure::usage)?;
    if let Some(directory) = output_directory.as_deref() {
        if directory.exists() && !directory.is_dir() {
            return Err(CliFailure::usage(format!(
                "--output must be a directory when multiple input files are supplied: {}",
                directory.display()
            )));
        }
        fs::create_dir_all(directory).map_err(|error| {
            CliFailure::runtime(format!(
                "Failed to create batch output directory {}: {error}",
                directory.display()
            ))
        })?;
    }

    let total = inputs.len();
    let mut reserved = HashSet::new();
    let mut results = Vec::with_capacity(total);
    let mut completed = 0usize;
    let mut failed = 0usize;
    for (index, input) in inputs.into_iter().enumerate() {
        ensure_headless_conversion_not_cancelled()?;
        let destination = output_directory
            .as_deref()
            .or_else(|| input.parent())
            .ok_or_else(|| {
                CliFailure::usage(format!(
                    "Input has no parent directory: {}",
                    input.display()
                ))
            })?;
        let output = unique_cli_batch_output(destination, &input, output_extension, &mut reserved)?;
        if !args.json {
            eprintln!("TTS batch: file {} of {}", index + 1, total);
        }
        let mut file_args = args.clone();
        file_args.convert_file = vec![input.clone()];
        file_args.output = Some(output);
        match convert_text_to_audio(app, &file_args, input.clone()).await {
            Ok(metadata) => {
                completed += 1;
                results.push(metadata);
            }
            Err(error) => {
                failed += 1;
                if !args.json {
                    eprintln!("error: {}: {}", input.display(), error.message);
                }
                results.push(json!({
                    "ok": false,
                    "input": input,
                    "error": error.message,
                    "exit_code": error.exit_code,
                }));
            }
        }
    }
    if !args.json {
        eprintln!("TTS batch finished: {completed} completed, {failed} failed, {total} total.");
    }
    Ok((
        json!({
            "ok": failed == 0,
            "operation": "text_to_audio_batch",
            "total": total,
            "completed": completed,
            "failed": failed,
            "results": results,
        }),
        failed,
    ))
}

fn unique_cli_batch_output(
    output_directory: &Path,
    input: &Path,
    output_extension: &str,
    reserved: &mut HashSet<String>,
) -> Result<PathBuf, CliFailure> {
    let stem = input
        .file_stem()
        .ok_or_else(|| CliFailure::usage(format!("Input has no file name: {}", input.display())))?;
    for index in 1..=10_000 {
        let mut file_name = stem.to_os_string();
        if index > 1 {
            file_name.push(format!("-{index}"));
        }
        file_name.push(format!(".{output_extension}"));
        let candidate = output_directory.join(file_name);
        let key = if cfg!(windows) {
            candidate.to_string_lossy().to_ascii_lowercase()
        } else {
            candidate.to_string_lossy().into_owned()
        };
        if !candidate.exists() && reserved.insert(key) {
            return Ok(candidate);
        }
    }
    Err(CliFailure::runtime(format!(
        "Could not allocate a collision-safe output name for {}",
        input.display()
    )))
}

fn run_file_conversion_inner(app: &AppHandle, args: &CliArgs) -> Result<Value, CliFailure> {
    let plan = build_plan(args)?;
    validate_direction_specific_args(args, plan.kind)?;
    initialize_file_conversion_managers(app, plan.kind).map_err(CliFailure::runtime)?;
    ensure_headless_conversion_not_cancelled()?;

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
        .first()
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
    if kind == CliFileConversionKind::AudioToText && args.has_tts_file_conversion_args() {
        return Err(CliFailure::usage(
            "TTS conversion flags apply only to TXT/MD to audio conversion",
        ));
    }
    Ok(())
}

async fn convert_text_to_audio(
    app: &AppHandle,
    args: &CliArgs,
    input: PathBuf,
) -> Result<Value, CliFailure> {
    let mut settings = get_settings(app)
        .tts
        .effective_for_scope(crate::settings::TtsOperationScope::File);
    apply_tts_provider_override(args, &mut settings)?;
    apply_tts_conversion_overrides(args, &mut settings)?;
    let llm_cleanup_instruction_source = apply_tts_llm_overrides(args, &mut settings)?;
    let (output, format) =
        resolve_tts_output(args.output.as_deref(), &input, settings.output_format)?;
    if args.tts_format.is_some() && format != settings.output_format {
        return Err(CliFailure::usage(format!(
            "--tts-format {} conflicts with output extension .{}",
            output_format_name(settings.output_format),
            output_format_name(format)
        )));
    }
    refuse_existing_output(&output)?;
    settings.output_format = format;
    if args.tts_bitrate.is_some() && format != TtsOutputFormat::Mp3 {
        return Err(CliFailure::usage(
            "--tts-bitrate applies only when the final output format is MP3",
        ));
    }
    let instruction_source = apply_tts_instruction_override(args, &mut settings)?;
    if settings.provider == TtsProvider::OpenAi {
        TtsManager::validate_openai_instructions(&settings.openai_instructions)
            .map_err(|error| CliFailure::usage(error.to_string()))?;
    }
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
            "TTS instruction prompts require the selected OpenAI provider; current provider is {}",
            settings.provider.as_str()
        )));
    }
    TtsManager::validate_settings(&settings)
        .map_err(|error| CliFailure::usage(error.to_string()))?;
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
    if settings.file_history_enabled && app.try_state::<Arc<TtsHistoryManager>>().is_none() {
        let history = Arc::new(TtsHistoryManager::new(app).map_err(|error| {
            CliFailure::runtime(format!("Failed to initialize TTS History: {error}"))
        })?);
        app.manage(history);
    }
    let history_source = if settings.file_history_enabled {
        Some(
            manager
                .read_original_text_file(&input)
                .map_err(|error| CliFailure::runtime(error.to_string()))?,
        )
    } else {
        None
    };
    if !args.json {
        eprintln!(
            "TTS: converting {} with {}…",
            input.display(),
            settings.provider.as_str()
        );
    }

    let started = Instant::now();
    let mut operation = Box::pin(manager.convert_text_file_resolved(&input, &output, &settings));
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
    drop(operation);
    settings = result.settings;
    let result = result.value;

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
                history
                    .save_explicit_capture_success(
                        metadata_from_settings(
                            &settings,
                            TtsHistoryScope::File,
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
                        settings.disk_reserve_mb,
                    )
                    .map(Some)
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
        "model": effective_tts_model(&settings),
        "voice": effective_tts_voice(&settings),
        "language": effective_tts_language(&settings),
        "key_source": effective_tts_key_source(&settings),
        "speed": effective_tts_speed(&settings),
        "provider_controls": effective_provider_controls(&settings),
        "output_format": result.output_format,
        "mp3_bitrate_kbps": result.mp3_bitrate_kbps,
        "file_chunk_target_chars": settings.file_target_chars,
        "retry_count": settings.retry_count,
        "retry_base_delay_ms": settings.retry_base_delay_ms,
        "inter_chunk_pause_ms": settings.inter_chunk_pause_ms,
        "paragraph_pause_ms": settings.paragraph_pause_ms,
        "preprocessing_enabled": settings.preprocessing_enabled,
        "preprocessing_rule_count": settings.preprocessing_rules.len(),
        "llm_preprocessing_enabled": settings.llm_preprocessing.file_enabled,
        "llm_provider": settings.llm_preprocessing.provider_id,
        "llm_model": settings.llm_preprocessing.model,
        "llm_key_source": settings.llm_preprocessing.key_source,
        "llm_prompt_id": settings.llm_preprocessing.file_selected_prompt_id,
        "llm_chunk_target_chars": settings.llm_preprocessing.chunk_target_chars,
        "llm_retry_count": settings.llm_preprocessing.retry_count,
        "llm_retry_base_delay_ms": settings.llm_preprocessing.retry_base_delay_ms,
        "llm_request_timeout_seconds": settings.llm_preprocessing.request_timeout_seconds,
        "llm_cleanup_instruction_source": llm_cleanup_instruction_source,
        "disk_reserve_mb": settings.disk_reserve_mb,
        "history_requested": settings.file_history_enabled,
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

fn apply_tts_provider_override(
    args: &CliArgs,
    settings: &mut TtsSettings,
) -> Result<(), CliFailure> {
    if let Some(provider) = args.tts_provider {
        settings.provider = match provider {
            CliTtsProvider::Soniox => TtsProvider::Soniox,
            CliTtsProvider::Deepgram => TtsProvider::Deepgram,
            CliTtsProvider::Openai => TtsProvider::OpenAi,
            CliTtsProvider::Murf => TtsProvider::Murf,
            CliTtsProvider::Elevenlabs => TtsProvider::ElevenLabs,
            CliTtsProvider::Cartesia => TtsProvider::Cartesia,
            CliTtsProvider::OpenAiCompatible => TtsProvider::OpenAiCompatible,
            CliTtsProvider::Edge => TtsProvider::Edge,
            CliTtsProvider::LocalQwen => TtsProvider::LocalQwen,
            CliTtsProvider::LocalKokoro => TtsProvider::LocalKokoro,
            CliTtsProvider::Windows => TtsProvider::Windows,
        };
        if provider == CliTtsProvider::Windows && args.tts_voice.is_none() {
            settings.windows_voice_id.clear();
            settings.windows_voice_language.clear();
        }
    }
    if let Some(model) = args.tts_model.as_deref() {
        let model = nonempty_cli_value("--tts-model", model)?;
        match settings.provider {
            TtsProvider::Soniox => settings.soniox_model = model.to_string(),
            TtsProvider::Deepgram => settings.deepgram_model = model.to_string(),
            TtsProvider::OpenAi => settings.openai_model = model.to_string(),
            TtsProvider::OpenAiCompatible => settings.openai_compatible_model = model.to_string(),
            TtsProvider::Murf => {
                if !matches!(model, "falcon-2" | "gen2") {
                    return Err(CliFailure::usage(
                        "--tts-model for murf must be falcon-2 or gen2",
                    ));
                }
                if settings.murf_model != model && args.tts_voice.is_none() {
                    settings.murf_voice = if model == "gen2" {
                        DEFAULT_TTS_MURF_GEN2_VOICE
                    } else {
                        DEFAULT_TTS_MURF_VOICE
                    }
                    .to_string();
                }
                settings.murf_model = model.to_string();
            }
            TtsProvider::ElevenLabs => {
                if !matches!(
                    model,
                    "eleven_flash_v2_5" | "eleven_v3" | "eleven_multilingual_v2"
                ) {
                    return Err(CliFailure::usage(
                        "--tts-model for elevenlabs must be eleven_flash_v2_5, eleven_v3, or eleven_multilingual_v2",
                    ));
                }
                settings.elevenlabs_model = model.to_string();
            }
            TtsProvider::Cartesia => {
                if model != "sonic-3.5" {
                    return Err(CliFailure::usage(
                        "--tts-model for cartesia must be sonic-3.5",
                    ));
                }
                settings.cartesia_model = model.to_string();
            }
            TtsProvider::Edge => {
                return Err(CliFailure::usage(
                    "--tts-model is not supported by edge because the experimental adapter uses one fixed service; use --tts-voice instead",
                ));
            }
            TtsProvider::LocalQwen => {
                return Err(CliFailure::usage(
                    "--tts-model is not supported by local-qwen because AivoRelay uses one pinned model; remove the flag",
                ));
            }
            TtsProvider::LocalKokoro => {
                return Err(CliFailure::usage(
                    "--tts-model is not supported by local-kokoro because AivoRelay uses one pinned model; remove the flag",
                ));
            }
            TtsProvider::Windows => {
                return Err(CliFailure::usage(
                    "--tts-model is not supported by windows voices; choose an installed voice with --tts-voice",
                ));
            }
        }
    }
    if let Some(voice) = args.tts_voice.as_deref() {
        let voice = nonempty_cli_value("--tts-voice", voice)?;
        match settings.provider {
            TtsProvider::Soniox => settings.soniox_voice = voice.to_string(),
            TtsProvider::Deepgram => {
                return Err(CliFailure::usage(
                    "Deepgram selects its voice through the model ID; use --tts-model instead of --tts-voice",
                ));
            }
            TtsProvider::OpenAi => settings.openai_voice = voice.to_string(),
            TtsProvider::OpenAiCompatible => settings.openai_compatible_voice = voice.to_string(),
            TtsProvider::Murf => settings.murf_voice = voice.to_string(),
            TtsProvider::ElevenLabs => settings.elevenlabs_voice = voice.to_string(),
            TtsProvider::Cartesia => settings.cartesia_voice = voice.to_string(),
            TtsProvider::Edge => {
                settings.edge_voice = voice.to_string();
                settings.edge_voice_language = crate::managers::edge_tts::voice_language(voice);
            }
            TtsProvider::LocalQwen => settings.local_qwen_voice = voice.to_string(),
            TtsProvider::LocalKokoro => settings.local_kokoro_voice = voice.to_string(),
            TtsProvider::Windows => {
                if voice.eq_ignore_ascii_case("default") {
                    settings.windows_voice_id.clear();
                    settings.windows_voice_language.clear();
                } else {
                    settings.windows_voice_id = voice.to_string();
                    settings.windows_voice_language.clear();
                }
            }
        }
    }
    if let Some(language) = args.tts_language.as_deref() {
        let language = nonempty_cli_value("--tts-language", language)?;
        match settings.provider {
            TtsProvider::Soniox => settings.soniox_language = language.to_string(),
            TtsProvider::LocalQwen => settings.local_qwen_language = language.to_string(),
            TtsProvider::LocalKokoro => settings.local_kokoro_language = language.to_string(),
            TtsProvider::Murf => settings.murf_language = language.to_string(),
            TtsProvider::ElevenLabs => {
                if settings.elevenlabs_model == "eleven_multilingual_v2" {
                    return Err(CliFailure::usage(
                        "--tts-language is not supported by ElevenLabs Multilingual v2; the model infers language from the text",
                    ));
                }
                settings.elevenlabs_language = iso_639_1_cli_value("--tts-language", language)?;
            }
            TtsProvider::Cartesia => {
                settings.cartesia_language = iso_639_1_cli_value("--tts-language", language)?;
            }
            TtsProvider::Deepgram => {
                return Err(CliFailure::usage(
                    "Deepgram TTS language is part of its model/voice ID; use --tts-model instead of --tts-language",
                ));
            }
            TtsProvider::OpenAi | TtsProvider::OpenAiCompatible => {
                return Err(CliFailure::usage(
                    "--tts-language is not supported by OpenAI/OpenAI-compatible TTS; provide language in the input text",
                ));
            }
            TtsProvider::Edge => {
                return Err(CliFailure::usage(
                    "Edge-TTS derives language from its voice ID; use --tts-voice instead of --tts-language",
                ));
            }
            TtsProvider::Windows => {
                return Err(CliFailure::usage(
                    "Windows derives language from the installed voice; use --tts-voice with a stable Windows voice ID",
                ));
            }
        }
    }
    if let Some(source) = args.tts_key_source {
        let source = match source {
            CliTtsKeySource::Shared => TtsKeySource::Shared,
            CliTtsKeySource::Separate => TtsKeySource::Separate,
        };
        match settings.provider {
            TtsProvider::Soniox => settings.soniox_key_source = source,
            TtsProvider::Deepgram => settings.deepgram_key_source = source,
            TtsProvider::OpenAi => settings.openai_key_source = source,
            TtsProvider::OpenAiCompatible => settings.openai_compatible_key_source = source,
            TtsProvider::Murf => {
                if source != TtsKeySource::Separate {
                    return Err(CliFailure::usage(
                        "--tts-key-source for murf must be separate",
                    ));
                }
                settings.murf_key_source = source;
            }
            TtsProvider::ElevenLabs => {
                if source != TtsKeySource::Separate {
                    return Err(CliFailure::usage(
                        "--tts-key-source for elevenlabs must be separate",
                    ));
                }
                settings.elevenlabs_key_source = source;
            }
            TtsProvider::Cartesia => {
                if source != TtsKeySource::Separate {
                    return Err(CliFailure::usage(
                        "--tts-key-source for cartesia must be separate",
                    ));
                }
                settings.cartesia_key_source = source;
            }
            TtsProvider::Edge
            | TtsProvider::LocalQwen
            | TtsProvider::LocalKokoro
            | TtsProvider::Windows => {
                return Err(CliFailure::usage(format!(
                    "--tts-key-source is not supported by {} because it does not use an API key",
                    settings.provider.as_str()
                )));
            }
        }
    }
    if let Some(base_url) = args.tts_base_url.as_deref() {
        let base_url = nonempty_cli_value("--tts-base-url", base_url)?;
        require_tts_provider(
            settings,
            TtsProvider::OpenAiCompatible,
            "--tts-base-url",
        )?;
        settings.openai_compatible_base_url = base_url.to_string();
    }
    match settings.provider {
        TtsProvider::Murf => settings.murf_key_source = TtsKeySource::Separate,
        TtsProvider::ElevenLabs => settings.elevenlabs_key_source = TtsKeySource::Separate,
        TtsProvider::Cartesia => settings.cartesia_key_source = TtsKeySource::Separate,
        _ => {}
    }
    Ok(())
}

fn apply_tts_conversion_overrides(
    args: &CliArgs,
    settings: &mut TtsSettings,
) -> Result<(), CliFailure> {
    if let Some(speed) = args.tts_speed {
        if settings.provider == TtsProvider::Murf {
            return Err(CliFailure::usage(
                "--tts-speed is not supported by murf; use --tts-murf-rate instead",
            ));
        }
        if settings.provider == TtsProvider::ElevenLabs
            && settings.elevenlabs_model == "eleven_v3"
        {
            return Err(CliFailure::usage(
                "--tts-speed is not supported by Eleven v3; use v3 audio tags and punctuation to guide pacing",
            ));
        }
        if !speed.is_finite() {
            return Err(CliFailure::usage("--tts-speed must be a finite number"));
        }
        let (minimum, maximum) = match settings.provider {
            TtsProvider::Soniox => (0.7, 1.3),
            TtsProvider::Deepgram => (0.7, 1.5),
            TtsProvider::OpenAi | TtsProvider::OpenAiCompatible => (0.25, 4.0),
            TtsProvider::Murf => unreachable!("handled above"),
            TtsProvider::ElevenLabs => (0.7, 1.2),
            TtsProvider::Cartesia => (0.6, 1.5),
            TtsProvider::Edge
            | TtsProvider::LocalQwen
            | TtsProvider::LocalKokoro
            | TtsProvider::Windows => (0.5, 2.0),
        };
        if !(minimum..=maximum).contains(&speed) {
            return Err(CliFailure::usage(format!(
                "--tts-speed for {} must be between {minimum} and {maximum}",
                settings.provider.as_str()
            )));
        }
        settings.speed = speed;
    }
    if let Some(rate) = args.tts_murf_rate {
        require_tts_provider(settings, TtsProvider::Murf, "--tts-murf-rate")?;
        if !(-50..=50).contains(&rate) {
            return Err(CliFailure::usage(
                "--tts-murf-rate must be between -50 and 50",
            ));
        }
        settings.murf_rate = rate;
    }
    if let Some(pitch) = args.tts_murf_pitch {
        require_tts_provider(settings, TtsProvider::Murf, "--tts-murf-pitch")?;
        if !(-50..=50).contains(&pitch) {
            return Err(CliFailure::usage(
                "--tts-murf-pitch must be between -50 and 50",
            ));
        }
        settings.murf_pitch = pitch;
    }
    if let Some(variation) = args.tts_murf_variation {
        require_tts_provider(settings, TtsProvider::Murf, "--tts-murf-variation")?;
        if settings.murf_model != "gen2" {
            return Err(CliFailure::usage(
                "--tts-murf-variation is supported only with --tts-model gen2",
            ));
        }
        if variation > 5 {
            return Err(CliFailure::usage(
                "--tts-murf-variation must be between 0 and 5",
            ));
        }
        settings.murf_variation = variation;
    }
    if let Some(style) = args.tts_murf_style.as_deref() {
        require_tts_provider(settings, TtsProvider::Murf, "--tts-murf-style")?;
        settings.murf_style = optional_cli_control(style);
    }
    if let Some(value) = args.tts_elevenlabs_stability {
        require_tts_provider(
            settings,
            TtsProvider::ElevenLabs,
            "--tts-elevenlabs-stability",
        )?;
        settings.elevenlabs_stability = validate_cli_float_range(
            "--tts-elevenlabs-stability",
            value,
            0.0,
            1.0,
        )?;
    }
    if let Some(value) = args.tts_elevenlabs_similarity_boost {
        require_tts_provider(
            settings,
            TtsProvider::ElevenLabs,
            "--tts-elevenlabs-similarity-boost",
        )?;
        if settings.elevenlabs_model == "eleven_v3" {
            return Err(CliFailure::usage(
                "--tts-elevenlabs-similarity-boost is unavailable for Eleven v3",
            ));
        }
        settings.elevenlabs_similarity_boost = validate_cli_float_range(
            "--tts-elevenlabs-similarity-boost",
            value,
            0.0,
            1.0,
        )?;
    }
    if let Some(value) = args.tts_elevenlabs_style {
        require_tts_provider(
            settings,
            TtsProvider::ElevenLabs,
            "--tts-elevenlabs-style",
        )?;
        settings.elevenlabs_style =
            validate_cli_float_range("--tts-elevenlabs-style", value, 0.0, 1.0)?;
    }
    if let Some(value) = args.tts_elevenlabs_speaker_boost {
        require_tts_provider(
            settings,
            TtsProvider::ElevenLabs,
            "--tts-elevenlabs-speaker-boost",
        )?;
        if settings.elevenlabs_model == "eleven_v3" {
            return Err(CliFailure::usage(
                "--tts-elevenlabs-speaker-boost is unavailable for Eleven v3",
            ));
        }
        settings.elevenlabs_use_speaker_boost = value;
    }
    if let Some(value) = args.tts_elevenlabs_text_normalization {
        require_tts_provider(
            settings,
            TtsProvider::ElevenLabs,
            "--tts-elevenlabs-text-normalization",
        )?;
        settings.elevenlabs_apply_text_normalization = match value {
            CliElevenLabsTextNormalization::Auto => ElevenLabsTextNormalization::Auto,
            CliElevenLabsTextNormalization::On => ElevenLabsTextNormalization::On,
            CliElevenLabsTextNormalization::Off => ElevenLabsTextNormalization::Off,
        };
    }
    if let Some(emotion) = args.tts_cartesia_emotion.as_deref() {
        require_tts_provider(
            settings,
            TtsProvider::Cartesia,
            "--tts-cartesia-emotion",
        )?;
        settings.cartesia_emotion = optional_cli_control(emotion);
    }
    if let Some(volume) = args.tts_cartesia_volume {
        require_tts_provider(
            settings,
            TtsProvider::Cartesia,
            "--tts-cartesia-volume",
        )?;
        settings.cartesia_volume =
            validate_cli_float_range("--tts-cartesia-volume", volume, 0.5, 2.0)?;
    }
    if let Some(format) = args.tts_format {
        settings.output_format = match format {
            CliTtsOutputFormat::Mp3 => TtsOutputFormat::Mp3,
            CliTtsOutputFormat::Wav => TtsOutputFormat::Wav,
        };
    }
    if let Some(bitrate) = args.tts_bitrate {
        if !SUPPORTED_MP3_BITRATES.contains(&bitrate) {
            return Err(CliFailure::usage(format!(
                "--tts-bitrate must be one of: {}",
                SUPPORTED_MP3_BITRATES
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        settings.mp3_bitrate_kbps = bitrate;
    }
    if let Some(chars) = args.tts_chunk_chars {
        let hard_limit = TtsManager::settings_character_limit(settings) as u32;
        if !(50..=hard_limit).contains(&chars) {
            let qualification = if settings.provider == TtsProvider::Cartesia {
                " (AivoRelay's conservative per-request cap, not a published Cartesia limit)"
            } else {
                ""
            };
            return Err(CliFailure::usage(format!(
                "--tts-chunk-chars for {} must be between 50 and {hard_limit}{qualification}",
                settings.provider.as_str()
            )));
        }
        settings.file_target_chars = chars;
    }
    if let Some(retries) = args.tts_retries {
        if retries > 10 {
            return Err(CliFailure::usage("--tts-retries must be between 0 and 10"));
        }
        settings.retry_count = retries;
    }
    if let Some(delay) = args.tts_retry_delay_ms {
        if !(100..=30_000).contains(&delay) {
            return Err(CliFailure::usage(
                "--tts-retry-delay-ms must be between 100 and 30000",
            ));
        }
        settings.retry_base_delay_ms = delay;
    }
    if let Some(pause) = args.tts_chunk_pause_ms {
        if pause > 5_000 {
            return Err(CliFailure::usage(
                "--tts-chunk-pause-ms must be between 0 and 5000",
            ));
        }
        settings.inter_chunk_pause_ms = pause;
    }
    if let Some(pause) = args.tts_paragraph_pause_ms {
        if pause > 10_000 {
            return Err(CliFailure::usage(
                "--tts-paragraph-pause-ms must be between 0 and 10000",
            ));
        }
        settings.paragraph_pause_ms = pause;
    }
    if let Some(enabled) = args.tts_preprocessing {
        settings.preprocessing_enabled = enabled;
    }
    if let Some(path) = args.tts_replacements_file.as_deref() {
        if args.tts_preprocessing == Some(false) {
            return Err(CliFailure::usage(
                "--tts-replacements-file conflicts with --tts-preprocessing false",
            ));
        }
        settings.preprocessing_rules = read_tts_replacement_rules(path)?;
        settings.preprocessing_enabled = true;
    }
    if let Some(reserve) = args.tts_disk_reserve_mb {
        if reserve > 1_048_576 {
            return Err(CliFailure::usage(
                "--tts-disk-reserve-mb must be between 0 and 1048576",
            ));
        }
        settings.disk_reserve_mb = reserve;
    }
    if let Some(history) = args.tts_history {
        settings.file_history_enabled = history;
    }
    Ok(())
}

fn nonempty_cli_value<'a>(flag: &str, value: &'a str) -> Result<&'a str, CliFailure> {
    let value = value.trim();
    if value.is_empty() {
        Err(CliFailure::usage(format!("{flag} must not be empty")))
    } else {
        Ok(value)
    }
}

fn iso_639_1_cli_value(flag: &str, value: &str) -> Result<String, CliFailure> {
    if value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        Ok(value.to_ascii_lowercase())
    } else {
        Err(CliFailure::usage(format!(
            "{flag} must be a two-letter ISO 639-1 code for this provider"
        )))
    }
}

fn require_tts_provider(
    settings: &TtsSettings,
    expected: TtsProvider,
    flag: &str,
) -> Result<(), CliFailure> {
    if settings.provider == expected {
        Ok(())
    } else {
        Err(CliFailure::usage(format!(
            "{flag} requires --tts-provider {} (effective provider is {})",
            expected.as_str(),
            settings.provider.as_str()
        )))
    }
}

fn validate_cli_float_range(
    flag: &str,
    value: f32,
    minimum: f32,
    maximum: f32,
) -> Result<f32, CliFailure> {
    if !value.is_finite() || value < minimum || value > maximum {
        Err(CliFailure::usage(format!(
            "{flag} must be a finite number between {minimum} and {maximum}"
        )))
    } else {
        Ok(value)
    }
}

fn optional_cli_control(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && !value.eq_ignore_ascii_case("none")).then(|| value.to_string())
}

fn apply_tts_llm_overrides(
    args: &CliArgs,
    settings: &mut TtsSettings,
) -> Result<String, CliFailure> {
    let has_configuration_override = args.tts_llm_prompt.is_some()
        || args.tts_llm_instructions.is_some()
        || args.tts_llm_instructions_file.is_some()
        || args.tts_llm_provider.is_some()
        || args.tts_llm_model.is_some()
        || args.tts_llm_key_source.is_some()
        || args.tts_llm_base_url.is_some()
        || args.tts_llm_allow_insecure_http.is_some()
        || args.tts_llm_reasoning.is_some()
        || args.tts_llm_reasoning_budget.is_some()
        || args.tts_llm_chunk_chars.is_some()
        || args.tts_llm_retries.is_some()
        || args.tts_llm_retry_delay_ms.is_some()
        || args.tts_llm_timeout_seconds.is_some();
    if args.tts_llm_preprocessing == Some(false) && has_configuration_override {
        return Err(CliFailure::usage(
            "--tts-llm-preprocessing false conflicts with other --tts-llm-* overrides",
        ));
    }
    if let Some(enabled) = args.tts_llm_preprocessing {
        settings.llm_preprocessing.file_enabled = enabled;
    } else if has_configuration_override {
        settings.llm_preprocessing.file_enabled = true;
    }

    if let Some(provider) = args.tts_llm_provider.as_deref() {
        settings.llm_preprocessing.provider_id =
            nonempty_cli_value("--tts-llm-provider", provider)?.to_string();
    }
    if let Some(model) = args.tts_llm_model.as_deref() {
        settings.llm_preprocessing.model =
            nonempty_cli_value("--tts-llm-model", model)?.to_string();
    }
    if let Some(source) = args.tts_llm_key_source {
        settings.llm_preprocessing.key_source = match source {
            CliTtsKeySource::Shared => TtsKeySource::Shared,
            CliTtsKeySource::Separate => TtsKeySource::Separate,
        };
    }
    if let Some(base_url) = args.tts_llm_base_url.as_deref() {
        if settings.llm_preprocessing.provider_id != "custom" {
            return Err(CliFailure::usage(
                "--tts-llm-base-url is supported only with --tts-llm-provider custom",
            ));
        }
        settings.llm_preprocessing.custom_base_url =
            nonempty_cli_value("--tts-llm-base-url", base_url)?
                .trim_end_matches('/')
                .to_string();
    }
    if let Some(allow) = args.tts_llm_allow_insecure_http {
        if settings.llm_preprocessing.provider_id != "custom" {
            return Err(CliFailure::usage(
                "--tts-llm-allow-insecure-http is supported only by the custom TTS cleanup provider",
            ));
        }
        settings.llm_preprocessing.custom_allow_insecure_http = allow;
    }
    if let Some(reasoning) = args.tts_llm_reasoning {
        settings.llm_preprocessing.reasoning_enabled = reasoning;
    }
    if let Some(budget) = args.tts_llm_reasoning_budget {
        if !(1_024..=1_000_000).contains(&budget) {
            return Err(CliFailure::usage(
                "--tts-llm-reasoning-budget must be between 1024 and 1000000",
            ));
        }
        if args.tts_llm_reasoning == Some(false)
            || (args.tts_llm_reasoning.is_none() && !settings.llm_preprocessing.reasoning_enabled)
        {
            return Err(CliFailure::usage(
                "--tts-llm-reasoning-budget requires reasoning to be enabled; add --tts-llm-reasoning true",
            ));
        }
        settings.llm_preprocessing.reasoning_budget = budget;
    }
    if let Some(chars) = args.tts_llm_chunk_chars {
        if !(1_000..=50_000).contains(&chars) {
            return Err(CliFailure::usage(
                "--tts-llm-chunk-chars must be between 1000 and 50000",
            ));
        }
        settings.llm_preprocessing.chunk_target_chars = chars;
    }
    if let Some(retries) = args.tts_llm_retries {
        if retries > 10 {
            return Err(CliFailure::usage(
                "--tts-llm-retries must be between 0 and 10",
            ));
        }
        settings.llm_preprocessing.retry_count = retries;
    }
    if let Some(delay) = args.tts_llm_retry_delay_ms {
        if !(100..=30_000).contains(&delay) {
            return Err(CliFailure::usage(
                "--tts-llm-retry-delay-ms must be between 100 and 30000",
            ));
        }
        settings.llm_preprocessing.retry_base_delay_ms = delay;
    }
    if let Some(timeout) = args.tts_llm_timeout_seconds {
        if !(10..=600).contains(&timeout) {
            return Err(CliFailure::usage(
                "--tts-llm-timeout-seconds must be between 10 and 600",
            ));
        }
        settings.llm_preprocessing.request_timeout_seconds = timeout;
    }

    let instructions = if let Some(path) = args.tts_llm_instructions_file.as_deref() {
        Some((
            "instructions_file",
            read_utf8_bom_bounded(
                path,
                maximum_utf8_file_bytes(TTS_LLM_INSTRUCTIONS_MAX_CHARS),
                "TTS LLM cleanup instructions must not exceed 32768 characters",
            )?,
        ))
    } else if let Some(instructions) = args.tts_llm_instructions.as_ref() {
        Some(("inline", instructions.clone()))
    } else {
        None
    };
    if let Some((source, instructions)) = instructions {
        if instructions.trim().is_empty() {
            return Err(CliFailure::usage(
                "TTS LLM cleanup instructions must not be empty",
            ));
        }
        if instructions.chars().count() > TTS_LLM_INSTRUCTIONS_MAX_CHARS {
            return Err(CliFailure::usage(
                "TTS LLM cleanup instructions must not exceed 32768 characters",
            ));
        }
        let id = "cli_tts_llm_instructions".to_string();
        settings.llm_preprocessing.file_prompts.push(LLMPrompt {
            id: id.clone(),
            name: "CLI instructions".to_string(),
            prompt: instructions,
        });
        settings.llm_preprocessing.file_selected_prompt_id = id;
        return Ok(source.to_string());
    }
    if let Some(name) = args.tts_llm_prompt.as_deref() {
        let name = nonempty_cli_value("--tts-llm-prompt", name)?;
        let matches = settings
            .llm_preprocessing
            .file_prompts
            .iter()
            .filter(|prompt| prompt.name.eq_ignore_ascii_case(name))
            .collect::<Vec<_>>();
        let prompt = match matches.as_slice() {
            [] => {
                let available = settings
                    .llm_preprocessing
                    .file_prompts
                    .iter()
                    .map(|prompt| prompt.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(CliFailure::usage(format!(
                    "Unknown TTS File Operations cleanup prompt '{name}'. Available presets: {available}"
                )));
            }
            [prompt] => *prompt,
            _ => {
                return Err(CliFailure::usage(format!(
                    "More than one TTS File Operations cleanup prompt is named '{name}'; rename one in TTS File Operations settings"
                )))
            }
        };
        settings.llm_preprocessing.file_selected_prompt_id = prompt.id.clone();
        return Ok(format!("preset:{}", prompt.name));
    }
    if settings.llm_preprocessing.file_enabled {
        let selected = settings
            .llm_preprocessing
            .file_prompts
            .iter()
            .find(|prompt| prompt.id == settings.llm_preprocessing.file_selected_prompt_id)
            .map(|prompt| prompt.name.as_str())
            .unwrap_or("missing");
        Ok(format!("saved_preset:{selected}"))
    } else {
        Ok("disabled".to_string())
    }
}

fn output_format_name(format: TtsOutputFormat) -> &'static str {
    match format {
        TtsOutputFormat::Mp3 => "mp3",
        TtsOutputFormat::Wav => "wav",
    }
}

fn read_tts_replacement_rules(path: &Path) -> Result<Vec<TextReplacement>, CliFailure> {
    let absolute = absolute_path(path).map_err(CliFailure::usage)?;
    let size = fs::metadata(&absolute)
        .map_err(|error| {
            CliFailure::usage(format!(
                "Cannot inspect --tts-replacements-file {}: {error}",
                absolute.display()
            ))
        })?
        .len();
    if size > 1_048_576 {
        return Err(CliFailure::usage(
            "--tts-replacements-file must not exceed 1 MiB",
        ));
    }
    let json = read_utf8_bom_bounded(
        &absolute,
        1_048_576,
        "--tts-replacements-file must not exceed 1 MiB",
    )?;
    let rules: Vec<TextReplacement> = serde_json::from_str(&json).map_err(|error| {
        CliFailure::usage(format!(
            "Invalid --tts-replacements-file JSON in {}: {error}",
            absolute.display()
        ))
    })?;
    if rules.len() > 1_000 {
        return Err(CliFailure::usage(
            "--tts-replacements-file must contain at most 1000 rules",
        ));
    }
    for (index, rule) in rules.iter().enumerate() {
        if rule.enabled && rule.from.is_empty() {
            return Err(CliFailure::usage(format!(
                "Enabled replacement rule {} has an empty 'from' value",
                index + 1
            )));
        }
        if rule.enabled && rule.is_regex {
            let pattern = if rule.case_sensitive {
                rule.from.clone()
            } else {
                format!("(?i){}", rule.from)
            };
            regex::Regex::new(&pattern).map_err(|error| {
                CliFailure::usage(format!(
                    "Replacement rule {} has an invalid regular expression: {error}",
                    index + 1
                ))
            })?;
        }
    }
    Ok(rules)
}

fn effective_tts_model(settings: &TtsSettings) -> &str {
    match settings.provider {
        TtsProvider::Soniox => &settings.soniox_model,
        TtsProvider::Deepgram => &settings.deepgram_model,
        TtsProvider::OpenAi => &settings.openai_model,
        TtsProvider::OpenAiCompatible => &settings.openai_compatible_model,
        TtsProvider::Murf => &settings.murf_model,
        TtsProvider::ElevenLabs => &settings.elevenlabs_model,
        TtsProvider::Cartesia => &settings.cartesia_model,
        TtsProvider::Edge => crate::managers::edge_tts::EDGE_TTS_MODEL,
        TtsProvider::LocalQwen => "Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice",
        TtsProvider::LocalKokoro => crate::managers::local_kokoro::KOKORO_MODEL_REPOSITORY,
        TtsProvider::Windows => "windows.media.speechsynthesis",
    }
}

fn effective_tts_voice(settings: &TtsSettings) -> &str {
    match settings.provider {
        TtsProvider::Soniox => &settings.soniox_voice,
        TtsProvider::Deepgram => &settings.deepgram_model,
        TtsProvider::OpenAi => &settings.openai_voice,
        TtsProvider::OpenAiCompatible => &settings.openai_compatible_voice,
        TtsProvider::Murf => &settings.murf_voice,
        TtsProvider::ElevenLabs => &settings.elevenlabs_voice,
        TtsProvider::Cartesia => &settings.cartesia_voice,
        TtsProvider::Edge => &settings.edge_voice,
        TtsProvider::LocalQwen => &settings.local_qwen_voice,
        TtsProvider::LocalKokoro => &settings.local_kokoro_voice,
        TtsProvider::Windows => &settings.windows_voice_id,
    }
}

fn effective_tts_language(settings: &TtsSettings) -> &str {
    match settings.provider {
        TtsProvider::Soniox => &settings.soniox_language,
        TtsProvider::LocalQwen => &settings.local_qwen_language,
        TtsProvider::LocalKokoro => &settings.local_kokoro_language,
        TtsProvider::Windows => &settings.windows_voice_language,
        TtsProvider::Edge => &settings.edge_voice_language,
        TtsProvider::Murf => &settings.murf_language,
        TtsProvider::ElevenLabs => &settings.elevenlabs_language,
        TtsProvider::Cartesia => &settings.cartesia_language,
        TtsProvider::Deepgram | TtsProvider::OpenAi | TtsProvider::OpenAiCompatible => "",
    }
}

fn effective_tts_key_source(settings: &TtsSettings) -> Option<TtsKeySource> {
    match settings.provider {
        TtsProvider::Soniox => Some(settings.soniox_key_source),
        TtsProvider::Deepgram => Some(settings.deepgram_key_source),
        TtsProvider::OpenAi => Some(settings.openai_key_source),
        TtsProvider::OpenAiCompatible => Some(settings.openai_compatible_key_source),
        TtsProvider::Murf | TtsProvider::ElevenLabs | TtsProvider::Cartesia => {
            Some(TtsKeySource::Separate)
        }
        TtsProvider::Edge
        | TtsProvider::LocalQwen
        | TtsProvider::LocalKokoro
        | TtsProvider::Windows => None,
    }
}

fn effective_provider_controls(settings: &TtsSettings) -> Value {
    match settings.provider {
        TtsProvider::Murf if settings.murf_model == "gen2" => json!({
            "rate": settings.murf_rate,
            "pitch": settings.murf_pitch,
            "variation": settings.murf_variation,
            "style": settings.murf_style,
        }),
        TtsProvider::Murf => json!({
            "rate": settings.murf_rate,
            "pitch": settings.murf_pitch,
            "style": settings.murf_style,
        }),
        TtsProvider::ElevenLabs if settings.elevenlabs_model == "eleven_v3" => json!({
            "stability": settings.elevenlabs_stability,
            "style": settings.elevenlabs_style,
            "apply_text_normalization": settings.elevenlabs_apply_text_normalization,
        }),
        TtsProvider::ElevenLabs => json!({
            "speed": settings.speed,
            "stability": settings.elevenlabs_stability,
            "similarity_boost": settings.elevenlabs_similarity_boost,
            "style": settings.elevenlabs_style,
            "use_speaker_boost": settings.elevenlabs_use_speaker_boost,
            "apply_text_normalization": settings.elevenlabs_apply_text_normalization,
        }),
        TtsProvider::Cartesia => json!({
            "speed": settings.speed,
            "volume": settings.cartesia_volume,
            "emotion": settings.cartesia_emotion,
        }),
        _ => Value::Null,
    }
}

fn effective_tts_speed(settings: &TtsSettings) -> f32 {
    match settings.provider {
        TtsProvider::Murf => 1.0,
        TtsProvider::ElevenLabs if settings.elevenlabs_model == "eleven_v3" => 1.0,
        _ => settings.speed,
    }
}

async fn convert_audio_to_text(
    app: &AppHandle,
    args: &CliArgs,
    input: PathBuf,
) -> Result<Value, CliFailure> {
    let output = resolve_transcription_output(args.output.as_deref(), &input)?;
    refuse_existing_output(&output)?;
    let settings = get_settings(app);
    let provider = transcription_provider_name(
        settings.transcription_provider,
        &settings.remote_stt.provider_preset,
    );

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
        settings.openai_instructions = read_utf8_bom_bounded(
            path,
            maximum_utf8_file_bytes(OPENAI_TTS_INSTRUCTIONS_MAX_CHARS),
            "OpenAI voice instructions must not exceed 4096 characters",
        )?;
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

fn maximum_utf8_file_bytes(maximum_characters: usize) -> usize {
    maximum_characters
        .saturating_mul(UTF8_MAX_BYTES_PER_CHAR)
        .saturating_add(UTF8_BOM_BYTES)
}

fn read_utf8_bom_bounded(
    path: &Path,
    maximum_bytes: usize,
    oversized_message: &str,
) -> Result<String, CliFailure> {
    let path = absolute_path(path).map_err(CliFailure::usage)?;
    let mut bytes = Vec::new();
    File::open(&path)
        .and_then(|file| {
            file.take(maximum_bytes.saturating_add(1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|error| {
            CliFailure::usage(format!(
                "Failed to read TTS instructions file {}: {error}",
                path.display()
            ))
        })?;
    if bytes.len() > maximum_bytes {
        return Err(CliFailure::usage(oversized_message));
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.drain(..UTF8_BOM_BYTES);
    }
    String::from_utf8(bytes).map_err(|_| {
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
        TtsPhase::Preprocessing => eprintln!(
            "TTS: AI text cleanup {}/{} (attempt {})…",
            state.completed_chunks, state.total_chunks, state.current_attempt
        ),
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

fn transcription_provider_name(
    provider: TranscriptionProvider,
    remote_provider_preset: &str,
) -> &'static str {
    match provider {
        TranscriptionProvider::Local => "local",
        TranscriptionProvider::RemoteOpenAiCompatible => match remote_provider_preset {
            "vercel" => "Vercel AI Gateway (Gemini 3.5 Transcribe)",
            "google" => "Google Gemini API (Gemini 3.5 Transcribe)",
            _ => "OpenAI-compatible",
        },
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
            convert_file: vec![PathBuf::from("chapter.md")],
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
    fn provider_and_voice_overrides_are_temporary_and_provider_aware() {
        let mut windows = TtsSettings::default();
        let args = CliArgs {
            convert_file: vec![PathBuf::from("chapter.md")],
            tts_provider: Some(CliTtsProvider::Windows),
            tts_voice: Some(" Windows voice ID ".to_string()),
            ..CliArgs::default()
        };
        apply_tts_provider_override(&args, &mut windows).unwrap();
        assert_eq!(windows.provider, TtsProvider::Windows);
        assert_eq!(windows.windows_voice_id, "Windows voice ID");

        let mut qwen = TtsSettings::default();
        let args = CliArgs {
            convert_file: vec![PathBuf::from("chapter.md")],
            tts_provider: Some(CliTtsProvider::LocalQwen),
            tts_voice: Some("Vivian".to_string()),
            ..CliArgs::default()
        };
        apply_tts_provider_override(&args, &mut qwen).unwrap();
        assert_eq!(qwen.provider, TtsProvider::LocalQwen);
        assert_eq!(qwen.local_qwen_voice, "Vivian");
    }

    #[test]
    fn new_provider_cli_overrides_are_provider_specific() {
        let mut murf = TtsSettings::default();
        let murf_args = CliArgs {
            convert_file: vec![PathBuf::from("chapter.md")],
            tts_provider: Some(CliTtsProvider::Murf),
            tts_model: Some("gen2".to_string()),
            tts_voice: Some("en-US-natalie".to_string()),
            tts_language: Some("en-US".to_string()),
            tts_murf_rate: Some(4),
            tts_murf_pitch: Some(-2),
            tts_murf_variation: Some(3),
            tts_murf_style: Some("Conversational".to_string()),
            ..CliArgs::default()
        };
        apply_tts_provider_override(&murf_args, &mut murf).unwrap();
        apply_tts_conversion_overrides(&murf_args, &mut murf).unwrap();
        assert_eq!(murf.provider, TtsProvider::Murf);
        assert_eq!(murf.murf_model, "gen2");
        assert_eq!(murf.murf_variation, 3);
        assert_eq!(murf.murf_style.as_deref(), Some("Conversational"));

        let mut elevenlabs = TtsSettings::default();
        let elevenlabs_args = CliArgs {
            convert_file: vec![PathBuf::from("chapter.md")],
            tts_provider: Some(CliTtsProvider::Elevenlabs),
            tts_speed: Some(1.1),
            tts_elevenlabs_stability: Some(0.6),
            tts_elevenlabs_text_normalization:
                Some(CliElevenLabsTextNormalization::On),
            ..CliArgs::default()
        };
        apply_tts_provider_override(&elevenlabs_args, &mut elevenlabs).unwrap();
        apply_tts_conversion_overrides(&elevenlabs_args, &mut elevenlabs).unwrap();
        assert_eq!(elevenlabs.provider, TtsProvider::ElevenLabs);
        assert_eq!(elevenlabs.speed, 1.1);
        assert_eq!(elevenlabs.elevenlabs_stability, 0.6);
        assert_eq!(
            elevenlabs.elevenlabs_apply_text_normalization,
            ElevenLabsTextNormalization::On
        );

        let mut cartesia = TtsSettings::default();
        let cartesia_args = CliArgs {
            convert_file: vec![PathBuf::from("chapter.md")],
            tts_provider: Some(CliTtsProvider::Cartesia),
            tts_speed: Some(1.2),
            tts_cartesia_volume: Some(1.25),
            tts_cartesia_emotion: Some("calm".to_string()),
            ..CliArgs::default()
        };
        apply_tts_provider_override(&cartesia_args, &mut cartesia).unwrap();
        apply_tts_conversion_overrides(&cartesia_args, &mut cartesia).unwrap();
        assert_eq!(cartesia.provider, TtsProvider::Cartesia);
        assert_eq!(cartesia.speed, 1.2);
        assert_eq!(cartesia.cartesia_volume, 1.25);
        assert_eq!(cartesia.cartesia_emotion.as_deref(), Some("calm"));

        let mut wrong_provider = TtsSettings::default();
        assert!(apply_tts_conversion_overrides(&elevenlabs_args, &mut wrong_provider).is_err());
    }

    #[test]
    fn murf_model_override_selects_a_compatible_default_voice() {
        let mut settings = TtsSettings::default();
        let gen2_args = CliArgs {
            convert_file: vec![PathBuf::from("chapter.md")],
            tts_provider: Some(CliTtsProvider::Murf),
            tts_model: Some("gen2".to_string()),
            ..CliArgs::default()
        };

        apply_tts_provider_override(&gen2_args, &mut settings).unwrap();
        assert_eq!(settings.murf_model, "gen2");
        assert_eq!(settings.murf_voice, DEFAULT_TTS_MURF_GEN2_VOICE);

        let falcon_args = CliArgs {
            convert_file: vec![PathBuf::from("chapter.md")],
            tts_provider: Some(CliTtsProvider::Murf),
            tts_model: Some("falcon-2".to_string()),
            ..CliArgs::default()
        };

        apply_tts_provider_override(&falcon_args, &mut settings).unwrap();
        assert_eq!(settings.murf_model, "falcon-2");
        assert_eq!(settings.murf_voice, DEFAULT_TTS_MURF_VOICE);
    }

    #[test]
    fn new_provider_cli_rejects_model_incompatible_controls() {
        let v3_cases = [
            (
                CliArgs {
                    tts_provider: Some(CliTtsProvider::Elevenlabs),
                    tts_model: Some("eleven_v3".to_string()),
                    tts_speed: Some(1.1),
                    ..CliArgs::default()
                },
                "--tts-speed",
            ),
            (
                CliArgs {
                    tts_provider: Some(CliTtsProvider::Elevenlabs),
                    tts_model: Some("eleven_v3".to_string()),
                    tts_elevenlabs_similarity_boost: Some(0.8),
                    ..CliArgs::default()
                },
                "--tts-elevenlabs-similarity-boost",
            ),
            (
                CliArgs {
                    tts_provider: Some(CliTtsProvider::Elevenlabs),
                    tts_model: Some("eleven_v3".to_string()),
                    tts_elevenlabs_speaker_boost: Some(true),
                    ..CliArgs::default()
                },
                "--tts-elevenlabs-speaker-boost",
            ),
        ];

        for (args, expected) in v3_cases {
            let mut settings = TtsSettings::default();
            apply_tts_provider_override(&args, &mut settings).unwrap();
            let error = apply_tts_conversion_overrides(&args, &mut settings)
                .expect_err("Eleven v3-only restriction must fail");
            assert!(error.message.contains(expected), "{}", error.message);
        }

        let multilingual_language = CliArgs {
            tts_provider: Some(CliTtsProvider::Elevenlabs),
            tts_model: Some("eleven_multilingual_v2".to_string()),
            tts_language: Some("en".to_string()),
            ..CliArgs::default()
        };
        let mut settings = TtsSettings::default();
        let error = apply_tts_provider_override(&multilingual_language, &mut settings)
            .expect_err("Multilingual v2 language override must fail");
        assert!(error.message.contains("infers language"), "{}", error.message);

        let flash = CliArgs {
            tts_provider: Some(CliTtsProvider::Elevenlabs),
            tts_model: Some("eleven_flash_v2_5".to_string()),
            tts_language: Some("en".to_string()),
            tts_speed: Some(1.1),
            ..CliArgs::default()
        };
        let mut settings = TtsSettings::default();
        apply_tts_provider_override(&flash, &mut settings).unwrap();
        apply_tts_conversion_overrides(&flash, &mut settings).unwrap();
        assert_eq!(settings.elevenlabs_model, "eleven_flash_v2_5");
        assert_eq!(settings.elevenlabs_language, "en");
        assert_eq!(settings.speed, 1.1);
    }

    #[test]
    fn cli_json_reports_only_effective_new_provider_controls() {
        let mut murf = TtsSettings {
            provider: TtsProvider::Murf,
            speed: 1.5,
            murf_variation: 5,
            ..TtsSettings::default()
        };
        assert_eq!(effective_tts_speed(&murf), 1.0);
        assert!(effective_provider_controls(&murf).get("variation").is_none());

        murf.murf_model = "gen2".to_string();
        assert_eq!(effective_provider_controls(&murf)["variation"], 5);

        let eleven_v3 = TtsSettings {
            provider: TtsProvider::ElevenLabs,
            elevenlabs_model: "eleven_v3".to_string(),
            speed: 1.2,
            ..TtsSettings::default()
        };
        let controls = effective_provider_controls(&eleven_v3);
        assert_eq!(effective_tts_speed(&eleven_v3), 1.0);
        assert!(controls.get("similarity_boost").is_none());
        assert!(controls.get("use_speaker_boost").is_none());

        let cartesia = TtsSettings {
            provider: TtsProvider::Cartesia,
            speed: 1.4,
            cartesia_volume: 1.5,
            ..TtsSettings::default()
        };
        let controls = effective_provider_controls(&cartesia);
        assert_eq!(effective_tts_speed(&cartesia), 1.4);
        assert_eq!(controls["speed"], 1.4);
        assert_eq!(controls["volume"], 1.5);
    }

    #[test]
    fn all_supported_file_overrides_apply_without_mutating_the_source_snapshot() {
        let original = TtsSettings::default();
        let mut effective = original.clone();
        let args = CliArgs {
            convert_file: vec![PathBuf::from("chapter.md")],
            tts_provider: Some(CliTtsProvider::Soniox),
            tts_model: Some("sonic-preview".to_string()),
            tts_voice: Some("voice-id".to_string()),
            tts_language: Some("ru".to_string()),
            tts_speed: Some(1.2),
            tts_key_source: Some(CliTtsKeySource::Separate),
            tts_format: Some(CliTtsOutputFormat::Mp3),
            tts_bitrate: Some(192),
            tts_chunk_chars: Some(1_400),
            tts_retries: Some(4),
            tts_retry_delay_ms: Some(750),
            tts_chunk_pause_ms: Some(80),
            tts_paragraph_pause_ms: Some(300),
            tts_preprocessing: Some(false),
            tts_disk_reserve_mb: Some(1_024),
            tts_history: Some(false),
            ..CliArgs::default()
        };

        apply_tts_provider_override(&args, &mut effective).unwrap();
        apply_tts_conversion_overrides(&args, &mut effective).unwrap();

        assert_eq!(effective.provider, TtsProvider::Soniox);
        assert_eq!(effective.soniox_model, "sonic-preview");
        assert_eq!(effective.soniox_voice, "voice-id");
        assert_eq!(effective.soniox_language, "ru");
        assert_eq!(effective.soniox_key_source, TtsKeySource::Separate);
        assert_eq!(effective.speed, 1.2);
        assert_eq!(effective.output_format, TtsOutputFormat::Mp3);
        assert_eq!(effective.mp3_bitrate_kbps, 192);
        assert_eq!(effective.file_target_chars, 1_400);
        assert_eq!(effective.retry_count, 4);
        assert_eq!(effective.retry_base_delay_ms, 750);
        assert_eq!(effective.inter_chunk_pause_ms, 80);
        assert_eq!(effective.paragraph_pause_ms, 300);
        assert!(!effective.preprocessing_enabled);
        assert_eq!(effective.disk_reserve_mb, 1_024);
        assert!(!effective.file_history_enabled);

        assert_eq!(original.provider, TtsSettings::default().provider);
        assert_eq!(original.soniox_model, TtsSettings::default().soniox_model);
        assert_eq!(original.speed, TtsSettings::default().speed);
        assert_eq!(
            original.mp3_bitrate_kbps,
            TtsSettings::default().mp3_bitrate_kbps
        );
    }

    #[test]
    fn provider_incompatible_overrides_return_actionable_usage_errors() {
        let cases = [
            (
                CliArgs {
                    tts_provider: Some(CliTtsProvider::Deepgram),
                    tts_voice: Some("aura-voice".to_string()),
                    ..CliArgs::default()
                },
                "--tts-model",
            ),
            (
                CliArgs {
                    tts_provider: Some(CliTtsProvider::Openai),
                    tts_language: Some("ru".to_string()),
                    ..CliArgs::default()
                },
                "not supported by OpenAI",
            ),
            (
                CliArgs {
                    tts_provider: Some(CliTtsProvider::LocalQwen),
                    tts_model: Some("another-model".to_string()),
                    ..CliArgs::default()
                },
                "one pinned model",
            ),
            (
                CliArgs {
                    tts_provider: Some(CliTtsProvider::Windows),
                    tts_key_source: Some(CliTtsKeySource::Separate),
                    ..CliArgs::default()
                },
                "does not use an API key",
            ),
        ];

        for (args, expected) in cases {
            let mut settings = TtsSettings::default();
            let error = apply_tts_provider_override(&args, &mut settings)
                .expect_err("unsupported provider argument must fail");
            assert_eq!(error.exit_code, 2);
            assert!(
                error.message.contains(expected),
                "expected {:?} in {:?}",
                expected,
                error.message
            );
        }
    }

    #[test]
    fn invalid_scalar_overrides_return_usage_errors_instead_of_clamping() {
        let cases = [
            (
                CliArgs {
                    tts_provider: Some(CliTtsProvider::Soniox),
                    tts_speed: Some(1.31),
                    ..CliArgs::default()
                },
                "--tts-speed for soniox must be between 0.7 and 1.3",
            ),
            (
                CliArgs {
                    tts_bitrate: Some(160),
                    ..CliArgs::default()
                },
                "--tts-bitrate must be one of",
            ),
            (
                CliArgs {
                    tts_retries: Some(11),
                    ..CliArgs::default()
                },
                "--tts-retries must be between 0 and 10",
            ),
        ];

        for (args, expected) in cases {
            let mut settings = TtsSettings::default();
            apply_tts_provider_override(&args, &mut settings).unwrap();
            let error = apply_tts_conversion_overrides(&args, &mut settings)
                .expect_err("invalid scalar override must fail");
            assert_eq!(error.exit_code, 2);
            assert!(error.message.contains(expected));
        }
    }

    #[test]
    fn replacement_file_is_validated_and_applied_as_one_off_configuration() {
        let path = std::env::temp_dir().join(format!(
            "aivorelay-tts-replacements-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::write(
            &path,
            br#"[{"id":"cli-rule","from":"\\bAI\\b","to":"A I","enabled":true,"case_sensitive":false,"is_regex":true}]"#,
        )
        .expect("temporary rules should be writable");

        let mut settings = TtsSettings::default();
        let args = CliArgs {
            tts_replacements_file: Some(path.clone()),
            ..CliArgs::default()
        };
        apply_tts_conversion_overrides(&args, &mut settings)
            .expect("valid replacement rules should apply");
        let _ = fs::remove_file(path);

        assert!(settings.preprocessing_enabled);
        assert_eq!(settings.preprocessing_rules.len(), 1);
        assert_eq!(settings.preprocessing_rules[0].id, "cli-rule");
    }

    #[test]
    fn windows_provider_without_voice_override_uses_os_default_not_saved_voice() {
        let mut settings = TtsSettings::default();
        settings.windows_voice_id = "saved-voice-id".to_string();
        settings.windows_voice_language = "en-US".to_string();
        let args = CliArgs {
            tts_provider: Some(CliTtsProvider::Windows),
            ..CliArgs::default()
        };

        apply_tts_provider_override(&args, &mut settings).unwrap();

        assert_eq!(settings.provider, TtsProvider::Windows);
        assert!(settings.windows_voice_id.is_empty());
        assert!(settings.windows_voice_language.is_empty());
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
            convert_file: vec![PathBuf::from("chapter.md")],
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
    fn inline_utf8_cyrillic_instructions_survive_without_normalization() {
        let instructions = "Говори спокойно и произноси «АИ» по буквам.";
        let mut settings = TtsSettings::default();
        settings.provider = TtsProvider::OpenAi;
        let args = CliArgs {
            tts_instructions: Some(instructions.to_string()),
            ..CliArgs::default()
        };

        apply_tts_instruction_override(&args, &mut settings)
            .expect("UTF-8 inline instructions should resolve");

        assert_eq!(settings.openai_instructions, instructions);
    }

    #[test]
    fn large_utf8_instruction_file_is_loaded_without_copying_into_inline_argument() {
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-tts-instructions-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("create isolated instruction directory");
        let path = directory.join("large-instructions.txt");
        let instructions =
            "Говори медленно; сохраняй Unicode: русский, Ελληνικά, 日本語. ".repeat(64);
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(instructions.as_bytes());
        fs::write(&path, bytes).expect("write large UTF-8 instruction file");

        let mut settings = TtsSettings::default();
        settings.provider = TtsProvider::OpenAi;
        let args = CliArgs {
            tts_instructions_file: Some(path.clone()),
            ..CliArgs::default()
        };
        apply_tts_instruction_override(&args, &mut settings)
            .expect("large UTF-8 instruction file should resolve");

        assert_eq!(settings.openai_instructions, instructions);
        TtsManager::validate_openai_instructions(&settings.openai_instructions)
            .expect("the file fixture should remain valid for a real OpenAI TTS request");
        assert!(args.tts_instructions.is_none());
        assert_eq!(args.tts_instructions_file.as_deref(), Some(path.as_path()));

        fs::remove_dir_all(directory).expect("remove isolated instruction directory");
    }

    #[test]
    fn all_tts_llm_overrides_are_temporary_and_select_inline_instructions() {
        let original = TtsSettings::default();
        let mut effective = original.clone();
        let args = CliArgs {
            tts_llm_preprocessing: Some(true),
            tts_llm_instructions: Some("Remove page numbers.".to_string()),
            tts_llm_provider: Some("custom".to_string()),
            tts_llm_model: Some("cleanup-model".to_string()),
            tts_llm_key_source: Some(CliTtsKeySource::Separate),
            tts_llm_base_url: Some("https://example.test/v1/".to_string()),
            tts_llm_allow_insecure_http: Some(false),
            tts_llm_reasoning: Some(true),
            tts_llm_reasoning_budget: Some(4096),
            tts_llm_chunk_chars: Some(12_000),
            tts_llm_retries: Some(3),
            tts_llm_retry_delay_ms: Some(900),
            tts_llm_timeout_seconds: Some(120),
            ..CliArgs::default()
        };

        let source =
            apply_tts_llm_overrides(&args, &mut effective).expect("LLM overrides should apply");

        assert_eq!(source, "inline");
        assert!(effective.llm_preprocessing.file_enabled);
        assert_eq!(effective.llm_preprocessing.provider_id, "custom");
        assert_eq!(effective.llm_preprocessing.model, "cleanup-model");
        assert_eq!(
            effective.llm_preprocessing.key_source,
            TtsKeySource::Separate
        );
        assert_eq!(
            effective.llm_preprocessing.custom_base_url,
            "https://example.test/v1"
        );
        assert!(!effective.llm_preprocessing.custom_allow_insecure_http);
        assert!(effective.llm_preprocessing.reasoning_enabled);
        assert_eq!(effective.llm_preprocessing.reasoning_budget, 4096);
        assert_eq!(effective.llm_preprocessing.chunk_target_chars, 12_000);
        assert_eq!(effective.llm_preprocessing.retry_count, 3);
        assert_eq!(effective.llm_preprocessing.retry_base_delay_ms, 900);
        assert_eq!(effective.llm_preprocessing.request_timeout_seconds, 120);
        let selected = effective
            .llm_preprocessing
            .file_prompts
            .iter()
            .find(|prompt| prompt.id == effective.llm_preprocessing.file_selected_prompt_id)
            .expect("CLI prompt should be selected");
        assert_eq!(selected.prompt, "Remove page numbers.");

        assert!(!original.llm_preprocessing.file_enabled);
        assert_eq!(
            original.llm_preprocessing.provider_id,
            TtsSettings::default().llm_preprocessing.provider_id
        );
    }

    #[test]
    fn invalid_tts_llm_combinations_return_actionable_usage_errors() {
        let cases = [
            (
                CliArgs {
                    tts_llm_preprocessing: Some(false),
                    tts_llm_model: Some("cleanup-model".to_string()),
                    ..CliArgs::default()
                },
                "conflicts with other --tts-llm-* overrides",
            ),
            (
                CliArgs {
                    tts_llm_provider: Some("openrouter".to_string()),
                    tts_llm_base_url: Some("https://example.test/v1".to_string()),
                    ..CliArgs::default()
                },
                "only with --tts-llm-provider custom",
            ),
            (
                CliArgs {
                    tts_llm_reasoning: Some(false),
                    tts_llm_reasoning_budget: Some(4096),
                    ..CliArgs::default()
                },
                "requires reasoning to be enabled",
            ),
        ];

        for (args, expected) in cases {
            let mut settings = TtsSettings::default();
            let error = apply_tts_llm_overrides(&args, &mut settings)
                .expect_err("invalid LLM override combination must fail");
            assert_eq!(error.exit_code, 2);
            assert!(
                error.message.contains(expected),
                "expected {:?} in {:?}",
                expected,
                error.message
            );
        }
    }

    #[test]
    fn text_preview_is_unicode_safe() {
        assert_eq!(text_preview("Привет 🌍", 8), "Привет 🌍");
        assert_eq!(text_preview("日本語テスト", 3), "日本語…");
    }
}
