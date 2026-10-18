#[cfg(unix)]
use crate::cli::{RecordingCommand, RemoteCommand};
#[cfg(target_os = "macos")]
use signal_hook::consts::SIGUSR1;
#[cfg(unix)]
use signal_hook::{consts::SIGUSR2, iterator::Signals};

/// Linux reserves SIGUSR1 for WebKitGTK's garbage collector. Supported external
/// triggers use the same ordered coordinator as keyboard shortcuts.
#[cfg(unix)]
pub fn setup_signal_handler(app: tauri::AppHandle, mut signals: Signals) {
    std::thread::spawn(move || {
        for signal in signals.forever() {
            let post_process = match signal {
                #[cfg(target_os = "macos")]
                SIGUSR1 => true,
                SIGUSR2 => false,
                _ => continue,
            };
            crate::remote_control::dispatch(&app,
                RemoteCommand::Recording(RecordingCommand::Toggle { post_process }));
        }
    });
}
