//! Voice Command Tauri commands
//!
//! Commands for executing voice-triggered PowerShell scripts.
//! Uses direct PowerShell invocation with configurable execution options.

use log::{debug, info};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::Command;

use crate::settings::{ExecutionPolicy, ResolvedExecutionOptions};

#[cfg(target_os = "windows")]
const CREATE_NEW_CONSOLE: u32 = 0x00000010;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Executes a PowerShell command with the given execution options.
///
/// Parameters:
/// - `script`: The PowerShell script/command to execute
/// - `options`: Resolved execution options (silent, no_profile, use_pwsh, etc.)
///
/// Returns the output on success or an error message on failure.
#[tauri::command]
#[specta::specta]
#[cfg(target_os = "windows")]
pub fn execute_voice_command(
    script: String,
    silent: bool,
    no_profile: bool,
    use_pwsh: bool,
    execution_policy: Option<String>,
    working_directory: Option<String>,
) -> Result<String, String> {
    if script.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    // Parse execution policy from string
    let policy = execution_policy.as_deref().and_then(|p| match p {
        "bypass" => Some(ExecutionPolicy::Bypass),
        "unrestricted" => Some(ExecutionPolicy::Unrestricted),
        "remote_signed" => Some(ExecutionPolicy::RemoteSigned),
        "default" | "" => None,
        _ => None,
    });

    let options = ResolvedExecutionOptions {
        silent,
        no_profile,
        use_pwsh,
        execution_policy: policy.unwrap_or(ExecutionPolicy::Default),
        working_directory,
    };

    execute_powershell_command(&script, &options)
}

/// Internal function to execute PowerShell commands.
#[cfg(target_os = "windows")]
fn execute_powershell_command(
    script: &str,
    options: &ResolvedExecutionOptions,
) -> Result<String, String> {
    let shell = if options.use_pwsh { "pwsh" } else { "powershell" };

    info!(
        "Executing voice command via {}: {} (silent={}, no_profile={}, policy={:?})",
        shell, script, options.silent, options.no_profile, options.execution_policy
    );

    let mut cmd = Command::new(shell);

    // Add -NoProfile flag if requested
    if options.no_profile {
        cmd.arg("-NoProfile");
    }

    // Add -NonInteractive for silent execution
    if options.silent {
        cmd.arg("-NonInteractive");
    }

    // Add execution policy if not default
    match options.execution_policy {
        ExecutionPolicy::Default => {}
        ExecutionPolicy::Bypass => {
            cmd.args(["-ExecutionPolicy", "Bypass"]);
        }
        ExecutionPolicy::Unrestricted => {
            cmd.args(["-ExecutionPolicy", "Unrestricted"]);
        }
        ExecutionPolicy::RemoteSigned => {
            cmd.args(["-ExecutionPolicy", "RemoteSigned"]);
        }
    }

    // Set working directory if specified
    if let Some(ref dir) = options.working_directory {
        if !dir.trim().is_empty() {
            cmd.current_dir(dir);
            debug!("Working directory set to: {}", dir);
        }
    }

    // Add the command
    cmd.args(["-Command", script]);

    if options.silent {
        // Silent execution: hide window, fire-and-forget (non-blocking)
        cmd.creation_flags(CREATE_NO_WINDOW);

        cmd.spawn()
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        Ok("Command started in background".to_string())
    } else {
        // Windowed execution: show console, add -NoExit to keep window open
        debug!("Opening {} window with -NoExit for: {}", shell, script);

        // Rebuild command with -NoExit before -Command
        let mut windowed_cmd = Command::new(shell);

        if options.no_profile {
            windowed_cmd.arg("-NoProfile");
        }

        match options.execution_policy {
            ExecutionPolicy::Default => {}
            ExecutionPolicy::Bypass => {
                windowed_cmd.args(["-ExecutionPolicy", "Bypass"]);
            }
            ExecutionPolicy::Unrestricted => {
                windowed_cmd.args(["-ExecutionPolicy", "Unrestricted"]);
            }
            ExecutionPolicy::RemoteSigned => {
                windowed_cmd.args(["-ExecutionPolicy", "RemoteSigned"]);
            }
        }

        if let Some(ref dir) = options.working_directory {
            if !dir.trim().is_empty() {
                windowed_cmd.current_dir(dir);
            }
        }

        // Add -NoExit before -Command to keep window open
        windowed_cmd.args(["-NoExit", "-Command", script]);
        windowed_cmd.creation_flags(CREATE_NEW_CONSOLE);

        windowed_cmd
            .spawn()
            .map_err(|e| format!("Failed to open {} window: {}", shell, e))?;

        Ok("Command opened in PowerShell window".to_string())
    }
}

/// Non-Windows stub
#[tauri::command]
#[specta::specta]
#[cfg(not(target_os = "windows"))]
pub fn execute_voice_command(
    _script: String,
    _silent: bool,
    _no_profile: bool,
    _use_pwsh: bool,
    _execution_policy: Option<String>,
    _working_directory: Option<String>,
) -> Result<String, String> {
    Err("Voice commands are only supported on Windows".to_string())
}

/// Tests voice command matching with mock text (simulates STT output).
/// Runs the same matching logic as if the text was spoken.
#[tauri::command]
#[specta::specta]
#[cfg(target_os = "windows")]
pub async fn test_voice_command_mock(
    app: tauri::AppHandle,
    mock_text: String,
) -> Result<String, String> {
    use crate::actions::{
        find_matching_command, generate_command_with_llm, CommandConfirmPayload, FuzzyMatchConfig,
    };
    use crate::settings::get_settings;
    use log::debug;

    if mock_text.trim().is_empty() {
        return Err("Mock text is empty".to_string());
    }

    info!("Testing voice command with mock text: '{}'", mock_text);

    let settings = get_settings(&app);
    let fuzzy_config = FuzzyMatchConfig::from_settings(&settings);

    // Step 1: Try to match against predefined commands
    if let Some((matched_cmd, score)) = find_matching_command(
        &mock_text,
        &settings.voice_commands,
        settings.voice_command_default_threshold,
        &fuzzy_config,
    ) {
        debug!(
            "Mock test matched: '{}' -> '{}' (score: {:.2})",
            matched_cmd.trigger_phrase, matched_cmd.script, score
        );

        // Resolve execution options for this command
        let resolved = matched_cmd.resolve_execution_options(&settings.voice_command_defaults);

        // Show confirmation overlay with resolved options
        crate::overlay::show_command_confirm_overlay(
            &app,
            CommandConfirmPayload {
                command: matched_cmd.script.clone(),
                spoken_text: mock_text.clone(),
                from_llm: false,
                silent: resolved.silent,
                no_profile: resolved.no_profile,
                use_pwsh: resolved.use_pwsh,
                execution_policy: format_execution_policy(resolved.execution_policy),
                working_directory: resolved.working_directory,
                auto_run: settings.voice_command_auto_run,
                auto_run_seconds: settings.voice_command_auto_run_seconds,
            },
        );

        return Ok(format!(
            "Matched predefined command: '{}' (score: {:.0}%)",
            matched_cmd.name,
            score * 100.0
        ));
    }

    // Step 2: No predefined match - try LLM fallback if enabled
    if settings.voice_command_llm_fallback {
        debug!(
            "No predefined match, using LLM fallback for mock text: '{}'",
            mock_text
        );

        match generate_command_with_llm(&app, &mock_text).await {
            Ok(suggested_command) => {
                debug!("LLM suggested command: '{}'", suggested_command);

                // LLM fallback uses global defaults
                let resolved = settings.voice_command_defaults.to_resolved_options();

                // Show confirmation overlay
                crate::overlay::show_command_confirm_overlay(
                    &app,
                    CommandConfirmPayload {
                        command: suggested_command.clone(),
                        spoken_text: mock_text,
                        from_llm: true,
                        silent: resolved.silent,
                        no_profile: resolved.no_profile,
                        use_pwsh: resolved.use_pwsh,
                        execution_policy: format_execution_policy(resolved.execution_policy),
                        working_directory: resolved.working_directory,
                        auto_run: false, // Never auto-run LLM-generated commands
                        auto_run_seconds: 0,
                    },
                );

                return Ok(format!("LLM generated command: '{}'", suggested_command));
            }
            Err(e) => {
                return Err(format!("LLM fallback failed: {}", e));
            }
        }
    }

    Err(format!(
        "No matching command found for: '{}' (LLM fallback disabled)",
        mock_text
    ))
}

/// Format ExecutionPolicy for frontend display.
#[cfg(target_os = "windows")]
fn format_execution_policy(policy: ExecutionPolicy) -> Option<String> {
    match policy {
        ExecutionPolicy::Default => None,
        ExecutionPolicy::Bypass => Some("bypass".to_string()),
        ExecutionPolicy::Unrestricted => Some("unrestricted".to_string()),
        ExecutionPolicy::RemoteSigned => Some("remote_signed".to_string()),
    }
}

/// Non-Windows stub for mock testing
#[tauri::command]
#[specta::specta]
#[cfg(not(target_os = "windows"))]
pub async fn test_voice_command_mock(
    _app: tauri::AppHandle,
    _mock_text: String,
) -> Result<String, String> {
    Err("Voice commands are only supported on Windows".to_string())
}
