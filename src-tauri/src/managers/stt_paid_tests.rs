//! Paid speech-to-text requests to Soniox, Deepgram and Gemini 3.5 Transcribe with the
//! keys saved in the app, on English speech synthesized by Windows. Ignored by default:
//! `./test-local.ps1 -LibOnly -Filter stt_paid,--ignored,--nocapture`
use super::deepgram_realtime::{DeepgramRealtimeManager, DeepgramRealtimeOptions};
use super::deepgram_stt::{DeepgramSttManager, DeepgramTranscriptionOptions};
use super::gemini_realtime::{
    GeminiRealtimeManager, GeminiRealtimeOptions, GEMINI_LIVE_DEFAULT_MODEL,
    GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
};
use super::history_replay::HistoryReplayGuard;
use super::remote_stt::{get_remote_stt_api_key, RemoteSttManager};
use super::soniox_realtime::{SonioxRealtimeManager, SonioxRealtimeOptions};
use super::soniox_stt::{SonioxAsyncTranscriptionOptions, SonioxSttManager};
use crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID;
use crate::audio_toolkit::audio::{encode_wav_bytes, FrameResampler};
use crate::gemini_config::{resolve_effective_config, GeminiWorkflow};
use crate::settings::{get_default_settings, AppSettings, TextReplacement};
use crate::stream_chunk_processor::StreamChunkProcessor;
use crate::url_security::{
    remote_stt_base_url_for_preset, REMOTE_STT_GOOGLE_DEFAULT_MODEL, REMOTE_STT_PRESET_GOOGLE,
    REMOTE_STT_PRESET_VERCEL, REMOTE_STT_VERCEL_DEFAULT_MODEL,
};
use parking_lot::Mutex;
use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::OnceCell;

const SENTENCES: &[&str] = &[
    "The quick brown fox jumps over the lazy dog.",
    "Then the fox rests under an old oak tree until the evening.",
];
const MIN_RECALL: f64 = 0.85;
const SAMPLE_RATE: usize = 16_000;
const FRAME_SAMPLES: usize = 480;
const FINISH_TIMEOUT_MS: u32 = 20_000;
const REQUEST_TIMEOUT_SECONDS: u32 = 120;

/// A real app handle without windows or plugins, enough for the provider managers.
fn app() -> &'static AppHandle {
    static APP: OnceLock<AppHandle> = OnceLock::new();
    APP.get_or_init(|| {
        crate::tls::initialize();
        let app = tauri::Builder::default()
            .any_thread()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("test app");
        let handle = app.handle().clone();
        std::mem::forget(app);
        handle
    })
}

fn silence(ms: usize) -> Vec<f32> {
    vec![0.0; SAMPLE_RATE * ms / 1000]
}

/// Both sentences as 16 kHz mono, separated by a pause long enough for endpointing.
async fn speech() -> &'static [f32] {
    static SPEECH: OnceCell<Vec<f32>> = OnceCell::const_new();
    SPEECH
        .get_or_init(|| async {
            let mut samples = silence(300);
            for sentence in SENTENCES {
                let pcm = super::windows_tts::synthesize(sentence.to_string(), String::new(), 1.0)
                    .await
                    .unwrap_or_else(|error| panic!("Windows TTS failed: {}", error.safe_message));
                let input: Vec<f32> = pcm.iter().map(|s| *s as f32 / 32768.0).collect();
                let mut resampler = FrameResampler::new(
                    super::tts::PROVIDER_PCM_SAMPLE_RATE as usize,
                    SAMPLE_RATE,
                    Duration::from_millis(30),
                );
                resampler.push(&input, |frame| samples.extend_from_slice(frame));
                resampler.finish(|frame| samples.extend_from_slice(frame));
                samples.extend(silence(1500));
            }
            samples
        })
        .await
}

fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| word.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
        .filter(|word| !word.is_empty())
        .collect()
}

/// Share of the spoken words found in the transcript.
fn check_recall(text: &str) -> Result<String, String> {
    let heard = words(text);
    let expected: Vec<String> = SENTENCES.iter().flat_map(|sentence| words(sentence)).collect();
    let found = expected.iter().filter(|word| heard.contains(word)).count();
    let recall = found as f64 / expected.len() as f64;
    if recall < MIN_RECALL {
        return Err(format!("heard {found}/{} words: {text:?}", expected.len()));
    }
    let missed: Vec<&String> = expected.iter().filter(|word| !heard.contains(word)).collect();
    if missed.is_empty() {
        return Ok(format!("{found}/{} words", expected.len()));
    }
    Ok(format!("{found}/{} words, missed {missed:?} in {text:?}", expected.len()))
}

fn ms(duration: Duration) -> u128 {
    duration.as_millis()
}

#[derive(Default)]
struct Report {
    failures: Vec<String>,
}

impl Report {
    fn record(&mut self, name: &str, result: Result<String, String>) {
        match result {
            Ok(report) => println!("OK   {name}: {report}"),
            Err(error) => {
                println!("FAIL {name}: {error}");
                self.failures.push(name.to_string());
            }
        }
    }

    fn assert_ok(self) {
        assert!(self.failures.is_empty(), "failed checks: {:?}", self.failures);
    }
}

/// A replacement applied while chunks stream in, as the app inserts them.
fn chunk_settings() -> AppSettings {
    let mut settings = get_default_settings();
    settings.text_replacements_enabled = true;
    settings.text_replacements = vec![TextReplacement {
        id: "tr_paid".into(),
        from: "fox".into(),
        to: "cat".into(),
        enabled: true,
        case_sensitive: false,
        is_regex: false,
    }];
    settings
}

type ChunkLog = Arc<Mutex<Vec<String>>>;

fn chunk_collector() -> (ChunkLog, Arc<dyn Fn(String) + Send + Sync>) {
    let chunks: ChunkLog = Arc::default();
    let sink = Arc::clone(&chunks);
    (chunks, Arc::new(move |chunk| sink.lock().push(chunk)))
}

/// Feeds the speech in real time, one 30 ms frame per tick, like the microphone.
async fn feed_live(samples: &[f32], push: impl Fn(Vec<f32>)) {
    let mut tick = tokio::time::interval(Duration::from_millis(30));
    for frame in samples.chunks(FRAME_SAMPLES) {
        tick.tick().await;
        push(frame.to_vec());
    }
}

/// Checks a finished live session: the words, that the streamed chunks add up to the
/// final text, and that inserting them one by one gives the same output as the whole.
fn check_live(text: &str, chunks: &[String], finalize: Duration) -> Result<String, String> {
    let recall = check_recall(text)?;
    let joined = chunks.concat();
    if !joined.split_whitespace().eq(text.split_whitespace()) {
        return Err(format!("chunks {chunks:?} differ from final text {text:?}"));
    }
    let settings = chunk_settings();
    let mut streamed = StreamChunkProcessor::from_settings(&settings);
    let mut inserted: String = chunks.iter().map(|chunk| streamed.push_chunk(chunk)).collect();
    inserted.push_str(&streamed.flush());
    let mut whole = StreamChunkProcessor::from_settings(&settings);
    let mut expected = whole.push_chunk(&joined);
    expected.push_str(&whole.flush());
    if inserted != expected {
        return Err(format!("streamed insertion differs:\n  live: {inserted:?}\n  whole: {expected:?}"));
    }
    if words(&inserted).iter().any(|word| word == "fox") {
        return Err(format!("replacement was not applied while streaming: {inserted:?}"));
    }
    Ok(format!(
        "{} chunks, finalized in {} ms, {recall}, inserted {:?}",
        chunks.len(),
        ms(finalize),
        inserted.trim()
    ))
}

async fn timed_text(request: impl Future<Output = anyhow::Result<String>>) -> Result<String, String> {
    let started = Instant::now();
    let text = request.await.map_err(|error| format!("{error:#}"))?;
    Ok(format!("{} ms, {}", ms(started.elapsed()), check_recall(&text)?))
}

fn replay_guard(id: i64, samples: &[f32]) -> HistoryReplayGuard {
    HistoryReplayGuard::start(app(), id, samples.len()).expect("history replay guard")
}

const SONIOX_LIVE_MODEL: &str = "stt-rt-v5";
const SONIOX_FILE_MODEL: &str = "stt-async-v5";

fn soniox_live_options() -> SonioxRealtimeOptions {
    SonioxRealtimeOptions {
        enable_speaker_diarization: false,
        show_preview: false,
        ..SonioxRealtimeOptions::default()
    }
}

async fn soniox_live(key: &str, samples: &[f32]) -> Result<String, String> {
    let manager = SonioxRealtimeManager::new(app()).map_err(|error| error.to_string())?;
    let (chunks, on_chunk) = chunk_collector();
    manager
        .start_session(
            LIVE_SOUND_TRANSCRIPTION_BINDING_ID,
            key,
            SONIOX_LIVE_MODEL,
            soniox_live_options(),
            Some(on_chunk),
        )
        .map_err(|error| format!("{error:#}"))?;
    feed_live(samples, |frame| manager.push_audio_frame(frame)).await;
    let started = Instant::now();
    let text = manager
        .finish_session(FINISH_TIMEOUT_MS)
        .await
        .map_err(|error| format!("{error:#}"))?;
    let chunks = chunks.lock().clone();
    check_live(&text, &chunks, started.elapsed())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "paid Soniox requests with the saved key"]
async fn stt_paid_soniox() {
    let key = crate::secure_keys::get_soniox_api_key();
    assert!(!key.trim().is_empty(), "no saved Soniox key");
    let samples = speech().await;
    let mut report = Report::default();

    report.record("Soniox live", soniox_live(&key, samples).await);

    let manager = SonioxRealtimeManager::new(app()).expect("Soniox live manager");
    let guard = replay_guard(-9_101, samples);
    report.record(
        "Soniox history replay",
        timed_text(manager.transcribe_history_replay(
            samples,
            &key,
            SONIOX_LIVE_MODEL,
            soniox_live_options(),
            &guard.replay,
            FINISH_TIMEOUT_MS,
        ))
        .await,
    );
    drop(guard);

    let files = SonioxSttManager::new(app()).expect("Soniox manager");
    report.record(
        "Soniox recording",
        timed_text(files.transcribe(
            None,
            &key,
            SONIOX_LIVE_MODEL,
            REQUEST_TIMEOUT_SECONDS,
            samples,
            Some("en"),
            None,
        ))
        .await,
    );
    report.record(
        "Soniox file",
        timed_text(async {
            files
                .transcribe_file_async(
                    None,
                    &key,
                    SONIOX_FILE_MODEL,
                    REQUEST_TIMEOUT_SECONDS,
                    samples,
                    Some("en"),
                    SonioxAsyncTranscriptionOptions::default(),
                )
                .await
                .map(|transcript| transcript.text)
        })
        .await,
    );
    report.assert_ok();
}

const DEEPGRAM_MODEL: &str = "nova-3";

async fn deepgram_live(key: &str, samples: &[f32]) -> Result<String, String> {
    let manager = DeepgramRealtimeManager::new(app()).map_err(|error| error.to_string())?;
    let (chunks, on_chunk) = chunk_collector();
    manager
        .start_session(
            LIVE_SOUND_TRANSCRIPTION_BINDING_ID,
            key,
            DEEPGRAM_MODEL,
            DeepgramRealtimeOptions::default(),
            Some(on_chunk),
        )
        .map_err(|error| format!("{error:#}"))?;
    feed_live(samples, |frame| manager.push_audio_frame(frame)).await;
    let started = Instant::now();
    let text = manager
        .finish_session(FINISH_TIMEOUT_MS)
        .await
        .map_err(|error| format!("{error:#}"))?;
    let chunks = chunks.lock().clone();
    check_live(&text, &chunks, started.elapsed())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "paid Deepgram requests with the saved key"]
async fn stt_paid_deepgram() {
    let key = crate::secure_keys::get_deepgram_api_key();
    assert!(!key.trim().is_empty(), "no saved Deepgram key");
    let samples = speech().await;
    let mut report = Report::default();

    report.record("Deepgram live", deepgram_live(&key, samples).await);

    let manager = DeepgramRealtimeManager::new(app()).expect("Deepgram live manager");
    let guard = replay_guard(-9_201, samples);
    report.record(
        "Deepgram history replay",
        timed_text(manager.transcribe_history_replay(
            samples,
            &key,
            DEEPGRAM_MODEL,
            DeepgramRealtimeOptions::default(),
            &guard.replay,
            FINISH_TIMEOUT_MS,
        ))
        .await,
    );
    drop(guard);

    let files = DeepgramSttManager::new(app()).expect("Deepgram manager");
    report.record(
        "Deepgram recording",
        timed_text(files.transcribe(
            None,
            &key,
            DEEPGRAM_MODEL,
            REQUEST_TIMEOUT_SECONDS,
            samples,
            Some("en"),
            DeepgramTranscriptionOptions::default(),
        ))
        .await,
    );
    let wav = encode_wav_bytes(samples).expect("WAV");
    report.record(
        "Deepgram file",
        timed_text(async {
            files
                .transcribe_prerecorded_bytes(
                    None,
                    &key,
                    DEEPGRAM_MODEL,
                    REQUEST_TIMEOUT_SECONDS,
                    &wav,
                    Some("en"),
                    DeepgramTranscriptionOptions::default(),
                )
                .await
                .map(|transcript| transcript.text)
        })
        .await,
    );
    report.assert_ok();
}

fn gemini_settings(preset: &str, model: &str) -> AppSettings {
    let mut settings = get_default_settings();
    settings.remote_stt.provider_preset = preset.to_string();
    settings.remote_stt.base_url = remote_stt_base_url_for_preset(preset).unwrap_or_default().to_string();
    settings.remote_stt.model_id = model.to_string();
    settings
}

fn gemini_key(settings: &AppSettings) -> Option<String> {
    get_remote_stt_api_key(&settings.remote_stt)
        .ok()
        .filter(|key| !key.trim().is_empty())
}

/// Dictation options as the app builds them, without the OS input language.
fn gemini_live_options(settings: &AppSettings) -> Result<GeminiRealtimeOptions, String> {
    let config = resolve_effective_config(settings, None, GeminiWorkflow::Dictation, None)?;
    Ok(GeminiRealtimeOptions {
        model: settings.remote_stt.model_id.clone(),
        language: config.language_code,
        preset: settings.remote_stt.provider_preset.clone(),
        custom_vocabulary: config.custom_vocabulary,
        mode: config.mode,
        validation_error: None,
    })
}

async fn gemini_live(key: &str, settings: &AppSettings, samples: &[f32]) -> Result<String, String> {
    let manager = GeminiRealtimeManager::new(app()).map_err(|error| error.to_string())?;
    let (chunks, on_chunk) = chunk_collector();
    manager
        .start_session(
            LIVE_SOUND_TRANSCRIPTION_BINDING_ID,
            None,
            key,
            gemini_live_options(settings)?,
            Some(on_chunk),
        )
        .map_err(|error| format!("{error:#}"))?;
    feed_live(samples, |frame| manager.push_audio_frame(frame)).await;
    let started = Instant::now();
    let text = manager
        .finish_session(FINISH_TIMEOUT_MS)
        .await
        .map_err(|error| format!("{error:#}"))?;
    let chunks = chunks.lock().clone();
    check_live(&text, &chunks, started.elapsed())
}

async fn gemini_replay(key: &str, settings: &AppSettings, replay_id: i64, samples: &[f32]) -> Result<String, String> {
    let manager = GeminiRealtimeManager::new(app()).map_err(|error| error.to_string())?;
    let options = gemini_live_options(settings)?;
    let guard = replay_guard(replay_id, samples);
    timed_text(manager.transcribe_history_replay(samples, key, options, &guard.replay)).await
}

async fn gemini_recording(settings: &AppSettings, samples: &[f32]) -> Result<String, String> {
    let manager = RemoteSttManager::new(app()).map_err(|error| error.to_string())?;
    let config = resolve_effective_config(settings, None, GeminiWorkflow::Dictation, None)?;
    timed_text(manager.transcribe_with_operation(
        manager.start_operation(),
        &settings.remote_stt,
        samples,
        None,
        None,
        false,
        Some(config),
    ))
    .await
}

async fn gemini_file(settings: &AppSettings, samples: &[f32]) -> Result<String, String> {
    let manager = RemoteSttManager::new(app()).map_err(|error| error.to_string())?;
    let workflow = GeminiWorkflow::File { word_timestamps: false };
    let config = resolve_effective_config(settings, None, workflow, None)?;
    timed_text(async {
        manager
            .transcribe_file_with_operation(
                manager.start_operation(),
                &settings.remote_stt,
                samples,
                None,
                None,
                false,
                false,
                Some(config),
            )
            .await
            .map(|transcription| transcription.text)
    })
    .await
}

async fn gemini_checks(
    report: &mut Report,
    route: &str,
    preset: &str,
    live_model: &str,
    file_model: &str,
    replay_id: i64,
    samples: &[f32],
) {
    let live = gemini_settings(preset, live_model);
    let Some(key) = gemini_key(&live) else {
        println!("SKIP Gemini via {route}: no saved key");
        return;
    };
    let files = gemini_settings(preset, file_model);
    report.record(&format!("Gemini via {route} live"), gemini_live(&key, &live, samples).await);
    report.record(
        &format!("Gemini via {route} history replay"),
        gemini_replay(&key, &live, replay_id, samples).await,
    );
    report.record(&format!("Gemini via {route} recording"), gemini_recording(&files, samples).await);
    report.record(&format!("Gemini via {route} file"), gemini_file(&files, samples).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "paid Gemini 3.5 Transcribe requests with the saved keys"]
async fn stt_paid_gemini() {
    let vercel = gemini_settings(REMOTE_STT_PRESET_VERCEL, GEMINI_LIVE_DEFAULT_MODEL);
    assert!(gemini_key(&vercel).is_some(), "no saved Vercel AI Gateway key");
    let samples = speech().await;
    let mut report = Report::default();
    gemini_checks(
        &mut report,
        "Vercel",
        REMOTE_STT_PRESET_VERCEL,
        GEMINI_LIVE_DEFAULT_MODEL,
        REMOTE_STT_VERCEL_DEFAULT_MODEL,
        -9_301,
        samples,
    )
    .await;
    gemini_checks(
        &mut report,
        "Google",
        REMOTE_STT_PRESET_GOOGLE,
        GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
        REMOTE_STT_GOOGLE_DEFAULT_MODEL,
        -9_302,
        samples,
    )
    .await;
    report.assert_ok();
}
