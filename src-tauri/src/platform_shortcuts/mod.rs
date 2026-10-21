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

fn ignore_busy_input(snapshot: &Snapshot, blocked: bool) -> bool {
    matches!(snapshot, Snapshot::Processing { .. })
        || (matches!(snapshot, Snapshot::Idle) && blocked)
}

fn allow_input(input: &QueuedInput, snapshot: &Snapshot, blocked: bool) -> bool {
    static ADMISSION: std::sync::OnceLock<std::sync::Mutex<busy_admission::InputAdmission>> = std::sync::OnceLock::new();
    ADMISSION.get_or_init(|| std::sync::Mutex::new(busy_admission::InputAdmission::default()))
        .lock().unwrap_or_else(|error| error.into_inner())
        .allows(&input.event, input.received_at, ignore_busy_input(snapshot, blocked))
}

struct RuntimeState {
    machine: CoordinatorState,
    operation: Option<u64>,
    processing: bool,
}

impl RuntimeState {
    fn new() -> Self { Self { machine: CoordinatorState::new(), operation: None, processing: false } }

    fn on_input(&mut self, input: QueuedInput, snapshot: &Snapshot, blocked: bool) -> Option<Effect> {
        // Busy presses are discarded, not remembered for an automatic start
        // after the previous operation.
        // Recording releases must still pass while microphone startup owns its guard.
        if ignore_busy_input(snapshot, blocked) { return None; }
        apply_input(&mut self.machine, input)
    }

    fn reconcile(&mut self, snapshot: Snapshot, blocked: bool) -> Option<Effect> {
        match snapshot {
            Snapshot::Idle if !blocked => {
                if self.operation.take().is_some() {
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
            if let Some(effect) = runtime.reconcile(snapshot(&app), session_manager::recording_start_is_blocked()) {
                run_effect(&app, &mut runtime, effect);
            }
            let input = match command {
                Command::Input(input) => Some(input),
                Command::Recording(action) => external_input(action, &snapshot(&app))
                    .map(|event| QueuedInput { event, received_at: Instant::now() }),
                _ => None,
            };
            if let Some(mut input) = input {
                let current = snapshot(&app);
                let blocked = session_manager::recording_start_is_blocked();
                if !allow_input(&input, &current, blocked) { continue; }
                use_captured_activation(&app, &mut input);
                if let Some(effect) = runtime.on_input(input, &current, blocked) {
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
    // Check arrival as well as execution: a busy event must not become a
    // fresh start just because the executor reads it after drain.
    if !allow_input(&input, &snapshot(app), session_manager::recording_start_is_blocked()) { return; }
    send(app, Command::Input(input));
}

fn external_input(action: RecordingCommand, snapshot: &Snapshot) -> Option<InputEvent> {
    let should_toggle = match action {
        RecordingCommand::Toggle { .. } => true,
        RecordingCommand::Start { .. } => matches!(snapshot, Snapshot::Idle),
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
    if action != RecordingCommand::Cancel
        && ignore_busy_input(&snapshot(app), session_manager::recording_start_is_blocked()) { return; }
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
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false);
        runtime.machine.on_input(InputEvent { binding_id: "transcribe_profile".into(), hotkey_string: "test".into(),
            is_pressed: true, mode: ShortcutActivation::Toggle, hold_threshold: Duration::ZERO, external: false }, Instant::now());
        assert!(runtime.reconcile(Snapshot::Idle, true).is_none());
        assert!(matches!(runtime.reconcile(Snapshot::Idle, false), Some(Effect::Start { binding_id, .. }) if binding_id == "transcribe_profile"));
    }

    #[test]
    fn busy_presses_never_start_after_processing_drains() {
        for mode in [ShortcutActivation::Toggle, ShortcutActivation::PushToTalk, ShortcutActivation::HoldOrToggle] {
            for external in [false, true] {
                let mut runtime = RuntimeState::new();
                runtime.reconcile(Snapshot::Processing { operation: 7 }, false);
                let input = |pressed| QueuedInput { received_at: Instant::now(), event: InputEvent {
                    binding_id: "transcribe_work".into(), hotkey_string: "test".into(),
                    is_pressed: pressed, mode, hold_threshold: Duration::from_millis(300), external } };
                assert!(runtime.on_input(input(true), &Snapshot::Processing { operation: 7 }, false).is_none());
                assert!(runtime.on_input(input(false), &Snapshot::Processing { operation: 7 }, false).is_none());
                assert!(runtime.machine.grace_deadline().is_none());
                assert!(runtime.reconcile(Snapshot::Idle, false).is_none(), "busy input must not queue a start");
                assert!(matches!(runtime.on_input(input(true), &Snapshot::Idle, false), Some(Effect::Start { .. })),
                    "a new press after processing must still start");
            }
        }
    }

    #[test]
    fn completion_guard_does_not_remember_a_new_press() {
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false);
        assert!(runtime.reconcile(Snapshot::Idle, true).is_none());
        let input = QueuedInput { received_at: Instant::now(), event: InputEvent {
            binding_id: "transcribe".into(), hotkey_string: "test".into(), is_pressed: true,
            mode: ShortcutActivation::Toggle, hold_threshold: Duration::ZERO, external: false } };
        assert!(runtime.on_input(input, &Snapshot::Idle, true).is_none());
        assert!(runtime.reconcile(Snapshot::Idle, false).is_none());
    }

    #[test]
    fn busy_admission_keeps_recording_releases_and_late_releases_safe() {
        let recording = Snapshot::Recording { binding: "transcribe".into(), operation: 8 };
        assert!(ignore_busy_input(&Snapshot::Processing { operation: 7 }, false));
        assert!(ignore_busy_input(&Snapshot::Idle, true));
        assert!(!ignore_busy_input(&Snapshot::Idle, false));
        assert!(!ignore_busy_input(&recording, true), "startup guard must not swallow an accepted key-up");

        let input = |pressed| QueuedInput { received_at: Instant::now(), event: InputEvent {
            binding_id: "transcribe".into(), hotkey_string: "test".into(), is_pressed: pressed,
            mode: ShortcutActivation::PushToTalk, hold_threshold: Duration::ZERO, external: false } };
        let mut accepted = RuntimeState::new();
        assert!(matches!(accepted.on_input(input(true), &Snapshot::Idle, false), Some(Effect::Start { .. })));
        assert!(accepted.on_input(input(false), &recording, true).is_none());
        assert!(matches!(accepted.machine.on_grace_expired(), Some(Effect::Stop { .. })),
            "a valid recording must stop on release even while startup owns its guard");

        let mut runtime = RuntimeState::new();
        runtime.reconcile(recording, true);
        let release = QueuedInput { received_at: Instant::now(), event: InputEvent {
            binding_id: "transcribe".into(), hotkey_string: "test".into(), is_pressed: false,
            mode: ShortcutActivation::PushToTalk, hold_threshold: Duration::ZERO, external: false } };
        assert!(runtime.on_input(release, &Snapshot::Recording { binding: "transcribe".into(), operation: 8 }, true).is_none());
        assert!(runtime.machine.on_grace_expired().is_none(), "an ignored old press cannot stop a separately started recording");
    }

    #[test]
    fn new_operation_discards_an_older_queued_start() {
        let mut runtime = RuntimeState::new();
        runtime.reconcile(Snapshot::Processing { operation: 7 }, false);
        runtime.machine.on_input(InputEvent { binding_id: "transcribe_old".into(), hotkey_string: "test".into(),
            is_pressed: true, mode: ShortcutActivation::Toggle, hold_threshold: Duration::ZERO, external: false }, Instant::now());
        runtime.reconcile(Snapshot::Recording { binding: "transcribe_new".into(), operation: 8 }, false);
        runtime.reconcile(Snapshot::Processing { operation: 8 }, false);
        assert!(runtime.reconcile(Snapshot::Idle, false).is_none());
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
}
