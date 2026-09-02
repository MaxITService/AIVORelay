use log::debug;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct TranscriptEntry {
    text: String,
    last_updated: Instant,
}

static TRANSCRIPT_CONTEXT: Lazy<Mutex<HashMap<String, TranscriptEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn get_short_prev_transcript(app_name: &str, max_words: usize, expiry: Duration) -> String {
    if app_name.trim().is_empty() || max_words == 0 {
        return String::new();
    }

    let mut context = match TRANSCRIPT_CONTEXT.lock() {
        Ok(guard) => guard,
        Err(e) => {
            debug!("Failed to lock transcript context: {}", e);
            return String::new();
        }
    };

    cleanup_expired_entries(&mut context, expiry);

    context
        .get(app_name)
        .map(|entry| trim_to_last_words(&entry.text, max_words))
        .unwrap_or_default()
}

pub fn update_transcript_context(
    app_name: &str,
    transcript: &str,
    max_words: usize,
    expiry: Duration,
) {
    if app_name.trim().is_empty() || transcript.trim().is_empty() || max_words == 0 {
        return;
    }

    let mut context = match TRANSCRIPT_CONTEXT.lock() {
        Ok(guard) => guard,
        Err(e) => {
            debug!("Failed to lock transcript context for update: {}", e);
            return;
        }
    };

    cleanup_expired_entries(&mut context, expiry);

    let incoming = trim_to_last_words(transcript, max_words);
    let entry = context
        .entry(app_name.to_string())
        .or_insert_with(|| TranscriptEntry {
            text: String::new(),
            last_updated: Instant::now(),
        });

    if !entry.text.is_empty() {
        let combined = format!("{} {}", entry.text, incoming);
        entry.text = trim_to_last_words(&combined, max_words);
    } else {
        entry.text = incoming;
    }

    entry.last_updated = Instant::now();
}

fn trim_to_last_words(text: &str, max_words: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= max_words {
        words.join(" ")
    } else {
        words[words.len() - max_words..].join(" ")
    }
}

fn cleanup_expired_entries(context: &mut HashMap<String, TranscriptEntry>, expiry: Duration) {
    let expired: Vec<String> = context
        .iter()
        .filter(|(_, entry)| entry.last_updated.elapsed() >= expiry)
        .map(|(key, _)| key.clone())
        .collect();

    for key in expired {
        context.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trimming_normalizes_whitespace_and_keeps_complete_unicode_words() {
        assert_eq!(
            trim_to_last_words("  one\nдва\t三   four  ", 3),
            "два 三 four"
        );
        assert_eq!(trim_to_last_words("one two", 5), "one two");
        assert_eq!(trim_to_last_words("one two", 0), "");
    }

    #[test]
    fn cleanup_removes_only_entries_at_or_beyond_expiry() {
        let now = Instant::now();
        let mut context = HashMap::from([
            (
                "expired".to_string(),
                TranscriptEntry {
                    text: "old".to_string(),
                    last_updated: now - Duration::from_secs(61),
                },
            ),
            (
                "fresh".to_string(),
                TranscriptEntry {
                    text: "new".to_string(),
                    last_updated: now,
                },
            ),
        ]);

        cleanup_expired_entries(&mut context, Duration::from_secs(60));

        assert!(!context.contains_key("expired"));
        assert_eq!(
            context.get("fresh").map(|entry| entry.text.as_str()),
            Some("new")
        );
    }

    #[test]
    fn transcript_updates_accumulate_with_a_word_limit_and_isolate_apps() {
        let app_a = format!("transcript-context-test-a-{}", std::process::id());
        let app_b = format!("transcript-context-test-b-{}", std::process::id());
        let expiry = Duration::from_secs(60);

        update_transcript_context(&app_a, "one two three", 4, expiry);
        update_transcript_context(&app_a, "four five", 4, expiry);
        update_transcript_context(&app_b, "separate value", 4, expiry);

        assert_eq!(
            get_short_prev_transcript(&app_a, 4, expiry),
            "two three four five"
        );
        assert_eq!(get_short_prev_transcript(&app_a, 2, expiry), "four five");
        assert_eq!(
            get_short_prev_transcript(&app_b, 4, expiry),
            "separate value"
        );

        let mut context = TRANSCRIPT_CONTEXT.lock().unwrap();
        context.remove(&app_a);
        context.remove(&app_b);
    }

    #[test]
    fn invalid_context_updates_are_ignored_without_creating_entries() {
        let app = format!("transcript-context-invalid-test-{}", std::process::id());
        let expiry = Duration::from_secs(60);

        update_transcript_context("   ", "text", 5, expiry);
        update_transcript_context(&app, "   ", 5, expiry);
        update_transcript_context(&app, "text", 0, expiry);

        assert_eq!(get_short_prev_transcript(&app, 5, expiry), "");
        assert_eq!(get_short_prev_transcript(&app, 0, expiry), "");
        assert!(!TRANSCRIPT_CONTEXT.lock().unwrap().contains_key(&app));
    }
}
