/**
 * Typed bridge to the Rust backend. Types are generated from the Rust
 * definitions (ts-rs export) — never hand-written on this side.
 *
 * In a plain browser (vite dev outside Tauri) every call degrades to
 * design-preview mock data so screens can be reviewed without a backend.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppInfo } from "../types/generated/AppInfo";
import type { AccountInfo } from "../types/generated/AccountInfo";
import type { DiscoveredInfo } from "../types/generated/DiscoveredInfo";
import type { ImportResult } from "../types/generated/ImportResult";
import type { LaunchResult } from "../types/generated/LaunchResult";
import type { ProjectInfo } from "../types/generated/ProjectInfo";
import type { ProfileInfo } from "../types/generated/ProfileInfo";
import type { SessionInfo } from "../types/generated/SessionInfo";
import type { HarnessInfo } from "../types/generated/HarnessInfo";
import type { StatsInfo } from "../types/generated/StatsInfo";
import type { TerminalInfo } from "../types/generated/TerminalInfo";

export type {
  AppInfo,
  AccountInfo,
  DiscoveredInfo,
  ImportResult,
  LaunchResult,
  ProfileInfo,
  ProjectInfo,
  SessionInfo,
  HarnessInfo,
  StatsInfo,
  TerminalInfo,
};

/** Event names mirrored from juggler_app constants (single source: Rust). */
export const EVENTS = {
  appReady: "app://ready",
  sessionsUpdated: "session://updated",
} as const;

const hasBackend =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function hasLiveBackend(): boolean {
  return hasBackend;
}

// --- design-preview mocks (plain-browser mode only) -----------------------

const MOCK_ACCOUNTS: AccountInfo[] = [
  { id: "claude-work", harness: "claude-code", label: "Work Max", plan: Some("max-20x"), login_state: "logged_in" },
  { id: "claude-personal", harness: "claude-code", label: "Personal", plan: None(), login_state: "logged_out" },
];

const MOCK_PROFILES: ProfileInfo[] = [
  { id: "p-1", name: "Deep work", account_id: "claude-work", account_label: "Work Max", harness: "claude-code", model: Some("opus"), default_cwd: None() },
  { id: "p-2", name: "Quick fixes", account_id: "claude-work", account_label: "Work Max", harness: "claude-code", model: Some("sonnet"), default_cwd: None() },
  { id: "p-3", name: "Weekend build", account_id: "claude-personal", account_label: "Personal", harness: "claude-code", model: None(), default_cwd: None() },
];

const MOCK_STATS: StatsInfo = { accounts: 2, profiles: 3, active_sessions: 0 };

const MOCK_SESSIONS: SessionInfo[] = [
  {
    id: "sess-live-1",
    profile_id: "p-1",
    harness: "claude-code",
    label: "Work Max",
    status: "running",
    started_at_ms: Date.now() - 1_200_000,
    kind: "terminal",
    project_id: "proj-ai-juggler",
  },
  {
    id: "sess-done-1",
    profile_id: "p-3",
    harness: "claude-code",
    label: "Personal",
    status: "finished",
    started_at_ms: Date.now() - 86_400_000,
    kind: "terminal",
    project_id: "proj-payments",
  },
  {
    id: "sess-done-2",
    profile_id: "p-2",
    harness: "claude-code",
    label: "Work Max",
    status: "finished",
    started_at_ms: Date.now() - 43_200_000,
    kind: "terminal",
    project_id: "proj-payments",
  },
];

const MOCK_PROJECTS: ProjectInfo[] = [
  {
    id: "proj-ai-juggler",
    root: "/Users/you/Dev/ai-juggler",
    name: "ai-juggler",
    last_activity_ms: Date.now() - 60_000,
    active_instance: MOCK_SESSIONS[0],
    accounts_used: ["Work Max"],
    instance_count: 1,
  },
  {
    id: "proj-payments",
    root: "/Users/you/Dev/payments-api",
    name: "payments-api",
    last_activity_ms: Date.now() - 3_600_000,
    active_instance: null,
    accounts_used: ["Personal", "Work Max"],
    instance_count: 2,
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

export async function harnessesList(): Promise<HarnessInfo[]> {
  if (!hasBackend) {
    return [{ id: "claude-code", name: "Claude Code" }];
  }
  return invoke<HarnessInfo[]>("harnesses_list");
}

export async function accountsList(): Promise<AccountInfo[]> {
  if (!hasBackend) {
    return MOCK_ACCOUNTS;
  }
  return invoke<AccountInfo[]>("accounts_list");
}

export async function createAccount(
  harness: string,
  label: string,
  plan: string | null,
): Promise<AccountInfo> {
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

export async function profilesList(): Promise<ProfileInfo[]> {
  if (!hasBackend) {
    return MOCK_PROFILES;
  }
  return invoke<ProfileInfo[]>("profiles_list");
}

export async function createProfile(
  accountId: string,
  name: string,
  model: string | null,
  defaultCwd: string | null,
): Promise<ProfileInfo> {
  return invoke<ProfileInfo>("profile_create", {
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

export async function sessionsList(): Promise<SessionInfo[]> {
  if (!hasBackend) {
    return MOCK_SESSIONS;
  }
  return invoke<SessionInfo[]>("sessions_list");
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
  handler: (sessions: SessionInfo[]) => void,
): () => void {
  if (!hasBackend) {
    return () => {};
  }
  let unlisten: (() => void) | null = null;
  let disposed = false;
  listen<SessionInfo[]>(EVENTS.sessionsUpdated, (event) => handler(event.payload))
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
