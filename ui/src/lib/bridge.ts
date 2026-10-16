import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { AppInfo } from "../types/generated/AppInfo";
import type { AccountInfo } from "../types/generated/AccountInfo";
import type { DiscoveredInfo } from "../types/generated/DiscoveredInfo";
import type { ImportResult } from "../types/generated/ImportResult";
import type { InstanceInfo } from "../types/generated/InstanceInfo";
import type { InstanceQuotaInfo } from "../types/generated/InstanceQuotaInfo";
import type { InstanceResourcesInfo } from "../types/generated/InstanceResourcesInfo";
import type { LaunchRecipeInfo } from "../types/generated/LaunchRecipeInfo";
import type { OrphanVerifyInfo } from "../types/generated/OrphanVerifyInfo";
import type { DesktopLaunchInfo } from "../types/generated/DesktopLaunchInfo";
import type { PreviewFrameInfo } from "../types/generated/PreviewFrameInfo";
import type { PreviewStateInfo } from "../types/generated/PreviewStateInfo";
import type { FocusOutcomeInfo } from "../types/generated/FocusOutcomeInfo";
import type { ProcessesInfo } from "../types/generated/ProcessesInfo";
import type { StopOutcomeInfo } from "../types/generated/StopOutcomeInfo";
import type { LaunchResult } from "../types/generated/LaunchResult";
import type { NeedsAttention } from "../types/generated/NeedsAttention";
import type { ProjectInfo } from "../types/generated/ProjectInfo";
import type { ProviderInfo } from "../types/generated/ProviderInfo";
import type { QuotaSnapshotInfo } from "../types/generated/QuotaSnapshotInfo";
import type { QualitySignalsInfo } from "../types/generated/QualitySignalsInfo";
import type { StatsInfo } from "../types/generated/StatsInfo";
import type { TerminalInfo } from "../types/generated/TerminalInfo";
import type { TokenCountsInfo } from "../types/generated/TokenCountsInfo";

export type {
  AppInfo,
  FocusOutcomeInfo,
  PreviewFrameInfo,
  PreviewStateInfo,
  QualitySignalsInfo,
  ProcessesInfo,
  StopOutcomeInfo,
  OrphanVerifyInfo,
  AccountInfo,
  DiscoveredInfo,
  ImportResult,
  InstanceInfo,
  InstanceQuotaInfo,
  InstanceResourcesInfo,
  LaunchRecipeInfo,
  LaunchResult,
  NeedsAttention,
  ProjectInfo,
  ProviderInfo,
  QuotaSnapshotInfo,
  StatsInfo,
  TerminalInfo,
  TokenCountsInfo,
};

// Deprecated one-version aliases (AJ-E107).
export type { ProviderInfo as HarnessInfo } from "../types/generated/ProviderInfo";
export type { LaunchRecipeInfo as ProfileInfo } from "../types/generated/LaunchRecipeInfo";
export type { InstanceInfo as SessionInfo } from "../types/generated/InstanceInfo";

/** Event names mirrored from juggler_app constants (single source: Rust). */
export const EVENTS = {
  appReady: "app://ready",
  sessionsUpdated: "session://updated",
  /** 5 s resource tick (I-07/I-08 deltas; AJ-E401). */
  sessionsResources: "session://resources",
  /** 30 s quota tick (I-05 deltas; AJ-E401). */
  sessionsQuota: "session://quota",
  /** Global "Go to instance…" hotkey (AJ-E414). */
  goTo: "app://go-to",
  /** Live-preview frame (AJ-E415). */
  previewFrame: "preview://frame",
  /** Live-preview phase change (AJ-E415). */
  previewState: "preview://state",
} as const;

const hasBackend =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function hasLiveBackend(): boolean {
  return hasBackend;
}

// --- design-preview mocks (plain-browser + visual harness) ----------------
// The harness (AJ-E802) seeds `window.__AIJ_FIXTURE__` before this module
// loads; mock calls then serve the seeded fixture set (deterministic
// captures) instead of the defaults below.

export interface FixtureSet {
  accounts: AccountInfo[];
  profiles: LaunchRecipeInfo[];
  sessions: InstanceInfo[];
  projects: ProjectInfo[];
  stats: StatsInfo;
}

declare global {
  interface Window {
    __AIJ_FIXTURE__?: Partial<FixtureSet>;
  }
}

const seeded = (typeof window !== "undefined" && window.__AIJ_FIXTURE__) || {};

/** A `YYYY-MM-DD` string N days from today (fixture dates move with the clock). */
function renewalFixtureInDays(days: number): string {
  const target = new Date(Date.now() + days * 86_400_000);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${target.getFullYear()}-${pad(target.getMonth() + 1)}-${pad(target.getDate())}`;
}

const MOCK_ACCOUNTS: AccountInfo[] = [
  { id: "claude-work", provider: "claude-code", label: "Work Max", plan: Some("max-20x"), login_state: "logged_in", cleanup_opt_in: false, renewal_date: Some(renewalFixtureInDays(6)), preview_opt_in: false },
  { id: "claude-personal", provider: "claude-code", label: "Personal", plan: None(), login_state: "logged_out", cleanup_opt_in: false, renewal_date: None(), preview_opt_in: false },
];

const MOCK_PROFILES: LaunchRecipeInfo[] = [
  { id: "p-1", name: "Deep work", account_id: "claude-work", account_label: "Work Max", provider: "claude-code", model: Some("opus"), default_cwd: None() },
  { id: "p-2", name: "Quick fixes", account_id: "claude-work", account_label: "Work Max", provider: "claude-code", model: Some("sonnet"), default_cwd: None() },
  { id: "p-3", name: "Weekend build", account_id: "claude-personal", account_label: "Personal", provider: "claude-code", model: None(), default_cwd: None() },
];

const MOCK_QUALITY: Record<string, QualitySignalsInfo> = {
  // sess-8 sorts first (needs-me) and is what the captures click; sess-2
  // is the running row.
  "sess-8": {
    tokens_bars: [300, 410, 380, 500, 440],
    avg_tokens_per_min: 406,
    error_label: "1 errors / 12 turns",
    errors_not_measurable: false,
    context_used: null,
    context_window: null,
    cumulative: { input: 4_200, output: 1_800, cache_read: 20, cache_write: 40, reasoning: 8 },
    session_cost_usd: 0.19,
    priced_model: "opus",
    quota_forecast: "not burning",
    stuck: { rule: "stuck_no_output", inputs: ["no output 8m", "cpu 0.3%", "transcript stale 8m"], since_ms: Date.now() - 8 * 60_000 },
  },
  "sess-2": {
    tokens_bars: [420, 510, 380, 600, 540],
    avg_tokens_per_min: 490,
    error_label: "2 errors / 41 turns",
    errors_not_measurable: false,
    context_used: null,
    context_window: null,
    cumulative: { input: 12_345, output: 6_789, cache_read: 100, cache_write: 200, reasoning: 42 },
    session_cost_usd: 0.69,
    priced_model: "opus",
    quota_forecast: "not burning",
    stuck: null,
  },
  "sess-live-1": {
    tokens_bars: [420, 510, 380, 600, 540],
    avg_tokens_per_min: 490,
    error_label: "2 errors / 41 turns",
    errors_not_measurable: false,
    context_used: null,
    context_window: null,
    cumulative: { input: 12_345, output: 6_789, cache_read: 100, cache_write: 200, reasoning: 42 },
    session_cost_usd: 0.69,
    priced_model: "opus",
    quota_forecast: "not burning",
    stuck: null,
  },
};

// AJ-E410 mock trees: a 3-level live tree (the §7.3 wireframe shape)
// plus a 2-orphan section — one fresh, one already `gone`.
const MOCK_PROCESSES: Record<string, ProcessesInfo> = {
  "sess-8": {
    rows: [
      { pid: 41201, parent_pid: 40100, name: "claude", cpu_pct: 11.0, rss_bytes: 890_000_000, age_ms: 42 * 60_000, child_count: 2, depth: 0 },
      { pid: 41233, parent_pid: 41201, name: "node (mcp)", cpu_pct: 1.0, rss_bytes: 96_000_000, age_ms: 40 * 60_000, child_count: 1, depth: 1 },
      { pid: 41302, parent_pid: 41233, name: "rg (search)", cpu_pct: 4.0, rss_bytes: 120_000_000, age_ms: 12_000, child_count: 0, depth: 2 },
    ],
    spark: Array.from({ length: 24 }, (_, i) => ({
      at_ms: Date.now() - (24 - i) * 2_000,
      cpu_pct: 6 + ((i * 7) % 9),
      rss_bytes: 980_000_000 + ((i * 13) % 5) * 20_000_000,
    })),
    orphans: [
      { session_id: "sess-8", pid: 39871, name: "node dev-server", rss_bytes: 210_000_000, parent_label: "claude", age_since_parent_exit_ms: 18 * 60_000, stop_state: null },
      { session_id: "sess-8", pid: 39890, name: "vite", rss_bytes: 88_000_000, parent_label: "claude", age_since_parent_exit_ms: 18 * 60_000, stop_state: "gone" },
    ],
    unavailable_reason: null,
  },
  "sess-2": {
    rows: [
      { pid: 51001, parent_pid: 50900, name: "claude", cpu_pct: 3.0, rss_bytes: 240_000_000, age_ms: 9 * 60_000, child_count: 0, depth: 0 },
    ],
    spark: Array.from({ length: 24 }, (_, i) => ({
      at_ms: Date.now() - (24 - i) * 2_000,
      cpu_pct: 2 + ((i * 3) % 4),
      rss_bytes: 240_000_000,
    })),
    orphans: [],
    unavailable_reason: null,
  },
};

const MOCK_STATS: StatsInfo =
  seeded.stats ?? {
    accounts: 2,
    profiles: 3,
    active_sessions: 0,
    ledger_degraded: false,
  };

// Mock rows mirror the AJ-E401 read-model fixtures: one full row, one
// quota-less row, one model-less/terminal row — nulls where the backend
// reports absence, never zeros or guessed names.
const MOCK_SESSIONS: InstanceInfo[] = seeded.sessions ?? [
  {
    id: "sess-live-1",
    adopted: false,
    launch_recipe_id: "p-1",
    provider: "claude-code",
    account_id: "claude-work",
    account_label: "Work Max",
    model: Some("opus"),
    launch_model: "opus",
    running_model: null,
    model_source: "opus" === null ? "unknown" : "launch",
    status: "running",
    quota: Some({
      used_percentage: 42.5,
      resets_at_ms: Date.now() + 3_600_000,
      confidence: "computed",
    }),
    tokens: Some({ input: 12_345, output: 6_789, cache_read: 100, cache_write: 200, reasoning: 42 }),
    cpu_pct: Some(3.5),
    rss_bytes: Some(123_456),
    child_count: Some(2),
    subtask_count: null,
    git_branch: Some("feat/read-model"),
    git_dirty_count: null,
    needs_attention: null,
    attention: null,
    started_at_ms: Date.now() - 1_200_000,
    last_activity_ms: Date.now() - 30_000,
    machine_id: "this-mac",
    cwd: "/Users/dev/ai-juggler",
    kind: "terminal",
    isolation_label: null,
    project_id: "proj-ai-juggler",
    conversation_id: "822e4125-e07d-4866-9954-ec9295efb079",
  },
  {
    id: "sess-done-1",
    adopted: false,
    launch_recipe_id: "p-3",
    provider: "claude-code",
    account_id: "claude-personal",
    account_label: "Personal",
    model: null,
    launch_model: null,
    running_model: null,
    model_source: null === null ? "unknown" : "launch",
    status: "finished",
    quota: null,
    tokens: null,
    cpu_pct: null,
    rss_bytes: null,
    child_count: Some(0),
    subtask_count: null,
    git_branch: null,
    git_dirty_count: null,
    needs_attention: null,
    attention: null,
    started_at_ms: Date.now() - 86_400_000,
    last_activity_ms: Date.now() - 80_000_000,
    machine_id: "this-mac",
    cwd: "/Users/dev/ai-juggler",
    kind: "terminal",
    isolation_label: null,
    project_id: "proj-payments",
    conversation_id: null,
  },
  {
    id: "sess-done-2",
    adopted: false,
    launch_recipe_id: "p-2",
    provider: "claude-code",
    account_id: "claude-work",
    account_label: "Work Max",
    model: Some("sonnet"),
    launch_model: "sonnet",
    running_model: null,
    model_source: "sonnet" === null ? "unknown" : "launch",
    status: "finished",
    quota: Some({ used_percentage: 100, resets_at_ms: Date.now() + 7_200_000, confidence: "measured" }),
    tokens: Some({ input: 98_000, output: 51_000, cache_read: 9_000, cache_write: 4_000, reasoning: 800 }),
    cpu_pct: null,
    rss_bytes: null,
    child_count: Some(0),
    subtask_count: null,
    git_branch: Some("main"),
    git_dirty_count: null,
    needs_attention: null,
    attention: null,
    started_at_ms: Date.now() - 43_200_000,
    last_activity_ms: Date.now() - 600_000,
    machine_id: "this-mac",
    cwd: "/Users/dev/ai-juggler",
    kind: "terminal",
    isolation_label: null,
    project_id: "proj-payments",
    conversation_id: "c3f10b52-9a41-4f0e-8f2a-7717db9c04aa",
  },
];

const MOCK_PROJECTS: ProjectInfo[] = seeded.projects ?? [
  {
    id: "proj-ai-juggler",
    root: "/Users/you/Dev/ai-juggler",
    name: "ai-juggler",
    last_activity_ms: Date.now() - 60_000,
    active_instance: MOCK_SESSIONS[0],
    accounts_used: ["Work Max"],
    instance_count: 1,
    history_sessions: 0,
    missing: false,
  },
  {
    id: "proj-payments",
    root: "/Users/you/Dev/payments-api",
    name: "payments-api",
    last_activity_ms: Date.now() - 3_600_000,
    active_instance: null,
    accounts_used: ["Personal", "Work Max"],
    instance_count: 2,
    history_sessions: 14, missing: false,
  },
];

function Some<T>(value: T): T | null {
  return value;
}

function None<T>(): T | null {
  return null;
}

// --- typed calls -----------------------------------------------------------

export async function fetchAppInfo(): Promise<AppInfo | null> {
  if (!hasBackend) {
    return { version: "0.0.0-preview", name: "AI Juggler" };
  }
  try {
    return await invoke<AppInfo>("app_info");
  } catch {
    return null;
  }
}

export async function providersList(): Promise<ProviderInfo[]> {
  if (!hasBackend) {
    return [{ id: "claude-code", name: "Claude Code" }];
  }
  return invoke<ProviderInfo[]>("providers_list");
}

/** @deprecated legacy alias (AJ-E107) — use providersList. */
export const harnessesList = providersList;

export async function accountsList(): Promise<AccountInfo[]> {
  if (!hasBackend) {
    return seeded.accounts ?? MOCK_ACCOUNTS;
  }
  return invoke<AccountInfo[]>("accounts_list");
}

export async function createAccount(
  harness: string,
  label: string,
  plan: string | null,
): Promise<AccountInfo> {
  if (!hasBackend) {
    return {
      id: `claude-${label.toLowerCase().replace(/\s+/g, "-")}`,
      provider: harness,
      label,
      plan,
      login_state: "logged_out",
      cleanup_opt_in: false,
      renewal_date: null,
      preview_opt_in: false,
    };
  }
  return invoke<AccountInfo>("account_create", { harness, label, plan });
}

/**
 * Scans the machine for existing harness setups the onboarding wizard can
 * import (metadata-only). Mock: a plausible "found your default install"
 * result so the wizard is reviewable in a plain browser.
 */
export async function environmentScan(): Promise<DiscoveredInfo[]> {
  if (!hasBackend) {
    return [
      {
        harness: "claude-code",
        path: "/Users/you/.claude",
        origin: "vendor-default",
        login: "logged_in",
        bytes: 48_000_000,
        last_used_secs: Math.floor(Date.now() / 1000) - 86_400,
        managed: false,
        notes: [
          "Claude Code's standard location — what a plain `claude` command uses.",
          "Signed in — importing keeps your settings and session history.",
        ],
      },
      {
        harness: "claude-code",
        path: "/Users/you/.claude-side",
        origin: "home-pattern",
        login: "logged_out",
        bytes: 1_200_000,
        last_used_secs: Math.floor(Date.now() / 1000) - 2_592_000,
        managed: false,
        notes: [
          "A separate Claude configuration — created manually, by another tool, or by an AI assistant.",
        ],
      },
    ];
  }
  return invoke<DiscoveredInfo[]>("environment_scan");
}

/** Imports a discovered setup as a managed account (byte copy; source untouched). */
export async function accountImport(
  harness: string,
  sourcePath: string,
  label: string,
  plan: string | null,
): Promise<ImportResult> {
  if (!hasBackend) {
    return {
      account: {
        id: `claude-${label.toLowerCase().replace(/\s+/g, "-")}`,
        provider: harness,
        label,
        plan,
        login_state: "logged_out",
        cleanup_opt_in: false,
        renewal_date: null,
        preview_opt_in: false,
      },
      files_copied: 128,
      bytes_copied: 46_000_000,
      skipped: ["shell-snapshots/ (rebuildable cache)", "statsig/ (rebuildable cache)"],
      login: "logged_out",
      login_note:
        "Your Claude login lives in the macOS Keychain, locked to the original folder — it can't (and shouldn't) be moved. Sign in once inside the imported account; it takes a few seconds.",
    };
  }
  return invoke<ImportResult>("account_import", { harness, sourcePath, label, plan });
}

/** Opens the vendor's own web sign-in page in the system browser. */
export async function loginOpenWeb(harness: string): Promise<void> {
  return invoke<void>("login_open_web", { harness });
}

export async function accountLoginCommand(accountId: string): Promise<string> {
  if (!hasBackend) {
    return `CLAUDE_CONFIG_DIR='~/Library/Application Support/AI Juggler/accounts/claude-code/${accountId}' claude`;
  }
  return invoke<string>("account_login_command", { accountId });
}

export async function accountLoginState(accountId: string): Promise<string> {
  if (!hasBackend) {
    return "logged_out";
  }
  return invoke<string>("account_login_state", { accountId });
}

export async function launchRecipesList(): Promise<LaunchRecipeInfo[]> {
  if (!hasBackend) {
    return MOCK_PROFILES;
  }
  return invoke<LaunchRecipeInfo[]>("launch_recipes_list");
}

/** @deprecated legacy alias (AJ-E107) — use launchRecipesList. */
export const profilesList = launchRecipesList;

export async function createProfile(
  accountId: string,
  name: string,
  model: string | null,
  defaultCwd: string | null,
): Promise<LaunchRecipeInfo> {
  return invoke<LaunchRecipeInfo>("profile_create", {
    accountId,
    name,
    model,
    defaultCwd,
  });
}

export async function deleteProfile(profileId: string): Promise<void> {
  await invoke("profile_delete", { profileId });
}

export async function launchSession(
  profileId: string,
  cwd: string | null,
): Promise<{ session_id: string; pid: number } | null> {
  if (!hasBackend) {
    return null; // preview mode: nothing really launches
  }
  return invoke<{ session_id: string; pid: number }>("launch_session", {
    profileId,
    cwd,
  });
}

/** Tracked projects — the juggling anchor (which instance is on each directory). */
export async function projectsList(): Promise<ProjectInfo[]> {
  if (!hasBackend) {
    return MOCK_PROJECTS;
  }
  return invoke<ProjectInfo[]>("projects_list");
}

export async function instancesList(): Promise<InstanceInfo[]> {
  if (!hasBackend) {
    return MOCK_SESSIONS;
  }
  return invoke<InstanceInfo[]>("instances_list");
}

/** @deprecated legacy alias (AJ-E107) — use instancesList. */
export const sessionsList = instancesList;

/** AJ-E413: launch the Provider's desktop app under an Account. */
export async function desktopLaunch(accountId: string): Promise<DesktopLaunchInfo> {
  if (!hasBackend) {
    return {
      session_id: "sess-desktop-1",
      pid: 42_042,
      bundle_id: "dev.aijuggler.fake-desktop",
      isolation_label: "Single-account app — Juggler tracks, cannot isolate",
    };
  }
  return invoke<DesktopLaunchInfo>("desktop_launch", { accountId });
}

/** AJ-E414: run the focus tiers for one desktop Instance — activate,
 * match the recorded window identity, clear AXMinimized, AXRaise. The
 * returned outcome says exactly which tier ran (never overstated). */
export async function desktopFocus(sessionId: string): Promise<FocusOutcomeInfo> {
  if (!hasBackend) {
    // Harness seam (AJ-E802): captures may pin the outcome they render.
    const w = window as unknown as { __AIJ_FOCUS_OUTCOME__?: FocusOutcomeInfo };
    return (
      w.__AIJ_FOCUS_OUTCOME__ ?? {
        outcome: "Focused window",
        permission_missing: false,
        window_title: "notes-a",
      }
    );
  }
  return invoke<FocusOutcomeInfo>("desktop_focus", { sessionId });
}


/** AJ-E415: the deterministic mock preview frame (a rendered fake-app
 * window; harness captures pin phases via __AIJ_PREVIEW_PHASE__). */
const MOCK_PREVIEW_PNG_B64 = "iVBORw0KGgoAAAANSUhEUgAAAUAAAADICAIAAAAWZq/8AAACA0lEQVR42u3cMQqDQBCGUQ8hGEhY0SZ4hRSpQ4ocQSxzBk/vBazE5ncfvBMsfNXOTPN6f4FQjScAAQMCBgQMAgYEDAgYEDAIGBAwIGAQMCBgQMCAgEHAgIABAQMCBgEDAgYOB1yGCQglYEgOuO0KEErAIGBAwICAQcCAgAEBAwIGAQMCBgQMAgYEDAgYEDBcP+DlvwKhBAwCBgQMCBgEDAgYEDAgYBAwIGBAwFBzwMbBwTYSIGBAwCBgQMCAgAEBg4ABAQMCBgEDiQF/fjMQSsAgYEDAgIBBwICAAQEDewH7CgeTWICAAQGDgAEBAwIGBAwCBgQMCBgEDCQG/OifQCgBg4ABAQMCBgEDAgYEDAgYBAwIGBAw1Bzw7T4CoQQMAgYEDAgYBAwIGBAwIGAQMCBgQMBQc8DGwcE2EiBgQMAgYEDAgIABAYOAAQEDAgYBA+5CA7aRAAGDgAEBAwIGBAwCBgQMCBgEDAgYcBcasI0EAgYEDAgYEDAIGBAwIGAQMCBgQMCAu9BgGwkQMCBgELBXAAEDAgYEDAIGBAwIGBAwuAsN2EYCBAwCBvICbrsChBIwCBgQMCBgEDAgYEDAgIBBwICAAQGDgAEBAwIGBAwCBgQMCBgQMAgYEDAgYBAwIGBAwICAQcCAgAEBAwIGAQMCBgQMAgYEDAgYEDAIGBAwcIoN1C7gkSubCwYAAAAASUVORK5CYII=";

/** AJ-E415: starts the read-only live preview for the selected desktop
 * Instance (requires the Account's opt-in; one at a time). */
export async function previewStart(sessionId: string): Promise<void> {
  if (!hasBackend) {
    return;
  }
  await invoke("preview_start", { sessionId });
}

/** AJ-E415: stops the live preview (bounded teardown <1 s). */
export async function previewStop(): Promise<void> {
  if (!hasBackend) {
    return;
  }
  await invoke("preview_stop");
}

/** AJ-E415: sets the per-Account live-preview opt-in (the Screen
 * Recording consent moment; off by default). */
export async function accountSetPreviewOptIn(
  accountId: string,
  optIn: boolean,
): Promise<AccountInfo> {
  if (!hasBackend) {
    return {
      ...(MOCK_ACCOUNTS.find((a) => a.id === accountId) ?? MOCK_ACCOUNTS[0]!),
      preview_opt_in: optIn,
    };
  }
  return invoke<AccountInfo>("account_set_preview_opt_in", { accountId, optIn });
}

/** AJ-E415: live frames (filtered to one session by the caller). */
export function onPreviewFrame(
  handler: (frame: PreviewFrameInfo) => void,
): () => void {
  if (!hasBackend) {
    // Harness mode: emit one deterministic frame immediately so the
    // panel (and captures) render a real preview; the phase comes from
    // onPreviewState below.
    handler({
      session_id: "sess-desktop-1",
      png_b64: MOCK_PREVIEW_PNG_B64,
      at_ms: Date.now(),
      seq: 0,
    });
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<PreviewFrameInfo>(EVENTS.previewFrame, (event) => handler(event.payload))
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    })
    .catch(() => {
      /* backend gone; the panel shows its paused state */
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

/** AJ-E415: phase changes (live | paused | denied | unsupported). */
export function onPreviewState(
  handler: (state: PreviewStateInfo) => void,
): () => void {
  if (!hasBackend) {
    const w = window as unknown as { __AIJ_PREVIEW_PHASE__?: PreviewStateInfo };
    const seeded = w.__AIJ_PREVIEW_PHASE__;
    if (seeded) {
      handler(seeded);
    } else {
      handler({
        session_id: "sess-desktop-1",
        phase: "live",
        text: "",
      });
    }
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<PreviewStateInfo>(EVENTS.previewState, (event) => handler(event.payload))
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    })
    .catch(() => {
      /* backend gone */
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

/** Reads one app-level setting (schema v10 kv); null = never set. */
export async function appSettingGet(key: string): Promise<string | null> {
  if (!hasBackend) {
    return key === "go_to_hotkey" ? "Alt+Space" : null;
  }
  return invoke<string | null>("app_setting_get", { key });
}

/** Writes one app-level setting (empty value = reset to default). */
export async function appSettingSet(key: string, value: string): Promise<void> {
  if (!hasBackend) {
    return;
  }
  await invoke("app_setting_set", { key, value });
}

/** Rebinds the global "Go to instance…" hotkey (registers with the OS
 * first; a failure restores the previous binding). */
export async function goToHotkeySet(hotkey: string): Promise<void> {
  if (!hasBackend) {
    return;
  }
  await invoke("go_to_hotkey_set", { hotkey });
}

/** Opens the system Accessibility privacy pane (the explainer's deep
 * link; the target is fixed in Rust — never caller-supplied). */
export async function openAccessibilitySettings(): Promise<void> {
  if (!hasBackend) {
    return;
  }
  await invoke("open_accessibility_settings");
}

/** Fires when the global "Go to instance…" hotkey is pressed anywhere
 * in the system (AJ-E414 Req.4). No-backend: the palette stays
 * reachable in-app via ⌘K. */
export function onGoToHotkey(handler: () => void): () => void {
  if (!hasBackend) {
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen(EVENTS.goTo, () => handler())
    .then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    })
    .catch(() => {
      // backend gone; ⌘K remains
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

/** AJ-E409: D-07 quality signals for the selected Instance (no score). */
export async function instanceQuality(sessionId: string): Promise<QualitySignalsInfo | null> {
  if (!hasBackend) {
    return MOCK_QUALITY[sessionId] ?? null;
  }
  return invoke<QualitySignalsInfo>("instance_quality", { sessionId });
}

/**
 * AJ-E410: the Processes tab payload — live tree rows, the 2 s spark
 * ring, and the confirmed orphans for one Instance.
 */
export async function instanceProcesses(sessionId: string): Promise<ProcessesInfo | null> {
  if (!hasBackend) {
    return MOCK_PROCESSES[sessionId] ?? null;
  }
  return invoke<ProcessesInfo>("instance_processes", { sessionId });
}

/** AJ-E505: one port-inventory row (classification is the key). */
export interface PortRowInfo {
  port: number;
  pid: number;
  class: string;
  session: string | null;
  provider: string | null;
  account: string | null;
  project: string | null;
  listener_age_ms: number | null;
  name: string | null;
  hint: string | null;
  killable: boolean;
}

/** AJ-E505: the inventory response (honest probe note + hog facts). */
export interface PortInventoryInfo {
  rows: PortRowInfo[];
  note: string | null;
  hogs: Array<{
    pid: number;
    session: string;
    provider: string;
    rss_bytes: number;
    cpu_pct: number | null;
    rule: string;
  }>;
}

/** AJ-E505: the pre-launch collision warning (copy-exact message). */
export interface PortWarningInfo {
  port: number;
  pid: number;
  instance: string;
  age_ms: number;
  message: string;
}

/** AJ-E509: one cleanup-audit history row (outcome vocabulary). */
export interface AuditHistoryRow {
  id: number;
  at_ms: number;
  actor: string;
  automatic: boolean;
  pid: number;
  cmdline_trunc: string;
  session_id: string;
  provider: string;
  account_id: string | null;
  machine_id: string;
  outcome: string;
  ports_before: string | null;
  ports_after: string | null;
}

/** AJ-E509: the audit export (JSONL text; first line = redaction note). */
export async function cleanupAuditExport(): Promise<string> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_AUDIT_EXPORT__?: string }).__AIJ_AUDIT_EXPORT__
      ?? JSON.stringify({ note: "cleanup audit export — command lines are hashed and truncated; full cmdlines are never stored" });
  }
  return invoke<string>("cleanup_audit_export");
}

/** AJ-E506: the read-only Cleanup Centre DTOs (zero kill fields). */
export type { CleanupRowInfo, EvidenceChainInfo, VerificationBadgeInfo } from "../components/ProcessRow";
export type { CleanupGroupInfo } from "../screens/CleanupCentre";
import type { CleanupInventoryData } from "../screens/CleanupCentre";

/** AJ-E512: one passive Cleanup Centre alert (hog / idle nudge). */
export interface CleanupAlertData {
  kind: "hog" | "idle" | string;
  text: string;
  pid: number;
  session: string;
}

/** AJ-E512: the passive alert set (hog alerts + idle-orphan nudges). */
export interface CleanupAlertsData {
  alerts: CleanupAlertData[];
}

/** AJ-E512: reads the passive alerts (hog alerts name the top helper;
 * idle nudges fire once per orphan). Purely passive — no alert
 * carries a signal path; the hog alert routes to the staged sheet. */
/** AJ-E609: one cost rollup row (session × model × day). */
export interface CostRollupRow {
  key: string;
  model?: string;
  day?: string;
  tokens: { input: number; output: number; cache_read: number; cache_write: number; reasoning: number };
  usd: { usd?: number; label: string };
  credits?: number;
  confidence: "Measured" | "Computed" | "Estimate";
}

/** AJ-E609: the dual subscription view ("≈$X at API prices · you pay flat"). */
export interface SubscriptionViewData {
  api_equivalent_usd?: number;
  flat_fee_usd?: number;
  value_ratio?: number;
}

/** AJ-E609: the cost matrix payload (the UI and the CSV read the same rows). */
export interface CostRollupsData {
  rows: CostRollupRow[];
  subscription: SubscriptionViewData;
  pricing_stale: boolean;
  pricing_last_synced_ms?: number;
  total_usd?: number;
  scope: string;
}

/** AJ-E611: one ranked (or filtered) account row. */
export interface RankedRowData {
  account: string;
  score?: number;
  filtered_reason?: string;
  top_factors: string[];
  factors: [string, number][];
  headroom_unknown: boolean;
  confidence: string;
  tte_min?: number;
  suggested: boolean;
}

/** AJ-E611: the explainable ranking — suggest-only, the user clicks. */
export async function rankingRows(project?: string): Promise<RankedRowData[]> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_RANKING__?: RankedRowData[] })
      .__AIJ_RANKING__ ?? [];
  }
  return invoke<RankedRowData[]>("ranking_rows", { project });
}

/** AJ-E611: one emitted alert (the notification region's payload).
 * `digest` lines are the hourly batch — visually quieter than window
 * alerts (a renewal item never reads as "switch now"). */
export interface AlertEventData {
  kind: "tray-amber" | "actionable" | "exhausted" | "reset-reached" | "digest";
  account: string;
  text: string;
  at_ms: number;
}

/** AJ-E611: one alert evaluation pass (at-most-once by the machines).
 * The webview's wall-clock local hour rides along — the quiet-hours
 * gate for the digest flush (the backend carries no tz database). */
export async function alertsTick(): Promise<AlertEventData[]> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_ALERTS__?: AlertEventData[] })
      .__AIJ_ALERTS__ ?? [];
  }
  return invoke<AlertEventData[]>("alerts_tick", { localHour: new Date().getHours() });
}

/** AJ-E611: one reset-queue row. */
export interface ResetQueueRow {
  id: string;
  session: string;
  account: string;
  resume_at_ms: number;
  state: string;
}

/** AJ-E611: the queue rows (countdown / ready / stale explained). */
export async function resetQueueList(): Promise<ResetQueueRow[]> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_RESET_QUEUE__?: ResetQueueRow[] })
      .__AIJ_RESET_QUEUE__ ?? [];
  }
  return invoke<ResetQueueRow[]>("reset_queue_list");
}

/** AJ-E611: queue a resume at the reset (passive timer; the resume
 * itself is always an explicit click). */
export async function resetQueueQueue(
  session: string,
  account: string,
  resumeAtMs: number,
): Promise<void> {
  if (!hasBackend) return;
  await invoke("reset_queue_queue", { session, account, resumeAtMs });
}

/** AJ-E611: dismiss a queued resume. */
export async function resetQueueDismiss(id: string): Promise<void> {
  if (!hasBackend) return;
  await invoke("reset_queue_dismiss", { id });
}

/** AJ-E610: one window clock's typed rendered state. */
export interface WindowDisplay {
  state: "unknown" | "learning" | "tracking" | "stale" | "exhausted";
  percentage?: number;
  badge?: string;
  stale_note_ms?: number;
  resets_at_ms?: number;
  burned_tokens: number;
  burned_credits?: number;
}

/** AJ-E610: the subscription clock's typed state (calendar line, D-12). */
export interface RenewalDisplay {
  state: "none" | "set" | "approaching" | "overdue";
  label: string;
  entered_by_user: boolean;
}

/** AJ-E610: the forecast pill data. */
export interface ForecastDisplay {
  state: "learning" | "wont-exhaust" | "exhausts" | "no-headroom";
  tte_min?: number;
  range_min?: number;
  range_max?: number;
  badge: string;
}

/** AJ-E610: the two-clock display for one account. */
export interface QuotaDisplayData {
  account: string;
  five_hour?: WindowDisplay;
  five_hour_omitted_reason?: string;
  weekly?: WindowDisplay;
  weekly_omitted_reason?: string;
  renewal: RenewalDisplay;
  forecast: ForecastDisplay;
  unavailable_reason?: string;
}

/** AJ-E610: one account's two clocks (engine-evaluated). */
export async function quotaDisplay(account: string): Promise<QuotaDisplayData> {
  if (!hasBackend) {
    const all = await quotaDisplayAll();
    return (
      all.find((entry) => entry.account === account) ?? {
        account,
        renewal: { state: "none", label: "Renewal: unknown — add date", entered_by_user: false },
        forecast: { state: "learning", badge: "estimated" },
      }
    );
  }
  return invoke<QuotaDisplayData>("quota_display", { account });
}

/** AJ-E610: every account's two clocks (rail/row source; one query). */
export async function quotaDisplayAll(): Promise<QuotaDisplayData[]> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_QUOTA_DISPLAY__?: QuotaDisplayData[] })
      .__AIJ_QUOTA_DISPLAY__ ?? [];
  }
  return invoke<QuotaDisplayData[]>("quota_display_all");
}

/** AJ-E610: set/clear the user-entered flat fee (dual view ratio). */
export async function accountSetFee(account: string, feeUsd: number | null): Promise<void> {
  if (!hasBackend) {
    const w = window as unknown as { __AIJ_SET_FEE__?: (a: string, f: number | null) => void };
    w.__AIJ_SET_FEE__?.(account, feeUsd);
    return;
  }
  await invoke("account_set_fee", { account, feeUsd });
}

/** AJ-E609: the session cost matrix + dual subscription view. */
export async function costRollups(account?: string): Promise<CostRollupsData> {
  if (!hasBackend) {
    return (
      (window as unknown as { __AIJ_COST_ROLLUPS__?: CostRollupsData })
        .__AIJ_COST_ROLLUPS__ ?? {
        rows: [],
        subscription: {},
        pricing_stale: false,
        scope: account ?? "ALL",
      }
    );
  }
  return invoke<CostRollupsData>("cost_rollups", { account });
}

/** AJ-E609: stream the CSV export; resolves to the written file path. */
export async function costCsvExport(account?: string): Promise<string> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_COST_CSV__?: string }).__AIJ_COST_CSV__ ?? "";
  }
  return invoke<string>("cost_csv_export", { account });
}

export async function cleanupAlerts(): Promise<CleanupAlertsData> {
  if (!hasBackend) {
    return (
      (window as unknown as { __AIJ_CLEANUP_ALERTS__?: CleanupAlertsData })
        .__AIJ_CLEANUP_ALERTS__ ?? { alerts: [] }
    );
  }
  return invoke<CleanupAlertsData>("cleanup_alerts");
}

/** AJ-E508: the automation status (banners + pause + Tier-2 notices). */
export interface CleanupStatusData {
  paused: boolean;
  suspended: boolean;
  suspended_until_ms: number | null;
  tier2_pending: {
    pid: number;
    session: string;
    cmdline_trunc: string;
    deadline_ms: number;
  }[];
}

/** AJ-E508: per-Account tier opt-ins (two separate toggles). */
export interface CleanupTierSettingsData {
  tier1_opt_in: boolean;
  tier2_opt_in: boolean;
}

/** AJ-E510: the one-time recovery summary. */
export interface RecoverySummaryData {
  summary: string;
  adopted: number;
  left_behind: number;
  unverifiable: number;
}

/** Reads the unconsumed recovery summary, if pending (shown once). */
export async function recoverySummaryGet(): Promise<RecoverySummaryData | null> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_RECOVERY__?: RecoverySummaryData }).__AIJ_RECOVERY__
      ?? null;
  }
  return invoke<RecoverySummaryData | null>("recovery_summary_get");
}

/** Dismisses the recovery summary (the only way it goes away). */
export async function recoverySummaryDismiss(): Promise<void> {
  if (!hasBackend) {
    return Promise.resolve();
  }
  return invoke("recovery_summary_dismiss");
}

/** Reads the automation status (Tier 0's fallback — never signals). */
export async function cleanupStatus(): Promise<CleanupStatusData> {
  if (!hasBackend) {
    return (
      (window as unknown as { __AIJ_CLEANUP_STATUS__?: CleanupStatusData }).__AIJ_CLEANUP_STATUS__
      ?? { paused: false, suspended: false, suspended_until_ms: null, tier2_pending: [] }
    );
  }
  return invoke<CleanupStatusData>("cleanup_status");
}

/** Sets the global `Pause automatic cleanup` kill-switch. */
export async function cleanupPauseSet(paused: boolean): Promise<CleanupStatusData> {
  if (!hasBackend) {
    return {
      paused,
      suspended: false,
      suspended_until_ms: null,
      tier2_pending: [],
    };
  }
  return invoke<CleanupStatusData>("cleanup_pause_set", { paused });
}

/** The Tier-2 notice decision: `keep` = Undo / Keep running. */
export async function cleanupTier2Decide(
  pid: number,
  keep: boolean,
): Promise<CleanupStatusData> {
  if (!hasBackend) {
    return { paused: false, suspended: false, suspended_until_ms: null, tier2_pending: [] };
  }
  return invoke<CleanupStatusData>("cleanup_tier2_decide", { pid, keep });
}

/** Re-confirms automation after the 24 h crash suspension. */
export async function cleanupResumeAfterCrash(): Promise<CleanupStatusData> {
  if (!hasBackend) {
    return { paused: false, suspended: false, suspended_until_ms: null, tier2_pending: [] };
  }
  return invoke<CleanupStatusData>("cleanup_resume_after_crash");
}

/** Reads one Account's two tier opt-ins (absent rows = both off). */
export async function cleanupTiersGet(accountId: string): Promise<CleanupTierSettingsData> {
  if (!hasBackend) {
    return (
      (window as unknown as { __AIJ_CLEANUP_TIERS__?: CleanupTierSettingsData })
        .__AIJ_CLEANUP_TIERS__ ?? { tier1_opt_in: false, tier2_opt_in: false }
    );
  }
  return invoke<CleanupTierSettingsData>("cleanup_tiers_get", { accountId });
}

/** Sets one Account's two SEPARATE toggles. */
export async function cleanupTiersSet(
  accountId: string,
  tier1OptIn: boolean,
  tier2OptIn: boolean,
): Promise<CleanupTierSettingsData> {
  if (!hasBackend) {
    return { tier1_opt_in: tier1OptIn, tier2_opt_in: tier2OptIn };
  }
  return invoke<CleanupTierSettingsData>("cleanup_tiers_set", {
    accountId,
    tier1OptIn,
    tier2OptIn,
  });
}
import type {
  KillExecuteData,
  KillScope,
  KillVerifyData,
} from "../components/KillSheet";

/** AJ-E506: the read-only inventory (Tier 0). */
export async function cleanupInventory(
  filter: string,
  machineScope: "this" | "all" = "this",
): Promise<CleanupInventoryData> {
  if (!hasBackend) {
    const seeded = (window as unknown as { __AIJ_CLEANUP__?: CleanupInventoryData })
      .__AIJ_CLEANUP__;
    if (seeded === undefined) {
      return {
        groups: [],
        clean_instances: 0,
        local_machine: "",
        ledger_degraded: false,
        history: null,
      };
    }
    // Mirror the backend contract for the seam (AJ-E511): the
    // default scope lists THIS machine's rows only; "all" widens.
    if (machineScope !== "all" && seeded.local_machine !== "") {
      const local = seeded.local_machine;
      const groups = seeded.groups
        .map((group) => ({
          ...group,
          rows: group.rows.filter(
            (row) => row.machine_id === undefined || row.machine_id === local,
          ),
        }))
        .filter((group) => group.rows.length > 0);
      return { ...seeded, groups };
    }
    return seeded;
  }
  return invoke<CleanupInventoryData>("cleanup_inventory", { filter, machineScope });
}

/** AJ-E507: the kill sheet's step-1 verify (zero signalling). */
export async function killVerify(sessionId: string, pid: number): Promise<KillVerifyData> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_KILL_VERIFY__?: (session: string, pid: number) => KillVerifyData })
      .__AIJ_KILL_VERIFY__?.(sessionId, pid) ?? {
      pid,
      session: sessionId,
      verified: false,
      refusal: "This PID now belongs to a different program — Juggler will not touch it.",
      exe_path: "?",
      cmdline: "",
      cwd: "",
      ports: [],
      pgid: null,
      escaped_group: false,
      members: [],
      tree_pids: [],
      platform: "TERM → grace → KILL",
      grace_secs: 5,
      provider: "unknown",
      account_id: null,
      project_id: null,
    };
  }
  return invoke<KillVerifyData>("kill_verify", { sessionId, pid });
}

/** AJ-E507: the audited TERM→grace→KILL ladder (step 3). */
export async function killExecute(
  sessionId: string,
  pid: number,
  scope: KillScope,
  attempt: string,
  goalPort: number | null,
): Promise<KillExecuteData> {
  if (!hasBackend) {
    const fixture = (
      window as unknown as {
        __AIJ_KILL_EXECUTE__?: (scope: KillScope, goalPort: number | null) => KillExecuteData;
      }
    ).__AIJ_KILL_EXECUTE__;
    if (fixture !== undefined) {
      return fixture(scope, goalPort);
    }
    // The countdown state's seam: a ladder that never resolves.
    if ((window as unknown as { __AIJ_KILL_EXECUTE_PENDING__?: boolean }).__AIJ_KILL_EXECUTE_PENDING__) {
      return new Promise<KillExecuteData>(() => {});
    }
    return {
      refusal: null,
      members: [{ pid, outcome: "already gone" }],
      cancelled: false,
      all_succeeded: true,
      still_holding: [],
      reoffer_pid: null,
    };
  }
  return invoke<KillExecuteData>("kill_execute", { sessionId, pid, scope, attempt, goalPort });
}

/** AJ-E507: cancel a running ladder's grace window. */
export async function killCancel(attempt: string): Promise<void> {
  if (!hasBackend) {
    return Promise.resolve();
  }
  return invoke("kill_cancel", { attempt });
}

/** AJ-E507: the grace window's per-process exit ticks. */
export async function killLiveness(pids: number[]): Promise<boolean[]> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_KILL_LIVENESS__?: (pids: number[]) => boolean[] })
      .__AIJ_KILL_LIVENESS__?.(pids) ?? pids.map(() => false);
  }
  return invoke<boolean[]>("kill_liveness", { pids });
}

/** AJ-E505: the port occupancy map. */
export async function portInventory(): Promise<PortInventoryInfo> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_PORTS__?: PortInventoryInfo }).__AIJ_PORTS__
      ?? { rows: [], note: null, hogs: [] };
  }
  return invoke<PortInventoryInfo>("port_inventory");
}

/** AJ-E505: the launch pre-check — a warning NEVER blocks launch. */
export async function launchPortWarning(projectId: string | null): Promise<PortWarningInfo | null> {
  if (!hasBackend) {
    return (window as unknown as { __AIJ_PORT_WARNING__?: PortWarningInfo }).__AIJ_PORT_WARNING__
      ?? null;
  }
  return invoke<PortWarningInfo | null>("launch_port_warning", { projectId });
}

/** AJ-E410: stop one orphan (allow-list guarded, TERM→KILL ladder). */
export async function orphanStop(sessionId: string, pid: number): Promise<StopOutcomeInfo> {
  if (!hasBackend) {
    // Mock: the allow-list guard refuses anything but fixture orphans.
    const orphan = MOCK_PROCESSES[sessionId]?.orphans.find((row) => row.pid === pid);
    const state = orphan
      ? orphan.stop_state ?? "gone"
      : "refused - not a recorded descendant";
    return { pid, state };
  }
  return invoke<StopOutcomeInfo>("orphan_stop", { sessionId, pid });
}

/** AJ-E410: stop every confirmed orphan of one Instance. */
export async function orphanStopAll(sessionId: string): Promise<StopOutcomeInfo[]> {
  if (!hasBackend) {
    const orphans = MOCK_PROCESSES[sessionId]?.orphans ?? [];
    return orphans.map((orphan) => ({ pid: orphan.pid, state: orphan.stop_state ?? "gone" }));
  }
  return invoke<StopOutcomeInfo[]>("orphan_stop_all", { sessionId });
}

/** AJ-E410: re-check the ppid chain for one orphan pid. */
export async function orphanVerify(sessionId: string, pid: number): Promise<OrphanVerifyInfo> {
  if (!hasBackend) {
    const orphan = MOCK_PROCESSES[sessionId]?.orphans.find((row) => row.pid === pid);
    if (orphan) {
      return {
        pid,
        alive: !orphan.stop_state,
        parent_pid: 1,
        note: "alive — re-parented to init/launchd (its parent exited)",
      };
    }
    return { pid, alive: false, parent_pid: null, note: "process is gone" };
  }
  return invoke<OrphanVerifyInfo>("orphan_verify", { sessionId, pid });
}

/** AJ-E411 (D-12): set (validate server-side) or clear the user-entered renewal date. */
export async function accountSetRenewal(
  accountId: string,
  date: string | null,
): Promise<AccountInfo> {
  if (!hasBackend) {
    const account = MOCK_ACCOUNTS.find((row) => row.id === accountId);
    if (!account) throw new Error(`unknown account \${accountId}`);
    return { ...account, renewal_date: date };
  }
  return invoke<AccountInfo>("account_set_renewal", { accountId, date });
}

/** AJ-E410 (D-08): set the per-Account Tier-1 cleanup opt-in. */
export async function accountSetCleanup(accountId: string, optIn: boolean): Promise<AccountInfo> {
  if (!hasBackend) {
    const account = MOCK_ACCOUNTS.find((row) => row.id === accountId);
    if (!account) throw new Error(`unknown account ${accountId}`);
    return { ...account, cleanup_opt_in: optIn };
  }
  return invoke<AccountInfo>("account_set_cleanup", { accountId, optIn });
}

export async function stopSession(sessionId: string): Promise<void> {
  if (!hasBackend) {
    return;
  }
  await invoke("stop_session", { sessionId });
}

export async function dashboardStats(): Promise<StatsInfo> {
  if (!hasBackend) {
    return MOCK_STATS;
  }
  return invoke<StatsInfo>("dashboard_stats");
}

export function onSessionsUpdated(
  handler: (sessions: InstanceInfo[]) => void,
): () => void {
  if (!hasBackend) {
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<InstanceInfo[]>(EVENTS.sessionsUpdated, (event) => handler(event.payload))
    .then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    })
    .catch(() => {
      // backend gone; callers poll via sessionsList
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

/**
 * 5 s resource tick (AJ-E401): coalesced I-07/I-08 deltas for live
 * Instances — merge by `id` into the session rows; absent ids keep
 * their honest `null`s.
 */
export function onSessionsResources(
  handler: (deltas: InstanceResourcesInfo[]) => void,
): () => void {
  if (!hasBackend) {
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<InstanceResourcesInfo[]>(EVENTS.sessionsResources, (event) =>
    handler(event.payload),
  )
    .then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    })
    .catch(() => {
      // backend gone; the row payload still carries resource fields
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

/**
 * 30 s quota tick (AJ-E401): coalesced I-05 deltas — merge by `id`;
 * `quota: null` is the honest "no feed exists for this Provider".
 */
export function onSessionsQuota(
  handler: (deltas: InstanceQuotaInfo[]) => void,
): () => void {
  if (!hasBackend) {
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<InstanceQuotaInfo[]>(EVENTS.sessionsQuota, (event) =>
    handler(event.payload),
  )
    .then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    })
    .catch(() => {
      // backend gone; the row payload still carries the quota field
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

export function onAppReady(handler: (info: AppInfo) => void): () => void {
  if (!hasBackend) {
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<AppInfo>(EVENTS.appReady, (event) => handler(event.payload))
    .then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    })
    .catch(() => {});
  return () => {
    disposed = true;
    unlisten?.();
  };
}

// ---- embedded terminal (M1-4) --------------------------------------------

export const TERMINAL_EVENTS = {
  output: "terminal://output",
  exit: "terminal://exit",
} as const;

/** Spawns the guided login for an account inside an embedded terminal. */
export async function terminalSpawnLogin(accountId: string): Promise<TerminalInfo> {
  return invoke<TerminalInfo>("terminal_spawn_login", { accountId });
}

/** Starts streaming a supervised session into the dock (idempotent). */
export async function terminalAttach(id: string): Promise<TerminalInfo> {
  return invoke<TerminalInfo>("terminal_attach", { id });
}

/** Sends keystrokes to a terminal. */
export async function terminalWrite(id: string, data: string): Promise<void> {
  return invoke<void>("terminal_write", { id, data });
}

/** Resizes a terminal's PTY. */
export async function terminalResize(
  id: string,
  cols: number,
  rows: number,
): Promise<void> {
  return invoke<void>("terminal_resize", { id, cols, rows });
}

/** Stops a terminal's process (graceful ladder). */
export async function terminalKill(id: string): Promise<void> {
  return invoke<void>("terminal_kill", { id });
}

/** The retained output tail (base64) for replay on panel reopen. */
export async function terminalSnapshot(id: string): Promise<string> {
  return invoke<string>("terminal_snapshot", { id });
}

/** Live output stream for one terminal (base64 chunks). */
export function onTerminalOutput(
  id: string,
  handler: (base64: string) => void,
): () => void {
  return listenScoped(TERMINAL_EVENTS.output, (payload: { id: string; data: string }) => {
    if (payload.id === id) handler(payload.data);
  });
}

/** A terminal's process exited. */
export function onTerminalExit(handler: (id: string) => void): () => void {
  return listenScoped(TERMINAL_EVENTS.exit, (payload: { id: string }) =>
    handler(payload.id),
  );
}

function listenScoped<T>(
  event: string,
  handler: (payload: T) => void,
): () => void {
  if (!hasBackend) {
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<T>(event, (event_) => handler(event_.payload)).then((stop) => {
    if (disposed) {
      stop();
    } else {
      unlisten = stop;
    }
  });
  return () => {
    disposed = true;
    unlisten?.();
  };
}


// ---- golden path: backfill, handover, directory picking -------------------

/** Backfills the projects rail from imported transcript history (idempotent). */
export async function projectsBackfill(): Promise<number> {
  if (!hasBackend) return 0;
  return invoke<number>("projects_backfill");
}

/** The juggle: move a session's context to another account and resume it. */
export async function sessionHandover(
  sessionId: string,
  targetProfileId: string,
): Promise<LaunchResult> {
  return invoke<LaunchResult>("session_handover", { sessionId, targetProfileId });
}

/** Native macOS folder picker; null when cancelled. */
export async function pickDirectory(): Promise<string | null> {
  if (!hasBackend) return null;
  const selection = await openDialog({ directory: true, multiple: false });
  return typeof selection === "string" ? selection : null;
}

/** Folders dragged onto the window (Tauri drag-drop events). */
export function onFolderDropped(handler: (path: string) => void): () => void {
  if (!hasBackend) return () => {};
  let disposed = false;
  let unlisten: (() => void) | null = null;
  import("@tauri-apps/api/webview")
    .then(({ getCurrentWebview }) =>
      getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          for (const path of event.payload.paths) handler(path);
        }
      }),
    )
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}
