import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Card, Chip } from "../components/primitives";
import { IconJuggler, IconProfiles, IconTerminal } from "../components/icons";
import {
  accountImport,
  accountLoginCommand,
  accountLoginState,
  createAccount,
  environmentScanFull,
  hasLiveBackend,
  loginOpenWeb,
  providersDiscover,
  scanCancel,
  terminalSpawnLogin,
  type ProbeRowData,
  type ScanReportData,
  type AccountInfo,
  type DiscoveredInfo,
  type ImportResult,
  type ProviderInstallData,
  errorText,
} from "../lib/bridge";

const PLANS: { id: string | null; label: string; hint: string }[] = [
  { id: null, label: "Not sure yet", hint: "you can set this later" },
  { id: "free", label: "Free", hint: "trying things out" },
  { id: "pro", label: "Pro", hint: "regular daily use" },
  // AJ-E303 Req.4: Max 5× joins the list (Deep-03 §2.1) — never
  // inferred, chosen.
  { id: "max-5x", label: "Max 5×", hint: "regular heavy use" },
  { id: "max-20x", label: "Max 20×", hint: "extreme parallel work" },
];

const STEPS = ["install", "detect", "identity", "login", "summary"] as const;
type Step = (typeof STEPS)[number];

function humanBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** One Step-0 provider row: Installed vX · Not installed · Installed
 * (vX, unverified). A missing binary is a gate, not an error. */
function ProviderRow({
  install,
  selected,
  onNotice,
}: {
  install: ProviderInstallData;
  selected: boolean;
  onNotice: () => void;
}): JSX.Element {
  const missing = install.status === "not_installed";
  const unverified = install.status === "unverified";
  const [showHelp, setShowHelp] = useState(false);
  return (
    <div
      data-testid="provider-row"
      data-status={install.status}
      data-selected={selected && !missing ? "true" : "false"}
      className={
        "flex flex-col gap-2 rounded-xl border px-4 py-3.5 " +
        (selected && !missing ? "border-accent/40 bg-accent-soft" : "border-hairline")
      }
    >
      <button
        type="button"
        aria-pressed={selected && !missing}
        aria-expanded={missing ? showHelp : undefined}
        onClick={() => {
          // A missing binary toggles the install help (the GATE is
          // the disabled Continue button — the row itself never
          // advances, and a disabled button can't even be clicked).
          if (missing) {
            setShowHelp((open) => !open);
            if (!showHelp) onNotice();
          }
        }}
        className="flex items-center gap-2 text-left"
      >
        <span className="min-w-0 flex-1 text-sm font-medium">{install.display_name}</span>
        {install.status === "installed" && (
          <Chip tone="success">Installed v{install.version}</Chip>
        )}
        {unverified && <Chip tone="warning">Installed (v{install.version}, unverified)</Chip>}
        {missing && <Chip tone="neutral">Not installed</Chip>}
        {install.status === "probe_failed" && <Chip tone="warning">Couldn't check</Chip>}
      </button>
      {unverified && install.notes.length > 0 && (
        <p className="text-[11px] leading-relaxed text-muted">{install.notes.join(" · ")}</p>
      )}
      {missing && showHelp && (
        <div
          className="flex flex-col gap-2 rounded-lg border border-hairline bg-canvas px-3 py-2.5"
          data-testid="install-help"
        >
          <p className="text-[11px] leading-relaxed text-muted">
            {install.display_name} isn't installed on this Mac. AI Juggler drives the CLI
            itself — install it first, or continue without it and set it up later.
          </p>
          <div className="flex gap-2">
            <a
              href={install.install_url}
              target="_blank"
              rel="noreferrer"
              className="rounded-lg border border-accent/40 px-2.5 py-1 text-[11px] text-accent-hi transition-colors hover:border-accent/60"
            >
              Install {install.display_name} →
            </a>
            <button
              type="button"
              onClick={() => setShowHelp(false)}
              className="rounded-lg border border-hairline px-2.5 py-1 text-[11px] text-muted transition-colors hover:border-accent/30"
            >
              Continue without it
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/** Denied probes in a report (the wizard's honest-empty guard: an
 * empty result with ANY denied probe never reads "no setups
 * found"). */
function reportDeniedCount(probes: ProbeRowData[]): number {
  return probes.filter((probe) => probe.health === "denied").length;
}

/** One probe row inside Details (text health, never colour-only). */
function ScanProbeRow({ probe }: { probe: ProbeRowData }): JSX.Element {
  const tone =
    probe.health === "ok"
      ? "text-muted"
      : probe.health === "partial"
        ? "text-warning"
        : "text-danger";
  const word =
    probe.health === "ok" ? "checked" : probe.health === "partial" ? "partial" : "couldn't check";
  return (
    <li className="flex flex-col text-[11px] leading-relaxed" data-testid="scan-probe-row">
      <span>
        <span className="text-ink">{probe.label}</span>{" "}
        <span className={tone}>{word}</span>
        <span className="text-muted"> — {probe.detail}</span>
      </span>
      {probe.reason && <span className="text-muted">{probe.reason}</span>}
    </li>
  );
}

/** One candidate card (pass2/03 §4.2 wireframe, AJ-E303): line 1
 * provider · path · login chip · confidence words; line 2 the
 * inference line ("Likely: Work · Active this week"); actions Import
 * / Edit / Ignore; already-managed renders disabled inline. All notes
 * render (never notes[0]-truncated); the keychain caveat reads
 * "signed in here — you'll sign in once more after import". */
function CandidateCards({
  installs,
  edits,
  editing,
  onEdit,
  onEditField,
  onImport,
  onIgnore,
}: {
  installs: DiscoveredInfo[];
  edits: Record<string, { purpose: string; plan: string | null }>;
  editing: string | null;
  onEdit: (path: string) => void;
  onEditField: (path: string, patch: { purpose?: string; plan?: string | null }) => void;
  onImport: (found: DiscoveredInfo, purpose: string) => void;
  onIgnore: (path: string) => void;
}): JSX.Element {
  const confidenceWord = (wire: string): string =>
    wire === "found" ? "Found" : wire === "likely" ? "Likely" : "Guessed";
  return (
    <ul className="flex flex-col gap-2" data-testid="candidate-cards">
      {installs.map((found) => {
        const edit = edits[found.path];
        const purpose = edit?.purpose || found.inference.purpose;
        const plan = edit?.plan ?? null;
        const managed = found.managed;
        const loginWord =
          found.inference.login === "signed-in"
            ? "Signed in"
            : found.inference.login === "maybe-expired"
              ? "May be expired"
              : "Not signed in";
        return (
          <li
            key={found.path}
            data-testid="candidate-card"
            data-managed={managed ? "true" : "false"}
            data-confidence={found.inference.purpose_confidence}
            // Managed cards stay readable (no opacity dimming): the
            // disabled actions + inline label carry the state.
            className={
              "flex flex-col gap-2 rounded-xl border px-4 py-3.5 " +
              (managed ? "border-hairline/60" : "border-hairline")
            }
          >
            <div className="flex items-center gap-2">
              <IconProfiles width={14} height={14} />
              <span className="text-xs font-medium">{found.harness === "claude-code" ? "Claude" : found.harness}</span>
              <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-ink" title={found.path}>
                {found.path}
              </span>
              <Chip
                tone={
                  found.inference.login === "signed-in"
                    ? "success"
                    : found.inference.login === "maybe-expired"
                      ? "warning"
                      : "neutral"
                }
              >
                {loginWord}
              </Chip>
            </div>
            <p className="text-[11px] leading-relaxed text-muted">
              <span className="text-ink">
                {confidenceWord(found.inference.purpose_confidence)}: {purpose}
              </span>{" "}
              · {found.inference.activity_words}
              {found.signals.length > 1 && (
                <span className="text-muted"> · {found.signals.length} signals merged</span>
              )}
              {found.bytes > 0 && (
                <span className="text-muted"> · {humanBytes(found.bytes)} of state</span>
              )}
            </p>
            {found.inference.keychain_bound && (
              <p className="text-[11px] text-warning" data-testid="keychain-caveat">
                signed in here — you'll sign in once more after import (the keychain entry is
                path-locked)
              </p>
            )}
            {found.notes.length > 0 && (
              <ul className="flex flex-col gap-0.5 text-[10px] leading-relaxed text-muted">
                {found.notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
            )}
            {editing === found.path ? (
              <div className="flex flex-col gap-2 rounded-lg border border-hairline bg-canvas px-3 py-2.5" data-testid="inline-edit">
                <label className="block text-[10px] uppercase tracking-wider text-faint">
                  Name
                  <input
                    value={purpose}
                    onChange={(event) => onEditField(found.path, { purpose: event.target.value })}
                    className="mt-1 w-full rounded-lg border border-hairline bg-canvas px-2.5 py-1.5 text-xs text-ink outline-none focus:border-accent/50"
                    data-testid="inline-edit-purpose"
                  />
                </label>
                <div>
                  <p className="text-[10px] uppercase tracking-wider text-faint">Plan</p>
                  <div className="mt-1 flex flex-wrap gap-1.5">
                    {PLANS.map((tier) => (
                      <button
                        key={tier.label}
                        type="button"
                        onClick={() => onEditField(found.path, { plan: tier.id })}
                        className={
                          "rounded-lg border px-2 py-1 text-[10px] transition-colors " +
                          (plan === tier.id
                            ? "border-accent/40 bg-accent-soft text-accent-hi"
                            : "border-hairline text-muted hover:border-accent/30")
                        }
                      >
                        {tier.label}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            ) : null}
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                disabled={managed}
                aria-label={
                  managed
                    ? `Already in AI Juggler: ${found.path}`
                    : `Import ${found.path} as “${purpose}”`
                }
                title={managed ? "Already managed by AI Juggler" : undefined}
                onClick={() => onImport(found, purpose)}
                className="rounded-lg border border-accent/40 bg-accent-soft px-2.5 py-1 text-[11px] text-accent-hi transition-colors hover:border-accent/60 disabled:border-hairline disabled:bg-transparent disabled:text-faint"
                data-testid="import-as"
              >
                {managed ? "Already in AI Juggler" : `Import as “${purpose}”`}
              </button>
              <button
                type="button"
                disabled={managed}
                onClick={() => onEdit(found.path)}
                aria-label={`Edit the inferred name and plan for ${found.path}`}
                className="rounded-lg border border-hairline px-2.5 py-1 text-[11px] text-muted transition-colors hover:border-accent/30 disabled:opacity-50"
              >
                Edit
              </button>
              <button
                type="button"
                disabled={managed}
                onClick={() => onIgnore(found.path)}
                aria-label={`Ignore ${found.path} for this import`}
                className="rounded-lg border border-hairline px-2.5 py-1 text-[11px] text-muted transition-colors hover:border-accent/30 disabled:opacity-50"
              >
                Ignore
              </button>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

/**
 * The onboarding wizard (ADR-0006, spec §12.1; AJ-E301): one shell
 * for EVERY Provider (`provider` parameter — no baked-in harness),
 * gated at Step 0 by the install probe so a missing CLI is caught
 * before any account exists. Detect what exists → offer import or a
 * guided fresh start → sign in with the vendor's own flow → a summary
 * that says exactly what happened and what wasn't touched.
 *
 * A11y: `role="dialog"` + focus trap + return-focus; Esc closes only
 * when idle; the close control is disabled while an import/create is
 * in flight (with an explicit "still running" confirm as the escape
 * hatch); focus moves to the primary button on every step change.
 */
export function OnboardingWizard({
  provider,
  onClose,
  onAccountCreated,
  openLoginTerminal,
}: {
  /** The Provider this run sets up (AJ-E301 Req.1 — explicit at every
   * call site: first-run, Accounts → Add account, summary loop-back). */
  provider: string;
  onClose: () => void;
  onAccountCreated: () => void;
  openLoginTerminal: (id: string, title: string) => void;
}) {
  const [step, setStep] = useState<Step>("install");
  const [installs, setInstalls] = useState<ProviderInstallData[] | null>(null);
  const [report, setReport] = useState<ScanReportData | null>(null);
  // AJ-E303: inline edits (path → purpose/plan) + reversible ignores.
  // Editing NEVER re-runs the scan (Req.5 — pinned by test).
  const [edits, setEdits] = useState<Record<string, { purpose: string; plan: string | null }>>({});
  const [editing, setEditing] = useState<string | null>(null);
  const [ignored, setIgnored] = useState<Record<string, boolean>>({});
  const [showIgnored, setShowIgnored] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [importing, setImporting] = useState<DiscoveredInfo | null>(null);
  const [label, setLabel] = useState("");
  const [plan, setPlan] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retryable, setRetryable] = useState(false);
  const [confirmClose, setConfirmClose] = useState(false);
  const [account, setAccount] = useState<AccountInfo | null>(null);
  const [importResult, setImportResult] = useState<ImportResult | null>(null);
  const [loginState, setLoginState] = useState<string | null>(null);
  const [command, setCommand] = useState("");
  const [copied, setCopied] = useState(false);

  const dialogRef = useRef<HTMLDivElement | null>(null);
  const primaryRef = useRef<HTMLButtonElement | null>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);

  // Step 0 probes the Providers (off the UI thread; a missing binary
  // is a row, not an error).
  useEffect(() => {
    let cancelled = false;
    void providersDiscover().then((found) => {
      if (!cancelled) setInstalls(found);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // Step 1 runs the 10-signal scan automatically once the gate is
  // passed — progress streams as "n of 10"; closing cancels.
  useEffect(() => {
    if (step !== "detect" || report !== null) return;
    let cancelled = false;
    void environmentScanFull((done, total) => {
      if (!cancelled) setProgress({ done, total });
    }).then((found) => {
      if (!cancelled) {
        setReport(found);
        setProgress(null);
      }
    });
    return () => {
      cancelled = true;
      void scanCancel();
    };
  }, [step, report]);

  useEffect(() => {
    if (account) void accountLoginCommand(account.id).then(setCommand);
  }, [account]);

  // Focus trap + return-focus + Esc (idle only — a running setup
  // never dies by stray key).
  useEffect(() => {
    restoreFocusRef.current = document.activeElement as HTMLElement | null;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key !== "Tab") return;
      const focusables = dialogRef.current?.querySelectorAll<HTMLElement>(
        "button:not(:disabled), a[href], input, [tabindex]:not([tabindex='-1'])",
      );
      if (focusables === undefined || focusables.length === 0) return;
      const first = focusables[0]!;
      const last = focusables[focusables.length - 1]!;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      restoreFocusRef.current?.focus?.();
    };
  }, [busy, onClose]);

  // Focus moves to the primary button on every step change.
  useEffect(() => {
    primaryRef.current?.focus();
  }, [step]);

  const gate = useMemo(() => {
    const mine = installs?.find((row) => row.provider === provider) ?? null;
    return { mine, ready: mine !== null && mine.status !== "not_installed" };
  }, [installs, provider]);

  const title = useMemo(() => {
    if (importing) return `Import “${importing.path.split("/").pop() || importing.path}”`;
    return "Add an account";
  }, [importing]);

  const createTheAccount = async () => {
    setBusy(true);
    setError(null);
    try {
      if (importing) {
        const result = await accountImport(provider, importing.path, label.trim(), plan);
        setImportResult(result);
        setAccount(result.account);
      } else {
        const created = await createAccount(provider, label.trim(), plan);
        setAccount(created);
      }
      setBusy(false);
      setStep("login");
    } catch (cause) {
      setBusy(false);
      setRetryable(
        cause !== null &&
          typeof cause === "object" &&
          (cause as { retryable?: unknown }).retryable === true,
      );
      setError(errorText(cause));
    }
  };

  const recheck = async () => {
    if (!account) return;
    setLoginState(await accountLoginState(account.id));
    onAccountCreated();
  };

  const requestClose = () => {
    // In-flight setup never dies silently (Req.5): block the close,
    // offer the explicit "it will appear in Accounts" confirm.
    if (busy) {
      setConfirmClose(true);
      return;
    }
    onClose();
  };

  const providerName =
    installs?.find((row) => row.provider === provider)?.display_name ?? provider;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6 backdrop-blur-sm">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={`${providerName} setup wizard`}
        className="flex max-h-[86vh] w-full max-w-xl flex-col overflow-hidden rounded-card border border-hairline bg-canvas shadow-card"
        data-testid="onboarding-wizard"
      >
        {/* Wizard header: progress + close */}
        <div className="flex items-center gap-3 border-b border-hairline/60 px-6 py-4">
          <span className="text-accent">
            <IconJuggler width={18} height={18} />
          </span>
          <h3 className="text-sm font-semibold">{title}</h3>
          <div className="ml-auto flex items-center gap-1.5">
            {STEPS.map((s, i) => (
              <span
                key={s}
                className={
                  "h-1 rounded-full transition-all " +
                  (STEPS.indexOf(step) >= i ? "w-5 bg-accent" : "w-2 bg-hairline")
                }
              />
            ))}
          </div>
          <button
            type="button"
            onClick={requestClose}
            aria-label="Close setup"
            title={busy ? "Setup is still running — it will appear in Accounts" : undefined}
            className="ml-3 rounded-md px-2 py-1 text-[11px] text-faint hover:bg-surface hover:text-ink"
          >
            ✕
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
          {/* -------------------------------------------------- install */}
          {step === "install" && (
            <div className="flex flex-col gap-4" data-testid="wizard-step-install">
              <div>
                <p className="text-sm font-medium">First, the CLI itself.</p>
                <p className="mt-1 text-xs leading-relaxed text-muted">
                  AI Juggler drives each provider's own command-line tool. Here's what's
                  installed on this Mac — you can't continue with a provider that isn't
                  installed yet (that never ends well three steps later).
                </p>
              </div>

              {installs === null ? (
                <div className="flex flex-col gap-2" data-testid="install-checking">
                  {[0, 1].map((row) => (
                    <div
                      key={row}
                      className="h-16 animate-pulse rounded-xl border border-hairline bg-surface"
                    />
                  ))}
                </div>
              ) : (
                <div className="flex flex-col gap-2">
                  {installs.map((install) => (
                    <ProviderRow
                      key={install.provider}
                      install={install}
                      selected={provider === install.provider}
                      onNotice={() => {
                        /* Informing only — the gate lives on Continue. */
                      }}
                    />
                  ))}
                </div>
              )}
              {error && <p className="text-xs text-danger">{error}</p>}
            </div>
          )}

          {/* --------------------------------------------------- detect */}
          {step === "detect" && (
            <div className="flex flex-col gap-4">
              <div>
                <p className="text-sm font-medium">
                  Let's look at what {providerName} already has.
                </p>
                <p className="mt-1 text-xs leading-relaxed text-muted">
                  AI Juggler scans the usual spots for existing setups — your standard
                  install, configs left by other tools, anything an AI assistant may have
                  created. Importing keeps your settings and history; the original is never
                  touched.
                </p>
              </div>

              {report === null ? (
                <div
                  className="flex items-center gap-3 rounded-xl border border-hairline px-4 py-5"
                  aria-live="polite"
                  data-testid="scan-progress"
                >
                  <span className="dot-live size-2 rounded-full bg-live" />
                  <span className="text-xs text-muted">
                    Checking your machine… {progress ? `${progress.done} of ${progress.total} checks` : "…"}
                  </span>
                </div>
              ) : reportDeniedCount(report.probes) > 0 ? (
                <div
                  className="flex flex-col gap-2 rounded-xl border border-hairline px-4 py-4"
                  data-testid="scan-denied"
                >
                  <p className="text-xs leading-relaxed text-muted">
                    We couldn't check {reportDeniedCount(report.probes)}{" "}
                    {reportDeniedCount(report.probes) === 1 ? "place" : "places"} — your setups{" "}
                    {reportDeniedCount(report.probes) === 1 ? "there won't" : "there won't"}{" "}
                    appear.
                  </p>
                  {report.cancelled && (
                    <p className="text-[11px] text-faint">The check was stopped early.</p>
                  )}
                </div>
              ) : report.installs.length === 0 ? (
                <div className="rounded-xl border border-dashed border-hairline px-4 py-5 text-center">
                  <p className="text-xs text-muted">
                    No existing {providerName} setups found — we'll create a fresh one.
                  </p>
                </div>
              ) : (
                <CandidateCards
                  installs={report.installs.filter((found) => !ignored[found.path])}
                  edits={edits}
                  editing={editing}
                  onEdit={(path) => setEditing((current) => (current === path ? null : path))}
                  onEditField={(path, patch) =>
                    setEdits((current) => ({
                      ...current,
                      [path]: {
                        purpose: patch.purpose ?? current[path]?.purpose ?? "",
                        plan: patch.plan !== undefined ? patch.plan : (current[path]?.plan ?? null),
                      },
                    }))
                  }
                  onImport={(found, purpose) => {
                    setImporting(found);
                    setLabel(purpose);
                    setStep("identity");
                  }}
                  onIgnore={(path) =>
                    setIgnored((current) => ({ ...current, [path]: true }))
                  }
                />
              )}
              {report !== null &&
                report.installs.filter((found) => ignored[found.path]).length > 0 && (
                  <div className="flex flex-col gap-1.5">
                    <button
                      type="button"
                      aria-expanded={showIgnored}
                      onClick={() => setShowIgnored((open) => !open)}
                      className="self-start text-[11px] text-muted underline-offset-2 hover:text-ink hover:underline"
                      data-testid="ignored-toggle"
                    >
                      Ignored ({report.installs.filter((found) => ignored[found.path]).length}){" "}
                      {showIgnored ? "▾" : "▸"}
                    </button>
                    {showIgnored && (
                      <ul className="flex flex-col gap-1" data-testid="ignored-list">
                        {report.installs
                          .filter((found) => ignored[found.path])
                          .map((found) => (
                            <li
                              key={found.path}
                              className="flex items-center gap-2 rounded-lg border border-hairline px-3 py-1.5 text-[11px]"
                            >
                              <span className="min-w-0 flex-1 truncate font-mono text-muted">
                                {found.path}
                              </span>
                              <button
                                type="button"
                                onClick={() =>
                                  setIgnored((current) => {
                                    const next = { ...current };
                                    delete next[found.path];
                                    return next;
                                  })
                                }
                                className="text-muted underline hover:text-ink"
                              >
                                Unignore
                              </button>
                            </li>
                          ))}
                      </ul>
                    )}
                  </div>
                )}

              {report !== null && (
                <>
                  <button
                    type="button"
                    onClick={() => {
                      setImporting(null);
                      setLabel("");
                      setStep("identity");
                    }}
                    className="self-start text-xs text-muted underline-offset-2 hover:text-ink hover:underline"
                  >
                    {report.installs.length > 0
                      ? "None of these — add a fresh account"
                      : "Continue with a fresh account →"}
                  </button>
                  <div className="flex flex-col gap-1.5">
                    <button
                      type="button"
                      aria-expanded={detailsOpen}
                      onClick={() => setDetailsOpen((open) => !open)}
                      className="self-start text-[11px] text-muted underline-offset-2 hover:text-ink hover:underline"
                      data-testid="scan-details-toggle"
                    >
                      Details {detailsOpen ? "▾" : "▸"}
                    </button>
                    {detailsOpen && (
                      <ul
                        className="flex flex-col gap-1 rounded-lg border border-hairline bg-canvas px-3 py-2"
                        data-testid="scan-details"
                      >
                        {report.probes.map((probe) => (
                          <ScanProbeRow key={probe.id} probe={probe} />
                        ))}
                      </ul>
                    )}
                  </div>
                </>
              )}
            </div>
          )}

          {/* ------------------------------------------------- identity */}
          {step === "identity" && (
            <div className="flex flex-col gap-4">
              {importing ? (
                <div className="rounded-xl border border-hairline bg-canvas px-4 py-3 text-xs leading-relaxed text-muted">
                  Importing from{" "}
                  <span className="font-mono text-[11px]">{importing.path}</span>
                  {importing.login === "logged_in" &&
                    " — your settings and session history come along."}
                  {importing.login === "logged_out" &&
                    " — its settings come along; you'll sign in after."}
                </div>
              ) : (
                <p className="text-xs leading-relaxed text-muted">
                  Each account is a separate {providerName} login, fully isolated — what
                  happens in one never touches the others, or your existing install.
                </p>
              )}

              <label className="block text-[11px] uppercase tracking-wider text-faint">
                What will you use this account for?
                <input
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

              {error && (
                <div className="flex flex-col gap-1.5" data-testid="wizard-error">
                  <p className="text-xs text-danger">{error}</p>
                  {retryable && (
                    <button
                      type="button"
                      onClick={() => void createTheAccount()}
                      className="self-start text-[11px] text-muted underline hover:text-ink"
                    >
                      Try again
                    </button>
                  )}
                </div>
              )}
            </div>
          )}

          {/* ---------------------------------------------------- login */}
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
                    <p className="text-sm font-medium">
                      Sign in to finish setting up “{account.label}”.
                    </p>
                    <p className="mt-1 text-xs leading-relaxed text-muted">
                      This is {providerName}'s own sign-in — AI Juggler never sees your
                      credentials.
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
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => void loginOpenWeb(provider)}
                    >
                      Open the {providerName} site
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
                      Not signed in yet — complete {providerName}'s sign-in (the terminal
                      opened in the panel), then Re-check.
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
                <span className="dot-live size-2.5 rounded-full bg-success" />
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
                    : `Created a private, isolated ${providerName} account with a “Main” profile`}
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
                  Launch it anytime via <span className="text-ink">New session</span> in the
                  rail, or from a project's “launch with” list
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
          {step === "detect" && (
            <Button size="sm" variant="secondary" onClick={() => setStep("install")}>
              Back
            </Button>
          )}
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
          {step === "install" && (
            <Button
              ref={primaryRef}
              size="sm"
              variant="primary"
              disabled={installs === null || !gate.ready}
              title={
                installs !== null && !gate.ready && gate.mine
                  ? `${gate.mine.display_name} isn't installed — install it first or continue without it`
                  : undefined
              }
              onClick={() => setStep("detect")}
              data-testid="install-continue"
            >
              Continue with {gate.mine?.display_name ?? providerName}
            </Button>
          )}
          {step === "identity" && (
            <Button
              ref={primaryRef}
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
              ref={primaryRef}
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
              ref={primaryRef}
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
      </div>

      {confirmClose && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-6">
          <Card className="w-full max-w-sm p-5" data-testid="wizard-close-confirm">
            <p className="text-sm font-medium">Setup is still running</p>
            <p className="mt-1.5 text-xs leading-relaxed text-muted">
              Closing now is safe — the account will finish setting up and appear in
              Accounts. You can re-check its sign-in there.
            </p>
            <div className="mt-4 flex justify-end gap-2">
              <Button size="sm" variant="secondary" onClick={() => setConfirmClose(false)}>
                Keep waiting
              </Button>
              <Button size="sm" variant="primary" onClick={onClose}>
                Close anyway
              </Button>
            </div>
          </Card>
        </div>
      )}
    </div>
  );
}
