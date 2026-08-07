//! Crash-safe checkpoints for provider-generated TTS PCM.
//!
//! The final WAV/MP3 remains non-resumable and is always encoded from one
//! verified PCM prefix. Each PCM segment is synced before an alternating JSON
//! checkpoint is atomically published.

use super::tts::{TtsChunk, TtsManager, MAX_TTS_TEXT_INPUT_BYTES, PROVIDER_PCM_SAMPLE_RATE};
use crate::settings::{
    ElevenLabsTextNormalization, TtsKeySource, TtsProvider, TtsSettings,
};
use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SCHEMA_VERSION: u32 = 1;
const PIPELINE_REVISION: &str = "pcm24k-mono-semantic-v2-provider-controls";
const DIRECTORY_SUFFIX: &str = ".aivorelay-tts-resume";
const MANAGED_ROOT: &str = "file-resume";
const CHECKPOINT_SLOTS: [&str; 2] = ["checkpoint-0.json", "checkpoint-1.json"];
const MAX_CHECKPOINT_BYTES: u64 = 16 * 1024 * 1024;
const OWNER_MARKER: &str = "aivorelay-owner-v1";
const OWNER_MARKER_CONTENTS: &[u8] = b"AIVORelay TTS resume workspace v1\n";
const RAW_PCM_FILE: &str = "audio.pcm";
const ENCODED_PARTIAL_FILE: &str = "audio.output.partial";
const LEASE_FILE: &str = "lease.lock";
const UI_JOBS_ROOT: &str = "ui-file-jobs";
const UI_JOB_SLOTS: [&str; 2] = ["job-0.json", "job-1.json"];
const UI_JOB_SOURCE_FILE: &str = "source.txt";
const MAX_UI_JOB_SOURCE_BYTES: usize = MAX_TTS_TEXT_INPUT_BYTES * 2;
// The source snapshot is stored separately. A manifest keeps one processed
// chunk copy; 64 MiB still accommodates the worst JSON escaping expansion of
// an otherwise valid 8 MiB input without allowing unbounded recovery files.
const MAX_UI_JOB_BYTES: u64 = 64 * 1024 * 1024;
static UI_JOB_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResumeOrigin {
    Manual,
    UiJob {
        job_id: String,
        source_path: PathBuf,
        output_path: PathBuf,
    },
    Watcher {
        source_path: PathBuf,
        output_path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiFileJobStatus {
    Planned,
    Preparing,
    Running,
    Retrying,
    Paused,
    Interrupted,
    Failed,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UiFileJobManifest {
    schema_version: u32,
    generation: u64,
    pub job_id: String,
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processed_text: Option<String>,
    #[serde(default)]
    pub processed_character_count: Option<usize>,
    pub chunks: Vec<TtsChunk>,
    pub settings: TtsSettings,
    pub status: UiFileJobStatus,
    pub completed_chunks: usize,
    pub last_error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UiFileJobSummary {
    pub job_id: String,
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub provider: TtsProvider,
    pub output_format: crate::settings::TtsOutputFormat,
    pub status: UiFileJobStatus,
    pub completed_chunks: usize,
    pub total_chunks: usize,
    pub partial_available: bool,
    pub last_error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl UiFileJobManifest {
    pub fn summary(&self) -> UiFileJobSummary {
        UiFileJobSummary {
            job_id: self.job_id.clone(),
            source_path: self.source_path.clone(),
            output_path: self.output_path.clone(),
            provider: self.settings.provider,
            output_format: self.settings.output_format,
            status: self.status,
            completed_chunks: self.completed_chunks.min(self.chunks.len()),
            total_chunks: self.chunks.len(),
            partial_available: self.completed_chunks > 0 && !self.output_path.exists(),
            last_error: self.last_error.clone(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatcherResumeTask {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResumeSegment {
    chunk_index: usize,
    end_offset: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResumeCheckpoint {
    schema_version: u32,
    pipeline_revision: String,
    generation: u64,
    synthesis_signature: String,
    total_chunks: usize,
    origin: ResumeOrigin,
    segments: Vec<ResumeSegment>,
}

#[derive(Serialize)]
struct EffectiveSynthesisSignature<'a> {
    pipeline_revision: &'static str,
    sample_rate: u32,
    chunks: &'a [TtsChunk],
    provider: TtsProvider,
    model: &'a str,
    language: &'a str,
    voice: &'a str,
    instructions: &'a str,
    speed_bits: u32,
    provider_controls: ProviderSynthesisSignature<'a>,
    inter_chunk_pause_ms: u32,
    paragraph_pause_ms: u32,
    llm_cleanup_enabled: bool,
    llm_cleanup_provider: &'a str,
    llm_cleanup_model: &'a str,
    llm_cleanup_key_source: TtsKeySource,
    llm_cleanup_custom_base_url: &'a str,
    llm_cleanup_prompt_id: &'a str,
    llm_cleanup_prompt: &'a str,
    llm_cleanup_reasoning_enabled: bool,
    llm_cleanup_reasoning_budget: u32,
    llm_cleanup_chunk_target_chars: u32,
}

#[derive(Serialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
enum ProviderSynthesisSignature<'a> {
    None,
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible {
        base_url: &'a str,
        allow_insecure_http: bool,
    },
    Murf {
        rate: i8,
        pitch: i8,
        variation: Option<u8>,
        style: Option<&'a str>,
    },
    #[serde(rename = "elevenlabs")]
    ElevenLabs {
        stability_bits: u32,
        similarity_boost_bits: Option<u32>,
        style_bits: u32,
        use_speaker_boost: Option<bool>,
        apply_text_normalization: ElevenLabsTextNormalization,
    },
    Cartesia {
        volume_bits: u32,
        emotion: Option<&'a str>,
    },
}

pub struct ResumeWorkspace {
    root: PathBuf,
    raw_path: PathBuf,
    raw: File,
    _lease: File,
    signature: String,
    total_chunks: usize,
    origin: ResumeOrigin,
    checkpoint: Option<ResumeCheckpoint>,
}

impl ResumeWorkspace {
    pub fn open_for_output(
        output_path: &Path,
        signature: String,
        total_chunks: usize,
        origin: ResumeOrigin,
    ) -> Result<Self> {
        Self::open_at(
            output_workspace_root(output_path),
            signature,
            total_chunks,
            origin,
        )
    }

    pub fn open_managed(
        cache_root: &Path,
        namespace: &str,
        signature: String,
        total_chunks: usize,
        origin: ResumeOrigin,
    ) -> Result<Self> {
        if namespace.trim().is_empty() {
            return Err(anyhow!("TTS resume namespace must not be empty"));
        }
        let namespace_hash = sha256_hex(namespace.as_bytes());
        Self::open_at(
            cache_root.join(MANAGED_ROOT).join(namespace_hash),
            signature,
            total_chunks,
            origin,
        )
    }

    fn open_at(
        root: PathBuf,
        signature: String,
        total_chunks: usize,
        origin: ResumeOrigin,
    ) -> Result<Self> {
        if total_chunks == 0 {
            return Err(anyhow!(
                "Cannot create a TTS resume workspace with no chunks"
            ));
        }
        ensure_owned_workspace(&root)?;

        let lease_path = root.join(LEASE_FILE);
        reject_link_if_present(&lease_path)?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lease_path)
            .with_context(|| {
                format!(
                    "Failed to open TTS resume workspace lease {}",
                    lease_path.display()
                )
            })?;
        lease.try_lock_exclusive().map_err(|error| {
            anyhow!(
                "Another process is already using the TTS resume workspace {}: {}",
                root.display(),
                error
            )
        })?;
        cleanup_checkpoint_temporaries(&root)?;
        remove_owned_file_if_present(&root.join(ENCODED_PARTIAL_FILE))?;

        let raw_path = root.join(RAW_PCM_FILE);
        reject_link_if_present(&raw_path)?;
        let checkpoint = load_checkpoint(&root, &raw_path, &signature, total_chunks, &origin)?;
        let committed_bytes = checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.segments.last())
            .map(|segment| segment.end_offset)
            .unwrap_or(0);
        let mut raw = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&raw_path)
            .with_context(|| {
                format!(
                    "Failed to open resumable TTS PCM file {}",
                    raw_path.display()
                )
            })?;
        raw.set_len(committed_bytes).with_context(|| {
            format!(
                "Failed to truncate resumable TTS PCM to its last checkpoint: {}",
                raw_path.display()
            )
        })?;
        raw.seek(SeekFrom::End(0))?;

        Ok(Self {
            root,
            raw_path,
            raw,
            _lease: lease,
            signature,
            total_chunks,
            origin,
            checkpoint,
        })
    }

    pub fn completed_chunks(&self) -> usize {
        self.checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.segments.len())
            .unwrap_or(0)
    }

    pub fn committed_bytes(&self) -> u64 {
        self.checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.segments.last())
            .map(|segment| segment.end_offset)
            .unwrap_or(0)
    }

    pub fn raw_path(&self) -> &Path {
        &self.raw_path
    }

    pub fn encoded_partial_path(&self) -> PathBuf {
        self.root.join(ENCODED_PARTIAL_FILE)
    }

    pub fn append_segment(&mut self, chunk_index: usize, bytes: &[u8]) -> Result<()> {
        let expected_index = self.completed_chunks().saturating_add(1);
        if chunk_index != expected_index {
            return Err(anyhow!(
                "Refusing non-sequential TTS resume checkpoint: expected chunk {}, received {}",
                expected_index,
                chunk_index
            ));
        }
        if chunk_index > self.total_chunks || bytes.is_empty() || bytes.len() % 2 != 0 {
            return Err(anyhow!(
                "Refusing invalid PCM segment for TTS chunk {}",
                chunk_index
            ));
        }

        let previous_end = self.committed_bytes();
        self.raw.write_all(bytes)?;
        self.raw.flush()?;
        self.raw.sync_data().with_context(|| {
            format!(
                "Failed to sync resumable TTS PCM {}",
                self.raw_path.display()
            )
        })?;
        let end_offset = previous_end.saturating_add(bytes.len() as u64);
        let mut checkpoint = self.checkpoint.clone().unwrap_or(ResumeCheckpoint {
            schema_version: SCHEMA_VERSION,
            pipeline_revision: PIPELINE_REVISION.to_string(),
            generation: 0,
            synthesis_signature: self.signature.clone(),
            total_chunks: self.total_chunks,
            origin: self.origin.clone(),
            segments: Vec::with_capacity(self.total_chunks),
        });
        checkpoint.generation = checkpoint.generation.saturating_add(1);
        checkpoint.segments.push(ResumeSegment {
            chunk_index,
            end_offset,
            sha256: sha256_hex(bytes),
        });
        persist_checkpoint(&self.root, &checkpoint)?;
        self.checkpoint = Some(checkpoint);
        Ok(())
    }

    pub fn discard(self) {
        let root = self.root.clone();
        drop(self);
        if let Err(error) = discard_owned_workspace(&root, true) {
            log::warn!(
                "Failed to clear TTS resume workspace {}: {}",
                root.display(),
                error
            );
        }
    }
}

pub fn synthesis_signature(chunks: &[TtsChunk], settings: &TtsSettings) -> Result<String> {
    let normalized_openai_compatible_base_url =
        crate::managers::tts::normalize_openai_compatible_base_url_with_insecure_option(
            &settings.openai_compatible_base_url,
            settings.openai_compatible_allow_insecure_http,
        )
        .unwrap_or_else(|_| settings.openai_compatible_base_url.trim().trim_end_matches('/').to_string());
    let (model, language, voice, instructions, speed, provider_controls) = match settings.provider {
        TtsProvider::Soniox => (
            settings.soniox_model.trim(),
            settings.soniox_language.trim(),
            settings.soniox_voice.trim(),
            "",
            settings.speed.clamp(0.7, 1.3),
            ProviderSynthesisSignature::None,
        ),
        TtsProvider::Deepgram => (
            settings.deepgram_model.trim(),
            "",
            "",
            "",
            settings.speed.clamp(0.7, 1.5),
            ProviderSynthesisSignature::None,
        ),
        TtsProvider::OpenAi => (
            settings.openai_model.trim(),
            "",
            settings.openai_voice.trim(),
            if TtsManager::openai_model_supports_instructions(&settings.openai_model) {
                settings.openai_instructions.as_str()
            } else {
                ""
            },
            settings.speed.clamp(0.25, 4.0),
            ProviderSynthesisSignature::None,
        ),
        TtsProvider::OpenAiCompatible => (
            settings.openai_compatible_model.trim(),
            "",
            settings.openai_compatible_voice.trim(),
            "",
            settings.speed.clamp(0.25, 4.0),
            ProviderSynthesisSignature::OpenAiCompatible {
                base_url: normalized_openai_compatible_base_url.as_str(),
                allow_insecure_http: settings.openai_compatible_allow_insecure_http,
            },
        ),
        TtsProvider::Murf => (
            settings.murf_model.trim(),
            settings.murf_language.trim(),
            settings.murf_voice.trim(),
            "",
            1.0,
            ProviderSynthesisSignature::Murf {
                rate: settings.murf_rate,
                pitch: settings.murf_pitch,
                variation: (settings.murf_model == "gen2")
                    .then_some(settings.murf_variation),
                style: settings.murf_style.as_deref(),
            },
        ),
        TtsProvider::ElevenLabs => (
            settings.elevenlabs_model.trim(),
            if settings.elevenlabs_model == "eleven_v3" {
                settings.elevenlabs_language.trim()
            } else {
                ""
            },
            settings.elevenlabs_voice.trim(),
            "",
            if settings.elevenlabs_model == "eleven_v3" {
                1.0
            } else {
                settings.speed.clamp(0.7, 1.2)
            },
            ProviderSynthesisSignature::ElevenLabs {
                stability_bits: settings.elevenlabs_stability.to_bits(),
                similarity_boost_bits: (settings.elevenlabs_model != "eleven_v3")
                    .then_some(settings.elevenlabs_similarity_boost.to_bits()),
                style_bits: settings.elevenlabs_style.to_bits(),
                use_speaker_boost: (settings.elevenlabs_model != "eleven_v3")
                    .then_some(settings.elevenlabs_use_speaker_boost),
                apply_text_normalization: settings.elevenlabs_apply_text_normalization,
            },
        ),
        TtsProvider::Cartesia => (
            settings.cartesia_model.trim(),
            settings.cartesia_language.trim(),
            settings.cartesia_voice.trim(),
            "",
            settings.speed.clamp(0.6, 1.5),
            ProviderSynthesisSignature::Cartesia {
                volume_bits: settings.cartesia_volume.clamp(0.5, 2.0).to_bits(),
                emotion: settings.cartesia_emotion.as_deref(),
            },
        ),
        TtsProvider::Edge => (
            crate::managers::edge_tts::EDGE_TTS_MODEL,
            settings.edge_voice_language.trim(),
            settings.edge_voice.trim(),
            "",
            settings.speed.clamp(0.5, 2.0),
            ProviderSynthesisSignature::None,
        ),
        TtsProvider::LocalQwen => (
            crate::managers::local_tts::LOCAL_TTS_MODEL_REVISION,
            settings.local_qwen_language.trim(),
            settings.local_qwen_voice.trim(),
            "",
            settings.speed.clamp(0.5, 2.0),
            ProviderSynthesisSignature::None,
        ),
        TtsProvider::LocalKokoro => (
            crate::managers::local_kokoro::KOKORO_MODEL_REVISION,
            settings.local_kokoro_language.trim(),
            settings.local_kokoro_voice.trim(),
            "",
            settings.speed.clamp(0.5, 2.0),
            ProviderSynthesisSignature::None,
        ),
        TtsProvider::Windows => {
            let voice = settings.windows_voice_id.trim();
            if voice.is_empty() {
                return Err(anyhow!(
                    "Windows default voice must be resolved to a stable ID before checkpointing"
                ));
            }
            (
                "windows.media.speechsynthesis",
                settings.windows_voice_language.trim(),
                voice,
                "",
                settings.speed.clamp(0.5, 2.0),
                ProviderSynthesisSignature::None,
            )
        }
    };
    let payload = EffectiveSynthesisSignature {
        pipeline_revision: PIPELINE_REVISION,
        sample_rate: PROVIDER_PCM_SAMPLE_RATE,
        chunks,
        provider: settings.provider,
        model,
        language,
        voice,
        instructions,
        speed_bits: speed.to_bits(),
        provider_controls,
        inter_chunk_pause_ms: settings.inter_chunk_pause_ms.min(5_000),
        paragraph_pause_ms: settings.paragraph_pause_ms.min(10_000),
        llm_cleanup_enabled: settings.llm_preprocessing.file_enabled,
        llm_cleanup_provider: settings.llm_preprocessing.provider_id.trim(),
        llm_cleanup_model: settings.llm_preprocessing.model.trim(),
        llm_cleanup_key_source: settings.llm_preprocessing.key_source,
        llm_cleanup_custom_base_url: settings.llm_preprocessing.custom_base_url.trim(),
        llm_cleanup_prompt_id: settings.llm_preprocessing.file_selected_prompt_id.trim(),
        llm_cleanup_prompt: settings
            .llm_preprocessing
            .file_prompts
            .iter()
            .find(|prompt| prompt.id == settings.llm_preprocessing.file_selected_prompt_id)
            .map(|prompt| prompt.prompt.trim())
            .unwrap_or(""),
        llm_cleanup_reasoning_enabled: settings.llm_preprocessing.reasoning_enabled,
        llm_cleanup_reasoning_budget: settings.llm_preprocessing.reasoning_budget,
        llm_cleanup_chunk_target_chars: settings.llm_preprocessing.chunk_target_chars,
    };
    let bytes = serde_json::to_vec(&payload).context("Failed to fingerprint TTS synthesis plan")?;
    Ok(sha256_hex(&bytes))
}

pub(crate) fn create_ui_file_job(
    cache_root: &Path,
    source_path: PathBuf,
    output_path: PathBuf,
    source_text: String,
    settings: TtsSettings,
) -> Result<UiFileJobManifest> {
    if find_ui_file_job_by_output(cache_root, &output_path)?.is_some() {
        return Err(anyhow!(
            "An unfinished TTS conversion already owns this output path. Resume or discard it first."
        ));
    }
    let now = chrono::Utc::now().timestamp_millis();
    let nonce = UI_JOB_NONCE.fetch_add(1, Ordering::Relaxed);
    let job_id = format!("{:x}-{:x}-{:x}", now.max(0), std::process::id(), nonce);
    let manifest = UiFileJobManifest {
        schema_version: SCHEMA_VERSION,
        generation: 1,
        job_id,
        source_path,
        output_path,
        source_text,
        processed_text: None,
        processed_character_count: None,
        chunks: Vec::new(),
        settings,
        status: UiFileJobStatus::Planned,
        completed_chunks: 0,
        last_error: None,
        created_at_ms: now,
        updated_at_ms: now,
    };
    if let Err(error) = persist_ui_file_job(cache_root, &manifest) {
        let _ = remove_ui_file_job_record(cache_root, &manifest.job_id);
        return Err(error);
    }
    Ok(manifest)
}

pub(crate) fn find_ui_file_job_by_output(
    cache_root: &Path,
    output_path: &Path,
) -> Result<Option<UiFileJobManifest>> {
    let root = cache_root.join(UI_JOBS_ROOT);
    reject_link_if_present(&root)?;
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut matching = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let job_id = entry.file_name().to_string_lossy().into_owned();
        if validate_ui_job_id(&job_id).is_err() {
            continue;
        }
        let candidates = match read_ui_job_candidates(&entry.path()) {
            Ok(candidates) => candidates,
            Err(error) => {
                log::warn!(
                    "Ignoring corrupt unrelated TTS job while checking output ownership: {error}"
                );
                continue;
            }
        };
        if let Some(manifest) = candidates
            .into_iter()
            .filter(|job| job.schema_version == SCHEMA_VERSION && job.job_id == job_id)
            .max_by_key(|job| job.generation)
            .filter(|job| {
                job.status != UiFileJobStatus::Completed
                    && paths_refer_to_same_output(&job.output_path, output_path)
            })
        {
            matching.push(manifest);
        }
    }
    matching.sort_by_key(|job| job.updated_at_ms);
    Ok(matching.pop())
}

pub(crate) fn load_ui_file_job(cache_root: &Path, job_id: &str) -> Result<UiFileJobManifest> {
    validate_ui_job_id(job_id)?;
    read_ui_job_candidates(&ui_job_root(cache_root, job_id))?
        .into_iter()
        .filter(|job| job.schema_version == SCHEMA_VERSION && job.job_id == job_id)
        .max_by_key(|job| job.generation)
        .ok_or_else(|| anyhow!("The saved TTS conversion job was not found"))
}

pub(crate) fn persist_ui_file_job(cache_root: &Path, manifest: &UiFileJobManifest) -> Result<()> {
    validate_ui_job_id(&manifest.job_id)?;
    let root = ui_job_root(cache_root, &manifest.job_id);
    reject_link_if_present(&root)?;
    fs::create_dir_all(&root)
        .with_context(|| format!("Failed to create TTS job directory {}", root.display()))?;
    reject_link_if_present(&root)?;
    persist_ui_job_source(&root, &manifest.source_text)?;

    let mut compact = manifest.clone();
    compact.processed_character_count = compact.processed_character_count.or_else(|| {
        compact
            .processed_text
            .as_ref()
            .map(|text| text.chars().count())
    });
    compact.source_text.clear();
    compact.processed_text = None;
    let bytes = serde_json::to_vec(&compact).context("Failed to serialize the saved TTS job")?;
    if bytes.len() as u64 > MAX_UI_JOB_BYTES {
        return Err(anyhow!(
            "The saved TTS job exceeds the {} MiB safety limit",
            MAX_UI_JOB_BYTES / (1024 * 1024)
        ));
    }
    let slot_index = (manifest.generation % UI_JOB_SLOTS.len() as u64) as usize;
    let destination = root.join(UI_JOB_SLOTS[slot_index]);
    reject_link_if_present(&destination)?;
    let temporary = root.join(format!(
        ".job-{}-{}-{}.partial",
        std::process::id(),
        manifest.generation,
        unique_nonce()
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        if destination.exists() {
            fs::remove_file(&destination)?;
        }
        crate::no_clobber::publish_new_file(&temporary, &destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("Failed to save TTS job {}", manifest.job_id))
}

pub fn list_ui_file_jobs(
    cache_root: &Path,
    active_job_id: Option<&str>,
) -> Result<Vec<UiFileJobSummary>> {
    let root = cache_root.join(UI_JOBS_ROOT);
    reject_link_if_present(&root)?;
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut jobs = Vec::new();
    let mut completed_jobs = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let job_id = entry.file_name().to_string_lossy().into_owned();
        if validate_ui_job_id(&job_id).is_err() {
            continue;
        }
        let Some(mut manifest) = read_ui_job_candidates(&entry.path())?
            .into_iter()
            .filter(|job| job.schema_version == SCHEMA_VERSION && job.job_id == job_id)
            .max_by_key(|job| job.generation)
        else {
            continue;
        };
        let was_left_in_flight = matches!(
            manifest.status,
            UiFileJobStatus::Preparing | UiFileJobStatus::Running | UiFileJobStatus::Retrying
        );
        if was_left_in_flight && active_job_id != Some(manifest.job_id.as_str()) {
            manifest.completed_chunks = ui_job_completed_chunks(&manifest)?;
            manifest.status = UiFileJobStatus::Interrupted;
            manifest.last_error = Some("AivoRelay closed before this conversion finished".into());
            touch_ui_job(&mut manifest);
            persist_ui_file_job(cache_root, &manifest)?;
        }
        if manifest.status == UiFileJobStatus::Completed || manifest.output_path.exists() {
            completed_jobs.push(manifest);
        } else {
            jobs.push(manifest.summary());
        }
    }
    for mut manifest in completed_jobs {
        if let Err(error) = discard_ui_file_job(cache_root, &manifest.job_id) {
            manifest.status = UiFileJobStatus::Completed;
            manifest.last_error = Some(format!(
                "The final output exists, but saved recovery data could not be cleaned up: {error}"
            ));
            jobs.push(manifest.summary());
        }
    }
    jobs.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
    Ok(jobs)
}

pub(crate) fn touch_ui_job(manifest: &mut UiFileJobManifest) {
    manifest.generation = manifest.generation.saturating_add(1);
    manifest.updated_at_ms = chrono::Utc::now().timestamp_millis();
}

pub(crate) fn remove_ui_file_job_record(cache_root: &Path, job_id: &str) -> Result<()> {
    validate_ui_job_id(job_id)?;
    let root = ui_job_root(cache_root, job_id);
    reject_link_if_present(&root)?;
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() && !file_type.is_symlink() {
            return Err(anyhow!(
                "The TTS job directory contains an unexpected folder"
            ));
        }
        fs::remove_file(entry.path())?;
    }
    fs::remove_dir(&root)?;
    Ok(())
}

pub fn discard_ui_file_job(cache_root: &Path, job_id: &str) -> Result<()> {
    let manifest = load_ui_file_job(cache_root, job_id)?;
    let workspace = output_workspace_root(&manifest.output_path);
    if workspace.exists() {
        ensure_owned_workspace(&workspace)?;
        let lease_path = workspace.join(LEASE_FILE);
        reject_link_if_present(&lease_path)?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lease_path)
            .with_context(|| {
                format!(
                    "Failed to open TTS resume workspace lease {}",
                    lease_path.display()
                )
            })?;
        lease.try_lock_exclusive().map_err(|error| {
            anyhow!(
                "The TTS conversion is still active and cannot be discarded: {}",
                error
            )
        })?;
        let candidates = read_checkpoint_candidates(&workspace);
        let owns_workspace = candidates
            .iter()
            .max_by_key(|checkpoint| checkpoint.generation)
            .map(|checkpoint| {
                resume_origins_are_compatible(
                    &checkpoint.origin,
                    &ResumeOrigin::UiJob {
                        job_id: manifest.job_id.clone(),
                        source_path: manifest.source_path.clone(),
                        output_path: manifest.output_path.clone(),
                    },
                )
            })
            .unwrap_or_else(|| !workspace.join(RAW_PCM_FILE).exists());
        if owns_workspace {
            discard_owned_workspace(&workspace, false)?;
            drop(lease);
            discard_owned_workspace(&workspace, true)?;
        } else {
            log::warn!(
                "Preserving TTS resume workspace not owned by UI job {}: {}",
                manifest.job_id,
                workspace.display()
            );
        }
    }
    remove_ui_file_job_record(cache_root, job_id)
}

fn ui_job_root(cache_root: &Path, job_id: &str) -> PathBuf {
    cache_root.join(UI_JOBS_ROOT).join(job_id)
}

fn ui_job_completed_chunks(manifest: &UiFileJobManifest) -> Result<usize> {
    if manifest.chunks.is_empty() || manifest.processed_character_count.is_none() {
        return Ok(0);
    }
    let signature = synthesis_signature(&manifest.chunks, &manifest.settings)?;
    let root = output_workspace_root(&manifest.output_path);
    let raw_path = root.join(RAW_PCM_FILE);
    let checkpoint = read_checkpoint_candidates(&root)
        .into_iter()
        .filter(|checkpoint| {
            checkpoint.schema_version == SCHEMA_VERSION
                && checkpoint.pipeline_revision == PIPELINE_REVISION
                && checkpoint.synthesis_signature == signature
                && checkpoint.total_chunks == manifest.chunks.len()
        })
        .filter(|checkpoint| validate_checkpoint(checkpoint, &raw_path).is_ok())
        .max_by_key(|checkpoint| checkpoint.generation);
    Ok(checkpoint
        .map(|checkpoint| checkpoint.segments.len())
        .unwrap_or(0))
}

fn validate_ui_job_id(job_id: &str) -> Result<()> {
    if job_id.is_empty()
        || job_id.len() > 128
        || !job_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(anyhow!("The TTS conversion job ID is invalid"));
    }
    Ok(())
}

fn read_ui_job_candidates(root: &Path) -> Result<Vec<UiFileJobManifest>> {
    let retained_source = read_ui_job_source(root);
    let mut candidates = Vec::new();
    let mut errors = Vec::new();
    for slot in UI_JOB_SLOTS {
        match read_ui_file_job(&root.join(slot)) {
            Ok(Some(mut manifest)) => {
                if manifest.source_text.is_empty() {
                    match &retained_source {
                        Ok(Some(source)) => manifest.source_text = source.clone(),
                        Ok(None) => {
                            errors.push("the immutable source snapshot is missing".to_string());
                            continue;
                        }
                        Err(error) => {
                            errors.push(error.to_string());
                            continue;
                        }
                    }
                }
                if manifest.processed_character_count.is_none() {
                    manifest.processed_character_count = manifest
                        .processed_text
                        .as_ref()
                        .map(|text| text.chars().count());
                }
                candidates.push(manifest);
            }
            Ok(None) => {}
            Err(error) => errors.push(error.to_string()),
        }
    }
    if candidates.is_empty() && !errors.is_empty() {
        return Err(anyhow!(
            "Saved TTS job {} is corrupt or inaccessible: {}",
            root.display(),
            errors.join("; ")
        ));
    }
    if candidates.is_empty() && matches!(retained_source, Ok(Some(_))) {
        return Err(anyhow!(
            "Saved TTS job {} has a source snapshot but no readable manifest",
            root.display()
        ));
    }
    Ok(candidates)
}

fn persist_ui_job_source(root: &Path, source_text: &str) -> Result<()> {
    if source_text.len() > MAX_UI_JOB_SOURCE_BYTES {
        return Err(anyhow!(
            "The decoded retained TTS source exceeds the 16 MiB safety limit"
        ));
    }
    let destination = root.join(UI_JOB_SOURCE_FILE);
    reject_link_if_present(&destination)?;
    if destination.exists() {
        let existing = read_ui_job_source(root)?
            .ok_or_else(|| anyhow!("The retained TTS source is unavailable"))?;
        if existing != source_text {
            return Err(anyhow!(
                "Refusing to replace the immutable source of a saved TTS job"
            ));
        }
        return Ok(());
    }
    let temporary = root.join(format!(
        ".source-{}-{}.partial",
        std::process::id(),
        unique_nonce()
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(source_text.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        crate::no_clobber::publish_new_file(&temporary, &destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.context("Failed to retain the immutable TTS job source")
}

fn read_ui_job_source(root: &Path) -> Result<Option<String>> {
    let path = root.join(UI_JOB_SOURCE_FILE);
    reject_link_if_present(&path)?;
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() > MAX_UI_JOB_SOURCE_BYTES as u64 {
        return Err(anyhow!("Invalid retained TTS source file"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(&path)?
        .take(MAX_UI_JOB_SOURCE_BYTES.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_UI_JOB_SOURCE_BYTES {
        return Err(anyhow!("The retained TTS source exceeds its safety limit"));
    }
    String::from_utf8(bytes)
        .map(Some)
        .context("The retained TTS source is not valid UTF-8")
}

fn read_ui_file_job(path: &Path) -> Result<Option<UiFileJobManifest>> {
    reject_link_if_present(path)?;
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() > MAX_UI_JOB_BYTES {
        return Err(anyhow!("Invalid saved TTS job file"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)?
        .take(MAX_UI_JOB_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_UI_JOB_BYTES {
        return Err(anyhow!("The saved TTS job exceeds its safety limit"));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .with_context(|| format!("Failed to parse saved TTS job {}", path.display()))
}

pub fn discover_watcher_tasks(output_dir: &Path) -> Result<Vec<WatcherResumeTask>> {
    let mut tasks = Vec::new();
    for entry in fs::read_dir(output_dir)
        .with_context(|| format!("Failed to scan TTS watcher output {}", output_dir.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        if !name.to_string_lossy().ends_with(DIRECTORY_SUFFIX) {
            continue;
        }
        let root = entry.path();
        let Some(checkpoint) = read_checkpoint_candidates(&root)
            .into_iter()
            .filter(|checkpoint| {
                checkpoint.schema_version == SCHEMA_VERSION
                    && checkpoint.pipeline_revision == PIPELINE_REVISION
            })
            .max_by_key(|checkpoint| checkpoint.generation)
        else {
            continue;
        };
        let ResumeOrigin::Watcher {
            source_path,
            output_path,
        } = checkpoint.origin
        else {
            continue;
        };
        if output_path.exists()
            || !paths_refer_to_same_output(&output_workspace_root(&output_path), &root)
        {
            continue;
        }
        tasks.push(WatcherResumeTask {
            source_path,
            output_path,
        });
    }
    tasks.sort_by(|left, right| left.output_path.cmp(&right.output_path));
    tasks.dedup();
    Ok(tasks)
}

pub fn discard_managed(cache_root: &Path, namespace: &str) -> Result<()> {
    if namespace.trim().is_empty() {
        return Err(anyhow!("TTS resume namespace must not be empty"));
    }
    let root = cache_root
        .join(MANAGED_ROOT)
        .join(sha256_hex(namespace.as_bytes()));
    if !root.exists() {
        return Ok(());
    }
    ensure_owned_workspace(&root)?;
    let lease_path = root.join(LEASE_FILE);
    reject_link_if_present(&lease_path)?;
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lease_path)
        .with_context(|| {
            format!(
                "Failed to open TTS resume workspace lease {}",
                lease_path.display()
            )
        })?;
    lease.try_lock_exclusive().map_err(|error| {
        anyhow!(
            "Another process is already using the TTS resume workspace {}: {}",
            root.display(),
            error
        )
    })?;
    discard_owned_workspace(&root, false)
}

fn load_checkpoint(
    root: &Path,
    raw_path: &Path,
    signature: &str,
    total_chunks: usize,
    requested_origin: &ResumeOrigin,
) -> Result<Option<ResumeCheckpoint>> {
    let raw_length = fs::metadata(raw_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let candidates = read_checkpoint_candidates(root);
    let latest = candidates
        .iter()
        .max_by_key(|checkpoint| checkpoint.generation);
    if let Some(foreign) = latest
        .filter(|checkpoint| !resume_origins_are_compatible(&checkpoint.origin, requested_origin))
    {
        let can_claim_legacy_manual = matches!(&foreign.origin, ResumeOrigin::Manual)
            && matches!(requested_origin, ResumeOrigin::UiJob { .. })
            && foreign.schema_version == SCHEMA_VERSION
            && foreign.pipeline_revision == PIPELINE_REVISION
            && foreign.synthesis_signature == signature
            && foreign.total_chunks == total_chunks
            && validate_checkpoint(foreign, raw_path).is_ok();
        if can_claim_legacy_manual {
            let mut claimed = foreign.clone();
            claimed.generation = claimed.generation.saturating_add(1);
            claimed.origin = requested_origin.clone();
            persist_checkpoint(root, &claimed)?;
            return Ok(Some(claimed));
        }
        return Err(anyhow!(
            "This output path already has saved TTS progress owned by {}. Resume or clear that operation before reusing the output path.",
            resume_origin_name(&foreign.origin)
        ));
    }
    let compatible = candidates
        .iter()
        .filter(|checkpoint| {
            checkpoint.schema_version == SCHEMA_VERSION
                && checkpoint.pipeline_revision == PIPELINE_REVISION
                && checkpoint.synthesis_signature == signature
                && checkpoint.total_chunks == total_chunks
                && resume_origins_are_compatible(&checkpoint.origin, requested_origin)
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut valid = compatible
        .iter()
        .filter(|checkpoint| validate_checkpoint(checkpoint, raw_path).is_ok())
        .cloned()
        .collect::<Vec<_>>();
    valid.sort_by_key(|checkpoint| checkpoint.generation);
    if let Some(checkpoint) = valid.pop() {
        return Ok(Some(checkpoint));
    }

    let checkpoint_artifacts_exist = CHECKPOINT_SLOTS.iter().any(|slot| root.join(slot).exists());
    if !compatible.is_empty() || (raw_length > 0 && candidates.is_empty()) {
        reset_artifacts(root, raw_path)?;
        return Err(anyhow!(
            "The saved TTS resume checkpoint was corrupt and has been removed. Retry the conversion to start safely from the beginning."
        ));
    }
    if raw_length > 0 || checkpoint_artifacts_exist {
        reset_artifacts(root, raw_path)?;
    }
    Ok(None)
}

fn ensure_owned_workspace(root: &Path) -> Result<()> {
    reject_link_if_present(root)?;
    let parent = root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "Failed to create TTS resume workspace parent {}",
            parent.display()
        )
    })?;
    let marker_path = root.join(OWNER_MARKER);
    if owner_marker_present(&marker_path)? {
        return validate_owner_marker(root);
    }
    if root.exists() && !workspace_directory_is_empty(root)? {
        return Err(anyhow!(
            "Refusing unowned TTS resume directory {}; it is not empty and has no AivoRelay ownership marker",
            root.display()
        ));
    }

    let temporary_marker = parent.join(format!(
        ".aivorelay-tts-owner-{}-{}.partial",
        std::process::id(),
        unique_nonce()
    ));
    let claim_result = (|| -> Result<()> {
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_marker)
            .with_context(|| {
                format!(
                    "Failed to create TTS resume ownership claim {}",
                    temporary_marker.display()
                )
            })?;
        marker.write_all(OWNER_MARKER_CONTENTS)?;
        marker.flush()?;
        marker.sync_all()?;
        drop(marker);

        match fs::create_dir(root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to create TTS resume workspace {}", root.display())
                })
            }
        }
        reject_link_if_present(root)?;
        if owner_marker_present(&marker_path)? {
            return validate_owner_marker(root);
        }
        if !workspace_directory_is_empty(root)? {
            return Err(anyhow!(
                "Refusing unowned TTS resume directory {}; it changed before AivoRelay could claim it",
                root.display()
            ));
        }
        match crate::no_clobber::publish_new_file(&temporary_marker, &marker_path) {
            Ok(()) => validate_owner_marker(root),
            Err(_) if owner_marker_present(&marker_path)? => validate_owner_marker(root),
            Err(error) => Err(error).with_context(|| {
                format!("Failed to claim TTS resume workspace {}", root.display())
            }),
        }
    })();
    if temporary_marker.exists() {
        let _ = fs::remove_file(&temporary_marker);
    }
    claim_result
}

fn owner_marker_present(marker_path: &Path) -> Result<bool> {
    match fs::symlink_metadata(marker_path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn workspace_directory_is_empty(root: &Path) -> Result<bool> {
    let mut entries = fs::read_dir(root)
        .with_context(|| format!("Failed to inspect TTS resume workspace {}", root.display()))?;
    Ok(entries.next().transpose()?.is_none())
}

fn validate_owner_marker(root: &Path) -> Result<()> {
    let marker_path = root.join(OWNER_MARKER);
    reject_link_if_present(&marker_path)?;
    let metadata = fs::metadata(&marker_path).with_context(|| {
        format!(
            "Refusing unowned TTS resume directory {}; the AivoRelay ownership marker is missing",
            root.display()
        )
    })?;
    if !metadata.is_file() || metadata.len() != OWNER_MARKER_CONTENTS.len() as u64 {
        return Err(anyhow!(
            "Refusing unowned TTS resume directory {}; its ownership marker is invalid",
            root.display()
        ));
    }
    let contents = fs::read(&marker_path)?;
    if contents != OWNER_MARKER_CONTENTS {
        return Err(anyhow!(
            "Refusing unowned TTS resume directory {}; its ownership marker is invalid",
            root.display()
        ));
    }
    Ok(())
}

fn discard_owned_workspace(root: &Path, remove_shell: bool) -> Result<()> {
    validate_owner_marker(root)?;
    cleanup_checkpoint_temporaries(root)?;
    for path in std::iter::once(root.join(RAW_PCM_FILE))
        .chain(std::iter::once(root.join(ENCODED_PARTIAL_FILE)))
        .chain(CHECKPOINT_SLOTS.iter().map(|slot| root.join(slot)))
    {
        remove_owned_file_if_present(&path)?;
    }
    if !remove_shell {
        return Ok(());
    }

    remove_owned_file_if_present(&root.join(LEASE_FILE))?;
    let mut remaining = false;
    for entry in fs::read_dir(root)? {
        if entry?.file_name().as_os_str() != std::ffi::OsStr::new(OWNER_MARKER) {
            remaining = true;
            break;
        }
    }
    if remaining {
        log::warn!(
            "Preserving non-AivoRelay files found in TTS resume workspace {}",
            root.display()
        );
        return Ok(());
    }
    remove_owned_file_if_present(&root.join(OWNER_MARKER))?;
    match fs::remove_dir(root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "Failed to remove empty TTS resume workspace {}",
                root.display()
            )
        }),
    }
}

fn remove_owned_file_if_present(path: &Path) -> Result<()> {
    reject_link_if_present(path)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to remove TTS resume artifact {}", path.display())),
    }
}

fn cleanup_checkpoint_temporaries(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)
        .with_context(|| format!("Failed to inspect TTS resume workspace {}", root.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(".checkpoint-") || !name.ends_with(".partial") {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_file() || file_type.is_symlink() {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn validate_checkpoint(checkpoint: &ResumeCheckpoint, raw_path: &Path) -> Result<()> {
    if checkpoint.segments.len() > checkpoint.total_chunks {
        return Err(anyhow!("TTS resume checkpoint contains too many chunks"));
    }
    if checkpoint.segments.is_empty() {
        return Ok(());
    }
    let metadata = fs::metadata(raw_path)
        .with_context(|| format!("Missing resumable TTS PCM {}", raw_path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!("Resumable TTS PCM is not a regular file"));
    }
    let mut raw = File::open(raw_path)?;
    let mut previous_end = 0_u64;
    for (position, segment) in checkpoint.segments.iter().enumerate() {
        if segment.chunk_index != position + 1
            || segment.end_offset <= previous_end
            || segment.end_offset % 2 != 0
            || segment.end_offset > metadata.len()
        {
            return Err(anyhow!("Invalid TTS resume segment boundary"));
        }
        let length = segment.end_offset - previous_end;
        raw.seek(SeekFrom::Start(previous_end))?;
        let actual_hash = sha256_reader(&mut raw, length)?;
        if actual_hash != segment.sha256 {
            return Err(anyhow!(
                "TTS resume PCM checksum mismatch at chunk {}",
                segment.chunk_index
            ));
        }
        previous_end = segment.end_offset;
    }
    Ok(())
}

fn persist_checkpoint(root: &Path, checkpoint: &ResumeCheckpoint) -> Result<()> {
    let bytes =
        serde_json::to_vec(checkpoint).context("Failed to serialize TTS resume checkpoint")?;
    if bytes.len() as u64 > MAX_CHECKPOINT_BYTES {
        return Err(anyhow!(
            "TTS resume checkpoint exceeds the safety size limit"
        ));
    }
    let slot_index = (checkpoint.generation % CHECKPOINT_SLOTS.len() as u64) as usize;
    let destination = root.join(CHECKPOINT_SLOTS[slot_index]);
    let temporary = root.join(format!(
        ".checkpoint-{}-{}-{}.partial",
        std::process::id(),
        checkpoint.generation,
        unique_nonce()
    ));
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        if destination.exists() {
            fs::remove_file(&destination)?;
        }
        crate::no_clobber::publish_new_file(&temporary, &destination)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result.with_context(|| {
        format!(
            "Failed to publish TTS resume checkpoint {}",
            destination.display()
        )
    })
}

fn read_checkpoint_candidates(root: &Path) -> Vec<ResumeCheckpoint> {
    CHECKPOINT_SLOTS
        .iter()
        .filter_map(|slot| read_checkpoint(&root.join(slot)).ok().flatten())
        .collect()
}

fn read_checkpoint(path: &Path) -> Result<Option<ResumeCheckpoint>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() > MAX_CHECKPOINT_BYTES {
        return Err(anyhow!("Invalid TTS resume checkpoint file"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)?
        .take(MAX_CHECKPOINT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CHECKPOINT_BYTES {
        return Err(anyhow!(
            "TTS resume checkpoint exceeds the safety size limit"
        ));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .with_context(|| format!("Failed to parse TTS resume checkpoint {}", path.display()))
}

fn reset_artifacts(root: &Path, raw_path: &Path) -> Result<()> {
    for path in std::iter::once(raw_path.to_path_buf()).chain(
        CHECKPOINT_SLOTS
            .iter()
            .map(|slot| root.join(slot))
            .collect::<Vec<_>>(),
    ) {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to reset TTS resume artifact {}", path.display())
                })
            }
        }
    }
    Ok(())
}

fn output_workspace_root(output_path: &Path) -> PathBuf {
    let mut directory_name = OsString::from(".");
    directory_name.push(
        output_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("tts-audio")),
    );
    directory_name.push(DIRECTORY_SUFFIX);
    output_path.with_file_name(directory_name)
}

pub(crate) fn paths_refer_to_same_output(left: &Path, right: &Path) -> bool {
    output_path_identity(left) == output_path_identity(right)
}

fn output_path_identity(path: &Path) -> String {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_parent = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    let normalized = canonical_parent.join(
        path.file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("tts-audio")),
    );
    #[cfg(windows)]
    {
        normalized
            .to_string_lossy()
            .replace('/', "\\")
            .to_lowercase()
    }
    #[cfg(not(windows))]
    {
        normalized.to_string_lossy().into_owned()
    }
}

fn resume_origins_are_compatible(existing: &ResumeOrigin, requested: &ResumeOrigin) -> bool {
    match (existing, requested) {
        (ResumeOrigin::Manual, ResumeOrigin::Manual) => true,
        (
            ResumeOrigin::UiJob {
                job_id: existing_id,
                source_path: existing_source,
                output_path: existing_output,
            },
            ResumeOrigin::UiJob {
                job_id: requested_id,
                source_path: requested_source,
                output_path: requested_output,
            },
        ) => {
            existing_id == requested_id
                && paths_refer_to_same_output(existing_source, requested_source)
                && paths_refer_to_same_output(existing_output, requested_output)
        }
        (
            ResumeOrigin::Watcher {
                source_path: existing_source,
                output_path: existing_output,
            },
            ResumeOrigin::Watcher {
                source_path: requested_source,
                output_path: requested_output,
            },
        ) => {
            paths_refer_to_same_output(existing_source, requested_source)
                && paths_refer_to_same_output(existing_output, requested_output)
        }
        _ => false,
    }
}

fn resume_origin_name(origin: &ResumeOrigin) -> &'static str {
    match origin {
        ResumeOrigin::Manual => "another manual conversion",
        ResumeOrigin::UiJob { .. } => "an unfinished UI conversion",
        ResumeOrigin::Watcher { .. } => "automatic folder conversion",
    }
}

fn reject_link_if_present(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "Refusing symbolic-link TTS resume path: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn sha256_reader(reader: &mut File, mut length: u64) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    while length > 0 {
        let requested = usize::try_from(length.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = reader.read(&mut buffer[..requested])?;
        if read == 0 {
            return Err(anyhow!(
                "Resumable TTS PCM ended before its checkpoint boundary"
            ));
        }
        hasher.update(&buffer[..read]);
        length -= read as u64;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn unique_nonce() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::tts::{TtsBoundary, TtsChunk};

    fn temp_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aivorelay-tts-resume-{name}-{}-{}",
            std::process::id(),
            unique_nonce()
        ))
    }

    #[test]
    fn synthesis_signature_ignores_retry_and_output_encoding_settings() {
        let chunks = vec![TtsChunk {
            index: 1,
            text: "hello".to_string(),
            character_count: 5,
            boundary_after: TtsBoundary::End,
        }];
        let baseline = TtsSettings::default();
        let expected = synthesis_signature(&chunks, &baseline).unwrap();
        let mut changed = baseline.clone();
        changed.retry_count = changed.retry_count.saturating_add(1);
        changed.retry_base_delay_ms = changed.retry_base_delay_ms.saturating_add(100);
        changed.disk_reserve_mb = changed.disk_reserve_mb.saturating_add(1);
        changed.file_history_enabled = !changed.file_history_enabled;
        changed.mp3_bitrate_kbps = 64;
        assert_eq!(synthesis_signature(&chunks, &changed).unwrap(), expected);

        changed.soniox_voice.push_str("-different");
        assert_ne!(synthesis_signature(&chunks, &changed).unwrap(), expected);
    }

    #[test]
    fn synthesis_signature_tracks_only_effective_new_provider_controls() {
        let chunks = vec![TtsChunk {
            index: 1,
            text: "hello".to_string(),
            character_count: 5,
            boundary_after: TtsBoundary::End,
        }];
        let mut murf = TtsSettings {
            provider: TtsProvider::Murf,
            ..TtsSettings::default()
        };
        let falcon = synthesis_signature(&chunks, &murf).unwrap();
        murf.murf_variation = 5;
        assert_eq!(synthesis_signature(&chunks, &murf).unwrap(), falcon);
        murf.murf_rate = 1;
        assert_ne!(synthesis_signature(&chunks, &murf).unwrap(), falcon);

        murf.murf_model = "gen2".to_string();
        let gen2 = synthesis_signature(&chunks, &murf).unwrap();
        murf.murf_variation = 4;
        assert_ne!(synthesis_signature(&chunks, &murf).unwrap(), gen2);

        let mut elevenlabs = TtsSettings {
            provider: TtsProvider::ElevenLabs,
            ..TtsSettings::default()
        };
        let elevenlabs_signature = synthesis_signature(&chunks, &elevenlabs).unwrap();
        elevenlabs.elevenlabs_stability = 0.6;
        assert_ne!(
            synthesis_signature(&chunks, &elevenlabs).unwrap(),
            elevenlabs_signature
        );

        elevenlabs.elevenlabs_model = "eleven_v3".to_string();
        let v3_signature = synthesis_signature(&chunks, &elevenlabs).unwrap();
        elevenlabs.speed = 1.2;
        elevenlabs.elevenlabs_similarity_boost = 0.1;
        elevenlabs.elevenlabs_use_speaker_boost = false;
        assert_eq!(
            synthesis_signature(&chunks, &elevenlabs).unwrap(),
            v3_signature
        );
        elevenlabs.elevenlabs_stability = 0.7;
        assert_ne!(
            synthesis_signature(&chunks, &elevenlabs).unwrap(),
            v3_signature
        );

        let mut cartesia = TtsSettings {
            provider: TtsProvider::Cartesia,
            ..TtsSettings::default()
        };
        let cartesia_signature = synthesis_signature(&chunks, &cartesia).unwrap();
        cartesia.speed = 1.4;
        assert_ne!(
            synthesis_signature(&chunks, &cartesia).unwrap(),
            cartesia_signature
        );
        let cartesia_speed_signature = synthesis_signature(&chunks, &cartesia).unwrap();
        cartesia.cartesia_volume = 1.5;
        assert_ne!(
            synthesis_signature(&chunks, &cartesia).unwrap(),
            cartesia_speed_signature
        );
        let cartesia_volume_signature = synthesis_signature(&chunks, &cartesia).unwrap();
        cartesia.cartesia_emotion = Some("content".to_string());
        assert_ne!(
            synthesis_signature(&chunks, &cartesia).unwrap(),
            cartesia_volume_signature
        );
    }

    #[test]
    fn windows_signature_requires_and_tracks_resolved_voice_identity() {
        let chunks = vec![TtsChunk {
            index: 1,
            text: "hello".to_string(),
            character_count: 5,
            boundary_after: TtsBoundary::End,
        }];
        let mut settings = TtsSettings {
            provider: TtsProvider::Windows,
            ..TtsSettings::default()
        };
        assert!(synthesis_signature(&chunks, &settings).is_err());

        settings.windows_voice_id = "voice-en".to_string();
        settings.windows_voice_language = "en-US".to_string();
        let english = synthesis_signature(&chunks, &settings).unwrap();

        settings.windows_voice_id = "voice-ru".to_string();
        settings.windows_voice_language = "ru-RU".to_string();
        assert_ne!(synthesis_signature(&chunks, &settings).unwrap(), english);
    }

    #[test]
    fn reload_verifies_checkpoint_and_truncates_uncommitted_pcm_tail() {
        let directory = temp_directory("tail");
        fs::create_dir(&directory).unwrap();
        let output = directory.join("voice.mp3");
        let mut workspace = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            2,
            ResumeOrigin::Manual,
        )
        .unwrap();
        workspace.append_segment(1, &[1, 0, 2, 0]).unwrap();
        let raw_path = workspace.raw_path().to_path_buf();
        drop(workspace);
        OpenOptions::new()
            .append(true)
            .open(&raw_path)
            .unwrap()
            .write_all(&[9, 0])
            .unwrap();

        let workspace = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            2,
            ResumeOrigin::Manual,
        )
        .unwrap();
        assert_eq!(workspace.completed_chunks(), 1);
        assert_eq!(fs::metadata(workspace.raw_path()).unwrap().len(), 4);
        workspace.discard();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn workspace_lease_blocks_a_second_process_owner() {
        let directory = temp_directory("lease");
        fs::create_dir(&directory).unwrap();
        let output = directory.join("voice.mp3");
        let first = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            ResumeOrigin::Manual,
        )
        .unwrap();
        assert!(ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            ResumeOrigin::Manual,
        )
        .is_err());
        drop(first);
        let second = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            ResumeOrigin::Manual,
        )
        .unwrap();
        second.discard();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn discard_preserves_unknown_files_in_an_owned_workspace() {
        let directory = temp_directory("preserve-unknown");
        fs::create_dir(&directory).unwrap();
        let output = directory.join("voice.mp3");
        let workspace = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            ResumeOrigin::Manual,
        )
        .unwrap();
        let important = output_workspace_root(&output).join("important.txt");
        fs::write(&important, b"keep me").unwrap();

        workspace.discard();

        assert_eq!(fs::read(&important).unwrap(), b"keep me");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn preexisting_unowned_workspace_is_refused_without_deleting_it() {
        let directory = temp_directory("refuse-unowned");
        fs::create_dir(&directory).unwrap();
        let output = directory.join("voice.mp3");
        let workspace_root = output_workspace_root(&output);
        fs::create_dir(&workspace_root).unwrap();
        let important = workspace_root.join("important.txt");
        fs::write(&important, b"keep me").unwrap();

        assert!(ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            ResumeOrigin::Manual,
        )
        .is_err());
        assert_eq!(fs::read(&important).unwrap(), b"keep me");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn empty_workspace_left_before_owner_publish_is_recoverable() {
        let directory = temp_directory("recover-empty-claim");
        fs::create_dir(&directory).unwrap();
        let output = directory.join("voice.mp3");
        fs::create_dir(output_workspace_root(&output)).unwrap();

        let workspace = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            ResumeOrigin::Manual,
        )
        .unwrap();
        workspace.discard();
        fs::remove_dir(directory).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_output_identity_ignores_path_spelling_case() {
        let directory = temp_directory("windows-path-case");
        fs::create_dir(&directory).unwrap();
        let upper = directory.join("Speech.MP3");
        let lower = PathBuf::from(directory.to_string_lossy().to_lowercase()).join("speech.mp3");

        assert!(paths_refer_to_same_output(&upper, &lower));
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn foreign_origin_is_refused_without_erasing_its_checkpoint() {
        let directory = temp_directory("foreign-origin");
        fs::create_dir(&directory).unwrap();
        let source = directory.join("source.txt");
        let output = directory.join("voice.mp3");
        let watcher_origin = ResumeOrigin::Watcher {
            source_path: source.clone(),
            output_path: output.clone(),
        };
        let mut watcher = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            watcher_origin.clone(),
        )
        .unwrap();
        watcher.append_segment(1, &[1, 0, 2, 0]).unwrap();
        drop(watcher);

        let ui_origin = ResumeOrigin::UiJob {
            job_id: "job-1".to_string(),
            source_path: source,
            output_path: output.clone(),
        };
        assert!(
            ResumeWorkspace::open_for_output(&output, "signature".to_string(), 1, ui_origin,)
                .is_err()
        );

        let watcher =
            ResumeWorkspace::open_for_output(&output, "signature".to_string(), 1, watcher_origin)
                .unwrap();
        assert_eq!(watcher.completed_chunks(), 1);
        watcher.discard();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn ui_job_safely_claims_matching_legacy_manual_checkpoint() {
        let directory = temp_directory("claim-manual");
        fs::create_dir(&directory).unwrap();
        let source = directory.join("source.txt");
        let output = directory.join("voice.mp3");
        let mut manual = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            ResumeOrigin::Manual,
        )
        .unwrap();
        manual.append_segment(1, &[1, 0, 2, 0]).unwrap();
        drop(manual);

        let ui_origin = ResumeOrigin::UiJob {
            job_id: "job-1".to_string(),
            source_path: source,
            output_path: output.clone(),
        };
        let claimed = ResumeWorkspace::open_for_output(
            &output,
            "signature".to_string(),
            1,
            ui_origin.clone(),
        )
        .unwrap();
        assert_eq!(claimed.completed_chunks(), 1);
        drop(claimed);

        let reopened =
            ResumeWorkspace::open_for_output(&output, "signature".to_string(), 1, ui_origin)
                .unwrap();
        assert_eq!(reopened.completed_chunks(), 1);
        reopened.discard();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn ui_job_source_is_retained_outside_compact_manifest() {
        let directory = temp_directory("compact-job");
        fs::create_dir(&directory).unwrap();
        let source_path = directory.join("book.txt");
        let output_path = directory.join("book.mp3");
        let source_text = "unique-source-quote-\"-backslash-\\".to_string();
        let manifest = create_ui_file_job(
            &directory,
            source_path,
            output_path,
            source_text.clone(),
            TtsSettings::default(),
        )
        .unwrap();
        let root = ui_job_root(&directory, &manifest.job_id);
        assert_eq!(
            fs::read_to_string(root.join(UI_JOB_SOURCE_FILE)).unwrap(),
            source_text
        );
        for slot in UI_JOB_SLOTS {
            let path = root.join(slot);
            if path.exists() {
                assert!(!fs::read_to_string(path)
                    .unwrap()
                    .contains("unique-source-quote"));
            }
        }
        let loaded = load_ui_file_job(&directory, &manifest.job_id).unwrap();
        assert_eq!(loaded.source_text, source_text);
        remove_ui_file_job_record(&directory, &manifest.job_id).unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn openai_compatible_synthesis_signature_includes_normalized_base_url() {
        let chunk = TtsChunk {
            index: 1,
            text: "Hello world".to_string(),
            character_count: 11,
            boundary_after: TtsBoundary::Paragraph,
        };
        let mut settings_a = TtsSettings::default();
        settings_a.provider = TtsProvider::OpenAiCompatible;
        settings_a.openai_compatible_base_url = "http://localhost:8000/v1".to_string();

        let mut settings_b = TtsSettings::default();
        settings_b.provider = TtsProvider::OpenAiCompatible;
        settings_b.openai_compatible_base_url = "http://localhost:8000/v1/".to_string();

        let mut settings_c = TtsSettings::default();
        settings_c.provider = TtsProvider::OpenAiCompatible;
        settings_c.openai_compatible_base_url = "https://api.openai.com/v1".to_string();

        let sig_a = synthesis_signature(&[chunk.clone()], &settings_a).unwrap();
        let sig_b = synthesis_signature(&[chunk.clone()], &settings_b).unwrap();
        let sig_c = synthesis_signature(&[chunk.clone()], &settings_c).unwrap();

        assert_eq!(sig_a, sig_b);
        assert_ne!(sig_a, sig_c);
    }
}
