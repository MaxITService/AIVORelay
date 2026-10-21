//! One-shot commands delivered to the running app by the single-instance plugin.
use crate::cli::{RecordingCommand, RemoteCommand, TranscriptCommand};
use crate::managers::history::HistoryManager;
use crate::{clipboard, utils};
use log::{error, warn};
use std::sync::{mpsc, Arc, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

// The Windows single-instance callback runs inside the native window procedure.
// Queue work off that thread: microphone I/O and clipboard waits must not block
// the event loop, and successive requests must retain their arrival order.
static COMMAND_QUEUE: OnceLock<mpsc::SyncSender<(AppHandle, RemoteCommand)>> = OnceLock::new();

pub(crate) fn dispatch(app: &AppHandle, command: RemoteCommand) {
    let sender = COMMAND_QUEUE.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel::<(AppHandle, RemoteCommand)>(64);
        std::thread::spawn(move || {
            for (app, command) in receiver {
                let result = match command {
                    RemoteCommand::Recording(action) => execute_recording(&app, action),
                    RemoteCommand::Transcript(action) => execute_transcript(&app, action),
                };
                if let Err(error) = result {
                    error!("Failed to execute CLI remote command: {error}");
                }
            }
        });
        sender
    });
    if let Err(error) = sender.try_send((app.clone(), command)) {
        error!("Could not queue CLI remote command: {error}");
    }
}

fn execute_recording(app: &AppHandle, action: RecordingCommand) -> Result<(), String> {
    if action == RecordingCommand::Cancel {
        utils::cancel_current_operation(app);
    } else {
        // The shortcut coordinator owns start/stop parity with hotkeys.
        crate::platform_shortcuts::send_recording(app, action);
    }
    Ok(())
}

fn execute_transcript(app: &AppHandle, action: TranscriptCommand) -> Result<(), String> {
    let history = app.state::<Arc<HistoryManager>>();
    let Some(entry) = history
        .get_latest_completed_entry()
        .map_err(|error| format!("Failed to fetch latest completed transcript: {error}"))?
    else {
        warn!("No completed dictation is available for the CLI command");
        return Ok(());
    };
    let text = crate::tray::last_transcript_text(&entry).to_string();
    if text.trim().is_empty() {
        warn!("Latest completed transcript is empty; skipping CLI delivery");
        return Ok(());
    }
    match action {
        TranscriptCommand::Clipboard => {
            let handle = app.clone();
            app.run_on_main_thread(move || {
                let result = (|| {
                    let _guard = clipboard::lock_clipboard_transaction(
                        "copying the latest transcript from CLI",
                    )?;
                    handle.clipboard().write_text(text)
                        .map_err(|error| format!("Failed to copy transcript: {error}"))
                })();
                if let Err(error) = result {
                    error!("{error}");
                }
            }).map_err(|error| format!("Failed to schedule transcript copy: {error}"))
        }
        TranscriptCommand::Paste { method } => {
            let handle = app.clone();
            crate::output_dispatch::dispatch_generic(app, method.map(Into::into), move |method| {
                if let Err(error) = clipboard::paste_with_method_override(
                    text, handle.clone(), method,
                ) {
                    error!("Failed to paste transcript from CLI: {error}");
                    let _ = handle.emit("paste-error", ());
                }
            }).map_err(|error| format!("Failed to schedule transcript paste: {error}"))
        }
    }
}
