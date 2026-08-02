//! App-managed local Qwen3-TTS runtime and model lifecycle.
//!
//! Installation is explicit and network-enabled. Synthesis is forced offline
//! and communicates with a persistent Python worker over a versioned JSON-lines
//! protocol; no local HTTP endpoint is exposed.

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use fs2::available_space;
use futures_util::StreamExt;
use hf_hub::api::tokio::{ApiBuilder, ApiError, Progress};
use hf_hub::{Repo, RepoType};
use parking_lot::{Mutex, RwLock};
use reqwest::header::{CONTENT_RANGE, RANGE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio_util::sync::CancellationToken;

pub const LOCAL_TTS_EVENT_STATUS: &str = "local-tts://status";
pub const LOCAL_TTS_PROVIDER_LIMIT: usize = 4_096;
pub const LOCAL_TTS_MODEL_REPO: &str = "Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice";
pub const LOCAL_TTS_MODEL_REVISION: &str = "85e237c12c027371202489a0ec509ded67b5e4b5";
pub const LOCAL_TTS_MODEL_BYTES: u64 = 2_498_388_392;
pub const LOCAL_TTS_QWEN_PACKAGE: &str = "qwen-tts==0.1.1";
pub const LOCAL_TTS_PYTHON_VERSION: &str = "3.12.8";
pub const LOCAL_TTS_UV_VERSION: &str = "0.11.16";

const INSTALL_MANIFEST_VERSION: u32 = 2;
const WORKER_PROTOCOL_VERSION: u32 = 1;
const EXPECTED_SAMPLE_RATE: u32 = 24_000;
const MAX_WORKER_WAV_BYTES: u64 = 512 * 1024 * 1024;
// Opening every file for a Windows file ID made status scans take minutes in a
// Python environment with tens of thousands of small files. Large hard links
// account for nearly all duplicated bytes, so deduplicate those exactly and
// conservatively count smaller files by their logical length.
pub(super) const HARD_LINK_DEDUPLICATION_MIN_BYTES: u64 = 1024 * 1024;
// Includes the model cache layout, managed Python environment, CUDA/CPU wheels,
// extraction overhead, and enough headroom for installer updates.
pub const LOCAL_TTS_INSTALL_ESTIMATE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const LOCAL_TTS_MODEL_SOURCE_URL: &str = "https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice/tree/85e237c12c027371202489a0ec509ded67b5e4b5";
pub const LOCAL_TTS_MODEL_LICENSE_URL: &str = "https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice/blob/85e237c12c027371202489a0ec509ded67b5e4b5/README.md";
pub(crate) const UV_WINDOWS_URL: &str =
    "https://github.com/astral-sh/uv/releases/download/0.11.16/uv-x86_64-pc-windows-msvc.zip";
pub(crate) const UV_WINDOWS_SHA256: &str =
    "dd9d6d6554bfab265bfa98aa8e8a406c5c3a7b97582f93de1f4d48d9154a0395";
pub(crate) const UV_WINDOWS_ZIP_NAME: &str = "uv-x86_64-pc-windows-msvc.zip";
const WORKER_SOURCE: &str = include_str!("local_tts_worker.py");
const APACHE_LICENSE: &str = include_str!("../../resources/licenses/Apache-2.0.txt");
const UV_MIT_LICENSE: &str = include_str!("../../resources/licenses/uv-MIT.txt");
const QWEN_NOTICE: &str = include_str!("../../resources/licenses/Qwen3-TTS-NOTICE.txt");

const MODEL_FILES: &[(&str, u64, Option<&str>)] = &[
    (".gitattributes", 1_519, None),
    ("README.md", 3_263, None),
    ("config.json", 4_908, None),
    ("generation_config.json", 245, None),
    ("merges.txt", 1_671_839, None),
    (
        "model.safetensors",
        1_811_626_576,
        Some("bc3c7e785eb961179c25450d1acff03f839e0002f2f3a5aeb67b5735c0fa2adb"),
    ),
    ("preprocessor_config.json", 127, None),
    ("speech_tokenizer/config.json", 2_336, None),
    ("speech_tokenizer/configuration.json", 76, None),
    (
        "speech_tokenizer/model.safetensors",
        682_293_092,
        Some("836b7b357f5ea43e889936a3709af68dfe3751881acefe4ecf0dbd30ba571258"),
    ),
    ("speech_tokenizer/preprocessor_config.json", 234, None),
    ("tokenizer_config.json", 7_344, None),
    ("vocab.json", 2_776_833, None),
];

pub const LOCAL_TTS_VOICES: &[&str] = &[
    "Vivian", "Serena", "Uncle_Fu", "Dylan", "Eric", "Ryan", "Aiden", "Ono_Anna", "Sohee",
];

pub const LOCAL_TTS_LANGUAGES: &[&str] = &[
    "Auto",
    "Chinese",
    "English",
    "Japanese",
    "Korean",
    "German",
    "French",
    "Russian",
    "Portuguese",
    "Spanish",
    "Italian",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalTtsKind {
    Qwen,
    Kokoro,
}

impl Default for LocalTtsKind {
    fn default() -> Self {
        Self::Qwen
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocalTtsStatus {
    #[serde(default)]
    pub kind: LocalTtsKind,
    pub installed: bool,
    pub installing: bool,
    pub phase: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
    pub runtime_profile: String,
    pub model_repository: String,
    pub model_revision: String,
    pub model_download_bytes: u64,
    #[serde(default)]
    pub install_root: String,
    #[serde(default)]
    pub installed_size_bytes: u64,
    #[serde(default)]
    pub estimated_install_bytes: u64,
    #[serde(default)]
    pub model_author: String,
    #[serde(default)]
    pub model_source_url: String,
    #[serde(default)]
    pub model_license_name: String,
    #[serde(default)]
    pub model_license_url: String,
    #[serde(default)]
    pub model_license_path: String,
    #[serde(default)]
    pub model_license_declaration_path: String,
    #[serde(default)]
    pub model_license_available: bool,
    pub error: Option<String>,
}

impl Default for LocalTtsStatus {
    fn default() -> Self {
        Self {
            kind: LocalTtsKind::Qwen,
            installed: false,
            installing: false,
            phase: "not_installed".to_string(),
            downloaded_bytes: 0,
            total_bytes: LOCAL_TTS_MODEL_BYTES,
            percentage: 0.0,
            runtime_profile: String::new(),
            model_repository: LOCAL_TTS_MODEL_REPO.to_string(),
            model_revision: LOCAL_TTS_MODEL_REVISION.to_string(),
            model_download_bytes: LOCAL_TTS_MODEL_BYTES,
            install_root: String::new(),
            installed_size_bytes: 0,
            estimated_install_bytes: LOCAL_TTS_INSTALL_ESTIMATE_BYTES,
            model_author: "Qwen Team (Alibaba Cloud)".to_string(),
            model_source_url: LOCAL_TTS_MODEL_SOURCE_URL.to_string(),
            model_license_name: "Apache License 2.0".to_string(),
            model_license_url: LOCAL_TTS_MODEL_LICENSE_URL.to_string(),
            model_license_path: String::new(),
            model_license_declaration_path: String::new(),
            model_license_available: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallManifest {
    manifest_version: u32,
    worker_protocol_version: u32,
    model_repository: String,
    model_revision: String,
    model_bytes: u64,
    qwen_package: String,
    python_version: String,
    uv_version: String,
    worker_sha256: String,
    #[serde(default)]
    notice_bundle_sha256: String,
    #[serde(default)]
    runtime_smoke_tested: bool,
    runtime_profile: String,
    resolved_packages: Vec<String>,
}

#[derive(Debug)]
pub struct LocalTtsAttemptError {
    pub safe_message: String,
    pub transient: bool,
}

struct LocalWorker {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    profile: String,
}

#[derive(Debug, Deserialize)]
struct WorkerResponse {
    #[serde(rename = "type")]
    kind: String,
    protocol: u32,
    id: Option<String>,
    output_path: Option<String>,
    sample_rate: Option<u32>,
    samples: Option<u64>,
    message: Option<String>,
    retryable: Option<bool>,
}

pub struct LocalTtsRuntime {
    app_handle: AppHandle,
    root: PathBuf,
    client: reqwest::Client,
    status: Arc<RwLock<LocalTtsStatus>>,
    lifecycle: tokio::sync::Mutex<()>,
    install_cancel: Mutex<Option<CancellationToken>>,
    worker: tokio::sync::Mutex<Option<LocalWorker>>,
    request_id: AtomicU64,
}

impl LocalTtsRuntime {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let root = crate::portable::app_data_dir(app_handle)
            .map_err(|error| anyhow!("Failed to resolve app data directory: {error}"))?
            .join("local-tts")
            .join("qwen3-tts-0.6b-custom-voice");
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(30 * 60))
            .build()
            .context("Failed to build local TTS installer HTTP client")?;
        let runtime = Self {
            app_handle: app_handle.clone(),
            status: Arc::new(RwLock::new(qwen_status(&root))),
            root,
            client,
            lifecycle: tokio::sync::Mutex::new(()),
            install_cancel: Mutex::new(None),
            worker: tokio::sync::Mutex::new(None),
            request_id: AtomicU64::new(1),
        };
        runtime.refresh_status();
        Ok(runtime)
    }

    pub fn status(&self) -> LocalTtsStatus {
        self.refresh_status();
        self.status.read().clone()
    }

    pub fn is_installed(&self) -> bool {
        self.installation_manifest().is_ok()
    }

    pub async fn install(&self, disk_reserve_mb: u32) -> Result<LocalTtsStatus> {
        let _lifecycle_guard = self
            .lifecycle
            .try_lock()
            .map_err(|_| anyhow!("Another local TTS install or delete operation is running"))?;
        if self.is_installed() {
            return Ok(self.status());
        }
        let cancel = CancellationToken::new();
        *self.install_cancel.lock() = Some(cancel.clone());

        let result = async {
            if self.upgrade_worker_if_compatible(&cancel).await? {
                return Ok(());
            }

            cancel_if_requested(&cancel)?;
            fs::create_dir_all(&self.root).context("Failed to create local TTS directory")?;
            self.preflight_disk_space(disk_reserve_mb)?;
            self.set_status("preparing", true, 0, LOCAL_TTS_MODEL_BYTES, None);
            self.install_inner(&cancel).await
        }
        .await;
        self.install_cancel.lock().take();
        match result {
            Ok(()) => {
                self.refresh_status();
                let status = self.status.read().clone();
                self.emit_status(&status);
                Ok(status)
            }
            Err(_error) if cancel.is_cancelled() => {
                self.set_status(
                    "cancelled",
                    false,
                    self.model_downloaded_bytes(),
                    LOCAL_TTS_MODEL_BYTES,
                    None,
                );
                Err(anyhow!("Local TTS installation cancelled"))
            }
            Err(error) => {
                self.set_status(
                    "error",
                    false,
                    self.model_downloaded_bytes(),
                    LOCAL_TTS_MODEL_BYTES,
                    Some(error.to_string()),
                );
                Err(error)
            }
        }
    }

    pub fn cancel_install(&self) -> bool {
        if let Some(token) = self.install_cancel.lock().as_ref() {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn delete(&self) -> Result<()> {
        let _lifecycle_guard = self
            .lifecycle
            .try_lock()
            .map_err(|_| anyhow!("Another local TTS install or delete operation is running"))?;
        if self.install_cancel.lock().is_some() {
            return Err(anyhow!(
                "Cancel the active local TTS installation before deleting it"
            ));
        }
        self.stop_worker().await;
        if self.root.exists() {
            let marker = self.root.join(".aivorelay-local-tts");
            if !marker.is_file() {
                return Err(anyhow!(
                    "Refusing to delete a directory without an AivoRelay ownership marker"
                ));
            }
            fs::remove_dir_all(&self.root).context("Failed to delete local TTS files")?;
        }
        self.set_status("not_installed", false, 0, LOCAL_TTS_MODEL_BYTES, None);
        Ok(())
    }

    pub async fn synthesize(
        &self,
        text: &str,
        speaker: &str,
        language: &str,
        instructions: &str,
        speed: f32,
    ) -> std::result::Result<Vec<i16>, LocalTtsAttemptError> {
        if !LOCAL_TTS_VOICES.contains(&speaker) {
            return Err(LocalTtsAttemptError {
                safe_message: format!("Unsupported Qwen3-TTS voice: {speaker}"),
                transient: false,
            });
        }
        if !LOCAL_TTS_LANGUAGES.contains(&language) {
            return Err(LocalTtsAttemptError {
                safe_message: format!("Unsupported Qwen3-TTS language: {language}"),
                transient: false,
            });
        }
        let manifest = self.installation_manifest().map_err(|error| LocalTtsAttemptError {
            safe_message: format!(
                "Local Qwen3-TTS is not installed or needs repair: {error}. Install it in Text to Speech settings."
            ),
            transient: false,
        })?;
        let request_id = self.request_id.fetch_add(1, Ordering::Relaxed).to_string();
        let output_path = self
            .worker_output_dir()
            .join(format!("request-{request_id}.wav"));
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| LocalTtsAttemptError {
                safe_message: format!("Failed to prepare local TTS output: {error}"),
                transient: false,
            })?;
        }
        let _ = fs::remove_file(&output_path);

        let mut guard = self.worker.lock().await;
        if guard
            .as_ref()
            .is_none_or(|worker| worker.profile != manifest.runtime_profile)
        {
            if let Some(mut worker) = guard.take() {
                let _ = worker.child.kill().await;
            }
            *guard = Some(self.start_worker(&manifest, None).await?);
        }
        let worker = guard.as_mut().expect("worker initialized");
        let request = serde_json::json!({
            "type": "synthesize",
            "protocol": WORKER_PROTOCOL_VERSION,
            "id": request_id,
            "text_b64": base64::engine::general_purpose::STANDARD.encode(text.as_bytes()),
            "speaker": speaker,
            "language": language,
            "instruct": instructions,
            "speed": speed.clamp(0.5, 2.0),
            "output_path": output_path,
        });
        let serialized = serde_json::to_string(&request).map_err(permanent_local_error)?;
        if let Err(error) = worker
            .stdin
            .write_all(format!("{serialized}\n").as_bytes())
            .await
        {
            let _ = worker.child.kill().await;
            *guard = None;
            return Err(transient_local_error(format!(
                "Local TTS worker stopped accepting requests: {error}"
            )));
        }
        if let Err(error) = worker.stdin.flush().await {
            let _ = worker.child.kill().await;
            *guard = None;
            return Err(transient_local_error(format!(
                "Local TTS worker request flush failed: {error}"
            )));
        }

        let line = match worker.stdout.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => {
                let _ = worker.child.kill().await;
                *guard = None;
                return Err(transient_local_error(
                    "Local TTS worker exited before returning audio".to_string(),
                ));
            }
            Err(error) => {
                let _ = worker.child.kill().await;
                *guard = None;
                return Err(transient_local_error(format!(
                    "Failed to read local TTS worker response: {error}"
                )));
            }
        };
        let response: WorkerResponse =
            serde_json::from_str(&line).map_err(|error| LocalTtsAttemptError {
                safe_message: format!("Local TTS worker returned malformed protocol data: {error}"),
                transient: false,
            })?;
        validate_worker_response(&response, &request_id)?;
        if response.kind == "error" {
            return Err(LocalTtsAttemptError {
                safe_message: response
                    .message
                    .unwrap_or_else(|| "Local TTS worker reported an unknown error".to_string()),
                transient: response.retryable.unwrap_or(false),
            });
        }
        let response_path = PathBuf::from(
            response
                .output_path
                .as_deref()
                .ok_or_else(|| permanent_local_error("Worker omitted the output path"))?,
        );
        if response_path != output_path {
            return Err(permanent_local_error(
                "Local TTS worker returned an unexpected output path",
            ));
        }
        let expected_samples =
            response
                .samples
                .filter(|samples| *samples > 0)
                .ok_or_else(|| {
                    permanent_local_error("Local TTS worker omitted valid audio sample metadata")
                })?;
        if response.sample_rate != Some(EXPECTED_SAMPLE_RATE) {
            return Err(permanent_local_error(
                "Local TTS worker returned invalid audio metadata",
            ));
        }
        drop(guard);

        let parsed = read_worker_wav(&output_path).and_then(|samples| {
            if samples.len() as u64 != expected_samples {
                return Err(permanent_local_error(
                    "Local TTS worker audio length does not match its response metadata",
                ));
            }
            Ok(samples)
        });
        let _ = fs::remove_file(&output_path);
        parsed
    }

    pub async fn stop_worker(&self) {
        let mut guard = self.worker.lock().await;
        if let Some(mut worker) = guard.take() {
            let shutdown = serde_json::json!({
                "type": "shutdown",
                "protocol": WORKER_PROTOCOL_VERSION,
                "id": "shutdown",
            });
            let _ = worker
                .stdin
                .write_all(format!("{shutdown}\n").as_bytes())
                .await;
            let _ = worker.stdin.flush().await;
            let _ = tokio::time::timeout(Duration::from_secs(2), worker.child.wait()).await;
            let _ = worker.child.kill().await;
        }
    }

    async fn install_inner(&self, cancel: &CancellationToken) -> Result<()> {
        write_owned_marker(&self.root)?;
        self.install_uv(cancel).await?;
        let runtime_profile = detect_runtime_profile();
        self.install_python_runtime(&runtime_profile, cancel)
            .await?;
        self.download_model(cancel).await?;
        cancel_if_requested(cancel)?;
        fs::write(self.worker_path(), WORKER_SOURCE)
            .context("Failed to install local TTS worker")?;
        self.write_managed_notices()?;
        let resolved_packages = self.freeze_packages(cancel).await?;
        let mut manifest = InstallManifest {
            manifest_version: INSTALL_MANIFEST_VERSION,
            worker_protocol_version: WORKER_PROTOCOL_VERSION,
            model_repository: LOCAL_TTS_MODEL_REPO.to_string(),
            model_revision: LOCAL_TTS_MODEL_REVISION.to_string(),
            model_bytes: LOCAL_TTS_MODEL_BYTES,
            qwen_package: LOCAL_TTS_QWEN_PACKAGE.to_string(),
            python_version: LOCAL_TTS_PYTHON_VERSION.to_string(),
            uv_version: LOCAL_TTS_UV_VERSION.to_string(),
            worker_sha256: sha256_bytes(WORKER_SOURCE.as_bytes()),
            notice_bundle_sha256: notice_bundle_sha256(),
            runtime_smoke_tested: false,
            runtime_profile,
            resolved_packages,
        };
        let mut worker = self
            .start_verified_worker_with_fallback(&mut manifest, Some(cancel))
            .await?;
        if let Err(error) = cancel_if_requested(cancel) {
            let _ = worker.child.kill().await;
            return Err(error);
        }
        manifest.runtime_smoke_tested = true;
        write_json_atomic(&self.manifest_path(), &manifest)?;
        if let Err(error) = cancel_if_requested(cancel) {
            let _ = fs::remove_file(self.manifest_path());
            let _ = worker.child.kill().await;
            return Err(error);
        }
        *self.worker.lock().await = Some(worker);
        Ok(())
    }

    async fn install_uv(&self, cancel: &CancellationToken) -> Result<()> {
        if self.uv_path().is_file() {
            return Ok(());
        }
        self.set_status("downloading_runtime", true, 0, 0, None);
        fs::create_dir_all(self.runtime_dir())?;
        let archive_path = self.runtime_dir().join(UV_WINDOWS_ZIP_NAME);
        download_resumable(
            &self.client,
            UV_WINDOWS_URL,
            &archive_path,
            cancel,
            |downloaded, total| {
                self.set_status("downloading_runtime", true, downloaded, total, None)
            },
        )
        .await?;
        verify_sha256(&archive_path, UV_WINDOWS_SHA256)?;
        let runtime_dir = self.runtime_dir();
        let archive_copy = archive_path.clone();
        tokio::task::spawn_blocking(move || extract_uv(&archive_copy, &runtime_dir))
            .await
            .map_err(|error| anyhow!("uv extraction task failed: {error}"))??;
        if !self.uv_path().is_file() {
            return Err(anyhow!("The verified uv archive did not contain uv.exe"));
        }
        let _ = fs::remove_file(archive_path);
        Ok(())
    }

    async fn install_python_runtime(
        &self,
        profile: &str,
        cancel: &CancellationToken,
    ) -> Result<()> {
        self.set_status("installing_python", true, 0, 0, None);
        fs::create_dir_all(self.runtime_dir())?;
        if !self.venv_python_path().is_file() {
            self.run_uv(
                &[
                    "venv",
                    "--python",
                    LOCAL_TTS_PYTHON_VERSION,
                    "--managed-python",
                    self.venv_dir().to_string_lossy().as_ref(),
                ],
                cancel,
            )
            .await?;
        }

        self.set_status("installing_pytorch", true, 0, 0, None);
        let python = self.venv_python_path().to_string_lossy().to_string();
        let (torch, torchaudio, index) = if profile == "cuda" {
            (
                "torch==2.10.0+cu130",
                "torchaudio==2.10.0+cu130",
                "https://download.pytorch.org/whl/cu130",
            )
        } else {
            (
                "torch==2.10.0+cpu",
                "torchaudio==2.10.0+cpu",
                "https://download.pytorch.org/whl/cpu",
            )
        };
        self.run_uv(
            &[
                "pip",
                "install",
                "--python",
                &python,
                "--index-url",
                index,
                torch,
                torchaudio,
            ],
            cancel,
        )
        .await?;

        self.set_status("installing_qwen", true, 0, 0, None);
        self.run_uv(
            &[
                "pip",
                "install",
                "--python",
                &python,
                LOCAL_TTS_QWEN_PACKAGE,
            ],
            cancel,
        )
        .await?;
        Ok(())
    }

    async fn download_model(&self, cancel: &CancellationToken) -> Result<()> {
        self.set_status(
            "downloading_model",
            true,
            self.model_downloaded_bytes(),
            LOCAL_TTS_MODEL_BYTES,
            None,
        );
        fs::create_dir_all(self.hf_cache_dir())?;
        let api = ApiBuilder::new()
            .with_token(None)
            .with_cache_dir(self.hf_cache_dir())
            .build()
            .context("Failed to create Hugging Face download client")?;
        let repo = api.repo(Repo::with_revision(
            LOCAL_TTS_MODEL_REPO.to_string(),
            RepoType::Model,
            LOCAL_TTS_MODEL_REVISION.to_string(),
        ));
        let mut completed = 0_u64;
        for &(filename, size, expected_sha) in MODEL_FILES {
            cancel_if_requested(cancel)?;
            let progress = AggregateHfProgress {
                app_handle: self.app_handle.clone(),
                status: Arc::clone(&self.status),
                completed_before: completed,
                expected_file_size: size,
            };
            let path = match repo
                .download_with_progress_cancellable(filename, progress, cancel.clone())
                .await
            {
                Ok(path) => path,
                Err(ApiError::Cancelled) => {
                    return Err(anyhow!("Local TTS installation cancelled"))
                }
                Err(error) => {
                    return Err(anyhow!(
                        "Hugging Face model download failed for {filename}: {error}"
                    ))
                }
            };
            if path.metadata()?.len() != size {
                return Err(anyhow!(
                    "Downloaded model file has the wrong size: {filename}"
                ));
            }
            if let Some(expected_sha) = expected_sha {
                verify_sha256(&path, expected_sha)
                    .with_context(|| format!("Model integrity check failed for {filename}"))?;
            }
            completed = completed.saturating_add(size);
        }
        Ok(())
    }

    async fn freeze_packages(&self, cancel: &CancellationToken) -> Result<Vec<String>> {
        let output = self
            .run_uv_capture(
                &[
                    "pip",
                    "freeze",
                    "--python",
                    self.venv_python_path().to_string_lossy().as_ref(),
                ],
                cancel,
            )
            .await?;
        Ok(output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    }

    async fn run_uv(&self, args: &[&str], cancel: &CancellationToken) -> Result<()> {
        self.run_uv_capture(args, cancel).await.map(|_| ())
    }

    async fn run_uv_capture(&self, args: &[&str], cancel: &CancellationToken) -> Result<String> {
        cancel_if_requested(cancel)?;
        let mut command = Command::new(self.uv_path());
        command
            .arg("--no-config")
            .args(args)
            .current_dir(self.runtime_dir())
            .env("UV_CACHE_DIR", self.runtime_dir().join("uv-cache"))
            .env("UV_PYTHON_INSTALL_DIR", self.runtime_dir().join("python"))
            .env("UV_NO_CONFIG", "1")
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH")
            .env_remove("VIRTUAL_ENV")
            .env_remove("CONDA_PREFIX")
            .env_remove("HF_TOKEN")
            .env_remove("HUGGING_FACE_HUB_TOKEN")
            .env_remove("UV_INDEX_URL")
            .env_remove("UV_DEFAULT_INDEX")
            .env_remove("UV_EXTRA_INDEX_URL")
            .env_remove("UV_FIND_LINKS")
            .env_remove("UV_KEYRING_PROVIDER")
            .env_remove("PIP_CONFIG_FILE")
            .env_remove("PIP_INDEX_URL")
            .env_remove("PIP_EXTRA_INDEX_URL")
            .env_remove("PIP_FIND_LINKS")
            .env_remove("PIP_TRUSTED_HOST")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        hide_child_window(&mut command);
        let child = command.spawn().context("Failed to start managed uv")?;
        let output = tokio::select! {
            output = child.wait_with_output() => output.context("Managed uv process failed")?,
            _ = cancel.cancelled() => return Err(anyhow!("Local TTS installation cancelled")),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow!(
                "Managed runtime installation failed (exit {}): {}{}",
                output.status,
                stderr.trim(),
                if stdout.trim().is_empty() {
                    String::new()
                } else {
                    format!("\n{}", stdout.trim())
                }
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn start_worker(
        &self,
        manifest: &InstallManifest,
        cancel: Option<&CancellationToken>,
    ) -> std::result::Result<LocalWorker, LocalTtsAttemptError> {
        let model_path = self
            .model_snapshot_path()
            .map_err(|error| LocalTtsAttemptError {
                safe_message: format!("The local Qwen model is incomplete: {error}"),
                transient: false,
            })?;
        fs::create_dir_all(self.worker_output_dir()).map_err(permanent_local_error)?;
        let mut command = Command::new(self.venv_python_path());
        command
            .arg("-I")
            .arg(self.worker_path())
            .arg("--model")
            .arg(model_path)
            .arg("--output-root")
            .arg(self.worker_output_dir())
            .arg("--device")
            .arg(&manifest.runtime_profile)
            .current_dir(self.runtime_dir())
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("TOKENIZERS_PARALLELISM", "false")
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH")
            .env_remove("VIRTUAL_ENV")
            .env_remove("CONDA_PREFIX")
            .env_remove("HF_TOKEN")
            .env_remove("HUGGING_FACE_HUB_TOKEN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        hide_child_window(&mut command);
        let mut child = command.spawn().map_err(|error| LocalTtsAttemptError {
            safe_message: format!("Failed to start local TTS worker: {error}"),
            transient: true,
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            transient_local_error("Local TTS worker stdin was not available".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            transient_local_error("Local TTS worker stdout was not available".to_string())
        })?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                // Always drain the pipe so third-party diagnostics cannot
                // block the worker. Do not log arbitrary model/runtime text:
                // some libraries may echo user input, and actionable errors
                // are returned through the sanitized JSON protocol instead.
                while let Ok(Some(_line)) = lines.next_line().await {}
            });
        }
        let mut worker = LocalWorker {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            profile: manifest.runtime_profile.clone(),
        };
        let ready_wait =
            tokio::time::timeout(Duration::from_secs(5 * 60), worker.stdout.next_line());
        let ready_result = if let Some(cancel) = cancel {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => None,
                result = ready_wait => Some(result),
            }
        } else {
            Some(ready_wait.await)
        };
        let Some(ready_result) = ready_result else {
            let _ = worker.child.kill().await;
            return Err(permanent_local_error("Local TTS installation cancelled"));
        };
        let ready_line = ready_result
            .map_err(|_| {
                transient_local_error(
                    "Timed out while loading the local Qwen3-TTS model".to_string(),
                )
            })?
            .map_err(|error| {
                transient_local_error(format!(
                    "Failed to read local TTS startup response: {error}"
                ))
            })?
            .ok_or_else(|| {
                transient_local_error("Local TTS worker exited while loading the model".to_string())
            })?;
        let response: WorkerResponse =
            serde_json::from_str(&ready_line).map_err(|error| LocalTtsAttemptError {
                safe_message: format!("Local TTS worker startup protocol is malformed: {error}"),
                transient: false,
            })?;
        if response.protocol != WORKER_PROTOCOL_VERSION || response.kind != "ready" {
            let _ = worker.child.kill().await;
            return Err(LocalTtsAttemptError {
                safe_message: response
                    .message
                    .unwrap_or_else(|| "Local TTS worker failed to load the model".to_string()),
                transient: false,
            });
        }
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            let _ = worker.child.kill().await;
            return Err(permanent_local_error("Local TTS installation cancelled"));
        }
        Ok(worker)
    }

    async fn start_verified_worker_with_fallback(
        &self,
        manifest: &mut InstallManifest,
        cancel: Option<&CancellationToken>,
    ) -> Result<LocalWorker> {
        self.set_status("validating_runtime", true, 0, 0, None);
        match self.start_worker(manifest, cancel).await {
            Ok(worker) => Ok(worker),
            Err(_) if cancel.is_some_and(CancellationToken::is_cancelled) => {
                Err(anyhow!("Local TTS installation cancelled"))
            }
            Err(cuda_error) if manifest.runtime_profile == "cuda" => {
                log::warn!(
                    "Local TTS CUDA validation failed; retrying on CPU: {}",
                    cuda_error.safe_message
                );
                manifest.runtime_profile = "cpu".to_string();
                self.start_worker(manifest, cancel)
                    .await
                    .map_err(|cpu_error| {
                        anyhow!(
                            "Local TTS runtime validation failed on CUDA ({}) and CPU ({})",
                            cuda_error.safe_message,
                            cpu_error.safe_message
                        )
                    })
            }
            Err(error) => Err(anyhow!(
                "Local TTS runtime validation failed: {}",
                error.safe_message
            )),
        }
    }

    fn installation_manifest(&self) -> Result<InstallManifest> {
        let content =
            fs::read_to_string(self.manifest_path()).context("installation manifest is missing")?;
        let manifest: InstallManifest =
            serde_json::from_str(&content).context("installation manifest is invalid")?;
        validate_install_manifest(&manifest)?;
        if !self.uv_path().is_file()
            || !self.venv_python_path().is_file()
            || !self.worker_path().is_file()
            || !self.model_snapshot_path()?.is_dir()
        {
            return Err(anyhow!("installed runtime or model files are incomplete"));
        }
        verify_sha256(&self.worker_path(), &sha256_bytes(WORKER_SOURCE.as_bytes()))
            .context("installed local TTS worker failed integrity verification")?;
        if !self.managed_notices_valid() {
            return Err(anyhow!(
                "installed local TTS license or notice files are incomplete"
            ));
        }
        Ok(manifest)
    }

    async fn upgrade_worker_if_compatible(&self, cancel: &CancellationToken) -> Result<bool> {
        let content = match fs::read_to_string(self.manifest_path()) {
            Ok(content) => content,
            Err(_) => return Ok(false),
        };
        let mut manifest: InstallManifest = match serde_json::from_str(&content) {
            Ok(manifest) => manifest,
            Err(_) => return Ok(false),
        };
        let original_manifest = manifest.clone();
        let current_worker_sha = sha256_bytes(WORKER_SOURCE.as_bytes());
        if manifest.manifest_version != INSTALL_MANIFEST_VERSION
            || manifest.worker_protocol_version != WORKER_PROTOCOL_VERSION
            || manifest.model_repository != LOCAL_TTS_MODEL_REPO
            || manifest.model_revision != LOCAL_TTS_MODEL_REVISION
            || manifest.model_bytes != LOCAL_TTS_MODEL_BYTES
            || manifest.qwen_package != LOCAL_TTS_QWEN_PACKAGE
            || manifest.python_version != LOCAL_TTS_PYTHON_VERSION
            || manifest.uv_version != LOCAL_TTS_UV_VERSION
            || !matches!(manifest.runtime_profile.as_str(), "cuda" | "cpu")
            || !self.uv_path().is_file()
            || !self.venv_python_path().is_file()
            || !self.model_snapshot_path()?.is_dir()
        {
            return Ok(false);
        }
        let current_notice_sha = notice_bundle_sha256();
        let worker_file_valid = verify_sha256(&self.worker_path(), &current_worker_sha).is_ok();
        let needs_runtime_validation = !manifest.runtime_smoke_tested
            || manifest.worker_sha256 != current_worker_sha
            || !worker_file_valid;
        if manifest.worker_sha256 == current_worker_sha
            && manifest.notice_bundle_sha256 == current_notice_sha
            && manifest.runtime_smoke_tested
            && worker_file_valid
            && self.managed_notices_valid()
        {
            return Ok(false);
        }
        cancel_if_requested(cancel)?;
        fs::write(self.worker_path(), WORKER_SOURCE)
            .context("Failed to update the managed local TTS worker")?;
        self.write_managed_notices()?;
        manifest.worker_sha256 = current_worker_sha;
        manifest.notice_bundle_sha256 = current_notice_sha;
        let mut validated_worker = None;
        if needs_runtime_validation {
            let mut worker = self
                .start_verified_worker_with_fallback(&mut manifest, Some(cancel))
                .await?;
            if let Err(error) = cancel_if_requested(cancel) {
                let _ = worker.child.kill().await;
                return Err(error);
            }
            manifest.runtime_smoke_tested = true;
            validated_worker = Some(worker);
        }
        cancel_if_requested(cancel)?;
        write_json_atomic(&self.manifest_path(), &manifest)?;
        if let Err(error) = cancel_if_requested(cancel) {
            let _ = write_json_atomic(&self.manifest_path(), &original_manifest);
            if let Some(mut worker) = validated_worker {
                let _ = worker.child.kill().await;
            }
            return Err(error);
        }
        if let Some(worker) = validated_worker {
            *self.worker.lock().await = Some(worker);
        }
        Ok(true)
    }

    fn write_managed_notices(&self) -> Result<()> {
        let directory = self.root.join("licenses");
        fs::create_dir_all(&directory)?;
        fs::write(directory.join("Apache-2.0.txt"), APACHE_LICENSE)?;
        fs::write(directory.join("uv-MIT.txt"), UV_MIT_LICENSE)?;
        fs::write(directory.join("Qwen3-TTS-NOTICE.txt"), QWEN_NOTICE)?;
        let upstream_model_card = fs::read(self.model_snapshot_path()?.join("README.md"))
            .context("Failed to read the downloaded Qwen model card")?;
        fs::write(
            directory.join("Qwen3-TTS-UPSTREAM-MODEL-CARD.md"),
            upstream_model_card,
        )?;
        Ok(())
    }

    fn managed_notices_valid(&self) -> bool {
        let directory = self.root.join("licenses");
        let bundled_notices_valid = [
            ("Apache-2.0.txt", APACHE_LICENSE),
            ("uv-MIT.txt", UV_MIT_LICENSE),
            ("Qwen3-TTS-NOTICE.txt", QWEN_NOTICE),
        ]
        .into_iter()
        .all(|(name, expected)| {
            fs::read(directory.join(name))
                .map(|actual| actual == expected.as_bytes())
                .unwrap_or(false)
        });
        let upstream_model_card_valid = self
            .model_snapshot_path()
            .and_then(|root| fs::read(root.join("README.md")).map_err(Into::into))
            .and_then(|expected| {
                fs::read(
                    self.root
                        .join("licenses")
                        .join("Qwen3-TTS-UPSTREAM-MODEL-CARD.md"),
                )
                .map(|actual| actual == expected)
                .map_err(Into::into)
            })
            .unwrap_or(false);
        bundled_notices_valid && upstream_model_card_valid
    }

    fn model_snapshot_path(&self) -> Result<PathBuf> {
        let cache = hf_hub::Cache::new(self.hf_cache_dir());
        let repo = cache.repo(Repo::with_revision(
            LOCAL_TTS_MODEL_REPO.to_string(),
            RepoType::Model,
            LOCAL_TTS_MODEL_REVISION.to_string(),
        ));
        let config = repo
            .get("config.json")
            .ok_or_else(|| anyhow!("config.json is not present in the managed model cache"))?;
        let root = config
            .parent()
            .ok_or_else(|| anyhow!("model snapshot root is invalid"))?
            .to_path_buf();
        for &(filename, size, _) in MODEL_FILES {
            let path = root.join(filename);
            if path.metadata().map(|metadata| metadata.len()).unwrap_or(0) != size {
                return Err(anyhow!("model file is missing or incomplete: {filename}"));
            }
        }
        Ok(root)
    }

    fn refresh_status(&self) {
        if self.install_cancel.lock().is_some() {
            return;
        }
        match self.installation_manifest() {
            Ok(manifest) => {
                let mut status = self.status.write();
                status.installed = true;
                status.installing = false;
                status.phase = "ready".to_string();
                status.downloaded_bytes = LOCAL_TTS_MODEL_BYTES;
                status.total_bytes = LOCAL_TTS_MODEL_BYTES;
                status.percentage = 100.0;
                status.runtime_profile = manifest.runtime_profile;
                if status.installed_size_bytes == 0 {
                    status.installed_size_bytes = directory_size_bytes(&self.root);
                }
                status.model_license_available = self.model_license_path().is_file()
                    && self.model_license_declaration_path().is_file();
                status.error = None;
            }
            Err(error) => {
                let partial = self.model_downloaded_bytes();
                let mut status = self.status.write();
                status.installed = false;
                status.installing = false;
                if status.phase != "error" && status.phase != "cancelled" {
                    status.phase = if partial > 0 {
                        "partial".to_string()
                    } else {
                        "not_installed".to_string()
                    };
                    status.error = if self.manifest_path().exists() {
                        Some(error.to_string())
                    } else {
                        None
                    };
                }
                status.downloaded_bytes = partial;
                status.total_bytes = LOCAL_TTS_MODEL_BYTES;
                status.percentage =
                    (partial as f64 / LOCAL_TTS_MODEL_BYTES as f64 * 100.0).clamp(0.0, 100.0);
                status.installed_size_bytes = directory_size_bytes(&self.root);
                status.model_license_available = self.model_license_path().is_file()
                    && self.model_license_declaration_path().is_file();
            }
        }
    }

    fn set_status(
        &self,
        phase: &str,
        installing: bool,
        downloaded: u64,
        total: u64,
        error: Option<String>,
    ) {
        let snapshot = {
            let mut status = self.status.write();
            status.installed = phase == "ready";
            status.installing = installing;
            status.phase = phase.to_string();
            status.downloaded_bytes = downloaded;
            status.total_bytes = total;
            status.percentage = if total > 0 {
                (downloaded as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            status.error = error;
            status.clone()
        };
        self.emit_status(&snapshot);
    }

    fn emit_status(&self, status: &LocalTtsStatus) {
        let _ = self.app_handle.emit(LOCAL_TTS_EVENT_STATUS, status);
    }

    fn preflight_disk_space(&self, disk_reserve_mb: u32) -> Result<()> {
        let available = available_space(&self.root).or_else(|_| {
            self.root
                .parent()
                .ok_or_else(|| std::io::Error::other("No parent data directory"))
                .and_then(available_space)
        })?;
        let required = required_install_bytes(disk_reserve_mb);
        if !has_required_disk_space(available, required) {
            return Err(anyhow!(
                "Local Qwen3-TTS needs about {:.1} GiB free plus the configured disk reserve; only {:.1} GiB is available",
                LOCAL_TTS_INSTALL_ESTIMATE_BYTES as f64 / 1024_f64.powi(3),
                available as f64 / 1024_f64.powi(3)
            ));
        }
        Ok(())
    }

    fn model_downloaded_bytes(&self) -> u64 {
        let cache = hf_hub::Cache::new(self.hf_cache_dir());
        let repo = cache.repo(Repo::with_revision(
            LOCAL_TTS_MODEL_REPO.to_string(),
            RepoType::Model,
            LOCAL_TTS_MODEL_REVISION.to_string(),
        ));
        MODEL_FILES
            .iter()
            .filter_map(|(filename, expected, _)| {
                repo.get(filename).and_then(|path| {
                    path.metadata()
                        .ok()
                        .filter(|metadata| metadata.len() == *expected)
                        .map(|_| *expected)
                })
            })
            .sum()
    }

    fn runtime_dir(&self) -> PathBuf {
        self.root.join("runtime")
    }

    fn uv_path(&self) -> PathBuf {
        self.runtime_dir().join("uv.exe")
    }

    fn venv_dir(&self) -> PathBuf {
        self.runtime_dir().join("venv")
    }

    fn venv_python_path(&self) -> PathBuf {
        self.venv_dir().join("Scripts").join("python.exe")
    }

    fn hf_cache_dir(&self) -> PathBuf {
        self.root.join("model-cache")
    }

    fn worker_path(&self) -> PathBuf {
        self.runtime_dir().join("aivorelay_qwen_worker.py")
    }

    fn worker_output_dir(&self) -> PathBuf {
        self.root.join("worker-output")
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("install.json")
    }

    fn model_license_path(&self) -> PathBuf {
        self.root.join("licenses").join("Apache-2.0.txt")
    }

    fn model_license_declaration_path(&self) -> PathBuf {
        self.root
            .join("licenses")
            .join("Qwen3-TTS-UPSTREAM-MODEL-CARD.md")
    }
}

fn qwen_status(root: &Path) -> LocalTtsStatus {
    let mut status = LocalTtsStatus::default();
    status.install_root = root.to_string_lossy().to_string();
    status.model_license_path = root
        .join("licenses")
        .join("Apache-2.0.txt")
        .to_string_lossy()
        .to_string();
    status.model_license_declaration_path = root
        .join("licenses")
        .join("Qwen3-TTS-UPSTREAM-MODEL-CARD.md")
        .to_string_lossy()
        .to_string();
    status
}

pub(super) fn directory_size_bytes(root: &Path) -> u64 {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut size_counts = std::collections::HashMap::<u64, usize>::new();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = path.symlink_metadata() else {
                continue;
            };
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                let length = metadata.len();
                files.push((path, metadata));
                *size_counts.entry(length).or_default() += 1;
            }
        }
    }

    let mut total = 0_u64;
    let mut seen_file_ids = std::collections::HashSet::new();
    for (path, metadata) in files {
        let length = metadata.len();
        if length >= HARD_LINK_DEDUPLICATION_MIN_BYTES
            && size_counts.get(&length).copied().unwrap_or_default() > 1
        {
            if let Some(file_id) = platform_file_id(&path, &metadata) {
                if !seen_file_ids.insert(file_id) {
                    continue;
                }
            }
        }
        total = total.saturating_add(length);
    }
    total
}

#[cfg(windows)]
fn platform_file_id(path: &Path, _metadata: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let file = File::open(path).ok()?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // The handle remains owned by `file` for the duration of this call.
    unsafe {
        GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information).ok()?;
    }
    let file_index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    Some((u64::from(information.dwVolumeSerialNumber), file_index))
}

#[cfg(unix)]
fn platform_file_id(_path: &Path, metadata: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;

    Some((metadata.dev(), metadata.ino()))
}

#[cfg(not(any(windows, unix)))]
fn platform_file_id(_path: &Path, _metadata: &fs::Metadata) -> Option<(u64, u64)> {
    None
}

fn validate_install_manifest(manifest: &InstallManifest) -> Result<()> {
    if manifest.manifest_version != INSTALL_MANIFEST_VERSION
        || manifest.worker_protocol_version != WORKER_PROTOCOL_VERSION
        || manifest.model_repository != LOCAL_TTS_MODEL_REPO
        || manifest.model_revision != LOCAL_TTS_MODEL_REVISION
        || manifest.model_bytes != LOCAL_TTS_MODEL_BYTES
        || manifest.qwen_package != LOCAL_TTS_QWEN_PACKAGE
        || manifest.python_version != LOCAL_TTS_PYTHON_VERSION
        || manifest.uv_version != LOCAL_TTS_UV_VERSION
        || manifest.worker_sha256 != sha256_bytes(WORKER_SOURCE.as_bytes())
        || manifest.notice_bundle_sha256 != notice_bundle_sha256()
        || !manifest.runtime_smoke_tested
        || !matches!(manifest.runtime_profile.as_str(), "cuda" | "cpu")
    {
        return Err(anyhow!("installation manifest is incompatible"));
    }
    Ok(())
}

fn required_install_bytes(disk_reserve_mb: u32) -> u64 {
    LOCAL_TTS_INSTALL_ESTIMATE_BYTES.saturating_add(u64::from(disk_reserve_mb) * 1024 * 1024)
}

fn has_required_disk_space(available: u64, required: u64) -> bool {
    available >= required
}

#[derive(Clone)]
struct AggregateHfProgress {
    app_handle: AppHandle,
    status: Arc<RwLock<LocalTtsStatus>>,
    completed_before: u64,
    expected_file_size: u64,
}

impl AggregateHfProgress {
    fn emit_update(&self, current_file: u64) {
        let snapshot = {
            let mut status = self.status.write();
            status.phase = "downloading_model".to_string();
            status.installing = true;
            status.downloaded_bytes = self
                .completed_before
                .saturating_add(current_file.min(self.expected_file_size));
            status.total_bytes = LOCAL_TTS_MODEL_BYTES;
            status.percentage =
                status.downloaded_bytes as f64 / LOCAL_TTS_MODEL_BYTES as f64 * 100.0;
            status.clone()
        };
        let _ = self.app_handle.emit(LOCAL_TTS_EVENT_STATUS, snapshot);
    }
}

impl Progress for AggregateHfProgress {
    async fn init(&mut self, _size: usize, _filename: &str) {
        self.emit_update(0);
    }

    async fn update(&mut self, size: usize) {
        let current = {
            let status = self.status.read();
            status
                .downloaded_bytes
                .saturating_sub(self.completed_before)
        };
        self.emit_update(current.saturating_add(size as u64));
    }

    async fn finish(&mut self) {
        self.emit_update(self.expected_file_size);
    }
}

pub(crate) async fn download_resumable(
    client: &reqwest::Client,
    url: &str,
    final_path: &Path,
    cancel: &CancellationToken,
    progress: impl Fn(u64, u64),
) -> Result<()> {
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let partial_path = final_path.with_extension("zip.partial");
    let mut resume_from = partial_path
        .metadata()
        .map(|value| value.len())
        .unwrap_or(0);
    let mut request = client.get(url);
    if resume_from > 0 {
        request = request.header(RANGE, format!("bytes={resume_from}-"));
    }
    let mut response = send_download_request(request, cancel, "Runtime download failed").await?;
    if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
        let _ = fs::remove_file(&partial_path);
        resume_from = 0;
        response =
            send_download_request(client.get(url), cancel, "Runtime download restart failed")
                .await?;
    }
    if resume_from > 0 && response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        if unsatisfied_range_length(&response) == Some(resume_from) {
            progress(resume_from, resume_from);
            publish_managed_download(&partial_path, final_path)?;
            return Ok(());
        }
        fs::remove_file(&partial_path)
            .context("Failed to reset an incompatible partial runtime download")?;
        resume_from = 0;
        response =
            send_download_request(client.get(url), cancel, "Runtime download restart failed")
                .await?;
    }
    if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(anyhow!(
            "Runtime download failed with HTTP {}",
            response.status()
        ));
    }
    let total = resume_from.saturating_add(response.content_length().unwrap_or(0));
    let mut downloaded = resume_from;
    let mut file = OpenOptions::new()
        .create(true)
        .append(resume_from > 0)
        .write(true)
        .truncate(resume_from == 0)
        .open(&partial_path)?;
    let mut stream = response.bytes_stream();
    progress(downloaded, total);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Err(anyhow!("Local TTS installation cancelled")),
            chunk = stream.next() => match chunk {
                Some(Ok(chunk)) => {
                    file.write_all(&chunk)?;
                    downloaded = downloaded.saturating_add(chunk.len() as u64);
                    progress(downloaded, total);
                }
                Some(Err(error)) => return Err(error.into()),
                None => break,
            }
        }
    }
    file.flush()?;
    if total > 0 && downloaded != total {
        return Err(anyhow!(
            "Runtime download is incomplete: expected {total} bytes, received {downloaded}"
        ));
    }
    publish_managed_download(&partial_path, final_path)?;
    Ok(())
}

async fn send_download_request(
    request: reqwest::RequestBuilder,
    cancel: &CancellationToken,
    context: &'static str,
) -> Result<reqwest::Response> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(anyhow!("Local TTS installation cancelled")),
        response = request.send() => response.context(context),
    }
}

fn publish_managed_download(partial_path: &Path, final_path: &Path) -> Result<()> {
    match fs::remove_file(final_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to replace stale managed download {}",
                    final_path.display()
                )
            });
        }
    }
    fs::rename(partial_path, final_path).with_context(|| {
        format!(
            "Failed to publish managed download {}",
            final_path.display()
        )
    })
}

fn unsatisfied_range_length(response: &reqwest::Response) -> Option<u64> {
    let value = response.headers().get(CONTENT_RANGE)?.to_str().ok()?;
    let mut parts = value.split_whitespace();
    if !parts.next()?.eq_ignore_ascii_case("bytes") {
        return None;
    }
    let complete_length = parts.next()?.strip_prefix("*/")?;
    if parts.next().is_some() {
        return None;
    }
    complete_length.parse().ok()
}

pub(crate) fn extract_uv(archive_path: &Path, runtime_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for name in ["uv.exe", "uvx.exe"] {
        let mut entry = archive
            .by_name(name)
            .with_context(|| format!("Verified uv archive is missing {name}"))?;
        let destination = runtime_dir.join(name);
        let partial = destination.with_extension("exe.partial");
        let mut output = File::create(&partial)?;
        std::io::copy(&mut entry, &mut output)?;
        output.flush()?;
        fs::rename(partial, destination)?;
    }
    Ok(())
}

pub(crate) fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 8 * 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(anyhow!(
            "SHA-256 mismatch for {}: expected {expected}, received {actual}",
            path.display()
        ));
    }
    Ok(())
}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn notice_bundle_sha256() -> String {
    let mut hasher = Sha256::new();
    hasher.update(APACHE_LICENSE.as_bytes());
    hasher.update(UV_MIT_LICENSE.as_bytes());
    hasher.update(QWEN_NOTICE.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn write_owned_marker(root: &Path) -> Result<()> {
    fs::create_dir_all(root)?;
    let marker = root.join(".aivorelay-local-tts");
    if !marker.exists() {
        fs::write(marker, b"AivoRelay local TTS managed directory\n")?;
    }
    Ok(())
}

pub(crate) fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let partial = path.with_extension("json.partial");
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut file = File::create(&partial)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_all()?;
    }
    replace_file_atomic(&partial, path)?;
    Ok(())
}

#[cfg(windows)]
fn replace_file_atomic(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|_| std::io::Error::last_os_error())
}

#[cfg(not(windows))]
fn replace_file_atomic(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

fn detect_runtime_profile() -> String {
    let mut command = std::process::Command::new("nvidia-smi.exe");
    command.args(["--query-gpu=name", "--format=csv,noheader"]);
    hide_std_child_window(&mut command);
    match command.output() {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => "cuda".to_string(),
        _ => "cpu".to_string(),
    }
}

fn validate_worker_response(
    response: &WorkerResponse,
    request_id: &str,
) -> std::result::Result<(), LocalTtsAttemptError> {
    if response.protocol != WORKER_PROTOCOL_VERSION {
        return Err(permanent_local_error(
            "Local TTS worker protocol version does not match AivoRelay",
        ));
    }
    if response.id.as_deref() != Some(request_id) {
        return Err(permanent_local_error(
            "Local TTS worker returned a response for a different operation",
        ));
    }
    if !matches!(response.kind.as_str(), "result" | "error") {
        return Err(permanent_local_error(
            "Local TTS worker returned an unsupported response type",
        ));
    }
    Ok(())
}

pub(crate) fn read_worker_wav(path: &Path) -> std::result::Result<Vec<i16>, LocalTtsAttemptError> {
    let metadata = path.metadata().map_err(permanent_local_error)?;
    if metadata.len() == 0 || metadata.len() > MAX_WORKER_WAV_BYTES {
        return Err(permanent_local_error(
            "Local TTS worker returned an empty or unreasonably large WAV file",
        ));
    }
    let mut reader = hound::WavReader::open(path).map_err(permanent_local_error)?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != EXPECTED_SAMPLE_RATE
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err(permanent_local_error(
            "Local TTS worker returned WAV audio outside the required 24 kHz mono PCM16 format",
        ));
    }
    let samples: std::result::Result<Vec<i16>, _> = reader.samples::<i16>().collect();
    let samples = samples.map_err(permanent_local_error)?;
    if samples.is_empty() {
        return Err(permanent_local_error(
            "Local TTS worker returned no audio samples",
        ));
    }
    Ok(samples)
}

pub(crate) fn cancel_if_requested(cancel: &CancellationToken) -> Result<()> {
    if cancel.is_cancelled() {
        Err(anyhow!("Local TTS installation cancelled"))
    } else {
        Ok(())
    }
}

pub(crate) fn permanent_local_error(error: impl std::fmt::Display) -> LocalTtsAttemptError {
    LocalTtsAttemptError {
        safe_message: error.to_string(),
        transient: false,
    }
}

pub(crate) fn transient_local_error(message: String) -> LocalTtsAttemptError {
    LocalTtsAttemptError {
        safe_message: message,
        transient: true,
    }
}

#[cfg(windows)]
pub(crate) fn hide_child_window(command: &mut Command) {
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
pub(crate) fn hide_child_window(_command: &mut Command) {}

#[cfg(windows)]
fn hide_std_child_window(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn hide_std_child_window(_command: &mut std::process::Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_install_manifest() -> InstallManifest {
        InstallManifest {
            manifest_version: INSTALL_MANIFEST_VERSION,
            worker_protocol_version: WORKER_PROTOCOL_VERSION,
            model_repository: LOCAL_TTS_MODEL_REPO.to_string(),
            model_revision: LOCAL_TTS_MODEL_REVISION.to_string(),
            model_bytes: LOCAL_TTS_MODEL_BYTES,
            qwen_package: LOCAL_TTS_QWEN_PACKAGE.to_string(),
            python_version: LOCAL_TTS_PYTHON_VERSION.to_string(),
            uv_version: LOCAL_TTS_UV_VERSION.to_string(),
            worker_sha256: sha256_bytes(WORKER_SOURCE.as_bytes()),
            notice_bundle_sha256: notice_bundle_sha256(),
            runtime_smoke_tested: true,
            runtime_profile: "cpu".to_string(),
            resolved_packages: vec![LOCAL_TTS_QWEN_PACKAGE.to_string()],
        }
    }

    #[test]
    fn local_tts_status_identifies_qwen_kind() {
        let status = qwen_status(Path::new(r"C:\AivoRelay\qwen"));
        let value = serde_json::to_value(&status).unwrap();
        assert_eq!(value["kind"], "qwen");
        assert_eq!(
            status.estimated_install_bytes,
            LOCAL_TTS_INSTALL_ESTIMATE_BYTES
        );
        assert!(status.model_source_url.starts_with("https://"));
        assert!(status.model_license_url.starts_with("https://"));
        assert!(status
            .model_license_declaration_path
            .ends_with("Qwen3-TTS-UPSTREAM-MODEL-CARD.md"));
    }

    #[test]
    fn official_model_file_sizes_match_snapshot_total() {
        assert_eq!(
            MODEL_FILES.iter().map(|(_, size, _)| size).sum::<u64>(),
            LOCAL_TTS_MODEL_BYTES
        );
    }

    #[test]
    fn worker_protocol_rejects_wrong_request_id() {
        let response = WorkerResponse {
            kind: "result".to_string(),
            protocol: WORKER_PROTOCOL_VERSION,
            id: Some("other".to_string()),
            output_path: None,
            sample_rate: None,
            samples: None,
            message: None,
            retryable: None,
        };
        assert!(validate_worker_response(&response, "expected").is_err());
    }

    #[test]
    fn manifest_validation_rejects_wrong_revision() {
        let mut manifest = valid_install_manifest();
        validate_install_manifest(&manifest).expect("the pinned manifest should validate");

        manifest.model_revision = "different-revision".to_string();
        let error = validate_install_manifest(&manifest).expect_err("revision drift must fail");
        assert!(error.to_string().contains("incompatible"));
    }

    #[test]
    fn disk_reserve_is_included_in_local_install_preflight() {
        let without_reserve = required_install_bytes(0);
        let with_reserve = required_install_bytes(512);

        assert_eq!(without_reserve, 16_u64 * 1024 * 1024 * 1024);
        assert!(with_reserve > without_reserve);
        assert!(has_required_disk_space(with_reserve, with_reserve));
        assert!(!has_required_disk_space(with_reserve - 1, with_reserve));
    }

    #[test]
    fn managed_download_publish_replaces_a_stale_final_file() {
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-local-tts-download-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("create isolated download test directory");
        let partial = directory.join("runtime.zip.partial");
        let final_path = directory.join("runtime.zip");
        fs::write(&partial, b"new archive").expect("write completed partial archive");
        fs::write(&final_path, b"stale archive").expect("write stale final archive");

        publish_managed_download(&partial, &final_path).expect("publish managed download");

        assert_eq!(fs::read(&final_path).unwrap(), b"new archive");
        assert!(!partial.exists());
        fs::remove_dir_all(directory).expect("remove isolated download test directory");
    }

    #[test]
    fn atomic_json_write_replaces_an_existing_manifest() {
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-local-tts-manifest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("create isolated manifest test directory");
        let manifest = directory.join("install.json");
        write_json_atomic(&manifest, &serde_json::json!({ "version": 1 }))
            .expect("write initial manifest");

        write_json_atomic(&manifest, &serde_json::json!({ "version": 2 }))
            .expect("replace existing manifest");

        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        assert_eq!(value["version"], 2);
        assert!(!directory.join("install.json.partial").exists());
        fs::remove_dir_all(directory).expect("remove isolated manifest test directory");
    }

    #[test]
    fn worker_output_rejects_empty_malformed_and_wrong_format_wavs() {
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-local-tts-wav-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("create isolated WAV test directory");

        let empty = directory.join("empty.wav");
        fs::write(&empty, &[] as &[u8]).expect("write empty worker output");
        assert!(read_worker_wav(&empty).is_err());

        let malformed = directory.join("malformed.wav");
        fs::write(&malformed, b"not a RIFF/WAV file").expect("write malformed worker output");
        assert!(read_worker_wav(&malformed).is_err());

        let wrong_format = directory.join("wrong-format.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: EXPECTED_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wrong_format, spec).unwrap();
        writer.write_sample(0_i16).unwrap();
        writer.write_sample(0_i16).unwrap();
        writer.finalize().unwrap();
        let error = read_worker_wav(&wrong_format).expect_err("stereo output must be rejected");
        assert!(error.safe_message.contains("outside the required"));

        fs::remove_dir_all(directory).expect("remove isolated WAV test directory");
    }

    #[test]
    fn local_runtime_error_helpers_preserve_retry_classification() {
        let permanent = permanent_local_error("invalid worker response");
        assert!(!permanent.transient);
        assert_eq!(permanent.safe_message, "invalid worker response");

        let transient = transient_local_error("worker stopped".to_string());
        assert!(transient.transient);
        assert_eq!(transient.safe_message, "worker stopped");
    }
}
