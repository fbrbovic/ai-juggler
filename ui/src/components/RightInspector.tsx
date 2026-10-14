import { useEffect, useRef, useState, type ReactNode } from "react";
import { Button, Card, Chip, type ChipTone } from "./primitives";
import { TerminalView } from "./TerminalView";
import {
  IconFleet,
  IconJuggler,
  IconProfiles,
  IconProjects,
  IconSessions,
  IconTerminal,
} from "./icons";
import type { AccountInfo, InstanceInfo, LaunchRecipeInfo, ProjectInfo } from "../lib/bridge";
import { quotaTierTag } from "./primitives";
import { formatBytes, stateChip } from "./indicator";

/**
 * RightInspector (AJ-E406, D-06): the one stable right-hand home. Six
 * tabs — Overview · Usage · History · Quality / Performance · Processes
 * · Actions — exist for EVERY selection type (instance, session,
 * provider, account, project) from day one; a tab without measurable
 * data renders its honest empty state with the owning task, never a
 * hidden or disabled tab. Tab state persists per selection type; the
 * embedded terminal NEVER unmounts on tab switches (panels toggle
 * `hidden`, they do not remount).
 */

export type InspectorKind = "instance" | "session" | "provider" | "account" | "project";

export type InspectorSelection =
  | { kind: "none" }
  | { kind: "login"; id: string }
  | { kind: InspectorKind; id: string };

export const INSPECTOR_TABS = [
  { id: "overview", label: "Overview" },
  { id: "usage", label: "Usage" },
  { id: "history", label: "History" },
  { id: "quality", label: "Quality / Performance" },
  { id: "processes", label: "Processes" },
  { id: "actions", label: "Actions" },
] as const;

export type InspectorTab = (typeof INSPECTOR_TABS)[number]["id"];

/** Real data source per tab (footer contract, Req.5). */
export const TAB_SOURCES: Record<InspectorTab, string> = {
  overview: "supervisor",
  usage: "usage-engine",
  history: "transcript",
  quality: "supervisor",
  processes: "supervisor",
  actions: "supervisor",
};

const TAB_KEY = (kind: InspectorKind) => `aij.inspector.tab.${kind}`;

function readTab(kind: InspectorKind): InspectorTab {
  const raw = localStorage.getItem(TAB_KEY(kind));
  return INSPECTOR_TABS.some((tab) => tab.id === raw) ? (raw as InspectorTab) : "overview";
}

/** The honest empty state (D-06): visible, explained, never hidden. */
function HonestEmpty({ tab, task }: { tab: InspectorTab; task: string }) {
  const copy: Record<InspectorTab, string> = {
    overview: "Overview fields appear as their sources land.",
    usage: "No measured usage yet — the two-clock usage engine feeds this tab.",
    history: "No history yet — transcript discovery feeds this tab.",
    quality: "No measurable quality signals yet — no invented scores, ever (D-07).",
    processes: "No process inventory yet — the verified process collector feeds this tab.",
    actions: "No actions available for this selection yet.",
  };
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 py-10 text-center">
      <p className="text-[12px] font-medium text-muted">Unavailable</p>
      <p className="max-w-xs text-[11px] leading-relaxed text-muted">{copy[tab]}</p>
      <p className="text-[10px] text-muted">lands with {task}</p>
    </div>
  );
}

function statusTone(status: string): { tone: ChipTone; label: string } {
  const chip = stateChip(status);
  const tone: ChipTone =
    chip.tone === "live"
      ? "live"
      : chip.tone === "success"
        ? "success"
        : chip.tone === "warning"
          ? "warning"
          : chip.tone === "danger"
            ? "danger"
            : chip.tone === "accent"
              ? "accent"
              : "neutral";
  return { tone, label: chip.label };
}

function since(ms: number, now: number): string {
  const secs = Math.max(1, Math.floor((now - ms) / 1000));
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}h`;
}

function KindIcon({ kind }: { kind: InspectorKind }) {
  const props = { width: 16, height: 16 };
  switch (kind) {
    case "instance":
      return <IconTerminal {...props} />;
    case "session":
      return <IconSessions {...props} />;
    case "provider":
      return <IconJuggler {...props} />;
    case "account":
      return <IconProfiles {...props} />;
    case "project":
      return <IconProjects {...props} />;
  }
}

export const KIND_LABELS: Record<InspectorKind, string> = {
  instance: "Instance",
  session: "Session",
  provider: "Provider",
  account: "Account",
  project: "Project",
};

/** One resolved selection (the caller owns lookup). */
export type InspectorData = {
  instance?: InstanceInfo | null;
  project?: ProjectInfo | null;
  account?: AccountInfo | null;
  provider?: { id: string; name: string } | null;
  machineId?: string | null;
};

export function RightInspector({
  selection,
  data,
  sessions,
  projects,
  accounts,
  profiles,
  terminalTitle,
  freshness,
  now,
  primaryAction,
  onOpenProject,
  onOpenSession,
  onLaunch,
  onStopped,
  onHandover,
  onNew,
}: {
  selection: InspectorSelection;
  data: InspectorData;
  sessions: InstanceInfo[];
  projects: ProjectInfo[];
  accounts: AccountInfo[];
  profiles: LaunchRecipeInfo[];
  terminalTitle: string | null;
  freshness: Record<string, { resources: number; quota: number }>;
  now: number;
  primaryAction?: ReactNode;
  onOpenProject: (id: string) => void;
  onOpenSession: (id: string) => void;
  onLaunch: (recipe: LaunchRecipeInfo, project: ProjectInfo) => void;
  onStopped: () => void;
  onHandover: (sessionId: string, targetProfileId: string) => void;
  onNew: () => void;
}) {
  const kind: InspectorKind | "none" | "login" = selection.kind;
  const [tab, setTab] = useState<InspectorTab>(() =>
    kind === "none" || kind === "login" ? "overview" : readTab(kind),
  );
  const tabsRef = useRef<HTMLDivElement | null>(null);

  // Switching selection restores that type's last tab (Req.3).
  useEffect(() => {
    if (kind !== "none" && kind !== "login") setTab(readTab(kind));
  }, [kind]);

  const chooseTab = (next: InspectorTab) => {
    setTab(next);
    if (kind !== "none" && kind !== "login") localStorage.setItem(TAB_KEY(kind), next);
  };

  // Cmd/Ctrl+1..6 switches tabs (Req.6). Focus rings stay visible; the
  // tab strip supports arrow-key roving.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.shiftKey || event.altKey) return;
      const index = Number(event.key);
      if (index >= 1 && index <= INSPECTOR_TABS.length) {
        event.preventDefault();
        chooseTab(INSPECTOR_TABS[index - 1]!.id);
        const buttons = tabsRef.current?.querySelectorAll<HTMLButtonElement>("[role='tab']");
        buttons?.[index - 1]?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const headerName =
    kind === "instance"
      ? (data.instance?.account_label ?? "Instance")
      : kind === "session"
        ? "Conversation"
        : kind === "provider"
          ? (data.provider?.name ?? "Provider")
          : kind === "account"
            ? (data.account?.label ?? "Account")
            : kind === "project"
              ? (data.project?.name ?? "Project")
              : "Fleet";
  const stateChipNode =
    kind === "instance" && data.instance ? (
      (() => {
        const view = statusTone(data.instance.status);
        return <Chip tone={view.tone}>{view.label}</Chip>;
      })()
    ) : null;

  if (kind === "none") {
    return <FleetAtAGlance sessions={sessions} accounts={accounts} now={now} onNew={onNew} />;
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3" data-testid="right-inspector" data-inspector-kind={kind}>
      {/* Header: type icon, name, I-04 state chip, primary action (Req.1). */}
      <header className="flex flex-wrap items-center gap-3">
        <span className="text-accent-hi" title={KIND_LABELS[kind as InspectorKind]}>
          <KindIcon kind={kind as InspectorKind} />
        </span>
        <h2 className="text-lg font-semibold tracking-tight">{headerName}</h2>
        {stateChipNode}
        {/* Fleet-shaped (Req.7): remote selections carry a machine chip. */}
        {data.machineId && data.machineId !== "this-mac" && (
          <Chip tone="accent">
            <IconFleet width={10} height={10} /> {data.machineId}
          </Chip>
        )}
        <div className="ml-auto flex items-center gap-2">{primaryAction ?? null}</div>
      </header>

      {/* The fixed six tabs (D-06): never hidden, never reordered. */}
      <div ref={tabsRef} role="tablist" aria-label={`${KIND_LABELS[kind as InspectorKind]} inspector tabs`} className="flex flex-wrap gap-1 border-b border-hairline pb-2">
        {INSPECTOR_TABS.map((entry, index) => (
          <button
            key={entry.id}
            type="button"
            role="tab"
            id={`inspector-tab-${entry.id}`}
            aria-selected={tab === entry.id}
            aria-controls={`inspector-panel-${entry.id}`}
            tabIndex={tab === entry.id ? 0 : -1}
            onClick={() => chooseTab(entry.id)}
            onKeyDown={(event) => {
              if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
                event.preventDefault();
                const delta = event.key === "ArrowRight" ? 1 : -1;
                const next = (INSPECTOR_TABS.indexOf(INSPECTOR_TABS.find((t) => t.id === tab)!) + delta + INSPECTOR_TABS.length) % INSPECTOR_TABS.length;
                chooseTab(INSPECTOR_TABS[next]!.id);
                tabsRef.current?.querySelectorAll<HTMLButtonElement>(`[role='tab']`)[next]?.focus();
              }
            }}
            className={
              "rounded-lg px-2.5 py-1 text-[11.5px] font-medium outline-none transition-colors focus-visible:ring-1 focus-visible:ring-accent/60 " +
              (tab === entry.id
                ? "bg-accent-soft text-accent-hi"
                : "text-muted hover:bg-surface hover:text-ink") +
              (index < 5 ? " hidden sm:inline-flex" : "") // Actions stays; guard tiny widths
            }
          >
            {entry.label}
          </button>
        ))}
      </div>

      {/* Panels toggle `hidden` — the terminal never unmounts (gate). */}
      <div className="flex min-h-0 flex-1 flex-col">
        {INSPECTOR_TABS.map((entry) => (
          <section
            key={entry.id}
            role="tabpanel"
            id={`inspector-panel-${entry.id}`}
            aria-labelledby={`inspector-tab-${entry.id}`}
            hidden={tab !== entry.id}
            className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto"
          >
            <InspectorPanel
              tab={entry.id}
              kind={kind as InspectorKind}
              data={data}
              sessions={sessions}
              projects={projects}
              accounts={accounts}
              profiles={profiles}
              terminalTitle={terminalTitle}
              onOpenProject={onOpenProject}
              onOpenSession={onOpenSession}
              onLaunch={onLaunch}
              onStopped={onStopped}
              onHandover={onHandover}
            />
            {/* Footer: updated-ago + the field's REAL source (Req.5). */}
            <footer className="mt-auto flex items-center gap-1.5 border-t border-hairline/60 pt-2 text-[10px] text-faint">
              <span>
                Updated {since(freshness[data.instance?.id ?? ""]?.resources ?? now, now)} ago
              </span>
              <span aria-hidden="true">·</span>
              <span>source: {TAB_SOURCES[entry.id]}</span>
            </footer>
          </section>
        ))}
      </div>
    </div>
  );
}

function InspectorPanel(props: {
  tab: InspectorTab;
  kind: InspectorKind;
  data: InspectorData;
  sessions: InstanceInfo[];
  projects: ProjectInfo[];
  accounts: AccountInfo[];
  profiles: LaunchRecipeInfo[];
  terminalTitle: string | null;
  onOpenProject: (id: string) => void;
  onOpenSession: (id: string) => void;
  onLaunch: (recipe: LaunchRecipeInfo, project: ProjectInfo) => void;
  onStopped: () => void;
  onHandover: (sessionId: string, targetProfileId: string) => void;
}) {
  const { tab, kind } = props;
  if (tab === "overview") {
    return <OverviewPanel {...props} />;
  }
  // Honest empties for the tabs whose sources land later (D-06):
  // visible with the owning task, never hidden or disabled.
  const owner: Record<InspectorTab, string> = {
    overview: "AJ-E407",
    usage: "AJ-E606+ (two-clock usage engine)",
    history: "AJ-E407 (transcript discovery)",
    quality: "AJ-E409 (measurable signals, no score)",
    processes: "AJ-E410 + E5 (verified process collector)",
    actions: kind === "instance" ? "E2 switching" : "AJ-E408",
  };
  return <HonestEmpty tab={tab} task={owner[tab]} />;
}

function OverviewPanel(props: {
  kind: InspectorKind;
  data: InspectorData;
  sessions: InstanceInfo[];
  projects: ProjectInfo[];
  accounts: AccountInfo[];
  profiles: LaunchRecipeInfo[];
  terminalTitle: string | null;
  onOpenProject: (id: string) => void;
  onOpenSession: (id: string) => void;
  onLaunch: (recipe: LaunchRecipeInfo, project: ProjectInfo) => void;
  onStopped: () => void;
  onHandover: (sessionId: string, targetProfileId: string) => void;
}) {
  const { kind, data, sessions, accounts, profiles, terminalTitle } = props;

  if (kind === "instance" && data.instance) {
    const session = data.instance;
    const terminalId = session.id;
    const live = session.status === "running" || session.status === "starting";
    const project = props.projects.find((p) => p.id === session.project_id) ?? null;
    return (
      <div className="flex min-h-0 flex-1 flex-col gap-3">
        {project && (
          <button
            type="button"
            onClick={() => props.onOpenProject(project.id)}
            className="flex w-fit items-center gap-1.5 rounded-md px-2 py-1 text-[11px] text-muted hover:bg-surface hover:text-ink"
          >
            <IconProjects width={12} height={12} />
            {project.name}
          </button>
        )}
        {/* The terminal IS the Overview for a terminal-kind Instance. */}
        <Card className="min-h-0 flex-1 overflow-hidden bg-[#05070b] p-0">
          <TerminalView id={terminalId} title={terminalTitle ?? session.account_label} />
        </Card>
        {!live && (
          <p className="text-[11px] text-faint">
            ran {since(session.started_at_ms, Date.now())} · scrollback replayed
          </p>
        )}
        <div className="flex flex-col gap-2">
          <p className="text-[11px] uppercase tracking-wider text-faint">
            Continue on another account
          </p>
          <div className="flex flex-wrap gap-1.5">
            {profiles
              .filter((recipe) => recipe.id !== session.launch_recipe_id)
              .slice(0, 6)
              .map((recipe) => {
                const ready = accounts.find((a) => a.id === recipe.account_id)?.login_state === "logged_in";
                return (
                  <button
                    key={recipe.id}
                    type="button"
                    disabled={!ready}
                    onClick={() => props.onHandover(session.id, recipe.id)}
                    title={ready ? "Moves this session's context to this account and resumes it" : "Needs sign-in first (Ready home)"}
                    className={
                      "rounded-lg border px-2.5 py-1.5 text-[11px] transition-colors " +
                      (ready
                        ? "border-accent/30 bg-accent-soft text-accent-hi hover:border-accent/50"
                        : "border-hairline/50 text-faint")
                    }
                  >
                    {recipe.account_label}
                    {!ready && " · sign in first"}
                  </button>
                );
              })}
            {profiles.filter((p) => p.id !== session.launch_recipe_id).length === 0 && (
              <p className="text-[11px] text-faint">Add another account to juggle between them.</p>
            )}
          </div>
        </div>
      </div>
    );
  }

  if (kind === "project" && data.project) {
    const project = data.project;
    const history = sessions.filter((s) => s.project_id === project.id);
    return (
      <div className="flex flex-col gap-3">
        <p className="font-mono text-[11px] text-faint">{project.root}</p>
        <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
          <Card className="flex flex-col gap-3 p-4">
            <p className="text-[11px] uppercase tracking-wider text-faint">On it now</p>
            {project.active_instance ? (
              <button
                type="button"
                onClick={() => props.onOpenSession(project.active_instance!.id)}
                className="flex items-center gap-2 rounded-lg border border-hairline px-3 py-2.5 text-left transition-colors hover:border-accent/40 hover:bg-surface"
              >
                <span className="dot-live size-1.5 rounded-full bg-live" />
                <span className="text-sm">{project.active_instance.account_label}</span>
                <Chip tone={statusTone(project.active_instance.status).tone}>
                  {statusTone(project.active_instance.status).label}
                </Chip>
                <span className="ml-auto flex items-center gap-1 text-[11px] text-muted">
                  open <IconTerminal width={12} height={12} />
                </span>
              </button>
            ) : (
              <p className="text-xs text-muted">No live instance — free to juggle.</p>
            )}
            <p className="mt-2 text-[11px] uppercase tracking-wider text-faint">
              Launch with another account
            </p>
            <div className="flex flex-col gap-1.5">
              {profiles.slice(0, 5).map((recipe) => {
                const ready = accounts.find((a) => a.id === recipe.account_id)?.login_state === "logged_in";
                return (
                  <button
                    key={recipe.id}
                    type="button"
                    disabled={!ready}
                    title={ready ? undefined : "Needs sign-in first (Ready home)"}
                    onClick={() => props.onLaunch(recipe, project)}
                    className={
                      "flex items-center gap-2 rounded-lg border border-hairline px-3 py-2 text-left text-xs transition-colors " +
                      (ready ? "hover:border-accent/40 hover:bg-surface" : "border-hairline/50 opacity-50")
                    }
                  >
                    <IconTerminal width={13} height={13} />
                    <span className="font-medium">{recipe.account_label}</span>
                    <span className="text-faint">·</span>
                    <span className="text-muted">{recipe.name}</span>
                    {recipe.model && (
                      <span className="ml-auto font-mono text-[10px] text-faint">{recipe.model}</span>
                    )}
                    {!ready && <Chip tone="neutral">sign in first</Chip>}
                  </button>
                );
              })}
              {profiles.length === 0 && (
                <p className="text-xs text-muted">No launch settings yet — add an account first.</p>
              )}
            </div>
          </Card>
          <Card className="flex flex-col gap-2 p-4">
            <p className="text-[11px] uppercase tracking-wider text-faint">Sessions in this project</p>
            {history.length === 0 ? (
              <p className="text-xs text-muted">Nothing recorded yet.</p>
            ) : (
              <div className="flex flex-col divide-y divide-hairline/50">
                {history.map((s) => (
                  <button
                    key={s.id}
                    type="button"
                    onClick={() => props.onOpenSession(s.id)}
                    className="flex items-center gap-2.5 py-2 text-left text-xs"
                  >
                    <span className="min-w-0 flex-1 truncate font-medium">{s.account_label}</span>
                    <Chip tone={statusTone(s.status).tone}>{statusTone(s.status).label}</Chip>
                  </button>
                ))}
              </div>
            )}
          </Card>
        </div>
      </div>
    );
  }

  if (kind === "account" && data.account) {
    const account = data.account;
    const rows = sessions.filter((s) => s.account_id === account.id);
    return (
      <div className="flex flex-col gap-3">
        <Card className="flex flex-col gap-2 p-4">
          <p className="text-[11px] uppercase tracking-wider text-faint">Account</p>
          <div className="flex flex-wrap items-center gap-x-5 gap-y-1 text-[11.5px]">
            <span>
              provider: <span className="text-muted">{account.provider}</span>
            </span>
            <span>
              plan: <span className="text-muted">{account.plan ?? "unknown"}</span>
            </span>
            <Chip tone={account.login_state === "logged_in" ? "success" : "neutral"}>
              {account.login_state === "logged_in" ? "signed in" : "signed out"}
            </Chip>
          </div>
        </Card>
        <Card className="flex flex-col gap-2 p-4">
          <p className="text-[11px] uppercase tracking-wider text-faint">Instances on this account</p>
          {rows.length === 0 ? (
            <p className="text-xs text-muted">No instances recorded yet.</p>
          ) : (
            rows.slice(0, 8).map((row) => (
              <button
                key={row.id}
                type="button"
                onClick={() => props.onOpenSession(row.id)}
                className="flex items-center gap-2 py-1.5 text-left text-xs"
              >
                <span className="min-w-0 flex-1 truncate font-medium">{row.account_label}</span>
                <Chip tone={statusTone(row.status).tone}>{statusTone(row.status).label}</Chip>
              </button>
            ))
          )}
        </Card>
      </div>
    );
  }

  if (kind === "provider" && data.provider) {
    const providerAccounts = accounts.filter((a) => a.provider === data.provider!.id);
    return (
      <div className="flex flex-col gap-3">
        <Card className="flex flex-col gap-2 p-4">
          <p className="text-[11px] uppercase tracking-wider text-faint">Provider</p>
          <p className="font-mono text-[11.5px]">{data.provider.id}</p>
          <p className="text-[11.5px] text-muted">
            {providerAccounts.length} account{providerAccounts.length === 1 ? "" : "s"} ·{" "}
            {providerAccounts.filter((a) => a.login_state === "logged_in").length} signed in
          </p>
        </Card>
        <Card className="flex flex-col gap-2 p-4">
          <p className="text-[11px] uppercase tracking-wider text-faint">Accounts</p>
          {providerAccounts.map((account) => (
            <p key={account.id} className="flex items-center gap-2 py-1 text-xs">
              <span className="min-w-0 flex-1 truncate font-medium">{account.label}</span>
              <Chip tone={account.login_state === "logged_in" ? "success" : "neutral"}>
                {account.login_state === "logged_in" ? "signed in" : "signed out"}
              </Chip>
            </p>
          ))}
          {providerAccounts.length === 0 && (
            <p className="text-xs text-muted">No accounts on this provider yet.</p>
          )}
        </Card>
      </div>
    );
  }

  // Session (conversation) — content lands with AJ-E407; the shell is honest.
  return <HonestEmpty tab="overview" task="AJ-E407 (conversation inspector)" />;
}

/** Empty selection: the fleet-at-a-glance summary — never a blank panel (Req.4). */
function FleetAtAGlance({
  sessions,
  accounts,
  now,
  onNew,
}: {
  sessions: InstanceInfo[];
  accounts: AccountInfo[];
  now: number;
  onNew: () => void;
}) {
  const counts = new Map<string, number>();
  for (const row of sessions) counts.set(row.status, (counts.get(row.status) ?? 0) + 1);
  const withQuota = sessions.filter((s) => s.quota !== null);
  const lowest = withQuota.reduce<InstanceInfo | null>((min, row) => {
    if (!min) return row;
    const left = 100 - (row.quota?.used_percentage ?? 100);
    const minLeft = 100 - (min.quota?.used_percentage ?? 100);
    return left < minLeft ? row : min;
  }, null);
  const lowestAccount = lowest
    ? accounts.find((a) => a.id === lowest.account_id)?.label ?? lowest.account_label
    : null;
  const live = sessions.filter((s) => s.status === "running" || s.status === "starting");
  const cpuSum = live.reduce((sum, row) => sum + (row.cpu_pct ?? 0), 0);
  const ramSum = live.reduce((sum, row) => sum + (row.rss_bytes ?? 0), 0);
  const machines = new Set(sessions.map((row) => row.machine_id));

  return (
    <div className="flex h-full flex-col items-center justify-center gap-5" data-testid="fleet-at-a-glance">
      <div className="text-center">
        <p className="text-sm font-medium">Nothing selected — the fleet at a glance</p>
        <p className="mt-1 text-xs text-muted">
          Pick anything in the rail; this home never moves.
        </p>
      </div>
      <div className="grid w-full max-w-2xl grid-cols-2 gap-3 xl:grid-cols-4">
        <Card className="p-3">
          <p className="text-[10px] uppercase tracking-wider text-muted">By state</p>
          <div className="mt-1.5 flex flex-col gap-0.5 text-[11px]">
            {sessions.length === 0 ? (
              <span className="text-muted">No instances yet</span>
            ) : (
              [...counts.entries()].map(([status, count]) => (
                <span key={status} className="flex justify-between gap-2">
                  <span className="text-muted">{stateChip(status).label}</span>
                  <span className="tabular-nums text-ink">{count}</span>
                </span>
              ))
            )}
          </div>
        </Card>
        <Card className="p-3">
          <p className="text-[10px] uppercase tracking-wider text-muted">Lowest quota</p>
          <p className="mt-1.5 text-[11px] text-muted">
            {lowest && lowest.quota ? (
              <>
                <span className="text-ink">{lowestAccount}</span> ·{" "}
                {Math.round(100 - lowest.quota.used_percentage)}% left ·{" "}
                {quotaTierTag(lowest.quota.confidence)}
              </>
            ) : (
              "Quota: no data"
            )}
          </p>
        </Card>
        <Card className="p-3">
          <p className="text-[10px] uppercase tracking-wider text-muted">
            this-mac · {machines.size} machine{machines.size === 1 ? "" : "s"}
          </p>
          <div className="mt-1.5 flex flex-col gap-0.5 text-[11px]">
            <span className="flex justify-between gap-2">
              <span className="text-muted">live CPU (sum)</span>
              <span className="tabular-nums text-ink">{live.length > 0 ? `${Math.round(cpuSum)}%` : "—"}</span>
            </span>
            <span className="flex justify-between gap-2">
              <span className="text-muted">live RAM (sum)</span>
              <span className="tabular-nums text-ink">{live.length > 0 ? formatBytes(ramSum) : "—"}</span>
            </span>
          </div>
          <p className="mt-1 text-[9px] text-muted">
            {live.length === 0
              ? "gauges show — until instances report resources"
              : `measured over ${live.length} live instance${live.length === 1 ? "" : "s"} (AJ-E405+ collector)`}
          </p>
        </Card>
        <Card className="flex flex-col items-start justify-center gap-2 p-3">
          <p className="text-[10px] uppercase tracking-wider text-muted">Start</p>
          <Button size="sm" variant="primary" onClick={onNew}>
            <IconTerminal width={13} height={13} />
            New session
          </Button>
        </Card>
      </div>
      <p className="text-[10px] text-muted">
        Updated {since(now, now)} ago · source: supervisor
      </p>
    </div>
  );
}

