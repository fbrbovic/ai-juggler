//! AJ-E501 evidence: the persistent spawn ledger records root AND
//! helper rows for a real spawned tree (sh + 3 sleeps), survives the
//! supervisor going away (reopen the same file), and never misses a
//! helper after two reconciler ticks. Run with `--nocapture` to
//! capture the evidence log + row dump.

// Test crates are pragmatic by policy (QUALITY.md §2).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::pedantic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use juggler_adapter::LaunchSpec;
use juggler_core::model::{MachineId, SessionId};
use juggler_core::storage::{LedgerRow, ProcessLedgerRepo, STATE_EXITED, STATE_RUNNING};
use juggler_supervisor::Supervisor;
use juggler_supervisor::supervisor::LiveStatsCollector;
use juggler_supervisor::tree::{CoreLedgerStore, SpawnContext};

fn ctx() -> SpawnContext {
    SpawnContext {
        provider: "claude-code".into(),
        account_id: Some("acc-evidence".into()),
        project_id: Some("proj-evidence".into()),
        machine: MachineId::new("machine-evidence"),
        role: "root",
    }
}

fn sh_spec(script: &str, cwd: &std::path::Path) -> LaunchSpec {
    LaunchSpec {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: Default::default(),
        cwd: cwd.to_path_buf(),
        needs_pty: false,
    }
}

fn wait_for<F: FnMut() -> bool>(deadline_ms: u64, mut probe: F) -> bool {
    let deadline = Instant::now() + Duration::from_millis(deadline_ms);
    while Instant::now() < deadline {
        if probe() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn dump_rows(rows: &[LedgerRow]) {
    for row in rows {
        println!(
            "[ledger] pid={:?} role={} state={} depth={} exe={} acct={:?} proj={:?} machine={} started={:?} exit={:?}",
            row.pid,
            row.role,
            row.state,
            row.depth,
            row.exe_path,
            row.account_id,
            row.project_id,
            row.machine_id,
            row.started_at,
            row.exit_kind,
        );
    }
}

#[test]
fn ledger_records_root_and_helpers_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    // Evidence runs may pin a persistent DB (AIJ_E501_DB) so the
    // sqlite dump can be taken after the test.
    let db = std::env::var("AIJ_E501_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| dir.path().join("ledger-evidence.sqlite"));
    let repo = ProcessLedgerRepo::open(&db).unwrap();
    let store = Arc::new(CoreLedgerStore::new(repo));

    let supervisor = Supervisor::new().with_ledger(store.clone());
    let session = SessionId::new("sess-evidence");

    // Root + exactly three helpers that OUTLIVE the root's shell
    // (the vite-server-outlived-its-instance shape, pass2 §3).
    let spawned = supervisor
        .spawn(
            &session,
            &sh_spec("sleep 300 & sleep 300 & sleep 300 & wait", dir.path()),
            &ctx(),
        )
        .unwrap();
    println!("[ledger] root spawned pid={}", spawned.pid);

    // Reconciler ticks until the helpers land (full scans during
    // warmup; the throttle allows one batched upsert per 2 s).
    let mut collector = LiveStatsCollector::new();
    let base_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let mut tick_n = 0_i64;
    let landed = wait_for(10_000, || {
        tick_n += 1;
        collector.tick(&supervisor, base_ms + tick_n * 2_000);
        store
            .rows_for_session(&session)
            .map(|rows| rows.iter().filter(|r| r.role == "helper").count() >= 3)
            .unwrap_or(false)
    });
    assert!(
        landed,
        "helpers did not land in the ledger within the budget"
    );

    let rows = store.rows_for_session(&session).unwrap();
    dump_rows(&rows);
    let roots = rows.iter().filter(|r| r.role == "root").count();
    let helpers = rows.iter().filter(|r| r.role == "helper").count();
    assert_eq!(roots, 1, "exactly one root row");
    assert_eq!(helpers, 3, "all three helpers recorded — 0 missing");
    let root = rows.iter().find(|r| r.role == "root").unwrap();
    assert_eq!(root.state, STATE_RUNNING);
    assert!(root.started_at.is_some(), "OS start time patched at spawn");
    assert_eq!(root.provider, "claude-code");
    assert_eq!(root.account_id.as_deref(), Some("acc-evidence"));
    assert_eq!(root.project_id.as_deref(), Some("proj-evidence"));
    assert_eq!(root.machine_id, "machine-evidence");
    for helper in rows.iter().filter(|r| r.role == "helper") {
        assert_eq!(helper.machine_id, "machine-evidence", "D-13 on every row");
        assert_eq!(helper.root_pid, Some(spawned.pid));
        assert!(helper.started_at.is_some());
    }

    // Stop the root through the ladder (the helpers keep running —
    // the orphan case the ledger exists for), wait for the exit patch.
    supervisor.stop(&session, Duration::from_secs(5)).unwrap();
    let exited = wait_for(5_000, || {
        store
            .rows_for_session(&session)
            .unwrap()
            .iter()
            .any(|r| r.role == "root" && r.state == STATE_EXITED)
    });
    assert!(exited, "root exit patched in the ledger");

    // "App exit": drop the supervisor + store entirely, REOPEN the same
    // file — every row survives (reboot is the same reopen).
    drop(supervisor);
    drop(store);
    let repo = ProcessLedgerRepo::open(&db).unwrap();
    let reopened = repo.rows_for_session(session.as_str()).unwrap();
    dump_rows(&reopened);
    assert_eq!(
        reopened.len(),
        4,
        "root + 3 helpers persisted across reopen"
    );
    assert!(
        reopened
            .iter()
            .any(|r| r.role == "root" && r.state == STATE_EXITED)
    );
    assert_eq!(
        reopened.iter().filter(|r| r.state == STATE_RUNNING).count(),
        3,
        "the outlived helpers are still running rows — cleanable, not forgotten"
    );

    // Cleanup: the helpers are OURS (journal + ledger) — kill them via
    // the ladder-shaped direct stop so the test leaves nothing behind.
    for row in &reopened {
        if let Some(pid) = row.pid
            && row.role == "helper"
        {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(i32::try_from(pid).unwrap()),
                nix::sys::signal::Signal::SIGTERM,
            );
        }
    }
}
