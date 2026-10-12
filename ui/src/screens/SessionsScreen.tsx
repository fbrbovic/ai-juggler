import { useCallback, useEffect, useMemo, useState } from "react";
import { Card, Chip } from "../components/primitives";
import { IconSessions } from "../components/icons";
import {
  projectsList,
  sessionsList,
  onSessionsUpdated,
  type ProjectInfo,
  type SessionInfo,
} from "../lib/bridge";

function dateOf(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * Sessions — the continuity view (ADR-0007): the work conversations inside
 * instances, grouped by project. Resume and hand-over attach here as those
 * milestones land; the grouping already shows the juggling history.
 */
export function SessionsScreen() {
  const [sessions, setSessions] = useState<SessionInfo[] | null>(null);
  const [projects, setProjects] = useState<ProjectInfo[]>([]);

  const refresh = useCallback(async () => {
    const [sessionRows, projectRows] = await Promise.all([sessionsList(), projectsList()]);
    setSessions(sessionRows);
    setProjects(projectRows);
  }, []);

  useEffect(() => {
    void refresh();
    return onSessionsUpdated(() => void refresh());
  }, [refresh]);

  const grouped = useMemo(() => {
    const byProject = new Map<string, SessionInfo[]>();
    for (const session of sessions ?? []) {
      const key = session.project_id ?? "unassigned";
      byProject.set(key, [...(byProject.get(key) ?? []), session]);
    }
    return [...byProject.entries()].map(([projectId, rows]) => ({
      project: projects.find((p) => p.id === projectId) ?? null,
      rows: [...rows].sort((a, b) => b.started_at_ms - a.started_at_ms),
    }));
  }, [sessions, projects]);

  if (sessions === null) {
    return <div className="py-24 text-center text-sm text-muted">Loading…</div>;
  }

  return (
    <div className="mx-auto flex w-full max-w-4xl flex-col gap-5">
      <header className="flex items-end justify-between gap-4">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Sessions</h2>
          <p className="text-xs text-muted">
            Work conversations inside instances, grouped by project — the
            history that hand-over will move between accounts.
          </p>
        </div>
      </header>

      {sessions.length === 0 ? (
        <Card className="flex flex-col items-center justify-center gap-3 py-14">
          <span className="flex size-11 items-center justify-center rounded-xl border border-accent/20 bg-accent-soft text-accent-hi">
            <IconSessions width={22} height={22} />
          </span>
          <p className="max-w-md text-center text-sm text-muted">
            No sessions yet — every launched instance records its sessions
            here, grouped by project.
          </p>
        </Card>
      ) : (
        <div className="flex flex-col gap-5">
          {grouped.map(({ project, rows }) => (
            <Card key={project?.id ?? "unassigned"} className="p-4">
              <div className="mb-3 flex items-center gap-2">
                <span className="text-sm font-medium">{project?.name ?? "Unassigned"}</span>
                {project && (
                  <span className="truncate font-mono text-[11px] text-faint">{project.root}</span>
                )}
                <Chip tone="neutral">{rows.length}</Chip>
              </div>
              <div className="flex flex-col divide-y divide-hairline/50">
                {rows.map((session) => (
                  <div key={session.id} className="flex items-center gap-3 py-2.5 text-xs">
                    <span
                      className={
                        "size-1.5 shrink-0 rounded-full " +
                        (session.status === "running" || session.status === "starting"
                          ? "bg-live animate-breathe"
                          : session.status === "failed"
                            ? "bg-danger"
                            : "bg-faint")
                      }
                    />
                    <span className="min-w-0 flex-1 truncate font-medium">{session.label}</span>
                    <span className="text-faint">{session.kind}</span>
                    <span className="text-faint">{dateOf(session.started_at_ms)}</span>
                    <Chip
                      tone={
                        session.status === "failed"
                          ? "danger"
                          : session.status === "running"
                            ? "live"
                            : "neutral"
                      }
                    >
                      {session.status}
                    </Chip>
                  </div>
                ))}
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
