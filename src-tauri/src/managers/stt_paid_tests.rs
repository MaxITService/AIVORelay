//! Paid speech-to-text requests with saved provider keys: synthesized speech or an
//! explicitly supplied MP3-derived speech fixture. Network tests are ignored by default:
//! `./test-local.ps1 -LibOnly -Filter stt_paid,--ignored,--nocapture`
use super::deepgram_realtime::{DeepgramRealtimeManager, DeepgramRealtimeOptions};
use super::deepgram_stt::{DeepgramSttManager, DeepgramTranscriptionOptions};
use super::gemini_realtime::{
    GeminiRealtimeManager, GeminiRealtimeOptions, GEMINI_LIVE_DEFAULT_MODEL,
    GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
};
use super::history_replay::HistoryReplayGuard;
use super::remote_stt::{get_remote_stt_api_key, RemoteSttManager};
use super::reson8_realtime::{
    Reson8RealtimeManager, Reson8RealtimeOptions, RESON8_LIVE_FINALIZE_TIMEOUT_MS,
};
use super::soniox_realtime::{SonioxRealtimeManager, SonioxRealtimeOptions};
use super::soniox_stt::{SonioxAsyncTranscriptionOptions, SonioxSttManager};
use crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID;
use crate::audio_toolkit::audio::{encode_wav_bytes, FrameResampler};
use crate::gemini_config::{resolve_effective_config, GeminiWorkflow};
use crate::settings::{get_default_settings, AppSettings, TextReplacement};
use crate::stream_chunk_processor::StreamChunkProcessor;
use crate::url_security::{
    remote_stt_base_url_for_preset, REMOTE_STT_GOOGLE_DEFAULT_MODEL, REMOTE_STT_PRESET_GOOGLE,
    REMOTE_STT_PRESET_RESON8, REMOTE_STT_PRESET_VERCEL, REMOTE_STT_RESON8_DEFAULT_MODEL,
    REMOTE_STT_VERCEL_DEFAULT_MODEL,
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
const FINISH_TIMEOUT_MS: u32 = 20_000;
const REQUEST_TIMEOUT_SECONDS: u32 = 120;

/// A real app handle without windows or plugins, enough for the provider managers.
fn app() -> &'static AppHandle {
    static APP: OnceLock<AppHandle> = OnceLock::new();
    APP.get_or_init(|| {
        crate::tls::initialize();
        let builder = tauri::Builder::default();
        // `any_thread` exists only on Windows and Linux; macOS tests must stay on
        // the main thread.
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        let builder = builder.any_thread();
        let app = builder
            .manage(std::sync::Mutex::new(crate::session_manager::SessionState::Idle))
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
    feed_live_with_frame_ms(samples, 30, push).await;
}

async fn feed_live_with_frame_ms(samples: &[f32], frame_ms: u64, push: impl Fn(Vec<f32>)) {
    let mut tick = tokio::time::interval(Duration::from_millis(frame_ms));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    for frame in samples.chunks(SAMPLE_RATE * frame_ms as usize / 1000) {
        tick.tick().await;
        push(frame.to_vec());
    }
}

/// Ordered word recall against an independently supplied speech reference.
/// Repeated words count separately; extra hypotheses cannot hide missing speech.
fn ordered_speech_recall(reference: &str, transcript: &str) -> (usize, usize) {
    let reference = words(reference);
    let transcript = words(transcript);
    let mut matched = vec![0usize; transcript.len() + 1];
    for expected in &reference {
        let mut diagonal = 0;
        for (index, heard) in transcript.iter().enumerate() {
            let previous = matched[index + 1];
            matched[index + 1] = if expected == heard {
                diagonal + 1
            } else {
                matched[index + 1].max(matched[index])
            };
            diagonal = previous;
        }
    }
    (*matched.last().unwrap(), reference.len())
}

#[test]
fn speech_recall_rejects_missing_sentences_and_reordered_repetitions() {
    assert_eq!(ordered_speech_recall("One two. One three.", "one two one three"), (4, 4));
    assert_eq!(ordered_speech_recall("One two. One three.", "one two three"), (3, 4));
    assert_eq!(ordered_speech_recall("One two. One three.", "three one two one"), (3, 4));
    assert_eq!(ordered_speech_recall("Hello world. Missing speech.", "Hello world."), (2, 4));
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

const SONIOX_LIVE_MODEL: &str = "stt-rt-v6";
const SONIOX_FILE_MODEL: &str = "stt-async-v6";

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

fn speech_replay_fixture() -> (Vec<f32>, String) {
    let path = std::env::var("AIVORELAY_STT_REPLAY_WAV")
        .expect("set AIVORELAY_STT_REPLAY_WAV to decoded 16 kHz mono PCM16 speech");
    let spec = hound::WavReader::open(&path).expect("open speech WAV").spec();
    assert_eq!((spec.sample_rate, spec.channels, spec.bits_per_sample), (16_000, 1, 16));
    let samples = crate::audio_toolkit::read_wav_samples(&path).expect("decode speech WAV");
    let reference_path = std::env::var("AIVORELAY_STT_REPLAY_EXPECTED")
        .expect("set AIVORELAY_STT_REPLAY_EXPECTED to a UTF-8 reference transcript");
    let reference = std::fs::read_to_string(reference_path).expect("read speech reference");
    assert!(words(&reference).len() >= 20, "use a meaningful reference transcript");
    let duration = samples.len() as f64 / SAMPLE_RATE as f64;
    assert!((30.0..=90.0).contains(&duration), "use a bounded 30–90 second speech clip");
    (samples, reference)
}

/// Compare the same speech with Gemini's prerecorded model to isolate live-only loss.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "paid Gemini file API with a saved key and explicitly supplied speech fixture"]
async fn stt_paid_mp3_gemini_file() {
    let (samples, reference) = speech_replay_fixture();
    let mut settings = gemini_settings(REMOTE_STT_PRESET_VERCEL, REMOTE_STT_VERCEL_DEFAULT_MODEL);
    settings.gemini_file_mode = crate::settings::GeminiTranscriptionMode::Smart;
    let key = gemini_key(&settings).expect("no saved Vercel STT key available");
    let manager = RemoteSttManager::new(app()).expect("Gemini file manager");
    let config = resolve_effective_config(
        &settings, None, GeminiWorkflow::File { word_timestamps: false }, None,
    ).expect("Gemini file configuration");
    let started = Instant::now();
    let result = manager.transcribe_file_with_operation(
        manager.start_operation(), &settings.remote_stt, &samples,
        None, None, false, false, Some(config),
    ).await;
    let text = result.unwrap_or_else(|error| panic!("{}",
        super::remote_stt::redact_remote_stt_api_key(&format!("{error:#}"), &key),
    )).text;
    let (matched, expected) = ordered_speech_recall(&reference, &text);
    println!("Gemini file: {matched}/{expected} reference words in {} ms; {text:?}", ms(started.elapsed()));
    assert!((matched as f64 / expected as f64) >= MIN_RECALL,
        "missing speech: ordered reference recall {matched}/{expected}");
}

/// Explicit paid smoke: decode a speech MP3 to 16 kHz mono PCM16 WAV first,
/// then set AIVORELAY_STT_REPLAY_WAV to that WAV. No microphone or paste is used.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "paid live APIs with saved keys and an explicitly supplied speech fixture"]
async fn stt_paid_mp3_live_chunks() {
    use super::elevenlabs_realtime::{
        ElevenLabsRealtimeManager, ElevenLabsRealtimeOptions, ELEVENLABS_LIVE_FINALIZE_TIMEOUT_MS,
    };
    use crate::text_replacement_decapitalize as decapitalize;

    let _guard = decapitalize::TEST_MUTEX.lock().unwrap();
    let (samples, reference) = speech_replay_fixture();
    let frame_ms = std::env::var("AIVORELAY_STT_REPLAY_FRAME_MS")
        .map(|value| value.parse::<u64>().expect("frame duration in milliseconds"))
        .unwrap_or(30);
    assert!((10..=200).contains(&frame_ms), "use bounded 10–200 ms frames");
    let duration = samples.len() as f64 / SAMPLE_RATE as f64;
    println!("MP3 replay: {duration:.2} seconds, {frame_ms} ms audio frames, real-time pacing");

    #[derive(Default)]
    struct Output {
        chunks: Vec<(u128, String)>,
        processor: StreamChunkProcessor,
        inserted: String,
        lowercase_at: Vec<usize>,
        raw_len: usize,
    }
    let mut report = Report::default();
    let providers = std::env::var("AIVORELAY_STT_REPLAY_PROVIDERS")
        .unwrap_or_else(|_| "elevenlabs,vercel".to_string());
    let providers: Vec<_> = providers.split(',').map(str::trim).collect();
    assert!(!providers.is_empty() && providers.iter().all(|provider| matches!(*provider, "elevenlabs" | "vercel" | "google")),
        "replay providers must be elevenlabs, vercel, and/or google");
    for provider in providers {
        let model = match provider {
            "elevenlabs" => "scribe_v3_live",
            "google" => GEMINI_LIVE_GOOGLE_DEFAULT_MODEL,
            _ => GEMINI_LIVE_DEFAULT_MODEL,
        };
        let mut settings = gemini_settings(provider, model);
        if let Ok(mode) = std::env::var("AIVORELAY_STT_REPLAY_MODE") {
            settings.gemini_dictation_mode = Some(match mode.as_str() {
                "smart" => crate::settings::GeminiTranscriptionMode::Smart,
                "verbatim" => crate::settings::GeminiTranscriptionMode::Verbatim,
                _ => panic!("replay mode must be smart or verbatim"),
            });
        }
        if let Ok(language) = std::env::var("AIVORELAY_STT_REPLAY_LANGUAGE") {
            settings.gemini_language_code = language;
        }
        let Ok(key) = get_remote_stt_api_key(&settings.remote_stt) else {
            report.record(provider, Err("no saved STT key available".to_string()));
            continue;
        };
        let _ = decapitalize::maybe_decapitalize_next_chunk_standard("Reset");
        let output = Arc::new(Mutex::new(Output::default()));
        let collected = Arc::clone(&output);
        let start = Instant::now();
        let callback: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |chunk| {
            let mut output = collected.lock();
            if let Some((offset, first)) = chunk.char_indices().find(|(_, ch)| ch.is_alphabetic()) {
                if first.is_uppercase() {
                    let at = output.raw_len + offset;
                    output.lowercase_at.push(at);
                    // Rearm within this same recording before each uppercase speech chunk.
                    decapitalize::mark_edit_key_pressed(60_000, false);
                }
            }
            let delta = output.processor.push_chunk(&chunk);
            output.inserted.push_str(&delta);
            output.raw_len += chunk.len();
            output.chunks.push((start.elapsed().as_millis(), chunk));
        });
        let finish_started;
        let result = if provider == "elevenlabs" {
            let manager = ElevenLabsRealtimeManager::new(app());
            match manager.start_session(
                LIVE_SOUND_TRANSCRIPTION_BINDING_ID, None, &key,
                ElevenLabsRealtimeOptions { language_code: Some("en".to_string()), ..Default::default() },
                Some(callback),
            ) {
                Ok(()) => {
                    feed_live_with_frame_ms(&samples, frame_ms, |frame| manager.push_audio_frame(frame)).await;
                    finish_started = Instant::now();
                    manager.finish_session(ELEVENLABS_LIVE_FINALIZE_TIMEOUT_MS).await
                }
                Err(error) => { finish_started = Instant::now(); Err(error) }
            }
        } else {
            let manager = GeminiRealtimeManager::new(app()).expect("Gemini manager");
            match manager.start_session(
                LIVE_SOUND_TRANSCRIPTION_BINDING_ID, None, &key,
                gemini_live_options(&settings).expect("Gemini options"), Some(callback),
            ) {
                Ok(()) => {
                    feed_live_with_frame_ms(&samples, frame_ms, |frame| manager.push_audio_frame(frame)).await;
                    finish_started = Instant::now();
                    manager.finish_session(FINISH_TIMEOUT_MS).await
                }
                Err(error) => { finish_started = Instant::now(); Err(error) }
            }
        };
        let finalize_ms = finish_started.elapsed().as_millis();
        let mut output = output.lock();
        let tail = output.processor.flush();
        output.inserted.push_str(&tail);
        let raw: String = output.chunks.iter().map(|(_, text)| text.as_str()).collect();
        println!("{provider}: chunk arrivals {:?}", output.chunks);
        let check = result.map_err(|error| {
            super::remote_stt::redact_remote_stt_api_key(&format!("{error:#}"), &key)
        }).and_then(|text| {
            println!("{provider}: final transcript {text:?}");
            let (matched, expected) = ordered_speech_recall(&reference, &text);
            println!("{provider}: ordered reference recall {matched}/{expected} words");
            if output.chunks.len() < 2 || output.chunks[0].0 >= (duration * 1000.0) as u128 {
                return Err("no incremental output while audio was being fed".to_string());
            }
            if !raw.split_whitespace().eq(text.split_whitespace()) {
                return Err("streamed chunks differ from final transcript".to_string());
            }
            if (matched as f64 / expected as f64) < MIN_RECALL {
                return Err(format!("missing speech: ordered reference recall {matched}/{expected}, requires at least {:.0}%", MIN_RECALL * 100.0));
            }
            if output.lowercase_at.len() < 2 {
                return Err("fewer than two uppercase speech chunks to exercise rearming".to_string());
            }
            let mut expected = raw.clone();
            for &offset in output.lowercase_at.iter().rev() {
                let first = raw[offset..].chars().next().unwrap();
                expected.replace_range(offset..offset + first.len_utf8(), &first.to_lowercase().to_string());
            }
            if output.inserted != expected || decapitalize::is_any_trigger_armed_now() {
                return Err("decapitalization changed unexpected letters or failed to consume a rearmed trigger".to_string());
            }
            Ok(format!("{} real chunks, {} words, finalization {finalize_ms} ms, {} decapitalization triggers consumed in one recording", output.chunks.len(), words(&text).len(), output.lowercase_at.len()))
        });
        let _ = decapitalize::maybe_decapitalize_next_chunk_standard("Reset");
        report.record(provider, check);
    }
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

fn reson8_settings() -> AppSettings {
    gemini_settings(REMOTE_STT_PRESET_RESON8, REMOTE_STT_RESON8_DEFAULT_MODEL)
}

/// `interim` requests partial text as Live Preview and Live Monitor do; only finals may be inserted.
/// The preview window itself needs the settings store, which this headless app lacks.
async fn reson8_live(key: &str, samples: &[f32], interim: bool) -> Result<String, String> {
    let manager = Reson8RealtimeManager::new(app()).map_err(|error| error.to_string())?;
    let (chunks, on_chunk) = chunk_collector();
    let options = Reson8RealtimeOptions {
        language: Some("en".to_string()),
        include_interim: interim,
        ..Reson8RealtimeOptions::default()
    };
    manager
        .start_session("transcribe", None, key, options, Some(on_chunk))
        .map_err(|error| format!("{error:#}"))?;
    feed_live(samples, |frame| manager.push_audio_frame(frame)).await;
    let started = Instant::now();
    let text = manager
        .finish_session(RESON8_LIVE_FINALIZE_TIMEOUT_MS)
        .await
        .map_err(|error| format!("{error:#}"))?;
    if manager.has_active_session() {
        return Err("session still active after finish".to_string());
    }
    let chunks = chunks.lock().clone();
    check_live(&text, &chunks, started.elapsed())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "paid Reson8 Rumble-2 requests with the saved key"]
async fn stt_paid_reson8() {
    let settings = reson8_settings();
    let key = gemini_key(&settings).expect("no saved Reson8 key");
    let samples = speech().await;
    let mut report = Report::default();

    report.record("Reson8 live", reson8_live(&key, samples, false).await);
    report.record("Reson8 live with interim", reson8_live(&key, samples, true).await);

    let manager = Reson8RealtimeManager::new(app()).expect("Reson8 live manager");
    let guard = replay_guard(-9_401, samples);
    report.record(
        "Reson8 history replay",
        timed_text(manager.transcribe_history_replay(
            samples,
            &key,
            Reson8RealtimeOptions { include_interim: true, show_preview: true, ..Default::default() },
            &guard.replay,
        ))
        .await,
    );
    drop(guard);

    report.record(
        "Reson8 recording",
        timed_text(async {
            let manager = RemoteSttManager::new(app())?;
            manager
                .transcribe_with_operation(
                    manager.start_operation(), &settings.remote_stt, samples,
                    None, Some("en".to_string()), false, None,
                )
                .await
        })
        .await,
    );
    report.record(
        "Reson8 file",
        timed_text(async {
            let manager = RemoteSttManager::new(app())?;
            manager
                .transcribe_file_with_operation(
                    manager.start_operation(), &settings.remote_stt, samples,
                    None, Some("en".to_string()), false, false, None,
                )
                .await
                .map(|transcription| transcription.text)
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
