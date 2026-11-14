use crate::settings::PasteMethod;
use log::warn;
#[cfg(target_os = "linux")]
use std::time::Duration;

/// Maximum bytes read from `wl-paste` stdout (matches external script 8 MiB limit).
pub(crate) const MAX_PRIMARY_READ_BYTES: usize = 8 * 1024 * 1024;
#[cfg(any(test, target_os = "linux"))]
const MAX_STDERR_BYTES: usize = 64 * 1024;
#[cfg(target_os = "linux")]
const WAYLAND_READ_TIMEOUT: Duration = Duration::from_millis(500);
#[cfg(target_os = "linux")]
const WAYLAND_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PrimaryRead {
    Text(String),
    Empty,
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrimaryBackup {
    pub(crate) saved: Option<String>,
    pub(crate) last_written: String,
}

pub(crate) trait PrimarySelectionBackend {
    fn read_status(&self) -> PrimaryRead;
    fn write(&self, text: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

pub(crate) fn uses_primary_selection_on_platform(
    is_linux: bool,
    paste_method: PasteMethod,
) -> bool {
    is_linux && paste_method == PasteMethod::ShiftInsert
}

#[cfg(target_os = "linux")]
pub(crate) fn uses_primary_selection(paste_method: PasteMethod) -> bool {
    uses_primary_selection_on_platform(true, paste_method)
}

/// Reads existing PRIMARY selection and writes `text` before any `Shift+Insert` keystroke.
///
/// Fails if PRIMARY cannot be safely read or cannot be written so the caller never sends
/// `Shift+Insert` over stale user selection content.
pub(crate) fn prepare_primary_write(
    backend: &dyn PrimarySelectionBackend,
    text: &str,
) -> Result<PrimaryBackup, String> {
    let saved = match backend.read_status() {
        PrimaryRead::Text(existing) => (!existing.is_empty()).then_some(existing),
        PrimaryRead::Empty => None,
        PrimaryRead::Unavailable(reason) => {
            return Err(format!(
                "Cannot paste with Shift+Insert because Linux PRIMARY selection is unavailable ({}). Install wl-clipboard on Wayland, ensure your compositor or X11 server supports PRIMARY selection, or choose another Paste Method in settings.",
                reason
            ));
        }
    };

    backend.write(text).map_err(|reason| {
        format!(
            "Cannot paste with Shift+Insert because Linux PRIMARY selection could not be written ({}). Ensure your compositor or X11 server supports PRIMARY selection, or choose another Paste Method in settings.",
            reason
        )
    })?;

    Ok(PrimaryBackup {
        saved,
        last_written: text.to_string(),
    })
}

/// Updates PRIMARY selection for a subsequent streaming chunk while preserving `backup.saved`.
pub(crate) fn update_primary_write(
    backend: &dyn PrimarySelectionBackend,
    backup: &mut PrimaryBackup,
    text: &str,
) -> Result<(), String> {
    backend.write(text).map_err(|reason| {
        format!(
            "Cannot paste with Shift+Insert because Linux PRIMARY selection could not be updated ({}). Ensure your compositor or X11 server supports PRIMARY selection, or choose another Paste Method in settings.",
            reason
        )
    })?;
    backup.last_written = text.to_string();
    Ok(())
}

/// Restores the original PRIMARY selection ONLY if a fresh read still matches `backup.last_written`.
///
/// If PRIMARY was changed, cleared, or cannot be read (timeout/unavailable/error), restoration
/// is skipped to avoid blindly overwriting a newer selection owner.
pub(crate) fn restore_primary_if_unchanged(
    backend: &dyn PrimarySelectionBackend,
    backup: PrimaryBackup,
) -> Result<(), String> {
    let unchanged = match backend.read_status() {
        PrimaryRead::Text(current) => current == backup.last_written,
        PrimaryRead::Empty => backup.last_written.is_empty(),
        PrimaryRead::Unavailable(reason) => {
            warn!(
                "Skipping PRIMARY selection restore because current state is unknown: {}",
                reason
            );
            return Ok(());
        }
    };

    if !unchanged {
        return Ok(());
    }

    match backup.saved {
        Some(previous) => backend.write(&previous),
        None => backend.clear(),
    }
}

pub(crate) fn wl_copy_write_args(text: &str) -> [&str; 3] {
    ["--primary", "--", text]
}

pub(crate) fn wl_copy_clear_args() -> [&'static str; 2] {
    ["--primary", "--clear"]
}

pub(crate) fn wl_paste_read_args() -> [&'static str; 4] {
    ["--primary", "--no-newline", "--type", "text"]
}

pub(crate) fn read_bounded_bytes(
    reader: impl std::io::Read,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let limit = (max_bytes as u64).saturating_add(1);
    let mut bytes = Vec::new();
    reader
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read PRIMARY selection output: {}", e))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "PRIMARY selection output exceeded {} byte limit",
            max_bytes
        ));
    }
    Ok(bytes)
}

pub(crate) fn read_capped_bytes(
    mut reader: impl std::io::Read,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut kept = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Failed to read command stderr: {}", e))?;
        if n == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(kept.len());
        if remaining > 0 {
            kept.extend_from_slice(&buf[..n.min(remaining)]);
        }
    }
    Ok(kept)
}

pub(crate) fn classify_wl_paste_result(
    success: bool,
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> PrimaryRead {
    if success {
        return match std::str::from_utf8(stdout) {
            Ok("") => PrimaryRead::Empty,
            Ok(text) => PrimaryRead::Text(text.to_string()),
            Err(_) => PrimaryRead::Unavailable(
                "Wayland PRIMARY selection contains invalid UTF-8".to_string(),
            ),
        };
    }

    let err_lossy = String::from_utf8_lossy(stderr);
    let detail = err_lossy.trim();
    let lower = detail.to_ascii_lowercase();
    if lower.contains("nothing is copied")
        || lower.contains("nothing is selected")
        || lower.contains("no suitable type")
        || lower.contains("not available as requested type")
    {
        return PrimaryRead::Empty;
    }

    if detail.is_empty() {
        PrimaryRead::Unavailable(format!("wl-paste exited with status {:?}", exit_code))
    } else {
        PrimaryRead::Unavailable(format!("wl-paste failed: {}", detail))
    }
}

#[cfg(target_os = "linux")]
pub(crate) struct LinuxPrimaryBackend;

#[cfg(target_os = "linux")]
impl PrimarySelectionBackend for LinuxPrimaryBackend {
    fn read_status(&self) -> PrimaryRead {
        if crate::utils::is_wayland() {
            if !tool_in_path("wl-copy") || !tool_in_path("wl-paste") {
                return PrimaryRead::Unavailable(
                    "wl-copy and wl-paste (wl-clipboard) are not installed".to_string(),
                );
            }
            wayland_read_primary()
        } else {
            x11_read_primary()
        }
    }

    fn write(&self, text: &str) -> Result<(), String> {
        if crate::utils::is_wayland() {
            if !tool_in_path("wl-copy") || !tool_in_path("wl-paste") {
                return Err("wl-copy and wl-paste (wl-clipboard) are not installed".to_string());
            }
            wayland_write_primary(text)
        } else {
            x11_write_primary(text)
        }
    }

    fn clear(&self) -> Result<(), String> {
        if crate::utils::is_wayland() {
            if !tool_in_path("wl-copy") {
                return Err("wl-copy (wl-clipboard) is not installed".to_string());
            }
            wayland_clear_primary()
        } else {
            x11_clear_primary()
        }
    }
}

#[cfg(target_os = "linux")]
fn tool_in_path(name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(name);
        std::fs::metadata(&candidate)
            .map(|meta| meta.is_file() && (meta.permissions().mode() & 0o111 != 0))
            .unwrap_or(false)
    })
}

#[cfg(target_os = "linux")]
fn terminate_and_reap(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(target_os = "linux")]
fn run_wl_copy(args: &[&str], timeout: Duration) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("wl-copy")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to execute wl-copy: {}", e))?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!(
                    "wl-copy --primary failed with exit status {:?}",
                    status.code()
                ));
            }
            Ok(None) if std::time::Instant::now() >= deadline => {
                terminate_and_reap(&mut child);
                return Err(format!(
                    "Timed out after {}ms waiting for wl-copy --primary",
                    timeout.as_millis()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => {
                terminate_and_reap(&mut child);
                return Err(format!("Failed waiting for wl-copy --primary: {}", e));
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn wayland_write_primary(text: &str) -> Result<(), String> {
    run_wl_copy(&wl_copy_write_args(text), WAYLAND_WRITE_TIMEOUT)
}

#[cfg(target_os = "linux")]
fn wayland_clear_primary() -> Result<(), String> {
    run_wl_copy(&wl_copy_clear_args(), WAYLAND_WRITE_TIMEOUT)
}

#[cfg(target_os = "linux")]
fn wayland_read_primary() -> PrimaryRead {
    use std::process::{Command, Stdio};
    use std::sync::mpsc;

    let mut child = match Command::new("wl-paste")
        .args(wl_paste_read_args())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return PrimaryRead::Unavailable(format!("Failed to execute wl-paste: {}", e));
        }
    };

    let Some(stdout) = child.stdout.take() else {
        terminate_and_reap(&mut child);
        return PrimaryRead::Unavailable("Failed to capture wl-paste stdout".to_string());
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_and_reap(&mut child);
        return PrimaryRead::Unavailable("Failed to capture wl-paste stderr".to_string());
    };

    let (stdout_tx, stdout_rx) = mpsc::channel();
    let stdout_handle = match std::thread::Builder::new()
        .name("wl-paste-stdout".into())
        .spawn(move || {
            let _ = stdout_tx.send(read_bounded_bytes(stdout, MAX_PRIMARY_READ_BYTES));
        }) {
        Ok(handle) => handle,
        Err(e) => {
            terminate_and_reap(&mut child);
            return PrimaryRead::Unavailable(format!(
                "Failed to spawn wl-paste stdout reader: {}",
                e
            ));
        }
    };

    let (stderr_tx, stderr_rx) = mpsc::channel();
    let stderr_handle = match std::thread::Builder::new()
        .name("wl-paste-stderr".into())
        .spawn(move || {
            let _ = stderr_tx.send(read_capped_bytes(stderr, MAX_STDERR_BYTES));
        }) {
        Ok(handle) => handle,
        Err(e) => {
            terminate_and_reap(&mut child);
            return PrimaryRead::Unavailable(format!(
                "Failed to spawn wl-paste stderr reader: {}",
                e
            ));
        }
    };

    let deadline = std::time::Instant::now() + WAYLAND_READ_TIMEOUT;
    let mut stdout_result: Option<Result<Vec<u8>, String>> = None;
    let mut stderr_result: Option<Result<Vec<u8>, String>> = None;
    let mut exit_status: Option<std::process::ExitStatus> = None;

    loop {
        if stdout_result.is_none() {
            match stdout_rx.try_recv() {
                Ok(Ok(bytes)) => stdout_result = Some(Ok(bytes)),
                Ok(Err(err)) => {
                    terminate_and_reap(&mut child);
                    return PrimaryRead::Unavailable(err);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    terminate_and_reap(&mut child);
                    return PrimaryRead::Unavailable(
                        "wl-paste stdout reader stopped unexpectedly".to_string(),
                    );
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if stderr_result.is_none() {
            match stderr_rx.try_recv() {
                Ok(res) => stderr_result = Some(res),
                Err(mpsc::TryRecvError::Disconnected) => {
                    stderr_result = Some(Ok(Vec::new()));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if exit_status.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => exit_status = Some(status),
                Ok(None) => {}
                Err(e) => {
                    terminate_and_reap(&mut child);
                    return PrimaryRead::Unavailable(format!(
                        "Failed waiting for wl-paste: {}",
                        e
                    ));
                }
            }
        }

        if let (Some(status), Some(Ok(stdout_bytes)), Some(stderr_res)) =
            (exit_status, &stdout_result, &stderr_result)
        {
            let _ = stdout_handle.join();
            let _ = stderr_handle.join();
            let stderr_bytes = stderr_res.as_deref().unwrap_or(&[]);
            return classify_wl_paste_result(
                status.success(),
                status.code(),
                stdout_bytes,
                stderr_bytes,
            );
        }

        if std::time::Instant::now() >= deadline {
            terminate_and_reap(&mut child);
            return PrimaryRead::Unavailable(format!(
                "Timed out after {}ms reading Wayland PRIMARY selection",
                WAYLAND_READ_TIMEOUT.as_millis()
            ));
        }

        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(target_os = "linux")]
static ARBOARD: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);

#[cfg(target_os = "linux")]
fn with_x11_clipboard<T>(
    f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = ARBOARD
        .lock()
        .map_err(|_| "X11 clipboard mutex poisoned".to_string())?;
    if guard.is_none() {
        let cb = arboard::Clipboard::new()
            .map_err(|e| format!("Failed to initialize X11 clipboard: {}", e))?;
        *guard = Some(cb);
    }
    let cb = guard.as_mut().expect("X11 clipboard initialized");
    f(cb)
}

#[cfg(target_os = "linux")]
fn x11_read_primary() -> PrimaryRead {
    use arboard::{GetExtLinux, LinuxClipboardKind};
    let result = with_x11_clipboard(|cb| {
        Ok(cb.get().clipboard(LinuxClipboardKind::Primary).text())
    });
    match result {
        Ok(Ok(text)) if text.is_empty() => PrimaryRead::Empty,
        Ok(Ok(text)) => PrimaryRead::Text(text),
        Ok(Err(arboard::Error::ContentNotAvailable)) => PrimaryRead::Empty,
        Ok(Err(err)) => {
            PrimaryRead::Unavailable(format!("Failed to read X11 PRIMARY selection: {}", err))
        }
        Err(err) => PrimaryRead::Unavailable(err),
    }
}

#[cfg(target_os = "linux")]
fn x11_write_primary(text: &str) -> Result<(), String> {
    use arboard::{LinuxClipboardKind, SetExtLinux};
    with_x11_clipboard(|cb| {
        cb.set()
            .clipboard(LinuxClipboardKind::Primary)
            .text(text.to_string())
            .map_err(|e| format!("Failed to write X11 PRIMARY selection: {}", e))
    })
}

#[cfg(target_os = "linux")]
fn x11_clear_primary() -> Result<(), String> {
    use arboard::{ClearExtLinux, LinuxClipboardKind};
    with_x11_clipboard(|cb| {
        cb.clear_with()
            .clipboard(LinuxClipboardKind::Primary)
            .map_err(|e| format!("Failed to clear X11 PRIMARY selection: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn wayland_argument_builders_protect_leading_dash_text() {
        assert_eq!(
            wl_copy_write_args("--leading-dash"),
            ["--primary", "--", "--leading-dash"]
        );
        assert_eq!(wl_copy_clear_args(), ["--primary", "--clear"]);
        assert_eq!(
            wl_paste_read_args(),
            ["--primary", "--no-newline", "--type", "text"]
        );
    }

    #[test]
    fn read_bounded_bytes_accepts_large_payload_within_limit_and_rejects_overflow() {
        let large = vec![b'a'; 128 * 1024];
        let read = read_bounded_bytes(Cursor::new(large.clone()), MAX_PRIMARY_READ_BYTES).unwrap();
        assert_eq!(read.len(), large.len());

        let exact = vec![b'b'; 16];
        assert_eq!(
            read_bounded_bytes(Cursor::new(exact.clone()), 16).unwrap(),
            exact
        );

        let over = vec![b'c'; 17];
        assert!(read_bounded_bytes(Cursor::new(over), 16).is_err());
    }

    #[test]
    fn read_capped_bytes_keeps_prefix_without_erroring_on_large_stderr() {
        let huge_err = vec![b'e'; MAX_STDERR_BYTES + 5000];
        let kept = read_capped_bytes(Cursor::new(huge_err), MAX_STDERR_BYTES).unwrap();
        assert_eq!(kept.len(), MAX_STDERR_BYTES);
    }

    #[test]
    fn classify_wl_paste_distinguishes_empty_from_unavailable_and_errors() {
        assert_eq!(
            classify_wl_paste_result(true, Some(0), b"", b""),
            PrimaryRead::Empty
        );
        assert_eq!(
            classify_wl_paste_result(true, Some(0), b"hello", b""),
            PrimaryRead::Text("hello".to_string())
        );
        assert!(matches!(
            classify_wl_paste_result(true, Some(0), &[0xff, 0xfe], b""),
            PrimaryRead::Unavailable(_)
        ));
        assert_eq!(
            classify_wl_paste_result(false, Some(1), b"", b"Nothing is copied\n"),
            PrimaryRead::Empty
        );
        assert_eq!(
            classify_wl_paste_result(false, Some(1), b"", b"No suitable type of content copied\n"),
            PrimaryRead::Empty
        );
        assert_eq!(
            classify_wl_paste_result(
                false,
                Some(1),
                b"",
                b"Clipboard content is not available as requested type \"text\"\n"
            ),
            PrimaryRead::Empty
        );
        assert!(matches!(
            classify_wl_paste_result(
                false,
                Some(1),
                b"",
                b"Compositor doesn't support primary selection\n"
            ),
            PrimaryRead::Unavailable(_)
        ));
        assert!(matches!(
            classify_wl_paste_result(false, None, b"", b""),
            PrimaryRead::Unavailable(_)
        ));
    }
}
