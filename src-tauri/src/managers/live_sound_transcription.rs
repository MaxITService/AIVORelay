use crate::file_transcription_diarization::{
    normalize_raw_speaker_blocks_with_state, RawSpeakerBlock, SpeakerBlockNormalizationState,
    UNATTRIBUTED_SPEAKER_KEY, UNATTRIBUTED_SPEAKER_NAME,
};
use crate::managers::elevenlabs_realtime::{ElevenLabsLivePacket, ElevenLabsLiveWord};
use serde::Serialize;
use specta::Type;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};

const EVENT_KEBAB: &str = "live-sound-transcription-state";
const EVENT_SNAKE: &str = "live_sound_transcription_state";

#[derive(Serialize, Clone, Debug, Default, Type)]
pub struct LiveSoundTranscriptSegmentPayload {
    pub speaker_id: Option<u32>,
    pub speaker_label: Option<String>,
    pub text: String,
    pub is_interim: bool,
    pub timestamp_ms: u64,
}

#[derive(Serialize, Clone, Debug, Default, Type)]
pub struct LiveSoundTranscriptionStatePayload {
    pub active: bool,
    pub recording: bool,
    pub processing_llm: bool,
    pub binding_id: Option<String>,
    pub error_message: Option<String>,
    pub final_text: String,
    pub interim_text: String,
    pub segments: Vec<LiveSoundTranscriptSegmentPayload>,
    pub auto_stop_remaining_seconds: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ElevenLabsFinalSegmentRecord {
    segment_index: Option<u64>,
    committed_text: String,
    diarized_blocks: Option<Vec<RawSpeakerBlock>>,
}

fn append_elevenlabs_word_token(target: &mut String, token: &str) {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return;
    }
    let no_leading_space = trimmed.chars().next().is_some_and(|ch| {
        matches!(
            ch,
            ',' | '.' | '!' | '?' | ';' | ':' | ')' | ']' | '}' | '%' | '\''
        )
    });
    if !target.is_empty() && !no_leading_space {
        target.push(' ');
    }
    target.push_str(trimmed);
}

fn raw_speaker_blocks_from_elevenlabs_words(words: &[ElevenLabsLiveWord]) -> Vec<RawSpeakerBlock> {
    let has_any_speaker = words.iter().any(|word| {
        word.speaker_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|id| !id.is_empty())
    });
    if !has_any_speaker {
        return Vec::new();
    }

    let mut blocks: Vec<RawSpeakerBlock> = Vec::new();
    for word in words {
        let token = word.text.trim();
        if token.is_empty() {
            continue;
        }
        let attributed = word
            .speaker_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let speaker_key = attributed.unwrap_or(UNATTRIBUTED_SPEAKER_KEY);
        let default_name = attributed
            .is_none()
            .then(|| UNATTRIBUTED_SPEAKER_NAME.to_string());

        if let Some(last) = blocks.last_mut() {
            if last.speaker_key == speaker_key {
                append_elevenlabs_word_token(&mut last.text, token);
                continue;
            }
        }
        blocks.push(RawSpeakerBlock {
            speaker_key: speaker_key.to_string(),
            default_name,
            text: token.to_string(),
        });
    }
    blocks
}

fn rewrite_merged_speaker_in_blocks(
    blocks: &mut Vec<RawSpeakerBlock>,
    from_speaker_id: &str,
    to_speaker_id: &str,
) {
    if blocks.is_empty() {
        return;
    }
    let mut coalesced: Vec<RawSpeakerBlock> = Vec::with_capacity(blocks.len());
    for mut block in blocks.drain(..) {
        if block.speaker_key.trim() == from_speaker_id {
            block.speaker_key = to_speaker_id.to_string();
            block.default_name = None;
        }
        let text = block.text.trim();
        if text.is_empty() {
            continue;
        }
        if let Some(last) = coalesced.last_mut() {
            if last.speaker_key == block.speaker_key {
                if !last.text.is_empty() {
                    last.text.push(' ');
                }
                last.text.push_str(text);
                continue;
            }
        }
        block.text = text.to_string();
        coalesced.push(block);
    }
    *blocks = coalesced;
}

#[derive(Clone, Debug, Default)]
struct LiveSoundTranscriptionRuntime {
    active: bool,
    recording: bool,
    processing_llm: bool,
    binding_id: Option<String>,
    error_message: Option<String>,
    final_text: String,
    processed_final_text: Option<String>,
    interim_text: String,
    final_raw_blocks: Vec<RawSpeakerBlock>,
    interim_raw_blocks: Vec<RawSpeakerBlock>,
    elevenlabs_segments: Vec<ElevenLabsFinalSegmentRecord>,
    speaker_normalization_state: SpeakerBlockNormalizationState,
    final_segment_timestamps_ms: Vec<u64>,
    interim_segment_timestamps_ms: Vec<u64>,
    transcript_started_at: Option<Instant>,
    session_id: u64,
    auto_stop_deadline: Option<Instant>,
}

impl LiveSoundTranscriptionRuntime {
    fn push_final_text(&mut self, text: &str, add_separator: bool) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        if self.final_text.trim().is_empty() {
            self.final_text = trimmed.to_string();
            return;
        }

        let ends_with_whitespace = self
            .final_text
            .chars()
            .last()
            .map(char::is_whitespace)
            .unwrap_or(false);
        let starts_with_whitespace = trimmed
            .chars()
            .next()
            .map(char::is_whitespace)
            .unwrap_or(false);
        if add_separator && !ends_with_whitespace && !starts_with_whitespace {
            self.final_text.push(' ');
        }
        self.final_text.push_str(trimmed);
    }

    fn apply_elevenlabs_packet(&mut self, packet: ElevenLabsLivePacket) {
        match packet {
            ElevenLabsLivePacket::Partial { text, words } => {
                self.interim_text = text.trim().to_string();
                self.interim_raw_blocks = raw_speaker_blocks_from_elevenlabs_words(&words);
                self.interim_segment_timestamps_ms.clear();
            }
            ElevenLabsLivePacket::Committed {
                text,
                segment_index,
            } => {
                self.interim_text.clear();
                self.interim_raw_blocks.clear();
                self.interim_segment_timestamps_ms.clear();

                let trimmed = text.trim();
                if let Some(idx) = segment_index {
                    if let Some(existing) = self
                        .elevenlabs_segments
                        .iter_mut()
                        .find(|seg| seg.segment_index == Some(idx))
                    {
                        if existing.committed_text.is_empty() && !trimmed.is_empty() {
                            existing.committed_text = trimmed.to_string();
                        }
                        return;
                    }
                }

                if !trimmed.is_empty() {
                    self.push_final_text(trimmed, true);
                }
                if !trimmed.is_empty() || segment_index.is_some() {
                    self.elevenlabs_segments.push(ElevenLabsFinalSegmentRecord {
                        segment_index,
                        committed_text: trimmed.to_string(),
                        diarized_blocks: None,
                    });
                }
            }
            ElevenLabsLivePacket::Diarized {
                segment_index,
                text,
                words,
            } => {
                let trimmed = text.trim();
                let mut blocks = raw_speaker_blocks_from_elevenlabs_words(&words);
                if blocks.is_empty() && !trimmed.is_empty() {
                    blocks.push(RawSpeakerBlock {
                        speaker_key: UNATTRIBUTED_SPEAKER_KEY.to_string(),
                        default_name: Some(UNATTRIBUTED_SPEAKER_NAME.to_string()),
                        text: trimmed.to_string(),
                    });
                }

                let existing_idx = segment_index
                    .and_then(|idx| {
                        self.elevenlabs_segments
                            .iter()
                            .position(|seg| seg.segment_index == Some(idx))
                    })
                    .or_else(|| {
                        if segment_index.is_none() {
                            self.elevenlabs_segments
                                .iter()
                                .position(|seg| seg.diarized_blocks.is_none())
                        } else {
                            None
                        }
                    });

                if let Some(pos) = existing_idx {
                    if self.elevenlabs_segments[pos].committed_text.is_empty()
                        && !trimmed.is_empty()
                    {
                        self.elevenlabs_segments[pos].committed_text = trimmed.to_string();
                    }
                    self.elevenlabs_segments[pos].diarized_blocks = Some(blocks);
                } else {
                    if !trimmed.is_empty() {
                        self.push_final_text(trimmed, true);
                    }
                    self.elevenlabs_segments.push(ElevenLabsFinalSegmentRecord {
                        segment_index,
                        committed_text: trimmed.to_string(),
                        diarized_blocks: Some(blocks),
                    });
                }
            }
            ElevenLabsLivePacket::SpeakersMerged {
                from_speaker_id,
                to_speaker_id,
            } => {
                let from_id = from_speaker_id.trim();
                let to_id = to_speaker_id.trim();
                if from_id.is_empty() || to_id.is_empty() || from_id == to_id {
                    return;
                }
                for seg in &mut self.elevenlabs_segments {
                    if let Some(blocks) = seg.diarized_blocks.as_mut() {
                        rewrite_merged_speaker_in_blocks(blocks, from_id, to_id);
                    }
                }
                rewrite_merged_speaker_in_blocks(&mut self.final_raw_blocks, from_id, to_id);
                rewrite_merged_speaker_in_blocks(&mut self.interim_raw_blocks, from_id, to_id);
                self.speaker_normalization_state
                    .merge_raw_speaker_id(from_id, to_id);
            }
        }
    }

    fn flush_attributed_blocks(
        pending_raw: &mut Vec<RawSpeakerBlock>,
        normalization: &mut SpeakerBlockNormalizationState,
        out: &mut Vec<LiveSoundTranscriptSegmentPayload>,
        is_interim: bool,
    ) {
        if pending_raw.is_empty() {
            return;
        }
        let blocks = std::mem::take(pending_raw);
        for block in normalize_raw_speaker_blocks_with_state(blocks, normalization) {
            out.push(LiveSoundTranscriptSegmentPayload {
                speaker_id: Some(block.speaker_id),
                speaker_label: Some(block.default_name),
                text: block.text,
                is_interim,
                timestamp_ms: 0,
            });
        }
    }

    fn normalize_blocks_without_burning_unattributed(
        blocks: &[RawSpeakerBlock],
        normalization: &mut SpeakerBlockNormalizationState,
        out: &mut Vec<LiveSoundTranscriptSegmentPayload>,
        is_interim: bool,
    ) {
        let mut pending_attributed: Vec<RawSpeakerBlock> = Vec::new();
        for block in blocks {
            if block.speaker_key.trim() == UNATTRIBUTED_SPEAKER_KEY {
                Self::flush_attributed_blocks(&mut pending_attributed, normalization, out, is_interim);
                let text = block.text.trim();
                if !text.is_empty() {
                    out.push(LiveSoundTranscriptSegmentPayload {
                        speaker_id: None,
                        speaker_label: None,
                        text: text.to_string(),
                        is_interim,
                        timestamp_ms: 0,
                    });
                }
            } else {
                pending_attributed.push(block.clone());
            }
        }
        Self::flush_attributed_blocks(&mut pending_attributed, normalization, out, is_interim);
    }

    fn to_payload(&mut self) -> LiveSoundTranscriptionStatePayload {
        let auto_stop_remaining_seconds = self
            .auto_stop_deadline
            .filter(|_| self.recording)
            .map(|deadline| remaining_seconds(deadline.saturating_duration_since(Instant::now())));

        let has_elevenlabs_diarized = self.processed_final_text.is_none()
            && self
                .elevenlabs_segments
                .iter()
                .any(|seg| seg.diarized_blocks.is_some());

        let mut final_segments: Vec<LiveSoundTranscriptSegmentPayload> = Vec::new();
        if has_elevenlabs_diarized {
            let blocks: Vec<RawSpeakerBlock> = self.elevenlabs_segments.iter().flat_map(|seg| {
                seg.diarized_blocks.clone().unwrap_or_else(|| vec![RawSpeakerBlock {
                    speaker_key: UNATTRIBUTED_SPEAKER_KEY.to_string(),
                    default_name: Some(UNATTRIBUTED_SPEAKER_NAME.to_string()),
                    text: seg.committed_text.clone(),
                }])
            }).collect();
            Self::normalize_blocks_without_burning_unattributed(
                &blocks,
                &mut self.speaker_normalization_state,
                &mut final_segments,
                false,
            );
        } else {
            final_segments = normalize_raw_speaker_blocks_with_state(
                self.final_raw_blocks.clone(),
                &mut self.speaker_normalization_state,
            )
            .into_iter()
            .map(|block| LiveSoundTranscriptSegmentPayload {
                speaker_id: Some(block.speaker_id),
                speaker_label: Some(block.default_name),
                text: block.text,
                is_interim: false,
                timestamp_ms: 0,
            })
            .collect();
        }

        if final_segments.is_empty() && !self.final_text.trim().is_empty() {
            final_segments.push(LiveSoundTranscriptSegmentPayload {
                speaker_id: None,
                speaker_label: None,
                text: self.final_text.clone(),
                is_interim: false,
                timestamp_ms: 0,
            });
        } else if !has_elevenlabs_diarized {
            if let Some(processed_text) = self
                .processed_final_text
                .as_ref()
                .filter(|text| !text.trim().is_empty())
            {
                final_segments.insert(
                    0,
                    LiveSoundTranscriptSegmentPayload {
                        speaker_id: None,
                        speaker_label: None,
                        text: processed_text.clone(),
                        is_interim: false,
                        timestamp_ms: 0,
                    },
                );
            }
        }

        let mut interim_segments: Vec<LiveSoundTranscriptSegmentPayload> = Vec::new();
        Self::normalize_blocks_without_burning_unattributed(
            &self.interim_raw_blocks,
            &mut self.speaker_normalization_state,
            &mut interim_segments,
            true,
        );

        if !self.interim_text.trim().is_empty() && interim_segments.is_empty() {
            interim_segments.push(LiveSoundTranscriptSegmentPayload {
                speaker_id: None,
                speaker_label: None,
                text: self.interim_text.clone(),
                is_interim: true,
                timestamp_ms: 0,
            });
        }

        let current_timestamp_ms = if final_segments.is_empty() && interim_segments.is_empty() {
            0
        } else {
            self.transcript_started_at
                .get_or_insert_with(Instant::now)
                .elapsed()
                .as_millis() as u64
        };
        self.final_segment_timestamps_ms
            .truncate(final_segments.len());
        self.final_segment_timestamps_ms
            .resize(final_segments.len(), current_timestamp_ms);
        for (segment, timestamp_ms) in final_segments
            .iter_mut()
            .zip(self.final_segment_timestamps_ms.iter().copied())
        {
            segment.timestamp_ms = timestamp_ms;
        }
        self.interim_segment_timestamps_ms
            .truncate(interim_segments.len());
        self.interim_segment_timestamps_ms
            .resize(interim_segments.len(), current_timestamp_ms);
        for (segment, timestamp_ms) in interim_segments
            .iter_mut()
            .zip(self.interim_segment_timestamps_ms.iter().copied())
        {
            segment.timestamp_ms = timestamp_ms;
        }
        final_segments.extend(interim_segments);

        LiveSoundTranscriptionStatePayload {
            active: self.active,
            recording: self.recording,
            processing_llm: self.processing_llm,
            binding_id: self.binding_id.clone(),
            error_message: self.error_message.clone(),
            final_text: self.final_text.clone(),
            interim_text: self.interim_text.clone(),
            segments: final_segments,
            auto_stop_remaining_seconds,
        }
    }
}

static LIVE_SOUND_TRANSCRIPTION_STATE: LazyLock<Mutex<LiveSoundTranscriptionRuntime>> =
    LazyLock::new(|| Mutex::new(LiveSoundTranscriptionRuntime::default()));
static LIVE_SOUND_AUTO_STOP_TASK: LazyLock<Mutex<Option<JoinHandle<()>>>> =
    LazyLock::new(|| Mutex::new(None));
static NEXT_LIVE_SOUND_SESSION_ID: AtomicU64 = AtomicU64::new(1);

fn remaining_seconds(duration: Duration) -> u64 {
    let millis = duration.as_millis();
    if millis == 0 {
        0
    } else {
        ((millis + 999) / 1000) as u64
    }
}

fn emit_state_update(app: &AppHandle, payload: &LiveSoundTranscriptionStatePayload) {
    let _ = app.emit(EVENT_KEBAB, payload.clone());
    let _ = app.emit(EVENT_SNAKE, payload.clone());
}

fn cancel_auto_stop_task() {
    if let Ok(mut guard) = LIVE_SOUND_AUTO_STOP_TASK.lock() {
        if let Some(handle) = guard.take() {
            handle.abort();
        }
    }
}

fn current_auto_stop_remaining(session_id: u64) -> Option<Duration> {
    LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .ok()
        .and_then(|state| {
            if state.session_id != session_id || !state.recording {
                return None;
            }
            state
                .auto_stop_deadline
                .map(|deadline| deadline.saturating_duration_since(Instant::now()))
        })
}

fn spawn_auto_stop_task(app: &AppHandle, session_id: u64) {
    cancel_auto_stop_task();

    let app = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            let Some(remaining) = current_auto_stop_remaining(session_id) else {
                return;
            };

            emit_state_update(&app, &get_state_payload());

            if remaining.is_zero() {
                let app_for_stop = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::actions::stop_live_sound_transcription_session(&app_for_stop);
                });
                return;
            }

            tokio::time::sleep(remaining.min(Duration::from_secs(1))).await;
        }
    });

    if let Ok(mut guard) = LIVE_SOUND_AUTO_STOP_TASK.lock() {
        *guard = Some(handle);
    }
}

fn update_state<F>(app: &AppHandle, updater: F)
where
    F: FnOnce(&mut LiveSoundTranscriptionRuntime),
{
    let payload = {
        let mut guard = match LIVE_SOUND_TRANSCRIPTION_STATE.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        updater(&mut guard);
        guard.to_payload()
    };
    emit_state_update(app, &payload);
}

fn try_update_state<T, F>(app: &AppHandle, updater: F) -> Result<T, String>
where
    F: FnOnce(&mut LiveSoundTranscriptionRuntime) -> Result<T, String>,
{
    let (result, payload) = {
        let mut state = LIVE_SOUND_TRANSCRIPTION_STATE
            .lock()
            .map_err(|_| "Failed to lock live sound transcript".to_string())?;
        let result = updater(&mut state)?;
        (result, state.to_payload())
    };
    emit_state_update(app, &payload);
    Ok(result)
}

fn update_state_if_session_matches<F>(app: &AppHandle, session_id: u64, updater: F) -> bool
where
    F: FnOnce(&mut LiveSoundTranscriptionRuntime),
{
    let payload = {
        let mut guard = match LIVE_SOUND_TRANSCRIPTION_STATE.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        if session_id == 0 || guard.session_id != session_id {
            return false;
        }
        updater(&mut guard);
        guard.to_payload()
    };
    emit_state_update(app, &payload);
    true
}

pub fn get_state_payload() -> LiveSoundTranscriptionStatePayload {
    LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .map(|mut state| state.to_payload())
        .unwrap_or_default()
}

pub fn current_session_id() -> u64 {
    LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .map(|state| state.session_id)
        .unwrap_or_default()
}

pub fn is_session_current(session_id: u64) -> bool {
    session_id != 0
        && LIVE_SOUND_TRANSCRIPTION_STATE
            .lock()
            .map(|state| state.session_id == session_id)
            .unwrap_or(false)
}

pub fn is_recording() -> bool {
    LIVE_SOUND_TRANSCRIPTION_STATE
        .lock()
        .map(|state| state.recording)
        .unwrap_or(false)
}

pub fn activate_session(
    app: &AppHandle,
    binding_id: String,
    auto_stop_minutes: u32,
) -> Result<u64, String> {
    let session_id = NEXT_LIVE_SOUND_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    let auto_stop_deadline = if auto_stop_minutes > 0 {
        Some(Instant::now() + Duration::from_secs(u64::from(auto_stop_minutes).saturating_mul(60)))
    } else {
        None
    };
    let should_start_auto_stop = auto_stop_deadline.is_some();

    try_update_state(app, move |state| {
        if state.processing_llm {
            return Err(
                "Wait for LLM processing before starting a new Live Monitor session.".to_string(),
            );
        }
        if state.recording {
            return Err("Live sound audio session is already active".to_string());
        }
        state.active = true;
        state.recording = true;
        state.processing_llm = false;
        state.binding_id = Some(binding_id);
        state.error_message = None;
        state.interim_text.clear();
        state.interim_raw_blocks.clear();
        state.interim_segment_timestamps_ms.clear();
        state.session_id = session_id;
        state.auto_stop_deadline = auto_stop_deadline;
        Ok(())
    })?;

    cancel_auto_stop_task();
    if should_start_auto_stop {
        spawn_auto_stop_task(app, session_id);
    }

    Ok(session_id)
}

pub fn finish_session(app: &AppHandle) {
    cancel_auto_stop_task();
    update_state(app, |state| {
        state.active = false;
        state.recording = false;
        state.processing_llm = false;
        state.binding_id = None;
        state.interim_text.clear();
        state.interim_raw_blocks.clear();
        state.interim_segment_timestamps_ms.clear();
        state.session_id = 0;
        state.auto_stop_deadline = None;
    });
}

pub fn set_recording_if_session_matches(app: &AppHandle, session_id: u64, recording: bool) {
    if !recording && is_session_current(session_id) {
        cancel_auto_stop_task();
    }

    update_state_if_session_matches(app, session_id, move |state| {
        state.recording = recording;
        if recording {
            state.active = true;
        } else {
            state.auto_stop_deadline = None;
        }
    });
}

pub fn begin_processing_llm(app: &AppHandle) -> Result<(u64, String), String> {
    try_update_state(app, |state| {
        if state.recording || state.processing_llm {
            return Err("Wait for the current Live Monitor operation to finish.".to_string());
        }
        let text = state.final_text.trim().to_string();
        if text.is_empty() {
            return Err("Live sound transcript is empty.".to_string());
        }
        state.processing_llm = true;
        state.error_message = None;
        Ok((state.session_id, text))
    })
}

pub fn finish_processing_llm(
    app: &AppHandle,
    session_id: u64,
    result: Result<Option<String>, String>,
) -> Result<(), String> {
    try_update_state(app, |state| {
        if state.session_id != session_id || !state.processing_llm {
            return Err("The Live Monitor session changed during LLM processing.".to_string());
        }
        state.processing_llm = false;
        match &result {
            Ok(Some(text)) => replace_final_text_in_state(state, text.clone()),
            Ok(None) => state.error_message = None,
            Err(error) => state.error_message = Some(error.clone()),
        }
        Ok(())
    })?;
    result.map(|_| ())
}

pub fn set_error(app: &AppHandle, error_message: Option<String>) {
    update_state(app, move |state| {
        state.error_message = error_message;
    });
}

pub fn set_error_if_session_matches(
    app: &AppHandle,
    session_id: u64,
    error_message: Option<String>,
) {
    update_state_if_session_matches(app, session_id, move |state| {
        state.error_message = error_message;
    });
}

pub fn clear_transcript(app: &AppHandle) -> Result<(), String> {
    try_update_state(app, |state| {
        if state.processing_llm {
            return Err("Wait for LLM processing before clearing the transcript.".to_string());
        }
        state.final_text.clear();
        state.processed_final_text = None;
        state.interim_text.clear();
        state.final_raw_blocks.clear();
        state.interim_raw_blocks.clear();
        state.elevenlabs_segments.clear();
        state.speaker_normalization_state = SpeakerBlockNormalizationState::default();
        state.final_segment_timestamps_ms.clear();
        state.interim_segment_timestamps_ms.clear();
        state.transcript_started_at = None;
        state.error_message = None;
        Ok(())
    })
}

pub fn replace_final_text(app: &AppHandle, final_text: String) {
    update_state(app, move |state| {
        replace_final_text_in_state(state, final_text);
    });
}

fn replace_final_text_in_state(state: &mut LiveSoundTranscriptionRuntime, final_text: String) {
    let final_text = final_text.trim().to_string();
    state.processed_final_text = (!final_text.is_empty()).then(|| final_text.clone());
    state.final_text = final_text;
    state.interim_text.clear();
    state.final_raw_blocks.clear();
    state.interim_raw_blocks.clear();
    state.elevenlabs_segments.clear();
    state.speaker_normalization_state = SpeakerBlockNormalizationState::default();
    state.final_segment_timestamps_ms.truncate(1);
    state.interim_segment_timestamps_ms.clear();
    state.error_message = None;
}

pub fn append_final_result_if_session_matches(
    app: &AppHandle,
    session_id: u64,
    final_text: &str,
    raw_blocks: Vec<RawSpeakerBlock>,
    add_separator: bool,
) {
    update_state_if_session_matches(app, session_id, move |state| {
        state.push_final_text(final_text, add_separator);
        if !raw_blocks.is_empty() {
            state.final_raw_blocks.extend(raw_blocks);
        }
    });
}

pub fn set_interim_result_if_session_matches(
    app: &AppHandle,
    session_id: u64,
    interim_text: String,
    raw_blocks: Vec<RawSpeakerBlock>,
) {
    update_state_if_session_matches(app, session_id, move |state| {
        state.interim_text = interim_text.trim().to_string();
        state.interim_raw_blocks = raw_blocks;
        state.interim_segment_timestamps_ms.clear();
    });
}

pub fn apply_elevenlabs_packet_if_session_matches(
    app: &AppHandle,
    session_id: u64,
    packet: ElevenLabsLivePacket,
) {
    update_state_if_session_matches(app, session_id, move |state| {
        state.apply_elevenlabs_packet(packet);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, speaker_id: Option<&str>) -> ElevenLabsLiveWord {
        ElevenLabsLiveWord {
            text: text.to_string(),
            start_ms: None,
            end_ms: None,
            speaker_id: speaker_id.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn committed_then_diarized_replaces_segment_without_duplicating_text() {
        let mut runtime = LiveSoundTranscriptionRuntime::default();
        runtime.recording = true;

        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Partial {
            text: "Hel".to_string(),
            words: vec![word("Hel", None)],
        });
        let p1 = runtime.to_payload();
        assert_eq!(p1.interim_text, "Hel");
        assert_eq!(p1.segments.len(), 1);
        assert!(p1.segments[0].is_interim);
        assert_eq!(p1.segments[0].speaker_id, None);

        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Committed {
            text: "Hello world".to_string(),
            segment_index: Some(0),
        });
        let p2 = runtime.to_payload();
        assert_eq!(p2.final_text, "Hello world");
        assert_eq!(p2.interim_text, "");
        assert_eq!(p2.segments.len(), 1);
        assert!(!p2.segments[0].is_interim);
        assert_eq!(p2.segments[0].text, "Hello world");

        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Diarized {
            segment_index: Some(0),
            text: "Hello world".to_string(),
            words: vec![word("Hello", Some("spk_0")), word("world", Some("spk_1"))],
        });
        let p3 = runtime.to_payload();
        assert_eq!(p3.final_text, "Hello world");
        assert_eq!(p3.segments.len(), 2);
        assert_eq!(p3.segments[0].speaker_id, Some(0));
        assert_eq!(p3.segments[0].text, "Hello");
        assert_eq!(p3.segments[1].speaker_id, Some(1));
        assert_eq!(p3.segments[1].text, "world");

        // Duplicate committed packet for the same segment index must not append again
        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Committed {
            text: "Hello world".to_string(),
            segment_index: Some(0),
        });
        let p4 = runtime.to_payload();
        assert_eq!(p4.final_text, "Hello world");
        assert_eq!(p4.segments.len(), 2);
    }

    #[test]
    fn retroactive_speakers_merged_rewrites_committed_turns_and_coalesces_adjacent_blocks() {
        let mut runtime = LiveSoundTranscriptionRuntime::default();
        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Committed {
            text: "Hi there".to_string(),
            segment_index: Some(0),
        });
        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Diarized {
            segment_index: Some(0),
            text: "Hi there".to_string(),
            words: vec![word("Hi", Some("spk_a")), word("there", Some("spk_b"))],
        });
        let before = runtime.to_payload();
        assert_eq!(before.segments.len(), 2);
        assert_eq!(before.segments[0].speaker_id, Some(0));
        assert_eq!(before.segments[1].speaker_id, Some(1));

        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::SpeakersMerged {
            from_speaker_id: "spk_b".to_string(),
            to_speaker_id: "spk_a".to_string(),
        });
        let after = runtime.to_payload();
        assert_eq!(after.segments.len(), 1);
        assert_eq!(after.segments[0].speaker_id, Some(0));
        assert_eq!(after.segments[0].speaker_label.as_deref(), Some("Speaker 0"));
        assert_eq!(after.segments[0].text, "Hi there");
    }

    #[test]
    fn pending_committed_segment_appears_alongside_prior_diarized_without_burning_speaker_id() {
        let mut runtime = LiveSoundTranscriptionRuntime::default();
        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Committed {
            text: "First".to_string(),
            segment_index: Some(0),
        });
        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Diarized {
            segment_index: Some(0),
            text: "First".to_string(),
            words: vec![word("First", Some("spk_0"))],
        });
        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Committed {
            text: "Second pending".to_string(),
            segment_index: Some(1),
        });
        let pending = runtime.to_payload();
        assert_eq!(pending.segments.len(), 2);
        assert_eq!(pending.segments[0].speaker_id, Some(0));
        assert_eq!(pending.segments[0].text, "First");
        assert_eq!(pending.segments[1].speaker_id, None);
        assert_eq!(pending.segments[1].text, "Second pending");

        runtime.apply_elevenlabs_packet(ElevenLabsLivePacket::Diarized {
            segment_index: Some(1),
            text: "Second pending".to_string(),
            words: vec![
                word("Second", Some("spk_1")),
                word("pending", Some("spk_1")),
            ],
        });
        let finalized = runtime.to_payload();
        assert_eq!(finalized.segments.len(), 2);
        assert_eq!(finalized.segments[0].speaker_id, Some(0));
        assert_eq!(finalized.segments[1].speaker_id, Some(1));
        assert_eq!(finalized.segments[1].text, "Second pending");
    }
}
