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
import type { LaunchResult } from "../types/generated/LaunchResult";
import type { NeedsAttention } from "../types/generated/NeedsAttention";
import type { ProjectInfo } from "../types/generated/ProjectInfo";
import type { ProviderInfo } from "../types/generated/ProviderInfo";
import type { QuotaSnapshotInfo } from "../types/generated/QuotaSnapshotInfo";
import type { StatsInfo } from "../types/generated/StatsInfo";
import type { TerminalInfo } from "../types/generated/TerminalInfo";
import type { TokenCountsInfo } from "../types/generated/TokenCountsInfo";

export type {
  AppInfo,
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

const MOCK_ACCOUNTS: AccountInfo[] = [
  { id: "claude-work", provider: "claude-code", label: "Work Max", plan: Some("max-20x"), login_state: "logged_in" },
  { id: "claude-personal", provider: "claude-code", label: "Personal", plan: None(), login_state: "logged_out" },
];

const MOCK_PROFILES: LaunchRecipeInfo[] = [
  { id: "p-1", name: "Deep work", account_id: "claude-work", account_label: "Work Max", provider: "claude-code", model: Some("opus"), default_cwd: None() },
  { id: "p-2", name: "Quick fixes", account_id: "claude-work", account_label: "Work Max", provider: "claude-code", model: Some("sonnet"), default_cwd: None() },
  { id: "p-3", name: "Weekend build", account_id: "claude-personal", account_label: "Personal", provider: "claude-code", model: None(), default_cwd: None() },
];

const MOCK_STATS: StatsInfo =
  seeded.stats ?? { accounts: 2, profiles: 3, active_sessions: 0 };

// Mock rows mirror the AJ-E401 read-model fixtures: one full row, one
// quota-less row, one model-less/terminal row — nulls where the backend
// reports absence, never zeros or guessed names.
const MOCK_SESSIONS: InstanceInfo[] = seeded.sessions ?? [
  {
    id: "sess-live-1",
    launch_recipe_id: "p-1",
    provider: "claude-code",
    account_id: "claude-work",
    account_label: "Work Max",
    model: Some("opus"),
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
    started_at_ms: Date.now() - 1_200_000,
    last_activity_ms: Date.now() - 30_000,
    machine_id: "this-mac",
    kind: "terminal",
    project_id: "proj-ai-juggler",
    conversation_id: "822e4125-e07d-4866-9954-ec9295efb079",
  },
  {
    id: "sess-done-1",
    launch_recipe_id: "p-3",
    provider: "claude-code",
    account_id: "claude-personal",
    account_label: "Personal",
    model: null,
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
    started_at_ms: Date.now() - 86_400_000,
    last_activity_ms: Date.now() - 80_000_000,
    machine_id: "this-mac",
    kind: "terminal",
    project_id: "proj-payments",
    conversation_id: null,
  },
  {
    id: "sess-done-2",
    launch_recipe_id: "p-2",
    provider: "claude-code",
    account_id: "claude-work",
    account_label: "Work Max",
    model: Some("sonnet"),
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
    started_at_ms: Date.now() - 43_200_000,
    last_activity_ms: Date.now() - 600_000,
    machine_id: "this-mac",
    kind: "terminal",
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
  },
  {
    id: "proj-payments",
    root: "/Users/you/Dev/payments-api",
    name: "payments-api",
    last_activity_ms: Date.now() - 3_600_000,
    active_instance: null,
    accounts_used: ["Personal", "Work Max"],
    instance_count: 2,
    history_sessions: 14,
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
