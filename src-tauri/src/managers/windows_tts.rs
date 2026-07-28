//! Windows.Media.SpeechSynthesis integration and strict WAV normalization.

use anyhow::{anyhow, Context, Result};
use rubato::{FftFixedIn, Resampler};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;
use std::io::Cursor;

use super::tts::PROVIDER_PCM_SAMPLE_RATE;

pub const WINDOWS_TTS_PROVIDER_LIMIT: usize = 4_096;
const MAX_SYNTHESIS_BYTES: u64 = 128 * 1024 * 1024;
const MAX_CHANNELS: u16 = 8;
const MAX_SAMPLE_RATE: u32 = 192_000;
const MAX_SOURCE_MONO_SAMPLES: usize = 32 * 1024 * 1024;
const MAX_NORMALIZED_SAMPLES: usize = PROVIDER_PCM_SAMPLE_RATE as usize * 60 * 30;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowsVoiceGender {
    Female,
    Male,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct WindowsVoice {
    pub id: String,
    pub display_name: String,
    pub language: String,
    pub description: String,
    pub gender: WindowsVoiceGender,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct WindowsVoiceCatalog {
    pub available: bool,
    pub voices: Vec<WindowsVoice>,
    pub default_voice_id: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug)]
pub struct WindowsTtsAttemptError {
    pub safe_message: String,
    pub transient: bool,
}

#[derive(Debug)]
struct PermanentWindowsTtsError(String);

impl std::fmt::Display for PermanentWindowsTtsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PermanentWindowsTtsError {}

impl WindowsVoiceCatalog {
    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            voices: Vec::new(),
            default_voice_id: None,
            unavailable_reason: Some(reason.into()),
        }
    }
}

fn normalize_catalog(
    mut voices: Vec<WindowsVoice>,
    default_voice_id: Option<String>,
) -> Result<WindowsVoiceCatalog> {
    let default_voice_id = default_voice_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());
    let mut ids = HashSet::new();
    for voice in &mut voices {
        voice.id = voice.id.trim().to_string();
        if voice.id.is_empty() {
            return Err(anyhow!(
                "Windows returned an installed voice with an empty ID"
            ));
        }
        if !ids.insert(voice.id.clone()) {
            return Err(anyhow!(
                "Windows returned duplicate installed voice ID '{}'",
                voice.id
            ));
        }
        voice.is_default = default_voice_id.as_deref() == Some(voice.id.as_str());
    }
    voices.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.language.cmp(&right.language))
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(WindowsVoiceCatalog {
        available: true,
        voices,
        default_voice_id,
        unavailable_reason: None,
    })
}

pub async fn voice_catalog() -> WindowsVoiceCatalog {
    #[cfg(windows)]
    {
        match tauri::async_runtime::spawn_blocking(windows_voice_catalog).await {
            Ok(Ok(catalog)) => catalog,
            Ok(Err(error)) => WindowsVoiceCatalog::unavailable(format!(
                "Windows installed voices could not be read: {error}"
            )),
            Err(error) => WindowsVoiceCatalog::unavailable(format!(
                "Windows installed-voice task failed: {error}"
            )),
        }
    }
    #[cfg(not(windows))]
    {
        WindowsVoiceCatalog::unavailable(
            "Windows installed voices are available only when AivoRelay runs on Windows",
        )
    }
}

pub async fn resolve_voice_selection(
    voice_id: &str,
) -> std::result::Result<WindowsVoice, WindowsTtsAttemptError> {
    #[cfg(windows)]
    {
        let catalog = tauri::async_runtime::spawn_blocking(windows_voice_catalog)
            .await
            .map_err(|error| WindowsTtsAttemptError {
                safe_message: format!("Windows installed-voice task failed: {error}"),
                transient: true,
            })?
            .map_err(|error| WindowsTtsAttemptError {
                safe_message: format!("Windows installed voices could not be read: {error}"),
                transient: true,
            })?;
        select_voice_from_catalog(&catalog, voice_id)
    }
    #[cfg(not(windows))]
    {
        let _ = voice_id;
        Err(WindowsTtsAttemptError {
            safe_message:
                "Windows installed voices are available only when AivoRelay runs on Windows"
                    .to_string(),
            transient: false,
        })
    }
}

fn select_voice_from_catalog(
    catalog: &WindowsVoiceCatalog,
    voice_id: &str,
) -> std::result::Result<WindowsVoice, WindowsTtsAttemptError> {
    if !catalog.available {
        return Err(WindowsTtsAttemptError {
            safe_message: catalog.unavailable_reason.clone().unwrap_or_else(|| {
                "Windows installed voices are currently unavailable".to_string()
            }),
            transient: false,
        });
    }
    let requested_id = voice_id.trim();
    let selected_id = if requested_id.is_empty() {
        catalog
            .default_voice_id
            .as_deref()
            .ok_or_else(|| WindowsTtsAttemptError {
                safe_message: "Windows did not report a default speech voice".to_string(),
                transient: false,
            })?
    } else {
        requested_id
    };
    catalog
        .voices
        .iter()
        .find(|voice| voice.id == selected_id)
        .cloned()
        .ok_or_else(|| WindowsTtsAttemptError {
            safe_message: if requested_id.is_empty() {
                "The Windows default speech voice is not present in the installed voice catalog"
                    .to_string()
            } else {
                format!(
                    "The selected Windows voice is no longer installed (voice ID '{}')",
                    requested_id
                )
            },
            transient: false,
        })
}

fn classify_windows_error(error: anyhow::Error) -> WindowsTtsAttemptError {
    WindowsTtsAttemptError {
        transient: error.downcast_ref::<PermanentWindowsTtsError>().is_none(),
        safe_message: error.to_string(),
    }
}

pub async fn synthesize(
    text: String,
    voice_id: String,
    speed: f32,
) -> std::result::Result<Vec<i16>, WindowsTtsAttemptError> {
    #[cfg(windows)]
    {
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let _cancel_on_drop = CancelOnDrop(std::sync::Arc::clone(&cancelled));
        let result = tauri::async_runtime::spawn_blocking(move || {
            windows_synthesize(&text, &voice_id, speed, &cancelled)
        })
        .await
        .map_err(|error| WindowsTtsAttemptError {
            safe_message: format!("Windows speech synthesis task failed: {error}"),
            transient: true,
        })?;
        result.map_err(classify_windows_error)
    }
    #[cfg(not(windows))]
    {
        let _ = (text, voice_id, speed);
        Err(WindowsTtsAttemptError {
            safe_message:
                "Windows installed voices are available only when AivoRelay runs on Windows"
                    .to_string(),
            transient: false,
        })
    }
}

#[cfg(windows)]
struct WinRtGuard;

#[cfg(windows)]
thread_local! {
    // Tauri reuses blocking-pool threads. SpeechSynthesizer keeps process-wide
    // state that can become invalid if WinRT is repeatedly torn down between
    // catalog and synthesis calls on the same worker. Keep the apartment alive
    // for the worker's lifetime and balance it when that thread exits.
    static WINRT_GUARD: std::result::Result<WinRtGuard, String> =
        WinRtGuard::initialize().map_err(|error| error.to_string());
}

#[cfg(windows)]
struct CancelOnDrop(std::sync::Arc<std::sync::atomic::AtomicBool>);

#[cfg(windows)]
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(windows)]
impl WinRtGuard {
    fn initialize() -> Result<Self> {
        use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .context("Could not initialize the Windows Runtime on the synthesis thread")?;
        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for WinRtGuard {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::WinRT::RoUninitialize() };
    }
}

#[cfg(windows)]
fn ensure_winrt_initialized() -> Result<()> {
    WINRT_GUARD.with(|guard| match guard {
        Ok(_) => Ok(()),
        Err(message) => Err(anyhow::anyhow!(message.clone())),
    })
}

#[cfg(windows)]
fn windows_voice_catalog() -> Result<WindowsVoiceCatalog> {
    use windows::Media::SpeechSynthesis::{SpeechSynthesizer, VoiceGender};

    ensure_winrt_initialized()?;
    let default_id = SpeechSynthesizer::DefaultVoice()
        .ok()
        .and_then(|voice| voice.Id().ok())
        .map(|id| id.to_string());
    let installed = SpeechSynthesizer::AllVoices()
        .context("Windows did not provide the installed voice collection")?;
    let mut voices = Vec::with_capacity(installed.Size()? as usize);
    for index in 0..installed.Size()? {
        let voice = installed.GetAt(index)?;
        let gender = match voice.Gender()? {
            VoiceGender::Female => WindowsVoiceGender::Female,
            VoiceGender::Male => WindowsVoiceGender::Male,
            _ => WindowsVoiceGender::Unknown,
        };
        voices.push(WindowsVoice {
            id: voice.Id()?.to_string(),
            display_name: voice.DisplayName()?.to_string(),
            language: voice.Language()?.to_string(),
            description: voice.Description()?.to_string(),
            gender,
            is_default: false,
        });
    }
    normalize_catalog(voices, default_id)
}

#[cfg(windows)]
fn windows_synthesize(
    text: &str,
    selected_voice_id: &str,
    speed: f32,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Vec<i16>> {
    use windows::core::HSTRING;
    use windows::Media::SpeechSynthesis::{
        SpeechAppendedSilence, SpeechPunctuationSilence, SpeechSynthesizer,
    };
    use windows::Storage::Streams::DataReader;

    if text.is_empty() {
        return Err(
            PermanentWindowsTtsError("Refusing to synthesize empty text".to_string()).into(),
        );
    }
    ensure_winrt_initialized()?;
    let synthesizer =
        SpeechSynthesizer::new().context("Could not create Windows speech synthesizer")?;
    let result = (|| {
        if !selected_voice_id.is_empty() {
            let voices = SpeechSynthesizer::AllVoices()?;
            let mut selected = None;
            for index in 0..voices.Size()? {
                let voice = voices.GetAt(index)?;
                if voice.Id()?.to_string() == selected_voice_id {
                    selected = Some(voice);
                    break;
                }
            }
            let selected = selected.ok_or_else(|| {
                PermanentWindowsTtsError(format!(
                    "The selected Windows voice is no longer installed (voice ID '{}')",
                    selected_voice_id
                ))
            })?;
            synthesizer.SetVoice(&selected)?;
        }
        let options = synthesizer.Options()?;
        options.SetSpeakingRate(f64::from(speed.clamp(0.5, 2.0)))?;
        // Shared assembly owns inter-chunk and paragraph pauses. Older Windows
        // versions may not expose these option interfaces, so silence tuning is
        // best-effort and must not make otherwise valid synthesis unavailable.
        if let Err(error) = options.SetAppendedSilence(SpeechAppendedSilence::Min) {
            log::debug!("Windows TTS appended-silence option unavailable: {error}");
        }
        if let Err(error) = options.SetPunctuationSilence(SpeechPunctuationSilence::Min) {
            log::debug!("Windows TTS punctuation-silence option unavailable: {error}");
        }
        let synthesis = synthesizer.SynthesizeTextToStreamAsync(&HSTRING::from(text))?;
        let stream_result = wait_for_winrt_operation(
            &synthesis,
            cancelled,
            "Windows could not synthesize the requested text",
        );
        let _ = synthesis.Close();
        let stream = stream_result?;
        let stream_result = (|| {
            let size = stream.Size()?;
            if size == 0 || size > MAX_SYNTHESIS_BYTES || size > u64::from(u32::MAX) {
                return Err(PermanentWindowsTtsError(
                    "Windows speech synthesis returned an empty or unreasonably large audio stream"
                        .to_string(),
                )
                .into());
            }
            let input = stream.GetInputStreamAt(0)?;
            let input_result = (|| {
                let reader = DataReader::CreateDataReader(&input)?;
                let reader_result = (|| {
                    let load = reader.LoadAsync(size as u32)?;
                    let loaded_result = wait_for_winrt_operation(
                        &load,
                        cancelled,
                        "Windows could not load synthesized audio",
                    );
                    let _ = load.Close();
                    let loaded = loaded_result?;
                    if loaded != size as u32 {
                        return Err(PermanentWindowsTtsError(
                            "Windows speech synthesis returned a truncated audio stream"
                                .to_string(),
                        )
                        .into());
                    }
                    ensure_not_cancelled(cancelled)?;
                    let mut bytes = vec![0_u8; size as usize];
                    reader.ReadBytes(&mut bytes)?;
                    ensure_not_cancelled(cancelled)?;
                    decode_wav_to_pcm_cancellable(&bytes, PROVIDER_PCM_SAMPLE_RATE, Some(cancelled))
                        .map_err(|error| PermanentWindowsTtsError(error.to_string()).into())
                })();
                let _ = reader.Close();
                reader_result
            })();
            let _ = input.Close();
            input_result
        })();
        let _ = stream.Close();
        stream_result
    })();
    let _ = synthesizer.Close();
    result
}

#[cfg(windows)]
fn ensure_not_cancelled(cancelled: &std::sync::atomic::AtomicBool) -> Result<()> {
    if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        Err(PermanentWindowsTtsError("Windows speech synthesis was cancelled".to_string()).into())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn wait_for_winrt_operation<T: windows::core::RuntimeType + 'static>(
    operation: &windows_future::IAsyncOperation<T>,
    cancelled: &std::sync::atomic::AtomicBool,
    context: &str,
) -> Result<T> {
    use std::sync::atomic::Ordering;
    use windows_future::AsyncStatus;

    loop {
        if cancelled.load(Ordering::SeqCst) {
            let _ = operation.Cancel();
            return Err(PermanentWindowsTtsError(
                "Windows speech synthesis was cancelled".to_string(),
            )
            .into());
        }
        let status = operation.Status()?;
        if status == AsyncStatus::Completed || status == AsyncStatus::Error {
            return operation.GetResults().with_context(|| context.to_string());
        }
        if status == AsyncStatus::Canceled {
            return Err(PermanentWindowsTtsError(
                "Windows speech synthesis was cancelled".to_string(),
            )
            .into());
        }
        if status != AsyncStatus::Started {
            return Err(PermanentWindowsTtsError(
                "Windows speech synthesis returned an invalid asynchronous status".to_string(),
            )
            .into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(test)]
pub(crate) fn decode_wav_to_pcm(bytes: &[u8], target_rate: u32) -> Result<Vec<i16>> {
    decode_wav_to_pcm_cancellable(bytes, target_rate, None)
}

fn decode_wav_to_pcm_cancellable(
    bytes: &[u8],
    target_rate: u32,
    cancelled: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<i16>> {
    check_decode_cancellation(cancelled)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_SYNTHESIS_BYTES {
        return Err(anyhow!(
            "Windows speech synthesis returned invalid WAV size"
        ));
    }
    let mut reader = hound::WavReader::new(Cursor::new(bytes))
        .context("Windows returned malformed WAV audio")?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.channels > MAX_CHANNELS {
        return Err(anyhow!("Windows returned an unsupported WAV channel count"));
    }
    if spec.sample_rate == 0 || spec.sample_rate > MAX_SAMPLE_RATE || target_rate == 0 {
        return Err(anyhow!("Windows returned an unsupported WAV sample rate"));
    }
    let channels = usize::from(spec.channels);
    let input_values = usize::try_from(reader.len()).unwrap_or(usize::MAX);
    if input_values == 0 || input_values % channels != 0 {
        return Err(anyhow!("Windows returned invalid WAV sample data"));
    }
    let input_frames = input_values / channels;
    if input_frames > MAX_SOURCE_MONO_SAMPLES {
        return Err(anyhow!("Windows returned too many WAV samples"));
    }
    normalized_frame_count(input_frames, spec.sample_rate, target_rate)?;
    let mono = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, bits @ (8 | 16 | 24 | 32)) => {
            let scale = (1_i64 << (bits - 1)) as f32;
            collect_downmixed_frames(
                reader.samples::<i32>(),
                input_frames,
                channels,
                |sample| sample as f32 / scale,
                cancelled,
            )?
        }
        (hound::SampleFormat::Float, 32) => collect_downmixed_frames(
            reader.samples::<f32>(),
            input_frames,
            channels,
            |sample| sample,
            cancelled,
        )?,
        _ => return Err(anyhow!("Windows returned an unsupported WAV sample format")),
    };
    let mono = resample_exact(mono, spec.sample_rate, target_rate, cancelled)?;
    if mono.is_empty() {
        return Err(anyhow!("Windows returned no usable audio samples"));
    }
    let mut pcm = Vec::with_capacity(mono.len());
    for (index, sample) in mono.into_iter().enumerate() {
        if index % 4_096 == 0 {
            check_decode_cancellation(cancelled)?;
        }
        pcm.push(
            (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX))
                .round()
                .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16,
        );
    }
    check_decode_cancellation(cancelled)?;
    Ok(pcm)
}

fn collect_downmixed_frames<T, I, F>(
    mut samples: I,
    frame_count: usize,
    channels: usize,
    normalize: F,
    cancelled: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<f32>>
where
    I: Iterator<Item = std::result::Result<T, hound::Error>>,
    F: Fn(T) -> f32,
{
    let mut mono = Vec::with_capacity(frame_count);
    for frame_index in 0..frame_count {
        if frame_index % 4_096 == 0 {
            check_decode_cancellation(cancelled)?;
        }
        let mut sum = 0.0_f32;
        for _ in 0..channels {
            let sample = samples
                .next()
                .ok_or_else(|| anyhow!("Windows returned truncated WAV sample data"))?
                .context("Windows returned truncated WAV sample data")?;
            let sample = normalize(sample);
            if !sample.is_finite() {
                return Err(anyhow!("Windows returned invalid WAV sample data"));
            }
            sum += sample;
        }
        let sample = sum / channels as f32;
        if !sample.is_finite() {
            return Err(anyhow!("Windows returned invalid WAV sample data"));
        }
        mono.push(sample);
    }
    Ok(mono)
}

fn check_decode_cancellation(cancelled: Option<&std::sync::atomic::AtomicBool>) -> Result<()> {
    if cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
        Err(anyhow!("Windows speech synthesis was cancelled"))
    } else {
        Ok(())
    }
}

fn normalized_frame_count(
    input_frames: usize,
    source_rate: u32,
    target_rate: u32,
) -> Result<usize> {
    u64::try_from(input_frames)
        .ok()
        .and_then(|length| length.checked_mul(u64::from(target_rate)))
        .and_then(|scaled| scaled.checked_add(u64::from(source_rate) / 2))
        .map(|rounded| rounded / u64::from(source_rate))
        .and_then(|frames| usize::try_from(frames).ok())
        .filter(|frames| *frames != 0 && *frames <= MAX_NORMALIZED_SAMPLES)
        .ok_or_else(|| anyhow!("Windows returned an unsupported WAV duration"))
}

fn resample_exact(
    samples: Vec<f32>,
    source_rate: u32,
    target_rate: u32,
    cancelled: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<f32>> {
    check_decode_cancellation(cancelled)?;
    let expected = normalized_frame_count(samples.len(), source_rate, target_rate)?;
    if source_rate == target_rate {
        return Ok(samples);
    }
    // One chunk must span an exact whole-number rate ratio. Otherwise
    // FftFixedIn may legitimately buffer many zero-padded partial calls before
    // producing anything when the two rates are coprime.
    let chunk = (source_rate / greatest_common_divisor(source_rate, target_rate)) as usize;
    let mut resampler =
        FftFixedIn::<f32>::new(source_rate as usize, target_rate as usize, chunk, 1, 1)
            .context("Could not configure Windows TTS audio resampling")?;
    let delay = resampler.output_delay();
    let required = delay
        .checked_add(expected)
        .filter(|frames| *frames <= MAX_NORMALIZED_SAMPLES.saturating_add(delay))
        .ok_or_else(|| anyhow!("Windows returned an unsupported WAV duration"))?;
    let mut output = Vec::with_capacity(required);
    let mut offset = 0;
    while offset + chunk <= samples.len() {
        check_decode_cancellation(cancelled)?;
        let result = resampler.process(&[&samples[offset..offset + chunk]], None)?;
        output.extend_from_slice(&result[0]);
        offset += chunk;
    }
    if offset < samples.len() {
        let result = resampler.process_partial(Some(&[&samples[offset..]]), None)?;
        output.extend_from_slice(&result[0]);
    }
    let mut flush_count = 0_u8;
    while output.len() < required {
        check_decode_cancellation(cancelled)?;
        let flush = resampler.process_partial::<&[f32]>(None, None)?;
        output.extend_from_slice(&flush[0]);
        flush_count = flush_count.saturating_add(1);
        if flush_count >= 2 && output.len() < required {
            return Err(anyhow!(
                "Windows TTS audio resampler returned truncated output"
            ));
        }
    }
    check_decode_cancellation(cancelled)?;
    Ok(output[delay..required].to_vec())
}

fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{SampleFormat, WavSpec, WavWriter};

    fn wav_i16(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(
                &mut cursor,
                WavSpec {
                    channels,
                    sample_rate: rate,
                    bits_per_sample: 16,
                    sample_format: SampleFormat::Int,
                },
            )
            .unwrap();
            for sample in samples {
                writer.write_sample(*sample).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn catalog_is_default_first_and_rejects_duplicate_ids() {
        let voice = |id: &str, language: &str| WindowsVoice {
            id: id.into(),
            display_name: id.into(),
            language: language.into(),
            description: String::new(),
            gender: WindowsVoiceGender::Unknown,
            is_default: false,
        };
        let catalog =
            normalize_catalog(vec![voice("b", "en"), voice("a", "fi")], Some("b".into())).unwrap();
        assert_eq!(catalog.voices[0].id, "b");
        assert!(normalize_catalog(vec![voice("a", "en"), voice("a", "fi")], None).is_err());
    }

    #[test]
    fn catalog_trims_default_voice_id() {
        let catalog = normalize_catalog(
            vec![WindowsVoice {
                id: "voice-a".into(),
                display_name: "Voice A".into(),
                language: "en-US".into(),
                description: String::new(),
                gender: WindowsVoiceGender::Unknown,
                is_default: false,
            }],
            Some(" voice-a ".into()),
        )
        .unwrap();
        assert_eq!(catalog.default_voice_id.as_deref(), Some("voice-a"));
        assert!(catalog.voices[0].is_default);
    }

    #[test]
    fn catalog_selection_resolves_default_and_explicit_stable_ids() {
        let catalog = normalize_catalog(
            vec![
                WindowsVoice {
                    id: "voice-en".into(),
                    display_name: "English".into(),
                    language: "en-US".into(),
                    description: String::new(),
                    gender: WindowsVoiceGender::Female,
                    is_default: false,
                },
                WindowsVoice {
                    id: "voice-ru".into(),
                    display_name: "Russian".into(),
                    language: "ru-RU".into(),
                    description: String::new(),
                    gender: WindowsVoiceGender::Male,
                    is_default: false,
                },
            ],
            Some("voice-en".into()),
        )
        .unwrap();

        assert_eq!(
            select_voice_from_catalog(&catalog, "").unwrap().id,
            "voice-en"
        );
        let russian = select_voice_from_catalog(&catalog, " voice-ru ").unwrap();
        assert_eq!(russian.id, "voice-ru");
        assert_eq!(russian.language, "ru-RU");
        assert!(
            !select_voice_from_catalog(&catalog, "missing")
                .unwrap_err()
                .transient
        );
    }

    #[test]
    fn permanent_windows_errors_do_not_retry() {
        let permanent = classify_windows_error(
            PermanentWindowsTtsError("malformed Windows audio".to_string()).into(),
        );
        assert!(!permanent.transient);
        assert_eq!(permanent.safe_message, "malformed Windows audio");

        let transient = classify_windows_error(anyhow!("temporary WinRT failure"));
        assert!(transient.transient);
    }

    #[cfg(windows)]
    #[test]
    fn dropping_synthesis_guard_requests_winrt_cancellation() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let _guard = CancelOnDrop(Arc::clone(&cancelled));
            assert!(!cancelled.load(Ordering::SeqCst));
        }
        assert!(cancelled.load(Ordering::SeqCst));
    }

    #[cfg(windows)]
    #[test]
    fn installed_voice_catalog_can_be_queried_repeatedly_on_one_thread() {
        let first = windows_voice_catalog().unwrap();
        let second = windows_voice_catalog().unwrap();
        assert!(!first.voices.is_empty());
        assert_eq!(first.default_voice_id, second.default_voice_id);
    }

    #[test]
    fn wav_mono_passthrough_and_stereo_downmix() {
        assert_eq!(
            decode_wav_to_pcm(&wav_i16(1, 24_000, &[100, -100]), 24_000).unwrap(),
            vec![100, -100]
        );
        assert_eq!(
            decode_wav_to_pcm(&wav_i16(2, 24_000, &[100, 300, -100, 100]), 24_000).unwrap(),
            vec![200, 0]
        );
    }

    #[test]
    fn resampling_preserves_rounded_duration() {
        let pcm = decode_wav_to_pcm(&wav_i16(1, 48_000, &vec![500; 4_800]), 24_000).unwrap();
        assert_eq!(pcm.len(), 2_400);
        assert_eq!(
            decode_wav_to_pcm(&wav_i16(1, 48_000, &[500]), 24_000)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            decode_wav_to_pcm(&wav_i16(1, 48_000, &[500, 500, 500]), 24_000)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn resampling_handles_coprime_rates_without_unbounded_flushes() {
        let pcm = decode_wav_to_pcm(&wav_i16(1, 48_001, &vec![500; 1_024]), 24_000).unwrap();
        assert_eq!(pcm.len(), 512);
    }

    #[test]
    fn resampling_rate_matrix_handles_ratio_chunk_boundaries() {
        for source_rate in [8_000, 22_050, 24_000, 44_100, 48_000, 96_000, 192_000] {
            let ratio_chunk = (source_rate / greatest_common_divisor(source_rate, 24_000)) as usize;
            for length in [
                ratio_chunk.saturating_sub(1),
                ratio_chunk,
                ratio_chunk.saturating_add(1),
            ] {
                if length == 0 {
                    continue;
                }
                let expected = normalized_frame_count(length, source_rate, 24_000);
                let decoded =
                    decode_wav_to_pcm(&wav_i16(1, source_rate, &vec![500; length]), 24_000);
                match expected {
                    Ok(expected) => assert_eq!(decoded.unwrap().len(), expected),
                    Err(_) => assert!(decoded.is_err()),
                }
            }
        }
    }

    #[test]
    fn normalized_frame_bounds_cover_passthrough_cap_and_zero_rounding() {
        assert_eq!(
            normalized_frame_count(
                MAX_NORMALIZED_SAMPLES,
                PROVIDER_PCM_SAMPLE_RATE,
                PROVIDER_PCM_SAMPLE_RATE,
            )
            .unwrap(),
            MAX_NORMALIZED_SAMPLES
        );
        assert!(normalized_frame_count(
            MAX_NORMALIZED_SAMPLES + 1,
            PROVIDER_PCM_SAMPLE_RATE,
            PROVIDER_PCM_SAMPLE_RATE,
        )
        .is_err());
        assert!(normalized_frame_count(1, MAX_SAMPLE_RATE, PROVIDER_PCM_SAMPLE_RATE).is_err());
    }

    #[test]
    fn resampling_rejects_unreasonably_long_normalized_output() {
        assert!(decode_wav_to_pcm(&wav_i16(1, 1, &[500; 1_801]), 24_000).is_err());
    }

    #[test]
    fn malformed_empty_and_truncated_wav_are_rejected() {
        assert!(decode_wav_to_pcm(&[], 24_000).is_err());
        assert!(decode_wav_to_pcm(b"RIFFbroken", 24_000).is_err());
        let mut wav = wav_i16(1, 24_000, &[1, 2, 3]);
        wav.pop();
        assert!(decode_wav_to_pcm(&wav, 24_000).is_err());
    }

    #[test]
    fn cancelled_decode_stops_before_audio_work() {
        use std::sync::atomic::AtomicBool;

        let cancelled = AtomicBool::new(true);
        let error = decode_wav_to_pcm_cancellable(
            &wav_i16(1, 24_000, &[100, -100]),
            24_000,
            Some(&cancelled),
        )
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
    }

    #[test]
    fn unsupported_and_non_finite_wav_samples_are_rejected() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(
                &mut cursor,
                WavSpec {
                    channels: 1,
                    sample_rate: 24_000,
                    bits_per_sample: 32,
                    sample_format: SampleFormat::Float,
                },
            )
            .unwrap();
            writer.write_sample(f32::NAN).unwrap();
            writer.finalize().unwrap();
        }
        assert!(decode_wav_to_pcm(&cursor.into_inner(), 24_000).is_err());
        assert!(decode_wav_to_pcm(&wav_i16(9, 24_000, &[0; 9]), 24_000).is_err());
    }
}
