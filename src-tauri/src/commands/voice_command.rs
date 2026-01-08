//! Voice Command Tauri commands
//!
//! Commands for executing voice-triggered scripts after user confirmation.

use log::{debug, error, info};
use std::process::Command;

/// Executes a PowerShell command after user confirmation.
///
/// Parameters:
/// - `command`: The PowerShell command to execute
/// - `ps_args`: PowerShell arguments (e.g., "-NoProfile -NonInteractive")
/// - `keep_window_open`: If true, opens a visible terminal window instead of silent execution
/// - `use_windows_terminal`: If true, uses Windows Terminal (wt); otherwise uses classic PowerShell window
///
/// Returns the output on success or an error message on failure.
/// When `keep_window_open` is true, returns success immediately (no output capture).
#[tauri::command]
#[specta::specta]
#[cfg(target_os = "windows")]
pub fn execute_voice_command(
    command: String,
    ps_args: String,
    keep_window_open: bool,
    use_windows_terminal: bool,
) -> Result<String, String> {
    if command.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    info!("Executing voice command: {}", command);
    debug!(
        "Options: ps_args='{}', keep_window_open={}, use_windows_terminal={}",
        ps_args, keep_window_open, use_windows_terminal
    );

    if keep_window_open {
        // Open a visible terminal window that stays open after command completes
        // Use -NoExit to keep the window open
        let ps_args_with_noexit = format!("{} -NoExit", ps_args);

        if use_windows_terminal {
            // Use Windows Terminal (wt) with PowerShell
            // wt new-tab powershell <args> -Command "<command>"
            let wt_args = format!(
                "new-tab powershell {} -Command \"{}\"",
                ps_args_with_noexit, command
            );

            info!("Opening Windows Terminal: wt {}", wt_args);

            Command::new("wt")
                .args(wt_args.split_whitespace().take(2).collect::<Vec<_>>())
                .arg("powershell")
                .args(ps_args_with_noexit.split_whitespace())
                .arg("-Command")
                .arg(&command)
                .spawn()
                .map_err(|e| format!("Failed to open Windows Terminal: {}", e))?;
        } else {
            // Use classic PowerShell window via Start-Process
            // This opens a separate powershell.exe window
            let start_args = format!(
                "Start-Process powershell -ArgumentList '{} -Command \"{}\"'",
                ps_args_with_noexit,
                command.replace("\"", "`\"")
            );

            info!("Opening PowerShell window: {}", start_args);

            Command::new("powershell")
                .args(["-NoProfile", "-Command", &start_args])
                .spawn()
                .map_err(|e| format!("Failed to open PowerShell window: {}", e))?;
        }

        Ok("Command opened in terminal window".to_string())
    } else {
        // Silent execution with output capture (original behavior)
        // Parse ps_args string into individual arguments
        let args: Vec<&str> = ps_args.split_whitespace().collect();

        let mut cmd = Command::new("powershell");
        for arg in &args {
            cmd.arg(arg);
        }
        cmd.arg("-Command").arg(&command);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to spawn PowerShell: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            debug!("Command executed successfully. Output: {}", stdout.trim());
            Ok(stdout)
        } else {
            error!("Command failed. Stderr: {}", stderr);
            Err(format!("Command failed: {}", stderr.trim()))
        }
    }
}

/// Non-Windows stub
#[tauri::command]
#[specta::specta]
#[cfg(not(target_os = "windows"))]
pub fn execute_voice_command(
    _command: String,
    _ps_args: String,
    _keep_window_open: bool,
    _use_windows_terminal: bool,
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
    use crate::actions::{find_matching_command, generate_command_with_llm, CommandConfirmPayload};
    use crate::settings::get_settings;
    use log::debug;

    if mock_text.trim().is_empty() {
        return Err("Mock text is empty".to_string());
    }

    info!("Testing voice command with mock text: '{}'", mock_text);

    let settings = get_settings(&app);

    // Step 1: Try to match against predefined commands
    if let Some((matched_cmd, score)) = find_matching_command(
        &mock_text,
        &settings.voice_commands,
        settings.voice_command_default_threshold,
    ) {
        debug!(
            "Mock test matched: '{}' -> '{}' (score: {:.2})",
            matched_cmd.trigger_phrase, matched_cmd.script, score
        );

        // Show confirmation overlay
        crate::overlay::show_command_confirm_overlay(
            &app,
            CommandConfirmPayload {
                command: matched_cmd.script.clone(),
                spoken_text: mock_text.clone(),
                from_llm: false,
                ps_args: settings.voice_command_ps_args.clone(),
                keep_window_open: settings.voice_command_keep_window_open,
                use_windows_terminal: settings.voice_command_use_windows_terminal,
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

                // Show confirmation overlay
                crate::overlay::show_command_confirm_overlay(
                    &app,
                    CommandConfirmPayload {
                        command: suggested_command.clone(),
                        spoken_text: mock_text,
                        from_llm: true,
                        ps_args: settings.voice_command_ps_args.clone(),
                        keep_window_open: settings.voice_command_keep_window_open,
                        use_windows_terminal: settings.voice_command_use_windows_terminal,
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
