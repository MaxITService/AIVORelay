use crate::input::{self, EnigoState};
use crate::settings::{get_settings, AutoSubmitKey, ClipboardHandling, OutputWhitespaceMode, PasteMethod, ResolvedOutputSettings};
use enigo::{Direction, Enigo, Key, Keyboard};
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[cfg(target_os = "linux")]
use crate::settings::TypingTool;
#[cfg(target_os = "linux")]
use crate::utils::{is_gnome_wayland, is_kde_wayland, is_wayland};
use std::process::Command;
#[cfg(target_os = "linux")]
use std::sync::OnceLock;

// Ctrl+V only queues work in the target application. Keep transcription text
// available long enough for a busy target to read it before another chunk or
// the user's original clipboard replaces it.
const CLIPBOARD_CONSUMER_GRACE: Duration = Duration::from_millis(200);

// Windows clipboard history observes clipboard updates asynchronously. Keep a
// history-eligible final transcript current briefly before restoring the
// user's previous clipboard so Win+V can capture the complete text once.
#[cfg(target_os = "windows")]
const CLIPBOARD_HISTORY_CAPTURE_GRACE: Duration = Duration::from_millis(200);

struct StreamingPasteSession {
    operation_id: u64,
    paste_method: PasteMethod,
    clipboard_handling: ClipboardHandling,
    paste_delay_ms: u64,
    check_active_window: bool,
    convert_lf_to_crlf: bool,
    exclude_final_from_history: bool,
    text_backup: String,
    #[cfg(not(target_os = "windows"))]
    image_backup: ClipboardImageBackup,
    last_clipboard_paste_sent_at: Option<Instant>,
    #[cfg(target_os = "windows")]
    last_clipboard_sequence: Option<u32>,
    #[cfg(target_os = "windows")]
    advanced_backup: Option<win_clipboard::ClipboardBackup>,
}

static STREAMING_PASTE_SESSION: Lazy<Mutex<Option<StreamingPasteSession>>> =
    Lazy::new(|| Mutex::new(None));

/// Live chunks never lose text afterwards: their trailing whitespace is held
/// back and resolved once at finalization, so no deleting keystrokes are needed.
/// `Preserve` bypasses the hold entirely and inserts chunks unchanged.
#[derive(Debug, Default)]
struct HeldTrailingWhitespace {
    tail: String,
    saw_text: bool,
}

impl HeldTrailingWhitespace {
    /// Returns the part of `held tail + chunk` that may be inserted now.
    fn push(&mut self, mode: OutputWhitespaceMode, chunk: String) -> String {
        if mode == OutputWhitespaceMode::Preserve || chunk.is_empty() {
            return chunk;
        }
        self.saw_text = true;
        let mut text = std::mem::take(&mut self.tail);
        text.push_str(&chunk);
        let body_len = text.trim_end_matches(char::is_whitespace).len();
        self.tail = text.split_off(body_len);
        text
    }

    /// Mirrors `apply_output_whitespace_policy`'s trailing rules for the held tail.
    fn resolve(&mut self, mode: OutputWhitespaceMode) -> String {
        let tail = std::mem::take(&mut self.tail);
        match mode {
            OutputWhitespaceMode::Preserve => tail,
            OutputWhitespaceMode::RemoveIfPresent => String::new(),
            OutputWhitespaceMode::AddIfMissing if tail.is_empty() && self.saw_text => " ".to_string(),
            OutputWhitespaceMode::AddIfMissing => tail,
        }
    }
}

struct DictationOutputSession {
    operation_id: u64,
    settings: ResolvedOutputSettings,
    delivered: bool,
    failed: bool,
    held_whitespace: HeldTrailingWhitespace,
}

/// The newest recording owns live chunks. An older operation may still have its
/// final delivery queued on the main thread after a synchronous wait timed out,
/// so its captured settings stay available until that delivery consumes them.
#[derive(Default)]
struct DictationOutputSessions {
    current_operation_id: Option<u64>,
    sessions: Vec<DictationOutputSession>,
}

/// Error text of a live paste that stopped because the foreground window was no longer the target.
/// `live_output_buffer` compares against it to tell this pause from a real failure.
pub(crate) const TARGET_WINDOW_CHANGED_ERROR: &str =
    "Streaming paste was interrupted because the target window changed";

/// Error of a dictation whose opening key was not pressed at recording start. That failure
/// already has its own notice, so the generic insertion error is not shown again.
pub(crate) const OPENING_KEY_NOT_PRESSED_ERROR: &str =
    "Nothing was inserted because the opening key was not pressed at recording start";

// Bounds entries whose delivery never ran; only a few can be pending at once.
const MAX_DICTATION_OUTPUT_SESSIONS: usize = 4;

impl DictationOutputSessions {
    fn begin(&mut self, operation_id: u64, settings: Option<ResolvedOutputSettings>) {
        self.current_operation_id = Some(operation_id);
        self.take(operation_id);
        if let Some(settings) = settings {
            if self.sessions.len() >= MAX_DICTATION_OUTPUT_SESSIONS {
                self.sessions.remove(0);
            }
            self.sessions.push(DictationOutputSession {
                operation_id, settings, delivered: false, failed: false,
                held_whitespace: HeldTrailingWhitespace::default(),
            });
        }
    }

    fn get(&self, operation_id: u64) -> Option<&DictationOutputSession> {
        self.sessions.iter().find(|session| session.operation_id == operation_id)
    }

    fn get_mut(&mut self, operation_id: u64) -> Option<&mut DictationOutputSession> {
        self.sessions.iter_mut().find(|session| session.operation_id == operation_id)
    }

    fn current(&self) -> Option<&DictationOutputSession> {
        self.get(self.current_operation_id?)
    }

    fn output_for_chunk(&self, operation_id: Option<u64>) -> Option<(u64, ResolvedOutputSettings)> {
        let session = match operation_id {
            Some(id) => self.get(id),
            None => self.current(),
        }?;
        Some((session.operation_id, session.settings.clone()))
    }

    fn take(&mut self, operation_id: u64) -> Option<DictationOutputSession> {
        let index = self.sessions.iter().position(|session| session.operation_id == operation_id)?;
        Some(self.sessions.remove(index))
    }

    /// Only whitespace is held, so the dictation itself stays complete without it.
    fn drop_held_whitespace(&mut self, operation_id: u64) {
        if let Some(session) = self.get_mut(operation_id) {
            session.held_whitespace = HeldTrailingWhitespace::default();
        }
    }
}

static DICTATION_OUTPUT_SESSIONS: Lazy<Mutex<DictationOutputSessions>> =
    Lazy::new(|| Mutex::new(DictationOutputSessions::default()));

fn lock_dictation_output_sessions() -> MutexGuard<'static, DictationOutputSessions> {
    DICTATION_OUTPUT_SESSIONS.lock().unwrap_or_else(|error| {
        warn!("Dictation output session lock poisoned; recovering");
        error.into_inner()
    })
}

pub(crate) fn begin_dictation_output_session(
    operation_id: u64,
    settings: Option<ResolvedOutputSettings>,
) {
    lock_dictation_output_sessions().begin(operation_id, settings);
}

fn current_dictation_output() -> Option<(u64, ResolvedOutputSettings)> {
    lock_dictation_output_sessions().output_for_chunk(None)
}

pub(crate) fn dictation_output_settings(operation_id: u64) -> Option<ResolvedOutputSettings> {
    lock_dictation_output_sessions().get(operation_id).map(|session| session.settings.clone())
}

pub(crate) fn end_dictation_output_session_if_matches(operation_id: u64) {
    lock_dictation_output_sessions().take(operation_id);
    crate::live_output_buffer::cancel_unfinished(operation_id);
    // Runs after every queued insertion, so a completed recording never gets the cancel key.
    crate::early_open::finish_operation(operation_id);
}

/// A finalization that runs while a newer recording or transcription owns the session
/// must not type its trailing whitespace into that recording's window.
pub(crate) fn drop_held_trailing_whitespace(operation_id: u64) {
    lock_dictation_output_sessions().drop_held_whitespace(operation_id);
}

pub(crate) fn dictation_output_is_buffered(operation_id: u64) -> bool {
    dictation_output_settings(operation_id).is_some_and(|settings| settings.buffers_external_output())
}
// Clipboard-backed paste and selection capture must be one transaction. Always
// acquire this before STREAMING_PASTE_SESSION and Enigo locks.
static CLIPBOARD_TRANSACTION: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub(crate) fn lock_clipboard_transaction(
    operation: &str,
) -> Result<MutexGuard<'static, ()>, String> {
    CLIPBOARD_TRANSACTION
        .lock()
        .map_err(|_| format!("Clipboard transaction lock is unavailable while {operation}"))
}

/// Windows-only: Advanced clipboard backup/restore that preserves all formats
#[cfg(target_os = "windows")]
mod win_clipboard {
    use log::{debug, info, warn};
    use std::mem::size_of;
    use std::ptr;
    use std::sync::atomic::{AtomicU32, Ordering};
    use windows::core::{Free, PCWSTR};
    use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
    use windows::Win32::Graphics::Gdi::{
        CopyEnhMetaFileW, CopyMetaFileW, HBITMAP, HENHMETAFILE, HMETAFILE,
    };
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
        GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData, METAFILEPICT,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GHND};
    use windows::Win32::UI::WindowsAndMessaging::{CopyImage, IMAGE_BITMAP, IMAGE_FLAGS};

    // Captured under the Windows clipboard lock, before paste delays allow external copies.
    // Callers serialize app transactions with CLIPBOARD_TRANSACTION.
    static LAST_WRITE_SEQUENCE: AtomicU32 = AtomicU32::new(0);

    pub fn last_write_sequence() -> u32 {
        LAST_WRITE_SEQUENCE.load(Ordering::Relaxed)
    }

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

    // SAFETY: The backup exclusively owns duplicated, process-wide GDI
    // handles. It is moved between threads but never accessed concurrently;
    // restoration or cleanup consumes each handle exactly once.
    unsafe impl Send for ClipboardBackup {}

    impl ClipboardBackup {
        pub fn len(&self) -> usize {
            self.entries.len()
        }
    }

    impl Drop for ClipboardBackup {
        fn drop(&mut self) {
            cleanup_entries(std::mem::take(&mut self.entries));
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

    const CF_UNICODETEXT_ID: u32 = 13;

    /// Registers a clipboard-history marker format (honored by the Windows
    /// clipboard history and cloud clipboard; also used by password managers
    /// such as KeePass) and sets it to the given DWORD value. Best-effort:
    /// failures are logged, not propagated, so the paste itself never breaks.
    unsafe fn write_history_marker(format_name: &str, value: u32) {
        let wide: Vec<u16> = format_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let format = RegisterClipboardFormatW(PCWSTR(wide.as_ptr()));
        if format == 0 {
            warn!("RegisterClipboardFormatW failed for {}", format_name);
            return;
        }
        if let Err(e) = write_global_memory(format, &value.to_le_bytes()) {
            warn!("Failed to set clipboard marker {}: {}", format_name, e);
        }
    }

    unsafe fn write_history_exclusion_markers() {
        write_history_marker("ExcludeClipboardContentFromMonitorProcessing", 1);
        write_history_marker("CanIncludeInClipboardHistory", 0);
        write_history_marker("CanUploadToCloudClipboard", 0);
    }

    /// Writes UTF-16 text with an explicit Windows clipboard-history policy.
    /// Excluded writes are also omitted from clipboard monitors and cloud sync;
    /// included writes explicitly request eligibility for local Win+V history.
    pub fn write_text_with_history_policy(
        text: &str,
        exclude_from_history: bool,
    ) -> Result<(), String> {
        write_text_if_unchanged(text, exclude_from_history, None).map(|_| ())
    }

    pub fn write_text_if_unchanged(
        text: &str,
        exclude_from_history: bool,
        expected_sequence: Option<u32>,
    ) -> Result<Option<u32>, String> {
        unsafe {
            OpenClipboard(None).map_err(|e| format!("Failed to open clipboard: {}", e))?;
            if let Some(expected) = expected_sequence {
                let current = GetClipboardSequenceNumber();
                if expected == 0 || expected != current {
                    let _ = CloseClipboard();
                    info!(
                        "Skipping clipboard text write: clipboard changed externally (expected sequence {}, current {})",
                        expected, current
                    );
                    return Ok(None);
                }
            }
            let result = (|| {
                EmptyClipboard().map_err(|e| format!("Failed to empty clipboard: {}", e))?;
                let mut wide: Vec<u16> = text.encode_utf16().collect();
                wide.push(0);
                let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
                write_global_memory(CF_UNICODETEXT_ID, bytes)?;
                if exclude_from_history {
                    write_history_exclusion_markers();
                } else {
                    write_history_marker("CanIncludeInClipboardHistory", 1);
                }
                Ok(())
            })();
            let _ = CloseClipboard();
            // CloseClipboard adds synthesized formats (CF_TEXT, CF_OEMTEXT,
            // CF_LOCALE), which bumps the sequence number again. Read it only
            // after closing so later "unchanged" checks compare the final value.
            let sequence = GetClipboardSequenceNumber();
            LAST_WRITE_SEQUENCE.store(sequence, Ordering::Relaxed);
            result.map(|()| Some(sequence))
        }
    }

    /// Restores all backed-up formats as a transient app-owned update. The
    /// exclusion markers prevent restoration itself from duplicating or
    /// reordering the user's Windows clipboard history.
    pub fn restore_all_formats(
        mut backup: ClipboardBackup,
        owner: Option<HWND>,
        expected_sequence: Option<u32>,
    ) -> Result<Option<RestoreStats>, String> {
        if backup.entries.is_empty() {
            debug!("No clipboard entries to restore");
            return Ok(Some(RestoreStats {
                restored_formats: 0,
                failed_formats: 0,
            }));
        }

        // Take ownership away from Drop. Every entry is now either transferred
        // to the system clipboard or explicitly cleaned up below.
        let entries = std::mem::take(&mut backup.entries);

        unsafe {
            if OpenClipboard(owner).is_err() {
                cleanup_entries(entries);
                return Err("Failed to open clipboard for restore".into());
            }

            if let Some(expected) = expected_sequence {
                let current = GetClipboardSequenceNumber();
                if expected == 0 || expected != current {
                    cleanup_entries(entries);
                    let _ = CloseClipboard();
                    info!(
                        "Skipping clipboard restore: clipboard changed externally (expected sequence {}, current {})",
                        expected, current
                    );
                    return Ok(None);
                }
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

            write_history_exclusion_markers();

            let _ = CloseClipboard();
            // Same as write_text_if_unchanged: the final sequence number is
            // only known once the clipboard is closed.
            LAST_WRITE_SEQUENCE.store(GetClipboardSequenceNumber(), Ordering::Relaxed);

            Ok(Some(RestoreStats {
                restored_formats,
                failed_formats,
            }))
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
            return Err(format!(
                "SetClipboardData failed for enhanced metafile: {}",
                e
            ));
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
            return Err(format!(
                "SetClipboardData failed for CF_METAFILEPICT: {}",
                e
            ));
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

#[cfg(target_os = "windows")]
fn restore_advanced_clipboard_with_text_fallback(
    app_handle: &AppHandle,
    backup: win_clipboard::ClipboardBackup,
    text_backup: &str,
    use_documented_owner: bool,
    expected_sequence: Option<u32>,
) -> Result<(), String> {
    let restore_result = if use_documented_owner {
        clipboard_owner_hwnd(app_handle)
            .and_then(|owner| win_clipboard::restore_all_formats(backup, Some(owner), expected_sequence))
    } else {
        win_clipboard::restore_all_formats(backup, None, expected_sequence)
    };
    let fallback_sequence = if restore_result.as_ref().is_ok_and(|stats| {
        stats.as_ref().is_some_and(|stats| stats.restored_formats + stats.failed_formats > 0)
    }) {
        expected_sequence.map(|_| win_clipboard::last_write_sequence())
    } else {
        expected_sequence
    };
    let needs_text_fallback = match restore_result {
        Ok(None) => return Ok(()),
        Ok(Some(stats)) if stats.failed_formats == 0 && stats.restored_formats > 0 => {
            info!(
                "Advanced clipboard restore completed successfully ({} formats)",
                stats.restored_formats
            );
            false
        }
        Ok(Some(stats)) => {
            warn!(
                "Advanced clipboard restore incomplete: restored={}, failed={}. Falling back to text restore.",
                stats.restored_formats, stats.failed_formats
            );
            true
        }
        Err(e) => {
            warn!(
                "Advanced clipboard restore failed: {}. Falling back to text restore.",
                e
            );
            true
        }
    };

    if needs_text_fallback {
        win_clipboard::write_text_if_unchanged(text_backup, true, fallback_sequence)
            .map_err(|e| format!("Fallback text clipboard restore failed: {}", e))?;
        info!("Fallback text clipboard restore completed");
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn clipboard_owner_hwnd(
    app_handle: &AppHandle,
) -> Result<windows::Win32::Foundation::HWND, String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        if let Ok(hwnd) = window.hwnd() {
            return Ok(hwnd);
        }
    }

    // Speech-only mode has a native clipboard-owner window, but no WebViews.
    if let Some(owner) = app_handle.try_state::<crate::webview_mode::ClipboardOwnerWindow>() {
        return Ok(owner.hwnd());
    }

    for window in app_handle.webview_windows().into_values() {
        if let Ok(hwnd) = window.hwnd() {
            return Ok(hwnd);
        }
    }

    Err("No AivoRelay window handle is available for documented clipboard ownership".to_string())
}

fn restores_all_clipboard_formats(handling: ClipboardHandling) -> bool {
    matches!(
        handling,
        ClipboardHandling::RestoreAdvanced | ClipboardHandling::RestoreAdvancedOwned
    )
}

fn restores_clipboard_contents(handling: ClipboardHandling) -> bool {
    matches!(
        handling,
        ClipboardHandling::RestorePlainText
            | ClipboardHandling::RestoreAdvanced
            | ClipboardHandling::RestoreAdvancedOwned
    )
}

fn uses_documented_clipboard_owner(handling: ClipboardHandling) -> bool {
    handling == ClipboardHandling::RestoreAdvancedOwned
}

fn convert_text_for_clipboard(text: &str, convert_lf_to_crlf: bool) -> String {
    #[cfg(target_os = "windows")]
    {
        if convert_lf_to_crlf {
            // First normalize any existing CRLF to LF, then convert all LF to CRLF.
            return text.replace("\r\n", "\n").replace('\n', "\r\n");
        }
    }
    text.to_string()
}

/// Serializes everything that types or pastes into the foreground application (a
/// paste, a streamed chunk, an opening or submit key) so they never interleave.
/// Holding the input system's own lock for a whole paste used to do this, which
/// also made every paste depend on the input system existing.
static OUTPUT_SERIALIZER: Mutex<()> = Mutex::new(());

fn lock_output() -> MutexGuard<'static, ()> {
    OUTPUT_SERIALIZER
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

/// Takes the input system only when a key press really needs it. Methods that run
/// an external tool, or none at all, then work where it is unavailable (for
/// example Wayland without XWayland).
struct LazyEnigo<'a> {
    app: &'a AppHandle,
    guard: Option<MutexGuard<'a, Enigo>>,
}

impl<'a> LazyEnigo<'a> {
    fn new(app: &'a AppHandle) -> Self {
        Self { app, guard: None }
    }

    fn get(&mut self) -> Result<&mut Enigo, String> {
        if self.guard.is_none() {
            let state: &'a EnigoState = self
                .app
                .try_state::<EnigoState>()
                .ok_or("Enigo state not initialized")?
                .inner();
            let guard = state
                .0
                .lock()
                .map_err(|e| format!("Failed to lock Enigo: {}", e))?;
            self.guard = Some(guard);
        }
        match self.guard.as_mut() {
            Some(guard) => Ok(&mut **guard),
            None => Err("Enigo state not initialized".to_string()),
        }
    }
}

fn send_paste_shortcut(enigo: &mut LazyEnigo, paste_method: &PasteMethod) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let key_combo_sent = try_send_key_combo_linux(paste_method)?;

    #[cfg(not(target_os = "linux"))]
    let key_combo_sent = false;

    // Fall back to enigo if no native tool handled it.
    if !key_combo_sent {
        match paste_method {
            PasteMethod::CtrlV => input::send_paste_ctrl_v(enigo.get()?)?,
            PasteMethod::CtrlShiftV => input::send_paste_ctrl_shift_v(enigo.get()?)?,
            PasteMethod::ShiftInsert => input::send_paste_shift_insert(enigo.get()?)?,
            _ => return Err("Invalid paste method for clipboard paste".into()),
        }
    }

    Ok(())
}

/// Writes plain text to the system clipboard. On Wayland, `wl-copy` is preferred
/// because the clipboard plugin cannot reliably own the selection there.
fn write_plain_clipboard_text(app_handle: &AppHandle, text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if is_wayland() && is_wl_copy_available() {
        info!("Using wl-copy for clipboard write on Wayland");
        return write_clipboard_via_wl_copy(text);
    }

    app_handle
        .clipboard()
        .write_text(text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))
}

/// Writes transcription-owned text to the clipboard. On Windows, when
/// `exclude_from_history` is set, the write is marked so the clipboard
/// history (Win+V) and cloud clipboard sync skip it.
fn write_transcription_text(
    app_handle: &AppHandle,
    text: &str,
    exclude_from_history: bool,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let _ = app_handle;
        return win_clipboard::write_text_with_history_policy(text, exclude_from_history);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = exclude_from_history;
        write_plain_clipboard_text(app_handle, text)
    }
}

/// Whether the final full-text write is excluded from the clipboard history.
/// In KeepTranscription mode the final text is always recorded (matching the
/// legacy copy_to_clipboard behavior); in the restore modes it follows the
/// user's history toggle.
fn exclude_final_write_from_history(settings: &ResolvedOutputSettings) -> bool {
    settings.clipboard_handling != ClipboardHandling::KeepTranscription
        && !settings.clipboard_history_allowed
}

/// Final full-text transcription write at the end of an operation. Unless
/// excluded, the clipboard always ends with the complete final text and the
/// history can capture it as one complete item.
#[cfg(not(target_os = "windows"))]
fn write_final_transcription_text(
    app_handle: &AppHandle,
    text: &str,
    exclude_from_history: bool,
) -> Result<(), String> {
    write_transcription_text(app_handle, text, exclude_from_history)
}

/// Image content worth restoring off Windows (a screenshot, say). Windows restores
/// every clipboard format through its own backup instead.
#[cfg(not(target_os = "windows"))]
type ClipboardImageBackup = Option<tauri::image::Image<'static>>;

/// Probes for an image only when there is no text to restore: reading one decodes
/// the whole bitmap, and text is by far the common case.
#[cfg(not(target_os = "windows"))]
fn backup_clipboard_image(
    app_handle: &AppHandle,
    text_backup: &str,
    handling: ClipboardHandling,
) -> ClipboardImageBackup {
    let restores = matches!(
        handling,
        ClipboardHandling::RestorePlainText
            | ClipboardHandling::RestoreAdvanced
            | ClipboardHandling::RestoreAdvancedOwned
    );
    if !restores || !text_backup.is_empty() {
        return None;
    }
    app_handle
        .clipboard()
        .read_image()
        .ok()
        .map(|image| image.to_owned())
}

/// Restores the saved clipboard off Windows: the text if there was any, else the
/// image, else an empty clipboard. Without this a screenshot was silently wiped.
#[cfg(not(target_os = "windows"))]
fn restore_clipboard_backup(
    app_handle: &AppHandle,
    text: &str,
    image: &ClipboardImageBackup,
) -> Result<(), String> {
    if text.is_empty() {
        if let Some(image) = image {
            return app_handle
                .clipboard()
                .write_image(image)
                .map_err(|e| format!("Failed to restore clipboard image: {}", e));
        }
    }
    restore_plain_text_without_history(app_handle, text)
}

fn restore_plain_text_without_history(app_handle: &AppHandle, text: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let _ = app_handle;
        return win_clipboard::write_text_with_history_policy(text, true);
    }

    #[cfg(not(target_os = "windows"))]
    write_plain_clipboard_text(app_handle, text)
        .map_err(|e| format!("Failed to restore clipboard: {}", e))
}

fn paste_via_clipboard_no_restore_guarded(
    enigo: &mut LazyEnigo,
    text: &str,
    app_handle: &AppHandle,
    paste_method: &PasteMethod,
    paste_delay_ms: u64,
    convert_lf_to_crlf: bool,
    exclude_from_history: bool,
    can_continue: &impl Fn() -> bool,
) -> Result<Option<Instant>, String> {
    let text = convert_text_for_clipboard(text, convert_lf_to_crlf);

    write_transcription_text(app_handle, &text, exclude_from_history)?;

    if !wait_for_output_delay(paste_delay_ms, can_continue) {
        return Ok(None);
    }
    send_paste_shortcut(enigo, paste_method)?;
    let paste_sent_at = Instant::now();
    std::thread::sleep(Duration::from_millis(50));
    Ok(Some(paste_sent_at))
}

fn remaining_clipboard_consumer_grace(paste_sent_at: Option<Instant>, now: Instant) -> Duration {
    paste_sent_at
        .map(|sent_at| {
            CLIPBOARD_CONSUMER_GRACE.saturating_sub(now.saturating_duration_since(sent_at))
        })
        .unwrap_or(Duration::ZERO)
}

fn wait_for_clipboard_consumer(paste_sent_at: Option<Instant>) {
    let remaining = remaining_clipboard_consumer_grace(paste_sent_at, Instant::now());
    if !remaining.is_zero() {
        debug!(
            "Waiting {}ms for target application to consume clipboard paste",
            remaining.as_millis()
        );
        std::thread::sleep(remaining);
    }
}

fn wait_for_clipboard_history_capture(
    exclude_from_history: bool,
    clipboard_handling: ClipboardHandling,
) {
    #[cfg(target_os = "windows")]
    if !exclude_from_history && restores_clipboard_contents(clipboard_handling) {
        debug!(
            "Waiting {}ms for Windows clipboard history to capture final transcription",
            CLIPBOARD_HISTORY_CAPTURE_GRACE.as_millis()
        );
        std::thread::sleep(CLIPBOARD_HISTORY_CAPTURE_GRACE);
    }

    #[cfg(not(target_os = "windows"))]
    let _ = (exclude_from_history, clipboard_handling);
}

fn restore_streaming_session_backup(
    session: StreamingPasteSession,
    app_handle: &AppHandle,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    if session.last_clipboard_sequence.is_none() {
        return Ok(());
    }
    if restores_all_clipboard_formats(session.clipboard_handling) {
        #[cfg(target_os = "windows")]
        if let Some(backup) = session.advanced_backup {
            return restore_advanced_clipboard_with_text_fallback(
                app_handle,
                backup,
                &session.text_backup,
                uses_documented_clipboard_owner(session.clipboard_handling),
                session.last_clipboard_sequence,
            );
        }
    }

    if restores_clipboard_contents(session.clipboard_handling) {
        #[cfg(target_os = "windows")]
        win_clipboard::write_text_if_unchanged(
            &session.text_backup, true, session.last_clipboard_sequence,
        )?;
        #[cfg(not(target_os = "windows"))]
        restore_clipboard_backup(app_handle, &session.text_backup, &session.image_backup)?;
    }

    Ok(())
}

fn restore_streaming_session(
    session: StreamingPasteSession,
    app_handle: &AppHandle,
) -> Result<(), String> {
    wait_for_clipboard_consumer(session.last_clipboard_paste_sent_at);
    restore_streaming_session_backup(session, app_handle)
}

fn delivery_after_clipboard_cleanup(
    delivery: Result<bool, String>,
    cleanup: Result<(), String>,
    report_cleanup_error: impl FnOnce(&str),
) -> Result<bool, String> {
    if let Err(error) = cleanup {
        report_cleanup_error(&error);
    }
    delivery
}

fn streaming_delivery_succeeded(
    delivered: bool,
    failed: bool,
    paste_method: PasteMethod,
    can_continue: bool,
) -> bool {
    !failed && (delivered || paste_method == PasteMethod::None) && can_continue
}

pub fn begin_streaming_paste_session(
    app_handle: &AppHandle,
    operation_id: u64,
) -> Result<(), String> {
    let dictation_output = dictation_output_settings(operation_id);
    if dictation_output.as_ref().is_some_and(|settings| settings.buffers_external_output()) {
        return Ok(());
    }
    let settings = dictation_output
        .unwrap_or_else(|| ResolvedOutputSettings::resolve(&get_settings(app_handle), None));
    if !matches!(
        settings.paste_method,
        PasteMethod::CtrlV | PasteMethod::CtrlShiftV | PasteMethod::ShiftInsert
    ) {
        return Ok(());
    }

    let _clipboard_guard = lock_clipboard_transaction("starting streaming paste")?;

    // Serialize session replacement with chunk insertion. Restoring a stale
    // session before taking the next backup prevents the new session from
    // accidentally snapshotting transient transcription text.
    let mut session_guard = STREAMING_PASTE_SESSION
        .lock()
        .map_err(|_| "Streaming clipboard session lock poisoned".to_string())?;
    if session_guard.as_ref().is_some_and(|session| session.operation_id == operation_id) {
        return Ok(());
    }
    if let Some(previous) = session_guard.take() {
        warn!("Replacing stale streaming clipboard session; restoring previous backup");
        if let Err(error) = restore_streaming_session(previous, app_handle) {
            warn!(
                "Failed to restore stale streaming clipboard session: {}",
                error
            );
        }
    }

    let clipboard = app_handle.clipboard();

    let text_backup = if matches!(
        settings.clipboard_handling,
        ClipboardHandling::RestorePlainText
            | ClipboardHandling::RestoreAdvanced
            | ClipboardHandling::RestoreAdvancedOwned
    ) {
        clipboard.read_text().unwrap_or_default()
    } else {
        String::new()
    };
    #[cfg(not(target_os = "windows"))]
    let image_backup = backup_clipboard_image(app_handle, &text_backup, settings.clipboard_handling);

    #[cfg(target_os = "windows")]
    let advanced_backup = if restores_all_clipboard_formats(settings.clipboard_handling) {
        match win_clipboard::backup_all_formats() {
            Ok(backup) => {
                info!(
                    "Advanced streaming clipboard backup: {} formats saved",
                    backup.len()
                );
                Some(backup)
            }
            Err(e) => {
                warn!(
                    "Advanced streaming clipboard backup failed: {}. Falling back to text-only restore.",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    let new_session = StreamingPasteSession {
        operation_id,
        paste_method: settings.paste_method,
        clipboard_handling: settings.clipboard_handling,
        paste_delay_ms: settings.paste_delay_ms,
        check_active_window: settings.check_active_window,
        convert_lf_to_crlf: settings.convert_lf_to_crlf,
        exclude_final_from_history: exclude_final_write_from_history(&settings),
        text_backup,
        #[cfg(not(target_os = "windows"))]
        image_backup,
        last_clipboard_paste_sent_at: None,
        #[cfg(target_os = "windows")]
        last_clipboard_sequence: None,
        #[cfg(target_os = "windows")]
        advanced_backup,
    };

    *session_guard = Some(new_session);

    Ok(())
}

pub fn end_streaming_paste_session(app_handle: &AppHandle) -> Result<(), String> {
    let _clipboard_guard = lock_clipboard_transaction("ending streaming paste")?;
    {
        let mut sessions = lock_dictation_output_sessions();
        if let Some(operation_id) = sessions.current_operation_id {
            sessions.take(operation_id);
        }
    }
    let mut guard = STREAMING_PASTE_SESSION
        .lock()
        .map_err(|_| "Streaming clipboard session lock poisoned".to_string())?;
    if let Some(session) = guard.take() {
        restore_streaming_session(session, app_handle)?;
    }

    Ok(())
}

pub fn end_streaming_paste_session_if_matches(
    app_handle: &AppHandle,
    operation_id: u64,
) -> Result<bool, String> {
    end_dictation_output_session_if_matches(operation_id);
    release_streaming_paste_session_if_matches(app_handle, operation_id)
}

/// Restore a detached operation's clipboard without discarding its captured
/// output settings. Gemini can still deliver chunks while it finalizes.
pub(crate) fn release_streaming_paste_session_if_matches(
    app_handle: &AppHandle,
    operation_id: u64,
) -> Result<bool, String> {
    let _clipboard_guard = lock_clipboard_transaction("ending streaming paste")?;
    let mut guard = STREAMING_PASTE_SESSION
        .lock()
        .map_err(|_| "Streaming clipboard session lock poisoned".to_string())?;
    if guard
        .as_ref()
        .is_some_and(|session| session.operation_id == operation_id)
    {
        if let Some(session) = guard.take() {
            restore_streaming_session(session, app_handle)?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Completes an operation-scoped streaming clipboard session. The complete
/// transcript is written once with the captured history policy, then a restore
/// mode puts the user's previous clipboard back without creating another Win+V
/// entry. Cancellation paths use `end_streaming_paste_session_if_matches`
/// instead and never publish a partial transcript as final history.
pub fn finalize_streaming_paste_session_if_matches(
    app_handle: &AppHandle,
    operation_id: u64,
    final_text: &str,
) -> Result<bool, String> {
    let _clipboard_guard = lock_clipboard_transaction("finalizing streaming paste")?;
    let mut guard = STREAMING_PASTE_SESSION
        .lock()
        .map_err(|_| "Streaming clipboard session lock poisoned".to_string())?;
    if !guard
        .as_ref()
        .is_some_and(|session| session.operation_id == operation_id)
    {
        return Ok(false);
    }

    let Some(session) = guard.take() else {
        return Ok(false);
    };
    #[cfg(target_os = "windows")]
    let mut session = session;
    wait_for_clipboard_consumer(session.last_clipboard_paste_sent_at);

    let clipboard_handling = session.clipboard_handling;
    let exclude_final_from_history = session.exclude_final_from_history;
    let final_text = convert_text_for_clipboard(final_text, session.convert_lf_to_crlf);
    #[cfg(target_os = "windows")]
    let final_write_result = {
        let expected = if restores_clipboard_contents(clipboard_handling) {
            let Some(sequence) = session.last_clipboard_sequence else {
                return Ok(true);
            };
            Some(sequence)
        } else {
            None
        };
        match win_clipboard::write_text_if_unchanged(&final_text, exclude_final_from_history, expected) {
            Ok(Some(sequence)) => {
                session.last_clipboard_sequence = Some(sequence);
                Ok(())
            }
            Ok(None) => return Ok(true),
            Err(error) => Err(error),
        }
    };
    #[cfg(not(target_os = "windows"))]
    let final_write_result =
        write_final_transcription_text(app_handle, &final_text, exclude_final_from_history);
    if final_write_result.is_ok() {
        wait_for_clipboard_history_capture(exclude_final_from_history, clipboard_handling);
    }

    // Always attempt restoration, even if the final history write failed.
    let restore_result = restore_streaming_session_backup(session, app_handle);
    match (final_write_result, restore_result) {
        (Ok(()), Ok(())) => Ok(true),
        (Err(write_error), Ok(())) => Err(format!(
            "Failed to write final streaming transcription to clipboard: {}",
            write_error
        )),
        (Ok(()), Err(restore_error)) => Err(restore_error),
        (Err(write_error), Err(restore_error)) => Err(format!(
            "Failed to write final streaming transcription to clipboard: {}; clipboard restoration also failed: {}",
            write_error, restore_error
        )),
    }
}

/// Pastes text using the clipboard: saves current content, writes text, sends paste keystroke, restores clipboard.
fn paste_via_clipboard_guarded(
    enigo: &mut LazyEnigo,
    text: &str,
    app_handle: &AppHandle,
    paste_method: &PasteMethod,
    paste_delay_ms: u64,
    convert_lf_to_crlf: bool,
    clipboard_handling: ClipboardHandling,
    exclude_final_from_history: bool,
    can_continue: &impl Fn() -> bool,
) -> Result<bool, String> {
    let clipboard = app_handle.clipboard();

    // Backup clipboard content based on handling mode.
    #[cfg(target_os = "windows")]
    let advanced_backup = if restores_all_clipboard_formats(clipboard_handling) {
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
    // - RestorePlainText mode (existing behavior)
    // - RestoreAdvanced fallback when rich-format restore is partial/failed
    let text_backup = if matches!(
        clipboard_handling,
        ClipboardHandling::RestorePlainText
            | ClipboardHandling::RestoreAdvanced
            | ClipboardHandling::RestoreAdvancedOwned
    ) {
        clipboard.read_text().unwrap_or_default()
    } else {
        String::new()
    };
    #[cfg(not(target_os = "windows"))]
    let image_backup = backup_clipboard_image(app_handle, &text_backup, clipboard_handling);

    // The non-streaming paste write is also the final write.
    let paste_result = paste_via_clipboard_no_restore_guarded(
        enigo,
        text,
        app_handle,
        paste_method,
        paste_delay_ms,
        convert_lf_to_crlf,
        exclude_final_from_history,
        can_continue,
    );
    if let Ok(Some(paste_sent_at)) = &paste_result {
        wait_for_clipboard_consumer(Some(*paste_sent_at));
    }

    // Restore clipboard based on handling mode.
    #[cfg(target_os = "windows")]
    if let Some(backup) = advanced_backup {
        let restore_result = restore_advanced_clipboard_with_text_fallback(
            app_handle,
            backup,
            &text_backup,
            uses_documented_clipboard_owner(clipboard_handling),
            None,
        );
        return delivery_after_clipboard_cleanup(
            paste_result.map(|sent_at| sent_at.is_some()), restore_result,
            |error| warn!("Clipboard restoration failed after paste attempt: {}", error),
        );
    }

    // Text-only restore for RestorePlainText and as the fallback when an advanced
    // backup could not be created (including non-Windows platforms).
    let restore_result = if matches!(
        clipboard_handling,
        ClipboardHandling::RestorePlainText
            | ClipboardHandling::RestoreAdvanced
            | ClipboardHandling::RestoreAdvancedOwned
    ) {
        #[cfg(target_os = "windows")]
        {
            restore_plain_text_without_history(app_handle, &text_backup)
        }
        #[cfg(not(target_os = "windows"))]
        {
            restore_clipboard_backup(app_handle, &text_backup, &image_backup)
        }
    } else {
        Ok(())
    };

    delivery_after_clipboard_cleanup(
        paste_result.map(|sent_at| sent_at.is_some()), restore_result,
        |error| warn!("Clipboard restoration failed after paste attempt: {}", error),
    )
}

/// Attempts to send a key combination using Linux-native tools.
/// Returns `Ok(true)` if a native tool handled it, `Ok(false)` to fall back to enigo.
#[cfg(target_os = "linux")]
fn try_send_key_combo_linux(paste_method: &PasteMethod) -> Result<bool, String> {
    if is_wayland() {
        // Wayland: prefer wtype (but not on KDE or GNOME), then dotool, then ydotool
        // Note: wtype doesn't work on KDE (no zwp_virtual_keyboard_manager_v1 support)
        // or on GNOME/Mutter (same reason — Mutter deliberately does not implement
        // the virtual-keyboard-v1 protocol).
        if !is_kde_wayland() && !is_gnome_wayland() && is_wtype_available() {
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
fn try_direct_typing_linux(text: &str, preferred_tool: TypingTool) -> Result<bool, String> {
    // If user specified a tool, try only that one
    if preferred_tool != TypingTool::Auto {
        return match preferred_tool {
            TypingTool::Wtype if is_wtype_available() => {
                info!("Using user-specified wtype");
                type_text_via_wtype(text)?;
                Ok(true)
            }
            TypingTool::Kwtype if is_kwtype_available() => {
                info!("Using user-specified kwtype");
                type_text_via_kwtype(text)?;
                Ok(true)
            }
            TypingTool::Dotool if is_dotool_available() => {
                info!("Using user-specified dotool");
                type_text_via_dotool(text)?;
                Ok(true)
            }
            TypingTool::Ydotool if is_ydotool_available() => {
                info!("Using user-specified ydotool");
                type_text_via_ydotool(text)?;
                Ok(true)
            }
            TypingTool::Xdotool if is_xdotool_available() => {
                info!("Using user-specified xdotool");
                type_text_via_xdotool(text)?;
                Ok(true)
            }
            _ => Err(format!(
                "Typing tool {:?} is not available on this system",
                preferred_tool
            )),
        };
    }

    // Auto mode - existing fallback chain
    if is_wayland() {
        // KDE Wayland: prefer kwtype (uses KDE Fake Input protocol, supports umlauts)
        if is_kde_wayland() && is_kwtype_available() {
            info!("Using kwtype for direct text input on KDE Wayland");
            type_text_via_kwtype(text)?;
            return Ok(true);
        }
        // Wayland: prefer wtype, then dotool, then ydotool
        // Note: wtype doesn't work on KDE (no zwp_virtual_keyboard_manager_v1 support)
        // or on GNOME/Mutter (same reason — Mutter deliberately does not implement
        // the virtual-keyboard-v1 protocol).
        if !is_kde_wayland() && !is_gnome_wayland() && is_wtype_available() {
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

/// Returns the list of available typing tools on this system.
/// Always includes "auto" as the first entry.
#[cfg(target_os = "linux")]
pub fn get_available_typing_tools() -> Vec<String> {
    let mut tools = vec!["auto".to_string()];
    if is_wtype_available() {
        tools.push("wtype".to_string());
    }
    if is_kwtype_available() {
        tools.push("kwtype".to_string());
    }
    if is_dotool_available() {
        tools.push("dotool".to_string());
    }
    if is_ydotool_available() {
        tools.push("ydotool".to_string());
    }
    if is_xdotool_available() {
        tools.push("xdotool".to_string());
    }
    tools
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

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum YdotoolKeySyntax {
    Symbolic,
    RawKeycodes,
}

#[cfg(target_os = "linux")]
const YDOTOOL_UNKNOWN_HELP_FALLBACK: YdotoolKeySyntax = YdotoolKeySyntax::RawKeycodes;

#[cfg(target_os = "linux")]
static YDOTOOL_KEY_SYNTAX: OnceLock<YdotoolKeySyntax> = OnceLock::new();

/// Classifies `ydotool key --help` output without relying on version or distro metadata.
#[cfg(target_os = "linux")]
fn classify_ydotool_key_syntax(help: &str) -> Option<YdotoolKeySyntax> {
    let help = help.to_ascii_lowercase();

    // Check modern markers first in case a future help message mentions the legacy syntax.
    if help.contains("syntax: <keycode>:<pressed>")
        || help.contains("[keycodes]")
        || help.contains("using raw keycodes")
    {
        Some(YdotoolKeySyntax::RawKeycodes)
    } else if help.contains("separated by plus (+)")
        || (help.contains("<key sequence>") && help.contains("alt+r"))
    {
        Some(YdotoolKeySyntax::Symbolic)
    } else {
        None
    }
}

/// Detects and caches a recognized ydotool key syntax. Unknown or failed probes are not cached,
/// allowing a transient daemon or PATH problem to recover on a later paste attempt.
#[cfg(target_os = "linux")]
fn detect_ydotool_key_syntax() -> YdotoolKeySyntax {
    if let Some(syntax) = YDOTOOL_KEY_SYNTAX.get() {
        return *syntax;
    }

    match Command::new("ydotool").args(["key", "--help"]).output() {
        Ok(output) => {
            // ydotool 0.x writes help to stderr and its exit status varies by build.
            let mut help = String::from_utf8_lossy(&output.stdout).into_owned();
            if !help.is_empty() && !output.stderr.is_empty() {
                help.push('\n');
            }
            help.push_str(&String::from_utf8_lossy(&output.stderr));

            if let Some(syntax) = classify_ydotool_key_syntax(&help) {
                *YDOTOOL_KEY_SYNTAX.get_or_init(|| {
                    info!("Detected ydotool key syntax: {:?}", syntax);
                    syntax
                })
            } else {
                // Preserve Handy's existing behavior and compatibility with current ydotool.
                log::warn!(
                    "Could not recognize ydotool key --help output (exit status {:?}); using raw-keycode syntax",
                    output.status.code()
                );
                YDOTOOL_UNKNOWN_HELP_FALLBACK
            }
        }
        Err(error) => {
            log::warn!(
                "Could not query ydotool key syntax: {}; using raw-keycode syntax",
                error
            );
            YDOTOOL_UNKNOWN_HELP_FALLBACK
        }
    }
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

/// Check if kwtype is available (KDE Wayland virtual keyboard input tool)
#[cfg(target_os = "linux")]
fn is_kwtype_available() -> bool {
    Command::new("which")
        .arg("kwtype")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check if wl-copy is available (Wayland clipboard tool)
#[cfg(target_os = "linux")]
fn is_wl_copy_available() -> bool {
    Command::new("which")
        .arg("wl-copy")
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

    // `--clearmodifiers` restores the modifiers that were held when xdotool
    // started. If the user releases one while xdotool is typing, that synthetic
    // restore can leave the modifier latched on the XTEST keyboard (#1817).
    // Release both sides of Handy's supported push-style modifiers to clear any
    // stale restore. Lock keys are intentionally excluded because key events
    // toggle them.
    //
    // The release is unconditional, so it can make a modifier that is still
    // physically held appear released until the next physical event. This is
    // preferable to leaving a synthetic modifier latched system-wide.
    //
    // Clean up before checking the typing status because xdotool may have
    // changed modifier state before returning an error. Cleanup remains
    // best-effort because the text may already have been partially or fully
    // typed, but failures are logged so they can be diagnosed.
    match Command::new("xdotool")
        .arg("keyup")
        .args([
            "Control_L",
            "Control_R",
            "Shift_L",
            "Shift_R",
            "Alt_L",
            "Alt_R",
            "Super_L",
            "Super_R",
        ])
        .output()
    {
        Ok(cleanup_output) if !cleanup_output.status.success() => {
            let stderr = String::from_utf8_lossy(&cleanup_output.stderr);
            log::warn!(
                "xdotool modifier cleanup failed with status {:?}: {}",
                cleanup_output.status.code(),
                stderr.trim()
            );
        }
        Err(error) => log::warn!("Failed to execute xdotool modifier cleanup: {}", error),
        _ => {}
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("xdotool failed: {}", stderr));
    }

    Ok(())
}

/// dotool reads one command per line, so text containing a newline would run its
/// later lines as commands. Type each line on its own and press Enter between them.
#[cfg(target_os = "linux")]
fn dotool_script(text: &str) -> String {
    let mut script = String::new();
    for (index, line) in text.replace('\r', "").split('\n').enumerate() {
        if index > 0 {
            script.push_str("key enter\n");
        }
        if !line.is_empty() {
            script.push_str("type ");
            script.push_str(line);
            script.push('\n');
        }
    }
    script
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
        stdin
            .write_all(dotool_script(text).as_bytes())
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

/// Type text directly via kwtype (KDE Wayland virtual keyboard, uses KDE Fake Input protocol).
#[cfg(target_os = "linux")]
fn type_text_via_kwtype(text: &str) -> Result<(), String> {
    let output = Command::new("kwtype")
        .arg("--")
        .arg(text)
        .output()
        .map_err(|e| format!("Failed to execute kwtype: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("kwtype failed: {}", stderr));
    }

    Ok(())
}

/// Write text to clipboard via wl-copy (Wayland clipboard tool).
/// Uses Stdio::null() to avoid blocking on repeated calls — wl-copy forks a
/// daemon that inherits piped fds, causing read_to_end to hang indefinitely.
#[cfg(target_os = "linux")]
fn write_clipboard_via_wl_copy(text: &str) -> Result<(), String> {
    use std::process::Stdio;
    let status = Command::new("wl-copy")
        .arg("--")
        .arg(text)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to execute wl-copy: {}", e))?;

    if !status.success() {
        return Err("wl-copy failed".into());
    }

    Ok(())
}

/// Send a key combination (e.g., Ctrl+V) via wtype on Wayland.
#[cfg(target_os = "linux")]
fn send_key_combo_via_wtype(paste_method: &PasteMethod) -> Result<(), String> {
    let args: Vec<&str> = match paste_method {
        PasteMethod::CtrlV => vec!["-M", "ctrl", "-k", "v", "-m", "ctrl"],
        PasteMethod::ShiftInsert => vec!["-M", "shift", "-k", "Insert", "-m", "shift"],
        PasteMethod::CtrlShiftV => vec![
            "-M", "ctrl", "-M", "shift", "-k", "v", "-m", "shift", "-m", "ctrl",
        ],
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
    use std::process::Stdio;
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to execute dotool: {}", e))?;
    if !status.success() {
        return Err("dotool failed".into());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn ydotool_key_args(
    paste_method: &PasteMethod,
    syntax: YdotoolKeySyntax,
) -> Result<&'static [&'static str], String> {
    let args = match (paste_method, syntax) {
        (PasteMethod::CtrlV, YdotoolKeySyntax::Symbolic) => &["key", "ctrl+v"][..],
        (PasteMethod::CtrlShiftV, YdotoolKeySyntax::Symbolic) => &["key", "ctrl+shift+v"][..],
        (PasteMethod::ShiftInsert, YdotoolKeySyntax::Symbolic) => &["key", "shift+insert"][..],
        (PasteMethod::CtrlV, YdotoolKeySyntax::RawKeycodes) => {
            &["key", "29:1", "47:1", "47:0", "29:0"][..]
        }
        (PasteMethod::CtrlShiftV, YdotoolKeySyntax::RawKeycodes) => {
            &["key", "29:1", "42:1", "47:1", "47:0", "42:0", "29:0"][..]
        }
        (PasteMethod::ShiftInsert, YdotoolKeySyntax::RawKeycodes) => {
            &["key", "42:1", "110:1", "110:0", "42:0"][..]
        }
        _ => return Err("Unsupported paste method".into()),
    };

    Ok(args)
}

/// Send a key combination (e.g., Ctrl+V) via ydotool (requires ydotoold daemon).
#[cfg(target_os = "linux")]
fn send_key_combo_via_ydotool(paste_method: &PasteMethod) -> Result<(), String> {
    let syntax = detect_ydotool_key_syntax();
    let args = ydotool_key_args(paste_method, syntax)?;

    let output = Command::new("ydotool")
        .args(args)
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

/// Longest an external paste script may run before it is killed.
const EXTERNAL_SCRIPT_TIMEOUT: Duration = Duration::from_millis(crate::settings::EXTERNAL_SCRIPT_TIMEOUT_MS);

/// Pastes text by invoking an external script.
/// The script receives the text to paste as a single argument.
fn paste_via_external_script(
    text: &str,
    script_path: &str,
    timeout: Duration,
    can_continue: impl Fn() -> bool,
) -> Result<bool, String> {
    info!("Pasting via external script: {}", script_path);

    // Do not capture the script's stdio. Wayland clipboard helpers such as
    // wl-copy may fork a background selection daemon that inherits those file
    // descriptors; waiting for captured output would then block until the
    // clipboard selection is replaced instead of returning after the script
    // itself exits.
    use std::process::Stdio;
    if !can_continue() { return Ok(false); }
    let mut command = Command::new(script_path);
    command.arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()
        .map_err(|e| format!("Failed to execute external script '{}': {}", script_path, e))?;

    // A hung script must not block output (and the input system) forever.
    let deadline = Instant::now() + timeout;
    let status = loop {
        if !can_continue() || crate::output_dispatch::is_cancelled() {
            terminate_external_script(&mut child);
            return Ok(false);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                terminate_external_script(&mut child);
                return Err(format!(
                    "External script '{}' did not finish within {} seconds",
                    script_path,
                    timeout.as_secs_f64()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                terminate_external_script(&mut child);
                return Err(format!(
                    "Failed to wait for external script '{}': {}",
                    script_path, e
                ))
            }
        }
    };

    if !status.success() {
        return Err(format!(
            "External script '{}' failed with exit code {:?}",
            script_path,
            status.code()
        ));
    }

    Ok(true)
}

fn terminate_external_script(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The script may have spawned a delayed paste helper. Kill the whole
        // process group so it cannot insert after cancellation or a timeout.
        unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL); }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Types text directly by simulating individual key presses.
fn paste_direct(enigo: &mut LazyEnigo, app_handle: &AppHandle, text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        if try_direct_typing_linux(text, get_settings(app_handle).typing_tool)? {
            return Ok(());
        }
        info!("Falling back to enigo for direct text input");
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app_handle;

    input::paste_text_direct(enigo.get()?, text)
}

/// Hands text to the user's configured external paste script.
fn paste_external_script(app_handle: &AppHandle, text: &str, can_continue: impl Fn() -> bool) -> Result<bool, String> {
    let script_path = get_settings(app_handle)
        .external_script_path
        .filter(|path| !path.trim().is_empty())
        .ok_or("External script path is not configured")?;
    paste_via_external_script(text, &script_path, EXTERNAL_SCRIPT_TIMEOUT, can_continue)
}

fn send_return_key(enigo: &mut LazyEnigo, key_type: AutoSubmitKey) -> Result<(), String> {
    send_output_key(enigo, match key_type {
        AutoSubmitKey::Enter => "enter",
        AutoSubmitKey::CtrlEnter => "ctrl+enter",
        AutoSubmitKey::CmdEnter => "meta+enter",
    })
}

fn should_send_auto_submit(auto_submit: bool, paste_method: PasteMethod) -> bool {
    auto_submit && paste_method != PasteMethod::None
}

pub(crate) fn output_foreground_window() -> Option<usize> {
    #[cfg(target_os = "windows")]
    {
        let hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        (!hwnd.0.is_null()).then_some(hwnd.0 as usize)
    }
    #[cfg(not(target_os = "windows"))]
    { None }
}

fn output_target_is_current(target_window: Option<usize>, current_window: Option<usize>) -> bool {
    (!cfg!(target_os = "windows") || target_window.is_some())
        && target_window == current_window
}

fn output_target_allows_delivery(
    check_active_window: bool,
    target_window: Option<usize>,
    current_window: impl FnOnce() -> Option<usize>,
) -> bool {
    !check_active_window || output_target_is_current(target_window, current_window())
}

fn wait_for_output_delay(delay_ms: u64, can_continue: &impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_millis(delay_ms.min(5000));
    while Instant::now() < deadline {
        if !can_continue() { return false; }
        std::thread::sleep(deadline.saturating_duration_since(Instant::now()).min(Duration::from_millis(10)));
    }
    can_continue()
}

fn parse_output_key(value: &str) -> Result<(Vec<Key>, Key), String> {
    let parts: Vec<_> = value.split('+').map(str::trim).collect();
    let (last, modifiers) = parts.split_last().ok_or("An opening key is required")?;
    let mut keys = Vec::new();
    for modifier in modifiers {
        let key = match *modifier {
            "ctrl" | "control" => Key::Control,
            "shift" => Key::Shift,
            "alt" | "option" => Key::Alt,
            "win" | "super" | "meta" | "command" | "cmd" => Key::Meta,
            _ => return Err(format!("Unsupported opening-key modifier: {}", modifier)),
        };
        if !keys.contains(&key) { keys.push(key); }
    }
    let key = match *last {
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "escape" | "esc" => Key::Escape,
        "backspace" => Key::Backspace,
        "capslock" => Key::CapsLock,
        "delete" => Key::Delete,
        #[cfg(not(target_os = "macos"))]
        "insert" => Key::Insert,
        #[cfg(not(target_os = "macos"))]
        "printscreen" => Key::PrintScr,
        #[cfg(not(target_os = "macos"))]
        "pause" => Key::Pause,
        #[cfg(not(target_os = "macos"))]
        "numlock" => Key::Numlock,
        #[cfg(target_os = "windows")]
        "scrolllock" => Key::Scroll,
        #[cfg(target_os = "linux")]
        "scrolllock" => Key::ScrollLock,
        #[cfg(target_os = "windows")]
        "menu" => Key::Apps,
        #[cfg(target_os = "linux")]
        "menu" => Key::LMenu,
        "numpad 0" => Key::Numpad0, "numpad 1" => Key::Numpad1,
        "numpad 2" => Key::Numpad2, "numpad 3" => Key::Numpad3,
        "numpad 4" => Key::Numpad4, "numpad 5" => Key::Numpad5,
        "numpad 6" => Key::Numpad6, "numpad 7" => Key::Numpad7,
        "numpad 8" => Key::Numpad8, "numpad 9" => Key::Numpad9,
        "numpad *" => Key::Multiply,
        "numadd" => Key::Add,
        "numpad -" => Key::Subtract,
        "numpad ." => Key::Decimal,
        "numpad /" => Key::Divide,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" | "arrowup" => Key::UpArrow,
        "down" | "arrowdown" => Key::DownArrow,
        "left" | "arrowleft" => Key::LeftArrow,
        "right" | "arrowright" => Key::RightArrow,
        "f1" => Key::F1, "f2" => Key::F2, "f3" => Key::F3, "f4" => Key::F4,
        "f5" => Key::F5, "f6" => Key::F6, "f7" => Key::F7, "f8" => Key::F8,
        "f9" => Key::F9, "f10" => Key::F10, "f11" => Key::F11, "f12" => Key::F12,
        "f13" => Key::F13, "f14" => Key::F14, "f15" => Key::F15, "f16" => Key::F16,
        "f17" => Key::F17, "f18" => Key::F18, "f19" => Key::F19, "f20" => Key::F20,
        #[cfg(not(target_os = "macos"))]
        "f21" => Key::F21,
        #[cfg(not(target_os = "macos"))]
        "f22" => Key::F22,
        #[cfg(not(target_os = "macos"))]
        "f23" => Key::F23,
        #[cfg(not(target_os = "macos"))]
        "f24" => Key::F24,
        // Browser capture identifies the physical punctuation key. Preserve
        // that key on Windows so games receive a key event, not Unicode input.
        name if matches!(name, ";" | "=" | "," | "-" | "." | "/" | "`" | "[" | "\\" | "]" | "'") => {
            #[cfg(target_os = "windows")]
            {
                let virtual_key = match name {
                    ";" => 0xBA, "=" => 0xBB, "," => 0xBC, "-" => 0xBD,
                    "." => 0xBE, "/" => 0xBF, "`" => 0xC0, "[" => 0xDB,
                    "\\" => 0xDC, "]" => 0xDD, "'" => 0xDE,
                    _ => unreachable!(),
                };
                Key::Other(virtual_key)
            }
            #[cfg(not(target_os = "windows"))]
            { Key::Unicode(name.chars().next().unwrap()) }
        }
        name if name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric() => {
            let character = name.chars().next().unwrap();
            #[cfg(target_os = "windows")]
            { Key::Other(character.to_ascii_uppercase() as u32) }
            #[cfg(not(target_os = "windows"))]
            { Key::Unicode(character) }
        }
        _ => return Err(format!("Unsupported opening key: {}", last)),
    };
    Ok((keys, key))
}

pub(crate) fn validate_output_key(value: &str) -> Result<(), String> {
    parse_output_key(value).map(|_| ())
}

fn send_output_key(enigo: &mut LazyEnigo, value: &str) -> Result<(), String> {
    let (modifiers, key) = parse_output_key(value)?;
    let enigo = enigo.get()?;
    let mut pressed = Vec::new();
    let result = (|| {
        for modifier in modifiers {
            enigo.key(modifier, Direction::Press).map_err(|error| error.to_string())?;
            pressed.push(modifier);
        }
        enigo.key(key, Direction::Click).map_err(|error| error.to_string())
    })();
    let mut release_error = None;
    for modifier in pressed.into_iter().rev() {
        if let Err(error) = enigo.key(modifier, Direction::Release) {
            release_error = Some(error.to_string());
        }
    }
    result.and_then(|_| release_error.map_or(Ok(()), Err))
}

/// Presses a validated key combination with the managed keyboard (opening/cancel keys).
pub(crate) fn press_output_key_now(app: &AppHandle, value: &str) -> Result<(), String> {
    let _output_guard = lock_output();
    send_output_key(&mut LazyEnigo::new(app), value)
}

/// After-insertion key for a recording whose opening key was pressed at start. The text
/// went in live, so this runs only when all of it was delivered and the key's window is
/// still in front.
pub(crate) fn send_early_session_submit(
    app: &AppHandle,
    settings: &ResolvedOutputSettings,
    target_window: Option<usize>,
    can_continue: &impl Fn() -> bool,
) {
    let target_is_current = || output_target_allows_delivery(
        settings.check_active_window, target_window, output_foreground_window);
    let output_can_continue = || can_continue() && target_is_current();
    if !should_send_auto_submit(settings.auto_submit, settings.paste_method)
        || !output_can_continue()
        || !wait_for_output_delay(settings.auto_submit_delay_ms, &output_can_continue)
    {
        return;
    }
    let _output_guard = lock_output();
    let mut enigo = LazyEnigo::new(app);
    if let Err(error) = send_return_key(&mut enigo, settings.auto_submit_key) {
        warn!("Text was inserted, but the submit key failed: {}", error);
    }
}

/// After a background finalization released the streaming clipboard session, the
/// whitespace goes through the fallback paste. It must not replace the clipboard there.
fn trailing_whitespace_output(settings: &ResolvedOutputSettings) -> ResolvedOutputSettings {
    let mut output = settings.clone();
    if !restores_clipboard_contents(output.clipboard_handling) {
        output.clipboard_handling = ClipboardHandling::RestorePlainText;
    }
    output
}

/// Called once at the final delivery boundary, after all queued live chunks.
pub(crate) fn complete_dictation_output(
    app: &AppHandle,
    operation_id: u64,
    final_text: &str,
    can_continue: impl Fn() -> bool,
) -> Result<bool, String> {
    // Consume the configuration before delivery so every return path,
    // including an interrupted delay or clipboard error, releases it.
    let config = lock_dictation_output_sessions().take(operation_id);
    let Some(DictationOutputSession { settings, mut delivered, failed, mut held_whitespace, .. }) = config else {
        return finalize_streaming_paste_session_if_matches(app, operation_id, final_text);
    };
    if !can_continue() || final_text.trim().is_empty() {
        // Any held trailing whitespace is dropped with the session.
        crate::live_output_buffer::cancel_unfinished(operation_id);
        end_streaming_paste_session_if_matches(app, operation_id)?;
        return Ok(false);
    }
    if settings.buffers_external_output() {
        return paste_with_output_settings(final_text.to_string(), app.clone(), &settings, can_continue);
    }
    // Opening key pressed at recording start: live text went into the opened chat. Only
    // its delay and the after-insertion key remain. If the key was never pressed, nothing
    // was inserted and the text stays in History.
    let early_press = match settings.early_opening_key.state() {
        crate::early_open::EarlyKeyState::Idle => None,
        crate::early_open::EarlyKeyState::Pressed { at, window } => Some((at, window)),
        state => {
            warn!("Live output skipped: the opening key was not pressed at recording start ({:?})", state);
            crate::live_output_buffer::cancel_unfinished(operation_id);
            end_streaming_paste_session_if_matches(app, operation_id)?;
            return Err(OPENING_KEY_NOT_PRESSED_ERROR.to_string());
        }
    };
    if let Some((at, _)) = early_press {
        // Held live text is released only after the key's delay; wait it out before `finish`.
        let remaining = crate::early_open::remaining_delay_ms(settings.recording_start_delay_ms, at.elapsed());
        if !wait_for_output_delay(remaining, &can_continue) {
            crate::live_output_buffer::cancel_unfinished(operation_id);
            end_streaming_paste_session_if_matches(app, operation_id)?;
            return Ok(false);
        }
    }
    // Same route as chunks, and before `finish`, so ordering and window protection hold.
    let trailing = held_whitespace.resolve(settings.trailing_whitespace_mode);
    if !trailing.is_empty() && settings.paste_method != PasteMethod::None {
        let protected = crate::live_output_buffer::enabled(operation_id)
            && crate::live_output_buffer::enqueue(app, operation_id, trailing.clone()).is_some();
        if !protected {
            // Only trailing whitespace: the dictated text itself is already in, so a failure
            // here is logged and does not mark the dictation as undelivered.
            let output = trailing_whitespace_output(&settings);
            match paste_stream_chunk_with_output(trailing, app.clone(), Some(&output), Some(operation_id), None) {
                Ok(()) => delivered = true,
                Err(error) => warn!("Trailing whitespace after live dictation was not inserted: {}", error),
            }
        }
    }
    // A failed chunk leaves the target incomplete. Still publish the complete
    // transcript so the clipboard/history fallback holds the full text.
    let cleanup_result = finalize_streaming_paste_session_if_matches(app, operation_id, final_text)
        .map(|_| ());
    let live_finish = crate::live_output_buffer::finish_detailed(app, operation_id);
    let retained_live_output = live_finish.is_some();
    let delivered_to_target = retained_live_output || streaming_delivery_succeeded(
        delivered, failed, settings.paste_method, can_continue());
    let result = delivery_after_clipboard_cleanup(Ok(delivered_to_target), cleanup_result, |error| {
        warn!("Streaming dictation final clipboard update or restoration failed after chunk delivery: {}", error);
    });
    if let Some((_, window)) = early_press {
        settings.early_opening_key.mark_completed();
        // Pressed only after the trailing-whitespace tail above was delivered, and only if
        // everything reached the window that received the opening key.
        if live_finish.unwrap_or(!failed) {
            send_early_session_submit(app, &settings, window, &can_continue);
        }
    }
    result
}

pub fn paste(text: String, app_handle: AppHandle) -> Result<(), String> {
    paste_with_method_override(text, app_handle, None)
}

/// Override only this paste operation, without changing saved settings.
pub(crate) fn paste_with_method_override(
    text: String,
    app_handle: AppHandle,
    method: Option<PasteMethod>,
) -> Result<(), String> {
    let settings = get_settings(&app_handle);
    let mut output = ResolvedOutputSettings::resolve(&settings, None);
    output.paste_method = method.unwrap_or(settings.paste_method);
    // Generic pastes (including selection restoration) do not open chat.
    output.before_insert_key.clear();
    output.recording_start_key.clear();
    if text.is_empty() {
        return Ok(());
    }
    if !paste_with_output_settings(text, app_handle, &output, || true)? {
        return Err("Paste was cancelled because the target window changed or could not be identified".to_string());
    }
    Ok(())
}

/// Returns false when the text did not reach the target. `PasteMethod::None`
/// completes without inserting, matching the legacy successful paste result.
pub(crate) fn paste_with_output_settings(
    text: String,
    app_handle: AppHandle,
    settings: &ResolvedOutputSettings,
    can_continue: impl Fn() -> bool,
) -> Result<bool, String> {
    if text.is_empty() || !can_continue() {
        return Ok(false);
    }
    if settings.paste_method == PasteMethod::None {
        info!("PasteMethod::None selected - skipping paste action");
        return Ok(true);
    }
    // Opening key pressed at recording start: it is not pressed again, the target is the
    // window that received it, and only the rest of its delay is waited. A key that was
    // never pressed (failed, or the recording ended first) means nothing is inserted.
    let early_press = match settings.early_opening_key.state() {
        crate::early_open::EarlyKeyState::Idle => None,
        crate::early_open::EarlyKeyState::Pressed { at, window } => Some((at, window)),
        state => {
            warn!("Insertion skipped: the opening key was not pressed at recording start ({:?})", state);
            return Err(OPENING_KEY_NOT_PRESSED_ERROR.to_string());
        }
    };
    let paste_method = settings.paste_method;
    let clipboard_handling = settings.clipboard_handling;
    let exclude_final_from_history = exclude_final_write_from_history(settings);
    let paste_delay_ms = settings.paste_delay_ms;

    info!(
        "Using paste method: {:?}, clipboard handling: {:?}, delay: {}ms",
        paste_method, clipboard_handling, paste_delay_ms
    );

    let _clipboard_guard = matches!(
        paste_method,
        PasteMethod::CtrlV | PasteMethod::CtrlShiftV | PasteMethod::ShiftInsert
    )
    .then(|| lock_clipboard_transaction("pasting text"))
    .transpose()?;

    // The input system is taken lazily, on the first key press that needs it.
    let _output_guard = lock_output();
    let mut enigo = LazyEnigo::new(&app_handle);

    let target_window = match early_press {
        Some((_, window)) => window,
        None => settings.check_active_window.then(output_foreground_window).flatten(),
    };
    let target_is_current = || output_target_allows_delivery(
        settings.check_active_window, target_window, output_foreground_window);
    let output_can_continue = || can_continue() && target_is_current();
    if let Some((at, _)) = early_press {
        let remaining = crate::early_open::remaining_delay_ms(settings.recording_start_delay_ms, at.elapsed());
        if !wait_for_output_delay(remaining, &output_can_continue) { return Ok(false); }
    }
    if !settings.before_insert_key.is_empty() {
        if !output_can_continue() { return Ok(false); }
        send_output_key(&mut enigo, &settings.before_insert_key)?;
        if !wait_for_output_delay(settings.before_insert_delay_ms, &output_can_continue) {
            return Ok(false);
        }
    }
    if !output_can_continue() { return Ok(false); }
    // Perform the paste operation
    match paste_method {
        PasteMethod::None => {
            info!("PasteMethod::None selected - skipping paste action");
        }
        PasteMethod::Direct => {
            paste_direct(&mut enigo, &app_handle, &text)?;
        }
        PasteMethod::ExternalScript => {
            if !paste_external_script(&app_handle, &text, &output_can_continue)? {
                return Ok(false);
            }
        }
        PasteMethod::CtrlV | PasteMethod::CtrlShiftV | PasteMethod::ShiftInsert => {
            if !paste_via_clipboard_guarded(
                &mut enigo,
                &text,
                &app_handle,
                &paste_method,
                paste_delay_ms,
                settings.convert_lf_to_crlf,
                clipboard_handling,
                exclude_final_from_history,
                &output_can_continue,
            )? {
                return Ok(false);
            }
        }
    }

    settings.early_opening_key.mark_completed();
    if should_send_auto_submit(settings.auto_submit, paste_method) {
        if !wait_for_output_delay(settings.auto_submit_delay_ms, &output_can_continue) {
            // Text already reached the target. Do not retain a preview buffer
            // that could insert the same text again after a cancelled submit.
            return Ok(true);
        }
        if let Err(error) = send_return_key(&mut enigo, settings.auto_submit_key) {
            // Delivery succeeded: retaining Preview here would duplicate text
            // on the next insertion attempt.
            warn!("Text was inserted, but the submit key failed: {}", error);
        }
    }

    Ok(true)
}

/// Pastes a streaming chunk without appending trailing space and without
/// restoring clipboard contents until the session ends.
pub fn paste_stream_chunk(text: String, app_handle: AppHandle) -> Result<(), String> {
    let output = current_dictation_output();
    paste_stream_chunk_for_output(text, app_handle, output, None)
}

/// Deliver a queued chunk using only the settings captured by its recording.
/// A missing session must not fall back to current settings or a newer profile.
pub(crate) fn paste_stream_chunk_for_operation(
    text: String,
    app_handle: AppHandle,
    operation_id: u64,
) -> Result<(), String> {
    let output = lock_dictation_output_sessions().output_for_chunk(Some(operation_id))
        .ok_or_else(|| format!("Dictation output settings for operation {} are no longer available", operation_id))?;
    paste_stream_chunk_for_output(text, app_handle, Some(output), Some(operation_id))
}

fn paste_stream_chunk_for_output(
    text: String,
    app_handle: AppHandle,
    output: Option<(u64, ResolvedOutputSettings)>,
    operation_id: Option<u64>,
) -> Result<(), String> {
    if output.as_ref().is_some_and(|(_, settings)| settings.buffers_external_output()) {
        return Ok(());
    }
    // Opening key pressed at start: chunks wait in the live buffer for the key and its
    // delay. If the key failed or the recording ended, nothing is inserted (History keeps it).
    if let Some((operation_id, settings)) = output.as_ref() {
        match settings.early_opening_key.state() {
            crate::early_open::EarlyKeyState::Idle | crate::early_open::EarlyKeyState::Pressed { .. } => {}
            crate::early_open::EarlyKeyState::Waiting
                if crate::live_output_buffer::enabled(*operation_id) => {}
            _ => return Ok(()),
        }
    }
    // Hold back trailing whitespace before any protected-window redirect.
    // Without a session there is no finalizer, so the chunk is inserted whole.
    let text = match output.as_ref() {
        Some((operation_id, settings)) => match lock_dictation_output_sessions().get_mut(*operation_id) {
            Some(session) => session.held_whitespace.push(settings.trailing_whitespace_mode, text),
            None => text,
        },
        None => text,
    };
    if text.is_empty() {
        return Ok(());
    }
    if let Some((operation_id, _)) = output.as_ref() {
        if crate::live_output_buffer::enabled(*operation_id) {
            if let Some(result) = crate::live_output_buffer::enqueue(&app_handle, *operation_id, text.clone()) {
                return result;
            }
        }
    }
    let has_text = !text.is_empty();
    let result = paste_stream_chunk_with_output(text, app_handle, output.as_ref().map(|(_, settings)| settings), operation_id, None);
    if let Some((operation_id, settings)) = output {
        if let Some(session) = lock_dictation_output_sessions().get_mut(operation_id) {
            session.failed |= result.is_err();
            session.delivered |= result.is_ok() && has_text && settings.paste_method != PasteMethod::None;
        }
    }
    result
}

pub(crate) fn paste_live_buffer_text(
    text: String,
    app: AppHandle,
    settings: &ResolvedOutputSettings,
    operation_id: u64,
    target_window: Option<usize>,
) -> Result<(), String> {
    paste_stream_chunk_with_output(text, app, Some(settings), Some(operation_id), target_window)
}

pub(crate) fn mark_live_buffer_delivered(operation_id: u64) {
    if let Some(session) = lock_dictation_output_sessions().get_mut(operation_id) {
        session.delivered = true;
    }
}

fn paste_stream_chunk_with_output(
    text: String,
    app_handle: AppHandle,
    output: Option<&ResolvedOutputSettings>,
    operation_id: Option<u64>,
    pinned_target_window: Option<usize>,
) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    let _clipboard_guard = lock_clipboard_transaction("pasting streaming text")?;

    // Keep the session locked until the target application has received the
    // paste shortcut. This prevents a concurrent stop/error path from
    // restoring the previous clipboard during paste_delay_ms.
    let mut stream_session_guard = STREAMING_PASTE_SESSION
        .lock()
        .map_err(|_| "Streaming clipboard session lock poisoned".to_string())?;
    // A late chunk may arrive while a newer recording owns this clipboard
    // session. Use the old operation's captured fallback settings in that case.
    let active_stream_config = stream_session_guard.as_ref()
        .filter(|session| operation_id.map_or(true, |id| session.operation_id == id))
        .map(|session| {
            (
                session.paste_method,
                session.paste_delay_ms,
                session.convert_lf_to_crlf,
                session.check_active_window,
            )
        });

    let fallback_settings = active_stream_config.is_none().then(|| {
        output.cloned()
            .unwrap_or_else(|| ResolvedOutputSettings::resolve(&get_settings(&app_handle), None))
    });
    if active_stream_config.as_ref().is_some_and(|(method, _, _, _)| *method == PasteMethod::None)
        || fallback_settings.as_ref().is_some_and(|settings| settings.paste_method == PasteMethod::None)
    {
        info!("PasteMethod::None selected - skipping streaming chunk paste");
        return Ok(());
    }
    let _output_guard = lock_output();
    let mut enigo = LazyEnigo::new(&app_handle);

    let check_active_window = active_stream_config.as_ref()
        .map(|(_, _, _, check_active_window)| *check_active_window)
        .unwrap_or_else(|| fallback_settings.as_ref().is_some_and(|settings| settings.check_active_window));
    let target_window = if check_active_window {
        pinned_target_window.or_else(output_foreground_window)
    } else { None };
    let target_is_current = || (pinned_target_window.is_none()
        || operation_id.is_some_and(crate::live_output_buffer::allows_delivery))
        && output_target_allows_delivery(check_active_window, target_window, output_foreground_window);
    if !target_is_current() {
        return Err(TARGET_WINDOW_CHANGED_ERROR.to_string());
    }

    if let Some((paste_method, paste_delay_ms, convert_lf_to_crlf, _)) = active_stream_config {
        match paste_method {
            PasteMethod::None => {
                info!("PasteMethod::None selected - skipping streaming chunk paste");
            }
            PasteMethod::Direct => {
                paste_direct(&mut enigo, &app_handle, &text)?;
            }
            PasteMethod::ExternalScript => {
                if !paste_external_script(&app_handle, &text, &target_is_current)? {
                    return Err(TARGET_WINDOW_CHANGED_ERROR.to_string());
                }
            }
            PasteMethod::CtrlV | PasteMethod::CtrlShiftV | PasteMethod::ShiftInsert => {
                // Do not overwrite a previous chunk until the target has had a
                // chance to read it from the clipboard.
                wait_for_clipboard_consumer(
                    stream_session_guard
                        .as_ref()
                        .and_then(|session| session.last_clipboard_paste_sent_at),
                );
                // Streaming chunks are always excluded from the clipboard
                // history; only the final full-text write may be captured.
                let paste_sent_at = paste_via_clipboard_no_restore_guarded(
                    &mut enigo,
                    &text,
                    &app_handle,
                    &paste_method,
                    paste_delay_ms,
                    convert_lf_to_crlf,
                    true,
                    &target_is_current,
                )?;
                if let Some(session) = stream_session_guard.as_mut() {
                    if let Some(paste_sent_at) = paste_sent_at {
                        session.last_clipboard_paste_sent_at = Some(paste_sent_at);
                    }
                    #[cfg(target_os = "windows")]
                    {
                        // An interrupted delay still wrote the chunk to the
                        // clipboard, so restoration must track that write.
                        session.last_clipboard_sequence = Some(win_clipboard::last_write_sequence());
                    }
                }
                if paste_sent_at.is_none() {
                    return Err(TARGET_WINDOW_CHANGED_ERROR.to_string());
                }
            }
        }
    } else if let Some(settings) = fallback_settings {
        // No active streaming session: preserve safe fallback behavior.
        match settings.paste_method {
            PasteMethod::None => {
                info!("PasteMethod::None selected - skipping streaming chunk paste");
            }
            PasteMethod::Direct => {
                paste_direct(&mut enigo, &app_handle, &text)?;
            }
            PasteMethod::ExternalScript => {
                if !paste_external_script(&app_handle, &text, &target_is_current)? {
                    return Err(TARGET_WINDOW_CHANGED_ERROR.to_string());
                }
            }
            PasteMethod::CtrlV | PasteMethod::CtrlShiftV | PasteMethod::ShiftInsert => {
                // Streaming chunks are always excluded from the clipboard history.
                if !paste_via_clipboard_guarded(
                    &mut enigo,
                    &text,
                    &app_handle,
                    &settings.paste_method,
                    settings.paste_delay_ms,
                    settings.convert_lf_to_crlf,
                    settings.clipboard_handling,
                    true,
                    &target_is_current,
                )? {
                    return Err(TARGET_WINDOW_CHANGED_ERROR.to_string());
                }
            }
        }
    }

    drop(stream_session_guard);

    Ok(())
}

fn restore_cut_selection_from_backup(app_handle: &AppHandle, text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    write_plain_clipboard_text(app_handle, text).map_err(|e| {
        format!(
            "Failed to restore clipboard text for selection recovery: {}",
            e
        )
    })?;

    let _output_guard = lock_output();
    let mut enigo = LazyEnigo::new(app_handle);
    send_paste_shortcut(&mut enigo, &PasteMethod::CtrlV)?;

    Ok(())
}

pub fn capture_selection_text(app_handle: &AppHandle) -> Result<String, String> {
    let _capture_guard = lock_clipboard_transaction("capturing selected text")?;
    let clipboard = app_handle.clipboard();
    let clipboard_backup = clipboard.read_text().unwrap_or_default();
    let mut cut_performed = false;

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
        cut_performed = true;
        std::thread::sleep(std::time::Duration::from_millis(80));

        clipboard
            .read_text()
            .map_err(|e| format!("Failed to read clipboard: {}", e))
    })();

    if capture_result.is_err() && cut_performed && !clipboard_backup.is_empty() {
        if let Err(err) = restore_cut_selection_from_backup(app_handle, &clipboard_backup) {
            warn!(
                "Failed to restore selection after cut-based capture error: {}",
                err
            );
        }
    }

    if let Err(err) = write_plain_clipboard_text(app_handle, &clipboard_backup) {
        warn!(
            "Failed to restore clipboard after selection capture: {}",
            err
        );
    }

    capture_result
}

pub fn capture_selection_text_copy(app_handle: &AppHandle) -> Result<String, String> {
    let _capture_guard = lock_clipboard_transaction("copying selected text")?;
    let clipboard = app_handle.clipboard();
    let clipboard_backup = clipboard.read_text().unwrap_or_default();
    let use_documented_owner =
        uses_documented_clipboard_owner(get_settings(app_handle).clipboard_handling);
    #[cfg(target_os = "windows")]
    let advanced_backup = match win_clipboard::backup_all_formats() {
        Ok(backup) => {
            info!(
                "Selection-copy clipboard backup: {} formats saved",
                backup.len()
            );
            Some(backup)
        }
        Err(error) => {
            warn!(
                "Selection-copy clipboard backup failed: {}. Falling back to text-only restore.",
                error
            );
            None
        }
    };

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

    #[cfg(target_os = "windows")]
    if let Some(backup) = advanced_backup {
        if let Err(error) = restore_advanced_clipboard_with_text_fallback(
            app_handle,
            backup,
            &clipboard_backup,
            use_documented_owner,
            None,
        ) {
            warn!(
                "Failed to restore clipboard after selection copy capture: {}",
                error
            );
        }
        return capture_result;
    }

    if let Err(err) = write_plain_clipboard_text(app_handle, &clipboard_backup) {
        warn!(
            "Failed to restore clipboard after selection copy capture: {}",
            err
        );
    }

    capture_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    const YDOTOOL_0_1_8_HELP: &str = r#"
Usage: key [--delay <ms>] [--key-delay <ms>] [--repeat <times>] [--repeat-delay <ms>] <key sequence> ...
Each key sequence can be any number of modifiers and keys, separated by plus (+)
For example: alt+r Alt+F4 CTRL+alt+f3 aLT+1+2+3 ctrl+Backspace
"#;

    #[cfg(target_os = "linux")]
    const YDOTOOL_1_0_4_HELP: &str = r#"
Usage: key [OPTION]... [KEYCODES]...
Since there's no way to know how many keyboard layouts are there in the world,
we're using raw keycodes now.
Syntax: <keycode>:<pressed>
e.g. 28:1 28:0 means pressing on the Enter button on a standard US keyboard.
"#;

    #[cfg(target_os = "linux")]
    #[test]
    fn classifies_ydotool_0_1_8_symbolic_help() {
        assert_eq!(
            classify_ydotool_key_syntax(YDOTOOL_0_1_8_HELP),
            Some(YdotoolKeySyntax::Symbolic)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn classifies_ydotool_1_0_4_raw_keycode_help() {
        assert_eq!(
            classify_ydotool_key_syntax(YDOTOOL_1_0_4_HELP),
            Some(YdotoolKeySyntax::RawKeycodes)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unknown_ydotool_help_falls_back_to_raw_keycodes() {
        let syntax = classify_ydotool_key_syntax("unrecognized help output")
            .unwrap_or(YDOTOOL_UNKNOWN_HELP_FALLBACK);

        assert_eq!(syntax, YdotoolKeySyntax::RawKeycodes);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn generates_symbolic_ydotool_arguments_for_all_paste_methods() {
        assert_eq!(
            ydotool_key_args(&PasteMethod::CtrlV, YdotoolKeySyntax::Symbolic).unwrap(),
            ["key", "ctrl+v"]
        );
        assert_eq!(
            ydotool_key_args(&PasteMethod::CtrlShiftV, YdotoolKeySyntax::Symbolic).unwrap(),
            ["key", "ctrl+shift+v"]
        );
        assert_eq!(
            ydotool_key_args(&PasteMethod::ShiftInsert, YdotoolKeySyntax::Symbolic).unwrap(),
            ["key", "shift+insert"]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn generates_raw_ydotool_arguments_for_all_paste_methods() {
        assert_eq!(
            ydotool_key_args(&PasteMethod::CtrlV, YdotoolKeySyntax::RawKeycodes).unwrap(),
            ["key", "29:1", "47:1", "47:0", "29:0"]
        );
        assert_eq!(
            ydotool_key_args(&PasteMethod::CtrlShiftV, YdotoolKeySyntax::RawKeycodes).unwrap(),
            ["key", "29:1", "42:1", "47:1", "47:0", "42:0", "29:0"]
        );
        assert_eq!(
            ydotool_key_args(&PasteMethod::ShiftInsert, YdotoolKeySyntax::RawKeycodes).unwrap(),
            ["key", "42:1", "110:1", "110:0", "42:0"]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dotool_script_never_turns_text_lines_into_commands() {
        assert_eq!(
            dotool_script("hello\nkey ctrl+c"),
            "type hello\nkey enter\ntype key ctrl+c\n"
        );
        assert_eq!(
            dotool_script("a\r\n\r\nb"),
            "type a\nkey enter\nkey enter\ntype b\n"
        );
    }

    #[test]
    fn delivered_paste_keeps_success_when_clipboard_cleanup_fails() {
        let mut warnings = Vec::new();
        let result = delivery_after_clipboard_cleanup(
            Ok(true), Err("clipboard restoration failed".to_string()),
            |error| warnings.push(error.to_string()),
        );
        assert_eq!(result, Ok(true));
        assert_eq!(warnings, vec!["clipboard restoration failed".to_string()]);
    }

    #[test]
    fn failed_paste_keeps_its_delivery_error_when_clipboard_cleanup_fails() {
        let mut warnings = Vec::new();
        let result = delivery_after_clipboard_cleanup(
            Err("paste shortcut failed".to_string()),
            Err("clipboard restoration failed".to_string()),
            |error| warnings.push(error.to_string()),
        );
        assert_eq!(result, Err("paste shortcut failed".to_string()));
        assert_eq!(warnings, vec!["clipboard restoration failed".to_string()]);
    }

    #[test]
    fn cancelled_paste_stays_cancelled_when_clipboard_cleanup_fails() {
        let mut warnings = Vec::new();
        let result = delivery_after_clipboard_cleanup(
            Ok(false), Err("clipboard restoration failed".to_string()),
            |error| warnings.push(error.to_string()),
        );
        assert_eq!(result, Ok(false));
        assert_eq!(warnings, vec!["clipboard restoration failed".to_string()]);
    }

    #[test]
    fn failed_or_partial_streaming_delivery_stays_failed_after_cleanup_error() {
        for (delivered, failed, can_continue) in
            [(false, false, true), (true, true, true), (true, false, false)]
        {
            let mut warnings = Vec::new();
            let delivery = streaming_delivery_succeeded(
                delivered, failed, PasteMethod::CtrlV, can_continue);
            let result = delivery_after_clipboard_cleanup(
                Ok(delivery), Err("final clipboard write failed".to_string()),
                |error| warnings.push(error.to_string()),
            );
            assert_eq!(result, Ok(false));
            assert_eq!(warnings, vec!["final clipboard write failed".to_string()]);
        }
        assert!(!streaming_delivery_succeeded(false, true, PasteMethod::None, true));
    }

    #[test]
    fn completed_streaming_delivery_stays_successful_after_cleanup_error() {
        let delivery = streaming_delivery_succeeded(true, false, PasteMethod::CtrlV, true);
        let mut warnings = Vec::new();
        let result = delivery_after_clipboard_cleanup(
            Ok(delivery), Err("final clipboard write failed".to_string()),
            |error| warnings.push(error.to_string()),
        );
        assert_eq!(result, Ok(true));
        assert_eq!(warnings, vec!["final clipboard write failed".to_string()]);
    }

    #[test]
    fn finishing_dictation_output_removes_buffered_configuration_once() {
        let mut settings = ResolvedOutputSettings::resolve(&crate::settings::get_default_settings(), None);
        settings.before_insert_key = "enter".to_string();
        let mut sessions = DictationOutputSessions::default();
        sessions.begin(7, Some(settings));
        sessions.get_mut(7).unwrap().delivered = true;

        let finished = sessions.take(7).unwrap();
        assert!(finished.settings.buffers_external_output());
        assert!(finished.delivered);
        assert!(sessions.current().is_none());
        assert!(sessions.take(7).is_none());
    }

    #[test]
    fn stale_dictation_output_cleanup_preserves_the_new_operation() {
        let settings = ResolvedOutputSettings::resolve(&crate::settings::get_default_settings(), None);
        let mut sessions = DictationOutputSessions::default();
        sessions.begin(8, Some(settings));
        sessions.get_mut(8).unwrap().failed = true;

        assert!(sessions.take(7).is_none());
        assert_eq!(sessions.current().unwrap().operation_id, 8);
        let cancelled = sessions.take(8).unwrap();
        assert!(cancelled.failed);
        assert!(sessions.current().is_none());
    }

    #[test]
    fn new_recording_keeps_the_queued_output_of_the_previous_operation() {
        let mut buffered = ResolvedOutputSettings::resolve(&crate::settings::get_default_settings(), None);
        buffered.auto_submit = true;
        let mut sessions = DictationOutputSessions::default();
        sessions.begin(7, Some(buffered));
        sessions.begin(8, None);

        // Live chunks follow the newest recording; 7 keeps its final delivery.
        assert!(sessions.current().is_none());
        assert!(sessions.take(7).unwrap().settings.buffers_external_output());

        let settings = ResolvedOutputSettings::resolve(&crate::settings::get_default_settings(), None);
        for operation_id in 10..20 {
            sessions.begin(operation_id, Some(settings.clone()));
        }
        assert_eq!(sessions.sessions.len(), MAX_DICTATION_OUTPUT_SESSIONS);
        assert_eq!(sessions.current().unwrap().operation_id, 19);
        assert!(sessions.get(10).is_none());
    }

    #[test]
    fn late_chunk_keeps_its_profile_after_a_new_recording_starts() {
        let mut old_settings = ResolvedOutputSettings::resolve(&crate::settings::get_default_settings(), None);
        old_settings.paste_method = PasteMethod::None;
        let mut new_settings = old_settings.clone();
        new_settings.paste_method = PasteMethod::CtrlV;

        let mut sessions = DictationOutputSessions::default();
        sessions.begin(7, Some(old_settings));
        // Background finalization keeps 7's settings while 8 becomes current.
        sessions.begin(8, Some(new_settings));

        assert_eq!(sessions.output_for_chunk(Some(7)).unwrap().1.paste_method, PasteMethod::None);
        assert_eq!(sessions.output_for_chunk(None).unwrap().1.paste_method, PasteMethod::CtrlV);
        sessions.take(7);
        assert!(sessions.output_for_chunk(Some(7)).is_none());
    }

    #[test]
    fn late_finalization_types_no_trailing_space_into_a_newer_recording() {
        let settings = ResolvedOutputSettings::resolve(&crate::settings::get_default_settings(), None);
        let mut sessions = DictationOutputSessions::default();
        sessions.begin(7, Some(settings.clone()));
        sessions.get_mut(7).unwrap().held_whitespace.push(OutputWhitespaceMode::AddIfMissing, "done".to_string());
        sessions.begin(8, Some(settings));
        sessions.get_mut(8).unwrap().held_whitespace.push(OutputWhitespaceMode::AddIfMissing, "new ".to_string());

        sessions.drop_held_whitespace(7);
        assert_eq!(sessions.take(7).unwrap().held_whitespace.resolve(OutputWhitespaceMode::AddIfMissing), "");
        assert_eq!(sessions.take(8).unwrap().held_whitespace.resolve(OutputWhitespaceMode::AddIfMissing), " ");
    }

    #[test]
    fn trailing_whitespace_never_replaces_the_clipboard() {
        let mut settings = ResolvedOutputSettings::resolve(&crate::settings::get_default_settings(), None);
        settings.clipboard_handling = ClipboardHandling::KeepTranscription;
        assert_eq!(trailing_whitespace_output(&settings).clipboard_handling, ClipboardHandling::RestorePlainText);
        settings.clipboard_handling = ClipboardHandling::RestoreAdvancedOwned;
        assert_eq!(trailing_whitespace_output(&settings).clipboard_handling, ClipboardHandling::RestoreAdvancedOwned);
    }

    fn pushed(held: &mut HeldTrailingWhitespace, mode: OutputWhitespaceMode, chunks: &[&str]) -> Vec<String> {
        chunks.iter().map(|chunk| held.push(mode, chunk.to_string())).collect()
    }

    #[test]
    fn held_trailing_space_is_prepended_to_the_next_chunk() {
        let mut held = HeldTrailingWhitespace::default();
        let inserted = pushed(&mut held, OutputWhitespaceMode::RemoveIfPresent, &["Hello ", "world "]);
        assert_eq!(inserted, ["Hello", " world"]);
        assert_eq!(held.tail, " ");
    }

    #[test]
    fn whitespace_only_chunk_extends_the_held_tail() {
        let mut held = HeldTrailingWhitespace::default();
        let inserted = pushed(&mut held, OutputWhitespaceMode::RemoveIfPresent, &["Hi", " ", "\n", "there"]);
        assert_eq!(inserted, ["Hi", "", "", " \nthere"]);
        assert!(held.tail.is_empty());
        pushed(&mut held, OutputWhitespaceMode::RemoveIfPresent, &[" ", "\t"]);
        assert_eq!(held.tail, " \t");
    }

    #[test]
    fn newline_and_unicode_whitespace_tails_are_held_whole() {
        let mut held = HeldTrailingWhitespace::default();
        assert_eq!(held.push(OutputWhitespaceMode::AddIfMissing, "line\n\n".to_string()), "line");
        assert_eq!(held.tail, "\n\n");
        assert_eq!(held.push(OutputWhitespaceMode::AddIfMissing, "текст\u{00A0}\u{3000}".to_string()), "\n\nтекст");
        assert_eq!(held.tail, "\u{00A0}\u{3000}");
        assert_eq!(held.resolve(OutputWhitespaceMode::AddIfMissing), "\u{00A0}\u{3000}");
    }

    #[test]
    fn finalization_applies_each_trailing_mode_to_the_held_tail() {
        for (mode, text, expected) in [
            (OutputWhitespaceMode::RemoveIfPresent, "done ", ""),
            (OutputWhitespaceMode::RemoveIfPresent, "done", ""),
            (OutputWhitespaceMode::AddIfMissing, "done ", " "),
            (OutputWhitespaceMode::AddIfMissing, "done\n", "\n"),
            (OutputWhitespaceMode::AddIfMissing, "done", " "),
        ] {
            let mut held = HeldTrailingWhitespace::default();
            held.push(mode, text.to_string());
            assert_eq!(held.resolve(mode), expected, "{mode:?} {text:?}");
            assert!(held.tail.is_empty());
        }
        let mut held = HeldTrailingWhitespace::default();
        held.tail = " ".to_string();
        assert_eq!(held.resolve(OutputWhitespaceMode::Preserve), " ");
    }

    #[test]
    fn nothing_is_added_or_inserted_when_no_text_was_delivered() {
        for mode in [OutputWhitespaceMode::RemoveIfPresent, OutputWhitespaceMode::AddIfMissing, OutputWhitespaceMode::Preserve] {
            let mut held = HeldTrailingWhitespace::default();
            assert_eq!(held.push(mode, String::new()), "");
            assert_eq!(held.resolve(mode), "");
        }
    }

    #[test]
    fn preserve_passes_chunks_through_unchanged_without_holding() {
        let mut held = HeldTrailingWhitespace::default();
        let inserted = pushed(&mut held, OutputWhitespaceMode::Preserve, &["a ", " ", "b\n"]);
        assert_eq!(inserted, ["a ", " ", "b\n"]);
        assert!(held.tail.is_empty());
        assert_eq!(held.resolve(OutputWhitespaceMode::Preserve), "");
    }

    #[test]
    fn opening_key_accepts_chat_keys_and_rejects_ambiguous_chords() {
        assert!(validate_output_key("enter").is_ok());
        assert!(validate_output_key("ctrl+enter").is_ok());
        assert!(validate_output_key("shift+t").is_ok());
        assert!(validate_output_key("/").is_ok());
        assert!(validate_output_key("ctrl+;").is_ok());
        assert!(validate_output_key("f20").is_ok());
        assert!(validate_output_key("").is_err());
        assert!(validate_output_key("ctrl").is_err());
        assert!(validate_output_key("t+y").is_err());
        assert!(validate_output_key("ctrl+").is_err());
    }

    #[test]
    fn opening_key_deduplicates_modifiers_and_delay_checks_cancellation() {
        let (modifiers, key) = parse_output_key("ctrl+control+enter").unwrap();
        assert_eq!(modifiers, vec![Key::Control]);
        assert_eq!(key, Key::Return);
        assert!(!wait_for_output_delay(5000, &|| false));
        assert!(!wait_for_output_delay(0, &|| false));
    }

    #[test]
    fn opening_key_accepts_browser_captured_lock_and_numpad_keys() {
        for (value, expected) in [
            ("capslock", Key::CapsLock), ("numpad 0", Key::Numpad0),
            ("numpad 9", Key::Numpad9), ("numpad *", Key::Multiply),
            ("numadd", Key::Add), ("numpad -", Key::Subtract),
            ("numpad .", Key::Decimal), ("numpad /", Key::Divide),
        ] {
            let (modifiers, key) = parse_output_key(&format!("ctrl+{}", value)).unwrap();
            assert_eq!(modifiers, vec![Key::Control]);
            assert_eq!(key, expected);
        }
        for value in ["audiovolumeup", "numpadenter", "capslock+t", "numpad 10"] {
            assert!(validate_output_key(value).is_err(), "{}", value);
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn opening_key_accepts_browser_captured_desktop_keys() {
        assert_eq!(parse_output_key("printscreen").unwrap().1, Key::PrintScr);
        assert_eq!(parse_output_key("pause").unwrap().1, Key::Pause);
        assert_eq!(parse_output_key("numlock").unwrap().1, Key::Numlock);
        assert!(validate_output_key("scrolllock").is_ok());
        assert!(validate_output_key("menu").is_ok());
    }

    #[test]
    fn output_delay_stops_when_permission_is_revoked_after_waiting_begins() {
        let checks = std::cell::Cell::new(0);
        assert!(!wait_for_output_delay(5000, &|| {
            checks.set(checks.get() + 1);
            checks.get() < 3
        }));
        assert_eq!(checks.get(), 3);
    }

    #[test]
    fn output_target_guard_does_not_query_windows_when_disabled() {
        for target in [None, Some(1)] {
            assert!(output_target_allows_delivery(false, target,
                || panic!("Disabled window check must not query Windows")));
        }
    }

    #[test]
    fn output_target_guard_rejects_changed_and_missing_windows() {
        assert!(output_target_allows_delivery(true, Some(1), || Some(1)));
        assert!(!output_target_allows_delivery(true, Some(1), || Some(2)));
        assert!(!output_target_allows_delivery(true, Some(1), || None));
        assert_eq!(output_target_allows_delivery(true, None, || None), !cfg!(target_os = "windows"));
    }

    #[test]
    fn output_target_guard_interrupts_delay_if_windows_loses_focus() {
        let current_window = std::cell::Cell::new(Some(1));
        assert!(!wait_for_output_delay(5000, &|| {
            output_target_allows_delivery(true, Some(1), || current_window.replace(None))
        }));
        assert!(wait_for_output_delay(0, &|| {
            output_target_allows_delivery(false, Some(1), || panic!("Window checking is disabled"))
        }));
    }

    #[test]
    fn output_delay_stops_when_the_target_window_changes() {
        let current_window = std::cell::Cell::new(1);
        assert!(!wait_for_output_delay(5000, &|| {
            let current = current_window.replace(2);
            output_target_is_current(Some(1), Some(current))
        }));
        assert!(output_target_is_current(Some(1), Some(1)));
        assert!(!output_target_is_current(Some(1), None));
        assert_eq!(output_target_is_current(None, None), !cfg!(target_os = "windows"));
    }

    #[test]
    fn convert_text_for_clipboard_keeps_text_when_conversion_disabled() {
        let input = "line1\nline2\r\nline3";
        assert_eq!(convert_text_for_clipboard(input, false), input);
    }

    #[test]
    fn convert_text_for_clipboard_normalizes_lf_to_crlf_when_enabled() {
        let input = "line1\nline2\nline3";

        #[cfg(target_os = "windows")]
        assert_eq!(
            convert_text_for_clipboard(input, true),
            "line1\r\nline2\r\nline3"
        );

        #[cfg(not(target_os = "windows"))]
        assert_eq!(convert_text_for_clipboard(input, true), input);
    }

    #[test]
    fn convert_text_for_clipboard_does_not_double_existing_crlf_when_enabled() {
        let input = "line1\r\nline2\nline3\r\n";

        #[cfg(target_os = "windows")]
        assert_eq!(
            convert_text_for_clipboard(input, true),
            "line1\r\nline2\r\nline3\r\n"
        );

        #[cfg(not(target_os = "windows"))]
        assert_eq!(convert_text_for_clipboard(input, true), input);
    }

    #[test]
    fn auto_submit_requires_setting_enabled() {
        assert!(!should_send_auto_submit(false, PasteMethod::CtrlV));
        assert!(!should_send_auto_submit(false, PasteMethod::Direct));
    }

    #[test]
    fn auto_submit_skips_none_paste_method() {
        assert!(!should_send_auto_submit(true, PasteMethod::None));
    }

    #[test]
    fn auto_submit_runs_for_active_paste_methods() {
        assert!(should_send_auto_submit(true, PasteMethod::CtrlV));
        assert!(should_send_auto_submit(true, PasteMethod::Direct));
        assert!(should_send_auto_submit(true, PasteMethod::CtrlShiftV));
        assert!(should_send_auto_submit(true, PasteMethod::ShiftInsert));
    }

    #[test]
    fn both_advanced_modes_restore_all_supported_formats() {
        assert!(restores_all_clipboard_formats(
            ClipboardHandling::RestoreAdvanced
        ));
        assert!(restores_all_clipboard_formats(
            ClipboardHandling::RestoreAdvancedOwned
        ));
        assert!(!restores_all_clipboard_formats(
            ClipboardHandling::RestorePlainText
        ));
    }

    #[test]
    fn only_owned_mode_requests_a_documented_clipboard_owner() {
        assert!(!uses_documented_clipboard_owner(
            ClipboardHandling::RestoreAdvanced
        ));
        assert!(uses_documented_clipboard_owner(
            ClipboardHandling::RestoreAdvancedOwned
        ));
    }

    #[test]
    fn clipboard_consumer_grace_is_full_immediately_after_paste() {
        let paste_sent_at = Instant::now();
        assert_eq!(
            remaining_clipboard_consumer_grace(Some(paste_sent_at), paste_sent_at),
            CLIPBOARD_CONSUMER_GRACE
        );
    }

    #[test]
    fn clipboard_consumer_grace_counts_down_and_expires() {
        let paste_sent_at = Instant::now();
        assert_eq!(
            remaining_clipboard_consumer_grace(
                Some(paste_sent_at),
                paste_sent_at + Duration::from_millis(75),
            ),
            Duration::from_millis(125)
        );
        assert_eq!(
            remaining_clipboard_consumer_grace(
                Some(paste_sent_at),
                paste_sent_at + CLIPBOARD_CONSUMER_GRACE,
            ),
            Duration::ZERO
        );
        assert_eq!(
            remaining_clipboard_consumer_grace(None, paste_sent_at),
            Duration::ZERO
        );
    }

    #[cfg(unix)]
    fn delayed_external_script() -> (std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("aivorelay-script-{}-{}.sh", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::write(&path, "#!/bin/sh\n(sleep 0.4; printf late > \"$1\") &\nwait\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let marker = path.with_extension("late");
        (path, marker)
    }

    #[cfg(unix)]
    #[test]
    fn external_script_timeout_stops_delayed_child_insertion() {
        let (script, marker) = delayed_external_script();
        let result = paste_via_external_script(marker.to_str().unwrap(), script.to_str().unwrap(),
            Duration::from_millis(80), || true);
        assert!(result.is_err());
        std::thread::sleep(Duration::from_millis(500));
        assert!(!marker.exists(), "a descendant inserted after the script timed out");
        std::fs::remove_file(script).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn external_script_cancellation_stops_delayed_child_insertion() {
        let (script, marker) = delayed_external_script();
        let started = Instant::now();
        let result = paste_via_external_script(marker.to_str().unwrap(), script.to_str().unwrap(),
            EXTERNAL_SCRIPT_TIMEOUT, || started.elapsed() < Duration::from_millis(80));
        assert_eq!(result.unwrap(), false);
        std::thread::sleep(Duration::from_millis(500));
        assert!(!marker.exists(), "a descendant inserted after cancellation");
        std::fs::remove_file(script).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn external_script_does_not_wait_for_inherited_stdio() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::mpsc;
        use std::thread;

        let script_path = std::env::temp_dir().join(format!(
            "handy-external-script-{}-{}.sh",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after UNIX_EPOCH")
                .as_nanos()
        ));
        fs::write(&script_path, "#!/bin/sh\nsleep 3 &\nexit 0\n").expect("write external script");
        let mut permissions = fs::metadata(&script_path)
            .expect("read external script metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script_path, permissions).expect("make external script executable");

        let (sender, receiver) = mpsc::channel();
        let script_path_for_thread = script_path.clone();
        thread::spawn(move || {
            let result =
                paste_via_external_script("test", script_path_for_thread.to_str().unwrap(), EXTERNAL_SCRIPT_TIMEOUT, || true);
            sender.send(result).expect("send script result");
        });

        let result = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("external script should return without waiting for its child");
        fs::remove_file(script_path).expect("remove external script");
        assert!(result.is_ok());
    }
}
