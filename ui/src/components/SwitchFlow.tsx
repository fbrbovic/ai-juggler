/**
 * AJ-E205 — the switch execution surface: the two-choice freeze
 * dialog (§5.2 — exactly Finish-turn or Stop-now; there is no "copy
 * live and hope"), preflight warnings rendered before anything
 * moves, determinate labelled stages while executing (never a bare
 * spinner), and the §7.1 source-safe failure banner. Focus is
 * trapped in the dialog; Esc cancels (the source keeps running);
 * ⏎ confirms; stage transitions announce via aria-live=polite.
 */
import { useEffect, useRef, useState } from "react";
import {
  switchPreflight,
  switchExecute,
  type PreflightInfoData,
} from "../lib/bridge";

type Phase =
  | { kind: "preflight" }
  | { kind: "confirm"; preflight: PreflightInfoData }
  | { kind: "executing"; startedAt: number }
  | { kind: "done"; sessionId: string }
  | { kind: "failed"; message: string };

const STAGES = [
  "Freeze",
  "Snapshot",
  "Stage & verify",
  "Launch",
  "Record",
] as const;

export function SwitchFlow({
  sessionId,
  targetProfileId,
  targetLabel,
  onClose,
  onSwitched,
}: {
  sessionId: string;
  targetProfileId: string;
  targetLabel: string;
  onClose: () => void;
  onSwitched: (sessionId: string) => void;
}): JSX.Element {
  const [phase, setPhase] = useState<Phase>({ kind: "preflight" });
  const [freeze, setFreeze] = useState<"finish-turn" | "stop-now">("finish-turn");
  const dialogRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let alive = true;
    switchPreflight(sessionId, targetProfileId)
      .then((preflight) => {
        if (alive) setPhase({ kind: "confirm", preflight });
      })
      .catch((cause) => {
        if (alive) setPhase({ kind: "failed", message: String(cause) });
      });
    return () => {
      alive = false;
    };
  }, [sessionId, targetProfileId]);

  // Focus trap + Esc/⏎ handling (2.1.2/2.1.1 — the dialog is the
  // only operable surface while open).
  useEffect(() => {
    const node = dialogRef.current;
    node?.querySelector<HTMLElement>("button, input")?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key === "Enter" && phase.kind === "confirm") {
        event.preventDefault();
        void execute();
      }
      if (event.key === "Tab" && node) {
        const focusables = Array.from(
          node.querySelectorAll<HTMLElement>("button, input"),
        );
        if (focusables.length === 0) return;
        const first = focusables[0];
        const last = focusables[focusables.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase.kind, onClose]);

  const execute = async () => {
    setPhase({ kind: "executing", startedAt: Date.now() });
    try {
      const result = await switchExecute(sessionId, targetProfileId, freeze);
      setPhase({ kind: "done", sessionId: result.session_id });
    } catch (cause) {
      setPhase({ kind: "failed", message: String(cause) });
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6 backdrop-blur-sm">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={`Switch to ${targetLabel}`}
        data-testid="switch-flow"
        className="w-full max-w-md rounded-xl border border-hairline bg-surface p-5 shadow-card"
      >
        {phase.kind === "preflight" && (
          <p className="text-xs text-muted" data-testid="switch-preflighting">
            Checking the switch (preflight)…
          </p>
        )}

        {phase.kind === "confirm" && (
          <div className="flex flex-col gap-3">
            <h3 className="text-sm font-semibold">
              Switch to {targetLabel}
              <span className="ml-2 rounded border border-accent/50 px-1 text-[10px] text-accent">
                {phase.preflight.fidelity} · beta
              </span>
            </h3>
            <p className="text-[11px] text-muted" data-testid="switch-experimental">
              {phase.preflight.experimental_copy}
            </p>
            {phase.preflight.warnings.length > 0 && (
              <ul className="flex flex-col gap-1" data-testid="switch-warnings">
                {phase.preflight.warnings.map((warning) => (
                  <li key={warning} className="rounded border border-warning/40 px-2 py-1 text-[11px] text-warning">
                    {warning}
                  </li>
                ))}
              </ul>
            )}
            <fieldset className="flex flex-col gap-1.5">
              <legend className="text-[11px] uppercase tracking-wider text-muted">
                {phase.preflight.source_live ? "Freeze the source first" : "The source is at rest"}
              </legend>
              <label className="flex items-center gap-2 text-xs">
                <input
                  type="radio"
                  name="freeze"
                  checked={freeze === "finish-turn"}
                  onChange={() => setFreeze("finish-turn")}
                />
                Finish this turn, then switch <span className="text-muted">(default)</span>
              </label>
              <label className="flex items-center gap-2 text-xs">
                <input
                  type="radio"
                  name="freeze"
                  checked={freeze === "stop-now"}
                  onChange={() => setFreeze("stop-now")}
                />
                Stop now and switch <span className="text-muted">(partial turn marked truncated)</span>
              </label>
            </fieldset>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={onClose}
                className="rounded-lg border border-hairline px-3 py-1.5 text-xs hover:bg-surface-2"
              >
                Cancel <span className="text-muted">(Esc)</span>
              </button>
              <button
                type="button"
                onClick={() => void execute()}
                data-testid="switch-confirm"
                className="rounded-lg border border-accent/40 bg-accent-soft px-3 py-1.5 text-xs text-accent-hi hover:border-accent/60"
              >
                Switch ▸ <span className="text-muted">(⏎)</span>
              </button>
            </div>
          </div>
        )}

        {phase.kind === "executing" && (
          <div className="flex flex-col gap-2" aria-live="polite" data-testid="switch-executing">
            <h3 className="text-sm font-semibold">Switching to {targetLabel}…</h3>
            {/* Honest determinism: the stages are NAMED as they run
                (never a bare spinner), but no stage claims completion
                the engine has not reported — the single command
                reports only its terminal state. */}
            <ol className="flex flex-col gap-1">
              {STAGES.map((stage) => (
                <li key={stage} className="text-[11px] text-muted">
                  · {stage}
                </li>
              ))}
            </ol>
            <p className="text-[10px] text-muted" data-testid="switch-elapsed">
              Running — freeze → stage → verify → launch in progress. The source stays resumable
              throughout.
            </p>
          </div>
        )}

        {phase.kind === "done" && (
          <div className="flex flex-col gap-3" data-testid="switch-done">
            <h3 className="text-sm font-semibold">Switched to {targetLabel}</h3>
            <p className="text-[11px] text-muted">
              You switched — the continuation instance is starting. The source rests and stays
              resumable from the graph.
            </p>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={() => onSwitched(phase.sessionId)}
                className="rounded-lg border border-accent/40 bg-accent-soft px-3 py-1.5 text-xs text-accent-hi"
              >
                Go to the new instance
              </button>
            </div>
          </div>
        )}

        {phase.kind === "failed" && (
          <div className="flex flex-col gap-3" data-testid="switch-failed">
            <h3 className="text-sm font-semibold text-warning">Switch didn’t complete</h3>
            <p className="rounded border border-warning/40 px-2 py-1 text-[11px] text-warning">
              {phase.message}
            </p>
            <p className="text-[11px] text-muted">
              The source is safe — resume it exactly where you left it.
            </p>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={onClose}
                className="rounded-lg border border-hairline px-3 py-1.5 text-xs hover:bg-surface-2"
              >
                Close
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
