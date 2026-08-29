use log::warn;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::subtitle::{
    segments_to_srt, segments_to_vtt, timed_tokens_to_subtitle_segments, OutputFormat,
    SubtitleSegment, TimedTranscriptToken,
};

const ARTIFACT_DIR_NAME: &str = "aivorelay-file-transcription-speakers";
const ARTIFACT_TTL: Duration = Duration::from_secs(60 * 60 * 24);
pub const UNATTRIBUTED_SPEAKER_KEY: &str = "__aivorelay_unattributed__";
pub const UNATTRIBUTED_SPEAKER_NAME: &str = "Unknown speaker";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSpeakerBlock {
    pub speaker_key: String,
    pub default_name: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizedTranscriptBlock {
    pub speaker_id: u32,
    pub default_name: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawDiarizedTranscriptWord {
    pub speaker_key: Option<String>,
    pub default_name: Option<String>,
    pub text: String,
    pub start: f32,
    pub end: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizedSubtitleSegment {
    pub speaker_id: Option<u32>,
    pub default_name: Option<String>,
    pub start: f32,
    pub end: f32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DiarizedTranscriptProvider {
    Deepgram,
    Soniox,
    Gemini,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileTranscriptionSpeaker {
    pub speaker_id: u32,
    pub default_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileTranscriptionSpeakerSession {
    pub artifact_path: String,
    pub provider: DiarizedTranscriptProvider,
    pub speakers: Vec<FileTranscriptionSpeaker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileTranscriptionSpeakerNameInput {
    pub speaker_id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiarizedTranscriptArtifact {
    provider: DiarizedTranscriptProvider,
    blocks: Vec<DiarizedTranscriptBlock>,
    #[serde(default)]
    output_format: OutputFormat,
    #[serde(default)]
    subtitle_segments: Vec<DiarizedSubtitleSegment>,
}

pub fn normalize_raw_diarized_words(
    raw_words: Vec<RawDiarizedTranscriptWord>,
) -> (Vec<DiarizedTranscriptBlock>, Vec<DiarizedSubtitleSegment>) {
    let mut speaker_metadata: HashMap<String, (u32, String)> = HashMap::new();
    let mut blocks = Vec::new();
    for word in &raw_words {
        let text = normalize_block_text(&word.text);
        if text.is_empty() {
            continue;
        }
        let attributed_speaker = word
            .speaker_key
            .as_deref()
            .map(str::trim)
            .filter(|speaker| !speaker.is_empty());
        let speaker_key = attributed_speaker.unwrap_or(UNATTRIBUTED_SPEAKER_KEY);
        if !speaker_metadata.contains_key(speaker_key) {
            let speaker_id = speaker_metadata.len() as u32;
            let default_name = attributed_speaker
                .and_then(|_| word.default_name.as_deref())
                .map(normalize_block_text)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| {
                    if attributed_speaker.is_some() {
                        fallback_default_name(speaker_id)
                    } else {
                        UNATTRIBUTED_SPEAKER_NAME.to_string()
                    }
                });
            speaker_metadata.insert(speaker_key.to_string(), (speaker_id, default_name));
        }

        if let Some((speaker_id, default_name)) = speaker_metadata.get(speaker_key) {
            push_or_merge_block(
                &mut blocks,
                *speaker_id,
                default_name.clone(),
                text,
            );
        }
    }

    let mut segments = Vec::new();
    let mut run_words: Vec<&RawDiarizedTranscriptWord> = Vec::new();
    let mut run_speaker: Option<(u32, String)> = None;
    let mut run_key: Option<String> = None;

    let flush_run = |segments: &mut Vec<DiarizedSubtitleSegment>,
                     run_words: &mut Vec<&RawDiarizedTranscriptWord>,
                     run_speaker: &Option<(u32, String)>| {
        if run_words.is_empty() {
            return;
        }
        let tokens = run_words
            .iter()
            .map(|word| TimedTranscriptToken {
                start: word.start,
                end: word.end,
                text: word.text.clone(),
                prepend_space: true,
            })
            .collect::<Vec<_>>();
        for segment in timed_tokens_to_subtitle_segments(&tokens) {
            segments.push(DiarizedSubtitleSegment {
                speaker_id: run_speaker.as_ref().map(|(speaker_id, _)| *speaker_id),
                default_name: run_speaker.as_ref().map(|(_, name)| name.clone()),
                start: segment.start,
                end: segment.end,
                text: segment.text,
            });
        }
        run_words.clear();
    };

    for word in &raw_words {
        let word_key = word
            .speaker_key
            .as_deref()
            .map(str::trim)
            .filter(|speaker| !speaker.is_empty())
            .unwrap_or(UNATTRIBUTED_SPEAKER_KEY)
            .to_string();
        if !run_words.is_empty() && run_key.as_deref() != Some(word_key.as_str()) {
            flush_run(&mut segments, &mut run_words, &run_speaker);
        }
        if run_words.is_empty() {
            run_speaker = speaker_metadata.get(&word_key).cloned();
            run_key = Some(word_key);
        }
        run_words.push(word);
    }
    flush_run(&mut segments, &mut run_words, &run_speaker);

    (blocks, segments)
}

pub fn normalize_raw_speaker_blocks(
    raw_blocks: Vec<RawSpeakerBlock>,
) -> Vec<DiarizedTranscriptBlock> {
    let mut speaker_ids = HashMap::new();
    let mut default_names = HashMap::new();
    let mut next_speaker_id = 0u32;
    let mut blocks = Vec::new();

    for raw_block in raw_blocks {
        let speaker_key = raw_block.speaker_key.trim();
        if speaker_key.is_empty() {
            continue;
        }

        let text = normalize_block_text(&raw_block.text);
        if text.is_empty() {
            continue;
        }

        let speaker_id = *speaker_ids
            .entry(speaker_key.to_string())
            .or_insert_with(|| {
                let assigned_id = next_speaker_id;
                next_speaker_id += 1;
                assigned_id
            });
        default_names
            .entry(speaker_key.to_string())
            .or_insert_with(|| {
                raw_block
                    .default_name
                    .as_deref()
                    .map(normalize_block_text)
                    .filter(|default_name| !default_name.is_empty())
                    .unwrap_or_else(|| fallback_default_name(speaker_id))
            });
        let default_name = default_names
            .get(speaker_key)
            .cloned()
            .unwrap_or_else(|| fallback_default_name(speaker_id));

        push_or_merge_block(&mut blocks, speaker_id, default_name, text);
    }

    blocks
}

pub fn create_diarized_transcript_session(
    provider: DiarizedTranscriptProvider,
    blocks: Vec<DiarizedTranscriptBlock>,
    output_format: OutputFormat,
    subtitle_segments: Vec<DiarizedSubtitleSegment>,
) -> Result<Option<(FileTranscriptionSpeakerSession, String)>, String> {
    if blocks.is_empty() {
        return Ok(None);
    }

    cleanup_old_artifacts();

    let artifact_dir = artifact_dir()?;
    let artifact_path = artifact_dir.join(format!("speaker-session-{}.json", unique_id()));
    let artifact = DiarizedTranscriptArtifact {
        provider: provider.clone(),
        blocks,
        output_format,
        subtitle_segments,
    };

    let serialized = serde_json::to_string(&artifact)
        .map_err(|e| format!("Failed to serialize speaker session: {}", e))?;
    fs::write(&artifact_path, serialized)
        .map_err(|e| format!("Failed to write speaker session file: {}", e))?;

    let session = FileTranscriptionSpeakerSession {
        artifact_path: artifact_path.to_string_lossy().to_string(),
        provider,
        speakers: build_speakers(&artifact.blocks),
    };
    let rendered = render_diarized_output(&artifact, &[]);

    Ok(Some((session, rendered)))
}

pub fn reapply_diarized_transcript(
    artifact_path: &str,
    speaker_names: &[FileTranscriptionSpeakerNameInput],
) -> Result<String, String> {
    let artifact = read_artifact(artifact_path)?;
    if artifact.blocks.is_empty() {
        return Err(
            "The temporary speaker session does not contain any speaker blocks".to_string(),
        );
    }

    Ok(render_diarized_output(&artifact, speaker_names))
}

fn render_diarized_output(
    artifact: &DiarizedTranscriptArtifact,
    speaker_names: &[FileTranscriptionSpeakerNameInput],
) -> String {
    match artifact.output_format {
        OutputFormat::Text => render_diarized_transcript(&artifact.blocks, speaker_names),
        OutputFormat::Srt => segments_to_srt(&render_diarized_subtitle_segments(
            &artifact.subtitle_segments,
            speaker_names,
        )),
        OutputFormat::Vtt => segments_to_vtt(&render_diarized_subtitle_segments(
            &artifact.subtitle_segments,
            speaker_names,
        )),
    }
}

pub fn render_diarized_subtitle_segments(
    segments: &[DiarizedSubtitleSegment],
    speaker_names: &[FileTranscriptionSpeakerNameInput],
) -> Vec<SubtitleSegment> {
    let mut names_by_speaker = HashMap::new();
    for speaker_name in speaker_names {
        let fallback_name = segments
            .iter()
            .find(|segment| segment.speaker_id == Some(speaker_name.speaker_id))
            .and_then(|segment| segment.default_name.clone())
            .unwrap_or_else(|| fallback_default_name(speaker_name.speaker_id));
        names_by_speaker.insert(
            speaker_name.speaker_id,
            sanitize_speaker_name(&fallback_name, &speaker_name.name),
        );
    }

    segments
        .iter()
        .map(|segment| {
            let speaker_name = segment.speaker_id.map(|speaker_id| {
                names_by_speaker
                    .get(&speaker_id)
                    .cloned()
                    .or_else(|| segment.default_name.clone())
                    .unwrap_or_else(|| fallback_default_name(speaker_id))
            });
            SubtitleSegment {
                start: segment.start,
                end: segment.end,
                text: speaker_name
                    .map(|name| format!("[{}] {}", name, segment.text))
                    .unwrap_or_else(|| segment.text.clone()),
            }
        })
        .collect()
}

fn build_speakers(blocks: &[DiarizedTranscriptBlock]) -> Vec<FileTranscriptionSpeaker> {
    let mut speakers: Vec<FileTranscriptionSpeaker> = Vec::new();

    for block in blocks {
        if speakers
            .iter()
            .any(|speaker| speaker.speaker_id == block.speaker_id)
        {
            continue;
        }

        speakers.push(FileTranscriptionSpeaker {
            speaker_id: block.speaker_id,
            default_name: block.default_name.clone(),
        });
    }

    speakers
}

pub fn render_diarized_transcript(
    blocks: &[DiarizedTranscriptBlock],
    speaker_names: &[FileTranscriptionSpeakerNameInput],
) -> String {
    let mut names_by_speaker = HashMap::new();
    for speaker_name in speaker_names {
        let fallback_name = blocks
            .iter()
            .find(|block| block.speaker_id == speaker_name.speaker_id)
            .map(|block| block.default_name.clone())
            .unwrap_or_else(|| fallback_default_name(speaker_name.speaker_id));
        names_by_speaker.insert(
            speaker_name.speaker_id,
            sanitize_speaker_name(&fallback_name, &speaker_name.name),
        );
    }

    blocks
        .iter()
        .map(|block| {
            let speaker_name = names_by_speaker
                .get(&block.speaker_id)
                .cloned()
                .unwrap_or_else(|| block.default_name.clone());
            format!("[{}] {}", speaker_name, block.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_artifact(artifact_path: &str) -> Result<DiarizedTranscriptArtifact, String> {
    let validated_path = validate_artifact_path(artifact_path)?;
    let raw = fs::read_to_string(&validated_path)
        .map_err(|e| format!("Failed to read temporary speaker session: {}", e))?;

    serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse temporary speaker session: {}", e))
}

fn validate_artifact_path(artifact_path: &str) -> Result<PathBuf, String> {
    let requested_path = PathBuf::from(artifact_path);
    if requested_path.as_os_str().is_empty() {
        return Err("Speaker session path is missing".to_string());
    }

    let artifact_dir = artifact_dir()?;
    let canonical_dir = fs::canonicalize(&artifact_dir)
        .map_err(|e| format!("Failed to validate speaker session directory: {}", e))?;
    let canonical_path = fs::canonicalize(&requested_path).map_err(|_| {
        "The temporary speaker session is no longer available. Run transcription again.".to_string()
    })?;

    if !canonical_path.starts_with(&canonical_dir) {
        return Err("Invalid speaker session path".to_string());
    }

    if canonical_path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err("Invalid speaker session file".to_string());
    }

    Ok(canonical_path)
}

fn artifact_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(ARTIFACT_DIR_NAME);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create speaker session directory: {}", e))?;
    Ok(dir)
}

fn cleanup_old_artifacts() {
    let Ok(dir) = artifact_dir() else {
        return;
    };

    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    let cutoff = SystemTime::now()
        .checked_sub(ARTIFACT_TTL)
        .unwrap_or(UNIX_EPOCH);

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };

        if modified >= cutoff {
            continue;
        }

        if let Err(err) = fs::remove_file(&path) {
            warn!(
                "Failed to remove stale speaker session {}: {}",
                path.display(),
                err
            );
        }
    }
}

fn unique_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:032x}", nanos)
}

fn normalize_block_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_or_merge_block(
    blocks: &mut Vec<DiarizedTranscriptBlock>,
    speaker_id: u32,
    default_name: String,
    text: String,
) {
    if let Some(last_block) = blocks.last_mut() {
        if last_block.speaker_id == speaker_id {
            if !last_block.text.is_empty() && !text.is_empty() {
                last_block.text.push(' ');
            }
            last_block.text.push_str(&text);
            return;
        }
    }

    blocks.push(DiarizedTranscriptBlock {
        speaker_id,
        default_name,
        text,
    });
}

fn fallback_default_name(speaker_id: u32) -> String {
    format!("Speaker {}", speaker_id)
}

fn sanitize_speaker_name(default_name: &str, name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return default_name.to_string();
    }

    let cleaned = trimmed
        .replace('[', "(")
        .replace(']', ")")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.is_empty() {
        default_name.to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diarized_words() -> Vec<RawDiarizedTranscriptWord> {
        vec![
            RawDiarizedTranscriptWord {
                speaker_key: Some("spk_1".to_string()),
                default_name: Some("spk_1".to_string()),
                text: "Hello".to_string(),
                start: 0.0,
                end: 0.3,
            },
            RawDiarizedTranscriptWord {
                speaker_key: Some("spk_1".to_string()),
                default_name: Some("spk_1".to_string()),
                text: "there".to_string(),
                start: 0.3,
                end: 0.6,
            },
            RawDiarizedTranscriptWord {
                speaker_key: Some("spk_2".to_string()),
                default_name: Some("spk_2".to_string()),
                text: "General".to_string(),
                start: 0.6,
                end: 0.9,
            },
            RawDiarizedTranscriptWord {
                speaker_key: Some("spk_2".to_string()),
                default_name: Some("spk_2".to_string()),
                text: "Kenobi".to_string(),
                start: 0.9,
                end: 1.2,
            },
        ]
    }

    #[test]
    fn speaker_change_starts_a_new_subtitle_cue() {
        let (_, segments) = normalize_raw_diarized_words(diarized_words());

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].speaker_id, Some(0));
        assert_eq!(segments[0].text, "Hello there");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 0.6);
        assert_eq!(segments[1].speaker_id, Some(1));
        assert_eq!(segments[1].text, "General Kenobi");
        assert_eq!(segments[1].start, 0.6);
    }

    #[test]
    fn text_blocks_preserve_words_without_speaker_metadata() {
        let mut words = diarized_words();
        words[1].speaker_key = None;
        words[1].default_name = None;

        let (blocks, segments) = normalize_raw_diarized_words(words);

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].text, "Hello");
        assert_eq!(blocks[1].default_name, UNATTRIBUTED_SPEAKER_NAME);
        assert_eq!(blocks[1].text, "there");
        assert_eq!(blocks[2].text, "General Kenobi");
        assert_eq!(
            blocks
                .iter()
                .map(|block| block.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            "Hello there General Kenobi"
        );
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.text.split_whitespace().count())
                .sum::<usize>(),
            4
        );
    }

    #[test]
    fn diarized_srt_and_vtt_keep_timing_syntax_and_speaker_labels() {
        let (_, segments) = normalize_raw_diarized_words(diarized_words());
        let labelled = render_diarized_subtitle_segments(&segments, &[]);

        let srt = segments_to_srt(&labelled);
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:00,600\n[spk_1] Hello there"));
        assert!(srt.contains("2\n00:00:00,600 --> 00:00:01,200\n[spk_2] General Kenobi"));

        let vtt = segments_to_vtt(&labelled);
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:00.000 --> 00:00:00.600\n[spk_1] Hello there"));
        assert!(vtt.contains("00:00:00.600 --> 00:00:01.200\n[spk_2] General Kenobi"));
    }

    #[test]
    fn save_to_file_subtitle_rendering_does_not_require_an_interactive_session() {
        let (_, segments) = normalize_raw_diarized_words(diarized_words());
        let labelled = render_diarized_subtitle_segments(&segments, &[]);

        assert!(segments_to_srt(&labelled).contains("[spk_1] Hello there"));
    }

    #[test]
    fn reapply_preserves_srt_format_and_changes_only_speaker_names() {
        let (blocks, segments) = normalize_raw_diarized_words(diarized_words());
        let (session, _) = create_diarized_transcript_session(
            DiarizedTranscriptProvider::Gemini,
            blocks,
            OutputFormat::Srt,
            segments,
        )
        .unwrap()
        .unwrap();

        let reapplied = reapply_diarized_transcript(
            &session.artifact_path,
            &[
                FileTranscriptionSpeakerNameInput {
                    speaker_id: 0,
                    name: "Alice".to_string(),
                },
                FileTranscriptionSpeakerNameInput {
                    speaker_id: 1,
                    name: "Bob".to_string(),
                },
            ],
        )
        .unwrap();

        assert!(reapplied.contains("1\n00:00:00,000 --> 00:00:00,600\n[Alice] Hello there"));
        assert!(reapplied.contains("2\n00:00:00,600 --> 00:00:01,200\n[Bob] General Kenobi"));
        fs::remove_file(session.artifact_path).unwrap();
    }

    #[test]
    fn reapply_preserves_vtt_header_and_timestamps() {
        let (blocks, segments) = normalize_raw_diarized_words(diarized_words());
        let artifact = DiarizedTranscriptArtifact {
            provider: DiarizedTranscriptProvider::Gemini,
            blocks,
            output_format: OutputFormat::Vtt,
            subtitle_segments: segments,
        };

        let reapplied = render_diarized_output(
            &artifact,
            &[FileTranscriptionSpeakerNameInput {
                speaker_id: 0,
                name: "Alice".to_string(),
            }],
        );

        assert!(reapplied.starts_with("WEBVTT\n\n"));
        assert!(reapplied.contains("00:00:00.000 --> 00:00:00.600\n[Alice] Hello there"));
    }
}
