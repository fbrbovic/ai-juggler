import { useEffect, useState } from "react";
import {
  Button,
  Card,
  Chip,
  QuotaRing,
  StatCard,
} from "./components/primitives";
import {
  IconDashboard,
  IconFleet,
  IconJuggler,
  IconProfiles,
  IconSessions,
  IconTerminal,
  IconUsage,
} from "./components/icons";
import { fetchAppInfo, onAppReady, type AppInfo } from "./lib/bridge";

/**
 * App shell: left rail navigation, top status bar, content area.
 * This is the M1-1 design-direction scaffold — screens land in M1-3+.
 */

const NAV = [
  { id: "dashboard", label: "Dashboard", icon: IconDashboard },
  { id: "sessions", label: "Sessions", icon: IconSessions },
  { id: "profiles", label: "Profiles", icon: IconProfiles },
  { id: "usage", label: "Usage", icon: IconUsage },
  { id: "fleet", label: "Fleet", icon: IconFleet },
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
    <div className="flex h-screen w-screen overflow-hidden bg-canvas">
      {/* Left rail */}
      <nav className="flex w-[232px] shrink-0 flex-col border-r border-hairline bg-surface">
        <div className="flex h-14 items-center gap-2.5 px-5">
          <span className="text-accent">
            <IconJuggler width={22} height={22} />
          </span>
          <span className="text-[15px] font-semibold tracking-tight">
            AI Juggler
          </span>
        </div>
        <div className="flex flex-col gap-0.5 px-3 pt-2">
          {NAV.map((item) => (
            <button
              key={item.id}
              onClick={() => setNav(item.id)}
              className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm
                transition-colors cursor-default ${
                  nav === item.id
                    ? "bg-accent-soft text-ink"
                    : "text-muted hover:bg-surface-2 hover:text-ink"
                }`}
            >
              <item.icon />
              {item.label}
            </button>
          ))}
        </div>
        <div className="mt-auto flex flex-col gap-2 p-4 text-[11px] text-faint">
          <div className="flex items-center gap-2">
            <span className="size-1.5 rounded-full bg-success" />
            this-mac · healthy
          </div>
          <div className="font-mono">v{info?.version ?? "0.0.0"}</div>
        </div>
      </nav>

      {/* Main column */}
      <div className="flex min-w-0 flex-1 flex-col">
        {/* Top bar */}
        <header className="flex h-14 shrink-0 items-center gap-3 border-b border-hairline bg-surface/60 px-6 backdrop-blur">
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
          {nav === "dashboard" ? <Dashboard /> : <ComingSoon screen={nav} />}
        </main>
      </div>
    </div>
  );
}

function Dashboard() {
  return (
    <div className="flex flex-col gap-6">
      {/* Health strip */}
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <StatCard label="Profiles" value="0" hint="no accounts yet" />
        <StatCard label="Active sessions" value="0" hint="nothing running" />
        <StatCard
          label="Machine"
          value="OK"
          hint="RAM · CPU · disk nominal"
          tone="success"
        />
        <StatCard label="Fleet" value="1" hint="this-mac only" />
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
      <span className="text-faint">
        <IconProfiles width={28} height={28} />
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
