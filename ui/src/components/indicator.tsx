import type { ReactNode } from "react";
import type { InstanceInfo } from "../lib/bridge";
import {
  IconFork,
  IconGitBranch,
  IconProviderClaude,
  IconProviderCodex,
  IconProviderGeneric,
  IconProviderZCode,
  IconStateFailed,
  IconStateFinished,
  IconStateHandover,
  IconStateQuota,
  IconStateRunning,
  IconStateStarting,
  IconStateStopped,
  IconStateStuck,
  IconStateUnknown,
  IconStateWaiting,
  IconSubtasks,
} from "./icons";

/**
 * I-01..I-11 display mapping for the Instance indicator row and card
 * (AJ-E402). One source for both variants — no number may exist on a
 * card the rail cannot explain. Absence renders exactly: `Quota: no
 * data`, `default model`, `Unknown`, hidden children at 0, omitted
 * branch for non-git cwds, `—` for unsampled resources. Never a
 * fabricated `0`, `$0.00`, or guessed model.
 */

// --- I-01 Provider marks ---------------------------------------------------

export const PROVIDER_MARKS: Record<
  string,
  { abbrev: string; Icon: typeof IconProviderClaude }
> = {
  "claude-code": { abbrev: "Claude", Icon: IconProviderClaude },
  codex: { abbrev: "Codex", Icon: IconProviderCodex },
  zcode: { abbrev: "ZCode", Icon: IconProviderZCode },
};

/** Unknown Provider degrades to the generic `Tool` mark — never blank. */
export function providerMark(provider: string) {
  return PROVIDER_MARKS[provider] ?? { abbrev: "Tool", Icon: IconProviderGeneric };
}

// --- I-04 state chips (icon + text + colour, never colour-only) ------------

export type StateChipTone = "neutral" | "success" | "warning" | "danger" | "live" | "accent";

export function stateChip(status: string): {
  label: string;
  Icon: typeof IconStateRunning;
  tone: StateChipTone;
} {
  switch (status) {
    case "starting":
      return { label: "Starting", Icon: IconStateStarting, tone: "accent" };
    case "running":
      return { label: "Running", Icon: IconStateRunning, tone: "live" };
    case "waiting_user":
      return { label: "Needs you", Icon: IconStateWaiting, tone: "warning" };
    case "stuck":
      return { label: "Stuck?", Icon: IconStateStuck, tone: "warning" };
    case "quota_blocked":
      return { label: "Quota blocked", Icon: IconStateQuota, tone: "danger" };
    case "finished":
      return { label: "Finished", Icon: IconStateFinished, tone: "success" };
    case "stopped":
      return { label: "Stopped", Icon: IconStateStopped, tone: "neutral" };
    case "failed":
      return { label: "Failed", Icon: IconStateFailed, tone: "danger" };
    case "handed_over":
      return { label: "Handed over", Icon: IconStateHandover, tone: "accent" };
    // Unknown wire value → `Unknown` chip, never a neutral dot.
    default:
      return { label: "Unknown", Icon: IconStateUnknown, tone: "neutral" };
  }
}

// --- I-02 account identity ---------------------------------------------------

/** Initials for the account disc: first letters of the first two words. */
export function initialsOf(label: string): string {
  const words = label.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return words[0]!.slice(0, 2).toUpperCase();
  return (words[0]![0]! + words[1]![0]!).toUpperCase();
}

/** Stable per-Account hue (0–359) from a hash of the account id. */
export function hueOf(accountId: string | null): number {
  const key = accountId ?? "unknown";
  let hash = 0;
  for (let i = 0; i < key.length; i += 1) {
    hash = (hash * 31 + key.charCodeAt(i)) % 100_000;
  }
  return hash % 360;
}

// --- I-06 tokens (no pricing source exists: tokens only, never `$0.00`) ----

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

/** `18.2k tok` from input+output; `null` when usage was never observed. */
export function tokenSummary(
  tokens: InstanceInfo["tokens"],
): string | null {
  if (!tokens) return null;
  return `${formatTokens(tokens.input + tokens.output)} tok`;
}

// --- I-10 / I-11 time formatting ---------------------------------------------

/** Compact duration: `10s`, `4m`, `2h05m`, `1d3h`. */
export function formatDuration(ms: number): string {
  if (ms < 0) return "0s";
  const seconds = Math.round(ms / 1_000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h${String(minutes % 60).padStart(2, "0")}m`;
  return `${Math.floor(hours / 24)}d${hours % 24}h`;
}

/** `17:00`-style wall clock for reset countdowns (local time). */
export function formatClock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}

/**
 * I-11 age + last-activity line: `42m · active 10s ago`; just-started →
 * `starting…`; never-active → `no output yet`.
 */
export function ageSummary(
  startedAtMs: number,
  lastActivityMs: number | null,
  nowMs: number = Date.now(),
): string {
  if (nowMs - startedAtMs < 5_000) return "starting…";
  const age = formatDuration(nowMs - startedAtMs);
  if (lastActivityMs === null) return `${age} · no output yet`;
  return `${age} · active ${formatDuration(Math.max(0, nowMs - lastActivityMs))} ago`;
}

/** I-10 waiting duration from last activity (`wait 4m`). */
export function waitingFor(
  lastActivityMs: number | null,
  nowMs: number = Date.now(),
): string | null {
  if (lastActivityMs === null) return null;
  return `wait ${formatDuration(Math.max(0, nowMs - lastActivityMs))}`;
}

/**
 * I-10 needs-attention badge. For the attention states the badge IS the
 * line-1 state indicator (icon + text + colour — the chip contract still
 * holds); `null` renders the plain I-04 chip instead. Exact copy:
 * `Needs you`, `Stuck?`, `Quota blocked`.
 */
export function attentionBadge(
  session: InstanceInfo,
): { text: string; tone: StateChipTone } | null {
  switch (session.needs_attention) {
    case "waiting_user":
      return { text: "Needs you", tone: "warning" };
    case "stuck":
      return { text: "Stuck?", tone: "warning" };
    case "quota_blocked":
      return { text: "Quota blocked", tone: "danger" };
    case "failed":
      return { text: "Failed", tone: "danger" };
    default:
      return null;
  }
}

/** I-10 badge suffix: waiting duration or reset countdown, when known. */
export function attentionSuffix(
  session: InstanceInfo,
  nowMs: number = Date.now(),
): string | null {
  if (session.needs_attention === "waiting_user") {
    return waitingFor(session.last_activity_ms, nowMs);
  }
  if (session.needs_attention === "quota_blocked" && session.quota?.resets_at_ms != null) {
    return `resets ${formatClock(session.quota.resets_at_ms)}`;
  }
  return null;
}

// --- I-07 sparkline history (honest: measured samples only) ------------------

const CPU_HISTORY_MAX = 60; // 5 min at the 5 s resource tick

const cpuHistory = new Map<string, number[]>();
const lastAppendAt = new Map<string, number>();

/**
 * Remembers one measured CPU sample for the Instance and returns the
 * trailing window (oldest → newest). Only real samples enter the
 * history; absence renders the placeholder dash, never a flat zero.
 * Rapid identical re-appends (React StrictMode double-render) collapse
 * into one sample.
 */
export function rememberCpuSample(id: string, sample: number | null, nowMs: number = Date.now()): number[] {
  if (sample !== null) {
    const window = cpuHistory.get(id) ?? [];
    const lastAt = lastAppendAt.get(id) ?? 0;
    const isDuplicate =
      window.length > 0 && window[window.length - 1] === sample && nowMs - lastAt < 2_500;
    if (!isDuplicate) {
      window.push(sample);
      if (window.length > CPU_HISTORY_MAX) window.shift();
      cpuHistory.set(id, window);
      lastAppendAt.set(id, nowMs);
    }
  }
  return cpuHistory.get(id) ?? [];
}

// --- Screen-reader summary ----------------------------------------------------

/** Full field text for `aria-label`: every number reachable as text. */
export function ariaSummary(session: InstanceInfo, nowMs: number = Date.now()): string {
  const mark = providerMark(session.provider);
  const chip = stateChip(session.status);
  const badge = attentionBadge(session);
  const parts: string[] = [
    `${mark.abbrev} provider`,
    session.account_label,
    session.model ?? "default model",
    badge ? badge.text : chip.label,
  ];
  if (session.quota) {
    parts.push(
      `quota ${Math.round(100 - session.quota.used_percentage)}% left, ${session.quota.confidence}`,
    );
  } else {
    parts.push("Quota: no data");
  }
  const tokens = tokenSummary(session.tokens);
  if (tokens) parts.push(tokens);
  if (session.cpu_pct !== null && session.rss_bytes !== null) {
    parts.push(`cpu ${Math.round(session.cpu_pct)}%, ram ${formatBytes(session.rss_bytes)}`);
  }
  if (session.child_count) parts.push(`${session.child_count} children`);
  if (session.subtask_count) parts.push(`${session.subtask_count} subtasks`);
  if (session.git_branch) parts.push(`branch ${session.git_branch}`);
  parts.push(ageSummary(session.started_at_ms, session.last_activity_ms, nowMs));
  const suffix = attentionSuffix(session, nowMs);
  if (badge && suffix) parts.push(suffix);
  return parts.join(", ");
}

/** `1.1 GB` / `890 MB` byte formatting. */
export function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${Math.round(bytes / 1024 ** 2)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

// --- small shared renderers ---------------------------------------------------

/** I-02 initials disc with the per-Account hue. */
export function AccountDisc({
  accountId,
  label,
  size = 18,
}: {
  accountId: string | null;
  label: string;
  size?: number;
}) {
  return (
    <span
      className="acct-disc inline-flex shrink-0 items-center justify-center rounded-full text-[9px] font-semibold"
      style={
        {
          "--acct-hue": hueOf(accountId),
          width: size,
          height: size,
        } as React.CSSProperties
      }
      title={label}
      aria-hidden="true"
    >
      {initialsOf(label)}
    </span>
  );
}

/** I-08 children/subtask badges — hidden at 0/`null` (no `⑂ 0` noise). */
export function ChildBadges({ session }: { session: InstanceInfo }): ReactNode {
  const children = session.child_count ?? 0;
  const subtasks = session.subtask_count ?? 0;
  if (children === 0 && subtasks === 0) return null;
  return (
    <span className="inline-flex shrink-0 items-center gap-1.5 text-[10px] text-muted">
      {children > 0 && (
        <span className="inline-flex items-center gap-0.5" title={`${children} child processes`}>
          <IconFork width={10} height={10} /> {children}
        </span>
      )}
      {subtasks > 0 && (
        <span className="inline-flex items-center gap-0.5" title={`${subtasks} subtasks`}>
          <IconSubtasks width={10} height={10} /> {subtasks}
        </span>
      )}
    </span>
  );
}

/** I-09 branch + dirty — omitted entirely for non-git cwds. */
export function GitBadge({ session }: { session: InstanceInfo }): ReactNode {
  if (!session.git_branch) return null;
  const dirty = session.git_dirty_count;
  return (
    <span className="inline-flex min-w-0 items-center gap-0.5 text-[10px] text-muted" title={`branch ${session.git_branch}${dirty ? ` · ${dirty} dirty` : ""}`}>
      <IconGitBranch width={10} height={10} />
      <span className="truncate">{session.git_branch}</span>
      {dirty ? <span className="shrink-0 text-warning">· {dirty}●</span> : null}
    </span>
  );
}
