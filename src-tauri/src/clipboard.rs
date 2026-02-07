use crate::input::{self, EnigoState};
use crate::settings::{get_settings, ClipboardHandling, PasteMethod};
use enigo::Enigo;
use log::{info, warn};
use std::time::Duration;
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
    use std::mem::size_of;
    use std::ptr;
    use windows::core::{Free, PCWSTR};
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::Graphics::Gdi::{CopyEnhMetaFileW, CopyMetaFileW, HBITMAP, HENHMETAFILE, HMETAFILE};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard, SetClipboardData,
        METAFILEPICT,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GHND};
    use windows::Win32::UI::WindowsAndMessaging::{CopyImage, IMAGE_BITMAP, IMAGE_FLAGS};

    // Standard clipboard format IDs (Win32 API constants).
    const CF_BITMAP_ID: u32 = 2;
    const CF_METAFILEPICT_ID: u32 = 3;
    const CF_PALETTE_ID: u32 = 9;
    const CF_ENHMETAFILE_ID: u32 = 14;
    const CF_OWNERDISPLAY_ID: u32 = 0x0080;
    const CF_DSPBITMAP_ID: u32 = 0x0082;
    const CF_DSPMETAFILEPICT_ID: u32 = 0x0083;
    const CF_DSPENHMETAFILE_ID: u32 = 0x008E;

    /// A typed snapshot of supported clipboard formats.
    /// We only store handle types we can safely duplicate and restore.
    pub struct ClipboardBackup {
        entries: Vec<ClipboardEntry>,
    }

    impl ClipboardBackup {
        pub fn len(&self) -> usize {
            self.entries.len()
        }
    }

    pub struct RestoreStats {
        pub restored_formats: usize,
        pub failed_formats: usize,
    }

    struct ClipboardEntry {
        format: u32,
        payload: ClipboardPayload,
    }

    enum ClipboardPayload {
        GlobalMemory(Vec<u8>),
        Bitmap(HBITMAP),
        EnhancedMetafile(HENHMETAFILE),
        MetafilePict {
            mm: i32,
            x_ext: i32,
            y_ext: i32,
            metafile: HMETAFILE,
        },
    }

    #[derive(Clone, Copy)]
    enum ClipboardFormatKind {
        GlobalMemory,
        Bitmap,
        EnhancedMetafile,
        MetafilePict,
        Unsupported,
    }

    /// Backup all clipboard formats
    pub fn backup_all_formats() -> Result<ClipboardBackup, String> {
        let mut entries = Vec::new();

        unsafe {
            // Open clipboard (None = current task)
            if OpenClipboard(None).is_err() {
                return Err("Failed to open clipboard for backup".into());
            }

            let result = (|| -> Result<ClipboardBackup, String> {
                // Enumerate all formats.
                let mut format = EnumClipboardFormats(0);
                while format != 0 {
                    match read_format(format) {
                        Ok(Some(entry)) => {
                            debug!("Backed up clipboard format {}", format);
                            entries.push(entry);
                        }
                        Ok(None) => {
                            debug!(
                                "Skipped clipboard format {} (unsupported or not safely copyable)",
                                format
                            );
                        }
                        Err(e) => {
                            warn!("Failed to back up clipboard format {}: {}", format, e);
                        }
                    }
                    format = EnumClipboardFormats(format);
                }

                debug!("Backed up {} clipboard formats", entries.len());
                Ok(ClipboardBackup { entries })
            })();

            let _ = CloseClipboard();
            result
        }
    }

    /// Read data for a specific clipboard format using safe, typed duplication.
    /// Never assume every clipboard handle is HGLOBAL.
    unsafe fn read_format(format: u32) -> Result<Option<ClipboardEntry>, String> {
        let handle =
            GetClipboardData(format).map_err(|e| format!("GetClipboardData failed: {}", e))?;
        if handle.0.is_null() {
            return Ok(None);
        }

        let payload = match format_kind(format) {
            ClipboardFormatKind::GlobalMemory => match copy_global_memory_bytes(handle) {
                Some(data) => ClipboardPayload::GlobalMemory(data),
                None => return Ok(None),
            },
            ClipboardFormatKind::Bitmap => {
                let copied = copy_bitmap_handle(handle)?;
                ClipboardPayload::Bitmap(copied)
            }
            ClipboardFormatKind::EnhancedMetafile => {
                let copied = copy_enh_metafile_handle(handle)?;
                ClipboardPayload::EnhancedMetafile(copied)
            }
            ClipboardFormatKind::MetafilePict => copy_metafile_pict_payload(handle)?,
            ClipboardFormatKind::Unsupported => return Ok(None),
        };

        Ok(Some(ClipboardEntry { format, payload }))
    }

    fn format_kind(format: u32) -> ClipboardFormatKind {
        match format {
            CF_BITMAP_ID | CF_DSPBITMAP_ID => ClipboardFormatKind::Bitmap,
            CF_ENHMETAFILE_ID | CF_DSPENHMETAFILE_ID => ClipboardFormatKind::EnhancedMetafile,
            CF_METAFILEPICT_ID | CF_DSPMETAFILEPICT_ID => ClipboardFormatKind::MetafilePict,
            // Owner-display and palette require owner-specific or palette-specific handling.
            CF_OWNERDISPLAY_ID | CF_PALETTE_ID => ClipboardFormatKind::Unsupported,
            // Remaining formats are treated as HGLOBAL-backed data (text, DIB, HTML/RTF, HDROP, etc.).
            _ => ClipboardFormatKind::GlobalMemory,
        }
    }

    unsafe fn copy_global_memory_bytes(handle: HANDLE) -> Option<Vec<u8>> {
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
        Some(data)
    }

    unsafe fn copy_bitmap_handle(handle: HANDLE) -> Result<HBITMAP, String> {
        let copied = CopyImage(handle, IMAGE_BITMAP, 0, 0, IMAGE_FLAGS(0))
            .map_err(|e| format!("CopyImage(IMAGE_BITMAP) failed: {}", e))?;
        let bitmap = HBITMAP(copied.0);
        if bitmap.is_invalid() {
            return Err("CopyImage returned invalid bitmap handle".into());
        }
        Ok(bitmap)
    }

    unsafe fn copy_enh_metafile_handle(handle: HANDLE) -> Result<HENHMETAFILE, String> {
        let copied = CopyEnhMetaFileW(HENHMETAFILE(handle.0), PCWSTR::null());
        if copied.is_invalid() {
            return Err("CopyEnhMetaFileW failed".into());
        }
        Ok(copied)
    }

    unsafe fn copy_metafile_pict_payload(handle: HANDLE) -> Result<ClipboardPayload, String> {
        let hglobal = HGLOBAL(handle.0);
        let raw_ptr = GlobalLock(hglobal) as *const METAFILEPICT;
        if raw_ptr.is_null() {
            return Err("GlobalLock failed for CF_METAFILEPICT".into());
        }

        let pict = *raw_ptr;
        let _ = GlobalUnlock(hglobal);

        if pict.hMF.is_invalid() {
            return Err("CF_METAFILEPICT contained invalid HMETAFILE".into());
        }

        let copied_mf = CopyMetaFileW(pict.hMF, PCWSTR::null());
        if copied_mf.is_invalid() {
            return Err("CopyMetaFileW failed for CF_METAFILEPICT".into());
        }

        Ok(ClipboardPayload::MetafilePict {
            mm: pict.mm,
            x_ext: pict.xExt,
            y_ext: pict.yExt,
            metafile: copied_mf,
        })
    }

    /// Restore all backed-up clipboard formats
    pub fn restore_all_formats(backup: ClipboardBackup) -> Result<RestoreStats, String> {
        if backup.entries.is_empty() {
            debug!("No clipboard entries to restore");
            return Ok(RestoreStats {
                restored_formats: 0,
                failed_formats: 0,
            });
        }

        let entries = backup.entries;

        unsafe {
            // Open clipboard (None = current task)
            if OpenClipboard(None).is_err() {
                cleanup_entries(entries);
                return Err("Failed to open clipboard for restore".into());
            }

            // Clear existing content
            if EmptyClipboard().is_err() {
                cleanup_entries(entries);
                let _ = CloseClipboard();
                return Err("Failed to empty clipboard".into());
            }

            let mut restored_formats = 0usize;
            let mut failed_formats = 0usize;

            // Restore each format
            for entry in entries {
                let format = entry.format;
                if let Err(e) = write_entry(entry) {
                    failed_formats += 1;
                    warn!("Failed to restore clipboard format {}: {}", format, e);
                } else {
                    restored_formats += 1;
                    debug!("Restored clipboard format {}", format);
                }
            }

            let _ = CloseClipboard();

            Ok(RestoreStats {
                restored_formats,
                failed_formats,
            })
        }
    }

    unsafe fn write_entry(entry: ClipboardEntry) -> Result<(), String> {
        match entry.payload {
            ClipboardPayload::GlobalMemory(data) => write_global_memory(entry.format, &data),
            ClipboardPayload::Bitmap(bitmap) => write_bitmap(entry.format, bitmap),
            ClipboardPayload::EnhancedMetafile(meta) => write_enh_metafile(entry.format, meta),
            ClipboardPayload::MetafilePict {
                mm,
                x_ext,
                y_ext,
                metafile,
            } => write_metafile_pict(entry.format, mm, x_ext, y_ext, metafile),
        }
    }

    unsafe fn write_global_memory(format: u32, data: &[u8]) -> Result<(), String> {
        let hmem =
            GlobalAlloc(GHND, data.len()).map_err(|e| format!("GlobalAlloc failed: {}", e))?;

        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            let mut h = hmem;
            h.free();
            return Err("GlobalLock failed".into());
        }

        ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
        let _ = GlobalUnlock(hmem);

        let handle = HANDLE(hmem.0);
        if let Err(e) = SetClipboardData(format, Some(handle)) {
            let mut h = hmem;
            h.free();
            return Err(format!("SetClipboardData failed: {}", e));
        }

        Ok(())
    }

    unsafe fn write_bitmap(format: u32, bitmap: HBITMAP) -> Result<(), String> {
        let handle = HANDLE(bitmap.0);
        if let Err(e) = SetClipboardData(format, Some(handle)) {
            let mut bitmap = bitmap;
            bitmap.free();
            return Err(format!("SetClipboardData failed for bitmap: {}", e));
        }
        Ok(())
    }

    unsafe fn write_enh_metafile(format: u32, metafile: HENHMETAFILE) -> Result<(), String> {
        let handle = HANDLE(metafile.0);
        if let Err(e) = SetClipboardData(format, Some(handle)) {
            let mut metafile = metafile;
            metafile.free();
            return Err(format!("SetClipboardData failed for enhanced metafile: {}", e));
        }
        Ok(())
    }

    unsafe fn write_metafile_pict(
        format: u32,
        mm: i32,
        x_ext: i32,
        y_ext: i32,
        metafile: HMETAFILE,
    ) -> Result<(), String> {
        let hmem = GlobalAlloc(GHND, size_of::<METAFILEPICT>())
            .map_err(|e| format!("GlobalAlloc failed for CF_METAFILEPICT: {}", e))?;

        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            let mut h = hmem;
            h.free();
            let mut metafile = metafile;
            metafile.free();
            return Err("GlobalLock failed".into());
        }

        let payload_ptr = ptr as *mut METAFILEPICT;
        *payload_ptr = METAFILEPICT {
            mm,
            xExt: x_ext,
            yExt: y_ext,
            hMF: metafile,
        };
        let _ = GlobalUnlock(hmem);

        let handle = HANDLE(hmem.0);
        if let Err(e) = SetClipboardData(format, Some(handle)) {
            let mut h = hmem;
            h.free();
            let mut metafile = metafile;
            metafile.free();
            return Err(format!("SetClipboardData failed for CF_METAFILEPICT: {}", e));
        }

        Ok(())
    }

    fn cleanup_entries(entries: Vec<ClipboardEntry>) {
        for entry in entries {
            cleanup_entry(entry);
        }
    }

    fn cleanup_entry(entry: ClipboardEntry) {
        unsafe {
            match entry.payload {
                ClipboardPayload::GlobalMemory(_) => {}
                ClipboardPayload::Bitmap(mut bitmap) => bitmap.free(),
                ClipboardPayload::EnhancedMetafile(mut metafile) => metafile.free(),
                ClipboardPayload::MetafilePict { mut metafile, .. } => metafile.free(),
            }
        }
    }
}

/// Pastes text using the clipboard: saves current content, writes text, sends paste keystroke, restores clipboard.
fn paste_via_clipboard(
    enigo: &mut Enigo,
    text: &str,
    app_handle: &AppHandle,
    paste_method: &PasteMethod,
    paste_delay_ms: u64,
    convert_lf_to_crlf: bool,
    clipboard_handling: ClipboardHandling,
) -> Result<(), String> {
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

    // Capture text backup for:
    // - DontModify mode (existing behavior)
    // - RestoreAdvanced fallback when rich-format restore is partial/failed
    let text_backup = if matches!(
        clipboard_handling,
        ClipboardHandling::DontModify | ClipboardHandling::RestoreAdvanced
    ) {
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

    // Write text to clipboard first
    clipboard
        .write_text(&text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

    std::thread::sleep(Duration::from_millis(paste_delay_ms));

    // Send paste key combo
    #[cfg(target_os = "linux")]
    let key_combo_sent = try_send_key_combo_linux(paste_method)?;

    #[cfg(not(target_os = "linux"))]
    let key_combo_sent = false;

    // Fall back to enigo if no native tool handled it
    if !key_combo_sent {
        match paste_method {
            PasteMethod::CtrlV => input::send_paste_ctrl_v(enigo)?,
            PasteMethod::CtrlShiftV => input::send_paste_ctrl_shift_v(enigo)?,
            PasteMethod::ShiftInsert => input::send_paste_shift_insert(enigo)?,
            _ => return Err("Invalid paste method for clipboard paste".into()),
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Restore clipboard based on handling mode
    #[cfg(target_os = "windows")]
    if let Some(backup) = advanced_backup {
        let mut needs_text_fallback = true;

        match win_clipboard::restore_all_formats(backup) {
            Ok(stats) if stats.failed_formats == 0 && stats.restored_formats > 0 => {
                info!(
                    "Advanced clipboard restore completed successfully ({} formats)",
                    stats.restored_formats
                );
                needs_text_fallback = false;
            }
            Ok(stats) => {
                warn!(
                    "Advanced clipboard restore incomplete: restored={}, failed={}. Falling back to text restore.",
                    stats.restored_formats, stats.failed_formats
                );
            }
            Err(e) => {
                warn!(
                    "Advanced clipboard restore failed: {}. Falling back to text restore.",
                    e
                );
            }
        }

        if needs_text_fallback {
            if let Err(e) = clipboard.write_text(&text_backup) {
                warn!("Fallback text clipboard restore failed: {}", e);
            } else {
                info!("Fallback text clipboard restore completed");
            }
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

/// Attempts to send a key combination using Linux-native tools.
/// Returns `Ok(true)` if a native tool handled it, `Ok(false)` to fall back to enigo.
#[cfg(target_os = "linux")]
fn try_send_key_combo_linux(paste_method: &PasteMethod) -> Result<bool, String> {
    if is_wayland() {
        // Wayland: prefer wtype, then dotool, then ydotool
        if is_wtype_available() {
            info!("Using wtype for key combo");
            send_key_combo_via_wtype(paste_method)?;
            return Ok(true);
        }
        if is_dotool_available() {
            info!("Using dotool for key combo");
            send_key_combo_via_dotool(paste_method)?;
            return Ok(true);
        }
        if is_ydotool_available() {
            info!("Using ydotool for key combo");
            send_key_combo_via_ydotool(paste_method)?;
            return Ok(true);
        }
    } else {
        // X11: prefer xdotool, then ydotool
        if is_xdotool_available() {
            info!("Using xdotool for key combo");
            send_key_combo_via_xdotool(paste_method)?;
            return Ok(true);
        }
        if is_ydotool_available() {
            info!("Using ydotool for key combo");
            send_key_combo_via_ydotool(paste_method)?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Attempts to type text directly using Linux-native tools.
/// Returns `Ok(true)` if a native tool handled it, `Ok(false)` to fall back to enigo.
#[cfg(target_os = "linux")]
fn try_direct_typing_linux(text: &str) -> Result<bool, String> {
    if is_wayland() {
        // Wayland: prefer wtype, then dotool, then ydotool
        if is_wtype_available() {
            info!("Using wtype for direct text input");
            type_text_via_wtype(text)?;
            return Ok(true);
        }
        if is_dotool_available() {
            info!("Using dotool for direct text input");
            type_text_via_dotool(text)?;
            return Ok(true);
        }
        if is_ydotool_available() {
            info!("Using ydotool for direct text input");
            type_text_via_ydotool(text)?;
            return Ok(true);
        }
    } else {
        // X11: prefer xdotool, then ydotool
        if is_xdotool_available() {
            info!("Using xdotool for direct text input");
            type_text_via_xdotool(text)?;
            return Ok(true);
        }
        if is_ydotool_available() {
            info!("Using ydotool for direct text input");
            type_text_via_ydotool(text)?;
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

/// Check if ydotool is available (uinput-based, works on both Wayland and X11)
#[cfg(target_os = "linux")]
fn is_ydotool_available() -> bool {
    Command::new("which")
        .arg("ydotool")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn is_xdotool_available() -> bool {
    Command::new("which")
        .arg("xdotool")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Type text directly via wtype on Wayland.
#[cfg(target_os = "linux")]
fn type_text_via_wtype(text: &str) -> Result<(), String> {
    let output = Command::new("wtype")
        .arg("--") // Protect against text starting with -
        .arg(text)
        .output()
        .map_err(|e| format!("Failed to execute wtype: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("wtype failed: {}", stderr));
    }

    Ok(())
}

/// Type text directly via xdotool on X11.
#[cfg(target_os = "linux")]
fn type_text_via_xdotool(text: &str) -> Result<(), String> {
    let output = Command::new("xdotool")
        .arg("type")
        .arg("--clearmodifiers")
        .arg("--")
        .arg(text)
        .output()
        .map_err(|e| format!("Failed to execute xdotool: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("xdotool failed: {}", stderr));
    }

    Ok(())
}

/// Type text directly via dotool (works on both Wayland and X11 via uinput).
#[cfg(target_os = "linux")]
fn type_text_via_dotool(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("dotool")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn dotool: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        // dotool uses "type <text>" command
        writeln!(stdin, "type {}", text)
            .map_err(|e| format!("Failed to write to dotool stdin: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for dotool: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("dotool failed: {}", stderr));
    }

    Ok(())
}

/// Type text directly via ydotool (uinput-based, requires ydotoold daemon).
#[cfg(target_os = "linux")]
fn type_text_via_ydotool(text: &str) -> Result<(), String> {
    let output = Command::new("ydotool")
        .arg("type")
        .arg("--")
        .arg(text)
        .output()
        .map_err(|e| format!("Failed to execute ydotool: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ydotool failed: {}", stderr));
    }

    Ok(())
}

/// Send a key combination (e.g., Ctrl+V) via wtype on Wayland.
#[cfg(target_os = "linux")]
fn send_key_combo_via_wtype(paste_method: &PasteMethod) -> Result<(), String> {
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

/// Send a key combination (e.g., Ctrl+V) via dotool.
#[cfg(target_os = "linux")]
fn send_key_combo_via_dotool(paste_method: &PasteMethod) -> Result<(), String> {
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

/// Send a key combination (e.g., Ctrl+V) via ydotool (requires ydotoold daemon).
#[cfg(target_os = "linux")]
fn send_key_combo_via_ydotool(paste_method: &PasteMethod) -> Result<(), String> {
    // ydotool uses Linux input event keycodes with format <keycode>:<pressed>
    // where pressed is 1 for down, 0 for up. Keycodes: ctrl=29, shift=42, v=47, insert=110
    let args: Vec<&str> = match paste_method {
        PasteMethod::CtrlV => vec!["key", "29:1", "47:1", "47:0", "29:0"],
        PasteMethod::ShiftInsert => vec!["key", "42:1", "110:1", "110:0", "42:0"],
        PasteMethod::CtrlShiftV => vec!["key", "29:1", "42:1", "47:1", "47:0", "42:0", "29:0"],
        _ => return Err("Unsupported paste method".into()),
    };

    let output = Command::new("ydotool")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to execute ydotool: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ydotool failed: {}", stderr));
    }

    Ok(())
}

/// Send a key combination (e.g., Ctrl+V) via xdotool on X11.
#[cfg(target_os = "linux")]
fn send_key_combo_via_xdotool(paste_method: &PasteMethod) -> Result<(), String> {
    let key_combo = match paste_method {
        PasteMethod::CtrlV => "ctrl+v",
        PasteMethod::CtrlShiftV => "ctrl+shift+v",
        PasteMethod::ShiftInsert => "shift+Insert",
        _ => return Err("Unsupported paste method".into()),
    };

    let output = Command::new("xdotool")
        .arg("key")
        .arg("--clearmodifiers")
        .arg(key_combo)
        .output()
        .map_err(|e| format!("Failed to execute xdotool: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("xdotool failed: {}", stderr));
    }

    Ok(())
}

/// Types text directly by simulating individual key presses.
fn paste_direct(enigo: &mut Enigo, text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        if try_direct_typing_linux(text)? {
            return Ok(());
        }
        info!("Falling back to enigo for direct text input");
    }

    input::paste_text_direct(enigo, text)
}

pub fn paste(text: String, app_handle: AppHandle) -> Result<(), String> {
    let settings = get_settings(&app_handle);
    let paste_method = settings.paste_method;
    let clipboard_handling = settings.clipboard_handling;
    let paste_delay_ms = settings.paste_delay_ms;

    // Append trailing space if setting is enabled
    let text = if settings.append_trailing_space {
        format!("{} ", text)
    } else {
        text
    };

    info!(
        "Using paste method: {:?}, clipboard handling: {:?}, delay: {}ms",
        paste_method, clipboard_handling, paste_delay_ms
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
        PasteMethod::Direct => {
            paste_direct(&mut enigo, &text)?;
        }
        PasteMethod::CtrlV | PasteMethod::CtrlShiftV | PasteMethod::ShiftInsert => {
            paste_via_clipboard(
                &mut enigo,
                &text,
                &app_handle,
                &paste_method,
                paste_delay_ms,
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
