import { useEffect, useMemo, useRef, useState } from "react";
import {
  CleanupEmpty,
  ProcessRow,
  type CleanupRowInfo,
} from "../components/ProcessRow";

/**
 * AJ-E506: the global Cleanup Centre — a READ-ONLY inventory (Tier 0,
 * D-08 default): what Instances left behind, what it costs, and WHY
 * Juggler thinks it is theirs. Zero signalling: no kill control is
 * registered, rendered, or keyboard-reachable (Req.7 — not even a
 * disabled preview, which would train the wrong trust model).
 *
 * Reached from the top-right global menus (ADR-0007); ⌘⇧C opens it,
 * ⌘F focuses the filter. Healthy trees collapse by default so
 * attention lands on orphans (Apple HIG clarity/deference).
 */

export interface CleanupGroupInfo {
  session: string;
  provider: string;
  account: string | null;
  project: string | null;
  machine: string;
  healthy: boolean;
  rows: CleanupRowInfo[];
}

export interface CleanupInventoryData {
  groups: CleanupGroupInfo[];
  clean_instances: number;
  ledger_degraded: boolean;
}

export const CLEANUP_FILTERS = [
  { id: "needs-attention", label: "Needs attention" },
  { id: "all-managed", label: "All managed" },
  { id: "re-adopted", label: "Re-adopted" },
  { id: "history", label: "History (7 days)" },
] as const;

export type CleanupFilterId = (typeof CLEANUP_FILTERS)[number]["id"];

export type CleanupBridge = {
  load: (filter: string) => Promise<CleanupInventoryData>;
};

// Dynamic import (the vi.mock TDZ rule).
export const defaultCleanupBridge: CleanupBridge = {
  load: (filter) =>
    import("../lib/bridge").then((bridge) => bridge.cleanupInventory(filter)),
};

function GroupBlock(props: { group: CleanupGroupInfo }) {
  const group = props.group;
  const [collapsed, setCollapsed] = useState(group.healthy);
  return (
    <section
      className="flex flex-col rounded-xl border border-hairline bg-panel p-3"
      data-testid={`cleanup-group-${group.session}`}
    >
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => setCollapsed((open) => !open)}
          aria-expanded={!collapsed}
          className="flex min-w-0 flex-1 items-center gap-2 rounded-md px-1 py-0.5 text-left outline-none focus-visible:ring-1 focus-visible:ring-accent/60"
        >
          <span aria-hidden="true" className="w-3 text-center text-[9px] text-muted">
            {collapsed ? "▸" : "▾"}
          </span>
          <span className="truncate text-[11.5px] font-medium text-ink">
            {group.provider}
            {group.account !== null ? ` · ${group.account}` : ""}
            {group.project !== null ? ` · ${group.project}` : ""}
          </span>
          <span className="font-mono text-[9.5px] text-faint">
            {group.machine} · {group.rows.length}{" "}
            {group.rows.length === 1 ? "process" : "processes"}
          </span>
          {group.healthy && (
            <span className="ml-auto rounded-full border border-hairline px-1.5 py-[1px] text-[9px] text-muted">
              healthy
            </span>
          )}
        </button>
      </div>
      {!collapsed && (
        <ul className="mt-1">
          {group.rows.map((row) => (
            <ProcessRow key={row.pid} row={row} />
          ))}
        </ul>
      )}
    </section>
  );
}

export function CleanupCentre(props: { bridge?: CleanupBridge }) {
  const bridge = props.bridge ?? defaultCleanupBridge;
  const [filter, setFilter] = useState<CleanupFilterId>("needs-attention");
  const [data, setData] = useState<CleanupInventoryData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const filterRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let alive = true;
    bridge
      .load(filter)
      .then((result) => {
        if (alive) {
          setData(result);
          setError(null);
        }
      })
      .catch((cause: unknown) => {
        if (alive) setError(String(cause));
      });
    return () => {
      alive = false;
    };
  }, [filter, bridge]);

  const visibleGroups = useMemo(() => data?.groups ?? [], [data]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3" data-testid="cleanup-centre">
      <div className="flex items-center gap-2">
        <h2 className="text-sm font-semibold tracking-wide">Cleanup Centre</h2>
        <span className="rounded-full border border-hairline px-1.5 py-[1px] text-[9.5px] text-muted">
          read-only · Tier 0
        </span>
        {/* ⌘F lands here (2.1.1/2.4.7 keyboard-complete + visible). */}
        <div
          ref={filterRef}
          role="radiogroup"
          aria-label="Cleanup filter"
          className="ml-auto flex items-center rounded-lg border border-hairline bg-surface/60 p-0.5"
        >
          {CLEANUP_FILTERS.map((entry) => (
            <button
              key={entry.id}
              type="button"
              role="radio"
              aria-checked={filter === entry.id}
              onClick={() => setFilter(entry.id)}
              className={
                "rounded-md px-2 py-1 text-[10.5px] font-medium transition-colors outline-none focus-visible:ring-1 focus-visible:ring-accent/60 " +
                (filter === entry.id
                  ? "bg-accent-soft text-accent-hi"
                  : "text-muted hover:bg-surface hover:text-ink")
              }
            >
              {entry.label}
            </button>
          ))}
        </div>
      </div>

      {/* Filter changes announced politely (4.1.3). */}
      <p aria-live="polite" className="sr-only" data-testid="cleanup-filter-announcement">
        {`Cleanup filter: ${
          CLEANUP_FILTERS.find((entry) => entry.id === filter)?.label ?? filter
        } — ${visibleGroups.length} Instance ${
          visibleGroups.length === 1 ? "group" : "groups"
        }`}
      </p>

      {data?.ledger_degraded && (
        <p
          className="rounded-lg border border-warning/40 bg-warning/5 p-2 text-[11px] text-ink"
          role="alert"
          data-testid="cleanup-degraded-banner"
        >
          Process tracking degraded — cleanup is read-only until restart.
        </p>
      )}

      {error !== null && (
        <p className="text-[11px] text-muted" role="alert" data-testid="cleanup-error">
          Cleanup inventory unavailable: {error}
        </p>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {data !== null && visibleGroups.length === 0 && error === null && (
          <CleanupEmpty cleanInstances={data.clean_instances} />
        )}
        <div className="flex flex-col gap-2">
          {visibleGroups.map((group) => (
            <GroupBlock key={group.session} group={group} />
          ))}
        </div>
      </div>

      <p className="text-[10px] text-faint" data-testid="cleanup-glossary">
        An <dfn>orphan</dfn> is a process still running after its Instance closed; a{" "}
        <dfn>helper</dfn> is any process an Instance spawned (dev server, watcher…).
      </p>
    </div>
  );
}

/** Focuses the filter group (the ⌘F target). */
export function focusCleanupFilter() {
  const group = document.querySelector<HTMLElement>('[role="radiogroup"][aria-label="Cleanup filter"]');
  const first = group?.querySelector<HTMLElement>("button");
  first?.focus();
}
