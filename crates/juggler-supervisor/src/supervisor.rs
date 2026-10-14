//! The process supervisor: spawns harness sessions (PTY or piped), captures
//! output, runs the stop ladder, and journals lifecycle for crash re-adoption
//! (spec §5.1, §5.3).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use juggler_adapter::LaunchSpec;
use juggler_core::model::{SessionId, Timestamp};

use crate::error::SupervisorError;
use crate::journal::Journal;
use crate::lifecycle::{ExitKind, LifecycleEvent};
use crate::ring::OutputRing;

/// Default output retention per session (spec §1 footprint budget).
const DEFAULT_RING_CAPACITY: usize = 512 * 1024;

/// A successfully spawned supervised process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedProcess {
    /// The session this process runs for.
    pub session: SessionId,
    /// OS process id.
    pub pid: u32,
}

/// Live process statistics for one supervised Instance (AJ-E401 read
/// model). Sums cover the root process plus its descendants; CPU% is
/// only reported once the collector has two table samples (a single
/// refresh cannot measure a delta).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LiveProcessStats {
    /// Descendant process count (the root itself is not counted).
    pub child_count: u32,
    /// CPU utilization summed over the tree, percent; `None` until the
    /// second table refresh (never a fabricated 0.0).
    pub cpu_pct: Option<f64>,
    /// Resident memory summed over the tree, bytes.
    pub rss_bytes: u64,
}

impl LiveProcessStats {
    /// Definitive zeros for a session whose process has exited — nothing
    /// can be running or resident after exit.
    #[must_use]
    pub fn dead() -> Self {
        Self {
            child_count: 0,
            cpu_pct: Some(0.0),
            rss_bytes: 0,
        }
    }
}

/// One live process in a supervised tree (the Processes tab row,
/// AJ-E410). `cpu_pct` stays `None` until the second table sample.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProcessRow {
    /// OS process id.
    pub pid: u32,
    /// Parent pid (`None` when unreadable); the root's own parent.
    pub parent_pid: Option<u32>,
    /// Process name.
    pub name: String,
    /// CPU utilization, percent; `None` before two samples.
    pub cpu_pct: Option<f64>,
    /// Resident memory, bytes.
    pub rss_bytes: u64,
    /// Process age, ms (from the kernel's start time).
    pub age_ms: u64,
    /// Direct children of this process (within the tree).
    pub child_count: u32,
    /// Tree depth (root = 0) — the UI's indent level.
    pub depth: u8,
}

/// One point of the per-tree resource ring (the I-07 sparkline source).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeSample {
    /// Sample time, epoch ms.
    pub at_ms: i64,
    /// Tree CPU%, percent; `None` before two table samples.
    pub cpu_pct: Option<f64>,
    /// Tree RSS sum, bytes.
    pub rss_bytes: u64,
}

/// Per-session collector state: the 5-minute sample ring plus the
/// freshest per-process rows.
#[derive(Debug, Clone, Default)]
pub struct TreeState {
    /// 2 s samples retained 5 min (150/tree, the performance budget).
    pub ring: std::collections::VecDeque<TreeSample>,
    /// Latest rows (root first, then descendants breadth-first); frozen
    /// at the last pre-exit tick for finished sessions (the orphan
    /// snapshot source).
    pub rows: Vec<ProcessRow>,
}

/// Ring capacity: 150 samples × 2 s = the 5-minute retention budget.
const RING_CAPACITY: usize = 150;

/// Full-table discovery cadence: every 5th tick (a full scan finds NEW
/// descendants; the 2 s hot path refreshes only known pids). The first
/// three ticks are full scans too (warmup: a fast-moving fixture's
/// children must land in the table before it exits).
const FULL_SCAN_EVERY: u32 = 5;
/// Warmup full-scan ticks after collector creation.
const WARMUP_FULL_TICKS: u32 = 3;

/// Owns the persistent process table so per-process CPU% deltas survive
/// between ticks (sysinfo computes usage between refreshes). One
/// collector serves the 2 s per-tree tick (AJ-E410), the 5 s resource
/// emission, and `instances_list`.
///
/// Refresh shape (staggered by design): the 2 s hot path refreshes ONLY
/// the known supervised pids (one targeted batch — there is exactly one
/// sampler thread, so the per-tree herd the stagger defends against
/// cannot exist by construction); every 10th tick does one full-table
/// refresh to discover newly spawned descendants.
#[derive(Debug, Default)]
pub struct LiveStatsCollector {
    system: sysinfo::System,
    sampled: bool,
    ticks: u32,
    trees: HashMap<SessionId, TreeState>,
    stats: HashMap<SessionId, LiveProcessStats>,
    /// Per-session ledger upsert throttle (AJ-E501 Req.4: at most one
    /// batched helper upsert per 2 s per Instance).
    last_ledger_upsert: HashMap<SessionId, i64>,
}

impl LiveStatsCollector {
    /// Creates a collector with an empty (not yet refreshed) table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One 2 s sampling tick (AJ-E410): targeted refresh of known pids
    /// (full scan every 5th tick for discovery),
    /// recompute per-tree rows + stats, append one ring sample per live
    /// tree. Finished sessions keep their last rows frozen (the exit
    /// snapshot source) and report definitive dead stats.
    pub fn tick(&mut self, supervisor: &Supervisor, now_ms: i64) {
        self.ticks = self.ticks.saturating_add(1);
        let full = self.ticks <= WARMUP_FULL_TICKS || self.ticks.is_multiple_of(FULL_SCAN_EVERY);
        let roots = supervisor.live_roots();
        if full {
            self.refresh_all();
            let parents = parents_map(&self.system);
            self.rebuild_trees(&roots, &parents, now_ms);
        } else {
            let known: Vec<u32> = self
                .trees
                .values()
                .flat_map(|tree| tree.rows.iter().map(|row| row.pid))
                .chain(roots.iter().map(|(_, pid, _)| *pid))
                .collect();
            self.refresh_pids(&known);
            // Targeted refresh keeps parent edges of known pids fresh;
            // brand-new children appear at the next full scan.
            let parents = parents_map(&self.system);
            self.rebuild_trees(&roots, &parents, now_ms);
        }
        self.ledger_upserts(supervisor, now_ms);
        let sampled = self.sampled;
        self.sampled = true;
        if !sampled {
            // First-ever tick: CPU deltas do not exist yet — honest
            // absence (None), never a zero that would read as "idle".
            for tree in self.trees.values_mut() {
                if let Some(last) = tree.ring.back_mut() {
                    last.cpu_pct = None;
                }
                for row in &mut tree.rows {
                    row.cpu_pct = None;
                }
            }
            for stat in self.stats.values_mut() {
                if stat.cpu_pct == Some(0.0_f64) {
                    stat.cpu_pct = None;
                }
            }
        }
    }

    /// One batched helper upsert per live session per 2 s (AJ-E501
    /// Req.4): rows come from the freshly rebuilt trees; the root's
    /// own row was patched at spawn and is owned by the exit patch.
    fn ledger_upserts(&mut self, supervisor: &Supervisor, now_ms: i64) {
        let Some(ledger) = supervisor.ledger.clone() else {
            return;
        };
        let contexts = supervisor
            .spawn_contexts
            .lock()
            .map(|contexts| contexts.clone())
            .unwrap_or_default();
        let boot_id = ledger.boot_id();
        for (session, tree) in &self.trees {
            let Some(root_row) = tree.rows.first() else {
                continue;
            };
            let last = self.last_ledger_upsert.get(session).copied().unwrap_or(0);
            if now_ms - last < 2_000 {
                continue;
            }
            let Some(ctx) = contexts.get(session) else {
                continue;
            };
            let mut helpers = Vec::new();
            for row in tree.rows.iter().skip(1) {
                let started_at = now_ms
                    .checked_sub(i64::try_from(row.age_ms).unwrap_or(0))
                    .map(|ms| ms.div_euclid(1000));
                helpers.push(juggler_core::storage::LedgerRow {
                    pid: Some(row.pid),
                    pgid: None,
                    started_at,
                    exe_path: row.name.clone(),
                    cmdline_hash: format!("name:{}", row.name),
                    cmdline_trunc: row.name.clone(),
                    boot_id: boot_id.clone(),
                    session_id: session.as_str().to_string(),
                    instance_id: None,
                    provider: ctx.provider.clone(),
                    account_id: ctx.account_id.clone(),
                    project_id: ctx.project_id.clone(),
                    machine_id: ctx.machine.as_str().to_string(),
                    root_pid: Some(root_row.pid),
                    parent_pid: row.parent_pid,
                    depth: 1,
                    role: juggler_core::storage::ROLE_HELPER.to_string(),
                    state: String::new(),
                    listens_on: None,
                    cwd: String::new(),
                    peak_rss: Some(i64::try_from(row.rss_bytes).unwrap_or(0)),
                    peak_cpu: row.cpu_pct,
                    exit_kind: None,
                    reaped_by_us: None,
                    first_seen: now_ms,
                    last_seen: now_ms,
                });
            }
            if helpers.is_empty() {
                // Nothing to write this tick; keep the throttle open so
                // the first helper appearance is not delayed by 2 s.
                continue;
            }
            if ledger.upsert_helpers(session, &helpers).is_err() {
                supervisor
                    .ledger_degraded
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                eprintln!("ai-juggler: process ledger degraded at helper upsert");
            }
            self.last_ledger_upsert.insert(session.clone(), now_ms);
        }
    }

    fn refresh_all(&mut self) {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
    }

    /// Targeted refresh of specific pids (orphan liveness probes).
    pub fn refresh_pids(&mut self, pids: &[u32]) {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
        let targets: Vec<sysinfo::Pid> = pids
            .iter()
            .map(|pid| sysinfo::Pid::from_u32(*pid))
            .collect();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&targets),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
    }

    /// Recomputes every tree's rows/stats/ring from the current table.
    fn rebuild_trees(
        &mut self,
        roots: &[(SessionId, u32, bool)],
        parents: &HashMap<u32, Vec<u32>>,
        now_ms: i64,
    ) {
        // Drop state for sessions the supervisor no longer knows.
        let live_ids: Vec<SessionId> = roots.iter().map(|(id, _, _)| id.clone()).collect();
        self.trees.retain(|id, _| live_ids.contains(id));
        self.stats.retain(|id, _| live_ids.contains(id));
        for (session, root, finished) in roots {
            let finished = *finished;
            let tree = self.trees.entry(session.clone()).or_default();
            if finished {
                // Frozen: keep the last pre-exit rows (orphan snapshot
                // source) and report definitive dead stats.
                self.stats
                    .insert((*session).clone(), LiveProcessStats::dead());
                continue;
            }
            let (rows, stats) = tree_rows(&self.system, parents, *root);
            tree.rows = rows;
            tree.ring.push_back(TreeSample {
                at_ms: now_ms,
                cpu_pct: stats.cpu_pct,
                rss_bytes: stats.rss_bytes,
            });
            tree.ring.truncate(RING_CAPACITY);
            self.stats.insert((*session).clone(), stats);
        }
    }

    /// Cached batched stats from the last tick (no refresh — the read
    /// model must not trigger table refreshes per row, AJ-E401).
    #[must_use]
    pub fn stats_snapshot(&self) -> HashMap<SessionId, LiveProcessStats> {
        self.stats.clone()
    }

    /// The latest per-process rows for one session (frozen at the last
    /// pre-exit tick for finished sessions).
    #[must_use]
    pub fn rows(&self, session: &SessionId) -> Option<Vec<ProcessRow>> {
        self.trees.get(session).map(|tree| tree.rows.clone())
    }

    /// The 5-minute sample ring for one session (the I-07 sparkline
    /// source), oldest first.
    #[must_use]
    pub fn ring(&self, session: &SessionId) -> Vec<TreeSample> {
        self.trees
            .get(session)
            .map(|tree| tree.ring.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Live, non-zombie lookup against the current table: `(name, rss)`
    /// or `None` when the pid is gone/zombie.
    #[must_use]
    pub fn lookup(&self, pid: u32) -> Option<(String, u64)> {
        self.system
            .process(sysinfo::Pid::from_u32(pid))
            .filter(|process| {
                !matches!(
                    process.status(),
                    sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead
                )
            })
            .map(|process| {
                (
                    process.name().to_string_lossy().into_owned(),
                    process.memory(),
                )
            })
    }

    /// Refreshes the process table once and returns batched stats for
    /// every supervised session (compat API for one-shot callers; the
    /// steady state is [`LiveStatsCollector::tick`]).
    pub fn refresh(&mut self, supervisor: &Supervisor) -> HashMap<SessionId, LiveProcessStats> {
        let now = juggler_core::model::Timestamp::now().millis();
        self.tick(supervisor, now);
        self.stats.clone()
    }
}

/// ppid → children map over the current table.
fn parents_map(system: &sysinfo::System) -> HashMap<u32, Vec<u32>> {
    let mut parents: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, process) in system.processes() {
        if let Some(parent) = process.parent() {
            parents
                .entry(parent.as_u32())
                .or_default()
                .push(pid.as_u32());
        }
    }
    parents
}

/// Per-process rows (root first, breadth-first over descendants) plus
/// the tree aggregate, read from a table the caller already refreshed.
fn tree_rows(
    system: &sysinfo::System,
    parents: &HashMap<u32, Vec<u32>>,
    root: u32,
) -> (Vec<ProcessRow>, LiveProcessStats) {
    let mut rows = Vec::new();
    let mut cpu = 0.0_f64;
    let mut rss = 0_u64;
    let mut child_count = 0_u32;
    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<(u32, u8)> =
        std::collections::VecDeque::from([(root, 0)]);
    while let Some((pid, depth)) = queue.pop_front() {
        if !seen.insert(pid) {
            continue; // ppid cycles must not loop forever
        }
        let children = parents.get(&pid).cloned().unwrap_or_default();
        let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) else {
            queue.extend(children.into_iter().map(|child| (child, depth + 1)));
            continue;
        };
        let age_ms = process.run_time().saturating_mul(1000);
        rows.push(ProcessRow {
            pid,
            parent_pid: process.parent().map(sysinfo::Pid::as_u32),
            name: process.name().to_string_lossy().into_owned(),
            cpu_pct: Some(f64::from(process.cpu_usage())),
            rss_bytes: process.memory(),
            age_ms,
            child_count: u32::try_from(children.len()).unwrap_or(u32::MAX),
            depth,
        });
        cpu += f64::from(process.cpu_usage());
        rss += process.memory();
        if pid != root {
            child_count += 1;
        }
        queue.extend(children.into_iter().map(|child| (child, depth + 1)));
    }
    (
        rows,
        LiveProcessStats {
            child_count,
            cpu_pct: Some(cpu),
            rss_bytes: rss,
        },
    )
}

/// Caller-supplied handle extras (PTY writer + pre-created output
/// tracking atomic) — keeps `register` under the argument budget.
struct ChildExtras {
    pty: Option<Arc<Mutex<PtyHandle>>>,
    last_output_ms: Arc<std::sync::atomic::AtomicI64>,
}

struct ChildHandle {
    pid: u32,
    ring: Arc<Mutex<OutputRing>>,
    finished: Arc<AtomicBool>,
    /// Last output arrival (epoch ms, 0 = none) — feeds the Stuck rule
    /// and event-driven recovery (AJ-E405).
    last_output_ms: Arc<std::sync::atomic::AtomicI64>,
    force_kill: Arc<dyn Fn() + Send + Sync>,
    /// Live PTY master for interactive sessions: keyboard input and
    /// resize. `None` for piped (headless) sessions.
    pty: Option<Arc<Mutex<PtyHandle>>>,
}

/// The write/resize side of a supervised PTY.
struct PtyHandle {
    writer: Box<dyn std::io::Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
}

/// Owns supervised sessions for one AI Juggler process.
///
/// Children **survive** the supervisor (and the app) by design: dropping it
/// neither signals nor waits. Re-adoption after a restart happens via
/// [`Journal::adopt_orphans`].
pub struct Supervisor {
    journal: Option<Journal>,
    /// Persistent spawn ledger (AJ-E501): intent rows before spawn,
    /// helper upserts per tick — survives app exit and reboot. `None`
    /// = unconfigured (no degraded flag; journal still records roots).
    ledger: Option<crate::tree::SharedLedger>,
    /// Set when a CONFIGURED ledger fails: spawns never fail because
    /// of it — cleanup turns read-only until restart (the banner).
    ledger_degraded: std::sync::atomic::AtomicBool,
    children: Mutex<HashMap<SessionId, Arc<ChildHandle>>>,
    /// Sessions stopped via [`Supervisor::stop`] — PTY children surface
    /// user kills as plain non-zero exit codes, so the stop intent is the
    /// only reliable "not a failure" signal.
    user_stopped: Mutex<Vec<SessionId>>,
    events_tx: std::sync::mpsc::Sender<LifecycleEvent>,
    events_rx: Mutex<std::sync::mpsc::Receiver<LifecycleEvent>>,
    /// Spawn contexts (AJ-E501): helper rows inherit the root's
    /// provider/account/project/machine; kept for the ledger tick.
    spawn_contexts: Mutex<HashMap<SessionId, crate::tree::SpawnContext>>,
    /// Sessions with fresh output since the last drain (throttled per
    /// session) — the recovery signal (AJ-E405 Req.5, <300 ms).
    activity_tx: std::sync::mpsc::SyncSender<SessionId>,
    activity_rx: Mutex<std::sync::mpsc::Receiver<SessionId>>,
}

impl Default for Supervisor {
    fn default() -> Self {
        let (events_tx, events_rx) = std::sync::mpsc::channel();
        let (activity_tx, activity_rx) = std::sync::mpsc::sync_channel(256);
        Self {
            journal: None,
            ledger: None,
            ledger_degraded: std::sync::atomic::AtomicBool::new(false),
            spawn_contexts: Mutex::new(HashMap::new()),
            children: Mutex::new(HashMap::new()),
            user_stopped: Mutex::new(Vec::new()),
            events_tx,
            events_rx: Mutex::new(events_rx),
            activity_tx,
            activity_rx: Mutex::new(activity_rx),
        }
    }
}

impl Supervisor {
    /// Creates a supervisor without a journal (no crash re-adoption).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a supervisor that journals lifecycle to `journal_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Journal`] when the journal cannot be
    /// opened.
    pub fn with_journal(journal_dir: &Path) -> Result<Self, SupervisorError> {
        Ok(Self {
            journal: Some(Journal::open(journal_dir.join("supervision.jsonl"))?),
            ..Self::default()
        })
    }

    /// Attaches the persistent spawn ledger (AJ-E501). Without it the
    /// supervisor still runs — the ledger is additive.
    #[must_use]
    pub fn with_ledger(mut self, ledger: crate::tree::SharedLedger) -> Self {
        self.ledger = Some(ledger);
        self
    }

    /// True when a configured ledger failed (Req.3): cleanup is
    /// read-only until restart — surfaced as the degraded banner.
    #[must_use]
    pub fn ledger_degraded(&self) -> bool {
        self.ledger_degraded
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Best-effort ledger write: a failure degrades health, never the
    /// spawn (Req.3).
    fn ledger_write(
        &self,
        write: impl FnOnce(&crate::tree::SharedLedger) -> Result<(), crate::tree::LedgerError>,
    ) {
        if let Some(ledger) = &self.ledger
            && let Err(error) = write(ledger)
        {
            self.ledger_degraded
                .store(true, std::sync::atomic::Ordering::SeqCst);
            eprintln!("ai-juggler: process ledger degraded: {error}");
        }
    }

    /// Builds the Intent row for a spawn context.
    fn intent_row(
        ctx: &crate::tree::SpawnContext,
        session: &SessionId,
        program: &std::path::Path,
        args: &[String],
        cwd: &std::path::Path,
        boot_id: &str,
    ) -> juggler_core::storage::LedgerRow {
        let (cmdline_hash, cmdline_trunc) = crate::tree::cmdline_fields(program, args);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        juggler_core::storage::LedgerRow {
            pid: None,
            pgid: None,
            started_at: None,
            exe_path: program.to_string_lossy().into_owned(),
            cmdline_hash,
            cmdline_trunc,
            boot_id: boot_id.to_string(),
            session_id: session.as_str().to_string(),
            instance_id: None,
            provider: ctx.provider.clone(),
            account_id: ctx.account_id.clone(),
            project_id: ctx.project_id.clone(),
            machine_id: ctx.machine.as_str().to_string(),
            root_pid: None,
            parent_pid: None,
            depth: 0,
            role: ctx.role.to_string(),
            state: String::new(),
            listens_on: None,
            cwd: cwd.to_string_lossy().into_owned(),
            peak_rss: None,
            peak_cpu: None,
            exit_kind: None,
            reaped_by_us: None,
            first_seen: now,
            last_seen: now,
        }
    }

    /// The OS start time (epoch s) of a just-spawned pid, via a
    /// one-shot table probe; `None` when the table has not caught up
    /// (patched later by the reconciler tick).
    fn os_start_time(pid: u32) -> Option<i64> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
        let mut system = sysinfo::System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
            true,
            ProcessRefreshKind::nothing(),
        );
        system
            .process(sysinfo::Pid::from_u32(pid))
            .and_then(|p| i64::try_from(p.start_time()).ok())
    }

    /// Spawns one session per the launch spec. Interactive specs get a real
    /// PTY; headless specs run with piped output.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Spawn`] when the process cannot start.
    pub fn spawn(
        &self,
        session: &SessionId,
        spec: &LaunchSpec,
        ctx: &crate::tree::SpawnContext,
    ) -> Result<SpawnedProcess, SupervisorError> {
        // Intent BEFORE spawn (AJ-E501 Req.3): the ownership record
        // exists even if the spawn or the app dies mid-breath. Ledger
        // failure degrades health, never the spawn.
        let boot_id = self
            .ledger
            .as_ref()
            .map_or_else(|| "boot-unconfigured".to_string(), |l| l.boot_id());
        self.ledger_write(|ledger| {
            ledger.intent(&Self::intent_row(
                ctx,
                session,
                &spec.program,
                &spec.args,
                &spec.cwd,
                &boot_id,
            ))
        });
        if let Ok(mut contexts) = self.spawn_contexts.lock() {
            contexts.insert(session.clone(), ctx.clone());
        }
        let spawned = if spec.needs_pty {
            self.spawn_pty(session, spec)?
        } else {
            self.spawn_piped(session, spec)?
        };
        let started_at = Self::os_start_time(spawned.pid);
        self.ledger_write(|ledger| ledger.spawned(session, spawned.pid, None, started_at));
        if let Some(journal) = &self.journal {
            journal.record_spawned(session, spawned.pid, &spec.program)?;
        }
        let _ = self.events_tx.send(LifecycleEvent::Started {
            session: session.clone(),
            pid: spawned.pid,
        });
        Ok(spawned)
    }

    /// Spawns a desktop-class harness app (AJ-E413): tracked in the
    /// SAME spawn ledger as terminal instances (journal + lifecycle
    /// events + wait thread), with **no PTY and no output capture** —
    /// the window is the surface; stdout/stderr/stdin are null so the
    /// app can never hold our pipes.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Spawn`] when the process cannot start.
    pub fn spawn_desktop(
        &self,
        session: &SessionId,
        launch: &juggler_adapter::DesktopLaunch,
        ctx: &crate::tree::SpawnContext,
    ) -> Result<SpawnedProcess, SupervisorError> {
        use std::process::{Command, Stdio};

        let boot_id = self
            .ledger
            .as_ref()
            .map_or_else(|| "boot-unconfigured".to_string(), |l| l.boot_id());
        self.ledger_write(|ledger| {
            ledger.intent(&Self::intent_row(
                ctx,
                session,
                &launch.program,
                &launch.args,
                &launch.cwd,
                &boot_id,
            ))
        });
        if let Ok(mut contexts) = self.spawn_contexts.lock() {
            contexts.insert(session.clone(), ctx.clone());
        }

        let mut command = Command::new(&launch.program);
        command
            .args(&launch.args)
            .envs(&launch.env)
            .current_dir(&launch.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command.spawn().map_err(|e| SupervisorError::Spawn {
            program: launch.program.clone(),
            message: e.to_string(),
        })?;
        let pid = child.id();
        let started_at = Self::os_start_time(pid);
        self.ledger_write(|ledger| ledger.spawned(session, pid, None, started_at));
        let ring = Arc::new(Mutex::new(OutputRing::new(DEFAULT_RING_CAPACITY)));
        let child = Arc::new(Mutex::new(child));
        let finished = Arc::new(AtomicBool::new(false));
        let kill_child = Arc::clone(&child);
        let force_kill: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = kill_child.lock().map(|mut c| c.kill());
        });
        self.register(
            session.clone(),
            pid,
            ring,
            Arc::clone(&finished),
            force_kill,
            ChildExtras {
                pty: None,
                last_output_ms: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            },
        );
        if let Some(journal) = &self.journal {
            journal.record_spawned(session, pid, &launch.program)?;
        }
        let _ = self.events_tx.send(LifecycleEvent::Started {
            session: session.clone(),
            pid,
        });

        let session_for_thread = session.clone();
        let journal = self.journal.clone();
        let ledger = self.ledger.clone();
        let events = self.events_tx.clone();
        let done = Arc::clone(&finished);
        std::thread::spawn(move || {
            let status = child.lock().ok().and_then(|mut c| c.wait().ok());
            if let Some(status) = status {
                emit_exit(
                    &session_for_thread,
                    pid,
                    exit_kind_std(status),
                    &done,
                    journal.as_ref(),
                    ledger.as_ref(),
                    &events,
                );
            } else {
                done.store(true, Ordering::Release);
            }
        });

        Ok(SpawnedProcess {
            session: session.clone(),
            pid,
        })
    }

    fn spawn_piped(
        &self,
        session: &SessionId,
        spec: &LaunchSpec,
    ) -> Result<SpawnedProcess, SupervisorError> {
        use std::process::{Command, Stdio};

        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .envs(&spec.env)
            .current_dir(&spec.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().map_err(|e| SupervisorError::Spawn {
            program: spec.program.clone(),
            message: e.to_string(),
        })?;
        let pid = child.id();
        let ring = Arc::new(Mutex::new(OutputRing::new(DEFAULT_RING_CAPACITY)));
        let child = Arc::new(Mutex::new(child));
        let finished = Arc::new(AtomicBool::new(false));
        let kill_child = Arc::clone(&child);
        let force_kill: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = kill_child.lock().map(|mut c| c.kill());
        });
        // Register before readers start so output tracking exists from
        // the first chunk (AJ-E405).
        let last_output = self.register(
            session.clone(),
            pid,
            Arc::clone(&ring),
            Arc::clone(&finished),
            force_kill,
            ChildExtras {
                pty: None,
                last_output_ms: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            },
        );
        let (out_atomic, err_atomic) = (Arc::clone(&last_output), last_output);
        let (out_tx, err_tx) = (
            std::sync::mpsc::SyncSender::clone(&self.activity_tx),
            std::sync::mpsc::SyncSender::clone(&self.activity_tx),
        );
        let (out_session, err_session) = (session.clone(), session.clone());
        spawn_reader(
            child_lock(&child).stdout.take(),
            Arc::clone(&ring),
            out_atomic,
            out_session,
            out_tx,
        );
        spawn_reader(
            child_lock(&child).stderr.take(),
            ring,
            err_atomic,
            err_session,
            err_tx,
        );

        let session_for_thread = session.clone();
        let journal = self.journal.clone();
        let ledger = self.ledger.clone();
        let events = self.events_tx.clone();
        let done = Arc::clone(&finished);
        std::thread::spawn(move || {
            let status = child.lock().ok().and_then(|mut c| c.wait().ok());
            if let Some(status) = status {
                emit_exit(
                    &session_for_thread,
                    pid,
                    exit_kind_std(status),
                    &done,
                    journal.as_ref(),
                    ledger.as_ref(),
                    &events,
                );
            } else {
                done.store(true, Ordering::Release);
            }
        });

        Ok(SpawnedProcess {
            session: session.clone(),
            pid,
        })
    }

    fn spawn_pty(
        &self,
        session: &SessionId,
        spec: &LaunchSpec,
    ) -> Result<SpawnedProcess, SupervisorError> {
        use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

        let pty_system: Box<dyn PtySystem> = Box::new(NativePtySystem {});
        let pair = pty_system
            .openpty(PtySize::default())
            .map_err(|e| SupervisorError::Spawn {
                program: spec.program.clone(),
                message: e.to_string(),
            })?;
        let mut builder = CommandBuilder::new(&spec.program);
        builder.cwd(&spec.cwd);
        builder.args(&spec.args);
        // Env semantics (spec §4): the overlay merges over the supervisor's
        // base environment. portable-pty's builder does not reliably inherit
        // the parent env on all platforms, so inherit explicitly, then apply
        // the overlay last so it wins.
        for (key, value) in std::env::vars_os() {
            builder.env(key, value);
        }
        for (key, value) in &spec.env {
            builder.env(key, value);
        }
        let mut child = pair
            .slave
            .spawn_command(builder)
            .map_err(|e| SupervisorError::Spawn {
                program: spec.program.clone(),
                message: e.to_string(),
            })?;
        let Some(pid) = child.process_id() else {
            return Err(SupervisorError::Spawn {
                program: spec.program.clone(),
                message: "pty child had no process id".into(),
            });
        };
        let killer = std::sync::Mutex::new(child.clone_killer());
        let ring = Arc::new(Mutex::new(OutputRing::new(DEFAULT_RING_CAPACITY)));
        // Output tracking exists from chunk 1 without pre-registering the
        // handle: the atomic is independent of the children map.
        let last_output = Arc::new(std::sync::atomic::AtomicI64::new(0));
        if let Ok(reader) = pair.master.try_clone_reader() {
            spawn_reader(
                Some(reader),
                Arc::clone(&ring),
                Arc::clone(&last_output),
                session.clone(),
                std::sync::mpsc::SyncSender::clone(&self.activity_tx),
            );
        }
        drop(pair.slave);
        let master = pair.master;
        let writer = master.take_writer().map_err(|e| SupervisorError::Spawn {
            program: spec.program.clone(),
            message: format!("cannot take pty writer: {e}"),
        })?;
        let pty = Some(Arc::new(Mutex::new(PtyHandle { writer, master })));

        let finished = Arc::new(AtomicBool::new(false));
        let force_kill: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = killer.lock().map(|mut k| k.kill());
        });

        let session_for_thread = session.clone();
        let journal = self.journal.clone();
        let ledger = self.ledger.clone();
        let events = self.events_tx.clone();
        let done = Arc::clone(&finished);
        std::thread::spawn(move || {
            let status = child.wait().ok();
            if let Some(status) = status {
                // portable-pty reports a single exit code; a signal death surfaces
                // as a non-zero code here (skeleton-level approximation).
                let kind = ExitKind::Code(i32::try_from(status.exit_code()).unwrap_or(i32::MAX));
                emit_exit(
                    &session_for_thread,
                    pid,
                    kind,
                    &done,
                    journal.as_ref(),
                    ledger.as_ref(),
                    &events,
                );
            } else {
                done.store(true, Ordering::Release);
            }
        });

        self.register(
            session.clone(),
            pid,
            ring,
            finished,
            force_kill,
            ChildExtras {
                pty,
                last_output_ms: last_output,
            },
        );
        Ok(SpawnedProcess {
            session: session.clone(),
            pid,
        })
    }

    #[allow(clippy::needless_pass_by_value)] // Arc handles are moved intentionally
    /// Registers a child handle; `parts.pty` and `parts.last_output_ms`
    /// come from the caller because the PTY path starts its reader before
    /// the handle exists (output tracking from chunk 1).
    fn register(
        &self,
        session: SessionId,
        pid: u32,
        ring: Arc<Mutex<OutputRing>>,
        finished: Arc<AtomicBool>,
        force_kill: Arc<dyn Fn() + Send + Sync>,
        parts: ChildExtras,
    ) -> Arc<std::sync::atomic::AtomicI64> {
        let last_output_ms = parts.last_output_ms;
        let handle = Arc::new(ChildHandle {
            pid,
            ring,
            finished,
            last_output_ms: Arc::clone(&last_output_ms),
            force_kill,
            pty: parts.pty,
        });
        if let Ok(mut children) = self.children.lock() {
            children.insert(session, handle);
        }
        last_output_ms
    }

    /// Stops a session: SIGTERM (unix), waits `grace`, then force-kills.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::UnknownSession`] when the id is not
    /// supervised.
    pub fn stop(&self, session: &SessionId, grace: Duration) -> Result<(), SupervisorError> {
        let handle = self
            .children
            .lock()
            .map_err(|_| SupervisorError::UnknownSession(session.clone()))?
            .get(session)
            .cloned()
            .ok_or_else(|| SupervisorError::UnknownSession(session.clone()))?;

        if let Ok(mut stopped) = self.user_stopped.lock() {
            stopped.retain(|id| id != session);
            stopped.push(session.clone());
        }
        send_terminate(handle.pid);
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline && !handle.finished.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !handle.finished.load(Ordering::Acquire) {
            (handle.force_kill)();
        }
        Ok(())
    }

    /// Snapshot of the retained output tail for a session.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::UnknownSession`] when the id is not
    /// supervised.
    pub fn output(&self, session: &SessionId) -> Result<Vec<u8>, SupervisorError> {
        let handle = self
            .children
            .lock()
            .map_err(|_| SupervisorError::UnknownSession(session.clone()))?
            .get(session)
            .cloned()
            .ok_or_else(|| SupervisorError::UnknownSession(session.clone()))?;
        handle
            .ring
            .lock()
            .map(|ring| ring.snapshot())
            .map_err(|_| SupervisorError::UnknownSession(session.clone()))
    }

    /// Live descendant process ids of a session's process tree (ppid walk).
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::UnknownSession`] when the id is not
    /// supervised.
    pub fn descendants(&self, session: &SessionId) -> Result<Vec<u32>, SupervisorError> {
        let handle = self
            .children
            .lock()
            .map_err(|_| SupervisorError::UnknownSession(session.clone()))?
            .get(session)
            .cloned()
            .ok_or_else(|| SupervisorError::UnknownSession(session.clone()))?;
        Ok(descendants_of(handle.pid))
    }

    /// When output last arrived for the session (epoch ms); `None` when
    /// unknown/never. Feeds the Stuck rule (AJ-E405).
    #[must_use]
    pub fn last_output_ms(&self, session: &SessionId) -> Option<i64> {
        let children = self.children.lock().ok()?;
        let handle = children.get(session)?;
        let at = handle.last_output_ms.load(Ordering::Relaxed);
        (at > 0).then_some(at)
    }

    /// The last `max_bytes` of captured output (for prompt and
    /// rate-limit signature detection; never the whole ring).
    #[must_use]
    pub fn output_tail(&self, session: &SessionId, max_bytes: usize) -> Option<String> {
        let output = self.output(session).ok()?;
        let start = output.len().saturating_sub(max_bytes);
        Some(String::from_utf8_lossy(&output[start..]).into_owned())
    }

    /// Drains sessions with fresh output since the last call — the
    /// event-driven recovery signal (<300 ms, Req.5).
    pub fn drain_activity(&self) -> Vec<SessionId> {
        let Ok(receiver) = self.activity_rx.lock() else {
            return Vec::new();
        };
        let mut drained = Vec::new();
        while let Ok(session) = receiver.try_recv() {
            if drained.last() != Some(&session) {
                drained.push(session);
            }
        }
        drained
    }

    /// Batched live stats for every supervised session, read from ONE
    /// already-refreshed process table (AJ-E401: the read model must not
    /// trigger a table refresh per row). The caller owns the [`sysinfo`]
    /// table — via [`LiveStatsCollector`] — so CPU% deltas persist
    /// between ticks. Finished sessions report definitive zeros; a live
    /// root sums CPU/RSS over the root process and its descendants
    /// (children counted, the root itself not).
    ///
    /// CPU% is only meaningful when the table has been refreshed at least
    /// twice; [`LiveStatsCollector::refresh`] masks the first sample.
    #[must_use]
    pub fn live_stats(&self, system: &sysinfo::System) -> HashMap<SessionId, LiveProcessStats> {
        let parents = parents_map(system);
        self.live_roots()
            .into_iter()
            .map(|(session, root, finished)| {
                let stats = if finished {
                    LiveProcessStats::dead()
                } else {
                    tree_stats(system, &parents, root)
                };
                (session, stats)
            })
            .collect()
    }

    /// Supervised roots as (session, root pid, finished) — the shared
    /// enumeration for the collector and the read model.
    #[must_use]
    pub fn live_roots(&self) -> Vec<(SessionId, u32, bool)> {
        self.children
            .lock()
            .map(|children| {
                children
                    .iter()
                    .map(|(session, handle)| {
                        (
                            session.clone(),
                            handle.pid,
                            handle.finished.load(Ordering::Relaxed),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Feeds keyboard input to a PTY session's process.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::UnknownSession`] when the id is not
    /// supervised, [`SupervisorError::NotATty`] for piped sessions.
    pub fn write_input(&self, session: &SessionId, bytes: &[u8]) -> Result<(), SupervisorError> {
        let handle = self.child(session)?;
        let pty = handle.pty.as_ref().ok_or(SupervisorError::NotATty {
            session: session.clone(),
        })?;
        let mut pty = pty
            .lock()
            .map_err(|_| SupervisorError::UnknownSession(session.clone()))?;
        pty.writer
            .write_all(bytes)
            .and_then(|()| pty.writer.flush())
            .map_err(SupervisorError::Io)
    }

    /// Resizes a PTY session (cols/rows in character cells).
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::UnknownSession`] when the id is not
    /// supervised, [`SupervisorError::NotATty`] for piped sessions.
    pub fn resize_pty(
        &self,
        session: &SessionId,
        cols: u16,
        rows: u16,
    ) -> Result<(), SupervisorError> {
        let handle = self.child(session)?;
        let pty = handle.pty.as_ref().ok_or(SupervisorError::NotATty {
            session: session.clone(),
        })?;
        let pty = pty
            .lock()
            .map_err(|_| SupervisorError::UnknownSession(session.clone()))?;
        pty.master
            .resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SupervisorError::Io(std::io::Error::other(e.to_string())))
    }

    /// Subscribes to this session's live output stream (each chunk as
    /// written). Snapshot-then-tap is race-free: the ring forwards every
    /// byte to listeners, so a snapshot followed by a tap yields the
    /// stream exactly once from the snapshot point onward.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::UnknownSession`] when the id is not
    /// supervised.
    pub fn tap(
        &self,
        session: &SessionId,
    ) -> Result<std::sync::mpsc::Receiver<Vec<u8>>, SupervisorError> {
        let handle = self.child(session)?;
        let mut ring = handle
            .ring
            .lock()
            .map_err(|_| SupervisorError::UnknownSession(session.clone()))?;
        let (tx, rx) = std::sync::mpsc::channel();
        ring.add_listener(tx);
        Ok(rx)
    }

    /// Sessions from previous app runs whose processes are still alive
    /// (journal adoption; spec §5.3 crash re-adoption).
    pub fn alive_adopted_sessions(&self) -> Vec<SessionId> {
        self.journal
            .as_ref()
            .and_then(|journal| journal.adopt_orphans().ok())
            .map(|adopted| adopted.into_iter().map(|process| process.session).collect())
            .unwrap_or_default()
    }

    /// The supervision journal, when one is open (descendant snapshots
    /// and kill audit writes, AJ-E410).
    #[must_use]
    pub fn journal_ref(&self) -> Option<&Journal> {
        self.journal.as_ref()
    }

    /// True when the session was stopped through [`Supervisor::stop`]
    /// (user intent — not a crash), regardless of how the exit surfaces.
    #[must_use]
    pub fn was_stopped(&self, session: &SessionId) -> bool {
        self.user_stopped
            .lock()
            .is_ok_and(|stopped| stopped.iter().any(|id| id == session))
    }

    /// True when this app's supervisor owns a handle for the session
    /// (live or finished — adopted-from-journal sessions are not).
    #[must_use]
    pub fn is_supervised(&self, session: &SessionId) -> bool {
        self.children
            .lock()
            .is_ok_and(|children| children.contains_key(session))
    }

    /// True when the session's process has exited.
    #[must_use]
    pub fn is_finished(&self, session: &SessionId) -> bool {
        self.children
            .lock()
            .ok()
            .and_then(|children| {
                children
                    .get(session)
                    .map(|h| h.finished.load(Ordering::Acquire))
            })
            .unwrap_or(true)
    }

    fn child(&self, session: &SessionId) -> Result<Arc<ChildHandle>, SupervisorError> {
        self.children
            .lock()
            .map_err(|_| SupervisorError::UnknownSession(session.clone()))?
            .get(session)
            .cloned()
            .ok_or_else(|| SupervisorError::UnknownSession(session.clone()))
    }

    /// Non-blocking event read.
    #[must_use]
    pub fn try_recv(&self) -> Option<LifecycleEvent> {
        self.events_rx.lock().ok().and_then(|rx| rx.try_recv().ok())
    }

    /// Blocks up to `timeout` for the next lifecycle event.
    #[must_use]
    pub fn recv_timeout(&self, timeout: Duration) -> Option<LifecycleEvent> {
        self.events_rx
            .lock()
            .ok()
            .and_then(|rx| rx.recv_timeout(timeout).ok())
    }
}

fn spawn_reader(
    reader: Option<impl std::io::Read + Send + 'static>,
    ring: Arc<Mutex<OutputRing>>,
    last_output_ms: Arc<std::sync::atomic::AtomicI64>,
    session: SessionId,
    activity: std::sync::mpsc::SyncSender<SessionId>,
) {
    let Some(mut reader) = reader else {
        return;
    };
    std::thread::spawn(move || {
        let mut chunk = [0_u8; 4096];
        let mut last_notify = 0_i64;
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let now = Timestamp::now().millis();
                    last_output_ms.store(now, Ordering::Relaxed);
                    if let Ok(mut ring) = ring.lock() {
                        ring.push(&chunk[..n]);
                    }
                    // Throttle activity signals to ~1/250 ms per session —
                    // recovery latency stays <300 ms without spam.
                    if now - last_notify >= 250 {
                        last_notify = now;
                        if activity.send(session.clone()).is_err() {
                            break; // receiver gone
                        }
                    }
                }
            }
        }
    });
}

/// Helper: lock a mutex-wrapped child without unwrap (clippy: no unwrap
/// in production paths).
fn child_lock(
    child: &Arc<Mutex<std::process::Child>>,
) -> std::sync::MutexGuard<'_, std::process::Child> {
    // A poisoned child mutex means the wait thread panicked; treat as
    // no-op by leaking a dummy is impossible — fall back to blocking
    // lock, mapping poison to an empty guard via unwrap_or_else.
    child
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn emit_exit(
    session: &SessionId,
    pid: u32,
    kind: ExitKind,
    done: &AtomicBool,
    journal: Option<&Journal>,
    ledger: Option<&crate::tree::SharedLedger>,
    events: &std::sync::mpsc::Sender<LifecycleEvent>,
) {
    done.store(true, Ordering::Release);
    if let Some(journal) = journal {
        let _ = journal.record_exited(session, pid, kind);
    }
    // The ledger exit patch (AJ-E501): roots are reaped by us; a
    // failure degrades health on the next supervisor touch (the flag
    // lives on the shared supervisor, mirrored here best-effort).
    if let Some(ledger) = ledger {
        let exit_kind = match kind {
            ExitKind::Code(code) => format!("code:{code}"),
            ExitKind::Signaled(signal) => {
                format!(
                    "signal:{}",
                    signal.map_or_else(|| "unknown".into(), |s| s.to_string())
                )
            }
        };
        if ledger
            .mark_exited(pid, &ledger.boot_id(), &exit_kind, true)
            .is_err()
        {
            eprintln!("ai-juggler: process ledger degraded at exit patch (pid {pid})");
        }
    }
    let _ = events.send(LifecycleEvent::Exited {
        session: session.clone(),
        pid,
        kind,
    });
}

fn exit_kind_std(status: std::process::ExitStatus) -> ExitKind {
    match status.code() {
        Some(code) => ExitKind::Code(code),
        None => signaled_kind(status),
    }
}

#[cfg(unix)]
fn signaled_kind(status: std::process::ExitStatus) -> ExitKind {
    use std::os::unix::process::ExitStatusExt;
    ExitKind::Signaled(status.signal())
}

#[cfg(not(unix))]
fn signaled_kind(_status: std::process::ExitStatus) -> ExitKind {
    ExitKind::Signaled(None)
}

#[cfg(unix)]
fn send_terminate(pid: u32) {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;
    let nix_pid = Pid::from_raw(i32::try_from(pid).unwrap_or_default());
    let _ = kill(nix_pid, Some(Signal::SIGTERM));
}

#[cfg(not(unix))]
fn send_terminate(_pid: u32) {
    // No SIGTERM on this platform; the stop ladder goes straight to kill.
}

fn descendants_of(root: u32) -> Vec<u32> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let mut parents: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, process) in system.processes() {
        if let Some(parent) = process.parent() {
            parents
                .entry(parent.as_u32())
                .or_default()
                .push(pid.as_u32());
        }
    }
    let mut found = Vec::new();
    let mut queue: std::collections::VecDeque<u32> =
        parents.get(&root).cloned().into_iter().flatten().collect();
    while let Some(pid) = queue.pop_front() {
        found.push(pid);
        queue.extend(parents.get(&pid).cloned().into_iter().flatten());
    }
    found
}

/// Sums CPU/RSS over the root process and its descendants from a table
/// that has already been refreshed by the caller (no refresh here —
/// `Supervisor::live_stats` batches many sessions over one table).
fn tree_stats(
    system: &sysinfo::System,
    parents: &HashMap<u32, Vec<u32>>,
    root: u32,
) -> LiveProcessStats {
    let mut cpu = 0.0_f64;
    let mut rss = 0_u64;
    let mut child_count = 0_u32;
    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<u32> = std::iter::once(root).collect();
    while let Some(pid) = queue.pop_front() {
        if !seen.insert(pid) {
            continue; // ppid cycles must not loop forever
        }
        if let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) {
            cpu += f64::from(process.cpu_usage());
            rss += process.memory();
            if pid != root {
                child_count += 1;
            }
        }
        queue.extend(parents.get(&pid).cloned().into_iter().flatten());
    }
    LiveProcessStats {
        child_count,
        cpu_pct: Some(cpu),
        rss_bytes: rss,
    }
}

/// Path-qualified helper for callers constructing journal locations.
#[must_use]
pub fn default_journal_path(state_dir: &Path) -> PathBuf {
    state_dir.join("supervision.jsonl")
}
