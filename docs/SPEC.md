# AI Juggler — Technical Specification

**Status:** draft v0.2 for review (revised after first review round) · **Date:** 2026-10-12
Companion: [proposal](./PROPOSAL.md) · Evidence: [research notes](./research/)

This document specifies the architecture and product behavior of AI Juggler v0.x. Everything here is grounded in the verified findings in `docs/research/`; where a mechanism is version-fragile, the research note is cited.

---

## 1. Overview & constraints

AI Juggler is a native desktop application that **supervises** AI coding harnesses the user already has installed. It never proxies model traffic, never automates vendor logins, and never stores vendor credentials (see proposal §5 guardrails — these are product-level invariants, enforced in code review and tests).

**Non-functional requirements**

| Requirement | Target |
|---|---|
| Platform (v0.x) | macOS 13+ (Universal: Apple Silicon + Intel). Windows/Linux post-1.0; the Rust core and adapters are cross-platform from day one (CI builds all three OSes) so ports need no rewrites |
| Startup | < 1.5s cold start; session board interactive < 500ms after launch |
| Footprint | < 150 MB RSS typical; UI at 60 fps with 20 live sessions |
| Reliability | App crash never kills supervised sessions (children survive; state journaled); recovery on relaunch |
| Resilience | All writes atomic (temp+rename); SQLite in WAL mode; harness-format parse failures degrade, never crash |
| Privacy | No network calls except: update feed check, and user-initiated actions in managed harnesses. Telemetry opt-in only |
| Resource monitoring | < 1% CPU overhead; launch-advisor projections within ±20% of observed session peaks after calibration |

**Automation policy (product invariant).** The app *prepares*; the user *executes*. Automated behavior is strictly passive — monitoring, indexing, usage accounting, notifications, alerts, suggestions, and staging of actions. Every state-changing action (switching an account for new work, transplanting a session, priming a target, syncing project context) is bound to an explicit user click; there is no code path that performs one without it (asserted in `tests/guardrails.rs`). One deliberate, narrowly-scoped exception: an opt-in, per-profile *auto-restart on crash* (off by default) that relaunches a crashed process into the **same** profile — it never changes accounts, conversation content, or anything else user-authored.

## 2. System architecture

```mermaid
flowchart LR
    subgraph UI["Tauri shell (webview UI)"]
        FE[Dashboard · Profiles · Session Board · Usage · Handover]
        TERM[Embedded terminals<br/>xterm.js]
    end
    subgraph CORE["Rust core (tokio)"]
        API[command/event API]
        REG[Adapter registry]
        SUP[Process supervisor<br/>portable-pty]
        RM[Resource manager<br/>sysinfo · footprints]
        IDX[(Session indexer<br/>notify + parsers)]
        USE[Usage engine]
        HO[Handover engine]
        DB[(SQLite<br/>rusqlite, WAL)]
        KEY[OS keychain<br/>keyring]
    end
    subgraph HARNESSES["Managed harnesses (user-installed, unmodified)"]
        CC[Claude Code<br/>CLAUDE_CONFIG_DIR=...]
        CX[Codex<br/>CODEX_HOME=...]
        ZC[ZCode<br/>HOME=...]
    end
    FE <--> API
    TERM <--> SUP
    API --> REG --> SUP
    SUP <--> RM
    SUP -->|spawn| CC & CX & ZC
    IDX -->|watch & parse| CC & CX & ZC
    USE --> IDX
    HO --> IDX
    CORE --> DB & KEY
    CORE -.-|gRPC · mTLS · mDNS| PEERS[Peer AI Juggler<br/>instances (fleet)]
```

**Layering rules**

- The UI is a thin view; all state lives in the Rust core and is streamed to the UI via Tauri events (pre-aggregated payloads — the webview never parses raw transcripts).
- Only adapters know harness specifics. Core speaks only in domain types.
- Everything on disk is append-only or atomic; nothing in a harness's own directories is ever written except through adapter-defined, user-visible actions (e.g., creating an isolated profile directory).

### Workspace layout (Cargo)

```
ai-juggler/
├── crates/
│   ├── juggler-core/        # domain types, events, storage (SQLite), usage math
│   ├── juggler-adapter/     # HarnessAdapter trait, capabilities, IR schemas
│   ├── juggler-adapters/    # claude-code, codex, zcode implementations
│   ├── juggler-supervisor/  # PTY/process lifecycle, health, stuck detection, resources
│   ├── juggler-handover/    # context packs, rules-file sync, takeover flows
│   ├── juggler-link/        # fleet: mDNS discovery, pairing, gRPC/mTLS sync
│   └── juggler-app/         # Tauri shell, commands/events, updater wiring
├── ui/                      # frontend (React + TypeScript)
└── docs/
```

## 3. Domain model

```rust
// crates/juggler-core/src/model.rs (sketch)

pub struct HarnessId(Arc<str>);          // "claude-code" | "codex" | "zcode" | ...

pub struct Account {
    pub id: AccountId,
    pub harness: HarnessId,
    pub label: String,                  // "Work Max", "Personal"
    pub plan: Option<PlanTier>,         // max-20x, plus, glm-pro, ...
    pub state_dir: PathBuf,             // the isolated dir AI Juggler manages
}

pub struct Profile {
    pub id: ProfileId,
    pub account: AccountId,
    pub settings_overlay: SettingsOverlay,  // model, permissions, mcp, env
    pub default_cwd: Option<PathBuf>,
}

pub struct SessionRecord {
    pub id: SessionId,                  // juggler's id
    pub native_id: Option<String>,      // claude uuid / codex thread / zcode sess_*
    pub profile: ProfileId,
    pub machine: MachineId,             // fleet: origin computer (default: this Mac)
    pub cwd: PathBuf,
    pub status: SessionStatus,          // Starting|Running|WaitingUser|Stuck|Finished|Failed|HandedOver
    pub started_at: DateTime<Utc>,
    pub last_activity: Option<DateTime<Utc>>,
    pub usage: UsageSnapshot,           // tokens + estimated plan impact
    pub transcript_ref: TranscriptRef,  // pointer into harness-native storage
}

pub struct UsageSnapshot {
    pub tokens: TokenCounts,            // input/output/cache_read/cache_write/reasoning
    pub window: Option<QuotaWindow>,    // 5h/weekly pool state if known
    pub confidence: Confidence,         // Measured | Estimated (Codex) | Computed (ZCode credits)
}
```

**Profiles ↔ harness state isolation** (the mechanism research verified):

| Harness | Isolation primitive | Managed by AI Juggler |
|---|---|---|
| Claude Code | `CLAUDE_CONFIG_DIR=<managed>/claude/<account>/` — settings, sessions, plugins, and (since v2.1.56) a namespaced keychain entry per dir all follow it | Creates dir; first-run `claude login` is launched for the user to complete |
| Codex | `CODEX_HOME=<managed>/codex/<account>/` — config.toml, auth.json, sessions all follow it | Same pattern (`codex login`) |
| ZCode | `HOME=<managed>/zcode/<account>/` — the only complete redirect (research B4: `ZCODE_DATA_BASE_DIR` is insufficient; full `HOME` works). Desktop app remains single-account; managed ZCode profiles use the bundled CLI engine | Creates minimal home + config; `zcode login --no-browser` surfaced to user |

Account *selection* is per-launch and user-initiated; there is no code path that changes the active account of a running session, and no automatic account switching exists anywhere in the codebase (guardrail test: `tests/guardrails.rs` asserts no such API exists).

## 4. Adapter system

The adapter is the extensibility contract. First-class Rust adapters ship for the big three; post-v1, a declarative manifest (TOML) covers simple CLIs without recompiling.

```rust
// crates/juggler-adapter/src/lib.rs (sketch)

bitflags! {
    pub struct Capabilities: u32 {
        const HEADLESS | RESUME | TRANSCRIPT_EXPORT | TRANSCRIPT_WATCH
            | USAGE_TOKENS | USAGE_QUOTA_FEED | PROTOCOL_SERVER | FORK = 0;
    }
}

#[async_trait]
pub trait HarnessAdapter: Send + Sync {
    fn id(&self) -> HarnessId;
    fn capabilities(&self) -> Capabilities;

    /// Detect install, version, and drift vs. versions this adapter was verified against.
    async fn discover(&self) -> DiscoveryReport;

    /// Prepare an isolated account state dir (idempotent) and return launch env.
    async fn prepare_account(&self, dir: &Path) -> Result<EnvMap>;

    /// Build the launch spec for a profile in a given cwd.
    async fn launch_spec(&self, profile: &Profile, cwd: &Path, mode: LaunchMode)
        -> Result<LaunchSpec>;               // argv + env + tty needs

    /// Watch/index native sessions (read-only!).
    fn watcher(&self) -> Box<dyn SessionWatcher>;

    /// Extract usage signals; never performs network calls.
    async fn usage(&self, account: &Account) -> UsageReport;

    /// Export a session to the neutral Transcript IR.
    async fn export(&self, transcript: &TranscriptRef) -> Result<Transcript>;

    /// Prime a new session from a ContextPack (handover in).
    async fn prime(&self, pack: &ContextPack, spec: LaunchSpec) -> LaunchSpec;
}
```

**Design rules**

- Adapters are *read-only* toward native state except for their own managed dirs, and `prepare_account` is the only writer.
- Every parser is versioned: adapters declare `verified_against: ["2.1.296"]`-style ranges; `discover()` warns on drift and capability-probes at runtime (never assumes).
- Session IR is a versioned, neutral schema (`Transcript { meta, messages[], usage[] }`) with per-harness converters — this is what handover and the indexer share.

### 4.1 Adapter: Claude Code (reference implementation)

- **Launch:** real `claude` binary; interactive via PTY, or headless `-p --output-format stream-json` for driven runs. Env: `CLAUDE_CONFIG_DIR=<managed>`.
- **Sessions:** watch `<config_dir>/projects/**/*.jsonl` (notify debouncer); parse lines (`uuid/parentUuid/type/message/usage`) with fixture-tested, version-pinned parser; dedupe by message `uuid` across resumed forks (ccusage pattern).
- **Usage/quota (per research §5):**
  - *Quota feed:* install a managed statusline script per profile dir; its stdin JSON carries `rate_limits.five_hour/seven_day.used_percentage/resets_at` for Pro/Max — captured to the usage engine as **Measured**.
  - *Tokens:* transcript `usage` objects per assistant message → **Measured**.
  - *Exhaustion:* `system/api_retry` rate_limit events (headless), `StopFailure` hook, and transcript pattern matching for "hit your session/weekly limit" (installed as a managed hook in profile dirs) → session status transitions + notifications.
- **Resume:** `claude --resume <id>` / `--continue`; transcript-path resume for cross-dir portability.
- **Prime (handover-in):** launch with the context pack as initial user turn (headless or interactive), or `--settings` overlay for per-session config.

### 4.2 Adapter: Codex

- **Launch:** `codex` with `CODEX_HOME=<managed>`; profiles via adapter-generated `<name>.config.toml` (post-0.134 semantics — separate files, not legacy tables).
- **Control plane (phase 2):** `codex app-server` JSON-RPC (thread/turn lifecycle, approvals) — schemas generated via `codex app-server generate-json-schema` and vendored; phase 1 uses PTY + `codex exec --json` for driven runs.
- **Sessions:** watch `~/.codex… <home>/sessions/**/rollout-*.jsonl`; index from `session_meta` without full replay; `token_usage_record` lines → **Measured** tokens.
- **Usage/quota:** no public API (research §5) — token burn is Measured; plan-credit correlation is **Estimated** (window reconstruction from token timestamps; calibrated as the user observes real resets) and labeled as such in the UI.
- **Resume:** `codex resume` (interactive) / `codex exec resume <id>`.

### 4.3 Adapter: ZCode

- **Launch:** bundled engine at `/Applications/ZCode.app/Contents/Resources/glm/zcode.cjs` via `node`, with `HOME=<managed>`; `--surface terminal`, `-p ... --json` headless; version pinned to the installed app version.
- **Sessions:** read-only queries against `<home>/.zcode/cli/db/db.sqlite` (session/message/model_usage tables) + file watching; WAL mode makes concurrent reads safe.
- **Usage/quota:** **Computed exactly** — `model_usage` token counts + the published credit formula (per-model multipliers, off-peak 50% windows) → exact credit burn per session/account; remote pool totals (5h/weekly) remain as configured plan caps since no public API exists.
- **Resume:** `--resume <sess_id>` / `-c`.
- **Constraints honored:** no reverse-engineered endpoints, no patched binaries — documented/observable surfaces only (research A3/B7).

## 5. Process supervisor & resource manager

### 5.1 Supervision

- One tokio task per session. Spawn via `portable-pty` (real TTY) with per-profile env; optional direct child (no PTY) for headless runs.
- Output fan-out: (1) UI terminal buffer (xterm.js via Tauri events, back-pressured ring buffer), (2) heuristic engine, (3) adapter stream parser (stream-json / exec --json) for structured events.
- **Stuck detection heuristics** (all thresholds user-tunable):
  - `no_output && cpu < x% && transcript mtime stale` for N minutes → *Stuck?* (waiting-on-model vs hung disambiguated by CPU/IO),
  - process alive but harness waiting for permission > N minutes → *WaitingUser*,
  - exited non-zero with rate-limit signature → *QuotaBlocked* (distinct status; triggers the handover *suggestion*, never an action),
  - zombie/orphan detection on app restart (children survive app death by design; supervisor re-adopts by PID + log correlation).
- Lifecycle: start, stop (SIGTERM→SIGKILL ladder), attach/detach, and *pause* only where the harness supports it (none do today — pause is emulated at the task level via handover, never by SIGSTOP, which could corrupt in-flight writes).

### 5.2 Resource manager

Parallel harnesses are heavy (an Electron-based ZCode plus a Node CLI plus VS Code add up fast), and AI Juggler is the one component that can see them all — and the machine underneath.

**Monitored signals** (sysinfo, staggered refresh: ~2 s for supervised process trees, ~15 s for system aggregates, respecting `refresh_specifics` / `MINIMUM_CPU_UPDATE_INTERVAL`):

- **System:** available RAM, swap usage *and* swap-in/out rate (thrash detection), CPU load and per-core saturation, free disk on the volumes holding session data (ZCode logs measured ~4 MB/day in research), battery state (optional), thermal pressure (macOS `pmset -g therm`; informational).
- **Per session:** the full process tree (CLI + children: subagents, node/electron helpers, git/ripgrep spawned by harnesses) — RSS, CPU%, cumulative CPU time — attributed to the session even as children come and go (ppid-map tracking).
- **Historical footprints:** peak RSS/CPU percentiles per (harness, model, repo size bucket) learned from past runs and stored in SQLite (`footprint_stats`) — this is what makes the advisor below smarter over time instead of guessing.

**Admission advisor (pre-launch) — "can this machine take another session?":** projected headroom = available RAM − safety margin − Σ historical peak footprints of running sessions − the candidate's own footprint percentile. Verdicts: **OK** / **Tight** (warn, and show what would make it safe — e.g. "stopping session X (2.9 GB peak) makes this comfortable") / **No** (explain, suggest alternatives: fewer sessions, a lighter profile, or handover instead of a new session). Launching anyway is always allowed — the advisor informs, it never blocks.

**Overload alerts** (passive, per the automation policy; each carries one-click suggestions that execute only on click):

- available RAM below threshold, or sustained swap thrashing → "memory pressure — sessions may stall or be killed by the OS"
- CPU sustained ≥ N% for M minutes, or thermal pressure (macOS) → performance-degradation warning
- session process-tree RSS anomaly (sudden ≥ 3× growth — runaway context/tool loop)
- disk below threshold on a transcript/session volume

**Recommendation engine** (dashboard "Machine advisor" panel, derived from footprints + alert history): e.g. "your Claude sessions peak at 3.1 GB each — 3 in parallel plus Docker Desktop exceeds 16 GB; 2 + handover fits", "limiting Codex `max_concurrent_threads_per_session` in profile X to 2 cut its peak CPU by 40% last week", "ZCode's desktop app holds ~1.2 GB idle; CLI-only profiles halve background weight" (knobs are surfaced per profile, never enforced).

### 5.3 Crash recovery

- **Harness process crashes:** the supervisor records exit status/signal and classifies the cause where possible — OOM/system-kill heuristics (SIGKILL + measured memory pressure at exit time; Linux OOM-killer is the common case), quota-exit signatures (adapter), or plain crash. Recovery is offered on click: **native resume** (transcripts persist, so the common case loses nothing), relaunch with the same profile+cwd, or hand over. The opt-in per-profile *auto-restart on crash* (§1 automation policy) relaunches into the same profile with native resume.
- **AI Juggler crash:** supervised children survive by design; on relaunch the supervisor re-adopts process trees (PID + log correlation), rebuilds the board from the session index, and reports what happened while it was down.
- **System sleep/reboot:** sessions that died while the machine was down are surfaced on wake/boot with the same recovery options; on battery below a user threshold, the app suggests (never executes) pausing its own sessions.

## 6. Usage engine

- Inputs: per-message token events (all harnesses), quota feed events (Claude Measured; ZCode Computed; Codex Estimated), plan config (tier, window sizes, multipliers) entered by the user and stored per account.
- Outputs: per-session, per-account, per-window aggregates; burn-down projections ("at current rate, 5h window exhausts in ~40 min"); reset timers; threshold alerts (system notifications + tray state).
- Storage: append-only `usage_events` table; rollups materialized nightly and on demand.
- Honesty rule: every number in the UI carries its confidence (Measured/Computed/Estimated) — no synthesized precision.

## 7. Handover & session-sync engine

**Automation policy (product invariant).** The engine *prepares*; the user *executes*. Automated behavior is strictly passive — watching, indexing, accounting, alerting, suggesting, and staging. Every state-changing action (switching an account for new work, transplanting a session, priming a target, syncing project context) is bound to an explicit user click, and there is no code path that performs one without it (asserted in `tests/guardrails.rs`).

The goal is **actual session continuity — sessions and chats moved with their context — not a generated hand-off prompt**. What is achievable depends on what the target harness can natively consume, so the engine always selects the highest tier available and reports which tier it used:

### 7.1 Fidelity tiers

| Tier | Situation | Mechanism | Fidelity |
|---|---|---|---|
| **T1 — Native resume** | Same harness, same account | Adapter-native resume (`claude --resume`, `codex exec resume`, `zcode --resume`) | Lossless: the harness's own session state continues |
| **T2 — Session transplant** | Same harness, different account | Copy the native session artifacts into the target account's managed state dir, then native-resume there (adapter capability `TRANSPLANT`) | Lossless conversation: the target account's harness resumes the identical session, history, and in-session todos |
| **T3 — Transcript replay** | Different harness | Reconstruct the *complete* conversation from the Transcript IR (every user/assistant turn; tool outcomes distilled) and inject it as the target session's opening context via the adapter's priming mechanism | Content-complete: the target model re-reads the real conversation as text and rebuilds its working context from it |
| **T4 — Context pack** | Export/portability fallback; works for any target, including harnesses without adapters | The versioned pack format (§7.3) | Summary-level; user-editable before use |

Honest boundary: no harness exposes model-internal state (KV caches), so nothing can copy that — for anyone, in any tool. T1/T2 are effectively indistinguishable from having started on the target account; T3 preserves the full conversation content, with the target reconstructing its working context from it. A vendor precedent for cross-harness import already exists: Codex ships `/import` of Claude Code/Cursor chats (2026-08-11 changelog) — AI Juggler's version is vendor-neutral and user-triggered.

**T2 mechanics per harness** (adapter capability, fixture-tested):

- **Claude Code:** copy the session `.jsonl` into the target account's `projects/<mangled-cwd>/`; resume by transcript path (supported per CLI reference). Re-transplant ID collisions are resolved by forking (`--fork-session` semantics or a mechanical `sessionId` rewrite).
- **Codex:** copy the `rollout-*.jsonl` into the target `CODEX_HOME` sessions tree; `codex exec resume <session_id>` picks it up. Both accounts share the same installed binary, so format compatibility is guaranteed.
- **ZCode:** sessions live in SQLite; transplant means cloning session rows into the target account's DB — schema-coupled and under investigation. Until verified, ZCode targets use T3, and ZCode sources feed T1–T3 targets natively.

### 7.2 Flows (all execute on click)

1. **Switch profile (one click):** choose which account a *new* session runs under — staged instantly from the profile registry. Running sessions are never switched underneath the user.
2. **Takeover (two clicks):** "Hand over…" on a session → targets listed with the fidelity tier each supports and current quota headroom → click executes: freeze source, transplant or replay, launch target primed, record lineage. The source session always remains resumable (T2 copies; it never moves or destroys).
3. **Return (one click, suggested):** when a source account's window resets (the app alerts), "Resume lineage" resumes the original session natively, primed with a delta of what the takeover session did ("while you were away").
4. **Project context sync (one click, diff-previewed):** align project-level context across profiles working the same repo — rules files (AGENTS.md ↔ CLAUDE.md ↔ `.zcode/AGENTS.md`; AGENTS.md is the emerging standard and Claude Code reads it natively since v2.1.277), harness memory files, todo lists, and uncommitted work. Master copy chosen by the user; every change is shown as a diff before it is applied.
5. **Export/import:** any session exports to a T4 pack file; importing a pack creates a primed session in any managed harness.

### 7.3 Context pack format (T4 + staging/preview artifact)

For exports, and as the always-available preview of exactly what a T2/T3 handover will carry over:

```
pack/
├── PACK.md          # task, constraints, decisions, open TODOs, next steps
├── conversation.md  # the conversation as carried over
│                    # (T3: full replay text; T4: distilled summary + last N turns)
├── state.json       # machine-readable: repo, branch, diff summary, env,
│                    # harness, session ids, pack lineage (A→B→A chains)
└── diffs/           # key patches if not committed
```

Generation is template-driven from the Transcript IR (deterministic; an optional LLM-assisted "smart distill" is a post-v1 feature). `PACK.md` is written to be usable by *any* harness, even one without an adapter.

## 8. Security & privacy

- **Vendor credentials:** never read, copied, or stored by AI Juggler. They live wherever each harness keeps them inside the managed account dir (Claude: namespaced keychain; Codex: auth.json; ZCode: credentials.json). The app's keychain entries hold only AI Juggler's own config encryption key (if needed) — by default nothing.
- **Managed dirs** are 0700; the app runs with normal user privileges, no helpers, no agents.
- **Supply chain:** lockfile-audited deps, `cargo deny`, minimal JS in the webview (no remote content, CSP locked to self).
- **Updates:** Tauri updater, minisign-style signatures, verification cannot be disabled; OS notarization/signing from v0.1 (Apple Developer + Azure Trusted Signing, individual tier — budgeted).
- **Data:** everything local; "export diagnostics" redacts paths/tokens by default.

## 9. App & UX (v0.1 scope)

- **Dashboard:** health strip (accounts × quota rings with reset countdowns, plus machine gauges: CPU / RAM / swap / disk), active sessions count, alerts.
- **Profiles:** cards per account (harness icon, plan tier, quota ring, last used); "New profile" wizard (detect harness → name account → open vendor login in guided terminal → verify → done).
- **Session board:** table/grid of live sessions (status chip, harness, profile, cwd, age, tokens, quota impact, RAM/CPU); row actions: Attach, Open in terminal, Stop, **Hand over…** (wizard).
- **Usage:** per-account timelines (uPlot), windows (5h/weekly/credits), per-session breakdowns, export CSV.
- **Handover:** staged on the session board — "Hand over…" lists target profiles with the fidelity tier each supports and current quota headroom; one click executes (a preview/edit of exactly what will be carried over is always available first).
- **Machine advisor:** overload banners and notifications, per-session resource breakdown, launch-headroom verdicts (OK / Tight / No + why), optimization recommendations, and crash-recovery cards (resume / relaunch / hand over) after incidents.
- **Fleet view:** connected machines (state, resources, permissions) as filters across board/dashboard/usage; "Add a machine" pairing wizard; cross-machine handover in the standard wizard.
- **Terminals:** embedded xterm.js tabs (one per session) — *open decision #2: v1 or v2.*
- **Tray:** aggregate quota state, alerts, quick-launch menu. Global hotkey for the board.
- **Settings:** thresholds (stuck heuristics, alert levels), terminal prefs, update channel (stable/beta), telemetry (off).

## 10. Self-update & harness updates

- **App:** `tauri-plugin-updater` + GitHub Releases feed (`latest.json` via tauri-action), staged: download → verify signature → apply on quit or relaunch prompt; channel switch in settings; "check for updates" manual trigger; disable-able entirely.
- **Harness tracking:** adapters' `discover()` records installed versions; a background daily check against vendor release channels (GitHub/npm APIs) surfaces "Claude Code 2.1.301 available (you run 2.1.296)" with the adapter's verified-range status — the app never auto-updates a harness (its updater, your call).
- **Versioning & channels:** strict semver. Pre-1.0 ships as `0.x.0-beta.N` releases on the **beta** channel (early feedback); the **stable** channel opens with 1.0. Every release is a git tag `vX.Y.Z` with a CHANGELOG entry, updater-signed artifacts, and a notarized macOS build.
- **macOS signing/notarization:** Apple Developer Program membership (Vuxtra) provides Developer ID Application signing + `notarytool` notarization in CI (secrets via GitHub Environments). Open-source license unaffected — binaries are simply signed by the publishing identity, the standard model for company-published OSS.

## 11. Fleet: multi-machine operation

AI Juggler instances link over the network so one console can see — and manage — every harness session on every machine you own. The design goal is **onboarding ease**: connecting a new machine is a pairing code, not a config file.

### 11.1 Topology & roles

- One binary, two roles: every instance is a **node** (serves its local state); any instance can act as a **console** (view/control the fleet). In practice one machine is "your" console; multiple consoles are allowed.
- **LAN-first, zero config:** discovery via mDNS/Bonjour (`mdns-sd`). Remote machines join through the user's own VPN (Tailscale/WireGuard) — AI Juggler never operates an internet relay or cloud service (privacy posture; also keeps the compliance story purely local).

### 11.2 Transport & security

- **gRPC over TLS 1.3 with mutual certificate auth (tonic + rustls)** — a standard, schema'd (protobuf), streaming-capable transport; protobuf contracts are versioned in-repo and breaking-change-linted in CI. (Codex's own app-server uses the same JSON-RPC-over-stdio pattern — the style is proven in this exact ecosystem.)
- Each instance generates an Ed25519 identity at first run; its self-signed certificate identifies the node.
- **Pairing:** the console displays a 6-digit code / QR; completing the exchange on the new node pins both identities. Unpaired connections are dropped — there is no anonymous mode.
- Commands from a console are authenticated to the node; per-machine permissions (e.g. "view-only") are set at pairing or later.
- **Link discipline:** the fleet link carries telemetry and control commands only — never model traffic, never vendor credentials, never raw transcripts except an explicit on-demand fetch for a user-executed cross-machine handover.

### 11.3 What syncs

- **Continuous (streamed):** heartbeats, machine resource stats, session-board deltas (status, tokens, quota impact), alerts.
- **On demand:** usage rollups, footprint stats, handover packs (fetched only when a cross-machine handover is executed).
- **Never leaves the machine:** vendor credentials (invariant), keychain contents; full transcripts stay local outside explicit handovers.

### 11.4 Fleet UX

- "Add a machine" wizard: enable Link on the node → code/QR → paired; machines appear as filters/columns across the board, dashboard, and usage views.
- **Cross-machine handover:** same wizard as local; the fidelity tier is computed across machines — T2 requires the same harness binary version and the repo checked out at the same path (sessions store absolute paths — surfaced honestly in the wizard); T3 always available.
- Fleet alerts route to every console ("mac-mini-2: memory pressure").

### 11.5 Protocol versioning

- Protobuf contracts are versioned; mixed-version fleets supported within a minor window — nodes reject unknown command versions gracefully, and a console degrades to core telemetry when talking to an older node.

## 12. Storage layout

```
~/Library/Application Support/AI Juggler/        (macOS; XDG on Linux, %APPDATA% on Win)
├── juggler.sqlite          # accounts, profiles, sessions index, usage events, lineage, footprint stats (WAL)
├── accounts/<harness>/<account-id>/   # managed harness state dirs (the isolation roots)
├── packs/                  # context packs + lineage metadata
├── fleet/                  # node identity keypair + paired-peer registry
├── logs/                   # rotated app logs (redaction-aware)
└── cache/                  # parser fixtures cache, rollups
```

Nothing is ever written inside `~/.claude`, `~/.codex`, or `~/.zcode` (the user's pre-existing single-account installs remain untouched; the smart-onboarding import (§12.1, ADR-0006) *reads* them and offers a managed copy).

### 12.1 Onboarding workflow (smart onboarding, ADR-0006)

Onboarding assumes the user knows nothing about harness internals. The app
does the discovery; the user makes choices in plain language.

1. **Detect** — `environment_scan` runs at wizard start (metadata-only:
   presence/size/mtime + keychain existence probes, never contents):
   - the vendor default (`~/.claude`),
   - `$CLAUDE_CONFIG_DIR` overrides (other manager tools, manual setups),
   - home-naming-pattern matches `.claude*` (manual or AI-created setups),
   filtered to dirs that actually look like Claude configs, deduplicated,
   sorted most-useful-first.
2. **Choose** — one-tap **Import** (byte copy into a managed dir; source
   untouched; caches skipped; 2 GiB ceiling) or **Add fresh**.
3. **Identity** — plain-words naming ("What's this account for?") and plan
   tier; a default `Main` profile is created so every account is
   immediately launchable.
4. **Login** — the vendor's own flow, maximally aided: open the vendor's
   sign-in page in the browser, run the env-scoped login command in the
   embedded terminal (M1-4), or copy it out. Credential entry is never
   automated; keychain-locked logins honestly require one re-login in the
   isolated copy.
5. **Summary** — what was created, where it lives, and confirmation that
   pre-existing setups were not modified.

## 13. Testing strategy

- **Unit:** usage math (esp. ZCode credit formula vs. published examples), window reconstruction, pack rendering.
- **Fixture corpus:** committed, anonymized transcript samples per harness × version; parsers tested against them; CI job re-verifies against live format dumps monthly (canary).
- **Fake harness adapter:** a script-driven CLI implementing the adapter contract for e2e supervisor/stuck/handover tests — CI runs the full lifecycle headlessly on all three OSes.
- **Resource heuristics:** fixture process trees (fake harness spawning memory/CPU-hog children) drive overload alerts, admission-projection accuracy, and OOM-crash classification tests.
- **Fleet:** loopback multi-instance tests — two live instances in CI pairing over ephemeral ports, telemetry sync, permission enforcement, and a mixed-version downgrade matrix.
- **Guardrail tests:** static + runtime assertions that no code path writes outside managed dirs, performs network calls outside allowlist, exposes account-switching of live sessions, or executes any account switch / handover / project sync without a user-initiated command.
- **UI:** Playwright on the webview for board/wizard flows.

## 14. Milestones

Task-level breakdown, live tracking, and the per-milestone validation-gate process live in [docs/plan/EPICS.md](./plan/EPICS.md); the engineering standards and per-task Definition of Done live in [docs/plan/QUALITY.md](./plan/QUALITY.md). Every milestone closes with a full validation pass, a committed gate report (`docs/qa/gate-MX.md`), and user confirmation before the next one starts.

| M | Scope | Acceptance |
|---|---|---|
| **M0** | Workspace scaffold, CI (fmt/clippy/test, 3-OS build), adapter trait + fake harness, domain model + SQLite | Fake session launches, is watched, and shows on a debug board |
| **M1** | Claude Code adapter: account wizard (login hand-off), isolated profiles, parallel launch, session board, embedded terminal *(pending decision #2)* | Two Claude accounts in parallel; zero writes to `~/.claude` |
| **M2** | Codex + ZCode adapters; adopt-existing flow; stuck detection v1; resource manager v1 (monitoring, overload alerts, admission advisor, crash recovery) | Three harnesses, one board; stuck + overload heuristics pass fixture corpus; crashed session → one-click native resume |
| **M3** | Usage engine + dashboards + alerts (Claude Measured, ZCode Computed, Codex Estimated); tray | Quota rings within ±5% of vendor-reported for Claude/ZCode |
| **M4** | Handover: packs, takeover/return wizard, rules-file sync, export/import; lineage | Round-trip A→B→A preserves continuity (dogfood criterion) |
| **M5** | Hardening, signed + notarized self-update, harness update tracking, onboarding polish; fleet transport spike (tonic/rustls prototype) → **v0.1 public beta** | Fresh-machine install → 2 parallel accounts < 5 min; updater works end-to-end on a signed build |
| **M6** | Fleet link: pairing wizard, mDNS discovery, telemetry sync, remote board (read-only) → **v0.2** | Second machine joins in < 60 s with a code; one board across two machines |
| **M7** | Fleet control: launch/stop/handover from a console, aggregated usage, fleet alerts; Windows/Linux bring-up begins → **v0.3** | Full manage workflow across 3 machines; cross-machine handover demo |
| **1.0** | Stabilization from beta feedback (protocol, adapters, UX) | Stable update channel opens |

## 15. Decisions

**Resolved (review rounds 1–3):**

1. **UI framework: React 18 + TypeScript + Tailwind + shadcn/ui.** ✅
2. **Embedded terminal ships in M1.** ✅
3. **License: MIT.** ✅
4. **Naming: binary `ai-juggler`, crates `juggler-*`.** ✅
5. **MVP order: Claude Code first (M1); Codex + ZCode in M2.** ✅
6. **Codex control plane: PTY-first (M2); app-server explored at M4.** ✅
7. **Platform: macOS-first (Universal, v0.x); Windows/Linux post-1.0; cross-platform core from day one.** ✅
8. **Fleet: gRPC over mTLS + mDNS pairing; one binary, console/node roles; LAN/user-VPN only — no cloud relay.** ✅ (M5 transport spike de-risks tonic/rustls before M6.)
9. **Release: strict semver; `0.x.0-beta.N` on the beta channel for feedback; stable channel opens at 1.0; macOS signing/notarization via Vuxtra's Apple Developer account.** ✅

**Open (non-blocking):**

1. Fleet wire format detail: protobuf-first vs JSON-RPC in the app-server style — decided at the M5 spike.
2. Whether the stable update channel opens earlier than 1.0 — revisit at v0.3 based on beta feedback.
