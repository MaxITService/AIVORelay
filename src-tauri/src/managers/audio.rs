use crate::audio_toolkit::{
    list_input_devices, list_output_devices,
    vad::{
        frames_for_duration_ms, EarshotVad, SmoothedVad, VAD_OFFLINE_HANGOVER_MS, VAD_ONSET_MS,
        VAD_PREFILL_MS,
    },
    AudioCaptureSource, AudioRecorder, SileroVad, StreamFrameCallback, VoiceActivityDetector,
};
use crate::helpers::clamshell;
use crate::settings::{
    get_settings, resolve_live_sound_provider, write_settings, AppSettings, LiveSoundCaptureSource,
    TranscriptionProvider, VadBackend,
};
use crate::utils;
use log::{debug, error, info, trace, warn};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;

const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

fn set_mute(mute: bool) {
    // Expected behavior:
    // - Windows: works on most systems using standard audio drivers.
    // - Linux: works on many systems (PipeWire, PulseAudio, ALSA),
    //   but some distros may lack the tools used.
    // - macOS: works on most standard setups via AppleScript.
    // If unsupported, fails silently.

    #[cfg(target_os = "windows")]
    {
        unsafe {
            use windows::Win32::{
                Media::Audio::{
                    eMultimedia, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator,
                    MMDeviceEnumerator,
                },
                System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
            };

            macro_rules! unwrap_or_return {
                ($expr:expr) => {
                    match $expr {
                        Ok(val) => val,
                        Err(_) => return,
                    }
                };
            }

            // Initialize the COM library for this thread.
            // If already initialized (e.g., by another library like Tauri), this does nothing.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let all_devices: IMMDeviceEnumerator =
                unwrap_or_return!(CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL));
            let default_device =
                unwrap_or_return!(all_devices.GetDefaultAudioEndpoint(eRender, eMultimedia));
            let volume_interface = unwrap_or_return!(
                default_device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            );

            let _ = volume_interface.SetMute(mute, std::ptr::null());
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let mute_val = if mute { "1" } else { "0" };
        let amixer_state = if mute { "mute" } else { "unmute" };

        // Try multiple backends to increase compatibility
        // 1. PipeWire (wpctl)
        if Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", mute_val])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }

        // 2. PulseAudio (pactl)
        if Command::new("pactl")
            .args(["set-sink-mute", "@DEFAULT_SINK@", mute_val])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }

        // 3. ALSA (amixer)
        let _ = Command::new("amixer")
            .args(["set", "Master", amixer_state])
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = format!(
            "set volume output muted {}",
            if mute { "true" } else { "false" }
        );
        let _ = Command::new("osascript").args(["-e", &script]).output();
    }
}

/// Reads the current system output mute state using the same platform backend
/// as `set_mute`. `None` means the state could not be determined.
#[cfg(target_os = "windows")]
fn get_mute() -> Option<bool> {
    unsafe {
        use windows::Win32::{
            Media::Audio::{
                eMultimedia, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator,
                MMDeviceEnumerator,
            },
            System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
        };

        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let all_devices: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let default_device = all_devices
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .ok()?;
        let volume_interface = default_device
            .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            .ok()?;

        Some(volume_interface.GetMute().ok()?.as_bool())
    }
}

#[cfg(target_os = "linux")]
fn get_mute() -> Option<bool> {
    use std::process::Command;

    if let Ok(output) = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
    {
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).contains("[MUTED]"));
        }
    }

    if let Ok(output) = Command::new("pactl")
        .env("LC_ALL", "C")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
    {
        if output.status.success() {
            let state = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if state.contains("yes") {
                return Some(true);
            }
            if state.contains("no") {
                return Some(false);
            }
        }
    }

    if let Ok(output) = Command::new("amixer")
        .env("LC_ALL", "C")
        .args(["get", "Master"])
        .output()
    {
        if output.status.success() {
            let state = String::from_utf8_lossy(&output.stdout);
            if state.contains("[off]") {
                return Some(true);
            }
            if state.contains("[on]") {
                return Some(false);
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn get_mute() -> Option<bool> {
    use std::process::Command;

    let output = Command::new("osascript")
        .args(["-e", "output muted of (get volume settings)"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    match String::from_utf8_lossy(&output.stdout).trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn get_mute() -> Option<bool> {
    None
}

fn restore_mute(previously_muted: Option<bool>) {
    // Preserve an intentional pre-existing mute. For an unknown state, retain
    // the old fail-safe behavior and unmute so AIVO cannot strand system audio.
    if previously_muted != Some(true) {
        set_mute(false);
    }
}

#[cfg(target_os = "windows")]
fn pause_media_playback() -> Vec<String> {
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };

    let mut paused_sessions = Vec::new();

    let manager = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .and_then(|operation| operation.get())
    {
        Ok(manager) => manager,
        Err(err) => {
            debug!("Media pause unavailable: {}", err);
            return paused_sessions;
        }
    };

    let sessions = match manager.GetSessions() {
        Ok(sessions) => sessions,
        Err(err) => {
            debug!("Media pause failed to enumerate sessions: {}", err);
            return paused_sessions;
        }
    };

    let session_count = sessions.Size().unwrap_or(0);
    for index in 0..session_count {
        let Ok(session) = sessions.GetAt(index) else {
            continue;
        };

        let is_playing = session
            .GetPlaybackInfo()
            .and_then(|info| info.PlaybackStatus())
            .map(|status| {
                status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
            })
            .unwrap_or(false);

        if !is_playing {
            continue;
        }

        let source_id = session
            .SourceAppUserModelId()
            .map(|id| id.to_string_lossy())
            .unwrap_or_default();

        match session
            .TryPauseAsync()
            .and_then(|operation| operation.get())
        {
            Ok(true) => {
                if !source_id.is_empty() {
                    paused_sessions.push(source_id);
                }
            }
            Ok(false) => debug!("Media pause declined by session"),
            Err(err) => debug!("Media pause failed: {}", err),
        }
    }

    paused_sessions
}

#[cfg(target_os = "windows")]
fn resume_media_playback(paused_sessions: &[String]) {
    use std::collections::HashSet;
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

    if paused_sessions.is_empty() {
        return;
    }

    let paused_ids: HashSet<&str> = paused_sessions.iter().map(String::as_str).collect();
    let manager = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .and_then(|operation| operation.get())
    {
        Ok(manager) => manager,
        Err(err) => {
            debug!("Media resume unavailable: {}", err);
            return;
        }
    };

    let sessions = match manager.GetSessions() {
        Ok(sessions) => sessions,
        Err(err) => {
            debug!("Media resume failed to enumerate sessions: {}", err);
            return;
        }
    };

    let session_count = sessions.Size().unwrap_or(0);
    for index in 0..session_count {
        let Ok(session) = sessions.GetAt(index) else {
            continue;
        };
        let source_id = session
            .SourceAppUserModelId()
            .map(|id| id.to_string_lossy())
            .unwrap_or_default();

        if !paused_ids.contains(source_id.as_str()) {
            continue;
        }

        if let Err(err) = session.TryPlayAsync().and_then(|operation| operation.get()) {
            debug!("Media resume failed: {}", err);
        }
    }
}

#[cfg(target_os = "linux")]
fn pause_media_playback() -> Vec<String> {
    use std::process::Command;

    let mut paused_players = Vec::new();
    let output = match Command::new("playerctl").arg("-l").output() {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return paused_players,
    };

    let players = String::from_utf8_lossy(&output.stdout);
    for player in players.lines().map(str::trim).filter(|p| !p.is_empty()) {
        let status = Command::new("playerctl")
            .args(["-p", player, "status"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());

        if status.as_deref() != Some("Playing") {
            continue;
        }

        if Command::new("playerctl")
            .args(["-p", player, "pause"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            paused_players.push(player.to_string());
        }
    }

    paused_players
}

#[cfg(target_os = "linux")]
fn resume_media_playback(paused_players: &[String]) {
    use std::process::Command;

    for player in paused_players {
        let _ = Command::new("playerctl")
            .args(["-p", player, "play"])
            .output();
    }
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> Option<String> {
    use std::process::Command;

    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn macos_process_is_running(process_name: &str) -> bool {
    let escaped_name = process_name.replace('"', "\\\"");
    let script = format!(
        "tell application \"System Events\" to (name of processes) contains \"{}\"",
        escaped_name
    );
    run_osascript(&script).as_deref() == Some("true")
}

#[cfg(target_os = "macos")]
fn pause_media_playback() -> Vec<String> {
    let mut paused_apps = Vec::new();
    for app_name in ["Music", "Spotify", "QuickTime Player"] {
        if !macos_process_is_running(app_name) {
            continue;
        }

        let escaped_name = app_name.replace('"', "\\\"");
        let state_script = format!(
            "tell application \"{}\" to player state as string",
            escaped_name
        );
        if run_osascript(&state_script).as_deref() != Some("playing") {
            continue;
        }

        let pause_script = format!("tell application \"{}\" to pause", escaped_name);
        if run_osascript(&pause_script).is_some() {
            paused_apps.push(app_name.to_string());
        }
    }
    paused_apps
}

#[cfg(target_os = "macos")]
fn resume_media_playback(paused_apps: &[String]) {
    for app_name in paused_apps {
        if !macos_process_is_running(app_name) {
            continue;
        }

        let escaped_name = app_name.replace('"', "\\\"");
        let play_script = format!("tell application \"{}\" to play", escaped_name);
        let _ = run_osascript(&play_script);
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn pause_media_playback() -> Vec<String> {
    Vec::new()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn resume_media_playback(_paused_sessions: &[String]) {}

const WHISPER_SAMPLE_RATE: usize = 16000;

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
    Stopping,
}

#[derive(Clone, Debug)]
pub enum MicrophoneMode {
    AlwaysOn,
    OnDemand,
}

#[derive(Clone, Copy, Debug, Default)]
struct MuteState {
    did_mute: bool,
    previously_muted: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveRecorderSelection {
    source: AudioCaptureSource,
    device_name: Option<String>,
    clear_selected_microphone_on_fallback: bool,
}

struct CaptureDeviceResolution {
    device: Option<cpal::Device>,
    unavailable_selected_microphone: Option<String>,
}

#[derive(Clone, Debug)]
pub enum StartRecordingError {
    AlreadyRecording,
    StreamOpenFailed {
        source: AudioCaptureSource,
        message: String,
    },
    RecorderStartFailed {
        source: AudioCaptureSource,
        message: String,
    },
    RecorderUnavailable {
        source: AudioCaptureSource,
    },
}

impl StartRecordingError {
    pub fn source(&self) -> Option<AudioCaptureSource> {
        match self {
            StartRecordingError::AlreadyRecording => None,
            StartRecordingError::StreamOpenFailed { source, .. }
            | StartRecordingError::RecorderStartFailed { source, .. }
            | StartRecordingError::RecorderUnavailable { source } => Some(*source),
        }
    }

    pub fn is_microphone_related(&self) -> bool {
        matches!(self.source(), Some(AudioCaptureSource::Microphone))
    }
}

impl fmt::Display for StartRecordingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartRecordingError::AlreadyRecording => {
                write!(f, "Recording is already in progress.")
            }
            StartRecordingError::StreamOpenFailed { message, .. }
            | StartRecordingError::RecorderStartFailed { message, .. } => write!(f, "{}", message),
            StartRecordingError::RecorderUnavailable { source } => match source {
                AudioCaptureSource::Microphone => {
                    write!(f, "Microphone recorder is not available.")
                }
                AudioCaptureSource::SystemOutputLoopback => {
                    write!(f, "System output recorder is not available.")
                }
            },
        }
    }
}

/* ──────────────────────────────────────────────────────────────── */

fn create_audio_recorder(
    app_handle: &tauri::AppHandle,
    settings: &AppSettings,
    backend: VadBackend,
) -> Result<AudioRecorder, anyhow::Error> {
    let mut recorder = AudioRecorder::new()
        .map_err(|e| anyhow::anyhow!("Failed to create AudioRecorder: {}", e))?;

    // Attach VAD when silence filtering is enabled.
    if settings.filter_silence {
        let detector: Box<dyn VoiceActivityDetector> = match backend {
            VadBackend::Silero => {
                let vad_path = app_handle
                    .path()
                    .resolve(
                        "resources/models/silero_vad_v4.onnx",
                        tauri::path::BaseDirectory::Resource,
                    )
                    .map_err(|e| anyhow::anyhow!("Failed to resolve VAD path: {e}"))?;
                Box::new(
                    SileroVad::new(vad_path, settings.vad_threshold)
                        .map_err(|e| anyhow::anyhow!("Failed to create SileroVad: {e}"))?,
                )
            }
            VadBackend::Earshot => Box::new(
                EarshotVad::new(settings.vad_threshold)
                    .map_err(|e| anyhow::anyhow!("Failed to create EarshotVad: {e}"))?,
            ),
        };

        let frame_samples = detector.frame_samples();
        let prefill_frames = frames_for_duration_ms(VAD_PREFILL_MS, frame_samples);
        let hangover_frames = frames_for_duration_ms(VAD_OFFLINE_HANGOVER_MS, frame_samples);
        let onset_frames = frames_for_duration_ms(VAD_ONSET_MS, frame_samples);
        let smoothed_vad =
            SmoothedVad::new(detector, prefill_frames, hangover_frames, onset_frames);
        recorder = recorder.with_vad(Box::new(smoothed_vad));
    }

    recorder = recorder.with_selected_channel(settings.selected_channel);

    recorder = recorder.with_level_callback({
        let app_handle = app_handle.clone();
        move |levels| {
            utils::emit_levels(&app_handle, &levels);
        }
    });

    Ok(recorder)
}

fn should_apply_extra_recording_buffer(settings: &AppSettings, binding_id: &str) -> bool {
    if settings.extra_recording_buffer_ms == 0 {
        return false;
    }

    let effective_provider = if binding_id == crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
        resolve_live_sound_provider(settings)
    } else {
        settings.transcription_provider
    };

    effective_provider == TranscriptionProvider::Local
}

/* ──────────────────────────────────────────────────────────────── */

/// One recording session's first-sample notification. Callers wait on a
/// dedicated worker so shortcut handling never blocks on slow audio hardware.
pub struct RecordingReadiness {
    receiver: mpsc::Receiver<()>,
    generation: u64,
}

impl RecordingReadiness {
    pub fn wait(self) -> bool {
        self.receiver.recv().is_ok()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone)]
pub struct AudioRecordingManager {
    /// Never assign through this directly — route every write through
    /// `set_state()`, which keeps `recording_active` in sync.
    state: Arc<Mutex<RecordingState>>,
    mode: Arc<Mutex<MicrophoneMode>>,
    app_handle: tauri::AppHandle,

    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    is_open: Arc<Mutex<bool>>,
    is_recording: Arc<Mutex<bool>>,
    mute_state: Arc<Mutex<MuteState>>,
    paused_media_sessions: Arc<Mutex<Vec<String>>>,
    close_generation: Arc<AtomicU64>,
    cancel_generation: Arc<AtomicU64>,
    active_selection: Arc<Mutex<Option<ActiveRecorderSelection>>>,
    stream_frame_callback: Arc<Mutex<Option<StreamFrameCallback>>>,
    /// Lock-free mirror of "is the state in {Recording, Stopping}",
    /// maintained by `set_state()`. The hot-path `is_recording()` reads THIS
    /// instead of the std `state` mutex, so a UI poll can no longer deadlock
    /// the main/webview thread when a worker holds `state` across a slow
    /// CoreAudio open/close.
    recording_active: Arc<AtomicBool>,
    /// Invalidates delayed first-sample UI/chime work after stop or cancel.
    capture_generation: Arc<AtomicU64>,
    cached_device: Arc<Mutex<Option<(ActiveRecorderSelection, cpal::Device)>>>,
}

impl AudioRecordingManager {
    /* ---------- construction ------------------------------------------------ */

    pub fn new(app: &tauri::AppHandle) -> Result<Self, anyhow::Error> {
        let settings = get_settings(app);
        let mode = if settings.always_on_microphone {
            MicrophoneMode::AlwaysOn
        } else {
            MicrophoneMode::OnDemand
        };

        let manager = Self {
            state: Arc::new(Mutex::new(RecordingState::Idle)),
            mode: Arc::new(Mutex::new(mode.clone())),
            app_handle: app.clone(),

            recorder: Arc::new(Mutex::new(None)),
            is_open: Arc::new(Mutex::new(false)),
            is_recording: Arc::new(Mutex::new(false)),
            mute_state: Arc::new(Mutex::new(MuteState::default())),
            paused_media_sessions: Arc::new(Mutex::new(Vec::new())),
            close_generation: Arc::new(AtomicU64::new(0)),
            cancel_generation: Arc::new(AtomicU64::new(0)),
            active_selection: Arc::new(Mutex::new(None)),
            stream_frame_callback: Arc::new(Mutex::new(None)),
            recording_active: Arc::new(AtomicBool::new(false)),
            capture_generation: Arc::new(AtomicU64::new(0)),
            cached_device: Arc::new(Mutex::new(None)),
        };

        // Always-on?  Open immediately.
        if matches!(mode, MicrophoneMode::AlwaysOn) {
            manager.start_microphone_stream()?;
        }

        Ok(manager)
    }

    /* ---------- helper methods --------------------------------------------- */

    fn get_effective_microphone_selection(
        &self,
        settings: &AppSettings,
    ) -> (Option<String>, bool) {
        if settings.clamshell_microphone.is_some() {
            let clamshell_started = Instant::now();
            let is_clamshell = clamshell::is_clamshell().unwrap_or(false);
            debug!(
                "device resolve: clamshell_check={:?} (clamshell={})",
                clamshell_started.elapsed(),
                is_clamshell
            );
            if is_clamshell {
                return (settings.clamshell_microphone.clone(), false);
            }
        }

        (
            settings.selected_microphone.clone(),
            settings.selected_microphone.is_some(),
        )
    }

    pub fn invalidate_device_cache(&self) {
        *self.cached_device.lock().unwrap() = None;
    }

    fn resolve_selection_for_binding(
        &self,
        settings: &AppSettings,
        binding_id: Option<&str>,
    ) -> ActiveRecorderSelection {
        let use_live_sound_output = binding_id
            == Some(crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID)
            && settings.live_sound_capture_source == LiveSoundCaptureSource::SystemOutput;

        if use_live_sound_output {
            ActiveRecorderSelection {
                source: AudioCaptureSource::SystemOutputLoopback,
                device_name: settings.selected_output_device.clone(),
                clear_selected_microphone_on_fallback: false,
            }
        } else {
            let (device_name, clear_selected_microphone_on_fallback) =
                self.get_effective_microphone_selection(settings);
            ActiveRecorderSelection {
                source: AudioCaptureSource::Microphone,
                device_name,
                clear_selected_microphone_on_fallback,
            }
        }
    }

    fn resolve_device_for_selection(
        &self,
        selection: &ActiveRecorderSelection,
    ) -> CaptureDeviceResolution {
        let Some(device_name) = selection.device_name.as_ref() else {
            return CaptureDeviceResolution {
                device: None,
                unavailable_selected_microphone: None,
            };
        };

        if let Some((cached_selection, device)) = self.cached_device.lock().unwrap().as_ref() {
            if cached_selection == selection {
                debug!("device resolve: cache hit for '{}'", device_name);
                return CaptureDeviceResolution {
                    device: Some(device.clone()),
                    unavailable_selected_microphone: None,
                };
            }
        }

        let enumerate_started = Instant::now();
        let listed_devices = match selection.source {
            AudioCaptureSource::Microphone => list_input_devices(),
            AudioCaptureSource::SystemOutputLoopback => list_output_devices(),
        };

        let (device, enumeration_succeeded) = match listed_devices {
            Ok(devices) => (
                devices
                    .into_iter()
                    .find(|d| d.name == *device_name)
                    .map(|d| d.device),
                true,
            ),
            Err(e) => {
                debug!("Failed to list devices, using default: {}", e);
                (None, false)
            }
        };
        debug!(
            "device resolve: enumerate={:?} (found={})",
            enumerate_started.elapsed(),
            device.is_some()
        );
        if let Some(device) = &device {
            *self.cached_device.lock().unwrap() = Some((selection.clone(), device.clone()));
        }
        let unavailable_selected_microphone = if enumeration_succeeded
            && device.is_none()
            && selection.clear_selected_microphone_on_fallback
        {
            Some(device_name.clone())
        } else {
            None
        };

        CaptureDeviceResolution {
            device,
            unavailable_selected_microphone,
        }
    }

    /// Keep persisted settings and the UI aligned after the system-default
    /// microphone successfully replaces a selected device that disappeared.
    fn persist_default_microphone_after_fallback(&self, unavailable_name: &str) {
        let mut settings = get_settings(&self.app_handle);
        if settings.selected_microphone.as_deref() != Some(unavailable_name) {
            return;
        }

        settings.selected_microphone = None;
        write_settings(&self.app_handle, settings);
        crate::managers::microphone_auto_switch::emit_audio_input_state_changed(&self.app_handle);
    }

    fn should_use_lazy_stream_close(&self) -> bool {
        if !get_settings(&self.app_handle).lazy_stream_close {
            return false;
        }

        self.active_selection
            .lock()
            .unwrap()
            .as_ref()
            .map(|selection| selection.source == AudioCaptureSource::Microphone)
            .unwrap_or(false)
    }

    fn schedule_lazy_close(&self) {
        let generation = self.close_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let app_handle = self.app_handle.clone();

        std::thread::spawn(move || {
            std::thread::sleep(STREAM_IDLE_TIMEOUT);
            let manager = app_handle.state::<Arc<AudioRecordingManager>>();
            let state = manager.state.lock().unwrap();

            if manager.close_generation.load(Ordering::SeqCst) == generation
                && matches!(*state, RecordingState::Idle)
            {
                info!(
                    "Closing idle microphone stream after {:?}",
                    STREAM_IDLE_TIMEOUT
                );
                manager.stop_microphone_stream();
            }
        });
    }

    /* ---------- microphone life-cycle -------------------------------------- */

    /// Applies mute if mute_while_recording is enabled and stream is open.
    /// The user's previous mute state is captured once per recording.
    pub fn apply_mute(&self) {
        let settings = get_settings(&self.app_handle);
        if !settings.mute_while_recording {
            return;
        }

        let is_open = self.is_open.lock().unwrap();
        if !*is_open {
            return;
        }

        // Before muting, ensure we didn't cancel/stop recording while waiting
        if !self.is_recording() {
            return;
        }

        let mut mute_state = self.mute_state.lock().unwrap();
        if mute_state.did_mute {
            return;
        }

        mute_state.previously_muted = get_mute();
        set_mute(true);
        mute_state.did_mute = true;
        debug!(
            "Mute applied (previously_muted={:?})",
            mute_state.previously_muted
        );
    }

    /// Pauses active media if pause_media_while_recording is enabled.
    pub fn apply_media_pause(&self) {
        let settings = get_settings(&self.app_handle);
        if !settings.pause_media_while_recording {
            return;
        }

        // Before pausing, ensure we didn't cancel/stop recording while waiting.
        if !self.is_recording() {
            return;
        }

        let mut paused_guard = self.paused_media_sessions.lock().unwrap();
        if !paused_guard.is_empty() {
            return;
        }

        let paused_sessions = pause_media_playback();
        if !paused_sessions.is_empty() {
            debug!(
                "Paused {} media session(s) while recording",
                paused_sessions.len()
            );
        }
        *paused_guard = paused_sessions;
    }

    /// Removes AIVO's mute while preserving an intentional pre-existing mute.
    pub fn remove_mute(&self) {
        let mut mute_state = self.mute_state.lock().unwrap();
        if mute_state.did_mute {
            restore_mute(mute_state.previously_muted);
            mute_state.did_mute = false;
            debug!(
                "Mute removed (restored previously_muted={:?})",
                mute_state.previously_muted
            );
        }
    }

    /// Resumes media sessions that this recording paused.
    pub fn resume_media_if_paused(&self) {
        let mut paused_guard = self.paused_media_sessions.lock().unwrap();
        if paused_guard.is_empty() {
            return;
        }

        resume_media_playback(&paused_guard);
        debug!(
            "Requested resume for {} media session(s)",
            paused_guard.len()
        );
        paused_guard.clear();
    }

    pub fn start_microphone_stream(&self) -> Result<(), anyhow::Error> {
        self.start_stream_for_binding(None)
    }

    pub fn start_stream_for_binding(&self, binding_id: Option<&str>) -> Result<(), anyhow::Error> {
        let settings = get_settings(&self.app_handle);
        let selection = self.resolve_selection_for_binding(&settings, binding_id);
        self.start_stream_for_selection(selection, &settings)
    }

    pub fn preload_audio_recorder(&self) -> Result<(), anyhow::Error> {
        // Serialize the detached preload with Filter Silence/backend changes.
        // Read settings only after taking the state lock so a stale snapshot
        // cannot install an obsolete recorder after invalidation completes.
        let state_guard = self.state.lock().unwrap();
        if !matches!(*state_guard, RecordingState::Idle) {
            return Ok(());
        }
        let settings = get_settings(&self.app_handle);
        self.ensure_recorder(&settings)
    }

    fn ensure_recorder(&self, settings: &AppSettings) -> Result<(), anyhow::Error> {
        if self.recorder.lock().unwrap().is_none() {
            // Build outside the recorder mutex: Silero model initialization can
            // be comparatively slow, and a candidate must be fully valid before
            // it becomes visible to the rest of the manager.
            let recorder = create_audio_recorder(&self.app_handle, settings, settings.vad_backend)?;
            let mut recorder_guard = self.recorder.lock().unwrap();
            if recorder_guard.is_none() {
                let callback = self
                    .stream_frame_callback
                    .lock()
                    .ok()
                    .and_then(|guard| guard.clone());
                recorder.set_stream_frame_callback(callback);
                *recorder_guard = Some(recorder);
            }
        }

        Ok(())
    }

    /// Replace the recording-side VAD while idle. A candidate is constructed
    /// before the current recorder is discarded; a warm stream is reopened on
    /// the same source/device and rolled back on failure.
    pub fn update_vad_backend(&self, backend: VadBackend) -> Result<(), anyhow::Error> {
        let state_guard = self.state.lock().unwrap();
        if !matches!(*state_guard, RecordingState::Idle) {
            return Err(anyhow::anyhow!(
                "Cannot change the VAD backend while recording"
            ));
        }

        let settings = get_settings(&self.app_handle);
        if !settings.filter_silence {
            return Ok(());
        }

        // Build and validate the new detector before touching the active one.
        let replacement = create_audio_recorder(&self.app_handle, &settings, backend)?;

        let was_open = *self.is_open.lock().unwrap();
        let restart_selection = self.active_selection.lock().unwrap().clone();
        if was_open {
            self.stop_microphone_stream();
        }

        let previous_recorder = {
            let mut recorder_guard = self.recorder.lock().unwrap();
            let callback = self
                .stream_frame_callback
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            replacement.set_stream_frame_callback(callback);
            recorder_guard.replace(replacement)
        };
        if !was_open {
            info!(
                "Prepared {:?} VAD backend while capture was closed",
                backend
            );
            return Ok(());
        }

        let reopen_selection = restart_selection
            .clone()
            .unwrap_or_else(|| self.resolve_selection_for_binding(&settings, None));
        if let Err(change_error) = self.start_stream_for_selection(reopen_selection, &settings) {
            // Close the partial candidate before restoring the known-good
            // recorder. Do not hold the recorder mutex while opening again.
            let candidate = self.recorder.lock().unwrap().take();
            if let Some(mut recorder) = candidate {
                let _ = recorder.close();
            }
            *self.recorder.lock().unwrap() = previous_recorder;

            let rollback_selection = restart_selection
                .unwrap_or_else(|| self.resolve_selection_for_binding(&settings, None));
            if let Err(rollback_error) =
                self.start_stream_for_selection(rollback_selection, &settings)
            {
                error!(
                    "Failed to restore audio capture after VAD backend change failed: {rollback_error}"
                );
            }
            return Err(anyhow::anyhow!(
                "Failed to reopen audio capture with {:?} VAD: {change_error}",
                backend
            ));
        }

        info!("VAD backend changed to {:?}", backend);
        Ok(())
    }

    fn start_stream_for_selection(
        &self,
        mut selection: ActiveRecorderSelection,
        settings: &AppSettings,
    ) -> Result<(), anyhow::Error> {
        self.close_generation.fetch_add(1, Ordering::SeqCst);

        let is_open = *self.is_open.lock().unwrap();
        let active_selection = self.active_selection.lock().unwrap().clone();
        if is_open && active_selection.as_ref() == Some(&selection) {
            // `is_open` only records that we opened a stream at some point, not
            // that one is still healthy. If cpal has reported a stream error or
            // the capture worker exited, rebuild before the next recording.
            let needs_reopen = self
                .recorder
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|rec| rec.needs_reopen());

            if !needs_reopen {
                trace!(
                    "Audio capture stream already active for {:?}",
                    selection.source
                );
                return Ok(());
            }

            warn!(
                "Audio capture stream for {:?} is no longer running; reopening",
                selection.source
            );
            if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
                let _ = rec.close();
            }
            *self.is_recording.lock().unwrap() = false;
            *self.is_open.lock().unwrap() = false;
            *self.active_selection.lock().unwrap() = None;
            self.resume_media_if_paused();
            self.invalidate_device_cache();
            // Fall through and open a fresh stream.
        } else if is_open {
            self.stop_microphone_stream();
        }

        let start_time = Instant::now();

        // Don't mute immediately - caller will handle muting after audio feedback.
        // Restore a stale forced mute instead of merely forgetting about it.
        {
            let mut mute_state = self.mute_state.lock().unwrap();
            if mute_state.did_mute {
                restore_mute(mute_state.previously_muted);
                mute_state.did_mute = false;
            }
        }
        self.paused_media_sessions.lock().unwrap().clear();

        self.ensure_recorder(settings)?;

        let resolve_started = Instant::now();
        let mut resolution = self.resolve_device_for_selection(&selection);
        let resolve_elapsed = resolve_started.elapsed();

        let open_started = Instant::now();
        let mut recorder_opt = self.recorder.lock().unwrap();
        if let Some(rec) = recorder_opt.as_mut() {
            let boost_device_name = if resolution.unavailable_selected_microphone.is_some() {
                None
            } else {
                selection.device_name.as_deref()
            };
            rec.set_microphone_input_boost_db(
                settings.microphone_input_boost_db_for_device(boost_device_name),
            );
            rec.set_microphone_noise_cancellation_enabled(
                selection.source == AudioCaptureSource::Microphone
                    && settings.microphone_noise_cancellation_enabled,
            );
            if let Err(first_err) =
                rec.open_with_source(resolution.device.clone(), selection.source)
            {
                warn!("Recorder open failed ({first_err}); re-resolving device and retrying once");
                self.invalidate_device_cache();
                resolution = self.resolve_device_for_selection(&selection);
                let boost_device_name = if resolution.unavailable_selected_microphone.is_some() {
                    None
                } else {
                    selection.device_name.as_deref()
                };
                rec.set_microphone_input_boost_db(
                    settings.microphone_input_boost_db_for_device(boost_device_name),
                );
                rec.open_with_source(resolution.device.clone(), selection.source)
                    .map_err(|e| anyhow::anyhow!("Failed to open recorder: {}", e))?;
            }
        }
        drop(recorder_opt);

        if resolution.unavailable_selected_microphone.is_some() {
            selection.device_name = None;
            selection.clear_selected_microphone_on_fallback = false;
        }

        *self.is_open.lock().unwrap() = true;
        *self.active_selection.lock().unwrap() = Some(selection.clone());

        if let Some(unavailable_name) = resolution.unavailable_selected_microphone {
            self.persist_default_microphone_after_fallback(&unavailable_name);
        }

        info!(
            "Audio capture stream initialized for {:?} in {:?}",
            selection.source,
            start_time.elapsed()
        );
        debug!(
            "audio stream breakdown: device_resolve={:?} open={:?}",
            resolve_elapsed,
            open_started.elapsed()
        );
        Ok(())
    }

    pub fn stop_microphone_stream(&self) {
        self.close_generation.fetch_add(1, Ordering::SeqCst);
        let mut open_flag = self.is_open.lock().unwrap();
        if !*open_flag {
            return;
        }

        {
            let mut mute_state = self.mute_state.lock().unwrap();
            if mute_state.did_mute {
                restore_mute(mute_state.previously_muted);
            }
            *mute_state = MuteState::default();
        }
        self.resume_media_if_paused();

        if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
            // If still recording, stop first.
            if *self.is_recording.lock().unwrap() {
                let _ = rec.stop();
                *self.is_recording.lock().unwrap() = false;
            }
            let _ = rec.close();
        }

        *open_flag = false;
        *self.active_selection.lock().unwrap() = None;
        debug!("Audio capture stream stopped");
    }

    /* ---------- mode switching --------------------------------------------- */

    pub fn update_mode(&self, new_mode: MicrophoneMode) -> Result<(), anyhow::Error> {
        // Keep mode-driven stream changes atomic with recording start and VAD
        // backend replacement.
        let state = self.state.lock().unwrap();
        let cur_mode = self.mode.lock().unwrap().clone();

        match (cur_mode, &new_mode) {
            (MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand) => {
                if matches!(*state, RecordingState::Idle) {
                    self.stop_microphone_stream();
                }
            }
            (MicrophoneMode::OnDemand, MicrophoneMode::AlwaysOn) => {
                self.start_microphone_stream()?;
            }
            _ => {}
        }

        *self.mode.lock().unwrap() = new_mode;
        Ok(())
    }

    /* ---------- recording --------------------------------------------------- */

    /// The one place `state` is written. Derives `recording_active` (the
    /// lock-free mirror read by `is_recording()`) from the new value itself.
    fn set_state(&self, guard: &mut RecordingState, new_state: RecordingState) {
        *guard = new_state;
        self.recording_active.store(
            matches!(
                *guard,
                RecordingState::Recording { .. } | RecordingState::Stopping
            ),
            Ordering::SeqCst,
        );
    }

    pub fn try_start_recording_detailed(
        &self,
        binding_id: &str,
    ) -> Result<RecordingReadiness, StartRecordingError> {
        let settings = get_settings(&self.app_handle);
        let selection = self.resolve_selection_for_binding(&settings, Some(binding_id));
        if selection.source == AudioCaptureSource::Microphone {
            if let Err(err) =
                crate::managers::microphone_auto_switch::reconcile_selected_microphone_before_recording(
                    &self.app_handle,
                )
            {
                warn!(
                    "Failed to reconcile selected microphone before recording starts: {}",
                    err
                );
            }
        }

        let mut state = self.state.lock().unwrap();

        if let RecordingState::Idle = *state {
            // Ensure the correct capture source is open for this binding.
            if let Err(e) = self.start_stream_for_selection(selection.clone(), &settings) {
                let message = e.to_string();
                error!("Failed to open audio capture stream: {}", message);
                return Err(StartRecordingError::StreamOpenFailed {
                    source: selection.source,
                    message,
                });
            }

            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                let receiver = rec.start().map_err(|err| {
                    let message = err.to_string();
                    error!(
                        "Failed to start recorder for binding {binding_id}: {}",
                        message
                    );
                    StartRecordingError::RecorderStartFailed {
                        source: selection.source,
                        message,
                    }
                })?;
                let generation = self.capture_generation.fetch_add(1, Ordering::AcqRel) + 1;

                *self.is_recording.lock().unwrap() = true;
                self.set_state(
                    &mut state,
                    RecordingState::Recording {
                        binding_id: binding_id.to_string(),
                    },
                );
                debug!("Recording requested for binding {binding_id}");
                return Ok(RecordingReadiness {
                    receiver,
                    generation,
                });
            }
            error!("Recorder not available");
            Err(StartRecordingError::RecorderUnavailable {
                source: selection.source,
            })
        } else {
            Err(StartRecordingError::AlreadyRecording)
        }
    }

    pub fn update_selected_device(&self) -> Result<(), anyhow::Error> {
        // A persisted device change may arrive during recording. Defer the
        // physical reopen instead of interrupting capture, and serialize idle
        // reopens with VAD/backend replacement.
        let state = self.state.lock().unwrap();
        self.invalidate_device_cache();
        if !matches!(*state, RecordingState::Idle) {
            debug!("Deferring audio device reopen until the active recording finishes");
            return Ok(());
        }
        let current_selection = self.active_selection.lock().unwrap().clone();
        if *self.is_open.lock().unwrap()
            && current_selection
                .as_ref()
                .map(|selection| selection.source == AudioCaptureSource::Microphone)
                .unwrap_or(false)
        {
            let settings = get_settings(&self.app_handle);
            self.stop_microphone_stream();
            let selection = self.resolve_selection_for_binding(&settings, None);
            self.start_stream_for_selection(selection, &settings)?;
        }
        Ok(())
    }

    pub fn update_selected_channel(
        &self,
        selected_channel: Option<u16>,
    ) -> Result<(), anyhow::Error> {
        // Restarting capture would discard an active recording, so serialize
        // the change against recording start/stop.
        let state = self.state.lock().unwrap();
        if !matches!(*state, RecordingState::Idle) {
            return Err(anyhow::anyhow!(
                "Cannot change the input channel while recording"
            ));
        }

        let previous_channel = get_settings(&self.app_handle).selected_channel;
        let restart_selection = self.active_selection.lock().unwrap().clone();
        let restart_microphone = *self.is_open.lock().unwrap()
            && restart_selection
                .as_ref()
                .is_some_and(|selection| selection.source == AudioCaptureSource::Microphone);

        if restart_microphone {
            self.stop_microphone_stream();
        }
        if let Some(recorder) = self.recorder.lock().unwrap().as_mut() {
            recorder.set_selected_channel(selected_channel);
        }

        if restart_microphone {
            let settings = get_settings(&self.app_handle);
            let selection = restart_selection
                .unwrap_or_else(|| self.resolve_selection_for_binding(&settings, None));
            if let Err(error) = self.start_stream_for_selection(selection, &settings) {
                if let Some(recorder) = self.recorder.lock().unwrap().as_mut() {
                    recorder.set_selected_channel(previous_channel);
                }
                return Err(error);
            }
        }

        drop(state);
        Ok(())
    }

    /// Recreate the recorder from current settings (for VAD/silence toggle changes).
    /// Restarts the stream if it was already open.
    /// Returns false if invalidation is unsafe (e.g. while actively recording).
    pub fn invalidate_recorder(&self) -> bool {
        // Keep state locked for the full operation so a new recording cannot begin
        // between our safety check and stream restart.
        let state_guard = self.state.lock().unwrap();
        if !matches!(*state_guard, RecordingState::Idle) {
            warn!("Refusing to invalidate recorder while recording is active");
            return false;
        }

        let was_open = *self.is_open.lock().unwrap();
        let restart_selection = self.active_selection.lock().unwrap().clone();
        if was_open {
            self.stop_microphone_stream();
        }

        *self.recorder.lock().unwrap() = None;
        debug!("Recorder invalidated (will be re-created on next use)");

        if was_open {
            let settings = get_settings(&self.app_handle);
            let selection = restart_selection
                .unwrap_or_else(|| self.resolve_selection_for_binding(&settings, None));
            if let Err(e) = self.start_stream_for_selection(selection, &settings) {
                error!("Failed to restart audio capture stream after recorder invalidation: {e}");
            }
        }

        true
    }

    pub fn cancel_generation(&self) -> u64 {
        self.cancel_generation.load(Ordering::Acquire)
    }

    pub fn was_cancelled_since(&self, generation: u64) -> bool {
        self.cancel_generation.load(Ordering::Acquire) != generation
    }

    /// Prevent a slow audio device from producing a ready event or start chime
    /// after the active recording has already stopped or been cancelled.
    pub fn invalidate_recording_readiness(&self) {
        self.capture_generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn is_recording_readiness_current(&self, generation: u64) -> bool {
        self.capture_generation.load(Ordering::Acquire) == generation
    }

    pub fn stop_recording(&self, binding_id: &str) -> Option<Vec<f32>> {
        self.invalidate_recording_readiness();
        let cancel_generation = self.cancel_generation();
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording {
                binding_id: ref active,
            } if active == binding_id => {
                self.set_state(&mut state, RecordingState::Stopping);
                drop(state);

                let settings = get_settings(&self.app_handle);
                if should_apply_extra_recording_buffer(&settings, binding_id) {
                    debug!(
                        "Extra local recording buffer: sleeping {}ms before stopping",
                        settings.extra_recording_buffer_ms
                    );
                    let buffer = Duration::from_millis(settings.extra_recording_buffer_ms);
                    let started = Instant::now();
                    while started.elapsed() < buffer {
                        if self.was_cancelled_since(cancel_generation) {
                            debug!("Recording stop cancelled during extra buffer");
                            break;
                        }
                        let remaining = buffer.saturating_sub(started.elapsed());
                        std::thread::sleep(remaining.min(Duration::from_millis(25)));
                    }
                }

                let samples = if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                    match rec.stop() {
                        Ok(buf) => buf,
                        Err(e) => {
                            error!("stop() failed: {e}");
                            Vec::new()
                        }
                    }
                } else {
                    error!("Recorder not available");
                    Vec::new()
                };

                *self.is_recording.lock().unwrap() = false;
                self.set_state(&mut self.state.lock().unwrap(), RecordingState::Idle);

                // In on-demand mode, close the microphone lazily only for real mic capture.
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                    if self.should_use_lazy_stream_close() {
                        self.schedule_lazy_close();
                    } else {
                        self.stop_microphone_stream();
                    }
                }

                if self.was_cancelled_since(cancel_generation) {
                    debug!("Recording stop cancelled; discarding captured samples");
                    return None;
                }

                // Pad if very short
                let s_len = samples.len();
                // debug!("Got {} samples", s_len);
                if s_len < WHISPER_SAMPLE_RATE && s_len > 0 {
                    let mut padded = samples;
                    padded.resize(WHISPER_SAMPLE_RATE * 5 / 4, 0.0);
                    Some(padded)
                } else {
                    Some(samples)
                }
            }
            _ => None,
        }
    }

    pub fn flush_recording(
        &self,
        binding_id: &str,
        keep_samples: usize,
        min_samples: usize,
    ) -> Option<Vec<f32>> {
        let is_matching_recording = {
            let state = self.state.lock().unwrap();
            matches!(
                &*state,
                RecordingState::Recording { binding_id: active } if active == binding_id
            )
        };

        if !is_matching_recording {
            return None;
        }

        let samples = if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            match rec.flush(keep_samples, min_samples) {
                Ok(buf) => buf,
                Err(e) => {
                    error!("flush() failed: {e}");
                    Vec::new()
                }
            }
        } else {
            error!("Recorder not available");
            Vec::new()
        };

        Some(samples)
    }

    pub fn is_recording(&self) -> bool {
        // Lock-free: mirrors the `state` {Recording, Stopping} membership via
        // an atomic maintained by `set_state()`. Polled from the webview/main
        // thread, so it MUST NOT take the `state` mutex (a worker can hold it
        // across a slow CoreAudio open/close → main-thread deadlock / UI
        // freeze).
        self.recording_active.load(Ordering::SeqCst)
    }

    /// Cancel any ongoing recording without returning audio samples
    pub fn cancel_recording(&self) {
        self.invalidate_recording_readiness();
        self.cancel_generation.fetch_add(1, Ordering::AcqRel);
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording { .. } => {
                self.set_state(&mut state, RecordingState::Idle);
                drop(state);

                if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                    let _ = rec.stop(); // Discard the result
                }

                *self.is_recording.lock().unwrap() = false;

                // In on-demand mode, close the microphone lazily only for real mic capture.
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                    if self.should_use_lazy_stream_close() {
                        self.schedule_lazy_close();
                    } else {
                        self.stop_microphone_stream();
                    }
                }
            }
            RecordingState::Stopping => {
                debug!("Cancellation requested while recording is stopping");
            }
            RecordingState::Idle => {}
        }
    }
    pub fn update_vad_threshold(&self, threshold: f32) {
        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.set_vad_threshold(threshold);
        }
    }

    pub fn refresh_microphone_input_boost_from_settings(&self) {
        let selection = self.active_selection.lock().unwrap().clone();
        let settings = get_settings(&self.app_handle);

        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            let boost_db = selection
                .as_ref()
                .filter(|selection| selection.source == AudioCaptureSource::Microphone)
                .map(|selection| {
                    settings.microphone_input_boost_db_for_device(selection.device_name.as_deref())
                })
                .unwrap_or(0.0);
            rec.set_microphone_input_boost_db(boost_db);
        }
    }

    pub fn refresh_microphone_noise_cancellation_from_settings(&self) {
        let selection = self.active_selection.lock().unwrap().clone();
        let settings = get_settings(&self.app_handle);

        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            let enabled = selection
                .as_ref()
                .map(|selection| {
                    selection.source == AudioCaptureSource::Microphone
                        && settings.microphone_noise_cancellation_enabled
                })
                .unwrap_or(false);
            rec.set_microphone_noise_cancellation_enabled(enabled);
        }
    }

    pub fn set_stream_frame_callback(&self, callback: StreamFrameCallback) {
        if let Ok(mut guard) = self.stream_frame_callback.lock() {
            *guard = Some(callback.clone());
        }
        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.set_stream_frame_callback(Some(callback));
        }
    }

    pub fn clear_stream_frame_callback(&self) {
        if let Ok(mut guard) = self.stream_frame_callback.lock() {
            *guard = None;
        }
        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.set_stream_frame_callback(None);
        }
    }
}
