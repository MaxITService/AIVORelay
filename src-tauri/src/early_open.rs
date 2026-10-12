//! "Press the key as soon as recording starts": the opening key is pressed once the
//! user's hotkey modifiers are released, live text waits for it plus its delay, and a
//! cancelled or empty recording can press a closing key. One shared state per recording.

use log::{debug, warn};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum EarlyKeyState {
    /// Not armed: any opening key is pressed at completion, as without this feature.
    #[default]
    Idle,
    /// Armed; the key is waiting for the hotkey modifiers to be released.
    Waiting,
    Pressed { at: Instant, window: Option<usize> },
    /// The key could not be pressed. Nothing is inserted; text stays in History.
    Failed,
    /// The recording ended without a result (cancelled, empty or failed); nothing more may
    /// be pressed or inserted from this state.
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    Wait,
    Open,
    Closed,
}

/// Decides whether text may be inserted yet: only after the key and its delay.
pub(crate) fn gate_for(state: EarlyKeyState, delay_ms: u64, now: Instant) -> Gate {
    match state {
        EarlyKeyState::Idle => Gate::Open,
        EarlyKeyState::Waiting => Gate::Wait,
        EarlyKeyState::Pressed { at, .. } => {
            if now.saturating_duration_since(at) >= Duration::from_millis(delay_ms.min(5000)) {
                Gate::Open
            } else {
                Gate::Wait
            }
        }
        EarlyKeyState::Failed | EarlyKeyState::Done => Gate::Closed,
    }
}

/// Milliseconds of the opening-key delay still left when the text is ready.
pub(crate) fn remaining_delay_ms(delay_ms: u64, elapsed: Duration) -> u64 {
    delay_ms.min(5000).saturating_sub(elapsed.as_millis().min(u128::from(u64::MAX)) as u64)
}

/// The window that received the key stays the target; a missing window blocks on Windows.
fn window_still_target(start: Option<usize>, current: Option<usize>) -> bool {
    !cfg!(target_os = "windows") || (start.is_some() && start == current)
}

#[derive(Clone, Debug)]
pub(crate) struct EarlyKeyConfig {
    pub key: String,
    pub cancel_key: String,
    pub check_window: bool,
    pub start_window: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct CancelAction {
    pub key: String,
    pub check_window: bool,
    pub window: Option<usize>,
}

#[derive(Default)]
struct Inner {
    state: EarlyKeyState,
    completed: bool,
    config: Option<EarlyKeyConfig>,
}

/// Shared by every clone of one recording's output settings.
#[derive(Clone, Default)]
pub struct EarlyOpeningKey(Arc<Mutex<Inner>>);

impl std::fmt::Debug for EarlyOpeningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EarlyOpeningKey").field(&self.state()).finish()
    }
}

impl EarlyOpeningKey {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn state(&self) -> EarlyKeyState {
        self.lock().state
    }

    pub(crate) fn is_waiting(&self) -> bool {
        self.state() == EarlyKeyState::Waiting
    }

    pub(crate) fn gate(&self, delay_ms: u64, now: Instant) -> Gate {
        gate_for(self.state(), delay_ms, now)
    }

    fn arm(&self, config: EarlyKeyConfig) {
        let mut inner = self.lock();
        inner.state = EarlyKeyState::Waiting;
        inner.completed = false;
        inner.config = Some(config);
    }

    /// Runs `press` only while waiting. Success records the press; failure blocks insertion.
    fn press_with(
        &self,
        press: impl FnOnce(&EarlyKeyConfig) -> Result<Option<usize>, String>,
    ) -> Option<Result<(), String>> {
        let mut inner = self.lock();
        if inner.state != EarlyKeyState::Waiting {
            return None;
        }
        let config = inner.config.clone()?;
        Some(match press(&config) {
            Ok(window) => {
                inner.state = EarlyKeyState::Pressed { at: Instant::now(), window };
                Ok(())
            }
            Err(error) => {
                inner.state = EarlyKeyState::Failed;
                Err(error)
            }
        })
    }

    /// The recording produced its result (text was delivered or attempted).
    pub(crate) fn mark_completed(&self) {
        self.lock().completed = true;
    }

    /// Ends the recording's use of the key. A cancel action is returned only when the key
    /// was pressed and the recording produced no result.
    fn end(&self) -> Option<CancelAction> {
        let mut inner = self.lock();
        let previous = inner.state;
        if matches!(previous, EarlyKeyState::Idle | EarlyKeyState::Done) {
            return None;
        }
        // A pressed key whose recording already produced its result stays pressed: a
        // remainder of that result may still wait for its window (live_output_buffer) and
        // must be inserted later. Only a failure or a cancel closes the gate.
        if inner.completed && matches!(previous, EarlyKeyState::Pressed { .. }) {
            return None;
        }
        inner.state = EarlyKeyState::Done;
        let EarlyKeyState::Pressed { window, .. } = previous else { return None };
        let config = inner.config.as_ref()?;
        (!inner.completed && !config.cancel_key.is_empty()).then(|| CancelAction {
            key: config.cancel_key.clone(),
            check_window: config.check_window,
            window,
        })
    }
}

struct Entry {
    operation_id: u64,
    app: AppHandle,
    key: EarlyOpeningKey,
}

// Only a few recordings can overlap (one recording plus finishing ones).
static REGISTRY: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

fn registry() -> MutexGuard<'static, Vec<Entry>> {
    REGISTRY.lock().unwrap_or_else(|error| error.into_inner())
}

/// Arms the key for this recording. Call before the recording's settings are shared.
pub(crate) fn arm(
    app: &AppHandle,
    operation_id: u64,
    key: &EarlyOpeningKey,
    config: EarlyKeyConfig,
) {
    key.arm(config);
    let mut entries = registry();
    entries.retain(|entry| entry.operation_id != operation_id);
    if entries.len() >= 8 {
        entries.remove(0);
    }
    entries.push(Entry { operation_id, app: app.clone(), key: key.clone() });
}

pub(crate) fn cell_for_operation(operation_id: u64) -> Option<EarlyOpeningKey> {
    registry().iter().find(|entry| entry.operation_id == operation_id)
        .map(|entry| entry.key.clone())
}

/// True while the key of this recording is still waiting to be pressed.
pub(crate) fn is_waiting(operation_id: u64) -> bool {
    cell_for_operation(operation_id).is_some_and(|key| key.is_waiting())
}

/// Ends one recording: nothing is pressed if the key never was; the cancel key is pressed
/// if it was and no result was produced. Safe to call repeatedly.
pub(crate) fn finish_operation(operation_id: u64) {
    let entry = {
        let mut entries = registry();
        entries.iter().position(|entry| entry.operation_id == operation_id)
            .map(|index| entries.remove(index))
    };
    if let Some(entry) = entry {
        finish_entry(entry);
    }
}

/// Cancel: every armed recording ends without a result.
pub(crate) fn cancel_all() {
    let entries: Vec<Entry> = registry().drain(..).collect();
    for entry in entries {
        finish_entry(entry);
    }
}

fn finish_entry(entry: Entry) {
    let Some(action) = entry.key.end() else { return };
    run_key_action(&entry.app.clone(), move || press_cancel_key(&entry.app, action));
}

fn press_cancel_key(app: &AppHandle, action: CancelAction) {
    if action.check_window
        && !window_still_target(action.window, crate::clipboard::output_foreground_window())
    {
        debug!("Cancel key skipped: the target window is no longer in the foreground");
        return;
    }
    if let Err(error) = crate::clipboard::press_output_key_now(app, &action.key) {
        warn!("Failed to press the cancel key: {}", error);
    }
}

/// Windows can press keys from any thread; other platforms must use the main thread.
fn run_key_action(app: &AppHandle, action: impl FnOnce() + Send + 'static) {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        action();
    }
    #[cfg(not(target_os = "windows"))]
    if let Err(error) = app.run_on_main_thread(action) {
        warn!("Failed to queue a key press: {}", error);
    }
}

#[cfg(target_os = "windows")]
fn modifiers_held() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    // Shift, Ctrl, Alt, left and right Win.
    [0x10, 0x11, 0x12, 0x5B, 0x5C]
        .into_iter()
        .any(|key| (unsafe { GetAsyncKeyState(key) } as u16 & 0x8000) != 0)
}

#[cfg(not(target_os = "windows"))]
fn modifiers_held() -> bool {
    false
}

/// Waits (without a time limit) until the hotkey modifiers are released, then presses the
/// opening key. `still_active` must turn false when the recording is cancelled or over, so
/// the worker never presses after the operation ended.
pub(crate) fn spawn_press_worker(operation_id: u64, still_active: impl Fn() -> bool + Send + 'static) {
    let Some(entry_app) = registry().iter().find(|entry| entry.operation_id == operation_id)
        .map(|entry| (entry.app.clone(), entry.key.clone()))
    else {
        return;
    };
    let (app, key) = entry_app;
    std::thread::spawn(move || {
        while key.is_waiting() {
            if !still_active() {
                finish_operation(operation_id);
                return;
            }
            if !modifiers_held() {
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if !key.is_waiting() {
            return;
        }
        let app_for_press = app.clone();
        run_key_action(&app, move || press_opening_key(&app_for_press, &key));
    });
}

fn press_opening_key(app: &AppHandle, key: &EarlyOpeningKey) {
    let outcome = key.press_with(|config| {
        let window = if config.check_window {
            let current = crate::clipboard::output_foreground_window();
            if !window_still_target(config.start_window, current) {
                return Err("The window changed before the opening key could be pressed".to_string());
            }
            current
        } else {
            None
        };
        crate::clipboard::press_output_key_now(app, &config.key)?;
        Ok(window)
    });
    if let Some(Err(error)) = outcome {
        warn!("Opening key was not pressed at recording start; nothing will be inserted: {}", error);
        let _ = app.emit(
            "live-window-output-state",
            serde_json::json!({ "state": "earlyOpenFailed", "pendingChars": 0 }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(cancel_key: &str) -> EarlyKeyConfig {
        EarlyKeyConfig {
            key: "enter".into(),
            cancel_key: cancel_key.into(),
            check_window: false,
            start_window: None,
        }
    }

    #[test]
    fn gate_holds_text_until_the_key_and_its_delay_and_closes_after_failure() {
        let now = Instant::now();
        assert_eq!(gate_for(EarlyKeyState::Idle, 150, now), Gate::Open);
        assert_eq!(gate_for(EarlyKeyState::Waiting, 0, now), Gate::Wait);
        let pressed = EarlyKeyState::Pressed { at: now, window: None };
        assert_eq!(gate_for(pressed, 150, now + Duration::from_millis(149)), Gate::Wait);
        assert_eq!(gate_for(pressed, 150, now + Duration::from_millis(150)), Gate::Open);
        assert_eq!(gate_for(pressed, 0, now), Gate::Open);
        assert_eq!(gate_for(EarlyKeyState::Failed, 0, now), Gate::Closed);
        assert_eq!(gate_for(EarlyKeyState::Done, 0, now), Gate::Closed);
    }

    #[test]
    fn remaining_delay_never_exceeds_the_configured_delay_or_underflows() {
        assert_eq!(remaining_delay_ms(150, Duration::from_millis(40)), 110);
        assert_eq!(remaining_delay_ms(150, Duration::from_millis(500)), 0);
        assert_eq!(remaining_delay_ms(9000, Duration::ZERO), 5000);
    }

    #[test]
    fn press_happens_once_and_only_while_waiting() {
        let key = EarlyOpeningKey::default();
        assert!(key.press_with(|_| Ok(None)).is_none());
        key.arm(config(""));
        assert!(key.is_waiting());
        assert!(matches!(key.press_with(|_| Ok(Some(7))), Some(Ok(()))));
        assert!(matches!(key.state(), EarlyKeyState::Pressed { window: Some(7), .. }));
        assert!(key.press_with(|_| panic!("must not press twice")).is_none());
    }

    #[test]
    fn failed_press_blocks_insertion_and_never_presses_the_cancel_key() {
        let key = EarlyOpeningKey::default();
        key.arm(config("escape"));
        assert!(matches!(key.press_with(|_| Err("no".into())), Some(Err(_))));
        assert_eq!(key.state(), EarlyKeyState::Failed);
        assert!(key.end().is_none());
        assert_eq!(key.state(), EarlyKeyState::Done);
    }

    #[test]
    fn ending_before_the_press_presses_nothing_and_stops_a_late_press() {
        let key = EarlyOpeningKey::default();
        key.arm(config("escape"));
        assert!(key.end().is_none());
        assert!(key.press_with(|_| panic!("must not press after the operation ended")).is_none());
        assert_eq!(key.state(), EarlyKeyState::Done);
    }

    #[test]
    fn cancel_key_is_pressed_only_after_a_press_without_a_result() {
        let key = EarlyOpeningKey::default();
        key.arm(config("escape"));
        key.press_with(|_| Ok(Some(3)));
        let action = key.end().expect("cancelled recording after a press");
        assert_eq!((action.key.as_str(), action.window), ("escape", Some(3)));
        assert!(key.end().is_none());

        let completed = EarlyOpeningKey::default();
        completed.arm(config("escape"));
        completed.press_with(|_| Ok(None));
        completed.mark_completed();
        assert!(completed.end().is_none());

        let no_cancel_key = EarlyOpeningKey::default();
        no_cancel_key.arm(config(""));
        no_cancel_key.press_with(|_| Ok(None));
        assert!(no_cancel_key.end().is_none());
    }

    #[test]
    fn finished_recording_keeps_the_gate_open_for_a_remainder_waiting_for_its_window() {
        let now = Instant::now();
        let later = now + Duration::from_millis(500);

        // Result produced (live text delivered, remainder still waiting): stays open, no cancel key.
        let key = EarlyOpeningKey::default();
        key.arm(config("escape"));
        key.press_with(|_| Ok(Some(3)));
        key.mark_completed();
        assert!(key.end().is_none());
        assert_eq!(key.gate(150, later), Gate::Open);
        assert!(key.end().is_none());

        // Cancelled or empty after the press: closed, and the cancel key is pressed.
        let cancelled = EarlyOpeningKey::default();
        cancelled.arm(config("escape"));
        cancelled.press_with(|_| Ok(Some(3)));
        assert!(cancelled.end().is_some());
        assert_eq!(cancelled.gate(150, later), Gate::Closed);

        // A failed press stays closed even if the recording is later marked complete.
        let failed = EarlyOpeningKey::default();
        failed.arm(config("escape"));
        failed.press_with(|_| Err("no".into()));
        failed.mark_completed();
        assert!(failed.end().is_none());
        assert_eq!(failed.gate(150, later), Gate::Closed);

        // Never pressed: closed, nothing pressed.
        let waiting = EarlyOpeningKey::default();
        waiting.arm(config("escape"));
        waiting.mark_completed();
        assert!(waiting.end().is_none());
        assert_eq!(waiting.gate(150, later), Gate::Closed);
    }

    #[test]
    fn window_must_still_be_the_captured_one_on_windows() {
        assert!(window_still_target(Some(1), Some(1)) || !cfg!(target_os = "windows"));
        assert!(!window_still_target(Some(1), Some(2)) || !cfg!(target_os = "windows"));
        assert!(!window_still_target(None, None) || !cfg!(target_os = "windows"));
    }
}
