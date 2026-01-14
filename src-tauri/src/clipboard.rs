use crate::input::{self, EnigoState};
use crate::settings::{get_settings, ClipboardHandling, PasteMethod};
use enigo::Enigo;
use log::{info, warn};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[cfg(target_os = "linux")]
use crate::utils::is_wayland;
#[cfg(target_os = "linux")]
use std::process::Command;

/// Windows-only: Advanced clipboard backup/restore that preserves all formats
#[cfg(target_os = "windows")]
mod win_clipboard {
    use log::{debug, warn};
    use std::ptr;
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GHND};

    /// Represents a single clipboard format and its data
    pub struct ClipboardEntry {
        pub format: u32,
        pub data: Vec<u8>,
    }

    /// Backup all clipboard formats
    pub fn backup_all_formats() -> Result<Vec<ClipboardEntry>, String> {
        let mut entries = Vec::new();

        unsafe {
            // Open clipboard (None = current task)
            if OpenClipboard(None).is_err() {
                return Err("Failed to open clipboard for backup".into());
            }

            // Enumerate all formats
            let mut format = EnumClipboardFormats(0);
            while format != 0 {
                if let Some(entry) = read_format(format) {
                    debug!(
                        "Backed up clipboard format {}: {} bytes",
                        format,
                        entry.data.len()
                    );
                    entries.push(entry);
                }
                format = EnumClipboardFormats(format);
            }

            let _ = CloseClipboard();
        }

        debug!("Backed up {} clipboard formats", entries.len());
        Ok(entries)
    }

    /// Read data for a specific clipboard format
    unsafe fn read_format(format: u32) -> Option<ClipboardEntry> {
        let handle = GetClipboardData(format).ok()?;
        if handle.0.is_null() {
            return None;
        }

        // Convert HANDLE to HGLOBAL for memory operations
        let hglobal = HGLOBAL(handle.0);

        let size = GlobalSize(hglobal);
        if size == 0 {
            return None;
        }

        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            return None;
        }

        let data = std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
        let _ = GlobalUnlock(hglobal);

        Some(ClipboardEntry { format, data })
    }

    /// Restore all backed-up clipboard formats
    pub fn restore_all_formats(entries: Vec<ClipboardEntry>) -> Result<(), String> {
        if entries.is_empty() {
            debug!("No clipboard entries to restore");
            return Ok(());
        }

        unsafe {
            // Open clipboard (None = current task)
            if OpenClipboard(None).is_err() {
                return Err("Failed to open clipboard for restore".into());
            }

            // Clear existing content
            if EmptyClipboard().is_err() {
                let _ = CloseClipboard();
                return Err("Failed to empty clipboard".into());
            }

            // Restore each format
            for entry in entries {
                if let Err(e) = write_format(entry.format, &entry.data) {
                    warn!("Failed to restore clipboard format {}: {}", entry.format, e);
                    // Continue with other formats
                } else {
                    debug!(
                        "Restored clipboard format {}: {} bytes",
                        entry.format,
                        entry.data.len()
                    );
                }
            }

            let _ = CloseClipboard();
        }

        Ok(())
    }

    /// Write data for a specific clipboard format
    unsafe fn write_format(format: u32, data: &[u8]) -> Result<(), String> {
        // Allocate global memory
        let hmem =
            GlobalAlloc(GHND, data.len()).map_err(|e| format!("GlobalAlloc failed: {}", e))?;

        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            return Err("GlobalLock failed".into());
        }

        // Copy data
        ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
        let _ = GlobalUnlock(hmem);

        // Set clipboard data (clipboard takes ownership of memory)
        // Convert HGLOBAL to HANDLE for SetClipboardData
        let handle = HANDLE(hmem.0);
        SetClipboardData(format, Some(handle))
            .map_err(|e| format!("SetClipboardData failed: {}", e))?;

        Ok(())
    }
}

/// Pastes text using the clipboard: saves current content, writes text, sends paste keystroke, restores clipboard.
fn paste_via_clipboard(
    enigo: &mut Enigo,
    text: &str,
    app_handle: &AppHandle,
    paste_method: &PasteMethod,
    convert_lf_to_crlf: bool,
    clipboard_handling: ClipboardHandling,
) -> Result<(), String> {
    // Check for Wayland first
    #[cfg(target_os = "linux")]
    if try_wayland_send_paste(paste_method)? {
        return Ok(());
    }

    let clipboard = app_handle.clipboard();

    // Backup clipboard content based on handling mode
    #[cfg(target_os = "windows")]
    let advanced_backup = if clipboard_handling == ClipboardHandling::RestoreAdvanced {
        match win_clipboard::backup_all_formats() {
            Ok(entries) => {
                info!("Advanced clipboard backup: {} formats saved", entries.len());
                Some(entries)
            }
            Err(e) => {
                warn!(
                    "Advanced clipboard backup failed: {}. Falling back to text-only.",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    // Text-only backup for non-advanced modes
    let text_backup = if clipboard_handling == ClipboardHandling::DontModify {
        clipboard.read_text().unwrap_or_default()
    } else {
        String::new()
    };

    // Convert LF to CRLF on Windows if enabled (fixes newlines being eaten by some apps)
    #[cfg(target_os = "windows")]
    let text = if convert_lf_to_crlf {
        // First normalize any existing CRLF to LF, then convert all LF to CRLF
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_string()
    };
    #[cfg(not(target_os = "windows"))]
    let text = text.to_string();

    clipboard
        .write_text(&text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

    std::thread::sleep(std::time::Duration::from_millis(50));

    match paste_method {
        PasteMethod::CtrlV => input::send_paste_ctrl_v(enigo)?,
        PasteMethod::CtrlShiftV => input::send_paste_ctrl_shift_v(enigo)?,
        PasteMethod::ShiftInsert => input::send_paste_shift_insert(enigo)?,
        _ => return Err("Invalid paste method for clipboard paste".into()),
    }

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Restore clipboard based on handling mode
    #[cfg(target_os = "windows")]
    if let Some(entries) = advanced_backup {
        if let Err(e) = win_clipboard::restore_all_formats(entries) {
            warn!(
                "Advanced clipboard restore failed: {}. Clipboard may contain transcription.",
                e
            );
        } else {
            info!("Advanced clipboard restore completed successfully");
        }
        return Ok(());
    }

    // Text-only restore for DontModify mode
    if clipboard_handling == ClipboardHandling::DontModify {
        clipboard
            .write_text(&text_backup)
            .map_err(|e| format!("Failed to restore clipboard: {}", e))?;
    }

    Ok(())
}

/// Attempts to paste using Wayland-specific tools (`wtype` or `dotool`).
/// Returns `Ok(true)` if a Wayland tool handled the paste, `Ok(false)` if not applicable,
/// or `Err` on failure from the underlying tool.
#[cfg(target_os = "linux")]
fn try_wayland_send_paste(paste_method: &PasteMethod) -> Result<bool, String> {
    if is_wayland() {
        if is_wtype_available() {
            send_paste_via_wtype(paste_method)?;
            return Ok(true);
        } else if is_dotool_available() {
            send_paste_via_dotool(paste_method)?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Check if wtype is available (Wayland text input tool)
#[cfg(target_os = "linux")]
fn is_wtype_available() -> bool {
    Command::new("which")
        .arg("wtype")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check if dotool is available (another Wayland text input tool)
#[cfg(target_os = "linux")]
fn is_dotool_available() -> bool {
    Command::new("which")
        .arg("dotool")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Paste using wtype and return a friendly error on failure.
#[cfg(target_os = "linux")]
fn send_paste_via_wtype(paste_method: &PasteMethod) -> Result<(), String> {
    let args: Vec<&str> = match paste_method {
        PasteMethod::CtrlV => vec!["-M", "ctrl", "-k", "v"],
        PasteMethod::ShiftInsert => vec!["-M", "shift", "-k", "Insert"],
        PasteMethod::CtrlShiftV => vec!["-M", "ctrl", "-M", "shift", "-k", "v"],
        _ => return Err("Unsupported paste method".into()),
    };

    let output = Command::new("wtype")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to execute wtype: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("wtype failed: {}", stderr));
    }

    Ok(())
}

/// Paste using dotool and return a friendly error on failure.
#[cfg(target_os = "linux")]
fn send_paste_via_dotool(paste_method: &PasteMethod) -> Result<(), String> {
    let command;
    match paste_method {
        PasteMethod::CtrlV => command = "echo key ctrl+v | dotool",
        PasteMethod::ShiftInsert => command = "echo key shift+insert | dotool",
        PasteMethod::CtrlShiftV => command = "echo key ctrl+shift+v | dotool",
        _ => return Err("Unsupported paste method".into()),
    }
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| format!("Failed to execute dotool: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("dotool failed: {}", stderr));
    }

    Ok(())
}

pub fn paste(text: String, app_handle: AppHandle) -> Result<(), String> {
    let settings = get_settings(&app_handle);
    let paste_method = settings.paste_method;
    let clipboard_handling = settings.clipboard_handling;

    // Append trailing space if setting is enabled
    let text = if settings.append_trailing_space {
        format!("{} ", text)
    } else {
        text
    };

    info!(
        "Using paste method: {:?}, clipboard handling: {:?}",
        paste_method, clipboard_handling
    );

    // Get the managed Enigo instance
    let enigo_state = app_handle
        .try_state::<EnigoState>()
        .ok_or("Enigo state not initialized")?;
    let mut enigo = enigo_state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock Enigo: {}", e))?;

    // Perform the paste operation
    match paste_method {
        PasteMethod::None => {
            info!("PasteMethod::None selected - skipping paste action");
        }
        PasteMethod::Direct => input::paste_text_direct(&mut enigo, &text)?,
        PasteMethod::CtrlV | PasteMethod::CtrlShiftV | PasteMethod::ShiftInsert => {
            paste_via_clipboard(
                &mut enigo,
                &text,
                &app_handle,
                &paste_method,
                settings.convert_lf_to_crlf,
                clipboard_handling,
            )?
        }
    }

    // After pasting, optionally copy to clipboard based on settings
    // (only if CopyToClipboard mode, which means we intentionally want to keep the transcription)
    if clipboard_handling == ClipboardHandling::CopyToClipboard {
        let clipboard = app_handle.clipboard();
        clipboard
            .write_text(&text)
            .map_err(|e| format!("Failed to copy to clipboard: {}", e))?;
    }

    Ok(())
}

pub fn capture_selection_text(app_handle: &AppHandle) -> Result<String, String> {
    let clipboard = app_handle.clipboard();
    let clipboard_backup = clipboard.read_text().unwrap_or_default();

    let capture_result = (|| -> Result<String, String> {
        let enigo_state = app_handle
            .try_state::<EnigoState>()
            .ok_or("Enigo state not initialized")?;
        let mut enigo = enigo_state
            .0
            .lock()
            .map_err(|e| format!("Failed to lock Enigo: {}", e))?;

        // Clear clipboard to ensure we don't pick up old content if selection is empty
        let _ = clipboard.write_text("");

        input::send_cut_ctrl_x(&mut enigo)?;
        std::thread::sleep(std::time::Duration::from_millis(80));

        clipboard
            .read_text()
            .map_err(|e| format!("Failed to read clipboard: {}", e))
    })();

    if let Err(err) = clipboard.write_text(&clipboard_backup) {
        warn!(
            "Failed to restore clipboard after selection capture: {}",
            err
        );
    }

    capture_result
}

pub fn capture_selection_text_copy(app_handle: &AppHandle) -> Result<String, String> {
    let clipboard = app_handle.clipboard();
    let clipboard_backup = clipboard.read_text().unwrap_or_default();

    let capture_result = (|| -> Result<String, String> {
        let enigo_state = app_handle
            .try_state::<EnigoState>()
            .ok_or("Enigo state not initialized")?;
        let mut enigo = enigo_state
            .0
            .lock()
            .map_err(|e| format!("Failed to lock Enigo: {}", e))?;

        // Clear clipboard so empty selections read as empty.
        let _ = clipboard.write_text("");

        input::send_copy_ctrl_c(&mut enigo)?;
        std::thread::sleep(std::time::Duration::from_millis(80));

        clipboard
            .read_text()
            .map_err(|e| format!("Failed to read clipboard: {}", e))
    })();

    if let Err(err) = clipboard.write_text(&clipboard_backup) {
        warn!(
            "Failed to restore clipboard after selection copy capture: {}",
            err
        );
    }

    capture_result
}
