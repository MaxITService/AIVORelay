//! Independent audio pipeline for Live Sound Transcription.
//!
//! This module owns its own `AudioRecorder` and its own Soniox/Deepgram
//! realtime-manager instances, completely bypassing `AudioRecordingManager`
//! and the shared singleton managers.  This allows Live Sound and regular STT
//! to run concurrently without blocking each other.

use crate::audio_toolkit::{
    list_input_devices, list_output_devices, AudioCaptureSource, AudioRecorder,
};
use crate::managers::deepgram_realtime::DeepgramRealtimeManager;
use crate::managers::soniox_realtime::SonioxRealtimeManager;
use crate::settings::{get_settings, AppSettings, LiveSoundCaptureSource, TranscriptionProvider};
use log::{info, warn};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::AppHandle;

/* ── internal types ───────────────────────────────────────────────────────── */

enum ActiveRealtimeManager {
    Soniox(Arc<SonioxRealtimeManager>),
    Deepgram(Arc<DeepgramRealtimeManager>),
}

struct LiveSoundAudioSession {
    recorder: AudioRecorder,
    /// Second recorder used only in `Both` mode (mic alongside loopback).
    mic_recorder: Option<AudioRecorder>,
    realtime: Option<ActiveRealtimeManager>,
}

static SESSION: LazyLock<Mutex<Option<LiveSoundAudioSession>>> =
    LazyLock::new(|| Mutex::new(None));

/* ── helpers ──────────────────────────────────────────────────────────────── */

fn resolve_device(
    source: &AudioCaptureSource,
    device_name: Option<&str>,
) -> Option<cpal::Device> {
    let name = device_name?;
    let devices = match source {
        AudioCaptureSource::Microphone => list_input_devices(),
        AudioCaptureSource::SystemOutputLoopback => list_output_devices(),
    };
    devices
        .ok()?
        .into_iter()
        .find(|d| d.name == name)
        .map(|d| d.device)
}

/// Wire the frame callback so that each frame from `recorder` is mixed with
/// whatever mic samples are currently buffered, then pushed to `manager`.
///
/// Used for `Both` mode: loopback drives the clock; mic fills in.
fn wire_mixed_callback(
    recorder: &mut AudioRecorder,
    mic_buf: Arc<Mutex<Vec<f32>>>,
    manager: &ActiveRealtimeManager,
) {
    match manager {
        ActiveRealtimeManager::Soniox(m) => {
            let m = Arc::clone(m);
            recorder.set_stream_frame_callback(Some(Arc::new(move |frame| {
                let mixed = mix_with_mic_buf(&frame, &mic_buf);
                m.push_audio_frame(mixed);
            })));
        }
        ActiveRealtimeManager::Deepgram(m) => {
            let m = Arc::clone(m);
            recorder.set_stream_frame_callback(Some(Arc::new(move |frame| {
                let mixed = mix_with_mic_buf(&frame, &mic_buf);
                m.push_audio_frame(mixed);
            })));
        }
    }
}

fn mix_with_mic_buf(loopback: &[f32], mic_buf: &Mutex<Vec<f32>>) -> Vec<f32> {
    let mic_samples: Vec<f32> = mic_buf
        .lock()
        .map(|mut buf| {
            let take = loopback.len().min(buf.len());
            buf.drain(..take).collect()
        })
        .unwrap_or_default();

    loopback
        .iter()
        .enumerate()
        .map(|(i, &lb)| {
            let mic = mic_samples.get(i).copied().unwrap_or(0.0);
            (lb + mic) * 0.5
        })
        .collect()
}

fn start_live_session(
    app: &AppHandle,
    settings: &AppSettings,
    recorder: &mut AudioRecorder,
) -> Result<ActiveRealtimeManager, String> {
    match settings.transcription_provider {
        TranscriptionProvider::RemoteSoniox => {
            let manager = Arc::new(
                SonioxRealtimeManager::new(app)
                    .map_err(|e| format!("Failed to create Soniox manager: {}", e))?,
            );

            // Wire frame callback before starting the session so buffered
            // early frames are flushed once the WebSocket connects.
            let manager_cb = Arc::clone(&manager);
            recorder.set_stream_frame_callback(Some(Arc::new(move |frame| {
                manager_cb.push_audio_frame(frame);
            })));

            #[cfg(target_os = "windows")]
            let api_key = crate::secure_keys::get_soniox_api_key();
            #[cfg(not(target_os = "windows"))]
            let api_key = String::new();

            let options = crate::actions::build_soniox_options_for_live_sound(settings);
            manager
                .start_session(
                    crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID,
                    &api_key,
                    &settings.soniox_model,
                    options,
                    None, // Live Sound doesn't paste — no chunk callback needed
                )
                .map_err(|e| format!("Failed to start Soniox live session: {}", e))?;

            Ok(ActiveRealtimeManager::Soniox(manager))
        }

        TranscriptionProvider::RemoteDeepgram => {
            let manager = Arc::new(
                DeepgramRealtimeManager::new(app)
                    .map_err(|e| format!("Failed to create Deepgram manager: {}", e))?,
            );

            let manager_cb = Arc::clone(&manager);
            recorder.set_stream_frame_callback(Some(Arc::new(move |frame| {
                manager_cb.push_audio_frame(frame);
            })));

            #[cfg(target_os = "windows")]
            let api_key = crate::secure_keys::get_deepgram_api_key();
            #[cfg(not(target_os = "windows"))]
            let api_key = String::new();

            let options = crate::actions::build_deepgram_options_for_live_sound(settings);
            manager
                .start_session(
                    crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID,
                    &api_key,
                    &settings.deepgram_model,
                    options,
                    None,
                )
                .map_err(|e| format!("Failed to start Deepgram live session: {}", e))?;

            Ok(ActiveRealtimeManager::Deepgram(manager))
        }

        _ => Err("Live streaming requires Soniox or Deepgram provider".to_string()),
    }
}

/// Opens a mic recorder for `Both` mode and re-wires the loopback recorder's
/// callback to blend mic samples in before forwarding to the realtime manager.
fn open_mic_recorder_for_both(
    settings: &AppSettings,
    loopback_recorder: &mut AudioRecorder,
    realtime: Option<&ActiveRealtimeManager>,
) -> Result<AudioRecorder, String> {
    let Some(rt) = realtime else {
        return Err("No realtime manager — Both mode requires live streaming".to_string());
    };

    // Shared ring buffer: mic callback writes, loopback callback reads.
    let mic_buf: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

    // Re-wire the loopback recorder to mix in mic samples.
    wire_mixed_callback(loopback_recorder, Arc::clone(&mic_buf), rt);

    // Open the mic recorder; its sole job is to fill mic_buf.
    let mut mic_rec = AudioRecorder::new()
        .map_err(|e| format!("Failed to create mic recorder: {}", e))?;

    let mic_device = resolve_device(&AudioCaptureSource::Microphone, settings.selected_microphone.as_deref());
    mic_rec
        .open_with_source(mic_device, AudioCaptureSource::Microphone)
        .map_err(|e| format!("Failed to open mic stream: {}", e))?;

    mic_rec.set_stream_frame_callback(Some(Arc::new(move |frame| {
        if let Ok(mut buf) = mic_buf.lock() {
            // Cap buffer to ~1 second to avoid unbounded growth if mic runs fast.
            const MAX_SAMPLES: usize = 16_000;
            if buf.len() + frame.len() > MAX_SAMPLES {
                let drop = (buf.len() + frame.len()).saturating_sub(MAX_SAMPLES);
                buf.drain(..drop);
            }
            buf.extend_from_slice(&frame);
        }
    })));

    Ok(mic_rec)
}

/* ── public API ───────────────────────────────────────────────────────────── */

/// Start the Live Sound independent audio pipeline.
///
/// Opens the audio stream (loopback or mic according to settings), creates
/// a private realtime-manager instance if live-streaming is enabled, wires
/// the frame callback, and starts recording.
pub fn start(app: &AppHandle) -> Result<(), String> {
    let mut guard = SESSION
        .lock()
        .map_err(|_| "Failed to lock live sound audio session".to_string())?;

    if guard.is_some() {
        return Err("Live sound audio session is already active".to_string());
    }

    let settings = get_settings(app);
    let use_live = crate::actions::live_sound_use_live_streaming(&settings);

    let is_both = settings.live_sound_capture_source == LiveSoundCaptureSource::Both;

    let (source, device_name) = match settings.live_sound_capture_source {
        LiveSoundCaptureSource::SystemOutput | LiveSoundCaptureSource::Both => (
            AudioCaptureSource::SystemOutputLoopback,
            settings.selected_output_device.clone(),
        ),
        LiveSoundCaptureSource::Microphone => (
            AudioCaptureSource::Microphone,
            settings.selected_microphone.clone(),
        ),
    };

    let mut recorder = AudioRecorder::new()
        .map_err(|e| format!("Failed to create audio recorder: {}", e))?;

    let device = resolve_device(&source, device_name.as_deref());

    // Open the primary stream before starting the WebSocket session.
    recorder
        .open_with_source(device, source)
        .map_err(|e| format!("Failed to open audio stream: {}", e))?;

    let realtime = if use_live {
        match start_live_session(app, &settings, &mut recorder) {
            Ok(rt) => Some(rt),
            Err(e) => {
                let _ = recorder.close();
                return Err(e);
            }
        }
    } else {
        None
    };

    // In Both mode, open a second recorder for the mic and wire a mixer so
    // the loopback callback blends in mic samples before pushing to WebSocket.
    let mic_recorder = if is_both {
        match open_mic_recorder_for_both(&settings, &mut recorder, realtime.as_ref()) {
            Ok(r) => Some(r),
            Err(e) => {
                warn!("Both mode: mic recorder failed to open, falling back to loopback only: {}", e);
                None
            }
        }
    } else {
        None
    };

    recorder
        .start()
        .map_err(|e| format!("Failed to start recording: {}", e))?;

    if let Some(ref mic) = mic_recorder {
        if let Err(e) = mic.start() {
            warn!("Both mode: mic recorder failed to start: {}", e);
        }
    }

    *guard = Some(LiveSoundAudioSession { recorder, mic_recorder, realtime });
    info!(
        "Live sound audio pipeline started (live_streaming={}, both_mode={})",
        use_live, is_both
    );
    Ok(())
}

/// Stop the Live Sound audio pipeline.
///
/// For live-streaming mode: stops the recorder and spawns an async task that
/// finalizes the WebSocket session, then clears the recording flag once done.
///
/// For batch mode: stops the recorder; transcription of the captured samples
/// is a future concern (currently no-op beyond clearing recording state).
pub fn stop(app: &AppHandle) {
    let session = {
        let mut guard = match SESSION.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.take()
    };

    let Some(mut session) = session else {
        return;
    };

    // Stop both recorders (discard samples — live mode uses WebSocket stream).
    if let Err(e) = session.recorder.stop() {
        warn!("Live sound recorder stop returned error: {}", e);
    }
    let _ = session.recorder.close();

    if let Some(mut mic) = session.mic_recorder {
        let _ = mic.stop();
        let _ = mic.close();
    }

    match session.realtime {
        Some(ActiveRealtimeManager::Soniox(manager)) => {
            let app = app.clone();
            let timeout_ms = get_settings(&app).soniox_live_finalize_timeout_ms;
            tauri::async_runtime::spawn(async move {
                if let Err(e) = manager.finish_session(timeout_ms).await {
                    warn!("Live sound Soniox finalization error: {}", e);
                }
                // Session loop already cleared interim text on "finished" payload.
                crate::managers::live_sound_transcription::set_recording(&app, false);
            });
        }

        Some(ActiveRealtimeManager::Deepgram(manager)) => {
            let app = app.clone();
            let timeout_ms = get_settings(&app).deepgram_live_finalize_timeout_ms;
            tauri::async_runtime::spawn(async move {
                if let Err(e) = manager.finish_session(timeout_ms).await {
                    warn!("Live sound Deepgram finalization error: {}", e);
                }
                crate::managers::live_sound_transcription::set_recording(&app, false);
            });
        }

        None => {
            // Batch mode — mark as not recording immediately.
            // Full batch transcription support is a future enhancement.
            crate::managers::live_sound_transcription::set_recording(app, false);
        }
    }

    info!("Live sound audio pipeline stopped");
}

/// Returns true if a pipeline session is currently active (recording).
pub fn is_active() -> bool {
    SESSION.lock().map(|g| g.is_some()).unwrap_or(false)
}
