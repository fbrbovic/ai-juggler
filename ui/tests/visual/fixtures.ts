import type { FixtureSet, InstanceInfo } from "../../src/lib/bridge";

/**
 * Deterministic fixture sets for the visual harness (AJ-E802 Req.1):
 * every SessionStatus value, quota honesty tiers where the current
 * read-model carries them, and honest-empty variants.
 */
const baseAccounts = [
  { id: "claude-work", provider: "claude-code", label: "Work Max", plan: "max-20x", login_state: "logged_in" },
  { id: "claude-personal", provider: "claude-code", label: "Personal", plan: null, login_state: "logged_in" },
];

const baseProfiles = [
  { id: "p-1", name: "Main", account_id: "claude-work", account_label: "Work Max", provider: "claude-code", model: "opus", default_cwd: null },
  { id: "p-2", name: "Main", account_id: "claude-personal", account_label: "Personal", provider: "claude-code", model: null, default_cwd: null },
];

// One session per SessionStatus (model.rs:188 + Stopped). Rows carry the
// full AJ-E401 read model: nulls where a real row would have no source
// (never zeros), quota/token/resource data only where measured.
const T0 = 1_728_800_000_000; // pinned clock base (deterministic)
const mk = (n: number, status: InstanceInfo["status"], label: string): InstanceInfo => {
  const work = label === "Work Max";
  const active = !["finished", "stopped", "failed", "handed_over"].includes(status);
  const attention: InstanceInfo["needs_attention"] =
    status === "waiting_user"
      ? "waiting_user"
      : status === "stuck"
        ? "stuck"
        : status === "quota_blocked"
          ? "quota_blocked"
          : status === "failed"
            ? "failed"
            : null;
  const quota: InstanceInfo["quota"] =
    status === "running"
      ? { used_percentage: 42.5, resets_at_ms: T0 + 3_600_000, confidence: "computed" }
      : status === "quota_blocked"
        ? { used_percentage: 100, resets_at_ms: T0 + 7_200_000, confidence: "measured" }
        : null;
  return {
    id: `sess-${n}`,
    launch_recipe_id: work ? "p-1" : "p-2",
    provider: "claude-code",
    account_id: work ? "claude-work" : "claude-personal",
    account_label: label,
    model: work ? "opus" : null,
    status,
    quota,
    tokens:
      status === "running"
        ? { input: 12_345, output: 6_789, cache_read: 100, cache_write: 200, reasoning: 42 }
        : null,
    cpu_pct: status === "running" ? 3.5 : null,
    rss_bytes: status === "running" ? 123_456 : null,
    child_count: active ? (status === "running" ? 2 : null) : 0,
    subtask_count: null,
    git_branch: work ? "feat/read-model" : null,
    git_dirty_count: null,
    needs_attention: attention,
    attention:
      attention === null
        ? null
        : {
            rule: attention === "waiting_user" ? "waiting_prompt" : attention === "stuck" ? "stuck_no_output" : "quota_exit",
            inputs:
              attention === "waiting_user"
                ? ["prompt alive", `no output ${n}m`]
                : attention === "stuck"
                  ? [`no output ${n}m`, "cpu 0.3%", `transcript stale ${n}m`]
                  : ["exit code 1", "rate-limit signature in output"],
            since_ms: T0 - n * 3_500_000,
          },
    started_at_ms: T0 - n * 3_600_000,
    last_activity_ms: T0 - n * 3_500_000,
    machine_id: "this-mac",
    kind: "terminal",
    project_id: work ? "proj-1" : "proj-2",
    conversation_id: status === "running" ? "822e4125-e07d-4866-9954-ec9295efb079" : null,
  };
};

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
    { id: "a1", provider: "claude-code", label: "Work Max", plan: "max-20x", login_state: "logged_out" },
    { id: "a2", provider: "claude-code", label: "Personal", plan: null, login_state: "logged_in" },
  ],
  profiles: [],
  sessions: [],
  projects: [],
  stats: { accounts: 2, profiles: 0, active_sessions: 0 },
};
