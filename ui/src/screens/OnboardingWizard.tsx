import { useEffect, useMemo, useState } from "react";
import { Button, Card, Chip } from "../components/primitives";
import { IconJuggler, IconProfiles, IconTerminal } from "../components/icons";
import {
  accountImport,
  accountLoginCommand,
  accountLoginState,
  createAccount,
  environmentScan,
  hasLiveBackend,
  loginOpenWeb,
  terminalSpawnLogin,
  type AccountInfo,
  type DiscoveredInfo,
  type ImportResult,
} from "../lib/bridge";

const HARNESS = "claude-code";

const PLANS: { id: string | null; label: string; hint: string }[] = [
  { id: null, label: "Not sure yet", hint: "you can set this later" },
  { id: "free", label: "Free", hint: "trying things out" },
  { id: "pro", label: "Pro", hint: "regular daily use" },
  { id: "max-20x", label: "Max 20×", hint: "heavy parallel work" },
];

function humanBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/**
 * The onboarding wizard (ADR-0006, spec §12.1): assumes the user knows
 * nothing about harness internals. Detect what exists → offer import or
 * a guided fresh start → sign in with the vendor's own flow → a summary
 * that says exactly what happened and what wasn't touched.
 */
export function OnboardingWizard({
  onClose,
  onAccountCreated,
  openLoginTerminal,
}: {
  onClose: () => void;
  onAccountCreated: () => void;
  openLoginTerminal: (id: string, title: string) => void;
}) {
  const [step, setStep] = useState<"detect" | "identity" | "login" | "summary">("detect");
  const [scan, setScan] = useState<DiscoveredInfo[] | null>(null);
  const [importing, setImporting] = useState<DiscoveredInfo | null>(null);
  const [label, setLabel] = useState("");
  const [plan, setPlan] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [account, setAccount] = useState<AccountInfo | null>(null);
  const [importResult, setImportResult] = useState<ImportResult | null>(null);
  const [loginState, setLoginState] = useState<string | null>(null);
  const [command, setCommand] = useState("");
  const [copied, setCopied] = useState(false);

  // Step 1 runs the scan automatically — the user never asks for anything.
  useEffect(() => {
    let cancelled = false;
    void environmentScan().then((found) => {
      if (!cancelled) setScan(found);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (account) void accountLoginCommand(account.id).then(setCommand);
  }, [account]);

  const title = useMemo(() => {
    if (importing) return `Import “${importing.path.split("/").pop() || importing.path}”`;
    return "Add a Claude account";
  }, [importing]);

  const createTheAccount = async () => {
    setBusy(true);
    setError(null);
    try {
      if (importing) {
        const result = await accountImport(HARNESS, importing.path, label.trim(), plan);
        setImportResult(result);
        setAccount(result.account);
      } else {
        const created = await createAccount(HARNESS, label.trim(), plan);
        setAccount(created);
      }
      setBusy(false);
      setStep("login");
    } catch (cause) {
      setBusy(false);
      setError(String(cause));
    }
  };

  const recheck = async () => {
    if (!account) return;
    setLoginState(await accountLoginState(account.id));
    onAccountCreated();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6 backdrop-blur-sm">
      <Card className="flex max-h-[86vh] w-full max-w-xl flex-col overflow-hidden p-0">
        {/* Wizard header: progress + close */}
        <div className="flex items-center gap-3 border-b border-hairline/60 px-6 py-4">
          <span className="text-accent">
            <IconJuggler width={18} height={18} />
          </span>
          <h3 className="text-sm font-semibold">{title}</h3>
          <div className="ml-auto flex items-center gap-1.5">
            {(["detect", "identity", "login", "summary"] as const).map((s, i) => (
              <span
                key={s}
                className={
                  "h-1 rounded-full transition-all " +
                  (["detect", "identity", "login", "summary"].indexOf(step) >= i
                    ? "w-5 bg-accent"
                    : "w-2 bg-hairline")
                }
              />
            ))}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="ml-3 rounded-md px-2 py-1 text-[11px] text-faint hover:bg-surface hover:text-ink"
          >
            ✕
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
          {/* ---------------------------------------------------- detect */}
          {step === "detect" && (
            <div className="flex flex-col gap-4">
              <div>
                <p className="text-sm font-medium">First, let's look at what you already have.</p>
                <p className="mt-1 text-xs leading-relaxed text-muted">
                  AI Juggler scans the usual spots for existing Claude setups — your standard
                  install, configs left by other tools, anything an AI assistant may have
                  created. Importing keeps your settings and history; the original is never
                  touched.
                </p>
              </div>

              {scan === null ? (
                <div className="flex items-center gap-3 rounded-xl border border-hairline px-4 py-5">
                  <span className="dot-live size-2 rounded-full bg-live" />
                  <span className="text-xs text-muted">Scanning your machine…</span>
                </div>
              ) : scan.length === 0 ? (
                <div className="rounded-xl border border-dashed border-hairline px-4 py-5 text-center">
                  <p className="text-xs text-muted">
                    No existing Claude setups found — we'll create a fresh one.
                  </p>
                </div>
              ) : (
                <div className="flex flex-col gap-2">
                  {scan.map((found) => (
                    <button
                      key={found.path}
                      type="button"
                      onClick={() => {
                        setImporting(found);
                        setLabel(found.path.split("/").pop() || "Imported account");
                        setStep("identity");
                      }}
                      className="flex flex-col gap-2 rounded-xl border border-hairline px-4 py-3.5 text-left transition-colors hover:border-accent/40 hover:bg-surface"
                    >
                      <div className="flex items-center gap-2">
                        <IconProfiles width={14} height={14} />
                        <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-ink">
                          {found.path}
                        </span>
                        {found.login === "logged_in" ? (
                          <Chip tone="success">signed in</Chip>
                        ) : (
                          <Chip tone="neutral">not signed in</Chip>
                        )}
                      </div>
                      <p className="text-[11px] leading-relaxed text-muted">
                        {found.notes[0]}
                        {found.bytes > 0 && (
                          <span className="text-faint"> · {humanBytes(found.bytes)} of state</span>
                        )}
                      </p>
                    </button>
                  ))}
                </div>
              )}

              {scan !== null && (
                <button
                  type="button"
                  onClick={() => {
                    setImporting(null);
                    setLabel("");
                    setStep("identity");
                  }}
                  className="self-start text-xs text-muted underline-offset-2 hover:text-ink hover:underline"
                >
                  {scan.length > 0
                    ? "None of these — add a fresh account"
                    : "Continue with a fresh account →"}
                </button>
              )}
            </div>
          )}

          {/* -------------------------------------------------- identity */}
          {step === "identity" && (
            <div className="flex flex-col gap-4">
              {importing ? (
                <div className="rounded-xl border border-hairline bg-canvas px-4 py-3 text-xs leading-relaxed text-muted">
                  Importing from <span className="font-mono text-[11px]">{importing.path}</span>
                  {importing.login === "logged_in" && " — your settings and session history come along."}
                  {importing.login === "logged_out" && " — its settings come along; you'll sign in after."}
                </div>
              ) : (
                <p className="text-xs leading-relaxed text-muted">
                  Each account is a separate Claude login, fully isolated — what happens in one
                  never touches the others, or your existing install.
                </p>
              )}

              <label className="block text-[11px] uppercase tracking-wider text-faint">
                What will you use this account for?
                <input
                  autoFocus
                  value={label}
                  onChange={(event) => setLabel(event.target.value)}
                  placeholder="e.g. Work, Personal, Side project"
                  className="mt-1.5 w-full rounded-lg border border-hairline bg-canvas px-3 py-2.5 text-sm text-ink outline-none placeholder:text-faint focus:border-accent/50"
                />
              </label>

              <div>
                <p className="text-[11px] uppercase tracking-wider text-faint">Plan</p>
                <div className="mt-1.5 grid grid-cols-2 gap-2">
                  {PLANS.map((tier) => (
                    <button
                      key={tier.label}
                      type="button"
                      onClick={() => setPlan(tier.id)}
                      className={
                        "rounded-lg border px-3 py-2 text-left text-xs transition-colors " +
                        (plan === tier.id
                          ? "border-accent/40 bg-accent-soft text-accent-hi"
                          : "border-hairline hover:border-hairline/80 hover:bg-surface")
                      }
                    >
                      <span className="block font-medium">{tier.label}</span>
                      <span className="block text-[10px] text-faint">{tier.hint}</span>
                    </button>
                  ))}
                </div>
              </div>

              {error && <p className="text-xs text-danger">{error}</p>}
            </div>
          )}

          {/* ----------------------------------------------------- login */}
          {step === "login" && account && (
            <div className="flex flex-col gap-4">
              {loginState === "logged_in" || account.login_state === "logged_in" ? (
                <div className="flex flex-col items-center gap-3 py-6 text-center">
                  <span className="dot-live size-2.5 rounded-full bg-success" />
                  <p className="text-sm font-medium">Signed in and ready</p>
                  <p className="text-xs text-muted">
                    {importResult?.login_note ?? "This account is authenticated."}
                  </p>
                </div>
              ) : (
                <>
                  <div>
                    <p className="text-sm font-medium">Sign in to finish setting up “{account.label}”.</p>
                    <p className="mt-1 text-xs leading-relaxed text-muted">
                      This is Claude's own sign-in — AI Juggler never sees your credentials.
                      {importResult?.login_note && (
                        <span className="mt-2 block rounded-lg border border-hairline bg-canvas px-3 py-2 text-[11px] leading-relaxed text-faint">
                          {importResult.login_note}
                        </span>
                      )}
                    </p>
                  </div>

                  <div className="rounded-xl border border-hairline bg-canvas px-3 py-2.5 font-mono text-[11px] leading-relaxed text-live">
                    {command || "…"}
                  </div>

                  <div className="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      variant="primary"
                      onClick={async () => {
                        const terminal = await terminalSpawnLogin(account.id);
                        openLoginTerminal(terminal.id, `Sign in · ${account.label}`);
                      }}
                    >
                      <IconTerminal width={14} height={14} />
                      Sign in inside AI Juggler
                    </Button>
                    <Button size="sm" variant="secondary" onClick={() => void loginOpenWeb(HARNESS)}>
                      Open claude.ai
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={async () => {
                        await navigator.clipboard.writeText(command);
                        setCopied(true);
                        window.setTimeout(() => setCopied(false), 1500);
                      }}
                    >
                      {copied ? "Copied" : "Copy command"}
                    </Button>
                    <Button size="sm" variant="secondary" onClick={() => void recheck()}>
                      Re-check
                    </Button>
                  </div>

                  {loginState === "logged_out" && (
                    <p className="text-[11px] text-faint">
                      Not signed in yet — complete Claude's sign-in (the terminal opened in the
                      panel), then Re-check.
                    </p>
                  )}
                </>
              )}
            </div>
          )}

          {/* -------------------------------------------------- summary */}
          {step === "summary" && account && (
            <div className="flex flex-col gap-4">
              <div className="flex items-center gap-2">
                <span className="dot-live size-2 rounded-full bg-success" />
                <p className="text-sm font-medium">“{account.label}” is ready</p>
                {account.plan && <Chip tone="accent">{account.plan}</Chip>}
              </div>

              <ul className="flex flex-col gap-2 text-xs leading-relaxed text-muted">
                <li className="flex gap-2">
                  <span className="text-success">✓</span>
                  {importing
                    ? `Imported ${importResult ? importResult.files_copied : 0} files${
                        importResult ? ` (${humanBytes(importResult.bytes_copied)})` : ""
                      } into a private, isolated copy` +
                      (importResult && importResult.skipped.length > 0
                        ? ` — ${importResult.skipped.length} cache folders skipped`
                        : "")
                    : "Created a private, isolated account with a “Main” profile"}
                </li>
                {importing && (
                  <li className="flex gap-2">
                    <span className="text-success">✓</span>
                    Your original at{" "}
                    <span className="font-mono text-[10.5px]">{importing.path}</span> was not
                    modified
                  </li>
                )}
                <li className="flex gap-2">
                  <span className="text-success">✓</span>
                  Launch it anytime via <span className="text-ink">New session</span> in the rail,
                  or from a project's “launch with” list
                </li>
              </ul>

              {!hasLiveBackend() && (
                <p className="text-[11px] text-faint">(preview mode — data shown is sample data)</p>
              )}
            </div>
          )}
        </div>

        {/* Footer actions */}
        <div className="flex items-center gap-2 border-t border-hairline/60 px-6 py-4">
          {step === "identity" && (
            <Button size="sm" variant="secondary" onClick={() => setStep("detect")}>
              Back
            </Button>
          )}
          {step === "login" && (
            <Button size="sm" variant="secondary" onClick={() => setStep("identity")}>
              Back
            </Button>
          )}
          <span className="flex-1" />
          {error && step === "identity" && <span className="text-[11px] text-danger">{error}</span>}
          {step === "identity" && (
            <Button
              size="sm"
              variant="primary"
              disabled={busy || label.trim().length === 0}
              onClick={() => void createTheAccount()}
            >
              {busy ? "Setting up…" : importing ? "Import account" : "Create account"}
            </Button>
          )}
          {step === "login" && (
            <Button
              size="sm"
              variant="primary"
              onClick={() => {
                onAccountCreated();
                setStep("summary");
              }}
            >
              {loginState === "logged_in" ? "Continue" : "I'll sign in later"}
            </Button>
          )}
          {step === "summary" && (
            <Button
              size="sm"
              variant="primary"
              onClick={() => {
                onAccountCreated();
                onClose();
              }}
            >
              Done
            </Button>
          )}
        </div>
      </Card>
    </div>
  );
}
