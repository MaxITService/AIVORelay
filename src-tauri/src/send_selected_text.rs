use crate::commands::voice_command::{
    execute_powershell_command_captured, execute_powershell_command_with_environment_tracked,
};
use crate::managers::send_selected_text_history::{
    NewSendSelectedTextHistoryEntry, SendSelectedTextHistoryManager, SendSelectedTextHistoryStatus,
};
use crate::settings::{
    get_settings, ResolvedExecutionOptions, SendSelectedTextCaptureMode, SendSelectedTextFormat,
    SendSelectedTextOversizeBehavior, SendSelectedTextPreset, SendSelectedTextWriteMode,
};
use chrono::{Local, SecondsFormat, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const MAX_COMMAND_OUTPUT_CHARS: usize = 100_000;
const MAX_SELECTED_TEXT_CHARS: u32 = 2_000_000;
static OPERATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static OUTPUT_WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static ACTIVE_COMMAND_INPUT_FILES: Lazy<Mutex<HashSet<PathBuf>>> =
    Lazy::new(|| Mutex::new(HashSet::new()));

#[derive(Clone, Debug, Serialize, Type)]
pub struct SendSelectedTextOperationResult {
    pub history_id: i64,
    pub operation_id: String,
    pub output_path: String,
    pub status: SendSelectedTextHistoryStatus,
    pub command_output: Option<String>,
    pub command_output_truncated: bool,
}

#[derive(Clone, Debug)]
struct TemplateContext {
    record_id: String,
    timestamp: String,
    timestamp_local: String,
    date: String,
    time: String,
    preset_id: String,
    preset_name: String,
    text: String,
}

struct CommandInputFile {
    path: PathBuf,
}

impl Drop for CommandInputFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        ACTIVE_COMMAND_INPUT_FILES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.path);
    }
}

struct WriteRequest<'a> {
    preset: &'a SendSelectedTextPreset,
    context: &'a TemplateContext,
    history: &'a SendSelectedTextHistoryManager,
}

trait SelectedTextWriter {
    fn write(&self, request: &WriteRequest<'_>) -> Result<PathBuf, String>;
}

struct MarkdownWriter;
struct JsonWriter;

#[derive(Debug, Deserialize, Serialize)]
struct JsonSelectedTextDocument {
    version: u32,
    entries: Vec<JsonSelectedTextEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonSelectedTextEntry {
    id: String,
    timestamp: String,
    preset_id: String,
    preset_name: String,
    text: String,
}

impl SelectedTextWriter for MarkdownWriter {
    fn write(&self, request: &WriteRequest<'_>) -> Result<PathBuf, String> {
        let path = resolve_output_path(request, "md")?;
        let content = render_content_template(&request.preset.content_template, request.context);
        match request.preset.write_mode {
            SendSelectedTextWriteMode::CreateNew | SendSelectedTextWriteMode::AppendLast => {
                if path.exists() {
                    append_markdown(&path, &content)?;
                } else {
                    write_new_file(&path, content.as_bytes())?;
                }
            }
            SendSelectedTextWriteMode::AppendFile => append_markdown(&path, &content)?,
            SendSelectedTextWriteMode::OverwriteFile => {
                write_bytes_atomic(&path, content.as_bytes())?
            }
        }
        Ok(path)
    }
}

impl SelectedTextWriter for JsonWriter {
    fn write(&self, request: &WriteRequest<'_>) -> Result<PathBuf, String> {
        if request.preset.write_mode == SendSelectedTextWriteMode::AppendLast {
            return Err("Append to last file is available only for Markdown presets.".to_string());
        }

        let path = resolve_output_path(request, "json")?;
        let mut document = if request.preset.write_mode == SendSelectedTextWriteMode::OverwriteFile
            || !path.exists()
        {
            JsonSelectedTextDocument {
                version: 1,
                entries: Vec::new(),
            }
        } else {
            read_json_document(&path)?
        };
        document.entries.push(JsonSelectedTextEntry {
            id: request.context.record_id.clone(),
            timestamp: request.context.timestamp.clone(),
            preset_id: request.context.preset_id.clone(),
            preset_name: request.context.preset_name.clone(),
            text: request.context.text.clone(),
        });
        let keep_last = request.preset.json_keep_last as usize;
        if keep_last > 0 && document.entries.len() > keep_last {
            let remove_count = document.entries.len() - keep_last;
            document.entries.drain(0..remove_count);
        }
        write_json_atomic(&path, &document)?;
        Ok(path)
    }
}

pub async fn run_preset(
    app: AppHandle,
    preset_id: String,
    supplied_text: Option<String>,
) -> Result<SendSelectedTextOperationResult, String> {
    let app_for_work = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_preset_blocking(&app_for_work, &preset_id, supplied_text)
    })
    .await
    .map_err(|error| format!("Send Selected Text worker stopped unexpectedly: {error}"))?
}

fn run_preset_blocking(
    app: &AppHandle,
    preset_id: &str,
    supplied_text: Option<String>,
) -> Result<SendSelectedTextOperationResult, String> {
    let settings = get_settings(app);
    let feature_settings = settings.send_selected_text.clone();
    let preset = match feature_settings
        .presets
        .iter()
        .find(|preset| preset.id == preset_id)
        .cloned()
    {
        Some(preset) => preset,
        None => {
            let error = format!("Send Selected Text preset '{preset_id}' was not found");
            show_workflow_error(app, &feature_settings, &error);
            return Err(error);
        }
    };
    let operation_id = new_operation_id();
    let history = Arc::clone(&app.state::<Arc<SendSelectedTextHistoryManager>>());
    if let Err(error) = validate_preset(&preset) {
        return record_and_report_failure(
            app,
            &history,
            &feature_settings,
            &preset,
            operation_id,
            String::new(),
            error,
        );
    }
    if !preset.enabled {
        return record_and_report_failure(
            app,
            &history,
            &feature_settings,
            &preset,
            operation_id,
            String::new(),
            format!("Preset '{}' is disabled", preset.name),
        );
    }

    let captured = match supplied_text {
        Some(text) => Ok(text),
        None => capture_selected_text(app, preset.capture_mode),
    };
    let selected_text = match captured {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => {
            return record_and_report_failure(
                app,
                &history,
                &feature_settings,
                &preset,
                operation_id,
                String::new(),
                "No readable text is selected.".to_string(),
            )
        }
        Err(error) => {
            return record_and_report_failure(
                app,
                &history,
                &feature_settings,
                &preset,
                operation_id,
                String::new(),
                error,
            )
        }
    };

    let selected_text = match enforce_text_limit(&selected_text, &preset) {
        Ok(text) => text,
        Err(error) => {
            log::warn!(
                "Send Selected Text preset '{}' rejected the selection: {error}",
                preset.name
            );
            show_workflow_error(app, &feature_settings, &error);
            return Err(error);
        }
    };
    let now = Utc::now();
    let local_now = now.with_timezone(&Local);
    let context = TemplateContext {
        record_id: operation_id.clone(),
        timestamp: now.to_rfc3339_opts(SecondsFormat::Millis, true),
        timestamp_local: local_now.to_rfc3339_opts(SecondsFormat::Millis, true),
        date: local_now.format("%Y-%m-%d").to_string(),
        time: local_now.format("%H-%M-%S-%3f").to_string(),
        preset_id: preset.id.clone(),
        preset_name: preset.name.clone(),
        text: selected_text.clone(),
    };

    let command_template = preset
        .command_enabled
        .then(|| preset.command.trim().to_string())
        .filter(|command| !command.is_empty());
    let initial_status = if command_template.is_some() {
        SendSelectedTextHistoryStatus::Saved
    } else {
        SendSelectedTextHistoryStatus::Completed
    };

    let (output_path, history_entry) = {
        let _write_guard = OUTPUT_WRITE_LOCK
            .lock()
            .map_err(|_| "Send Selected Text output lock is unavailable".to_string())?;
        let request = WriteRequest {
            preset: &preset,
            context: &context,
            history: &history,
        };
        let writer: Box<dyn SelectedTextWriter> = match preset.format {
            SendSelectedTextFormat::Markdown => Box::new(MarkdownWriter),
            SendSelectedTextFormat::Json => Box::new(JsonWriter),
        };
        let output_path = match writer.write(&request) {
            Ok(path) => path,
            Err(error) => {
                return record_and_report_failure(
                    app,
                    &history,
                    &feature_settings,
                    &preset,
                    operation_id,
                    selected_text,
                    error,
                )
            }
        };
        let output_path_string = output_path.to_string_lossy().into_owned();
        let history_entry = match history.insert(
            NewSendSelectedTextHistoryEntry {
                operation_id: operation_id.clone(),
                preset_id: preset.id.clone(),
                preset_name: preset.name.clone(),
                timestamp_ms: now.timestamp_millis(),
                selected_text: selected_text.clone(),
                output_path: Some(output_path_string),
                output_format: format_name(preset.format).to_string(),
                write_mode: write_mode_name(preset.write_mode).to_string(),
                status: initial_status,
                command: command_template.clone(),
                command_output: None,
                command_output_truncated: false,
                error: None,
            },
            feature_settings.history_limit,
        ) {
            Ok(entry) => entry,
            Err(error) => {
                let error = format!("Text was saved, but history could not be updated: {error}");
                show_workflow_error(app, &feature_settings, &error);
                return Err(error);
            }
        };
        (output_path, history_entry)
    };
    let output_path_string = output_path.to_string_lossy().into_owned();

    let Some(command_template) = command_template else {
        let result = SendSelectedTextOperationResult {
            history_id: history_entry.id,
            operation_id,
            output_path: output_path_string,
            status: SendSelectedTextHistoryStatus::Completed,
            command_output: None,
            command_output_truncated: false,
        };
        let _ = app.emit("send-selected-text-completed", &result);
        return Ok(result);
    };

    let input_file = match create_command_input_file(&context) {
        Ok(input_file) => input_file,
        Err(error) => {
            return finish_command_failure(
                app,
                &history,
                &feature_settings,
                history_entry.id,
                error,
            )
        }
    };
    let command = match render_command_template(
        &command_template,
        &context,
        &output_path,
        &input_file.path,
        &preset,
        preset.allow_text_variable,
    ) {
        Ok(command) => command,
        Err(error) => {
            let _ = history.update_command_result(
                history_entry.id,
                SendSelectedTextHistoryStatus::CommandFailed,
                None,
                false,
                Some(error.clone()),
                feature_settings.history_limit,
            );
            show_workflow_error(app, &feature_settings, &error);
            return Err(error);
        }
    };
    let options = ResolvedExecutionOptions {
        silent: preset.command_silent,
        no_profile: preset.command_no_profile,
        use_pwsh: preset.command_use_pwsh,
        execution_policy: preset.command_execution_policy,
        working_directory: nonempty_string(&preset.command_working_directory),
    };
    let environment = command_environment(&context, &output_path, &input_file.path, &preset);

    if !preset.command_silent {
        match execute_powershell_command_with_environment_tracked(
            &command,
            &options,
            &environment,
            Box::new(move || drop(input_file)),
        ) {
            Ok(output) => {
                let (stored_output, truncated) = truncate_for_history(&output);
                let _ = history.update_command_result(
                    history_entry.id,
                    SendSelectedTextHistoryStatus::CommandStarted,
                    Some(stored_output.clone()),
                    truncated,
                    None,
                    feature_settings.history_limit,
                );
                let result = SendSelectedTextOperationResult {
                    history_id: history_entry.id,
                    operation_id,
                    output_path: output_path_string,
                    status: SendSelectedTextHistoryStatus::CommandStarted,
                    command_output: Some(stored_output),
                    command_output_truncated: truncated,
                };
                let _ = app.emit("send-selected-text-completed", &result);
                return Ok(result);
            }
            Err(error) => {
                return finish_command_failure(
                    app,
                    &history,
                    &feature_settings,
                    history_entry.id,
                    error,
                )
            }
        }
    }

    let captured = match execute_powershell_command_captured(&command, &options, &environment) {
        Ok(output) => output,
        Err(error) => {
            return finish_command_failure(
                app,
                &history,
                &feature_settings,
                history_entry.id,
                error,
            )
        }
    };
    let combined_output = combine_command_output(&captured.stdout, &captured.stderr);
    let (stored_output, output_truncated) = truncate_for_history(&combined_output);
    if !captured.succeeded() {
        let exit_code = captured
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let detail = if combined_output.trim().is_empty() {
            format!("Command exited with code {exit_code}")
        } else {
            format!(
                "Command exited with code {exit_code}: {}",
                stored_output.trim()
            )
        };
        let _ = history.update_command_result(
            history_entry.id,
            SendSelectedTextHistoryStatus::CommandFailed,
            Some(stored_output),
            output_truncated,
            Some(detail.clone()),
            feature_settings.history_limit,
        );
        show_workflow_error(app, &feature_settings, &detail);
        return Err(detail);
    }

    match history.update_command_result(
        history_entry.id,
        SendSelectedTextHistoryStatus::Completed,
        (!stored_output.is_empty()).then_some(stored_output.clone()),
        output_truncated,
        None,
        feature_settings.history_limit,
    ) {
        Ok(Some(_)) => {}
        Ok(None) => log::warn!(
            "Send Selected Text command completed after history entry {} was removed",
            history_entry.id
        ),
        Err(error) => {
            let error = format!("Command succeeded, but history could not be updated: {error}");
            show_workflow_error(app, &feature_settings, &error);
            return Err(error);
        }
    }
    let result = SendSelectedTextOperationResult {
        history_id: history_entry.id,
        operation_id,
        output_path: output_path_string,
        status: SendSelectedTextHistoryStatus::Completed,
        command_output: (!stored_output.is_empty()).then_some(stored_output),
        command_output_truncated: output_truncated,
    };
    let _ = app.emit("send-selected-text-completed", &result);
    Ok(result)
}

pub fn trim_json_for_preset(app: &AppHandle, preset_id: &str) -> Result<usize, String> {
    let settings = get_settings(app);
    let preset = settings
        .send_selected_text
        .presets
        .iter()
        .find(|preset| preset.id == preset_id)
        .ok_or_else(|| format!("Send Selected Text preset '{preset_id}' was not found"))?;
    if preset.format != SendSelectedTextFormat::Json {
        return Err("Only JSON presets can be trimmed.".to_string());
    }
    if preset.json_keep_last == 0 {
        return Err("Set 'Keep latest JSON entries' above zero before trimming.".to_string());
    }
    if preset.write_mode == SendSelectedTextWriteMode::CreateNew {
        return Err(
            "Create a new file produces a different JSON document on every run, so there is no single existing file to trim."
                .to_string(),
        );
    }
    validate_preset(preset)?;
    let now = Utc::now();
    let local_now = now.with_timezone(&Local);
    let context = TemplateContext {
        record_id: new_operation_id(),
        timestamp: now.to_rfc3339_opts(SecondsFormat::Millis, true),
        timestamp_local: local_now.to_rfc3339_opts(SecondsFormat::Millis, true),
        date: local_now.format("%Y-%m-%d").to_string(),
        time: local_now.format("%H-%M-%S-%3f").to_string(),
        preset_id: preset.id.clone(),
        preset_name: preset.name.clone(),
        text: String::new(),
    };
    let history = Arc::clone(&app.state::<Arc<SendSelectedTextHistoryManager>>());
    let request = WriteRequest {
        preset,
        context: &context,
        history: &history,
    };
    let _guard = OUTPUT_WRITE_LOCK
        .lock()
        .map_err(|_| "Send Selected Text output lock is unavailable".to_string())?;
    let last_json_path = history
        .last_output_path_for_preset(&preset.id, "json")
        .map_err(|error| format!("Failed to find the preset's last JSON file: {error}"))?
        .map(PathBuf::from);
    let path = match last_json_path {
        Some(path) if path.exists() => path,
        _ => resolve_output_path(&request, "json")?,
    };
    if !path.exists() {
        return Err(format!("JSON file does not exist: {}", path.display()));
    }
    let mut document = read_json_document(&path)?;
    let previous = document.entries.len();
    let keep = preset.json_keep_last as usize;
    if previous > keep {
        document.entries.drain(0..(previous - keep));
        write_json_atomic(&path, &document)?;
    }
    Ok(previous.saturating_sub(document.entries.len()))
}

fn capture_selected_text(
    app: &AppHandle,
    mode: SendSelectedTextCaptureMode,
) -> Result<String, String> {
    match mode {
        SendSelectedTextCaptureMode::ClipboardCopy => {
            crate::clipboard::capture_selection_text_copy(app)
        }
        SendSelectedTextCaptureMode::Accessibility => {
            crate::selection::read_selected_text_without_copying()
        }
        SendSelectedTextCaptureMode::Auto => {
            match crate::selection::read_selected_text_without_copying() {
                Ok(text) if !text.trim().is_empty() => Ok(text),
                _ => crate::clipboard::capture_selection_text_copy(app),
            }
        }
    }
}

fn enforce_text_limit(text: &str, preset: &SendSelectedTextPreset) -> Result<String, String> {
    let maximum = preset.max_chars.clamp(1, MAX_SELECTED_TEXT_CHARS) as usize;
    let count = text.chars().count();
    if count <= maximum {
        return Ok(text.to_string());
    }
    match preset.oversize_behavior {
        SendSelectedTextOversizeBehavior::Reject => Err(format!(
            "Selected text has {count} characters; preset '{}' allows at most {maximum}.",
            preset.name
        )),
        SendSelectedTextOversizeBehavior::Truncate => Ok(text.chars().take(maximum).collect()),
    }
}

fn validate_preset(preset: &SendSelectedTextPreset) -> Result<(), String> {
    if preset.id.trim().is_empty() {
        return Err("Preset ID is missing.".to_string());
    }
    if preset.name.trim().is_empty() {
        return Err("Preset name is required.".to_string());
    }
    if preset.destination_directory.trim().is_empty() {
        return Err(format!(
            "Choose an output folder for preset '{}'.",
            preset.name
        ));
    }
    if !Path::new(preset.destination_directory.trim()).is_absolute() {
        return Err("Output folder must be an absolute path.".to_string());
    }
    if preset.filename_template.trim().is_empty() {
        return Err("Filename template is required.".to_string());
    }
    if preset.format == SendSelectedTextFormat::Markdown
        && !preset.content_template.contains("{{text}}")
    {
        return Err("Markdown content template must contain {{text}}.".to_string());
    }
    if preset.format == SendSelectedTextFormat::Json
        && preset.write_mode == SendSelectedTextWriteMode::AppendLast
    {
        return Err("Append to last file is available only for Markdown presets.".to_string());
    }
    if preset.command_enabled && preset.command.trim().is_empty() {
        return Err("Post-save command is enabled, but the command is empty.".to_string());
    }
    Ok(())
}

fn resolve_output_path(request: &WriteRequest<'_>, extension: &str) -> Result<PathBuf, String> {
    if request.preset.write_mode == SendSelectedTextWriteMode::AppendLast {
        if let Some(previous) = request
            .history
            .last_output_path_for_preset(&request.preset.id, format_name(request.preset.format))
            .map_err(|error| format!("Failed to find the preset's last output file: {error}"))?
        {
            let path = PathBuf::from(previous);
            if path.exists() {
                return Ok(path);
            }
        }
    }

    let directory = PathBuf::from(request.preset.destination_directory.trim());
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Failed to create output folder {}: {error}",
            directory.display()
        )
    })?;
    let filename = render_filename_template(
        &request.preset.filename_template,
        request.context,
        extension,
    )?;
    let path = directory.join(filename);
    if request.preset.write_mode == SendSelectedTextWriteMode::CreateNew
        || request.preset.write_mode == SendSelectedTextWriteMode::AppendLast
    {
        return unused_path(path);
    }
    Ok(path)
}

fn render_filename_template(
    template: &str,
    context: &TemplateContext,
    extension: &str,
) -> Result<String, String> {
    let rendered = render_template_once(template.trim(), Some("Filename template"), |name| {
        let value = match name {
            "date" => &context.date,
            "time" => &context.time,
            "timestamp" => &context.timestamp,
            "timestamp_local" => &context.timestamp_local,
            "record_id" => &context.record_id,
            "preset_id" => &context.preset_id,
            "preset_name" => &context.preset_name,
            _ => return None,
        };
        Some(sanitize_filename_part(value))
    })?;
    let candidate = Path::new(&rendered);
    if candidate.components().count() != 1
        || !matches!(candidate.components().next(), Some(Component::Normal(_)))
    {
        return Err("Filename template must produce a filename, not a path.".to_string());
    }
    let mut filename = sanitize_filename_part(&rendered);
    let expected_suffix = format!(".{extension}");
    if !filename.to_ascii_lowercase().ends_with(&expected_suffix) {
        filename.push_str(&expected_suffix);
    }
    if filename
        .trim_matches(|character| character == '.' || character == ' ')
        .is_empty()
    {
        return Err("Filename template produced an empty filename.".to_string());
    }
    #[cfg(target_os = "windows")]
    if is_reserved_windows_filename(&filename) {
        return Err(format!(
            "Filename template produced the reserved Windows name '{filename}'."
        ));
    }
    Ok(filename)
}

#[cfg(target_os = "windows")]
fn is_reserved_windows_filename(filename: &str) -> bool {
    let device_name = filename
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(|character| character == ' ' || character == '.')
        .to_ascii_uppercase();
    matches!(
        device_name.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || device_name
        .strip_prefix("COM")
        .is_some_and(|suffix| matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        || device_name.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

fn sanitize_filename_part(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect::<String>();
    while sanitized.ends_with('.') || sanitized.ends_with(' ') {
        sanitized.pop();
    }
    if sanitized.is_empty() {
        "selected-text".to_string()
    } else {
        sanitized
    }
}

fn unused_path(path: PathBuf) -> Result<PathBuf, String> {
    if !path.exists() {
        return Ok(path);
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("selected-text");
    let extension = path.extension().and_then(|value| value.to_str());
    for suffix in 2..=10_000 {
        let filename = match extension {
            Some(extension) => format!("{stem}-{suffix}.{extension}"),
            None => format!("{stem}-{suffix}"),
        };
        let candidate = parent.join(filename);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("Could not find an unused output filename.".to_string())
}

fn render_content_template(template: &str, context: &TemplateContext) -> String {
    render_template_once(template, None, |name| match name {
        "text" => Some(context.text.clone()),
        "timestamp" => Some(context.timestamp.clone()),
        "timestamp_local" => Some(context.timestamp_local.clone()),
        "date" => Some(context.date.clone()),
        "time" => Some(context.time.clone()),
        "record_id" => Some(context.record_id.clone()),
        "preset_id" => Some(context.preset_id.clone()),
        "preset_name" => Some(context.preset_name.clone()),
        _ => None,
    })
    .expect("preserving unknown content-template variables cannot fail")
}

fn render_template_once(
    template: &str,
    reject_unknown_for: Option<&str>,
    mut resolve: impl FnMut(&str) -> Option<String>,
) -> Result<String, String> {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;

    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        let after_open = &remaining[start + 2..];
        let Some(end) = after_open.find("}}") else {
            if let Some(scope) = reject_unknown_for {
                return Err(format!("{scope} contains an incomplete template variable."));
            }
            output.push_str(&remaining[start..]);
            return Ok(output);
        };
        let name = &after_open[..end];
        if let Some(value) = resolve(name) {
            output.push_str(&value);
        } else if let Some(scope) = reject_unknown_for {
            return Err(format!(
                "{scope} contains unsupported variable '{{{{{name}}}}}'."
            ));
        } else {
            output.push_str(&remaining[start..start + end + 4]);
        }
        remaining = &after_open[end + 2..];
    }

    if remaining.contains("}}") {
        if let Some(scope) = reject_unknown_for {
            return Err(format!("{scope} contains an incomplete template variable."));
        }
    }
    output.push_str(remaining);
    Ok(output)
}

fn append_markdown(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create output folder {}: {error}",
                parent.display()
            )
        })?;
    }
    let needs_separator = path
        .metadata()
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("Failed to open {} for append: {error}", path.display()))?;
    if needs_separator {
        file.write_all(b"\n\n")
            .map_err(|error| format!("Failed to append to {}: {error}", path.display()))?;
    }
    file.write_all(content.as_bytes())
        .and_then(|_| file.flush())
        .map_err(|error| format!("Failed to append to {}: {error}", path.display()))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("Failed to create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

fn read_json_document(path: &Path) -> Result<JsonSelectedTextDocument, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let document: JsonSelectedTextDocument = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "{} is not a valid Send Selected Text JSON document: {error}",
            path.display()
        )
    })?;
    if document.version != 1 {
        return Err(format!(
            "{} uses unsupported JSON version {}.",
            path.display(),
            document.version
        ));
    }
    Ok(document)
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Failed to serialize JSON: {error}"))?;
    write_bytes_atomic(path, &bytes)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create output folder {}: {error}",
                parent.display()
            )
        })?;
    }
    let partial = path.with_extension(format!(
        "{}.aivorelay-partial",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("tmp")
    ));
    let result = (|| {
        let mut file = File::create(&partial)
            .map_err(|error| format!("Failed to create {}: {error}", partial.display()))?;
        file.write_all(bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("Failed to write {}: {error}", partial.display()))?;
        replace_file_atomic(&partial, path)
            .map_err(|error| format!("Failed to publish {}: {error}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

#[cfg(windows)]
fn replace_file_atomic(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|_| std::io::Error::last_os_error())
}

#[cfg(not(windows))]
fn replace_file_atomic(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

fn render_command_template(
    template: &str,
    context: &TemplateContext,
    output_path: &Path,
    input_path: &Path,
    preset: &SendSelectedTextPreset,
    allow_text_variable: bool,
) -> Result<String, String> {
    if template.contains("{{text}}") && !allow_text_variable {
        return Err(
            "This preset uses {{text}} in its command. Enable direct text insertion or use {{file_path}} instead."
                .to_string(),
        );
    }
    let output_dir = output_path.parent().unwrap_or_else(|| Path::new(""));
    let filename = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    render_template_once(template, Some("Command"), |name| {
        let value = match name {
            "file_path" => output_path.to_string_lossy().into_owned(),
            "input_file" => input_path.to_string_lossy().into_owned(),
            "output_dir" | "directory" => output_dir.to_string_lossy().into_owned(),
            "filename" => filename.to_string(),
            "text" => context.text.clone(),
            "record_id" => context.record_id.clone(),
            "timestamp" => context.timestamp.clone(),
            "timestamp_local" => context.timestamp_local.clone(),
            "date" => context.date.clone(),
            "time" => context.time.clone(),
            "preset_id" => context.preset_id.clone(),
            "preset_name" => context.preset_name.clone(),
            "format" => format_name(preset.format).to_string(),
            "write_mode" => write_mode_name(preset.write_mode).to_string(),
            "text_length" => context.text.chars().count().to_string(),
            "status" => "saved".to_string(),
            "error" => String::new(),
            "working_directory" => preset.command_working_directory.trim().to_string(),
            _ => return None,
        };
        Some(powershell_single_quoted(&value))
    })
}

fn powershell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn create_command_input_file(context: &TemplateContext) -> Result<CommandInputFile, String> {
    let directory = std::env::temp_dir()
        .join("AivoRelay")
        .join("send-selected-text-input");
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Failed to create command input folder {}: {error}",
            directory.display()
        )
    })?;
    cleanup_stale_command_input_files(&directory);
    let path = directory.join(format!(
        "{}.txt",
        sanitize_filename_part(&context.record_id)
    ));
    write_new_file(&path, context.text.as_bytes())?;
    ACTIVE_COMMAND_INPUT_FILES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path.clone());
    Ok(CommandInputFile { path })
}

fn cleanup_stale_command_input_files(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let cutoff =
        std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(24 * 60 * 60));
    let active_paths = ACTIVE_COMMAND_INPUT_FILES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("txt") {
            continue;
        }
        if active_paths.contains(&path) {
            continue;
        }
        let is_stale = cutoff.is_some_and(|cutoff| {
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .map(|modified| modified < cutoff)
                .unwrap_or(false)
        });
        if is_stale {
            let _ = fs::remove_file(path);
        }
    }
}

fn command_environment(
    context: &TemplateContext,
    output_path: &Path,
    input_path: &Path,
    preset: &SendSelectedTextPreset,
) -> Vec<(String, String)> {
    vec![
        (
            "AIVORELAY_SELECTED_TEXT_FILE".to_string(),
            output_path.to_string_lossy().into_owned(),
        ),
        (
            "AIVORELAY_SELECTED_TEXT_DIR".to_string(),
            output_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_string_lossy()
                .into_owned(),
        ),
        (
            "AIVORELAY_SELECTED_TEXT_INPUT_FILE".to_string(),
            input_path.to_string_lossy().into_owned(),
        ),
        (
            "AIVORELAY_SELECTED_TEXT_ID".to_string(),
            context.record_id.clone(),
        ),
        (
            "AIVORELAY_SELECTED_TEXT_TIMESTAMP".to_string(),
            context.timestamp.clone(),
        ),
        (
            "AIVORELAY_SELECTED_TEXT_PRESET_ID".to_string(),
            context.preset_id.clone(),
        ),
        (
            "AIVORELAY_SELECTED_TEXT_PRESET_NAME".to_string(),
            context.preset_name.clone(),
        ),
        (
            "AIVORELAY_SELECTED_TEXT_FORMAT".to_string(),
            format_name(preset.format).to_string(),
        ),
        (
            "AIVORELAY_SELECTED_TEXT_WRITE_MODE".to_string(),
            write_mode_name(preset.write_mode).to_string(),
        ),
    ]
}

fn combine_command_output(stdout: &str, stderr: &str) -> String {
    match (stdout.trim(), stderr.trim()) {
        ("", "") => String::new(),
        (stdout, "") => stdout.to_string(),
        ("", stderr) => stderr.to_string(),
        (stdout, stderr) => format!("STDOUT:\n{stdout}\n\nSTDERR:\n{stderr}"),
    }
}

fn truncate_for_history(value: &str) -> (String, bool) {
    let count = value.chars().count();
    if count <= MAX_COMMAND_OUTPUT_CHARS {
        return (value.to_string(), false);
    }
    let mut truncated = value
        .chars()
        .take(MAX_COMMAND_OUTPUT_CHARS)
        .collect::<String>();
    truncated.push_str("\n\n[Output truncated by AivoRelay]");
    (truncated, true)
}

fn record_and_report_failure(
    app: &AppHandle,
    history: &SendSelectedTextHistoryManager,
    settings: &crate::settings::SendSelectedTextSettings,
    preset: &SendSelectedTextPreset,
    operation_id: String,
    selected_text: String,
    error: String,
) -> Result<SendSelectedTextOperationResult, String> {
    log::error!(
        "Send Selected Text preset '{}' failed: {error}",
        preset.name
    );
    let _ = history.insert(
        NewSendSelectedTextHistoryEntry {
            operation_id,
            preset_id: preset.id.clone(),
            preset_name: preset.name.clone(),
            timestamp_ms: Utc::now().timestamp_millis(),
            selected_text,
            output_path: None,
            output_format: format_name(preset.format).to_string(),
            write_mode: write_mode_name(preset.write_mode).to_string(),
            status: SendSelectedTextHistoryStatus::Failed,
            command: preset.command_enabled.then(|| preset.command.clone()),
            command_output: None,
            command_output_truncated: false,
            error: Some(error.clone()),
        },
        settings.history_limit,
    );
    show_workflow_error(app, settings, &error);
    Err(error)
}

fn finish_command_failure(
    app: &AppHandle,
    history: &SendSelectedTextHistoryManager,
    settings: &crate::settings::SendSelectedTextSettings,
    history_id: i64,
    error: String,
) -> Result<SendSelectedTextOperationResult, String> {
    log::error!("Send Selected Text post-save command failed: {error}");
    let _ = history.update_command_result(
        history_id,
        SendSelectedTextHistoryStatus::CommandFailed,
        None,
        false,
        Some(error.clone()),
        settings.history_limit,
    );
    show_workflow_error(app, settings, &error);
    Err(error)
}

fn show_workflow_error(
    app: &AppHandle,
    settings: &crate::settings::SendSelectedTextSettings,
    error: &str,
) {
    crate::plus_overlay_state::show_send_selected_text_error_overlay(
        app,
        error,
        settings.error_overlay_auto_hide_ms,
    );
}

fn format_name(format: SendSelectedTextFormat) -> &'static str {
    match format {
        SendSelectedTextFormat::Markdown => "markdown",
        SendSelectedTextFormat::Json => "json",
    }
}

fn write_mode_name(mode: SendSelectedTextWriteMode) -> &'static str {
    match mode {
        SendSelectedTextWriteMode::CreateNew => "create_new",
        SendSelectedTextWriteMode::AppendLast => "append_last",
        SendSelectedTextWriteMode::AppendFile => "append_file",
        SendSelectedTextWriteMode::OverwriteFile => "overwrite_file",
    }
}

fn nonempty_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn new_operation_id() -> String {
    let sequence = OPERATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "sst_{}_{}_{}",
        Utc::now().timestamp_millis(),
        std::process::id(),
        sequence
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context(text: &str) -> TemplateContext {
        TemplateContext {
            record_id: "sst_test".to_string(),
            timestamp: "2026-08-23T12:34:56.000Z".to_string(),
            timestamp_local: "2026-08-23T15:34:56.000+03:00".to_string(),
            date: "2026-08-23".to_string(),
            time: "15-34-56-000".to_string(),
            preset_id: "preset_test".to_string(),
            preset_name: "Test Preset".to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn text_limit_counts_unicode_characters() {
        let mut preset = SendSelectedTextPreset::default();
        preset.max_chars = 2;
        preset.oversize_behavior = SendSelectedTextOversizeBehavior::Truncate;
        assert_eq!(enforce_text_limit("é😀x", &preset).unwrap(), "é😀");

        preset.oversize_behavior = SendSelectedTextOversizeBehavior::Reject;
        let error = enforce_text_limit("é😀x", &preset).unwrap_err();
        assert!(error.contains("3 characters"));
    }

    #[test]
    fn filename_templates_reject_paths_and_unknown_variables() {
        let context = test_context("text");
        assert_eq!(
            render_filename_template("note-{{date}}", &context, "md").unwrap(),
            "note-2026-08-23.md"
        );
        assert!(render_filename_template("../note", &context, "md").is_err());
        assert!(render_filename_template("{{unknown}}", &context, "md").is_err());
        assert_eq!(sanitize_filename_part("a:b?. "), "a_b_");
        #[cfg(target_os = "windows")]
        assert!(render_filename_template("CON", &context, "md").is_err());
    }

    #[test]
    fn command_templates_quote_values_and_gate_direct_text() {
        let context = test_context("it's selected");
        let preset = SendSelectedTextPreset::default();
        let output = Path::new("C:/notes/out.md");
        let input = Path::new("C:/temp/input.txt");

        assert!(render_command_template(
            "Write-Output {{text}}",
            &context,
            output,
            input,
            &preset,
            false,
        )
        .is_err());
        assert_eq!(
            render_command_template(
                "Write-Output {{text}}",
                &context,
                output,
                input,
                &preset,
                true,
            )
            .unwrap(),
            "Write-Output 'it''s selected'"
        );

        let path_with_placeholder = Path::new("C:/notes/{{text}}/out.md");
        let rendered = render_command_template(
            "Write-Output {{file_path}}",
            &context,
            path_with_placeholder,
            input,
            &preset,
            false,
        )
        .unwrap();
        assert!(rendered.contains("{{text}}"));
        assert!(!rendered.contains("it's selected"));
    }

    #[test]
    fn json_serialization_escapes_and_round_trips_selected_text() {
        let text = "quote: \"hello\"; slash: \\; newline:\nnext";
        let document = JsonSelectedTextDocument {
            version: 1,
            entries: vec![JsonSelectedTextEntry {
                id: "entry_1".to_string(),
                timestamp: "2026-08-23T12:34:56Z".to_string(),
                preset_id: "preset_1".to_string(),
                preset_name: "Inbox".to_string(),
                text: text.to_string(),
            }],
        };
        let serialized = serde_json::to_string(&document).unwrap();
        assert!(serialized.contains("\\\"hello\\\""));
        assert!(serialized.contains("\\\\"));
        assert!(serialized.contains("\\n"));
        let restored: JsonSelectedTextDocument = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.entries[0].text, text);
    }
}
