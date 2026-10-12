import { ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { IconProjects, IconSessions, IconTerminal } from "./icons";
import type { ProjectInfo, SessionInfo } from "../lib/bridge";

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
  sessions: SessionInfo[];
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
                  title={s.label}
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
  sessions: SessionInfo[];
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
  sessions: SessionInfo[];
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

function SessionRow({
  session,
  selected,
  onSelect,
}: {
  session: SessionInfo;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(session.id)}
      className={
        "relative flex min-w-0 w-full items-center gap-2 rounded-lg py-[7px] pl-3 pr-2.5 text-left outline-none transition-colors " +
        "focus-visible:ring-1 focus-visible:ring-accent/60 " +
        (selected ? "bg-surface" : "hover:bg-surface/60")
      }
    >
      {selected && (
        <span className="absolute left-0.5 top-1/2 h-4 w-[2px] -translate-y-1/2 rounded-full bg-accent" />
      )}
      <span
        className={
          "size-1.5 shrink-0 rounded-full " +
          (isLive(session.status)
            ? "bg-live animate-breathe"
            : session.status === "failed"
              ? "bg-danger"
              : "bg-faint")
        }
      />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[12.5px] font-medium leading-tight text-ink">
          {session.label}
        </span>
        <span className="block truncate text-[10.5px] leading-tight text-faint">
          {session.status}
        </span>
      </span>
      {session.kind === "desktop" && (
        <span className="shrink-0 text-[9px] font-medium uppercase tracking-wide text-accent-hi">
          app
        </span>
      )}
    </button>
  );
}
