//! Deterministic transport tests: no desktop, sockets, microphone, or credentials.
use super::*;
use crate::soniox_stream_processor::SonioxStreamProcessor;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_tungstenite::tungstenite::Error as WebSocketError;

#[derive(Default)]
struct WireState {
    sent: Vec<Message>,
    closes: usize,
    fail_next_send: bool,
    fail_close: bool,
}

struct MockWrite(Arc<Mutex<WireState>>);

impl Sink<Message> for MockWrite {
    type Error = WebSocketError;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        let mut wire = self.0.lock();
        if std::mem::take(&mut wire.fail_next_send) {
            return Err(WebSocketError::ConnectionClosed);
        }
        wire.sent.push(item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut wire = self.0.lock();
        wire.closes += 1;
        Poll::Ready(if wire.fail_close { Err(WebSocketError::ConnectionClosed) } else { Ok(()) })
    }
}

struct MockRead(mpsc::UnboundedReceiver<Result<Message, WebSocketError>>);

impl Stream for MockRead {
    type Item = Result<Message, WebSocketError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_recv(cx)
    }
}

#[derive(Default)]
struct Observed {
    chunks: Vec<String>,
    inserted: String,
    live_text: Vec<String>,
    setup_count: usize,
    stop_count: usize,
    completions: Vec<Value>,
    processor: SonioxStreamProcessor,
}

struct TestSession {
    task: Pin<Box<dyn Future<Output = Result<()>>>>,
    network: Option<mpsc::UnboundedSender<Result<Message, WebSocketError>>>,
    audio: Option<mpsc::Sender<Vec<u8>>>,
    control: mpsc::UnboundedSender<ControlMessage>,
    wire: Arc<Mutex<WireState>>,
    observed: Arc<Mutex<Observed>>,
    final_text: Arc<Mutex<String>>,
    completion: Arc<Mutex<Option<GeminiTimeLimitCompletion>>>,
    window: Arc<GeminiOutputWindow>,
    delivery: Arc<Mutex<(bool, bool, bool)>>,
}

impl TestSession {
    fn new(transport: GeminiLiveTransport) -> Self {
        Self::with_insertion(transport, true)
    }

    fn with_insertion(transport: GeminiLiveTransport, insertion: bool) -> Self {
        let (network, rx) = mpsc::unbounded_channel();
        let (audio, audio_rx) = mpsc::channel(AUDIO_QUEUE_CAPACITY);
        let (control, control_rx) = mpsc::unbounded_channel();
        let wire = Arc::new(Mutex::new(WireState::default()));
        let observed = Arc::new(Mutex::new(Observed::default()));
        let final_text = Arc::new(Mutex::new(String::new()));
        let completion = Arc::new(Mutex::new(None));
        let window = Arc::new(GeminiOutputWindow::default());
        let delivery = Arc::new(Mutex::new((false, true, false)));
        let delivery_for_callback = Arc::clone(&delivery);
        let chunks = Arc::clone(&observed);
        let output_window = Arc::clone(&window);
        let on_chunk: FinalChunkCallback = Arc::new(move |chunk| {
            let mut observed = chunks.lock();
            observed.chunks.push(chunk.clone());
            let accepted = output_window.accept_chunk(chunk);
            if !accepted.is_empty() {
                let delta = observed.processor.push_chunk(&accepted);
                // Simulated paste sink; exercise the real chunk processor and
                // the second deadline check used before main-thread insertion.
                let (background, current, cancelled) = *delivery_for_callback.lock();
                if output_window.allows_delivery()
                    && crate::actions::gemini_stream_operation_allows_delivery(
                        background, || current, || cancelled,
                    )
                {
                    observed.processor.record_output_ending(delta.chars().last());
                    observed.inserted.push_str(&delta);
                }
            }
        });
        let text_events = Arc::clone(&observed);
        let setup_events = Arc::clone(&observed);
        let stop_events = Arc::clone(&observed);
        let completion_events = Arc::clone(&observed);
        let events = SessionEvents {
            live_text: Box::new(move |text| text_events.lock().live_text.push(text.to_string())),
            setup_complete: Box::new(move || setup_events.lock().setup_count += 1),
            time_limit_stop: Box::new(move || stop_events.lock().stop_count += 1),
            time_limit_completed: Box::new(move |payload| completion_events.lock().completions.push(payload)),
        };
        let mut write = MockWrite(Arc::clone(&wire));
        let mut read = MockRead(rx);
        let text_for_task = Arc::clone(&final_text);
        let completion_for_task = Arc::clone(&completion);
        let task = Box::pin(async move {
            GeminiRealtimeManager::drive_session_loop(
                &mut write, &mut read, audio_rx, control_rx, text_for_task,
                "transcribe_test".to_string(), insertion.then_some(on_chunk), transport,
                GeminiRealtimeOptions::default(), Duration::from_secs(10),
                completion_for_task, events,
            ).await
        });
        Self {
            task, network: Some(network), audio: Some(audio), control, wire,
            observed, final_text, completion, window, delivery,
        }
    }

    async fn pending(&mut self) {
        assert!(futures::poll!(self.task.as_mut()).is_pending(), "session ended unexpectedly");
    }

    fn frame(&self, payload: Value) {
        self.message(Message::Text(payload.to_string()));
    }

    fn message(&self, message: Message) {
        self.network.as_ref().unwrap().send(Ok(message)).unwrap();
    }

    async fn ready_google(&mut self) {
        self.pending().await;
        self.frame(json!({ "setupComplete": {} }));
        self.pending().await;
    }

    async fn finish_input(&mut self) {
        self.control.send(ControlMessage::Finish).unwrap();
        self.audio.take();
        self.pending().await;
    }

    async fn result(&mut self) -> Result<()> {
        match futures::poll!(self.task.as_mut()) {
            Poll::Ready(result) => result,
            Poll::Pending => panic!("expected session to finish without waiting"),
        }
    }

    fn sent_json(&self) -> Vec<Value> {
        self.wire.lock().sent.iter().filter_map(|message| {
            if let Message::Text(text) = message { Some(serde_json::from_str(text).unwrap()) } else { None }
        }).collect()
    }
}

fn run_test(test: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(async {
        tokio::time::pause();
        test.await;
    });
}

#[test]
fn google_queues_audio_until_setup_then_sends_base64_pcm_and_end_signal() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.audio.as_ref().unwrap().try_send(vec![0, 1, 254, 255]).unwrap();
        session.audio.as_ref().unwrap().try_send(Vec::new()).unwrap();
        session.pending().await;
        assert!(session.wire.lock().sent.is_empty());
        session.frame(json!({ "setup_complete": {} }));
        session.pending().await;
        assert_eq!(session.observed.lock().setup_count, 1);
        assert_eq!(session.sent_json(), vec![json!({ "realtimeInput": { "audio": {
            "mimeType": "audio/pcm;rate=16000", "data": "AAH+/w=="
        } } })]);
        session.finish_input().await;
        assert_eq!(session.sent_json().last(), Some(&live_audio_done_payload(GeminiLiveTransport::GoogleDirect)));
        session.frame(json!({ "serverContent": { "interactionStatus": "IDLE" } }));
        session.result().await.unwrap();
        assert_eq!(session.wire.lock().closes, 1);
        assert!(session.observed.lock().completions.is_empty());
    });
}

#[test]
fn gateway_splits_large_pcm_frames_without_loss_and_ignores_empty_audio() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::VercelGateway);
        let audio = vec![42; VERCEL_MAX_AUDIO_FRAME_BYTES * 2 + 7];
        session.audio.as_ref().unwrap().try_send(audio.clone()).unwrap();
        session.audio.as_ref().unwrap().try_send(Vec::new()).unwrap();
        session.pending().await;
        let wire = session.wire.lock();
        let frames: Vec<&Vec<u8>> = wire.sent.iter().map(|message| {
            let Message::Binary(bytes) = message else { panic!("expected binary audio") };
            bytes
        }).collect();
        assert_eq!(frames.iter().map(|bytes| bytes.len()).collect::<Vec<_>>(), vec![65536, 65536, 7]);
        assert_eq!(frames.into_iter().flat_map(|bytes| bytes.iter().copied()).collect::<Vec<_>>(), audio);
        drop(wire);
        session.finish_input().await;
        assert_eq!(session.sent_json(), vec![live_audio_done_payload(GeminiLiveTransport::VercelGateway)]);
        session.frame(json!({ "type": "finish" }));
        session.result().await.unwrap();
    });
}

#[test]
fn revised_gateway_finish_updates_transcript_without_reinserting_streamed_words() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::VercelGateway);
        session.frame(json!({ "type": "transcript-delta", "delta": "I scream." }));
        session.frame(json!({ "type": "transcript-final", "text": "Ice cream." }));
        session.pending().await;
        session.finish_input().await;
        session.frame(json!({ "type": "finish", "text": "Ice cream!" }));
        session.result().await.unwrap();
        assert_eq!(*session.final_text.lock(), "Ice cream!");
        assert_eq!(session.observed.lock().inserted, "I scream. ");
        assert_eq!(session.observed.lock().chunks, vec!["I scream.", " "]);
    });
}

#[test]
fn gateway_partial_revisions_are_preview_only_and_final_text_is_inserted_once() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::VercelGateway);
        session.frame(json!({ "type": "transcript-partial", "text": "write words" }));
        session.frame(json!({ "type": "transcript-partial", "text": "right words" }));
        session.pending().await;
        assert_eq!(*session.final_text.lock(), "right words");
        assert!(session.observed.lock().inserted.is_empty());
        session.frame(json!({ "type": "transcript-final", "text": "Right words." }));
        session.frame(json!({ "type": "finish" }));
        session.result().await.unwrap();
        assert_eq!(session.observed.lock().inserted, "Right words. ");
        assert_eq!(*session.final_text.lock(), "Right words. ");
    });
}

#[test]
fn gateway_final_segments_reset_delta_state_and_keep_sentence_boundaries() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::VercelGateway);
        for frame in [
            json!({ "type": "transcript-delta", "delta": "Hello." }),
            json!({ "type": "transcript-delta", "delta": "Next" }),
            json!({ "type": "transcript-final", "text": "ignored duplicate" }),
            json!({ "type": "transcript-final", "text": "Third." }),
            json!({ "type": "finish" }),
        ] { session.frame(frame); }
        session.result().await.unwrap();
        assert_eq!(session.observed.lock().inserted, "Hello. Next Third. ");
        assert_eq!(*session.final_text.lock(), "Hello. Next Third. ");
    });
}

#[test]
fn google_interim_revision_is_replaced_by_final_during_finalization_without_duplicates() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.ready_google().await;
        session.frame(json!({ "serverContent": { "interimInputTranscription": { "text": "write" } } }));
        session.pending().await;
        session.finish_input().await;
        session.frame(json!({ "serverContent": { "interimInputTranscription": "right" } }));
        session.frame(json!({ "serverContent": {
            "inputTranscription": { "text": "Right words.", "finished": true },
            "turnComplete": true, "interactionStatus": "IDLE"
        } }));
        session.result().await.unwrap();
        assert_eq!(*session.final_text.lock(), "Right words.");
        assert_eq!(session.observed.lock().inserted, "Right words. ");
        assert_eq!(session.observed.lock().chunks, vec!["Right words.", " "]);
    });
}

#[test]
fn google_fragmented_final_and_repeated_completion_markers_do_not_duplicate_segments() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.ready_google().await;
        for frame in [
            json!({ "server_content": { "input_transcription": "Hel" } }),
            json!({ "server_content": { "input_transcription": { "text": "lo.", "finished": true }, "turn_complete": true } }),
            json!({ "serverContent": { "turnComplete": true } }),
            json!({ "input_transcription": { "text": "Next.", "finished": true } }),
        ] { session.frame(frame); }
        session.pending().await;
        session.finish_input().await;
        session.frame(json!({ "server_content": { "interaction_status": "REQUIRES_ACTION" } }));
        session.result().await.unwrap();
        assert_eq!(session.observed.lock().inserted, "Hello. Next. ");
        assert_eq!(*session.final_text.lock(), "Hello. Next.");
    });
}

#[test]
fn google_turn_complete_can_commit_the_latest_interim_once() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.ready_google().await;
        session.finish_input().await;
        session.frame(json!({ "serverContent": {
            "interimInputTranscription": { "text": "  fallback words  " }, "turnComplete": true
        } }));
        session.result().await.unwrap();
        assert_eq!(*session.final_text.lock(), "fallback words");
        assert_eq!(session.observed.lock().chunks, vec!["fallback words", " "]);
    });
}

#[test]
fn ping_gets_matching_pong_and_non_text_frames_are_ignored_on_both_routes() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let mut session = TestSession::new(route);
            session.message(Message::Ping(vec![1, 2, 3]));
            session.message(Message::Pong(vec![4]));
            session.message(Message::Binary(vec![5]));
            session.frame(json!({ "unrecognized": true }));
            session.pending().await;
            assert_eq!(session.wire.lock().sent, vec![Message::Pong(vec![1, 2, 3])]);
            assert!(session.observed.lock().inserted.is_empty());
            session.control.send(ControlMessage::Cancel).unwrap();
            session.result().await.unwrap();
            assert_eq!(session.wire.lock().closes, 1);
        }
    });
}

#[test]
fn malformed_json_is_bounded_and_reported_instead_of_inserted() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let mut session = TestSession::new(route);
            session.message(Message::Text("é".repeat(300)));
            let error = session.result().await.unwrap_err().to_string();
            assert!(error.starts_with("Invalid Gemini 3.5 Transcribe Live payload:"));
            assert!(error.contains(&"é".repeat(200)));
            assert!(!error.contains(&"é".repeat(201)));
            assert!(session.observed.lock().inserted.is_empty());
        }
    });
}

#[test]
fn provider_errors_use_supported_shapes_and_never_become_completion_notices() {
    run_test(async {
        for (route, payload, expected) in [
            (GeminiLiveTransport::GoogleDirect, json!({ "error": { "message": "quota exceeded" } }), "quota exceeded"),
            (GeminiLiveTransport::GoogleDirect, json!({ "error": {} }), "Unknown Gemini"),
            (GeminiLiveTransport::VercelGateway, json!({ "type": "error", "error": { "message": "quota exceeded" } }), "quota exceeded"),
            (GeminiLiveTransport::VercelGateway, json!({ "type": "error", "error": "socket failed" }), "socket failed"),
            (GeminiLiveTransport::VercelGateway, json!({ "type": "error", "message": "top-level failure" }), "top-level failure"),
            (GeminiLiveTransport::VercelGateway, json!({ "type": "error" }), "Unknown streaming"),
        ] {
            let mut session = TestSession::new(route);
            session.frame(payload);
            assert!(session.result().await.unwrap_err().to_string().contains(expected));
            assert!(session.observed.lock().completions.is_empty());
            assert_eq!(*session.completion.lock(), None);
        }
    });
}

#[test]
fn unexpected_close_eof_and_socket_error_fail_on_both_routes() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            for kind in 0..3 {
                let mut session = TestSession::new(route);
                match kind {
                    0 => session.message(Message::Close(None)),
                    1 => { session.network.take(); },
                    _ => session.network.as_ref().unwrap().send(Err(WebSocketError::ConnectionClosed)).unwrap(),
                }
                let error = session.result().await.unwrap_err().to_string();
                assert!(error.contains(if kind == 2 { "WebSocket read failed" } else { "closed before" }));
                assert!(session.observed.lock().completions.is_empty());
            }
        }
    });
}

#[test]
fn google_close_and_eof_after_end_signal_flush_pending_interim() {
    run_test(async {
        for eof in [false, true] {
            let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
            session.ready_google().await;
            session.frame(json!({ "serverContent": { "interimInputTranscription": "retained words" } }));
            session.pending().await;
            session.finish_input().await;
            if eof { session.network.take(); } else { session.message(Message::Close(None)); }
            session.result().await.unwrap();
            assert_eq!(*session.final_text.lock(), "retained words");
            assert_eq!(session.observed.lock().inserted, "retained words ");
        }
    });
}

#[test]
fn gateway_close_after_audio_done_still_requires_provider_finish() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::VercelGateway);
        session.finish_input().await;
        session.message(Message::Close(None));
        assert!(session.result().await.unwrap_err().to_string().contains("before a finish message"));
    });
}

#[test]
fn audio_and_end_signal_send_failures_have_actionable_errors_on_both_routes() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            for ending in [false, true] {
                let mut session = TestSession::new(route);
                if route == GeminiLiveTransport::GoogleDirect { session.ready_google().await; }
                session.wire.lock().fail_next_send = true;
                if ending { session.audio.take(); } else { session.audio.as_ref().unwrap().try_send(vec![1]).unwrap(); }
                let error = session.result().await.unwrap_err().to_string();
                assert!(error.contains(if ending { "Failed to finish" } else { "Failed to send audio chunk" }));
                assert!(session.observed.lock().inserted.is_empty());
            }
        }
    });
}

#[test]
fn pong_send_failure_is_reported_and_cancel_tolerates_socket_close_failure() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::VercelGateway);
        session.wire.lock().fail_next_send = true;
        session.message(Message::Ping(vec![1]));
        assert!(session.result().await.unwrap_err().to_string().contains("Failed to answer"));
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.wire.lock().fail_close = true;
        session.control.send(ControlMessage::Cancel).unwrap();
        session.result().await.unwrap();
        assert_eq!(session.wire.lock().closes, 1);
    });
}

#[test]
fn google_setup_timeout_blocks_audio_and_does_not_trigger_recording_completion() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.audio.as_ref().unwrap().try_send(vec![1]).unwrap();
        session.pending().await;
        tokio::time::advance(Duration::from_secs(GOOGLE_SETUP_TIMEOUT_SECS)).await;
        assert!(session.result().await.unwrap_err().to_string().contains("did not confirm setup"));
        assert!(session.wire.lock().sent.is_empty());
        assert_eq!(session.observed.lock().stop_count, 0);
        assert_eq!(*session.completion.lock(), None);
    });
}

#[test]
fn google_session_limit_starts_at_setup_ack_and_duplicate_ack_does_not_extend_it() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.pending().await;
        tokio::time::advance(Duration::from_secs(4)).await;
        session.frame(json!({ "setupComplete": {} }));
        session.pending().await;
        tokio::time::advance(Duration::from_secs(9)).await;
        session.frame(json!({ "setupComplete": {} }));
        session.pending().await;
        assert_eq!(session.observed.lock().stop_count, 0);
        tokio::time::advance(Duration::from_secs(1)).await;
        session.pending().await;
        assert_eq!(session.observed.lock().setup_count, 1);
        assert_eq!(session.observed.lock().stop_count, 1);
        session.frame(json!({ "serverContent": { "turnComplete": true } }));
        session.result().await.unwrap();
        assert_eq!(*session.completion.lock(), Some(GeminiTimeLimitCompletion::Complete));
    });
}

#[test]
fn time_limit_sends_one_end_signal_stops_audio_and_reports_confirmed_completion() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let mut session = TestSession::new(route);
            if route == GeminiLiveTransport::GoogleDirect { session.ready_google().await; } else { session.pending().await; }
            tokio::time::advance(Duration::from_secs(10)).await;
            session.pending().await;
            assert_eq!(*session.completion.lock(), Some(GeminiTimeLimitCompletion::Partial));
            session.audio.as_ref().unwrap().try_send(vec![99]).unwrap();
            session.audio.take();
            session.pending().await;
            assert_eq!(session.sent_json(), vec![live_audio_done_payload(route)]);
            assert_eq!(session.observed.lock().stop_count, 1);
            session.frame(if route == GeminiLiveTransport::GoogleDirect {
                json!({ "serverContent": { "inputTranscription": { "text": "last words", "finished": true }, "interactionStatus": "IDLE" } })
            } else { json!({ "type": "finish", "text": "last words" }) });
            session.result().await.unwrap();
            assert_eq!(*session.final_text.lock(), "last words");
            assert_eq!(*session.completion.lock(), Some(GeminiTimeLimitCompletion::Complete));
            let observed = session.observed.lock();
            assert_eq!(observed.completions.len(), 1);
            assert_eq!(observed.completions[0]["partial"], false);
        }
    });
}

#[test]
fn time_limit_close_and_eof_report_partial_completion_on_both_routes() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            for eof in [false, true] {
                let mut session = TestSession::new(route);
                if route == GeminiLiveTransport::GoogleDirect { session.ready_google().await; } else { session.pending().await; }
                tokio::time::advance(Duration::from_secs(10)).await;
                session.pending().await;
                if eof { session.network.take(); } else { session.message(Message::Close(None)); }
                session.result().await.unwrap();
                assert_eq!(*session.completion.lock(), Some(GeminiTimeLimitCompletion::Partial));
                assert_eq!(session.observed.lock().completions[0]["partial"], true);
            }
        }
    });
}

#[test]
fn planned_finalization_provider_and_read_errors_cannot_report_success() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            for read_error in [false, true] {
                let mut session = TestSession::new(route);
                if route == GeminiLiveTransport::GoogleDirect { session.ready_google().await; } else { session.pending().await; }
                tokio::time::advance(Duration::from_secs(10)).await;
                session.pending().await;
                if read_error {
                    session.network.as_ref().unwrap().send(Err(WebSocketError::ConnectionClosed)).unwrap();
                } else {
                    session.frame(if route == GeminiLiveTransport::GoogleDirect {
                        json!({ "error": { "message": "provider failed" } })
                    } else { json!({ "type": "error", "error": "provider failed" }) });
                }
                let error = session.result().await.unwrap_err().to_string();
                assert!(error.contains("after the time-limit end signal"));
                assert_eq!(*session.completion.lock(), Some(GeminiTimeLimitCompletion::Failed(error)));
                assert!(session.observed.lock().completions.is_empty());
                assert_eq!(session.wire.lock().closes, 1);
            }
        }
    });
}

#[test]
fn time_limit_end_signal_failure_does_not_claim_a_successful_stop() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let mut session = TestSession::new(route);
            if route == GeminiLiveTransport::GoogleDirect { session.ready_google().await; } else { session.pending().await; }
            session.wire.lock().fail_next_send = true;
            tokio::time::advance(Duration::from_secs(10)).await;
            assert!(session.result().await.unwrap_err().to_string().contains("Failed to finalize Gemini Live at the safe time limit"));
            assert_eq!(session.observed.lock().stop_count, 0);
            assert_eq!(*session.completion.lock(), None);
        }
    });
}

#[test]
fn google_late_interim_extends_finish_grace_and_timeout_keeps_latest_revision() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.ready_google().await;
        session.finish_input().await;
        tokio::time::advance(Duration::from_millis(2999)).await;
        session.frame(json!({ "serverContent": { "interimInputTranscription": "revised words" } }));
        session.pending().await;
        tokio::time::advance(Duration::from_millis(2999)).await;
        session.pending().await;
        tokio::time::advance(Duration::from_millis(1)).await;
        session.result().await.unwrap();
        assert_eq!(*session.final_text.lock(), "revised words");
        assert_eq!(session.observed.lock().inserted, "revised words ");
    });
}

#[test]
fn google_grace_timeout_at_time_limit_is_partial_and_preserves_final_fragments() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.ready_google().await;
        tokio::time::advance(Duration::from_secs(10)).await;
        session.pending().await;
        session.frame(json!({ "inputTranscription": { "text": "unfinished final" } }));
        session.pending().await;
        tokio::time::advance(Duration::from_millis(GOOGLE_FINAL_TRANSCRIPT_GRACE_MS)).await;
        session.result().await.unwrap();
        assert_eq!(*session.final_text.lock(), "unfinished final");
        assert_eq!(session.observed.lock().chunks, vec!["unfinished final", " "]);
        assert_eq!(*session.completion.lock(), Some(GeminiTimeLimitCompletion::Partial));
    });
}

#[test]
fn cutoff_blocks_revised_final_chunks_and_completion_spaces_without_blocking_new_session() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let mut old = TestSession::new(route);
            let mut new = TestSession::new(route);
            if route == GeminiLiveTransport::GoogleDirect { old.ready_google().await; new.ready_google().await; }
            old.window.finish_at(Instant::now() + Duration::from_secs(60));
            old.frame(if route == GeminiLiveTransport::GoogleDirect {
                json!({ "serverContent": { "inputTranscription": { "text": "Old words", "finished": true } } })
            } else { json!({ "type": "transcript-delta", "delta": "Old words" }) });
            old.pending().await;
            old.window.finish_at(Instant::now());
            old.finish_input().await;
            old.frame(if route == GeminiLiveTransport::GoogleDirect {
                json!({ "serverContent": { "inputTranscription": { "text": " revised late", "finished": true }, "turnComplete": true } })
            } else { json!({ "type": "transcript-delta", "delta": " revised late" }) });
            if route == GeminiLiveTransport::VercelGateway { old.frame(json!({ "type": "finish", "text": "Old transcript revised" })); }
            old.result().await.unwrap();
            new.frame(if route == GeminiLiveTransport::GoogleDirect {
                json!({ "serverContent": { "inputTranscription": { "text": "New words", "finished": true } } })
            } else { json!({ "type": "transcript-final", "text": "New words" }) });
            new.pending().await;
            assert_eq!(old.observed.lock().inserted, "Old words");
            assert_eq!(new.observed.lock().inserted, "New words ");
            assert_eq!(new.observed.lock().stop_count, 0);
            assert_eq!(*new.completion.lock(), None);
            new.control.send(ControlMessage::Cancel).unwrap();
            new.result().await.unwrap();
        }
    });
}

#[test]
fn old_socket_failure_cannot_change_new_session_audio_transcript_or_completion_state() {
    run_test(async {
        let mut old = TestSession::new(GeminiLiveTransport::VercelGateway);
        let mut new = TestSession::new(GeminiLiveTransport::VercelGateway);
        old.finish_input().await;
        new.audio.as_ref().unwrap().try_send(vec![7, 8]).unwrap();
        new.frame(json!({ "type": "transcript-delta", "delta": "new session" }));
        new.pending().await;
        old.network.as_ref().unwrap().send(Err(WebSocketError::ConnectionClosed)).unwrap();
        old.result().await.unwrap_err();
        assert_eq!(new.wire.lock().sent, vec![Message::Binary(vec![7, 8])]);
        assert_eq!(new.observed.lock().inserted, "new session");
        assert_eq!(*new.final_text.lock(), "new session");
        assert_eq!(new.observed.lock().stop_count, 0);
        assert_eq!(*new.completion.lock(), None);
        new.frame(json!({ "type": "finish", "text": "new session" }));
        new.result().await.unwrap();
    });
}

fn active_fixture<F>(
    operation_id: Option<u64>,
    initial_text: &str,
    run: impl FnOnce(mpsc::Receiver<Vec<u8>>, mpsc::UnboundedReceiver<ControlMessage>, Arc<Mutex<String>>) -> F,
) -> ActiveSession
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    let (audio_tx, audio_rx) = mpsc::channel(AUDIO_QUEUE_CAPACITY);
    let (control_tx, control_rx) = mpsc::unbounded_channel();
    let final_text = Arc::new(Mutex::new(initial_text.to_string()));
    let task = run(audio_rx, control_rx, Arc::clone(&final_text));
    ActiveSession {
        binding_id: "same_hotkey".to_string(), operation_id, audio_tx, control_tx,
        final_text, join_handle: JoinHandle::Tokio(tokio::spawn(task)),
        output_window: Arc::new(GeminiOutputWindow::default()),
    }
}

#[test]
fn early_finish_sends_finish_drains_audio_and_waits_until_configured_insertion_time() {
    run_test(async {
        let started = tokio::time::Instant::now();
        let session = active_fixture(Some(1), "", |mut audio, mut control, text| async move {
            assert!(matches!(control.recv().await, Some(ControlMessage::Finish)));
            assert_eq!(audio.recv().await, Some(vec![1, 2]));
            assert_eq!(audio.recv().await, None);
            *text.lock() = "  provider finished early  ".to_string();
            Ok(())
        });
        session.audio_tx.try_send(vec![1, 2]).unwrap();
        let text = finish_early_session(session, (started + Duration::from_millis(500)).into_std()).await.unwrap();
        assert_eq!(text, "provider finished early");
        let elapsed = tokio::time::Instant::now() - started;
        assert!(elapsed >= Duration::from_millis(500));
        assert!(elapsed <= Duration::from_millis(501), "only timer tick rounding is allowed");
    });
}

#[test]
fn early_finish_timeout_aborts_and_joins_old_callbacks_before_returning_partial_text() {
    run_test(async {
        struct Cleanup(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Cleanup {
            fn drop(&mut self) { self.0.store(true, std::sync::atomic::Ordering::SeqCst); }
        }
        let cleaned = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cleanup = Arc::clone(&cleaned);
        let session = active_fixture(Some(1), "accepted before cutoff", move |_, _, text| async move {
            let _cleanup = Cleanup(cleanup);
            tokio::time::sleep(Duration::from_secs(10)).await;
            *text.lock() = "late replacement must never arrive".to_string();
            Ok(())
        });
        let captured = Arc::clone(&session.final_text);
        let deadline = (tokio::time::Instant::now() + Duration::from_millis(500)).into_std();
        assert_eq!(finish_early_session(session, deadline).await.unwrap(), "accepted before cutoff");
        assert!(cleaned.load(std::sync::atomic::Ordering::SeqCst));
        tokio::time::advance(Duration::from_secs(20)).await;
        assert_eq!(*captured.lock(), "accepted before cutoff");
    });
}

#[test]
fn early_finish_keeps_revised_snapshot_when_provider_fails_after_partial_output() {
    run_test(async {
        let session = active_fixture(Some(1), "original words", |_, _, text| async move {
            *text.lock() = "corrected during finalization".to_string();
            Err(anyhow!("provider disconnected"))
        });
        let deadline = (tokio::time::Instant::now() + Duration::from_millis(500)).into_std();
        assert_eq!(finish_early_session(session, deadline).await.unwrap(), "corrected during finalization");
    });
}

#[test]
fn early_finish_reports_provider_error_when_no_transcript_exists() {
    run_test(async {
        let session = active_fixture(Some(1), "   ", |_, _, _| async { Err(anyhow!("provider disconnected")) });
        let deadline = (tokio::time::Instant::now() + Duration::from_millis(500)).into_std();
        assert_eq!(finish_early_session(session, deadline).await.unwrap_err().to_string(), "provider disconnected");
    });
}

#[test]
fn early_finish_reports_join_failure_only_when_no_partial_text_exists() {
    run_test(async {
        for partial in ["", "preserved partial"] {
            let session = active_fixture(Some(1), partial, |_, _, _| async {
                Err(anyhow!("unreachable"))
            });
            // A cancelled Tokio task exercises the same JoinError path as a panic.
            session.join_handle.abort();
            let deadline = (tokio::time::Instant::now() + Duration::from_millis(500)).into_std();
            let result = finish_early_session(session, deadline).await;
            if partial.is_empty() {
                assert!(result.unwrap_err().to_string().contains("early finalization task failed"));
            } else { assert_eq!(result.unwrap(), partial); }
        }
    });
}

#[test]
fn early_finish_without_output_returns_empty_on_success_or_cutoff() {
    run_test(async {
        for timeout in [false, true] {
            let session = active_fixture(Some(1), "", move |_, _, _| async move {
                if timeout { std::future::pending::<()>().await; }
                Ok(())
            });
            let deadline = (tokio::time::Instant::now() + Duration::from_millis(500)).into_std();
            assert_eq!(finish_early_session(session, deadline).await.unwrap(), "");
        }
    });
}

#[test]
fn stale_old_finish_does_not_detach_new_session_or_clear_its_preconnect_audio() {
    run_test(async {
        let mut active = Some(active_fixture(Some(2), "new words", |_, _, _| async { Ok(()) }));
        let window = Arc::clone(&active.as_ref().unwrap().output_window);
        let mut pending = Some(PendingAudio {
            operation_id: Some(2), frames: vec![vec![7, 8]],
            output_window: Arc::clone(&window),
        });
        assert!(detach_session_if_matches(&mut active, &mut pending, Some(1)).is_none());
        assert_eq!(active.as_ref().unwrap().operation_id, Some(2));
        assert_eq!(*active.as_ref().unwrap().final_text.lock(), "new words");
        assert_eq!(pending.as_ref().unwrap().frames, vec![vec![7, 8]]);
        assert!(Arc::ptr_eq(&pending.as_ref().unwrap().output_window, &window));
        assert!(detach_session_if_matches(&mut active, &mut pending, None).is_none());
        active.take().unwrap().join_handle.abort();
    });
}

#[test]
fn detached_old_finalization_and_new_recording_keep_distinct_transcripts_and_windows() {
    run_test(async {
        let mut active = Some(active_fixture(Some(1), "old initial", |_, _, text| async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            *text.lock() = "old revised final".to_string();
            Ok(())
        }));
        let mut pending = None;
        let old = detach_session_if_matches(&mut active, &mut pending, Some(1)).unwrap();
        assert!(active.is_none());
        active = Some(active_fixture(Some(2), "new recording", |_, _, _| async { Ok(()) }));
        assert!(!Arc::ptr_eq(&old.output_window, &active.as_ref().unwrap().output_window));
        let deadline = (tokio::time::Instant::now() + Duration::from_millis(500)).into_std();
        assert_eq!(finish_early_session(old, deadline).await.unwrap(), "old revised final");
        assert_eq!(active.as_ref().unwrap().operation_id, Some(2));
        assert_eq!(*active.as_ref().unwrap().final_text.lock(), "new recording");
        assert!(active.as_ref().unwrap().output_window.allows_delivery());
        active.take().unwrap().join_handle.abort();
    });
}

#[test]
fn matching_detach_clears_only_its_own_pending_audio_including_live_monitor() {
    run_test(async {
        for operation_id in [None, Some(1)] {
            let mut active = Some(active_fixture(operation_id, "", |_, _, _| async { Ok(()) }));
            let mut pending = Some(PendingAudio {
                operation_id, frames: vec![vec![1]], output_window: Arc::new(GeminiOutputWindow::default()),
            });
            let detached = detach_session_if_matches(&mut active, &mut pending, operation_id).unwrap();
            assert!(active.is_none());
            assert!(pending.is_none());
            detached.join_handle.abort();
        }
        let mut active = None;
        let mut pending = Some(PendingAudio {
            operation_id: Some(1), frames: vec![vec![1]], output_window: Arc::new(GeminiOutputWindow::default()),
        });
        assert!(detach_session_if_matches(&mut active, &mut pending, Some(1)).is_none());
        assert!(pending.is_none());
    });
}

#[test]
fn finishing_old_active_session_preserves_newer_pending_connection_buffer() {
    run_test(async {
        let mut active = Some(active_fixture(Some(1), "", |_, _, _| async { Ok(()) }));
        let mut pending = Some(PendingAudio {
            operation_id: Some(2), frames: vec![vec![9]], output_window: Arc::new(GeminiOutputWindow::default()),
        });
        let old = detach_session_if_matches(&mut active, &mut pending, Some(1)).unwrap();
        assert_eq!(pending.as_ref().unwrap().operation_id, Some(2));
        assert_eq!(pending.as_ref().unwrap().frames, vec![vec![9]]);
        old.join_handle.abort();
    });
}

#[test]
fn stale_or_cancelled_session_cannot_insert_late_network_text_into_new_recording() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            for cancelled in [false, true] {
                let mut old = TestSession::new(route);
                let mut new = TestSession::new(route);
                if route == GeminiLiveTransport::GoogleDirect { old.ready_google().await; new.ready_google().await; }
                *old.delivery.lock() = (false, cancelled, cancelled);
                old.frame(if route == GeminiLiveTransport::GoogleDirect {
                    json!({ "serverContent": { "inputTranscription": { "text": "old late text", "finished": true } } })
                } else { json!({ "type": "transcript-final", "text": "old late text" }) });
                old.pending().await;
                new.frame(if route == GeminiLiveTransport::GoogleDirect {
                    json!({ "serverContent": { "inputTranscription": { "text": "new target", "finished": true } } })
                } else { json!({ "type": "transcript-final", "text": "new target" }) });
                new.pending().await;
                assert!(old.observed.lock().inserted.is_empty());
                assert_eq!(new.observed.lock().inserted, "new target ");
                for session in [&mut old, &mut new] {
                    session.control.send(ControlMessage::Cancel).unwrap();
                    session.result().await.unwrap();
                }
            }
        }
    });
}

#[test]
fn authorized_background_finalization_keeps_its_delivery_until_its_own_cutoff() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::VercelGateway);
        *session.delivery.lock() = (true, false, false);
        session.frame(json!({ "type": "transcript-delta", "delta": "background tail" }));
        session.pending().await;
        assert_eq!(session.observed.lock().inserted, "background tail");
        session.window.finish_at(Instant::now());
        session.frame(json!({ "type": "transcript-delta", "delta": " too late" }));
        session.frame(json!({ "type": "finish", "text": "background tail corrected" }));
        session.result().await.unwrap();
        assert_eq!(session.observed.lock().inserted, "background tail");
        assert_eq!(*session.final_text.lock(), "background tail corrected");
    });
}

#[test]
fn connection_uses_production_handshake_and_sends_route_specific_setup_to_mock_socket() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let model = if route == GeminiLiveTransport::GoogleDirect {
                GEMINI_LIVE_GOOGLE_DEFAULT_MODEL
            } else { GEMINI_LIVE_DEFAULT_MODEL };
            let options = GeminiRealtimeOptions { model: model.to_string(), ..Default::default() };
            let wire = Arc::new(Mutex::new(WireState::default()));
            let connected_wire = Arc::clone(&wire);
            let mut socket = connect_live_socket(route, model, "fake-test-key", move |request| async move {
                let url = reqwest::Url::parse(&request.uri().to_string()).unwrap();
                assert_eq!(url.scheme(), "wss");
                if route == GeminiLiveTransport::GoogleDirect {
                    assert_eq!(url.host_str(), Some("generativelanguage.googleapis.com"));
                    assert_eq!(url.query_pairs().find(|(key, _)| key == "key").unwrap().1, "fake-test-key");
                    assert!(request.headers().get("authorization").is_none());
                } else {
                    assert_eq!(url.host_str(), Some("ai-gateway.vercel.sh"));
                    assert_eq!(request.headers()["authorization"], "Bearer fake-test-key");
                    assert_eq!(request.headers()["ai-model-id"], GEMINI_LIVE_DEFAULT_MODEL);
                }
                Ok(MockWrite(connected_wire))
            }).await.unwrap();
            send_live_setup(&mut socket, route, &options).await.unwrap();
            let wire = wire.lock();
            assert_eq!(wire.sent.len(), 1);
            let Message::Text(text) = &wire.sent[0] else { panic!("expected text setup") };
            let payload: Value = serde_json::from_str(text).unwrap();
            if route == GeminiLiveTransport::GoogleDirect {
                assert_eq!(payload["setup"]["model"], "models/gemini-3.5-transcribe-live");
                assert_eq!(payload["setup"]["inputAudioTranscription"]["mode"], "SMART");
            } else {
                assert_eq!(payload["type"], "transcription-stream.start");
                assert_eq!(payload["providerOptions"]["google"]["mode"], "SMART");
            }
        }
    });
}

#[test]
fn connection_timeout_is_bounded_to_ten_seconds_on_both_routes() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let started = tokio::time::Instant::now();
            let result: Result<()> = connect_live_socket(route, GEMINI_LIVE_DEFAULT_MODEL, "fake", |_| {
                std::future::pending::<Result<(), WebSocketError>>()
            }).await;
            assert_eq!(result.unwrap_err().to_string(), "Timed out while connecting to Gemini 3.5 Transcribe Live");
            let elapsed = tokio::time::Instant::now() - started;
            let limit = Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS);
            assert!(elapsed >= limit && elapsed <= limit + Duration::from_millis(1));
        }
    });
}

#[test]
fn connection_failure_and_setup_send_failure_are_distinct() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let result: Result<()> = connect_live_socket(route, GEMINI_LIVE_DEFAULT_MODEL, "fake", |_| async {
                Err(WebSocketError::ConnectionClosed)
            }).await;
            assert!(result.unwrap_err().to_string().starts_with("Failed to connect to Gemini 3.5 Transcribe Live:"));
            let wire = Arc::new(Mutex::new(WireState { fail_next_send: true, ..Default::default() }));
            let mut socket = MockWrite(Arc::clone(&wire));
            let error = send_live_setup(&mut socket, route, &GeminiRealtimeOptions::default()).await.unwrap_err();
            assert!(error.to_string().starts_with("Failed to send Gemini 3.5 Transcribe Live setup message:"));
            assert!(wire.lock().sent.is_empty());
        }
    });
}

#[test]
fn invalid_gateway_credentials_fail_before_invoking_network_connector() {
    run_test(async {
        let called = std::cell::Cell::new(false);
        let result: Result<()> = connect_live_socket(
            GeminiLiveTransport::VercelGateway, GEMINI_LIVE_DEFAULT_MODEL, "bad\nkey", |_| {
                called.set(true);
                async { Ok(()) }
            },
        ).await;
        assert!(result.unwrap_err().to_string().contains("Invalid Vercel auth header"));
        assert!(!called.get());
    });
}

#[test]
fn google_query_key_cannot_inject_extra_url_parameters() {
    let key = "fake&model=evil?#% unicode-é";
    let request = build_live_websocket_request(
        GeminiLiveTransport::GoogleDirect, GEMINI_LIVE_GOOGLE_DEFAULT_MODEL, key,
    ).unwrap();
    let url = reqwest::Url::parse(&request.uri().to_string()).unwrap();
    assert_eq!(url.query_pairs().collect::<Vec<_>>(), vec![
        (std::borrow::Cow::Borrowed("key"), std::borrow::Cow::Borrowed(key)),
    ]);
    assert!(url.fragment().is_none());
    assert!(request.headers().get("authorization").is_none());
}

#[test]
fn preview_only_sessions_keep_final_text_without_invoking_insertion_callbacks() {
    run_test(async {
        for route in [GeminiLiveTransport::GoogleDirect, GeminiLiveTransport::VercelGateway] {
            let mut session = TestSession::with_insertion(route, false);
            if route == GeminiLiveTransport::GoogleDirect { session.ready_google().await; }
            session.finish_input().await;
            if route == GeminiLiveTransport::GoogleDirect {
                session.frame(json!({ "serverContent": {
                    "inputTranscription": { "text": "preview only", "finished": true }, "turnComplete": true
                } }));
            } else {
                session.frame(json!({ "type": "transcript-delta", "delta": "preview only" }));
                session.frame(json!({ "type": "transcript-final", "text": "preview only" }));
                session.frame(json!({ "type": "finish", "text": "preview only" }));
            }
            session.result().await.unwrap();
            assert_eq!(*session.final_text.lock(), "preview only");
            assert!(session.observed.lock().inserted.is_empty());
            assert!(session.observed.lock().chunks.is_empty());
            assert!(session.observed.lock().live_text.iter().any(|text| text == "preview only"));
        }
    });
}

#[test]
fn google_active_status_during_finalization_waits_for_processing_confirmation() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.ready_google().await;
        session.finish_input().await;
        session.frame(json!({ "serverContent": {
            "inputTranscription": { "text": "done words", "finished": true },
            "turnComplete": true, "interactionStatus": "ACTIVE"
        } }));
        session.pending().await;
        assert_eq!(*session.final_text.lock(), "done words");
        session.frame(json!({ "serverContent": { "interactionStatus": "IDLE" } }));
        session.result().await.unwrap();
        assert_eq!(session.observed.lock().chunks, vec!["done words", " "]);
    });
}

#[test]
fn google_top_level_final_replaces_interim_and_empty_frames_do_not_insert() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.ready_google().await;
        for payload in [
            json!({ "serverContent": { "interimInputTranscription": { "text": "old hypothesis" } } }),
            json!({ "serverContent": { "interimInputTranscription": {} } }),
            json!({ "serverContent": { "inputTranscription": {} } }),
            json!({ "inputTranscription": {} }),
            json!({ "inputTranscription": { "text": "corrected final", "finished": true } }),
        ] { session.frame(payload); }
        session.pending().await;
        session.finish_input().await;
        session.frame(json!({ "serverContent": { "turnComplete": true } }));
        session.result().await.unwrap();
        assert_eq!(*session.final_text.lock(), "corrected final");
        assert_eq!(session.observed.lock().chunks, vec!["corrected final", " "]);
    });
}

#[test]
fn gateway_missing_text_fields_and_whitespace_endings_do_not_manufacture_duplicate_output() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::VercelGateway);
        for payload in [
            json!({ "type": "transcript-delta" }),
            json!({ "type": "transcript-partial" }),
            json!({ "type": "transcript-final" }),
            json!({ "type": "transcript-delta", "delta": "Words\n" }),
            json!({ "type": "transcript-final", "text": "must not repeat" }),
            json!({ "type": "finish" }),
        ] { session.frame(payload); }
        session.result().await.unwrap();
        assert_eq!(session.observed.lock().chunks, vec!["Words\n"]);
        assert_eq!(session.observed.lock().inserted, "Words\n");
        assert_eq!(*session.final_text.lock(), "Words\n");
    });
}

#[test]
fn google_final_fragment_resets_finish_grace_in_nested_and_top_level_payloads() {
    run_test(async {
        for top_level in [false, true] {
            let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
            session.ready_google().await;
            session.finish_input().await;
            tokio::time::advance(Duration::from_millis(2999)).await;
            let transcription = json!({ "text": "final fragment" });
            session.frame(if top_level { json!({ "inputTranscription": transcription }) }
                else { json!({ "serverContent": { "inputTranscription": transcription } }) });
            session.pending().await;
            tokio::time::advance(Duration::from_millis(2999)).await;
            session.pending().await;
            tokio::time::advance(Duration::from_millis(1)).await;
            session.result().await.unwrap();
            assert_eq!(*session.final_text.lock(), "final fragment");
            assert_eq!(session.observed.lock().chunks, vec!["final fragment", " "]);
        }
    });
}

#[test]
fn cancelling_after_time_limit_suppresses_completion_notice_and_final_interim_insertion() {
    run_test(async {
        let mut session = TestSession::new(GeminiLiveTransport::GoogleDirect);
        session.ready_google().await;
        session.frame(json!({ "serverContent": { "interimInputTranscription": "cancelled hypothesis" } }));
        session.pending().await;
        tokio::time::advance(Duration::from_secs(10)).await;
        session.pending().await;
        session.control.send(ControlMessage::Cancel).unwrap();
        session.result().await.unwrap();
        assert!(session.observed.lock().inserted.is_empty());
        assert!(session.observed.lock().completions.is_empty());
    });
}

#[test]
fn pcm_encoding_clips_audio_and_preserves_signed_little_endian_samples() {
    assert_eq!(frame_16khz_mono_to_pcm_s16le_bytes(&[-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0]),
        vec![0, 128, 0, 128, 0, 192, 0, 0, 0, 64, 255, 127, 255, 127]);
    assert!(frame_16khz_mono_to_pcm_s16le_bytes(&[]).is_empty());
}

#[test]
fn transcript_join_preserves_existing_boundaries_and_normalizes_new_sentence_deltas() {
    for (left, right, joined) in [
        ("", "words", "words"), ("words", "", "words"),
        ("first", "second", "first second"), ("first\t", "second", "first\tsecond"),
        ("first", "\nsecond", "first\nsecond"),
    ] { assert_eq!(join_transcript_text(left, right), joined); }
    for (left, right, delta) in [
        ("", "start", "start"), ("end", "", ""), ("end ", "next", "next"),
        ("end.", " next", " next"), ("Hello.", "Next", " Next"),
        ("こんにちは。", "次", " 次"), ("end!", "“Next", " “Next"),
        ("frag", "ment", "ment"), ("end.", "42", "42"),
    ] { assert_eq!(normalize_gateway_delta_boundary(left, right), delta); }
}
