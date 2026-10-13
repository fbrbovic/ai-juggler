import type { FixtureSet } from "../../src/lib/bridge";

/**
 * Deterministic fixture sets for the visual harness (AJ-E802 Req.1):
 * every SessionStatus value, quota honesty tiers where the current
 * read-model carries them, and honest-empty variants.
 */
const baseAccounts = [
  { id: "claude-work", harness: "claude-code", label: "Work Max", plan: "max-20x", login_state: "logged_in" },
  { id: "claude-personal", harness: "claude-code", label: "Personal", plan: null, login_state: "logged_in" },
];

const baseProfiles = [
  { id: "p-1", name: "Main", account_id: "claude-work", account_label: "Work Max", harness: "claude-code", model: "opus", default_cwd: null },
  { id: "p-2", name: "Main", account_id: "claude-personal", account_label: "Personal", harness: "claude-code", model: null, default_cwd: null },
];

// One session per SessionStatus (model.rs:188 + Stopped).
const T0 = 1_728_800_000_000; // pinned clock base (deterministic)
const mk = (n: number, status: string, label: string) => ({
  id: `sess-${n}`,
  profile_id: n % 2 ? "p-1" : "p-2",
  harness: "claude-code",
  label,
  status,
  started_at_ms: T0 - n * 3_600_000,
  kind: "terminal",
  project_id: n % 2 ? "proj-1" : "proj-2",
});

export const allStatuses: FixtureSet = {
  accounts: baseAccounts,
  profiles: baseProfiles,
  sessions: [
    mk(1, "starting", "Work Max"),
    mk(2, "running", "Work Max"),
    mk(3, "waiting_user", "Personal"),
    mk(4, "stuck", "Work Max"),
    mk(5, "quota_blocked", "Personal"),
    mk(6, "finished", "Work Max"),
    mk(7, "stopped", "Personal"),
    mk(8, "failed", "Work Max"),
    mk(9, "handed_over", "Personal"),
  ],
  projects: [
    { id: "proj-1", root: "/Users/dev/ai-juggler", name: "ai-juggler", last_activity_ms: T0 - 60_000, active_instance: null, accounts_used: ["Work Max"], instance_count: 5, history_sessions: 12 },
    { id: "proj-2", root: "/Users/dev/booknyx", name: "booknyx", last_activity_ms: T0 - 3_600_000, active_instance: null, accounts_used: ["Personal"], instance_count: 4, history_sessions: 7 },
  ],
  stats: { accounts: 2, profiles: 2, active_sessions: 2 },
};

export const honestEmpty: FixtureSet = {
  accounts: [],
  profiles: [],
  sessions: [],
  projects: [],
  stats: { accounts: 0, profiles: 0, active_sessions: 0 },
};

export const readyHome: FixtureSet = {
  accounts: [
    { id: "a1", harness: "claude-code", label: "Work Max", plan: "max-20x", login_state: "logged_out" },
    { id: "a2", harness: "claude-code", label: "Personal", plan: null, login_state: "logged_in" },
  ],
  profiles: [],
  sessions: [],
  projects: [],
  stats: { accounts: 2, profiles: 0, active_sessions: 0 },
};
