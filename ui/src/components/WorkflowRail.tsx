import { ReactNode } from "react";
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

/**
 * The workflow navigator (ADR-0007): one rail, three tabs, and the live
 * list of whichever the user is working through — projects, instances, or
 * sessions — with the New-session action at hand. Collapses to icons.
 */
export function WorkflowRail({
  tab,
  onTab,
  collapsed,
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
  return (
    <aside
      className={
        "flex h-full shrink-0 flex-col border-r border-hairline bg-surface/30 backdrop-blur-xl transition-[width] duration-150 " +
        (collapsed ? "w-[64px]" : "w-[268px]")
      }
    >
      {header(collapsed)}

      {/* Tab switcher — collapses to icons with the rail */}
      <nav className={"flex gap-1 px-2 pb-2 " + (collapsed ? "flex-col items-center" : "")}>
        {TABS.map((item) => (
          <button
            key={item.id}
            type="button"
            title={item.label}
            onClick={() => onTab(item.id)}
            className={
              "flex items-center gap-2 rounded-lg text-[12px] font-medium transition-colors " +
              (collapsed ? "size-9 justify-center" : "flex-1 justify-center px-2 py-1.5 ") +
              (tab === item.id
                ? "bg-accent-soft text-accent-hi border border-accent/25"
                : "text-muted border border-transparent hover:bg-surface hover:text-ink")
            }
          >
            {item.icon}
            {!collapsed && item.label}
          </button>
        ))}
      </nav>

      {/* The live list for the active tab */}
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {collapsed ? (
          <div className="flex flex-col items-center gap-2 pt-1">
            {sessions
              .filter((s) => isLive(s.status))
              .slice(0, 4)
              .map((s) => (
                <button
                  key={s.id}
                  type="button"
                  title={s.label}
                  onClick={() => onSelectSession(s.id)}
                  className="flex size-8 items-center justify-center"
                >
                  <span
                    className={
                      "size-2 rounded-full " +
                      (s.id === selectedId ? "bg-live" : "bg-live/60")
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
          title="New session"
          className={
            "flex w-full items-center justify-center gap-2 rounded-lg border border-accent/30 bg-accent-soft px-3 py-2 text-[12px] font-medium text-accent-hi transition-colors hover:border-accent/50 hover:bg-accent/15 " +
            (collapsed ? "px-0" : "")
          }
        >
          <IconTerminal width={14} height={14} />
          {!collapsed && "New session"}
        </button>
      </div>

      {footer(collapsed)}
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
      <p className="px-2 py-6 text-center text-[11px] text-faint">
        Launch a session in any directory — it becomes a project here.
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-1">
      {projects.map((project) => (
        <button
          key={project.id}
          type="button"
          onClick={() => onSelect(project.id)}
          className={
            "rounded-lg px-2.5 py-2 text-left transition-colors " +
            (project.id === selectedId
              ? "bg-surface sheen"
              : "hover:bg-surface/60")
          }
        >
          <div className="flex items-center gap-2">
            <span className="text-accent-hi">
              <IconProjects width={13} height={13} />
            </span>
            <span className="min-w-0 flex-1 truncate text-[12px] font-medium">
              {project.name}
            </span>
            {project.active_instance ? (
              <span className="dot-live size-1.5 shrink-0 rounded-full bg-live" />
            ) : null}
          </div>
          <p className="mt-0.5 truncate text-[10px] text-faint">
            {project.accounts_used.length} account{project.accounts_used.length === 1 ? "" : "s"} ·{" "}
            {project.instance_count} run{project.instance_count === 1 ? "" : "s"}
          </p>
        </button>
      ))}
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
    <div className="flex flex-col gap-3">
      <ListSection label="Live now">
        {live.length === 0 ? (
          <p className="px-2.5 py-1 text-[11px] text-faint">Nothing running</p>
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
    <div className="flex flex-col gap-3">
      {loginTerminals.length > 0 && (
        <ListSection label="Sign-ins">
          {loginTerminals.map((t) => (
            <button
              key={t.id}
              type="button"
              onClick={() => onSelect(t.id)}
              className={
                "flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left transition-colors " +
                (t.id === selectedId ? "bg-surface sheen" : "hover:bg-surface/60")
              }
            >
              <span
                className={
                  "size-1.5 shrink-0 rounded-full " + (t.ended ? "bg-faint" : "bg-live animate-breathe")
                }
              />
              <span className="min-w-0 flex-1 truncate text-[12px]">{t.title}</span>
            </button>
          ))}
        </ListSection>
      )}
      <ListSection label="Sessions">
        {sessions.length === 0 ? (
          <p className="px-2.5 py-1 text-[11px] text-faint">
            No sessions yet — start one with New session.
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
      <p className="px-2.5 pb-1 pt-1 text-[10px] uppercase tracking-wider text-faint">{label}</p>
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
        "flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left transition-colors " +
        (selected ? "bg-surface sheen" : "hover:bg-surface/60")
      }
    >
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
        <span className="block truncate text-[12px] font-medium">{session.label}</span>
        <span className="block truncate text-[10px] text-faint">{session.status}</span>
      </span>
      {session.kind === "desktop" && (
        <span className="shrink-0 text-[9px] uppercase tracking-wide text-accent-hi">app</span>
      )}
    </button>
  );
}
