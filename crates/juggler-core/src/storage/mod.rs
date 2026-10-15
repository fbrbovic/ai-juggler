//! SQLite persistence: open/migrate plus typed repositories.
//!
//! Policy (`docs/plan/QUALITY.md` §1, §7):
//! - WAL journal, `NORMAL` sync, foreign keys on.
//! - Forward-only migrations: each entry is one schema version; databases
//!   written by *newer* builds are rejected with
//!   [`CoreError::SchemaTooNew`] instead of being risked.
//! - Corrupt or foreign files degrade to [`CoreError::NotADatabase`].

mod accounts;
mod profiles;
mod sessions;

mod cleanup_audit;
mod cleanup_settings;
mod process_ledger;
mod projects;
mod settings;

use std::path::Path;

use rusqlite::Connection;

pub use accounts::AccountsRepo;
pub use cleanup_audit::{
    ACTOR_AUTO_TIER1, ACTOR_AUTO_TIER2, ACTOR_PREFIX_USER, AuditOutcomeInfo, CleanupAuditRecord,
    CleanupAuditRepo, OUTCOME_PENDING,
};
pub use cleanup_settings::{CleanupSettingsRepo, CleanupTierSettings};
pub use process_ledger::{
    LedgerRow, ProcessLedgerRepo, ROLE_HELPER, ROLE_LOGIN, ROLE_ROOT, STATE_EXITED, STATE_ORPHANED,
    STATE_RUNNING, STATE_STARTING, STATE_UNVERIFIABLE,
};
pub use profiles::ProfilesRepo;
pub use projects::ProjectsRepo;
pub use sessions::SessionsRepo;
pub use settings::AppSettingsRepo;

use crate::error::CoreError;

/// Schema version 1: accounts, profiles, sessions.
const V1: &str = "
CREATE TABLE accounts (
    id         TEXT PRIMARY KEY,
    harness    TEXT NOT NULL,
    label      TEXT NOT NULL,
    plan       TEXT,
    state_dir  TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE profiles (
    id            TEXT PRIMARY KEY,
    account_id    TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    settings_json TEXT NOT NULL,
    default_cwd   TEXT,
    created_at    INTEGER NOT NULL
);

CREATE TABLE sessions (
    id                  TEXT PRIMARY KEY,
    native_id           TEXT,
    profile_id          TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    machine             TEXT NOT NULL,
    cwd                 TEXT NOT NULL,
    status              TEXT NOT NULL,
    started_at          INTEGER NOT NULL,
    last_activity       INTEGER,
    usage_json          TEXT NOT NULL,
    transcript_ref_json TEXT
);
";

/// Schema version 2: profile names.
const V2: &str = "ALTER TABLE profiles ADD COLUMN name TEXT NOT NULL DEFAULT '';";

/// Schema version 3 (ADR-0007): instance kinds + tracked projects, the
/// juggling anchor.
const V3: &str = "
ALTER TABLE sessions ADD COLUMN kind TEXT NOT NULL DEFAULT 'terminal';
ALTER TABLE sessions ADD COLUMN project TEXT;
CREATE TABLE projects (
    id            TEXT PRIMARY KEY,
    root          TEXT NOT NULL UNIQUE,
    name          TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    last_activity INTEGER
);
";

/// Ordered, forward-only migrations; index + 1 is the schema version.
/// Schema version 4: imported-history enrichment on projects (which
/// accounts worked there before AI Juggler, from imported transcripts).
const V4: &str = "ALTER TABLE projects ADD COLUMN history_json TEXT NOT NULL DEFAULT '{}';";

/// Schema version 5 (ADR-0008 renames; AJ-E106): pure `ALTER … RENAME`
/// — identities change, values do not. Forward-only; a v5 DB opened by a
/// v4 build fails with `SchemaTooNew` (never silent corruption).
const V5: &str = "
ALTER TABLE accounts RENAME COLUMN harness TO provider_id;
ALTER TABLE sessions RENAME TO instances;
ALTER TABLE instances RENAME COLUMN native_id TO conversation_id;
ALTER TABLE instances RENAME COLUMN profile_id TO launch_recipe_id;
ALTER TABLE profiles RENAME TO launch_recipes;
";

/// Schema version 6 (AJ-E405): the structured attention reason rides on
/// the Instance row — additive, nullable, default unchanged.
const V6: &str = "ALTER TABLE instances ADD COLUMN attention_json TEXT;";

/// Schema version 7 (AJ-E410, D-08): the per-Account Tier-1 cleanup
/// opt-in — additive column, default `0` = notify-only (the default tier).
const V7: &str = "ALTER TABLE accounts ADD COLUMN cleanup_opt_in INTEGER NOT NULL DEFAULT 0;";

/// Schema version 8 (AJ-E411, D-12): the optional user-entered renewal
/// date — additive nullable column; `NULL` = `Expiry: unknown`.
const V8: &str = "ALTER TABLE accounts ADD COLUMN renewal_date TEXT;";

/// Schema version 9 (AJ-E412): the transcript-observed running model —
/// additive nullable JSON blob (`{model, source}`); `NULL` = not
/// observed (never the launch overlay).
const V9: &str = "ALTER TABLE instances ADD COLUMN running_model_json TEXT;";

/// Schema version 10 (AJ-E414): the desktop window identity — additive
/// nullable JSON blob (`{pid, bundle_id, title, cgwindow_id}`); `NULL`
/// for terminal Instances — plus the app-level settings kv (hotkey
/// bindings and future user preferences; values are strings, namespaced
/// by feature).
const V10: &str = "
ALTER TABLE instances ADD COLUMN window_identity_json TEXT;
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Schema version 11 (AJ-E415): the per-Account live-preview opt-in —
/// additive column, default `0` = off (Screen Recording is consented
/// only by an explicit enable).
const V11: &str = "ALTER TABLE accounts ADD COLUMN preview_opt_in INTEGER NOT NULL DEFAULT 0;";

/// Schema version 12 (AJ-E501, pass2 §5): the persistent process
/// ledger — one row per Intent (before spawn), patched with PID/PGID/
/// OS start time at spawn, helpers upserted per reconciler tick. It
/// survives root exit, app exit, and reboot; exited rows retain 7 days.
const V12: &str = "
CREATE TABLE process_ledger (
    pid INTEGER,
    pgid INTEGER,
    started_at INTEGER,
    exe_path TEXT NOT NULL,
    cmdline_hash TEXT NOT NULL,
    cmdline_trunc TEXT NOT NULL,
    boot_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    instance_id INTEGER,
    provider TEXT NOT NULL,
    account_id TEXT,
    project_id TEXT,
    machine_id TEXT NOT NULL,
    root_pid INTEGER,
    parent_pid INTEGER,
    depth INTEGER NOT NULL DEFAULT 0,
    role TEXT NOT NULL,
    state TEXT NOT NULL,
    listens_on TEXT,
    cwd TEXT NOT NULL,
    peak_rss INTEGER,
    peak_cpu REAL,
    exit_kind TEXT,
    reaped_by_us INTEGER,
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL
);
CREATE INDEX process_ledger_session_state ON process_ledger(session_id, state);
CREATE INDEX process_ledger_pid_start ON process_ledger(pid, started_at, boot_id);
CREATE INDEX process_ledger_state_last_seen ON process_ledger(state, last_seen);
";

/// v13 (AJ-E502): escaped-group flag — a helper whose live PGID left
/// the root's group at reconcile time. Sticky once set: containment
/// is void for that row, forever (pass2 §4 F-9).
const V13: &str = "
ALTER TABLE process_ledger ADD COLUMN escaped_group INTEGER NOT NULL DEFAULT 0;
";

/// v14 (AJ-E509): the append-only cleanup audit — intent BEFORE the
/// first signal, completed with outcomes after the ladder. Every
/// kill (manual or automatic) is accountable for 7 days; cmdlines
/// persist HASHED + truncated only (the privacy boundary — full
/// command lines never outlive the confirm sheet).
const V14: &str = "
CREATE TABLE cleanup_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at_ms INTEGER NOT NULL,
    actor TEXT NOT NULL,
    pid INTEGER NOT NULL,
    pgid INTEGER,
    exe_path TEXT NOT NULL,
    cmdline_hash TEXT NOT NULL,
    cmdline_trunc TEXT NOT NULL,
    session_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    account_id TEXT,
    project_id TEXT,
    machine_id TEXT NOT NULL,
    identity TEXT NOT NULL,
    scope TEXT NOT NULL,
    outcome TEXT NOT NULL,
    ports_before TEXT,
    ports_after TEXT
);
CREATE INDEX cleanup_audit_at ON cleanup_audit(at_ms);
CREATE INDEX cleanup_audit_account ON cleanup_audit(account_id, at_ms);
";

/// v15 (AJ-E508): per-Account cleanup tier settings — TWO separate
/// opt-ins (D-08 / ADR-0010's E508 amendment), both off by default.
/// The E410 prototype's single `accounts.cleanup_opt_in` seeds Tier 1
/// only (its promise was tidy-after-exit); Tier 2 starts off for
/// everyone, including previous opt-ins.
const V15: &str = "
CREATE TABLE IF NOT EXISTS cleanup_settings (
    account_id TEXT PRIMARY KEY,
    tier1_opt_in INTEGER NOT NULL DEFAULT 0,
    tier2_opt_in INTEGER NOT NULL DEFAULT 0
);
INSERT INTO cleanup_settings (account_id, tier1_opt_in, tier2_opt_in)
    SELECT id, cleanup_opt_in, 0 FROM accounts WHERE cleanup_opt_in = 1;
";

/// v16 (AJ-E510): durable down-period marker — a helper orphaned
/// because its root died WHILE Juggler was closed, written only by
/// the recovery pass. Drives the distinct cleanup label ("left
/// behind while Juggler was closed") versus ordinary grace-based
/// orphans, and survives any number of further restarts.
const V16: &str = "ALTER TABLE process_ledger ADD COLUMN down_period INTEGER NOT NULL DEFAULT 0;";

const MIGRATIONS: &[&str] = &[
    V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14, V15, V16,
];

/// An opened AI Juggler database with typed repositories.
#[derive(Debug)]
pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Opens (creating if needed) and migrates the database at `path`.
    ///
    /// # Errors
    ///
    /// - [`CoreError::NotADatabase`] if the file exists but is not a valid
    ///   database.
    /// - [`CoreError::SchemaTooNew`] if the database needs a newer build.
    /// - [`CoreError::Storage`] for other SQLite failures.
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        let conn = Connection::open(path).map_err(|e| CoreError::NotADatabase {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::initialize(conn, Some(path.to_path_buf()))
    }

    /// Opens an in-memory database (tests and ephemeral indexes).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on SQLite failures.
    pub fn open_in_memory() -> Result<Self, CoreError> {
        let conn = Connection::open_in_memory()?;
        Self::initialize(conn, None)
    }

    /// Applies connection pragmas and runs migrations.
    fn initialize(conn: Connection, path: Option<std::path::PathBuf>) -> Result<Self, CoreError> {
        // Everything in this phase touches the file for the first time, so
        // any failure here means "not a database" for on-disk paths.
        let current: i64 = conn
            .pragma_update(None, "journal_mode", "WAL")
            .and_then(|()| conn.pragma_update(None, "synchronous", "NORMAL"))
            .and_then(|()| conn.pragma_update(None, "foreign_keys", "ON"))
            .and_then(|()| conn.query_row("PRAGMA user_version", [], |row| row.get(0)))
            .map_err(|e| match path {
                Some(p) => CoreError::NotADatabase { path: p, source: e },
                None => CoreError::Storage(e),
            })?;
        let storage = Self { conn };
        storage.migrate(current)?;
        Ok(storage)
    }

    /// Runs pending forward migrations from the current schema version.
    fn migrate(&self, current: i64) -> Result<(), CoreError> {
        let supported = i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX);
        if current > supported {
            return Err(CoreError::SchemaTooNew {
                found: current,
                supported,
            });
        }
        // A negative user_version means a tampered database; treating it as 0
        // re-runs migrations and fails cleanly on the existing tables.
        let applied = usize::try_from(current).unwrap_or(0);
        for (idx, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
            let version = i64::try_from(idx).unwrap_or(0) + 1;
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Repository for [`crate::model::Account`] rows.
    #[must_use]
    pub fn accounts(&self) -> AccountsRepo<'_> {
        AccountsRepo::new(&self.conn)
    }

    /// Repository for [`crate::model::Profile`] rows.
    #[must_use]
    pub fn profiles(&self) -> ProfilesRepo<'_> {
        ProfilesRepo::new(&self.conn)
    }

    /// Repository for [`crate::model::SessionRecord`] rows.
    #[must_use]
    pub fn sessions(&self) -> SessionsRepo<'_> {
        SessionsRepo::new(&self.conn)
    }

    /// Repository for [`crate::model::ProjectRecord`] rows.
    #[must_use]
    pub fn projects(&self) -> ProjectsRepo<'_> {
        ProjectsRepo::new(&self.conn)
    }

    /// Repository for app-level settings (schema v10 kv).
    #[must_use]
    pub fn app_settings(&self) -> AppSettingsRepo<'_> {
        AppSettingsRepo::new(&self.conn)
    }

    /// Raw connection access for migrations/fixture tooling (tests,
    /// diagnostics). Normal writes go through the repositories.
    #[must_use]
    pub fn raw_conn(&self) -> &Connection {
        &self.conn
    }

    /// The full forward-only migration list (diagnostics + upgrade
    /// tests build earlier schema states from it).
    #[must_use]
    pub fn migrations() -> &'static [&'static str] {
        MIGRATIONS
    }
}
