import { useState } from "react";

/**
 * AJ-E506: the shared Cleanup row (the Cleanup Centre AND the
 * Instance Processes tab render THIS component — no copies, no
 * divergent labels; divergence is how T3 survivors get mislabelled).
 *
 * Read-only by construction (Tier 0): the row's data DTO carries no
 * kill fields, and this component renders no signalling controls —
 * not even disabled ones (a rendered-but-disabled kill trains the
 * wrong trust model, AJ-E508's guardrail).
 */

export interface VerificationBadgeInfo {
  verdict: string;
  detail: string;
}

export interface EvidenceChainInfo {
  root_pid: number | null;
  parent_pid: number | null;
  pgid: number | null;
  escaped_group: boolean;
  verification: VerificationBadgeInfo;
  first_seen: number;
  last_seen: number;
}

export interface CleanupRowInfo {
  pid: number;
  pgid: number | null;
  role: string;
  state: string;
  state_label: string;
  role_guess: string;
  exe_path: string;
  cmdline: string;
  age_ms: number | null;
  rss_bytes: number | null;
  cpu_pct: number | null;
  ports: string | null;
  hog: boolean;
  hog_rules: string[];
  evidence: EvidenceChainInfo;
}

function ageText(ms: number | null): string {
  if (ms === null) return "age unknown";
  const secs = Math.max(0, Math.floor(ms / 1000));
  if (secs < 60) return `${secs}s`;
  if (secs < 3_600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86_400) return `${Math.floor(secs / 3_600)}h`;
  return `${Math.floor(secs / 86_400)}d`;
}

function rssText(bytes: number | null): string {
  if (bytes === null) return "—";
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
}

/** State badge icon per class — icon+text, never colour alone (1.4.1). */
function stateIcon(state: string): string {
  switch (state) {
    case "orphaned":
      return "⚑";
    case "adopted":
      return "↻";
    case "unverifiable":
      return "⊘";
    case "exited":
      return "✓";
    default:
      return "●";
  }
}

export function ProcessRow(props: { row: CleanupRowInfo }) {
  const row = props.row;
  const [expanded, setExpanded] = useState(false);
  const evidence = row.evidence;
  return (
    <li className="border-t border-hairline" data-testid={`cleanup-row-${row.pid}`}>
      <div className="flex items-center gap-2 py-1.5 text-[11px]">
        <button
          type="button"
          onClick={() => setExpanded((open) => !open)}
          aria-expanded={expanded}
          className="flex min-w-0 flex-1 items-center gap-2 rounded-md px-1 py-0.5 text-left outline-none focus-visible:ring-1 focus-visible:ring-accent/60"
          title="Why does Juggler think this is mine?"
        >
          <span aria-hidden="true" className="w-3 text-center text-[9px] text-muted">
            {expanded ? "▾" : "▸"}
          </span>
          <span
            className={`inline-flex shrink-0 items-center gap-1 rounded-full border px-1.5 py-[1px] text-[9.5px] ${
              row.state === "orphaned"
                ? "border-accent/50 text-accent"
                : row.state === "unverifiable"
                  ? "border-danger/50 text-danger"
                  : "border-hairline text-muted"
            }`}
            data-testid={`cleanup-state-${row.pid}`}
          >
            <span aria-hidden="true">{stateIcon(row.state)}</span>
            {row.state_label}
          </span>
          {row.hog && (
            <span
              className="inline-flex shrink-0 items-center gap-1 rounded-full border border-warning/50 px-1.5 py-[1px] text-[9.5px] text-warning"
              title={row.hog_rules.join("; ")}
            >
              <span aria-hidden="true">▲</span>hog
            </span>
          )}
          <span className="truncate font-mono text-[10.5px] text-ink">{row.cmdline}</span>
          <span className="ml-auto flex shrink-0 items-center gap-2 font-mono text-[10px] text-muted">
            <span>PID {row.pid}</span>
            {row.pgid !== null && <span>PGID {row.pgid}</span>}
            <span>{ageText(row.age_ms)}</span>
            <span>{rssText(row.rss_bytes)}</span>
            {row.cpu_pct !== null && <span>{row.cpu_pct.toFixed(0)}%</span>}
            {row.ports !== null && <span className="text-accent">:{row.ports.split(",").join(" :")}</span>}
          </span>
        </button>
      </div>
      {expanded && (
        <div
          className="flex flex-col gap-1 rounded-lg border border-hairline bg-surface/40 p-2.5 text-[10.5px] text-muted"
          data-testid={`cleanup-evidence-${row.pid}`}
        >
          <p className="font-medium text-ink">Why does Juggler think this is mine?</p>
          <dl className="grid grid-cols-[auto_1fr] items-baseline gap-x-3 gap-y-0.5">
            <dt className="text-faint">Spawned by</dt>
            <dd>
              Instance root PID {evidence.root_pid ?? "—"}
              {evidence.parent_pid !== null && <> · first-seen parent {evidence.parent_pid}</>}
            </dd>
            <dt className="text-faint">Group</dt>
            <dd>
              PGID {evidence.pgid ?? "—"}{" "}
              {evidence.escaped_group
                ? "— escaped its process group (containment void)"
                : evidence.pgid !== null && evidence.pgid === evidence.root_pid
                  ? "— matches the root (contained)"
                  : ""}
            </dd>
            <dt className="text-faint">Identity check</dt>
            <dd data-testid={`cleanup-verification-${row.pid}`}>
              {evidence.verification.verdict} — {evidence.verification.detail}
            </dd>
            <dt className="text-faint">Role</dt>
            <dd>{row.role_guess}</dd>
            <dt className="text-faint">Seen</dt>
            <dd>
              first {new Date(evidence.first_seen).toISOString().slice(0, 19)} · last{" "}
              {new Date(evidence.last_seen).toISOString().slice(0, 19)}
            </dd>
          </dl>
          <p className="text-faint">Read-only (Tier 0) — verified stop actions arrive with AJ-E507.</p>
        </div>
      )}
    </li>
  );
}

/** The empty success state — copy EXACT (Req.5). */
export function CleanupEmpty(props: { cleanInstances: number }) {
  return (
    <p className="text-[12px] text-muted" data-testid="cleanup-empty">
      Nothing left behind — {props.cleanInstances} Instances running clean.
    </p>
  );
}
