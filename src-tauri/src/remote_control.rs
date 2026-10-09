//! One-shot commands delivered to the running app by the single-instance plugin.
use crate::cli::{RecordingCommand, RemoteCommand, TranscriptCommand};
use crate::managers::history::HistoryManager;
use crate::session_manager::{self, ManagedSessionState, SessionState};
use crate::{actions, clipboard, utils, ManagedToggleState};
use log::{debug, error, warn};
use std::sync::{mpsc, Arc, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

// The Windows single-instance callback runs inside the native window procedure.
// Queue work off that thread: microphone I/O and clipboard waits must not block
// the event loop, and successive requests must retain their arrival order.
static COMMAND_QUEUE: OnceLock<mpsc::SyncSender<(AppHandle, RemoteCommand)>> = OnceLock::new();
const STANDARD_BINDING: &str = "transcribe";

#[derive(Debug, PartialEq, Eq)]
enum RecordingDecision {
    Start {
        post_process: bool,
    },
    Stop {
        binding_id: String,
        operation_id: u64,
        post_process: bool,
    },
    Ignore,
}

fn decide_recording_control(
    action: RecordingCommand,
    idle: bool,
    recording: Option<(&str, u64)>,
    startup_or_cleanup: bool,
) -> RecordingDecision {
    if startup_or_cleanup {
        return RecordingDecision::Ignore;
    }
    match action {
        RecordingCommand::Start { post_process } | RecordingCommand::Toggle { post_process }
            if idle =>
        {
            RecordingDecision::Start { post_process }
        }
        RecordingCommand::Stop | RecordingCommand::Toggle { .. } => recording
            .map(|(binding_id, operation_id)| RecordingDecision::Stop {
                binding_id: binding_id.to_string(),
                operation_id,
                post_process: matches!(action, RecordingCommand::Toggle { post_process: true }),
            })
            .unwrap_or(RecordingDecision::Ignore),
        _ => RecordingDecision::Ignore,
    }
}

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
        return Ok(());
    }
    let state = app.state::<ManagedSessionState>();
    let decision = {
        let guard = session_manager::lock_session_state(&state, "CLI recording control");
        let recording = match &*guard {
            SessionState::Recording {
                binding_id,
                operation_id,
                ..
            } => Some((binding_id.as_str(), *operation_id)),
            _ => None,
        };
        decide_recording_control(
            action,
            matches!(*guard, SessionState::Idle),
            recording,
            session_manager::recording_start_is_blocked(),
        )
    };
    match decision {
        RecordingDecision::Start { post_process } => {
            let action = actions::transcribe_action_for_binding(STANDARD_BINDING)
                .ok_or("Transcription action is unavailable")?;
            let source = if post_process { "CLI_POST_PROCESS" } else { "CLI" };
            action.start(app, STANDARD_BINDING, source);
            // Set toggle ownership only while the admitted session is still active.
            // Cancel/auto-stop may have run while microphone startup was doing I/O.
            let guard = session_manager::lock_session_state(&state, "CLI toggle ownership");
            if matches!(&*guard, SessionState::Recording { binding_id, .. }
                if binding_id == STANDARD_BINDING)
            {
                let toggles = app.state::<ManagedToggleState>();
                toggles
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .active_toggles
                    .insert(STANDARD_BINDING.to_string(), true);
            }
        }
        RecordingDecision::Stop {
            binding_id,
            operation_id,
            post_process,
        } => {
            let Some(_control_guard) = session_manager::try_begin_operation_control(
                app,
                Some((&binding_id, operation_id)),
            ) else {
                return Ok(());
            };
            let action = actions::transcribe_action_for_binding(&binding_id)
                .ok_or_else(|| format!("Recording action '{binding_id}' is unavailable"))?;
            if post_process {
                actions::force_post_process_for_next_transcription(&binding_id);
            }
            action.stop(app, &binding_id, "CLI");
            actions::reset_toggle_state(app, &binding_id);
        }
        RecordingDecision::Ignore => {
            debug!("CLI recording control ignored: already satisfied or busy");
        }
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
            app.run_on_main_thread(move || {
                if let Err(error) = clipboard::paste_with_method_override(
                    text, handle.clone(), method.map(Into::into),
                ) {
                    error!("Failed to paste transcript from CLI: {error}");
                    let _ = handle.emit("paste-error", ());
                }
            }).map_err(|error| format!("Failed to schedule transcript paste: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_start_preserves_an_active_profile_recording() {
        assert_eq!(decide_recording_control(RecordingCommand::Start { post_process: true },
            false, Some(("transcribe_profile_work", 42)), false), RecordingDecision::Ignore);
    }

    #[test]
    fn stop_and_toggle_preserve_active_binding_and_generation() {
        for (action, post_process) in [(RecordingCommand::Stop, false),
            (RecordingCommand::Toggle { post_process: false }, false),
            (RecordingCommand::Toggle { post_process: true }, true)] {
            assert_eq!(decide_recording_control(action, false,
                Some(("transcribe_profile_work", 42)), false), RecordingDecision::Stop {
                    binding_id: "transcribe_profile_work".to_string(), operation_id: 42,
                    post_process,
                });
        }
    }

    #[test]
    fn processing_and_startup_never_start_or_stop_another_operation() {
        for action in [RecordingCommand::Start { post_process: false },
            RecordingCommand::Stop, RecordingCommand::Toggle { post_process: true }]
        {
            assert_eq!(decide_recording_control(action, false, None, false), RecordingDecision::Ignore);
            assert_eq!(decide_recording_control(action, true, None, true), RecordingDecision::Ignore);
            assert_eq!(decide_recording_control(action, false, Some(("transcribe", 42)), true), RecordingDecision::Ignore);
        }
    }

    #[test]
    fn idle_stop_is_a_noop_and_start_and_toggle_use_the_requested_mode() {
        assert_eq!(decide_recording_control(RecordingCommand::Stop, true, None, false), RecordingDecision::Ignore);
        for action in [RecordingCommand::Start { post_process: true }, RecordingCommand::Toggle { post_process: true }] {
            assert_eq!(decide_recording_control(action, true, None, false), RecordingDecision::Start { post_process: true });
        }
    }
}
