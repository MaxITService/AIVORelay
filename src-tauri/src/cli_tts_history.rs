//! Headless Text-to-Speech history CLI.
//!
//! All machine-readable invocations emit exactly one JSON object on stdout.
//! Human prompts/progress use stderr. Destructive and paid operations require
//! an interactive confirmation or `--yes`.

use crate::cli::{
    CliArgs, CliCommand, CliTtsHistoryScope, CliTtsOutputFormat, CliTtsProvider, TtsHistoryCommand,
    TtsHistoryDeleteArgs, TtsHistoryExportArgs, TtsHistoryListArgs, TtsHistoryRegenerateArgs,
    TtsHistoryShowArgs,
};
use crate::commands::tts_history::{
    regenerate_tts_history_entry_core, RegenerateTtsHistoryRequest,
};
use crate::managers::tts::{TtsManager, TtsPhase};
use crate::managers::tts_history::{
    TtsHistoryEntry, TtsHistoryManagedAudioDeleteStatus, TtsHistoryManager, TtsHistoryScope,
};
use crate::settings::{TtsOutputFormat, TtsProvider};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const EXIT_RUNTIME: i32 = 1;
const EXIT_USAGE: i32 = 2;
const EXIT_NOT_CONFIRMED: i32 = 3;
const EXIT_NOT_FOUND: i32 = 4;
const EXIT_OUTPUT_COLLISION: i32 = 5;
const EXIT_RETAINED_AUDIO_MISSING: i32 = 6;
const EXIT_PARTIAL: i32 = 7;
const ALLOWED_MP3_BITRATES: &[u16] = &[64, 96, 128, 192, 256, 320];

fn cli_history_scope(scope: CliTtsHistoryScope) -> TtsHistoryScope {
    match scope {
        CliTtsHistoryScope::Interactive => TtsHistoryScope::Interactive,
        CliTtsHistoryScope::File => TtsHistoryScope::File,
    }
}

fn history_scope_name(scope: TtsHistoryScope) -> &'static str {
    match scope {
        TtsHistoryScope::Interactive => "interactive",
        TtsHistoryScope::File => "file",
    }
}

fn history_capture_enabled(app: &AppHandle, scope: TtsHistoryScope) -> bool {
    let settings = crate::settings::get_settings(app).tts;
    match scope {
        TtsHistoryScope::Interactive => settings.interactive_history_enabled,
        TtsHistoryScope::File => settings.file_history_enabled,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmationPolicy {
    Confirmed,
    Prompt,
    Unavailable,
}

#[derive(Debug)]
struct CliFailure {
    exit_code: i32,
    message: String,
    details: Option<Value>,
}

impl CliFailure {
    fn new(exit_code: i32, message: impl Into<String>) -> Self {
        Self {
            exit_code,
            message: message.into(),
            details: None,
        }
    }

    fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

pub fn is_tts_history_requested(args: &CliArgs) -> bool {
    matches!(args.command, Some(CliCommand::TtsHistory(_)))
}

pub fn run_tts_history(app: &AppHandle, args: &CliArgs) -> i32 {
    let result = run_tts_history_inner(app, args);
    match result {
        Ok(value) => {
            if args.json {
                println!("{value}");
            }
            0
        }
        Err(failure) => {
            if args.json {
                println!("{}", failure_json(&failure));
            } else {
                eprintln!("error: {}", failure.message);
            }
            failure.exit_code
        }
    }
}

fn failure_json(failure: &CliFailure) -> Value {
    json!({
        "ok": false,
        "operation": "tts_history",
        "error": failure.message,
        "exit_code": failure.exit_code,
        "details": failure.details,
    })
}

fn run_tts_history_inner(app: &AppHandle, args: &CliArgs) -> Result<Value, CliFailure> {
    validate_root_scope(args)?;
    let history_args = match args.command.as_ref() {
        Some(CliCommand::TtsHistory(history)) => history,
        _ => return Err(CliFailure::new(EXIT_USAGE, "Missing tts-history command")),
    };
    let command = &history_args.command;
    let scope = cli_history_scope(history_args.scope);
    initialize_history_manager(app)?;
    let history = app.state::<Arc<TtsHistoryManager>>().inner().clone();

    match command {
        TtsHistoryCommand::List(options) => list_history(app, &history, scope, options, args.json),
        TtsHistoryCommand::Show(options) => show_history(app, &history, scope, options, args.json),
        TtsHistoryCommand::Export(options) => export_history(&history, scope, options, args.json),
        TtsHistoryCommand::Delete(options) => delete_history(&history, scope, options, args.json),
        TtsHistoryCommand::Regenerate(options) => {
            initialize_tts_manager(app)?;
            regenerate_history(app, &history, scope, options, args.json)
        }
    }
}

fn initialize_history_manager(app: &AppHandle) -> Result<(), CliFailure> {
    if app.try_state::<Arc<TtsHistoryManager>>().is_none() {
        let manager = Arc::new(TtsHistoryManager::new(app).map_err(|error| {
            CliFailure::new(
                EXIT_RUNTIME,
                format!("Failed to initialize TTS history: {error}"),
            )
        })?);
        app.manage(manager);
    }
    Ok(())
}

fn initialize_tts_manager(app: &AppHandle) -> Result<(), CliFailure> {
    if app.try_state::<Arc<TtsManager>>().is_none() {
        let manager = TtsManager::new(app).map_err(|error| {
            CliFailure::new(
                EXIT_RUNTIME,
                format!("Failed to initialize Text-to-Speech: {error}"),
            )
        })?;
        app.manage(manager);
    }
    Ok(())
}

fn validate_root_scope(args: &CliArgs) -> Result<(), CliFailure> {
    if args.toggle_transcription
        || args.toggle_post_process
        || args.cancel
        || args.transcribe_file.is_some()
        || args.convert_file.is_some()
        || args.output.is_some()
        || args.has_tts_file_conversion_args()
        || args.model.is_some()
        || args.device_index.is_some()
        || args.list_devices
        || args.repeat.is_some()
    {
        return Err(CliFailure::new(
            EXIT_USAGE,
            "tts-history cannot be combined with another AivoRelay operation",
        ));
    }
    Ok(())
}

fn list_history(
    app: &AppHandle,
    history: &TtsHistoryManager,
    scope: TtsHistoryScope,
    options: &TtsHistoryListArgs,
    json_mode: bool,
) -> Result<Value, CliFailure> {
    if options
        .group
        .as_deref()
        .is_some_and(|group| group.trim().is_empty())
    {
        return Err(CliFailure::new(EXIT_USAGE, "--group must not be empty"));
    }
    let mut entries = history
        .list_entries(scope)
        .map_err(|error| CliFailure::new(EXIT_RUNTIME, error.to_string()))?;
    if let Some(group) = options.group.as_deref() {
        entries.retain(|entry| entry.group_id == group);
        if entries.is_empty() {
            return Err(CliFailure::new(
                EXIT_NOT_FOUND,
                format!("TTS history group '{group}' not found"),
            ));
        }
    }
    if let Some(limit) = options.limit {
        entries.truncate(limit);
    }
    let history_enabled = history_capture_enabled(app, scope);
    let values = entries
        .iter()
        .map(|entry| history_entry_json(history, entry))
        .collect::<Result<Vec<_>, _>>()?;

    if !json_mode {
        if entries.is_empty() {
            let capture = if history_enabled {
                "enabled"
            } else {
                "disabled"
            };
            println!(
                "{} TTS history is empty (new-history capture is {capture}).",
                history_scope_name(scope)
            );
        } else {
            println!(
                "{:<8} {:<24} {:<10} {:<22} {:<7} TEXT",
                "ID", "TIME", "PROVIDER", "VOICE", "AUDIO"
            );
            for (entry, value) in entries.iter().zip(values.iter()) {
                let available = value["audio_available"].as_bool().unwrap_or(false);
                println!(
                    "{:<8} {:<24} {:<10} {:<22} {:<7} {}",
                    entry.id,
                    format_timestamp(entry.timestamp),
                    entry.provider.as_str(),
                    single_line(&entry.voice, 22),
                    if available { "yes" } else { "missing" },
                    single_line(&entry.source_text, 72)
                );
            }
            if !history_enabled {
                eprintln!(
                    "note: new-history capture is disabled; existing records remain available."
                );
            }
        }
    }

    Ok(json!({
        "ok": true,
        "operation": "tts_history_list",
        "scope": scope,
        "history_enabled": history_enabled,
        "count": values.len(),
        "group": options.group,
        "entries": values,
    }))
}

fn show_history(
    app: &AppHandle,
    history: &TtsHistoryManager,
    scope: TtsHistoryScope,
    options: &TtsHistoryShowArgs,
    json_mode: bool,
) -> Result<Value, CliFailure> {
    let entry = require_entry(history, options.id, scope)?;
    let value = history_entry_json(history, &entry)?;
    let history_enabled = history_capture_enabled(app, scope);
    if !json_mode {
        println!("ID: {}", entry.id);
        println!("Scope: {}", history_scope_name(entry.scope));
        println!("Group: {}", entry.group_id);
        println!("Time: {}", format_timestamp(entry.timestamp));
        println!("Provider: {}", entry.provider.as_str());
        println!("Model: {}", entry.model);
        println!(
            "Voice: {}",
            if entry.provider == TtsProvider::Windows && entry.voice.trim().is_empty() {
                "(Windows default voice)"
            } else {
                entry.voice.as_str()
            }
        );
        println!(
            "Language: {}",
            if entry.language.trim().is_empty() {
                "(not recorded)"
            } else {
                entry.language.as_str()
            }
        );
        println!("Format: {}", output_format_name(entry.output_format));
        println!("Source kind: {:?}", entry.source_kind);
        println!(
            "Prompt preset: {}",
            entry.prompt_preset_name.as_deref().unwrap_or("(none)")
        );
        println!(
            "Retained audio: {}",
            if value["audio_available"].as_bool().unwrap_or(false) {
                "available"
            } else {
                "MISSING"
            }
        );
        if let Some(path) = entry.external_output_path.as_deref() {
            println!("Original external output: {path}");
        }
        if let Some(instructions) = entry.resolved_instructions.as_deref() {
            println!("Resolved instructions:\n{instructions}");
        }
        println!("Source text:\n{}", entry.source_text);
    }
    Ok(json!({
        "ok": true,
        "operation": "tts_history_show",
        "scope": scope,
        "history_enabled": history_enabled,
        "entry": value,
    }))
}

fn export_history(
    history: &TtsHistoryManager,
    scope: TtsHistoryScope,
    options: &TtsHistoryExportArgs,
    json_mode: bool,
) -> Result<Value, CliFailure> {
    let entry = require_entry(history, options.id, scope)?;
    let retained = require_retained_audio(history, &entry)?;
    let output = absolute_path(&options.output)?;
    ensure_new_output(&output)?;
    let output_format = format_from_audio_path(&output)?;
    if output_format != entry.output_format {
        return Err(CliFailure::new(
            EXIT_USAGE,
            format!(
                "Export extension must match retained {} audio",
                output_format_name(entry.output_format)
            ),
        ));
    }
    if !json_mode {
        eprintln!(
            "TTS history: exporting retained audio {}…",
            retained.display()
        );
    }
    let exported = history
        .export_audio(entry.id, &output)
        .map_err(|error| classify_export_error(error.to_string()))?;
    let bytes = std::fs::metadata(&exported)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if !json_mode {
        println!("Created {}", exported.display());
    }
    Ok(json!({
        "ok": true,
        "operation": "tts_history_export",
        "scope": entry.scope,
        "id": entry.id,
        "group_id": entry.group_id,
        "output": exported,
        "output_bytes": bytes,
        "api_request_made": false,
    }))
}

fn delete_history(
    history: &TtsHistoryManager,
    scope: TtsHistoryScope,
    options: &TtsHistoryDeleteArgs,
    json_mode: bool,
) -> Result<Value, CliFailure> {
    let entry = require_entry(history, options.id, scope)?;
    let confirmed = confirm(
        options.yes,
        json_mode,
        &format!(
            "Delete TTS history result {} and its retained audio? External exports will not be deleted. [y/N] ",
            entry.id
        ),
    )?;
    if !confirmed {
        return Err(CliFailure::new(
            EXIT_NOT_CONFIRMED,
            "TTS history deletion was not confirmed",
        ));
    }

    let outcome = history
        .delete_entry_detailed(entry.id)
        .map_err(|error| CliFailure::new(EXIT_RUNTIME, error.to_string()))?
        .ok_or_else(|| {
            CliFailure::new(
                EXIT_NOT_FOUND,
                format!("TTS history entry {} no longer exists", entry.id),
            )
        })?;
    let outcome_json = serde_json::to_value(&outcome)
        .map_err(|error| CliFailure::new(EXIT_RUNTIME, error.to_string()))?;
    match outcome.managed_audio_status {
        TtsHistoryManagedAudioDeleteStatus::Deleted => {
            if !json_mode {
                println!("Deleted TTS history result {}.", entry.id);
            }
            Ok(json!({
                "ok": true,
                "operation": "tts_history_delete",
                "scope": entry.scope,
                "outcome": outcome_json,
            }))
        }
        TtsHistoryManagedAudioDeleteStatus::Missing => Err(CliFailure::new(
            EXIT_PARTIAL,
            format!(
                "History record {} was deleted, but its retained audio was already missing",
                entry.id
            ),
        )
        .with_details(outcome_json)),
        TtsHistoryManagedAudioDeleteStatus::Failed => Err(CliFailure::new(
            EXIT_PARTIAL,
            format!(
                "History record {} was deleted, but its retained audio could not be removed",
                entry.id
            ),
        )
        .with_details(outcome_json)),
    }
}

fn regenerate_history(
    app: &AppHandle,
    history: &Arc<TtsHistoryManager>,
    scope: TtsHistoryScope,
    options: &TtsHistoryRegenerateArgs,
    json_mode: bool,
) -> Result<Value, CliFailure> {
    let source_entry = require_entry(history, options.id, scope)?;
    let requested_format = options.format.map(cli_output_format);
    let (output, output_format) = if let Some(output) = options.output.as_deref() {
        let output = absolute_path(output)?;
        ensure_new_output(&output)?;
        let format_from_path = format_from_audio_path(&output)?;
        if requested_format.is_some_and(|format| format != format_from_path) {
            return Err(CliFailure::new(
                EXIT_USAGE,
                "--format must match the --output extension",
            ));
        }
        (Some(output), format_from_path)
    } else {
        (None, requested_format.unwrap_or(TtsOutputFormat::Mp3))
    };
    if let Some(bitrate) = options.bitrate {
        if output_format != TtsOutputFormat::Mp3 {
            return Err(CliFailure::new(
                EXIT_USAGE,
                "--bitrate is valid only for MP3 regeneration",
            ));
        }
        if !ALLOWED_MP3_BITRATES.contains(&bitrate) {
            return Err(CliFailure::new(
                EXIT_USAGE,
                format!(
                    "Unsupported MP3 bitrate {bitrate}; use {} kb/s",
                    ALLOWED_MP3_BITRATES
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
    if options
        .model
        .as_deref()
        .is_some_and(|model| model.trim().is_empty())
        || options
            .voice
            .as_deref()
            .is_some_and(|voice| voice.trim().is_empty())
    {
        return Err(CliFailure::new(
            EXIT_USAGE,
            "--model and --voice overrides must not be empty",
        ));
    }

    let provider = options.provider.map(cli_provider);
    let final_provider = provider.unwrap_or(source_entry.provider);
    if final_provider == TtsProvider::Deepgram {
        if let (Some(model), Some(voice)) = (options.model.as_deref(), options.voice.as_deref()) {
            if !model.trim().eq_ignore_ascii_case(voice.trim()) {
                return Err(CliFailure::new(
                    EXIT_USAGE,
                    "Deepgram uses one model identifier as its voice; --model and --voice must match",
                ));
            }
        }
    }
    let instructions = resolve_cli_instructions(options)?;
    let effective_prompt_name =
        if options.tts_instructions_file.is_none() && options.tts_instructions.is_none() {
            options.tts_prompt.clone()
        } else {
            None
        };
    if (instructions.is_some() || effective_prompt_name.is_some())
        && final_provider != TtsProvider::OpenAi
    {
        return Err(CliFailure::new(
            EXIT_USAGE,
            "TTS instruction prompts require the OpenAI provider",
        ));
    }
    if let Some(instructions) = instructions.as_deref() {
        TtsManager::validate_openai_instructions(instructions)
            .map_err(|error| CliFailure::new(EXIT_USAGE, error.to_string()))?;
    }
    if let Some(name) = effective_prompt_name.as_deref() {
        validate_prompt_name(app, name)?;
    }

    let confirmed = !final_provider.requires_paid_confirmation()
        || confirm(
            options.yes,
            json_mode,
            &format!(
                "WARNING: regenerate result {} with {} as a NEW PAID API request? The old result stays unchanged and the new variant is appended to group {}. [y/N] ",
                source_entry.id,
                final_provider.as_str(),
                source_entry.group_id
            ),
        )?;
    if !confirmed {
        return Err(CliFailure::new(
            EXIT_NOT_CONFIRMED,
            "Paid TTS history regeneration was not confirmed",
        ));
    }

    let request = RegenerateTtsHistoryRequest {
        id: source_entry.id,
        output_path: output
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        provider,
        model: options.model.clone(),
        voice: options.voice.clone(),
        prompt_preset_id: None,
        prompt_preset_name: effective_prompt_name,
        instructions,
        output_format: Some(output_format),
        mp3_bitrate_kbps: options
            .bitrate
            .or((output.is_none() && output_format == TtsOutputFormat::Mp3).then_some(256)),
        confirmed_api_charge: true,
    };
    let tts = app.state::<Arc<TtsManager>>().inner().clone();
    if !json_mode {
        eprintln!(
            "TTS history: starting {} regeneration with {}…",
            if final_provider.requires_paid_confirmation() {
                "paid API"
            } else {
                "offline"
            },
            final_provider.as_str()
        );
    }
    let started = Instant::now();
    let response = tauri::async_runtime::block_on(async {
        let mut operation = Box::pin(regenerate_tts_history_entry_core(
            app, history, &tts, request,
        ));
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_status = None;
        loop {
            tokio::select! {
                result = &mut operation => break result,
                _ = interval.tick(), if !json_mode => {
                    let state = tts.current_state();
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
    })
    .map_err(classify_regeneration_error)?;

    let elapsed_ms = started.elapsed().as_millis();
    if !json_mode {
        println!(
            "Created {}",
            response
                .output_path
                .as_deref()
                .unwrap_or("(managed history copy)")
        );
        eprintln!(
            "TTS history: appended result {} to group {} in {} ms ({} chunk(s) recovered from checkpoint).",
            response.new_entry.id,
            response.new_entry.group_id,
            elapsed_ms,
            response.resumed_chunks
        );
    }
    Ok(json!({
        "ok": true,
        "operation": "tts_history_regenerate",
        "scope": response.new_entry.scope,
        "source_entry_id": response.source_entry_id,
        "new_entry": response.new_entry,
        "output": response.output_path,
        "operation_id": response.operation_id,
        "chunks": response.chunk_count,
        "resumed_chunks": response.resumed_chunks,
        "processed_characters": response.processed_character_count,
        "elapsed_ms": elapsed_ms,
        "api_request_made": regeneration_makes_api_request(
            final_provider,
            response.resumed_chunks,
            response.chunk_count,
        ),
        "prior_variants_preserved": true,
    }))
}

fn regeneration_makes_api_request(
    provider: TtsProvider,
    resumed_chunks: usize,
    chunk_count: usize,
) -> bool {
    provider.requires_paid_confirmation() && resumed_chunks < chunk_count
}

fn require_entry(
    history: &TtsHistoryManager,
    id: i64,
    scope: TtsHistoryScope,
) -> Result<TtsHistoryEntry, CliFailure> {
    if id <= 0 {
        return Err(CliFailure::new(
            EXIT_USAGE,
            "TTS history result ID must be a positive integer",
        ));
    }
    let entry = history
        .get_entry_by_id(id)
        .map_err(|error| CliFailure::new(EXIT_RUNTIME, error.to_string()))?
        .ok_or_else(|| {
            CliFailure::new(EXIT_NOT_FOUND, format!("TTS history entry {id} not found"))
        })?;
    if entry.scope != scope {
        return Err(CliFailure::new(
            EXIT_NOT_FOUND,
            format!(
                "TTS history entry {id} belongs to the {} scope, not {}",
                history_scope_name(entry.scope),
                history_scope_name(scope)
            ),
        ));
    }
    Ok(entry)
}

fn require_retained_audio(
    history: &TtsHistoryManager,
    entry: &TtsHistoryEntry,
) -> Result<PathBuf, CliFailure> {
    let path = history
        .retained_audio_path(entry.id)
        .map_err(|error| CliFailure::new(EXIT_RUNTIME, error.to_string()))?
        .ok_or_else(|| {
            CliFailure::new(
                EXIT_NOT_FOUND,
                format!("TTS history entry {} not found", entry.id),
            )
        })?;
    if !path.is_file() {
        return Err(CliFailure::new(
            EXIT_RETAINED_AUDIO_MISSING,
            format!(
                "Retained audio for TTS history entry {} is missing: {}",
                entry.id,
                path.display()
            ),
        ));
    }
    Ok(path)
}

fn history_entry_json(
    history: &TtsHistoryManager,
    entry: &TtsHistoryEntry,
) -> Result<Value, CliFailure> {
    let retained_path = history
        .retained_audio_path(entry.id)
        .map_err(|error| CliFailure::new(EXIT_RUNTIME, error.to_string()))?;
    let audio_available = retained_path.as_deref().is_some_and(Path::is_file);
    Ok(json!({
        "id": entry.id,
        "timestamp": entry.timestamp,
        "timestamp_iso": format_timestamp(entry.timestamp),
        "scope": entry.scope,
        "group_id": entry.group_id,
        "source_text": entry.source_text,
        "source_kind": entry.source_kind,
        "provider": entry.provider,
        "model": entry.model,
        "voice": entry.voice,
        "language": entry.language,
        "output_format": entry.output_format,
        "managed_audio_filename": entry.managed_audio_filename,
        "external_output_path": entry.external_output_path,
        "prompt_preset_id": entry.prompt_preset_id,
        "prompt_preset_name": entry.prompt_preset_name,
        "resolved_instructions": entry.resolved_instructions,
        "audio_available": audio_available,
        "retained_audio_path": retained_path,
    }))
}

fn validate_prompt_name(app: &AppHandle, name: &str) -> Result<(), CliFailure> {
    let settings = crate::settings::get_settings(app).tts;
    let matches = settings
        .prompt_presets
        .iter()
        .filter(|preset| preset.name.eq_ignore_ascii_case(name.trim()))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(CliFailure::new(
            EXIT_USAGE,
            format!("Unknown TTS prompt preset '{}'", name.trim()),
        )),
        [preset] => TtsManager::validate_openai_instructions(&preset.instructions)
            .map_err(|error| CliFailure::new(EXIT_USAGE, error.to_string())),
        _ => Err(CliFailure::new(
            EXIT_USAGE,
            format!("More than one TTS prompt preset is named '{}'", name.trim()),
        )),
    }
}

fn resolve_cli_instructions(
    options: &TtsHistoryRegenerateArgs,
) -> Result<Option<String>, CliFailure> {
    if let Some(path) = options.tts_instructions_file.as_deref() {
        return read_utf8_bom(path).map(Some);
    }
    Ok(options.tts_instructions.clone())
}

fn read_utf8_bom(path: &Path) -> Result<String, CliFailure> {
    let path = absolute_path(path)?;
    let mut bytes = Vec::new();
    File::open(&path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| {
            CliFailure::new(
                EXIT_USAGE,
                format!(
                    "Failed to read TTS instructions file {}: {error}",
                    path.display()
                ),
            )
        })?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        CliFailure::new(
            EXIT_USAGE,
            format!("TTS instructions file must be UTF-8: {}", path.display()),
        )
    })
}

fn confirm(yes: bool, json_mode: bool, prompt: &str) -> Result<bool, CliFailure> {
    match confirmation_policy(yes, json_mode, std::io::stdin().is_terminal()) {
        ConfirmationPolicy::Confirmed => return Ok(true),
        ConfirmationPolicy::Unavailable => {
            let context = if json_mode {
                "JSON mode"
            } else {
                "this non-interactive context"
            };
            return Err(CliFailure::new(
                EXIT_NOT_CONFIRMED,
                format!("Confirmation is unavailable in {context}; pass --yes explicitly"),
            ));
        }
        ConfirmationPolicy::Prompt => {}
    }
    eprint!("{prompt}");
    std::io::stderr()
        .flush()
        .map_err(|error| CliFailure::new(EXIT_RUNTIME, error.to_string()))?;
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|error| CliFailure::new(EXIT_RUNTIME, error.to_string()))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn confirmation_policy(yes: bool, json_mode: bool, stdin_is_terminal: bool) -> ConfirmationPolicy {
    if yes {
        ConfirmationPolicy::Confirmed
    } else if json_mode || !stdin_is_terminal {
        ConfirmationPolicy::Unavailable
    } else {
        ConfirmationPolicy::Prompt
    }
}

fn ensure_new_output(path: &Path) -> Result<(), CliFailure> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match output_preflight(path.exists(), parent.is_dir()) {
        Err(EXIT_OUTPUT_COLLISION) => {
            return Err(CliFailure::new(
                EXIT_OUTPUT_COLLISION,
                format!(
                    "Output file already exists: {}. Existing files are never overwritten.",
                    path.display()
                ),
            ));
        }
        Err(_) => {
            return Err(CliFailure::new(
                EXIT_USAGE,
                format!("Output directory does not exist: {}", parent.display()),
            ));
        }
        Ok(()) => {}
    }
    Ok(())
}

fn output_preflight(output_exists: bool, parent_is_directory: bool) -> Result<(), i32> {
    if output_exists {
        Err(EXIT_OUTPUT_COLLISION)
    } else if !parent_is_directory {
        Err(EXIT_USAGE)
    } else {
        Ok(())
    }
}

fn classify_export_error(message: String) -> CliFailure {
    let lower = message.to_ascii_lowercase();
    if lower.starts_with("tts history entry ") && lower.ends_with(" not found") {
        CliFailure::new(EXIT_NOT_FOUND, message)
    } else if is_output_collision_message(&lower) {
        CliFailure::new(EXIT_OUTPUT_COLLISION, message)
    } else if lower.contains("not found") || lower.contains("failed to inspect audio source") {
        CliFailure::new(EXIT_RETAINED_AUDIO_MISSING, message)
    } else {
        CliFailure::new(EXIT_RUNTIME, message)
    }
}

fn classify_regeneration_error(message: String) -> CliFailure {
    if message.contains("but the new history variant could not be retained") {
        CliFailure::new(EXIT_PARTIAL, message)
    } else if message.starts_with("TTS history entry ") && message.ends_with(" not found") {
        CliFailure::new(EXIT_NOT_FOUND, message)
    } else if message.starts_with("Output file already exists:")
        || (message.starts_with("Failed to publish completed audio file")
            && is_output_collision_message(&message.to_ascii_lowercase()))
    {
        CliFailure::new(EXIT_OUTPUT_COLLISION, message)
    } else if is_regeneration_usage_error(&message) {
        CliFailure::new(EXIT_USAGE, message)
    } else {
        CliFailure::new(EXIT_RUNTIME, message)
    }
}

fn is_output_collision_message(lowercase_message: &str) -> bool {
    lowercase_message.contains("already exists") || lowercase_message.contains("file exists")
}

fn is_regeneration_usage_error(message: &str) -> bool {
    [
        "--bitrate",
        "--model",
        "--voice",
        "Deepgram uses one model identifier",
        "OpenAI voice instructions require",
        "Regeneration output must end",
        "Requested output format does not match",
        "Saved MP3 bitrate",
        "Specify a TTS prompt preset",
        "TTS instruction prompts require",
        "Unknown TTS prompt preset",
        "Unsupported MP3 bitrate",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}

fn format_from_audio_path(path: &Path) -> Result<TtsOutputFormat, CliFailure> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp3") => Ok(TtsOutputFormat::Mp3),
        Some("wav") => Ok(TtsOutputFormat::Wav),
        _ => Err(CliFailure::new(
            EXIT_USAGE,
            "Output path must end in .mp3 or .wav",
        )),
    }
}

fn cli_provider(provider: CliTtsProvider) -> TtsProvider {
    match provider {
        CliTtsProvider::Soniox => TtsProvider::Soniox,
        CliTtsProvider::Deepgram => TtsProvider::Deepgram,
        CliTtsProvider::Openai => TtsProvider::OpenAi,
        CliTtsProvider::LocalQwen => TtsProvider::LocalQwen,
        CliTtsProvider::Windows => TtsProvider::Windows,
    }
}

fn cli_output_format(format: CliTtsOutputFormat) -> TtsOutputFormat {
    match format {
        CliTtsOutputFormat::Mp3 => TtsOutputFormat::Mp3,
        CliTtsOutputFormat::Wav => TtsOutputFormat::Wav,
    }
}

fn output_format_name(format: TtsOutputFormat) -> &'static str {
    match format {
        TtsOutputFormat::Mp3 => "MP3",
        TtsOutputFormat::Wav => "WAV",
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, CliFailure> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| CliFailure::new(EXIT_USAGE, error.to_string()))
    }
}

fn format_timestamp(timestamp_ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp_ms)
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| timestamp_ms.to_string())
}

fn single_line(text: &str, maximum_characters: usize) -> String {
    let sanitized = text
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let mut preview = sanitized
        .chars()
        .take(maximum_characters)
        .collect::<String>();
    if sanitized.chars().count() > maximum_characters {
        preview.push('…');
    }
    preview
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
        TtsPhase::Retrying => eprintln!(
            "TTS: retry {} for chunk {}/{} — {}",
            state.current_attempt,
            state.completed_chunks.saturating_add(1),
            state.total_chunks,
            state
                .message
                .as_deref()
                .unwrap_or("provider request failed")
        ),
        TtsPhase::Ready => eprintln!("TTS: audio chunks ready…"),
        TtsPhase::Completed => eprintln!("TTS: retaining new history variant…"),
        TtsPhase::Cancelled => eprintln!("TTS: cancelled."),
        TtsPhase::Error => {
            if let Some(message) = state.message.as_deref() {
                eprintln!("TTS: provider error — {message}");
            }
        }
        TtsPhase::Idle => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{CliTtsOutputFormat, CliTtsProvider};

    #[test]
    fn provider_and_format_mapping_are_explicit() {
        assert_eq!(cli_provider(CliTtsProvider::Openai), TtsProvider::OpenAi);
        assert_eq!(
            cli_output_format(CliTtsOutputFormat::Wav),
            TtsOutputFormat::Wav
        );
    }

    #[test]
    fn regeneration_api_request_flag_distinguishes_cloud_offline_and_full_resume() {
        assert!(regeneration_makes_api_request(TtsProvider::OpenAi, 1, 2));
        assert!(!regeneration_makes_api_request(TtsProvider::OpenAi, 2, 2));
        assert!(!regeneration_makes_api_request(TtsProvider::Windows, 0, 2));
        assert!(!regeneration_makes_api_request(
            TtsProvider::LocalQwen,
            0,
            2
        ));
    }

    #[test]
    fn audio_output_format_rejects_unknown_extension() {
        assert_eq!(
            format_from_audio_path(Path::new("voice.MP3")).expect("MP3"),
            TtsOutputFormat::Mp3
        );
        assert!(format_from_audio_path(Path::new("voice.flac")).is_err());
    }

    #[test]
    fn single_line_removes_terminal_controls_and_truncates() {
        assert_eq!(single_line("line\nbreak\u{0007}", 20), "line break ");
        assert_eq!(single_line("abcdef", 3), "abc…");
    }

    #[test]
    fn confirmation_policy_requires_yes_when_prompting_is_unavailable() {
        assert_eq!(
            confirmation_policy(true, true, false),
            ConfirmationPolicy::Confirmed
        );
        assert_eq!(
            confirmation_policy(false, true, true),
            ConfirmationPolicy::Unavailable
        );
        assert_eq!(
            confirmation_policy(false, false, false),
            ConfirmationPolicy::Unavailable
        );
        assert_eq!(
            confirmation_policy(false, false, true),
            ConfirmationPolicy::Prompt
        );
    }

    #[test]
    fn output_collision_and_missing_parent_have_distinct_codes() {
        assert_eq!(output_preflight(true, true), Err(EXIT_OUTPUT_COLLISION));
        assert_eq!(output_preflight(false, false), Err(EXIT_USAGE));
        assert_eq!(output_preflight(false, true), Ok(()));
    }

    #[test]
    fn missing_retained_audio_has_a_dedicated_exit_code() {
        let failure = classify_export_error(
            "Failed to inspect audio source C:\\missing\\retained.mp3".to_string(),
        );
        assert_eq!(failure.exit_code, EXIT_RETAINED_AUDIO_MISSING);
    }

    #[test]
    fn export_races_keep_not_found_and_collision_exit_codes() {
        let missing = classify_export_error("TTS history entry 42 not found".to_string());
        assert_eq!(missing.exit_code, EXIT_NOT_FOUND);
        let collision = classify_export_error(
            "Failed to publish audio copy C:\\voice.mp3: File exists (os error 80)".to_string(),
        );
        assert_eq!(collision.exit_code, EXIT_OUTPUT_COLLISION);
    }

    #[test]
    fn unrelated_not_found_regeneration_error_is_not_a_missing_history_id() {
        let failure = classify_regeneration_error("Provider model endpoint not found".to_string());
        assert_eq!(failure.exit_code, EXIT_RUNTIME);
        let missing = classify_regeneration_error("TTS history entry 42 not found".to_string());
        assert_eq!(missing.exit_code, EXIT_NOT_FOUND);
    }

    #[test]
    fn regeneration_validation_and_collision_errors_have_specific_exit_codes() {
        let usage = classify_regeneration_error(
            "OpenAI voice instructions require a gpt-4o-mini-tts model".to_string(),
        );
        assert_eq!(usage.exit_code, EXIT_USAGE);
        let collision =
            classify_regeneration_error("Output file already exists: C:\\voice.mp3".to_string());
        assert_eq!(collision.exit_code, EXIT_OUTPUT_COLLISION);
        let publish_race = classify_regeneration_error(
            "Failed to publish completed audio file C:\\voice.mp3: File exists (os error 80)"
                .to_string(),
        );
        assert_eq!(publish_race.exit_code, EXIT_OUTPUT_COLLISION);
        let unrelated =
            classify_regeneration_error("Provider says account already exists".to_string());
        assert_eq!(unrelated.exit_code, EXIT_RUNTIME);
    }

    #[test]
    fn json_failure_output_is_one_parseable_object_without_human_noise() {
        let failure =
            CliFailure::new(EXIT_NOT_FOUND, "entry not found").with_details(json!({ "id": 42 }));
        let serialized = serde_json::to_string(&failure_json(&failure)).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        assert!(!serialized.contains('\n'));
        assert_eq!(parsed["ok"], false);
        assert_eq!(parsed["operation"], "tts_history");
        assert_eq!(parsed["exit_code"], EXIT_NOT_FOUND);
        assert_eq!(parsed["details"]["id"], 42);
    }
}
