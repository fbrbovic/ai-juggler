//! Session repository.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::de::Error as _;

use crate::error::CoreError;
use crate::model::{
    MachineId, ProfileId, SessionId, SessionRecord, SessionStatus, Timestamp, TranscriptRef,
};

/// Converts a stored-JSON parse failure into a row-mapping failure.
fn json_err(e: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
}

/// Typed read/write access to the `sessions` table.
#[derive(Debug)]
pub struct SessionsRepo<'a> {
    conn: &'a Connection,
}

impl SessionsRepo<'_> {
    /// Wraps an open connection.
    #[must_use]
    pub fn new(conn: &Connection) -> SessionsRepo<'_> {
        SessionsRepo { conn }
    }

    /// Inserts a new session record.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on I/O or constraint violations
    /// (unknown profile, duplicate id).
    pub fn insert(&self, session: &SessionRecord) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO sessions
                 (id, native_id, profile_id, machine, cwd, status,
                  started_at, last_activity, usage_json, transcript_ref_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                session.id.as_str(),
                session.native_id,
                session.profile.as_str(),
                session.machine.as_str(),
                session.cwd.to_string_lossy(),
                session.status.as_str(),
                session.started_at.millis(),
                session.last_activity.map(Timestamp::millis),
                serde_json::to_string(&session.usage)?,
                session
                    .transcript_ref
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
            ],
        )?;
        Ok(())
    }

    /// Updates an existing session record by id.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::NotFound`] when the id does not exist,
    /// [`CoreError::Storage`] on SQLite failures, or
    /// [`CoreError::Serialization`] for values that cannot be stored.
    pub fn update(&self, session: &SessionRecord) -> Result<(), CoreError> {
        let changed = self.conn.execute(
            "UPDATE sessions SET
                 native_id = ?2, status = ?3, last_activity = ?4,
                 usage_json = ?5, transcript_ref_json = ?6
             WHERE id = ?1",
            params![
                session.id.as_str(),
                session.native_id,
                session.status.as_str(),
                session.last_activity.map(Timestamp::millis),
                serde_json::to_string(&session.usage)?,
                session
                    .transcript_ref
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
            ],
        )?;
        super::accounts::require_changed(changed, "session", session.id.as_str())
    }

    /// Fetches one session by id.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::NotFound`] when missing, [`CoreError::Storage`]
    /// on SQLite failures, or [`CoreError::Serialization`] when stored state
    /// cannot be interpreted (degrades as an error, never a panic).
    pub fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.conn
            .query_row(
                "SELECT * FROM sessions WHERE id = ?1",
                params![id.as_str()],
                map_session,
            )
            .optional()?
            .ok_or_else(|| super::accounts::not_found("session", id.as_str()))
    }

    /// Lists sessions, newest first, optionally filtered by profile.
    ///
    /// # Errors
    ///
    /// Same as [`SessionsRepo::get`].
    pub fn list(&self, profile: Option<&ProfileId>) -> Result<Vec<SessionRecord>, CoreError> {
        let mut out = Vec::new();
        if let Some(profile) = profile {
            let mut stmt = self.conn.prepare(
                "SELECT * FROM sessions WHERE profile_id = ?1 ORDER BY started_at DESC, id",
            )?;
            let rows = stmt.query_map(params![profile.as_str()], map_session)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = self
                .conn
                .prepare("SELECT * FROM sessions ORDER BY started_at DESC, id")?;
            let rows = stmt.query_map([], map_session)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }
}

/// Maps a row to a [`SessionRecord`].
fn map_session(row: &Row<'_>) -> rusqlite::Result<SessionRecord> {
    let status_raw: String = row.get("status")?;
    let usage_json: String = row.get("usage_json")?;
    let transcript_json: Option<String> = row.get("transcript_ref_json")?;
    Ok(SessionRecord {
        id: SessionId::new(row.get::<_, String>("id")?),
        native_id: row.get("native_id")?,
        profile: ProfileId::new(row.get::<_, String>("profile_id")?),
        machine: MachineId::new(row.get::<_, String>("machine")?),
        cwd: PathBuf::from(row.get::<_, String>("cwd")?),
        // Unknown status values (e.g. written by a future build that somehow
        // shares this database) degrade to an error, never a panic.
        status: SessionStatus::parse(&status_raw).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(serde_json::Error::custom(format!(
                    "unknown session status: {status_raw}"
                ))),
            )
        })?,
        started_at: crate::model::Timestamp::from_millis(row.get("started_at")?),
        last_activity: row
            .get::<_, Option<i64>>("last_activity")?
            .map(crate::model::Timestamp::from_millis),
        usage: serde_json::from_str(&usage_json).map_err(json_err)?,
        transcript_ref: transcript_json
            .as_deref()
            .map(serde_json::from_str::<TranscriptRef>)
            .transpose()
            .map_err(json_err)?,
    })
}
