use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, Utc};
use log::{debug, error, info};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

use crate::audio_toolkit::save_wav_file;

/// Database migrations for transcription history.
/// Each migration is applied in order. The library tracks which migrations
/// have been applied using SQLite's user_version pragma.
///
/// Note: For users upgrading from tauri-plugin-sql, migrate_from_tauri_plugin_sql()
/// converts the old _sqlx_migrations table tracking to the user_version pragma,
/// ensuring migrations don't re-run on existing databases.
static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS transcription_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_name TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            saved BOOLEAN NOT NULL DEFAULT 0,
            title TEXT NOT NULL,
            transcription_text TEXT NOT NULL
        );",
    ),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_processed_text TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_prompt TEXT;"),
    // Migration 4: Add columns for AI Replace history support
    M::up(
        "ALTER TABLE transcription_history ADD COLUMN action_type TEXT DEFAULT 'transcribe';
         ALTER TABLE transcription_history ADD COLUMN original_selection TEXT;
         ALTER TABLE transcription_history ADD COLUMN ai_response TEXT;",
    ),
    M::up(
        "ALTER TABLE transcription_history ADD COLUMN post_process_requested BOOLEAN NOT NULL DEFAULT 0;",
    ),
];

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PaginatedHistory {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[serde(tag = "action")]
pub enum HistoryUpdatePayload {
    #[serde(rename = "added")]
    Added { entry: HistoryEntry },
    #[serde(rename = "updated")]
    Updated { entry: HistoryEntry },
    #[serde(rename = "deleted")]
    Deleted { id: i64 },
    #[serde(rename = "toggled")]
    Toggled { id: i64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct HistoryEntry {
    pub id: i64,
    pub file_name: String,
    pub timestamp: i64,
    pub saved: bool,
    pub title: String,
    pub transcription_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    pub post_process_requested: bool,
    /// Type of action: "transcribe", "ai_replace", etc.
    pub action_type: String,
    /// For AI Replace: the original selected text that was transformed
    pub original_selection: Option<String>,
    /// For AI Replace: the AI response (None if request failed/never received)
    pub ai_response: Option<String>,
}

pub struct HistoryManager {
    app_handle: AppHandle,
    recordings_dir: PathBuf,
    db_path: PathBuf,
}

impl HistoryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Create recordings directory in app data dir
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let recordings_dir = app_data_dir.join("recordings");
        let db_path = app_data_dir.join("history.db");

        // Ensure recordings directory exists
        if !recordings_dir.exists() {
            fs::create_dir_all(&recordings_dir)?;
            debug!("Created recordings directory: {:?}", recordings_dir);
        }

        let manager = Self {
            app_handle: app_handle.clone(),
            recordings_dir,
            db_path,
        };

        // Initialize database and run migrations synchronously
        manager.init_database()?;

        Ok(manager)
    }

    fn init_database(&self) -> Result<()> {
        info!("Initializing database at {:?}", self.db_path);

        let mut conn = Connection::open(&self.db_path)?;

        // Handle migration from tauri-plugin-sql to rusqlite_migration
        // tauri-plugin-sql used _sqlx_migrations table, rusqlite_migration uses user_version pragma
        self.migrate_from_tauri_plugin_sql(&conn)?;

        // Create migrations object and run to latest version
        let migrations = Migrations::new(MIGRATIONS.to_vec());

        // Validate migrations in debug builds
        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid migrations");

        // Get current version before migration
        let version_before: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        debug!("Database version before migration: {}", version_before);

        // Apply any pending migrations
        migrations.to_latest(&mut conn)?;

        // Get version after migration
        let version_after: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if version_after > version_before {
            info!(
                "Database migrated from version {} to {}",
                version_before, version_after
            );
        } else {
            debug!("Database already at latest version {}", version_after);
        }

        Ok(())
    }

    /// Migrate from tauri-plugin-sql's migration tracking to rusqlite_migration's.
    /// tauri-plugin-sql used a _sqlx_migrations table, while rusqlite_migration uses
    /// SQLite's user_version pragma. This function checks if the old system was in use
    /// and sets the user_version accordingly so migrations don't re-run.
    fn migrate_from_tauri_plugin_sql(&self, conn: &Connection) -> Result<()> {
        // Check if the old _sqlx_migrations table exists
        let has_sqlx_migrations: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_sqlx_migrations {
            return Ok(());
        }

        // Check current user_version
        let current_version: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if current_version > 0 {
            // Already migrated to rusqlite_migration system
            return Ok(());
        }

        // Get the highest version from the old migrations table
        let old_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if old_version > 0 {
            info!(
                "Migrating from tauri-plugin-sql (version {}) to rusqlite_migration",
                old_version
            );

            // Set user_version to match the old migration state
            conn.pragma_update(None, "user_version", old_version)?;

            // Optionally drop the old migrations table (keeping it doesn't hurt)
            // conn.execute("DROP TABLE IF EXISTS _sqlx_migrations", [])?;

            info!(
                "Migration tracking converted: user_version set to {}",
                old_version
            );
        }

        Ok(())
    }

    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    pub fn recordings_dir(&self) -> &std::path::Path {
        &self.recordings_dir
    }

    /// Save a transcription to history (both database and WAV file)
    pub async fn save_transcription(
        &self,
        audio_samples: Vec<f32>,
        transcription_text: String,
        post_process_requested: bool,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
    ) -> Result<()> {
        let file_name = format!("aivorelay-{}.wav", chrono::Utc::now().timestamp_millis());

        // Save WAV file
        let file_path = self.recordings_dir.join(&file_name);
        save_wav_file(file_path, &audio_samples)?;

        self.save_entry(
            file_name,
            transcription_text,
            post_process_requested,
            post_processed_text,
            post_process_prompt,
        )?;

        Ok(())
    }

    pub fn save_entry(
        &self,
        file_name: String,
        transcription_text: String,
        post_process_requested: bool,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
    ) -> Result<HistoryEntry> {
        let timestamp = Utc::now().timestamp();
        let title = self.format_timestamp_title(timestamp);
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                action_type
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                file_name,
                timestamp,
                false,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                "transcribe"
            ],
        )?;

        let entry = HistoryEntry {
            id: conn.last_insert_rowid(),
            file_name,
            timestamp,
            saved: false,
            title,
            transcription_text,
            post_processed_text,
            post_process_prompt,
            post_process_requested,
            action_type: "transcribe".to_string(),
            original_selection: None,
            ai_response: None,
        };

        debug!("Saved transcription to database");
        self.cleanup_old_entries()?;
        self.emit_history_added(entry.clone());
        Ok(entry)
    }

    pub fn update_transcription(
        &self,
        id: i64,
        transcription_text: String,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
    ) -> Result<HistoryEntry> {
        let conn = self.get_connection()?;
        let updated = conn.execute(
            "UPDATE transcription_history
             SET transcription_text = ?1,
                 post_processed_text = ?2,
                 post_process_prompt = ?3
             WHERE id = ?4",
            params![
                transcription_text,
                post_processed_text,
                post_process_prompt,
                id
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("History entry {} not found", id));
        }

        let entry = conn
            .query_row(
                "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, action_type, original_selection, ai_response
                 FROM transcription_history
                 WHERE id = ?1",
                params![id],
                Self::map_history_entry,
            )?;

        if let Err(e) = self.app_handle.emit(
            "history-update-payload",
            &HistoryUpdatePayload::Updated {
                entry: entry.clone(),
            },
        ) {
            error!("Failed to emit history-update-payload event: {}", e);
        }

        if let Err(e) = self.app_handle.emit("history-updated", ()) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    pub fn cleanup_old_entries(&self) -> Result<()> {
        let retention_period = crate::settings::get_recording_retention_period(&self.app_handle);

        match retention_period {
            crate::settings::RecordingRetentionPeriod::Never => {
                // Don't delete anything
                return Ok(());
            }
            crate::settings::RecordingRetentionPeriod::PreserveLimit => {
                // Use the old count-based logic with history_limit
                let limit = crate::settings::get_history_limit(&self.app_handle);
                return self.cleanup_by_count(limit);
            }
            _ => {
                // Use time-based logic
                return self.cleanup_by_time(retention_period);
            }
        }
    }

    fn delete_entries_and_files(&self, entries: &[(i64, String)]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let conn = self.get_connection()?;
        let mut deleted_count = 0;

        for (id, file_name) in entries {
            // Delete database entry
            conn.execute(
                "DELETE FROM transcription_history WHERE id = ?1",
                params![id],
            )?;
            self.emit_history_deleted(*id);

            // Delete WAV file
            let file_path = self.recordings_dir.join(file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete WAV file {}: {}", file_name, e);
                } else {
                    debug!("Deleted old WAV file: {}", file_name);
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }

    fn cleanup_by_count(&self, limit: usize) -> Result<()> {
        let conn = self.get_connection()?;

        // Get all entries that are not saved, ordered by timestamp desc
        let mut stmt = conn.prepare(
            "SELECT id, file_name FROM transcription_history WHERE saved = 0 ORDER BY timestamp DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
        })?;

        let mut entries: Vec<(i64, String)> = Vec::new();
        for row in rows {
            entries.push(row?);
        }

        if entries.len() > limit {
            let entries_to_delete = &entries[limit..];
            let deleted_count = self.delete_entries_and_files(entries_to_delete)?;

            if deleted_count > 0 {
                debug!("Cleaned up {} old history entries by count", deleted_count);
            }
        }

        Ok(())
    }

    fn cleanup_by_time(
        &self,
        retention_period: crate::settings::RecordingRetentionPeriod,
    ) -> Result<()> {
        let conn = self.get_connection()?;

        // Calculate cutoff timestamp (current time minus retention period)
        let now = Utc::now().timestamp();
        let cutoff_timestamp = match retention_period {
            crate::settings::RecordingRetentionPeriod::Days3 => now - (3 * 24 * 60 * 60), // 3 days in seconds
            crate::settings::RecordingRetentionPeriod::Weeks2 => now - (2 * 7 * 24 * 60 * 60), // 2 weeks in seconds
            crate::settings::RecordingRetentionPeriod::Months3 => now - (3 * 30 * 24 * 60 * 60), // 3 months in seconds (approximate)
            _ => unreachable!("Should not reach here"),
        };

        // Get all unsaved entries older than the cutoff timestamp
        let mut stmt = conn.prepare(
            "SELECT id, file_name FROM transcription_history WHERE saved = 0 AND timestamp < ?1",
        )?;

        let rows = stmt.query_map(params![cutoff_timestamp], |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
        })?;

        let mut entries_to_delete: Vec<(i64, String)> = Vec::new();
        for row in rows {
            entries_to_delete.push(row?);
        }

        let deleted_count = self.delete_entries_and_files(&entries_to_delete)?;

        if deleted_count > 0 {
            debug!(
                "Cleaned up {} old history entries based on retention period",
                deleted_count
            );
        }

        Ok(())
    }

    fn map_history_entry(row: &rusqlite::Row) -> rusqlite::Result<HistoryEntry> {
        Ok(HistoryEntry {
            id: row.get("id")?,
            file_name: row.get("file_name")?,
            timestamp: row.get("timestamp")?,
            saved: row.get("saved")?,
            title: row.get("title")?,
            transcription_text: row.get("transcription_text")?,
            post_processed_text: row.get("post_processed_text")?,
            post_process_prompt: row.get("post_process_prompt")?,
            post_process_requested: row
                .get::<_, Option<bool>>("post_process_requested")?
                .unwrap_or(false),
            action_type: row
                .get::<_, Option<String>>("action_type")?
                .unwrap_or_else(|| "transcribe".to_string()),
            original_selection: row.get("original_selection")?,
            ai_response: row.get("ai_response")?,
        })
    }

    fn emit_history_added(&self, entry: HistoryEntry) {
        if let Err(e) = self.app_handle.emit(
            "history-update-payload",
            &HistoryUpdatePayload::Added {
                entry: entry.clone(),
            },
        ) {
            error!("Failed to emit history-update-payload event: {}", e);
        }

        if let Err(e) = self.app_handle.emit("history-updated", ()) {
            error!("Failed to emit history-updated event: {}", e);
        }
    }

    fn emit_history_deleted(&self, id: i64) {
        if let Err(e) = self.app_handle.emit(
            "history-update-payload",
            &HistoryUpdatePayload::Deleted { id },
        ) {
            error!("Failed to emit history-update-payload event: {}", e);
        }

        if let Err(e) = self.app_handle.emit("history-updated", ()) {
            error!("Failed to emit history-updated event: {}", e);
        }
    }

    fn emit_history_toggled(&self, id: i64) {
        if let Err(e) = self.app_handle.emit(
            "history-update-payload",
            &HistoryUpdatePayload::Toggled { id },
        ) {
            error!("Failed to emit history-update-payload event: {}", e);
        }

        if let Err(e) = self.app_handle.emit("history-updated", ()) {
            error!("Failed to emit history-updated event: {}", e);
        }
    }

    pub async fn get_history_entries(
        &self,
        cursor: Option<i64>,
        limit: Option<usize>,
    ) -> Result<PaginatedHistory> {
        let conn = self.get_connection()?;
        let limit = limit.map(|value| value.min(100));

        let mut entries = match (cursor, limit) {
            (Some(cursor_id), Some(page_size)) => {
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, action_type, original_selection, ai_response
                     FROM transcription_history
                     WHERE id < ?1
                     ORDER BY id DESC
                     LIMIT ?2",
                )?;
                let rows = stmt.query_map(
                    params![cursor_id, (page_size + 1) as i64],
                    Self::map_history_entry,
                )?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            }
            (None, Some(page_size)) => {
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, action_type, original_selection, ai_response
                     FROM transcription_history
                     ORDER BY id DESC
                     LIMIT ?1",
                )?;
                let rows =
                    stmt.query_map(params![(page_size + 1) as i64], Self::map_history_entry)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            }
            (_, None) => {
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, action_type, original_selection, ai_response
                     FROM transcription_history
                     ORDER BY id DESC",
                )?;
                let rows = stmt.query_map([], Self::map_history_entry)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            }
        };

        let has_more = limit.is_some_and(|page_size| entries.len() > page_size);
        if has_more {
            entries.pop();
        }

        Ok(PaginatedHistory { entries, has_more })
    }

    pub fn get_latest_entry(&self) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        Self::get_latest_entry_with_conn(&conn)
    }

    fn get_latest_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, action_type, original_selection, ai_response
             FROM transcription_history
             ORDER BY timestamp DESC
             LIMIT 1",
        )?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;

        Ok(entry)
    }

    pub fn get_latest_completed_entry(&self) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        Self::get_latest_completed_entry_with_conn(&conn)
    }

    fn get_latest_completed_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, action_type, original_selection, ai_response
             FROM transcription_history
             WHERE action_type = 'transcribe' AND transcription_text != ''
             ORDER BY timestamp DESC
             LIMIT 1",
        )?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;

        Ok(entry)
    }

    pub async fn toggle_saved_status(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        // Get current saved status
        let current_saved: bool = conn.query_row(
            "SELECT saved FROM transcription_history WHERE id = ?1",
            params![id],
            |row| row.get("saved"),
        )?;

        let new_saved = !current_saved;

        conn.execute(
            "UPDATE transcription_history SET saved = ?1 WHERE id = ?2",
            params![new_saved, id],
        )?;

        debug!("Toggled saved status for entry {}: {}", id, new_saved);

        self.emit_history_toggled(id);

        Ok(())
    }

    pub fn get_audio_file_path(&self, file_name: &str) -> PathBuf {
        self.recordings_dir.join(file_name)
    }

    pub async fn get_entry_by_id(&self, id: i64) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, action_type, original_selection, ai_response
             FROM transcription_history WHERE id = ?1",
        )?;

        let entry = stmt.query_row([id], Self::map_history_entry).optional()?;

        Ok(entry)
    }

    pub async fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        // Get the entry to find the file name
        if let Some(entry) = self.get_entry_by_id(id).await? {
            // Delete the audio file first
            let file_path = self.get_audio_file_path(&entry.file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete audio file {}: {}", entry.file_name, e);
                    // Continue with database deletion even if file deletion fails
                }
            }
        }

        // Delete from database
        conn.execute(
            "DELETE FROM transcription_history WHERE id = ?1",
            params![id],
        )?;

        debug!("Deleted history entry with id: {}", id);

        self.emit_history_deleted(id);

        Ok(())
    }

    /// Save an AI Replace operation to history (no audio file, just the text data)
    pub async fn save_ai_replace_entry(
        &self,
        instruction: String,
        original_selection: String,
        ai_response: Option<String>,
    ) -> Result<()> {
        let timestamp = Utc::now().timestamp();
        let file_name = format!("ai-replace-{}.txt", timestamp); // Virtual file, not actually created
        let title = self.format_timestamp_title(timestamp);

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO transcription_history (file_name, timestamp, saved, title, transcription_text, action_type, original_selection, ai_response) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![file_name, timestamp, false, title, instruction, "ai_replace", original_selection, ai_response],
        )?;

        debug!("Saved AI Replace entry to database");
        self.cleanup_old_entries()?;
        self.emit_history_added(HistoryEntry {
            id: conn.last_insert_rowid(),
            file_name,
            timestamp,
            saved: false,
            title,
            transcription_text: instruction,
            post_processed_text: None,
            post_process_prompt: None,
            post_process_requested: false,
            action_type: "ai_replace".to_string(),
            original_selection: Some(original_selection),
            ai_response,
        });

        Ok(())
    }

    fn format_timestamp_title(&self, timestamp: i64) -> String {
        if let Some(utc_datetime) = DateTime::from_timestamp(timestamp, 0) {
            // Convert UTC to local timezone
            let local_datetime = utc_datetime.with_timezone(&Local);
            local_datetime.format("%B %e, %Y - %l:%M%p").to_string()
        } else {
            format!("Recording {}", timestamp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE transcription_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                saved BOOLEAN NOT NULL DEFAULT 0,
                title TEXT NOT NULL,
                transcription_text TEXT NOT NULL,
                post_processed_text TEXT,
                post_process_prompt TEXT,
                post_process_requested BOOLEAN NOT NULL DEFAULT 0,
                action_type TEXT DEFAULT 'transcribe',
                original_selection TEXT,
                ai_response TEXT
            );",
        )
        .expect("create transcription_history table");
        conn
    }

    fn insert_entry(conn: &Connection, timestamp: i64, text: &str, post_processed: Option<&str>) {
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                action_type,
                original_selection,
                ai_response
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                format!("aivorelay-{}.wav", timestamp),
                timestamp,
                false,
                format!("Recording {}", timestamp),
                text,
                post_processed,
                Option::<String>::None,
                false,
                "transcribe",
                Option::<String>::None,
                Option::<String>::None
            ],
        )
        .expect("insert history entry");
    }

    fn insert_ai_replace_entry(
        conn: &Connection,
        timestamp: i64,
        instruction: &str,
        original_selection: &str,
        ai_response: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                action_type,
                original_selection,
                ai_response
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                format!("ai-replace-{}.txt", timestamp),
                timestamp,
                false,
                format!("Recording {}", timestamp),
                instruction,
                Option::<String>::None,
                Option::<String>::None,
                false,
                "ai_replace",
                original_selection,
                ai_response
            ],
        )
        .expect("insert ai_replace entry");
    }

    #[test]
    fn get_latest_entry_returns_none_when_empty() {
        let conn = setup_conn();
        let entry = HistoryManager::get_latest_entry_with_conn(&conn).expect("fetch latest entry");
        assert!(entry.is_none());
    }

    #[test]
    fn get_latest_entry_returns_newest_entry() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first", None);
        insert_entry(&conn, 200, "second", Some("processed"));

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(entry.timestamp, 200);
        assert_eq!(entry.transcription_text, "second");
        assert_eq!(entry.post_processed_text.as_deref(), Some("processed"));
    }

    #[test]
    fn get_latest_completed_entry_skips_empty_entries() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "completed", None);
        insert_entry(&conn, 200, "", None);

        let entry = HistoryManager::get_latest_completed_entry_with_conn(&conn)
            .expect("fetch latest completed entry")
            .expect("completed entry exists");

        assert_eq!(entry.timestamp, 100);
        assert_eq!(entry.transcription_text, "completed");
    }

    #[test]
    fn get_latest_completed_entry_returns_none_when_only_empty_transcriptions_exist() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "", None);
        insert_entry(&conn, 200, "", Some("processed"));

        let entry =
            HistoryManager::get_latest_completed_entry_with_conn(&conn).expect("fetch latest");

        assert!(entry.is_none());
    }

    #[test]
    fn get_latest_completed_entry_ignores_newer_ai_replace_rows() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "spoken text", None);
        insert_ai_replace_entry(&conn, 200, "rewrite this", "original", Some("rewritten"));

        let entry = HistoryManager::get_latest_completed_entry_with_conn(&conn)
            .expect("fetch latest completed entry")
            .expect("completed entry exists");

        assert_eq!(entry.timestamp, 100);
        assert_eq!(entry.action_type, "transcribe");
    }

    #[test]
    fn get_latest_entry_returns_newest_ai_replace_row_with_payload_fields() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "spoken text", None);
        insert_ai_replace_entry(&conn, 300, "rewrite this", "selected", Some("rewritten"));

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(entry.timestamp, 300);
        assert_eq!(entry.action_type, "ai_replace");
        assert_eq!(entry.original_selection.as_deref(), Some("selected"));
        assert_eq!(entry.ai_response.as_deref(), Some("rewritten"));
    }

    #[test]
    fn get_latest_completed_entry_prefers_newest_non_empty_transcribe_row() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first", None);
        insert_entry(&conn, 150, "", None);
        insert_entry(&conn, 200, "second", Some("processed"));

        let entry = HistoryManager::get_latest_completed_entry_with_conn(&conn)
            .expect("fetch latest completed entry")
            .expect("completed entry exists");

        assert_eq!(entry.timestamp, 200);
        assert_eq!(entry.transcription_text, "second");
        assert_eq!(entry.post_processed_text.as_deref(), Some("processed"));
    }

    #[test]
    fn get_latest_entry_returns_none_when_only_table_exists_without_rows() {
        let conn = setup_conn();

        let entry = HistoryManager::get_latest_entry_with_conn(&conn).expect("fetch latest entry");

        assert!(entry.is_none());
    }
}
