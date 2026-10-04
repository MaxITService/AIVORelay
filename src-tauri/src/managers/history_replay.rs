use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use futures_util::Sink;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::{Instant, Sleep};
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_util::sync::CancellationToken;

pub(crate) const HISTORY_REPLAY_BINDING: &str = "history_replay";
pub(crate) const HISTORY_REPLAY_CANCELLED: &str = "History replay cancelled";
const SAMPLE_RATE: usize = 16_000;
// The enhanced microphone pipeline also delivers 30 ms frames.
const FRAME_SAMPLES: usize = 480;

static REPLAYS: Lazy<Mutex<HashMap<i64, Arc<HistoryReplay>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Serialize)]
pub(crate) struct ReplayProgress {
    pub id: i64,
    pub sent_seconds: f64,
    pub duration_seconds: f64,
    pub phase: &'static str,
}

pub(crate) struct HistoryReplay {
    cancel: CancellationToken,
    progress: Mutex<ReplayProgress>,
    on_progress: Box<dyn Fn(ReplayProgress) + Send + Sync>,
}

pub(crate) struct HistoryReplayGuard {
    pub replay: Arc<HistoryReplay>,
}

impl HistoryReplayGuard {
    pub fn start(app: &AppHandle, id: i64, sample_count: usize) -> Result<Self> {
        let app = app.clone();
        let replay = Arc::new(HistoryReplay {
            cancel: CancellationToken::new(),
            progress: Mutex::new(ReplayProgress {
                id,
                sent_seconds: 0.0,
                duration_seconds: sample_count as f64 / SAMPLE_RATE as f64,
                phase: "connecting",
            }),
            on_progress: Box::new(move |progress| {
                let _ = app.emit("history-replay-progress", progress);
            }),
        });
        let mut replays = REPLAYS.lock();
        if replays.contains_key(&id) {
            return Err(anyhow!("This recording is already being re-transcribed"));
        }
        replays.insert(id, Arc::clone(&replay));
        drop(replays);
        replay.emit("connecting");
        Ok(Self { replay })
    }
}

impl Drop for HistoryReplayGuard {
    fn drop(&mut self) {
        self.replay.cancel.cancel();
        REPLAYS.lock().remove(&self.replay.progress.lock().id);
    }
}

pub(crate) fn cancel(id: i64) {
    if let Some(replay) = REPLAYS.lock().get(&id) {
        replay.cancel.cancel();
    }
}

impl HistoryReplay {
    pub async fn cancelled(&self) {
        self.cancel.cancelled().await;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    pub fn emit(&self, phase: &'static str) {
        let mut progress = self.progress.lock();
        progress.phase = phase;
        let snapshot = progress.clone();
        drop(progress);
        (self.on_progress)(snapshot);
    }

    fn audio_sent(&self, seconds: f64) {
        let mut progress = self.progress.lock();
        let previous_seconds = progress.sent_seconds;
        let previous_phase = progress.phase;
        progress.sent_seconds = (progress.sent_seconds + seconds).min(progress.duration_seconds);
        progress.phase = if progress.duration_seconds - progress.sent_seconds < 0.0001 {
            "finalizing"
        } else {
            "sending"
        };
        if progress.phase == previous_phase
            && (progress.sent_seconds * 4.0).floor() == (previous_seconds * 4.0).floor()
        {
            return;
        }
        let snapshot = progress.clone();
        drop(progress);
        (self.on_progress)(snapshot);
    }

    // The transport stays in this future: cancelling it drops both socket halves,
    // including during setup or finalization, without touching microphone sessions.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_audio(
        &self,
        samples: &[f32],
        audio_tx: mpsc::Sender<Vec<u8>>,
        encode: impl Fn(&[f32]) -> Vec<u8>,
        finish: impl FnOnce() -> Result<()>,
        drive: impl Future<Output = Result<()>>,
        final_text: Arc<Mutex<String>>,
        timeout_ms: u32,
    ) -> Result<String> {
        let feed = async move {
            for frame in samples.chunks(FRAME_SAMPLES) {
                audio_tx.send(encode(frame)).await
                    .map_err(|_| anyhow!("Live transcription stopped accepting audio"))?;
            }
            drop(audio_tx);
            finish()
        };
        tokio::pin!(drive);
        tokio::pin!(feed);
        tokio::select! {
            biased;
            _ = self.cancelled() => return Err(anyhow!(HISTORY_REPLAY_CANCELLED)),
            result = &mut drive => {
                result?;
                return Err(anyhow!("Live transcription ended before all recording audio was sent"));
            }
            result = &mut feed => result?,
        }
        tokio::select! {
            biased;
            _ = self.cancelled() => return Err(anyhow!(HISTORY_REPLAY_CANCELLED)),
            result = tokio::time::timeout(
                Duration::from_millis(timeout_ms.clamp(500, 20_000) as u64),
                &mut drive,
            ) => {
                result.map_err(|_| anyhow!("Live re-transcription finalization timed out"))??;
            }
        }
        Ok(final_text.lock().trim().to_string())
    }
}

// Use the existing provider loops and messages, but pace actual socket writes.
// A stalled connection resets the deadline, so buffered audio never catches up
// in a burst. Progress advances only after the socket has flushed each frame.
pub(crate) struct ReplaySink<S> {
    sink: S,
    replay: Arc<HistoryReplay>,
    sample_rate: u32,
    pending: Option<Message>,
    pending_seconds: f64,
    sleep: Option<Pin<Box<Sleep>>>,
    flushing_seconds: f64,
}

impl<S> ReplaySink<S> {
    pub fn new(sink: S, replay: &Arc<HistoryReplay>, sample_rate: u32) -> Self {
        Self {
            sink,
            replay: Arc::clone(replay),
            sample_rate,
            pending: None,
            pending_seconds: 0.0,
            sleep: None,
            flushing_seconds: 0.0,
        }
    }
}

fn audio_bytes(message: &Message) -> usize {
    match message {
        Message::Binary(bytes) => bytes.len(),
        Message::Text(text) => {
            let Ok(payload) = serde_json::from_str::<serde_json::Value>(text) else { return 0; };
            let audio = if payload["type"] == "input_audio_buffer.append" {
                payload["audio"].as_str()
            } else {
                payload.pointer("/realtimeInput/audio/data").and_then(|value| value.as_str())
            };
            audio.and_then(|value| STANDARD.decode(value).ok()).map_or(0, |bytes| bytes.len())
        }
        _ => 0,
    }
}

impl<S: Sink<Message, Error = Error> + Unpin> Sink<Message> for ReplaySink<S> {
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Error> {
        let this = self.get_mut();
        this.pending_seconds = audio_bytes(&item) as f64 / 2.0 / this.sample_rate as f64;
        if this.pending_seconds > 0.0 {
            this.sleep = Some(Box::pin(tokio::time::sleep_until(
                Instant::now() + Duration::from_secs_f64(this.pending_seconds),
            )));
        }
        this.pending = Some(item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this = self.get_mut();
        if let Some(sleep) = this.sleep.as_mut() {
            std::task::ready!(sleep.as_mut().poll(cx));
            this.sleep = None;
        }
        if this.pending.is_some() {
            std::task::ready!(Pin::new(&mut this.sink).poll_ready(cx))?;
            Pin::new(&mut this.sink).start_send(this.pending.take().unwrap())?;
            this.flushing_seconds = this.pending_seconds;
            this.pending_seconds = 0.0;
        }
        std::task::ready!(Pin::new(&mut this.sink).poll_flush(cx))?;
        if this.flushing_seconds > 0.0 {
            this.replay.audio_sent(this.flushing_seconds);
            this.flushing_seconds = 0.0;
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        std::task::ready!(self.as_mut().poll_flush(cx))?;
        Pin::new(&mut self.get_mut().sink).poll_close(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::SinkExt;

    #[derive(Clone, Default)]
    struct RecordingSink(Arc<Mutex<Vec<(Instant, Message)>>>);

    impl Sink<Message> for RecordingSink {
        type Error = Error;

        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Error> {
            self.0.lock().push((Instant::now(), item));
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(Ok(()))
        }
    }

    fn replay(duration_seconds: f64) -> Arc<HistoryReplay> {
        Arc::new(HistoryReplay {
            cancel: CancellationToken::new(),
            progress: Mutex::new(ReplayProgress {
                id: 1, sent_seconds: 0.0, duration_seconds, phase: "connecting",
            }),
            on_progress: Box::new(|_| {}),
        })
    }

    #[tokio::test(start_paused = true)]
    async fn silent_frames_keep_their_duration_and_do_not_catch_up_after_a_stall() {
        let replay = replay(0.09);
        let recorded = RecordingSink::default();
        let mut sink = ReplaySink::new(recorded.clone(), &replay, 16_000);
        let started = Instant::now();
        sink.send(Message::Binary(vec![0; 960].into())).await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        sink.send(Message::Binary(vec![0; 960].into())).await.unwrap();
        sink.send(Message::Binary(vec![0; 960].into())).await.unwrap();

        let frames = recorded.0.lock();
        assert_eq!(frames.len(), 3);
        assert!(frames[0].0 - started >= Duration::from_millis(30));
        assert!(frames[1].0 - frames[0].0 >= Duration::from_millis(1_030));
        assert!(frames[2].0 - frames[1].0 >= Duration::from_millis(30));
        let progress = replay.progress.lock();
        assert!((progress.sent_seconds - 0.09).abs() < 0.0001);
        assert_eq!(progress.phase, "finalizing");
    }

    #[tokio::test(start_paused = true)]
    async fn openai_audio_counts_24khz_samples_and_control_messages_do_not_advance_progress() {
        let replay = replay(0.03);
        let recorded = RecordingSink::default();
        let mut sink = ReplaySink::new(recorded, &replay, 24_000);
        let started = Instant::now();
        sink.send(Message::Text(r#"{"type":"input_audio_buffer.commit"}"#.into())).await.unwrap();
        assert_eq!(Instant::now(), started);
        assert_eq!(replay.progress.lock().sent_seconds, 0.0);
        let append = serde_json::json!({
            "type": "input_audio_buffer.append",
            "audio": STANDARD.encode(vec![0; 1_440]),
        });
        sink.send(Message::Text(append.to_string().into())).await.unwrap();
        assert!(Instant::now() - started >= Duration::from_millis(30));
        assert!((replay.progress.lock().sent_seconds - 0.03).abs() < 0.0001);
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_during_finalization_discards_partial_text_immediately() {
        let replay = replay(0.0);
        let (audio_tx, _audio_rx) = mpsc::channel(1);
        let replay_for_finish = Arc::clone(&replay);
        let started = Instant::now();
        let result = replay.run_audio(
            &[], audio_tx, |_| Vec::new(),
            move || { replay_for_finish.cancel.cancel(); Ok(()) },
            std::future::pending(), Arc::new(Mutex::new("partial transcript".to_string())), 8_000,
        ).await;
        assert_eq!(result.unwrap_err().to_string(), HISTORY_REPLAY_CANCELLED);
        assert_eq!(Instant::now(), started);
    }
}
