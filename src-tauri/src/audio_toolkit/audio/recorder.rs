use std::{
    borrow::Cow,
    io::{Error, ErrorKind},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Sample, SizedSample,
};

use crate::audio_toolkit::{
    audio::{AudioVisualiser, FrameResampler, NoiseSuppressor},
    constants,
    vad::{self, VadFrame},
    VoiceActivityDetector,
};

enum Cmd {
    Start(Instant),
    Flush {
        keep_samples: usize,
        min_samples: usize,
        reply_tx: mpsc::Sender<Vec<f32>>,
    },
    Stop(mpsc::Sender<Vec<f32>>),
    Shutdown,
}

enum AudioChunk {
    Samples(Vec<f32>),
    EndOfStream,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioCaptureSource {
    Microphone,
    SystemOutputLoopback,
}

pub type StreamFrameCallback = Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>;

pub struct AudioRecorder {
    device: Option<Device>,
    cmd_tx: Option<mpsc::Sender<Cmd>>,
    worker_handle: Option<std::thread::JoinHandle<()>>,
    vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    stream_frame_cb: Arc<Mutex<Option<StreamFrameCallback>>>,
    microphone_input_gain: Arc<Mutex<f32>>,
    microphone_noise_cancellation_enabled: Arc<AtomicBool>,
    config_cache: Arc<Mutex<Option<(AudioCaptureSource, String, cpal::SupportedStreamConfig)>>>,
}

impl AudioRecorder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(AudioRecorder {
            device: None,
            cmd_tx: None,
            worker_handle: None,
            vad: None,
            level_cb: None,
            stream_frame_cb: Arc::new(Mutex::new(None)),
            microphone_input_gain: Arc::new(Mutex::new(1.0)),
            microphone_noise_cancellation_enabled: Arc::new(AtomicBool::new(false)),
            config_cache: Arc::new(Mutex::new(None)),
        })
    }

    pub fn with_vad(mut self, vad: Box<dyn VoiceActivityDetector>) -> Self {
        self.vad = Some(Arc::new(Mutex::new(vad)));
        self
    }

    pub fn with_level_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(Vec<f32>) + Send + Sync + 'static,
    {
        self.level_cb = Some(Arc::new(cb));
        self
    }

    pub fn with_microphone_input_boost_db(self, db: f32) -> Self {
        self.set_microphone_input_boost_db(db);
        self
    }

    pub fn with_microphone_noise_cancellation_enabled(self, enabled: bool) -> Self {
        self.set_microphone_noise_cancellation_enabled(enabled);
        self
    }

    pub fn set_stream_frame_callback(&self, callback: Option<StreamFrameCallback>) {
        if let Ok(mut guard) = self.stream_frame_cb.lock() {
            *guard = callback;
        }
    }

    pub fn open(&mut self, device: Option<Device>) -> Result<(), Box<dyn std::error::Error>> {
        self.open_with_source(device, AudioCaptureSource::Microphone)
    }

    pub fn open_with_source(
        &mut self,
        device: Option<Device>,
        source: AudioCaptureSource,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.worker_handle.is_some() {
            return Ok(());
        }

        let (sample_tx, sample_rx) = mpsc::channel::<AudioChunk>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();

        let host = crate::audio_toolkit::get_cpal_host();
        let device = match device {
            Some(dev) => dev,
            None => match source {
                AudioCaptureSource::Microphone => host.default_input_device().ok_or_else(|| {
                    Error::new(std::io::ErrorKind::NotFound, "No input device found")
                })?,
                AudioCaptureSource::SystemOutputLoopback => {
                    host.default_output_device().ok_or_else(|| {
                        Error::new(std::io::ErrorKind::NotFound, "No output device found")
                    })?
                }
            },
        };

        let thread_device = device.clone();
        let vad = self.vad.clone();
        let level_cb = self.level_cb.clone();
        let stream_frame_cb = Arc::clone(&self.stream_frame_cb);
        let microphone_input_gain = Arc::clone(&self.microphone_input_gain);
        let microphone_noise_cancellation_enabled =
            Arc::clone(&self.microphone_noise_cancellation_enabled);
        let config_cache = Arc::clone(&self.config_cache);

        let worker = std::thread::spawn(move || {
            let stop_flag = Arc::new(AtomicBool::new(false));
            let stop_flag_for_stream = Arc::clone(&stop_flag);

            let init_result = (|| -> Result<(cpal::Stream, u32), String> {
                let config_started = Instant::now();
                let device_name = thread_device.name().unwrap_or_default();
                let cached_config = config_cache
                    .lock()
                    .unwrap()
                    .as_ref()
                    .filter(|(cached_source, cached_name, _)| {
                        *cached_source == source
                            && !device_name.is_empty()
                            && *cached_name == device_name
                    })
                    .map(|(_, _, config)| config.clone());
                let config_was_cached = cached_config.is_some();
                let config = match cached_config {
                    Some(config) => config,
                    None => AudioRecorder::get_preferred_config(&thread_device, source)
                        .map_err(|e| format!("Failed to get audio config: {}", e))?,
                };
                let config_elapsed = config_started.elapsed();

                let sample_rate = config.sample_rate().0;
                let channels = config.channels() as usize;

                log::info!(
                    "Using audio capture device: {:?}\nSource: {:?}\nSample rate: {}\nChannels: {}\nFormat: {:?}",
                    thread_device.name(),
                    source,
                    sample_rate,
                    channels,
                    config.sample_format()
                );

                let build_started = Instant::now();
                let stream = match config.sample_format() {
                    cpal::SampleFormat::U8 => AudioRecorder::build_stream::<u8>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                    )
                    .map_err(|e| format!("Failed to build audio stream: {}", e))?,
                    cpal::SampleFormat::I8 => AudioRecorder::build_stream::<i8>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                    )
                    .map_err(|e| format!("Failed to build audio stream: {}", e))?,
                    cpal::SampleFormat::I16 => AudioRecorder::build_stream::<i16>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                    )
                    .map_err(|e| format!("Failed to build audio stream: {}", e))?,
                    cpal::SampleFormat::I32 => AudioRecorder::build_stream::<i32>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                    )
                    .map_err(|e| format!("Failed to build audio stream: {}", e))?,
                    cpal::SampleFormat::F32 => AudioRecorder::build_stream::<f32>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                    )
                    .map_err(|e| format!("Failed to build audio stream: {}", e))?,
                    other => return Err(format!("Unsupported sample format: {:?}", other)),
                };
                let build_elapsed = build_started.elapsed();

                let play_started = Instant::now();
                stream
                    .play()
                    .map_err(|e| format!("Failed to start audio stream: {}", e))?;
                log::debug!(
                    "audio worker init ({:?}): fetch_config={:?} (cached={}) build_stream={:?} play={:?}",
                    source,
                    config_elapsed,
                    config_was_cached,
                    build_elapsed,
                    play_started.elapsed()
                );

                if !config_was_cached && !device_name.is_empty() {
                    *config_cache.lock().unwrap() = Some((source, device_name, config));
                }

                Ok((stream, sample_rate))
            })();

            match init_result {
                Ok((stream, sample_rate)) => {
                    let _ = init_tx.send(Ok(()));
                    run_consumer(
                        sample_rate,
                        vad,
                        sample_rx,
                        cmd_rx,
                        level_cb,
                        stream_frame_cb,
                        source,
                        microphone_input_gain,
                        microphone_noise_cancellation_enabled,
                        stop_flag,
                    );
                    drop(stream);
                }
                Err(error_message) => {
                    *config_cache.lock().unwrap() = None;
                    let normalized_error = normalize_capture_open_error(source, error_message);
                    log::error!("{}", normalized_error);
                    let _ = init_tx.send(Err(normalized_error));
                }
            }
        });

        match init_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {
                self.device = Some(device);
                self.cmd_tx = Some(cmd_tx);
                self.worker_handle = Some(worker);
                Ok(())
            }
            Ok(Err(error_message)) => {
                let _ = worker.join();
                let kind = if source == AudioCaptureSource::Microphone
                    && is_microphone_access_denied(&error_message)
                {
                    ErrorKind::PermissionDenied
                } else {
                    ErrorKind::Other
                };
                Err(Box::new(Error::new(kind, error_message)))
            }
            Err(_) => Err(Box::new(Error::new(
                ErrorKind::TimedOut,
                "Timeout waiting for audio device initialization",
            ))),
        }
    }

    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = &self.cmd_tx {
            tx.send(Cmd::Start(Instant::now()))?;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let (resp_tx, resp_rx) = mpsc::channel();
        if let Some(tx) = &self.cmd_tx {
            tx.send(Cmd::Stop(resp_tx))?;
        }
        Ok(resp_rx.recv()?)
    }

    pub fn flush(
        &self,
        keep_samples: usize,
        min_samples: usize,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let (resp_tx, resp_rx) = mpsc::channel();
        if let Some(tx) = &self.cmd_tx {
            tx.send(Cmd::Flush {
                keep_samples,
                min_samples,
                reply_tx: resp_tx,
            })?;
        }
        Ok(resp_rx.recv()?)
    }

    pub fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(h) = self.worker_handle.take() {
            let _ = h.join();
        }
        self.device = None;
        Ok(())
    }

    pub fn set_vad_threshold(&self, threshold: f32) {
        if let Some(vad) = &self.vad {
            vad.lock().unwrap().set_threshold(threshold);
        }
    }

    pub fn set_microphone_input_boost_db(&self, db: f32) {
        if let Ok(mut gain) = self.microphone_input_gain.lock() {
            *gain = microphone_input_gain_from_db(db);
        }
    }

    pub fn set_microphone_noise_cancellation_enabled(&self, enabled: bool) {
        self.microphone_noise_cancellation_enabled
            .store(enabled, Ordering::Relaxed);
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        sample_tx: mpsc::Sender<AudioChunk>,
        channels: usize,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<cpal::Stream, cpal::BuildStreamError>
    where
        T: Sample + SizedSample + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        let mut output_buffer = Vec::new();
        let mut eos_sent = false;

        let stream_cb = move |data: &[T], _: &cpal::InputCallbackInfo| {
            if stop_flag.load(Ordering::Relaxed) {
                if !eos_sent {
                    let _ = sample_tx.send(AudioChunk::EndOfStream);
                    eos_sent = true;
                }
                return;
            }
            eos_sent = false;

            output_buffer.clear();

            if channels == 1 {
                output_buffer.extend(data.iter().map(|&sample| sample.to_sample::<f32>()));
            } else {
                let frame_count = data.len() / channels;
                output_buffer.reserve(frame_count);

                for frame in data.chunks_exact(channels) {
                    let mono_sample = frame
                        .iter()
                        .map(|&sample| sample.to_sample::<f32>())
                        .sum::<f32>()
                        / channels as f32;
                    output_buffer.push(mono_sample);
                }
            }

            if sample_tx
                .send(AudioChunk::Samples(output_buffer.clone()))
                .is_err()
            {
                log::error!("Failed to send samples");
            }
        };

        device.build_input_stream(
            &config.clone().into(),
            stream_cb,
            |err| log::error!("Stream error: {}", err),
            None,
        )
    }

    fn get_preferred_config(
        device: &cpal::Device,
        source: AudioCaptureSource,
    ) -> Result<cpal::SupportedStreamConfig, Box<dyn std::error::Error>> {
        let supported_configs: Vec<cpal::SupportedStreamConfigRange> = match source {
            AudioCaptureSource::Microphone => {
                let default_config = device.default_input_config()?;
                let target_rate = default_config.sample_rate();

                let supported_configs: Vec<cpal::SupportedStreamConfigRange> = match device
                    .supported_input_configs()
                {
                    Ok(configs) => configs.collect(),
                    Err(e) => {
                        log::warn!(
                                "Could not enumerate microphone input configs ({e}), using device default"
                            );
                        return Ok(default_config);
                    }
                };

                let mut best_config: Option<cpal::SupportedStreamConfigRange> = None;

                for config_range in supported_configs {
                    if config_range.min_sample_rate() <= target_rate
                        && config_range.max_sample_rate() >= target_rate
                    {
                        match best_config {
                            None => best_config = Some(config_range),
                            Some(ref current) => {
                                let score = |fmt: cpal::SampleFormat| match fmt {
                                    cpal::SampleFormat::F32 => 4,
                                    cpal::SampleFormat::I16 => 3,
                                    cpal::SampleFormat::I32 => 2,
                                    _ => 1,
                                };

                                if score(config_range.sample_format())
                                    > score(current.sample_format())
                                {
                                    best_config = Some(config_range);
                                }
                            }
                        }
                    }
                }

                if let Some(config) = best_config {
                    return Ok(config.with_sample_rate(target_rate));
                }

                log::warn!(
                    "No microphone config matched device default rate {:?}, using default config",
                    target_rate
                );
                return Ok(default_config);
            }
            AudioCaptureSource::SystemOutputLoopback => {
                device.supported_output_configs()?.collect()
            }
        };
        let mut best_config: Option<cpal::SupportedStreamConfigRange> = None;

        for config_range in supported_configs {
            if config_range.min_sample_rate().0 <= constants::WHISPER_SAMPLE_RATE
                && config_range.max_sample_rate().0 >= constants::WHISPER_SAMPLE_RATE
            {
                match best_config {
                    None => best_config = Some(config_range),
                    Some(ref current) => {
                        let score = |fmt: cpal::SampleFormat| match fmt {
                            cpal::SampleFormat::F32 => 4,
                            cpal::SampleFormat::I16 => 3,
                            cpal::SampleFormat::I32 => 2,
                            _ => 1,
                        };

                        if score(config_range.sample_format()) > score(current.sample_format()) {
                            best_config = Some(config_range);
                        }
                    }
                }
            }
        }

        if let Some(config) = best_config {
            return Ok(config.with_sample_rate(cpal::SampleRate(constants::WHISPER_SAMPLE_RATE)));
        }

        Ok(match source {
            AudioCaptureSource::Microphone => device.default_input_config()?,
            AudioCaptureSource::SystemOutputLoopback => device.default_output_config()?,
        })
    }
}

pub fn is_microphone_access_denied(error_message: &str) -> bool {
    let normalized = error_message.to_lowercase();
    normalized.contains("access is denied")
        || normalized.contains("permission denied")
        || normalized.contains("0x80070005")
}

pub fn is_no_input_device_error(error_message: &str) -> bool {
    let normalized = error_message.to_lowercase();
    normalized.contains("no input device found")
        || (normalized.contains("failed to fetch preferred config")
            && normalized.contains("coreaudio"))
}

fn normalize_capture_open_error(source: AudioCaptureSource, error_message: String) -> String {
    if source == AudioCaptureSource::Microphone && is_microphone_access_denied(&error_message) {
        return "Microphone access was denied by Windows. Enable Settings > Privacy & security > Microphone, make sure desktop app access is allowed, then restart the app.".to_string();
    }

    error_message
}

fn visualizer_window_size(sample_rate: u32) -> usize {
    let target_window = (f64::from(sample_rate) / 30.0).round() as usize;
    [256usize, 512, 1024, 2048]
        .into_iter()
        .min_by_key(|window| window.abs_diff(target_window))
        .unwrap_or(512)
}

#[cfg(test)]
mod tests {
    use super::{is_microphone_access_denied, is_no_input_device_error};
    use crate::audio_toolkit::constants;

    #[test]
    fn detects_access_is_denied() {
        assert!(is_microphone_access_denied("Access is denied"));
    }

    #[test]
    fn detects_permission_denied() {
        assert!(is_microphone_access_denied("permission denied"));
    }

    #[test]
    fn detects_windows_error_code() {
        assert!(is_microphone_access_denied("WASAPI error: 0x80070005"));
    }

    #[test]
    fn does_not_match_unrelated_errors() {
        assert!(!is_microphone_access_denied("device not found"));
    }

    #[test]
    fn detects_no_input_device() {
        assert!(is_no_input_device_error("No input device found"));
    }

    #[test]
    fn detects_coreaudio_config_error() {
        assert!(is_no_input_device_error(
            "Failed to fetch preferred config: A backend-specific error has occurred: An unknown error unknown to the coreaudio-rs API occurred"
        ));
    }

    #[test]
    fn does_not_match_other_errors_for_no_device() {
        assert!(!is_no_input_device_error("permission denied"));
        assert!(!is_no_input_device_error("device not found"));
    }

    #[test]
    fn microphone_input_boost_defaults_to_no_gain() {
        assert_eq!(super::microphone_input_gain_from_db(0.0), 1.0);
        assert_eq!(super::microphone_input_gain_from_db(-5.0), 1.0);
    }

    #[test]
    fn microphone_input_boost_clamps_to_supported_range() {
        let gain = super::microphone_input_gain_from_db(60.0);
        let expected = 10f32.powf(constants::MAX_MICROPHONE_INPUT_BOOST_DB / 20.0);
        assert!((gain - expected).abs() < 0.0001);
    }

    #[test]
    fn visualizer_window_scales_to_sample_rate() {
        assert_eq!(super::visualizer_window_size(8_000), 256);
        assert_eq!(super::visualizer_window_size(16_000), 512);
        assert_eq!(super::visualizer_window_size(48_000), 2048);
    }
}

fn handle_frame(
    samples: &[f32],
    recording: bool,
    vad: &Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    out_buf: &mut Vec<f32>,
) {
    if !recording {
        return;
    }

    if let Some(vad_arc) = vad {
        let mut det = vad_arc.lock().unwrap();
        match det.push_frame(samples).unwrap_or(VadFrame::Speech(samples)) {
            VadFrame::Speech(buf) => out_buf.extend_from_slice(buf),
            VadFrame::Noise => {}
        }
    } else {
        out_buf.extend_from_slice(samples);
    }
}

fn emit_stream_frame(stream_frame_cb: &Arc<Mutex<Option<StreamFrameCallback>>>, samples: &[f32]) {
    let callback = stream_frame_cb.lock().ok().and_then(|guard| guard.clone());
    if let Some(callback) = callback {
        callback(samples.to_vec());
    }
}

fn microphone_input_gain_from_db(db: f32) -> f32 {
    let sanitized = if db.is_finite() {
        db.clamp(0.0, constants::MAX_MICROPHONE_INPUT_BOOST_DB)
    } else {
        0.0
    };
    if sanitized <= 0.0 {
        1.0
    } else {
        10f32.powf(sanitized / 20.0)
    }
}

fn apply_input_gain_if_needed<'a>(
    samples: &'a [f32],
    source: AudioCaptureSource,
    microphone_input_gain: &Arc<Mutex<f32>>,
) -> Cow<'a, [f32]> {
    if source != AudioCaptureSource::Microphone {
        return Cow::Borrowed(samples);
    }

    let gain = microphone_input_gain
        .lock()
        .map(|guard| *guard)
        .unwrap_or(1.0);
    if (gain - 1.0).abs() <= f32::EPSILON {
        return Cow::Borrowed(samples);
    }

    Cow::Owned(
        samples
            .iter()
            .map(|sample| (*sample * gain).clamp(-1.0, 1.0))
            .collect(),
    )
}

fn apply_noise_cancellation_if_needed<'a>(
    samples: Cow<'a, [f32]>,
    source: AudioCaptureSource,
    microphone_noise_cancellation_enabled: &Arc<AtomicBool>,
    noise_suppressor: &mut Option<NoiseSuppressor>,
) -> Cow<'a, [f32]> {
    if source != AudioCaptureSource::Microphone
        || !microphone_noise_cancellation_enabled.load(Ordering::Relaxed)
    {
        return samples;
    }

    if noise_suppressor.is_none() {
        match NoiseSuppressor::new_16khz() {
            Ok(suppressor) => *noise_suppressor = Some(suppressor),
            Err(err) => {
                log::warn!("Failed to initialize RNNoise noise cancellation: {err}");
                return samples;
            }
        }
    }

    match noise_suppressor.as_mut() {
        Some(suppressor) => Cow::Owned(suppressor.process_16khz_frame(samples.as_ref())),
        None => samples,
    }
}

fn process_consumer_cmd(
    cmd: Cmd,
    recording: &mut bool,
    processed_samples: &mut Vec<f32>,
    pending_chunk: Option<&mut Option<AudioChunk>>,
    sample_rx: &mpsc::Receiver<AudioChunk>,
    frame_resampler: &mut FrameResampler,
    vad: &Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    stream_frame_cb: &Arc<Mutex<Option<StreamFrameCallback>>>,
    visualizer: &mut AudioVisualiser,
    source: AudioCaptureSource,
    microphone_input_gain: &Arc<Mutex<f32>>,
    microphone_noise_cancellation_enabled: &Arc<AtomicBool>,
    noise_suppressor: &mut Option<NoiseSuppressor>,
    stop_flag: &Arc<AtomicBool>,
) -> bool {
    match cmd {
        Cmd::Start(sent_at) => {
            log::debug!(
                "Cmd::Start processed {:?} after send; capture begins with the in-flight chunk",
                sent_at.elapsed()
            );
            stop_flag.store(false, Ordering::Relaxed);
            processed_samples.clear();
            *noise_suppressor = None;
            *recording = true;
            visualizer.reset();
            frame_resampler.reset();
            if let Some(v) = vad {
                v.lock().unwrap().reset();
            }
            false
        }
        Cmd::Flush {
            keep_samples,
            min_samples,
            reply_tx,
        } => {
            if !*recording {
                let _ = reply_tx.send(Vec::new());
                return false;
            }

            let flushable_len = processed_samples.len().saturating_sub(keep_samples);
            if flushable_len < min_samples {
                let _ = reply_tx.send(Vec::new());
                return false;
            }

            let flushed: Vec<f32> = processed_samples.drain(..flushable_len).collect();
            let _ = reply_tx.send(flushed);
            false
        }
        Cmd::Stop(reply_tx) => {
            *recording = false;
            stop_flag.store(true, Ordering::Relaxed);

            if let Some(Some(AudioChunk::Samples(remaining))) =
                pending_chunk.map(|pending| pending.take())
            {
                frame_resampler.push(&remaining, &mut |frame: &[f32]| {
                    let adjusted = apply_input_gain_if_needed(frame, source, microphone_input_gain);
                    let enhanced = apply_noise_cancellation_if_needed(
                        adjusted,
                        source,
                        microphone_noise_cancellation_enabled,
                        noise_suppressor,
                    );
                    emit_stream_frame(stream_frame_cb, enhanced.as_ref());
                    handle_frame(enhanced.as_ref(), true, vad, processed_samples)
                });
            }

            loop {
                match sample_rx.recv_timeout(Duration::from_secs(2)) {
                    Ok(AudioChunk::Samples(remaining)) => {
                        frame_resampler.push(&remaining, &mut |frame: &[f32]| {
                            let adjusted =
                                apply_input_gain_if_needed(frame, source, microphone_input_gain);
                            let enhanced = apply_noise_cancellation_if_needed(
                                adjusted,
                                source,
                                microphone_noise_cancellation_enabled,
                                noise_suppressor,
                            );
                            emit_stream_frame(stream_frame_cb, enhanced.as_ref());
                            handle_frame(enhanced.as_ref(), true, vad, processed_samples)
                        });
                    }
                    Ok(AudioChunk::EndOfStream) => break,
                    Err(_) => {
                        log::warn!("Timed out waiting for EndOfStream from audio callback");
                        break;
                    }
                }
            }

            frame_resampler.finish(&mut |frame: &[f32]| {
                let adjusted = apply_input_gain_if_needed(frame, source, microphone_input_gain);
                let enhanced = apply_noise_cancellation_if_needed(
                    adjusted,
                    source,
                    microphone_noise_cancellation_enabled,
                    noise_suppressor,
                );
                emit_stream_frame(stream_frame_cb, enhanced.as_ref());
                handle_frame(enhanced.as_ref(), true, vad, processed_samples)
            });

            let _ = reply_tx.send(std::mem::take(processed_samples));
            *noise_suppressor = None;

            stop_flag.store(false, Ordering::Relaxed);
            false
        }
        Cmd::Shutdown => {
            stop_flag.store(true, Ordering::Relaxed);
            true
        }
    }
}

fn run_consumer(
    in_sample_rate: u32,
    vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    sample_rx: mpsc::Receiver<AudioChunk>,
    cmd_rx: mpsc::Receiver<Cmd>,
    level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    stream_frame_cb: Arc<Mutex<Option<StreamFrameCallback>>>,
    source: AudioCaptureSource,
    microphone_input_gain: Arc<Mutex<f32>>,
    microphone_noise_cancellation_enabled: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
) {
    let mut frame_resampler = FrameResampler::new(
        in_sample_rate as usize,
        constants::WHISPER_SAMPLE_RATE as usize,
        Duration::from_millis(30),
    );

    let mut processed_samples = Vec::<f32>::new();
    let mut recording = false;
    let mut noise_suppressor: Option<NoiseSuppressor> = None;

    const BUCKETS: usize = 16;
    let window_size = visualizer_window_size(in_sample_rate);
    let mut visualizer = AudioVisualiser::new(in_sample_rate, window_size, BUCKETS, 400.0, 4000.0);

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            if process_consumer_cmd(
                cmd,
                &mut recording,
                &mut processed_samples,
                None,
                &sample_rx,
                &mut frame_resampler,
                &vad,
                &stream_frame_cb,
                &mut visualizer,
                source,
                &microphone_input_gain,
                &microphone_noise_cancellation_enabled,
                &mut noise_suppressor,
                &stop_flag,
            ) {
                return;
            }
        }

        let chunk = match sample_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(chunk) => chunk,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };

        let mut pending_chunk = Some(chunk);

        while let Ok(cmd) = cmd_rx.try_recv() {
            if process_consumer_cmd(
                cmd,
                &mut recording,
                &mut processed_samples,
                Some(&mut pending_chunk),
                &sample_rx,
                &mut frame_resampler,
                &vad,
                &stream_frame_cb,
                &mut visualizer,
                source,
                &microphone_input_gain,
                &microphone_noise_cancellation_enabled,
                &mut noise_suppressor,
                &stop_flag,
            ) {
                return;
            }
        }

        let raw = match pending_chunk.take() {
            Some(AudioChunk::Samples(samples)) => samples,
            _ => continue,
        };

        if let Some(buckets) = visualizer.feed(&raw) {
            if let Some(cb) = &level_cb {
                cb(buckets);
            }
        }

        frame_resampler.push(&raw, &mut |frame: &[f32]| {
            let adjusted = apply_input_gain_if_needed(frame, source, &microphone_input_gain);
            if recording {
                let enhanced = apply_noise_cancellation_if_needed(
                    adjusted,
                    source,
                    &microphone_noise_cancellation_enabled,
                    &mut noise_suppressor,
                );
                emit_stream_frame(&stream_frame_cb, enhanced.as_ref());
                handle_frame(enhanced.as_ref(), true, &vad, &mut processed_samples)
            } else {
                handle_frame(adjusted.as_ref(), false, &vad, &mut processed_samples)
            }
        });
    }
}
