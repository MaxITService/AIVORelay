use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct IndicatorState {
    pub eligible: bool,
    pub armed: bool,
}

#[derive(Default)]
struct DecapitalizeState {
    realtime_trigger_until: Option<Instant>,
    standard_monitor_until: Option<Instant>,
    standard_output_armed: bool,
}

static DECAPITALIZE_STATE: Lazy<Mutex<DecapitalizeState>> =
    Lazy::new(|| Mutex::new(DecapitalizeState::default()));

#[derive(Clone, Copy)]
enum ApplyMode {
    RealtimeChunk,
    StandardOutput,
}

impl DecapitalizeState {
    fn arm_after_edit(&mut self, timeout_ms: u32, arm_standard_output: bool, now: Instant) {
        let timeout = Duration::from_millis(timeout_ms.max(1) as u64);
        self.realtime_trigger_until = Some(now + timeout);
        if arm_standard_output || self.cleanup_expired_standard_monitor(now) {
            self.standard_output_armed = true;
        }
    }

    fn begin_standard_monitor(&mut self, window_ms: u32, now: Instant) {
        self.standard_monitor_until = if window_ms == 0 {
            None
        } else {
            Some(now + Duration::from_millis(window_ms.max(1) as u64))
        };
    }

    fn cleanup_expired_realtime_trigger(&mut self, now: Instant) -> bool {
        match self.realtime_trigger_until {
            Some(deadline) if now <= deadline => true,
            Some(_) => {
                self.realtime_trigger_until = None;
                false
            }
            None => false,
        }
    }

    fn cleanup_expired_standard_monitor(&mut self, now: Instant) -> bool {
        match self.standard_monitor_until {
            Some(deadline) if now <= deadline => true,
            Some(_) => {
                self.standard_monitor_until = None;
                false
            }
            None => false,
        }
    }

    fn is_trigger_pending(&mut self, mode: ApplyMode, now: Instant) -> bool {
        let mut pending = self.cleanup_expired_realtime_trigger(now);
        if matches!(mode, ApplyMode::StandardOutput) {
            let _ = self.cleanup_expired_standard_monitor(now);
            pending |= self.standard_output_armed;
        }
        pending
    }

    fn consume(&mut self, mode: ApplyMode) {
        self.realtime_trigger_until = None;
        if matches!(mode, ApplyMode::StandardOutput) {
            self.standard_output_armed = false;
            self.standard_monitor_until = None;
        }
    }

    fn any_trigger_armed(&mut self, now: Instant) -> bool {
        self.cleanup_expired_realtime_trigger(now) || self.standard_output_armed
    }
}

/// Arms a one-shot decapitalize trigger for the next matching chunk.
///
/// `arm_standard_output` should be true for standard/non-live dictation so a
/// delayed final transcription can still consume the trigger. The post-stop
/// monitor window can also arm standard output after recording has stopped.
pub fn mark_edit_key_pressed(timeout_ms: u32, arm_standard_output: bool) {
    let now = Instant::now();
    if let Ok(mut state) = DECAPITALIZE_STATE.lock() {
        state.arm_after_edit(timeout_ms, arm_standard_output, now);
    }
}

/// Arms a limited post-recording monitor window for standard STT.
/// During this window, pressing the monitored key marks the next standard output
/// as eligible for decapitalization (one-shot).
pub fn begin_standard_post_recording_monitor(window_ms: u32) {
    if let Ok(mut state) = DECAPITALIZE_STATE.lock() {
        state.begin_standard_monitor(window_ms, Instant::now());
    }
}

/// Realtime/chunk mode: only uses the immediate timeout-based trigger.
pub fn maybe_decapitalize_next_chunk_realtime(text: &str) -> String {
    maybe_transform_next_chunk_impl(text, ApplyMode::RealtimeChunk, true)
}

/// Preview/interim mode: shows the next chunk as decapitalized without consuming
/// the one-shot trigger yet. The trigger is still consumed by the next finalized
/// realtime chunk or standard output.
pub fn preview_decapitalize_next_chunk_realtime(text: &str) -> String {
    maybe_transform_next_chunk_impl(text, ApplyMode::RealtimeChunk, false)
}

/// Standard STT mode: uses both immediate trigger and post-recording monitor trigger.
pub fn maybe_decapitalize_next_chunk_standard(text: &str) -> String {
    maybe_transform_next_chunk_impl(text, ApplyMode::StandardOutput, true)
}

/// Returns true when any decapitalize trigger is armed (realtime or standard).
/// Expired realtime timeout state is cleaned up on read.
pub fn is_any_trigger_armed_now() -> bool {
    let now = Instant::now();
    let Ok(mut state) = DECAPITALIZE_STATE.lock() else {
        return false;
    };

    state.any_trigger_armed(now)
}

pub fn indicator_state(enabled: bool) -> IndicatorState {
    IndicatorState {
        eligible: enabled,
        armed: enabled && is_any_trigger_armed_now(),
    }
}

fn maybe_transform_next_chunk_impl(text: &str, mode: ApplyMode, consume: bool) -> String {
    if text.is_empty() || !is_trigger_pending(mode) {
        return text.to_string();
    }

    let Some((idx, ch)) = find_first_alphabetic_char(text) else {
        return text.to_string();
    };

    if !ch.is_uppercase() {
        return text.to_string();
    }

    let lowered = ch.to_lowercase().to_string();
    if lowered == ch.to_string() {
        return text.to_string();
    }

    if consume {
        consume_trigger(mode);
    }

    let end = idx + ch.len_utf8();
    let mut out = String::with_capacity(text.len() - ch.len_utf8() + lowered.len());
    out.push_str(&text[..idx]);
    out.push_str(&lowered);
    out.push_str(&text[end..]);
    out
}

fn is_trigger_pending(mode: ApplyMode) -> bool {
    let now = Instant::now();
    let Ok(mut state) = DECAPITALIZE_STATE.lock() else {
        return false;
    };

    state.is_trigger_pending(mode, now)
}

fn consume_trigger(mode: ApplyMode) {
    if let Ok(mut state) = DECAPITALIZE_STATE.lock() {
        state.consume(mode);
    }
}

fn find_first_alphabetic_char(text: &str) -> Option<(usize, char)> {
    text.char_indices().find(|(_, ch)| ch.is_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn reset_global_state() {
        *DECAPITALIZE_STATE.lock().unwrap() = DecapitalizeState::default();
    }

    #[test]
    fn arm_after_edit_sets_realtime_deadline_and_optionally_standard_output() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();

        state.arm_after_edit(250, true, now);

        assert!(state.realtime_trigger_until.is_some());
        assert!(state.standard_output_armed);
    }

    #[test]
    fn arm_after_edit_uses_active_monitor_to_arm_standard_output() {
        let now = Instant::now();
        let mut state = DecapitalizeState {
            standard_monitor_until: Some(now + Duration::from_millis(500)),
            ..Default::default()
        };

        state.arm_after_edit(100, false, now);

        assert!(state.standard_output_armed);
    }

    #[test]
    fn begin_standard_monitor_zero_window_disables_monitor() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();

        state.begin_standard_monitor(0, now);

        assert_eq!(state.standard_monitor_until, None);
    }

    #[test]
    fn cleanup_expired_realtime_trigger_clears_expired_deadline() {
        let now = Instant::now();
        let mut state = DecapitalizeState {
            realtime_trigger_until: Some(now - Duration::from_millis(1)),
            ..Default::default()
        };

        assert!(!state.cleanup_expired_realtime_trigger(now));
        assert_eq!(state.realtime_trigger_until, None);
    }

    #[test]
    fn cleanup_expired_standard_monitor_clears_expired_deadline() {
        let now = Instant::now();
        let mut state = DecapitalizeState {
            standard_monitor_until: Some(now - Duration::from_millis(1)),
            ..Default::default()
        };

        assert!(!state.cleanup_expired_standard_monitor(now));
        assert_eq!(state.standard_monitor_until, None);
    }

    #[test]
    fn is_trigger_pending_for_standard_output_includes_armed_output_flag() {
        let now = Instant::now();
        let mut state = DecapitalizeState {
            standard_output_armed: true,
            ..Default::default()
        };

        assert!(state.is_trigger_pending(ApplyMode::StandardOutput, now));
        assert!(!state.is_trigger_pending(ApplyMode::RealtimeChunk, now));
    }

    #[test]
    fn consume_standard_output_clears_all_standard_state() {
        let now = Instant::now();
        let mut state = DecapitalizeState {
            realtime_trigger_until: Some(now + Duration::from_millis(100)),
            standard_monitor_until: Some(now + Duration::from_millis(100)),
            standard_output_armed: true,
        };

        state.consume(ApplyMode::StandardOutput);

        assert_eq!(state.realtime_trigger_until, None);
        assert_eq!(state.standard_monitor_until, None);
        assert!(!state.standard_output_armed);
    }

    #[test]
    fn find_first_alphabetic_char_skips_punctuation_and_space() {
        assert_eq!(find_first_alphabetic_char("  ...Hello"), Some((5, 'H')));
        assert_eq!(find_first_alphabetic_char("1234"), None);
    }

    #[test]
    fn realtime_preview_does_not_consume_trigger_until_finalized_chunk() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();

        mark_edit_key_pressed(500, false);

        assert_eq!(preview_decapitalize_next_chunk_realtime(" Hello"), " hello");
        assert!(is_any_trigger_armed_now());
        assert_eq!(maybe_decapitalize_next_chunk_realtime(" Hello"), " hello");
        assert!(!is_any_trigger_armed_now());

        reset_global_state();
    }

    #[test]
    fn standard_monitor_allows_edit_key_to_arm_standard_output() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();

        begin_standard_post_recording_monitor(500);
        mark_edit_key_pressed(500, false);

        assert_eq!(maybe_decapitalize_next_chunk_standard(" World"), " world");
        assert!(!is_any_trigger_armed_now());

        reset_global_state();
    }

    #[test]
    fn indicator_state_reports_disabled_as_unarmed() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();
        mark_edit_key_pressed(500, false);

        let disabled = indicator_state(false);
        let enabled = indicator_state(true);

        assert!(!disabled.eligible);
        assert!(!disabled.armed);
        assert!(enabled.eligible);
        assert!(enabled.armed);

        reset_global_state();
    }

    #[test]
    fn maybe_transform_next_chunk_impl_preserves_lowercase_and_non_alpha_inputs() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();

        assert_eq!(
            maybe_transform_next_chunk_impl(" already lower", ApplyMode::RealtimeChunk, false),
            " already lower"
        );
        assert_eq!(
            maybe_transform_next_chunk_impl("...123", ApplyMode::RealtimeChunk, false),
            "...123"
        );

        reset_global_state();
    }
}
