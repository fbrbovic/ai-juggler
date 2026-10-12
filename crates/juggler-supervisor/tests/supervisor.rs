//! Supervisor skeleton integration tests (M0-5): spawn/capture/stop, kill →
//! journal, and crash re-adoption. Unix-only: tests shell out to /bin/sh.

// Test crates are pragmatic by policy (QUALITY.md §2: tests excepted).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic
)]
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use juggler_adapter::{EnvMap, LaunchSpec};
use juggler_core::model::SessionId;
use juggler_supervisor::{ExitKind, Journal, LifecycleEvent, Supervisor};

fn sh_spec(script: &str, cwd: &std::path::Path) -> LaunchSpec {
    LaunchSpec {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".into(), script.into()],
        env: EnvMap::from([("JUGGLER_TEST".to_string(), "1".to_string())]),
        cwd: cwd.to_path_buf(),
        needs_pty: false,
    }
}

fn wait_for_exit(supervisor: &Supervisor, session: &SessionId) -> LifecycleEvent {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if let Some(event @ LifecycleEvent::Exited { .. }) =
            supervisor.recv_timeout(Duration::from_millis(100))
        {
            assert_eq!(event.session(), session);
            return event;
        }
    }
    panic!("no exit event within deadline");
}

#[test]
fn spawn_capture_and_stop_cleanly() {
    let workdir = tempfile::tempdir().unwrap();
    let supervisor = Supervisor::new();
    let session = SessionId::new("s-clean");

    let spawned = supervisor
        .spawn(
            &session,
            &sh_spec("echo hello-juggler; sleep 60", workdir.path()),
        )
        .unwrap();
    assert_eq!(spawned.session, session);

    // Output is captured into the ring buffer.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let out = supervisor.output(&session).unwrap();
        if String::from_utf8_lossy(&out).contains("hello-juggler") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "output not captured: {:?}",
            String::from_utf8_lossy(&out)
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    // Stop ladder terminates within the grace window.
    supervisor.stop(&session, Duration::from_secs(5)).unwrap();
    let event = wait_for_exit(&supervisor, &session);
    let LifecycleEvent::Exited { kind, .. } = event else {
        unreachable!()
    };
    assert!(
        matches!(kind, ExitKind::Signaled(_) | ExitKind::Code(_)),
        "expected termination, got {kind:?}"
    );
}

#[test]
fn external_sigkill_is_journaled_as_exit() {
    let workdir = tempfile::tempdir().unwrap();
    let state = workdir.path().join("state");
    let supervisor = Supervisor::with_journal(&state).unwrap();
    let session = SessionId::new("s-killed");

    let spawned = supervisor
        .spawn(&session, &sh_spec("sleep 300", workdir.path()))
        .unwrap();

    // Kill -9 from outside (simulating the OS OOM killer).
    let status = std::process::Command::new("kill")
        .arg("-9")
        .arg(spawned.pid.to_string())
        .status()
        .unwrap();
    assert!(status.success());

    let event = wait_for_exit(&supervisor, &session);
    let LifecycleEvent::Exited { kind, .. } = event else {
        unreachable!()
    };
    assert!(matches!(kind, ExitKind::Signaled(Some(9))), "got {kind:?}");

    // The journal recorded the exit, so a later adoption will not pick it up.
    let orphans = Journal::open(state.join("supervision.jsonl"))
        .unwrap()
        .adopt_orphans()
        .unwrap();
    assert!(!orphans.iter().any(|o| o.session == session));
}

#[test]
fn children_survive_drop_and_are_readopted_from_journal() {
    let workdir = tempfile::tempdir().unwrap();
    let state = workdir.path().join("state");
    let session = SessionId::new("s-orphan");

    let pid = {
        let supervisor = Supervisor::with_journal(&state).unwrap();
        let spawned = supervisor
            .spawn(&session, &sh_spec("sleep 300", workdir.path()))
            .unwrap();
        spawned.pid
        // Supervisor dropped here; the child must survive (spec §5.3).
    };
    std::thread::sleep(Duration::from_millis(200));

    // "Restart": a fresh supervisor consults the journal.
    let orphans = Journal::open(state.join("supervision.jsonl"))
        .unwrap()
        .adopt_orphans()
        .unwrap();
    let adopted = orphans
        .iter()
        .find(|o| o.session == session)
        .unwrap_or_else(|| panic!("expected {session} to be adopted"));
    assert_eq!(adopted.pid, pid);
    assert!(adopted.program.contains("sh"));

    // Cleanup: kill the orphan so the test leaves nothing behind.
    let _ = std::process::Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .status();
}

#[test]
fn process_tree_descendants_are_tracked() {
    let workdir = tempfile::tempdir().unwrap();
    let supervisor = Supervisor::new();
    let session = SessionId::new("s-tree");

    // sh spawns a child; the supervisor must see the whole tree.
    supervisor
        .spawn(&session, &sh_spec("sleep 300 & sleep 300", workdir.path()))
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let descendants = supervisor.descendants(&session).unwrap();
    assert!(
        !descendants.is_empty(),
        "expected at least one descendant process"
    );

    supervisor.stop(&session, Duration::from_secs(5)).unwrap();
    wait_for_exit(&supervisor, &session);
}

#[test]
fn pty_spawn_captures_output() {
    let workdir = tempfile::tempdir().unwrap();
    let supervisor = Supervisor::new();
    let session = SessionId::new("s-pty");

    let spec = LaunchSpec {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".into(), "echo from-pty; exit 0".into()],
        env: BTreeMap::new(),
        cwd: workdir.path().to_path_buf(),
        needs_pty: true,
    };
    supervisor.spawn(&session, &spec).unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let out = supervisor.output(&session).unwrap();
        if String::from_utf8_lossy(&out).contains("from-pty") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "pty output not captured: {:?}",
            String::from_utf8_lossy(&out)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    wait_for_exit(&supervisor, &session);
}

#[test]
fn unknown_sessions_error_cleanly() {
    let supervisor = Supervisor::new();
    let ghost = SessionId::new("ghost");
    assert!(supervisor.output(&ghost).is_err());
    assert!(supervisor.stop(&ghost, Duration::from_secs(1)).is_err());
    assert!(supervisor.descendants(&ghost).is_err());
}

#[test]
fn pty_sessions_roundtrip_input_output_and_resize() {
    let workdir = tempfile::tempdir().unwrap();
    let supervisor = Supervisor::new();
    let session = SessionId::new("s-pty-cat");

    // cat under a PTY: everything we write is echoed back through the tap.
    let spec = LaunchSpec {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".into(), "cat".into()],
        env: EnvMap::new(),
        cwd: workdir.path().to_path_buf(),
        needs_pty: true,
    };
    supervisor.spawn(&session, &spec).unwrap();
    let tap = supervisor.tap(&session).unwrap();

    supervisor
        .write_input(&session, b"juggler-pty-roundtrip\n")
        .unwrap();
    let mut collected = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Ok(chunk) = tap.recv_timeout(Duration::from_millis(200)) {
            collected.extend_from_slice(&chunk);
            if collected.windows(21).any(|w| w == b"juggler-pty-roundtrip") {
                break;
            }
        }
    }
    assert!(
        collected.windows(21).any(|w| w == b"juggler-pty-roundtrip"),
        "typed input must stream back through the tap: {:?}",
        String::from_utf8_lossy(&collected)
    );

    // Resize is accepted (the kernel owns the winsize; no error means the
    // master is live).
    supervisor.resize_pty(&session, 120, 40).unwrap();

    // The ring snapshot contains the same stream (replay-after-reopen).
    supervisor.write_input(&session, b"tail-marker\n").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let snapshot = supervisor.output(&session).unwrap();
        if snapshot.windows(11).any(|w| w == b"tail-marker") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        supervisor
            .output(&session)
            .unwrap()
            .windows(11)
            .any(|w| w == b"tail-marker")
    );

    supervisor.stop(&session, Duration::from_secs(2)).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline && !supervisor.is_finished(&session) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(supervisor.is_finished(&session));

    // Piped sessions have no PTY: the typed error says so.
    let piped = SessionId::new("s-piped");
    supervisor
        .spawn(&piped, &sh_spec("true", workdir.path()))
        .unwrap();
    let err = supervisor.write_input(&piped, b"x").unwrap_err();
    assert!(err.to_string().contains("no pty"), "{err}");
}

#[test]
fn twenty_streaming_sessions_fan_out_without_deadlock() {
    // M1-4 perf floor: 20 concurrent PTY sessions each streaming output
    // through a tap; every stream must deliver within the deadline and
    // the supervisor must stay responsive (60 fps UI budget depends on
    // this path never wedging).
    let workdir = tempfile::tempdir().unwrap();
    let supervisor = Supervisor::new();
    let mut sessions = Vec::new();
    for i in 0..20 {
        let session = SessionId::new(format!("s-stream-{i}"));
        let spec = LaunchSpec {
            program: PathBuf::from("/bin/sh"),
            args: vec![
                "-c".into(),
                "i=0; while [ $i -lt 50 ]; do echo line-$i; i=$((i+1)); done".to_string(),
            ],
            env: EnvMap::new(),
            cwd: workdir.path().to_path_buf(),
            needs_pty: true,
        };
        supervisor.spawn(&session, &spec).unwrap();
        let tap = supervisor.tap(&session).unwrap();
        sessions.push((session, tap));
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    for (session, rx) in sessions {
        let mut tail = Vec::new();
        let mut got_last = false;
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(300)) {
                Ok(chunk) => {
                    tail.extend_from_slice(&chunk);
                    if tail.windows(7).any(|w| w == b"line-49") {
                        got_last = true;
                        break;
                    }
                }
                Err(_) if supervisor.is_finished(&session) => break,
                Err(_) => continue,
            }
        }
        assert!(
            got_last || tail.windows(7).any(|w| w == b"line-49"),
            "stream for {session} never delivered its final line"
        );
    }
}
