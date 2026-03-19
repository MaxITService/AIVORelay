use crate::audio_toolkit::{
    list_input_devices, list_output_devices, vad::SmoothedVad, AudioCaptureSource, AudioRecorder,
    SileroVad, StreamFrameCallback,
};
use crate::helpers::clamshell;
use crate::settings::{get_settings, AppSettings, LiveSoundCaptureSource};
use crate::utils;
use log::{debug, error, info, warn};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::Manager;

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

const WHISPER_SAMPLE_RATE: usize = 16000;

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
}

#[derive(Clone, Debug)]
pub enum MicrophoneMode {
    AlwaysOn,
    OnDemand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveRecorderSelection {
    source: AudioCaptureSource,
    device_name: Option<String>,
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
    vad_path: &str,
    app_handle: &tauri::AppHandle,
    vad_threshold: f32,
) -> Result<AudioRecorder, anyhow::Error> {
    let settings = get_settings(app_handle);

    let mut recorder = AudioRecorder::new()
        .map_err(|e| anyhow::anyhow!("Failed to create AudioRecorder: {}", e))?;

    // Attach VAD when silence filtering is enabled.
    if settings.filter_silence {
        let silero = SileroVad::new(vad_path, vad_threshold)
            .map_err(|e| anyhow::anyhow!("Failed to create SileroVad: {}", e))?;
        let smoothed_vad = SmoothedVad::new(Box::new(silero), 15, 15, 2);
        recorder = recorder.with_vad(Box::new(smoothed_vad));
    }

    recorder = recorder.with_level_callback({
        let app_handle = app_handle.clone();
        move |levels| {
            utils::emit_levels(&app_handle, &levels);
        }
    });

    Ok(recorder)
}

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone)]
pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    mode: Arc<Mutex<MicrophoneMode>>,
    app_handle: tauri::AppHandle,

    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    is_open: Arc<Mutex<bool>>,
    is_recording: Arc<Mutex<bool>>,
    did_mute: Arc<Mutex<bool>>,
    active_selection: Arc<Mutex<Option<ActiveRecorderSelection>>>,
    stream_frame_callback: Arc<Mutex<Option<StreamFrameCallback>>>,
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
            did_mute: Arc::new(Mutex::new(false)),
            active_selection: Arc::new(Mutex::new(None)),
            stream_frame_callback: Arc::new(Mutex::new(None)),
        };

        // Always-on?  Open immediately.
        if matches!(mode, MicrophoneMode::AlwaysOn) {
            manager.start_microphone_stream()?;
        }

        Ok(manager)
    }

    /* ---------- helper methods --------------------------------------------- */

    fn get_effective_microphone_name(&self, settings: &AppSettings) -> Option<String> {
        // Check if we're in clamshell mode and have a clamshell microphone configured
        let use_clamshell_mic = if let Ok(is_clamshell) = clamshell::is_clamshell() {
            is_clamshell && settings.clamshell_microphone.is_some()
        } else {
            false
        };

        if use_clamshell_mic {
            settings.clamshell_microphone.clone()
        } else {
            settings.selected_microphone.clone()
        }
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
            }
        } else {
            ActiveRecorderSelection {
                source: AudioCaptureSource::Microphone,
                device_name: self.get_effective_microphone_name(settings),
            }
        }
    }

    fn resolve_device_for_selection(
        &self,
        selection: &ActiveRecorderSelection,
    ) -> Option<cpal::Device> {
        let Some(device_name) = selection.device_name.as_ref() else {
            return None;
        };

        let listed_devices = match selection.source {
            AudioCaptureSource::Microphone => list_input_devices(),
            AudioCaptureSource::SystemOutputLoopback => list_output_devices(),
        };

        match listed_devices {
            Ok(devices) => devices
                .into_iter()
                .find(|d| d.name == *device_name)
                .map(|d| d.device),
            Err(e) => {
                debug!("Failed to list devices, using default: {}", e);
                None
            }
        }
    }

    /* ---------- microphone life-cycle -------------------------------------- */

    /// Applies mute if mute_while_recording is enabled and stream is open
    pub fn apply_mute(&self) {
        let settings = get_settings(&self.app_handle);
        if !settings.mute_while_recording {
            return;
        }

        let is_open = *self.is_open.lock().unwrap();
        if !is_open {
            return;
        }

        // Before muting, ensure we didn't cancel/stop recording while waiting
        if !self.is_recording() {
            return;
        }

        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if !*did_mute_guard {
            set_mute(true);
            *did_mute_guard = true;
            debug!("Mute applied");
        }
    }

    /// Removes mute if it was applied
    pub fn remove_mute(&self) {
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if *did_mute_guard {
            set_mute(false);
            *did_mute_guard = false;
            debug!("Mute removed");
        }
    }

    pub fn start_microphone_stream(&self) -> Result<(), anyhow::Error> {
        self.start_stream_for_binding(None)
    }

    pub fn start_stream_for_binding(&self, binding_id: Option<&str>) -> Result<(), anyhow::Error> {
        let settings = get_settings(&self.app_handle);
        let selection = self.resolve_selection_for_binding(&settings, binding_id);
        self.start_stream_for_selection(selection, &settings)
    }

    fn start_stream_for_selection(
        &self,
        selection: ActiveRecorderSelection,
        settings: &AppSettings,
    ) -> Result<(), anyhow::Error> {
        let is_open = *self.is_open.lock().unwrap();
        let active_selection = self.active_selection.lock().unwrap().clone();
        if is_open && active_selection.as_ref() == Some(&selection) {
            debug!(
                "Audio capture stream already active for {:?}",
                selection.source
            );
            return Ok(());
        }

        if is_open {
            self.stop_microphone_stream();
        }

        let start_time = Instant::now();

        // Don't mute immediately - caller will handle muting after audio feedback
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        *did_mute_guard = false;
        drop(did_mute_guard);

        let vad_path = self
            .app_handle
            .path()
            .resolve(
                "resources/models/silero_vad_v4.onnx",
                tauri::path::BaseDirectory::Resource,
            )
            .map_err(|e| anyhow::anyhow!("Failed to resolve VAD path: {}", e))?;
        let mut recorder_opt = self.recorder.lock().unwrap();

        if recorder_opt.is_none() {
            let recorder = create_audio_recorder(
                vad_path.to_str().unwrap(),
                &self.app_handle,
                settings.vad_threshold,
            )?;
            if let Some(cb) = self
                .stream_frame_callback
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            {
                recorder.set_stream_frame_callback(Some(cb));
            }
            *recorder_opt = Some(recorder);
        }

        let selected_device = self.resolve_device_for_selection(&selection);

        if let Some(rec) = recorder_opt.as_mut() {
            rec.set_microphone_input_boost_db(
                settings.microphone_input_boost_db_for_device(selection.device_name.as_deref()),
            );
            rec.open_with_source(selected_device, selection.source)
                .map_err(|e| anyhow::anyhow!("Failed to open recorder: {}", e))?;
        }

        *self.is_open.lock().unwrap() = true;
        *self.active_selection.lock().unwrap() = Some(selection.clone());

        info!(
            "Audio capture stream initialized for {:?} in {:?}",
            selection.source,
            start_time.elapsed()
        );
        Ok(())
    }

    pub fn stop_microphone_stream(&self) {
        let mut open_flag = self.is_open.lock().unwrap();
        if !*open_flag {
            return;
        }

        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if *did_mute_guard {
            set_mute(false);
        }
        *did_mute_guard = false;

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
        let cur_mode = self.mode.lock().unwrap().clone();

        match (cur_mode, &new_mode) {
            (MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand) => {
                if matches!(*self.state.lock().unwrap(), RecordingState::Idle) {
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

    pub fn try_start_recording_detailed(
        &self,
        binding_id: &str,
    ) -> Result<(), StartRecordingError> {
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
                if let Err(err) = rec.start() {
                    let message = err.to_string();
                    error!(
                        "Failed to start recorder for binding {binding_id}: {}",
                        message
                    );
                    return Err(StartRecordingError::RecorderStartFailed {
                        source: selection.source,
                        message,
                    });
                }

                *self.is_recording.lock().unwrap() = true;
                *state = RecordingState::Recording {
                    binding_id: binding_id.to_string(),
                };
                debug!("Recording started for binding {binding_id}");
                return Ok(());
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

    pub fn stop_recording(&self, binding_id: &str) -> Option<Vec<f32>> {
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording {
                binding_id: ref active,
            } if active == binding_id => {
                *state = RecordingState::Idle;
                drop(state);

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

                // In on-demand mode turn the mic off again
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                    self.stop_microphone_stream();
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
    pub fn is_recording(&self) -> bool {
        matches!(
            *self.state.lock().unwrap(),
            RecordingState::Recording { .. }
        )
    }

    /// Cancel any ongoing recording without returning audio samples
    pub fn cancel_recording(&self) {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Recording { .. } = *state {
            *state = RecordingState::Idle;
            drop(state);

            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                let _ = rec.stop(); // Discard the result
            }

            *self.is_recording.lock().unwrap() = false;

            // In on-demand mode turn the mic off again
            if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                self.stop_microphone_stream();
            }
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
