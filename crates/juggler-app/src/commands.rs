//! Tauri commands exposed to the webview, with TypeScript types generated
//! from the Rust definitions (`ts-rs`) — single source of truth for both
//! sides of the bridge.
//!
//! Commands take `tauri::State` by value per the framework's convention.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri_plugin_opener::OpenerExt;
use ts_rs::TS;

use juggler_adapter::LaunchMode;
use juggler_core::model::{
    Account, AccountId, MachineId, Profile, ProfileId, SessionId, SessionRecord, SessionStatus,
    SettingsOverlay, Timestamp,
};
use juggler_core::storage::Storage;
use juggler_supervisor::LifecycleEvent;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Static application information served to the UI at startup.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct AppInfo {
    /// Application version from the crate manifest.
    pub version: String,
    /// Machine-friendly app name.
    pub name: String,
}

impl AppInfo {
    /// Builds the info from compile-time crate metadata.
    #[must_use]
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            name: "AI Juggler".to_string(),
        }
    }
}

/// Returns static application information.
#[tauri::command]
#[must_use]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn app_info() -> AppInfo {
    AppInfo::current()
}

/// A Provider the app can manage (ADR-0008; AJ-E107).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct ProviderInfo {
    /// Provider id, e.g. `claude-code`.
    pub id: String,
    /// Display name.
    pub name: String,
}

/// Deprecated alias (one version). AJ-E107.
pub type HarnessInfo = ProviderInfo;

/// An account row for the UI.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct AccountInfo {
    /// Account id.
    pub id: String,
    /// Provider id (wire field `harness` aliased one version; AJ-E107).
    #[serde(alias = "harness")]
    pub provider: String,
    /// User label.
    pub label: String,
    /// Plan tier, if set.
    pub plan: Option<String>,
    /// Login state: `logged_in` or `logged_out`.
    pub login_state: String,
}

/// A Launch settings row for the UI (AJ-E107).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct LaunchRecipeInfo {
    /// Profile id.
    pub id: String,
    /// Human name.
    pub name: String,
    /// Owning account id.
    pub account_id: String,
    /// Owning Account label (denormalized for the list).
    pub account_label: String,
    /// Provider id (wire field `harness` aliased one version; AJ-E107).
    #[serde(alias = "harness")]
    pub provider: String,
    /// Preferred model from the overlay, if any.
    pub model: Option<String>,
    /// Default working directory, if any.
    pub default_cwd: Option<String>,
}

/// Deprecated alias (one version). AJ-E107.
pub type ProfileInfo = LaunchRecipeInfo;

/// An Instance row for the UI (the conversation lives inside; AJ-E107).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct InstanceInfo {
    /// Instance id.
    pub id: String,
    /// Launch settings id it runs under.
    pub launch_recipe_id: String,
    /// Provider id (wire field `harness` aliased one version).
    #[serde(alias = "harness")]
    pub provider: String,
    /// Account label for display.
    pub label: String,
    /// Lifecycle status string (storage vocabulary).
    pub status: String,
    /// Start time, epoch milliseconds.
    #[ts(type = "number")]
    pub started_at_ms: i64,
    /// Instance kind: `terminal` | `desktop`.
    pub kind: String,
    /// Owning project id, when the cwd resolved to a tracked project.
    pub project_id: Option<String>,
    /// The native conversation inside this Instance, once discovered
    /// (AJ-E104: Instance row → conversation inside; None while Starting).
    pub conversation_id: Option<String>,
}

/// Deprecated alias (one version). AJ-E107.
pub type SessionInfo = InstanceInfo;

/// A tracked project for the UI — the juggling anchor (ADR-0007).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct ProjectInfo {
    /// Project id.
    pub id: String,
    /// Canonical root directory.
    pub root: String,
    /// Display name.
    pub name: String,
    /// Last activity, epoch milliseconds (when known).
    #[ts(type = "number")]
    pub last_activity_ms: Option<i64>,
    /// The instance currently managing this project, when live.
    pub active_instance: Option<SessionInfo>,
    /// Account labels that have ever run in this project (AI Juggler runs
    /// plus imported pre-existing history).
    pub accounts_used: Vec<String>,
    /// Total instances run through AI Juggler in this project.
    #[ts(type = "number")]
    pub instance_count: u64,
    /// Sessions discovered in imported transcripts (pre-AI-Juggler work).
    #[ts(type = "number")]
    pub history_sessions: u64,
}

/// Dashboard counts.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct StatsInfo {
    /// Managed accounts.
    #[ts(type = "number")]
    pub accounts: u64,
    /// Configured profiles.
    #[ts(type = "number")]
    pub profiles: u64,
    /// Sessions not in a terminal state.
    #[ts(type = "number")]
    pub active_sessions: u64,
}

/// Result of launching a session.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct LaunchResult {
    /// The new session id.
    pub session_id: String,
    /// OS process id.
    pub pid: u32,
}

/// An existing harness setup found by the onboarding environment scan
/// (spec §12). Metadata-only discovery; the wizard shows `notes`
/// verbatim.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct DiscoveredInfo {
    /// Harness id.
    pub harness: String,
    /// Config dir path.
    pub path: String,
    /// `vendor-default` | `env-override` | `home-pattern`.
    pub origin: String,
    /// `logged_in` | `logged_out`.
    pub login: String,
    /// Approximate bytes on disk (caches excluded).
    #[ts(type = "number")]
    pub bytes: u64,
    /// Most recent activity, unix seconds (when known).
    #[ts(type = "number")]
    pub last_used_secs: Option<u64>,
    /// True when already managed by AI Juggler.
    pub managed: bool,
    /// Human-facing hints shown by the wizard.
    pub notes: Vec<String>,
}

/// Result of importing a discovered setup as a managed account.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct ImportResult {
    /// The created account.
    pub account: AccountInfo,
    /// Files copied into the managed dir.
    #[ts(type = "number")]
    pub files_copied: u64,
    /// Bytes copied into the managed dir.
    #[ts(type = "number")]
    pub bytes_copied: u64,
    /// Cache/noise entries skipped, human-readable.
    pub skipped: Vec<String>,
    /// Login state of the imported account: `logged_in` | `logged_out`.
    pub login: String,
    /// When login did not carry over, the plain-language reason and next
    /// step (e.g. macOS keychain path-locking).
    pub login_note: Option<String>,
}

// ---------------------------------------------------------------------------
// commands

/// Lists the Providers this build can manage (canonical name; the
/// legacy `harnesses_list` alias is registered in the handler).
#[tauri::command(rename = "providers_list")]
#[must_use]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn harnesses_list() -> Vec<ProviderInfo> {
    vec![ProviderInfo {
        id: juggler_adapters::CLAUDE_CODE_ID.to_string(),
        name: "Claude Code".to_string(),
    }]
}

/// Lists accounts with their login state.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn accounts_list(state: tauri::State<'_, Arc<AppState>>) -> AppResult<Vec<AccountInfo>> {
    let storage = lock_storage(&state)?;
    let mut out = Vec::new();
    for account in storage.accounts().list()? {
        out.push(account_info_of(&account));
    }
    Ok(out)
}

/// Creates a managed account: prepares the isolated harness state dir and
/// persists the account.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn account_create(
    state: tauri::State<'_, Arc<AppState>>,
    harness: String,
    label: String,
    plan: Option<String>,
) -> AppResult<AccountInfo> {
    create_account(state.inner(), &harness, &label, plan)
}

/// Core implementation shared with integration tests (commands need a
/// Tauri runtime; this does not).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn create_account(
    state: &Arc<AppState>,
    harness: &str,
    label: &str,
    plan: Option<String>,
) -> AppResult<AccountInfo> {
    let adapter = state
        .registry
        .get(&juggler_core::model::HarnessId::new(harness))
        .ok_or_else(|| AppError::UnknownHarness(harness.to_string()))?;
    let id = AccountId::new(format!("{harness}-{}", unique_suffix()));
    let state_dir = state.accounts_dir().join(harness).join(id.as_str());
    adapter.prepare_account(&state_dir)?;
    let account = Account {
        id,
        harness: juggler_core::model::HarnessId::new(harness),
        label: label.to_string(),
        plan: plan.map(juggler_core::model::PlanTier::new),
        state_dir,
    };
    lock_storage_arc(state)?.accounts().insert(&account)?;
    Ok(account_info_of(&account))
}

/// Re-probes an account's login state (metadata-only; ADR-0003).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn account_login_state(
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
) -> AppResult<String> {
    let account = find_account(&state, &account_id)?;
    Ok(login_state_name(login_state_of(&account)).to_string())
}

/// The copyable shell command that runs the vendor's own login against this
/// account's isolated config dir (external-terminal fallback, spec §4.1).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn account_login_command(
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
) -> AppResult<String> {
    let account = find_account(&state, &account_id)?;
    Ok(format!(
        "CLAUDE_CONFIG_DIR='{}' claude",
        account.state_dir.display()
    ))
}

/// Scans the machine for existing harness setups the onboarding wizard can
/// import (metadata-only; ADR-0006). Never fails — an empty list is a
/// valid "nothing found".
#[tauri::command]
#[must_use]
pub fn environment_scan(state: tauri::State<'_, Arc<AppState>>) -> Vec<DiscoveredInfo> {
    environment_scan_impl(state.inner())
}

/// Core implementation shared with integration tests.
#[must_use]
pub fn environment_scan_impl(state: &Arc<AppState>) -> Vec<DiscoveredInfo> {
    let mut out = Vec::new();
    for harness in state.registry.ids() {
        let Some(adapter) = state.registry.get(&harness) else {
            continue;
        };
        for found in adapter.scan_environment() {
            out.push(DiscoveredInfo {
                harness: found.harness.as_str().to_string(),
                path: found.path.to_string_lossy().into_owned(),
                origin: found.origin.as_str().to_string(),
                login: login_probe_name(found.login),
                bytes: found.bytes,
                last_used_secs: found.last_used_secs,
                managed: found.managed,
                notes: found.notes,
            });
        }
    }
    out
}

/// Imports a discovered setup as a new managed account: byte-copies the
/// config into an isolated dir (source untouched), creates a default
/// "Main" profile so the account is immediately launchable.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn account_import(
    state: tauri::State<'_, Arc<AppState>>,
    harness: String,
    source_path: String,
    label: String,
    plan: Option<String>,
) -> AppResult<ImportResult> {
    account_import_impl(state.inner(), &harness, &source_path, &label, plan)
}

/// Core implementation shared with integration tests.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn account_import_impl(
    state: &Arc<AppState>,
    harness: &str,
    source_path: &str,
    label: &str,
    plan: Option<String>,
) -> AppResult<ImportResult> {
    let adapter = state
        .registry
        .get(&juggler_core::model::HarnessId::new(harness))
        .ok_or_else(|| AppError::UnknownHarness(harness.to_string()))?;
    let id = AccountId::new(format!("{harness}-{}", unique_suffix()));
    let state_dir = state.accounts_dir().join(harness).join(id.as_str());
    let outcome = adapter.import_account(std::path::Path::new(source_path), &state_dir)?;
    let account = Account {
        id,
        harness: juggler_core::model::HarnessId::new(harness),
        label: label.to_string(),
        plan: plan.map(juggler_core::model::PlanTier::new),
        state_dir,
    };
    {
        let storage = lock_storage_arc(state)?;
        storage.accounts().insert(&account)?;
        // A default profile makes the imported account launchable in one
        // click — the wizard's promise.
        storage.profiles().insert(&Profile {
            id: ProfileId::new(format!("prof-{}", unique_suffix())),
            name: "Main".to_string(),
            account: account.id.clone(),
            settings_overlay: SettingsOverlay::default(),
            default_cwd: None,
        })?;
    }
    Ok(ImportResult {
        account: account_info_of(&account),
        files_copied: outcome.files_copied,
        bytes_copied: outcome.bytes_copied,
        skipped: outcome.skipped,
        login: login_probe_name(outcome.login),
        login_note: outcome.login_note,
    })
}

/// Spawns the guided login for an account inside an embedded terminal.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn terminal_spawn_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
) -> AppResult<crate::terminal::TerminalInfo> {
    crate::terminal::spawn_login_terminal_impl(&app, state.inner(), &account_id)
}

/// Starts streaming a supervised session into the terminal dock
/// (idempotent per terminal).
#[tauri::command]
#[must_use]
pub fn terminal_attach(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> crate::terminal::TerminalInfo {
    let info = crate::terminal::TerminalInfo { id: id.clone() };
    crate::terminal::attach_forwarder(
        &app,
        state.inner(),
        &juggler_core::model::SessionId::new(id),
    );
    info
}

/// Feeds keystrokes to a terminal.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn terminal_write(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    data: String,
) -> AppResult<()> {
    crate::terminal::terminal_write_impl(state.inner(), &id, &data)
}

/// Resizes a terminal.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn terminal_resize(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    cols: u16,
    rows: u16,
) -> AppResult<()> {
    crate::terminal::terminal_resize_impl(state.inner(), &id, cols, rows)
}

/// Stops a terminal's process (graceful ladder).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn terminal_kill(state: tauri::State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    crate::terminal::terminal_kill_impl(state.inner(), &id)
}

/// The retained output tail (base64) for replay on panel reopen.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn terminal_snapshot(state: tauri::State<'_, Arc<AppState>>, id: String) -> AppResult<String> {
    crate::terminal::terminal_snapshot_impl(state.inner(), &id)
}

/// Opens the vendor's own web sign-in page in the system browser — the
/// maximum login aid allowed (ADR-0003): the user completes the vendor's
/// flow; credentials never touch AI Juggler.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn login_open_web(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    harness: String,
) -> AppResult<()> {
    let adapter = state
        .registry
        .get(&juggler_core::model::HarnessId::new(harness.as_str()))
        .ok_or_else(|| AppError::UnknownHarness(harness.clone()))?;
    let Some(url) = adapter.login_url() else {
        return Err(AppError::Io {
            context: "open login page",
            message: format!("{harness} has no web sign-in page"),
        });
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Io {
            context: "open login page",
            message: e.to_string(),
        })
}

/// Lists Launch settings (canonical name `launch_recipes_list`;
/// legacy alias registered in the handler).
#[tauri::command(rename = "launch_recipes_list")]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn profiles_list(state: tauri::State<'_, Arc<AppState>>) -> AppResult<Vec<LaunchRecipeInfo>> {
    let storage = lock_storage(&state)?;
    profile_infos(&storage)
}

/// Creates a profile under an account.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn profile_create(
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
    name: String,
    model: Option<String>,
    default_cwd: Option<String>,
) -> AppResult<ProfileInfo> {
    create_profile(state.inner(), &account_id, &name, model, default_cwd)
}

/// Core implementation shared with integration tests.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn create_profile(
    state: &Arc<AppState>,
    account_id: &str,
    name: &str,
    model: Option<String>,
    default_cwd: Option<String>,
) -> AppResult<ProfileInfo> {
    if name.trim().is_empty() {
        return Err(AppError::Io {
            context: "create profile",
            message: "profile name must not be empty".into(),
        });
    }
    let storage = lock_storage_arc(state)?;
    let account = storage
        .accounts()
        .get(&AccountId::new(account_id))
        .map_err(|error| not_found_or_core(error, "account", account_id))?;
    let profile = Profile {
        id: ProfileId::new(format!("prof-{}", unique_suffix())),
        name: name.trim().to_string(),
        account: account.id,
        settings_overlay: SettingsOverlay {
            model,
            env: std::collections::BTreeMap::default(),
        },
        default_cwd: default_cwd.clone().map(std::path::PathBuf::from),
    };
    storage.profiles().insert(&profile)?;
    Ok(ProfileInfo {
        id: profile.id.to_string(),
        name: profile.name,
        account_id: profile.account.to_string(),
        account_label: account.label,
        provider: account.harness.to_string(),
        model: profile.settings_overlay.model,
        default_cwd,
    })
}

/// Deletes a profile (cascades to its session records).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn profile_delete(state: tauri::State<'_, Arc<AppState>>, profile_id: String) -> AppResult<()> {
    lock_storage(&state)?
        .profiles()
        .delete(&ProfileId::new(profile_id))?;
    Ok(())
}

/// Launches a session for a profile through the supervisor (one click on
/// the UI side; the only writer of harness state is the harness itself,
/// inside its managed dir).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn launch_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    profile_id: String,
    cwd: Option<String>,
) -> AppResult<LaunchResult> {
    let result = launch_profile(state.inner(), &profile_id, cwd)?;
    emit_sessions(&app, state.inner())?;
    Ok(result)
}

/// Core implementation shared with integration tests.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn launch_profile(
    state: &Arc<AppState>,
    profile_id: &str,
    cwd: Option<String>,
) -> AppResult<LaunchResult> {
    use juggler_adapter::LaunchMode;
    use juggler_core::model::MachineId;

    let (profile, account) = find_profile_with_account_arc(state, profile_id)?;
    if profile.settings_overlay.model.is_some()
        && account.harness.as_str() == juggler_adapters::CLAUDE_CODE_ID
    {
        juggler_adapters::claude_code::write_settings_overlay(
            &account.state_dir,
            &profile.settings_overlay,
        )?;
    }
    let adapter = state
        .registry
        .get(&account.harness)
        .ok_or_else(|| AppError::UnknownHarness(account.harness.to_string()))?;
    let effective_cwd = cwd
        .or_else(|| {
            profile
                .default_cwd
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
        })
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let spec = adapter.launch_spec(&account, &profile, &effective_cwd, &LaunchMode::Interactive)?;
    let session_id = SessionId::new(format!("sess-{}", unique_suffix()));
    let spawned = state.supervisor.spawn(&session_id, &spec)?;
    // The juggling anchor: this launch's cwd becomes (or refreshes) the
    // tracked project, and the session is linked to it (ADR-0007).
    let canonical_cwd = spec.cwd.canonicalize().unwrap_or_else(|_| spec.cwd.clone());
    {
        let storage = lock_storage_arc(state)?;
        let project = storage.projects().touch(&canonical_cwd, None)?;
        storage.sessions().insert(&SessionRecord {
            id: session_id.clone(),
            conversation: None,
            profile: profile.id,
            machine: MachineId::new("this-mac"),
            kind: juggler_core::model::InstanceKind::Terminal,
            project: Some(project.id.clone()),
            cwd: spec.cwd.clone(),
            status: SessionStatus::Starting,
            started_at: Timestamp::now(),
            last_activity: None,
            usage: juggler_core::model::UsageSnapshot::default(),
            transcript_ref: None,
        })?;
    };
    Ok(LaunchResult {
        session_id: session_id.to_string(),
        pid: spawned.pid,
    })
}

/// Lists Instances (board feed), newest first (canonical name
/// `instances_list`; legacy alias registered in the handler).
#[tauri::command(rename = "instances_list")]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn sessions_list(state: tauri::State<'_, Arc<AppState>>) -> AppResult<Vec<InstanceInfo>> {
    let storage = lock_storage(&state)?;
    session_infos(&storage)
}

/// Stops a session via the supervisor ladder (grace, then kill).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn stop_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> AppResult<()> {
    state
        .supervisor
        .stop(&SessionId::new(session_id), Duration::from_secs(5))?;
    emit_sessions(&app, state.inner())?;
    Ok(())
}

/// Dashboard counts.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn dashboard_stats(state: tauri::State<'_, Arc<AppState>>) -> AppResult<StatsInfo> {
    let storage = lock_storage(&state)?;
    let accounts = storage.accounts().list()?;
    let mut profiles = 0_u64;
    for account in &accounts {
        profiles += u64::try_from(storage.profiles().list_by_account(&account.id)?.len())
            .unwrap_or_default();
    }
    let active = storage
        .sessions()
        .list(None)?
        .into_iter()
        .filter(|record| {
            matches!(
                record.status,
                SessionStatus::Starting | SessionStatus::Running | SessionStatus::WaitingUser
            )
        })
        .count();
    Ok(StatsInfo {
        accounts: u64::try_from(accounts.len()).unwrap_or_default(),
        profiles,
        active_sessions: u64::try_from(active).unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// lifecycle forwarding

/// Applies a supervisor lifecycle event to storage (status transitions).
/// Returns the updated record when one matched.
pub fn apply_lifecycle_event(
    storage: &Storage,
    event: LifecycleEvent,
    stopped_by_user: bool,
) -> Option<SessionRecord> {
    let session_id = match &event {
        LifecycleEvent::Started { session, .. } | LifecycleEvent::Exited { session, .. } => session,
    };
    let mut record = storage.sessions().get(session_id).ok()?;
    match event {
        LifecycleEvent::Started { .. } => record.status = SessionStatus::Running,
        LifecycleEvent::Exited { kind, .. } => {
            record.status = if stopped_by_user {
                // The stop ladder (PTY exits surface kills as plain
                // non-zero codes) — user intent, not a failure.
                SessionStatus::Stopped
            } else {
                match kind {
                    kind if kind.is_success() => SessionStatus::Finished,
                    juggler_supervisor::ExitKind::Signaled(_) => SessionStatus::Stopped,
                    juggler_supervisor::ExitKind::Code(_) => SessionStatus::Failed,
                }
            };
        }
    }
    record.last_activity = Some(Timestamp::now());
    storage.sessions().update(&record).ok()?;
    Some(record)
}

/// Emits the refreshed session list to the UI.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn emit_sessions(app: &tauri::AppHandle, state: &Arc<AppState>) -> AppResult<()> {
    use tauri::Emitter;
    let sessions = {
        let storage = state.storage.lock().map_err(|_| AppError::StatePoisoned)?;
        session_infos(&storage)?
    };
    let _ = app.emit(crate::EVENT_SESSIONS_UPDATED, sessions);
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers

fn lock_storage_arc(state: &Arc<AppState>) -> Result<std::sync::MutexGuard<'_, Storage>, AppError> {
    state.storage.lock().map_err(|_| AppError::StatePoisoned)
}

fn lock_storage<'a>(
    state: &'a tauri::State<'_, Arc<AppState>>,
) -> Result<std::sync::MutexGuard<'a, Storage>, AppError> {
    state.storage.lock().map_err(|_| AppError::StatePoisoned)
}

fn account_info_of(account: &Account) -> AccountInfo {
    AccountInfo {
        login_state: login_state_name(login_state_of(account)).to_string(),
        id: account.id.to_string(),
        provider: account.harness.to_string(),
        label: account.label.clone(),
        plan: account.plan.as_ref().map(ToString::to_string),
    }
}

/// Wire-format name for a scan/import login probe.
fn login_probe_name(probe: juggler_adapter::LoginProbe) -> String {
    match probe {
        juggler_adapter::LoginProbe::LoggedIn => "logged_in",
        juggler_adapter::LoginProbe::LoggedOut => "logged_out",
    }
    .to_string()
}

/// Wire-format name for a login state.
fn login_state_name(state: juggler_adapters::LoginState) -> &'static str {
    match state {
        juggler_adapters::LoginState::LoggedIn => "logged_in",
        juggler_adapters::LoginState::LoggedOut => "logged_out",
    }
}

fn login_state_of(account: &Account) -> juggler_adapters::LoginState {
    if account.harness.as_str() == juggler_adapters::CLAUDE_CODE_ID {
        return juggler_adapters::ClaudeCodeAdapter::new().login_state(&account.state_dir);
    }
    juggler_adapters::LoginState::LoggedOut
}

/// Profile rows with denormalized account labels/harnesses.
fn profile_infos(storage: &Storage) -> AppResult<Vec<ProfileInfo>> {
    let accounts: std::collections::BTreeMap<String, (String, String)> = storage
        .accounts()
        .list()?
        .into_iter()
        .map(|account| {
            (
                account.id.to_string(),
                (account.label, account.harness.to_string()),
            )
        })
        .collect();
    let mut out = Vec::new();
    for (account_id, (label, harness)) in &accounts {
        for profile in storage
            .profiles()
            .list_by_account(&AccountId::new(account_id.as_str()))?
        {
            out.push(ProfileInfo {
                id: profile.id.to_string(),
                name: profile.name.clone(),
                account_id: profile.account.to_string(),
                account_label: label.clone(),
                provider: harness.clone(),
                model: profile.settings_overlay.model,
                default_cwd: profile
                    .default_cwd
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
            });
        }
    }
    Ok(out)
}

/// Session rows with denormalized labels/harnesses.
fn session_infos(storage: &Storage) -> AppResult<Vec<InstanceInfo>> {
    let accounts: std::collections::BTreeMap<String, String> = storage
        .accounts()
        .list()?
        .into_iter()
        .map(|account| (account.id.to_string(), account.harness.to_string()))
        .collect();
    let mut profile_index: std::collections::BTreeMap<String, (String, String)> =
        std::collections::BTreeMap::new();
    for (account_id, harness) in &accounts {
        for profile in storage
            .profiles()
            .list_by_account(&AccountId::new(account_id.as_str()))?
        {
            profile_index.insert(
                profile.id.to_string(),
                (profile.account.to_string(), harness.clone()),
            );
        }
    }
    let account_labels: std::collections::BTreeMap<String, String> = storage
        .accounts()
        .list()?
        .into_iter()
        .map(|account| (account.id.to_string(), account.label))
        .collect();
    Ok(storage
        .sessions()
        .list(None)?
        .into_iter()
        .map(|record| {
            let (account_id, harness) = profile_index
                .get(&record.profile.to_string())
                .cloned()
                .unwrap_or_default();
            SessionInfo {
                label: account_labels.get(&account_id).cloned().unwrap_or_default(),
                id: record.id.to_string(),
                launch_recipe_id: record.profile.to_string(),
                provider: harness,
                status: record.status.as_str().to_string(),
                started_at_ms: record.started_at.millis(),
                kind: record.kind.as_str().to_string(),
                project_id: record
                    .project
                    .as_ref()
                    .map(std::string::ToString::to_string),
                conversation_id: record.conversation.as_ref().map(|c| c.as_str().to_string()),
            }
        })
        .collect())
}

/// Scans imported transcripts and backfills the projects rail with the
/// user's REAL history: every project directory found in any account's
/// `projects/` transcripts (cwd read from the transcript itself, never
/// guessed from the encoded dir name), with per-account history merged.
/// Idempotent; safe to run at every startup.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn projects_backfill(state: tauri::State<'_, Arc<AppState>>) -> AppResult<u32> {
    projects_backfill_impl(state.inner())
}

/// Core implementation shared with tests.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn projects_backfill_impl(state: &Arc<AppState>) -> AppResult<u32> {
    use std::collections::BTreeMap;

    // root → (labels, native session count)
    let mut found: BTreeMap<PathBuf, (Vec<String>, u64)> = BTreeMap::new();
    let accounts = lock_storage_arc(state)?.accounts().list()?;
    for account in &accounts {
        let projects_root = account.state_dir.join("projects");
        let Ok(entries) = std::fs::read_dir(&projects_root) else {
            continue;
        };
        for project_dir in entries.flatten().filter(|e| e.path().is_dir()) {
            let jsonl = newest_jsonl(&project_dir.path());
            let Some(jsonl) = jsonl else { continue };
            let Some(cwd) = transcript_cwd(&jsonl) else {
                continue; // cannot decode the dir name unambiguously
            };
            let entry = found
                .entry(PathBuf::from(&cwd))
                .or_insert_with(|| (Vec::new(), 0));
            if !entry.0.contains(&account.label) {
                entry.0.push(account.label.clone());
            }
            entry.1 += count_jsonl(&project_dir.path());
        }
    }

    let storage = lock_storage_arc(state)?;
    let mut enriched = 0;
    for (root, (labels, sessions)) in found {
        let record = storage.projects().touch(&root, None)?;
        storage.projects().merge_history(
            &record.id,
            &juggler_core::model::ProjectHistory {
                accounts: labels,
                native_sessions: sessions,
            },
        )?;
        enriched += 1;
    }
    Ok(enriched)
}

/// Newest `.jsonl` in a project transcript dir by mtime.
fn newest_jsonl(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "jsonl"))
        .max_by_key(|entry| entry.metadata().ok().and_then(|m| m.modified().ok()))
        .map(|entry| entry.path())
}

fn count_jsonl(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|e| e == "jsonl"))
                .count() as u64
        })
        .unwrap_or_default()
}

/// Reads a transcript's true working directory from its first lines
/// (`"cwd"` is recorded on message records) — the only unambiguous decode.
fn transcript_cwd(jsonl: &std::path::Path) -> Option<String> {
    use std::io::BufRead;
    let file = std::fs::File::open(jsonl).ok()?;
    for line in std::io::BufReader::new(file).lines().take(25).flatten() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line)
            && let Some(cwd) = value.get("cwd").and_then(|v| v.as_str())
            && !cwd.is_empty()
        {
            return Some(cwd.to_string());
        }
    }
    None
}

/// The juggle (spec §7 T1/T2 v0): move a live session's transcript to
/// another account and resume it there — same project, same context, one
/// click. User-initiated only (automation policy).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn session_handover(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    target_profile_id: String,
) -> AppResult<LaunchResult> {
    let result = session_handover_impl(state.inner(), &session_id, &target_profile_id)?;
    emit_sessions(&app, state.inner())?;
    Ok(result)
}

/// Core implementation shared with tests.
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the source session, its
/// transcript, or the target profile cannot be resolved, or on copy or
/// launch failure.
pub fn session_handover_impl(
    state: &Arc<AppState>,
    session_id: &str,
    target_profile_id: &str,
) -> AppResult<LaunchResult> {
    let (profile_b, account_b) = find_profile_with_account_arc(state, target_profile_id)?;

    let (source, account_a) = {
        let storage = lock_storage_arc(state)?;
        let source = storage
            .sessions()
            .get(&SessionId::new(session_id))
            .map_err(|error| not_found_or_core(error, "session", session_id))?;
        let source_profile = storage
            .profiles()
            .get(&source.profile)
            .map_err(|error| not_found_or_core(error, "profile", &source.profile.to_string()))?;
        let account_a = storage
            .accounts()
            .get(&source_profile.account)
            .map_err(|error| {
                not_found_or_core(error, "account", &source_profile.account.to_string())
            })?;
        (source, account_a)
    };
    if source.project.is_none() {
        return Err(AppError::Io {
            context: "hand over",
            message: "session has no project link".into(),
        });
    }

    // The newest transcript for the source cwd in account A's config.
    let encoded = source.cwd.to_string_lossy().replace('/', "-");
    let dir_a = account_a.state_dir.join("projects").join(&encoded);
    let transcript = newest_jsonl(&dir_a).ok_or_else(|| AppError::Io {
        context: "hand over",
        message: "no transcript found for this session's project".into(),
    })?;
    let native_id = transcript
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Io {
            context: "hand over",
            message: "transcript has no session id".into(),
        })?;

    // Transplant: same encoded subpath under account B (same cwd → same
    // encoding), byte copy, credentials never involved.
    let dir_b = account_b.state_dir.join("projects").join(&encoded);
    std::fs::create_dir_all(&dir_b).map_err(|e| AppError::Io {
        context: "hand over",
        message: format!("cannot create target dir: {e}"),
    })?;
    let dest = dir_b.join(transcript.file_name().unwrap_or_default());
    std::fs::copy(&transcript, &dest).map_err(|e| AppError::Io {
        context: "hand over",
        message: format!("cannot copy transcript: {e}"),
    })?;

    // Resume under account B in the same project directory.
    let adapter = state
        .registry
        .get(&account_b.harness)
        .ok_or_else(|| AppError::UnknownHarness(account_b.harness.to_string()))?;
    let spec = adapter.launch_spec(
        &account_b,
        &profile_b,
        &source.cwd,
        &LaunchMode::Resume {
            session_id: native_id.clone(),
        },
    )?;
    let new_session = SessionId::new(format!("sess-{}", unique_suffix()));
    let spawned = state.supervisor.spawn(&new_session, &spec)?;

    {
        let storage = lock_storage_arc(state)?;
        // Source lineage: handed over.
        let mut handed = source.clone();
        handed.status = SessionStatus::HandedOver;
        storage.sessions().update(&handed)?;
        // The continuation session, linked to the same project.
        storage.sessions().insert(&SessionRecord {
            id: new_session.clone(),
            conversation: Some(juggler_core::model::ConversationId::new(native_id)),
            profile: profile_b.id,
            machine: MachineId::new("this-mac"),
            kind: juggler_core::model::InstanceKind::Terminal,
            project: source.project,
            cwd: source.cwd.clone(),
            status: SessionStatus::Starting,
            started_at: Timestamp::now(),
            last_activity: None,
            usage: juggler_core::model::UsageSnapshot::default(),
            transcript_ref: Some(juggler_core::model::TranscriptRef {
                path: dest,
                native_id: None,
            }),
        })?;
    }

    Ok(LaunchResult {
        session_id: new_session.to_string(),
        pid: spawned.pid,
    })
}

/// Lists tracked projects with their live instance, account history, and
/// totals — the juggling view (ADR-0007).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn projects_list(state: tauri::State<'_, Arc<AppState>>) -> AppResult<Vec<ProjectInfo>> {
    projects_list_impl(state.inner())
}

/// Core implementation shared with integration tests.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn projects_list_impl(state: &Arc<AppState>) -> AppResult<Vec<ProjectInfo>> {
    let storage = lock_storage_arc(state)?;
    let sessions = session_infos(&storage)?;
    // Statuses that mean "occupying the project right now".
    let live = |status: &str| matches!(status, "starting" | "running");
    Ok(storage
        .projects()
        .list()?
        .into_iter()
        .map(|project| {
            let id = project.id.to_string();
            let owned: Vec<&SessionInfo> = sessions
                .iter()
                .filter(|s| s.project_id.as_deref() == Some(id.as_str()))
                .collect();
            let active_instance = owned.iter().find(|s| live(&s.status)).map(|s| (*s).clone());
            let mut accounts_used: Vec<String> = owned
                .iter()
                .map(|s| s.label.clone())
                .filter(|label| !label.is_empty())
                .collect();
            accounts_used.extend(project.history.accounts.iter().cloned());
            accounts_used.sort();
            accounts_used.dedup();
            ProjectInfo {
                id,
                root: project.root.to_string_lossy().into_owned(),
                name: project.name,
                last_activity_ms: project.last_activity.map(juggler_core::Timestamp::millis),
                active_instance,
                accounts_used,
                instance_count: u64::try_from(owned.len()).unwrap_or_default(),
                history_sessions: project.history.native_sessions,
            }
        })
        .collect())
}

fn find_account(
    state: &tauri::State<'_, Arc<AppState>>,
    account_id: &str,
) -> Result<Account, AppError> {
    find_account_arc(state.inner(), account_id)
}

/// Core account lookup shared with the terminal module and tests.
///
/// # Errors
///
/// Returns [`AppError::NotFound`] when the account does not exist.
pub fn find_account_arc(state: &Arc<AppState>, account_id: &str) -> Result<Account, AppError> {
    lock_storage_arc(state)?
        .accounts()
        .get(&AccountId::new(account_id))
        .map_err(|error| not_found_or_core(error, "account", account_id))
}

fn find_profile_with_account_arc(
    state: &Arc<AppState>,
    profile_id: &str,
) -> Result<(Profile, Account), AppError> {
    let storage = lock_storage_arc(state)?;
    let profile = storage
        .profiles()
        .get(&ProfileId::new(profile_id))
        .map_err(|error| not_found_or_core(error, "profile", profile_id))?;
    let account = storage
        .accounts()
        .get(&profile.account)
        .map_err(|error| not_found_or_core(error, "account", &profile.account.to_string()))?;
    Ok((profile, account))
}

fn not_found_or_core(error: juggler_core::CoreError, entity: &'static str, id: &str) -> AppError {
    match error {
        juggler_core::CoreError::NotFound { .. } => AppError::NotFound {
            entity,
            id: id.to_string(),
        },
        other => AppError::Core(other),
    }
}

pub(crate) fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
        % 1_000_000_000_000
}

#[cfg(test)]
mod tests {
    use super::AppInfo;

    #[test]
    fn app_info_is_populated() {
        let info = AppInfo::current();
        assert_eq!(info.name, "AI Juggler");
        assert!(!info.version.is_empty());
    }
}

/// One-version IPC aliases (AJ-E107 Req.2): legacy command names keep
/// deserialising while UI call sites migrate to the canonical names.
pub mod legacy_aliases {
    use super::{
        AppResult, InstanceInfo, LaunchRecipeInfo, ProviderInfo, harnesses_list, profiles_list,
        sessions_list,
    };
    use crate::state::AppState;
    use std::sync::Arc;

    /// Legacy alias of `providers_list`.
    #[tauri::command]
    #[must_use]
    pub fn harnesses_list_alias() -> Vec<ProviderInfo> {
        harnesses_list()
    }

    /// Legacy alias of `launch_recipes_list`.
    #[tauri::command]
    ///
    /// # Errors
    ///
    /// Returns the typed [`AppError`] message on failure.
    pub fn profiles_list_alias(
        state: tauri::State<'_, Arc<AppState>>,
    ) -> AppResult<Vec<LaunchRecipeInfo>> {
        profiles_list(state)
    }

    /// Legacy alias of `instances_list`.
    #[tauri::command]
    ///
    /// # Errors
    ///
    /// Returns the typed [`AppError`] message on failure.
    pub fn sessions_list_alias(
        state: tauri::State<'_, Arc<AppState>>,
    ) -> AppResult<Vec<InstanceInfo>> {
        sessions_list(state)
    }

    // Silence unused-import when only aliases reference these.
}
