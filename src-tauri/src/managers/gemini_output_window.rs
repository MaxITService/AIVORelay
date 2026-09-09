use std::sync::Mutex;
use std::time::Instant;

/// One recording's delivery deadline; never shared with its successor.
#[derive(Default)]
pub(crate) struct GeminiOutputWindow {
    state: Mutex<(Option<Instant>, String)>,
}

impl GeminiOutputWindow {
    pub(crate) fn finish_at(&self, deadline: Instant) {
        self.state.lock().unwrap().0 = Some(deadline);
    }

    pub(crate) fn allows_delivery(&self) -> bool {
        self.allows_delivery_at(Instant::now())
    }

    fn allows_delivery_at(&self, now: Instant) -> bool {
        self.state.lock().unwrap().0.is_none_or(|deadline| now < deadline)
    }

    pub(crate) fn accept_chunk(&self, chunk: String) -> String {
        self.accept_chunk_at(chunk, Instant::now())
    }

    fn accept_chunk_at(&self, chunk: String, now: Instant) -> String {
        let mut state = self.state.lock().unwrap();
        let Some(deadline) = state.0 else { return chunk; };
        if now >= deadline {
            return String::new();
        }
        // During the grace period, attach separators to the next real chunk.
        // Never send an isolated segment-completion space to a new selection.
        if chunk.trim().is_empty() {
            state.1.push_str(&chunk);
            return String::new();
        }
        let mut pending = std::mem::take(&mut state.1);
        pending.push_str(&chunk);
        pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn disabled_option_preserves_recording_chunks() {
        let window = GeminiOutputWindow::default();
        assert_eq!(window.accept_chunk("Sentence.".into()), "Sentence.");
        assert_eq!(window.accept_chunk(" ".into()), " ");
        assert!(window.allows_delivery());
    }

    #[test]
    fn grace_period_attaches_separator_to_next_words() {
        let window = GeminiOutputWindow::default();
        let now = Instant::now();
        window.finish_at(now + Duration::from_millis(500));
        assert_eq!(window.accept_chunk_at(" ".into(), now), "");
        assert_eq!(window.accept_chunk_at("Next sentence.".into(), now), " Next sentence.");
        assert_eq!(window.accept_chunk_at(" ".into(), now), "");
    }

    #[test]
    fn deadline_blocks_late_text_and_segment_space() {
        let window = GeminiOutputWindow::default();
        let deadline = Instant::now();
        window.finish_at(deadline);
        assert_eq!(window.accept_chunk_at(" ".into(), deadline), "");
        assert_eq!(window.accept_chunk_at("Late words".into(), deadline), "");
        assert!(!window.allows_delivery_at(deadline));
    }

    #[test]
    fn chunk_queued_before_deadline_cannot_deliver_after_it() {
        let window = GeminiOutputWindow::default();
        let now = Instant::now();
        let deadline = now + Duration::from_millis(500);
        window.finish_at(deadline);
        assert_eq!(window.accept_chunk_at("Words".into(), now), "Words");
        assert!(!window.allows_delivery_at(deadline + Duration::from_millis(1)));
    }

    #[test]
    fn new_recording_does_not_inherit_pending_space() {
        let old = GeminiOutputWindow::default();
        let now = Instant::now();
        old.finish_at(now + Duration::from_secs(1));
        assert_eq!(old.accept_chunk_at(" ".into(), now), "");
        let new = GeminiOutputWindow::default();
        assert_eq!(new.accept_chunk("New field".into()), "New field");
    }
}
