//! Supervision journal: crash-safe JSONL record of spawns and exits used to
//! re-adopt surviving children after an AI Juggler restart (spec §5.3).

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use juggler_core::model::SessionId;

use crate::error::SupervisorError;
use crate::lifecycle::ExitKind;

/// A process found alive after a restart, reconstructed from the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptedProcess {
    /// The session the process belonged to.
    pub session: SessionId,
    /// OS process id (still running).
    pub pid: u32,
    /// The program that was spawned.
    pub program: String,
}

/// Append-only journal at a fixed path.
#[derive(Debug, Clone)]
pub struct Journal {
    path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Entry {
    Spawned {
        session: String,
        pid: u32,
        program: String,
        at: i64,
    },
    Exited {
        session: String,
        pid: u32,
        code: Option<i32>,
        signal: Option<i32>,
        at: i64,
    },
    /// AJ-E410: the descendant snapshot recorded at exit — the durable
    /// allow-list for orphan stops (kills only ever touch these PIDs).
    Descendants {
        session: String,
        root_pid: u32,
        clean: bool,
        rows: Vec<JournalDescendant>,
        at: i64,
    },
    /// AJ-E410 / D-08: the audit trail for every kill the app performs
    /// (manual orphan stops and opted-in Tier-1 auto-cleanup alike).
    Killed {
        session: String,
        pid: u32,
        trigger: String,
        outcome: String,
        at: i64,
    },
}

/// One recorded descendant row (journal form).
#[derive(Serialize, Deserialize)]
struct JournalDescendant {
    pid: u32,
    name: String,
}

#[derive(Default)]
struct Last {
    spawned: Option<AdoptedProcess>,
    exited: bool,
}

impl Journal {
    /// Opens (creating if needed) the journal at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Journal`] when the parent directory cannot
    /// be created or the file cannot be opened for append.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SupervisorError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SupervisorError::Journal {
                path: path.clone(),
                message: e.to_string(),
            })?;
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| SupervisorError::Journal {
                path: path.clone(),
                message: e.to_string(),
            })?;
        Ok(Self { path })
    }

    fn append(&self, entry: &Entry) -> Result<(), SupervisorError> {
        let line = serde_json::to_string(entry).map_err(|e| SupervisorError::Journal {
            path: self.path.clone(),
            message: e.to_string(),
        })?;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map_err(|e| SupervisorError::Journal {
                path: self.path.clone(),
                message: e.to_string(),
            })?;
        writeln!(file, "{line}").map_err(|e| SupervisorError::Journal {
            path: self.path.clone(),
            message: e.to_string(),
        })
    }

    /// Records that a process was spawned for `session`.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Journal`] on write failures.
    pub fn record_spawned(
        &self,
        session: &SessionId,
        pid: u32,
        program: &std::path::Path,
    ) -> Result<(), SupervisorError> {
        self.append(&Entry::Spawned {
            session: session.to_string(),
            pid,
            program: program.to_string_lossy().into_owned(),
            at: juggler_core::model::Timestamp::now().millis(),
        })
    }

    /// Records how a process ended.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Journal`] on write failures.
    pub fn record_exited(
        &self,
        session: &SessionId,
        pid: u32,
        kind: ExitKind,
    ) -> Result<(), SupervisorError> {
        let (code, signal) = match kind {
            ExitKind::Code(code) => (Some(code), None),
            ExitKind::Signaled(signal) => (None, signal),
        };
        self.append(&Entry::Exited {
            session: session.to_string(),
            pid,
            code,
            signal,
            at: juggler_core::model::Timestamp::now().millis(),
        })
    }

    /// Records the descendant snapshot taken at exit (AJ-E410) — the
    /// durable allow-list for orphan stops.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Journal`] on write failures.
    pub fn record_descendants(
        &self,
        session: &SessionId,
        root_pid: u32,
        clean: bool,
        rows: &[crate::orphans::DescendantRow],
    ) -> Result<(), SupervisorError> {
        self.append(&Entry::Descendants {
            session: session.to_string(),
            root_pid,
            clean,
            rows: rows
                .iter()
                .map(|row| JournalDescendant {
                    pid: row.pid,
                    name: row.name.clone(),
                })
                .collect(),
            at: juggler_core::model::Timestamp::now().millis(),
        })
    }

    /// Audit-logs one kill (D-08: every kill is journal-recorded — who
    /// (`trigger`: `manual` / `auto-tier1`), what, when, outcome).
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Journal`] on write failures.
    pub fn record_killed(
        &self,
        session: &SessionId,
        pid: u32,
        trigger: &str,
        outcome: &str,
    ) -> Result<(), SupervisorError> {
        self.append(&Entry::Killed {
            session: session.to_string(),
            pid,
            trigger: trigger.to_string(),
            outcome: outcome.to_string(),
            at: juggler_core::model::Timestamp::now().millis(),
        })
    }

    /// The journal's durable path (evidence/audit reads).
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Replays the journal and returns processes that were spawned without a
    /// recorded exit **and** are still alive. Zombies count as dead.
    ///
    /// Malformed lines are skipped (degrade, never crash).
    ///
    /// # Errors
    ///
    /// Returns [`SupervisorError::Journal`] when the file cannot be read.
    pub fn adopt_orphans(&self) -> Result<Vec<AdoptedProcess>, SupervisorError> {
        let content =
            std::fs::read_to_string(&self.path).map_err(|e| SupervisorError::Journal {
                path: self.path.clone(),
                message: e.to_string(),
            })?;
        let mut latest: HashMap<(String, u32), Last> = HashMap::new();
        for line in content.lines() {
            let Ok(entry) = serde_json::from_str::<Entry>(line) else {
                continue;
            };
            match entry {
                Entry::Spawned {
                    session,
                    pid,
                    program,
                    ..
                } => {
                    latest.insert(
                        (session.clone(), pid),
                        Last {
                            spawned: Some(AdoptedProcess {
                                session: SessionId::new(session),
                                pid,
                                program,
                            }),
                            exited: false,
                        },
                    );
                }
                Entry::Exited { session, pid, .. } => {
                    if let Some(last) = latest.get_mut(&(session, pid)) {
                        last.exited = true;
                    }
                }
                // Descendant snapshots and kill audit rows are not
                // adoption state — recorded for the allow-list/audit
                // readers, skipped by the replay.
                Entry::Descendants { .. } | Entry::Killed { .. } => {}
            }
        }
        let alive = alive_pids();
        Ok(latest
            .into_values()
            .filter_map(|last| {
                let process = last.spawned?;
                (!last.exited && alive.contains(&process.pid)).then_some(process)
            })
            .collect())
    }
}

/// One-shot snapshot of currently alive, non-zombie process ids.
fn alive_pids() -> std::collections::HashSet<u32> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .iter()
        .filter(|(_, process)| {
            !matches!(
                process.status(),
                sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead
            )
        })
        .map(|(pid, _)| pid.as_u32())
        .collect()
}
