//! Ledger types + the storage-agnostic seam (AJ-E501, pass2 §12): the
//! supervisor speaks to [`LedgerStore`]; the SQL lives behind
//! [`juggler_core::storage::ProcessLedgerRepo`] and is bridged by
//! [`CoreLedgerStore`]. The JSONL journal stays a crash log — this is
//! the query store that survives root exit, app exit, and reboot.

use std::sync::Arc;

use juggler_core::model::{MachineId, SessionId};
use juggler_core::storage::{LedgerRow, ProcessLedgerRepo};

/// Why the spawn context exists: ownership (provider/account/project/
/// machine) is known by the CALLER, not the supervisor — the ledger
/// records it intent-first so orphans stay attributable after exit.
#[derive(Debug, Clone)]
pub struct SpawnContext {
    /// Provider id.
    pub provider: String,
    /// Owning Account id, when one exists (login terminals may not).
    pub account_id: Option<String>,
    /// Owning project id.
    pub project_id: Option<String>,
    /// Machine identity (D-13; the stable local id for local rows —
    /// never a hard-coded literal at call sites).
    pub machine: MachineId,
    /// `root` | `login` (helpers are upserted by the tick, role set by
    /// the store).
    pub role: &'static str,
}

/// Ledger failures are HEALTH, not spawn failures (Req.3): every
/// method returns `Result` and the supervisor degrades — spawns never
/// fail because the ledger did.
#[derive(Debug)]
pub enum LedgerError {
    /// The underlying store failed (SQL/I/O); text for the log.
    Store(String),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(message) => write!(f, "ledger store: {message}"),
        }
    }
}

/// The supervisor-side ledger contract (pass2 §12): `intent` before
/// spawn, `spawned` to patch the pid, batched `upsert_helpers` per
/// reconciler tick, `mark_exited` from wait threads, `candidates` for
/// orphan detection (AJ-E504), `prune` for retention.
pub trait LedgerStore: Send + Sync {
    /// This machine's boot identity (OS boot time, epoch seconds) —
    /// every row and exit patch carries it.
    fn boot_id(&self) -> String;

    /// Writes the Intent row (state `Starting`, no PID).
    ///
    /// # Errors
    ///
    /// [`LedgerError::Store`] when the store fails — never panics,
    /// never fails the spawn.
    fn intent(&self, row: &LedgerRow) -> Result<(), LedgerError>;

    /// Patches the session's newest Intent row with pid/pgid/OS start
    /// time; state → `running`.
    ///
    /// # Errors
    ///
    /// [`LedgerError::Store`] on store failure.
    #[allow(clippy::similar_names)] // pid/pgid are the kernel's own names
    #[allow(clippy::similar_names)] // pid/pgid are the kernel's own names
    fn spawned(
        &self,
        session: &SessionId,
        pid: u32,
        pgid: Option<u32>,
        started_at: Option<i64>,
    ) -> Result<(), LedgerError>;

    /// Batched helper upsert (one transaction per tick).
    ///
    /// # Errors
    ///
    /// [`LedgerError::Store`] on store failure.
    fn upsert_helpers(&self, session: &SessionId, rows: &[LedgerRow]) -> Result<(), LedgerError>;

    /// Patches one row's exit classification.
    ///
    /// # Errors
    ///
    /// [`LedgerError::Store`] on store failure.
    fn mark_exited(
        &self,
        pid: u32,
        boot_id: &str,
        exit_kind: &str,
        reaped_by_us: bool,
    ) -> Result<(), LedgerError>;

    /// Running rows on a machine — the orphan-candidate input.
    ///
    /// # Errors
    ///
    /// [`LedgerError::Store`] on store failure.
    fn candidates(&self, machine: &MachineId) -> Result<Vec<LedgerRow>, LedgerError>;

    /// Retention prune (exited rows only).
    ///
    /// # Errors
    ///
    /// [`LedgerError::Store`] on store failure.
    fn prune(&self, exited_before_ms: i64) -> Result<usize, LedgerError>;
}

/// Bridges the core SQLite repo into [`LedgerStore`] — the ONLY SQL
/// implementation; the supervisor itself stays storage-agnostic.
#[derive(Debug)]
pub struct CoreLedgerStore {
    repo: ProcessLedgerRepo,
    /// Boot identity for this process's lifetime (OS boot time, epoch
    /// seconds — makes `(pid, started_at)` unique across reboots).
    boot_id: String,
}

impl CoreLedgerStore {
    /// Wraps an open repo; the boot id is the OS boot time (epoch
    /// seconds), probed once per process — it cannot change under a
    /// running process.
    #[must_use]
    pub fn new(repo: ProcessLedgerRepo) -> Self {
        Self {
            repo,
            boot_id: sysinfo::System::boot_time().to_string(),
        }
    }

    /// This store's boot id (tests assert it is stable per process).
    #[must_use]
    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }

    /// Rows for one session (tests, the Processes view, evidence
    /// dumps).
    ///
    /// # Errors
    ///
    /// [`juggler_core::error::CoreError`] on store failure.
    pub fn rows_for_session(
        &self,
        session: &SessionId,
    ) -> Result<Vec<LedgerRow>, juggler_core::error::CoreError> {
        self.repo.rows_for_session(session.as_str())
    }
}

use std::time::SystemTime;

impl LedgerStore for CoreLedgerStore {
    fn boot_id(&self) -> String {
        self.boot_id.clone()
    }

    fn intent(&self, row: &LedgerRow) -> Result<(), LedgerError> {
        self.repo
            .intent(row)
            .map_err(|e| LedgerError::Store(e.to_string()))
    }

    #[allow(clippy::similar_names)] // pid/pgid are the kernel's own names
    fn spawned(
        &self,
        session: &SessionId,
        pid: u32,
        pgid: Option<u32>,
        started_at: Option<i64>,
    ) -> Result<(), LedgerError> {
        self.repo
            .spawned(session.as_str(), pid, pgid, started_at, now_ms())
            .map_err(|e| LedgerError::Store(e.to_string()))
    }

    fn upsert_helpers(&self, session: &SessionId, rows: &[LedgerRow]) -> Result<(), LedgerError> {
        self.repo
            .upsert_helpers(session.as_str(), rows)
            .map_err(|e| LedgerError::Store(e.to_string()))
    }

    fn mark_exited(
        &self,
        pid: u32,
        boot_id: &str,
        exit_kind: &str,
        reaped_by_us: bool,
    ) -> Result<(), LedgerError> {
        self.repo
            .mark_exited(pid, boot_id, exit_kind, reaped_by_us, now_ms())
            .map_err(|e| LedgerError::Store(e.to_string()))
    }

    fn candidates(&self, machine: &MachineId) -> Result<Vec<LedgerRow>, LedgerError> {
        self.repo
            .candidates(machine.as_str())
            .map_err(|e| LedgerError::Store(e.to_string()))
    }

    fn prune(&self, exited_before_ms: i64) -> Result<usize, LedgerError> {
        self.repo
            .prune(exited_before_ms)
            .map_err(|e| LedgerError::Store(e.to_string()))
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Computes the display/hash fields of a command line (identity key —
/// `DefaultHasher`, not cryptography).
#[must_use]
pub fn cmdline_fields(program: &std::path::Path, args: &[String]) -> (String, String) {
    use std::hash::{Hash, Hasher};
    let mut line = program.to_string_lossy().into_owned();
    for arg in args {
        line.push(' ');
        line.push_str(arg);
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    line.hash(&mut hasher);
    let trunc: String = line.chars().take(200).collect();
    (format!("{:016x}", hasher.finish()), trunc)
}

/// Convenience alias for the shared store handle.
pub type SharedLedger = Arc<dyn LedgerStore>;
