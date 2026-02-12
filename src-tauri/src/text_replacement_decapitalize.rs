use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
struct DecapitalizeState {
    pending_until: Option<Instant>,
    standard_post_recording_monitor_until: Option<Instant>,
    standard_post_recording_pending: bool,
}

static DECAPITALIZE_STATE: Lazy<Mutex<DecapitalizeState>> =
    Lazy::new(|| Mutex::new(DecapitalizeState::default()));

#[derive(Clone, Copy)]
enum ApplyMode {
    RealtimeChunk,
    StandardOutput,
}

/// Arms a one-shot decapitalize trigger for the next matching chunk.
pub fn mark_edit_key_pressed(timeout_ms: u32) {
    let now = Instant::now();
    let timeout = Duration::from_millis(timeout_ms.max(1) as u64);
    if let Ok(mut state) = DECAPITALIZE_STATE.lock() {
        state.pending_until = Some(now + timeout);

        if is_standard_post_recording_monitor_active(&mut state, now) {
            state.standard_post_recording_pending = true;
        }
    }
}

/// Arms a limited post-recording monitor window for standard STT.
/// During this window, pressing the monitored key marks the next standard output
/// as eligible for decapitalization (one-shot).
pub fn begin_standard_post_recording_monitor(window_ms: u32) {
    if let Ok(mut state) = DECAPITALIZE_STATE.lock() {
        if window_ms == 0 {
            state.standard_post_recording_monitor_until = None;
            state.standard_post_recording_pending = false;
            return;
        }

        let window = Duration::from_millis(window_ms.max(1) as u64);
        state.standard_post_recording_monitor_until = Some(Instant::now() + window);
        state.standard_post_recording_pending = false;
    }
}

/// Realtime/chunk mode: only uses the immediate timeout-based trigger.
pub fn maybe_decapitalize_next_chunk_realtime(text: &str) -> String {
    maybe_decapitalize_next_chunk_impl(text, ApplyMode::RealtimeChunk)
}

/// Standard STT mode: uses both immediate trigger and post-recording monitor trigger.
pub fn maybe_decapitalize_next_chunk_standard(text: &str) -> String {
    maybe_decapitalize_next_chunk_impl(text, ApplyMode::StandardOutput)
}

fn maybe_decapitalize_next_chunk_impl(text: &str, mode: ApplyMode) -> String {
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

    consume_trigger(mode);

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

    let mut pending = match state.pending_until {
        Some(deadline) if now <= deadline => true,
        Some(_) => {
            state.pending_until = None;
            false
        }
        None => false,
    };

    if matches!(mode, ApplyMode::StandardOutput) {
        let _monitor_active = is_standard_post_recording_monitor_active(&mut state, now);
        pending |= state.standard_post_recording_pending;
    }

    pending
}

fn consume_trigger(mode: ApplyMode) {
    if let Ok(mut state) = DECAPITALIZE_STATE.lock() {
        state.pending_until = None;

        if matches!(mode, ApplyMode::StandardOutput) {
            state.standard_post_recording_pending = false;
            state.standard_post_recording_monitor_until = None;
        }
    }
}

fn is_standard_post_recording_monitor_active(state: &mut DecapitalizeState, now: Instant) -> bool {
    match state.standard_post_recording_monitor_until {
        Some(deadline) if now <= deadline => true,
        Some(_) => {
            state.standard_post_recording_monitor_until = None;
            state.standard_post_recording_pending = false;
            false
        }
        None => false,
    }
}

fn find_first_alphabetic_char(text: &str) -> Option<(usize, char)> {
    text.char_indices().find(|(_, ch)| ch.is_alphabetic())
}
