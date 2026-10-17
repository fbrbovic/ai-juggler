/**
 * AJ-E305: the onboarding funnel seam. Every flow step emits one
 * `aij:funnel` CustomEvent on `window` — `{ flow, step }` with
 * string keys only (AJ-E309 forbids path/label/token-shaped values
 * in the event schema, so this seam never carries them).
 *
 * The local event store (AJ-E309) subscribes to these events; until
 * then the Playwright click-budget specs assert the sequence has no
 * missing-step gaps by listening here. Tests can also spy on
 * `funnelStep` directly.
 */

export type FunnelFlow = "a-first-install" | "b-new-project";
export type FunnelStep = string;

const seen = new Map<string, string[]>();

/** Emit one funnel step. Idempotent per (flow, step): a step the user
 * re-visits (Back) does not double-count. */
export function funnelStep(flow: FunnelFlow, step: FunnelStep): void {
  const log = seen.get(flow) ?? [];
  if (log[log.length - 1] !== step) {
    log.push(step);
    seen.set(flow, log);
  }
  window.dispatchEvent(
    new CustomEvent("aij:funnel", { detail: { flow, step } }),
  );
}

/** The ordered steps observed for a flow (test seam). */
export function funnelSeen(flow: FunnelFlow): string[] {
  return [...(seen.get(flow) ?? [])];
}

/** Forget a flow's history (test seam). */
export function funnelReset(flow?: FunnelFlow): void {
  if (flow === undefined) {
    seen.clear();
  } else {
    seen.delete(flow);
  }
}
