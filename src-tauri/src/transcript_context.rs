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
