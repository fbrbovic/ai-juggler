//! M1-3b integration: smart onboarding — the environment scan feeds the
//! import, the import yields an immediately launchable account, and the
//! source setup is never modified (ADR-0006 guardrails).

// Test crates are pragmatic by policy (QUALITY.md §2: tests excepted).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic
)]
#![cfg(unix)]

use std::path::Path;
use std::sync::Arc;

use juggler_adapter::AdapterRegistry;
use juggler_adapters::{CLAUDE_CODE_ID, ClaudeCodeAdapter};
use juggler_app::commands;
use juggler_app::state::AppState;

/// A `claude` stub good enough for login state + launch assertions: writes
/// a transcript under its isolated config dir and exits.
fn claude_stub(dir: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("claude-stub");
    std::fs::write(
        &path,
        "#!/bin/sh\n\
         mkdir -p \"$CLAUDE_CONFIG_DIR/projects/p\"\n\
         printf '%s' \"$CLAUDE_CONFIG_DIR\" > \"$CLAUDE_CONFIG_DIR/projects/p/s1.jsonl\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn app_state(dir: &Path, stub: &Path) -> Arc<AppState> {
    let mut registry = AdapterRegistry::new();
    registry.register(Arc::new(ClaudeCodeAdapter::with_bin(stub)));
    Arc::new(AppState::init_with_registry(dir.join("app-data"), registry).unwrap())
}

/// A pre-existing user setup outside the app data dir: logged in (file
/// credentials), one session transcript, plus cache noise.
fn existing_setup(root: &Path) -> std::path::PathBuf {
    let dir = root.join("existing-claude");
    std::fs::create_dir_all(dir.join("projects/-users-dev-app")).unwrap();
    std::fs::write(dir.join("settings.json"), r#"{"model":"sonnet"}"#).unwrap();
    std::fs::write(dir.join(".credentials.json"), "{}").unwrap();
    std::fs::write(
        dir.join("projects/-users-dev-app/session-1.jsonl"),
        "{\"type\":\"user\"}\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("shell-snapshots")).unwrap();
    std::fs::write(dir.join("shell-snapshots/x.sh"), "noise").unwrap();
    dir
}

#[test]
fn import_makes_an_existing_setup_launchable_without_touching_it() {
    let work = tempfile::tempdir().unwrap();
    let stub = claude_stub(work.path());
    let state = app_state(work.path(), &stub);
    let source = existing_setup(work.path());
    let source_snapshot = snapshot(&source);

    // Import through the command surface the wizard uses.
    let result = commands::account_import_impl(
        &state,
        CLAUDE_CODE_ID,
        source.to_str().unwrap(),
        "Existing account",
        Some("pro".into()),
    )
    .unwrap();

    // The account exists, is logged in, and the settings/history carried.
    assert_eq!(result.login, "logged_in");
    assert_eq!(
        result.login_note.as_deref(),
        Some("Login carried over with the import.")
    );
    assert!(result.files_copied >= 3, "{}", result.files_copied);
    assert!(result.skipped.iter().any(|s| s.contains("shell-snapshots")));
    let account = result.account;
    assert_eq!(account.label, "Existing account");
    assert_eq!(account.plan.as_deref(), Some("pro"));

    // A default profile makes it launchable in one click.
    let storage = state.storage.lock().unwrap();
    let profiles = storage
        .profiles()
        .list_by_account(&juggler_core::model::AccountId::new(account.id.clone()))
        .unwrap();
    drop(storage);
    let main = profiles.first().expect("default profile created on import");
    assert_eq!(main.name, "Main");

    // Launch through the supervisor: the isolated copy behaves like the
    // original setup (stub writes into ITS config dir).
    let launched = commands::launch_profile(&state, main.id.as_str(), None).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut exited = false;
    while std::time::Instant::now() < deadline {
        if let Some(juggler_supervisor::LifecycleEvent::Exited { .. }) = state
            .supervisor
            .recv_timeout(std::time::Duration::from_millis(200))
        {
            exited = true;
            break;
        }
    }
    assert!(exited, "imported account's session must run and exit");
    let storage = state.storage.lock().unwrap();
    let sessions = storage.sessions().list(None).unwrap();
    drop(storage);
    assert!(
        sessions
            .iter()
            .any(|s| s.id.as_str() == launched.session_id)
    );

    // The source tree is byte-identical — the import is read-only.
    assert_eq!(snapshot(&source), source_snapshot);

    // Re-importing the same source still works (it was not marked), but
    // importing our own managed dir is refused.
    let state_dir = state
        .accounts_dir()
        .join(CLAUDE_CODE_ID)
        .join(account.id.as_str());
    let err = commands::account_import_impl(
        &state,
        CLAUDE_CODE_ID,
        state_dir.to_str().unwrap(),
        "Dup",
        None,
    )
    .unwrap_err();
    assert!(err.to_string().contains("already managed"));
}

#[test]
fn environment_scan_smoke_never_panics_on_a_real_home() {
    let work = tempfile::tempdir().unwrap();
    let stub = claude_stub(work.path());
    let state = app_state(work.path(), &stub);

    // Scans the developer's actual home: contents are machine-specific, so
    // assert only invariants — valid wire values, plausible paths.
    let found = commands::environment_scan_impl(&state);
    for install in &found {
        assert_eq!(install.harness, CLAUDE_CODE_ID);
        assert!(!install.path.is_empty());
        assert!(
            [
                "vendor-default",
                "env-override",
                "home-pattern",
                "manager-tool"
            ]
            .contains(&install.origin.as_str())
        );
        assert!(
            ["logged_in", "logged_out"].contains(&install.login.as_str()),
            "login wire value: {}",
            install.login
        );
        assert!(!install.notes.is_empty());
    }
}

/// Path→size snapshot of a tree (sorted) for read-only checks.
fn snapshot(root: &Path) -> Vec<(String, u64)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, u64)>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let meta = entry.metadata().unwrap();
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if meta.is_dir() {
                walk(&entry.path(), root, out);
            } else {
                out.push((rel, meta.len()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}
