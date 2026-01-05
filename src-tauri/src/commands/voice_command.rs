//! Voice Command Tauri commands
//!
//! Commands for executing voice-triggered scripts after user confirmation.

use log::{debug, error, info};
use std::process::Command;

/// Executes a PowerShell command after user confirmation.
/// Returns the output on success or an error message on failure.
#[tauri::command]
#[specta::specta]
#[cfg(target_os = "windows")]
pub fn execute_voice_command(command: String) -> Result<String, String> {
    if command.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    info!("Executing voice command: {}", command);

    // Execute via PowerShell
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &command])
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

/// Non-Windows stub
#[tauri::command]
#[specta::specta]
#[cfg(not(target_os = "windows"))]
pub fn execute_voice_command(_command: String) -> Result<String, String> {
    Err("Voice commands are only supported on Windows".to_string())
}
