//! Transcribe shortcut and CLI recording lifecycle on all platforms.
//!
//! The upstream decision machine is kept in state.rs with its upstream tests.
//! This adapter retains the fork's session arbiter as the authority for resource
//! ownership, including CLI, auto-stop, profile recordings, and cancellation.

mod state;
mod busy_admission;
#[cfg(not(target_os = "windows"))]
pub(crate) mod tauri_backend;
#[cfg(target_os = "macos")]
pub(crate) mod secure_input;

use state::{CoordinatorState, Effect, InputEvent, ShortcutActivation};
use crate::session_manager::{self, ManagedSessionState, SessionState};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use crate::cli::RecordingCommand;

struct QueuedInput { event: InputEvent, received_at: Instant }
enum Command { Input(QueuedInput), Recording(RecordingCommand), SessionChanged, Cancel }

fn apply_input(machine: &mut CoordinatorState, input: QueuedInput) -> Option<Effect> {
    // Fork microphone/model startup may block the executor. A quick tap must
    // remain a tap even when its release is dequeued after a long cold start.
    machine.on_input(input.event, input.received_at)
}

fn activation_mode(ptt: bool, auto: bool) -> ShortcutActivation {
    if auto { ShortcutActivation::HoldOrToggle } else if ptt { ShortcutActivation::PushToTalk } else { ShortcutActivation::Toggle }
}

fn captured_activation(settings: &crate::settings::AppSettings, profile_id: Option<&str>) -> ShortcutActivation {
    let profile = profile_id.and_then(|id| settings.transcription_profile(id));
    activation_mode(
        profile.map(|profile| profile.push_to_talk).unwrap_or(settings.push_to_talk),
        profile.map(|profile| profile.auto_shortcut_activation).unwrap_or(settings.auto_shortcut_activation),
    )
}

fn use_captured_activation(app: &AppHandle, input: &mut QueuedInput) {
    if input.event.external { return; }
    let session = app.state::<ManagedSessionState>();
    let session = session_manager::lock_session_state(&session, "shortcut captured activation");
    if let SessionState::Recording { binding_id, captured_settings, captured_profile_id, .. } = &*session {
        if binding_id == &input.event.binding_id {
            input.event.mode = captured_activation(captured_settings, captured_profile_id.as_deref());
        }
    }
}

pub(crate) struct Coordinator(Sender<Command>);

#[cfg(target_os = "macos")]
pub(crate) fn init_macos_backend(app: &AppHandle) {
    secure_input::init(app);
    if let Err(error) = crate::shortcut_handy_keys::initialize(app) {
        log::warn!("HandyKeys unavailable on macOS; using upstream Tauri backend: {error}");
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_uses_handy_keys(app: &AppHandle) -> bool {
    app.try_state::<crate::shortcut_handy_keys::HandyKeysState>().is_some()
}

#[derive(Debug, PartialEq, Eq)]
enum Snapshot {
    Idle,
    Recording { binding: String, operation: u64 },
    Processing { operation: u64 },
}

fn ignore_busy_input(snapshot: &Snapshot, blocked: bool, queue: bool) -> bool {
    (!queue && matches!(snapshot, Snapshot::Processing { .. }))
        || (matches!(snapshot, Snapshot::Idle) && blocked)
}

/// The queue setting matters only while processing, so idle input skips the settings read.
fn queue_applies(app: &AppHandle, snapshot: &Snapshot) -> bool {
    matches!(snapshot, Snapshot::Processing { .. })
        && crate::settings::get_settings(app).queue_recording_during_processing
}

/// A start remembered while processing must not replace a failure the user
/// may still retry, nor open a recording into a Preview that is still open.
fn keep_queued_start(app: &AppHandle) -> bool {
    crate::settings::get_settings(app).queue_recording_during_processing
        && !crate::tray::has_recent_error(app)
        && !crate::managers::preview_output_mode::is_active()
}

fn allow_input(input: &QueuedInput, snapshot: &Snapshot, blocked: bool, queue: bool) -> bool {
    static ADMISSION: std::sync::OnceLock<std::sync::Mutex<busy_admission::InputAdmission>> = std::sync::OnceLock::new();
    ADMISSION.get_or_init(|| std::sync::Mutex::new(busy_admission::InputAdmission::default()))
        .lock().unwrap_or_else(|error| error.into_inner())
        .allows(&input.event, input.received_at, ignore_busy_input(snapshot, blocked, queue))
}

struct RuntimeState {
    machine: CoordinatorState,
    operation: Option<u64>,
    processing: bool,
}

impl RuntimeState {
    fn new() -> Self { Self { machine: CoordinatorState::new(), operation: None, processing: false } }

    fn on_input(&mut self, input: QueuedInput, snapshot: &Snapshot, blocked: bool, queue: bool) -> Option<Effect> {
        // Busy presses are discarded unless the queue setting lets the machine
        // remember one for an automatic start after processing.
        // Recording releases must still pass while microphone startup owns its guard.
        if ignore_busy_input(snapshot, blocked, queue) { return None; }
        apply_input(&mut self.machine, input)
    }

    /// CLI start and stop set the outcome instead of flipping it like a toggle:
    /// while processing they first withdraw any queued start, so repeats leave
    /// one queued start or none, and the latest start wins.
    fn on_external(&mut self, action: RecordingCommand, snapshot: &Snapshot) -> Option<InputEvent> {
        if matches!(snapshot, Snapshot::Processing { .. })
            && matches!(action, RecordingCommand::Start { .. } | RecordingCommand::Stop) {
            self.machine.on_cancel(false);
        }
        external_input(action, snapshot)
    }

    /// `keep_queued` is asked only when processing drains, so a remembered
    /// start runs only if the queue still applies at that moment.
    fn reconcile(&mut self, snapshot: Snapshot, blocked: bool, keep_queued: impl FnOnce() -> bool) -> Option<Effect> {
        match snapshot {
            Snapshot::Idle if !blocked => {
                if self.operation.take().is_some() {
                    if !keep_queued() { self.machine.on_cancel(false); }
                    return self.machine.on_processing_finished();
                }
            }
            Snapshot::Recording { binding, operation } => {
                if self.operation != Some(operation) {
                    // A different owner may have been admitted by the fork's CLI
                    // or another action. Old queued edges cannot control it.
                    self.machine = CoordinatorState::new();
                    self.machine.observe_recording(binding);
                    self.operation = Some(operation);
                    self.processing = false;
                }
            }
            Snapshot::Processing { operation } => {
                if self.operation != Some(operation) {
                    self.machine = CoordinatorState::new();
                    self.operation = Some(operation);
                    self.processing = false;
                }
                if !self.processing {
                    self.machine.observe_processing();
                    self.processing = true;
                }
            }
            Snapshot::Idle => {}
        }
        None
    }
}

fn snapshot(app: &AppHandle) -> Snapshot {
    let state = app.state::<ManagedSessionState>();
    let state = session_manager::lock_session_state(&state, "shortcut coordinator");
    match &*state {
        SessionState::Idle => Snapshot::Idle,
        SessionState::Recording { binding_id, operation_id, .. } =>
            Snapshot::Recording { binding: binding_id.clone(), operation: *operation_id },
        SessionState::Processing { operation_id, .. } => Snapshot::Processing { operation: *operation_id },
    }
}

fn run_effect(app: &AppHandle, runtime: &mut RuntimeState, effect: Effect) {
    match effect {
        Effect::Start { binding_id, hotkey_string } => {
            // Resolve application-aware profiles when the remembered press is
            // actually admitted, rather than retaining a stale busy-time choice.
            crate::actions::prepare_transcribe_settings(app, &binding_id);
            if let Some(action) = crate::actions::transcribe_action_for_binding(&binding_id) {
                action.start(app, &binding_id, &hotkey_string);
            }
            let started = match snapshot(app) {
                Snapshot::Recording { binding, operation } if binding == binding_id => {
                    runtime.operation = Some(operation);
                    runtime.processing = false;
                    true
                }
                _ => false,
            };
            runtime.machine.on_start_result(&binding_id, started);
        }
        Effect::Stop { binding_id, hotkey_string } => {
            runtime.processing = true;
            if let Snapshot::Recording { binding, operation } = snapshot(app) {
                if binding == binding_id {
                    if let Some(_guard) = session_manager::try_begin_operation_control(app, Some((&binding, operation))) {
                        if let Some(action) = crate::actions::transcribe_action_for_binding(&binding_id) {
                            if hotkey_string == "CLI_POST_PROCESS" {
                                crate::actions::force_post_process_for_next_transcription(&binding_id);
                            }
                            action.stop(app, &binding_id, &hotkey_string);
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn install(app: &AppHandle) {
    let (sender, receiver) = mpsc::channel();
    app.manage(Coordinator(sender));
    let app = app.clone();
    std::thread::Builder::new().name("transcription-shortcuts".into()).spawn(move || {
        let mut runtime = RuntimeState::new();
        loop {
            let command = if let Some(deadline) = runtime.machine.grace_deadline() {
                match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(command) => command,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(effect) = runtime.machine.on_grace_expired() {
                            run_effect(&app, &mut runtime, effect);
                        }
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match receiver.recv() { Ok(command) => command, Err(_) => break }
            };
            if matches!(command, Command::Cancel) {
                // The fork clears Processing immediately on cancellation, and
                // late async completions are generation-checked by its arbiter.
                runtime = RuntimeState::new();
                continue;
            }
            if let Some(effect) = runtime.reconcile(snapshot(&app), session_manager::recording_start_is_blocked(),
                || keep_queued_start(&app)) {
                run_effect(&app, &mut runtime, effect);
            }
            let input = match command {
                Command::Input(input) => Some(input),
                Command::Recording(action) => runtime.on_external(action, &snapshot(&app))
                    .map(|event| QueuedInput { event, received_at: Instant::now() }),
                _ => None,
            };
            if let Some(mut input) = input {
                let current = snapshot(&app);
                let blocked = session_manager::recording_start_is_blocked();
                let queue = queue_applies(&app, &current);
                if !allow_input(&input, &current, blocked, queue) { continue; }
                use_captured_activation(&app, &mut input);
                if let Some(effect) = runtime.on_input(input, &current, blocked, queue) {
                    run_effect(&app, &mut runtime, effect);
                }
            }
        }
    }).expect("create transcription shortcut coordinator");
}

pub(crate) fn is_transcribe_binding(binding: &str) -> bool {
    binding == "transcribe" || binding.starts_with("transcribe_")
}

pub(crate) fn send_input(app: &AppHandle, binding: &str, source: &str, pressed: bool, ptt: bool, auto: bool) {
    let mode = activation_mode(ptt, auto);
    let input = QueuedInput { received_at: Instant::now(), event: InputEvent {
        binding_id: binding.into(), hotkey_string: source.into(), is_pressed: pressed,
        mode, hold_threshold: Duration::from_millis(300), external: false } };
    // Check arrival as well as execution: an event rejected as busy must not
    // become a fresh start just because the executor reads it after drain.
    // With the queue on, a press during processing is admitted and remembered.
    let current = snapshot(app);
    let queue = queue_applies(app, &current);
    if !allow_input(&input, &current, session_manager::recording_start_is_blocked(), queue) { return; }
    send(app, Command::Input(input));
}

fn external_input(action: RecordingCommand, snapshot: &Snapshot) -> Option<InputEvent> {
    let should_toggle = match action {
        RecordingCommand::Toggle { .. } => true,
        RecordingCommand::Start { .. } => !matches!(snapshot, Snapshot::Recording { .. }),
        RecordingCommand::Stop => matches!(snapshot, Snapshot::Recording { .. }),
        RecordingCommand::Cancel => false,
    };
    if !should_toggle { return None; }
    let binding = match snapshot {
        Snapshot::Recording { binding, .. } if is_transcribe_binding(binding) => binding.as_str(),
        Snapshot::Recording { .. } => return None,
        _ => "transcribe",
    };
    let post_process = matches!(action, RecordingCommand::Start { post_process: true }
        | RecordingCommand::Toggle { post_process: true });
    Some(InputEvent { binding_id: binding.into(), hotkey_string: if post_process { "CLI_POST_PROCESS" } else { "CLI" }.into(),
        is_pressed: true, mode: ShortcutActivation::Toggle, hold_threshold: Duration::ZERO, external: true })
}

pub(crate) fn send_recording(app: &AppHandle, action: RecordingCommand) {
    let current = snapshot(app);
    if action != RecordingCommand::Cancel
        && ignore_busy_input(&current, session_manager::recording_start_is_blocked(), queue_applies(app, &current)) { return; }
    send(app, Command::Recording(action));
}

pub(crate) fn notify_session_changed(app: &AppHandle) { send(app, Command::SessionChanged); }
pub(crate) fn notify_cancel(app: &AppHandle) { send(app, Command::Cancel); }

fn send(app: &AppHandle, command: Command) {
    if let Some(coordinator) = app.try_state::<Coordinator>() {
        if coordinator.0.send(command).is_err() { log::error!("Transcription shortcut coordinator stopped"); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_start_waits_for_the_fork_completion_guard() {
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        runtime.machine.on_input(InputEvent { binding_id: "transcribe_profile".into(), hotkey_string: "test".into(),
            is_pressed: true, mode: ShortcutActivation::Toggle, hold_threshold: Duration::ZERO, external: false }, Instant::now());
        assert!(runtime.reconcile(Snapshot::Idle, true, || true).is_none());
        assert!(matches!(runtime.reconcile(Snapshot::Idle, false, || true), Some(Effect::Start { binding_id, .. }) if binding_id == "transcribe_profile"));
    }

    #[test]
    fn busy_presses_never_start_after_processing_drains() {
        for mode in [ShortcutActivation::Toggle, ShortcutActivation::PushToTalk, ShortcutActivation::HoldOrToggle] {
            for external in [false, true] {
                let mut runtime = RuntimeState::new();
                runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || false);
                let input = |pressed| QueuedInput { received_at: Instant::now(), event: InputEvent {
                    binding_id: "transcribe_work".into(), hotkey_string: "test".into(),
                    is_pressed: pressed, mode, hold_threshold: Duration::from_millis(300), external } };
                assert!(runtime.on_input(input(true), &Snapshot::Processing { operation: 7 }, false, false).is_none());
                assert!(runtime.on_input(input(false), &Snapshot::Processing { operation: 7 }, false, false).is_none());
                assert!(runtime.machine.grace_deadline().is_none());
                assert!(runtime.reconcile(Snapshot::Idle, false, || false).is_none(), "busy input must not queue a start");
                assert!(matches!(runtime.on_input(input(true), &Snapshot::Idle, false, false), Some(Effect::Start { .. })),
                    "a new press after processing must still start");
            }
        }
    }

    #[test]
    fn queued_press_starts_after_processing_only_while_the_queue_still_applies() {
        let processing = Snapshot::Processing { operation: 7 };
        let input = || QueuedInput { received_at: Instant::now(), event: InputEvent {
            binding_id: "transcribe_work".into(), hotkey_string: "test".into(), is_pressed: true,
            mode: ShortcutActivation::Toggle, hold_threshold: Duration::ZERO, external: false } };
        for keep in [true, false] {
            let mut runtime = RuntimeState::new();
            runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || false);
            assert!(runtime.on_input(input(), &processing, false, true).is_none());
            let drained = runtime.reconcile(Snapshot::Idle, false, || keep);
            assert_eq!(matches!(drained, Some(Effect::Start { ref binding_id, .. }) if binding_id == "transcribe_work"), keep,
                "an error, an open Preview or a disabled setting at drain must drop the queued start");
        }
    }

    #[test]
    fn queued_external_toggles_keep_parity() {
        let processing = Snapshot::Processing { operation: 7 };
        let toggle = || QueuedInput { received_at: Instant::now(),
            event: external_input(RecordingCommand::Toggle { post_process: false }, &Snapshot::Processing { operation: 7 }).unwrap() };
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        assert!(runtime.on_input(toggle(), &processing, false, true).is_none());
        assert!(runtime.on_input(toggle(), &processing, false, true).is_none());
        assert!(runtime.reconcile(Snapshot::Idle, false, || true).is_none(), "a second toggle withdraws the queued start");
    }

    #[test]
    fn completion_guard_does_not_remember_a_new_press() {
        for queue in [false, true] {
            let mut runtime = RuntimeState::new();
            runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
            assert!(runtime.reconcile(Snapshot::Idle, true, || true).is_none());
            let input = QueuedInput { received_at: Instant::now(), event: InputEvent {
                binding_id: "transcribe".into(), hotkey_string: "test".into(), is_pressed: true,
                mode: ShortcutActivation::Toggle, hold_threshold: Duration::ZERO, external: false } };
            assert!(runtime.on_input(input, &Snapshot::Idle, true, queue).is_none());
            assert!(runtime.reconcile(Snapshot::Idle, false, || true).is_none());
        }
    }

    #[test]
    fn busy_admission_keeps_recording_releases_and_late_releases_safe() {
        let recording = Snapshot::Recording { binding: "transcribe".into(), operation: 8 };
        assert!(ignore_busy_input(&Snapshot::Processing { operation: 7 }, false, false));
        assert!(!ignore_busy_input(&Snapshot::Processing { operation: 7 }, false, true));
        assert!(ignore_busy_input(&Snapshot::Idle, true, false));
        assert!(ignore_busy_input(&Snapshot::Idle, true, true));
        assert!(!ignore_busy_input(&Snapshot::Idle, false, false));
        assert!(!ignore_busy_input(&recording, true, false), "startup guard must not swallow an accepted key-up");

        let input = |pressed| QueuedInput { received_at: Instant::now(), event: InputEvent {
            binding_id: "transcribe".into(), hotkey_string: "test".into(), is_pressed: pressed,
            mode: ShortcutActivation::PushToTalk, hold_threshold: Duration::ZERO, external: false } };
        let mut accepted = RuntimeState::new();
        assert!(matches!(accepted.on_input(input(true), &Snapshot::Idle, false, false), Some(Effect::Start { .. })));
        assert!(accepted.on_input(input(false), &recording, true, false).is_none());
        assert!(matches!(accepted.machine.on_grace_expired(), Some(Effect::Stop { .. })),
            "a valid recording must stop on release even while startup owns its guard");

        let mut runtime = RuntimeState::new();
        runtime.reconcile(recording, true, || true);
        let release = QueuedInput { received_at: Instant::now(), event: InputEvent {
            binding_id: "transcribe".into(), hotkey_string: "test".into(), is_pressed: false,
            mode: ShortcutActivation::PushToTalk, hold_threshold: Duration::ZERO, external: false } };
        assert!(runtime.on_input(release, &Snapshot::Recording { binding: "transcribe".into(), operation: 8 }, true, false).is_none());
        assert!(runtime.machine.on_grace_expired().is_none(), "an ignored old press cannot stop a separately started recording");
    }

    #[test]
    fn new_operation_discards_an_older_queued_start() {
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        runtime.machine.on_input(InputEvent { binding_id: "transcribe_old".into(), hotkey_string: "test".into(),
            is_pressed: true, mode: ShortcutActivation::Toggle, hold_threshold: Duration::ZERO, external: false }, Instant::now());
        runtime.reconcile(Snapshot::Recording { binding: "transcribe_new".into(), operation: 8 }, false, || true);
        runtime.reconcile(Snapshot::Processing { operation: 8 }, false, || true);
        assert!(runtime.reconcile(Snapshot::Idle, false, || true).is_none());
    }

    #[test]
    fn external_start_is_idempotent_and_stop_targets_the_active_profile() {
        let active = Snapshot::Recording { binding: "transcribe_work".into(), operation: 4 };
        assert!(external_input(RecordingCommand::Start { post_process: false }, &active).is_none());
        assert_eq!(external_input(RecordingCommand::Stop, &active).unwrap().binding_id, "transcribe_work");
        assert!(external_input(RecordingCommand::Stop, &Snapshot::Idle).is_none());
        let stop = external_input(RecordingCommand::Toggle { post_process: true }, &active).unwrap();
        assert_eq!((stop.binding_id.as_str(), stop.hotkey_string.as_str()), ("transcribe_work", "CLI_POST_PROCESS"));
        assert_eq!(external_input(RecordingCommand::Start { post_process: false }, &Snapshot::Idle).unwrap().binding_id, "transcribe");
        let processing = Snapshot::Processing { operation: 7 };
        assert_eq!(external_input(RecordingCommand::Start { post_process: false }, &processing).unwrap().binding_id, "transcribe");
        assert!(external_input(RecordingCommand::Stop, &processing).is_none());
    }

    #[test]
    fn cold_start_delay_does_not_turn_a_queued_tap_into_a_hold() {
        let mut machine = CoordinatorState::new();
        let pressed_at = Instant::now();
        let input = |pressed, received_at| QueuedInput { received_at, event: InputEvent {
            binding_id: "transcribe".into(), hotkey_string: "test".into(), is_pressed: pressed,
            mode: ShortcutActivation::HoldOrToggle, hold_threshold: Duration::from_millis(300), external: false } };
        assert!(matches!(apply_input(&mut machine, input(true, pressed_at)), Some(Effect::Start { .. })));
        let release = input(false, pressed_at + Duration::from_millis(50));
        // This represents blocking startup after the OS already delivered the release.
        std::thread::sleep(Duration::from_millis(350));
        assert!(apply_input(&mut machine, release).is_none());
        assert!(machine.on_grace_expired().is_none(), "a short physical tap must lock recording on");
        assert!(matches!(apply_input(&mut machine, input(true, Instant::now())), Some(Effect::Stop { .. })));
    }

    #[test]
    fn automatic_profile_activation_is_kept_for_the_matching_recording() {
        let mut captured = crate::settings::get_default_settings();
        captured.push_to_talk = false;
        captured.auto_shortcut_activation = false;
        captured.transcription_profiles.push(serde_json::from_value(serde_json::json!({
            "id": "profile_work", "name": "Work", "language": "auto", "translate_to_english": false,
            "push_to_talk": true, "auto_shortcut_activation": true,
        })).unwrap());
        assert_eq!(captured_activation(&captured, Some("profile_work")), ShortcutActivation::HoldOrToggle);
        assert_eq!(captured_activation(&captured, None), ShortcutActivation::Toggle);
    }

    #[test]
    fn queued_ptt_tap_during_processing_starts_nothing_after_the_drain() {
        let processing = Snapshot::Processing { operation: 7 };
        let base = Instant::now();
        let input = |pressed, offset_ms| QueuedInput { received_at: base + Duration::from_millis(offset_ms), event: InputEvent {
            binding_id: "transcribe_work".into(), hotkey_string: "test".into(), is_pressed: pressed,
            mode: ShortcutActivation::PushToTalk, hold_threshold: Duration::from_millis(300), external: false } };
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        assert!(runtime.on_input(input(true, 0), &processing, false, true).is_none());
        assert!(runtime.on_input(input(false, 40), &processing, false, true).is_none());
        assert!(runtime.machine.grace_deadline().is_some(), "the busy release waits out the auto-repeat grace");
        assert!(runtime.machine.on_grace_expired().is_none());
        assert!(runtime.reconcile(Snapshot::Idle, false, || true).is_none(), "a push-to-talk tap that ended while busy has nothing left to start");
    }

    #[test]
    fn queued_ptt_press_still_held_at_the_drain_starts_that_binding() {
        let processing = Snapshot::Processing { operation: 7 };
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        let press = QueuedInput { received_at: Instant::now(), event: InputEvent {
            binding_id: "transcribe_work".into(), hotkey_string: "test".into(), is_pressed: true,
            mode: ShortcutActivation::PushToTalk, hold_threshold: Duration::from_millis(300), external: false } };
        assert!(runtime.on_input(press, &processing, false, true).is_none());
        assert_eq!(runtime.reconcile(Snapshot::Idle, false, || true),
            Some(Effect::Start { binding_id: "transcribe_work".into(), hotkey_string: "test".into() }),
            "a key held through the drain must start recording");
    }

    #[test]
    fn queued_hold_or_toggle_tap_during_processing_starts_after_the_drain() {
        let processing = Snapshot::Processing { operation: 7 };
        let base = Instant::now();
        let input = |pressed, offset_ms| QueuedInput { received_at: base + Duration::from_millis(offset_ms), event: InputEvent {
            binding_id: "transcribe_work".into(), hotkey_string: "test".into(), is_pressed: pressed,
            mode: ShortcutActivation::HoldOrToggle, hold_threshold: Duration::from_millis(300), external: false } };
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        assert!(runtime.on_input(input(true, 0), &processing, false, true).is_none());
        assert!(runtime.on_input(input(false, 100), &processing, false, true).is_none());
        assert!(runtime.machine.on_grace_expired().is_none());
        assert!(matches!(runtime.reconcile(Snapshot::Idle, false, || true), Some(Effect::Start { binding_id, .. }) if binding_id == "transcribe_work"),
            "a tap shorter than the hold threshold queues a start");
    }

    #[test]
    fn dropped_queued_start_leaves_no_stale_release_behind() {
        let processing = Snapshot::Processing { operation: 7 };
        let base = Instant::now();
        let input = |pressed, offset_ms| QueuedInput { received_at: base + Duration::from_millis(offset_ms), event: InputEvent {
            binding_id: "transcribe_work".into(), hotkey_string: "test".into(), is_pressed: pressed,
            mode: ShortcutActivation::PushToTalk, hold_threshold: Duration::from_millis(300), external: false } };
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        assert!(runtime.on_input(input(true, 0), &processing, false, true).is_none());
        assert!(runtime.on_input(input(false, 40), &processing, false, true).is_none());
        assert!(runtime.machine.grace_deadline().is_some());
        assert!(runtime.reconcile(Snapshot::Idle, false, || false).is_none(), "the queued start is dropped at the drain");
        assert!(runtime.machine.grace_deadline().is_none(), "dropping the queued start also drops its deferred release");
        assert!(matches!(runtime.on_input(input(true, 2000), &Snapshot::Idle, false, true), Some(Effect::Start { .. })));
        assert!(runtime.machine.on_grace_expired().is_none(), "an old release must not stop the next recording");
    }

    #[test]
    fn keep_queued_is_asked_only_when_processing_drains() {
        let calls = std::cell::Cell::new(0);
        let keep = || { calls.set(calls.get() + 1); true };
        let mut runtime = RuntimeState::new();
        assert!(runtime.reconcile(Snapshot::Idle, false, keep).is_none());
        runtime.reconcile(Snapshot::Recording { binding: "transcribe".into(), operation: 6 }, false, keep);
        runtime.reconcile(Snapshot::Idle, true, keep);
        runtime.reconcile(Snapshot::Processing { operation: 6 }, false, keep);
        runtime.reconcile(Snapshot::Processing { operation: 6 }, false, keep);
        runtime.reconcile(Snapshot::Idle, true, keep);
        assert_eq!(calls.get(), 0, "settings must not be read before processing drains, or while the completion guard blocks");
        assert!(runtime.reconcile(Snapshot::Idle, false, keep).is_none());
        assert_eq!(calls.get(), 1, "the real drain asks exactly once");
        assert!(runtime.reconcile(Snapshot::Idle, false, keep).is_none());
        assert_eq!(calls.get(), 1, "an idle session with no tracked operation never asks");
    }

    #[test]
    fn queued_external_toggle_during_processing_starts_after_the_drain_with_its_hotkey() {
        let processing = Snapshot::Processing { operation: 7 };
        for (post_process, hotkey) in [(false, "CLI"), (true, "CLI_POST_PROCESS")] {
            let toggle = QueuedInput { received_at: Instant::now(),
                event: external_input(RecordingCommand::Toggle { post_process }, &processing).unwrap() };
            let mut runtime = RuntimeState::new();
            runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
            assert!(runtime.on_input(toggle, &processing, false, true).is_none());
            assert_eq!(runtime.reconcile(Snapshot::Idle, false, || true),
                Some(Effect::Start { binding_id: "transcribe".into(), hotkey_string: hotkey.into() }),
                "the remembered CLI press must start the default binding and keep its post-process marker");
        }
    }

    #[test]
    fn external_input_never_controls_a_non_transcribe_recording() {
        for binding in ["voice_command", "send_to_extension"] {
            let active = Snapshot::Recording { binding: binding.into(), operation: 4 };
            assert!(external_input(RecordingCommand::Stop, &active).is_none(), "{binding} is owned by its own action");
            assert!(external_input(RecordingCommand::Toggle { post_process: false }, &active).is_none());
            assert!(external_input(RecordingCommand::Toggle { post_process: true }, &active).is_none());
        }
    }

    /// Mirrors the executor with the queue setting on.
    fn send_cli(runtime: &mut RuntimeState, action: RecordingCommand, snapshot: &Snapshot) -> Option<Effect> {
        let event = runtime.on_external(action, snapshot)?;
        runtime.on_input(QueuedInput { event, received_at: Instant::now() }, snapshot, false, true)
    }

    fn busy_press(mode: ShortcutActivation) -> QueuedInput {
        QueuedInput { received_at: Instant::now(), event: InputEvent {
            binding_id: "transcribe_work".into(), hotkey_string: "test".into(), is_pressed: true,
            mode, hold_threshold: Duration::from_millis(300), external: false } }
    }

    #[test]
    fn repeated_external_start_during_processing_queues_one_start_and_the_latest_wins() {
        let processing = Snapshot::Processing { operation: 7 };
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        assert!(runtime.on_input(busy_press(ShortcutActivation::Toggle), &processing, false, true).is_none());
        for post_process in [true, true, false] {
            assert!(send_cli(&mut runtime, RecordingCommand::Start { post_process }, &processing).is_none());
        }
        assert_eq!(runtime.reconcile(Snapshot::Idle, false, || true),
            Some(Effect::Start { binding_id: "transcribe".into(), hotkey_string: "CLI".into() }),
            "unlike toggles, repeated starts must not cancel each other");
    }

    #[test]
    fn external_stop_during_processing_withdraws_any_queued_start() {
        let processing = Snapshot::Processing { operation: 7 };
        for queued in ["cli start", "cli toggle", "toggle key", "held push-to-talk key"] {
            let mut runtime = RuntimeState::new();
            runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
            match queued {
                "cli start" => assert!(send_cli(&mut runtime, RecordingCommand::Start { post_process: true }, &processing).is_none()),
                "cli toggle" => assert!(send_cli(&mut runtime, RecordingCommand::Toggle { post_process: false }, &processing).is_none()),
                "toggle key" => assert!(runtime.on_input(busy_press(ShortcutActivation::Toggle), &processing, false, true).is_none()),
                _ => assert!(runtime.on_input(busy_press(ShortcutActivation::PushToTalk), &processing, false, true).is_none()),
            }
            assert!(send_cli(&mut runtime, RecordingCommand::Stop, &processing).is_none());
            assert!(send_cli(&mut runtime, RecordingCommand::Stop, &processing).is_none());
            assert!(runtime.reconcile(Snapshot::Idle, false, || true).is_none(), "stop must withdraw the queued {queued}");
        }
    }

    #[test]
    fn external_start_after_a_queued_toggle_still_starts_once() {
        let processing = Snapshot::Processing { operation: 7 };
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false, || true);
        assert!(send_cli(&mut runtime, RecordingCommand::Toggle { post_process: false }, &processing).is_none());
        assert!(send_cli(&mut runtime, RecordingCommand::Start { post_process: true }, &processing).is_none());
        assert_eq!(runtime.reconcile(Snapshot::Idle, false, || true),
            Some(Effect::Start { binding_id: "transcribe".into(), hotkey_string: "CLI_POST_PROCESS".into() }),
            "a start must not withdraw a toggle that already queued one");
    }
}
