use log::warn;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ARTIFACT_DIR_NAME: &str = "aivorelay-file-transcription-speakers";
const ARTIFACT_TTL: Duration = Duration::from_secs(60 * 60 * 24);

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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DiarizedTranscriptProvider {
    Deepgram,
    Soniox,
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
}

pub fn normalize_raw_speaker_blocks(raw_blocks: Vec<RawSpeakerBlock>) -> Vec<DiarizedTranscriptBlock> {
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

        let speaker_id = *speaker_ids.entry(speaker_key.to_string()).or_insert_with(|| {
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
    let rendered = render_diarized_transcript(&artifact.blocks, &[]);

    Ok(Some((session, rendered)))
}

pub fn reapply_diarized_transcript(
    artifact_path: &str,
    speaker_names: &[FileTranscriptionSpeakerNameInput],
) -> Result<String, String> {
    let artifact = read_artifact(artifact_path)?;
    if artifact.blocks.is_empty() {
        return Err("The temporary speaker session does not contain any speaker blocks".to_string());
    }

    Ok(render_diarized_transcript(&artifact.blocks, speaker_names))
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
    let canonical_path = fs::canonicalize(&requested_path)
        .map_err(|_| "The temporary speaker session is no longer available. Run transcription again.".to_string())?;

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
            warn!("Failed to remove stale speaker session {}: {}", path.display(), err);
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
