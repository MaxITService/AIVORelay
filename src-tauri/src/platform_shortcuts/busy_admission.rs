//! Input received while busy must not start later.
//! Keep rejected physical gestures suppressed through their release, including
//! X11's synthetic release/press repeats. External commands have no release.

use super::state::{InputEvent, RELEASE_GRACE};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Default)]
pub(super) struct InputAdmission {
    rejected: HashMap<String, RejectedPress>,
}

struct RejectedPress {
    released_at: Option<Instant>,
    last_event: Instant,
}

impl InputAdmission {
    pub(super) fn allows(&mut self, input: &InputEvent, now: Instant, busy: bool) -> bool {
        if input.external { return !busy; }
        self.rejected.retain(|_, press| {
            !press.released_at.is_some_and(|released| now.saturating_duration_since(released) >= RELEASE_GRACE)
        });
        if let Some(press) = self.rejected.get_mut(&input.binding_id) {
            // The executor can see an older queued edge after the producer
            // has already rejected a newer repeat. Do not rewind that hold.
            if now < press.last_event { return false; }
            press.last_event = now;
            if input.is_pressed { press.released_at = None; }
            else if press.released_at.is_none() { press.released_at = Some(now); }
            return false;
        }
        if busy {
            if input.is_pressed {
                self.rejected.insert(input.binding_id.clone(), RejectedPress { released_at: None, last_event: now });
            }
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::state::ShortcutActivation;
    use std::time::Duration;

    fn event(pressed: bool, external: bool) -> InputEvent {
        InputEvent { binding_id: "transcribe".into(), hotkey_string: "test".into(),
            is_pressed: pressed, mode: ShortcutActivation::PushToTalk,
            hold_threshold: Duration::ZERO, external }
    }

    #[test]
    fn rejected_hold_stays_rejected_after_drain_until_a_real_release() {
        let mut admission = InputAdmission::default();
        let now = Instant::now();
        assert!(!admission.allows(&event(true, false), now, true));
        assert!(!admission.allows(&event(true, false), now + Duration::from_secs(1), false));
        assert!(!admission.allows(&event(false, false), now + Duration::from_secs(2), false));
        assert!(admission.allows(&event(true, false), now + Duration::from_secs(3), false));
    }

    #[test]
    fn synthetic_x11_repeat_cannot_resurrect_a_rejected_press() {
        let mut admission = InputAdmission::default();
        let now = Instant::now();
        assert!(!admission.allows(&event(true, false), now, true));
        assert!(!admission.allows(&event(false, false), now + Duration::from_millis(10), false));
        assert!(!admission.allows(&event(true, false), now + Duration::from_millis(20), false));
        assert!(!admission.allows(&event(false, false), now + Duration::from_millis(30), false));
        assert!(!admission.allows(&event(false, false), now + Duration::from_millis(40), false));
        assert!(admission.allows(&event(true, false), now + Duration::from_millis(81), false),
            "a duplicate release must not extend the first release grace");
    }

    #[test]
    fn external_commands_are_rejected_only_while_busy() {
        let mut admission = InputAdmission::default();
        let now = Instant::now();
        assert!(!admission.allows(&event(true, true), now, true));
        assert!(admission.allows(&event(true, true), now, false));
    }

    #[test]
    fn delayed_executor_release_does_not_clear_a_newer_rejected_hold() {
        let mut admission = InputAdmission::default();
        let now = Instant::now();
        assert!(!admission.allows(&event(true, false), now, true));
        assert!(!admission.allows(&event(true, false), now + Duration::from_secs(1), true));
        assert!(!admission.allows(&event(false, false), now + Duration::from_millis(10), false));
        assert!(!admission.allows(&event(true, false), now + Duration::from_secs(2), false));
        assert!(!admission.allows(&event(false, false), now + Duration::from_secs(3), false));
        assert!(admission.allows(&event(true, false), now + Duration::from_secs(4), false));
    }

    #[test]
    fn unowned_release_does_not_block_a_new_gesture() {
        let mut admission = InputAdmission::default();
        let now = Instant::now();
        assert!(!admission.allows(&event(false, false), now, true));
        assert!(admission.allows(&event(true, false), now, false));
    }
}
