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

// Tests that consume this state must share the same guard as tests that arm it.
#[cfg(test)]
pub(crate) static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

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

    fn promote_realtime_trigger_to_standard_output(&mut self, now: Instant) -> bool {
        if self.cleanup_expired_realtime_trigger(now) {
            self.standard_output_armed = true;
            true
        } else {
            false
        }
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

/// Standard STT records first and emits text later. If the user pressed the
/// edit key shortly before starting the recording, latch that pending trigger
/// so it stays visible and applies to the eventual final output.
pub fn promote_pending_realtime_trigger_to_standard_output() -> bool {
    let now = Instant::now();
    let Ok(mut state) = DECAPITALIZE_STATE.lock() else {
        return false;
    };

    state.promote_realtime_trigger_to_standard_output(now)
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

    // The first check is only a fast path. Another output or an edit key can
    // run while finding the letter, so claiming the trigger must recheck it
    // under the same lock that consumes it.
    if !claim_trigger(mode, consume) {
        return text.to_string();
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

fn claim_trigger(mode: ApplyMode, consume: bool) -> bool {
    let Ok(mut state) = DECAPITALIZE_STATE.lock() else {
        return false;
    };
    if !state.is_trigger_pending(mode, Instant::now()) {
        return false;
    }
    if consume { state.consume(mode); }
    true
}

fn find_first_alphabetic_char(text: &str) -> Option<(usize, char)> {
    text.char_indices().find(|(_, ch)| ch.is_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_global_state() {
        *DECAPITALIZE_STATE.lock().unwrap() = DecapitalizeState::default();
    }

    #[test]
    fn realtime_trigger_expires_only_after_its_exact_deadline() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();
        state.arm_after_edit(100, false, now);
        let deadline = now + Duration::from_millis(100);

        assert!(state.is_trigger_pending(ApplyMode::RealtimeChunk, deadline));
        assert!(!state.is_trigger_pending(ApplyMode::RealtimeChunk, deadline + Duration::from_nanos(1)));
        assert!(state.realtime_trigger_until.is_none());
    }

    #[test]
    fn standard_output_latch_survives_a_slow_transcription_after_realtime_timeout() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();
        state.arm_after_edit(100, true, now);
        let final_output_time = now + Duration::from_secs(30);

        assert!(!state.is_trigger_pending(ApplyMode::RealtimeChunk, final_output_time));
        assert!(state.is_trigger_pending(ApplyMode::StandardOutput, final_output_time));
        state.consume(ApplyMode::StandardOutput);
        assert!(!state.any_trigger_armed(final_output_time));
    }

    #[test]
    fn consuming_a_realtime_chunk_preserves_the_independent_standard_output_latch() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();
        state.begin_standard_monitor(500, now);
        state.arm_after_edit(100, true, now);

        state.consume(ApplyMode::RealtimeChunk);
        assert!(!state.is_trigger_pending(ApplyMode::RealtimeChunk, now));
        assert!(state.is_trigger_pending(ApplyMode::StandardOutput, now));
        assert_eq!(state.standard_monitor_until, Some(now + Duration::from_millis(500)));

        state.consume(ApplyMode::StandardOutput);
        assert!(!state.standard_output_armed);
        assert!(state.standard_monitor_until.is_none());
    }

    #[test]
    fn an_edit_after_the_monitor_window_cannot_arm_delayed_standard_output() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();
        state.begin_standard_monitor(100, now);
        let edit_time = now + Duration::from_millis(101);
        state.arm_after_edit(50, false, edit_time);

        assert!(state.is_trigger_pending(ApplyMode::RealtimeChunk, edit_time));
        assert!(!state.standard_output_armed);
        assert!(state.standard_monitor_until.is_none());
        assert!(!state.is_trigger_pending(
            ApplyMode::StandardOutput, edit_time + Duration::from_millis(51)
        ));
    }

    #[test]
    fn arm_after_edit_sets_realtime_deadline_and_optionally_standard_output() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();

        state.arm_after_edit(250, true, now);

        assert_eq!(
            state.realtime_trigger_until,
            Some(now + Duration::from_millis(250))
        );
        assert!(state.standard_output_armed);

        let mut unarmed_state = DecapitalizeState::default();
        unarmed_state.arm_after_edit(250, false, now);

        assert_eq!(
            unarmed_state.realtime_trigger_until,
            Some(now + Duration::from_millis(250))
        );
        assert!(!unarmed_state.standard_output_armed);
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
    fn shared_live_chunk_pipeline_decapitalizes_once_after_preview_and_nonletter_chunks() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();
        let mut processor = crate::stream_chunk_processor::StreamChunkProcessor::default();
        mark_edit_key_pressed(60_000, false);

        assert_eq!(processor.push_chunk(""), "");
        assert_eq!(processor.push_chunk(" …123 "), " …123 ");
        assert!(is_any_trigger_armed_now());
        assert_eq!(preview_decapitalize_next_chunk_realtime("Привет"), "привет");
        assert_eq!(preview_decapitalize_next_chunk_realtime("Привет мир"), "привет мир");
        assert!(is_any_trigger_armed_now());

        assert_eq!(processor.push_chunk("Привет"), "привет");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" Мир."), " Мир.");
        assert_eq!(processor.flush(), "");

        reset_global_state();
    }

    #[test]
    fn shared_live_chunk_pipeline_preserves_trigger_until_buffered_tail_is_flushed() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();
        let mut settings = crate::settings::get_default_settings();
        settings.custom_words_enabled = true;
        settings.custom_words = vec!["AivoRelay".to_string()];
        settings.soniox_realtime_fuzzy_correction_enabled = true;
        settings.soniox_realtime_keep_safety_buffer_enabled = true;
        let mut processor = crate::stream_chunk_processor::StreamChunkProcessor::from_settings(&settings);
        mark_edit_key_pressed(60_000, false);

        assert_eq!(processor.push_chunk("Привет "), "");
        assert_eq!(processor.push_chunk("мир"), "");
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.flush_without_standalone_whitespace(), "привет мир");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.flush_without_standalone_whitespace(), "");
        assert_eq!(processor.push_chunk(" Ещё."), "");
        assert_eq!(processor.flush_without_standalone_whitespace(), " Ещё.");

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
    fn standard_recording_start_latches_pending_realtime_trigger() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();

        mark_edit_key_pressed(500, false);

        assert!(promote_pending_realtime_trigger_to_standard_output());
        assert!(is_any_trigger_armed_now());
        assert_eq!(maybe_decapitalize_next_chunk_standard(" Hello"), " hello");
        assert!(!is_any_trigger_armed_now());

        reset_global_state();
    }

    #[test]
    fn standard_recording_start_does_not_latch_expired_realtime_trigger() {
        let now = Instant::now();
        let mut state = DecapitalizeState {
            realtime_trigger_until: Some(now - Duration::from_millis(1)),
            ..Default::default()
        };

        assert!(!state.promote_realtime_trigger_to_standard_output(now));
        assert!(!state.standard_output_armed);
        assert_eq!(state.realtime_trigger_until, None);
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

    #[test]
    fn shared_live_chunk_pipeline_rearms_across_cyrillic_and_latin_chunks_in_one_recording() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();
        let mut processor = crate::stream_chunk_processor::StreamChunkProcessor::default();

        mark_edit_key_pressed(60_000, false);
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk("Привет"), "привет");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" Мир"), " Мир");

        mark_edit_key_pressed(60_000, false);
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" Hello"), " hello");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" World"), " World");

        mark_edit_key_pressed(60_000, false);
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" Ёжик"), " ёжик");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" Next"), " Next");

        mark_edit_key_pressed(60_000, false);
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" (Again)"), " (again)");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" Конец."), " Конец.");
        assert_eq!(processor.flush(), "");

        reset_global_state();
    }

    #[test]
    fn shared_live_chunk_pipeline_refreshes_single_trigger_and_skips_lowercase_nonletter_chunks() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();
        let mut processor = crate::stream_chunk_processor::StreamChunkProcessor::default();

        mark_edit_key_pressed(60_000, false);
        mark_edit_key_pressed(60_000, false);
        mark_edit_key_pressed(60_000, false);
        assert!(is_any_trigger_armed_now());

        assert_eq!(processor.push_chunk(" 123... "), " 123... ");
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk("lowercase "), "lowercase ");
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk("слово "), "слово ");
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk("— !? "), "— !? ");
        assert!(is_any_trigger_armed_now());

        assert_eq!(processor.push_chunk("Привет"), "привет");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" Second"), " Second");
        assert_eq!(processor.push_chunk(" Третье"), " Третье");
        assert!(!is_any_trigger_armed_now());

        mark_edit_key_pressed(60_000, false);
        mark_edit_key_pressed(60_000, false);
        assert!(is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" 45 и "), " 45 и ");
        assert!(is_any_trigger_armed_now());

        assert_eq!(processor.push_chunk("Final"), "final");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.push_chunk(" Tail"), " Tail");
        assert_eq!(processor.push_chunk(" Хвост"), " Хвост");
        assert_eq!(processor.flush(), "");

        reset_global_state();
    }

    #[test]
    fn arm_after_edit_clamps_zero_timeout_and_refreshes_single_deadline() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();

        state.arm_after_edit(0, false, now);
        let initial_deadline = now + Duration::from_millis(1);
        assert_eq!(state.realtime_trigger_until, Some(initial_deadline));
        assert!(state.is_trigger_pending(ApplyMode::RealtimeChunk, initial_deadline));

        let refresh_time = initial_deadline;
        state.arm_after_edit(50, false, refresh_time);
        let refreshed_deadline = refresh_time + Duration::from_millis(50);
        assert_eq!(state.realtime_trigger_until, Some(refreshed_deadline));
        assert!(state.is_trigger_pending(ApplyMode::RealtimeChunk, refreshed_deadline));
        assert!(!state.is_trigger_pending(
            ApplyMode::RealtimeChunk,
            refreshed_deadline + Duration::from_nanos(1)
        ));
        assert!(state.realtime_trigger_until.is_none());
    }

    #[test]
    fn standard_monitor_expires_only_after_its_exact_deadline() {
        let now = Instant::now();
        let mut state = DecapitalizeState::default();
        state.begin_standard_monitor(100, now);
        let deadline = now + Duration::from_millis(100);

        assert!(state.cleanup_expired_standard_monitor(deadline));
        assert_eq!(state.standard_monitor_until, Some(deadline));
        assert!(!state.cleanup_expired_standard_monitor(deadline + Duration::from_nanos(1)));
        assert!(state.standard_monitor_until.is_none());
    }

    #[test]
    fn concurrent_final_chunks_consume_one_shared_trigger_exactly_once() {
        use std::sync::{Arc, Barrier};
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();
        // A long nonletter prefix keeps the original split check/consume path
        // overlapping reliably; the helper accepts complete transcripts as well
        // as short realtime chunks. No real keyboard or output is used.
        let text = Arc::new(format!("{}Hello", " ".repeat(65_536)));
        let barrier = Arc::new(Barrier::new(5));
        let mut lowered_per_round = vec![0usize; 32];
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..4).map(|_| {
                let text = Arc::clone(&text);
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    let mut lowered = Vec::new();
                    for _ in 0..32 {
                        barrier.wait();
                        lowered.push(maybe_decapitalize_next_chunk_realtime(&text).ends_with("hello"));
                        barrier.wait();
                    }
                    lowered
                })
            }).collect();
            for _ in 0..32 {
                mark_edit_key_pressed(60_000, false);
                barrier.wait();
                barrier.wait();
            }
            for worker in workers {
                for (round, was_lowered) in worker.join().unwrap().into_iter().enumerate() {
                    lowered_per_round[round] += usize::from(was_lowered);
                }
            }
        });
        reset_global_state();
        assert_eq!(lowered_per_round, vec![1; 32], "each key press is a one-shot claim across output threads");
    }


    #[test]
    fn buffered_live_pipeline_rearms_through_nonletter_lowercase_chunks_and_final_flush() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_global_state();
        let mut settings = crate::settings::get_default_settings();
        settings.custom_words_enabled = true;
        settings.custom_words = vec!["AivoRelay".into()];
        settings.soniox_realtime_fuzzy_correction_enabled = true;
        settings.soniox_realtime_keep_safety_buffer_enabled = true;
        settings.output_whitespace_leading_mode = crate::settings::OutputWhitespaceMode::Preserve;
        let mut processor = crate::stream_chunk_processor::StreamChunkProcessor::from_settings(&settings);
        assert_eq!(processor.push_chunk("... 123 lower "), "");
        let mut output = String::new();
        mark_edit_key_pressed(60_000, false);
        for (chunk, expected) in [("Hello ", "... "), ("again ", "123 "), ("world ", "lower ")] {
            let delta = processor.push_chunk(chunk);
            assert_eq!(delta, expected);
            output.push_str(&delta);
            assert!(is_any_trigger_armed_now());
        }
        output.push_str(&processor.push_chunk("Done "));
        assert!(!is_any_trigger_armed_now());
        mark_edit_key_pressed(60_000, false);
        for chunk in ["Next ", "word "] {
            output.push_str(&processor.push_chunk(chunk));
            assert!(is_any_trigger_armed_now());
        }
        output.push_str(&processor.push_chunk("tail "));
        assert!(!is_any_trigger_armed_now());
        mark_edit_key_pressed(60_000, false);
        output.push_str(&processor.flush());
        assert_eq!(output, "... 123 lower hello again world done next word tail ");
        assert!(!is_any_trigger_armed_now());
        assert_eq!(processor.flush(), "");
        reset_global_state();
    }

}
