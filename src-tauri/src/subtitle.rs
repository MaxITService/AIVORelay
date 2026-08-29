//! Subtitle format generation (SRT, VTT)
//!
//! Converts transcription segments with timestamps to standard subtitle formats.

use serde::{Deserialize, Serialize};
use specta::Type;

/// A transcription segment with timing information
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SubtitleSegment {
    /// Start time in seconds
    pub start: f32,
    /// End time in seconds
    pub end: f32,
    /// The transcribed text for this segment
    pub text: String,
}

/// A provider token with audio-relative timing. `prepend_space` is used by
/// word-based APIs; token-stream APIs such as Soniox already encode spacing in
/// the token text itself.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedTranscriptToken {
    pub start: f32,
    pub end: f32,
    pub text: String,
    pub prepend_space: bool,
}

const SUBTITLE_MAX_CUE_DURATION_SECONDS: f32 = 6.0;
const SUBTITLE_SPLIT_PAUSE_SECONDS: f32 = 0.8;
const SUBTITLE_MAX_CUE_CHARS: usize = 84;

fn ends_sentence(text: &str) -> bool {
    text.trim_end()
        .chars()
        .next_back()
        .is_some_and(|ch| matches!(ch, '.' | '!' | '?' | '。' | '！' | '？'))
}

fn push_token_text(target: &mut String, token: &TimedTranscriptToken) {
    if token.prepend_space
        && !target.is_empty()
        && !target.chars().next_back().is_some_and(char::is_whitespace)
    {
        target.push(' ');
    }
    target.push_str(&token.text);
}

/// Groups provider word/token timings into readable subtitle cues while
/// preserving the provider's real audio alignment.
pub fn timed_tokens_to_subtitle_segments(tokens: &[TimedTranscriptToken]) -> Vec<SubtitleSegment> {
    let valid_tokens = tokens.iter().filter(|token| {
        token.start.is_finite()
            && token.end.is_finite()
            && token.start >= 0.0
            && token.end >= token.start
            && !token.text.is_empty()
    });

    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut current_start = 0.0;
    let mut current_end = 0.0;

    let flush = |segments: &mut Vec<SubtitleSegment>,
                 current_text: &mut String,
                 current_start: f32,
                 current_end: f32| {
        let text = current_text.trim();
        if !text.is_empty() {
            segments.push(SubtitleSegment {
                start: current_start,
                end: current_end,
                text: text.to_string(),
            });
        }
        current_text.clear();
    };

    for token in valid_tokens {
        let mut candidate = current_text.clone();
        push_token_text(&mut candidate, token);
        let has_current = !current_text.is_empty();
        let pause = token.start - current_end;
        let would_be_too_long =
            has_current && token.end - current_start > SUBTITLE_MAX_CUE_DURATION_SECONDS;
        let would_be_too_wide = has_current && candidate.chars().count() > SUBTITLE_MAX_CUE_CHARS;

        if has_current
            && (pause >= SUBTITLE_SPLIT_PAUSE_SECONDS || would_be_too_long || would_be_too_wide)
        {
            flush(&mut segments, &mut current_text, current_start, current_end);
        }

        if current_text.is_empty() {
            current_start = token.start;
            current_end = token.end;
        } else {
            current_end = current_end.max(token.end);
        }
        push_token_text(&mut current_text, token);

        if ends_sentence(&current_text) && current_end - current_start >= 1.0 {
            flush(&mut segments, &mut current_text, current_start, current_end);
        }
    }

    if !current_text.is_empty() {
        flush(&mut segments, &mut current_text, current_start, current_end);
    }

    segments
}

/// Output format for transcription
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Plain text (default)
    #[default]
    Text,
    /// SRT subtitle format
    Srt,
    /// WebVTT subtitle format
    Vtt,
}

/// Format seconds to SRT timestamp (HH:MM:SS,mmm)
fn format_srt_time(seconds: f32) -> String {
    let total_ms = (seconds * 1000.0).round() as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let secs = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, ms)
}

/// Format seconds to VTT timestamp (HH:MM:SS.mmm)
fn format_vtt_time(seconds: f32) -> String {
    let total_ms = (seconds * 1000.0).round() as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let secs = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, secs, ms)
}

/// Convert segments to SRT format
pub fn segments_to_srt(segments: &[SubtitleSegment]) -> String {
    segments
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                i + 1,
                format_srt_time(seg.start),
                format_srt_time(seg.end),
                seg.text.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert segments to WebVTT format
pub fn segments_to_vtt(segments: &[SubtitleSegment]) -> String {
    let mut output = String::from("WEBVTT\n\n");

    for (i, seg) in segments.iter().enumerate() {
        output.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            format_vtt_time(seg.start),
            format_vtt_time(seg.end),
            seg.text.trim()
        ));
    }

    output
}

/// Get the file extension for an output format
pub fn get_format_extension(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Text => "txt",
        OutputFormat::Srt => "srt",
        OutputFormat::Vtt => "vtt",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_time_format() {
        assert_eq!(format_srt_time(0.0), "00:00:00,000");
        assert_eq!(format_srt_time(1.5), "00:00:01,500");
        assert_eq!(format_srt_time(61.234), "00:01:01,234");
        assert_eq!(format_srt_time(3661.9994), "01:01:02,000"); // rounds up with f32 input
    }

    #[test]
    fn test_vtt_time_format() {
        assert_eq!(format_vtt_time(0.0), "00:00:00.000");
        assert_eq!(format_vtt_time(1.5), "00:00:01.500");
    }

    #[test]
    fn test_vtt_time_format_rounds_up_with_representable_f32_boundary() {
        assert_eq!(format_vtt_time(3661.9994), "01:01:02.000");
    }

    #[test]
    fn test_segments_to_srt() {
        let segments = vec![
            SubtitleSegment {
                start: 0.0,
                end: 2.5,
                text: "Hello world".to_string(),
            },
            SubtitleSegment {
                start: 2.5,
                end: 5.0,
                text: "Goodbye".to_string(),
            },
        ];
        let srt = segments_to_srt(&segments);
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:02,500\nHello world"));
        assert!(srt.contains("2\n00:00:02,500 --> 00:00:05,000\nGoodbye"));
    }

    #[test]
    fn test_segments_to_srt_trims_surrounding_whitespace() {
        let segments = vec![SubtitleSegment {
            start: 0.0,
            end: 1.0,
            text: "  Hello world  ".to_string(),
        }];
        let srt = segments_to_srt(&segments);
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:01,000\nHello world"));
    }

    #[test]
    fn non_diarized_subtitles_remain_unlabelled() {
        let segments = vec![SubtitleSegment {
            start: 0.0,
            end: 1.0,
            text: "Hello world".to_string(),
        }];

        let srt = segments_to_srt(&segments);
        let vtt = segments_to_vtt(&segments);
        assert!(srt.contains("\nHello world\n"));
        assert!(vtt.contains("\nHello world\n"));
        assert!(!srt.contains("[Speaker"));
        assert!(!vtt.contains("[Speaker"));
    }

    #[test]
    fn test_segments_to_vtt() {
        let segments = vec![SubtitleSegment {
            start: 0.0,
            end: 2.5,
            text: "Hello world".to_string(),
        }];
        let vtt = segments_to_vtt(&segments);
        assert!(vtt.starts_with("WEBVTT\n"));
        assert!(vtt.contains("00:00:00.000 --> 00:00:02.500"));
    }

    #[test]
    fn test_segments_to_vtt_trims_surrounding_whitespace() {
        let segments = vec![SubtitleSegment {
            start: 0.0,
            end: 1.0,
            text: "  Hello world  ".to_string(),
        }];
        let vtt = segments_to_vtt(&segments);
        assert!(vtt.contains("1\n00:00:00.000 --> 00:00:01.000\nHello world\n\n"));
    }

    #[test]
    fn test_get_format_extension_returns_expected_values() {
        assert_eq!(get_format_extension(OutputFormat::Text), "txt");
        assert_eq!(get_format_extension(OutputFormat::Srt), "srt");
        assert_eq!(get_format_extension(OutputFormat::Vtt), "vtt");
    }

    #[test]
    fn timed_tokens_preserve_real_boundaries_and_split_on_pause() {
        let tokens = vec![
            TimedTranscriptToken {
                start: 0.2,
                end: 0.6,
                text: "Hello".to_string(),
                prepend_space: true,
            },
            TimedTranscriptToken {
                start: 0.6,
                end: 1.0,
                text: "world".to_string(),
                prepend_space: true,
            },
            TimedTranscriptToken {
                start: 2.0,
                end: 2.4,
                text: "Again".to_string(),
                prepend_space: true,
            },
        ];

        let segments = timed_tokens_to_subtitle_segments(&tokens);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start, 0.2);
        assert_eq!(segments[0].end, 1.0);
        assert_eq!(segments[0].text, "Hello world");
        assert_eq!(segments[1].start, 2.0);
        assert_eq!(segments[1].text, "Again");
    }

    #[test]
    fn timed_tokens_can_preserve_provider_encoded_spacing() {
        let tokens = vec![
            TimedTranscriptToken {
                start: 0.0,
                end: 0.2,
                text: "Beau".to_string(),
                prepend_space: false,
            },
            TimedTranscriptToken {
                start: 0.2,
                end: 0.4,
                text: "ti".to_string(),
                prepend_space: false,
            },
            TimedTranscriptToken {
                start: 0.4,
                end: 0.7,
                text: "ful".to_string(),
                prepend_space: false,
            },
            TimedTranscriptToken {
                start: 0.7,
                end: 1.0,
                text: " day.".to_string(),
                prepend_space: false,
            },
        ];

        let segments = timed_tokens_to_subtitle_segments(&tokens);
        assert_eq!(segments[0].text, "Beautiful day.");
        assert_eq!(segments[0].end, 1.0);
    }
}
