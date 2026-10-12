//! Round-trip property tests and edge cases for the storage layer (M0-3).

// Test crates are pragmatic by policy (QUALITY.md §2: tests excepted).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic
)]

use std::path::PathBuf;

use juggler_core::model::{
    Account, AccountId, Confidence, HarnessId, MachineId, PlanTier, Profile, ProfileId,
    QuotaWindow, SessionId, SessionRecord, SessionStatus, SettingsOverlay, Timestamp, TokenCounts,
    TranscriptRef, UsageSnapshot,
};
use juggler_core::storage::Storage;
use juggler_core::{CoreError, fs};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// strategies

fn id_str() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,9}"
}

fn label_str() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ._-]{1,12}"
}

fn path_str() -> impl Strategy<Value = String> {
    ("[a-z]{1,6}").prop_map(|segment| format!("/tmp/ai-juggler/{segment}"))
}

fn arb_account() -> impl Strategy<Value = Account> {
    (
        id_str(),
        id_str(),
        label_str(),
        proptest::option::of(label_str()),
        path_str(),
    )
        .prop_map(|(id, harness, label, plan, state_dir)| Account {
            id: AccountId::new(id),
            harness: HarnessId::new(harness),
            label,
            plan: plan.map(PlanTier::new),
            state_dir: PathBuf::from(state_dir),
        })
}

fn arb_overlay() -> impl Strategy<Value = SettingsOverlay> {
    (
        proptest::option::of(id_str()),
        proptest::collection::btree_map("[a-z]{1,5}", "[a-z0-9]{1,5}", 0..4),
    )
        .prop_map(|(model, env)| SettingsOverlay {
            model: model.map(|m| format!("model-{m}")),
            env,
        })
}

/// Epoch-millis range: wide but bounded, exercising sign handling.
fn millis_range() -> std::ops::Range<i64> {
    -(10_i64.pow(12))..10_i64.pow(12)
}

fn arb_usage() -> impl Strategy<Value = UsageSnapshot> {
    let tokens = (
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
    )
        .prop_map(
            |(input, output, cache_read, cache_write, reasoning)| TokenCounts {
                input,
                output,
                cache_read,
                cache_write,
                reasoning,
            },
        );
    let window =
        (0.0_f64..100.0, proptest::option::of(millis_range())).prop_map(|(used, resets)| {
            QuotaWindow {
                used_percentage: used,
                resets_at: resets.map(Timestamp::from_millis),
            }
        });
    (tokens, proptest::option::of(window), 0_usize..3).prop_map(|(tokens, window, c)| {
        let confidence = match c {
            0 => Confidence::Measured,
            1 => Confidence::Computed,
            _ => Confidence::Estimated,
        };
        UsageSnapshot {
            tokens,
            window,
            confidence,
        }
    })
}

fn arb_session(profile: ProfileId) -> impl Strategy<Value = SessionRecord> {
    (
        id_str(),
        proptest::option::of(id_str()),
        id_str(),
        path_str(),
        0_usize..8,
        millis_range(),
        proptest::option::of(millis_range()),
        arb_usage(),
        proptest::option::of((path_str(), proptest::option::of(id_str()))),
    )
        .prop_map(
            move |(id, native, machine, cwd, status, started, activity, usage, tr)| SessionRecord {
                id: SessionId::new(id),
                native_id: native,
                profile: profile.clone(),
                machine: MachineId::new(machine),
                cwd: PathBuf::from(cwd),
                status: SessionStatus::all()[status],
                started_at: Timestamp::from_millis(started),
                last_activity: activity.map(Timestamp::from_millis),
                usage,
                transcript_ref: tr.map(|(path, native)| TranscriptRef {
                    path: PathBuf::from(path),
                    native_id: native,
                }),
            },
        )
}

/// Field-wise session equality. The single float field
/// (`QuotaWindow::used_percentage`) is compared with a 1e-9 tolerance: the
/// ecosystem's default JSON float parser is ulp-lossy for rare f64 values,
/// and sub-nanopercent differences are meaningless for quota display.
fn sessions_equivalent(a: &SessionRecord, b: &SessionRecord) -> bool {
    match (&a.usage.window, &b.usage.window) {
        (None, None) => {}
        (Some(wa), Some(wb)) => {
            let windows_ok = wa.resets_at == wb.resets_at
                && (wa.used_percentage - wb.used_percentage).abs() < 1e-9;
            if !windows_ok {
                return false;
            }
        }
        _ => return false,
    }
    a.id == b.id
        && a.native_id == b.native_id
        && a.profile == b.profile
        && a.machine == b.machine
        && a.cwd == b.cwd
        && a.status == b.status
        && a.started_at == b.started_at
        && a.last_activity == b.last_activity
        && a.usage.tokens == b.usage.tokens
        && a.usage.confidence == b.usage.confidence
        && a.transcript_ref == b.transcript_ref
}

/// A fixed parent chain so property tests only exercise the entity under test.
fn seed_chain(storage: &Storage) -> (Account, Profile) {
    let account = Account {
        id: AccountId::new("seed-account"),
        harness: HarnessId::new("claude-code"),
        label: "Seed".into(),
        plan: Some(PlanTier::new("max-20x")),
        state_dir: PathBuf::from("/tmp/seed"),
    };
    let profile = Profile {
        id: ProfileId::new("seed-profile"),
        account: account.id.clone(),
        settings_overlay: SettingsOverlay::default(),
        default_cwd: None,
    };
    storage.accounts().insert(&account).unwrap();
    storage.profiles().insert(&profile).unwrap();
    (account, profile)
}

// ---------------------------------------------------------------------------
// property round-trips

proptest! {
    #[test]
    fn account_roundtrip(account in arb_account()) {
        let storage = Storage::open_in_memory().unwrap();
        storage.accounts().insert(&account).unwrap();
        prop_assert_eq!(storage.accounts().get(&account.id).unwrap(), account);
    }

    #[test]
    fn profile_roundtrip(overlay in arb_overlay(), id in id_str()) {
        let storage = Storage::open_in_memory().unwrap();
        let (account, _) = seed_chain(&storage);
        let profile = Profile { id: ProfileId::new(id), account: account.id, settings_overlay: overlay, default_cwd: None };
        storage.profiles().insert(&profile).unwrap();
        prop_assert_eq!(storage.profiles().get(&profile.id).unwrap(), profile);
    }

    #[test]
    fn session_roundtrip(session in arb_session(ProfileId::new("seed-profile"))) {
        let storage = Storage::open_in_memory().unwrap();
        seed_chain(&storage);
        storage.sessions().insert(&session).unwrap();
        prop_assert!(sessions_equivalent(
            &storage.sessions().get(&session.id).unwrap(),
            &session
        ));
        let listed = storage.sessions().list(Some(&session.profile)).unwrap();
        prop_assert!(listed.iter().any(|s| s.id == session.id));
    }

    #[test]
    fn session_update_roundtrip(mut session in arb_session(ProfileId::new("seed-profile"))) {
        let storage = Storage::open_in_memory().unwrap();
        seed_chain(&storage);
        storage.sessions().insert(&session).unwrap();
        session.status = SessionStatus::QuotaBlocked;
        session.native_id = Some("native-1".into());
        storage.sessions().update(&session).unwrap();
        prop_assert_eq!(storage.sessions().get(&session.id).unwrap().status, SessionStatus::QuotaBlocked);
    }
}

// ---------------------------------------------------------------------------
// edge cases

#[test]
fn missing_entities_are_not_found_not_panics() {
    let storage = Storage::open_in_memory().unwrap();
    let missing = AccountId::new("ghost");
    assert!(matches!(
        storage.accounts().get(&missing),
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        storage.accounts().delete(&missing),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn deleting_account_cascades() {
    let storage = Storage::open_in_memory().unwrap();
    let (account, profile) = seed_chain(&storage);
    let session = SessionRecord {
        id: SessionId::new("s1"),
        native_id: None,
        profile: profile.id,
        machine: MachineId::new("this-mac"),
        cwd: PathBuf::from("/tmp"),
        status: SessionStatus::Running,
        started_at: Timestamp::from_millis(1),
        last_activity: None,
        usage: UsageSnapshot::default(),
        transcript_ref: None,
    };
    storage.sessions().insert(&session).unwrap();
    storage.accounts().delete(&account.id).unwrap();
    assert!(storage.sessions().list(None).unwrap().is_empty());
    assert!(
        storage
            .profiles()
            .list_by_account(&account.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn reopen_is_idempotent_and_future_schema_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("juggler.sqlite");

    let storage = Storage::open(&db).unwrap();
    drop(storage);
    // Reopening an already-migrated database applies nothing and succeeds.
    Storage::open(&db).unwrap();

    // Simulate a database written by a newer build.
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.pragma_update(None, "user_version", 9_999).unwrap();
    drop(conn);
    match Storage::open(&db) {
        Err(CoreError::SchemaTooNew {
            found,
            supported: _,
        }) => assert_eq!(found, 9_999),
        other => panic!("expected SchemaTooNew, got {other:?}"),
    }
}

#[test]
fn corrupt_file_degrades_to_not_a_database() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("garbage.sqlite");
    std::fs::write(&db, b"this is definitely not sqlite").unwrap();
    match Storage::open(&db) {
        Err(CoreError::NotADatabase { .. }) => {}
        other => panic!("expected NotADatabase, got {other:?}"),
    }
}

#[test]
fn atomic_write_replaces_and_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("export.json");
    fs::atomic_write(&path, b"first").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"first");
    fs::atomic_write(&path, b"second").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"second");
    // No temporary siblings left behind.
    let leftovers = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".ai-juggler-tmp-")
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn every_status_roundtrips_through_its_stored_name() {
    for status in SessionStatus::all() {
        assert_eq!(SessionStatus::parse(status.as_str()), Some(status));
    }
    assert_eq!(SessionStatus::parse("sentient"), None);
}
