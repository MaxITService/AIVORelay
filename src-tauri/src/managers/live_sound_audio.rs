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
    primary_source: AudioCaptureSource,
    primary_device_name: Option<String>,
    mic_device_name: Option<String>,
    recorder: AudioRecorder,
    /// Second recorder used only in `Both` mode (mic alongside loopback).
    mic_recorder: Option<AudioRecorder>,
    realtime: Option<ActiveRealtimeManager>,
}

static SESSION: LazyLock<Mutex<Option<LiveSoundAudioSession>>> = LazyLock::new(|| Mutex::new(None));

/* ── helpers ──────────────────────────────────────────────────────────────── */

fn resolve_device(source: &AudioCaptureSource, device_name: Option<&str>) -> Option<cpal::Device> {
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

/// Wire the mic recorder's frame callback so that each mic frame is mixed with
/// whatever loopback samples are currently buffered, then pushed to `manager`.
///
/// Used for `Both` mode: mic drives the clock; loopback fills in.
/// This ensures audio always flows to the WebSocket even when speakers are silent.
fn wire_mic_clock_callback(
    mic_recorder: &mut AudioRecorder,
    loopback_buf: Arc<Mutex<Vec<f32>>>,
    manager: &ActiveRealtimeManager,
) {
    match manager {
        ActiveRealtimeManager::Soniox(m) => {
            let m = Arc::clone(m);
            mic_recorder.set_stream_frame_callback(Some(Arc::new(move |frame| {
                let mixed = mix_with_secondary_buf(&frame, &loopback_buf);
                m.push_audio_frame(mixed);
            })));
        }
        ActiveRealtimeManager::Deepgram(m) => {
            let m = Arc::clone(m);
            mic_recorder.set_stream_frame_callback(Some(Arc::new(move |frame| {
                let mixed = mix_with_secondary_buf(&frame, &loopback_buf);
                m.push_audio_frame(mixed);
            })));
        }
    }
}

/// Mix the primary (mic) frame with whatever secondary (loopback) samples are
/// buffered.  Uses a fixed `* 0.5` attenuation to prevent clipping when both
/// sources are loud.  When one source is silent the other loses 6 dB — this is
/// the standard equal-power trade-off and is intentional.  STT engines
/// (Soniox / Deepgram) normalise input internally, so the level reduction does
/// not degrade recognition quality in practice.
fn mix_with_secondary_buf(primary: &[f32], secondary_buf: &Mutex<Vec<f32>>) -> Vec<f32> {
    let secondary_samples: Vec<f32> = secondary_buf
        .lock()
        .map(|mut buf| {
            let take = primary.len().min(buf.len());
            buf.drain(..take).collect()
        })
        .unwrap_or_default();

    primary
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let s = secondary_samples.get(i).copied().unwrap_or(0.0);
            (p + s) * 0.5
        })
        .collect()
}

fn start_live_session(
    app: &AppHandle,
    settings: &AppSettings,
    recorder: &mut AudioRecorder,
) -> Result<ActiveRealtimeManager, String> {
    let provider = crate::settings::resolve_live_sound_provider(settings);
    match provider {
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

/// Opens a mic recorder for `Both` mode.
///
/// Mic drives the clock: its frame callback mixes in loopback samples and
/// pushes the result to the realtime manager. The loopback recorder's callback
/// is replaced to simply fill a ring buffer consumed by the mic callback.
///
/// This ensures audio always reaches the WebSocket even when speakers are
/// silent (loopback produces no callbacks during silence on Windows WASAPI).
fn open_mic_recorder_for_both(
    settings: &AppSettings,
    loopback_recorder: &mut AudioRecorder,
    realtime: Option<&ActiveRealtimeManager>,
) -> Result<AudioRecorder, String> {
    let Some(rt) = realtime else {
        return Err("No realtime manager — Both mode requires live streaming".to_string());
    };

    // Shared ring buffer: loopback callback writes, mic callback reads.
    let loopback_buf: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

    // Re-wire the loopback recorder to fill the buffer only — it no longer
    // pushes to the WebSocket directly.
    loopback_recorder.set_stream_frame_callback(Some(Arc::new({
        let loopback_buf = Arc::clone(&loopback_buf);
        move |frame| {
            if let Ok(mut buf) = loopback_buf.lock() {
                // Cap to ~1 second to avoid unbounded growth if speakers run fast.
                const MAX_SAMPLES: usize = 16_000;
                if buf.len() + frame.len() > MAX_SAMPLES {
                    let drop = (buf.len() + frame.len()).saturating_sub(MAX_SAMPLES);
                    buf.drain(..drop);
                }
                buf.extend_from_slice(&frame);
            }
        }
    })));

    let mic_device_name = settings
        .live_sound_microphone
        .as_deref()
        .or(settings.selected_microphone.as_deref());

    // Open the mic recorder; it drives the clock and mixes in loopback.
    let mut mic_rec = AudioRecorder::new()
        .map_err(|e| format!("Failed to create mic recorder: {}", e))?
        .with_microphone_input_boost_db(settings.microphone_input_boost_db_for_device(
            mic_device_name,
        ));

    let mic_device = resolve_device(&AudioCaptureSource::Microphone, mic_device_name);
    mic_rec
        .open_with_source(mic_device, AudioCaptureSource::Microphone)
        .map_err(|e| format!("Failed to open mic stream: {}", e))?;

    wire_mic_clock_callback(&mut mic_rec, loopback_buf, rt);

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
            settings
                .live_sound_microphone
                .clone()
                .or_else(|| settings.selected_microphone.clone()),
        ),
    };

    let mut recorder = AudioRecorder::new()
        .map_err(|e| format!("Failed to create audio recorder: {}", e))?
        .with_microphone_input_boost_db(
            if source == AudioCaptureSource::Microphone {
                settings.microphone_input_boost_db_for_device(device_name.as_deref())
            } else {
                0.0
            },
        );

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
                warn!(
                    "Both mode: mic recorder failed to open, falling back to loopback only: {}",
                    e
                );
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

    *guard = Some(LiveSoundAudioSession {
        primary_source: source,
        primary_device_name: device_name,
        mic_device_name: settings
            .live_sound_microphone
            .clone()
            .or_else(|| settings.selected_microphone.clone()),
        recorder,
        mic_recorder,
        realtime,
    });
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
pub fn stop(app: &AppHandle, session_id: u64) {
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
                crate::managers::live_sound_transcription::set_recording_if_session_matches(
                    &app, session_id, false,
                );
            });
        }

        Some(ActiveRealtimeManager::Deepgram(manager)) => {
            let app = app.clone();
            let timeout_ms = get_settings(&app).deepgram_live_finalize_timeout_ms;
            tauri::async_runtime::spawn(async move {
                if let Err(e) = manager.finish_session(timeout_ms).await {
                    warn!("Live sound Deepgram finalization error: {}", e);
                }
                crate::managers::live_sound_transcription::set_recording_if_session_matches(
                    &app, session_id, false,
                );
            });
        }

        None => {
            // Batch mode — mark as not recording immediately.
            // Full batch transcription support is a future enhancement.
            crate::managers::live_sound_transcription::set_recording_if_session_matches(
                app, session_id, false,
            );
        }
    }

    info!("Live sound audio pipeline stopped");
}

/// Returns true if a pipeline session is currently active (recording).
pub fn is_active() -> bool {
    SESSION.lock().map(|g| g.is_some()).unwrap_or(false)
}

pub fn refresh_microphone_input_boost_from_settings(app: &AppHandle) {
    if let Ok(mut guard) = SESSION.lock() {
        if let Some(session) = guard.as_mut() {
            let settings = get_settings(app);
            let primary_boost_db = if session.primary_source == AudioCaptureSource::Microphone {
                settings.microphone_input_boost_db_for_device(session.primary_device_name.as_deref())
            } else {
                0.0
            };
            session.recorder.set_microphone_input_boost_db(primary_boost_db);
            if let Some(mic_recorder) = session.mic_recorder.as_ref() {
                mic_recorder.set_microphone_input_boost_db(
                    settings.microphone_input_boost_db_for_device(
                        session.mic_device_name.as_deref(),
                    ),
                );
            }
        }
    }
}
