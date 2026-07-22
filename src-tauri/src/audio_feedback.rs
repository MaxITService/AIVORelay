use crate::settings::SoundTheme;
use crate::settings::{self, AppSettings};
use cpal::traits::{DeviceTrait, HostTrait};
use log::{debug, error, warn};
use rodio::OutputStreamBuilder;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub enum SoundType {
    Start,
    Stop,
}

/// Maximum time callers may wait for a feedback cue before continuing the
/// recording pipeline without it.
const BLOCKING_PLAY_TIMEOUT: Duration = Duration::from_secs(3);

/// A stalled output device should cost a cue, not permanently wedge the
/// long-lived feedback worker.
const PLAYBACK_STALL_TIMEOUT: Duration = Duration::from_secs(5);

fn resolve_sound_path(
    app: &AppHandle,
    settings: &AppSettings,
    sound_type: SoundType,
) -> Option<PathBuf> {
    let sound_file = get_sound_path(settings, sound_type);
    let base_dir = get_sound_base_dir(settings);
    match base_dir {
        tauri::path::BaseDirectory::AppData => {
            crate::portable::resolve_app_data(app, &sound_file).ok()
        }
        _ => app.path().resolve(&sound_file, base_dir).ok(),
    }
}

fn get_sound_path(settings: &AppSettings, sound_type: SoundType) -> String {
    match (settings.sound_theme, sound_type) {
        (SoundTheme::Custom, SoundType::Start) => "custom_start.wav".to_string(),
        (SoundTheme::Custom, SoundType::Stop) => "custom_stop.wav".to_string(),
        (_, SoundType::Start) => settings.sound_theme.to_start_path(),
        (_, SoundType::Stop) => settings.sound_theme.to_stop_path(),
    }
}

fn get_sound_base_dir(settings: &AppSettings) -> tauri::path::BaseDirectory {
    match settings.sound_theme {
        SoundTheme::Custom => tauri::path::BaseDirectory::AppData,
        _ => tauri::path::BaseDirectory::Resource,
    }
}

enum Request {
    Warm {
        device: Option<String>,
    },
    Play {
        path: PathBuf,
        device: Option<String>,
        volume: f32,
        done: Option<mpsc::Sender<()>>,
    },
}

static PLAYER: OnceLock<mpsc::Sender<Request>> = OnceLock::new();

fn player() -> &'static mpsc::Sender<Request> {
    PLAYER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("audio-feedback".into())
            .spawn(move || playback_worker(rx))
            .expect("failed to spawn audio feedback thread");
        tx
    })
}

/// Opens the selected feedback output once during startup, away from the
/// recording path. The worker owns the stream for its full lifetime so this
/// remains valid on platforms where CPAL streams are not `Send`.
pub fn init(app: &AppHandle) {
    let settings = settings::get_settings(app);
    if !settings.audio_feedback && !settings.result_ready_audio_feedback {
        return;
    }

    let _ = player().send(Request::Warm {
        device: settings.selected_output_device.clone(),
    });
}

pub fn play_feedback_sound(app: &AppHandle, sound_type: SoundType) {
    let settings = settings::get_settings(app);
    if !settings.audio_feedback {
        return;
    }
    if let Some(path) = resolve_sound_path(app, &settings, sound_type) {
        send_play(&settings, path, None);
    }
}

pub fn play_feedback_sound_blocking(app: &AppHandle, sound_type: SoundType) {
    let settings = settings::get_settings(app);
    if !settings.audio_feedback {
        return;
    }
    if let Some(path) = resolve_sound_path(app, &settings, sound_type) {
        wait_for_play(&settings, path);
    }
}

/// Plays the selected theme's start cue as a distinct "result ready" signal.
/// This setting is independent from recording start/stop feedback so users can
/// enable only the completion cue when that better suits their workflow.
pub fn play_result_ready_sound(app: &AppHandle) {
    let settings = settings::get_settings(app);
    if !settings.result_ready_audio_feedback {
        return;
    }
    if let Some(path) = resolve_sound_path(app, &settings, SoundType::Start) {
        send_play(&settings, path, None);
    }
}

pub fn play_test_sound(app: &AppHandle, sound_type: SoundType) {
    let settings = settings::get_settings(app);
    if let Some(path) = resolve_sound_path(app, &settings, sound_type) {
        wait_for_play(&settings, path);
    }
}

fn send_play(settings: &AppSettings, path: PathBuf, done: Option<mpsc::Sender<()>>) {
    let _ = player().send(Request::Play {
        path,
        device: settings.selected_output_device.clone(),
        volume: settings.audio_feedback_volume,
        done,
    });
}

fn wait_for_play(settings: &AppSettings, path: PathBuf) {
    let (tx, rx) = mpsc::channel();
    send_play(settings, path, Some(tx));
    if rx.recv_timeout(BLOCKING_PLAY_TIMEOUT).is_err() {
        warn!(
            "Audio feedback did not finish within {:?}; continuing without it",
            BLOCKING_PLAY_TIMEOUT
        );
    }
}

struct CachedStream {
    selection: Option<String>,
    default_name: Option<String>,
    stream: rodio::OutputStream,
}

fn playback_worker(rx: mpsc::Receiver<Request>) {
    let mut cached: Option<CachedStream> = None;

    while let Ok(request) = rx.recv() {
        match request {
            Request::Warm { device } => {
                ensure_stream(&mut cached, device);
            }
            Request::Play {
                path,
                device,
                volume,
                done,
            } => {
                if let Some(stream) = ensure_stream(&mut cached, device) {
                    if let Err(error) = play_on_stream(&stream.stream, &path, volume) {
                        error!(
                            "Failed to play sound '{}': {}; discarding output stream",
                            path.display(),
                            error
                        );
                        scrap_stream(&mut cached);
                    }
                }
                if let Some(done) = done {
                    let _ = done.send(());
                }
            }
        }
    }
}

/// Drop the stream on the worker that created it. This is required for CPAL's
/// non-`Send` CoreAudio and ALSA stream implementations.
fn scrap_stream(cached: &mut Option<CachedStream>) {
    *cached = None;
}

fn is_default_selection(device: &Option<String>) -> bool {
    device.as_deref().is_none_or(|name| name == "Default")
}

fn current_default_name() -> Option<String> {
    let host = crate::audio_toolkit::get_cpal_host();
    host.default_output_device()
        .and_then(|device| device.name().ok())
}

fn ensure_stream(
    cached: &mut Option<CachedStream>,
    device: Option<String>,
) -> Option<&CachedStream> {
    let default_name = if is_default_selection(&device) {
        current_default_name()
    } else {
        None
    };
    let is_stale = cached.as_ref().is_none_or(|stream| {
        stream.selection != device
            || (is_default_selection(&device) && stream.default_name != default_name)
    });

    if is_stale {
        scrap_stream(cached);
        match create_stream(device.as_deref()) {
            Ok(stream) => {
                *cached = Some(CachedStream {
                    selection: device,
                    default_name,
                    stream,
                });
            }
            Err(error) => error!("Failed to open audio feedback output stream: {}", error),
        }
    }

    cached.as_ref()
}

fn create_stream(
    device_name: Option<&str>,
) -> Result<rodio::OutputStream, Box<dyn std::error::Error>> {
    let stream_builder = if let Some(name) = device_name.filter(|name| *name != "Default") {
        let host = crate::audio_toolkit::get_cpal_host();
        let devices = host.output_devices()?;

        let mut found_device = None;
        for device in devices {
            if device.name()? == name {
                found_device = Some(device);
                break;
            }
        }

        match found_device {
            Some(device) => OutputStreamBuilder::from_device(device)?,
            None => {
                warn!("Device '{}' not found, using default device", name);
                OutputStreamBuilder::from_default_device()?
            }
        }
    } else {
        debug!("Using default device");
        OutputStreamBuilder::from_default_device()?
    };

    Ok(stream_builder.open_stream()?)
}

fn play_on_stream(
    stream: &rodio::OutputStream,
    path: &Path,
    volume: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let sink = rodio::play(stream.mixer(), BufReader::new(file))?;
    sink.set_volume(volume);

    let started = std::time::Instant::now();
    while !sink.empty() {
        if started.elapsed() > PLAYBACK_STALL_TIMEOUT {
            sink.stop();
            return Err(format!(
                "playback did not finish within {:?} (stalled output stream)",
                PLAYBACK_STALL_TIMEOUT
            )
            .into());
        }
        thread::sleep(Duration::from_millis(25));
    }

    Ok(())
}
