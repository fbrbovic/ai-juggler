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
use juggler_core::CoreError;
use juggler_core::model::{
    Account, AccountId, MachineId, Profile, ProfileId, SessionId, SessionRecord, SessionStatus,
    SettingsOverlay, Timestamp,
};
use juggler_core::storage::Storage;
use juggler_supervisor::{ExitKind, LifecycleEvent};

use crate::error::{AppError, AppResult};
use crate::focus;
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
    /// D-08 Tier-1 cleanup opt-in (off by default — notify-only tier).
    pub cleanup_opt_in: bool,
    /// Optional user-entered renewal date, local calendar `YYYY-MM-DD`
    /// (D-12); `null` renders `Expiry: unknown` — never inferred.
    pub renewal_date: Option<String>,
    /// AJ-E415 per-Account live-preview switch (off by default; the
    /// enable is the Screen-Recording consent moment).
    pub preview_opt_in: bool,
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

/// AJ-E409: the Quality/Performance signal set (D-07) — individually
/// labelled signals with tiers; NO composite score exists (grep gate).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct QualitySignalsInfo {
    /// tokens/min: five 1-min bars (oldest first) or `null` = no data.
    #[ts(type = "Array<number> | null")]
    pub tokens_bars: Option<Vec<u64>>,
    /// Trailing-5-min average tokens/min (a real 0 when idle).
    #[ts(type = "number")]
    pub avg_tokens_per_min: f64,
    /// `2 errors / 41 turns` (exact rendered string) or the
    /// provider-specific not-measurable reason.
    pub error_label: String,
    /// True when the provider has no structured error feed.
    pub errors_not_measurable: bool,
    /// Context tokens in use, when the provider reports them.
    #[ts(type = "number | null")]
    pub context_used: Option<u64>,
    /// Provider-reported context window size, when known.
    #[ts(type = "number | null")]
    pub context_window: Option<u64>,
    /// Cumulative session tokens (labelled `cumulative, not context`).
    pub cumulative: TokenCountsInfo,
    /// Session cost USD from the versioned pricing table (`Est ·
    /// value-equiv.`); `null` when the model has no pricing row.
    #[ts(type = "number | null")]
    pub session_cost_usd: Option<f64>,
    /// The model the cost was priced against (null = unpriced).
    pub priced_model: Option<String>,
    /// Quota forecast RANGE label (`~13-27m at current rate · Est`) or
    /// `not burning`.
    pub quota_forecast: String,
    /// The AJ-E405 stuck rule + inputs, when tripped (permanent `?`).
    pub stuck: Option<AttentionInfo>,
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
    /// I-03 display value: the RUNNING model once observed, else the
    /// launch overlay, else `default model` — never a guessed name.
    pub model: Option<String>,
    /// The launch-overlay model (intent, not telemetry; AJ-E412).
    pub launch_model: Option<String>,
    /// The transcript-OBSERVED running model; `null` until observed —
    /// never filled from the overlay.
    pub running_model: Option<String>,
    /// How `model` is known: `launch` (overlay only, not verified
    /// live), `in-session` (observed, differs from the overlay),
    /// `transcript` (observed), `unknown` (neither).
    pub model_source: String,
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
    /// True when crash recovery re-adopted this Instance's verified
    /// root (AJ-E510): `Running — re-adopted`, terminal detached
    /// (output from before the crash is unavailable), stop via the
    /// AJ-E507 sheet.
    pub adopted: bool,
    /// Owning project id, when the cwd resolved to a tracked project.
    pub project_id: Option<String>,
    /// The native conversation inside this Instance, once discovered
    /// (AJ-E104: Instance row → conversation inside; None while Starting).
    pub conversation_id: Option<String>,
    /// AJ-E413 Req.1: the per-Provider isolation claim for desktop
    /// Instances, verbatim (`Single-account app — Juggler tracks,
    /// cannot isolate` for single-account apps); `null` for terminal
    /// Instances and when the provider's adapter is not registered.
    pub isolation_label: Option<String>,
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
    /// AJ-E501: a configured process ledger failed — cleanup is
    /// read-only until restart (the degraded banner's data).
    pub ledger_degraded: bool,
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

/// Result of a desktop launch (AJ-E413 Req.1): the tracked Instance id,
/// the OS pid, and the bundle identifier when the platform has one.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct DesktopLaunchInfo {
    /// The new (desktop-kind) Instance id.
    pub session_id: String,
    /// OS process id.
    pub pid: u32,
    /// Bundle identifier (macOS), when known.
    pub bundle_id: Option<String>,
    /// The per-Provider isolation claim, shown verbatim (Req.1): e.g.
    /// `Single-account app — Juggler tracks, cannot isolate`.
    pub isolation_label: String,
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
        cleanup_opt_in: false,
        renewal_date: None,
        preview_opt_in: false,
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

/// The stable LOCAL machine id (AJ-E501 Req.6, D-13): generated once,
/// persisted in app settings — never a hard-coded `"this-mac"` literal
/// at call sites.
pub fn local_machine_id(state: &Arc<AppState>) -> MachineId {
    let stored = {
        let storage = lock_storage_arc(state);
        storage
            .ok()
            .and_then(|storage| storage.app_settings().get("local_machine_id").ok())
            .flatten()
    };
    if let Some(id) = stored {
        return MachineId::new(id);
    }
    let generated = format!("machine-{}", unique_suffix());
    if let Ok(storage) = lock_storage_arc(state) {
        let _ = storage.app_settings().set("local_machine_id", &generated);
    }
    MachineId::new(generated)
}

/// The ledger spawn context for an account launch (root role).
fn ledger_ctx(
    state: &Arc<AppState>,
    account: &Account,
    project_id: Option<String>,
) -> juggler_supervisor::tree::SpawnContext {
    juggler_supervisor::tree::SpawnContext {
        provider: account.harness.to_string(),
        account_id: Some(account.id.to_string()),
        project_id,
        machine: local_machine_id(state),
        role: juggler_core::storage::ROLE_ROOT,
    }
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
        cleanup_opt_in: false,
        renewal_date: None,
        preview_opt_in: false,
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
    // The juggling anchor: this launch's cwd becomes (or refreshes) the
    // tracked project, and the session is linked to it (ADR-0007).
    // Resolved BEFORE spawn so the ledger Intent row is attributable.
    let canonical_cwd = spec.cwd.canonicalize().unwrap_or_else(|_| spec.cwd.clone());
    let project_id = {
        let storage = lock_storage_arc(state)?;
        storage
            .projects()
            .touch(&canonical_cwd, None)?
            .id
            .to_string()
    };
    let spawned = state.supervisor.spawn(
        &session_id,
        &spec,
        &ledger_ctx(state, &account, Some(project_id.clone())),
    )?;
    {
        // The machine identity locks storage itself — resolve it
        // BEFORE the insert-scope lock (non-reentrant mutex).
        let machine = local_machine_id(state);
        let storage = lock_storage_arc(state)?;
        let project = storage.projects().touch(&canonical_cwd, None)?;
        storage.sessions().insert(&SessionRecord {
            id: session_id.clone(),
            conversation: None,
            profile: profile.id,
            machine: machine.clone(),
            kind: juggler_core::model::InstanceKind::Terminal,
            project: Some(project.id.clone()),
            cwd: spec.cwd.clone(),
            status: SessionStatus::Starting,
            started_at: Timestamp::now(),
            last_activity: None,
            usage: juggler_core::model::UsageSnapshot::default(),
            attention: None,
            running: None,
            // Terminal Instances own no window identity (AJ-E414).
            window: None,
            transcript_ref: None,
        })?;
    };
    Ok(LaunchResult {
        session_id: session_id.to_string(),
        pid: spawned.pid,
    })
}

/// Launches the Provider's desktop app under an Account and tracks it
/// as a desktop-kind Instance in the same ledger (AJ-E413). Capability-
/// gated: providers without `DESKTOP_APP` are refused by name.
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the provider ships no
/// desktop app or the process cannot start.
#[tauri::command]
pub fn desktop_launch(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
) -> AppResult<DesktopLaunchInfo> {
    let result = desktop_launch_arc(state.inner(), &account_id)?;
    emit_sessions(&app, state.inner())?;
    Ok(result)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the provider ships no
/// desktop app, the account lacks launch settings (the owning recipe is
/// the storage anchor), or the process cannot start.
pub fn desktop_launch_arc(state: &Arc<AppState>, account_id: &str) -> AppResult<DesktopLaunchInfo> {
    let (account, recipe_id) = {
        let storage = lock_storage_arc(state)?;
        let account = storage
            .accounts()
            .get(&AccountId::new(account_id.to_string()))?;
        // Desktop instances anchor to the account's launch settings row
        // (the storage FK); the overlay itself is not applied — env
        // isolation is what the DESKTOP adapter declares, honestly.
        let recipe = storage
            .profiles()
            .list_by_account(&account.id)?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound {
                entity: "launch settings",
                id: account.id.to_string(),
            })?;
        (account, recipe.id)
    };
    let adapter = state
        .registry
        .get(&account.harness)
        .ok_or_else(|| AppError::UnknownHarness(account.harness.to_string()))?;
    if !adapter
        .capabilities()
        .contains(juggler_adapter::Capabilities::DESKTOP_APP)
    {
        return Err(AppError::Validation {
            subject: "desktop launch",
            message: format!("provider {} ships no desktop app", account.harness.as_str()),
        });
    }
    // Idempotent account-dir preparation (the same contract terminal
    // launches honour) — the launch cwd must exist before spawn.
    adapter.prepare_account(&account.state_dir)?;
    let launch = adapter.desktop_launch(&account)?;
    let session_id = SessionId::new(format!("sess-{}", unique_suffix()));
    let project_id = {
        let storage = lock_storage_arc(state)?;
        storage.projects().touch(&launch.cwd, None)?.id.to_string()
    };
    let spawned = state.supervisor.spawn_desktop(
        &session_id,
        &launch,
        &ledger_ctx(state, &account, Some(project_id)),
    )?;
    {
        // The machine identity locks storage itself — resolve it
        // BEFORE the insert-scope lock (non-reentrant mutex).
        let machine = local_machine_id(state);
        let storage = lock_storage_arc(state)?;
        let project = storage.projects().touch(&launch.cwd, None)?;
        storage.sessions().insert(&SessionRecord {
            id: session_id.clone(),
            conversation: None,
            profile: recipe_id,
            machine: machine.clone(),
            kind: juggler_core::model::InstanceKind::Desktop,
            project: Some(project.id.clone()),
            cwd: launch.cwd.clone(),
            status: SessionStatus::Starting,
            started_at: Timestamp::now(),
            last_activity: None,
            usage: juggler_core::model::UsageSnapshot::default(),
            attention: None,
            running: None,
            // Window identity recorded at launch (AJ-E414 Req.2): the
            // owning PID + bundle id; the title is adopted at first
            // sight (first focus attempt) and re-discovered thereafter.
            window: Some(juggler_core::model::WindowIdentity {
                pid: spawned.pid,
                bundle_id: launch.bundle_id.clone(),
                title: None,
                cgwindow_id: None,
            }),
            transcript_ref: None,
        })?;
    }
    Ok(DesktopLaunchInfo {
        session_id: session_id.to_string(),
        pid: spawned.pid,
        bundle_id: launch.bundle_id,
        isolation_label: launch.isolation_label.to_string(),
    })
}

/// A focus attempt's honest result (AJ-E414 Req.5): the copy-exact
/// outcome label, whether the Accessibility grant was missing (the
/// attempt degraded to app activation), and the window title the
/// attempt targeted (after first-sight adoption / re-discovery).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct FocusOutcomeInfo {
    /// `Focused window` | `App opened — window not found` |
    /// `App not running — Relaunch?` | platform-unavailable note.
    pub outcome: String,
    /// True when tiers (b)/(c) were impossible without the
    /// Accessibility permission — the UI shows its explainer.
    pub permission_missing: bool,
    /// The window title this attempt targeted, once known.
    pub window_title: Option<String>,
}

/// The window identity for a desktop Instance: recorded at launch, or
/// synthesized for pre-v10 rows (bundle id from the adapter spec; pid 0
/// forces re-discovery). Shared by the focus tiers and the preview.
fn desktop_window_identity(
    state: &Arc<AppState>,
    record: &juggler_core::model::InstanceRecord,
) -> AppResult<juggler_core::model::WindowIdentity> {
    if let Some(identity) = record.window.clone() {
        return Ok(identity);
    }
    let bundle_id = {
        let storage = lock_storage_arc(state)?;
        let account = storage
            .accounts()
            .get(&storage.profiles().get(&record.profile)?.account)?;
        state
            .registry
            .get(&account.harness)
            .and_then(|adapter| adapter.desktop_launch(&account).ok())
            .and_then(|launch| launch.bundle_id)
    };
    Ok(juggler_core::model::WindowIdentity {
        pid: 0,
        bundle_id,
        title: None,
        cgwindow_id: None,
    })
}

/// Runs the AJ-E414 focus tiers for one desktop Instance: activate the
/// app, match the recorded window identity, clear `AXMinimized`, then
/// `AXRaise` (see `focus.rs` for the mechanism pin). The outcome says
/// exactly which tier ran — app activation alone is never reported as
/// window focus success.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Instance is unknown
/// or not desktop-kind (focus owns windows, terminals own panels).
pub fn desktop_focus(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> AppResult<FocusOutcomeInfo> {
    desktop_focus_with(
        state.inner(),
        &session_id,
        std::sync::Arc::new(focus::OsascriptRunner),
    )
}

/// The testable core (tests inject a scripted runner).
///
/// # Errors
///
/// Same as [`desktop_focus`].
pub fn desktop_focus_with(
    state: &Arc<AppState>,
    session_id: &str,
    runner: std::sync::Arc<dyn focus::ScriptRunner>,
) -> AppResult<FocusOutcomeInfo> {
    let mut record = {
        let storage = lock_storage_arc(state)?;
        storage
            .sessions()
            .get(&SessionId::new(session_id.to_string()))?
    };
    if record.kind != juggler_core::model::InstanceKind::Desktop {
        return Err(AppError::Validation {
            subject: "desktop focus",
            message: "window focus applies to desktop instances".into(),
        });
    }
    if is_terminal(record.status) {
        return Ok(FocusOutcomeInfo {
            outcome: focus::OUTCOME_NOT_RUNNING.to_string(),
            permission_missing: false,
            window_title: record.window.as_ref().and_then(|w| w.title.clone()),
        });
    }
    let identity = desktop_window_identity(state, &record)?;
    // The tier engine is platform-independent over the runner seam
    // (CI drives it everywhere); the platform gate lives in the
    // command wrapper above.
    let attempt = focus::FocusService::new(runner).attempt(&identity);
    // Persist the (possibly enriched) identity — first-sight titles and
    // re-discovered PIDs survive the attempt (Req.2).
    if attempt.identity != record.window {
        record.window.clone_from(&attempt.identity);
        let storage = lock_storage_arc(state)?;
        storage.sessions().update(&record)?;
    }
    Ok(FocusOutcomeInfo {
        outcome: attempt.outcome.to_string(),
        permission_missing: attempt.permission_missing,
        window_title: attempt
            .identity
            .as_ref()
            .and_then(|identity| identity.title.clone()),
    })
}

/// Wire payload for one preview frame (AJ-E415): a base64 PNG of the
/// Instance's window plus its capture time (frame age) and sequence.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct PreviewFrameInfo {
    /// Owning Instance id.
    pub session_id: String,
    /// Base64-encoded PNG bytes of the captured window.
    pub png_b64: String,
    /// Capture time, epoch milliseconds (the panel labels frame age).
    #[ts(type = "number")]
    pub at_ms: i64,
    /// Monotonic frame sequence.
    #[ts(type = "number")]
    pub seq: u64,
}

/// Wire payload for a preview phase change (AJ-E415): the phase name
/// (`live|paused|denied|unsupported`) and the copy-exact panel text.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct PreviewStateInfo {
    /// Owning Instance id.
    pub session_id: String,
    /// `live` | `paused` | `denied` | `unsupported`.
    pub phase: String,
    /// Copy-exact text the panel renders for the phase (empty for live).
    pub text: String,
}

/// The one live preview session (Req.5: one at a time — the selected
/// Instance). Stop is a flag + a done channel (bounded join).
pub struct PreviewSession {
    /// The Instance whose window is mirrored.
    pub session_id: String,
    /// Stop flag (the loop polls it every tick).
    pub stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Completes when the loop thread ended.
    pub done: std::sync::mpsc::Receiver<()>,
}

/// Translates one driver event into the wire events (`preview://frame`
/// / `preview://state`) with the copy-exact phase texts.
fn emit_preview_event(app: &tauri::AppHandle, event: crate::preview::PreviewEvent) {
    use tauri::Emitter;
    match event {
        crate::preview::PreviewEvent::Frame {
            session_id,
            png_b64,
            at_ms,
            seq,
        } => {
            let _ = app.emit(
                crate::EVENT_PREVIEW_FRAME,
                PreviewFrameInfo {
                    session_id,
                    png_b64,
                    at_ms,
                    seq,
                },
            );
        }
        crate::preview::PreviewEvent::Phase { session_id, phase } => {
            let text = match phase {
                crate::preview::PreviewPhase::Live => String::new(),
                crate::preview::PreviewPhase::Paused => {
                    crate::preview::PAUSED_NOT_VISIBLE.to_string()
                }
                crate::preview::PreviewPhase::Denied => {
                    "Screen Recording permission needed — grant it in System Settings, then re-enable preview"
                        .to_string()
                }
                crate::preview::PreviewPhase::Unsupported => {
                    // D-17 unblocked macOS; this phase is now only the
                    // honest non-macOS answer.
                    "Live preview is not supported on this platform".to_string()
                }
            };
            let _ = app.emit(
                crate::EVENT_PREVIEW_STATE,
                PreviewStateInfo {
                    session_id,
                    phase: phase.as_str().to_string(),
                    text,
                },
            );
        }
    }
}

/// Stops any running preview and waits ≤900 ms for the thread (the
/// poll cadence bounds the wait; a late exit leaks nothing — one
/// capture per poll, nothing persistent).
fn stop_preview_session(state: &Arc<AppState>) {
    let session = state
        .preview
        .lock()
        .map_err(|_| AppError::StatePoisoned)
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(session) = session {
        session
            .stop
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if session
            .done
            .recv_timeout(Duration::from_millis(900))
            .is_err()
        {
            eprintln!(
                "ai-juggler: preview thread for {} outlived the stop budget",
                session.session_id
            );
        }
    }
}

/// Starts the read-only live preview for one desktop Instance
/// (AJ-E415): strictly one at a time (the previous stream stops
/// first); requires the Account's explicit preview opt-in; frames and
/// phase changes arrive as `preview://frame` / `preview://state`
/// events. Read-only by construction — pixels flow app → panel only.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message for a non-desktop Instance,
/// a missing opt-in, or an unknown id.
pub fn preview_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> AppResult<()> {
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    let (record, identity) = {
        let storage = lock_storage(&state)?;
        let record = storage
            .sessions()
            .get(&SessionId::new(session_id.clone()))?;
        if record.kind != juggler_core::model::InstanceKind::Desktop {
            return Err(AppError::Validation {
                subject: "live preview",
                message: "preview mirrors desktop instance windows".into(),
            });
        }
        if is_terminal(record.status) {
            return Ok(()); // nothing to mirror; the panel renders status only
        }
        let identity = desktop_window_identity(state.inner(), &record)?;
        (record, identity)
    };
    // The Account's explicit consent gate (Req.3) — default off.
    {
        let storage = lock_storage(&state)?;
        let recipe = storage.profiles().get(&record.profile)?;
        let account = storage.accounts().get(&recipe.account)?;
        if !account.preview_opt_in {
            return Err(AppError::Validation {
                subject: "live preview",
                message: format!(
                    "preview is off for {} — enable it in Account Actions",
                    account.label
                ),
            });
        }
    }
    stop_preview_session(state.inner());

    let (done_tx, done_rx) = mpsc::channel();
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let session = session_id.clone();
    let live_id = SessionId::new(session_id.clone());
    let state_for_thread = Arc::clone(state.inner());
    let app = app.clone();
    std::thread::spawn(move || {
        let emit = move |event: crate::preview::PreviewEvent| {
            emit_preview_event(&app, event);
        };
        // D-17: the REAL capture source on macOS — the audited init
        // shim (crates/juggler-scap-init) satisfies the binding's init
        // requirement inside the workspace's unsafe-free law. Other
        // platforms keep the honest unsupported source.
        #[cfg(target_os = "macos")]
        let source: std::sync::Arc<dyn crate::preview::CaptureSource> =
            std::sync::Arc::new(crate::preview::ScreenshotSource);
        #[cfg(not(target_os = "macos"))]
        let source: std::sync::Arc<dyn crate::preview::CaptureSource> =
            std::sync::Arc::new(crate::preview::UnsupportedSource);
        let driver = crate::preview::PreviewDriver {
            session_id: session.as_str(),
            identity,
            source: source.as_ref(),
            is_live: &|| state_for_thread.supervisor.is_supervised(&live_id),
            emit: &emit,
            stopped: stop_flag.as_ref(),
        };
        driver.run();
        let _ = done_tx.send(());
    });
    state
        .preview
        .lock()
        .map_err(|_| AppError::StatePoisoned)?
        .replace(PreviewSession {
            session_id,
            stop,
            done: done_rx,
        });
    Ok(())
}

/// Stops the live preview (panel close / selection switch). Bounded
/// teardown: the stop flag ends the loop within one poll interval.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on state poisoning.
pub fn preview_stop(state: tauri::State<'_, Arc<AppState>>) -> AppResult<()> {
    stop_preview_session(state.inner());
    Ok(())
}

/// Sets the per-Account live-preview opt-in (Req.3's permanent off
/// switch; enabling is the Screen-Recording consent moment — the first
/// start triggers the system prompt at that point, never before).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Account is unknown.
pub fn account_set_preview_opt_in(
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
    opt_in: bool,
) -> AppResult<AccountInfo> {
    let account = {
        let storage = lock_storage(&state)?;
        let mut account = storage
            .accounts()
            .get(&AccountId::new(account_id.clone()))?;
        account.preview_opt_in = opt_in;
        storage.accounts().update(&account)?;
        account
    };
    Ok(account_info_of(&account))
}

/// Reads one app-level setting (schema v10 kv); `None` = never set.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on storage failures.
pub fn app_setting_get(
    state: tauri::State<'_, Arc<AppState>>,
    key: String,
) -> AppResult<Option<String>> {
    let storage = lock_storage(&state)?;
    storage.app_settings().get(&key).map_err(AppError::from)
}

/// Writes one app-level setting (empty value = reset to default).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message on storage failures.
pub fn app_setting_set(
    state: tauri::State<'_, Arc<AppState>>,
    key: String,
    value: String,
) -> AppResult<()> {
    let storage = lock_storage(&state)?;
    storage
        .app_settings()
        .set(&key, &value)
        .map_err(AppError::from)
}

/// Opens the system Accessibility privacy pane (the deep link behind
/// the AJ-E414 permission explainer; the link is fixed — never a
/// caller-supplied URL).
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the pane cannot open.
pub fn open_accessibility_settings(app: tauri::AppHandle) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            None::<&str>,
        )
        .map_err(|e| AppError::Io {
            context: "open Accessibility settings",
            message: e.to_string(),
        })
}

/// Rebinds the global "Go to instance…" hotkey (AJ-E414 Req.4). The
/// new binding is registered with the OS FIRST; only a successful
/// registration persists it — and a failed registration restores the
/// previous one, so the user is never left without a hotkey.
#[tauri::command]
///
/// # Errors
///
/// Returns the typed [`AppError`] message for an empty binding or an
/// OS registration failure (conflict, unparseable accelerator).
pub fn go_to_hotkey_set(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    hotkey: String,
) -> AppResult<()> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err(AppError::Validation {
            subject: "hotkey",
            message: "empty binding — the default is restored by setting it explicitly".into(),
        });
    }
    let previous = {
        let storage = lock_storage(&state)?;
        storage
            .app_settings()
            .get(crate::GO_TO_HOTKEY_KEY)
            .map_err(AppError::from)?
    };
    let shortcuts = app.global_shortcut();
    if let Some(previous) = &previous {
        let _ = shortcuts.unregister(previous.as_str());
    }
    match shortcuts.register(hotkey.as_str()) {
        Ok(()) => {
            let storage = lock_storage(&state)?;
            storage
                .app_settings()
                .set(crate::GO_TO_HOTKEY_KEY, &hotkey)
                .map_err(AppError::from)?;
            Ok(())
        }
        Err(error) => {
            // Best-effort restore of the previous binding.
            if let Some(previous) = &previous {
                let _ = shortcuts.register(previous.as_str());
            }
            Err(AppError::Io {
                context: "register hotkey",
                message: format!("{hotkey}: {error}"),
            })
        }
    }
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
    let labels = desktop_isolation_labels(&state.registry);
    let adopted = state.supervisor.alive_adopted_sessions();
    let storage = lock_storage(&state)?;
    session_infos(&storage, &live, &labels, &adopted)
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
    let ledger_degraded = state.supervisor.ledger_degraded();
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
        ledger_degraded,
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
        LifecycleEvent::Started { session, .. }
        | LifecycleEvent::Exited { session, .. }
        | LifecycleEvent::HelperOrphaned { session, .. } => session,
    };
    let mut record = storage.sessions().get(session_id).ok()?;
    match event {
        LifecycleEvent::Started { .. } => {
            record.status = SessionStatus::Running;
            record.attention = None;
        }
        // AJ-E504: helper-level delta — the SESSION record does not
        // change (the root still owns the row state); the Cleanup
        // Centre (AJ-E506) consumes these via the ledger.
        LifecycleEvent::HelperOrphaned { .. } => return None,
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
    let drained: Vec<SessionId> = state.supervisor.drain_activity();
    for session in &drained {
        if let Some(running) = state.heuristics.recover(session) {
            debug_assert_eq!(running, SessionStatus::Running);
            {
                let storage = state.storage.lock().map_err(|_| AppError::StatePoisoned)?;
                if apply_attention_recovery(&storage, session).is_some() {
                    changed = true;
                }
            }
        }
    }
    if changed {
        emit_sessions(app, state)?;
    }
    // AJ-E412 Req.2: the same fresh-output signal drives transcript
    // tail-scans for the running model (only the active Instances) —
    // the event path keeps the <300 ms budget; `detect_running_models`
    // emits `sessions_updated` itself when a model changed.
    if !drained.is_empty() {
        detect_running_models(app, state, Some(&drained));
    }
    Ok(())
}

/// The model a transcript line records for its turn — top-level
/// `model` or nested `message.model` (both shapes ship in the wild).
fn line_model(value: &serde_json::Value) -> Option<String> {
    value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("message")
                .and_then(|message| message.get("model"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_string)
}

/// Aggregated transcript facts feeding the quality signals.
struct TranscriptScan {
    events: Vec<juggler_core::quality::UsageEvent>,
    errors: u64,
    turns: u64,
    context_used: Option<u64>,
    context_window: Option<u64>,
}

/// Scans the native transcript JSONL for per-turn usage events,
/// timestamps, error markers and the last reported context pair.
/// Fixture-compatible shapes; Claude transcripts carry `timestamp` +
/// `usage` on assistant lines.
fn scan_transcript(record: &SessionRecord, now: i64) -> TranscriptScan {
    let mut scan = TranscriptScan {
        events: Vec::new(),
        errors: 0,
        turns: 0,
        context_used: None,
        context_window: None,
    };
    let bytes = record
        .transcript_ref
        .as_ref()
        .and_then(|transcript| std::fs::read(&transcript.path).ok());
    let Some(bytes) = bytes else {
        return scan;
    };
    for line in String::from_utf8_lossy(&bytes).lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("usage").is_none() {
            continue;
        }
        scan.turns += 1;
        let usage = &value["usage"];
        let tokens = juggler_core::model::TokenCounts {
            input: usage["input_tokens"].as_u64().unwrap_or(0),
            output: usage["output_tokens"].as_u64().unwrap_or(0),
            cache_read: 0,
            cache_write: 0,
            reasoning: 0,
        };
        // Wall clock: RFC3339 `timestamp` when present, else `now`
        // (honest single-point fallback → the per-minute bars degrade
        // to one bucket, never fabricated).
        let at_ms = value["timestamp"]
            .as_str()
            .and_then(chrono_like_ms)
            .unwrap_or(now);
        scan.events.push(juggler_core::quality::UsageEvent {
            at_ms,
            tokens,
            model: line_model(&value),
        });
        if value.get("error").is_some() {
            scan.errors += 1;
        }
        if let Some(used) = value["context_tokens"].as_u64() {
            scan.context_used = Some(used);
            scan.context_window = value["context_window"].as_u64();
        }
    }
    scan
}

/// Computes the D-07 quality signals for one Instance (AJ-E409): reads
/// the native transcript when present (per-turn usage + timestamps +
/// error markers); every field is a labelled signal or its honest
/// empty — never a composite score.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
#[tauri::command]
pub fn instance_quality(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> AppResult<QualitySignalsInfo> {
    let storage = lock_storage(&state)?;
    let record = storage
        .sessions()
        .get(&SessionId::new(session_id.clone()))
        .map_err(AppError::from)?;
    let now = Timestamp::now().millis();
    let scan = scan_transcript(&record, now);

    let tpm = juggler_core::quality::signals::tokens_per_min(&scan.events, now);
    let error_label = if scan.turns == 0 {
        "Not measurable (interactive)".to_string()
    } else {
        format!("{} errors / {} turns", scan.errors, scan.turns)
    };
    let table = juggler_core::quality::load_pricing_table();
    let recipe = storage.profiles().get(&record.profile).ok();
    let provider = storage
        .accounts()
        .list()
        .ok()
        .and_then(|accounts| {
            accounts
                .iter()
                .find(|a| recipe.as_ref().is_some_and(|r| r.account == a.id))
                .map(|a| a.harness.to_string())
        })
        .unwrap_or_default();
    // AJ-E412 Req.4: price per turn where the transcript records the
    // model; turns without one stay in the token totals but are
    // excluded from $. Falls back to tokens-only when no turn was
    // priceable (never to the launch overlay — that is intent, not
    // telemetry).
    let (cost, priced_model) = match juggler_core::quality::pricing::session_cost_per_turn(
        &table,
        &provider,
        &scan.events,
    ) {
        Some((total, models)) => (Some(total), Some(models.join(", "))),
        None => (None, None),
    };
    // Forecast: from quota deltas — none are observable yet, so idle
    // honestly until the usage engine (AJ-E606+) feeds burn rates.
    let forecast = juggler_core::quality::quota_forecast_label(
        record
            .usage
            .window
            .as_ref()
            .map_or(0.0, |w| w.used_percentage),
        0.0,
    );
    Ok(QualitySignalsInfo {
        tokens_bars: tpm.bars.map(|bars| bars.to_vec()),
        avg_tokens_per_min: tpm.avg_per_min,
        error_label,
        errors_not_measurable: scan.turns == 0,
        context_used: scan.context_used,
        context_window: scan.context_window,
        cumulative: TokenCountsInfo {
            input: record.usage.tokens.input,
            output: record.usage.tokens.output,
            cache_read: record.usage.tokens.cache_read,
            cache_write: record.usage.tokens.cache_write,
            reasoning: record.usage.tokens.reasoning,
        },
        session_cost_usd: cost,
        priced_model: priced_model.filter(|_| cost.is_some()),
        quota_forecast: forecast,
        stuck: record.attention.as_ref().map(|reason| AttentionInfo {
            rule: reason.rule.clone(),
            inputs: reason.inputs.clone(),
            since_ms: reason.since_ms,
        }),
    })
}

/// RFC3339-ish timestamp → epoch ms without pulling chrono (the
/// transcripts use `2026-10-13T12:34:56.789Z`-style stamps).
// Fractional seconds land in f64; the ms truncation is below the
// per-minute bar resolution we compute from it.
#[allow(clippy::cast_possible_truncation)]
fn chrono_like_ms(raw: &str) -> Option<i64> {
    let date_time = raw.split('T').nth(1)?.split(['Z', '+']).next()?;
    let mut parts = date_time.split(':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let sec: f64 = parts.next()?.parse().ok()?;
    let day_ms = raw.strip_suffix('Z').map(str::to_string).and_then(|s| {
        let date = s.split('T').next()?.to_string();
        let mut d = date.split('-');
        let y: i64 = d.next()?.parse().ok()?;
        let mo: i64 = d.next()?.parse().ok()?;
        let da: i64 = d.next()?.parse().ok()?;
        Some(days_from_civil(y, mo, da) * 86_400_000)
    })?;
    Some(day_ms + h * 3_600_000 + m * 60_000 + (sec * 1000.0) as i64)
}

/// Days since epoch (Howard Hinnant's civil-from-days inverse).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
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
        session_infos(
            &storage,
            &live,
            &desktop_isolation_labels(&state.registry),
            &state.supervisor.alive_adopted_sessions(),
        )?
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
        cleanup_opt_in: account.cleanup_opt_in,
        renewal_date: account.renewal_date.clone(),
        preview_opt_in: account.preview_opt_in,
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

/// AJ-E412 Req.1 classification: the I-03 display model prefers the
/// OBSERVED running model over the launch intent; the source names how
/// it is known (`in-session` = observed drift from the overlay).
fn display_model(
    running: Option<&juggler_core::model::RunningModel>,
    launch: Option<&String>,
) -> (Option<String>, String) {
    let observed = running.map(|entry| entry.model.clone());
    let source = match (&observed, launch) {
        (Some(running_model), Some(launch_model)) if running_model != launch_model => "in-session",
        (Some(_), _) => "transcript",
        (None, Some(_)) => "launch",
        (None, None) => "unknown",
    };
    (observed.or_else(|| launch.cloned()), source.to_string())
}

/// Per-Provider desktop isolation claims (AJ-E413 Req.1): provider id →
/// the label shown verbatim on that provider's desktop Instances.
/// Providers without a desktop app contribute nothing.
fn desktop_isolation_labels(
    registry: &juggler_adapter::AdapterRegistry,
) -> std::collections::BTreeMap<String, String> {
    registry
        .ids()
        .into_iter()
        .filter_map(|harness| {
            registry
                .get(&harness)
                .and_then(|adapter| adapter.desktop_isolation_label())
                .map(|label| (harness.to_string(), label.to_string()))
        })
        .collect()
}

/// Session rows with denormalized account/provider joins and the full
/// I-01..I-11 read model (AJ-E401). One storage pass per collection
/// (accounts, launch settings, instances) joined in memory — no per-row
/// storage reads; absent values serialise as `null`, never `0` or `""`.
/// Test/inspection surface: session rows with an empty live-stats map.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on storage failures.
pub fn session_infos_snapshot(storage: &Storage) -> AppResult<Vec<InstanceInfo>> {
    session_infos(
        storage,
        &LiveStatsMap::new(),
        &std::collections::BTreeMap::new(),
        &[],
    )
}

/// account id → (label, provider).
type AccountIndex = std::collections::BTreeMap<String, (String, String)>;
/// launch-recipe id → (account id, provider, overlay model).
type RecipeIndex = std::collections::BTreeMap<String, (String, String, Option<String>)>;

/// Join indexes for the read model: account id → (label, provider) and
/// launch-recipe id → (account id, provider, overlay model). One
/// storage pass per collection, joined in memory — no per-row reads.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on storage failures.
fn session_join_indexes(storage: &Storage) -> AppResult<(AccountIndex, RecipeIndex)> {
    let account_index: AccountIndex = storage
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
    let mut recipe_index: RecipeIndex = std::collections::BTreeMap::new();
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
    Ok((account_index, recipe_index))
}

fn session_infos(
    storage: &Storage,
    live: &LiveStatsMap,
    desktop_labels: &std::collections::BTreeMap<String, String>,
    adopted_ids: &[juggler_core::model::SessionId],
) -> AppResult<Vec<InstanceInfo>> {
    let (account_index, recipe_index) = session_join_indexes(storage)?;
    Ok(storage
        .sessions()
        .list(None)?
        .into_iter()
        .map(|record| {
            let (account_id, account_label, provider, launch_model) =
                resolved_join(&recipe_index, &account_index, &record.profile.to_string());
            let isolation_label = if record.kind == juggler_core::model::InstanceKind::Desktop {
                desktop_labels.get(&provider).cloned()
            } else {
                None
            };
            let (model, model_source) =
                display_model(record.running.as_ref(), launch_model.as_ref());
            let terminal = is_terminal(record.status);
            let stats = if terminal {
                None
            } else {
                live.get(&record.id.to_string()).copied()
            };
            InstanceInfo {
                adopted: adopted_ids.contains(&record.id),
                id: record.id.to_string(),
                launch_recipe_id: record.profile.to_string(),
                provider,
                account_id,
                account_label,
                model,
                launch_model,
                running_model: record
                    .running
                    .as_ref()
                    .map(|observed| observed.model.clone()),
                model_source,
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
                isolation_label,
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
/// The hand-over source row + its owning account (the read-side
/// preamble of [`session_handover_impl`]).
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the source session or
/// its account cannot be resolved.
fn handover_source(
    state: &Arc<AppState>,
    session_id: &str,
) -> AppResult<(juggler_core::model::InstanceRecord, Account)> {
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
    Ok((source, account_a))
}

/// Resumes the session under the target account (see the module docs).
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
    let (source, account_a) = handover_source(state, session_id)?;
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
    let spawned = state.supervisor.spawn(
        &new_session,
        &spec,
        &ledger_ctx(
            state,
            &account_b,
            source
                .project
                .as_ref()
                .map(std::string::ToString::to_string),
        ),
    )?;

    {
        // The machine identity locks storage itself — resolve it
        // BEFORE the insert-scope lock (non-reentrant mutex).
        let machine = local_machine_id(state);
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
            machine: machine.clone(),
            kind: juggler_core::model::InstanceKind::Terminal,
            project: source.project,
            cwd: source.cwd.clone(),
            status: SessionStatus::Starting,
            started_at: Timestamp::now(),
            last_activity: None,
            usage: juggler_core::model::UsageSnapshot::default(),
            attention: None,
            running: None,
            window: None,
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
    let sessions = session_infos(
        &storage,
        &live_stats,
        &desktop_isolation_labels(&state.registry),
        &state.supervisor.alive_adopted_sessions(),
    )?;
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

// ---------------------------------------------------------------------------
// AJ-E410: the Processes tab data, the orphan contract, and the Tier-1
// auto-cleanup engine (D-08).
// ---------------------------------------------------------------------------

/// One live process row for the Processes tab (PID / name / CPU / RAM /
/// age / child count / depth).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct ProcessRowInfo {
    /// OS process id.
    pub pid: u32,
    /// Parent pid, when readable.
    #[ts(type = "number | null")]
    pub parent_pid: Option<u32>,
    /// Process name.
    pub name: String,
    /// CPU utilization, percent; `null` until two table samples.
    pub cpu_pct: Option<f64>,
    /// Resident memory, bytes.
    #[ts(type = "number")]
    pub rss_bytes: u64,
    /// Process age, ms.
    #[ts(type = "number")]
    pub age_ms: u64,
    /// Direct children within the tree.
    #[ts(type = "number")]
    pub child_count: u32,
    /// Tree depth (root = 0) — the UI indent level.
    pub depth: u8,
}

/// One I-07 sparkline point (2 s sampling, 5 min retained).
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct TreeSampleInfo {
    /// Sample time, epoch ms.
    #[ts(type = "number")]
    pub at_ms: i64,
    /// Tree CPU%, percent; `null` before two samples.
    pub cpu_pct: Option<f64>,
    /// Tree RSS sum, bytes.
    #[ts(type = "number")]
    pub rss_bytes: u64,
}

/// One confirmed orphan row (`Orphaned - left behind (N)` section).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct OrphanInfo {
    /// Instance the process was left behind by.
    pub session_id: String,
    /// Orphan process id.
    pub pid: u32,
    /// Process name.
    pub name: String,
    /// Resident memory, bytes.
    #[ts(type = "number")]
    pub rss_bytes: u64,
    /// Label of the dead parent (its program name).
    pub parent_label: String,
    /// Age since the parent exited, ms.
    #[ts(type = "number")]
    pub age_since_parent_exit_ms: i64,
    /// Last stop state for this pid (`gone` / `still alive - escalated to
    /// SIGKILL` / …), when one exists.
    pub stop_state: Option<String>,
}

/// The Processes tab payload for one Instance (AJ-E410 Req.2/3).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct ProcessesInfo {
    /// Live tree rows (root first, breadth-first descendants).
    pub rows: Vec<ProcessRowInfo>,
    /// 5-minute 2 s resource ring (the I-07 sparkline source).
    pub spark: Vec<TreeSampleInfo>,
    /// Confirmed orphans for this Instance.
    pub orphans: Vec<OrphanInfo>,
    /// Honest unavailability note (adopted/deskto-kind without a tree):
    /// `resources unavailable`, never flat-zero rows.
    pub unavailable_reason: Option<String>,
}

/// Outcome payload for orphan stop actions.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct StopOutcomeInfo {
    /// The pid the action targeted.
    pub pid: u32,
    /// UI state: `gone`, `still alive - escalated to SIGKILL`, or the
    /// refusal reason.
    pub state: String,
}

/// `Verify owner` payload: the re-checked ppid chain evidence.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../ui/src/types/generated/")]
pub struct OrphanVerifyInfo {
    /// The pid examined.
    pub pid: u32,
    /// True when the process is alive and non-zombie.
    pub alive: bool,
    /// Current parent pid, when readable.
    #[ts(type = "number | null")]
    pub parent_pid: Option<u32>,
    /// Human note (e.g. `re-parented to init/launchd`).
    pub note: String,
}

/// The live Processes tab payload for one Instance (rows + spark +
/// orphans); rollups above Instance level aggregate several of these
/// read-only in the UI (Req.2).
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Instance is unknown.
#[tauri::command]
pub fn instance_processes(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> AppResult<ProcessesInfo> {
    instance_processes_arc(&state, &session_id)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Instance is unknown.
pub fn instance_processes_arc(state: &Arc<AppState>, session_id: &str) -> AppResult<ProcessesInfo> {
    let (record_exists, supervised) = {
        let storage = lock_storage_arc(state)?;
        let exists = storage
            .sessions()
            .get(&SessionId::new(session_id.to_string()))
            .is_ok();
        (
            exists,
            state
                .supervisor
                .is_supervised(&SessionId::new(session_id.to_string())),
        )
    };
    if !record_exists {
        return Err(AppError::from(CoreError::NotFound {
            entity: "instance",
            id: session_id.to_string(),
        }));
    }
    let session = SessionId::new(session_id.to_string());
    let session_id = session_id.to_string();
    let (rows, spark, unavailable) = {
        let Ok(collector) = state.resources.lock() else {
            return Ok(unavailable_processes("resources unavailable"));
        };
        let rows = collector.rows(&session).unwrap_or_default();
        let spark = collector.ring(&session);
        // Adopted/desktop-kind sessions this app never supervised have
        // no tree to read: honest unavailability, never flat-zero rows.
        let unavailable =
            (!supervised && rows.is_empty()).then(|| "resources unavailable".to_string());
        (rows, spark, unavailable)
    };
    let now = Timestamp::now().millis();
    let orphans = state
        .orphans
        .orphans_of(&session)
        .into_iter()
        .map(|orphan| OrphanInfo {
            session_id: session_id.clone(),
            pid: orphan.pid,
            name: orphan.name,
            rss_bytes: orphan.rss_bytes,
            parent_label: orphan.parent_label,
            age_since_parent_exit_ms: now - orphan.parent_exit_ms,
            stop_state: state
                .orphans
                .stop_state(orphan.pid)
                .map(|outcome| outcome.state_label().to_string()),
        })
        .collect();
    Ok(ProcessesInfo {
        rows: rows
            .into_iter()
            .map(|row| ProcessRowInfo {
                pid: row.pid,
                parent_pid: row.parent_pid,
                name: row.name,
                cpu_pct: row.cpu_pct,
                rss_bytes: row.rss_bytes,
                age_ms: row.age_ms,
                child_count: row.child_count,
                depth: row.depth,
            })
            .collect(),
        spark: spark
            .into_iter()
            .map(|sample| TreeSampleInfo {
                at_ms: sample.at_ms,
                cpu_pct: sample.cpu_pct,
                rss_bytes: sample.rss_bytes,
            })
            .collect(),
        orphans,
        unavailable_reason: unavailable,
    })
}

/// An honest `resources unavailable` payload (Req.7 — never flat zeros).
fn unavailable_processes(reason: &str) -> ProcessesInfo {
    ProcessesInfo {
        rows: Vec::new(),
        spark: Vec::new(),
        orphans: Vec::new(),
        unavailable_reason: Some(reason.to_string()),
    }
}

// --- AJ-E505: the port occupancy map ----------------------------------

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|dur| i64::try_from(dur.as_millis()).unwrap_or_default())
        .unwrap_or_default()
}

/// One port-inventory row (the API DTO; `NotManaged` rows carry
/// `killable: false` and NO stop/kill command ids — the F3 guard).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct PortRowInfo {
    /// The listening port (the headline).
    pub port: u16,
    /// The owning process id (the evidence).
    pub pid: u32,
    /// `managed-orphan` | `managed-running` | `re-adopted` |
    /// `not-managed`.
    pub class: String,
    /// Owning Instance, when managed.
    pub session: Option<String>,
    /// Provider id, when managed.
    pub provider: Option<String>,
    /// Account id, when managed.
    pub account: Option<String>,
    /// Project id, when managed.
    pub project: Option<String>,
    /// Listener age from the kernel process start, ms.
    pub listener_age_ms: Option<i64>,
    /// Process display name, when managed.
    pub name: Option<String>,
    /// The well-known dev-port hint (a LABEL, never identity).
    pub hint: Option<String>,
    /// False exactly for `not-managed` rows: no kill affordance, ever.
    pub killable: bool,
}

/// One sustained-breach hog fact (Req.5 — the Cleanup Centre's input).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct HogFactInfo {
    /// The hog pid (helper for RSS rules, the root for tree rules).
    pub pid: u32,
    /// Owning Instance.
    pub session: String,
    /// Provider id.
    pub provider: String,
    /// Live (or last-seen) RSS, bytes.
    pub rss_bytes: u64,
    /// Live CPU%, when sampled.
    pub cpu_pct: Option<f64>,
    /// Which rule fired: `helper-rss>2GiB` | `tree-cpu>90%` |
    /// `tree-rss>p95(provider)`.
    pub rule: String,
}

/// The inventory response: rows + the honest probe note + hog facts.
#[derive(Clone, Debug, serde::Serialize, PartialEq, Default)]
pub struct PortInventoryInfo {
    /// Rows sorted by port.
    pub rows: Vec<PortRowInfo>,
    /// `Some` when the platform has no probe or the OS call failed —
    /// the honest note, never an empty map presented as truth.
    pub note: Option<String>,
    /// Sustained-breach hogs (60 s window).
    pub hogs: Vec<HogFactInfo>,
}

/// The pre-launch collision warning (advisor informs, never blocks).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct PortWarningInfo {
    /// The contested port.
    pub port: u16,
    /// The orphan holding it.
    pub pid: u32,
    /// The Instance label (`Provider (id-prefix)`).
    pub instance: String,
    /// How long the holder has lived, ms.
    pub age_ms: i64,
    /// The copy-exact warning (Req.4).
    pub message: String,
}

/// The port inventory (Req.3): every listener classified as exactly
/// one of managed-orphan / managed-running / re-adopted /
/// not-managed; unmanaged rows are display-only.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
#[tauri::command]
pub fn port_inventory(state: tauri::State<'_, Arc<AppState>>) -> AppResult<PortInventoryInfo> {
    Ok(port_inventory_inner(&state))
}

/// The pre-launch warning (Req.4): a project's expected dev port
/// (learned from its instances' observed listeners) held by a managed
/// orphan. Launch itself is NEVER blocked — the UI offers `Launch
/// anyway` beside `Free this port`.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
#[tauri::command]
pub fn launch_port_warning(
    state: tauri::State<'_, Arc<AppState>>,
    project_id: Option<String>,
) -> AppResult<Option<PortWarningInfo>> {
    Ok(launch_port_warning_inner(&state, project_id))
}

/// Humanized age for the copy-exact warning: `45s` / `12m` / `3h` /
/// `2d`.
fn human_age(ms: i64) -> String {
    let secs = ms.div_euclid(1000).max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3_600)
    } else {
        format!("{}d", secs / 86_400)
    }
}

/// The real probe for THIS platform (query-time freshness; the tick
/// join stores the durable view).
fn os_listeners() -> Result<Vec<juggler_supervisor::Listener>, String> {
    use juggler_supervisor::PortProbe as _;
    #[cfg(unix)]
    let probe = juggler_supervisor::NetstatProbe;
    #[cfg(not(unix))]
    let probe = juggler_supervisor::UnsupportedProbe;
    probe.listeners().map_err(|error| error.to_string())
}

/// Every ledger row of the local machine's sessions (all states —
/// dead roots matter for classification).
/// EVERY ledger row, all machines (AJ-E511: the "All machines"
/// inventory scope — foreign rows are display-only in the UI). Walks
/// the composite keys, then reads each key — the same session on two
/// machines stays two groups.
fn all_ledger_rows(state: &Arc<AppState>) -> Vec<juggler_core::storage::LedgerRow> {
    use juggler_supervisor::tree::LedgerStore as _;
    let mut rows = Vec::new();
    for (session, machine) in state.ledger.live_session_machines().unwrap_or_default() {
        if let Ok(mut key_rows) = state
            .ledger
            .rows_for_session_on_machine(&SessionId::new(session), &MachineId::new(machine))
        {
            rows.append(&mut key_rows);
        }
    }
    rows
}

fn local_ledger_rows(state: &Arc<AppState>) -> Vec<juggler_core::storage::LedgerRow> {
    use juggler_supervisor::tree::LedgerStore as _;
    let machine = local_machine_id(state);
    let mut rows = Vec::new();
    let mut sessions = std::collections::HashSet::new();
    if let Ok(candidates) = state.ledger.candidates(&machine) {
        for row in candidates {
            sessions.insert(row.session_id.clone());
        }
    }
    for session in sessions {
        // COMPOSITE key (AJ-E511): a session may exist on two
        // machines — session-only reads would leak foreign rows into
        // this node's scope (the fleet-scale false-kill trap).
        if let Ok(mut session_rows) = state
            .ledger
            .rows_for_session_on_machine(&SessionId::new(session), &machine)
        {
            rows.append(&mut session_rows);
        }
    }
    rows
}

// --- AJ-E506: the Cleanup Centre (read-only, Tier 0) -------------------

/// One inventory row's verification badge (AJ-E503 at query time).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct VerificationBadge {
    /// `verified` | `unverifiable` | `unknown` (no recorded facts).
    pub verdict: String,
    /// The facts behind the verdict (the expansion's check result).
    pub detail: String,
}

/// The ownership evidence chain (Req.4 — `Why does Juggler think this
/// is mine?`).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct EvidenceChain {
    /// The Instance's root pid (spawned-by).
    pub root_pid: Option<u32>,
    /// First-seen parent pid (the ppid at record time).
    pub parent_pid: Option<u32>,
    /// The row's PGID.
    pub pgid: Option<u32>,
    /// True when the PGID left the root's group (E502's flag).
    pub escaped_group: bool,
    /// The exe/start-time check result (AJ-E503 at query time).
    pub verification: VerificationBadge,
    /// First seen, epoch ms.
    pub first_seen: i64,
    /// Last seen, epoch ms.
    pub last_seen: i64,
}

/// One read-only row (the shared `ProcessRow`'s data). NO kill fields
/// exist on this DTO by construction (Req.7).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct CleanupRow {
    /// The machine that owns this row (AJ-E511) — `local_machine`
    /// on the inventory names this node; foreign rows render
    /// display-only (no kill affordance).
    pub machine_id: String,
    /// OS process id.
    pub pid: u32,
    /// Process-group id.
    pub pgid: Option<u32>,
    /// `root` | `helper`.
    pub role: String,
    /// Ledger state (`running` | `orphaned` | `unverifiable` | `exited`).
    pub state: String,
    /// The pass2 §2 label, VERBATIM — except down-period orphans,
    /// which carry the AJ-E510 label `left behind while Juggler was
    /// closed` (their root died while the app was not running).
    pub state_label: String,
    /// True for helpers of roots that died while Juggler was closed
    /// (AJ-E510 Req.2) — the distinct down-period marker.
    pub down_period: bool,
    /// Role guess — always labelled a guess (E504 note).
    pub role_guess: String,
    /// Executable path (truncated for display, full on expand).
    pub exe_path: String,
    /// Truncated command line.
    pub cmdline: String,
    /// Age from the kernel start time, ms.
    pub age_ms: Option<i64>,
    /// Live RSS bytes (frozen at last tick for exited rows).
    pub rss_bytes: Option<u64>,
    /// Live CPU%.
    pub cpu_pct: Option<f64>,
    /// Ports held (csv) — the collision evidence.
    pub ports: Option<String>,
    /// True when a sustained hog rule fired (E505).
    pub hog: bool,
    /// Which hog rule(s) fired.
    pub hog_rules: Vec<String>,
    /// The verification badge + evidence chain (expansion).
    pub evidence: EvidenceChain,
}

/// One Instance group (healthy trees collapsed by default).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct CleanupGroup {
    /// The Instance (session) id.
    pub session: String,
    /// Provider id.
    pub provider: String,
    /// Account id.
    pub account: Option<String>,
    /// Project id.
    pub project: Option<String>,
    /// Machine id (AJ-E511).
    pub machine: String,
    /// True when the tree is healthy (collapse by default).
    pub healthy: bool,
    /// The rows.
    pub rows: Vec<CleanupRow>,
}

/// One Cleanup Centre history row (AJ-E509: the audit log's read
/// shape, newest first).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct CleanupHistoryInfo {
    /// The audit history (7 days), newest first.
    pub rows: Vec<juggler_core::storage::AuditOutcomeInfo>,
    /// The copy-exact empty note when there are no records.
    pub empty_note: Option<String>,
}

/// The Cleanup Centre response.
#[derive(Clone, Debug, serde::Serialize, PartialEq, Default)]
pub struct CleanupInventoryInfo {
    /// Groups matching the filter.
    pub groups: Vec<CleanupGroup>,
    /// The executing node's machine id (AJ-E511): rows whose
    /// `machine_id` equals this are `This Mac`; anything else is a
    /// future-synced foreign row — display-only.
    pub local_machine: String,
    /// Instances fully clean (the empty-state copy's N).
    pub clean_instances: usize,
    /// True when the ledger is degraded (AJ-E501 banner + read-only).
    pub ledger_degraded: bool,
    /// The History filter's audit rows (AJ-E509 Req.4; `None` for
    /// every other filter).
    pub history: Option<CleanupHistoryInfo>,
}

/// `pass2` §2 state labels, VERBATIM (Req.2).
#[must_use]
pub fn cleanup_state_label(state: &str) -> &'static str {
    match state {
        "orphaned" => "Still running after its Instance closed",
        "re-adopted" | "adopted" => "Running — re-adopted",
        "unverifiable" => "Unverifiable — will not be touched",
        "running" => "Running",
        "exited" => "Exited",
        "starting" => "Starting",
        _ => "Unknown",
    }
}

/// The read-only Cleanup Centre inventory (Tier 0 — zero signalling:
/// this command registers no kill surface and its DTOs carry none).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
#[tauri::command]
pub fn cleanup_inventory(
    state: tauri::State<'_, Arc<AppState>>,
    filter: String,
    machine_scope: Option<String>,
) -> AppResult<CleanupInventoryInfo> {
    Ok(cleanup_inventory_inner(
        &state,
        &filter,
        machine_scope.as_deref() == Some("all"),
    ))
}

/// `cleanup_inventory` sans-Tauri (tests + dogfood evidence).
#[allow(clippy::similar_names)] // row/root_row read naturally here
#[allow(clippy::too_many_lines)] // one pass per filter; splitting hides the grouping rules
pub fn cleanup_inventory_inner(
    state: &Arc<AppState>,
    filter: &str,
    all_machines: bool,
) -> CleanupInventoryInfo {
    let now = now_millis();
    let degraded = state.supervisor.ledger_degraded();
    // History (7 days) reads the AUDIT LOG (AJ-E509 Req.4) — the
    // outcome vocabulary is one language with AJ-E507's.
    if filter == "history" {
        let since = now - 7 * 24 * 3_600_000;
        let rows = state.audit.history(since).unwrap_or_default();
        let empty_note = rows
            .is_empty()
            .then(|| "No cleanups yet — automatic cleanup is off by default.".to_string());
        return CleanupInventoryInfo {
            groups: Vec::new(),
            local_machine: local_machine_id(state).as_str().to_string(),
            clean_instances: 0,
            ledger_degraded: degraded,
            history: Some(CleanupHistoryInfo { rows, empty_note }),
        };
    }
    let adopted: std::collections::HashSet<String> = state
        .supervisor
        .alive_adopted_sessions()
        .into_iter()
        .map(|session| session.as_str().to_string())
        .collect();
    let hogs = hog_facts(state, now);
    let live_rows: std::collections::HashMap<u32, juggler_supervisor::ProcessRow> = state
        .resources
        .lock()
        .ok()
        .map(|collector| {
            collector
                .stats_snapshot()
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .flat_map(|session| {
                    collector
                        .rows(&session)
                        .into_iter()
                        .flatten()
                        .map(|row| (row.pid, row))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default();
    let table = state.supervisor.snapshot_table();

    let mut groups: Vec<CleanupGroup> = Vec::new();
    let mut clean = 0_usize;
    // Group rows per (session, machine) — the D-13 composite key: the
    // same session may exist on two machines and MUST render as two
    // groups (attribution stays joinable, never merged). The default
    // scope reads THIS machine only; "all" adds future-synced
    // foreign rows (display-only in the UI).
    let mut by_key: std::collections::HashMap<
        (String, String),
        Vec<juggler_core::storage::LedgerRow>,
    > = std::collections::HashMap::new();
    for row in if all_machines {
        all_ledger_rows(state)
    } else {
        local_ledger_rows(state)
    } {
        by_key
            .entry((row.session_id.clone(), row.machine_id.clone()))
            .or_default()
            .push(row);
    }
    let mut session_order: Vec<(String, String)> = by_key.keys().cloned().collect();
    session_order.sort();
    for (session, row_machine) in session_order {
        let rows_raw = &by_key[&(session.clone(), row_machine.clone())];
        let provider = rows_raw
            .first()
            .map(|row| row.provider.clone())
            .unwrap_or_default();
        let account = rows_raw.first().and_then(|row| row.account_id.clone());
        let project = rows_raw.first().and_then(|row| row.project_id.clone());
        let machine = rows_raw
            .first()
            .map(|row| row.machine_id.clone())
            .unwrap_or_default();
        let root_row = rows_raw.iter().find(|row| row.role == "root");
        let root_pid = root_row.and_then(|row| row.pid);
        let root_alive = root_row.is_some_and(|row| row.state == "running");
        let session_adopted = adopted.contains(&session);

        let mut rows: Vec<CleanupRow> = Vec::new();
        let mut any_attention = false;
        for row in rows_raw {
            let pid = row.pid.unwrap_or_default();
            if pid == 0 {
                continue;
            }
            // History filter: exited rows within 7 days only.
            if filter == "history" && row.state != "exited" {
                continue;
            }
            if filter != "history" && row.state == "exited" {
                continue;
            }
            // Query-time verification (E503): table facts vs record.
            let verification = if row.state == "unverifiable" {
                VerificationBadge {
                    verdict: "unverifiable".into(),
                    detail: "identity check failed — permanently untouched".into(),
                }
            } else if let Some(facts) = table.as_ref().and_then(|t| t.facts.get(&pid)) {
                let start_ok = row
                    .started_at
                    .is_none_or(|start| start == 0 || start == facts.start_time);
                let exe_ok = !row.exe_path.starts_with('/')
                    || facts.exe.as_deref().is_none_or(|exe| exe == row.exe_path);
                if start_ok && exe_ok {
                    VerificationBadge {
                        verdict: "verified".into(),
                        detail: format!(
                            "start-time and exe match the live process (start {}, exe {:?})",
                            facts.start_time,
                            facts.exe.as_deref().unwrap_or("?")
                        ),
                    }
                } else {
                    VerificationBadge {
                        verdict: "unverifiable".into(),
                        detail: "live start-time or exe differs from the record".into(),
                    }
                }
            } else if row.state == "exited" {
                VerificationBadge {
                    verdict: "unknown".into(),
                    detail: "process exited — no live check applies".into(),
                }
            } else {
                VerificationBadge {
                    verdict: "unknown".into(),
                    detail: "not in the current process table".into(),
                }
            };

            let row_hogs: Vec<&HogFactInfo> = hogs
                .iter()
                .filter(|hog| hog.pid == pid || (hog.pid == 0 && hog.session == session))
                .collect();
            let port_collision = row.state == "orphaned" && row.listens_on.is_some();
            let effective_state = if session_adopted && row.state == "running" {
                "adopted"
            } else {
                row.state.as_str()
            };
            let needs_attention = row.state == "orphaned" || !row_hogs.is_empty() || port_collision;
            if needs_attention {
                any_attention = true;
            }
            // Filter sets.
            let matches = match filter {
                "needs-attention" => needs_attention,
                "re-adopted" => session_adopted,
                _ => true, // all-managed + history (its row gate is above)
            };
            if !matches {
                continue;
            }
            let live = live_rows.get(&pid);
            // AJ-E510 Req.2: the durable down-period marker (set by
            // crash recovery) — distinct from live-session orphans.
            let down_period = effective_state == "orphaned" && row.down_period;
            let state_label = if down_period {
                "left behind while Juggler was closed".to_string()
            } else {
                cleanup_state_label(effective_state).to_string()
            };
            rows.push(CleanupRow {
                machine_id: row.machine_id.clone(),
                pid,
                pgid: row.pgid,
                role: row.role.clone(),
                state: effective_state.to_string(),
                state_label,
                down_period,
                role_guess: format!("{} (guess)", row.exe_path.rsplit('/').next().unwrap_or("?")),
                exe_path: row.exe_path.clone(),
                cmdline: row.cmdline_trunc.clone(),
                age_ms: row
                    .started_at
                    .map(|start| (now - start.saturating_mul(1000)).max(0)),
                rss_bytes: live
                    .map(|l| l.rss_bytes)
                    .or(row.peak_rss.map(i64::unsigned_abs)),
                cpu_pct: live.and_then(|l| l.cpu_pct).or(row.peak_cpu),
                ports: row.listens_on.clone(),
                hog: !row_hogs.is_empty(),
                hog_rules: row_hogs.iter().map(|hog| hog.rule.clone()).collect(),
                evidence: EvidenceChain {
                    root_pid,
                    parent_pid: row.parent_pid,
                    pgid: row.pgid,
                    escaped_group: row.escaped_group,
                    verification,
                    first_seen: row.first_seen,
                    last_seen: row.last_seen,
                },
            });
        }
        if rows.is_empty() {
            continue;
        }
        let healthy = !any_attention && root_alive;
        if healthy && filter == "needs-attention" {
            // Healthy trees hold no attention rows by construction.
        }
        if healthy
            && root_alive
            && rows_raw
                .iter()
                .all(|row| row.state == "running" || row.state == "exited")
        {
            clean += 1;
        }
        groups.push(CleanupGroup {
            session: session.clone(),
            provider,
            account,
            project,
            machine,
            healthy,
            rows,
        });
    }
    CleanupInventoryInfo {
        groups,
        local_machine: local_machine_id(state).as_str().to_string(),
        clean_instances: clean,
        ledger_degraded: degraded,
        history: None,
    }
}

/// Exports the 7-day cleanup audit as JSONL (AJ-E509 Req.4). The
/// first line is the redaction note (itself valid JSON): cmdlines are
/// hashed; full command lines never persisted. User-initiated only.
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
#[tauri::command]
pub fn cleanup_audit_export(state: tauri::State<'_, Arc<AppState>>) -> AppResult<String> {
    Ok(cleanup_audit_export_inner(&state))
}

/// `cleanup_audit_export` sans-Tauri (tests + evidence).
pub fn cleanup_audit_export_inner(state: &Arc<AppState>) -> String {
    let since = now_millis() - 7 * 24 * 3_600_000;
    let rows = state.audit.rows_since(since).unwrap_or_default();
    let mut lines = vec![r#"{"note":"cleanup audit export — command lines are hashed and truncated; full cmdlines are never stored"}"#.to_string()];
    for row in rows {
        if let Ok(line) = serde_json::to_string(&row) {
            lines.push(line);
        }
    }
    lines.join("\n")
}

/// `port_inventory` sans-Tauri (tests + evidence runs).
pub fn port_inventory_inner(state: &Arc<AppState>) -> PortInventoryInfo {
    let now = now_millis();
    let listeners = match os_listeners() {
        Ok(listeners) => listeners,
        Err(note) => {
            return PortInventoryInfo {
                rows: Vec::new(),
                note: Some(note),
                hogs: Vec::new(),
            };
        }
    };
    let rows = local_ledger_rows(state);
    let adopted: std::collections::HashSet<String> = state
        .supervisor
        .alive_adopted_sessions()
        .into_iter()
        .map(|session| session.as_str().to_string())
        .collect();
    let inventory = juggler_supervisor::classify_listeners(&listeners, &rows, &adopted, now);
    PortInventoryInfo {
        rows: inventory
            .into_iter()
            .map(|row| PortRowInfo {
                port: row.port,
                pid: row.pid,
                class: row.class.as_str().to_string(),
                session: row.session,
                provider: row.provider,
                account: row.account,
                project: row.project,
                listener_age_ms: row.listener_age_ms,
                name: row.name,
                hint: row.hint.map(str::to_string),
                killable: row.class != juggler_supervisor::ListenerClass::NotManaged,
            })
            .collect(),
        note: state
            .resources
            .lock()
            .ok()
            .and_then(|collector| collector.ports_note()),
        hogs: hog_facts(state, now),
    }
}

/// Sustained-breach hog facts (Req.5): a helper >2 GiB, a tree >90%
/// of one core, or a tree above its Provider's learned p95 — each
/// sustained over the 60 s window of the collector's rings. The p95
/// rule needs ≥3 Provider trees to compare against (an honest floor;
/// smaller populations never fire it).
#[allow(clippy::similar_names)] // state/stats read naturally here
fn hog_facts(state: &Arc<AppState>, now_ms: i64) -> Vec<HogFactInfo> {
    const HOG_WINDOW_MS: i64 = 60_000;
    // AJ-E512: the thresholds are settings-backed (defaults per SPEC
    // §10) so tests can lower them without faking gigabytes; the
    // production values never change here.
    let helper_rss_limit = setting_u64(state, "hog_helper_rss_limit_mb", 2_048) * 1024 * 1024;
    let cpu_pct_limit = setting_f64(state, "hog_cpu_pct", 90.0);
    let Some(collector) = state.resources.lock().ok() else {
        return Vec::new();
    };
    let stats = collector.stats_snapshot();
    let contexts: Vec<(SessionId, String)> = state
        .supervisor
        .spawn_contexts_snapshot()
        .into_iter()
        .map(|(session, ctx)| (session, ctx.provider.clone()))
        .collect();
    let mut facts = Vec::new();
    for (session, provider) in &contexts {
        let Some(tree_stats) = stats.get(session) else {
            continue;
        };
        // Sustained = every ring sample in the window breaches.
        let ring = collector.ring(session);
        let in_window: Vec<_> = ring
            .iter()
            .filter(|sample| now_ms - sample.at_ms <= HOG_WINDOW_MS)
            .collect();
        if in_window.len() < 2 {
            continue; // not enough samples to call anything sustained
        }
        let cpu_sustained = in_window
            .iter()
            .all(|sample| sample.cpu_pct.is_some_and(|cpu| cpu > cpu_pct_limit));
        if cpu_sustained {
            facts.push(HogFactInfo {
                pid: 0, // tree-level: the Cleanup Centre resolves the
                // Instance's live root pid at render time
                session: session.as_str().to_string(),
                provider: provider.clone(),
                rss_bytes: tree_stats.rss_bytes,
                cpu_pct: tree_stats.cpu_pct,
                rule: "tree-cpu>90%".to_string(),
            });
        }
        // The per-Provider p95 population: the OTHER trees of this
        // Provider (self-excluded), needing at least three peers.
        let peers: Vec<u64> = contexts
            .iter()
            .filter(|(peer, peer_provider)| peer_provider == provider && peer != session)
            .filter_map(|(peer, _)| stats.get(peer).map(|s| s.rss_bytes))
            .collect();
        if peers.len() >= 3 {
            let mut sorted = peers.clone();
            sorted.sort_unstable();
            let p95 = sorted[sorted.len() * 95 / 100];
            let rss_sustained = in_window.iter().all(|sample| sample.rss_bytes > p95);
            if rss_sustained && tree_stats.rss_bytes > p95 {
                facts.push(HogFactInfo {
                    pid: 0,
                    session: session.as_str().to_string(),
                    provider: provider.clone(),
                    rss_bytes: tree_stats.rss_bytes,
                    cpu_pct: tree_stats.cpu_pct,
                    rule: "tree-rss>p95(provider)".to_string(),
                });
            }
        }
        // Single-helper RSS: the live rows of the tree (root excluded).
        for row in collector.rows(session).into_iter().flatten().skip(1) {
            if row.rss_bytes > helper_rss_limit {
                facts.push(HogFactInfo {
                    pid: row.pid,
                    session: session.as_str().to_string(),
                    provider: provider.clone(),
                    rss_bytes: row.rss_bytes,
                    cpu_pct: row.cpu_pct,
                    rule: "helper-rss>2GiB".to_string(),
                });
            }
        }
    }
    facts
}

/// `launch_port_warning` sans-Tauri: the learned expected ports of
/// `project_id`'s instances (their observed `listens_on` history,
/// any row state) crossed with CURRENT orphan-held listeners.
pub fn launch_port_warning_inner(
    state: &Arc<AppState>,
    project_id: Option<String>,
) -> Option<PortWarningInfo> {
    let now = now_millis();
    let expected: std::collections::HashSet<u16> = local_ledger_rows(state)
        .into_iter()
        .filter(|row| {
            project_id
                .as_deref()
                .is_none_or(|project| row.project_id.as_deref() == Some(project))
        })
        .filter_map(|row| row.listens_on)
        .flat_map(|ports| {
            ports
                .split(',')
                .filter_map(|port| port.parse::<u16>().ok())
                .collect::<Vec<_>>()
        })
        .collect();
    if expected.is_empty() {
        return None;
    }
    let listeners = os_listeners().ok()?;
    let rows = local_ledger_rows(state);
    let adopted: std::collections::HashSet<String> = state
        .supervisor
        .alive_adopted_sessions()
        .into_iter()
        .map(|session| session.as_str().to_string())
        .collect();
    let inventory = juggler_supervisor::classify_listeners(&listeners, &rows, &adopted, now);
    inventory
        .into_iter()
        .find(|row| {
            expected.contains(&row.port)
                && row.class == juggler_supervisor::ListenerClass::ManagedOrphan
        })
        .map(|row| {
            let row_port = row.port;
            let instance = format!(
                "{} ({})",
                row.provider.as_deref().unwrap_or("instance"),
                row.session
                    .as_deref()
                    .map(str::chars)
                    .map(|mut chars| chars.by_ref().take(8).collect::<String>())
                    .unwrap_or_default()
            );
            let age_ms = row.listener_age_ms.unwrap_or_default();
            let message = format!(
                ":{} is already held by an orphan from {instance} ({}).",
                row_port,
                human_age(age_ms)
            );
            PortWarningInfo {
                port: row_port,
                pid: row.pid,
                instance,
                age_ms,
                message,
            }
        })
}

/// Stops one orphan: the allow-list guard refuses PIDs that were not
/// recorded as descendants (fail closed, PROPOSAL.md:47), then the
/// TERM→KILL ladder runs with the post-stop re-read built in (Req.6).
/// Every kill lands in the journal's audit trail (D-08) AND the
/// append-only cleanup audit (AJ-E509: intent before signal).
///
/// # Errors
///
/// [`AppError::AuditUnavailable`] when the audit intent fails (no
/// audit, no kill); otherwise the typed stop outcome.
#[tauri::command]
pub fn orphan_stop(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    pid: u32,
) -> AppResult<StopOutcomeInfo> {
    orphan_stop_inner(&state, &SessionId::new(session_id), pid)
}

/// Stops every confirmed orphan of one Instance (the confirm popover
/// lists all PIDs before this runs; each kill is audited).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
#[tauri::command]
pub fn orphan_stop_all(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> AppResult<Vec<StopOutcomeInfo>> {
    let session = SessionId::new(session_id);
    let mut out = Vec::new();
    for orphan in state.orphans.orphans_of(&session) {
        // One kill at a time: a failed audit intent stands the run
        // down HERE (typed error out) — never silently continue.
        out.push(orphan_stop_inner(&state, &session, orphan.pid)?);
    }
    Ok(out)
}

/// Shared stop path for the command and `Stop all` (guard → audit
/// intent → ladder → tracker).
///
/// # Errors
///
/// [`AppError::AuditUnavailable`] when the audit intent fails (no
/// audit, no kill).
pub fn orphan_stop_inner(
    state: &Arc<AppState>,
    session: &SessionId,
    pid: u32,
) -> Result<StopOutcomeInfo, AppError> {
    orphan_stop_audited(state, state.audit.as_ref(), session, pid)
}

/// `orphan_stop_inner` with an injected audit writer (the
/// fault-injection gate's seam; production passes the real repo).
///
/// # Errors
///
/// [`AppError::AuditUnavailable`] when the audit intent fails (no
/// audit, no kill); otherwise the typed stop outcome.
pub fn orphan_stop_audited(
    state: &Arc<AppState>,
    audit: &dyn AuditWriter,
    session: &SessionId,
    pid: u32,
) -> Result<StopOutcomeInfo, AppError> {
    if !state.orphans.allows(session, pid) {
        let outcome = juggler_supervisor::StopOutcome::RefusedNotRecorded;
        state.orphans.record_stop(pid, outcome.clone());
        return Ok(StopOutcomeInfo {
            pid,
            state: outcome.state_label().to_string(),
        });
    }
    // AJ-E509: NO AUDIT, NO KILL — the intent row is written BEFORE
    // the first signal; a failed write refuses the kill with the
    // typed error (never a silent `let _ =`), and the ladder runs
    // only behind a recorded intent.
    let (intent_id, _ports_before) = audit_kill_intent(state, audit, session, pid)?;
    let outcome = juggler_supervisor::stop_pid_ladder(pid, Duration::from_secs(2), |pid| {
        juggler_supervisor::process_alive(pid).is_some()
    });
    state.orphans.record_stop(pid, outcome.clone());
    if let Some(journal) = state.supervisor.journal_ref() {
        let _ = journal.record_killed(session, pid, "manual", outcome.state_label());
    }
    // Complete the audit with the per-PID outcome and the honest
    // post-ladder port state (a still-held port is a fact, never a
    // success claim).
    let ports_after = ports_held_by(pid);
    if let Some(id) = intent_id {
        // Best-effort completion: the intent (the accountability
        // floor) already exists; a failed completion degrades to the
        // truthful pending record, read as `outcome unknown`.
        let _ = audit.complete(id, outcome.state_label(), ports_after.as_deref());
    }
    Ok(StopOutcomeInfo {
        pid,
        state: outcome.state_label().to_string(),
    })
}

/// The audit-writer seam (AJ-E509 Req.3's fault-injection gate):
/// production is the repo; tests inject failures to prove NO AUDIT,
/// NO KILL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The audit-writer failure (there is nothing to distinguish — any
/// failure refuses the kill).
pub struct AuditWriteError;

impl std::fmt::Display for AuditWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cleanup audit write failed")
    }
}

/// The audit-writer seam (production = the repo; tests inject
/// failures for the NO-AUDIT-NO-KILL gate).
pub trait AuditWriter {
    /// Writes the intent row; `Err` refuses the kill.
    ///
    /// # Errors
    ///
    /// [`AuditWriteError`] when the store fails — NO AUDIT, NO KILL.
    fn intent(
        &self,
        record: &juggler_core::storage::CleanupAuditRecord,
    ) -> Result<i64, AuditWriteError>;

    /// Completes the record with the ladder outcome (best-effort).
    ///
    /// # Errors
    ///
    /// [`AuditWriteError`] when the store fails.
    fn complete(
        &self,
        id: i64,
        outcome: &str,
        ports_after: Option<&str>,
    ) -> Result<(), AuditWriteError>;
}

impl AuditWriter for juggler_core::storage::CleanupAuditRepo {
    fn intent(
        &self,
        record: &juggler_core::storage::CleanupAuditRecord,
    ) -> Result<i64, AuditWriteError> {
        juggler_core::storage::CleanupAuditRepo::intent(self, record).map_err(|_| AuditWriteError)
    }

    fn complete(
        &self,
        id: i64,
        outcome: &str,
        ports_after: Option<&str>,
    ) -> Result<(), AuditWriteError> {
        juggler_core::storage::CleanupAuditRepo::complete(
            self,
            id,
            outcome,
            ports_after,
            now_millis(),
        )
        .map_err(|_| AuditWriteError)
    }
}

/// Writes the audit intent for a manual kill (AJ-E509 Req.2/3) and
/// captures the pre-ladder ports. `Err(AppError::AuditUnavailable)`
/// REFUSES the kill — the caller surfaces it.
///
/// # Errors
///
/// [`AppError::AuditUnavailable`] when the intent write fails.
fn audit_kill_intent(
    state: &Arc<AppState>,
    audit: &dyn AuditWriter,
    session: &SessionId,
    pid: u32,
) -> Result<(Option<i64>, Option<String>), AppError> {
    use juggler_core::storage::{ACTOR_PREFIX_USER, CleanupAuditRecord};
    let now = now_millis();
    let ports_before = ports_held_by(pid);
    let record = CleanupAuditRecord {
        id: 0,
        at_ms: now,
        actor: format!("{ACTOR_PREFIX_USER}orphan_stop"),
        pid,
        pgid: None,
        exe_path: format!("pid:{pid}"),
        cmdline_hash: format!("pid:{pid}"),
        cmdline_trunc: format!("PID {pid} (recorded orphan)"),
        session_id: session.as_str().to_string(),
        provider: "unknown".into(),
        account_id: None,
        project_id: None,
        machine_id: local_machine_id(state).as_str().to_string(),
        identity: "allow-list guard (journal-recorded descendant)".into(),
        scope: "pid".into(),
        outcome: String::new(),
        ports_before: ports_before.clone(),
        ports_after: None,
    };
    match audit.intent(&record) {
        Ok(id) => Ok((Some(id), ports_before)),
        Err(AuditWriteError) => Err(AppError::AuditUnavailable),
    }
}

/// The ports a pid holds RIGHT NOW (the audit's before/after facts).
fn ports_held_by(pid: u32) -> Option<String> {
    let listeners = os_listeners().ok()?;
    let ports: Vec<String> = listeners
        .iter()
        .filter(|listener| listener.pid == pid)
        .map(|listener| listener.port.to_string())
        .collect();
    (!ports.is_empty()).then(|| ports.join(","))
}

// --- AJ-E507: verify-before-kill sheet + manual TERM→KILL ladder ---

/// The kill sheet's default grace (pass2 §9.3 — matches the
/// `stop_session` grace).
pub const KILL_GRACE_SECS: u64 = 5;

/// The exact refusal copy when click-time verification fails
/// (Req.2 — the sheet renders it with the kill button REMOVED).
pub const KILL_REFUSAL_COPY: &str =
    "This PID now belongs to a different program — Juggler will not touch it.";
/// The AJ-E511 foreign-machine refusal (Req.2): typed
/// [`juggler_supervisor::SupervisorError::WrongMachine`] under the
/// hood — this is its sheet-facing copy.
pub const KILL_WRONG_MACHINE_COPY: &str =
    "This process record belongs to another machine — Juggler will not touch it.";

/// The exact unsaved-state warning copy (Req.5).
fn unsaved_state_copy(role: &str) -> String {
    format!("Includes a {role} that may hold unsaved state.")
}

/// Role guess, pass2 §5 vocabulary — inferred from exe/cmdline, always
/// shown as a guess, never as fact. Pure (unit-tested).
#[must_use]
pub fn guess_role(exe: &str, cmdline: &str) -> &'static str {
    let exe_name = exe.rsplit('/').next().unwrap_or(exe).to_ascii_lowercase();
    let line = cmdline.to_ascii_lowercase();
    let hay = format!("{exe_name} {line}");
    let contains_any = |needles: &[&str]| needles.iter().any(|n| hay.contains(n));
    if contains_any(&["mcp-server", "mcp_server", "/mcp ", "mcp.json"]) {
        "McpServer"
    } else if contains_any(&[
        "postgres",
        "mysqld",
        "mariadbd",
        "redis-server",
        "mongod",
        "memcached",
        "sqlite3",
    ]) {
        "Database"
    } else if contains_any(&[
        "vite",
        "webpack",
        "esbuild",
        "next dev",
        "--watch-tags",
        "dev-server",
        "serve -s",
        "http.server",
    ]) {
        "DevServer"
    } else if contains_any(&["watchman", "fswatch", "chokidar", "nodemon", "--watch"]) {
        "Watcher"
    } else if contains_any(&[
        "cargo build",
        "make -",
        "tsc ",
        "rustc",
        "clang",
        "esbuild --build",
    ]) {
        "BuildTool"
    } else if exe_name == "sh"
        || exe_name == "bash"
        || exe_name == "zsh"
        || exe_name == "fish"
        || exe_name.ends_with("node") && line.contains("-e")
    {
        "TerminalShell"
    } else if contains_any(&["claude", "codex", "gemini", "copilot"]) {
        "Harness"
    } else {
        "Unknown"
    }
}

/// One PGID member in the verify sheet's collateral list (Req.5).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct KillMemberInfo {
    /// OS process id.
    pub pid: u32,
    /// The member's live PGID (`None` when unreadable).
    pub pgid: Option<u32>,
    /// Executable basename (display).
    pub exe: String,
    /// Truncated command line (display).
    pub cmdline: String,
    /// pass2 §5 role guess — always labelled a guess in the UI.
    pub role_guess: &'static str,
    /// The exact Req.5 warning when the role may hold unsaved state.
    pub unsaved_state_warning: Option<String>,
    /// True when this pid has a Juggler ledger row (attributed).
    pub from_ledger: bool,
}

/// The verify sheet's data (step 1): click-time identity verdict +
/// the re-read-now facts + the PGID collateral list.
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct KillVerifyInfo {
    /// The machine that owns the target (AJ-E511: every kill-sheet
    /// payload names its machine — `This Mac` locally).
    pub machine_id: String,
    /// OS process id.
    pub pid: u32,
    /// Owning Instance.
    pub session: String,
    /// True only when click-time E503 verification SUCCEEDED.
    pub verified: bool,
    /// The exact refusal copy when `!verified`.
    pub refusal: Option<String>,
    /// Kernel-reported exe NOW.
    pub exe_path: String,
    /// Full command line NOW (recorded value when unreadable).
    pub cmdline: String,
    /// Working directory NOW (recorded value when unreadable).
    pub cwd: String,
    /// Ports the pid holds NOW.
    pub ports: Vec<u16>,
    /// The live PGID.
    pub pgid: Option<u32>,
    /// True when the row left the root's group (E502).
    pub escaped_group: bool,
    /// Every live PGID member (self included) with role guesses.
    pub members: Vec<KillMemberInfo>,
    /// The whole-Instance tree's live pids (scope 3's enumeration).
    pub tree_pids: Vec<u32>,
    /// `TERM → grace → KILL` | `TerminateProcess only` (verbatim).
    pub platform: String,
    /// The ladder grace, seconds.
    pub grace_secs: u64,
    /// Attribution.
    pub provider: String,
    /// Attribution.
    pub account_id: Option<String>,
    /// Attribution.
    pub project_id: Option<String>,
}

/// The verify sheet's step-1 command: re-runs the E503 identity check
/// NOW (not from the last tick), re-reads exe/cmdline/cwd/ports, and
/// enumerates the PGID collateral (Req.1/Req.5). Zero signalling.
///
/// # Errors
///
/// [`AppError::NotFound`] when the pid has no ledger row for the
/// session (the UI never offers a kill on unmanaged rows — Req.2 —
/// so this is a contract breach, not a UI state).
#[tauri::command]
pub fn kill_verify(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    pid: u32,
) -> AppResult<KillVerifyInfo> {
    kill_verify_inner(&state, &session_id, pid)
}

/// Click-time identity verification — ONE rule shared by the sheet
/// (step 1) and the execute gate (step 3) so they can never
/// disagree. Helpers carry the kernel-recorded exe (the full
/// boot/start-time/exe triple); LIVE-supervised roots verify via the
/// handle's settled identity with start-time only — the exe moves
/// legitimately across `exec` (E503/E504's `/bin/sh`-vs-`/bin/bash`
/// aliasing rule). Anything else (no record, probe error) fails
/// CLOSED.
fn verify_kill_target(
    state: &Arc<AppState>,
    session: &SessionId,
    row: &juggler_core::storage::LedgerRow,
) -> bool {
    let Some(pid) = row.pid else {
        return false;
    };
    // AJ-E511 Req.2 — the machine gate BEFORE any kernel probe: a
    // foreign-machine row is refused by the TYPED WrongMachine path
    // (kill_verify_inner / kill_execute_as); the sweep already skips
    // foreign rows at candidate read (audit-note-only, never a
    // signal).
    if row.machine_id != local_machine_id(state).as_str() {
        return false;
    }
    if row.role == "root"
        && let Some(identity) = state.supervisor.root_identity(session)
    {
        return juggler_supervisor::tree::verify_start_time(pid, &identity)
            == juggler_supervisor::tree::Verification::Verified;
    }
    row.started_at.is_some_and(|start_time| {
        let identity = juggler_supervisor::tree::ProcessIdentity {
            pid,
            pgid: row.pgid,
            start_time,
            exe: std::path::PathBuf::from(&row.exe_path),
            boot_id: row.boot_id.clone(),
        };
        juggler_supervisor::tree::verify(pid, &identity)
            == juggler_supervisor::tree::Verification::Verified
    })
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// [`AppError::NotFound`] when no ledger row exists for
/// `(session, pid)`.
#[allow(clippy::similar_names)] // pid/pgid are the kernel's own names
pub fn kill_verify_inner(
    state: &Arc<AppState>,
    session_id: &str,
    pid: u32,
) -> Result<KillVerifyInfo, AppError> {
    use juggler_supervisor::tree::LedgerStore as _;
    let session = SessionId::new(session_id.to_string());
    let rows = state.ledger.session_rows(&session).unwrap_or_default();

    let row = rows
        .iter()
        .find(|row| row.pid == Some(pid))
        .ok_or_else(|| AppError::NotFound {
            entity: "ledger row",
            id: format!("{session_id}/{pid}"),
        })?;
    // AJ-E511 Req.2 — the typed machine gate first: a foreign row
    // gets its OWN refusal copy (not the identity one) and never
    // reaches a kernel probe or a member listing.
    if row.machine_id != local_machine_id(state).as_str() {
        return Ok(KillVerifyInfo {
            machine_id: row.machine_id.clone(),
            pid,
            session: session_id.to_string(),
            verified: false,
            refusal: Some(KILL_WRONG_MACHINE_COPY.to_string()),
            exe_path: row.exe_path.clone(),
            cmdline: truncate_cmdline(&row.cmdline_trunc),
            cwd: row.cwd.clone(),
            ports: Vec::new(),
            pgid: None,
            escaped_group: row.escaped_group,
            members: Vec::new(),
            tree_pids: Vec::new(),
            platform: juggler_supervisor::ladder_platform().to_string(),
            grace_secs: KILL_GRACE_SECS,
            provider: row.provider.clone(),
            account_id: row.account_id.clone(),
            project_id: row.project_id.clone(),
        });
    }
    // Click-time verification: identity against the kernel NOW —
    // the shared rule (`verify_kill_target`). Unverifiable rows
    // (state `unverifiable`, a missing start-time, a probe error)
    // fail CLOSED — the sheet explains.
    let verified = verify_kill_target(state, &session, row);
    // Re-read NOW (the live facts; recorded values when unreadable).
    let (cmdline, cwd) = juggler_supervisor::live_details(pid).map_or_else(
        || (row.cmdline_trunc.clone(), row.cwd.clone()),
        |(cmd, cwd)| {
            (
                if cmd.is_empty() {
                    row.cmdline_trunc.clone()
                } else {
                    cmd
                },
                if cwd.is_empty() { row.cwd.clone() } else { cwd },
            )
        },
    );
    let ports = os_listeners()
        .map(|listeners| {
            listeners
                .iter()
                .filter(|listener| listener.pid == pid)
                .map(|listener| listener.port)
                .collect::<Vec<u16>>()
        })
        .unwrap_or_default();
    let pgid = juggler_supervisor::group::pgid_of(pid).ok();
    let (members, tree_pids) = if verified {
        kill_collateral(&rows, pgid)
    } else {
        (Vec::new(), Vec::new())
    };
    Ok(KillVerifyInfo {
        machine_id: row.machine_id.clone(),
        pid,
        session: session_id.to_string(),
        verified,
        refusal: (!verified).then(|| KILL_REFUSAL_COPY.to_string()),
        exe_path: row.exe_path.clone(),
        cmdline: truncate_cmdline(&cmdline),
        cwd,
        ports,
        pgid,
        escaped_group: row.escaped_group,
        members,
        tree_pids,
        platform: juggler_supervisor::ladder_platform().to_string(),
        grace_secs: KILL_GRACE_SECS,
        provider: row.provider.clone(),
        account_id: row.account_id.clone(),
        project_id: row.project_id.clone(),
    })
}

/// The PGID collateral list + the whole-tree enumeration (the verify
/// sheet's scope data). Ledger rows give attribution; strangers are
/// named as strangers (Req.5's shared-group warning data).
#[allow(clippy::similar_names)] // pid/pgid are the kernel's own names
fn kill_collateral(
    rows: &[juggler_core::storage::LedgerRow],
    pgid: Option<u32>,
) -> (Vec<KillMemberInfo>, Vec<u32>) {
    let ledger_by_pid: std::collections::HashMap<u32, &juggler_core::storage::LedgerRow> = rows
        .iter()
        .filter_map(|row| row.pid.map(|pid| (pid, row)))
        .collect();
    let mut members = Vec::new();
    if let Some(pgid) = pgid {
        for member_pid in juggler_supervisor::pgid_members(pgid) {
            let ledger_row = ledger_by_pid.get(&member_pid).copied();
            let (member_exe, member_cmd) = ledger_row
                .map(|row| (row.exe_path.clone(), row.cmdline_trunc.clone()))
                .or_else(|| {
                    juggler_supervisor::live_details(member_pid)
                        .map(|(cmd, _)| ("(unattributed)".into(), cmd))
                })
                .unwrap_or_else(|| ("(unattributed)".into(), String::new()));
            let role = guess_role(&member_exe, &member_cmd);
            members.push(KillMemberInfo {
                pid: member_pid,
                pgid: juggler_supervisor::group::pgid_of(member_pid).ok(),
                exe: member_exe.rsplit('/').next().unwrap_or("?").to_string(),
                cmdline: truncate_cmdline(&member_cmd),
                role_guess: role,
                unsaved_state_warning: matches!(role, "McpServer" | "TerminalShell")
                    .then(|| unsaved_state_copy(role)),
                from_ledger: ledger_row.is_some(),
            });
        }
    }
    // Scope 3's enumeration: the Instance tree's live pids.
    let tree_pids = rows
        .iter()
        .filter(|row| row.pid.is_some() && (row.state == "running" || row.state == "orphaned"))
        .filter_map(|row| row.pid)
        .collect();
    (members, tree_pids)
}

/// Display truncation for command lines (200 chars, the ledger's own
/// convention).
fn truncate_cmdline(cmdline: &str) -> String {
    if cmdline.chars().count() > 200 {
        let truncated: String = cmdline.chars().take(197).collect();
        format!("{truncated}...")
    } else {
        cmdline.to_string()
    }
}

/// One still-held port after the ladder (the honest post-kill report,
/// FMEA C2 — never a success claim).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct StillHoldingInfo {
    /// The port still held.
    pub port: u16,
    /// The pid holding it NOW (may differ from the killed pid).
    pub pid: u32,
    /// The holder's exe (basename).
    pub exe: String,
}

/// The ladder's per-process report (Req.6's vocabulary, one label per
/// member).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct KillMemberOutcome {
    /// OS process id.
    pub pid: u32,
    /// `exited on TERM` | `needed KILL` | `already gone` | `refused` |
    /// `TERM'd — outcome unknown`.
    pub outcome: String,
}

/// The kill outcome (step 3's report): per-process outcomes, the
/// honest still-holding list, and — when an escapee holds the port —
/// the pid-only re-offer data.
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct KillExecuteInfo {
    /// The exact refusal copy when the same-tick verification failed
    /// (zero signals were sent).
    pub refusal: Option<String>,
    /// Per-process outcomes.
    pub members: Vec<KillMemberOutcome>,
    /// True when the user cancelled mid-grace (audit left pending).
    pub cancelled: bool,
    /// True when every member succeeded (`already gone` included).
    pub all_succeeded: bool,
    /// Ports still held after the ladder (by anyone).
    pub still_holding: Vec<StillHoldingInfo>,
    /// The pid-only re-offer when an ESCAPED member still holds a
    /// killed target's port (fresh verification is the UI's next
    /// step, never an automatic signal).
    pub reoffer_pid: Option<u32>,
}

/// The kill sheet's step 3: the audited ladder. Guardrails, in
/// order: (1) same-tick re-verification of EVERY signalled pid's
/// anchor — a reused PID gets ZERO signals and the exact refusal
/// copy; (2) the AJ-E509 audit intent BEFORE the first signal (no
/// audit, no kill); (3) the ladder — TERM, cancellable
/// grace, KILL only survivors; (4) the honest post-ladder port
/// re-probe (still-held is a fact, never a failure to hide).
///
/// # Errors
///
/// [`AppError::NotFound`] when no ledger row exists; validation when
/// `scope` is not one of `pid`|`pgid`|`tree`;
/// [`AppError::AuditUnavailable`] when the audit intent fails (the
/// kill is refused, zero signals sent).
#[tauri::command]
pub fn kill_execute(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    pid: u32,
    scope: String,
    attempt: String,
    goal_port: Option<u16>,
) -> AppResult<KillExecuteInfo> {
    let flag = state.register_kill_cancel(&attempt);
    let result = kill_execute_audited(
        &state,
        state.audit.as_ref(),
        &session_id,
        pid,
        &scope,
        &flag,
        goal_port,
    );
    state.unregister_kill_cancel(&attempt);
    result
}

/// Cancels a running ladder's grace window (the UI's Cancel button).
/// Idempotent; unknown attempts are no-ops (the ladder already
/// finished).
#[tauri::command]
pub fn kill_cancel(state: tauri::State<'_, Arc<AppState>>, attempt: String) {
    state.flip_kill_cancel(&attempt);
}

/// The grace window's per-process exit ticks (Req.3): liveness of the
/// ladder's known members, probed cheaply (one targeted check per
/// pid — no scans). The sheet polls this while the ladder runs; the
/// AUTHORITATIVE outcomes still come from `kill_execute`'s report.
#[tauri::command]
#[must_use]
pub fn kill_liveness(pids: Vec<u32>) -> Vec<bool> {
    pids.iter()
        .map(|pid| juggler_supervisor::process_alive(*pid).is_some())
        .collect()
}

/// `kill_execute` with an injected audit writer (the guardrail
/// gate's seam; production passes the real repo).
///
/// # Errors
///
/// As [`kill_execute`].
pub fn kill_execute_audited(
    state: &Arc<AppState>,
    audit: &dyn AuditWriter,
    session_id: &str,
    pid: u32,
    scope: &str,
    cancel_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    goal_port: Option<u16>,
) -> Result<KillExecuteInfo, AppError> {
    kill_execute_as(
        state,
        audit,
        "user:kill_sheet",
        session_id,
        pid,
        scope,
        cancel_flag,
        goal_port,
    )
}

/// `kill_execute` with an explicit audit ACTOR — the automatic tiers'
/// entry (AJ-E508): actor `auto:tier-1` / `auto:tier-2` per the E509
/// vocabulary. Same guardrails, same ladder, same audit floor.
///
/// # Errors
///
/// As [`kill_execute`].
#[allow(clippy::too_many_arguments)] // the audit actor rides the E507 kill path's shape
pub fn kill_execute_as(
    state: &Arc<AppState>,
    audit: &dyn AuditWriter,
    actor: &str,
    session_id: &str,
    pid: u32,
    scope: &str,
    cancel_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    goal_port: Option<u16>,
) -> Result<KillExecuteInfo, AppError> {
    use juggler_supervisor::tree::LedgerStore as _;
    let session = SessionId::new(session_id.to_string());
    let rows = state.ledger.session_rows(&session).unwrap_or_default();
    let row = rows
        .iter()
        .find(|row| row.pid == Some(pid))
        .ok_or_else(|| AppError::NotFound {
            entity: "ledger row",
            id: format!("{session_id}/{pid}"),
        })?;
    // Guardrail 0 — the machine gate (AJ-E511 Req.2): a foreign row
    // gets the TYPED WrongMachine refusal with zero signals, before
    // any identity probe (which would misleadingly read as a
    // PID-reuse refusal).
    if row.machine_id != local_machine_id(state).as_str() {
        return Ok(KillExecuteInfo {
            refusal: Some(KILL_WRONG_MACHINE_COPY.to_string()),
            members: vec![KillMemberOutcome {
                pid,
                outcome: "refused".into(),
            }],
            cancelled: false,
            all_succeeded: false,
            still_holding: Vec::new(),
            reoffer_pid: None,
        });
    }
    // Guardrail 1 — SAME-TICK re-verification (E503 against the
    // kernel NOW, not the sheet's earlier verdict; the SAME shared
    // rule the sheet used): a mismatched or unverifiable anchor gets
    // ZERO signals and the exact copy.
    let verified = verify_kill_target(state, &session, row);
    if !verified {
        return Ok(KillExecuteInfo {
            refusal: Some(KILL_REFUSAL_COPY.to_string()),
            members: vec![KillMemberOutcome {
                pid,
                outcome: "refused".into(),
            }],
            cancelled: false,
            all_succeeded: false,
            still_holding: Vec::new(),
            reoffer_pid: None,
        });
    }
    // Scope enumeration (Req.1's three scopes; the UI defaults to
    // the narrowest that covers the goal).
    let members = scope_members(scope, &rows, pid)?;
    // The port facts: the goal port when the user stated one (the
    // port that opened the sheet — it may be held by an ESCAPED
    // sibling, which is exactly the honest-still-holding case),
    // else whatever the target itself holds.
    let member_ports = ports_held_by(pid);
    let ports_before = match goal_port {
        Some(port) => Some(port.to_string()),
        None => member_ports.clone(),
    };
    // Guardrail 2 — NO AUDIT, NO KILL: the intent row names the real
    // target (scope, pgid, attribution), written BEFORE any signal.
    let record = kill_intent_record(
        state,
        row,
        session_id,
        pid,
        scope,
        ports_before.as_deref(),
        member_ports.as_deref(),
        actor,
    );
    let intent_id = audit
        .intent(&record)
        .map_err(|_| AppError::AuditUnavailable)?;
    // Guardrail 3 — the ladder itself (cancellable through grace).
    let cancel = std::sync::Arc::clone(cancel_flag);
    let outcome = juggler_supervisor::terminate_group(
        // AJ-E512: settings-backed (default the spec's 5 s) so the
        // collateral suites can widen it on loaded machines — a starved
        // fixture's TERM handler must land inside the grace window.
        std::time::Duration::from_secs(setting_u64(state, "kill_grace_secs", KILL_GRACE_SECS)),
        move || members,
        move || cancel.load(std::sync::atomic::Ordering::Relaxed),
    );
    // The honest post-ladder port re-probe (C2): report what is
    // STILL held and by whom — including an escaped member that keeps
    // the port (then re-offer pid-only after fresh verification).
    let (still_holding, reoffer_pid) =
        still_holding_report(ports_before.as_deref(), pid, outcome.cancelled);
    // Audit completion: per-pid vocabulary joined; a CANCELLED
    // ladder leaves the intent pending ("TERM'd, outcome unknown").
    if !outcome.cancelled {
        let vocabulary = outcome
            .members
            .iter()
            .map(|(pid, outcome)| format!("pid {pid}: {}", outcome.as_str()))
            .collect::<Vec<_>>()
            .join("; ");
        let ports_after = still_holding
            .iter()
            .map(|held| held.port.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let ports_after = (!ports_after.is_empty()).then_some(ports_after);
        // Best-effort (E509's floor is the intent row itself).
        let _ = audit.complete(intent_id, &vocabulary, ports_after.as_deref());
    }
    let members = outcome
        .members
        .iter()
        .map(|(pid, label)| KillMemberOutcome {
            pid: *pid,
            outcome: label.as_str().to_string(),
        })
        .collect();
    Ok(KillExecuteInfo {
        refusal: None,
        members,
        cancelled: outcome.cancelled,
        all_succeeded: outcome.all_succeeded(),
        still_holding,
        reoffer_pid,
    })
}

/// The kill sheet's audit intent row (Req.6 / AJ-E509 Req.2): the
/// full attribution from the ledger row, the verified-at-click-time
/// identity note, and the goal-port-aware before/after facts.
#[allow(clippy::similar_names)] // pid/pgid are the kernel's own names
#[allow(clippy::too_many_arguments)] // mirrors the audit record's own columns
fn kill_intent_record(
    state: &Arc<AppState>,
    row: &juggler_core::storage::LedgerRow,
    session_id: &str,
    pid: u32,
    scope: &str,
    ports_before: Option<&str>,
    member_ports: Option<&str>,
    actor: &str,
) -> juggler_core::storage::CleanupAuditRecord {
    juggler_core::storage::CleanupAuditRecord {
        id: 0,
        at_ms: now_millis(),
        actor: actor.to_string(),
        pid,
        pgid: juggler_supervisor::group::pgid_of(pid).ok(),
        exe_path: row.exe_path.clone(),
        cmdline_hash: row.cmdline_hash.clone(),
        cmdline_trunc: truncate_cmdline(&row.cmdline_trunc),
        session_id: session_id.to_string(),
        provider: row.provider.clone(),
        account_id: row.account_id.clone(),
        project_id: row.project_id.clone(),
        machine_id: local_machine_id(state).as_str().to_string(),
        identity: format!(
            "E503 verified at click time (start {}, exe {:?})",
            row.started_at.map_or("?".to_string(), |t| t.to_string()),
            row.exe_path
        ),
        scope: scope.to_string(),
        outcome: String::new(),
        ports_before: ports_before
            .map(str::to_string)
            .or_else(|| member_ports.map(str::to_string)),
        ports_after: None,
    }
}

/// The three scopes' member enumeration (Req.1): `pid` — the one
/// verified process; `pgid` — every live member of its process group;
/// `tree` — the Instance's live ledger rows.
///
/// # Errors
///
/// [`AppError::Validation`] for an unknown scope, or for `pgid` when
/// the group is unreadable (the honest narrower-scope prompt).
#[allow(clippy::similar_names)] // pid/pgid are the kernel's own names
fn scope_members(
    scope: &str,
    rows: &[juggler_core::storage::LedgerRow],
    pid: u32,
) -> Result<Vec<u32>, AppError> {
    match scope {
        "pid" => Ok(vec![pid]),
        "pgid" => {
            let pgid =
                juggler_supervisor::group::pgid_of(pid).map_err(|_| AppError::Validation {
                    subject: "scope",
                    message: "the process group is unreadable — choose `This process only`".into(),
                })?;
            Ok(juggler_supervisor::pgid_members(pgid))
        }
        "tree" => Ok(rows
            .iter()
            .filter(|row| row.pid.is_some() && (row.state == "running" || row.state == "orphaned"))
            .filter_map(|row| row.pid)
            .collect()),
        other => Err(AppError::Validation {
            subject: "scope",
            message: format!("unknown scope {other:?} (pid | pgid | tree)"),
        }),
    }
}

/// The post-ladder port re-probe (FMEA C2): who STILL holds each of
/// the target's ports, and the pid-only re-offer when an escapee —
/// not the killed pid — keeps one. Never a success claim.
fn still_holding_report(
    ports_before: Option<&str>,
    pid: u32,
    cancelled: bool,
) -> (Vec<StillHoldingInfo>, Option<u32>) {
    let target_ports: Vec<u16> = ports_before
        .unwrap_or("")
        .split(',')
        .filter_map(|port| port.parse::<u16>().ok())
        .collect();
    let mut still_holding = Vec::new();
    if let Ok(listeners) = os_listeners() {
        for port in &target_ports {
            for listener in listeners.iter().filter(|l| l.port == *port) {
                let exe = holder_exe(listener.pid);
                still_holding.push(StillHoldingInfo {
                    port: *port,
                    pid: listener.pid,
                    exe,
                });
            }
        }
    }
    // The re-offer only after a COMPLETED ladder (a cancelled one
    // reports the facts and leaves the choice entirely to the user).
    let reoffer_pid = if cancelled {
        None
    } else {
        still_holding
            .iter()
            .find(|held| held.pid != pid)
            .map(|held| held.pid)
    };
    (still_holding, reoffer_pid)
}

/// `Verify owner`: re-reads the ppid chain for one orphan pid (manual,
/// honest evidence — never a silent re-check).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on failure.
#[tauri::command]
pub fn orphan_verify(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    pid: u32,
) -> AppResult<OrphanVerifyInfo> {
    orphan_verify_arc(&state, &session_id, pid)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on probe failure.
pub fn orphan_verify_arc(
    _state: &Arc<AppState>,
    _session_id: &str,
    pid: u32,
) -> AppResult<OrphanVerifyInfo> {
    // The probe itself is per-pid (the session scopes it in the UI).
    match juggler_supervisor::verify_owner(pid) {
        juggler_supervisor::OwnerState::Gone => Ok(OrphanVerifyInfo {
            pid,
            alive: false,
            parent_pid: None,
            note: "process is gone".to_string(),
        }),
        juggler_supervisor::OwnerState::Alive { ppid } => Ok(OrphanVerifyInfo {
            pid,
            alive: true,
            parent_pid: ppid,
            note: owner_note(ppid),
        }),
    }
}

/// Human note for a live pid's parent state.
fn owner_note(ppid: Option<u32>) -> String {
    match ppid {
        Some(1 | 0) => "alive — re-parented to init/launchd (its parent exited)".to_string(),
        Some(parent) => format!("alive — parent pid {parent}"),
        None => "alive — parent not readable".to_string(),
    }
}

/// Sets the per-Account Tier-1 cleanup opt-in (D-08: off by default;
/// the toggle is the only way automatic kills ever happen).
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Account is unknown.
#[tauri::command]
pub fn account_set_cleanup(
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
    opt_in: bool,
) -> AppResult<AccountInfo> {
    account_set_cleanup_arc(&state, &account_id, opt_in)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Account is unknown.
pub fn account_set_cleanup_arc(
    state: &Arc<AppState>,
    account_id: &str,
    opt_in: bool,
) -> AppResult<AccountInfo> {
    let account = {
        let storage = lock_storage_arc(state)?;
        let mut account = storage
            .accounts()
            .get(&AccountId::new(account_id.to_string()))?;
        account.cleanup_opt_in = opt_in;
        storage.accounts().update(&account)?;
        account
    };
    Ok(account_info_of(&account))
}

/// Sets or clears the optional user-entered renewal date (AJ-E411,
/// D-12). `date` must be a valid local calendar `YYYY-MM-DD` (strictly
/// validated — honest rejection, never a repair); `None` clears it.
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Account is unknown
/// or the date format is invalid.
#[tauri::command]
pub fn account_set_renewal(
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
    date: Option<String>,
) -> AppResult<AccountInfo> {
    account_set_renewal_arc(&state, &account_id, date.as_deref())
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Account is unknown
/// or the date format is invalid.
pub fn account_set_renewal_arc(
    state: &Arc<AppState>,
    account_id: &str,
    date: Option<&str>,
) -> AppResult<AccountInfo> {
    if let Some(raw) = date
        && juggler_core::renewal::CalendarDate::parse(raw).is_none()
    {
        return Err(AppError::Validation {
            subject: "renewal date",
            message: format!("must be YYYY-MM-DD (got {raw:?})"),
        });
    }
    let account = {
        let storage = lock_storage_arc(state)?;
        let mut account = storage
            .accounts()
            .get(&AccountId::new(account_id.to_string()))?;
        account.renewal_date = date.map(ToString::to_string);
        storage.accounts().update(&account)?;
        account
    };
    Ok(account_info_of(&account))
}

/// The exit-snapshot hook (AJ-E410 Req.3): freezes the pre-exit
/// descendant rows into the tracker and journals the durable
/// allow-list. Called by the lifecycle forwarder on every Exited event
/// (clean exits are recorded too — they simply never surface orphans).
pub fn record_exit_snapshot(
    state: &Arc<AppState>,
    session: &SessionId,
    root_pid: u32,
    kind: &ExitKind,
) {
    let (rows, root_program) = snapshot_descendants(state, session, root_pid);
    if let Some(journal) = state.supervisor.journal_ref() {
        let _ = journal.record_descendants(session, root_pid, kind.is_success(), &rows);
    }
    state.orphans.record_exit(juggler_supervisor::ExitSnapshot {
        session: session.clone(),
        root_pid,
        root_program,
        clean: kind.is_success(),
        taken_ms: Timestamp::now().millis(),
        rows,
    });
}

/// The frozen pre-exit rows, split into (descendants, root program name).
/// The root is dead already — its name survives only in the collector's
/// last tick while it was alive, which is exactly what this reads.
fn snapshot_descendants(
    state: &Arc<AppState>,
    session: &SessionId,
    root_pid: u32,
) -> (Vec<juggler_supervisor::DescendantRow>, String) {
    let Some(collector) = state.resources.lock().ok() else {
        return (Vec::new(), "the ended instance".to_string());
    };
    let frozen = collector.rows(session).unwrap_or_default();
    let root_program = frozen
        .iter()
        .find(|row| row.pid == root_pid)
        .map_or_else(|| "the ended instance".to_string(), |row| row.name.clone());
    let rows = frozen
        .into_iter()
        .filter(|row| row.pid != root_pid)
        .map(|row| juggler_supervisor::DescendantRow {
            pid: row.pid,
            name: row.name,
            rss_bytes: row.rss_bytes,
        })
        .collect();
    (rows, root_program)
}

/// The 2 s sampling tick (AJ-E410 Req.1): collector tick (targeted
/// refresh + periodic discovery), orphan grace poll with fresh liveness
/// probes, then the Tier-1 auto-cleanup sweep for opted-in Accounts
/// (60 s grace, recorded-descendant PIDs only — D-08).
pub fn processes_tick(state: &Arc<AppState>) {
    let now = Timestamp::now().millis();
    {
        let Ok(mut collector) = state.resources.lock() else {
            return;
        };
        collector.tick(&state.supervisor, now);
        // The E410 tracker's own confirmations advance here (the
        // Processes tab's orphan section); the AJ-E508 sweep reads
        // the LEDGER's orphan verdicts instead — the tracker
        // deliberately drops clean exits and cannot serve Tier 1.
        let tracked = state.orphans.tracked_pids();
        if !tracked.is_empty() {
            collector.refresh_pids(&tracked);
            let _ = state.orphans.poll(now, |pid| collector.lookup(pid));
        }
    }
    auto_cleanup_sweep(state, now);
}

/// One live Tier-2 notice (AJ-E508 Req.1): the crash-orphan's Undo
/// window. Exactly one per candidate; `keep` (Undo / Keep running)
/// dismisses it forever for that pid.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Tier2Notice {
    /// The candidate pid.
    pub pid: u32,
    /// The Instance it belonged to.
    pub session: String,
    /// Truncated command line (the notification names the process).
    pub cmdline_trunc: String,
    /// When the notice was shown, epoch ms.
    pub shown_at_ms: i64,
    /// The window's end, epoch ms (≥30 s after `shown_at_ms`; no
    /// signal before it).
    pub deadline_ms: i64,
    /// True once the user chose Undo / Keep running.
    pub keep: bool,
}

/// The Undo window's length (Req.1: ≥30 s — never shorter).
pub const TIER2_UNDO_WINDOW_MS: i64 = 30_000;
/// Tier 2's crash grace before the notification (Req.1: 5 min).
pub const TIER2_CRASH_GRACE_MS: i64 = 5 * 60_000;

/// The automatic tiers' sweep (AJ-E508; replaces the E410 prototype).
/// Tier 0 by default: without BOTH the Account opt-in and the tier
/// evaluator's eligibility, nothing is ever signalled. The candidate
/// source is the LEDGER's orphaned rows (the E504 reconciler's
/// verdict — the same authority the Cleanup Centre lists; the E410
/// tracker deliberately drops clean exits and cannot serve Tier 1).
/// Gates, in order: the global pause; the 24 h crash suspension; the
/// per-candidate evaluator (roles/roots/containment/machine/exit
/// kind); pacing (the auto grace; Tier 2's 5 min + ONE notification +
/// the Undo window); same-tick re-verification (a failure DEMOTES to
/// Tier 0 with an audit note); then the E507 ladder behind an E509
/// audit intent (actor `auto:tier-N`), via [`kill_execute_as`] — the
/// only kill surface, manual or automatic.
#[allow(clippy::too_many_lines)] // one pass per gate; splitting hides the ordering that IS the law
pub fn auto_cleanup_sweep(state: &Arc<AppState>, now: i64) {
    if cleanup_paused(state) || cleanup_suspended_until(state).is_some() {
        return;
    }
    let machine = local_machine_id(state);
    // AJ-E512 Req.3: a hog is NEVER auto-signalled at any tier —
    // Tier 3 is alert + staged sheet only (E508), so the sweep stands
    // down for hog-flagged pids (and hog-flagged trees) entirely; the
    // manual E507 sheet stays the only path.
    // Hog facts ride the REAL clock (`now` here is the sweep's
    // SCHEDULE clock — often minutes in the future in tests, which
    // would empty the 60 s ring window and un-hog everything).
    let hog_facts_now = hog_facts(state, now_millis());
    let hog_pids: std::collections::HashSet<u32> = hog_facts_now
        .iter()
        .filter(|hog| hog.pid != 0)
        .map(|hog| hog.pid)
        .collect();
    let hog_sessions: std::collections::HashSet<String> = hog_facts_now
        .iter()
        .map(|hog| hog.session.clone())
        .collect();
    // One candidates read: every ORPHANED row of this machine's
    // sessions, grouped by session with its root row (the exit-kind
    // source and the pacing clock).
    let mut by_session: std::collections::HashMap<
        String,
        (
            Vec<juggler_core::storage::LedgerRow>,
            Option<juggler_core::storage::LedgerRow>,
        ),
    > = std::collections::HashMap::new();
    for row in local_ledger_rows(state) {
        if row.machine_id != machine.as_str() {
            continue;
        }
        let entry = by_session.entry(row.session_id.clone()).or_default();
        if row.role == "root" {
            entry.1 = Some(row);
        } else if row.state == "orphaned" {
            entry.0.push(row);
        }
    }
    for (session_id, (candidates, root)) in by_session {
        let Some(root_row) = root else { continue };
        // The root's last tick while alive ≈ its exit time (its
        // last_seen stops refreshing at death).
        let parent_exit_ms = root_row.last_seen;
        if now - parent_exit_ms < juggler_supervisor::AUTO_CLEANUP_GRACE_MS {
            continue;
        }
        // The root's exit kind decides the TIER, never the
        // candidate's presence: code 0 → Tier 1's premise; anything
        // else → crash (Tier 2's); unknown → observe only (fail
        // closed).
        let root_exit_clean = root_row.exit_kind.as_deref().map(|kind| {
            match kind.strip_prefix("code:") {
                Some(code) => code == "0",
                // A signal death or an unparsable kind is a crash.
                None => false,
            }
        });
        let settings = account_tier_settings(state, &SessionId::new(session_id.clone()));
        for row in candidates {
            let Some(orphan_pid) = row.pid else { continue };
            if hog_pids.contains(&orphan_pid) || hog_sessions.contains(&session_id) {
                continue; // hog: alert + staged sheet only, never a signal
            }
            let candidate = juggler_supervisor::tree::TierCandidate {
                pid: orphan_pid,
                role_guess: guess_role(&row.exe_path, &row.cmdline_trunc).to_string(),
                is_root: false,
                state: row.state.clone(),
                escaped_group: row.escaped_group,
                root_exit_clean,
                machine_local: true,
                tier1_opt_in: settings.tier1_opt_in,
                tier2_opt_in: settings.tier2_opt_in,
            };
            let tier = match juggler_supervisor::tree::evaluate_tier(&candidate) {
                juggler_supervisor::tree::TierVerdict::Signal(tier) => tier,
                juggler_supervisor::tree::TierVerdict::ObserveOnly(_) => continue,
            };
            // Tier 2 pacing: 5 min after the crash AND one
            // notification AND an elapsed Undo window.
            if tier == 2 {
                if now - parent_exit_ms < TIER2_CRASH_GRACE_MS {
                    continue;
                }
                let orphan = juggler_supervisor::Orphan {
                    session: SessionId::new(session_id.clone()),
                    root_pid: root_row.pid.unwrap_or(0),
                    parent_label: root_row.exe_path.clone(),
                    pid: orphan_pid,
                    name: row.exe_path.clone(),
                    rss_bytes: 0,
                    parent_exit_ms,
                };
                match tier2_window(state, &orphan, &row, now) {
                    Tier2Window::Open | Tier2Window::Kept => continue,
                    Tier2Window::Elapsed => (),
                }
            }
            // Same-tick re-verification — a failure DEMOTES to Tier 0
            // (list-only) with an audit note, never a signal.
            let session = SessionId::new(session_id.clone());
            if !verify_kill_target(state, &session, &row) {
                demote_with_audit_note(state, &row, tier);
                continue;
            }
            let actor = if tier == 1 {
                juggler_core::storage::ACTOR_AUTO_TIER1
            } else {
                juggler_core::storage::ACTOR_AUTO_TIER2
            };
            let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let outcome = kill_execute_as(
                state,
                state.audit.as_ref(),
                actor,
                session_id.as_str(),
                orphan_pid,
                "pid",
                &flag,
                None,
            );
            if let Ok(outcome) = outcome
                && let Some(journal) = state.supervisor.journal_ref()
            {
                let _ = journal.record_killed(
                    &session,
                    orphan_pid,
                    if tier == 1 {
                        "auto-tier1"
                    } else {
                        "auto-tier2"
                    },
                    &outcome
                        .members
                        .iter()
                        .map(|member| member.outcome.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                );
            }
        }
    }
}

enum Tier2Window {
    /// Created now or still counting — no signal this tick.
    Open,
    /// The user chose Undo / Keep running — never signalled.
    Kept,
    /// The window elapsed without an Undo — eligible to signal.
    Elapsed,
}

fn tier2_window(
    state: &Arc<AppState>,
    orphan: &juggler_supervisor::Orphan,
    row: &juggler_core::storage::LedgerRow,
    now: i64,
) -> Tier2Window {
    let Ok(mut pending) = state.tier2_pending.lock() else {
        return Tier2Window::Open; // fail closed: no notice, no signal
    };
    match pending.get(&orphan.pid) {
        Some(notice) if notice.keep => Tier2Window::Kept,
        Some(notice) => {
            if now < notice.deadline_ms {
                Tier2Window::Open
            } else {
                pending.remove(&orphan.pid);
                Tier2Window::Elapsed
            }
        }
        None => {
            // Exactly one notification per crash-orphan (Req.1).
            pending.insert(
                orphan.pid,
                Tier2Notice {
                    pid: orphan.pid,
                    session: orphan.session.as_str().to_string(),
                    cmdline_trunc: truncate_cmdline(&row.cmdline_trunc),
                    shown_at_ms: now,
                    deadline_ms: now + TIER2_UNDO_WINDOW_MS,
                    keep: false,
                },
            );
            Tier2Window::Open
        }
    }
}

/// A failed same-tick re-verification: the candidate is silently
/// demoted to Tier 0 (list-only) WITH an audit note — the E509 log
/// carries intent + `demoted` completion so the History tells the
/// truth about what was considered and why nothing was signalled.
fn demote_with_audit_note(state: &Arc<AppState>, row: &juggler_core::storage::LedgerRow, tier: u8) {
    let actor = if tier == 1 {
        juggler_core::storage::ACTOR_AUTO_TIER1
    } else {
        juggler_core::storage::ACTOR_AUTO_TIER2
    };
    let record = kill_intent_record(
        state,
        row,
        &row.session_id,
        row.pid.unwrap_or(0),
        "pid",
        None,
        None,
        actor,
    );
    if let Ok(id) = state.audit.intent(&record) {
        let _ = state.audit.complete(
            id,
            "demoted — re-verification failed, listed only (Tier 0)",
            None,
            now_millis(),
        );
    }
}

/// True when the global `Pause automatic cleanup` kill-switch is on
/// (Req.4): the sweep stands down within one tick of the flag.
fn cleanup_paused(state: &Arc<AppState>) -> bool {
    let Ok(storage) = state.storage.lock() else {
        return true; // a lock failure pauses — never signals
    };
    storage
        .app_settings()
        .get("cleanup_paused")
        .ok()
        .flatten()
        .is_some_and(|value| value == "1")
}

/// The 24 h crash-suspension deadline (Req.5), if active: Tiers 1-2
/// stand down until the user re-confirms in the Cleanup Centre.
fn cleanup_suspended_until(state: &Arc<AppState>) -> Option<i64> {
    let storage = state.storage.lock().ok()?;
    let until = storage
        .app_settings()
        .get("cleanup_suspended_until")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<i64>().ok())?;
    (until > now_millis()).then_some(until)
}

/// Resolves an Instance's Account tier settings (session → launch
/// settings → account). Unknown links read as both-off (fail closed).
fn account_tier_settings(
    state: &Arc<AppState>,
    session: &SessionId,
) -> juggler_core::storage::CleanupTierSettings {
    let account = (|| {
        let storage = state.storage.lock().ok()?;
        let record = storage.sessions().get(session).ok()?;
        let recipe = storage.profiles().get(&record.profile).ok()?;
        storage.accounts().get(&recipe.account).ok()
    })();
    let account_id = account.as_ref().map(|a| a.id.as_str().to_string());
    match account_id {
        Some(account_id) => state.cleanup_settings.get(&account_id).unwrap_or_default(),
        None => juggler_core::storage::CleanupTierSettings::default(),
    }
}

/// One live Tier-2 notice as the UI reads it.
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct Tier2NoticeInfo {
    /// The candidate pid.
    pub pid: u32,
    /// The Instance it belonged to.
    pub session: String,
    /// Truncated command line.
    pub cmdline_trunc: String,
    /// The window's end, epoch ms.
    pub deadline_ms: i64,
}

/// Reads a u64 app setting with a default (AJ-E512 test seams for
/// the hog/idle thresholds; production defaults are the arguments).
fn setting_u64(state: &Arc<AppState>, key: &str, default: u64) -> u64 {
    state
        .storage
        .lock()
        .ok()
        .and_then(|storage| storage.app_settings().get(key).ok().flatten())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

/// Reads an f64 app setting with a default (same seam as
/// [`setting_u64`]).
fn setting_f64(state: &Arc<AppState>, key: &str, default: f64) -> f64 {
    state
        .storage
        .lock()
        .ok()
        .and_then(|storage| storage.app_settings().get(key).ok().flatten())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(default)
}

/// One passive Cleanup Centre alert (AJ-E512 Req.3). `role=status`
/// surfaces in the UI — never a focus-stealing dialog.
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct CleanupAlertInfo {
    /// `hog` (resource alert, staged-sheet eligible) or `idle` (the
    /// once-per-orphan nudge).
    pub kind: String,
    /// The exact copy (byte-asserted in tests; one language with the
    /// Rust suite).
    pub text: String,
    /// The helper pid the alert names (0 when unresolvable).
    pub pid: u32,
    /// The Instance it belonged to.
    pub session: String,
}

/// The passive alert set: hog alerts (exact template) + idle-orphan
/// nudges (fired at most ONCE per orphan — the once-flag is persisted
/// the moment the nudge is computed).
#[derive(Clone, Debug, serde::Serialize, PartialEq, Default)]
pub struct CleanupAlertsInfo {
    /// Hog alerts first (actionable), then idle nudges.
    pub alerts: Vec<CleanupAlertInfo>,
}

/// Formats bytes the way the alert copy needs (GiB one decimal, MiB
/// whole).
#[must_use]
pub fn rss_text(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)] // display rounding, not math
    let gib = bytes as f64 / 1_073_741_824.0;
    if bytes >= 1024 * 1024 * 1024 {
        format!("{gib:.1} GiB")
    } else {
        format!("{} MiB", bytes / (1024 * 1024))
    }
}

/// Formats an age the way the nudge copy needs.
fn age_text(ms: i64) -> String {
    let mins = ms / 60_000;
    if mins < 60 {
        format!("{mins}m")
    } else {
        format!("{}h", mins / 60)
    }
}

/// Computes the passive alerts (AJ-E512 Req.3): for every hog-flagged
/// tree, ONE alert naming its TOP-RSS helper (NN/g: name the helper,
/// not just the Instance), and — once per orphan — an idle nudge
/// (CPU≈0 + age only). Purely passive: no alert carries a signal
/// path; the hog alert's pid routes to the STAGED E507 sheet
/// (app prepares, user executes).
pub fn cleanup_alerts_inner(state: &Arc<AppState>) -> CleanupAlertsInfo {
    let now = now_millis();
    let machine = local_machine_id(state);
    let mut alerts = Vec::new();

    // Hog alerts: top-RSS live helper per hog-flagged session.
    let facts = hog_facts(state, now);
    let mut hog_sessions: Vec<String> = facts.iter().map(|f| f.session.clone()).collect();
    hog_sessions.sort();
    hog_sessions.dedup();
    if !hog_sessions.is_empty() {
        let Some(collector) = state.resources.lock().ok() else {
            return CleanupAlertsInfo { alerts };
        };
        for session in hog_sessions {
            let top = collector
                .rows(&SessionId::new(session.clone()))
                .into_iter()
                .flatten()
                .skip(1) // the root
                .max_by_key(|row| row.rss_bytes);
            let Some(top) = top else { continue };
            let helper = top.name.rsplit('/').next().unwrap_or(&top.name).to_string();
            let instance = instance_label_for(state, &session);
            alerts.push(CleanupAlertInfo {
                kind: "hog".into(),
                text: format!(
                    "{helper} under {instance} holds {} — review in Cleanup Centre.",
                    rss_text(top.rss_bytes)
                ),
                pid: top.pid,
                session: session.clone(),
            });
        }
    }

    // Idle nudges: orphaned helpers, CPU≈0, aged past the threshold —
    // fired at most ONCE per orphan (persisted flag).
    let min_age = setting_u64(state, "idle_nudge_min_age_ms", 3_600_000).cast_signed();
    for row in local_ledger_rows(state) {
        if row.state != "orphaned" || row.machine_id != machine.as_str() || row.role != "helper" {
            continue;
        }
        let Some(pid) = row.pid else { continue };
        let cpu_ok = state
            .resources
            .lock()
            .ok()
            .and_then(|collector| {
                collector
                    .rows(&SessionId::new(row.session_id.clone()))
                    .into_iter()
                    .flatten()
                    .find(|live| live.pid == pid)
                    .map(|live| live.cpu_pct.is_none_or(|cpu| cpu < 1.0))
            })
            .unwrap_or(true);
        if !cpu_ok {
            continue;
        }
        let age = now - row.started_at.map(|s| s * 1000).unwrap_or_default();
        if age < min_age {
            continue;
        }
        let flag = format!("idle_nudge:{}:{pid}", row.session_id);
        let already = state
            .storage
            .lock()
            .ok()
            .and_then(|storage| storage.app_settings().get(&flag).ok().flatten())
            .is_some();
        if already {
            continue;
        }
        if let Ok(storage) = state.storage.lock() {
            let _ = storage.app_settings().set(&flag, "1");
        }
        let helper = row
            .exe_path
            .rsplit('/')
            .next()
            .unwrap_or(&row.exe_path)
            .to_string();
        alerts.push(CleanupAlertInfo {
            kind: "idle".into(),
            text: format!(
                "{helper} has been idle for {} — review it in the Cleanup Centre.",
                age_text(age)
            ),
            pid,
            session: row.session_id.clone(),
        });
    }
    CleanupAlertsInfo { alerts }
}

/// The Instance's display label (account label → provider → session).
fn instance_label_for(state: &Arc<AppState>, session: &str) -> String {
    let lookup = (|| {
        let storage = state.storage.lock().ok()?;
        let record = storage
            .sessions()
            .get(&SessionId::new(session.to_string()))
            .ok()?;
        let recipe = storage.profiles().get(&record.profile).ok()?;
        let account = storage.accounts().get(&recipe.account).ok()?;
        Some(account.label.clone())
    })();
    lookup.unwrap_or_else(|| "its Instance".to_string())
}

/// Reads the passive Cleanup Centre alerts (AJ-E512 Req.3).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on read failure.
#[tauri::command]
pub fn cleanup_alerts(state: tauri::State<'_, Arc<AppState>>) -> AppResult<CleanupAlertsInfo> {
    Ok(cleanup_alerts_inner(&state))
}

/// The cleanup automation's status (the centre's banners + the
/// Settings surface read THIS — one truth).
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct CleanupStatusInfo {
    /// True while `Pause automatic cleanup` is on.
    pub paused: bool,
    /// True while the 24 h crash suspension is active.
    pub suspended: bool,
    /// The suspension's deadline, epoch ms (when `suspended`).
    pub suspended_until_ms: Option<i64>,
    /// The live Tier-2 notices (open Undo windows).
    pub tier2_pending: Vec<Tier2NoticeInfo>,
}

/// The one-time recovery summary (AJ-E510 Req.3): present exactly
/// once per crash — the second relaunch (no new findings) re-shows
/// nothing.
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct RecoverySummaryInfo {
    /// The exact template text.
    pub summary: String,
    /// `{a}` — Instances re-adopted (stoppable through the E507 path).
    pub adopted: usize,
    /// `{b}` — helpers left behind while Juggler was closed.
    pub left_behind: usize,
    /// `{c}` — processes that could not be verified (left alone).
    pub unverifiable: usize,
}

/// Reads the unconsumed recovery summary, if one is pending (shown
/// once, then dismissed forever until a new recovery finds something).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on read failure.
#[tauri::command]
pub fn recovery_summary_get(
    state: tauri::State<'_, Arc<AppState>>,
) -> AppResult<Option<RecoverySummaryInfo>> {
    recovery_summary_get_arc(&state)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on read failure.
pub fn recovery_summary_get_arc(state: &Arc<AppState>) -> AppResult<Option<RecoverySummaryInfo>> {
    let storage = lock_storage_arc(state)?;
    let Some(summary) = storage
        .app_settings()
        .get("recovery_summary")
        .map_err(AppError::from)?
        .filter(|text| !text.is_empty())
    else {
        return Ok(None);
    };
    let consumed = storage
        .app_settings()
        .get("recovery_summary_consumed")
        .map_err(AppError::from)?
        .is_some_and(|value| value == "1");
    if consumed {
        return Ok(None);
    }
    // Counts ride the text's shape (written by the same template).
    let count = |part: Option<&str>| {
        part.and_then(|text| text.split_whitespace().next())
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or_default()
    };
    let mut fields = summary.split(',');
    let adopted = count(fields.next());
    let left_behind = count(fields.next());
    let unverifiable = count(fields.next());
    Ok(Some(RecoverySummaryInfo {
        summary,
        adopted,
        left_behind,
        unverifiable,
    }))
}

/// Dismisses the summary (the only way it goes away; no permanent
/// dismiss shortcut exists by design).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on write failure.
#[tauri::command]
pub fn recovery_summary_dismiss(state: tauri::State<'_, Arc<AppState>>) -> AppResult<()> {
    let storage = lock_storage_arc(&state)?;
    storage
        .app_settings()
        .set("recovery_summary_consumed", "1")
        .map_err(AppError::from)?;
    Ok(())
}

/// Reads the automation status (Tier 0 is always the fallback — this
/// surface never signals).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on lock failure.
#[tauri::command]
pub fn cleanup_status(state: tauri::State<'_, Arc<AppState>>) -> AppResult<CleanupStatusInfo> {
    cleanup_status_inner(&state)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on lock failure.
pub fn cleanup_status_inner(state: &Arc<AppState>) -> AppResult<CleanupStatusInfo> {
    Ok(cleanup_status_of(state))
}

/// The status read (shared by the command and the sweep's tests).
fn cleanup_status_of(state: &Arc<AppState>) -> CleanupStatusInfo {
    let tier2_pending = state
        .tier2_pending
        .lock()
        .map(|pending| {
            pending
                .values()
                .filter(|notice| !notice.keep)
                .map(|notice| Tier2NoticeInfo {
                    pid: notice.pid,
                    session: notice.session.clone(),
                    cmdline_trunc: notice.cmdline_trunc.clone(),
                    deadline_ms: notice.deadline_ms,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    CleanupStatusInfo {
        paused: cleanup_paused(state),
        suspended: cleanup_suspended_until(state).is_some(),
        suspended_until_ms: cleanup_suspended_until(state),
        tier2_pending,
    }
}

/// Sets the global `Pause automatic cleanup` kill-switch (Req.4):
/// takes effect within one tick (the sweep reads the flag first).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on write failure.
#[tauri::command]
pub fn cleanup_pause_set(
    state: tauri::State<'_, Arc<AppState>>,
    paused: bool,
) -> AppResult<CleanupStatusInfo> {
    cleanup_pause_set_arc(&state, paused)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on write failure.
pub fn cleanup_pause_set_arc(state: &Arc<AppState>, paused: bool) -> AppResult<CleanupStatusInfo> {
    {
        let storage = lock_storage_arc(state)?;
        storage
            .app_settings()
            .set("cleanup_paused", if paused { "1" } else { "0" })
            .map_err(AppError::from)?;
    }
    Ok(cleanup_status_of(state))
}

/// The user's Tier-2 decision on one notice (Req.1): `keep` = Undo /
/// Keep running — the candidate is never signalled and the notice is
/// dismissed (exactly one notification per crash-orphan, by construction).
/// # Errors
///
/// Returns the typed [`AppError`] message on lock failure.
#[tauri::command]
pub fn cleanup_tier2_decide(
    state: tauri::State<'_, Arc<AppState>>,
    pid: u32,
    keep: bool,
) -> AppResult<CleanupStatusInfo> {
    cleanup_tier2_decide_arc(&state, pid, keep)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on lock failure.
pub fn cleanup_tier2_decide_arc(
    state: &Arc<AppState>,
    pid: u32,
    keep: bool,
) -> AppResult<CleanupStatusInfo> {
    if let Ok(mut pending) = state.tier2_pending.lock()
        && let Some(notice) = pending.get_mut(&pid)
    {
        notice.keep = keep;
    }
    Ok(cleanup_status_of(state))
}

/// Re-confirms automatic cleanup after the 24 h crash suspension
/// (Req.5 — the Cleanup Centre's button; shown, never silent).
/// # Errors
///
/// Returns the typed [`AppError`] message on write failure.
#[tauri::command]
pub fn cleanup_resume_after_crash(
    state: tauri::State<'_, Arc<AppState>>,
) -> AppResult<CleanupStatusInfo> {
    cleanup_resume_after_crash_arc(&state)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on write failure.
pub fn cleanup_resume_after_crash_arc(state: &Arc<AppState>) -> AppResult<CleanupStatusInfo> {
    {
        let storage = lock_storage_arc(state)?;
        storage
            .app_settings()
            .set("cleanup_suspended_until", "0")
            .map_err(AppError::from)?;
    }
    Ok(cleanup_status_of(state))
}

/// Reads one Account's two tier opt-ins (absent rows = both off).
/// # Errors
///
/// Returns the typed [`AppError`] message on read failure.
#[tauri::command]
pub fn cleanup_tiers_get(
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
) -> AppResult<juggler_core::storage::CleanupTierSettings> {
    cleanup_tiers_get_arc(&state, &account_id)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on read failure.
pub fn cleanup_tiers_get_arc(
    state: &Arc<AppState>,
    account_id: &str,
) -> AppResult<juggler_core::storage::CleanupTierSettings> {
    Ok(state.cleanup_settings.get(account_id).unwrap_or_default())
}

/// The Account launch-settings area's two SEPARATE toggles (Req.4,
/// exact labels in the UI). Also mirrors Tier 1 onto the legacy
/// `accounts.cleanup_opt_in` column so older readers stay truthful.
/// # Errors
///
/// Returns the typed [`AppError`] message on write failure.
#[tauri::command]
pub fn cleanup_tiers_set(
    state: tauri::State<'_, Arc<AppState>>,
    account_id: String,
    tier1_opt_in: bool,
    tier2_opt_in: bool,
) -> AppResult<juggler_core::storage::CleanupTierSettings> {
    cleanup_tiers_set_arc(&state, &account_id, tier1_opt_in, tier2_opt_in)
}

/// The Arc-based core (tests call this directly).
///
/// # Errors
///
/// Returns the typed [`AppError`] message on write failure.
pub fn cleanup_tiers_set_arc(
    state: &Arc<AppState>,
    account_id: &str,
    tier1_opt_in: bool,
    tier2_opt_in: bool,
) -> AppResult<juggler_core::storage::CleanupTierSettings> {
    let settings = juggler_core::storage::CleanupTierSettings {
        tier1_opt_in,
        tier2_opt_in,
    };
    state
        .cleanup_settings
        .set(account_id, &settings)
        .map_err(AppError::from)?;
    // Mirror Tier 1 onto the prototype column (one honest source:
    // the tier table; the account row follows).
    account_set_cleanup_arc(state, account_id, tier1_opt_in).ok();
    Ok(settings)
}

// ---------------------------------------------------------------------------
// AJ-E412: running-model telemetry — the transcript-observed model.
// ---------------------------------------------------------------------------

/// Scans NEW transcript bytes (tracked per-Instance parse offset) for
/// the newest recorded turn model, and persists + classifies it on
/// change (`transcript` when it matches the launch overlay,
/// `in-session` when it differs — an observed `/model` change). Only
/// transcript lines are read: no Provider config files, no network
/// (Req.2/Req.7 — local-only by construction).
///
/// Returns true when any Instance's model changed (the caller emits
/// `sessions_updated`).
pub fn detect_running_models(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    only: Option<&[SessionId]>,
) -> bool {
    let changed = detect_running_models_core(state, only);
    if changed {
        let _ = emit_sessions(app, state);
    }
    changed
}

/// The storage-side core (tests call this directly — no app handle).
pub fn detect_running_models_core(state: &Arc<AppState>, only: Option<&[SessionId]>) -> bool {
    let sessions: Vec<(SessionId, Option<String>, std::path::PathBuf)> = {
        let Ok(storage) = state.storage.lock() else {
            return false;
        };
        storage
            .sessions()
            .list(None)
            .unwrap_or_default()
            .into_iter()
            .filter(|record| !is_terminal(record.status))
            .filter(|record| only.is_none_or(|ids| ids.contains(&record.id)))
            .filter_map(|record| {
                let overlay = storage
                    .profiles()
                    .get(&record.profile)
                    .ok()
                    .and_then(|recipe| recipe.settings_overlay.model);
                let path = record.transcript_ref.as_ref()?.path.clone();
                Some((record.id, overlay, path))
            })
            .collect()
    };
    let mut changed = false;
    for (session, launch_model, path) in sessions {
        if scan_one_transcript(state, &session, launch_model.as_ref(), &path) {
            changed = true;
        }
    }
    changed
}

/// Tail-scans one Instance's transcript for the newest observed model;
/// true when the stored observation changed. Incremental by byte offset
/// (truncation resets to zero — re-parse, never assume).
fn scan_one_transcript(
    state: &Arc<AppState>,
    session: &SessionId,
    launch_model: Option<&String>,
    path: &std::path::Path,
) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(metadata) = std::fs::metadata(path) else {
        return false; // no transcript yet: not observed, never guessed
    };
    let len = metadata.len();
    let offset = state
        .model_offsets
        .lock()
        .ok()
        .and_then(|offsets| offsets.get(session).copied())
        .unwrap_or(0);
    if len < offset {
        // Truncated/rotated: reset and re-parse from the start.
        if let Ok(mut offsets) = state.model_offsets.lock() {
            offsets.insert(session.clone(), 0);
        }
        return scan_one_transcript(state, session, launch_model, path);
    }
    if len == offset {
        return false; // nothing new — one metadata read, no parse
    }
    // Read only the NEW bytes (plus a carry for the half-parsed line):
    // seek past the consumed prefix, never re-parse the head.
    let read_from = offset.saturating_sub(1);
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    if file.seek(SeekFrom::Start(read_from)).is_err() {
        return false;
    }
    let mut bytes = Vec::with_capacity(usize::try_from(len - read_from).unwrap_or(0));
    if file
        .by_ref()
        .take(len - read_from)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return false;
    }
    if let Ok(mut offsets) = state.model_offsets.lock() {
        offsets.insert(session.clone(), len);
    }
    // The newest model recorded on a complete line wins.
    let mut observed: Option<String> = None;
    for line in String::from_utf8_lossy(&bytes).lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(model) = line_model(&value) {
            observed = Some(model);
        }
    }
    let Some(model) = observed else {
        return false;
    };
    let source = if launch_model == Some(&model) {
        juggler_core::model::ModelSource::Transcript
    } else {
        juggler_core::model::ModelSource::InSession
    };
    let next = juggler_core::model::RunningModel { model, source };
    persist_running_model(state, session, &next)
}

/// Persists an observed running model; false when unchanged (no write,
/// no emit) or when the row is gone.
fn persist_running_model(
    state: &Arc<AppState>,
    session: &SessionId,
    next: &juggler_core::model::RunningModel,
) -> bool {
    let Ok(storage) = state.storage.lock() else {
        return false;
    };
    let Ok(mut record) = storage.sessions().get(session) else {
        return false;
    };
    if record.running.as_ref() == Some(next) {
        return false; // unchanged: no write, no emit
    }
    record.running = Some(next.clone());
    storage.sessions().update(&record).is_ok()
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
            running: None,
            window: None,
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
                cleanup_opt_in: false,
                renewal_date: None,
                preview_opt_in: false,
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
        let rows =
            session_infos(&storage, &live, &std::collections::BTreeMap::new(), &[]).expect("rows");
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
        let rows = session_infos(
            &storage,
            &LiveStatsMap::new(),
            &std::collections::BTreeMap::new(),
            &[],
        )
        .expect("rows");

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

    /// AJ-E413 Req.1: desktop rows carry the per-Provider isolation
    /// claim verbatim; terminal rows and unregistered providers stay
    /// `null` — the panel never guesses a limit.
    #[test]
    fn session_info_desktop_isolation_label_maps_per_provider() {
        let storage = fixture_storage();
        let mut desktop = record(
            "sess-desk",
            "rec-1",
            SessionStatus::Running,
            git_cwd("feat/x"),
        );
        desktop.kind = InstanceKind::Desktop;
        storage.sessions().insert(&desktop).expect("seed desktop");

        let labels = std::collections::BTreeMap::from([(
            "claude-code".to_string(),
            "Single-account app — Juggler tracks, cannot isolate".to_string(),
        )]);
        let rows = session_infos(&storage, &LiveStatsMap::new(), &labels, &[]).expect("rows");
        assert_eq!(
            row(&rows, "sess-desk").isolation_label.as_deref(),
            Some("Single-account app — Juggler tracks, cannot isolate")
        );
        // Terminal instances never carry a desktop claim.
        assert_eq!(row(&rows, "sess-full").isolation_label, None);
        // Desktop row whose provider has no registered label: null, not
        // an inherited or default claim.
        let bare = session_infos(
            &storage,
            &LiveStatsMap::new(),
            &std::collections::BTreeMap::new(),
            &[],
        )
        .expect("rows");
        assert_eq!(row(&bare, "sess-desk").isolation_label, None);
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
                cleanup_opt_in: false,
                renewal_date: None,
                preview_opt_in: false,
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
            let rows = session_infos(
                &storage,
                &LiveStatsMap::new(),
                &std::collections::BTreeMap::new(),
                &[],
            )
            .expect("rows");
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
        let rows = session_infos(
            &storage,
            &LiveStatsMap::new(),
            &std::collections::BTreeMap::new(),
            &[],
        )
        .expect("rows");
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

/// The still-holding report's holder exe (basename of the live
/// command's first token — `Listeners` carry no exe by design).
fn holder_exe(pid: u32) -> String {
    juggler_supervisor::live_details(pid).map_or_else(
        || "?".to_string(),
        |(cmdline, _)| {
            cmdline
                .split_whitespace()
                .next()
                .unwrap_or("?")
                .rsplit('/')
                .next()
                .unwrap_or("?")
                .to_string()
        },
    )
}
