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
        assert_eq!(format_srt_time(3661.999), "01:01:02,000"); // rounds up
    }

    #[test]
    fn test_vtt_time_format() {
        assert_eq!(format_vtt_time(0.0), "00:00:00.000");
        assert_eq!(format_vtt_time(1.5), "00:00:01.500");
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
}
