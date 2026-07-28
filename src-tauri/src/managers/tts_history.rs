//! Independent, opt-in history for successful text-to-speech output.
//!
//! This module deliberately does not use the transcription history database or
//! recordings directory. Only managed copies in `tts-history-audio` may be
//! removed; external user output paths are metadata and are never deleted.

use crate::settings::{TtsOutputFormat, TtsProvider, TtsSettings};
use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};

static UNIQUE_FILE_ID: AtomicU64 = AtomicU64::new(0);
pub const TTS_HISTORY_CHANGED_EVENT: &str = "tts-history-changed";

static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS tts_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            source_text TEXT NOT NULL,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            voice TEXT NOT NULL,
            output_format TEXT NOT NULL,
            managed_audio_filename TEXT NOT NULL UNIQUE,
            external_output_path TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_tts_history_timestamp
            ON tts_history(timestamp DESC, id DESC);",
    ),
    M::up(
        "ALTER TABLE tts_history
            ADD COLUMN group_id TEXT NOT NULL DEFAULT '';
        UPDATE tts_history
            SET group_id = 'legacy-' || id
            WHERE group_id = '';
        CREATE INDEX IF NOT EXISTS idx_tts_history_group
            ON tts_history(group_id, timestamp DESC, id DESC);",
    ),
    M::up(
        "ALTER TABLE tts_history ADD COLUMN prompt_preset_id TEXT;
        ALTER TABLE tts_history ADD COLUMN prompt_preset_name TEXT;
        ALTER TABLE tts_history ADD COLUMN resolved_instructions TEXT;",
    ),
    M::up(
        "ALTER TABLE tts_history
            ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'text';",
    ),
];

const ENTRY_COLUMNS: &str =
    "id, timestamp, group_id, source_text, source_kind, provider, model, voice, \
    output_format, managed_audio_filename, external_output_path, prompt_preset_id, \
    prompt_preset_name, resolved_instructions";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsHistorySourceKind {
    Text,
    Markdown,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TtsHistoryEntry {
    pub id: i64,
    pub timestamp: i64,
    /// Stable source identifier shared by append-only provider/voice variants.
    pub group_id: String,
    /// Original, unprocessed input text retained for later re-synthesis.
    pub source_text: String,
    pub source_kind: TtsHistorySourceKind,
    pub provider: TtsProvider,
    pub model: String,
    pub voice: String,
    pub output_format: TtsOutputFormat,
    pub managed_audio_filename: String,
    pub external_output_path: Option<String>,
    /// Optional saved TTS preset identity used for this variant.
    pub prompt_preset_id: Option<String>,
    pub prompt_preset_name: Option<String>,
    /// Resolved provider instructions, if any. API credentials are never
    /// stored in history.
    pub resolved_instructions: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewTtsHistoryEntry {
    /// Stable source identifier. Re-synthesized variants reuse this value but
    /// are always inserted as additional rows.
    pub group_id: String,
    /// Original, unprocessed input text; callers must not substitute the
    /// normalized/preprocessed provider request text.
    pub source_text: String,
    pub source_kind: TtsHistorySourceKind,
    pub provider: TtsProvider,
    pub model: String,
    pub voice: String,
    pub output_format: TtsOutputFormat,
    pub external_output_path: Option<PathBuf>,
    pub prompt_preset_id: Option<String>,
    pub prompt_preset_name: Option<String>,
    pub resolved_instructions: Option<String>,
}

pub fn metadata_from_settings(
    settings: &TtsSettings,
    source_text: String,
    source_kind: TtsHistorySourceKind,
    group_id: String,
    external_output_path: Option<PathBuf>,
) -> NewTtsHistoryEntry {
    let (model, voice) = match settings.provider {
        TtsProvider::Soniox => (settings.soniox_model.clone(), settings.soniox_voice.clone()),
        TtsProvider::Deepgram => (
            settings.deepgram_model.clone(),
            settings.deepgram_model.clone(),
        ),
        TtsProvider::OpenAi => (settings.openai_model.clone(), settings.openai_voice.clone()),
    };
    let instructions_supported = settings.provider == TtsProvider::OpenAi
        && crate::managers::tts::TtsManager::openai_model_supports_instructions(
            &settings.openai_model,
        );
    let selected_preset = instructions_supported
        .then(|| {
            settings
                .prompt_presets
                .iter()
                .find(|preset| preset.id == settings.selected_prompt_id)
        })
        .flatten();
    let resolved_instructions = instructions_supported
        .then(|| settings.openai_instructions.trim())
        .filter(|instructions| !instructions.is_empty())
        .map(str::to_string);

    NewTtsHistoryEntry {
        group_id,
        source_text,
        source_kind,
        provider: settings.provider,
        model,
        voice,
        output_format: settings.output_format,
        external_output_path,
        prompt_preset_id: selected_preset.map(|preset| preset.id.clone()),
        prompt_preset_name: selected_preset.map(|preset| preset.name.clone()),
        resolved_instructions,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsHistoryManagedAudioDeleteStatus {
    Deleted,
    Missing,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TtsHistoryDeleteOutcome {
    pub id: i64,
    pub record_deleted: bool,
    pub managed_audio_status: TtsHistoryManagedAudioDeleteStatus,
    pub managed_audio_error: Option<String>,
}

pub struct TtsHistoryManager {
    app_handle: AppHandle,
    db_path: PathBuf,
    audio_dir: PathBuf,
    mutation_lock: Mutex<()>,
}

impl TtsHistoryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let db_path = app_data_dir.join("tts-history.db");
        let audio_dir = app_data_dir.join("tts-history-audio");
        fs::create_dir_all(&audio_dir).with_context(|| {
            format!(
                "Failed to create TTS history audio directory {}",
                audio_dir.display()
            )
        })?;

        let manager = Self {
            app_handle: app_handle.clone(),
            db_path,
            audio_dir,
            mutation_lock: Mutex::new(()),
        };
        manager.init_database()?;
        Ok(manager)
    }

    fn init_database(&self) -> Result<()> {
        let mut connection = self.connection()?;
        migrations().to_latest(&mut connection)?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection> {
        let connection = Connection::open(&self.db_path)
            .with_context(|| format!("Failed to open {}", self.db_path.display()))?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(connection)
    }

    /// Saves a successful synthesis only when TTS history is enabled.
    ///
    /// Audio is first copied to a hidden sibling partial file and atomically
    /// renamed into the managed directory. If database insertion fails, that
    /// new managed copy is removed.
    pub fn save_success(
        &self,
        metadata: NewTtsHistoryEntry,
        audio_source_path: impl AsRef<Path>,
    ) -> Result<Option<TtsHistoryEntry>> {
        if !crate::settings::get_settings(&self.app_handle)
            .tts
            .history_enabled
        {
            return Ok(None);
        }

        self.store_success(metadata, audio_source_path.as_ref())
            .map(Some)
    }

    /// Appends an explicitly confirmed re-synthesis result.
    ///
    /// This intentionally bypasses the passive-capture toggle: the caller has
    /// already warned the user that regeneration is a paid API operation and
    /// obtained explicit confirmation. Existing variants are never replaced.
    pub fn append_confirmed_regeneration_success(
        &self,
        metadata: NewTtsHistoryEntry,
        audio_source_path: impl AsRef<Path>,
    ) -> Result<TtsHistoryEntry> {
        self.store_success(metadata, audio_source_path.as_ref())
    }

    fn store_success(
        &self,
        metadata: NewTtsHistoryEntry,
        audio_source_path: &Path,
    ) -> Result<TtsHistoryEntry> {
        let _mutation_guard = self.mutation_lock.lock();
        let settings = crate::settings::get_settings(&self.app_handle).tts;
        let source_bytes = fs::metadata(audio_source_path)
            .with_context(|| {
                format!(
                    "Failed to inspect TTS history audio source {}",
                    audio_source_path.display()
                )
            })?
            .len();
        let maximum_bytes = u64::from(settings.history_max_storage_mb.clamp(1, 1_048_576))
            .saturating_mul(1024 * 1024);
        if source_bytes > maximum_bytes {
            return Err(anyhow!(
                "The completed audio is {} bytes, larger than the configured TTS History maximum of {} MB",
                source_bytes,
                settings.history_max_storage_mb.clamp(1, 1_048_576)
            ));
        }
        let timestamp = chrono::Utc::now().timestamp_millis();
        let managed_audio_filename = new_managed_filename(timestamp, metadata.output_format);
        let managed_path = self.managed_audio_path(&managed_audio_filename)?;
        atomic_copy_new(audio_source_path, &managed_path, settings.disk_reserve_mb)?;

        let external_output_path = metadata
            .external_output_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        let connection = self.connection()?;
        let insert_result = insert_entry(
            &connection,
            timestamp,
            &metadata,
            &managed_audio_filename,
            external_output_path.as_deref(),
        );
        match insert_result {
            Ok(entry) => {
                if let Err(error) = self.enforce_retention_locked(&connection, &settings) {
                    self.report_retention_error(&error);
                }
                let _ = self.app_handle.emit(TTS_HISTORY_CHANGED_EVENT, ());
                Ok(entry)
            }
            Err(error) => {
                if let Err(remove_error) = fs::remove_file(&managed_path) {
                    log::warn!(
                        "Failed to remove unreferenced TTS history audio {}: {}",
                        managed_path.display(),
                        remove_error
                    );
                }
                Err(error)
            }
        }
    }

    pub fn list_entries(&self) -> Result<Vec<TtsHistoryEntry>> {
        list_entries_with_connection(&self.connection()?)
    }

    pub fn enforce_retention(&self) -> Result<usize> {
        let _mutation_guard = self.mutation_lock.lock();
        let settings = crate::settings::get_settings(&self.app_handle).tts;
        let connection = self.connection()?;
        let deleted = self.enforce_retention_locked(&connection, &settings)?;
        if deleted != 0 {
            let _ = self.app_handle.emit(TTS_HISTORY_CHANGED_EVENT, ());
        }
        Ok(deleted)
    }

    fn enforce_retention_locked(
        &self,
        connection: &Connection,
        settings: &crate::settings::TtsSettings,
    ) -> Result<usize> {
        let maximum_entries =
            usize::try_from(settings.history_max_entries.clamp(1, 100_000)).unwrap_or(100_000);
        let maximum_bytes = u64::from(settings.history_max_storage_mb.clamp(1, 1_048_576))
            .saturating_mul(1024 * 1024);
        let mut entries = list_entries_with_connection(connection)?;
        entries.reverse();
        let mut entry_sizes = Vec::with_capacity(entries.len());
        for entry in entries {
            let path = self.managed_audio_path(&entry.managed_audio_filename)?;
            let bytes = match fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => metadata.len(),
                Ok(_) => 0,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "Failed to inspect retained TTS History audio {}",
                            path.display()
                        )
                    })
                }
            };
            entry_sizes.push((entry, bytes));
        }

        let delete_count = retention_delete_count(
            &entry_sizes
                .iter()
                .map(|(_, bytes)| *bytes)
                .collect::<Vec<_>>(),
            maximum_entries,
            maximum_bytes,
        );
        let mut deleted = 0;
        for (entry, _) in entry_sizes.into_iter().take(delete_count) {
            let (status, error) = self.remove_managed_audio(&entry.managed_audio_filename);
            if status == TtsHistoryManagedAudioDeleteStatus::Failed {
                return Err(anyhow!(
                    "Could not remove oldest TTS History result {} while applying retention limits: {}",
                    entry.id,
                    error.as_deref().unwrap_or("unknown filesystem error")
                ));
            }
            let affected =
                connection.execute("DELETE FROM tts_history WHERE id = ?1", params![entry.id])?;
            if affected != 0 {
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    fn report_retention_error(&self, error: &anyhow::Error) {
        let message = format!("TTS History retention cleanup failed: {error}");
        log::error!("{message}");
        let _ = self.app_handle.emit("tts-history-error", message);
    }

    pub fn get_entry_by_id(&self, id: i64) -> Result<Option<TtsHistoryEntry>> {
        get_entry_with_connection(&self.connection()?, id)
    }

    /// Resolves a retained audio path exclusively from a validated database
    /// entry. Callers never provide a managed filename or directory fragment.
    pub fn retained_audio_path(&self, id: i64) -> Result<Option<PathBuf>> {
        let Some(entry) = self.get_entry_by_id(id)? else {
            return Ok(None);
        };
        Ok(Some(
            self.managed_audio_path(&entry.managed_audio_filename)?,
        ))
    }

    pub fn delete_entry(&self, id: i64) -> Result<bool> {
        let Some(outcome) = self.delete_entry_detailed(id)? else {
            return Ok(false);
        };
        if outcome.managed_audio_status == TtsHistoryManagedAudioDeleteStatus::Failed {
            return Err(anyhow!(
                "History record {id} was deleted, but its retained audio could not be removed: {}",
                outcome
                    .managed_audio_error
                    .as_deref()
                    .unwrap_or("unknown filesystem error")
            ));
        }
        Ok(true)
    }

    /// Deletes the database row and reports whether its managed audio was
    /// deleted, already missing, or could not be removed. External user output
    /// paths are metadata only and are never touched.
    pub fn delete_entry_detailed(&self, id: i64) -> Result<Option<TtsHistoryDeleteOutcome>> {
        let _mutation_guard = self.mutation_lock.lock();
        let connection = self.connection()?;
        let entry = get_entry_with_connection(&connection, id)?;
        let deleted = connection.execute("DELETE FROM tts_history WHERE id = ?1", params![id])?;
        if deleted == 0 {
            return Ok(None);
        }
        let (managed_audio_status, managed_audio_error) = match entry {
            Some(entry) => self.remove_managed_audio(&entry.managed_audio_filename),
            None => (
                TtsHistoryManagedAudioDeleteStatus::Missing,
                Some("History row disappeared before its audio metadata was read".to_string()),
            ),
        };
        let _ = self.app_handle.emit(TTS_HISTORY_CHANGED_EVENT, ());
        Ok(Some(TtsHistoryDeleteOutcome {
            id,
            record_deleted: true,
            managed_audio_status,
            managed_audio_error,
        }))
    }

    pub fn delete_all_entries(&self) -> Result<usize> {
        let _mutation_guard = self.mutation_lock.lock();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let filenames = {
            let mut statement =
                transaction.prepare("SELECT managed_audio_filename FROM tts_history")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let deleted = transaction.execute("DELETE FROM tts_history", [])?;
        transaction.commit()?;
        if deleted != 0 {
            let _ = self.app_handle.emit(TTS_HISTORY_CHANGED_EVENT, ());
        }

        for filename in filenames {
            let (status, error) = self.remove_managed_audio(&filename);
            if status == TtsHistoryManagedAudioDeleteStatus::Failed {
                log::warn!(
                    "TTS history row was deleted but managed audio removal failed: {}",
                    error.as_deref().unwrap_or("unknown filesystem error")
                );
            }
        }
        Ok(deleted)
    }

    /// Copies a retained managed audio file to a new user-selected path.
    ///
    /// Existing destination files are not overwritten. The destination becomes
    /// visible only after the complete copy has been flushed and renamed.
    pub fn export_audio(&self, id: i64, destination: impl AsRef<Path>) -> Result<PathBuf> {
        let entry = self
            .get_entry_by_id(id)?
            .ok_or_else(|| anyhow!("TTS history entry {id} not found"))?;
        let source = self.managed_audio_path(&entry.managed_audio_filename)?;
        let destination = destination.as_ref();
        let disk_reserve_mb = crate::settings::get_settings(&self.app_handle)
            .tts
            .disk_reserve_mb;
        atomic_copy_new(&source, destination, disk_reserve_mb)?;
        Ok(destination.to_path_buf())
    }

    fn managed_audio_path(&self, filename: &str) -> Result<PathBuf> {
        validate_managed_filename(filename)?;
        Ok(self.audio_dir.join(filename))
    }

    fn remove_managed_audio(
        &self,
        filename: &str,
    ) -> (TtsHistoryManagedAudioDeleteStatus, Option<String>) {
        let Ok(path) = self.managed_audio_path(filename) else {
            let message = format!("Refusing unsafe TTS history audio filename: {filename}");
            log::error!("{message}");
            return (TtsHistoryManagedAudioDeleteStatus::Failed, Some(message));
        };
        match fs::remove_file(&path) {
            Ok(()) => (TtsHistoryManagedAudioDeleteStatus::Deleted, None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (
                TtsHistoryManagedAudioDeleteStatus::Missing,
                Some(format!(
                    "Retained TTS history audio was already missing: {}",
                    path.display()
                )),
            ),
            Err(error) => {
                let message = format!(
                    "Failed to delete managed TTS history audio {}: {}",
                    path.display(),
                    error
                );
                log::warn!("{message}");
                (TtsHistoryManagedAudioDeleteStatus::Failed, Some(message))
            }
        }
    }
}

fn retention_delete_count(
    oldest_first_sizes: &[u64],
    maximum_entries: usize,
    maximum_bytes: u64,
) -> usize {
    let mut retained_entries = oldest_first_sizes.len();
    let mut retained_bytes = oldest_first_sizes
        .iter()
        .copied()
        .fold(0_u64, u64::saturating_add);
    let mut delete_count = 0;
    for bytes in oldest_first_sizes {
        if retained_entries <= maximum_entries && retained_bytes <= maximum_bytes {
            break;
        }
        retained_entries = retained_entries.saturating_sub(1);
        retained_bytes = retained_bytes.saturating_sub(*bytes);
        delete_count += 1;
    }
    delete_count
}

fn migrations() -> Migrations<'static> {
    Migrations::new(MIGRATIONS.to_vec())
}

fn insert_entry(
    connection: &Connection,
    timestamp: i64,
    metadata: &NewTtsHistoryEntry,
    managed_audio_filename: &str,
    external_output_path: Option<&str>,
) -> Result<TtsHistoryEntry> {
    if metadata.group_id.trim().is_empty() {
        return Err(anyhow!("TTS history group_id must not be empty"));
    }
    connection.execute(
        "INSERT INTO tts_history (
            timestamp, group_id, source_text, source_kind, provider, model, voice,
            output_format, managed_audio_filename, external_output_path,
            prompt_preset_id, prompt_preset_name, resolved_instructions
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            timestamp,
            &metadata.group_id,
            &metadata.source_text,
            source_kind_to_db(metadata.source_kind),
            provider_to_db(metadata.provider),
            &metadata.model,
            &metadata.voice,
            output_format_to_db(metadata.output_format),
            managed_audio_filename,
            external_output_path,
            metadata.prompt_preset_id.as_deref(),
            metadata.prompt_preset_name.as_deref(),
            metadata.resolved_instructions.as_deref(),
        ],
    )?;
    let id = connection.last_insert_rowid();
    get_entry_with_connection(connection, id)?
        .ok_or_else(|| anyhow!("Inserted TTS history entry {id} could not be read back"))
}

fn list_entries_with_connection(connection: &Connection) -> Result<Vec<TtsHistoryEntry>> {
    let query = format!("SELECT {ENTRY_COLUMNS} FROM tts_history ORDER BY timestamp DESC, id DESC");
    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map([], map_entry)?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn get_entry_with_connection(connection: &Connection, id: i64) -> Result<Option<TtsHistoryEntry>> {
    let query = format!("SELECT {ENTRY_COLUMNS} FROM tts_history WHERE id = ?1");
    Ok(connection
        .query_row(&query, params![id], map_entry)
        .optional()?)
}

fn map_entry(row: &Row<'_>) -> rusqlite::Result<TtsHistoryEntry> {
    let provider: String = row.get("provider")?;
    let output_format: String = row.get("output_format")?;
    let source_kind: String = row.get("source_kind")?;
    Ok(TtsHistoryEntry {
        id: row.get("id")?,
        timestamp: row.get("timestamp")?,
        group_id: row.get("group_id")?,
        source_text: row.get("source_text")?,
        source_kind: source_kind_from_db(&source_kind).map_err(sql_conversion_error)?,
        provider: provider_from_db(&provider).map_err(sql_conversion_error)?,
        model: row.get("model")?,
        voice: row.get("voice")?,
        output_format: output_format_from_db(&output_format).map_err(sql_conversion_error)?,
        managed_audio_filename: row.get("managed_audio_filename")?,
        external_output_path: row.get("external_output_path")?,
        prompt_preset_id: row.get("prompt_preset_id")?,
        prompt_preset_name: row.get("prompt_preset_name")?,
        resolved_instructions: row.get("resolved_instructions")?,
    })
}

fn sql_conversion_error(error: anyhow::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

fn provider_to_db(provider: TtsProvider) -> &'static str {
    provider.as_str()
}

fn provider_from_db(value: &str) -> Result<TtsProvider> {
    match value {
        "soniox" => Ok(TtsProvider::Soniox),
        "deepgram" => Ok(TtsProvider::Deepgram),
        "openai" => Ok(TtsProvider::OpenAi),
        _ => Err(anyhow!("Unknown TTS provider in history: {value}")),
    }
}

fn output_format_to_db(format: TtsOutputFormat) -> &'static str {
    match format {
        TtsOutputFormat::Mp3 => "mp3",
        TtsOutputFormat::Wav => "wav",
    }
}

fn source_kind_to_db(source_kind: TtsHistorySourceKind) -> &'static str {
    match source_kind {
        TtsHistorySourceKind::Text => "text",
        TtsHistorySourceKind::Markdown => "markdown",
    }
}

fn source_kind_from_db(value: &str) -> Result<TtsHistorySourceKind> {
    match value {
        "text" => Ok(TtsHistorySourceKind::Text),
        "markdown" => Ok(TtsHistorySourceKind::Markdown),
        _ => Err(anyhow!("Unknown TTS history source kind: {value}")),
    }
}

fn output_format_from_db(value: &str) -> Result<TtsOutputFormat> {
    match value {
        "mp3" => Ok(TtsOutputFormat::Mp3),
        "wav" => Ok(TtsOutputFormat::Wav),
        _ => Err(anyhow!("Unknown TTS output format in history: {value}")),
    }
}

fn new_managed_filename(timestamp: i64, format: TtsOutputFormat) -> String {
    let sequence = UNIQUE_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let extension = output_format_to_db(format);
    format!(
        "tts-{timestamp}-{}-{sequence}.{extension}",
        std::process::id()
    )
}

fn validate_managed_filename(filename: &str) -> Result<()> {
    let path = Path::new(filename);
    let mut components = path.components();
    let valid = matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && !filename.is_empty()
        && !filename.contains('/')
        && !filename.contains('\\');
    if valid {
        Ok(())
    } else {
        Err(anyhow!("Invalid managed TTS history audio filename"))
    }
}

fn atomic_copy_new(source: &Path, destination: &Path, disk_reserve_mb: u32) -> Result<()> {
    let metadata = fs::metadata(source)
        .with_context(|| format!("Failed to inspect audio source {}", source.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!(
            "TTS history audio source is not a regular file: {}",
            source.display()
        ));
    }
    if destination.exists() {
        return Err(anyhow!(
            "Destination already exists: {}",
            destination.display()
        ));
    }
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("Destination must include a parent directory"))?;
    if !parent.is_dir() {
        return Err(anyhow!(
            "Destination directory does not exist: {}",
            parent.display()
        ));
    }
    let available = fs2::available_space(parent).with_context(|| {
        format!(
            "Failed to check free disk space for TTS history destination {}",
            parent.display()
        )
    })?;
    let reserve_bytes = u64::from(disk_reserve_mb.min(1_048_576)).saturating_mul(1024 * 1024);
    let required = metadata.len().saturating_add(reserve_bytes);
    if available < required {
        return Err(anyhow!(
            "Insufficient disk space for TTS history audio: {:.1} MiB available, {:.1} MiB required (including {} MiB reserve)",
            available as f64 / (1024.0 * 1024.0),
            required as f64 / (1024.0 * 1024.0),
            disk_reserve_mb.min(1_048_576)
        ));
    }
    let destination_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Destination filename is not valid Unicode"))?;
    let sequence = UNIQUE_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let partial = parent.join(format!(
        ".{destination_name}.{}-{sequence}.partial",
        std::process::id()
    ));

    let result = (|| -> Result<()> {
        let mut input = File::open(source)
            .with_context(|| format!("Failed to open audio source {}", source.display()))?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .with_context(|| format!("Failed to create partial copy {}", partial.display()))?;
        std::io::copy(&mut input, &mut output)
            .with_context(|| format!("Failed to copy audio to {}", partial.display()))?;
        output
            .flush()
            .with_context(|| format!("Failed to flush {}", partial.display()))?;
        output
            .sync_all()
            .with_context(|| format!("Failed to finalize {}", partial.display()))?;
        drop(output);
        crate::no_clobber::publish_new_file(&partial, destination).with_context(|| {
            format!(
                "Failed to publish audio copy from {} to {}",
                partial.display(),
                destination.display()
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_connection() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        migrations()
            .to_latest(&mut connection)
            .expect("apply TTS history migrations");
        connection
    }

    fn metadata(source_text: &str, format: TtsOutputFormat) -> NewTtsHistoryEntry {
        NewTtsHistoryEntry {
            group_id: "source-group-1".to_string(),
            source_text: source_text.to_string(),
            source_kind: TtsHistorySourceKind::Markdown,
            provider: TtsProvider::OpenAi,
            model: "gpt-4o-mini-tts".to_string(),
            voice: "alloy".to_string(),
            output_format: format,
            external_output_path: Some(PathBuf::from(r"C:\Audio\external.mp3")),
            prompt_preset_id: Some("calm-narrator".to_string()),
            prompt_preset_name: Some("Calm narrator".to_string()),
            resolved_instructions: Some("Speak calmly.".to_string()),
        }
    }

    #[test]
    fn insert_list_and_get_are_independent_and_newest_first() {
        let connection = test_connection();
        insert_entry(
            &connection,
            100,
            &metadata("first", TtsOutputFormat::Mp3),
            "tts-first.mp3",
            Some(r"C:\Audio\first.mp3"),
        )
        .expect("insert first");
        let second = insert_entry(
            &connection,
            200,
            &metadata("second", TtsOutputFormat::Wav),
            "tts-second.wav",
            None,
        )
        .expect("insert second");

        let entries = list_entries_with_connection(&connection).expect("list entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].group_id, "source-group-1");
        assert_eq!(entries[1].group_id, "source-group-1");
        assert_eq!(entries[0].source_text, "second");
        assert_eq!(entries[1].source_text, "first");
        assert_eq!(
            entries[1].external_output_path.as_deref(),
            Some(r"C:\Audio\first.mp3")
        );
        assert_eq!(
            get_entry_with_connection(&connection, second.id)
                .expect("get entry")
                .expect("entry exists")
                .output_format,
            TtsOutputFormat::Wav
        );
        assert_eq!(
            entries[0].prompt_preset_name.as_deref(),
            Some("Calm narrator")
        );
        assert_eq!(
            entries[0].resolved_instructions.as_deref(),
            Some("Speak calmly.")
        );
        assert_eq!(entries[0].source_kind, TtsHistorySourceKind::Markdown);
    }

    #[test]
    fn unknown_database_enum_values_are_rejected() {
        let connection = test_connection();
        connection
            .execute(
                "INSERT INTO tts_history (
                    timestamp, source_text, provider, model, voice, output_format,
                    managed_audio_filename
                ) VALUES (1, 'text', 'unknown', 'model', 'voice', 'mp3', 'audio.mp3')",
                [],
            )
            .expect("insert malformed row");
        assert!(list_entries_with_connection(&connection).is_err());
    }

    #[test]
    fn managed_filename_rejects_path_traversal() {
        assert!(validate_managed_filename("tts-1.mp3").is_ok());
        assert!(validate_managed_filename("../outside.mp3").is_err());
        assert!(validate_managed_filename(r"folder\outside.mp3").is_err());
        assert!(validate_managed_filename("").is_err());
    }

    #[test]
    fn retention_deletes_oldest_until_count_and_storage_limits_both_fit() {
        let oldest_first_sizes = [10, 20, 30, 40];
        assert_eq!(retention_delete_count(&oldest_first_sizes, 3, u64::MAX), 1);
        assert_eq!(retention_delete_count(&oldest_first_sizes, 10, 65), 2);
        assert_eq!(retention_delete_count(&oldest_first_sizes, 2, 65), 2);
        assert_eq!(retention_delete_count(&oldest_first_sizes, 10, 100), 0);
    }

    #[test]
    fn atomic_copy_publishes_complete_file_and_never_overwrites() {
        let sequence = UNIQUE_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let test_dir = std::env::temp_dir().join(format!(
            "aivorelay-tts-history-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&test_dir).expect("create test directory");
        let source = test_dir.join("source.wav");
        let destination = test_dir.join("export.wav");
        fs::write(&source, b"complete audio").expect("write source");

        atomic_copy_new(&source, &destination, 0).expect("copy audio atomically");
        assert_eq!(
            fs::read(&destination).expect("read destination"),
            b"complete audio"
        );
        assert!(atomic_copy_new(&source, &destination, 0).is_err());
        assert_eq!(
            fs::read(&destination).expect("read unchanged destination"),
            b"complete audio"
        );

        fs::remove_dir_all(&test_dir).expect("remove test directory");
    }
}
