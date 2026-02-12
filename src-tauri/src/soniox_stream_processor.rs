use crate::audio_toolkit::apply_custom_words;
use crate::settings::{AppSettings, TextReplacement};
use log::warn;
use regex::Regex;

const DEFAULT_STABLE_TAIL_WORDS: usize = 3;

#[derive(Clone)]
enum StreamReplacementRule {
    Plain {
        from: String,
        to: String,
        case_sensitive: bool,
    },
    Regex {
        regex: Regex,
        to: String,
    },
}

#[derive(Clone, Default)]
struct StreamChunkReplacementEngine {
    rules: Vec<StreamReplacementRule>,
}

impl StreamChunkReplacementEngine {
    fn from_settings(settings: &AppSettings) -> Option<Self> {
        if !settings.text_replacements_enabled || settings.text_replacements.is_empty() {
            return None;
        }
        Self::from_replacements(&settings.text_replacements)
    }

    fn from_replacements(replacements: &[TextReplacement]) -> Option<Self> {
        let mut rules = Vec::new();
        for replacement in replacements
            .iter()
            .filter(|replacement| replacement.enabled && !replacement.from.is_empty())
        {
            let to_processed = process_text_replacement_escapes(&replacement.to);
            if replacement.is_regex {
                let pattern = if replacement.case_sensitive {
                    replacement.from.clone()
                } else {
                    format!("(?i){}", replacement.from)
                };

                match Regex::new(&pattern) {
                    Ok(regex) => rules.push(StreamReplacementRule::Regex {
                        regex,
                        to: to_processed,
                    }),
                    Err(err) => {
                        warn!(
                            "Invalid regex pattern '{}' in stream text replacement: {}",
                            replacement.from, err
                        );
                    }
                }
                continue;
            }

            let from_processed = process_text_replacement_escapes(&replacement.from);
            if from_processed.is_empty() {
                continue;
            }

            rules.push(StreamReplacementRule::Plain {
                from: from_processed,
                to: to_processed,
                case_sensitive: replacement.case_sensitive,
            });
        }

        if rules.is_empty() {
            None
        } else {
            Some(Self { rules })
        }
    }

    fn apply(&self, text: &str) -> String {
        let mut result = text.to_string();
        for rule in &self.rules {
            result = match rule {
                StreamReplacementRule::Plain {
                    from,
                    to,
                    case_sensitive,
                } => {
                    if *case_sensitive {
                        result.replace(from, to)
                    } else {
                        replace_case_insensitive(&result, from, to)
                    }
                }
                StreamReplacementRule::Regex { regex, to } => {
                    regex.replace_all(&result, to.as_str()).to_string()
                }
            };
        }
        result
    }
}

#[derive(Clone, Default)]
pub struct SonioxStreamProcessor {
    pending_raw: String,
    stable_tail_words: usize,
    fuzzy_enabled: bool,
    custom_words: Vec<String>,
    word_correction_threshold: f64,
    custom_words_ngram_enabled: bool,
    replacements: Option<StreamChunkReplacementEngine>,
}

impl SonioxStreamProcessor {
    pub fn from_settings(settings: &AppSettings) -> Self {
        let fuzzy_enabled = settings.custom_words_enabled
            && !settings.custom_words.is_empty()
            && settings.soniox_realtime_fuzzy_correction_enabled;
        let stable_tail_words = if settings.soniox_realtime_keep_safety_buffer_enabled {
            DEFAULT_STABLE_TAIL_WORDS
        } else {
            0
        };

        Self {
            pending_raw: String::new(),
            stable_tail_words,
            fuzzy_enabled,
            custom_words: settings.custom_words.clone(),
            word_correction_threshold: settings.word_correction_threshold,
            custom_words_ngram_enabled: settings.custom_words_ngram_enabled,
            replacements: StreamChunkReplacementEngine::from_settings(settings),
        }
    }

    pub fn push_chunk(&mut self, raw_chunk: &str) -> String {
        if raw_chunk.is_empty() {
            return String::new();
        }

        self.pending_raw.push_str(raw_chunk);
        let stable_end = stable_prefix_end(&self.pending_raw, self.stable_tail_words);
        if stable_end == 0 {
            return String::new();
        }

        let stable_raw = self.pending_raw[..stable_end].to_string();
        self.pending_raw.drain(..stable_end);
        self.process_pipeline(&stable_raw)
    }

    pub fn flush(&mut self) -> String {
        if self.pending_raw.is_empty() {
            return String::new();
        }

        let remaining = std::mem::take(&mut self.pending_raw);
        self.process_pipeline(&remaining)
    }

    fn process_pipeline(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }

        // Order is fixed for Soniox realtime chunks:
        // fuzzy custom words -> text replacements -> paste delta.
        let corrected = if self.fuzzy_enabled {
            apply_custom_words_preserving_whitespace(
                text,
                &self.custom_words,
                self.word_correction_threshold,
                self.custom_words_ngram_enabled,
            )
        } else {
            text.to_string()
        };

        let processed = match &self.replacements {
            Some(engine) => engine.apply(&corrected),
            None => corrected,
        };

        crate::text_replacement_decapitalize::maybe_decapitalize_next_chunk_realtime(&processed)
    }
}

fn apply_custom_words_preserving_whitespace(
    text: &str,
    custom_words: &[String],
    threshold: f64,
    enable_ngram: bool,
) -> String {
    if text.is_empty() || custom_words.is_empty() {
        return text.to_string();
    }

    let leading_bytes = count_leading_whitespace_bytes(text);
    let trailing_bytes = count_trailing_whitespace_bytes(text);
    let core_end = text.len().saturating_sub(trailing_bytes);
    let core = if leading_bytes <= core_end {
        &text[leading_bytes..core_end]
    } else {
        ""
    };

    if core.is_empty() {
        return text.to_string();
    }

    // apply_custom_words() tokenizes by whitespace and rejoins with single spaces.
    // Skip fuzzy for this chunk if internal whitespace is non-trivial so we do not
    // normalize tabs/newlines or repeated spaces in streaming output.
    let has_complex_whitespace = core.contains("  ")
        || core.chars().any(|c| matches!(c, '\n' | '\r' | '\t'));
    if has_complex_whitespace {
        return text.to_string();
    }

    let corrected_core = apply_custom_words(core, custom_words, threshold, enable_ngram);
    let mut out = String::with_capacity(text.len() + corrected_core.len());
    out.push_str(&text[..leading_bytes]);
    out.push_str(&corrected_core);
    out.push_str(&text[core_end..]);
    out
}

fn count_leading_whitespace_bytes(text: &str) -> usize {
    let mut total = 0;
    for ch in text.chars() {
        if !ch.is_whitespace() {
            break;
        }
        total += ch.len_utf8();
    }
    total
}

fn count_trailing_whitespace_bytes(text: &str) -> usize {
    let mut total = 0;
    for ch in text.chars().rev() {
        if !ch.is_whitespace() {
            break;
        }
        total += ch.len_utf8();
    }
    total
}

fn stable_prefix_end(text: &str, tail_words: usize) -> usize {
    if text.is_empty() {
        return 0;
    }
    if tail_words == 0 {
        return text.len();
    }

    let mut token_starts: Vec<usize> = Vec::new();
    let mut in_token = false;

    for (idx, ch) in text.char_indices() {
        if ch.is_whitespace() {
            in_token = false;
            continue;
        }
        if !in_token {
            token_starts.push(idx);
            in_token = true;
        }
    }

    if token_starts.len() <= tail_words {
        0
    } else {
        token_starts[token_starts.len() - tail_words]
    }
}

fn process_text_replacement_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => {
                    result.push('\n');
                    chars.next();
                }
                Some('r') => {
                    chars.next();
                    if chars.peek() == Some(&'\\') {
                        let mut temp = chars.clone();
                        temp.next();
                        if temp.peek() == Some(&'n') {
                            result.push_str("\r\n");
                            chars.next();
                            chars.next();
                        } else {
                            result.push('\r');
                        }
                    } else {
                        result.push('\r');
                    }
                }
                Some('t') => {
                    result.push('\t');
                    chars.next();
                }
                Some('\\') => {
                    result.push('\\');
                    chars.next();
                }
                Some('u') => {
                    chars.next();
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        let mut hex_str = String::new();
                        while let Some(&ch) = chars.peek() {
                            if ch == '}' {
                                chars.next();
                                break;
                            }
                            if ch.is_ascii_hexdigit() && hex_str.len() < 6 {
                                hex_str.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        if let Ok(code_point) = u32::from_str_radix(&hex_str, 16) {
                            if let Some(unicode_char) = char::from_u32(code_point) {
                                result.push(unicode_char);
                            } else {
                                result.push_str("\\u{");
                                result.push_str(&hex_str);
                                result.push('}');
                            }
                        } else {
                            result.push_str("\\u{");
                            result.push_str(&hex_str);
                            result.push('}');
                        }
                    } else {
                        result.push('\\');
                        result.push('u');
                    }
                }
                _ => result.push(c),
            }
        } else {
            result.push(c);
        }
    }

    result
}

fn replace_case_insensitive(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return text.to_string();
    }

    let pattern = format!("(?i){}", regex::escape(from));
    match Regex::new(&pattern) {
        Ok(re) => re.replace_all(text, regex::NoExpand(to)).into_owned(),
        Err(err) => {
            warn!(
                "Failed to build case-insensitive replacement regex for '{}': {}",
                from, err
            );
            text.to_string()
        }
    }
}
