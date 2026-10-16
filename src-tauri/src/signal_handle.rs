#[cfg(unix)]
use crate::actions::ACTION_MAP;
#[cfg(target_os = "macos")]
use crate::cli::{RecordingCommand, RemoteCommand};
#[cfg(unix)]
use crate::ManagedToggleState;
#[cfg(unix)]
use log::{debug, info, warn};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
use signal_hook::consts::SIGUSR1;
#[cfg(unix)]
use signal_hook::consts::SIGUSR2;
#[cfg(unix)]
use signal_hook::iterator::Signals;

/// SIGUSR2 toggles plain transcription on all Unix platforms. SIGUSR1
/// (transcription with post-processing) is only handled on macOS: on Linux,
/// WebKitGTK's JavaScriptCore garbage collector sends SIGUSR1 to its own
/// threads to suspend them, so handling it would start phantom recordings on
/// every GC cycle. Linux users should use `--toggle-post-process` instead.
#[cfg(unix)]
pub fn setup_signal_handler(app_handle: AppHandle, mut signals: Signals) {
    let app_handle_for_signal = app_handle.clone();

    debug!("Signal handler registered successfully");
    thread::spawn(move || {
        debug!("SIGUSR2 signal handler thread started");
        for sig in signals.forever() {
            match sig {
                #[cfg(target_os = "macos")]
                SIGUSR1 => {
                    debug!("Received SIGUSR1 signal: toggling transcription with post-processing");
                    crate::remote_control::dispatch(
                        &app_handle_for_signal,
                        RemoteCommand::Recording(RecordingCommand::Toggle { post_process: true }),
                    );
                }
                SIGUSR2 => {
                    debug!("Received SIGUSR2 signal (signal number: {sig})");

                    let binding_id = "transcribe";
                    let shortcut_string = "SIGUSR2";

                    if let Some(action) = ACTION_MAP.get(binding_id) {
                        // Determine action and update state while holding the lock,
                        // but RELEASE the lock before calling the action to avoid deadlocks.
                        // (Actions may need to acquire the lock themselves, e.g., cancel_current_operation)
                        let should_start: bool;
                        {
                            let toggle_state_manager =
                                app_handle_for_signal.state::<ManagedToggleState>();

                            let mut states = match toggle_state_manager.lock() {
                                Ok(s) => s,
                                Err(e) => {
                                    warn!("Failed to lock toggle state manager: {e}");
                                    continue;
                                }
                            };

                            let is_currently_active = states
                                .active_toggles
                                .entry(binding_id.to_string())
                                .or_insert(false);

                            should_start = !*is_currently_active;
                            *is_currently_active = should_start;
                            if !should_start {
                                states.active_presses.remove(binding_id);
                            }
                        } // Lock released here

                        // Now call the action without holding the lock
                        if should_start {
                            debug!("SIGUSR2: Starting transcription (was inactive)");
                            action.start(&app_handle_for_signal, binding_id, shortcut_string);
                            info!("SIGUSR2: Transcription started");
                        } else {
                            debug!("SIGUSR2: Stopping transcription (was active)");
                            action.stop(&app_handle_for_signal, binding_id, shortcut_string);
                            debug!("SIGUSR2: Transcription stopped");
                        }
                    } else {
                        warn!("No action defined in ACTION_MAP for binding ID '{binding_id}'");
                    }
                }
                _ => continue,
            }
        }
    });
}
