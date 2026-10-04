use crate::actions::{
    perform_history_live_transcription, perform_transcription_for_profile,
    process_transcription_output, settings_for_history_transcription, should_use_live_streaming,
    TranscriptionOutcome,
};
use crate::managers::{
    history::{HistoryManager, PaginatedHistory},
    history_replay::{HistoryReplayGuard, HISTORY_REPLAY_CANCELLED},
    transcription::TranscriptionManager,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
#[specta::specta]
pub async fn get_history_entries(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    cursor: Option<i64>,
    limit: Option<usize>,
) -> Result<PaginatedHistory, String> {
    history_manager
        .get_history_entries(cursor, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_history_entry_saved(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .toggle_saved_status(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_audio_file_path(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    file_name: String,
) -> Result<String, String> {
    let path = history_manager
        .get_audio_file_path(&file_name)
        .map_err(|e| e.to_string())?;
    path.to_str()
        .ok_or_else(|| "Invalid file path".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_history_entry(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .delete_entry(id)
        .map(|_| ())
        .map_err(serialize_history_delete_error)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_all_history_entries(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
) -> Result<usize, String> {
    history_manager
        .delete_all_entries()
        .map_err(serialize_history_delete_error)
}

fn serialize_history_delete_error(error: crate::managers::history::HistoryDeleteError) -> String {
    serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_transcription(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    _transcription_manager: State<'_, Arc<TranscriptionManager>>,
    id: i64,
) -> Result<(), String> {
    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;

    if entry.action_type != "transcribe" {
        return Err("Only transcription history entries can be re-transcribed".to_string());
    }

    let audio_path = history_manager
        .get_audio_file_path(&entry.file_name)
        .map_err(|e| e.to_string())?;
    let samples = crate::audio_toolkit::read_wav_samples(&audio_path)
        .map_err(|e| format!("Failed to load audio: {}", e))?;

    if samples.is_empty() {
        return Err("Recording has no audio samples".to_string());
    }

    let settings = settings_for_history_transcription(crate::settings::get_settings(&app));
    let profile_id = Some(settings.active_profile_id.clone());
    let replay_guard = if should_use_live_streaming(&settings) {
        Some(HistoryReplayGuard::start(&app, id, samples.len()).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let transcription = if let Some(guard) = &replay_guard {
        tokio::select! {
            biased;
            _ = guard.replay.cancelled() => return Err(HISTORY_REPLAY_CANCELLED.to_string()),
            result = perform_history_live_transcription(&app, &samples, &settings, &guard.replay) => result?,
        }
    } else {
        match perform_transcription_for_profile(&app, samples, None, profile_id.clone(), &settings).await {
            TranscriptionOutcome::Success(text) => text,
            TranscriptionOutcome::Cancelled => {
                return Err("Re-transcription was cancelled".to_string());
            }
            TranscriptionOutcome::Error { message, .. } => {
                return Err(message);
            }
        }
    };

    if transcription.is_empty() {
        return Err("Recording contains no speech".to_string());
    }

    let processing = process_transcription_output(
        &app,
        &settings,
        &transcription,
        profile_id.as_deref(),
        "History retry",
        false,
    );
    let processed = if let Some(guard) = &replay_guard {
        guard.replay.emit("processing");
        tokio::select! {
            biased;
            _ = guard.replay.cancelled() => return Err(HISTORY_REPLAY_CANCELLED.to_string()),
            result = processing => result,
        }
    } else {
        processing.await
    }
    .ok_or_else(|| "Re-transcription post-processing was cancelled".to_string())?;

    if replay_guard.as_ref().is_some_and(|guard| guard.replay.is_cancelled()) {
        return Err(HISTORY_REPLAY_CANCELLED.to_string());
    }
    history_manager
        .update_transcription(
            id,
            transcription,
            processed.post_processed_text,
            processed.post_process_prompt,
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn cancel_history_entry_transcription(id: i64) {
    crate::managers::history_replay::cancel(id);
}

#[tauri::command]
#[specta::specta]
pub async fn update_history_limit(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    limit: usize,
) -> Result<(), String> {
    let bounded_limit = limit.min(crate::settings::MAX_HISTORY_LIMIT);
    let mut settings = crate::settings::get_settings(&app);
    settings.history_limit = bounded_limit;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_recording_retention_period(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    period: String,
) -> Result<(), String> {
    use crate::settings::RecordingRetentionPeriod;

    let retention_period = match period.as_str() {
        "never" => RecordingRetentionPeriod::Never,
        "preserve_limit" => RecordingRetentionPeriod::PreserveLimit,
        "days_3" | "days3" => RecordingRetentionPeriod::Days3,
        "weeks_2" | "weeks2" => RecordingRetentionPeriod::Weeks2,
        "months_3" | "months3" => RecordingRetentionPeriod::Months3,
        _ => return Err(format!("Invalid retention period: {}", period)),
    };

    let mut settings = crate::settings::get_settings(&app);
    settings.recording_retention_period = retention_period;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}
