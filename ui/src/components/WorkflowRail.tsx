import { ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { IconProjects, IconSessions, IconTerminal } from "./icons";
import type { InstanceInfo, ProjectInfo } from "../lib/bridge";
import {
  AccountDisc,
  ChildBadges,
  GitBadge,
  attentionBadge,
  attentionSuffix,
  ariaSummary,
  providerMark,
  rememberCpuSample,
  stateChip,
  tokenSummary,
  ageSummary,
  formatBytes,
  type StateChipTone,
} from "./indicator";
import { QuotaBar, Sparkline } from "./primitives";

/** The three workflow tabs the whole navbar switches between. */
export type WorkTab = "projects" | "instances" | "sessions";

const TABS: { id: WorkTab; label: string; icon: ReactNode }[] = [
  { id: "projects", label: "Projects", icon: <IconProjects width={15} height={15} /> },
  { id: "instances", label: "Instances", icon: <IconTerminal width={15} height={15} /> },
  { id: "sessions", label: "Sessions", icon: <IconSessions width={15} height={15} /> },
];

const isLive = (status: string) => status === "running" || status === "starting";

const WIDTH_MIN = 208;
const WIDTH_MAX = 360;
const WIDTH_DEFAULT = 264;
const STORE_WIDTH = "aij.rail.width";

function readWidth(): number {
  const raw = Number(localStorage.getItem(STORE_WIDTH));
  if (!Number.isFinite(raw)) return WIDTH_DEFAULT;
  return Math.min(WIDTH_MAX, Math.max(WIDTH_MIN, raw));
}

/**
 * The workflow navigator (ADR-0007): one rail, three tabs, and the live
 * list of whichever the user is working through. The rail drag-resizes
 * (persisted), collapses to icons, and every element — tabs first —
 * reflows gracefully at any width: labels truncate cleanly, then fade to
 * icon-only below the label threshold. Nothing ever clips.
 */
export function WorkflowRail({
  tab,
  onTab,
  collapsed,
  onToggleCollapsed,
  projects,
  sessions,
  loginTerminals,
  selectedId,
  onSelectSession,
  onSelectProject,
  onNewSession,
  header,
  footer,
}: {
  tab: WorkTab;
  onTab: (tab: WorkTab) => void;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  projects: ProjectInfo[];
  sessions: InstanceInfo[];
  loginTerminals: { id: string; title: string; ended: boolean }[];
  selectedId: string | null;
  onSelectSession: (id: string) => void;
  onSelectProject: (id: string) => void;
  onNewSession: () => void;
  header: (collapsed: boolean) => ReactNode;
  footer: (collapsed: boolean) => ReactNode;
}) {
  const [width, setWidth] = useState<number>(readWidth);
  const [dragging, setDragging] = useState(false);
  const dragState = useRef<{ startX: number; startWidth: number } | null>(null);

  const beginDrag = useCallback(
    (event: React.PointerEvent) => {
      if (collapsed) return;
      event.preventDefault();
      dragState.current = { startX: event.clientX, startWidth: width };
      setDragging(true);
    },
    [collapsed, width],
  );

  useEffect(() => {
    if (!dragging) return;
    const onMove = (move: PointerEvent) => {
      const state = dragState.current;
      if (!state) return;
      const next = Math.min(
        WIDTH_MAX,
        Math.max(WIDTH_MIN, state.startWidth + (move.clientX - state.startX)),
      );
      setWidth(next);
    };
    const onUp = () => {
      setDragging(false);
      dragState.current = null;
      setWidth((current) => {
        localStorage.setItem(STORE_WIDTH, String(current));
        return current;
      });
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [dragging]);

  // Label threshold: below this width, tabs show icons only (with
  // tooltips) instead of squeezing text into clipping.
  const showLabels = !collapsed && width >= 244;

  return (
    <aside
      style={{ width: collapsed ? 60 : width }}
      className={
        "relative flex h-full shrink-0 select-none flex-col border-r border-hairline bg-surface/30 backdrop-blur-xl transition-[width] duration-150 ease-out " +
        (dragging ? "" : "")
      }
    >
      {header(collapsed)}

      {/* Tab switcher — adapts: icon+label → icon-only as the rail narrows */}
      <nav
        className={
          "flex gap-1 px-2 pb-2 pt-0.5 " +
          (collapsed ? "flex-col items-center" : "")
        }
        role="tablist"
      >
        {TABS.map((item) => {
          const active = tab === item.id;
          return (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={active}
              title={collapsed || !showLabels ? item.label : undefined}
              onClick={() => onTab(item.id)}
              className={
                "group/tab relative flex min-w-0 flex-1 items-center justify-center overflow-hidden rounded-lg text-[12px] font-medium outline-none transition-colors duration-100 " +
                "focus-visible:ring-1 focus-visible:ring-accent/60 " +
                (collapsed
                  ? "size-9 "
                  : showLabels
                    ? "gap-1.5 px-2 py-[7px] "
                    : "gap-0 px-0 py-[7px] ") +
                (active
                  ? "border border-accent/25 bg-accent-soft text-accent-hi"
                  : "border border-transparent text-muted hover:bg-surface hover:text-ink")
              }
            >
              <span className="shrink-0 leading-none">{item.icon}</span>
              {showLabels && (
                <span className="min-w-0 truncate opacity-100 transition-opacity duration-150">
                  {item.label}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {/* The live list for the active tab */}
      <div className="rail-scroll min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-2 pb-2">
        {collapsed ? (
          <div className="flex flex-col items-center gap-1 pt-1">
            {sessions
              .filter((s) => isLive(s.status))
              .slice(0, 6)
              .map((s) => (
                <button
                  key={s.id}
                  type="button"
                  title={s.account_label}
                  onClick={() => onSelectSession(s.id)}
                  className="flex size-8 items-center justify-center rounded-lg hover:bg-surface"
                >
                  <span
                    className={
                      "size-2 rounded-full " +
                      (s.id === selectedId ? "bg-live" : "bg-live/50")
                    }
                  />
                </button>
              ))}
          </div>
        ) : tab === "projects" ? (
          <ProjectList projects={projects} selectedId={selectedId} onSelect={onSelectProject} />
        ) : tab === "instances" ? (
          <InstanceList sessions={sessions} selectedId={selectedId} onSelect={onSelectSession} />
        ) : (
          <SessionList
            sessions={sessions}
            loginTerminals={loginTerminals}
            selectedId={selectedId}
            onSelect={onSelectSession}
          />
        )}
      </div>

      {/* New session — the workflow's primary action lives in the navigator */}
      <div className="border-t border-hairline/60 p-2">
        <button
          type="button"
          onClick={onNewSession}
          title={collapsed ? "New session (⌘N)" : undefined}
          className="flex w-full items-center justify-center gap-2 overflow-hidden rounded-lg border border-accent/30 bg-accent-soft px-3 py-[7px] text-[12px] font-medium text-accent-hi outline-none transition-colors hover:border-accent/50 hover:bg-accent/15 focus-visible:ring-1 focus-visible:ring-accent/60"
        >
          <IconTerminal width={14} height={14} />
          {!collapsed && <span className="truncate">New session</span>}
        </button>
      </div>

      {footer(collapsed)}

      {/* Drag-resize handle: hairline that lights up on hover/drag */}
      {!collapsed && (
        <div
          onPointerDown={beginDrag}
          onDoubleClick={onToggleCollapsed}
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize navigator"
          className={
            "absolute right-0 top-0 z-20 h-full w-1.5 cursor-col-resize " +
            (dragging ? "" : "transition-colors")
          }
        >
          <div
            className={
              "ml-auto h-full w-px " +
              (dragging ? "bg-accent/70" : "bg-transparent hover:bg-accent/50")
            }
          />
        </div>
      )}
    </aside>
  );
}

function ProjectList({
  projects,
  selectedId,
  onSelect,
}: {
  projects: ProjectInfo[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  if (projects.length === 0) {
    return (
      <p className="px-2 py-6 text-center text-[11px] leading-relaxed text-faint">
        Launch a session in any directory — it becomes a project here.
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-0.5">
      {projects.map((project) => {
        const active = project.id === selectedId;
        return (
          <button
            key={project.id}
            type="button"
            onClick={() => onSelect(project.id)}
            className={
              "relative min-w-0 rounded-lg py-[7px] pl-3 pr-2.5 text-left outline-none transition-colors " +
              "focus-visible:ring-1 focus-visible:ring-accent/60 " +
              (active ? "bg-surface" : "hover:bg-surface/60")
            }
          >
            {active && (
              <span className="absolute left-0.5 top-1/2 h-4 w-[2px] -translate-y-1/2 rounded-full bg-accent" />
            )}
            <div className="flex min-w-0 items-center gap-2">
              <span className="shrink-0 text-accent-hi/90">
                <IconProjects width={13} height={13} />
              </span>
              <span className="min-w-0 flex-1 truncate text-[12.5px] font-medium text-ink">
                {project.name}
              </span>
              {project.active_instance ? (
                <span className="dot-live size-1.5 shrink-0 rounded-full bg-live" />
              ) : null}
            </div>
            <p className="mt-0.5 truncate pl-[21px] text-[10.5px] leading-tight text-faint">
              {project.accounts_used.length} acct
              {project.accounts_used.length === 1 ? "" : "s"} · {project.instance_count} run
              {project.instance_count === 1 ? "" : "s"}
            </p>
          </button>
        );
      })}
    </div>
  );
}

function InstanceList({
  sessions,
  selectedId,
  onSelect,
}: {
  sessions: InstanceInfo[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const live = sessions.filter((s) => isLive(s.status));
  const rest = sessions.filter((s) => !isLive(s.status)).slice(0, 8);
  return (
    <div className="flex flex-col gap-2">
      <ListSection label="Live now">
        {live.length === 0 ? (
          <p className="px-3 py-1 text-[11px] text-faint">Nothing running</p>
        ) : (
          live.map((s) => (
            <SessionRow key={s.id} session={s} selected={s.id === selectedId} onSelect={onSelect} />
          ))
        )}
      </ListSection>
      {rest.length > 0 && (
        <ListSection label="Recent">
          {rest.map((s) => (
            <SessionRow key={s.id} session={s} selected={s.id === selectedId} onSelect={onSelect} />
          ))}
        </ListSection>
      )}
    </div>
  );
}

function SessionList({
  sessions,
  loginTerminals,
  selectedId,
  onSelect,
}: {
  sessions: InstanceInfo[];
  loginTerminals: { id: string; title: string; ended: boolean }[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      {loginTerminals.length > 0 && (
        <ListSection label="Sign-ins">
          {loginTerminals.map((t) => {
            const active = t.id === selectedId;
            return (
              <button
                key={t.id}
                type="button"
                onClick={() => onSelect(t.id)}
                className={
                  "relative flex min-w-0 w-full items-center gap-2 rounded-lg py-[7px] pl-3 pr-2.5 text-left outline-none transition-colors " +
                  "focus-visible:ring-1 focus-visible:ring-accent/60 " +
                  (active ? "bg-surface" : "hover:bg-surface/60")
                }
              >
                {active && (
                  <span className="absolute left-0.5 top-1/2 h-4 w-[2px] -translate-y-1/2 rounded-full bg-accent" />
                )}
                <span
                  className={
                    "size-1.5 shrink-0 rounded-full " +
                    (t.ended ? "bg-faint" : "bg-live animate-breathe")
                  }
                />
                <span className="min-w-0 flex-1 truncate text-[12.5px]">{t.title}</span>
              </button>
            );
          })}
        </ListSection>
      )}
      <ListSection label="Sessions">
        {sessions.length === 0 ? (
          <p className="px-3 py-1 text-[11px] text-faint">
            No sessions yet — start one below.
          </p>
        ) : (
          sessions.slice(0, 15).map((s) => (
            <SessionRow key={s.id} session={s} selected={s.id === selectedId} onSelect={onSelect} />
          ))
        )}
      </ListSection>
    </div>
  );
}

function ListSection({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <p className="px-3 pb-0.5 pt-1 text-[10px] font-medium uppercase tracking-[0.08em] text-faint">
        {label}
      </p>
      {children}
    </div>
  );
}

/** Tone classes for the compact state/attention chip (token-driven). */
const chipToneClasses: Record<StateChipTone, string> = {
  neutral: "bg-surface-2 text-muted border-hairline",
  success: "bg-success-soft text-success border-success/25",
  warning: "bg-warning-soft text-warning border-warning/25",
  danger: "bg-danger-soft text-danger border-danger/25",
  live: "bg-live-soft text-live border-live/25",
  accent: "bg-accent-soft text-accent-hi border-accent/25",
};

/**
 * The indicator row (AJ-E402): I-01..I-11 in pass2/04 §3 render order —
 * provider mark, account disc, model chip, state chip, quota bar with
 * tier tag, tokens, children, git, attention, age, resource sparkline.
 * Absent data renders the exact honest copy (`Quota: no data`,
 * `default model`, `Unknown`, `—`), never fabricated numbers.
 */
export function SessionRow({
  session,
  selected,
  onSelect,
}: {
  session: InstanceInfo;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  const mark = providerMark(session.provider);
  const badge = attentionBadge(session);
  const chip = stateChip(session.status);
  const suffix = attentionSuffix(session);
  const tokens = tokenSummary(session.tokens);
  const samples = rememberCpuSample(session.id, session.cpu_pct);
  const resources =
    session.cpu_pct !== null && session.rss_bytes !== null
      ? `${Math.round(session.cpu_pct)}% · ${formatBytes(session.rss_bytes)}`
      : null;

  return (
    <button
      type="button"
      onClick={() => onSelect(session.id)}
      aria-label={ariaSummary(session)}
      className={
        "relative flex min-w-0 w-full flex-col gap-1 rounded-lg px-2.5 py-2 pl-3 text-left outline-none transition-colors " +
        "focus-visible:ring-1 focus-visible:ring-accent/60 " +
        (selected ? "bg-surface" : "hover:bg-surface/60")
      }
    >
      {selected && (
        <span className="absolute left-0.5 top-1/2 h-4 w-[2px] -translate-y-1/2 rounded-full bg-accent" />
      )}
      {/* I-01 I-02 I-03 … I-04/I-10. Narrow rails make the provider
          abbrev and model yield first (truncate); the disc, chip text and
          glyphs never clip (WCAG 1.4.1). */}
      <span className="flex min-w-0 items-center gap-1.5 overflow-hidden">
        <span
          className="inline-flex min-w-0 shrink items-center gap-1 text-[10.5px] font-medium text-ink"
          title={session.provider}
        >
          <mark.Icon width={11} height={11} className="shrink-0 text-accent-hi" />
          <span className="truncate">{mark.abbrev}</span>
        </span>
        <AccountDisc accountId={session.account_id} label={session.account_label} />
        <span className="min-w-0 shrink truncate text-[10.5px] text-muted" title={session.model ?? "default model"}>
          {session.model ?? <span className="text-faint">default model</span>}
        </span>
        <span className="ml-auto inline-flex shrink-0 items-center gap-1">
          {badge ? (
            <span
              className={
                "inline-flex items-center gap-1 rounded-full border px-1.5 py-px text-[10px] font-medium " +
                chipToneClasses[badge.tone]
              }
            >
              {badge.tone === "warning" || badge.tone === "danger" ? "!" : null}
              {badge.text}
            </span>
          ) : (
            <span
              className={
                "inline-flex items-center gap-1 rounded-full border px-1.5 py-px text-[10px] font-medium " +
                chipToneClasses[chip.tone]
              }
            >
              <chip.Icon width={10} height={10} />
              {chip.label}
            </span>
          )}
        </span>
      </span>
      {/* I-10 suffix (wait duration / reset countdown) + I-09 I-11. The
          age text yields (truncate) before anything else clips. */}
      <span className="flex min-w-0 items-center gap-1.5 overflow-hidden text-[10px] text-faint">
        {badge && suffix ? (
          <span
            className={
              "shrink-0 whitespace-nowrap font-medium " +
              (badge.tone === "danger" ? "text-danger" : "text-warning")
            }
          >
            {suffix}
          </span>
        ) : null}
        <GitBadge session={session} />
        <span className="ml-auto min-w-0 shrink truncate whitespace-nowrap tabular-nums">
          {ageSummary(session.started_at_ms, session.last_activity_ms)}
        </span>
      </span>
      {/* I-05 */}
      <QuotaBar quota={session.quota} />
      {/* I-06 I-08 I-07 — right-hand resource text yields before anything
          clips. */}
      <span className="flex min-w-0 items-center gap-1.5 overflow-hidden text-[10px] text-muted">
        {tokens ? <span className="shrink-0 tabular-nums">{tokens}</span> : null}
        <ChildBadges session={session} />
        <span className="ml-auto inline-flex min-w-0 shrink items-center gap-1.5">
          <Sparkline
            samples={samples}
            title={resources ? `cpu ${resources}` : "resources unavailable"}
          />
          {resources ? <span className="min-w-0 truncate tabular-nums">{resources}</span> : null}
        </span>
      </span>
      {session.kind === "desktop" && (
        <span className="self-start text-[9px] font-medium uppercase tracking-wide text-accent-hi">
          app
        </span>
      )}
    </button>
  );
}

/**
 * The two-line card variant (AJ-E402 Req.4): identical I-01..I-11 fields
 * to the rail row, sparkline widened to the 5-min window. Used by the
 * Overview live list — no number exists here that the rail cannot
 * explain.
 */
export function SessionCard({ session, onSelect }: { session: InstanceInfo; onSelect?: (id: string) => void }) {
  const mark = providerMark(session.provider);
  const badge = attentionBadge(session);
  const chip = stateChip(session.status);
  const suffix = attentionSuffix(session);
  const tokens = tokenSummary(session.tokens);
  const samples = rememberCpuSample(session.id, session.cpu_pct);
  const resources =
    session.cpu_pct !== null && session.rss_bytes !== null
      ? `${Math.round(session.cpu_pct)}% · ${formatBytes(session.rss_bytes)}`
      : null;
  return (
    <button
      type="button"
      onClick={onSelect ? () => onSelect(session.id) : undefined}
      aria-label={ariaSummary(session)}
      className="flex w-full min-w-0 flex-col gap-1.5 rounded-xl px-3 py-2.5 text-left outline-none transition-colors hover:bg-surface-2/60 focus-visible:ring-1 focus-visible:ring-accent/60"
    >
      <span className="flex min-w-0 items-center gap-2">
        <span className="inline-flex shrink-0 items-center gap-1.5 text-[11.5px] font-medium text-ink">
          <mark.Icon width={12} height={12} className="text-accent-hi" />
          {mark.abbrev}
        </span>
        <AccountDisc accountId={session.account_id} label={session.account_label} size={20} />
        <span className="min-w-0 truncate text-[12px] text-ink">{session.account_label}</span>
        <span className="min-w-0 shrink-0 truncate text-[11px] text-muted">
          {session.model ?? <span className="text-faint">default model</span>}
        </span>
        <span className="ml-auto inline-flex shrink-0 items-center gap-1">
          {badge ? (
            <span
              className={
                "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium " +
                chipToneClasses[badge.tone]
              }
            >
              {badge.tone === "warning" || badge.tone === "danger" ? "!" : null}
              {badge.text}
            </span>
          ) : (
            <span
              className={
                "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium " +
                chipToneClasses[chip.tone]
              }
            >
              <chip.Icon width={11} height={11} />
              {chip.label}
            </span>
          )}
        </span>
      </span>
      <span className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted">
        {badge && suffix ? (
          <span
            className={
              "shrink-0 whitespace-nowrap font-medium " +
              (badge.tone === "danger" ? "text-danger" : "text-warning")
            }
          >
            {suffix}
          </span>
        ) : null}
        <QuotaBar quota={session.quota} />
        {tokens ? <span className="tabular-nums">{tokens}</span> : null}
        <ChildBadges session={session} />
        <GitBadge session={session} />
        <span className="ml-auto inline-flex shrink-0 items-center gap-2">
          <Sparkline
            samples={samples}
            width={64}
            title={resources ? `cpu ${resources}` : "resources unavailable"}
          />
          {resources ? <span className="tabular-nums">{resources}</span> : null}
        </span>
        <span className="shrink-0 whitespace-nowrap tabular-nums text-faint">
          {ageSummary(session.started_at_ms, session.last_activity_ms)}
        </span>
      </span>
    </button>
  );
}
