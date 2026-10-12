import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Card, Chip, QuotaRing, StatCard } from "./components/primitives";
import { WorkflowRail, type WorkTab } from "./components/WorkflowRail";
import {
  IconFleet,
  IconJuggler,
  IconTerminal,
  IconUsage,
} from "./components/icons";
import { ProfilesScreen } from "./screens/Profiles";
import {
  NewSessionModal,
  ProjectPanel,
  SessionPanel,
  WorkEmpty,
} from "./screens/WorkPanels";
import {
  onSessionsUpdated,
  onTerminalExit,
  projectsList,
  sessionsList,
  launchSession,
  dashboardStats,
  fetchAppInfo,
  onAppReady,
  type AppInfo,
  type ProjectInfo,
  type SessionInfo,
  type StatsInfo,
} from "./lib/bridge";

/**
 * App shell (ADR-0007, design iteration 5): a workflow navigator rail
 * (Projects / Instances / Sessions — the list lives in the rail) plus the
 * detail panel where the selected item loads, session terminals embedded.
 * Global oversight (Overview, Accounts, Usage, Fleet) sits top-right.
 */

const GLOBAL_NAV = [
  { id: "overview", label: "Overview" },
  { id: "accounts", label: "Accounts" },
  { id: "usage", label: "Usage" },
  { id: "fleet", label: "Fleet" },
] as const;

type GlobalId = (typeof GLOBAL_NAV)[number]["id"];

type Selection =
  | { kind: "none" }
  | { kind: "project"; id: string }
  | { kind: "session"; id: string }
  | { kind: "login"; id: string };

export default function App() {
  const [tab, setTab] = useState<WorkTab>("projects");
  const [railCollapsed, setRailCollapsed] = useState(
    () => localStorage.getItem("aij.rail.collapsed") === "1",
  );
  const toggleRail = useCallback(() => {
    setRailCollapsed((was) => {
      localStorage.setItem("aij.rail.collapsed", was ? "0" : "1");
      return !was;
    });
  }, []);
  const [selection, setSelection] = useState<Selection>({ kind: "none" });
  const [global, setGlobal] = useState<GlobalId | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loginTerminals, setLoginTerminals] = useState<
    { id: string; title: string; ended: boolean }[]
  >([]);
  const [newSession, setNewSession] = useState(false);

  useEffect(() => {
    void fetchAppInfo().then(setInfo);
    return onAppReady(setInfo);
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [projectRows, sessionRows] = await Promise.all([projectsList(), sessionsList()]);
      setProjects(projectRows);
      setSessions(sessionRows);
    } catch {
      // preview mode may lack backend mocks for some calls — keep last data
    }
  }, []);

  useEffect(() => {
    void refresh();
    return onSessionsUpdated(() => void refresh());
  }, [refresh]);

  // Login terminals end → mark ended (kept for scrollback until closed).
  useEffect(() => {
    return onTerminalExit((id) => {
      setLoginTerminals((current) =>
        current.map((t) => (t.id === id ? { ...t, ended: true } : t)),
      );
      void refresh();
    });
  }, [refresh]);

  const selectSession = useCallback((id: string) => {
    setGlobal(null);
    setSelection({ kind: "session", id });
  }, []);

  const selectProject = useCallback((id: string) => {
    setGlobal(null);
    setSelection({ kind: "project", id });
  }, []);

  const selectLogin = useCallback((id: string) => {
    setGlobal(null);
    setSelection({ kind: "login", id });
  }, []);

  // Login flows (Accounts screen) hand their terminal here; it joins the
  // Sessions rail list and gets selected.
  const openLoginTerminal = useCallback((id: string, title: string) => {
    setLoginTerminals((current) =>
      current.some((t) => t.id === id) ? current : [...current, { id, title, ended: false }],
    );
    setTab("sessions");
    setSelection({ kind: "login", id });
  }, []);

  const selectedSession = useMemo(
    () =>
      selection.kind === "session"
        ? (sessions.find((s) => s.id === selection.id) ?? null)
        : null,
    [selection, sessions],
  );
  const selectedProject = useMemo(
    () =>
      selection.kind === "project"
        ? (projects.find((p) => p.id === selection.id) ?? null)
        : null,
    [selection, projects],
  );
  const selectedLogin = useMemo(
    () =>
      selection.kind === "login"
        ? (loginTerminals.find((t) => t.id === selection.id) ?? null)
        : null,
    [selection, loginTerminals],
  );
  const sessionProject = useMemo(
    () => projects.find((p) => p.id === selectedSession?.project_id) ?? null,
    [projects, selectedSession],
  );

  // ⌘B toggles the rail, ⌘N starts a new session.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "b") {
        event.preventDefault();
        toggleRail();
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "n") {
        event.preventDefault();
        setNewSession(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const contextTitle =
    global !== null
      ? (GLOBAL_NAV.find((g) => g.id === global)?.label ?? "")
      : selection.kind === "project"
        ? (selectedProject?.name ?? "Project")
        : selectedSession
          ? selectedSession.label
          : selectedLogin
            ? selectedLogin.title
            : tab === "projects"
              ? "Projects"
              : tab === "instances"
                ? "Instances"
                : "Sessions";

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <WorkflowRail
        tab={tab}
        onTab={setTab}
        collapsed={railCollapsed}
        onToggleCollapsed={toggleRail}
        projects={projects}
        sessions={sessions}
        loginTerminals={loginTerminals}
        selectedId={selection.kind === "none" ? null : selection.id}
        onSelectSession={(id) =>
          loginTerminals.some((t) => t.id === id) ? selectLogin(id) : selectSession(id)
        }
        onSelectProject={selectProject}
        onNewSession={() => setNewSession(true)}
        header={(collapsed) => (
          <div
            className={
              "flex h-14 shrink-0 items-center gap-2.5 overflow-hidden " +
              (collapsed ? "justify-center px-0" : "px-4")
            }
          >
            <button
              type="button"
              title={collapsed ? "Expand (⌘B)" : "Collapse (⌘B)"}
              onClick={toggleRail}
              className="shrink-0 rounded-md p-1 text-accent hover:bg-surface"
            >
              <span className="block drop-shadow-[0_0_10px_#7c7cf466]">
                <IconJuggler width={22} height={22} />
              </span>
            </button>
            {!collapsed && (
              <span className="whitespace-nowrap text-[15px] font-semibold tracking-tight">
                AI Juggler
              </span>
            )}
          </div>
        )}
        footer={(collapsed) =>
          collapsed ? (
            <div className="flex justify-center py-2" title="this-mac · healthy">
              <span className="dot-live size-1.5 rounded-full bg-success" />
            </div>
          ) : (
            <div className="flex flex-col gap-1.5 px-3 py-2.5">
              <div className="flex items-center gap-2">
                <span className="dot-live size-1.5 shrink-0 rounded-full bg-success" />
                <span className="truncate text-[11px]">this-mac · healthy</span>
              </div>
              <div className="truncate font-mono text-[10px] text-faint">
                v{info?.version ?? "0.0.0"}
              </div>
            </div>
          )
        }
      />

      {/* Main column */}
      <div className="flex min-w-0 flex-1 flex-col">
        {/* Glass status bar: work context left, global oversight right */}
        <header
          data-tauri-drag-region
          className="flex h-14 shrink-0 items-center gap-3 border-b border-hairline bg-surface/50 px-5 backdrop-blur-xl"
        >
          <h1 className="text-sm font-semibold capitalize tracking-wide">{contextTitle}</h1>
          <Chip tone="accent">Claude Code · Codex · ZCode</Chip>

          <div className="ml-auto flex items-center gap-1.5" data-tauri-drag-region={false}>
            {GLOBAL_NAV.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setGlobal(item.id)}
                className={
                  "rounded-md px-2.5 py-1.5 text-[11px] font-medium transition-colors " +
                  (global === item.id
                    ? "bg-surface text-ink sheen"
                    : "text-muted hover:bg-surface/60 hover:text-ink")
                }
              >
                {item.label}
              </button>
            ))}
            <Button size="sm" variant="primary" onClick={() => setNewSession(true)}>
              <IconTerminal width={14} height={14} />
              New
            </Button>
          </div>
        </header>

        {/* Content: detail panel (work mode) or global screen */}
        <main className="min-h-0 flex-1 overflow-hidden p-5">
          {global === "accounts" ? (
            <div className="h-full overflow-y-auto">
              <ProfilesScreen openTerminal={openLoginTerminal} />
            </div>
          ) : global === "overview" ? (
            <div className="h-full overflow-y-auto">
              <Overview sessions={sessions} projects={projects} />
            </div>
          ) : global === "usage" ? (
            <ComingSoon screen="Usage" />
          ) : global === "fleet" ? (
            <ComingSoon screen="Fleet" />
          ) : selectedProject ? (
            <ProjectPanel
              project={selectedProject}
              sessions={sessions}
              onOpenSession={selectSession}
              onLaunch={async (profile, project) => {
                const result = await launchSession(profile.id, project.root);
                if (result) {
                  void refresh();
                  selectSession(result.session_id);
                }
              }}
            />
          ) : selectedSession || selectedLogin ? (
            <SessionPanel
              session={selectedSession}
              project={sessionProject}
              loginTerminal={selectedLogin}
              onOpenProject={selectProject}
              onStopped={() => void refresh()}
            />
          ) : (
            <WorkEmpty onNew={() => setNewSession(true)} />
          )}
        </main>
      </div>

      {newSession && (
        <NewSessionModal
          onClose={() => setNewSession(false)}
          onLaunched={(sessionId) => {
            setNewSession(false);
            setTab("sessions");
            void refresh();
            selectSession(sessionId);
          }}
        />
      )}
    </div>
  );
}

function Overview({
  sessions,
  projects,
}: {
  sessions: SessionInfo[];
  projects: ProjectInfo[];
}) {
  const [stats, setStats] = useState<StatsInfo | null>(null);
  useEffect(() => {
    void dashboardStats().then(setStats);
  }, []);
  void stats;
  const live = sessions.filter((s) => s.status === "running" || s.status === "starting");

  return (
    <div className="flex flex-col gap-6">
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <StatCard
          label="Live instances"
          value={String(live.length)}
          hint={`${sessions.length} total recorded`}
          tone="live"
          icon={<IconTerminal width={15} height={15} />}
        />
        <StatCard
          label="Projects"
          value={String(projects.length)}
          hint="directories juggled"
          icon={<IconJuggler width={15} height={15} />}
        />
        <StatCard
          label="Machine"
          value="OK"
          hint="RAM · CPU · disk nominal (live gauges: M3)"
          tone="success"
          icon={<IconFleet width={15} height={15} />}
        />
        <StatCard
          label="Fleet"
          value="1"
          hint="this-mac only"
          icon={<IconFleet width={15} height={15} />}
        />
      </div>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
        <Card className="p-5 xl:col-span-2">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold">Live instances</h2>
              <p className="text-xs text-muted">Select one in the rail to open its panel</p>
            </div>
            <Chip tone="neutral" dot>
              {live.length} live
            </Chip>
          </div>
          {live.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-hairline py-10">
              <span className="flex size-10 items-center justify-center rounded-xl border border-accent/20 bg-accent-soft text-accent-hi">
                <IconTerminal width={19} height={19} />
              </span>
              <p className="text-sm text-muted">
                Nothing running — start one from the rail's New session.
              </p>
            </div>
          ) : (
            <div className="flex flex-col divide-y divide-hairline/50">
              {live.map((s) => (
                <div key={s.id} className="flex items-center gap-3 py-2.5 text-xs">
                  <span className="dot-live size-1.5 rounded-full bg-live" />
                  <span className="font-medium">{s.label}</span>
                  <span className="text-faint">{s.kind}</span>
                  <Chip tone="live">{s.status}</Chip>
                </div>
              ))}
            </div>
          )}
        </Card>

        <Card className="flex flex-col items-center justify-center gap-3 p-5">
          <h2 className="self-start text-sm font-semibold">Quota windows</h2>
          <div className="flex items-center justify-around gap-4 pt-2">
            <QuotaRing percent={0} label="5h" />
            <QuotaRing percent={0} label="week" />
          </div>
          <p className="text-center text-[11px] text-faint">
            Rings fill per account once profiles exist
          </p>
        </Card>
      </div>
    </div>
  );
}

function ComingSoon({ screen }: { screen: string }) {
  return (
    <Card className="flex h-full flex-col items-center justify-center gap-2 py-16">
      <span className="text-sm font-medium">{screen}</span>
      <span className="flex items-center gap-1.5 text-xs text-muted">
        {screen === "Usage" ? (
          <IconUsage width={13} height={13} />
        ) : (
          <IconFleet width={13} height={13} />
        )}
        This screen lands in the M2+ build-out — see docs/plan/EPICS.md
      </span>
    </Card>
  );
}
