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
    use juggler_core::model::MachineId;

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
        let storage = lock_storage_arc(state)?;
        let project = storage.projects().touch(&launch.cwd, None)?;
        storage.sessions().insert(&SessionRecord {
            id: session_id.clone(),
            conversation: None,
            profile: recipe_id,
            machine: MachineId::new("this-mac"),
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
        // D-17: the REAL capture source — the audited init shim
        // (crates/juggler-scap-init) satisfies the binding's init
        // requirement inside the workspace's unsafe-free law.
        let source: std::sync::Arc<dyn crate::preview::CaptureSource> =
            std::sync::Arc::new(crate::preview::ScreenshotSource);
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
    let storage = lock_storage(&state)?;
    session_infos(&storage, &live, &labels)
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
        session_infos(&storage, &live, &desktop_isolation_labels(&state.registry))?
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

/// Stops one orphan: the allow-list guard refuses PIDs that were not
/// recorded as descendants (fail closed, PROPOSAL.md:47), then the
/// TERM→KILL ladder runs with the post-stop re-read built in (Req.6).
/// Every kill lands in the journal's audit trail (D-08).
///
/// # Errors
///
/// Returns the typed [`AppError`] message when the Instance is unknown.
#[tauri::command]
pub fn orphan_stop(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    pid: u32,
) -> AppResult<StopOutcomeInfo> {
    Ok(orphan_stop_inner(&state, &SessionId::new(session_id), pid))
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
    Ok(state
        .orphans
        .orphans_of(&session)
        .iter()
        .map(|orphan| orphan_stop_inner(&state, &session, orphan.pid))
        .collect())
}

/// Shared stop path for the command and `Stop all` (guard → ladder →
/// audit → tracker).
pub fn orphan_stop_inner(state: &Arc<AppState>, session: &SessionId, pid: u32) -> StopOutcomeInfo {
    if !state.orphans.allows(session, pid) {
        let outcome = juggler_supervisor::StopOutcome::RefusedNotRecorded;
        state.orphans.record_stop(pid, outcome.clone());
        return StopOutcomeInfo {
            pid,
            state: outcome.state_label().to_string(),
        };
    }
    let outcome = juggler_supervisor::stop_pid_ladder(pid, Duration::from_secs(2), |pid| {
        juggler_supervisor::process_alive(pid).is_some()
    });
    state.orphans.record_stop(pid, outcome.clone());
    if let Some(journal) = state.supervisor.journal_ref() {
        let _ = journal.record_killed(session, pid, "manual", outcome.state_label());
    }
    StopOutcomeInfo {
        pid,
        state: outcome.state_label().to_string(),
    }
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
    let mut confirmed = Vec::new();
    {
        let Ok(mut collector) = state.resources.lock() else {
            return;
        };
        collector.tick(&state.supervisor, now);
        let tracked = state.orphans.tracked_pids();
        if !tracked.is_empty() {
            collector.refresh_pids(&tracked);
            confirmed = state.orphans.poll(now, |pid| collector.lookup(pid));
        }
    }
    if confirmed.is_empty() {
        return;
    }
    // Tier-1 sweep: only Accounts the user opted in, only recorded
    // descendants, only past the 60 s auto grace.
    auto_cleanup_sweep(state, now, &confirmed);
}

/// Runs the opted-in auto-cleanup over the freshly confirmed orphans
/// past [`juggler_supervisor::AUTO_CLEANUP_GRACE_MS`]. Notify-only by
/// default: an Account without the opt-in is never touched here.
pub fn auto_cleanup_sweep(
    state: &Arc<AppState>,
    now: i64,
    confirmed: &[juggler_supervisor::Orphan],
) {
    for orphan in confirmed {
        if now - orphan.parent_exit_ms < juggler_supervisor::AUTO_CLEANUP_GRACE_MS {
            continue;
        }
        let opted_in = account_cleanup_opt_in(state, &orphan.session);
        if !opted_in {
            continue;
        }
        if !state.orphans.allows(&orphan.session, orphan.pid) {
            continue; // fail closed even inside the sweep
        }
        let outcome =
            juggler_supervisor::stop_pid_ladder(orphan.pid, Duration::from_secs(5), |pid| {
                juggler_supervisor::process_alive(pid).is_some()
            });
        state.orphans.record_stop(orphan.pid, outcome.clone());
        if let Some(journal) = state.supervisor.journal_ref() {
            let _ = journal.record_killed(
                &orphan.session,
                orphan.pid,
                "auto-tier1",
                outcome.state_label(),
            );
        }
    }
}

/// Resolves an Instance's Account opt-in (session → launch settings →
/// account). Unknown links read as not opted-in (fail closed).
fn account_cleanup_opt_in(state: &Arc<AppState>, session: &SessionId) -> bool {
    let Ok(storage) = state.storage.lock() else {
        return false;
    };
    let Ok(record) = storage.sessions().get(session) else {
        return false;
    };
    let Ok(recipe) = storage.profiles().get(&record.profile) else {
        return false;
    };
    storage
        .accounts()
        .get(&recipe.account)
        .is_ok_and(|account| account.cleanup_opt_in)
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
            session_infos(&storage, &live, &std::collections::BTreeMap::new()).expect("rows");
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
        let rows = session_infos(&storage, &LiveStatsMap::new(), &labels).expect("rows");
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
