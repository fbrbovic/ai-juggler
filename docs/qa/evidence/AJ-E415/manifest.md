# Evidence manifest — AJ-E415 (Desktop live preview) — PARTIAL, real capture BLOCKED

Task: `docs/tasks/E4-indicators-and-inspector.md` AJ-E415.
Branch: `stage2/e4-preview`. Local runs (macOS arm64, CLT-only) 2026-10-14,
reviewer zcode-primary.

**Status: the safe subset is DONE and green; the REAL ScreenCaptureKit
capture is BLOCKED by a binding gap versus a settled law (details in
the BLOCKED row). The shipped app never crashes and never fakes a
frame — the preview renders an honest parked state until the
maintainer records the decision that unblocks it.**

| Claim | Command | Threshold | Observed | Verdict | Date | Reviewer |
|---|---|---|---|---|---|---|
| Req.1 driver (cap ≤15 fps, ≤1280 px; pause on hidden/minimised/close) | `cargo test -p juggler-app --lib preview::` (8 tests) | pause text copy-exact; frames stop when not visible | `PreviewDriver` polls at 2 fps (`LIVE_POLL` 500 ms) and scales captures to ≤1280 px wide (aspect-preserved); minimised/off-screen windows pause with `Preview paused — window not visible` (copy-exact, asserted); the UI additionally stops the stream whenever its panel tab is hidden (`visible` prop — asserted) or the Instance exits (backend `is_live` + component liveness gate) | PASS | 2026-10-14 | zcode-primary |
| Req.2 read-only | code review + component test | no input injection anywhere; only pointer action is click-to-focus | pixels flow app → panel only; the frame's only handler calls the AJ-E414 focus tiers (asserted: exactly one button wired, `calls.focus`); no synthetic click/key code exists in the path | PASS | 2026-10-14 | zcode-primary |
| Req.3 permission + permanent off switch | unit tests + UI tests + schema v11 round-trip | prompt only on first enable; per-Account off; without permission: status/transcript only | `Account.preview_opt_in` (schema v11, default OFF — round-trip + default-off tests); the capture path is entered only after the opt-in gate in `preview_start`; denial maps to the `denied` phase whose panel card names the grant and keeps status/transcript data below (asserted); the Account Overview hosts the toggle beside the D-08 cleanup switch | PASS | 2026-10-14 | zcode-primary |
| Req.4 teardown + exclusions + live indicator | `stop_flag_ends_the_driver_within_budget_three_consecutive_runs` + component test | stop <1 s of close/exit/revocation; own windows excluded; `● Live preview` + frame age while streaming | teardown measured from stop-flag→join in THREE consecutive runs (<1 s each, asserted); one capture per poll — nothing persistent survives the loop; the filter contains ONLY the target window (single-window `SCContentFilter`) so Juggler's own windows can never appear; `● Live preview` chip + `frame age Ns` render with every live frame (asserted + vision-verified) | PASS | 2026-10-14 | zcode-primary |
| Req.5 one preview at a time | `preview_start` code path | selected instance only | the backend stops any running session before spawning the next (`stop_preview_session`); additional-preview pinning + cost display require the Settings surface (no such task exists — unowned §4.17); the default single-stream behaviour is exactly what ships | PASS (default tier; pinning unowned) | 2026-10-14 | zcode-primary |
| Quality gates (scripted fixture) | `just verify` + `pnpm --dir ui test` + `visual:capture` | changing fake window renders, pauses on minimise, stops on close; denied path renders; verify green | 8/8 driver units (changing frames — every payload distinct — minimise-pause-resume, close, teardown ×3, revocation-denies-and-stops [denied is TERMINAL: last event], unsupported, exit-stop, base64 vectors); 150/150 component (7 new preview cases); 105/105 visual; verify green (with the CLT Swift-link shim, `scripts/ensure_swift_runtime.sh`, no-op on CI/Xcode machines) | PASS | 2026-10-14 | zcode-primary |
| **BLOCKED: the real capture** | `cargo test -p juggler-app --test preview_real -- --ignored --nocapture` → `real-capture-crash.log` | live ScreenCaptureKit frames | the safe API chain (`SCShareableContent::get` → window filter → `SCScreenshotManager::capture_image` → `CGImage::save_png`) links and type-checks, but the binding REQUIRES one `unsafe { sc_initialize_core_graphics() }` call at process start — its own examples do exactly this "to prevent CGS_REQUIRE_INIT crashes" — and the workspace law `unsafe_code = "forbid"` (settled; changing it needs a new recorded decision) bars writing it. First real capture: **SIGSEGV** (log committed). The shipped wiring therefore uses `ParkedSource` (honest unsupported phase, no crash, no fake frame); `ScreenshotSource` and the record test stay on the branch for the day the decision lands | **BLOCKED — maintainer decision required** | 2026-10-14 | zcode-primary |
| Visual | `pnpm --dir ui visual:capture` | live, paused (minimised), denied, exited at 1280x800/1024x680 both themes | `_gallery/15-preview/` (5 captures): live dark 1280x800 + live light 1024x680, paused (copy-exact text, dimmed last frame), denied (guidance, NO frame), exited (no preview). Machine-vision checks on live + paused confirm: embedded window image, `● Live preview` + `frame age 0s`, read-only/click-to-focus hint, NO interactive-looking controls inside the preview; paused shows the exact text + greyed frame + no live indicator | PASS | 2026-10-14 | zcode-primary |
| Performance budget (what is measurable today) | driver tests + script-count analysis | <5% CPU, <300 ms frame latency, <1 s teardown, zero cost when closed | teardown <1 s (asserted ×3); frame latency = one screenshot + base64 + event (measured once un-parked — the screenshot call itself is the term; the mocked path exercises the full encode/emit pipeline); CPU: zero when no panel is open (nothing runs — the stream exists only while visible+enabled); the real CPU measure rides the un-block | PARTIAL (teardown + idle proven; live CPU pending un-block) | 2026-10-14 | zcode-primary |

## The decision that un-blocks (for the maintainer)

One of:
1. **Record a new decision** (D-17 candidate) permitting a vendored,
   audited, workspace-excluded init-shim crate whose entire unsafe
   surface is the binding's own documented one-liner:
   `pub fn init() { unsafe { screencapturekit::ffi::sc_initialize_core_graphics(); } }`
   — then re-point `preview_start` at `ScreenshotSource`, delete
   `ParkedSource`, re-run the record test (it already handles both the
   denied and granted worlds), and flip this task DONE-EVIDENCED.
2. **Wait for upstream**: ask doom-fish/screencapturekit-rs for a safe
   `initialize_core_graphics()` wrapper (the examples' requirement is
   their own API gap); same un-block steps once released.

## Notes for the next agent

- Everything except the live pixel path is DONE and merged-able: schema
  v11 opt-in, driver/state machine, wire events, panel states, account
  toggle, captures, and the CLT Swift-link shim
  (`scripts/ensure_swift_runtime.sh` + justfile `swift-flags` — no-op
  on CI and Xcode machines).
- `ParkedSource` is the ONLY thing standing between the app and the
  crash — do not "just try" ScreenshotSource in the shipped path
  without the decision.
- `tests/preview_real.rs` is the future evidence generator: denied
  world asserts the clean PermissionDenied path; granted world
  captures one real window and prints sizes/timings/CPU.
- The stream-delegate pixel path (`CVPixelBuffer` cursor) is `unsafe`
  in this binding — the polled-screenshot design (one `capture_image`
  per tick) was chosen precisely to stay on safe APIs end-to-end.
- The harness seam `__AIJ_PREVIEW_PHASE__` pins preview phases in
  captures; the mock frame is a deterministic generated PNG committed
  in `bridge.ts`.
