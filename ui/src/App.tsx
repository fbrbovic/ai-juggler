import { useEffect, useState } from "react";
import {
  Button,
  Card,
  Chip,
  QuotaRing,
  StatCard,
} from "./components/primitives";
import { Rail } from "./components/rail";
import {
  IconDashboard,
  IconFleet,
  IconJuggler,
  IconProfiles,
  IconSessions,
  IconTerminal,
  IconUsage,
} from "./components/icons";
import { ProfilesScreen } from "./screens/Profiles";
import {
  dashboardStats,
  fetchAppInfo,
  onAppReady,
  type AppInfo,
  type StatsInfo,
} from "./lib/bridge";

/**
 * App shell: collapsible/resizable rail, floating-glass status bar over an
 * overlay title bar (native macOS traffic lights), content area.
 * Design-direction scaffold — screens land in M1-3+.
 */

const NAV = [
  { id: "dashboard", label: "Dashboard", icon: <IconDashboard /> },
  { id: "sessions", label: "Sessions", icon: <IconSessions /> },
  { id: "profiles", label: "Profiles", icon: <IconProfiles /> },
  { id: "usage", label: "Usage", icon: <IconUsage /> },
  { id: "fleet", label: "Fleet", icon: <IconFleet /> },
] as const;

type NavId = (typeof NAV)[number]["id"];

export default function App() {
  const [nav, setNav] = useState<NavId>("dashboard");
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    void fetchAppInfo().then(setInfo);
    return onAppReady(setInfo);
  }, []);

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Rail
        items={[...NAV]}
        active={nav}
        onSelect={(id) => setNav(id as NavId)}
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
        {/* Glass status bar over the overlay title bar (traffic lights sit in
            the left gutter; the empty bar area drags the window). */}
        <header
          data-tauri-drag-region
          className="flex h-14 shrink-0 items-center gap-3 border-b border-hairline
            bg-surface/50 pl-[92px] pr-6 backdrop-blur-xl"
        >
          <h1 className="text-sm font-semibold capitalize tracking-wide">
            {nav}
          </h1>
          <span className="text-faint">/</span>
          <Chip tone="accent">Claude Code · Codex · ZCode</Chip>
          <div className="ml-auto flex items-center gap-2">
            <Chip tone="neutral">machine: this-mac</Chip>
            <Button size="sm" variant="primary">
              <IconTerminal width={14} height={14} />
              New session
            </Button>
          </div>
        </header>

        {/* Content */}
        <main className="min-h-0 flex-1 overflow-y-auto p-6">
          {nav === "dashboard" ? <Dashboard /> : nav === "profiles" ? <ProfilesScreen /> : <ComingSoon screen={nav} />}
        </main>
      </div>
    </div>
  );
}

function Dashboard() {
  const [stats, setStats] = useState<StatsInfo | null>(null);
  useEffect(() => {
    void dashboardStats().then(setStats);
  }, []);

  return (
    <div className="flex flex-col gap-6">
      {/* Health strip */}
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <StatCard
          label="Profiles"
          value={String(stats?.profiles ?? 0)}
          hint={`${stats?.accounts ?? 0} accounts`}
          icon={<IconProfiles width={15} height={15} />}
        />
        <StatCard
          label="Active sessions"
          value={String(stats?.active_sessions ?? 0)}
          hint="live across all harnesses"
          tone="live"
          icon={<IconTerminal width={15} height={15} />}
        />
        <StatCard
          label="Machine"
          value="OK"
          hint="RAM · CPU · disk nominal"
          tone="success"
          icon={<IconDashboard width={15} height={15} />}
        />
        <StatCard
          label="Fleet"
          value="1"
          hint="this-mac only"
          icon={<IconFleet width={15} height={15} />}
        />
      </div>

      {/* Quota + empty state row */}
      <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
        <Card className="p-5 xl:col-span-2">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold">Sessions</h2>
              <p className="text-xs text-muted">
                Live board across every harness and account
              </p>
            </div>
            <Chip tone="neutral" dot>
              waiting for first profile
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
        <IconProfiles width={22} height={22} />
      </span>
      <p className="text-sm text-muted">
        No profiles yet — connect your first harness account to start juggling.
      </p>
      <Button variant="primary" size="sm">
        Add account
      </Button>
    </div>
  );
}

function ComingSoon({ screen }: { screen: string }) {
  return (
    <Card className="flex flex-col items-center justify-center gap-2 py-16">
      <span className="text-sm font-medium capitalize">{screen}</span>
      <span className="text-xs text-muted">
        This screen lands in the M1 build-out — see docs/plan/EPICS.md
      </span>
    </Card>
  );
}
