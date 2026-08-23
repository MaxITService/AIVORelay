use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const HISTORY_UPDATED_EVENT: &str = "send-selected-text-history-updated";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SendSelectedTextHistoryStatus {
    Saved,
    CommandStarted,
    Completed,
    CommandFailed,
    Failed,
}

impl SendSelectedTextHistoryStatus {
    fn as_db(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::CommandStarted => "command_started",
            Self::Completed => "completed",
            Self::CommandFailed => "command_failed",
            Self::Failed => "failed",
        }
    }

    fn from_db(value: &str) -> Self {
        match value {
            "saved" => Self::Saved,
            "command_started" => Self::CommandStarted,
            "completed" => Self::Completed,
            "command_failed" => Self::CommandFailed,
            _ => Self::Failed,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct SendSelectedTextHistoryEntry {
    pub id: i64,
    pub operation_id: String,
    pub preset_id: String,
    pub preset_name: String,
    pub timestamp_ms: i64,
    pub selected_text: String,
    pub output_path: Option<String>,
    pub output_format: String,
    pub write_mode: String,
    pub status: SendSelectedTextHistoryStatus,
    pub command: Option<String>,
    pub command_output: Option<String>,
    pub command_output_truncated: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewSendSelectedTextHistoryEntry {
    pub operation_id: String,
    pub preset_id: String,
    pub preset_name: String,
    pub timestamp_ms: i64,
    pub selected_text: String,
    pub output_path: Option<String>,
    pub output_format: String,
    pub write_mode: String,
    pub status: SendSelectedTextHistoryStatus,
    pub command: Option<String>,
    pub command_output: Option<String>,
    pub command_output_truncated: bool,
    pub error: Option<String>,
}

pub struct SendSelectedTextHistoryManager {
    app_handle: AppHandle,
    db_path: PathBuf,
    mutation_lock: Mutex<()>,
}

impl SendSelectedTextHistoryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let db_path =
            crate::portable::app_data_dir(app_handle)?.join("send-selected-text-history.db");
        let manager = Self {
            app_handle: app_handle.clone(),
            db_path,
            mutation_lock: Mutex::new(()),
        };
        manager.initialize()?;
        Ok(manager)
    }

    fn initialize(&self) -> Result<()> {
        let mut connection = self.connection()?;
        let migrations = Migrations::new(vec![
            M::up(
                "CREATE TABLE IF NOT EXISTS send_selected_text_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                operation_id TEXT NOT NULL UNIQUE,
                preset_id TEXT NOT NULL,
                preset_name TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                selected_text TEXT NOT NULL,
                output_path TEXT,
                output_format TEXT NOT NULL,
                write_mode TEXT NOT NULL,
                status TEXT NOT NULL,
                command TEXT,
                command_output TEXT,
                command_output_truncated INTEGER NOT NULL DEFAULT 0,
                error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_send_selected_text_history_timestamp
                ON send_selected_text_history(timestamp_ms DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_send_selected_text_history_preset
                ON send_selected_text_history(preset_id, timestamp_ms DESC, id DESC);",
            ),
            M::up(
                "CREATE TABLE IF NOT EXISTS send_selected_text_preset_state (
                    preset_id TEXT NOT NULL,
                    output_format TEXT NOT NULL,
                    last_output_path TEXT NOT NULL,
                    PRIMARY KEY (preset_id, output_format)
                );
                INSERT OR IGNORE INTO send_selected_text_preset_state (
                    preset_id, output_format, last_output_path
                )
                SELECT history.preset_id, history.output_format, history.output_path
                FROM send_selected_text_history AS history
                WHERE history.output_path IS NOT NULL
                  AND history.status IN ('saved', 'command_started', 'completed', 'command_failed')
                  AND NOT EXISTS (
                      SELECT 1
                      FROM send_selected_text_history AS newer
                      WHERE newer.preset_id = history.preset_id
                        AND newer.output_format = history.output_format
                        AND newer.output_path IS NOT NULL
                        AND newer.status IN ('saved', 'command_started', 'completed', 'command_failed')
                        AND (
                            newer.timestamp_ms > history.timestamp_ms
                            OR (newer.timestamp_ms = history.timestamp_ms AND newer.id > history.id)
                        )
                  );",
            ),
        ]);
        migrations
            .to_latest(&mut connection)
            .context("Failed to initialize Send Selected Text history")?;
        connection.execute(
            "UPDATE send_selected_text_history
             SET status = 'command_failed',
                 error = COALESCE(error, 'AivoRelay closed before the command finished.')
             WHERE status = 'saved'",
            [],
        )?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection> {
        let connection = Connection::open(&self.db_path).with_context(|| {
            format!(
                "Failed to open Send Selected Text history at {}",
                self.db_path.display()
            )
        })?;
        connection.busy_timeout(Duration::from_secs(5))?;
        Ok(connection)
    }

    pub fn insert(
        &self,
        entry: NewSendSelectedTextHistoryEntry,
        history_limit: u32,
    ) -> Result<SendSelectedTextHistoryEntry> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Send Selected Text history lock is unavailable"))?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO send_selected_text_history (
                operation_id, preset_id, preset_name, timestamp_ms, selected_text,
                output_path, output_format, write_mode, status, command,
                command_output, command_output_truncated, error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                entry.operation_id,
                entry.preset_id,
                entry.preset_name,
                entry.timestamp_ms,
                entry.selected_text,
                entry.output_path,
                entry.output_format,
                entry.write_mode,
                entry.status.as_db(),
                entry.command,
                entry.command_output,
                entry.command_output_truncated,
                entry.error,
            ],
        )?;
        let id = transaction.last_insert_rowid();
        if let Some(output_path) = entry.output_path.as_deref() {
            transaction.execute(
                "INSERT INTO send_selected_text_preset_state (
                    preset_id, output_format, last_output_path
                 ) VALUES (?1, ?2, ?3)
                 ON CONFLICT(preset_id, output_format) DO UPDATE SET
                    last_output_path = excluded.last_output_path",
                params![entry.preset_id, entry.output_format, output_path],
            )?;
        }
        self.enforce_limit_with_connection(&transaction, history_limit)?;
        let inserted = self
            .get_with_connection(&transaction, id)?
            .context("New Send Selected Text history entry disappeared")?;
        transaction.commit()?;
        self.notify_updated();
        Ok(inserted)
    }

    pub fn update_command_result(
        &self,
        id: i64,
        status: SendSelectedTextHistoryStatus,
        output: Option<String>,
        output_truncated: bool,
        error: Option<String>,
        history_limit: u32,
    ) -> Result<Option<SendSelectedTextHistoryEntry>> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Send Selected Text history lock is unavailable"))?;
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE send_selected_text_history
             SET status = ?1, command_output = ?2, command_output_truncated = ?3, error = ?4
             WHERE id = ?5",
            params![status.as_db(), output, output_truncated, error, id],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        self.enforce_limit_with_connection(&connection, history_limit)?;
        let updated = self.get_with_connection(&connection, id)?;
        if updated.is_some() {
            self.notify_updated();
        }
        Ok(updated)
    }

    pub fn list(&self, limit: usize, offset: usize) -> Result<Vec<SendSelectedTextHistoryEntry>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, operation_id, preset_id, preset_name, timestamp_ms, selected_text,
                    output_path, output_format, write_mode, status, command, command_output,
                    command_output_truncated, error
             FROM send_selected_text_history
             ORDER BY timestamp_ms DESC, id DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let limit = i64::try_from(limit.min(500)).unwrap_or(500);
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = statement.query_map(params![limit, offset], Self::map_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn delete(&self, id: i64) -> Result<bool> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Send Selected Text history lock is unavailable"))?;
        let deleted = self
            .connection()?
            .execute("DELETE FROM send_selected_text_history WHERE id = ?1", [id])?
            > 0;
        if deleted {
            self.notify_updated();
        }
        Ok(deleted)
    }

    pub fn clear(&self) -> Result<usize> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Send Selected Text history lock is unavailable"))?;
        let deleted = self
            .connection()?
            .execute("DELETE FROM send_selected_text_history", [])?;
        if deleted > 0 {
            self.notify_updated();
        }
        Ok(deleted)
    }

    pub fn last_output_path_for_preset(
        &self,
        preset_id: &str,
        output_format: &str,
    ) -> Result<Option<String>> {
        let connection = self.connection()?;
        let state_path = connection
            .query_row(
                "SELECT last_output_path
                 FROM send_selected_text_preset_state
                 WHERE preset_id = ?1 AND output_format = ?2",
                params![preset_id, output_format],
                |row| row.get(0),
            )
            .optional()?;
        if state_path.is_some() {
            return Ok(state_path);
        }

        connection
            .query_row(
                "SELECT output_path
                 FROM send_selected_text_history
                 WHERE preset_id = ?1
                   AND output_format = ?2
                   AND output_path IS NOT NULL
                   AND status IN ('saved', 'command_started', 'completed', 'command_failed')
                 ORDER BY timestamp_ms DESC, id DESC
                 LIMIT 1",
                params![preset_id, output_format],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn enforce_limit(&self, history_limit: u32) -> Result<()> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Send Selected Text history lock is unavailable"))?;
        let connection = self.connection()?;
        self.enforce_limit_with_connection(&connection, history_limit)?;
        self.notify_updated();
        Ok(())
    }

    fn enforce_limit_with_connection(
        &self,
        connection: &Connection,
        history_limit: u32,
    ) -> Result<()> {
        let keep = history_limit.clamp(1, 5_000);
        connection.execute(
            "DELETE FROM send_selected_text_history
             WHERE status <> 'saved'
               AND id NOT IN (
                 SELECT id FROM send_selected_text_history
                 ORDER BY timestamp_ms DESC, id DESC
                 LIMIT ?1
             )",
            [keep],
        )?;
        Ok(())
    }

    fn get_with_connection(
        &self,
        connection: &Connection,
        id: i64,
    ) -> Result<Option<SendSelectedTextHistoryEntry>> {
        connection
            .query_row(
                "SELECT id, operation_id, preset_id, preset_name, timestamp_ms, selected_text,
                        output_path, output_format, write_mode, status, command, command_output,
                        command_output_truncated, error
                 FROM send_selected_text_history WHERE id = ?1",
                [id],
                Self::map_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SendSelectedTextHistoryEntry> {
        let status: String = row.get(9)?;
        Ok(SendSelectedTextHistoryEntry {
            id: row.get(0)?,
            operation_id: row.get(1)?,
            preset_id: row.get(2)?,
            preset_name: row.get(3)?,
            timestamp_ms: row.get(4)?,
            selected_text: row.get(5)?,
            output_path: row.get(6)?,
            output_format: row.get(7)?,
            write_mode: row.get(8)?,
            status: SendSelectedTextHistoryStatus::from_db(&status),
            command: row.get(10)?,
            command_output: row.get(11)?,
            command_output_truncated: row.get(12)?,
            error: row.get(13)?,
        })
    }

    fn notify_updated(&self) {
        if let Err(error) = self.app_handle.emit(HISTORY_UPDATED_EVENT, ()) {
            log::debug!("Failed to emit Send Selected Text history update: {error}");
        }
    }
}
