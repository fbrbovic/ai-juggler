import { useCallback, useEffect, useMemo, useState } from "react";
import { Card, Chip, type ChipTone } from "../components/primitives";
import { IconJuggler, IconProjects, IconTerminal } from "../components/icons";
import {
  launchSession,
  onSessionsUpdated,
  profilesList,
  projectsList,
  type ProfileInfo,
  type ProjectInfo,
} from "../lib/bridge";

function since(ms: number | null | undefined): string {
  if (!ms) return "never";
  const secs = Math.max(1, Math.floor((Date.now() - ms) / 1000));
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86_400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86_400)}d ago`;
}

function statusTone(status: string): { tone: ChipTone; label: string } {
  switch (status) {
    case "running":
      return { tone: "live", label: "running" };
    case "starting":
      return { tone: "accent", label: "starting" };
    case "finished":
      return { tone: "neutral", label: "finished" };
    case "failed":
      return { tone: "danger", label: "failed" };
    default:
      return { tone: "neutral", label: status };
  }
}

/**
 * Projects — the juggling anchor (ADR-0007). Every directory an instance
 * has run in: who's on it now, which accounts have worked here, and the
 * switch action (launch another account on this project).
 */
export function ProjectsScreen({
  openTerminal,
  onOpenInstances,
}: {
  openTerminal: (id: string, title: string) => void;
  onOpenInstances: () => void;
}) {
  const [projects, setProjects] = useState<ProjectInfo[] | null>(null);
  const [profiles, setProfiles] = useState<ProfileInfo[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [projectRows, profileRows] = await Promise.all([
        projectsList(),
        profilesList(),
      ]);
      setProjects(projectRows);
      setProfiles(profileRows);
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
    return onSessionsUpdated(() => void refresh());
  }, [refresh]);

  const active = useMemo(
    () => projects?.find((p) => p.id === selected) ?? null,
    [projects, selected],
  );

  const launchOnProject = async (project: ProjectInfo, profile: ProfileInfo) => {
    setError(null);
    try {
      const result = await launchSession(profile.id, project.root);
      if (result) {
        openTerminal(result.session_id, `${profile.account_label} · ${project.name}`);
      }
      void refresh();
    } catch (cause) {
      setError(String(cause));
    }
  };

  if (projects === null) {
    return <div className="py-24 text-center text-sm text-muted">Loading…</div>;
  }

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-5">
      <header className="flex items-end justify-between gap-4">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Projects</h2>
          <p className="text-xs text-muted">
            The directories you juggle accounts in — who's on each one now, and
            who has worked there before.
          </p>
        </div>
      </header>

      {error && (
        <Card className="border-danger/30 bg-danger-soft p-3 text-xs text-danger">{error}</Card>
      )}

      {projects.length === 0 ? (
        <Card className="flex flex-col items-center justify-center gap-3 py-14">
          <span className="flex size-11 items-center justify-center rounded-xl border border-accent/20 bg-accent-soft text-accent-hi">
            <IconProjects width={22} height={22} />
          </span>
          <p className="max-w-md text-center text-sm text-muted">
            No projects yet — launch a profile in any directory and it becomes a
            project you can juggle accounts on.
          </p>
        </Card>
      ) : (
        <div className="grid grid-cols-1 gap-4 xl:grid-cols-5">
          {/* Project list */}
          <div className="flex flex-col gap-3 xl:col-span-3">
            {projects.map((project) => {
              const live = project.active_instance;
              return (
                <Card
                  key={project.id}
                  className={
                    "cursor-pointer p-4 transition-colors " +
                    (project.id === selected ? "border-accent/40" : "hover:border-hairline/80")
                  }
                >
                  <div
                    role="button"
                    tabIndex={0}
                    onClick={() => setSelected(project.id)}
                    onKeyDown={(event) => event.key === "Enter" && setSelected(project.id)}
                    className="flex flex-col gap-3"
                  >
                    <div className="flex items-center gap-3">
                      <span className="text-accent-hi">
                        <IconProjects width={17} height={17} />
                      </span>
                      <span className="truncate text-sm font-medium">{project.name}</span>
                      {live ? (
                        <Chip tone={statusTone(live.status).tone} dot>
                          {live.label}
                        </Chip>
                      ) : (
                        <Chip tone="neutral">idle</Chip>
                      )}
                      <span className="ml-auto shrink-0 text-[11px] text-faint">
                        {since(project.last_activity_ms)}
                      </span>
                    </div>
                    <p className="truncate font-mono text-[11px] text-faint">{project.root}</p>
                    <div className="flex flex-wrap items-center gap-1.5">
                      {project.accounts_used.map((account) => (
                        <Chip key={account} tone="neutral">
                          {account}
                        </Chip>
                      ))}
                      <span className="ml-auto text-[11px] text-faint">
                        {project.instance_count} instance{project.instance_count === 1 ? "" : "s"}
                      </span>
                    </div>
                  </div>
                </Card>
              );
            })}
          </div>

          {/* Project panel */}
          <div className="xl:col-span-2">
            {active ? (
              <Card className="flex flex-col gap-4 p-5">
                <div className="flex items-center gap-2">
                  <span className="text-accent-hi">
                    <IconJuggler width={18} height={18} />
                  </span>
                  <h3 className="text-sm font-semibold">{active.name}</h3>
                </div>
                <p className="truncate font-mono text-[11px] text-faint">{active.root}</p>

                <div className="rounded-xl border border-hairline bg-canvas p-3">
                  <p className="mb-1.5 text-[11px] uppercase tracking-wider text-faint">
                    On it now
                  </p>
                  {active.active_instance ? (
                    <div className="flex items-center gap-2">
                      <span className="dot-live size-1.5 rounded-full bg-live" />
                      <span className="text-sm">{active.active_instance.label}</span>
                      <Chip tone={statusTone(active.active_instance.status).tone}>
                        {statusTone(active.active_instance.status).label}
                      </Chip>
                      <button
                        type="button"
                        className="ml-auto rounded-md px-2 py-1 text-[11px] text-muted hover:bg-surface hover:text-ink"
                        onClick={() => {
                          openTerminal(
                            active.active_instance!.id,
                            `${active.active_instance!.label} · ${active.name}`,
                          );
                        }}
                      >
                        open terminal
                      </button>
                    </div>
                  ) : (
                    <p className="text-xs text-muted">No live instance — free to juggle.</p>
                  )}
                </div>

                <div>
                  <p className="mb-2 text-[11px] uppercase tracking-wider text-faint">
                    Launch with another account
                  </p>
                  <div className="flex flex-col gap-1.5">
                    {profiles
                      .filter((profile) => profile.account_id !== active.active_instance?.label)
                      .slice(0, 4)
                      .map((profile) => (
                        <button
                          key={profile.id}
                          type="button"
                          onClick={() => void launchOnProject(active, profile)}
                          className="flex items-center gap-2 rounded-lg border border-hairline px-3 py-2 text-left text-xs transition-colors hover:border-accent/40 hover:bg-surface"
                        >
                          <IconTerminal width={13} height={13} />
                          <span className="font-medium">{profile.account_label}</span>
                          <span className="text-faint">·</span>
                          <span className="text-muted">{profile.name}</span>
                          {profile.model && (
                            <span className="ml-auto font-mono text-[10px] text-faint">
                              {profile.model}
                            </span>
                          )}
                        </button>
                      ))}
                    {profiles.length === 0 && (
                      <p className="text-xs text-muted">
                        No profiles yet — add an account first (Accounts, top right).
                      </p>
                    )}
                  </div>
                </div>

                <button
                  type="button"
                  onClick={onOpenInstances}
                  className="self-start text-[11px] text-muted underline-offset-2 hover:text-ink hover:underline"
                >
                  view all instances →
                </button>
              </Card>
            ) : (
              <Card className="hidden items-center justify-center p-8 text-xs text-faint xl:flex">
                Select a project to see who's on it and juggle accounts.
              </Card>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
