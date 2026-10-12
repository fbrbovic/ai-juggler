import { useEffect, useMemo, useState } from "react";
import { Button, Card, Chip } from "../components/primitives";
import { IconProjects, IconTerminal } from "../components/icons";
import {
  hasLiveBackend,
  launchSession,
  pickDirectory,
  type AccountInfo,
  type ProfileInfo,
  type ProjectInfo,
} from "../lib/bridge";

/**
 * New session — zero typing: recent projects (real history), a native
 * folder picker, or drag a folder onto the window; then an account whose
 * sign-in state is visible before you commit.
 */
export function NewSessionModal({
  projects,
  accounts,
  initialDirectory,
  onClose,
  onLaunched,
}: {
  projects: ProjectInfo[];
  accounts: AccountInfo[];
  initialDirectory: string | null;
  onClose: () => void;
  onLaunched: (sessionId: string, title: string) => void;
}) {
  const [directory, setDirectory] = useState<string | null>(initialDirectory);
  const [profiles, setProfiles] = useState<ProfileInfo[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    void import("../lib/bridge").then((bridge) =>
      bridge.profilesList().then(setProfiles),
    );
  }, []);

  const loginByAccount = useMemo(() => {
    const map = new Map<string, string>();
    for (const account of accounts) map.set(account.id, account.login_state);
    return map;
  }, [accounts]);

  const recents = useMemo(
    () =>
      [...projects]
        .sort(
          (a, b) => (b.last_activity_ms ?? 0) - (a.last_activity_ms ?? 0),
        )
        .slice(0, 6),
    [projects],
  );

  const launch = async (profile: ProfileInfo) => {
    setBusy(true);
    setError(null);
    try {
      const result = await launchSession(profile.id, directory);
      if (!result) {
        setError("Preview mode — no backend connected.");
        setBusy(false);
        return;
      }
      onLaunched(result.session_id, `${profile.account_label} · ${profile.name}`);
    } catch (cause) {
      setError(String(cause));
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6 backdrop-blur-sm"
      onKeyDown={(event) => event.key === "Escape" && onClose()}
    >
      <Card className="flex max-h-[86vh] w-full max-w-xl flex-col overflow-hidden p-0">
        <div className="flex items-center gap-3 border-b border-hairline/60 px-6 py-4">
          <span className="text-accent-hi">
            <IconTerminal width={16} height={16} />
          </span>
          <h3 className="text-sm font-semibold">New session</h3>
          <div className="ml-auto flex items-center gap-2">
            {directory && (
              <Chip tone="accent">
                <span className="max-w-64 truncate font-mono text-[10px]">
                  {directory.split("/").pop()}
                </span>
              </Chip>
            )}
          </div>
        </div>

        <div
          className="min-h-0 flex-1 overflow-y-auto px-6 py-5"
          onDragOver={(event) => {
            event.preventDefault();
            setDragging(true);
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={(event) => {
            event.preventDefault();
            setDragging(false);
            // In-browser drop of a folder: webview file drops carry names
            // only; the native Tauri drop path (App-level) carries full
            // paths and opens this modal pre-filled.
          }}
        >
          {/* Step 1 — where */}
          <p className="text-[11px] uppercase tracking-wider text-faint">
            Where do you want to work?
          </p>
          <div className="mt-2 grid grid-cols-2 gap-2">
            {recents.map((project) => (
              <button
                key={project.id}
                type="button"
                onClick={() => setDirectory(project.root)}
                className={
                  "flex min-w-0 flex-col gap-1 rounded-xl border px-3 py-2.5 text-left transition-colors " +
                  (directory === project.root
                    ? "border-accent/40 bg-accent-soft"
                    : "border-hairline hover:border-hairline/80 hover:bg-surface")
                }
              >
                <span className="flex items-center gap-1.5">
                  <IconProjects width={12} height={12} />
                  <span className="truncate text-xs font-medium">
                    {project.name}
                  </span>
                  {project.active_instance && (
                    <span className="dot-live ml-auto size-1.5 rounded-full bg-live" />
                  )}
                </span>
                <span className="truncate text-[10px] text-faint">
                  {project.history_sessions > 0
                    ? `${project.history_sessions} prior sessions · ${project.accounts_used.join(", ")}`
                    : project.root}
                </span>
              </button>
            ))}
            <button
              type="button"
              onClick={async () => {
                const picked = await pickDirectory();
                if (picked) setDirectory(picked);
              }}
              className={
                "flex flex-col items-center justify-center gap-1 rounded-xl border border-dashed px-3 py-2.5 text-center transition-colors " +
                (dragging
                  ? "border-accent/60 bg-accent-soft text-accent-hi"
                  : "border-hairline text-muted hover:border-accent/40 hover:text-ink")
              }
            >
              <IconProjects width={14} height={14} />
              <span className="text-xs font-medium">Choose folder…</span>
              <span className="text-[10px] text-faint">or drop it anywhere</span>
            </button>
          </div>

          {/* Step 2 — who */}
          <p className="mt-5 text-[11px] uppercase tracking-wider text-faint">
            Which account?
          </p>
          <div className="mt-2 flex flex-col gap-1.5">
            {(profiles ?? []).map((profile) => {
              const login = loginByAccount.get(profile.account_id) ?? "logged_out";
              const ready = login === "logged_in";
              return (
                <button
                  key={profile.id}
                  type="button"
                  disabled={busy || !ready}
                  onClick={() => void launch(profile)}
                  className={
                    "flex items-center gap-2 rounded-xl border px-3 py-2.5 text-left text-xs transition-colors " +
                    (ready
                      ? "border-hairline hover:border-accent/40 hover:bg-surface disabled:opacity-50"
                      : "border-hairline/50 opacity-50")
                  }
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
                  {!ready && (
                    <Chip tone="neutral">needs sign-in (Ready home)</Chip>
                  )}
                </button>
              );
            })}
            {profiles !== null && profiles.length === 0 && (
              <p className="py-3 text-center text-xs text-muted">
                No profiles yet — add an account first (Accounts, top right).
              </p>
            )}
            {profiles === null && (
              <p className="py-3 text-center text-xs text-faint">Loading…</p>
            )}
          </div>

          {error && <p className="mt-3 text-xs text-danger">{error}</p>}
        </div>

        <div className="flex items-center justify-between border-t border-hairline/60 px-6 py-4">
          <span className="text-[11px] text-faint">
            {directory
              ? `Launching in ${directory.split("/").pop()}`
              : "No folder picked — the profile's default applies"}
          </span>
          <Button size="sm" variant="secondary" onClick={onClose}>
            Close
          </Button>
        </div>
      </Card>
    </div>
  );
}

void hasLiveBackend;
