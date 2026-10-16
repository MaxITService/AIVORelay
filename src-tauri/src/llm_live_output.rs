//! Prepares a streamed LLM answer for live insertion into the target window.
//!
//! Pieces pass through the same steps as a finished answer, in the same order:
//! zero-width filter, text replacements after the LLM, leading whitespace and
//! the one-shot decapitalize. Trailing whitespace is left to the clipboard's
//! held-whitespace handling, which knows where the stream ends.

use crate::settings::{
    apply_output_whitespace_policy_for_settings, AppSettings, OutputWhitespaceMode,
    TranscriptionProfile,
};
use crate::stream_chunk_processor::{stable_prefix_end, StreamChunkReplacementEngine};

/// Words held back while text replacements are active, so a rule spanning a
/// few words sees them together before any of them is inserted.
const REPLACEMENT_TAIL_WORDS: usize = 3;
const ZERO_WIDTH_CHARS: [char; 4] = ['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'];

/// Whether this profile inserts the streamed LLM answer while it is generated.
/// The default profile (`None`) uses the global setting.
pub(crate) fn live_stream_insert_enabled(
    settings: &AppSettings,
    profile: Option<&TranscriptionProfile>,
) -> bool {
    profile.map_or(settings.post_process_live_stream_insert, |profile| {
        profile.llm_live_stream_insert
    })
}

pub(crate) struct LlmLiveOutputProcessor {
    zero_width_filter: bool,
    replacements: Option<StreamChunkReplacementEngine>,
    leading_mode: OutputWhitespaceMode,
    leading_applied: bool,
    decapitalize: fn(&str) -> String,
    decapitalize_done: bool,
    /// Answer text received so far, after the zero-width filter.
    raw: String,
    /// End of the part of `raw` that was already released.
    raw_done: usize,
    /// Released text after replacements, before leading whitespace and decapitalize.
    replaced: String,
    /// Later text changed what replacements had already released, so the
    /// rest is replaced segment by segment.
    diverged: bool,
    /// Text handed out for insertion.
    emitted: String,
}

impl LlmLiveOutputProcessor {
    pub(crate) fn from_settings(settings: &AppSettings) -> Self {
        let replacements = (settings.text_replacements_enabled
            && !settings.text_replacements_before_llm)
            .then(|| StreamChunkReplacementEngine::from_replacements(&settings.text_replacements))
            .flatten();
        Self::new(
            settings.zero_width_filter_enabled,
            replacements,
            settings.output_whitespace_leading_mode,
            crate::text_replacement_decapitalize::maybe_decapitalize_next_chunk_standard,
        )
    }

    fn new(
        zero_width_filter: bool,
        replacements: Option<StreamChunkReplacementEngine>,
        leading_mode: OutputWhitespaceMode,
        decapitalize: fn(&str) -> String,
    ) -> Self {
        Self {
            zero_width_filter,
            replacements,
            leading_mode,
            leading_applied: false,
            decapitalize,
            decapitalize_done: false,
            raw: String::new(),
            raw_done: 0,
            replaced: String::new(),
            diverged: false,
            emitted: String::new(),
        }
    }

    /// Takes the next piece of the answer and returns the text to insert now.
    pub(crate) fn push(&mut self, piece: &str) -> String {
        if self.zero_width_filter {
            self.raw
                .extend(piece.chars().filter(|ch| !ZERO_WIDTH_CHARS.contains(ch)));
        } else {
            self.raw.push_str(piece);
        }
        let end = match self.replacements {
            Some(_) => stable_prefix_end(&self.raw, REPLACEMENT_TAIL_WORDS),
            None => self.raw.len(),
        };
        self.release(end, false)
    }

    /// Returns everything still held back once the answer is complete.
    pub(crate) fn finish(&mut self) -> String {
        self.release(self.raw.len(), true)
    }

    /// Answer text received so far, before replacements, as the LLM returned it.
    pub(crate) fn raw_text(&self) -> &str {
        &self.raw
    }

    /// Answer text after replacements, as History stores a processed result.
    pub(crate) fn replaced_text(&self) -> &str {
        &self.replaced
    }

    pub(crate) fn has_emitted_text(&self) -> bool {
        !self.emitted.is_empty()
    }

    /// The complete inserted text with the trailing whitespace policy, for the
    /// clipboard and Win+V history once insertion finishes.
    pub(crate) fn final_text(&self, settings: &AppSettings) -> String {
        apply_output_whitespace_policy_for_settings(&self.emitted, settings)
    }

    fn release(&mut self, end: usize, complete: bool) -> String {
        if end <= self.raw_done {
            return String::new();
        }
        let replaced_piece = match &self.replacements {
            None => self.raw[self.raw_done..end].to_string(),
            Some(engine) if self.diverged => engine.apply(&self.raw[self.raw_done..end]),
            Some(engine) => {
                let released = engine.apply(&self.raw[..end]);
                // Hold the text while the words after it still change it.
                if !complete && !engine.apply(&self.raw).starts_with(&released) {
                    return String::new();
                }
                match released.strip_prefix(self.replaced.as_str()) {
                    Some(piece) => piece.to_string(),
                    None => {
                        self.diverged = true;
                        engine.apply(&self.raw[self.raw_done..end])
                    }
                }
            }
        };
        self.raw_done = end;
        self.replaced.push_str(&replaced_piece);
        let piece = self.shape(replaced_piece);
        self.emitted.push_str(&piece);
        piece
    }

    fn shape(&mut self, mut text: String) -> String {
        if text.is_empty() {
            return text;
        }
        if !self.leading_applied {
            text = match self.leading_mode {
                OutputWhitespaceMode::Preserve => text,
                OutputWhitespaceMode::RemoveIfPresent => {
                    text.trim_start_matches(char::is_whitespace).to_string()
                }
                OutputWhitespaceMode::AddIfMissing => {
                    if text.starts_with(char::is_whitespace) {
                        text
                    } else {
                        format!(" {}", text)
                    }
                }
            };
            // Remove-if-present keeps trimming whitespace-only pieces until text arrives.
            self.leading_applied =
                self.leading_mode != OutputWhitespaceMode::RemoveIfPresent || !text.is_empty();
            if text.is_empty() {
                return text;
            }
        }
        // A finished answer is decapitalized once, at its first letter.
        if !self.decapitalize_done && text.chars().any(char::is_alphabetic) {
            self.decapitalize_done = true;
            text = (self.decapitalize)(&text);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{apply_text_replacements, get_default_settings, TextReplacement};

    fn keep(text: &str) -> String {
        text.to_string()
    }

    fn lowercase_all(text: &str) -> String {
        text.to_lowercase()
    }

    fn rule(from: &str, to: &str, is_regex: bool) -> TextReplacement {
        TextReplacement {
            id: format!("tr_{from}"),
            from: from.to_string(),
            to: to.to_string(),
            enabled: true,
            case_sensitive: true,
            is_regex,
        }
    }

    fn processor(rules: &[TextReplacement], leading: OutputWhitespaceMode) -> LlmLiveOutputProcessor {
        LlmLiveOutputProcessor::new(
            true,
            StreamChunkReplacementEngine::from_replacements(rules),
            leading,
            keep,
        )
    }

    fn stream(processor: &mut LlmLiveOutputProcessor, pieces: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = pieces.iter().map(|piece| processor.push(piece)).collect();
        out.push(processor.finish());
        out
    }

    #[test]
    fn pieces_pass_through_at_once_without_replacements() {
        let mut p = processor(&[], OutputWhitespaceMode::Preserve);
        assert_eq!(stream(&mut p, &["Hel", "lo, wor", "ld."]), ["Hel", "lo, wor", "ld.", ""]);
        assert_eq!(p.replaced_text(), "Hello, world.");
    }

    #[test]
    fn zero_width_characters_are_dropped_from_every_piece() {
        let mut p = processor(&[], OutputWhitespaceMode::Preserve);
        let out = stream(&mut p, &["\u{FEFF}Hi\u{200B}", "\u{200D}", " there\u{200C}"]);
        assert_eq!(out.concat(), "Hi there");
        assert_eq!(p.replaced_text(), "Hi there");
    }

    #[test]
    fn leading_whitespace_is_removed_across_whitespace_only_pieces() {
        let mut p = processor(&[], OutputWhitespaceMode::RemoveIfPresent);
        assert_eq!(stream(&mut p, &["\n", "  ", " Hi", " there "]), ["", "", "Hi", " there ", ""]);
    }

    #[test]
    fn a_missing_leading_space_is_added_once() {
        let mut p = processor(&[], OutputWhitespaceMode::AddIfMissing);
        assert_eq!(stream(&mut p, &["Hi", " there"]).concat(), " Hi there");
    }

    #[test]
    fn only_the_first_piece_with_a_letter_is_decapitalized() {
        let mut p = LlmLiveOutputProcessor::new(false, None, OutputWhitespaceMode::Preserve, lowercase_all);
        assert_eq!(stream(&mut p, &["1. ", "Hello", " World"]).concat(), "1. hello World");
    }

    #[test]
    fn a_rule_split_across_pieces_is_applied_before_insertion() {
        let rules = [rule("new line", "\n", false)];
        let mut p = processor(&rules, OutputWhitespaceMode::Preserve);
        let out = stream(&mut p, &["Say new", " li", "ne please", " and more words here"]);
        assert!(out.iter().all(|piece| !piece.contains("new")));
        assert_eq!(out.concat(), "Say \n please and more words here");
    }

    #[test]
    fn streamed_output_matches_the_finished_answer_pipeline() {
        let rules = [
            rule("teh", "the", false),
            rule(r"\s+$", "", true),
            rule(r"^Answer: ", "", true),
            rule("two words", "2w", false),
        ];
        let text = "Answer: teh quick two words fox jumps over teh lazy dog.  ";
        let mut settings = get_default_settings();
        settings.text_replacements_enabled = true;
        settings.text_replacements_before_llm = false;
        settings.text_replacements = rules.to_vec();
        settings.output_whitespace_leading_mode = OutputWhitespaceMode::RemoveIfPresent;
        settings.output_whitespace_trailing_mode = OutputWhitespaceMode::AddIfMissing;
        let expected = apply_output_whitespace_policy_for_settings(
            &apply_text_replacements(text, &settings.text_replacements),
            &settings,
        );
        for size in 1..=7 {
            let chars: Vec<char> = text.chars().collect();
            let pieces: Vec<String> = chars.chunks(size).map(|chunk| chunk.iter().collect()).collect();
            let mut p = LlmLiveOutputProcessor::from_settings(&settings);
            p.decapitalize = keep;
            for piece in &pieces {
                p.push(piece);
            }
            p.finish();
            assert_eq!(p.final_text(&settings), expected, "piece size {size}");
            assert_eq!(p.replaced_text(), apply_text_replacements(text, &settings.text_replacements));
        }
    }

    #[test]
    fn a_rule_that_changes_the_end_holds_the_text_until_the_answer_completes() {
        let rules = [rule("$", "!", true)];
        let mut p = processor(&rules, OutputWhitespaceMode::Preserve);
        let out = stream(&mut p, &["one ", "two ", "three ", "four ", "five"]);
        assert!(out[..5].iter().all(String::is_empty));
        assert_eq!(out[5], "one two three four five!");
    }

    #[test]
    fn a_rule_longer_than_the_held_words_loses_no_text() {
        let rules = [rule("one two three four five", "X", false)];
        let mut p = processor(&rules, OutputWhitespaceMode::Preserve);
        let out = stream(&mut p, &["one ", "two ", "three ", "four ", "five"]);
        assert_eq!(out.concat(), "one two three four five");
        assert_eq!(p.replaced_text(), "one two three four five");
    }

    #[test]
    fn replacements_before_the_llm_are_not_applied_to_the_answer() {
        let mut settings = get_default_settings();
        settings.text_replacements_enabled = true;
        settings.text_replacements_before_llm = true;
        settings.text_replacements = vec![rule("a", "b", false)];
        let mut p = LlmLiveOutputProcessor::from_settings(&settings);
        p.decapitalize = keep;
        assert_eq!(p.push("a cat"), "a cat");
    }

    #[test]
    fn the_profile_setting_overrides_the_global_one() {
        let mut settings = get_default_settings();
        settings.post_process_live_stream_insert = true;
        assert!(live_stream_insert_enabled(&settings, None));
        let mut profile: TranscriptionProfile = serde_json::from_value(serde_json::json!({
            "id": "profile_test", "name": "Test", "language": "auto", "translate_to_english": false
        }))
        .unwrap();
        assert!(!live_stream_insert_enabled(&settings, Some(&profile)));
        profile.llm_live_stream_insert = true;
        settings.post_process_live_stream_insert = false;
        assert!(live_stream_insert_enabled(&settings, Some(&profile)));
    }
}
