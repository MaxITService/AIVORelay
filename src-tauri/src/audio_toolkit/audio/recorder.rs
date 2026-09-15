use std::{
    borrow::Cow,
    io::{Error, ErrorKind},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Sample, SizedSample,
};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::audio_toolkit::{
    audio::{AudioVisualiser, FrameResampler, NoiseSuppressor},
    constants,
    vad::{self, VadFrame},
    VoiceActivityDetector,
};

const CONTROL_REPLY_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const AUDIO_RING_SECONDS: usize = 2;
const CONSUMER_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_DRAIN_CHUNK: Duration = Duration::from_millis(50);
const PAUSE_ACK_TIMEOUT: Duration = Duration::from_secs(2);

enum Cmd {
    /// Begin capturing and acknowledge only after the first real audio chunk
    /// has passed through the active capture pipeline.
    Start(Instant, mpsc::Sender<()>),
    Flush {
        keep_samples: usize,
        min_samples: usize,
        reply_tx: mpsc::Sender<Vec<f32>>,
    },
    Stop(mpsc::Sender<Vec<f32>>),
    Shutdown,
}

/// Atomics shared by the callback and consumer; audio samples travel through a
/// wait-free single-producer/single-consumer ring.
#[derive(Default)]
struct CaptureTransportState {
    pause_requested: AtomicBool,
    pause_acknowledged: AtomicBool,
    overrun_samples: AtomicU64,
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
    /// Which microphone input channel to use. None averages all channels.
    selected_channel: Option<usize>,
    config_cache: Arc<Mutex<Option<(AudioCaptureSource, String, cpal::SupportedStreamConfig)>>>,
    /// Set by cpal's asynchronous stream error callback when capture must be rebuilt.
    stream_error: Arc<AtomicBool>,
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
            selected_channel: None,
            config_cache: Arc::new(Mutex::new(None)),
            stream_error: Arc::new(AtomicBool::new(false)),
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

    pub fn with_selected_channel(mut self, channel: Option<u16>) -> Self {
        self.set_selected_channel(channel);
        self
    }

    pub fn set_selected_channel(&mut self, channel: Option<u16>) {
        self.selected_channel = channel.map(usize::from);
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
            if !self.needs_reopen() {
                return Ok(()); // already open
            }
            log::warn!("Capture stream failed; rebuilding audio stream");
            self.close()?;
        }

        self.stream_error.store(false, Ordering::Relaxed);

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
        let selected_channel = if source == AudioCaptureSource::Microphone {
            self.selected_channel
        } else {
            None
        };
        let config_cache = Arc::clone(&self.config_cache);
        let stream_error = Arc::clone(&self.stream_error);

        let worker = std::thread::spawn(move || {
            let transport = Arc::new(CaptureTransportState::default());

            let init_result = (|| -> Result<(cpal::Stream, u32, Consumer<f32>), String> {
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
                let (stream, sample_consumer) = match config.sample_format() {
                    cpal::SampleFormat::U8 => AudioRecorder::build_stream::<u8>(
                        &thread_device,
                        &config,
                        channels,
                        selected_channel,
                        Arc::clone(&transport),
                        Arc::clone(&stream_error),
                    ),
                    cpal::SampleFormat::I8 => AudioRecorder::build_stream::<i8>(
                        &thread_device,
                        &config,
                        channels,
                        selected_channel,
                        Arc::clone(&transport),
                        Arc::clone(&stream_error),
                    ),
                    cpal::SampleFormat::I16 => AudioRecorder::build_stream::<i16>(
                        &thread_device,
                        &config,
                        channels,
                        selected_channel,
                        Arc::clone(&transport),
                        Arc::clone(&stream_error),
                    ),
                    cpal::SampleFormat::I32 => AudioRecorder::build_stream::<i32>(
                        &thread_device,
                        &config,
                        channels,
                        selected_channel,
                        Arc::clone(&transport),
                        Arc::clone(&stream_error),
                    ),
                    cpal::SampleFormat::F32 => AudioRecorder::build_stream::<f32>(
                        &thread_device,
                        &config,
                        channels,
                        selected_channel,
                        Arc::clone(&transport),
                        Arc::clone(&stream_error),
                    ),
                    other => return Err(format!("Unsupported sample format: {:?}", other)),
                }
                .map_err(|e| format!("Failed to build audio stream: {}", e))?;
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

                Ok((stream, sample_rate, sample_consumer))
            })();

            match init_result {
                Ok((stream, sample_rate, sample_consumer)) => {
                    let _ = init_tx.send(Ok(()));
                    run_consumer(
                        sample_rate,
                        vad,
                        sample_consumer,
                        cmd_rx,
                        level_cb,
                        stream_frame_cb,
                        source,
                        microphone_input_gain,
                        microphone_noise_cancellation_enabled,
                        transport,
                        Arc::clone(&stream_error),
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

    pub fn start(&self) -> Result<mpsc::Receiver<()>, Box<dyn std::error::Error>> {
        let tx = self
            .cmd_tx
            .as_ref()
            .ok_or_else(|| Error::other("Recorder is not open"))?;
        let (ready_tx, ready_rx) = mpsc::channel();
        tx.send(Cmd::Start(Instant::now(), ready_tx))?;
        Ok(ready_rx)
    }

    pub fn stop(&self) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let tx = self
            .cmd_tx
            .as_ref()
            .ok_or_else(|| Error::other("Recorder is not open"))?;
        let (resp_tx, resp_rx) = mpsc::channel();
        tx.send(Cmd::Stop(resp_tx))?;
        self.receive_audio_reply(resp_rx, "stop")
    }

    pub fn flush(
        &self,
        keep_samples: usize,
        min_samples: usize,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let tx = self
            .cmd_tx
            .as_ref()
            .ok_or_else(|| Error::other("Recorder is not open"))?;
        let (resp_tx, resp_rx) = mpsc::channel();
        tx.send(Cmd::Flush {
            keep_samples,
            min_samples,
            reply_tx: resp_tx,
        })?;
        self.receive_audio_reply(resp_rx, "flush")
    }

    fn receive_audio_reply(
        &self,
        receiver: mpsc::Receiver<Vec<f32>>,
        operation: &str,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        receiver.recv_timeout(CONTROL_REPLY_TIMEOUT).map_err(|error| {
            self.stream_error.store(true, Ordering::Relaxed);
            let message = format!("Audio recorder {} did not complete: {}", operation, error);
            log::error!("{}", message);
            let kind = match error {
                mpsc::RecvTimeoutError::Timeout => ErrorKind::TimedOut,
                mpsc::RecvTimeoutError::Disconnected => ErrorKind::BrokenPipe,
            };
            Box::new(Error::new(kind, message)) as Box<dyn std::error::Error>
        })
    }

    /// True when the active capture stream must be rebuilt.
    ///
    /// Some backends report a device disconnect through the error callback
    /// without closing the callback channel, so also honor its explicit flag.
    pub fn needs_reopen(&self) -> bool {
        self.stream_error.load(Ordering::Relaxed)
            || self
                .worker_handle
                .as_ref()
                .is_some_and(|handle| handle.is_finished())
    }

    pub fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(Cmd::Shutdown);
        }
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        while self
            .worker_handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
        {
            if Instant::now() >= deadline {
                self.stream_error.store(true, Ordering::Relaxed);
                log::error!("Audio worker did not shut down within {:?}", SHUTDOWN_TIMEOUT);
                // Keep ownership of this worker. Reopening must not create a
                // second device worker while the previous one is still alive.
                return Err(Box::new(Error::new(
                    ErrorKind::TimedOut,
                    "Audio worker is still shutting down",
                )));
            }
            std::thread::sleep(Duration::from_millis(10));
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
        channels: usize,
        selected_channel: Option<usize>,
        transport: Arc<CaptureTransportState>,
        stream_error: Arc<AtomicBool>,
    ) -> Result<(cpal::Stream, Consumer<f32>), cpal::BuildStreamError>
    where
        T: Sample + SizedSample + Copy + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        let ring_capacity = config.sample_rate().0 as usize * AUDIO_RING_SECONDS;
        let (mut sample_producer, mut sample_consumer) = RingBuffer::new(ring_capacity);

        // Fault in rtrb's backing pages before the device starts calling us.
        // This is outside the real-time callback and does not pin memory.
        {
            let chunk = sample_producer
                .write_chunk(ring_capacity)
                .expect("new audio ring has its full capacity available");
            chunk.commit_all();
        }
        {
            let chunk = sample_consumer
                .read_chunk(ring_capacity)
                .expect("pre-filled audio ring is readable");
            chunk.commit_all();
        }

        let use_channel = selected_channel.filter(|channel| *channel < channels);
        let callback_transport = Arc::clone(&transport);

        let stream_cb = move |data: &[T], _: &cpal::InputCallbackInfo| {
            AudioRecorder::write_input_to_ring(
                data,
                channels,
                use_channel,
                &mut sample_producer,
                &callback_transport,
            );
        };

        let stream = device.build_input_stream(
            &config.clone().into(),
            stream_cb,
            move |_err| {
                // Some backends invoke this on their audio thread. Defer
                // logging and recovery to the consumer/manager path.
                stream_error.store(true, Ordering::Release);
            },
            None,
        )?;
        Ok((stream, sample_consumer))
    }

    /// Real-time callback body: no allocation, locks, logging, clocks, or
    /// blocking operations. The first block that observes a pause is retained
    /// as boundary audio; subsequent callbacks stay silent until resumed.
    fn write_input_to_ring<T>(
        data: &[T],
        channels: usize,
        use_channel: Option<usize>,
        producer: &mut Producer<f32>,
        transport: &CaptureTransportState,
    ) where
        T: Sample + SizedSample + Copy,
        f32: cpal::FromSample<T>,
    {
        if channels == 0 {
            return;
        }

        if transport.pause_requested.load(Ordering::Acquire)
            && transport.pause_acknowledged.load(Ordering::Acquire)
        {
            return;
        }

        let frame_count = data.len() / channels;
        let writable_frames = producer.slots().min(frame_count);
        let written = if writable_frames == 0 {
            0
        } else {
            let chunk = producer
                .write_chunk_uninit(writable_frames)
                .expect("the producer just reported this many writable slots");
            if channels == 1 {
                chunk.fill_from_iter(
                    data.iter()
                        .take(writable_frames)
                        .map(|&sample| sample.to_sample::<f32>()),
                )
            } else if let Some(channel) = use_channel {
                chunk.fill_from_iter(
                    data.chunks_exact(channels)
                        .take(writable_frames)
                        .map(|frame| frame[channel].to_sample::<f32>()),
                )
            } else {
                chunk.fill_from_iter(data.chunks_exact(channels).take(writable_frames).map(
                    |frame| {
                        frame
                            .iter()
                            .map(|&sample| sample.to_sample::<f32>())
                            .sum::<f32>()
                            / channels as f32
                    },
                ))
            }
        };
        debug_assert_eq!(written, writable_frames);

        let dropped = frame_count - written;
        if dropped > 0 {
            transport
                .overrun_samples
                .fetch_add(dropped as u64, Ordering::Relaxed);
        }

        if transport.pause_requested.load(Ordering::Acquire) {
            transport.pause_acknowledged.store(true, Ordering::Release);
        }
    }

    pub fn preferred_input_channel_count(
        device: &cpal::Device,
    ) -> Result<u16, Box<dyn std::error::Error>> {
        Ok(Self::get_preferred_config(device, AudioCaptureSource::Microphone)?.channels())
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
    use super::{
        drain_available_samples, is_microphone_access_denied, is_no_input_device_error,
        run_consumer, AudioCaptureSource, AudioRecorder, CaptureTransportState, Cmd,
    };
    use crate::audio_toolkit::constants;
    use rtrb::RingBuffer;
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc, Mutex,
        },
        thread,
        time::Duration,
    };

    #[test]
    fn callback_writes_mono_samples() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
        let transport = CaptureTransportState::default();

        AudioRecorder::write_input_to_ring(
            &[0.25f32, -0.5, 1.0],
            1,
            None,
            &mut producer,
            &transport,
        );

        let mut output = [0.0; 3];
        consumer.pop_entire_slice(&mut output).expect("samples");
        assert_eq!(output, [0.25, -0.5, 1.0]);
    }

    #[test]
    fn callback_downmixes_or_selects_multichannel_input() {
        let transport = CaptureTransportState::default();
        let (mut average_tx, mut average_rx) = RingBuffer::<f32>::new(4);
        AudioRecorder::write_input_to_ring(
            &[1.0f32, 3.0, -1.0, 1.0],
            2,
            None,
            &mut average_tx,
            &transport,
        );
        let mut averaged = [0.0; 2];
        average_rx
            .pop_entire_slice(&mut averaged)
            .expect("averaged samples");
        assert_eq!(averaged, [2.0, 0.0]);

        let (mut selected_tx, mut selected_rx) = RingBuffer::<f32>::new(4);
        AudioRecorder::write_input_to_ring(
            &[1.0f32, 3.0, -1.0, 1.0],
            2,
            Some(1),
            &mut selected_tx,
            &transport,
        );
        let mut selected = [0.0; 2];
        selected_rx
            .pop_entire_slice(&mut selected)
            .expect("selected samples");
        assert_eq!(selected, [3.0, 1.0]);
    }

    #[test]
    fn callback_forwards_boundary_block_then_stays_silent_until_resumed() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
        let transport = CaptureTransportState::default();

        transport.pause_requested.store(true, Ordering::Release);
        AudioRecorder::write_input_to_ring(&[1.0f32, 2.0], 1, None, &mut producer, &transport);
        assert!(transport.pause_acknowledged.load(Ordering::Acquire));
        assert_eq!(consumer.slots(), 2);

        AudioRecorder::write_input_to_ring(&[3.0f32], 1, None, &mut producer, &transport);
        assert_eq!(consumer.slots(), 2);
        assert_eq!(transport.overrun_samples.load(Ordering::Relaxed), 0);

        transport.pause_acknowledged.store(false, Ordering::Relaxed);
        transport.pause_requested.store(false, Ordering::Release);
        AudioRecorder::write_input_to_ring(&[4.0f32], 1, None, &mut producer, &transport);
        let mut output = [0.0; 3];
        consumer.pop_entire_slice(&mut output).expect("samples");
        assert_eq!(output, [1.0, 2.0, 4.0]);
    }

    #[test]
    fn callback_partially_fills_ring_and_counts_dropped_audio() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(2);
        let transport = CaptureTransportState::default();

        AudioRecorder::write_input_to_ring(&[1.0f32, 2.0, 3.0], 1, None, &mut producer, &transport);

        let mut captured = [0.0; 2];
        consumer
            .pop_entire_slice(&mut captured)
            .expect("partial callback audio");
        assert_eq!(captured, [1.0, 2.0]);
        assert_eq!(transport.overrun_samples.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn bounded_drain_leaves_remaining_samples_for_next_command_cycle() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
        producer
            .push_entire_slice(&[1.0, 2.0, 3.0, 4.0, 5.0])
            .expect("samples");
        let mut drained = Vec::new();

        let count =
            drain_available_samples(&mut consumer, 3, |part| drained.extend_from_slice(part));

        assert_eq!(count, 3);
        assert_eq!(drained, [1.0, 2.0, 3.0]);
        assert_eq!(consumer.slots(), 2);
    }

    #[test]
    fn shutdown_is_processed_without_audio_samples() {
        let (_producer, consumer) = RingBuffer::<f32>::new(48_000);
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_consumer(
                48_000,
                None,
                consumer,
                cmd_rx,
                None,
                Arc::new(Mutex::new(None)),
                AudioCaptureSource::Microphone,
                Arc::new(Mutex::new(1.0)),
                Arc::new(AtomicBool::new(false)),
                Arc::new(CaptureTransportState::default()),
                Arc::new(AtomicBool::new(false)),
            );
            let _ = done_tx.send(());
        });

        cmd_tx.send(Cmd::Shutdown).expect("send shutdown");
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("consumer shutdown");
        worker.join().expect("consumer worker");
    }

    #[test]
    fn unopened_recorder_does_not_need_reopen() {
        // No worker has been spawned yet, so there is nothing to reap. Guards
        // against inverting the "no worker" case, which would make every first
        // open() take the rebuild path.
        let recorder = AudioRecorder::new().expect("recorder");
        assert!(!recorder.needs_reopen());
    }

    #[test]
    fn stream_error_requires_reopen() {
        let recorder = AudioRecorder::new().expect("recorder");
        recorder.stream_error.store(true, Ordering::Relaxed);
        assert!(recorder.needs_reopen());
    }

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

    #[test]
    fn vad_framer_reframes_enhanced_480_sample_chunks_for_earshot() {
        let mut framer = super::FrameResampler::new(
            constants::WHISPER_SAMPLE_RATE as usize,
            constants::WHISPER_SAMPLE_RATE as usize,
            std::time::Duration::from_millis(16),
        );
        let mut frames = Vec::new();
        framer.push(&[0.25; 480], |frame| frames.push(frame.to_vec()));
        framer.push(&[0.5; 480], |frame| frames.push(frame.to_vec()));
        framer.finish(|frame| frames.push(frame.to_vec()));

        assert_eq!(frames.len(), 4);
        assert!(frames[..3].iter().all(|frame| frame.len() == 256));
        assert_eq!(frames[0], vec![0.25; 256]);
        assert!(frames[1][..224].iter().all(|sample| *sample == 0.25));
        assert!(frames[1][224..].iter().all(|sample| *sample == 0.5));
        assert!(frames[3][..192].iter().all(|sample| *sample == 0.5));
        assert!(frames[3][192..].iter().all(|sample| *sample == 0.0));
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

/// Feed an enhanced 16 kHz frame into the detector-sized VAD framer. The
/// stream callback is intentionally invoked by the caller before this stage,
/// so live providers continue to receive enhanced 480-sample frames.
fn handle_enhanced_frame(
    samples: &[f32],
    vad: &Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    vad_frame_resampler: &mut Option<FrameResampler>,
    out_buf: &mut Vec<f32>,
) {
    if let Some(resampler) = vad_frame_resampler.as_mut() {
        resampler.push(samples, |frame: &[f32]| {
            handle_frame(frame, true, vad, out_buf)
        });
    } else {
        out_buf.extend_from_slice(samples);
    }
}

fn process_enhanced_capture_frame(
    frame: &[f32],
    vad: &Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    vad_frame_resampler: &mut Option<FrameResampler>,
    stream_frame_cb: &Arc<Mutex<Option<StreamFrameCallback>>>,
    source: AudioCaptureSource,
    microphone_input_gain: &Arc<Mutex<f32>>,
    microphone_noise_cancellation_enabled: &Arc<AtomicBool>,
    noise_suppressor: &mut Option<NoiseSuppressor>,
    processed_samples: &mut Vec<f32>,
) {
    let adjusted = apply_input_gain_if_needed(frame, source, microphone_input_gain);
    let enhanced = apply_noise_cancellation_if_needed(
        adjusted,
        source,
        microphone_noise_cancellation_enabled,
        noise_suppressor,
    );
    // Live streaming consumers must continue to see the enhanced main frame,
    // before recording-side silence filtering is applied.
    emit_stream_frame(stream_frame_cb, enhanced.as_ref());
    handle_enhanced_frame(
        enhanced.as_ref(),
        vad,
        vad_frame_resampler,
        processed_samples,
    );
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

fn drain_available_samples(
    consumer: &mut Consumer<f32>,
    max_samples: usize,
    mut process: impl FnMut(&[f32]),
) -> usize {
    let available = consumer.slots().min(max_samples);
    if available == 0 {
        return 0;
    }

    let chunk = consumer
        .read_chunk(available)
        .expect("reported audio ring slots must be readable");
    let (first, second) = chunk.as_slices();
    if !first.is_empty() {
        process(first);
    }
    if !second.is_empty() {
        process(second);
    }
    chunk.commit_all();
    available
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChunkDisposition {
    Capture,
    Discard,
}

struct CapturePipeline {
    vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    stream_frame_cb: Arc<Mutex<Option<StreamFrameCallback>>>,
    source: AudioCaptureSource,
    microphone_input_gain: Arc<Mutex<f32>>,
    microphone_noise_cancellation_enabled: Arc<AtomicBool>,
    noise_suppressor: Option<NoiseSuppressor>,
    visualizer: AudioVisualiser,
    frame_resampler: FrameResampler,
    vad_frame_resampler: Option<FrameResampler>,
    processed_samples: Vec<f32>,
    capture_ready_tx: Option<mpsc::Sender<()>>,
    max_drain_samples: usize,
    total_dropped_samples: u64,
    overrun_warning_logged: bool,
}

impl CapturePipeline {
    fn new(
        in_sample_rate: u32,
        vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
        level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
        stream_frame_cb: Arc<Mutex<Option<StreamFrameCallback>>>,
        source: AudioCaptureSource,
        microphone_input_gain: Arc<Mutex<f32>>,
        microphone_noise_cancellation_enabled: Arc<AtomicBool>,
    ) -> Self {
        let frame_resampler = FrameResampler::new(
            in_sample_rate as usize,
            constants::WHISPER_SAMPLE_RATE as usize,
            Duration::from_millis(30),
        );
        let vad_frame_resampler = vad.as_ref().map(|detector| {
            let frame_samples = detector.lock().unwrap().frame_samples();
            FrameResampler::new(
                constants::WHISPER_SAMPLE_RATE as usize,
                constants::WHISPER_SAMPLE_RATE as usize,
                Duration::from_secs_f64(
                    frame_samples as f64 / constants::WHISPER_SAMPLE_RATE as f64,
                ),
            )
        });
        const BUCKETS: usize = 16;
        let window_size = visualizer_window_size(in_sample_rate);
        let visualizer = AudioVisualiser::new(in_sample_rate, window_size, BUCKETS, 400.0, 4000.0);
        let max_drain_samples =
            ((in_sample_rate as u128 * MAX_DRAIN_CHUNK.as_millis()) / 1_000).max(1) as usize;

        Self {
            vad,
            level_cb,
            stream_frame_cb,
            source,
            microphone_input_gain,
            microphone_noise_cancellation_enabled,
            noise_suppressor: None,
            visualizer,
            frame_resampler,
            vad_frame_resampler,
            processed_samples: Vec::new(),
            capture_ready_tx: None,
            max_drain_samples,
            total_dropped_samples: 0,
            overrun_warning_logged: false,
        }
    }

    fn begin_recording(&mut self, sent_at: Instant, ready_tx: mpsc::Sender<()>) {
        log::debug!(
            "Cmd::Start processed {:?} after send; capture begins with the next available samples",
            sent_at.elapsed()
        );
        self.processed_samples.clear();
        self.noise_suppressor = None;
        self.capture_ready_tx = Some(ready_tx);
        self.total_dropped_samples = 0;
        self.overrun_warning_logged = false;
        self.visualizer.reset();
        self.frame_resampler.reset();
        if let Some(resampler) = self.vad_frame_resampler.as_mut() {
            resampler.reset();
        }
        if let Some(detector) = &self.vad {
            detector.lock().unwrap().reset();
        }
    }

    fn cancel_ready_signal(&mut self) {
        self.capture_ready_tx = None;
    }

    fn drain(&mut self, consumer: &mut Consumer<f32>, disposition: ChunkDisposition) -> usize {
        let max_samples = self.max_drain_samples;
        drain_available_samples(consumer, max_samples, |raw| {
            self.process_raw_chunk(raw, disposition)
        })
    }

    fn process_raw_chunk(&mut self, raw: &[f32], disposition: ChunkDisposition) {
        if disposition == ChunkDisposition::Discard {
            return;
        }

        if let Some(buckets) = self.visualizer.feed(raw) {
            if let Some(callback) = &self.level_cb {
                callback(buckets);
            }
        }

        self.frame_resampler.push(raw, &mut |frame: &[f32]| {
            process_enhanced_capture_frame(
                frame,
                &self.vad,
                &mut self.vad_frame_resampler,
                &self.stream_frame_cb,
                self.source,
                &self.microphone_input_gain,
                &self.microphone_noise_cancellation_enabled,
                &mut self.noise_suppressor,
                &mut self.processed_samples,
            );
        });

        if let Some(ready_tx) = self.capture_ready_tx.take() {
            // Silence still counts as ready: the host is delivering samples.
            let _ = ready_tx.send(());
        }
    }

    fn flush(&mut self, keep_samples: usize, min_samples: usize) -> Vec<f32> {
        let flushable_len = self.processed_samples.len().saturating_sub(keep_samples);
        if flushable_len < min_samples {
            return Vec::new();
        }
        self.processed_samples.drain(..flushable_len).collect()
    }

    fn observe_overrun(&mut self, samples: u64) {
        if samples == 0 {
            return;
        }
        self.total_dropped_samples = self.total_dropped_samples.saturating_add(samples);
        if !self.overrun_warning_logged {
            self.overrun_warning_logged = true;
            log::warn!(
                "Audio capture ring dropped {samples} samples; continuing the active recording"
            );
        }
    }

    fn finish_recording(&mut self) -> Vec<f32> {
        self.frame_resampler.finish(&mut |frame: &[f32]| {
            process_enhanced_capture_frame(
                frame,
                &self.vad,
                &mut self.vad_frame_resampler,
                &self.stream_frame_cb,
                self.source,
                &self.microphone_input_gain,
                &self.microphone_noise_cancellation_enabled,
                &mut self.noise_suppressor,
                &mut self.processed_samples,
            );
        });
        if let Some(resampler) = self.vad_frame_resampler.as_mut() {
            resampler.finish(|frame: &[f32]| {
                handle_frame(frame, true, &self.vad, &mut self.processed_samples)
            });
        }
        if self.total_dropped_samples > 0 {
            log::warn!(
                "Active recording completed after dropping {} audio samples",
                self.total_dropped_samples
            );
        }
        self.noise_suppressor = None;
        std::mem::take(&mut self.processed_samples)
    }
}

fn run_consumer(
    in_sample_rate: u32,
    vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    mut sample_consumer: Consumer<f32>,
    cmd_rx: mpsc::Receiver<Cmd>,
    level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    stream_frame_cb: Arc<Mutex<Option<StreamFrameCallback>>>,
    source: AudioCaptureSource,
    microphone_input_gain: Arc<Mutex<f32>>,
    microphone_noise_cancellation_enabled: Arc<AtomicBool>,
    transport: Arc<CaptureTransportState>,
    stream_error: Arc<AtomicBool>,
) {
    let mut pipeline = CapturePipeline::new(
        in_sample_rate,
        vad,
        level_cb,
        stream_frame_cb,
        source,
        microphone_input_gain,
        microphone_noise_cancellation_enabled,
    );
    let mut recording = false;
    let mut stream_error_logged = false;

    loop {
        // Commands are checked before every bounded drain so Stop cannot wait
        // behind a multi-second ring backlog.
        let mut command = if sample_consumer.slots() > 0 {
            match cmd_rx.try_recv() {
                Ok(command) => Some(command),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        } else {
            match cmd_rx.recv_timeout(CONSUMER_POLL_INTERVAL) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        };

        loop {
            if let Some(cmd) = command.take() {
                match cmd {
                    Cmd::Start(sent_at, ready_tx) => {
                        transport.overrun_samples.store(0, Ordering::Release);
                        pipeline.begin_recording(sent_at, ready_tx);
                        recording = true;
                    }
                    Cmd::Flush {
                        keep_samples,
                        min_samples,
                        reply_tx,
                    } => {
                        let samples = if recording {
                            pipeline.flush(keep_samples, min_samples)
                        } else {
                            Vec::new()
                        };
                        let _ = reply_tx.send(samples);
                    }
                    Cmd::Stop(reply_tx) => {
                        pipeline
                            .observe_overrun(transport.overrun_samples.swap(0, Ordering::AcqRel));
                        recording = false;
                        pipeline.cancel_ready_signal();

                        // Retain one callback block at the recording boundary,
                        // then drain everything committed before its acknowledgement.
                        transport.pause_acknowledged.store(false, Ordering::Relaxed);
                        transport.pause_requested.store(true, Ordering::Release);
                        let pause_started = Instant::now();
                        while !transport.pause_acknowledged.load(Ordering::Acquire)
                            && pause_started.elapsed() < PAUSE_ACK_TIMEOUT
                        {
                            if pipeline.drain(&mut sample_consumer, ChunkDisposition::Capture) == 0
                            {
                                std::thread::sleep(Duration::from_millis(1));
                            }
                        }

                        let pause_timed_out = !transport.pause_acknowledged.load(Ordering::Acquire);
                        if pause_timed_out {
                            log::warn!("Timed out waiting for the audio callback to pause");
                            stream_error.store(true, Ordering::Release);
                        }

                        while pipeline.drain(&mut sample_consumer, ChunkDisposition::Capture) > 0 {}
                        pipeline
                            .observe_overrun(transport.overrun_samples.swap(0, Ordering::AcqRel));
                        let samples = pipeline.finish_recording();

                        if !pause_timed_out {
                            // Resume before stop() returns so an immediate new
                            // recording cannot lose its first callback.
                            transport.pause_acknowledged.store(false, Ordering::Relaxed);
                            transport.pause_requested.store(false, Ordering::Release);
                        }
                        let _ = reply_tx.send(samples);
                        if pause_timed_out {
                            return;
                        }
                    }
                    Cmd::Shutdown => {
                        pipeline.cancel_ready_signal();
                        transport.pause_requested.store(true, Ordering::Release);
                        return;
                    }
                }
            }

            command = match cmd_rx.try_recv() {
                Ok(command) => Some(command),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            };
        }

        let disposition = if recording {
            ChunkDisposition::Capture
        } else {
            ChunkDisposition::Discard
        };
        pipeline.drain(&mut sample_consumer, disposition);

        let overrun_samples = transport.overrun_samples.swap(0, Ordering::AcqRel);
        if recording {
            pipeline.observe_overrun(overrun_samples);
        }

        if stream_error.load(Ordering::Acquire) && !stream_error_logged {
            log::error!("Audio backend reported a stream error; it will be rebuilt");
            stream_error_logged = true;
        }
    }
}
