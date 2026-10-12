//! Environment scan + config import (M1-3b, ADR-0006): the onboarding
//! promise — find every existing Claude setup and import it read-only.
//!
//! Login probes are injected closures so tests are deterministic and never
//! touch the real keychain.

// Test crates are pragmatic by policy (QUALITY.md §2: tests excepted).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic
)]

use std::path::Path;

use juggler_adapter::HarnessAdapter;
use juggler_adapter::environment::{InstallOrigin, LoginProbe};
use juggler_adapters::ClaudeCodeAdapter;
use juggler_adapters::claude_environment::{
    MAX_IMPORT_BYTES, import_with_cap, scan_claude_environment,
};

fn logged_in_probe(path: &Path) -> LoginProbe {
    // Deterministic stand-in for the metadata-only keychain probe: any
    // dir containing `.credentials.json` counts as signed in.
    if path.join(".credentials.json").is_file() {
        LoginProbe::LoggedIn
    } else {
        LoginProbe::LoggedOut
    }
}

fn logged_out_probe(_path: &Path) -> LoginProbe {
    LoginProbe::LoggedOut
}

/// Builds a realistic existing setup: settings, credentials, one project
/// transcript, plus noise that must be skipped on import.
fn existing_setup(home: &Path, name: &str) -> std::path::PathBuf {
    let dir = home.join(name);
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
    std::fs::create_dir_all(dir.join("statsig")).unwrap();
    std::fs::write(dir.join("statsig/cache.db"), "noise").unwrap();
    dir
}

#[test]
fn scan_finds_default_env_override_and_manual_patterns() {
    let work = tempfile::tempdir().unwrap();
    let home = work.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    let default = existing_setup(&home, ".claude");
    let manual = existing_setup(&home, ".claude-work");
    existing_setup(&home, ".claude-personal");

    // Noise the scan must ignore: the vendor's state FILE, an empty
    // look-alike dir, and an unrelated dotdir.
    std::fs::write(home.join(".claude.json"), "{}").unwrap();
    std::fs::create_dir_all(home.join(".claude-empty")).unwrap();
    std::fs::create_dir_all(home.join(".other")).unwrap();

    let env_override = existing_setup(work.path(), "managed-elsewhere");
    let found = scan_claude_environment(&home, Some(env_override.as_path()), &logged_in_probe);

    let paths: Vec<String> = found
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        paths.contains(&".claude".to_string()),
        "default must be found: {paths:?}"
    );
    assert!(paths.contains(&".claude-work".to_string()), "{paths:?}");
    assert!(paths.contains(&".claude-personal".to_string()), "{paths:?}");
    assert!(
        paths.contains(&"managed-elsewhere".to_string()),
        "env override must be found: {paths:?}"
    );
    assert_eq!(found.len(), 4, "noise filtered: {paths:?}");

    let by_origin = |origin: InstallOrigin| found.iter().filter(|f| f.origin == origin).count();
    assert_eq!(by_origin(InstallOrigin::VendorDefault), 1);
    assert_eq!(by_origin(InstallOrigin::EnvOverride), 1);
    assert_eq!(by_origin(InstallOrigin::HomePattern), 2);

    // Default sorts first; every found install has user-facing notes and
    // login evidence.
    assert_eq!(found[0].origin, InstallOrigin::VendorDefault);
    assert_eq!(found[0].path, default.canonicalize().unwrap());
    assert_eq!(found[0].login, LoginProbe::LoggedIn);
    assert!(!found[0].notes.is_empty());
    assert!(found[0].bytes > 0);
    assert!(found[0].last_used_secs.is_some());
    assert!(!found[0].managed);
    let _ = manual;
}

#[test]
fn scan_dedupes_when_env_override_points_at_default() {
    let work = tempfile::tempdir().unwrap();
    let home = work.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    existing_setup(&home, ".claude");

    let same = home.join(".claude");
    let found = scan_claude_environment(&home, Some(same.as_path()), &logged_in_probe);
    assert_eq!(found.len(), 1, "same path via two candidates → one entry");
    assert_eq!(found[0].origin, InstallOrigin::VendorDefault);
}

#[test]
fn scan_marks_juggler_managed_dirs() {
    let work = tempfile::tempdir().unwrap();
    let home = work.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let dir = existing_setup(&home, ".claude");

    // A dir prepared through the adapter carries the managed marker.
    let adapter = ClaudeCodeAdapter::new();
    adapter.prepare_account(&dir).unwrap();

    let found = scan_claude_environment(&home, None, &logged_out_probe);
    assert_eq!(found.len(), 1);
    assert!(found[0].managed);
    assert!(
        found[0]
            .notes
            .iter()
            .any(|note| note.contains("Already managed"))
    );
}

#[test]
fn import_copies_state_skips_caches_and_never_touches_source() {
    let work = tempfile::tempdir().unwrap();
    let home = work.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let source = existing_setup(&home, ".claude");
    let source_snapshot = file_tree(&source);
    let dest = work.path().join("app-data/accounts/claude-code/acc-1");

    let outcome = import_with_cap(&source, &dest, &logged_in_probe, MAX_IMPORT_BYTES).unwrap();

    // Settings, credentials, and transcripts carried over.
    assert!(dest.join("settings.json").is_file());
    assert!(dest.join(".credentials.json").is_file());
    assert!(
        dest.join("projects/-users-dev-app/session-1.jsonl")
            .is_file()
    );
    // Noise skipped, with recorded reasons.
    assert!(!dest.join("shell-snapshots").exists());
    assert!(!dest.join("statsig").exists());
    assert!(
        outcome
            .skipped
            .iter()
            .any(|s| s.contains("shell-snapshots"))
    );
    // The managed marker marks the copy as ours.
    assert!(juggler_adapters::claude_code::is_managed_dir(&dest));
    assert_eq!(outcome.files_copied, 3);
    assert!(outcome.bytes_copied > 0);
    // Login carried (file-based credentials) with the matching note.
    assert_eq!(outcome.login, LoginProbe::LoggedIn);
    assert_eq!(
        outcome.login_note.as_deref(),
        Some("Login carried over with the import.")
    );

    // The source is byte-identical and unmarked — read-only guarantee.
    assert_eq!(file_tree(&source), source_snapshot);
    assert!(!juggler_adapters::claude_code::is_managed_dir(&source));
}

#[test]
fn import_explains_keychain_locked_logins() {
    let work = tempfile::tempdir().unwrap();
    let home = work.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    // A macOS-style setup: logged in via keychain (probe says LoggedIn for
    // the source) but the copied dir has no credentials file.
    let source = existing_setup(&home, ".claude");
    std::fs::remove_file(source.join(".credentials.json")).unwrap();
    let dest = work.path().join("dest");

    // Source logged in (simulated keychain), destination not.
    let probe = |path: &Path| {
        if path == source.canonicalize().unwrap() {
            LoginProbe::LoggedIn
        } else {
            LoginProbe::LoggedOut
        }
    };
    let outcome = import_with_cap(&source, &dest, &probe, MAX_IMPORT_BYTES).unwrap();
    assert_eq!(outcome.login, LoginProbe::LoggedOut);
    let note = outcome.login_note.expect("keychain note");
    assert!(note.contains("Keychain"), "explains the real cause: {note}");
    assert!(note.contains("Sign in once"), "gives the next step: {note}");
}

#[test]
fn import_refuses_implausible_managed_and_existing() {
    let work = tempfile::tempdir().unwrap();
    let home = work.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    // Not a config dir: no evidence files.
    let bare = home.join(".claude-noise");
    std::fs::create_dir_all(&bare).unwrap();
    let err = import_with_cap(
        &bare,
        &work.path().join("d1"),
        &logged_out_probe,
        MAX_IMPORT_BYTES,
    )
    .unwrap_err();
    assert!(err.to_string().contains("does not look like"));

    // Already managed: refuse to import our own dir.
    let managed = existing_setup(&home, ".claude");
    ClaudeCodeAdapter::new().prepare_account(&managed).unwrap();
    let err = import_with_cap(
        &managed,
        &work.path().join("d2"),
        &logged_out_probe,
        MAX_IMPORT_BYTES,
    )
    .unwrap_err();
    assert!(err.to_string().contains("already managed"));

    // Destination already exists.
    let source = existing_setup(&home, ".claude-second");
    let dest = work.path().join("d3");
    std::fs::create_dir_all(&dest).unwrap();
    let err = import_with_cap(&source, &dest, &logged_out_probe, MAX_IMPORT_BYTES).unwrap_err();
    assert!(err.to_string().contains("destination already exists"));
}

#[test]
fn import_enforces_ceiling_before_writing() {
    let work = tempfile::tempdir().unwrap();
    let home = work.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let source = existing_setup(&home, ".claude");
    let dest = work.path().join("dest");

    // Ceiling smaller than the tree: fail with the plain-language message,
    // and write nothing.
    let err = import_with_cap(&source, &dest, &logged_out_probe, 1).unwrap_err();
    assert!(err.to_string().contains("import ceiling"), "{err}");
    assert!(!dest.exists(), "no partial import on ceiling breach");
}

/// Path→size snapshot for read-only verification of a source tree.
fn file_tree(root: &Path) -> Vec<(String, u64)> {
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
