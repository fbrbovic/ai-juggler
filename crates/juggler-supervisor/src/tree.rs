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

    /// Flips a running row to the permanent `unverifiable` state
    /// (AJ-E503 Req.5): identity verification failed, so the row
    /// leaves every kill path and the auto-tier candidate set
    /// forever — `candidates` selects `running` only, and the prune
    /// touches `exited` only.
    ///
    /// # Errors
    ///
    /// [`LedgerError::Store`] on store failure.
    fn mark_unverifiable(&self, pid: u32, boot_id: &str) -> Result<(), LedgerError>;

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

    fn mark_unverifiable(&self, pid: u32, boot_id: &str) -> Result<(), LedgerError> {
        self.repo
            .mark_unverifiable(pid, boot_id, now_ms())
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

/// The identity of a process we own (AJ-E503, pass2 §12 / §6.2): a
/// PID is a NAME, not an identity — this tuple is. `start_time` is
/// the OS process start (epoch s, fork-time — exec does not change
/// it), `exe` the kernel-reported executable (resolved; may legitimately
/// change across an `exec`, so both the pre- and post-settle records
/// count as ours), `boot_id` the machine's boot identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    /// OS process id (the name, kept for correlation).
    pub pid: u32,
    /// Process-group id (`None` where the platform lacks groups).
    pub pgid: Option<u32>,
    /// OS process start time, epoch seconds — the reuse discriminator.
    pub start_time: i64,
    /// Kernel-reported executable path at record time.
    pub exe: std::path::PathBuf,
    /// OS boot time, epoch seconds as text — same-machine-same-boot.
    pub boot_id: String,
}

/// Fail-closed verification outcome (§6.2): `Unverifiable` covers
/// every mismatch AND every probe error — the caller MUST refuse to
/// touch the process; `Verified` is only ever earned by full
/// agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verification {
    /// All recorded facts still name our process.
    Verified,
    /// Mismatch or probe error — the PID may belong to a stranger.
    Unverifiable,
}

/// What the OS says about a pid right now (the verify probe's facts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbedProcess {
    /// Live OS start time, epoch seconds.
    pub start_time: i64,
    /// Live kernel-reported executable path.
    pub exe: std::path::PathBuf,
}

/// The live-facts seam behind [`verify`]: production reads sysinfo
/// and the boot clock; tests forge results (the quality-gate
/// single-field mismatch tests).
pub trait IdentityProbe {
    /// Probes one pid; `None` is a PROBE ERROR (gone, unreadable exe,
    /// anything) — never a verdict of innocence.
    fn probe(&self, pid: u32) -> Option<ProbedProcess>;
    /// The machine's current boot identity.
    fn boot_id(&self) -> String;
}

/// The real probe: one targeted sysinfo refresh (start time + exe —
/// the §6.2 `stat`/`proc_pidinfo` cost) and the boot clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsIdentityProbe;

impl IdentityProbe for OsIdentityProbe {
    fn probe(&self, pid: u32) -> Option<ProbedProcess> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        let mut system = sysinfo::System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        let process = system.process(sysinfo::Pid::from_u32(pid))?;
        // A missing exe is a probe error (fail closed), not a pass.
        let exe = process.exe()?.to_path_buf();
        Some(ProbedProcess {
            start_time: i64::try_from(process.start_time()).ok()?,
            exe,
        })
    }

    fn boot_id(&self) -> String {
        sysinfo::System::boot_time().to_string()
    }
}

/// Verifies `pid` against the recorded [`ProcessIdentity`] using the
/// real OS probe (§6.2, fail closed).
#[must_use]
pub fn verify(pid: u32, expect: &ProcessIdentity) -> Verification {
    verify_with(&OsIdentityProbe, pid, expect)
}

/// The injectable core: ALL of (a) live start-time, (b) live exe,
/// (c) current boot id must equal the record. The boot gate runs
/// FIRST (FMEA C8): a reboot invalidates start-time comparisons as a
/// class, not field by field. Any mismatch — or any probe error —
/// is `Unverifiable`; errors are NEVER `Verified`.
pub fn verify_with(probe: &dyn IdentityProbe, pid: u32, expect: &ProcessIdentity) -> Verification {
    // (c) first: a different boot means the record predates the
    // machine's current life — every other comparison is meaningless.
    if probe.boot_id() != expect.boot_id {
        return Verification::Unverifiable;
    }
    // A probe error is a refusal, not an acquittal.
    let Some(probed) = probe.probe(pid) else {
        return Verification::Unverifiable;
    };
    if probed.start_time != expect.start_time {
        return Verification::Unverifiable;
    }
    if probed.exe != expect.exe {
        return Verification::Unverifiable;
    }
    Verification::Verified
}

/// Adoption replay helper: a pid passes when ANY recorded identity
/// fully verifies (the spawn-time record AND the post-`exec` settled
/// record both name our process — a stranger can match neither,
/// because start-time must agree in the matched record).
#[must_use]
pub fn verify_any(
    probe: &dyn IdentityProbe,
    pid: u32,
    expects: &[ProcessIdentity],
) -> Verification {
    if expects.is_empty() {
        return Verification::Unverifiable;
    }
    let mut outcome = Verification::Unverifiable;
    for expect in expects {
        if verify_with(probe, pid, expect) == Verification::Verified {
            outcome = Verification::Verified;
            break;
        }
    }
    outcome
}

#[cfg(test)]
mod verify_tests {
    use super::{
        IdentityProbe, ProbedProcess, ProcessIdentity, Verification, verify_any, verify_with,
    };
    use std::path::PathBuf;

    /// Forges probe results; `probed = None` simulates a probe error.
    struct FakeProbe {
        probed: Option<ProbedProcess>,
        boot: String,
    }

    impl IdentityProbe for FakeProbe {
        fn probe(&self, _pid: u32) -> Option<ProbedProcess> {
            self.probed.clone()
        }

        fn boot_id(&self) -> String {
            self.boot.clone()
        }
    }

    fn facts() -> ProbedProcess {
        ProbedProcess {
            start_time: 1_792_000_100,
            exe: PathBuf::from("/bin/sleep"),
        }
    }

    fn recorded() -> ProcessIdentity {
        ProcessIdentity {
            pid: 4242,
            pgid: Some(4242),
            start_time: 1_792_000_100,
            exe: PathBuf::from("/bin/sleep"),
            boot_id: "1791999000".to_string(),
        }
    }

    fn probe_with(probed: Option<ProbedProcess>, boot: &str) -> FakeProbe {
        FakeProbe {
            probed,
            boot: boot.to_string(),
        }
    }

    #[test]
    fn all_three_facts_agree_verifies() {
        let probe = probe_with(Some(facts()), "1791999000");
        assert_eq!(
            verify_with(&probe, 4242, &recorded()),
            Verification::Verified
        );
    }

    #[test]
    fn start_time_mismatch_is_unverifiable() {
        let probe = probe_with(Some(facts()), "1791999000");
        let mut expect = recorded();
        expect.start_time += 1;
        assert_eq!(
            verify_with(&probe, 4242, &expect),
            Verification::Unverifiable
        );
    }

    #[test]
    fn exe_mismatch_is_unverifiable() {
        let probe = probe_with(Some(facts()), "1791999000");
        let mut expect = recorded();
        expect.exe = PathBuf::from("/usr/local/bin/stranger");
        assert_eq!(
            verify_with(&probe, 4242, &expect),
            Verification::Unverifiable
        );
    }

    #[test]
    fn boot_id_mismatch_is_unverifiable() {
        // Live facts agree on everything except the boot.
        let probe = probe_with(Some(facts()), "1791999999");
        assert_eq!(
            verify_with(&probe, 4242, &recorded()),
            Verification::Unverifiable
        );
    }

    #[test]
    fn probe_error_is_unverifiable_never_verified() {
        let probe = probe_with(None, "1791999000");
        assert_eq!(
            verify_with(&probe, 4242, &recorded()),
            Verification::Unverifiable
        );
    }

    #[test]
    fn empty_or_partial_identity_sets_do_not_verify() {
        let probe = probe_with(Some(facts()), "1791999000");
        assert_eq!(verify_any(&probe, 4242, &[]), Verification::Unverifiable);
        // One matching record among mismatched ones still verifies —
        // each record must FULLY agree on its own.
        let mut stranger = recorded();
        stranger.start_time += 9;
        assert_eq!(
            verify_any(&probe, 4242, &[stranger, recorded()]),
            Verification::Verified
        );
    }
}
