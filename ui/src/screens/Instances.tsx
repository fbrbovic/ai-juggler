import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Card, Chip, type ChipTone } from "../components/primitives";
import { IconTerminal } from "../components/icons";
import {
  onSessionsUpdated,
  sessionsList,
  stopSession,
  type SessionInfo,
} from "../lib/bridge";

function since(ms: number): string {
  const secs = Math.max(1, Math.floor((Date.now() - ms) / 1000));
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}h`;
}

function statusView(status: string): { tone: ChipTone; label: string } {
  switch (status) {
    case "running":
      return { tone: "live", label: "running" };
    case "starting":
      return { tone: "accent", label: "starting" };
    case "finished":
      return { tone: "success", label: "finished" };
    case "failed":
      return { tone: "danger", label: "failed" };
    default:
      return { tone: "neutral", label: status };
  }
}

const isLive = (status: string) => status === "running" || status === "starting";

/**
 * Instances — every running/tracked occurrence of a harness across all
 * accounts (ADR-0007): CLI terminals today, desktop windows as those
 * adapters land. Selecting one opens its panel.
 */
export function InstancesScreen({
  selectedId,
  onSelect,
  openTerminal,
}: {
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  openTerminal: (id: string, title: string) => void;
}) {
  const [sessions, setSessions] = useState<SessionInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSessions(await sessionsList());
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
    return onSessionsUpdated(() => void refresh());
  }, [refresh]);

  const active = useMemo(
    () => sessions?.find((s) => s.id === selectedId) ?? null,
    [sessions, selectedId],
  );

  const live = useMemo(
    () => sessions?.filter((s) => isLive(s.status)) ?? [],
    [sessions],
  );
  const rest = useMemo(
    () => sessions?.filter((s) => !isLive(s.status)).slice(0, 12) ?? [],
    [sessions],
  );

  if (sessions === null) {
    return <div className="py-24 text-center text-sm text-muted">Loading…</div>;
  }

  const row = (instance: SessionInfo) => {
    const view = statusView(instance.status);
    return (
      <Card
        key={instance.id}
        className={
          "cursor-pointer p-3.5 transition-colors " +
          (instance.id === selectedId ? "border-accent/40" : "hover:border-hairline/80")
        }
      >
        <div
          role="button"
          tabIndex={0}
          onClick={() => onSelect(instance.id)}
          onKeyDown={(event) => event.key === "Enter" && onSelect(instance.id)}
          className="flex items-center gap-3"
        >
          <span
            className={
              "size-1.5 shrink-0 rounded-full " +
              (isLive(instance.status) ? "bg-live animate-breathe" : "bg-faint")
            }
          />
          <span className="min-w-0 flex-1 truncate text-sm">{instance.label}</span>
          <Chip tone={instance.kind === "desktop" ? "accent" : "neutral"}>
            {instance.kind}
          </Chip>
          <Chip tone={view.tone}>{view.label}</Chip>
          <span className="shrink-0 text-[11px] text-faint">{since(instance.started_at_ms)}</span>
        </div>
      </Card>
    );
  };

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-5">
      <header className="flex items-end justify-between gap-4">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Instances</h2>
          <p className="text-xs text-muted">
            Every running harness occurrence — CLI terminals and, as adapters
            land, desktop windows. Click one to open its panel.
          </p>
        </div>
        <Chip tone="neutral">{live.length} live</Chip>
      </header>

      {error && (
        <Card className="border-danger/30 bg-danger-soft p-3 text-xs text-danger">{error}</Card>
      )}

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-5">
        <div className="flex flex-col gap-4 xl:col-span-3">
          {sessions.length === 0 ? (
            <Card className="flex flex-col items-center justify-center gap-3 py-14">
              <span className="flex size-11 items-center justify-center rounded-xl border border-accent/20 bg-accent-soft text-accent-hi">
                <IconTerminal width={22} height={22} />
              </span>
              <p className="max-w-md text-center text-sm text-muted">
                Nothing running. Launch a profile from a project or from
                Accounts (top right) — it will appear here.
              </p>
            </Card>
          ) : (
            <>
              {live.length > 0 && (
                <div className="flex flex-col gap-2">
                  <p className="text-[11px] uppercase tracking-wider text-faint">Live now</p>
                  {live.map(row)}
                </div>
              )}
              {rest.length > 0 && (
                <div className="flex flex-col gap-2">
                  <p className="text-[11px] uppercase tracking-wider text-faint">Recent</p>
                  {rest.map(row)}
                </div>
              )}
            </>
          )}
        </div>

        {/* Instance panel */}
        <div className="xl:col-span-2">
          {active ? (
            <Card className="flex flex-col gap-4 p-5">
              <div className="flex items-center gap-2">
                <span
                  className={
                    "size-2 rounded-full " +
                    (isLive(active.status) ? "bg-live animate-breathe" : "bg-faint")
                  }
                />
                <h3 className="text-sm font-semibold">{active.label}</h3>
                <Chip tone={statusView(active.status).tone}>
                  {statusView(active.status).label}
                </Chip>
              </div>

              <dl className="grid grid-cols-2 gap-3 text-xs">
                <div>
                  <dt className="text-faint">Harness</dt>
                  <dd className="mt-0.5 font-medium">{active.harness}</dd>
                </div>
                <div>
                  <dt className="text-faint">Kind</dt>
                  <dd className="mt-0.5 font-medium capitalize">{active.kind}</dd>
                </div>
                <div>
                  <dt className="text-faint">Profile</dt>
                  <dd className="mt-0.5 font-mono text-[11px]">{active.profile_id}</dd>
                </div>
                <div>
                  <dt className="text-faint">Age</dt>
                  <dd className="mt-0.5 font-medium">{since(active.started_at_ms)}</dd>
                </div>
              </dl>

              <div className="flex flex-col gap-2">
                {active.kind === "terminal" ? (
                  <Button
                    size="sm"
                    variant="primary"
                    onClick={() => openTerminal(active.id, active.label)}
                  >
                    <IconTerminal width={14} height={14} />
                    Open terminal
                  </Button>
                ) : (
                  <Button size="sm" variant="primary" disabled>
                    Focus window (desktop adapters coming)
                  </Button>
                )}
                {isLive(active.status) ? (
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={async () => {
                      await stopSession(active.id);
                      void refresh();
                    }}
                  >
                    Stop
                  </Button>
                ) : null}
              </div>

              <p className="text-[11px] text-faint">
                Hand-over to another account lands with the handover milestone
                — the data lineage (project ↔ session ↔ account) is already
                tracked.
              </p>
            </Card>
          ) : (
            <Card className="hidden items-center justify-center p-8 text-xs text-faint xl:flex">
              Select an instance to open its panel.
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
