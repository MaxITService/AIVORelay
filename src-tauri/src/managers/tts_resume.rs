//! Crash-safe checkpoints for provider-generated TTS PCM.
//!
//! The final WAV/MP3 remains non-resumable and is always encoded from one
//! verified PCM prefix. Each PCM segment is synced before an alternating JSON
//! checkpoint is atomically published.

use super::tts::{TtsChunk, TtsManager, PROVIDER_PCM_SAMPLE_RATE};
use crate::settings::{TtsKeySource, TtsProvider, TtsSettings};
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
const PIPELINE_REVISION: &str = "pcm24k-mono-semantic-v1";
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
const MAX_UI_JOB_BYTES: u64 = 32 * 1024 * 1024;
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
    pub source_text: String,
    pub processed_text: Option<String>,
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
        let checkpoint = load_checkpoint(&root, &raw_path, &signature, total_chunks)?;
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
    let (model, language, voice, instructions, speed) = match settings.provider {
        TtsProvider::Soniox => (
            settings.soniox_model.trim(),
            settings.soniox_language.trim(),
            settings.soniox_voice.trim(),
            "",
            settings.speed.clamp(0.7, 1.3),
        ),
        TtsProvider::Deepgram => (
            settings.deepgram_model.trim(),
            "",
            "",
            "",
            settings.speed.clamp(0.7, 1.5),
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
        ),
        TtsProvider::Edge => (
            crate::managers::edge_tts::EDGE_TTS_MODEL,
            settings.edge_voice_language.trim(),
            settings.edge_voice.trim(),
            "",
            settings.speed.clamp(0.5, 2.0),
        ),
        TtsProvider::LocalQwen => (
            crate::managers::local_tts::LOCAL_TTS_MODEL_REVISION,
            settings.local_qwen_language.trim(),
            settings.local_qwen_voice.trim(),
            "",
            settings.speed.clamp(0.5, 2.0),
        ),
        TtsProvider::LocalKokoro => (
            crate::managers::local_kokoro::KOKORO_MODEL_REVISION,
            settings.local_kokoro_language.trim(),
            settings.local_kokoro_voice.trim(),
            "",
            settings.speed.clamp(0.5, 2.0),
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
        chunks: Vec::new(),
        settings,
        status: UiFileJobStatus::Planned,
        completed_chunks: 0,
        last_error: None,
        created_at_ms: now,
        updated_at_ms: now,
    };
    persist_ui_file_job(cache_root, &manifest)?;
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
        if let Some(manifest) = read_ui_job_candidates(&entry.path())
            .into_iter()
            .filter(|job| job.schema_version == SCHEMA_VERSION && job.job_id == job_id)
            .max_by_key(|job| job.generation)
            .filter(|job| {
                job.status != UiFileJobStatus::Completed && job.output_path == output_path
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
    read_ui_job_candidates(&ui_job_root(cache_root, job_id))
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

    let bytes = serde_json::to_vec(manifest).context("Failed to serialize the saved TTS job")?;
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
        let Some(mut manifest) = read_ui_job_candidates(&entry.path())
            .into_iter()
            .filter(|job| job.schema_version == SCHEMA_VERSION && job.job_id == job_id)
            .max_by_key(|job| job.generation)
        else {
            continue;
        };
        if let Ok(completed_chunks) = ui_job_completed_chunks(&manifest) {
            manifest.completed_chunks = completed_chunks;
        }
        if matches!(
            manifest.status,
            UiFileJobStatus::Preparing | UiFileJobStatus::Running | UiFileJobStatus::Retrying
        ) && active_job_id != Some(manifest.job_id.as_str())
        {
            manifest.status = UiFileJobStatus::Interrupted;
            manifest.last_error = Some("AivoRelay closed before this conversion finished".into());
            touch_ui_job(&mut manifest);
            persist_ui_file_job(cache_root, &manifest)?;
        }
        if manifest.status != UiFileJobStatus::Completed && !manifest.output_path.exists() {
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
        discard_owned_workspace(&workspace, false)?;
    }
    remove_ui_file_job_record(cache_root, job_id)
}

fn ui_job_root(cache_root: &Path, job_id: &str) -> PathBuf {
    cache_root.join(UI_JOBS_ROOT).join(job_id)
}

fn ui_job_completed_chunks(manifest: &UiFileJobManifest) -> Result<usize> {
    if manifest.chunks.is_empty() || manifest.processed_text.is_none() {
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

fn read_ui_job_candidates(root: &Path) -> Vec<UiFileJobManifest> {
    UI_JOB_SLOTS
        .iter()
        .filter_map(|slot| read_ui_file_job(&root.join(slot)).ok().flatten())
        .collect()
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
        if output_path.exists() || output_workspace_root(&output_path) != root {
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
) -> Result<Option<ResumeCheckpoint>> {
    let raw_length = fs::metadata(raw_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let candidates = read_checkpoint_candidates(root);
    let compatible = candidates
        .iter()
        .filter(|checkpoint| {
            checkpoint.schema_version == SCHEMA_VERSION
                && checkpoint.pipeline_revision == PIPELINE_REVISION
                && checkpoint.synthesis_signature == signature
                && checkpoint.total_chunks == total_chunks
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
}
