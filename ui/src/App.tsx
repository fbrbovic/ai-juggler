import { useCallback, useEffect, useState } from "react";
import {
  Button,
  Card,
  Chip,
  QuotaRing,
  StatCard,
} from "./components/primitives";
import { Rail } from "./components/rail";
import { TerminalDock, type TerminalTab } from "./components/TerminalDock";
import {
  IconFleet,
  IconJuggler,
  IconProfiles,
  IconProjects,
  IconSessions,
  IconTerminal,
  IconUsage,
} from "./components/icons";
import { ProfilesScreen } from "./screens/Profiles";
import { ProjectsScreen } from "./screens/Projects";
import { InstancesScreen } from "./screens/Instances";
import { SessionsScreen } from "./screens/SessionsScreen";
import {
  dashboardStats,
  fetchAppInfo,
  onAppReady,
  type AppInfo,
  type StatsInfo,
} from "./lib/bridge";

/**
 * App shell (ADR-0007): two modes.
 * - Work mode — the left rail: Projects (juggling anchor), Instances,
 *   Sessions.
 * - Global mode — top-right menus: Overview, Accounts, Usage, Fleet.
 * The terminal dock is an execution surface at the bottom, not navigation.
 */

const WORK_NAV = [
  { id: "projects", label: "Projects", icon: <IconProjects /> },
  { id: "instances", label: "Instances", icon: <IconTerminal /> },
  { id: "sessions", label: "Sessions", icon: <IconSessions /> },
] as const;

const GLOBAL_NAV = [
  { id: "overview", label: "Overview" },
  { id: "accounts", label: "Accounts" },
  { id: "usage", label: "Usage" },
  { id: "fleet", label: "Fleet" },
] as const;

type WorkId = (typeof WORK_NAV)[number]["id"];
type GlobalId = (typeof GLOBAL_NAV)[number]["id"];

type View = { kind: "work"; id: WorkId } | { kind: "global"; id: GlobalId };

export default function App() {
  const [view, setView] = useState<View>({ kind: "work", id: "projects" });
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [tabs, setTabs] = useState<TerminalTab[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null);
  const [selectedInstance, setSelectedInstance] = useState<string | null>(null);

  useEffect(() => {
    void fetchAppInfo().then(setInfo);
    return onAppReady(setInfo);
  }, []);

  const openTerminal = useCallback((id: string, title: string) => {
    setTabs((current) =>
      current.some((tab) => tab.id === id) ? current : [...current, { id, title }],
    );
    setActiveTab(id);
  }, []);

  const closeTerminal = useCallback((id: string) => {
    setTabs((current) => {
      const next = current.filter((tab) => tab.id !== id);
      setActiveTab((active) => (active === id ? (next.at(-1)?.id ?? null) : active));
      return next;
    });
  }, []);

  const markExited = useCallback((id: string) => {
    setTabs((current) =>
      current.map((tab) => (tab.id === id ? { ...tab, ended: true } : tab)),
    );
  }, []);

  const workActive = view.kind === "work" ? view.id : null;
  const globalActive = view.kind === "global" ? view.id : null;
  const contextTitle =
    view.kind === "global"
      ? (GLOBAL_NAV.find((g) => g.id === view.id)?.label ?? "")
      : (WORK_NAV.find((w) => w.id === view.id)?.label ?? "");

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Rail
        items={[...WORK_NAV]}
        active={workActive ?? ""}
        onSelect={(id) => {
          setSelectedInstance(null);
          setView({ kind: "work", id: id as WorkId });
        }}
        header={(collapsed) => (
          <div
            className={`flex h-14 items-center gap-2.5 overflow-hidden ${
              collapsed ? "justify-center px-0" : "px-4"
            }`}
          >
            <span className="shrink-0 text-accent drop-shadow-[0_0_10px_#7c7cf466]">
              <IconJuggler width={22} height={22} />
            </span>
            {!collapsed && (
              <span className="whitespace-nowrap text-[15px] font-semibold tracking-tight">
                AI Juggler
              </span>
            )}
          </div>
        )}
        footer={(collapsed) =>
          collapsed ? (
            <div className="flex justify-center" title="this-mac · healthy">
              <span className="dot-live size-1.5 rounded-full bg-success" />
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-2">
                <span className="dot-live size-1.5 shrink-0 rounded-full bg-success" />
                <span className="truncate">this-mac · healthy</span>
              </div>
              <div className="truncate font-mono">v{info?.version ?? "0.0.0"}</div>
            </div>
          )
        }
      />

      {/* Main column */}
      <div className="flex min-w-0 flex-1 flex-col">
        {/* Glass status bar: work context left, global mode right */}
        <header
          data-tauri-drag-region
          className="flex h-14 shrink-0 items-center gap-3 border-b border-hairline
            bg-surface/50 pl-[92px] pr-5 backdrop-blur-xl"
        >
          <h1 className="text-sm font-semibold capitalize tracking-wide">{contextTitle}</h1>
          <Chip tone="accent">Claude Code · Codex · ZCode</Chip>

          <div className="ml-auto flex items-center gap-1.5" data-tauri-drag-region={false}>
            {GLOBAL_NAV.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setView({ kind: "global", id: item.id })}
                className={
                  "rounded-md px-2.5 py-1.5 text-[11px] font-medium transition-colors " +
                  (globalActive === item.id
                    ? "bg-surface text-ink sheen"
                    : "text-muted hover:bg-surface/60 hover:text-ink")
                }
              >
                {item.label}
              </button>
            ))}
            <Button
              size="sm"
              variant="primary"
              onClick={() => setView({ kind: "global", id: "accounts" })}
            >
              <IconProfiles width={14} height={14} />
              Add
            </Button>
          </div>
        </header>

        {/* Content */}
        <main className="min-h-0 flex-1 overflow-y-auto p-6">
          {view.kind === "work" && view.id === "projects" ? (
            <ProjectsScreen
              openTerminal={openTerminal}
              onOpenInstances={() => setView({ kind: "work", id: "instances" })}
            />
          ) : view.kind === "work" && view.id === "instances" ? (
            <InstancesScreen
              selectedId={selectedInstance}
              onSelect={setSelectedInstance}
              openTerminal={openTerminal}
            />
          ) : view.kind === "work" && view.id === "sessions" ? (
            <SessionsScreen />
          ) : view.kind === "global" && view.id === "overview" ? (
            <Overview />
          ) : view.kind === "global" && view.id === "accounts" ? (
            <ProfilesScreen openTerminal={openTerminal} />
          ) : view.kind === "global" && view.id === "usage" ? (
            <ComingSoon screen="Usage" />
          ) : (
            <ComingSoon screen="Fleet" />
          )}
        </main>

        <TerminalDock
          tabs={tabs}
          activeId={activeTab}
          onSelect={setActiveTab}
          onClose={closeTerminal}
          onExit={markExited}
        />
      </div>
    </div>
  );
}

function Overview() {
  const [stats, setStats] = useState<StatsInfo | null>(null);
  useEffect(() => {
    void dashboardStats().then(setStats);
  }, []);

  return (
    <div className="flex flex-col gap-6">
      {/* Health strip */}
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <StatCard
          label="Live instances"
          value={String(stats?.active_sessions ?? 0)}
          hint="across all accounts"
          tone="live"
          icon={<IconTerminal width={15} height={15} />}
        />
        <StatCard
          label="Accounts"
          value={String(stats?.accounts ?? 0)}
          hint={`${stats?.profiles ?? 0} profiles`}
          icon={<IconProfiles width={15} height={15} />}
        />
        <StatCard
          label="Machine"
          value="OK"
          hint="RAM · CPU · disk nominal (live gauges: M3)"
          tone="success"
          icon={<IconJuggler width={15} height={15} />}
        />
        <StatCard
          label="Fleet"
          value="1"
          hint="this-mac only"
          icon={<IconFleet width={15} height={15} />}
        />
      </div>

      {/* Quota + getting-started row */}
      <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
        <Card className="p-5 xl:col-span-2">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold">Live instances</h2>
              <p className="text-xs text-muted">
                Open Instances in the rail for the full board with actions
              </p>
            </div>
            <Chip tone="neutral" dot>
              mission control
            </Chip>
          </div>
          <EmptyState />
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

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-hairline py-12">
      <span className="flex size-11 items-center justify-center rounded-xl border border-accent/20 bg-accent-soft text-accent-hi">
        <IconProjects width={22} height={22} />
      </span>
      <p className="max-w-sm text-center text-sm text-muted">
        Launch a profile into any directory — the project, its instance, and
        the session land in the rail views.
      </p>
    </div>
  );
}

function ComingSoon({ screen }: { screen: string }) {
  return (
    <Card className="flex flex-col items-center justify-center gap-2 py-16">
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
