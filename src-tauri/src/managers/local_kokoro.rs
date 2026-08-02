//! App-managed Kokoro-82M runtime backed by the official sherpa-onnx package.
//!
//! The native engine is isolated in a hidden, persistent Python worker. This
//! avoids loading sherpa's ONNX Runtime 1.27 into the AivoRelay process, which
//! already hosts a different ONNX Runtime for speech-to-text.

use super::local_tts::{
    cancel_if_requested, directory_size_bytes, download_resumable, extract_uv, hide_child_window,
    permanent_local_error, read_worker_wav, sha256_bytes, transient_local_error, verify_sha256,
    write_json_atomic, write_owned_marker, LocalTtsAttemptError, LocalTtsKind, LocalTtsStatus,
    LOCAL_TTS_EVENT_STATUS, LOCAL_TTS_PYTHON_VERSION, LOCAL_TTS_UV_VERSION, UV_WINDOWS_SHA256,
    UV_WINDOWS_URL, UV_WINDOWS_ZIP_NAME,
};
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use bzip2::read::BzDecoder;
use fs2::available_space;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio_util::sync::CancellationToken;

pub const KOKORO_PROVIDER_LIMIT: usize = 4_096;
pub const KOKORO_MODEL_REPOSITORY: &str =
    "k2-fsa/sherpa-onnx/tts-models/kokoro-int8-multi-lang-v1_1";
pub const KOKORO_MODEL_REVISION: &str =
    "tts-models:a1e94694776049035c4f2c6529f003aaece993c76aae9a78995831c3c4dcafc6";
pub const KOKORO_MODEL_DOWNLOAD_BYTES: u64 = 147_031_220;
pub const KOKORO_SHERPA_VERSION: &str = "1.13.4";
pub const KOKORO_PACKAGE: &str = "sherpa-onnx==1.13.4";
pub const KOKORO_LANGUAGES: &[&str] = &["English", "Chinese"];

const MODEL_ARCHIVE_NAME: &str = "kokoro-int8-multi-lang-v1_1.tar.bz2";
const MODEL_ARCHIVE_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_1.tar.bz2";
const MODEL_ARCHIVE_SHA256: &str =
    "a1e94694776049035c4f2c6529f003aaece993c76aae9a78995831c3c4dcafc6";
const MODEL_ARCHIVE_ROOT: &str = "kokoro-int8-multi-lang-v1_1";
const INSTALL_MANIFEST_VERSION: u32 = 1;
const WORKER_PROTOCOL_VERSION: u32 = 1;
const EXPECTED_SAMPLE_RATE: u32 = 24_000;
const EXPECTED_SPEAKERS: u32 = 103;
pub const KOKORO_INSTALL_ESTIMATE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const KOKORO_MODEL_SOURCE_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models";
pub const KOKORO_MODEL_LICENSE_URL: &str =
    "https://huggingface.co/csukuangfj/kokoro-int8-multi-lang-v1_1/blob/main/LICENSE";
const WORKER_SOURCE: &str = include_str!("local_kokoro_worker.py");
const APACHE_LICENSE: &str = include_str!("../../resources/licenses/Apache-2.0.txt");
const GPL3_LICENSE: &str = include_str!("../../resources/licenses/GPL-3.0.txt");
const UV_MIT_LICENSE: &str = include_str!("../../resources/licenses/uv-MIT.txt");
const KOKORO_NOTICE: &str = include_str!("../../resources/licenses/Kokoro-sherpa-NOTICE.txt");

const REQUIRED_MODEL_FILES: &[(&str, u64)] = &[
    ("model.int8.onnx", 114_299_010),
    ("voices.bin", 53_790_720),
    ("tokens.txt", 1_111),
    ("lexicon-us-en.txt", 5_956_885),
    ("lexicon-zh.txt", 2_119_465),
    ("number-zh.fst", 64_482),
    ("phone-zh.fst", 88_630),
    ("date-zh.fst", 59_154),
    ("LICENSE", 11_358),
];

/// Official sherpa speaker names and IDs for kokoro-multi-lang-v1_1.
pub const KOKORO_VOICES: &[(&str, i32)] = &[
    ("af_maple", 0),
    ("af_sol", 1),
    ("bf_vale", 2),
    ("zf_001", 3),
    ("zf_002", 4),
    ("zf_003", 5),
    ("zf_004", 6),
    ("zf_005", 7),
    ("zf_006", 8),
    ("zf_007", 9),
    ("zf_008", 10),
    ("zf_017", 11),
    ("zf_018", 12),
    ("zf_019", 13),
    ("zf_021", 14),
    ("zf_022", 15),
    ("zf_023", 16),
    ("zf_024", 17),
    ("zf_026", 18),
    ("zf_027", 19),
    ("zf_028", 20),
    ("zf_032", 21),
    ("zf_036", 22),
    ("zf_038", 23),
    ("zf_039", 24),
    ("zf_040", 25),
    ("zf_042", 26),
    ("zf_043", 27),
    ("zf_044", 28),
    ("zf_046", 29),
    ("zf_047", 30),
    ("zf_048", 31),
    ("zf_049", 32),
    ("zf_051", 33),
    ("zf_059", 34),
    ("zf_060", 35),
    ("zf_067", 36),
    ("zf_070", 37),
    ("zf_071", 38),
    ("zf_072", 39),
    ("zf_073", 40),
    ("zf_074", 41),
    ("zf_075", 42),
    ("zf_076", 43),
    ("zf_077", 44),
    ("zf_078", 45),
    ("zf_079", 46),
    ("zf_083", 47),
    ("zf_084", 48),
    ("zf_085", 49),
    ("zf_086", 50),
    ("zf_087", 51),
    ("zf_088", 52),
    ("zf_090", 53),
    ("zf_092", 54),
    ("zf_093", 55),
    ("zf_094", 56),
    ("zf_099", 57),
    ("zm_009", 58),
    ("zm_010", 59),
    ("zm_011", 60),
    ("zm_012", 61),
    ("zm_013", 62),
    ("zm_014", 63),
    ("zm_015", 64),
    ("zm_016", 65),
    ("zm_020", 66),
    ("zm_025", 67),
    ("zm_029", 68),
    ("zm_030", 69),
    ("zm_031", 70),
    ("zm_033", 71),
    ("zm_034", 72),
    ("zm_035", 73),
    ("zm_037", 74),
    ("zm_041", 75),
    ("zm_045", 76),
    ("zm_050", 77),
    ("zm_052", 78),
    ("zm_053", 79),
    ("zm_054", 80),
    ("zm_055", 81),
    ("zm_056", 82),
    ("zm_057", 83),
    ("zm_058", 84),
    ("zm_061", 85),
    ("zm_062", 86),
    ("zm_063", 87),
    ("zm_064", 88),
    ("zm_065", 89),
    ("zm_066", 90),
    ("zm_068", 91),
    ("zm_069", 92),
    ("zm_080", 93),
    ("zm_081", 94),
    ("zm_082", 95),
    ("zm_089", 96),
    ("zm_091", 97),
    ("zm_095", 98),
    ("zm_096", 99),
    ("zm_097", 100),
    ("zm_098", 101),
    ("zm_100", 102),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KokoroInstallManifest {
    manifest_version: u32,
    worker_protocol_version: u32,
    model_repository: String,
    model_revision: String,
    model_archive_sha256: String,
    model_download_bytes: u64,
    sherpa_package: String,
    python_version: String,
    uv_version: String,
    worker_sha256: String,
    notice_bundle_sha256: String,
    runtime_smoke_tested: bool,
    resolved_packages: Vec<String>,
}

struct KokoroWorker {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
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
    num_speakers: Option<u32>,
    runtime_version: Option<String>,
}

pub struct KokoroTtsRuntime {
    app_handle: AppHandle,
    root: PathBuf,
    client: reqwest::Client,
    status: Arc<RwLock<LocalTtsStatus>>,
    lifecycle: tokio::sync::Mutex<()>,
    install_cancel: Mutex<Option<CancellationToken>>,
    worker: tokio::sync::Mutex<Option<KokoroWorker>>,
    request_id: AtomicU64,
}

impl KokoroTtsRuntime {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let root = crate::portable::app_data_dir(app_handle)
            .map_err(|error| anyhow!("Failed to resolve app data directory: {error}"))?
            .join("local-tts")
            .join("kokoro-82m-v1.1-int8");
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(30 * 60))
            .build()
            .context("Failed to build Kokoro installer HTTP client")?;
        let runtime = Self {
            app_handle: app_handle.clone(),
            status: Arc::new(RwLock::new(kokoro_status(&root))),
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
            .map_err(|_| anyhow!("Another Kokoro install or delete operation is running"))?;
        if self.is_installed() {
            return Ok(self.status());
        }
        fs::create_dir_all(&self.root).context("Failed to create Kokoro directory")?;
        write_owned_marker(&self.root)?;
        self.preflight_disk_space(disk_reserve_mb)?;
        let cancel = CancellationToken::new();
        *self.install_cancel.lock() = Some(cancel.clone());
        self.set_status("preparing", true, 0, KOKORO_MODEL_DOWNLOAD_BYTES, None);
        let result = self.install_inner(&cancel).await;
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
                    KOKORO_MODEL_DOWNLOAD_BYTES,
                    None,
                );
                Err(anyhow!("Kokoro installation cancelled"))
            }
            Err(error) => {
                self.set_status(
                    "error",
                    false,
                    self.model_downloaded_bytes(),
                    KOKORO_MODEL_DOWNLOAD_BYTES,
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
            .map_err(|_| anyhow!("Another Kokoro install or delete operation is running"))?;
        if self.install_cancel.lock().is_some() {
            return Err(anyhow!(
                "Cancel the active Kokoro installation before deleting it"
            ));
        }
        self.stop_worker().await;
        if self.root.exists() {
            if !self.root.join(".aivorelay-local-tts").is_file() {
                return Err(anyhow!(
                    "Refusing to delete a directory without an AivoRelay ownership marker"
                ));
            }
            fs::remove_dir_all(&self.root).context("Failed to delete Kokoro files")?;
        }
        self.set_status("not_installed", false, 0, KOKORO_MODEL_DOWNLOAD_BYTES, None);
        Ok(())
    }

    pub async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        language: &str,
        speed: f32,
    ) -> std::result::Result<Vec<i16>, LocalTtsAttemptError> {
        let sid = validate_voice_language(voice, language)?;
        self.installation_manifest()
            .map_err(|error| LocalTtsAttemptError {
                safe_message: format!(
                    "Local Kokoro is not installed or needs repair: {error}. Install it in Text to Speech settings."
                ),
                transient: false,
            })?;
        let mut guard = self.worker.lock().await;
        if guard.is_none() {
            *guard = Some(self.start_worker(None).await?);
        }
        let id = self.request_id.fetch_add(1, Ordering::Relaxed).to_string();
        let worker = guard.as_mut().expect("Kokoro worker initialized");
        match self
            .synthesize_with_worker(worker, &id, text, sid, speed, None)
            .await
        {
            Ok(samples) => Ok(samples),
            Err(error) => {
                if error.transient {
                    let _ = worker.child.kill().await;
                    *guard = None;
                }
                Err(error)
            }
        }
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
        self.install_uv(cancel).await?;
        self.install_python_runtime(cancel).await?;
        self.download_and_extract_model(cancel).await?;
        cancel_if_requested(cancel)?;
        fs::write(self.worker_path(), WORKER_SOURCE).context("Failed to install Kokoro worker")?;
        self.write_managed_notices()?;
        let resolved_packages = self.freeze_packages(cancel).await?;
        self.set_status("validating_runtime", true, 0, 0, None);
        let mut worker = self
            .start_worker(Some(cancel))
            .await
            .map_err(|error| anyhow!("Kokoro runtime validation failed: {}", error.safe_message))?;
        let smoke_id = "install-smoke";
        let smoke = self
            .synthesize_with_worker(
                &mut worker,
                smoke_id,
                "AivoRelay Kokoro is ready.",
                0,
                1.0,
                Some(cancel),
            )
            .await
            .map_err(|error| {
                anyhow!("Kokoro synthesis smoke test failed: {}", error.safe_message)
            })?;
        if smoke.len() < 2_400 {
            let _ = worker.child.kill().await;
            return Err(anyhow!(
                "Kokoro synthesis smoke test returned too little audio"
            ));
        }
        if let Err(error) = cancel_if_requested(cancel) {
            let _ = worker.child.kill().await;
            return Err(error);
        }
        let manifest = KokoroInstallManifest {
            manifest_version: INSTALL_MANIFEST_VERSION,
            worker_protocol_version: WORKER_PROTOCOL_VERSION,
            model_repository: KOKORO_MODEL_REPOSITORY.to_string(),
            model_revision: KOKORO_MODEL_REVISION.to_string(),
            model_archive_sha256: MODEL_ARCHIVE_SHA256.to_string(),
            model_download_bytes: KOKORO_MODEL_DOWNLOAD_BYTES,
            sherpa_package: KOKORO_PACKAGE.to_string(),
            python_version: LOCAL_TTS_PYTHON_VERSION.to_string(),
            uv_version: LOCAL_TTS_UV_VERSION.to_string(),
            worker_sha256: sha256_bytes(WORKER_SOURCE.as_bytes()),
            notice_bundle_sha256: notice_bundle_sha256(),
            runtime_smoke_tested: true,
            resolved_packages,
        };
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
        let archive = self.runtime_dir().join(UV_WINDOWS_ZIP_NAME);
        download_resumable(
            &self.client,
            UV_WINDOWS_URL,
            &archive,
            cancel,
            |downloaded, total| {
                self.set_status("downloading_runtime", true, downloaded, total, None)
            },
        )
        .await?;
        verify_sha256(&archive, UV_WINDOWS_SHA256)?;
        let archive_copy = archive.clone();
        let runtime_dir = self.runtime_dir();
        tokio::task::spawn_blocking(move || extract_uv(&archive_copy, &runtime_dir))
            .await
            .map_err(|error| anyhow!("uv extraction task failed: {error}"))??;
        if !self.uv_path().is_file() {
            return Err(anyhow!("The verified uv archive did not contain uv.exe"));
        }
        let _ = fs::remove_file(archive);
        Ok(())
    }

    async fn install_python_runtime(&self, cancel: &CancellationToken) -> Result<()> {
        self.set_status("installing_python", true, 0, 0, None);
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
        self.set_status("installing_sherpa", true, 0, 0, None);
        self.run_uv(
            &[
                "pip",
                "install",
                "--python",
                self.venv_python_path().to_string_lossy().as_ref(),
                KOKORO_PACKAGE,
            ],
            cancel,
        )
        .await
    }

    async fn download_and_extract_model(&self, cancel: &CancellationToken) -> Result<()> {
        if validate_model_dir(&self.model_dir()).is_ok() {
            return Ok(());
        }
        self.set_status(
            "downloading_model",
            true,
            self.model_downloaded_bytes(),
            KOKORO_MODEL_DOWNLOAD_BYTES,
            None,
        );
        fs::create_dir_all(self.download_dir())?;
        let archive = self.download_dir().join(MODEL_ARCHIVE_NAME);
        download_resumable(
            &self.client,
            MODEL_ARCHIVE_URL,
            &archive,
            cancel,
            |downloaded, _total| {
                self.set_status(
                    "downloading_model",
                    true,
                    downloaded.min(KOKORO_MODEL_DOWNLOAD_BYTES),
                    KOKORO_MODEL_DOWNLOAD_BYTES,
                    None,
                )
            },
        )
        .await?;
        verify_sha256(&archive, MODEL_ARCHIVE_SHA256)
            .context("Kokoro model archive integrity check failed")?;
        cancel_if_requested(cancel)?;
        self.set_status(
            "extracting_model",
            true,
            KOKORO_MODEL_DOWNLOAD_BYTES,
            KOKORO_MODEL_DOWNLOAD_BYTES,
            None,
        );
        let staging = self.root.join("model.extracting");
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        let archive_copy = archive.clone();
        let staging_copy = staging.clone();
        tokio::task::spawn_blocking(move || {
            extract_tar_bz2(&archive_copy, &staging_copy, MODEL_ARCHIVE_ROOT)
        })
        .await
        .map_err(|error| anyhow!("Kokoro extraction task failed: {error}"))??;
        cancel_if_requested(cancel)?;
        let extracted = staging.join(MODEL_ARCHIVE_ROOT);
        validate_model_dir(&extracted)?;
        if self.model_dir().exists() {
            fs::remove_dir_all(self.model_dir())?;
        }
        fs::rename(&extracted, self.model_dir())?;
        let _ = fs::remove_dir_all(staging);
        let _ = fs::remove_file(archive);
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
            .env_remove("UV_INDEX_URL")
            .env_remove("UV_DEFAULT_INDEX")
            .env_remove("UV_EXTRA_INDEX_URL")
            .env_remove("UV_FIND_LINKS")
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
            _ = cancel.cancelled() => return Err(anyhow!("Kokoro installation cancelled")),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow!(
                "Managed Kokoro runtime installation failed (exit {}): {}{}",
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
        cancel: Option<&CancellationToken>,
    ) -> std::result::Result<KokoroWorker, LocalTtsAttemptError> {
        validate_model_dir(&self.model_dir()).map_err(permanent_local_error)?;
        fs::create_dir_all(self.output_dir()).map_err(permanent_local_error)?;
        let threads = std::thread::available_parallelism()
            .map(|value| value.get().clamp(1, 4))
            .unwrap_or(2);
        let mut command = Command::new(self.venv_python_path());
        command
            .arg("-I")
            .arg(self.worker_path())
            .arg("--model-root")
            .arg(self.model_dir())
            .arg("--output-root")
            .arg(self.output_dir())
            .arg("--threads")
            .arg(threads.to_string())
            .current_dir(self.runtime_dir())
            .env("HF_HUB_OFFLINE", "1")
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH")
            .env_remove("VIRTUAL_ENV")
            .env_remove("CONDA_PREFIX")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        hide_child_window(&mut command);
        let mut child = command.spawn().map_err(|error| LocalTtsAttemptError {
            safe_message: format!("Failed to start Kokoro worker: {error}"),
            transient: true,
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            transient_local_error("Kokoro worker stdin was not available".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            transient_local_error("Kokoro worker stdout was not available".to_string())
        })?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(_line)) = lines.next_line().await {}
            });
        }
        let mut worker = KokoroWorker {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
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
            return Err(permanent_local_error("Kokoro installation cancelled"));
        };
        let ready_line = ready_result
            .map_err(|_| transient_local_error("Timed out while loading Kokoro".to_string()))?
            .map_err(|error| {
                transient_local_error(format!("Failed to read Kokoro startup response: {error}"))
            })?
            .ok_or_else(|| {
                transient_local_error("Kokoro worker exited while loading the model".to_string())
            })?;
        let response: WorkerResponse =
            serde_json::from_str(&ready_line).map_err(|error| LocalTtsAttemptError {
                safe_message: format!("Kokoro startup protocol is malformed: {error}"),
                transient: false,
            })?;
        if response.protocol != WORKER_PROTOCOL_VERSION
            || response.kind != "ready"
            || response.sample_rate != Some(EXPECTED_SAMPLE_RATE)
            || response.num_speakers != Some(EXPECTED_SPEAKERS)
            || response.runtime_version.as_deref() != Some(KOKORO_SHERPA_VERSION)
        {
            let _ = worker.child.kill().await;
            return Err(LocalTtsAttemptError {
                safe_message: response
                    .message
                    .unwrap_or_else(|| "Kokoro worker reported incompatible metadata".to_string()),
                transient: false,
            });
        }
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            let _ = worker.child.kill().await;
            return Err(permanent_local_error("Kokoro installation cancelled"));
        }
        Ok(worker)
    }

    async fn synthesize_with_worker(
        &self,
        worker: &mut KokoroWorker,
        id: &str,
        text: &str,
        sid: i32,
        speed: f32,
        cancel: Option<&CancellationToken>,
    ) -> std::result::Result<Vec<i16>, LocalTtsAttemptError> {
        let output_path = self.output_dir().join(format!("request-{id}.wav"));
        let _ = fs::remove_file(&output_path);
        let request = serde_json::json!({
            "type": "synthesize",
            "protocol": WORKER_PROTOCOL_VERSION,
            "id": id,
            "text_b64": base64::engine::general_purpose::STANDARD.encode(text.as_bytes()),
            "sid": sid,
            "speed": speed.clamp(0.5, 2.0),
            "output_path": output_path,
        });
        let serialized = serde_json::to_string(&request).map_err(permanent_local_error)?;
        let exchange = async {
            worker
                .stdin
                .write_all(format!("{serialized}\n").as_bytes())
                .await
                .map_err(|error| {
                    transient_local_error(format!(
                        "Kokoro worker stopped accepting requests: {error}"
                    ))
                })?;
            worker.stdin.flush().await.map_err(|error| {
                transient_local_error(format!("Failed to flush Kokoro request: {error}"))
            })?;
            tokio::time::timeout(Duration::from_secs(10 * 60), worker.stdout.next_line())
                .await
                .map_err(|_| transient_local_error("Kokoro synthesis timed out".to_string()))?
                .map_err(|error| {
                    transient_local_error(format!("Failed to read Kokoro response: {error}"))
                })?
                .ok_or_else(|| {
                    transient_local_error("Kokoro worker exited during synthesis".to_string())
                })
        };
        let exchange_result = if let Some(cancel) = cancel {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => None,
                result = exchange => Some(result),
            }
        } else {
            Some(exchange.await)
        };
        let Some(exchange_result) = exchange_result else {
            let _ = worker.child.kill().await;
            let _ = fs::remove_file(&output_path);
            return Err(permanent_local_error("Kokoro installation cancelled"));
        };
        let line = exchange_result?;
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            let _ = worker.child.kill().await;
            let _ = fs::remove_file(&output_path);
            return Err(permanent_local_error("Kokoro installation cancelled"));
        }
        let response: WorkerResponse =
            serde_json::from_str(&line).map_err(|error| LocalTtsAttemptError {
                safe_message: format!("Kokoro response protocol is malformed: {error}"),
                transient: false,
            })?;
        if response.protocol != WORKER_PROTOCOL_VERSION
            || response.id.as_deref() != Some(id)
            || !matches!(response.kind.as_str(), "result" | "error")
        {
            return Err(permanent_local_error(
                "Kokoro worker returned a mismatched response",
            ));
        }
        if response.kind == "error" {
            return Err(LocalTtsAttemptError {
                safe_message: response
                    .message
                    .unwrap_or_else(|| "Kokoro synthesis failed".to_string()),
                transient: response.retryable.unwrap_or(false),
            });
        }
        if response.sample_rate != Some(EXPECTED_SAMPLE_RATE)
            || response.output_path.as_deref() != Some(output_path.to_string_lossy().as_ref())
        {
            return Err(permanent_local_error(
                "Kokoro worker returned invalid audio metadata",
            ));
        }
        let expected_samples = response
            .samples
            .filter(|value| *value > 0)
            .ok_or_else(|| permanent_local_error("Kokoro worker omitted audio sample metadata"))?;
        let parsed = read_worker_wav(&output_path).and_then(|samples| {
            if samples.len() as u64 != expected_samples {
                return Err(permanent_local_error(
                    "Kokoro audio length does not match its response metadata",
                ));
            }
            Ok(samples)
        });
        let _ = fs::remove_file(output_path);
        parsed
    }

    fn installation_manifest(&self) -> Result<KokoroInstallManifest> {
        let content =
            fs::read_to_string(self.manifest_path()).context("installation manifest is missing")?;
        let manifest: KokoroInstallManifest =
            serde_json::from_str(&content).context("installation manifest is invalid")?;
        validate_install_manifest(&manifest)?;
        if !self.uv_path().is_file()
            || !self.venv_python_path().is_file()
            || !self.worker_path().is_file()
        {
            return Err(anyhow!("installed Kokoro runtime files are incomplete"));
        }
        verify_sha256(&self.worker_path(), &sha256_bytes(WORKER_SOURCE.as_bytes()))
            .context("installed Kokoro worker failed integrity verification")?;
        validate_model_dir(&self.model_dir())?;
        if !self.managed_notices_valid() {
            return Err(anyhow!("installed Kokoro notices are incomplete"));
        }
        Ok(manifest)
    }

    fn preflight_disk_space(&self, disk_reserve_mb: u32) -> Result<()> {
        let target = self
            .root
            .parent()
            .ok_or_else(|| anyhow!("Kokoro install root is invalid"))?;
        fs::create_dir_all(target)?;
        let available = available_space(target)?;
        let required = required_install_bytes(disk_reserve_mb);
        if available < required {
            return Err(anyhow!(
                "Not enough disk space for Kokoro: need at least {:.2} GiB including the configured reserve, but only {:.2} GiB is available",
                required as f64 / 1024_f64.powi(3),
                available as f64 / 1024_f64.powi(3)
            ));
        }
        Ok(())
    }

    fn refresh_status(&self) {
        if self.install_cancel.lock().is_some() {
            return;
        }
        match self.installation_manifest() {
            Ok(_) => {
                let mut status = self.status.write();
                status.installed = true;
                status.installing = false;
                status.phase = "ready".to_string();
                status.downloaded_bytes = KOKORO_MODEL_DOWNLOAD_BYTES;
                status.total_bytes = KOKORO_MODEL_DOWNLOAD_BYTES;
                status.percentage = 100.0;
                status.runtime_profile = "cpu".to_string();
                if status.installed_size_bytes == 0 {
                    status.installed_size_bytes = directory_size_bytes(&self.root);
                }
                status.model_license_available = self.model_license_path().is_file();
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
                    status.error = self.manifest_path().exists().then(|| error.to_string());
                }
                status.downloaded_bytes = partial;
                status.total_bytes = KOKORO_MODEL_DOWNLOAD_BYTES;
                status.percentage = partial as f64 / KOKORO_MODEL_DOWNLOAD_BYTES as f64 * 100.0;
                status.runtime_profile.clear();
                status.installed_size_bytes = directory_size_bytes(&self.root);
                status.model_license_available = self.model_license_path().is_file();
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
            status.installed = false;
            status.installing = installing;
            status.phase = phase.to_string();
            status.downloaded_bytes = downloaded;
            status.total_bytes = total;
            status.percentage = if total == 0 {
                0.0
            } else {
                downloaded as f64 / total as f64 * 100.0
            }
            .clamp(0.0, 100.0);
            status.error = error;
            status.clone()
        };
        self.emit_status(&snapshot);
    }

    fn emit_status(&self, status: &LocalTtsStatus) {
        let _ = self.app_handle.emit(LOCAL_TTS_EVENT_STATUS, status);
    }

    fn model_downloaded_bytes(&self) -> u64 {
        let final_path = self.download_dir().join(MODEL_ARCHIVE_NAME);
        if final_path.is_file() {
            return final_path
                .metadata()
                .map(|value| value.len())
                .unwrap_or(0)
                .min(KOKORO_MODEL_DOWNLOAD_BYTES);
        }
        let partial = final_path.with_extension("zip.partial");
        partial
            .metadata()
            .map(|value| value.len())
            .unwrap_or(0)
            .min(KOKORO_MODEL_DOWNLOAD_BYTES)
    }

    fn runtime_dir(&self) -> PathBuf {
        self.root.join("runtime")
    }

    fn download_dir(&self) -> PathBuf {
        self.root.join("downloads")
    }

    fn model_dir(&self) -> PathBuf {
        self.root.join("model")
    }

    fn output_dir(&self) -> PathBuf {
        self.root.join("output")
    }

    fn uv_path(&self) -> PathBuf {
        self.runtime_dir().join("uv.exe")
    }

    fn venv_dir(&self) -> PathBuf {
        self.runtime_dir().join("venv")
    }

    fn venv_python_path(&self) -> PathBuf {
        #[cfg(windows)]
        {
            self.venv_dir().join("Scripts").join("python.exe")
        }
        #[cfg(not(windows))]
        {
            self.venv_dir().join("bin").join("python")
        }
    }

    fn worker_path(&self) -> PathBuf {
        self.runtime_dir().join("aivorelay_kokoro_worker.py")
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("install-manifest.json")
    }

    fn model_license_path(&self) -> PathBuf {
        self.model_dir().join("LICENSE")
    }

    fn write_managed_notices(&self) -> Result<()> {
        let dir = self.root.join("licenses");
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("Apache-2.0.txt"), APACHE_LICENSE)?;
        fs::write(dir.join("GPL-3.0.txt"), GPL3_LICENSE)?;
        fs::write(dir.join("uv-MIT.txt"), UV_MIT_LICENSE)?;
        fs::write(dir.join("Kokoro-sherpa-NOTICE.txt"), KOKORO_NOTICE)?;
        Ok(())
    }

    fn managed_notices_valid(&self) -> bool {
        let dir = self.root.join("licenses");
        [
            ("Apache-2.0.txt", APACHE_LICENSE),
            ("GPL-3.0.txt", GPL3_LICENSE),
            ("uv-MIT.txt", UV_MIT_LICENSE),
            ("Kokoro-sherpa-NOTICE.txt", KOKORO_NOTICE),
        ]
        .into_iter()
        .all(|(name, expected)| {
            fs::read(dir.join(name))
                .map(|actual| actual == expected.as_bytes())
                .unwrap_or(false)
        })
    }
}

fn kokoro_status(root: &Path) -> LocalTtsStatus {
    LocalTtsStatus {
        kind: LocalTtsKind::Kokoro,
        installed: false,
        installing: false,
        phase: "not_installed".to_string(),
        downloaded_bytes: 0,
        total_bytes: KOKORO_MODEL_DOWNLOAD_BYTES,
        percentage: 0.0,
        runtime_profile: String::new(),
        model_repository: KOKORO_MODEL_REPOSITORY.to_string(),
        model_revision: KOKORO_MODEL_REVISION.to_string(),
        model_download_bytes: KOKORO_MODEL_DOWNLOAD_BYTES,
        install_root: root.to_string_lossy().to_string(),
        installed_size_bytes: 0,
        estimated_install_bytes: KOKORO_INSTALL_ESTIMATE_BYTES,
        model_author: "k2-fsa (sherpa-onnx), based on hexgrad Kokoro-82M".to_string(),
        model_source_url: KOKORO_MODEL_SOURCE_URL.to_string(),
        model_license_name: "Apache License 2.0".to_string(),
        model_license_url: KOKORO_MODEL_LICENSE_URL.to_string(),
        model_license_path: root
            .join("model")
            .join("LICENSE")
            .to_string_lossy()
            .to_string(),
        model_license_declaration_path: root
            .join("model")
            .join("LICENSE")
            .to_string_lossy()
            .to_string(),
        model_license_available: false,
        error: None,
    }
}

fn validate_voice_language(
    voice: &str,
    language: &str,
) -> std::result::Result<i32, LocalTtsAttemptError> {
    if !KOKORO_LANGUAGES.contains(&language) {
        return Err(permanent_local_error(format!(
            "Unsupported Kokoro language: {language}. Use English or Chinese."
        )));
    }
    let sid = KOKORO_VOICES
        .iter()
        .find_map(|(name, sid)| (*name == voice).then_some(*sid))
        .ok_or_else(|| permanent_local_error(format!("Unsupported Kokoro voice: {voice}")))?;
    if language == "English" && sid > 2 {
        return Err(permanent_local_error(format!(
            "Kokoro voice {voice} is a Chinese speaker. Select af_maple, af_sol, or bf_vale for English."
        )));
    }
    if language == "Chinese" && sid < 3 {
        return Err(permanent_local_error(format!(
            "Kokoro voice {voice} is an English speaker. Select a zf_* or zm_* voice for Chinese."
        )));
    }
    Ok(sid)
}

fn validate_model_dir(root: &Path) -> Result<()> {
    for &(name, expected_size) in REQUIRED_MODEL_FILES {
        let path = root.join(name);
        let actual = path.metadata().map(|value| value.len()).unwrap_or(0);
        if actual != expected_size {
            return Err(anyhow!(
                "Kokoro model file is missing or incomplete: {name}"
            ));
        }
    }
    if !root.join("espeak-ng-data").is_dir() {
        return Err(anyhow!("Kokoro espeak-ng-data directory is missing"));
    }
    Ok(())
}

fn validate_install_manifest(manifest: &KokoroInstallManifest) -> Result<()> {
    if manifest.manifest_version != INSTALL_MANIFEST_VERSION
        || manifest.worker_protocol_version != WORKER_PROTOCOL_VERSION
        || manifest.model_repository != KOKORO_MODEL_REPOSITORY
        || manifest.model_revision != KOKORO_MODEL_REVISION
        || manifest.model_archive_sha256 != MODEL_ARCHIVE_SHA256
        || manifest.model_download_bytes != KOKORO_MODEL_DOWNLOAD_BYTES
        || manifest.sherpa_package != KOKORO_PACKAGE
        || manifest.python_version != LOCAL_TTS_PYTHON_VERSION
        || manifest.uv_version != LOCAL_TTS_UV_VERSION
        || manifest.worker_sha256 != sha256_bytes(WORKER_SOURCE.as_bytes())
        || manifest.notice_bundle_sha256 != notice_bundle_sha256()
        || !manifest.runtime_smoke_tested
    {
        return Err(anyhow!("installation manifest is incompatible"));
    }
    Ok(())
}

fn required_install_bytes(disk_reserve_mb: u32) -> u64 {
    KOKORO_INSTALL_ESTIMATE_BYTES.saturating_add(u64::from(disk_reserve_mb) * 1024 * 1024)
}

fn extract_tar_bz2(archive_path: &Path, destination: &Path, expected_root: &str) -> Result<()> {
    let file = File::open(archive_path)?;
    let decoder = BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(anyhow!("Kokoro archive contains an unsafe path"));
        }
        if path
            .components()
            .next()
            .and_then(|value| value.as_os_str().to_str())
            != Some(expected_root)
        {
            return Err(anyhow!("Kokoro archive contains an unexpected root"));
        }
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            return Err(anyhow!(
                "Kokoro archive contains an unsupported link or device entry"
            ));
        }
        if !entry.unpack_in(destination)? {
            return Err(anyhow!("Kokoro archive entry escaped its destination"));
        }
    }
    Ok(())
}

fn notice_bundle_sha256() -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(APACHE_LICENSE.as_bytes());
    bytes.extend_from_slice(GPL3_LICENSE.as_bytes());
    bytes.extend_from_slice(UV_MIT_LICENSE.as_bytes());
    bytes.extend_from_slice(KOKORO_NOTICE.as_bytes());
    sha256_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bzip2::{write::BzEncoder, Compression};
    use std::io::Cursor;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "aivorelay-kokoro-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create Kokoro test directory");
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_test_archive(
        path: &Path,
        entries: impl FnOnce(&mut tar::Builder<BzEncoder<File>>) -> Result<()>,
    ) -> Result<()> {
        let encoder = BzEncoder::new(File::create(path)?, Compression::best());
        let mut archive = tar::Builder::new(encoder);
        entries(&mut archive)?;
        let encoder = archive.into_inner()?;
        encoder.finish()?;
        Ok(())
    }

    fn valid_install_manifest() -> KokoroInstallManifest {
        KokoroInstallManifest {
            manifest_version: INSTALL_MANIFEST_VERSION,
            worker_protocol_version: WORKER_PROTOCOL_VERSION,
            model_repository: KOKORO_MODEL_REPOSITORY.to_string(),
            model_revision: KOKORO_MODEL_REVISION.to_string(),
            model_archive_sha256: MODEL_ARCHIVE_SHA256.to_string(),
            model_download_bytes: KOKORO_MODEL_DOWNLOAD_BYTES,
            sherpa_package: KOKORO_PACKAGE.to_string(),
            python_version: LOCAL_TTS_PYTHON_VERSION.to_string(),
            uv_version: LOCAL_TTS_UV_VERSION.to_string(),
            worker_sha256: sha256_bytes(WORKER_SOURCE.as_bytes()),
            notice_bundle_sha256: notice_bundle_sha256(),
            runtime_smoke_tested: true,
            resolved_packages: vec![KOKORO_PACKAGE.to_string()],
        }
    }

    #[test]
    fn official_voice_map_is_complete_and_unique() {
        assert_eq!(KOKORO_VOICES.len(), EXPECTED_SPEAKERS as usize);
        let names = KOKORO_VOICES
            .iter()
            .map(|(name, _)| *name)
            .collect::<std::collections::HashSet<_>>();
        let ids = KOKORO_VOICES
            .iter()
            .map(|(_, id)| *id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(names.len(), KOKORO_VOICES.len());
        assert_eq!(ids.len(), KOKORO_VOICES.len());
        assert_eq!(ids.iter().copied().min(), Some(0));
        assert_eq!(ids.iter().copied().max(), Some(102));
    }

    #[test]
    fn language_and_voice_pairs_are_rejected_before_inference() {
        assert_eq!(validate_voice_language("af_maple", "English").unwrap(), 0);
        assert_eq!(validate_voice_language("zf_001", "Chinese").unwrap(), 3);
        assert!(validate_voice_language("zf_001", "English").is_err());
        assert!(validate_voice_language("af_maple", "Chinese").is_err());
        assert!(validate_voice_language("af_maple", "Russian").is_err());
    }

    #[test]
    fn status_identifies_kokoro_kind_and_pinned_asset() {
        let status = kokoro_status(Path::new(r"C:\AivoRelay\kokoro"));
        assert_eq!(status.kind, LocalTtsKind::Kokoro);
        assert_eq!(status.model_download_bytes, KOKORO_MODEL_DOWNLOAD_BYTES);
        assert!(status.model_revision.contains(MODEL_ARCHIVE_SHA256));
        assert_eq!(
            status.estimated_install_bytes,
            KOKORO_INSTALL_ESTIMATE_BYTES
        );
        assert!(status.model_license_url.starts_with("https://"));
        assert!(status.model_license_path.ends_with("model\\LICENSE"));
    }

    #[test]
    fn directory_size_ignores_directories_and_counts_nested_files() {
        let temp = TestDir::new("directory-size");
        let nested = temp.0.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(temp.0.join("one.bin"), [1_u8, 2, 3]).unwrap();
        fs::write(nested.join("two.bin"), [4_u8, 5]).unwrap();

        assert_eq!(directory_size_bytes(&temp.0), 5);
    }

    #[test]
    fn directory_size_counts_large_hard_linked_content_once() {
        let temp = TestDir::new("directory-size-hard-link");
        let original = temp.0.join("original.bin");
        fs::write(
            &original,
            vec![1_u8; crate::managers::local_tts::HARD_LINK_DEDUPLICATION_MIN_BYTES as usize],
        )
        .unwrap();
        fs::hard_link(&original, temp.0.join("linked.bin")).unwrap();

        assert_eq!(
            directory_size_bytes(&temp.0),
            crate::managers::local_tts::HARD_LINK_DEDUPLICATION_MIN_BYTES
        );
    }

    #[test]
    fn manifest_validation_rejects_wrong_archive_hash() {
        let mut manifest = valid_install_manifest();
        validate_install_manifest(&manifest).expect("the pinned manifest should validate");

        manifest.model_archive_sha256 = "different-hash".to_string();
        let error = validate_install_manifest(&manifest).expect_err("hash drift must fail");
        assert!(error.to_string().contains("incompatible"));
    }

    #[test]
    fn disk_reserve_is_included_in_kokoro_install_preflight() {
        let without_reserve = required_install_bytes(0);
        let with_reserve = required_install_bytes(512);

        assert_eq!(with_reserve, without_reserve + 512_u64 * 1024 * 1024);
    }

    #[test]
    fn safe_extractor_accepts_regular_files_and_rejects_traversal_and_links() -> Result<()> {
        let temp = TestDir::new("safe-extraction");
        let expected_root = "kokoro-test";

        let valid_archive = temp.0.join("valid.tar.bz2");
        write_test_archive(&valid_archive, |archive| {
            let data = b"model";
            let mut header = tar::Header::new_gnu();
            header.set_path(format!("{expected_root}/model.onnx"))?;
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(0o644);
            header.set_size(data.len() as u64);
            header.set_cksum();
            archive.append(&header, Cursor::new(data))?;
            Ok(())
        })?;
        let valid_destination = temp.0.join("valid-output");
        fs::create_dir(&valid_destination)?;
        extract_tar_bz2(&valid_archive, &valid_destination, expected_root)?;
        assert_eq!(
            fs::read(valid_destination.join(expected_root).join("model.onnx"))?,
            b"model"
        );

        let traversal_archive = temp.0.join("traversal.tar.bz2");
        write_test_archive(&traversal_archive, |archive| {
            let data = b"x";
            let mut header = tar::Header::new_gnu();
            header.set_path(format!("{expected_root}/placeholder"))?;
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(0o644);
            header.set_size(data.len() as u64);
            let unsafe_path = format!("{expected_root}/../../escaped.txt");
            header.as_mut_bytes()[..100].fill(0);
            header.as_mut_bytes()[..unsafe_path.len()].copy_from_slice(unsafe_path.as_bytes());
            header.set_cksum();
            archive.append(&header, Cursor::new(data))?;
            Ok(())
        })?;
        let traversal_destination = temp.0.join("traversal-output");
        fs::create_dir(&traversal_destination)?;
        let error =
            extract_tar_bz2(&traversal_archive, &traversal_destination, expected_root).unwrap_err();
        assert!(error.to_string().contains("unsafe path"));
        assert!(!temp.0.join("escaped.txt").exists());

        let link_archive = temp.0.join("link.tar.bz2");
        write_test_archive(&link_archive, |archive| {
            let mut header = tar::Header::new_gnu();
            header.set_path(format!("{expected_root}/link"))?;
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name("model.onnx")?;
            header.set_mode(0o777);
            header.set_size(0);
            header.set_cksum();
            archive.append(&header, Cursor::new([]))?;
            Ok(())
        })?;
        let link_destination = temp.0.join("link-output");
        fs::create_dir(&link_destination)?;
        let error = extract_tar_bz2(&link_archive, &link_destination, expected_root).unwrap_err();
        assert!(error.to_string().contains("unsupported link or device"));
        assert!(!link_destination.join(expected_root).join("link").exists());

        Ok(())
    }
}
