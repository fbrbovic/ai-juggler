//! The process supervisor: spawns harness sessions (PTY or piped), captures
//! output, runs the stop ladder, and journals lifecycle for crash re-adoption
//! (spec §5.1, §5.3).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use juggler_adapter::LaunchSpec;
use juggler_core::model::SessionId;

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

struct ChildHandle {
    pid: u32,
    ring: Arc<Mutex<OutputRing>>,
    finished: Arc<AtomicBool>,
    force_kill: Arc<dyn Fn() + Send + Sync>,
}

/// Owns supervised sessions for one AI Juggler process.
///
/// Children **survive** the supervisor (and the app) by design: dropping it
/// neither signals nor waits. Re-adoption after a restart happens via
/// [`Journal::adopt_orphans`].
pub struct Supervisor {
    journal: Option<Journal>,
    children: Mutex<HashMap<SessionId, Arc<ChildHandle>>>,
    events_tx: std::sync::mpsc::Sender<LifecycleEvent>,
    events_rx: Mutex<std::sync::mpsc::Receiver<LifecycleEvent>>,
}

impl Default for Supervisor {
    fn default() -> Self {
        let (events_tx, events_rx) = std::sync::mpsc::channel();
        Self {
            journal: None,
            children: Mutex::new(HashMap::new()),
            events_tx,
            events_rx: Mutex::new(events_rx),
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
    ) -> Result<SpawnedProcess, SupervisorError> {
        let spawned = if spec.needs_pty {
            self.spawn_pty(session, spec)?
        } else {
            self.spawn_piped(session, spec)?
        };
        if let Some(journal) = &self.journal {
            journal.record_spawned(session, spawned.pid, &spec.program)?;
        }
        let _ = self.events_tx.send(LifecycleEvent::Started {
            session: session.clone(),
            pid: spawned.pid,
        });
        Ok(spawned)
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
        let mut child = command.spawn().map_err(|e| SupervisorError::Spawn {
            program: spec.program.clone(),
            message: e.to_string(),
        })?;
        let pid = child.id();
        let ring = Arc::new(Mutex::new(OutputRing::new(DEFAULT_RING_CAPACITY)));
        spawn_reader(child.stdout.take(), Arc::clone(&ring));
        spawn_reader(child.stderr.take(), Arc::clone(&ring));

        let child = Arc::new(Mutex::new(child));
        let finished = Arc::new(AtomicBool::new(false));
        let kill_child = Arc::clone(&child);
        let force_kill: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = kill_child.lock().map(|mut c| c.kill());
        });

        let session_for_thread = session.clone();
        let journal = self.journal.clone();
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
                    &events,
                );
            } else {
                done.store(true, Ordering::Release);
            }
        });

        self.register(session.clone(), pid, ring, finished, force_kill);
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
        if let Ok(reader) = pair.master.try_clone_reader() {
            spawn_reader(Some(reader), Arc::clone(&ring));
        }
        drop(pair.slave);

        let finished = Arc::new(AtomicBool::new(false));
        let force_kill: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = killer.lock().map(|mut k| k.kill());
        });

        let session_for_thread = session.clone();
        let journal = self.journal.clone();
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
                    &events,
                );
            } else {
                done.store(true, Ordering::Release);
            }
        });

        self.register(session.clone(), pid, ring, finished, force_kill);
        Ok(SpawnedProcess {
            session: session.clone(),
            pid,
        })
    }

    #[allow(clippy::needless_pass_by_value)] // Arc handles are moved intentionally
    fn register(
        &self,
        session: SessionId,
        pid: u32,
        ring: Arc<Mutex<OutputRing>>,
        finished: Arc<AtomicBool>,
        force_kill: Arc<dyn Fn() + Send + Sync>,
    ) {
        let handle = Arc::new(ChildHandle {
            pid,
            ring,
            finished,
            force_kill,
        });
        if let Ok(mut children) = self.children.lock() {
            children.insert(session, handle);
        }
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

fn spawn_reader(reader: Option<impl std::io::Read + Send + 'static>, ring: Arc<Mutex<OutputRing>>) {
    let Some(mut reader) = reader else {
        return;
    };
    std::thread::spawn(move || {
        let mut chunk = [0_u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut ring) = ring.lock() {
                        ring.push(&chunk[..n]);
                    }
                }
            }
        }
    });
}

fn emit_exit(
    session: &SessionId,
    pid: u32,
    kind: ExitKind,
    done: &AtomicBool,
    journal: Option<&Journal>,
    events: &std::sync::mpsc::Sender<LifecycleEvent>,
) {
    done.store(true, Ordering::Release);
    if let Some(journal) = journal {
        let _ = journal.record_exited(session, pid, kind);
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

/// Path-qualified helper for callers constructing journal locations.
#[must_use]
pub fn default_journal_path(state_dir: &Path) -> PathBuf {
    state_dir.join("supervision.jsonl")
}
