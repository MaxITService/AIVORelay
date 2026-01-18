//! Voice Command Tauri commands
//!
//! Commands for executing voice-triggered scripts after user confirmation.

use log::{debug, error, info};
use std::process::Command;

/// Executes a command using a template after user confirmation.
///
/// Parameters:
/// - `command`: The command to execute (will replace ${command} in template)
/// - `template`: The execution template (e.g., "powershell -NonInteractive -Command \"${command}\"")
/// - `keep_window_open`: If true, uses Windows Terminal to open a visible window
///
/// Returns the output on success or an error message on failure.
/// When `keep_window_open` is true, returns success immediately (no output capture).
#[tauri::command]
#[specta::specta]
#[cfg(target_os = "windows")]
pub fn execute_voice_command(
    command: String,
    template: String,
    keep_window_open: bool,
) -> Result<String, String> {
    if command.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    // Build the full command line by replacing ${command} in template
    let full_command_line = template.replace("${command}", &command);

    info!("Executing voice command: {}", command);
    debug!("Full command line: {}", full_command_line);
    debug!("Options: keep_window_open={}", keep_window_open);

    if keep_window_open {
        // Open in Windows Terminal with visible window
        let wt_path = find_windows_terminal()?;

        info!(
            "Opening Windows Terminal: {} new-tab -- {}",
            wt_path, full_command_line
        );

        // Parse the template to extract shell and args
        // wt new-tab -- <full_command_line>
        Command::new(&wt_path)
            .arg("new-tab")
            .arg("--")
            .arg("cmd")
            .arg("/k")
            .arg(&full_command_line)
            .spawn()
            .map_err(|e| format!("Failed to open Windows Terminal: {}", e))?;

        Ok("Command opened in terminal window".to_string())
    } else {
        // Silent execution using cmd /c to run the full command line
        let output = Command::new("cmd")
            .arg("/c")
            .arg(&full_command_line)
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;

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

/// Find Windows Terminal (wt.exe) by checking multiple locations.
/// Returns the path to wt.exe if found, or an error with helpful message.
#[cfg(target_os = "windows")]
fn find_windows_terminal() -> Result<String, String> {
    // First try: just "wt" (relies on PATH)
    if let Ok(output) = Command::new("where").arg("wt.exe").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = path.lines().next() {
                let trimmed = first_line.trim();
                if !trimmed.is_empty() {
                    debug!("Found wt.exe via PATH: {}", trimmed);
                    return Ok(trimmed.to_string());
                }
            }
        }
    }

    // Second try: WindowsApps in LOCALAPPDATA (user app execution alias)
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let windows_apps_path = format!("{}\\Microsoft\\WindowsApps\\wt.exe", local_app_data);
        if std::path::Path::new(&windows_apps_path).exists() {
            debug!("Found wt.exe in WindowsApps: {}", windows_apps_path);
            return Ok(windows_apps_path);
        }
    }

    // Third try: Check common Program Files locations (for non-Store installs)
    let program_files_paths = [
        "C:\\Program Files\\Windows Terminal\\wt.exe",
        "C:\\Program Files (x86)\\Windows Terminal\\wt.exe",
    ];
    for path in program_files_paths {
        if std::path::Path::new(path).exists() {
            debug!("Found wt.exe in Program Files: {}", path);
            return Ok(path.to_string());
        }
    }

    Err(
        "Windows Terminal (wt.exe) not found. Please ensure Windows Terminal is installed:\n\
         1. Check Start Menu for 'Windows Terminal'\n\
         2. Install from Microsoft Store"
            .to_string(),
    )
}

/// Non-Windows stub
#[tauri::command]
#[specta::specta]
#[cfg(not(target_os = "windows"))]
pub fn execute_voice_command(
    _command: String,
    _template: String,
    _keep_window_open: bool,
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
    use crate::actions::{find_matching_command, generate_command_with_llm, CommandConfirmPayload, FuzzyMatchConfig};
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

        // Show confirmation overlay
        crate::overlay::show_command_confirm_overlay(
            &app,
            CommandConfirmPayload {
                command: matched_cmd.script.clone(),
                spoken_text: mock_text.clone(),
                from_llm: false,
                template: settings.voice_command_template.clone(),
                keep_window_open: settings.voice_command_keep_window_open,
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

                // Show confirmation overlay
                crate::overlay::show_command_confirm_overlay(
                    &app,
                    CommandConfirmPayload {
                        command: suggested_command.clone(),
                        spoken_text: mock_text,
                        from_llm: true,
                        template: settings.voice_command_template.clone(),
                        keep_window_open: settings.voice_command_keep_window_open,
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
