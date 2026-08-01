//! Headless lifecycle CLI for the optional app-managed local TTS provider.

use crate::cli::{CliArgs, CliCommand, CliLocalTtsEngine, TtsLocalCommand};
use crate::managers::local_tts::LocalTtsKind;
use crate::managers::tts::TtsManager;
use crate::settings::{TtsOutputFormat, TtsProvider};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

const EXIT_RUNTIME: i32 = 1;
const EXIT_USAGE: i32 = 2;
const EXIT_NOT_CONFIRMED: i32 = 3;

pub fn is_local_tts_requested(args: &CliArgs) -> bool {
    matches!(args.command, Some(CliCommand::TtsLocal(_)))
}

pub fn run_local_tts(app: &AppHandle, args: &CliArgs) -> i32 {
    match run_local_tts_inner(app, args) {
        Ok(value) => {
            if args.json {
                println!("{value}");
            }
            0
        }
        Err((code, message)) => {
            if args.json {
                println!(
                    "{}",
                    json!({
                        "ok": false,
                        "operation": "tts_local",
                        "error": message,
                        "exit_code": code,
                    })
                );
            } else {
                eprintln!("error: {message}");
            }
            code
        }
    }
}

fn run_local_tts_inner(
    app: &AppHandle,
    args: &CliArgs,
) -> Result<serde_json::Value, (i32, String)> {
    validate_root_scope(args)?;
    if app.try_state::<Arc<TtsManager>>().is_none() {
        let manager = TtsManager::new(app)
            .map_err(|error| (EXIT_RUNTIME, format!("Failed to initialize TTS: {error}")))?;
        app.manage(manager);
    }
    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    let (engine, command) = match args.command.as_ref() {
        Some(CliCommand::TtsLocal(local)) => (local.engine, &local.command),
        _ => return Err((EXIT_USAGE, "Missing tts-local command".to_string())),
    };
    let kind = local_kind(engine);
    let engine_name = local_engine_name(engine);
    match command {
        TtsLocalCommand::Status => {
            let status = manager.local_tts_status(kind);
            if !args.json {
                println!(
                    "{engine_name}: {}\nModel: {}@{}\nRuntime: {}\nSource: {} ({})\nInstall path: {}\nDisk: {}\nLicense: {}\nLicense URL: {}\nLocal license: {}",
                    if status.installed {
                        "ready"
                    } else {
                        status.phase.as_str()
                    },
                    status.model_repository,
                    status.model_revision,
                    if status.runtime_profile.is_empty() {
                        "not installed"
                    } else {
                        status.runtime_profile.as_str()
                    },
                    status.model_author,
                    status.model_source_url,
                    status.install_root,
                    if status.installed {
                        format!(
                            "~{} currently used",
                            human_bytes(status.installed_size_bytes)
                        )
                    } else if status.installed_size_bytes > 0 {
                        format!(
                            "~{} existing; allow up to {} for repair or installation",
                            human_bytes(status.installed_size_bytes),
                            human_bytes(status.estimated_install_bytes)
                        )
                    } else {
                        format!(
                            "allow up to {} for installation",
                            human_bytes(status.estimated_install_bytes)
                        )
                    },
                    status.model_license_name,
                    status.model_license_url,
                    if status.model_license_available {
                        status.model_license_path.as_str()
                    } else {
                        "available after installation"
                    },
                );
            }
            Ok(json!({ "ok": true, "status": status }))
        }
        TtsLocalCommand::Install(options) => {
            if !options.yes {
                let status = manager.local_tts_status(kind);
                return Err((
                    EXIT_NOT_CONFIRMED,
                    format!(
                        "{engine_name} installation downloads a managed runtime and model from {}. Author: {}. Destination: {}. Allow up to {}. License: {} ({}). Review and trust the source, understand the risks, then re-run with --yes.",
                        status.model_source_url,
                        status.model_author,
                        status.install_root,
                        human_bytes(status.estimated_install_bytes),
                        status.model_license_name,
                        status.model_license_url,
                    ),
                ));
            }
            if !args.json {
                eprintln!("Installing the app-managed {engine_name} model and runtime…");
            }
            let reserve = crate::settings::get_settings(app).tts.disk_reserve_mb;
            let status = tauri::async_runtime::block_on(manager.install_local_tts(kind, reserve))
                .map_err(|error| (EXIT_RUNTIME, error.to_string()))?;
            if !args.json {
                println!(
                    "{engine_name} is ready ({} runtime).",
                    status.runtime_profile
                );
            }
            Ok(json!({ "ok": true, "status": status }))
        }
        TtsLocalCommand::Delete(options) => {
            if !options.yes {
                return Err((
                    EXIT_NOT_CONFIRMED,
                    "Deletion removes the local model and runtime. Re-run with --yes.".to_string(),
                ));
            }
            tauri::async_runtime::block_on(manager.delete_local_tts(kind))
                .map_err(|error| (EXIT_RUNTIME, error.to_string()))?;
            if !args.json {
                println!("{engine_name} model and runtime deleted.");
            }
            Ok(json!({ "ok": true }))
        }
        TtsLocalCommand::Test(options) => {
            if !manager.local_tts_status(kind).installed {
                return Err((
                    EXIT_RUNTIME,
                    format!(
                        "{engine_name} is not installed. Run `aivorelay tts-local --engine {} install --yes`.",
                        local_engine_cli_name(engine)
                    ),
                ));
            }
            let output = absolute_path(&options.output).map_err(|error| (EXIT_USAGE, error))?;
            if output.exists() {
                return Err((
                    EXIT_USAGE,
                    format!("Output already exists: {}", output.display()),
                ));
            }
            let format = match output
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("mp3") => TtsOutputFormat::Mp3,
                Some("wav") => TtsOutputFormat::Wav,
                _ => {
                    return Err((
                        EXIT_USAGE,
                        "Local TTS test output must end in .mp3 or .wav".to_string(),
                    ))
                }
            };
            let text = options.text.clone().unwrap_or_else(|| match engine {
                CliLocalTtsEngine::Qwen => {
                    "AivoRelay local speech is ready. Локальный синтез речи AivoRelay работает."
                        .to_string()
                }
                CliLocalTtsEngine::Kokoro => {
                    "AivoRelay local Kokoro speech is ready and running offline.".to_string()
                }
            });
            if text.trim().is_empty() {
                return Err((EXIT_USAGE, "--text must not be empty".to_string()));
            }
            let mut settings = crate::settings::get_settings(app).tts;
            settings.enabled = true;
            match engine {
                CliLocalTtsEngine::Qwen => {
                    settings.provider = TtsProvider::LocalQwen;
                    settings.local_qwen_voice =
                        options.voice.clone().unwrap_or_else(|| "Ryan".to_string());
                    settings.local_qwen_language = options
                        .language
                        .clone()
                        .unwrap_or_else(|| "Auto".to_string());
                }
                CliLocalTtsEngine::Kokoro => {
                    settings.provider = TtsProvider::LocalKokoro;
                    settings.local_kokoro_voice = options
                        .voice
                        .clone()
                        .unwrap_or_else(|| "af_maple".to_string());
                    settings.local_kokoro_language = options
                        .language
                        .clone()
                        .unwrap_or_else(|| "English".to_string());
                }
            }
            settings.output_format = format;
            settings.mp3_bitrate_kbps = 256;
            // Keep the diagnostic command fast and guarantee worker reuse for
            // moderately sized test passages without changing saved settings.
            settings.file_target_chars = 220;
            settings.interactive_history_enabled = false;
            settings.file_history_enabled = false;
            TtsManager::validate_settings(&settings)
                .map_err(|error| (EXIT_USAGE, error.to_string()))?;

            let input = std::env::temp_dir().join(format!(
                "aivorelay-local-tts-test-{}-{}.txt",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ));
            fs::write(&input, text.as_bytes()).map_err(|error| {
                (
                    EXIT_RUNTIME,
                    format!("Failed to create local TTS test input: {error}"),
                )
            })?;
            if !args.json {
                let (voice, language) = match engine {
                    CliLocalTtsEngine::Qwen => (
                        settings.local_qwen_voice.as_str(),
                        settings.local_qwen_language.as_str(),
                    ),
                    CliLocalTtsEngine::Kokoro => (
                        settings.local_kokoro_voice.as_str(),
                        settings.local_kokoro_language.as_str(),
                    ),
                };
                eprintln!(
                    "Synthesizing {engine_name} test with voice {voice} and language {language}…"
                );
            }
            let result = tauri::async_runtime::block_on(
                manager.convert_text_file(&input, &output, &settings),
            );
            let _ = fs::remove_file(&input);
            let result = result.map_err(|error| (EXIT_RUNTIME, error.to_string()))?;
            if !args.json {
                println!("Created {}", result.output_path.display());
            }
            Ok(json!({
                "ok": true,
                "output": result.output_path,
                "format": result.output_format,
                "mp3_bitrate_kbps": result.mp3_bitrate_kbps,
                "chunks": result.chunk_count,
                "resumed_chunks": result.resumed_chunks,
            }))
        }
    }
}

fn human_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", bytes as f64 / GIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}

fn local_kind(engine: CliLocalTtsEngine) -> LocalTtsKind {
    match engine {
        CliLocalTtsEngine::Qwen => LocalTtsKind::Qwen,
        CliLocalTtsEngine::Kokoro => LocalTtsKind::Kokoro,
    }
}

fn local_engine_name(engine: CliLocalTtsEngine) -> &'static str {
    match engine {
        CliLocalTtsEngine::Qwen => "Local Qwen3-TTS",
        CliLocalTtsEngine::Kokoro => "Local Kokoro-82M",
    }
}

fn local_engine_cli_name(engine: CliLocalTtsEngine) -> &'static str {
    match engine {
        CliLocalTtsEngine::Qwen => "qwen",
        CliLocalTtsEngine::Kokoro => "kokoro",
    }
}

fn absolute_path(path: &std::path::Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|error| format!("Failed to resolve output path: {error}"))
    }
}

fn validate_root_scope(args: &CliArgs) -> Result<(), (i32, String)> {
    if args.toggle_transcription
        || args.toggle_post_process
        || args.cancel
        || args.transcribe_file.is_some()
        || !args.convert_file.is_empty()
        || args.output.is_some()
        || args.has_tts_file_conversion_args()
        || args.model.is_some()
        || args.device_index.is_some()
        || args.list_devices
        || args.repeat.is_some()
    {
        return Err((
            EXIT_USAGE,
            "tts-local cannot be combined with another AivoRelay operation".to_string(),
        ));
    }
    Ok(())
}
