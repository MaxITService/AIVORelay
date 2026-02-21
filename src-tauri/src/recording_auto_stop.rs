use crate::actions::{reset_toggle_state, transcribe_action_for_binding};
use crate::settings::get_settings;
use crate::utils::cancel_current_operation;
use log::{debug, info};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub struct AutoStopToken {
    pub notify: tokio::sync::Notify,
}

pub type ManagedAutoStopToken = Mutex<Option<Arc<AutoStopToken>>>;

pub fn new_managed_state() -> ManagedAutoStopToken {
    Mutex::new(None)
}

pub fn start_auto_stop_timer(app: &AppHandle, binding_id: &str) {
    let settings = get_settings(app);
    if !settings.recording_auto_stop_enabled {
        return;
    }

    let timeout_secs = settings.recording_auto_stop_timeout_seconds;
    let paste = settings.recording_auto_stop_paste;

    let token = Arc::new(AutoStopToken {
        notify: tokio::sync::Notify::new(),
    });

    if let Ok(mut state) = app.state::<ManagedAutoStopToken>().lock() {
        *state = Some(Arc::clone(&token));
        debug!("Auto-stop timer registered for binding '{}'", binding_id);
    } else {
        log::error!("Failed to lock ManagedAutoStopToken");
        return;
    }

    let app_clone = app.clone();
    let binding_id = binding_id.to_string();
    let token_clone = Arc::clone(&token);

    tauri::async_runtime::spawn(async move {
        let timeout = Duration::from_secs(timeout_secs as u64);

        tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                debug!("Auto-stop timer sleep finished for binding '{}'", binding_id);
            }
            _ = token_clone.notify.notified() => {
                debug!("Auto-stop timer cancelled for binding '{}'", binding_id);
                return;
            }
        }

        // Verify we are still the active timer before executing the payload.
        // This prevents race conditions where we wake up right as someone cancels us,
        // or a new recording has already started with a new token.
        let is_valid_timer = if let Ok(mut state) = app_clone.state::<ManagedAutoStopToken>().lock()
        {
            if let Some(current_token) = state.take() {
                if Arc::ptr_eq(&current_token, &token) {
                    true // We claimed our own token, valid to proceed
                } else {
                    // It was a newer timer, put it back
                    *state = Some(current_token);
                    false // Our timer was overwritten, do not fire
                }
            } else {
                false // Token was already taken (cancelled), do not fire
            }
        } else {
            false
        };

        if !is_valid_timer {
            debug!("Auto-stop timer for binding '{}' woke up but was no longer the active token. Aborting.", binding_id);
            return;
        }

        info!(
            "Recording auto-stop timer fired after {} seconds",
            timeout_secs
        );

        if paste {
            debug!("Auto-stop: stopping action for binding_id {}", binding_id);
            if let Some(action) = transcribe_action_for_binding(&binding_id) {
                action.stop(&app_clone, &binding_id, "auto_stop");
                reset_toggle_state(&app_clone, &binding_id);
            } else {
                log::error!("Auto-stop: no action found for binding_id {}", binding_id);
                cancel_current_operation(&app_clone);
            }
        } else {
            debug!("Auto-stop: cancelling current operation");
            cancel_current_operation(&app_clone);
        }
    });
}

pub fn cancel_auto_stop_timer(app: &AppHandle) {
    if let Ok(mut state) = app.state::<ManagedAutoStopToken>().lock() {
        if let Some(token) = state.take() {
            token.notify.notify_one();
            debug!("Signaled recording auto-stop timer to cancel");
        }
    } else {
        log::error!("Failed to lock ManagedAutoStopToken for cancel");
    }
}
