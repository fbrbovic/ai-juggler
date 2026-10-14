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

/// I-05: quota window snapshot (SPEC §6). `null` when no feed exists for
/// the Provider — never a fabricated percentage (AJ-E401 honesty contract).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct QuotaSnapshotInfo {
    /// Percent of the window's allowance already used (0–100).
    pub used_percentage: f64,
    /// When the window resets, epoch milliseconds, if known.
    #[ts(type = "number | null")]
    pub resets_at_ms: Option<i64>,
    /// Trust level of the number: `measured` | `computed` | `estimated`.
    pub confidence: String,
}

/// I-06: token counters (SPEC §6). `null` until usage has actually been
/// observed — a never-sampled Instance reports no numbers, not zeros.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct TokenCountsInfo {
    /// Prompt tokens consumed.
    #[ts(type = "number")]
    pub input: u64,
    /// Completion tokens consumed (thinking included, billed as output).
    #[ts(type = "number")]
    pub output: u64,
    /// Tokens served from the prompt cache.
    #[ts(type = "number")]
    pub cache_read: u64,
    /// Tokens written to the prompt cache.
    #[ts(type = "number")]
    pub cache_write: u64,
    /// Reasoning tokens (subset semantics per harness; kept separate for
    /// analytics).
    #[ts(type = "number")]
    pub reasoning: u64,
}

/// AJ-E405: the structured attention reason carried on the row — the
/// UI renders it verbatim (rule + measured inputs + since); it never
/// re-derives the reason from the status alone.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct AttentionInfo {
    /// Rule id: `stuck_no_output` | `waiting_prompt` | `quota_exit`.
    pub rule: String,
    /// Human-readable measured inputs (`no output 6m`, `cpu 0.3%`).
    pub inputs: Vec<String>,
    /// When the condition began, epoch milliseconds.
    #[ts(type = "number")]
    pub since_ms: i64,
}

/// I-10: why an Instance needs its user (`null` when it does not).
/// Derived from the lifecycle status here; AJ-E405 replaces the
/// derivation with structured heuristic reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub enum NeedsAttention {
    /// A permission/wait prompt is alive.
    WaitingUser,
    /// No-output heuristics tripped (rendered `Stuck?` — heuristic).
    Stuck,
    /// Rate-limit signature on exit.
    QuotaBlocked,
    /// Non-zero, non-user exit.
    Failed,
}

/// I-07/I-08 delta payload of the 5 s resource tick (`session://resources`,
/// AJ-E401 Req.4). One emission per tick carries every live Instance —
/// coalesced by construction.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct InstanceResourcesInfo {
    /// Instance id.
    pub id: String,
    /// CPU utilization summed over the process tree, percent; `null`
    /// until two table samples exist.
    pub cpu_pct: Option<f64>,
    /// Resident memory summed over the process tree, bytes; `null`
    /// until sampled.
    #[ts(type = "number | null")]
    pub rss_bytes: Option<u64>,
    /// Live descendant count; `null` until sampled.
    #[ts(type = "number | null")]
    pub child_count: Option<u32>,
}

/// I-05 delta payload of the 30 s quota tick (`session://quota`,
/// AJ-E401 Req.4). `quota` is `null` while no feed exists for the
/// Provider (the two-clock engine, AJ-E606+, populates it later).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct InstanceQuotaInfo {
    /// Instance id.
    pub id: String,
    /// Quota window snapshot, or `null` when unknown.
    pub quota: Option<QuotaSnapshotInfo>,
}

/// An Instance row for the UI (the conversation lives inside; AJ-E107).
/// One payload carries every indicator-row and inspector field I-01..I-11
/// (AJ-E401): absent data is `null`, never `0`, `""`, or a guessed name.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct InstanceInfo {
    /// Instance id.
    pub id: String,
    /// Launch settings id it runs under.
    pub launch_recipe_id: String,
    /// Provider id (wire field `harness` aliased one version); `unknown`
    /// when the launch settings row is gone — never blank.
    #[serde(alias = "harness")]
    pub provider: String,
    /// Owning account id; `null` when the launch settings row is gone.
    pub account_id: Option<String>,
    /// Owning Account label for display (wire field `label` aliased one
    /// version); `unknown` when unresolvable — never blank.
    #[serde(alias = "label")]
    pub account_label: String,
    /// I-03: preferred model from the launch settings overlay; `null`
    /// means "default model", never a guessed model name.
    pub model: Option<String>,
    /// I-04: lifecycle status string (storage vocabulary).
    pub status: String,
    /// I-05: quota window snapshot; `null` when no feed exists.
    pub quota: Option<QuotaSnapshotInfo>,
    /// I-06: token counters; `null` until usage has been observed.
    pub tokens: Option<TokenCountsInfo>,
    /// I-07: CPU percent summed over the process tree; `null` until the
    /// resource tick has two samples.
    pub cpu_pct: Option<f64>,
    /// I-07: resident bytes summed over the process tree; `null` until
    /// sampled (a finished Instance is not measurable — not zero).
    #[ts(type = "number | null")]
    pub rss_bytes: Option<u64>,
    /// I-08: live descendant count; `null` until sampled, `0` after exit
    /// (nothing can be running then).
    #[ts(type = "number | null")]
    pub child_count: Option<u32>,
    /// I-08: subtask count; `null` — no transcript source exists yet
    /// (AJ-E409 lands the signals; never a guessed count).
    #[ts(type = "number | null")]
    pub subtask_count: Option<u64>,
    /// I-09: git branch of the working directory, parsed from
    /// `.git/HEAD` (no subprocess); `null` for non-git or detached.
    pub git_branch: Option<String>,
    /// I-09: dirty file count; `null` — requires a `git status` walk that
    /// must never run per-row on the hot path (collector comes later).
    #[ts(type = "number | null")]
    pub git_dirty_count: Option<u64>,
    /// I-10: why this Instance needs its user; `null` when it does not.
    pub needs_attention: Option<NeedsAttention>,
    /// AJ-E405: the structured reason (rule + measured inputs + since)
    /// behind `needs_attention`; `null` when healthy or pre-E405 rows.
    pub attention: Option<AttentionInfo>,
    /// Start time, epoch milliseconds.
    #[ts(type = "number")]
    pub started_at_ms: i64,
    /// I-11: last observed activity, epoch milliseconds; `null` when
    /// never observed.
    #[ts(type = "number | null")]
    pub last_activity_ms: Option<i64>,
    /// Fleet dimension (D-13): machine the Instance runs on.
    pub machine_id: String,
    /// Working directory (AJ-E407 Overview: copyable project + cwd).
    pub cwd: String,
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
    #[ts(type = "number | null")]
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
    /// True when the root directory no longer exists on disk (AJ-E408
    /// Overview danger banner + `Relocate…`).
    pub missing: bool,
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
            attention: None,
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
    // Collector lock first and alone: the resource tick shares it, and no
    // code path ever holds the storage and collector locks together.
    let live = live_stats_of(state.inner());
    let storage = lock_storage(&state)?;
    session_infos(&storage, &live)
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
    quota_blocked: Option<&juggler_core::model::AttentionReason>,
) -> Option<SessionRecord> {
    let session_id = match &event {
        LifecycleEvent::Started { session, .. } | LifecycleEvent::Exited { session, .. } => session,
    };
    let mut record = storage.sessions().get(session_id).ok()?;
    match event {
        LifecycleEvent::Started { .. } => {
            record.status = SessionStatus::Running;
            record.attention = None;
        }
        LifecycleEvent::Exited { kind, .. } => {
            record.status = if stopped_by_user {
                // The stop ladder (PTY exits surface kills as plain
                // non-zero codes) — user intent, not a failure.
                SessionStatus::Stopped
            } else if let Some(reason) = quota_blocked {
                // AJ-E405: non-zero exit with a rate-limit signature —
                // QuotaBlocked (immediate, no debounce) with the reason.
                record.attention = Some(reason.clone());
                SessionStatus::QuotaBlocked
            } else {
                match kind {
                    kind if kind.is_success() => SessionStatus::Finished,
                    juggler_supervisor::ExitKind::Signaled(_) => SessionStatus::Stopped,
                    juggler_supervisor::ExitKind::Code(_) => SessionStatus::Failed,
                }
            };
            if record.status != SessionStatus::QuotaBlocked {
                record.attention = None;
            }
        }
    }
    record.last_activity = Some(Timestamp::now());
    storage.sessions().update(&record).ok()?;
    Some(record)
}

/// Records one heuristic transition (status + structured reason) and
/// returns the updated record. No action ever follows (SPEC §7).
pub fn apply_attention_transition(
    storage: &Storage,
    session: &SessionId,
    transition: &juggler_supervisor::AttentionTransition,
) -> Option<SessionRecord> {
    let mut record = storage.sessions().get(session).ok()?;
    record.status = transition.status;
    record.attention = Some(transition.reason.clone());
    storage.sessions().update(&record).ok()?;
    Some(record)
}

/// Clears attention and returns the record to `Running` — the recovery
/// path (output resumed; AJ-E405 Req.5).
pub fn apply_attention_recovery(storage: &Storage, session: &SessionId) -> Option<SessionRecord> {
    let mut record = storage.sessions().get(session).ok()?;
    if !matches!(
        record.status,
        SessionStatus::Stuck | SessionStatus::WaitingUser | SessionStatus::QuotaBlocked
    ) {
        return None;
    }
    record.status = SessionStatus::Running;
    record.attention = None;
    record.last_activity = Some(Timestamp::now());
    storage.sessions().update(&record).ok()?;
    Some(record)
}

/// One 15 s heuristic cycle (AJ-E405): evaluates every active Instance
/// against the supervisor's measured inputs and records + emits any
/// confirmed transitions. Purely passive — no action follows.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn heuristic_tick(app: &tauri::AppHandle, state: &Arc<AppState>) -> AppResult<()> {
    let engine = &state.heuristics;
    let live = live_stats_of(state);
    let mut changed = false;
    {
        let storage = state.storage.lock().map_err(|_| AppError::StatePoisoned)?;
        for record in storage.sessions().list(None)? {
            if !is_active(record.status) {
                engine.forget(&record.id);
                continue;
            }
            let now = Timestamp::now().millis();
            let transcript_mtime_ms = record.transcript_ref.as_ref().and_then(|tr| {
                std::fs::metadata(&tr.path)
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|dur| i64::try_from(dur.as_millis()).unwrap_or_default())
            });
            let tail = state
                .supervisor
                .output_tail(&record.id, 2048)
                .unwrap_or_default();
            let inputs = juggler_supervisor::HeuristicInputs {
                last_output_ms: state.supervisor.last_output_ms(&record.id),
                cpu_pct: live
                    .get(&record.id.to_string())
                    .and_then(|stats| stats.cpu_pct),
                transcript_mtime_ms,
                prompt_alive: juggler_supervisor::prompt_alive(&tail),
                now_ms: now,
            };
            if let Some(transition) = engine.evaluate(&record.id, &inputs) {
                apply_attention_transition(&storage, &record.id, &transition);
                changed = true;
            }
        }
    }
    if changed {
        emit_sessions(app, state)?;
    }
    Ok(())
}

/// Event-driven recovery drain (Req.5, <300 ms): every session with
/// fresh output that currently sits in a heuristic state returns to
/// `Running` and the UI is told immediately.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn attention_recovery_drain(app: &tauri::AppHandle, state: &Arc<AppState>) -> AppResult<()> {
    let mut changed = false;
    for session in state.supervisor.drain_activity() {
        if let Some(running) = state.heuristics.recover(&session) {
            debug_assert_eq!(running, SessionStatus::Running);
            {
                let storage = state.storage.lock().map_err(|_| AppError::StatePoisoned)?;
                if apply_attention_recovery(&storage, &session).is_some() {
                    changed = true;
                }
            }
        }
    }
    if changed {
        emit_sessions(app, state)?;
    }
    Ok(())
}

/// Emits the refreshed session list to the UI.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn emit_sessions(app: &tauri::AppHandle, state: &Arc<AppState>) -> AppResult<()> {
    use tauri::Emitter;
    let live = live_stats_of(state);
    let sessions = {
        let storage = state.storage.lock().map_err(|_| AppError::StatePoisoned)?;
        session_infos(&storage, &live)?
    };
    let _ = app.emit(crate::EVENT_SESSIONS_UPDATED, sessions);
    Ok(())
}

/// One batched sample of live process stats for every supervised session
/// (locks only the collector — never together with the storage lock).
fn live_stats_of(state: &Arc<AppState>) -> LiveStatsMap {
    let Ok(mut collector) = state.resources.lock() else {
        // Poisoned collector: report nothing rather than fabricated zeros.
        return LiveStatsMap::new();
    };
    collector
        .refresh(&state.supervisor)
        .into_iter()
        .map(|(session, stats)| (session.to_string(), stats))
        .collect()
}

/// Emits the 5 s resource delta (I-07/I-08) on `session://resources`:
/// one coalesced payload per tick carrying every active Instance, and no
/// emission at all while nothing is live (idle costs zero wakeups for
/// the UI, AJ-E401 Req.4).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn emit_resources(app: &tauri::AppHandle, state: &Arc<AppState>) -> AppResult<()> {
    use tauri::Emitter;
    let live = live_stats_of(state);
    let payload: Vec<InstanceResourcesInfo> = {
        let storage = state.storage.lock().map_err(|_| AppError::StatePoisoned)?;
        storage
            .sessions()
            .list(None)?
            .into_iter()
            .filter(|record| is_active(record.status))
            .filter_map(|record| {
                // Unsampled rows are omitted — the UI keeps its honest
                // `null` instead of inheriting a zero.
                let sample = live.get(&record.id.to_string()).copied()?;
                Some(InstanceResourcesInfo {
                    id: record.id.to_string(),
                    cpu_pct: sample.cpu_pct,
                    rss_bytes: Some(sample.rss_bytes),
                    child_count: Some(sample.child_count),
                })
            })
            .collect()
    };
    if payload.is_empty() {
        return Ok(());
    }
    let _ = app.emit(crate::EVENT_SESSIONS_RESOURCES, payload);
    Ok(())
}

/// Emits the 30 s quota delta (I-05) on `session://quota`: one coalesced
/// payload per tick; per-Instance `quota` is `null` until a feed exists
/// (the two-clock engine, AJ-E606+, populates it later — the tick itself
/// is the contract this task ships).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
pub fn emit_quota(app: &tauri::AppHandle, state: &Arc<AppState>) -> AppResult<()> {
    use tauri::Emitter;
    let payload: Vec<InstanceQuotaInfo> = {
        let storage = state.storage.lock().map_err(|_| AppError::StatePoisoned)?;
        storage
            .sessions()
            .list(None)?
            .into_iter()
            .filter(|record| is_active(record.status))
            .map(|record| InstanceQuotaInfo {
                id: record.id.to_string(),
                quota: quota_snapshot_of(&record.usage),
            })
            .collect()
    };
    if payload.is_empty() {
        return Ok(());
    }
    let _ = app.emit(crate::EVENT_SESSIONS_QUOTA, payload);
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

/// Live supervisor stats keyed by Instance id (empty until the collector
/// has sampled; historical rows never have entries).
pub type LiveStatsMap = std::collections::HashMap<String, juggler_supervisor::LiveProcessStats>;

/// I-05 conversion shared by the row payload and the 30 s quota tick.
fn quota_snapshot_of(usage: &juggler_core::model::UsageSnapshot) -> Option<QuotaSnapshotInfo> {
    usage.window.map(|window| QuotaSnapshotInfo {
        used_percentage: window.used_percentage,
        resets_at_ms: window.resets_at.map(Timestamp::millis),
        confidence: confidence_name(usage.confidence).to_string(),
    })
}

/// Wire name for a usage-confidence level (SPEC §6 honesty rule).
fn confidence_name(confidence: juggler_core::model::Confidence) -> &'static str {
    match confidence {
        juggler_core::model::Confidence::Measured => "measured",
        juggler_core::model::Confidence::Computed => "computed",
        juggler_core::model::Confidence::Estimated => "estimated",
    }
}

/// Wire value for the I-10 attention reason derived from lifecycle status
/// (AJ-E405 replaces the derivation with structured heuristic reasons).
fn needs_attention_of(status: SessionStatus) -> Option<NeedsAttention> {
    match status {
        SessionStatus::WaitingUser => Some(NeedsAttention::WaitingUser),
        SessionStatus::Stuck => Some(NeedsAttention::Stuck),
        SessionStatus::QuotaBlocked => Some(NeedsAttention::QuotaBlocked),
        SessionStatus::Failed => Some(NeedsAttention::Failed),
        SessionStatus::Starting
        | SessionStatus::Running
        | SessionStatus::Finished
        | SessionStatus::Stopped
        | SessionStatus::HandedOver => None,
    }
}

/// I-09: branch name read from `<cwd>/.git/HEAD` — one file read, never a
/// `git` subprocess on the hot path (AJ-E401 performance budget). Detached
/// heads and worktree/gitfile layouts return `None`: unknown, not guessed.
fn git_branch_of(cwd: &std::path::Path) -> Option<String> {
    let head = std::fs::read_to_string(cwd.join(".git").join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
}

/// A terminal status: the process is gone, so nothing about it is live.
fn is_terminal(status: SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Finished
            | SessionStatus::Stopped
            | SessionStatus::Failed
            | SessionStatus::HandedOver
    )
}

/// An active status: the row carries live-state expectations (resource
/// ticks and quota ticks only ever emit for these).
fn is_active(status: SessionStatus) -> bool {
    !is_terminal(status)
}

/// Resolves the account/provider join for one Instance row. When the
/// launch-recipe or account row is gone (defensive only — storage FKs
/// prevent it today), the Provider and label degrade to a generic
/// `unknown` marker, never a blank string, and the account id to `null`.
fn resolved_join(
    recipe_index: &std::collections::BTreeMap<String, (String, String, Option<String>)>,
    account_index: &std::collections::BTreeMap<String, (String, String)>,
    recipe_id: &str,
) -> (Option<String>, String, String, Option<String>) {
    let (account_id, provider, model) = recipe_index.get(recipe_id).cloned().unwrap_or_default();
    let account_label = account_index
        .get(&account_id)
        .map_or_else(|| "unknown".to_string(), |(label, _)| label.clone());
    let provider = if provider.is_empty() {
        "unknown".to_string()
    } else {
        provider
    };
    (
        if account_id.is_empty() {
            None
        } else {
            Some(account_id)
        },
        account_label,
        provider,
        model,
    )
}

/// Session rows with denormalized account/provider joins and the full
/// I-01..I-11 read model (AJ-E401). One storage pass per collection
/// (accounts, launch settings, instances) joined in memory — no per-row
/// storage reads; absent values serialise as `null`, never `0` or `""`.
fn session_infos(storage: &Storage, live: &LiveStatsMap) -> AppResult<Vec<InstanceInfo>> {
    // account id → (label, provider)
    let account_index: std::collections::BTreeMap<String, (String, String)> = storage
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
    // launch-recipe id → (account id, provider, overlay model)
    let mut recipe_index: std::collections::BTreeMap<String, (String, String, Option<String>)> =
        std::collections::BTreeMap::new();
    for (account_id, (_, harness)) in &account_index {
        for recipe in storage
            .profiles()
            .list_by_account(&AccountId::new(account_id.as_str()))?
        {
            recipe_index.insert(
                recipe.id.to_string(),
                (
                    recipe.account.to_string(),
                    harness.clone(),
                    recipe.settings_overlay.model,
                ),
            );
        }
    }
    Ok(storage
        .sessions()
        .list(None)?
        .into_iter()
        .map(|record| {
            let (account_id, account_label, provider, model) =
                resolved_join(&recipe_index, &account_index, &record.profile.to_string());
            let terminal = is_terminal(record.status);
            let stats = if terminal {
                None
            } else {
                live.get(&record.id.to_string()).copied()
            };
            InstanceInfo {
                id: record.id.to_string(),
                launch_recipe_id: record.profile.to_string(),
                provider,
                account_id,
                account_label,
                model,
                status: record.status.as_str().to_string(),
                quota: quota_snapshot_of(&record.usage),
                tokens: if record.usage.tokens == juggler_core::model::TokenCounts::default() {
                    None
                } else {
                    let t = record.usage.tokens;
                    Some(TokenCountsInfo {
                        input: t.input,
                        output: t.output,
                        cache_read: t.cache_read,
                        cache_write: t.cache_write,
                        reasoning: t.reasoning,
                    })
                },
                cpu_pct: stats.and_then(|s| s.cpu_pct),
                rss_bytes: stats.map(|s| s.rss_bytes),
                child_count: if terminal {
                    Some(0)
                } else {
                    stats.map(|s| s.child_count)
                },
                // No transcript-derived source exists yet (AJ-E409) —
                // shipped nullable, never backfilled with a guess.
                subtask_count: None,
                git_branch: git_branch_of(&record.cwd),
                // No subprocess-free dirty-count source; collector lands
                // with the E4 signals work. Null = unavailable, not 0.
                git_dirty_count: None,
                needs_attention: needs_attention_of(record.status),
                attention: record.attention.as_ref().map(|reason| AttentionInfo {
                    rule: reason.rule.clone(),
                    inputs: reason.inputs.clone(),
                    since_ms: reason.since_ms,
                }),
                started_at_ms: record.started_at.millis(),
                last_activity_ms: record.last_activity.as_ref().map(|t| t.millis()),
                machine_id: record.machine.to_string(),
                cwd: record.cwd.to_string_lossy().into_owned(),
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
            attention: None,
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
    // Collector first (never locked together with storage), then the
    // storage guard for the whole join below.
    let live_stats = live_stats_of(state);
    let storage = lock_storage_arc(state)?;
    let sessions = session_infos(&storage, &live_stats)?;
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
                .map(|s| s.account_label.clone())
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
                missing: !project.root.exists(),
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
    use super::{
        AppInfo, InstanceInfo, LiveStatsMap, NeedsAttention, QuotaSnapshotInfo, TokenCountsInfo,
        git_branch_of, needs_attention_of, session_infos,
    };
    use juggler_core::model::{
        Account, AccountId, Confidence, InstanceKind, InstanceRecord, LaunchRecipe, MachineId,
        ProfileId, QuotaWindow, SessionId, SessionStatus, SettingsOverlay, Timestamp, TokenCounts,
        UsageSnapshot,
    };
    use juggler_core::storage::Storage;
    use juggler_supervisor::LiveProcessStats;

    #[test]
    fn app_info_is_populated() {
        let info = AppInfo::current();
        assert_eq!(info.name, "AI Juggler");
        assert!(!info.version.is_empty());
    }

    /// A sacrificial git cwd: `<tmp>/.git/HEAD` with a branch ref. Never a
    /// real repository — fixtures only (workspace safety rule). Every call
    /// gets a unique directory: tests run in parallel and a shared path
    /// would let one test's `fs::write` truncate while another reads.
    fn git_cwd(branch: &str) -> std::path::PathBuf {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "aij-e401-{}-{}-{n}",
            std::process::id(),
            branch.replace('/', "-")
        ));
        let git = dir.join(".git");
        std::fs::create_dir_all(&git).expect("fixture .git dir");
        std::fs::write(git.join("HEAD"), format!("ref: refs/heads/{branch}\n"))
            .expect("fixture HEAD");
        dir
    }

    fn record(
        id: &str,
        recipe: &str,
        status: SessionStatus,
        cwd: std::path::PathBuf,
    ) -> InstanceRecord {
        InstanceRecord {
            id: SessionId::new(id),
            conversation: None,
            profile: ProfileId::new(recipe),
            machine: MachineId::new("this-mac"),
            kind: InstanceKind::Terminal,
            project: None,
            cwd,
            status,
            started_at: Timestamp::from_millis(1_700_000_000_000),
            last_activity: Some(Timestamp::from_millis(1_700_000_060_000)),
            usage: UsageSnapshot::default(),
            attention: None,
            transcript_ref: None,
        }
    }

    /// Storage seeded with one account, two launch recipes (with and
    /// without a model) and the four AJ-E401 fixture rows.
    fn fixture_storage() -> Storage {
        let storage = Storage::open_in_memory().expect("in-memory storage");
        storage
            .accounts()
            .insert(&Account {
                id: AccountId::new("acc-1"),
                harness: juggler_core::model::HarnessId::new("claude-code"),
                label: "Work Max".into(),
                plan: None,
                state_dir: std::env::temp_dir().join("aij-e401-account-state"),
            })
            .expect("seed account");
        for (id, model) in [("rec-1", Some("opus")), ("rec-2", None)] {
            storage
                .profiles()
                .insert(&LaunchRecipe {
                    id: ProfileId::new(id),
                    name: "fixture".into(),
                    account: AccountId::new("acc-1"),
                    settings_overlay: SettingsOverlay {
                        model: model.map(String::from),
                        env: Default::default(),
                    },
                    default_cwd: None,
                })
                .expect("seed recipe");
        }

        // Full row: every source populated.
        let mut full = record(
            "sess-full",
            "rec-1",
            SessionStatus::Running,
            git_cwd("feat/read-model"),
        );
        full.usage = UsageSnapshot {
            tokens: TokenCounts {
                input: 12_345,
                output: 6_789,
                cache_read: 100,
                cache_write: 200,
                reasoning: 42,
            },
            window: Some(QuotaWindow {
                used_percentage: 42.5,
                resets_at: Some(Timestamp::from_millis(1_700_010_000_000)),
            }),
            confidence: Confidence::Computed,
        };
        storage.sessions().insert(&full).expect("seed full");

        // Quota-less row: no feed, tokens never observed.
        storage
            .sessions()
            .insert(&record(
                "sess-no-quota",
                "rec-1",
                SessionStatus::Running,
                std::env::temp_dir(),
            ))
            .expect("seed quota-less");

        // Model-less row, terminal: launch settings carry no model; a
        // finished Instance is not resource-measurable.
        storage
            .sessions()
            .insert(&record(
                "sess-no-model",
                "rec-2",
                SessionStatus::Finished,
                std::env::temp_dir(),
            ))
            .expect("seed model-less");

        // Dangling recipe rows cannot exist in storage (the sessions table
        // foreign-keys launch_recipe_id), so the join's generic-marker
        // fallback is covered by `unjoinable_rows_render_generic_markers`
        // below instead of an insert.

        // Waiting row with a resolvable account: attention reason present.
        storage
            .sessions()
            .insert(&record(
                "sess-waiting",
                "rec-1",
                SessionStatus::WaitingUser,
                std::env::temp_dir(),
            ))
            .expect("seed waiting");
        storage
    }

    #[allow(clippy::panic)] // fixture lookup: panicking names the missing row
    fn row<'a>(rows: &'a [InstanceInfo], id: &str) -> &'a InstanceInfo {
        rows.iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("row {id} missing"))
    }

    /// AJ-E401 fixtures: full row, quota-less row, model-less row — absent
    /// data serialises as `null`, never `0`, `""`, or a guessed name.
    #[test]
    fn session_info_full_row_carries_every_field() {
        let storage = fixture_storage();
        let live: LiveStatsMap = [(
            "sess-full".to_string(),
            LiveProcessStats {
                child_count: 2,
                cpu_pct: Some(3.5),
                rss_bytes: 123_456,
            },
        )]
        .into_iter()
        .collect();
        let rows = session_infos(&storage, &live).expect("rows");
        let full = row(&rows, "sess-full");

        assert_eq!(full.account_id.as_deref(), Some("acc-1"));
        assert_eq!(full.account_label, "Work Max");
        assert_eq!(full.provider, "claude-code");
        assert_eq!(full.model.as_deref(), Some("opus"));
        let quota = full.quota.as_ref().expect("quota present");
        assert_eq!(
            quota,
            &QuotaSnapshotInfo {
                used_percentage: 42.5,
                resets_at_ms: Some(1_700_010_000_000),
                confidence: "computed".into(),
            }
        );
        assert_eq!(
            full.tokens,
            Some(TokenCountsInfo {
                input: 12_345,
                output: 6_789,
                cache_read: 100,
                cache_write: 200,
                reasoning: 42,
            })
        );
        assert_eq!(full.cpu_pct, Some(3.5));
        assert_eq!(full.rss_bytes, Some(123_456));
        assert_eq!(full.child_count, Some(2));
        assert_eq!(full.git_branch.as_deref(), Some("feat/read-model"));
        assert_eq!(full.needs_attention, None);
        assert_eq!(full.last_activity_ms, Some(1_700_000_060_000));
        assert_eq!(full.machine_id, "this-mac");
    }

    #[test]
    fn session_info_absent_data_is_null_not_zero() {
        let storage = fixture_storage();
        let rows = session_infos(&storage, &LiveStatsMap::new()).expect("rows");

        // Quota-less: no feed, no observed tokens, never sampled.
        let no_quota = row(&rows, "sess-no-quota");
        assert_eq!(no_quota.quota, None);
        assert_eq!(no_quota.tokens, None);
        assert_eq!(no_quota.cpu_pct, None);
        assert_eq!(no_quota.rss_bytes, None);
        assert_eq!(no_quota.child_count, None);
        // Non-git cwd: the field is omitted entirely, not blank.
        assert_eq!(no_quota.git_branch, None);

        // Model-less: null, never a guessed default model name.
        let no_model = row(&rows, "sess-no-model");
        assert_eq!(no_model.model, None);
        // Terminal status: nothing is live; child_count 0 is a fact, cpu
        // and rss are unmeasurable — null, not zero.
        assert_eq!(no_model.child_count, Some(0));
        assert_eq!(no_model.cpu_pct, None);
        assert_eq!(no_model.rss_bytes, None);
        assert_eq!(no_model.needs_attention, None);

        // Waiting: structured attention reason, not a re-derived guess.
        assert_eq!(
            row(&rows, "sess-waiting").needs_attention,
            Some(NeedsAttention::WaitingUser)
        );

        // Wire honesty: the serialised payload shows null, not zeroed
        // objects or empty strings.
        for id in ["sess-no-quota", "sess-no-model"] {
            let json = serde_json::to_string(row(&rows, id)).expect("serialize");
            assert!(json.contains(r#""quota":null"#), "{id}: {json}");
            assert!(json.contains(r#""tokens":null"#), "{id}: {json}");
            assert!(
                json.contains(r#""account_label":"Work Max""#),
                "{id}: {json}"
            );
        }
    }

    /// AJ-E401 performance budget: the in-memory join (storage reads +
    /// per-row derivation, excluding the batched process-table refresh)
    /// stays under the 20 ms p95 budget at the 50-Instance target scale.
    #[test]
    fn session_info_join_p95_under_budget_at_50_instances() {
        let storage = Storage::open_in_memory().expect("storage");
        storage
            .accounts()
            .insert(&Account {
                id: AccountId::new("acc-bench"),
                harness: juggler_core::model::HarnessId::new("claude-code"),
                label: "Bench".into(),
                plan: None,
                state_dir: std::env::temp_dir().join("aij-e401-bench"),
            })
            .expect("seed account");
        storage
            .profiles()
            .insert(&LaunchRecipe {
                id: ProfileId::new("rec-bench"),
                name: "bench".into(),
                account: AccountId::new("acc-bench"),
                settings_overlay: SettingsOverlay::default(),
                default_cwd: None,
            })
            .expect("seed recipe");
        for n in 0..50 {
            let mut record = record(
                &format!("sess-bench-{n}"),
                "rec-bench",
                SessionStatus::Running,
                std::env::temp_dir(),
            );
            record.usage.tokens = TokenCounts {
                input: n,
                ..TokenCounts::default()
            };
            storage.sessions().insert(&record).expect("seed row");
        }
        let mut samples: Vec<u128> = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            let rows = session_infos(&storage, &LiveStatsMap::new()).expect("rows");
            assert_eq!(rows.len(), 50);
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        let p95 = samples[94]; // 95th of 100
        assert!(p95 < 20_000, "join p95 {p95}µs exceeds the 20 ms budget");
    }

    /// Unknown statuses cannot reach the read model: storage rejects them
    /// at row-mapping time (degrade-to-error, never a panic), so the wire
    /// only ever carries the nine vocabulary strings — verified by
    /// round-tripping every status through the mapper.
    #[test]
    fn session_info_status_vocabulary_is_the_nine_wire_strings() {
        let storage = fixture_storage();
        let rows = session_infos(&storage, &LiveStatsMap::new()).expect("rows");
        let vocabulary: Vec<&str> = rows.iter().map(|row| row.status.as_str()).collect();
        for wire in [
            "starting",
            "running",
            "waiting_user",
            "stuck",
            "quota_blocked",
            "finished",
            "stopped",
            "failed",
            "handed_over",
        ] {
            assert!(
                SessionStatus::parse(wire).is_some(),
                "wire status {wire} must parse"
            );
        }
        assert!(
            vocabulary
                .iter()
                .all(|status| SessionStatus::parse(status).is_some())
        );
        assert_eq!(
            needs_attention_of(SessionStatus::parse("quota_blocked").unwrap()),
            Some(NeedsAttention::QuotaBlocked)
        );
    }

    /// Defensive join fallback (storage FKs make this unreachable today):
    /// an unjoinable row renders generic `unknown` markers — account id
    /// `null`, provider/label never blank (AJ-E401 Req.2).
    #[test]
    fn unjoinable_rows_render_generic_markers() {
        use super::resolved_join;
        use std::collections::BTreeMap;
        let (account_id, account_label, provider, model) =
            resolved_join(&BTreeMap::new(), &BTreeMap::new(), "rec-gone");
        assert_eq!(account_id, None);
        assert_eq!(account_label, "unknown");
        assert_eq!(provider, "unknown");
        assert_eq!(model, None);
    }

    /// I-09 branch parsing: ref rows parse, detached and missing `.git`
    /// stay `None` (unknown, not a guess).
    #[test]
    fn git_branch_parses_refs_only() {
        let cwd = git_cwd("main");
        assert_eq!(git_branch_of(&cwd), Some("main".to_string()));
        assert_eq!(git_branch_of(&std::env::temp_dir()), None);
        let detached =
            std::env::temp_dir().join(format!("aij-e401-{}-detached", std::process::id()));
        std::fs::create_dir_all(detached.join(".git")).expect("fixture");
        std::fs::write(
            detached.join(".git/HEAD"),
            "0123456789abcdef0123456789abcdef01234567\n",
        )
        .expect("fixture");
        assert_eq!(git_branch_of(&detached), None);
        std::fs::remove_dir_all(detached).ok();
        std::fs::remove_dir_all(cwd.join(".git")).ok();
    }

    /// AJ-E401 Req.5: the committed ts-rs output contains every new read
    /// model field (regenerated by `cargo test`; committed with the code).
    #[test]
    #[allow(clippy::panic)] // failure message names the missing file
    fn generated_ts_contains_read_model_fields() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/types/generated");
        let read = |name: &str| {
            std::fs::read_to_string(format!("{dir}/{name}.ts"))
                .unwrap_or_else(|error| panic!("generated {name}.ts: {error}"))
        };
        let instance = read("InstanceInfo");
        for field in [
            "account_id",
            "account_label",
            "model",
            "quota",
            "tokens",
            "cpu_pct",
            "rss_bytes",
            "child_count",
            "subtask_count",
            "git_branch",
            "git_dirty_count",
            "needs_attention",
            "last_activity_ms",
            "machine_id",
        ] {
            assert!(
                instance.contains(&format!("{field}:")),
                "InstanceInfo.ts is missing `{field}`"
            );
        }
        for (name, fields) in [
            (
                "QuotaSnapshotInfo",
                &["used_percentage", "resets_at_ms", "confidence"][..],
            ),
            (
                "TokenCountsInfo",
                &["input", "output", "cache_read", "cache_write", "reasoning"][..],
            ),
            (
                "InstanceResourcesInfo",
                &["id", "cpu_pct", "rss_bytes", "child_count"][..],
            ),
            ("InstanceQuotaInfo", &["id", "quota"][..]),
        ] {
            let file = read(name);
            for field in fields {
                assert!(
                    file.contains(&format!("{field}:")),
                    "{name}.ts is missing `{field}`"
                );
            }
        }
        let needs = read("NeedsAttention");
        for variant in ["waiting_user", "stuck", "quota_blocked", "failed"] {
            assert!(
                needs.contains(variant),
                "NeedsAttention.ts is missing `{variant}`"
            );
        }
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
    /// Returns the parent command's typed error on failure.
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
    /// Returns the parent command's typed error on failure.
    pub fn sessions_list_alias(
        state: tauri::State<'_, Arc<AppState>>,
    ) -> AppResult<Vec<InstanceInfo>> {
        sessions_list(state)
    }

    // Silence unused-import when only aliases reference these.
}
