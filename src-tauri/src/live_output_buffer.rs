//! Opt-in live delivery to the window captured at recording startup.
//! Provider chunks keep their original order; already delivered text is never replayed.

use crate::settings::{LiveWindowStopAction, LiveWindowWaitLimitAction, ResolvedOutputSettings};
use serde::Serialize;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WindowTarget {
    handle: usize,
    process_id: u32,
}

pub(crate) fn foreground_target() -> Option<WindowTarget> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        let window = GetForegroundWindow();
        if window.0.is_null() { return None; }
        let mut process_id = 0;
        GetWindowThreadProcessId(window, Some(&mut process_id));
        (process_id != 0).then_some(WindowTarget { handle: window.0 as usize, process_id })
    }
    #[cfg(not(target_os = "windows"))]
    { None }
}

fn target_exists(target: WindowTarget) -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow};
        let window = HWND(target.handle as *mut std::ffi::c_void);
        if !IsWindow(Some(window)).as_bool() { return false; }
        let mut process_id = 0;
        GetWindowThreadProcessId(window, Some(&mut process_id));
        process_id == target.process_id
    }
    #[cfg(not(target_os = "windows"))]
    { false }
}

/// Clock and foreground identity are injected so the delivery policy can be
/// tested without interacting with real windows or the clipboard.
#[derive(Debug)]
struct Buffer {
    target: Option<WindowTarget>,
    delay: Duration,
    pending: String,
    paused: bool,
    returned_at: Option<Instant>,
    frozen: bool,
    /// False only for sessions held back by the early opening key without window protection.
    check_window: bool,
}

impl Buffer {
    fn new(target: Option<WindowTarget>, delay_ms: u64) -> Self {
        Self { target, delay: Duration::from_millis(delay_ms.min(5000)), pending: String::new(),
            paused: false, returned_at: None, frozen: false, check_window: true }
    }

    fn ready(&mut self, current: Option<WindowTarget>, now: Instant) -> bool {
        if self.check_window && (self.target.is_none() || current != self.target) {
            self.paused = true;
            self.returned_at = None;
            return false;
        }
        if self.frozen { return false; }
        if !self.paused { return true; }
        let returned_at = *self.returned_at.get_or_insert(now);
        now.saturating_duration_since(returned_at) >= self.delay
    }

    fn take_ready(&mut self, current: Option<WindowTarget>, now: Instant) -> Option<String> {
        if self.pending.is_empty() || !self.ready(current, now) { return None; }
        self.paused = false;
        self.returned_at = None;
        Some(std::mem::take(&mut self.pending))
    }

    fn restore_interrupted(&mut self, text: String) {
        self.pending.insert_str(0, &text);
        self.paused = true;
        self.returned_at = None;
    }
}

struct Session {
    operation_id: u64,
    binding_id: String,
    profile_id: Option<String>,
    settings: ResolvedOutputSettings,
    buffer: Buffer,
    worker_active: bool,
    poll_queued: bool,
    finished: bool,
    /// When the finished remainder started waiting for its window; drives the optional time limit.
    /// It lives in the session, so a new recording, cancel or delivery ends it with the session.
    waiting_since: Option<Instant>,
    failed: bool,
    /// Set only when the opening key is pressed at recording start: text is released
    /// once that key and its delay are done, and dropped if it could not be pressed.
    gate: Option<crate::early_open::EarlyOpeningKey>,
}

static SESSION: LazyLock<Mutex<Option<Session>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BufferNotice {
    state: &'static str,
    pending_chars: usize,
}

fn notice(app: &AppHandle, state: &'static str, pending_chars: usize) {
    let _ = app.emit("live-window-output-state", BufferNotice { state, pending_chars });
}

/// True while a finished recording's remainder waits for its window. The tray shows its
/// "Cancel" item for it, since no recording or Esc shortcut is active any more.
pub(crate) fn has_waiting_remainder() -> bool {
    SESSION.lock().unwrap_or_else(|error| error.into_inner()).as_ref()
        .is_some_and(|session| session.finished && !session.buffer.pending.is_empty())
}

/// Rebuilds the tray menu after a remainder started or stopped waiting. Runs off the calling
/// thread because the rebuild reads settings and devices.
fn waiting_changed(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || crate::tray::refresh_tray_menu(&app, None));
}

pub(crate) fn begin(
    operation_id: u64,
    binding_id: &str,
    profile_id: Option<String>,
    settings: ResolvedOutputSettings,
    target: Option<WindowTarget>,
) {
    let mut guard = SESSION.lock().unwrap_or_else(|error| error.into_inner());
    let gate = settings.opens_at_start()
        .then(|| settings.early_opening_key.clone())
        .filter(|key| key.state() != crate::early_open::EarlyKeyState::Idle);
    let mut buffer = Buffer::new(target, settings.live_window_resume_delay_ms);
    buffer.check_window = gate.is_none() || (settings.check_active_window && cfg!(target_os = "windows"));
    *guard = Some(Session { operation_id, binding_id: binding_id.to_string(), profile_id,
        buffer, settings,
        worker_active: false, poll_queued: false, finished: false,
        waiting_since: None, failed: false, gate });
}

pub(crate) fn enabled(operation_id: u64) -> bool {
    SESSION.lock().ok().is_some_and(|guard|
        guard.as_ref().is_some_and(|session| session.operation_id == operation_id))
}

pub(crate) fn allows_delivery(operation_id: u64) -> bool {
    SESSION.lock().ok().is_some_and(|guard| guard.as_ref().is_some_and(|session|
        session.operation_id == operation_id && !session.buffer.frozen && !session.failed
            && (!session.buffer.check_window
                || (session.buffer.target.is_some() && foreground_target() == session.buffer.target))))
}

/// Discards the session. A finished wait-for-window remainder is not inserted, so tell the user
/// it can be recovered from History (replaces the "waiting" toast, which would otherwise go stale).
pub(crate) fn cancel(app: &AppHandle) {
    let discarded = SESSION.lock().ok().and_then(|mut guard| guard.take());
    if let Some(count) = discarded.as_ref().and_then(discarded_remainder_chars) {
        notice(app, "history", count);
        waiting_changed(app);
    }
}

fn discarded_remainder_chars(session: &Session) -> Option<usize> {
    (session.finished && !session.buffer.pending.is_empty())
        .then(|| session.buffer.pending.chars().count())
}

/// Routine end-of-operation cleanup must not remove a finished wait-for-window job;
/// only an explicit cancel() discards it.
pub(crate) fn cancel_unfinished(operation_id: u64) {
    if let Ok(mut guard) = SESSION.lock() {
        if guard.as_ref().is_some_and(|session| session.operation_id == operation_id && !session.finished) {
            *guard = None;
        }
    }
}

pub(crate) fn stop(operation_id: u64) {
    if !enabled(operation_id) { return; }
    let current = foreground_target();
    if let Ok(mut guard) = SESSION.lock() {
        if let Some(session) = guard.as_mut().filter(|session| session.operation_id == operation_id) {
            let stopped_elsewhere = session.buffer.check_window
                && (session.buffer.target.is_none() || current != session.buffer.target);
            session.buffer.frozen = session.failed || (stopped_elsewhere
                && session.settings.live_window_stop_action != LiveWindowStopAction::WaitForWindow);
        }
    }
}

/// Also dismisses any "paused"/"waiting" notice, which would otherwise offer actions that do nothing.
fn present_preview(app: &AppHandle, session: Session) {
    notice(app, "previewed", 0);
    waiting_changed(app);
    crate::actions::show_retained_live_output_preview(app, session.buffer.pending,
        session.binding_id, session.profile_id, session.settings);
}

/// Takes a finished session whose remainder still waits for its window.
fn take_waiting() -> Option<Session> {
    SESSION.lock().ok().and_then(|mut guard| {
        if guard.as_ref().is_some_and(|session| session.finished && !session.buffer.pending.is_empty()) {
            guard.take()
        } else { None }
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn show_pending_live_output_preview(app: AppHandle) {
    if let Some(previous) = take_waiting() { present_preview(&app, previous); }
}

/// Drops a finished remainder that waits for its window, like `cancel()` but without touching
/// anything else. The full text stays in History; the notice replaces the "waiting" toast.
#[tauri::command]
#[specta::specta]
pub(crate) fn discard_pending_live_output(app: AppHandle) {
    if let Some(count) = take_waiting().as_ref().and_then(discarded_remainder_chars) {
        notice(&app, "history", count);
        waiting_changed(&app);
    }
}

/// A new recording never silently discards or interleaves an older waiting job.
/// Show that remainder for manual insertion before accepting another recording.
pub(crate) fn recover_waiting_before_recording(app: &AppHandle) -> bool {
    let (previous, still_finishing) = match take_waiting() {
        Some(previous) => (Some(previous), false),
        None => (None, SESSION.lock().ok()
            .is_some_and(|guard| guard.as_ref().is_some_and(|session| !session.finished))),
    };
    if let Some(previous) = previous {
        present_preview(app, previous);
        true
    } else {
        if still_finishing { notice(app, "busy", 0); }
        still_finishing
    }
}

pub(crate) fn enqueue(app: &AppHandle, operation_id: u64, text: String) -> Option<Result<(), String>> {
    {
        let mut guard = SESSION.lock().ok()?;
        let session = guard.as_mut().filter(|session| session.operation_id == operation_id)?;
        session.buffer.pending.push_str(&text);
    }
    flush(app, operation_id);
    start_worker(app, operation_id);
    Some(Ok(()))
}

fn start_worker(app: &AppHandle, operation_id: u64) {
    {
        let Ok(mut guard) = SESSION.lock() else { return; };
        let Some(session) = guard.as_mut().filter(|session| session.operation_id == operation_id) else { return; };
        if session.worker_active || session.buffer.pending.is_empty() { return; }
        session.worker_active = true;
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(50));
        let should_poll = {
            let Ok(mut guard) = SESSION.lock() else { return; };
            let Some(session) = guard.as_mut().filter(|session| session.operation_id == operation_id) else { return; };
            if session.buffer.pending.is_empty() || session.buffer.frozen {
                session.worker_active = false;
                return;
            }
            if session.poll_queued { false } else { session.poll_queued = true; true }
        };
        if should_poll {
            let app_for_poll = app.clone();
            if let Err(error) = app.run_on_main_thread(move || flush(&app_for_poll, operation_id)) {
                log::warn!("Failed to queue live window buffer update: {}", error);
                if let Ok(mut guard) = SESSION.lock() {
                    if let Some(session) = guard.as_mut().filter(|session| session.operation_id == operation_id) {
                        session.poll_queued = false;
                        session.worker_active = false;
                    }
                }
                return;
            }
        }
    });
}

/// How a session left the buffer in `flush` without delivering what it still held.
enum Ended {
    Preview(Session),
    /// The remainder is dropped (its character count, 0 if none): the full text stays in History.
    History(usize),
}

/// All calls run on the main thread, serially with ordinary chunk pastes.
fn flush(app: &AppHandle, operation_id: u64) {
    if !enabled(operation_id) { return; }
    let current = foreground_target();
    let (delivery, paused_notice, ended) = {
        let Ok(mut guard) = SESSION.lock() else { return; };
        let Some(session) = guard.as_mut().filter(|session| session.operation_id == operation_id) else { return; };
        session.poll_queued = false;
        let gate = session.gate.as_ref()
            .map(|key| key.gate(session.settings.recording_start_delay_ms, Instant::now()));
        if gate == Some(crate::early_open::Gate::Closed) {
            // The opening key could not be pressed or the recording ended: nothing is
            // inserted; the full text stays in History.
            let pending_chars = discarded_remainder_chars(session).unwrap_or(0);
            *guard = None;
            (None, None, Some(Ended::History(pending_chars)))
        } else if session.finished && session.buffer.check_window
            && session.buffer.target.is_none_or(|target| !target_exists(target))
        {
            (None, None, guard.take().map(Ended::Preview))
        } else {
            let was_paused = session.buffer.paused;
            let text = if gate == Some(crate::early_open::Gate::Wait) { None }
                else { session.buffer.take_ready(current, Instant::now()) };
            let paused_notice = (!was_paused && session.buffer.paused)
                .then(|| (session.buffer.pending.chars().count(), session.finished));
            let target_window = session.buffer.target.filter(|_| session.buffer.check_window)
                .map(|target| target.handle);
            if text.is_none() && wait_limit_reached(session.waiting_since,
                session.settings.live_window_wait_limit_secs, Instant::now())
            {
                // The worker polls a waiting remainder every 50 ms, so the limit needs no timer of its own.
                let pending_chars = session.buffer.pending.chars().count();
                let action = session.settings.live_window_wait_limit_action;
                let ended = guard.take().map(|expired| match action {
                    LiveWindowWaitLimitAction::Preview => Ended::Preview(expired),
                    LiveWindowWaitLimitAction::History => Ended::History(pending_chars),
                });
                (None, None, ended)
            } else {
                let delivery = text.map(|text| (text, session.settings.clone(), target_window, was_paused));
                (delivery, paused_notice, None)
            }
        }
    };
    match ended {
        Some(Ended::Preview(previous)) => { present_preview(app, previous); return; }
        Some(Ended::History(pending_chars)) => {
            if pending_chars > 0 {
                notice(app, "history", pending_chars);
                waiting_changed(app);
            }
            return;
        }
        None => {}
    }
    if let Some((count, finished)) = paused_notice {
        // A finished remainder keeps the persistent "waiting" notice (and its actions).
        notice(app, if finished { "waiting" } else { "paused" }, count);
    }
    let Some((text, settings, target, resumed)) = delivery else { return; };
    let result = crate::clipboard::paste_live_buffer_text(text.clone(), app.clone(), &settings,
        operation_id, target);
    let mut failed_preview = None;
    let mut remainder_delivered = false;
    let mut early_submit = None;
    if let Ok(mut guard) = SESSION.lock() {
        if let Some(session) = guard.as_mut().filter(|session| session.operation_id == operation_id) {
            if let Err(error) = result {
                session.buffer.restore_interrupted(text);
                if error != crate::clipboard::TARGET_WINDOW_CHANGED_ERROR {
                    log::warn!("Live buffer insertion failed: {}", error);
                    session.failed = true;
                    session.buffer.frozen = true;
                    if session.finished { failed_preview = guard.take(); }
                }
            } else {
                crate::clipboard::mark_live_buffer_delivered(operation_id);
                remainder_delivered = session.finished && session.buffer.pending.is_empty();
                if remainder_delivered {
                    // Everything reached the window that got the opening key pressed at recording
                    // start, so the after-insertion key it owns is pressed now (never earlier,
                    // while the remainder was still waiting).
                    early_submit = session.gate.as_ref().and_then(|key| match key.state() {
                        crate::early_open::EarlyKeyState::Pressed { window, .. } =>
                            Some((session.settings.clone(), window)),
                        _ => None,
                    });
                    *guard = None;
                }
                // A delivery that ends a pause, or the last delivery of a waiting remainder,
                // dismisses the paused/waiting notice.
                if resumed || remainder_delivered { notice(app, "resumed", 0); }
            }
        }
    }
    if remainder_delivered { waiting_changed(app); }
    if let Some((settings, window)) = early_submit {
        crate::clipboard::send_early_session_submit(app, &settings, window, &|| true);
    }
    if let Some(previous) = failed_preview { present_preview(app, previous); }
}

/// Final transcript/history remains owned by the normal completion path.
/// Only the undelivered suffix is retained here.
/// `None`: no live session. `Some(true)`: everything was delivered. `Some(false)`: a
/// remainder was left undelivered (History, Preview or waiting for the window).
pub(crate) fn finish_detailed(app: &AppHandle, operation_id: u64) -> Option<bool> {
    if !enabled(operation_id) { return None; }
    flush(app, operation_id);
    let preview = {
        let Ok(mut guard) = SESSION.lock() else { return None; };
        let Some(session) = guard.as_mut().filter(|session| session.operation_id == operation_id) else { return None; };
        session.finished = true;
        if session.buffer.pending.is_empty() {
            *guard = None;
            return Some(true);
        }
        // Still in the target window (or no window protection): what remains is held back only by
        // the return delay or the opening-key delay, so it is delivered shortly. A frozen session
        // (Stop pressed elsewhere with History/Preview) never delivers.
        let in_target_window = !session.buffer.frozen && (!session.buffer.check_window
            || (session.buffer.target.is_some() && foreground_target() == session.buffer.target));
        let action = completion_action(session.settings.live_window_stop_action, in_target_window,
            session.failed || (session.buffer.check_window
                && session.buffer.target.is_none_or(|target| !target_exists(target))));
        match action {
            LiveWindowStopAction::History => {
                let count = session.buffer.pending.chars().count();
                *guard = None;
                notice(app, "history", count);
                None
            }
            LiveWindowStopAction::Preview => guard.take(),
            LiveWindowStopAction::WaitForWindow => {
                session.waiting_since = Some(Instant::now());
                notice(app, "waiting", session.buffer.pending.chars().count());
                waiting_changed(app);
                None
            }
        }
    };
    if let Some(previous) = preview { present_preview(app, previous); }
    start_worker(app, operation_id);
    Some(false)
}

/// True once a finished remainder has waited for its window as long as the limit allows (0: never).
fn wait_limit_reached(waiting_since: Option<Instant>, limit_secs: u64, now: Instant) -> bool {
    limit_secs > 0 && waiting_since.is_some_and(|since|
        now.saturating_duration_since(since) >= Duration::from_secs(limit_secs))
}

/// What happens to a remainder that could not be delivered when the recording finished:
/// the profile's action when the user is not in the target window, a short wait when they are.
fn completion_action(selected: LiveWindowStopAction, in_target_window: bool, unavailable: bool) -> LiveWindowStopAction {
    if unavailable { LiveWindowStopAction::Preview }
    else if in_target_window { LiveWindowStopAction::WaitForWindow }
    else { selected }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(handle: usize) -> Option<WindowTarget> {
        Some(WindowTarget { handle, process_id: 1 })
    }

    #[test]
    fn switching_windows_buffers_until_return_delay_and_never_repeats_delivered_text() {
        let now = Instant::now();
        let mut buffer = Buffer::new(target(1), 500);
        buffer.pending.push_str("first ");
        assert_eq!(buffer.take_ready(target(1), now).as_deref(), Some("first "));
        buffer.pending.push_str("second ");
        assert_eq!(buffer.take_ready(target(2), now), None);
        buffer.pending.push_str("third");
        assert_eq!(buffer.take_ready(target(1), now), None);
        assert_eq!(buffer.take_ready(target(1), now + Duration::from_millis(499)), None);
        assert_eq!(buffer.take_ready(target(1), now + Duration::from_millis(500)).as_deref(), Some("second third"));
        assert!(buffer.pending.is_empty());
    }

    #[test]
    fn leaving_again_restarts_the_delay_and_zero_delay_resumes_immediately() {
        let now = Instant::now();
        let mut buffer = Buffer::new(target(1), 500);
        buffer.pending.push_str("text");
        assert!(!buffer.ready(target(2), now));
        assert!(!buffer.ready(target(1), now));
        assert!(!buffer.ready(target(2), now + Duration::from_millis(400)));
        assert!(!buffer.ready(target(1), now + Duration::from_millis(500)));
        assert!(!buffer.ready(target(1), now + Duration::from_millis(999)));
        assert!(buffer.ready(target(1), now + Duration::from_millis(1000)));
        buffer.delay = Duration::ZERO;
        assert!(!buffer.ready(target(2), now));
        assert!(buffer.ready(target(1), now));
    }

    #[test]
    fn missing_window_and_frozen_stop_do_not_deliver() {
        let now = Instant::now();
        let mut buffer = Buffer::new(None, 0);
        buffer.pending.push_str("текст🙂");
        assert_eq!(buffer.take_ready(None, now), None);
        buffer.target = target(1);
        buffer.pending.push_str("remaining");
        buffer.frozen = true;
        assert_eq!(buffer.take_ready(target(1), now), None);
    }

    #[test]
    fn interrupted_delivery_restores_original_order() {
        let now = Instant::now();
        let mut buffer = Buffer::new(target(1), 0);
        buffer.pending.push_str("one ");
        let text = buffer.take_ready(target(1), now).unwrap();
        buffer.pending.push_str("two");
        buffer.restore_interrupted(text);
        assert_eq!(buffer.take_ready(target(1), now).as_deref(), Some("one two"));
    }

    #[test]
    fn buffer_without_window_protection_ignores_the_foreground_window() {
        let now = Instant::now();
        let mut buffer = Buffer::new(None, 500);
        buffer.check_window = false;
        buffer.pending.push_str("text");
        assert_eq!(buffer.take_ready(target(9), now).as_deref(), Some("text"));
        assert!(!buffer.paused);
    }

    #[test]
    fn remainder_outside_the_target_window_at_the_end_follows_each_action() {
        for action in [LiveWindowStopAction::History, LiveWindowStopAction::Preview, LiveWindowStopAction::WaitForWindow] {
            assert_eq!(completion_action(action, false, false), action);
            assert_eq!(completion_action(action, true, false), LiveWindowStopAction::WaitForWindow);
            assert_eq!(completion_action(action, false, true), LiveWindowStopAction::Preview);
            assert_eq!(completion_action(action, true, true), LiveWindowStopAction::Preview);
        }
    }

    #[test]
    fn wait_limit_expires_only_for_a_waiting_remainder_with_a_limit() {
        let start = Instant::now();
        assert!(!wait_limit_reached(Some(start), 0, start + Duration::from_secs(10_000)));
        assert!(!wait_limit_reached(None, 60, start + Duration::from_secs(10_000)));
        assert!(!wait_limit_reached(Some(start), 60, start + Duration::from_secs(59)));
        assert!(wait_limit_reached(Some(start), 60, start + Duration::from_secs(60)));
        assert!(!wait_limit_reached(Some(start + Duration::from_secs(5)), 60, start));
    }

    #[test]
    fn stop_pressed_in_the_target_window_still_applies_the_action_when_the_user_left_before_the_end() {
        // Stop was pressed in the target window, so nothing was frozen; by the end the user is
        // elsewhere and the remainder is undelivered: the chosen action applies, not an endless wait.
        assert_eq!(completion_action(LiveWindowStopAction::History, false, false), LiveWindowStopAction::History);
        assert_eq!(completion_action(LiveWindowStopAction::Preview, false, false), LiveWindowStopAction::Preview);
    }
}
