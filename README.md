# 🤹 AI Juggler

**Start anywhere. Switch when you hit the wall. Never lose the thread — or leave a zombie behind.**

*One console for every Provider, Account, and Instance you juggle.*

AI Juggler is a free, open-source, native desktop app (Rust + Tauri 2,
macOS-first) for developers who run multiple AI coding subscriptions —
**Claude Code**, **Codex**, **ZCode**, and more — and are tired of
juggling them by hand. It supervises the tools you already have
installed: it never proxies traffic, never handles vendor logins, and
never stores credentials.

> **Status: in beta development.** The engine, Claude Code adapter,
> instance-centric UI, smart onboarding (machine scan + read-only
> import + guided sign-in), and handover v0 (labelled beta fidelity —
> see honesty notes below) are built and continuously validated. The
> full switching programme (Switch Graph, per-pair fidelity
> validation), cleanup tiers, usage clocks, and Fleet are specified and
> in progress — see [docs/tasks](./docs/tasks). No releases yet.

---

## The problem

Modern AI-assisted development means running several coding agents in parallel:

- Multiple subscriptions of the same tool (e.g. two Claude Max accounts) that don't natively run side by side
- Different tools for different strengths (Claude Code, Codex, ZCode …) with separate logins, configs, and conventions
- Quota that runs out mid-task — and no smooth way to let another account or tool take over and continue
- Sessions scattered across tools, impossible to track, hand over, or monitor for stuck states

Every developer solves this with a pile of shell scripts, terminal tabs, config-dir hacks, and sticky notes. **AI Juggler replaces that pile with one app.**

## What it will do

| Pillar | Capability | State |
| --- | --- | --- |
| 🔀 **Switch** | Unlimited, bidirectional switching of the work (conversation, context, rules, project state) between Accounts and Providers — one click when the pair is proven, an itemised wizard when lossy; switch-back resumes the original | v0 shipped (beta fidelity, same-Provider); full engine specified ([E2](./docs/tasks/E2-switching-engine.md)) |
| 👁️ **See** | A right-hand Inspector for any selection (Provider, Account, Instance, Session, Project): usage, history, quality signals, owned processes, actions — honest fields only | specified ([E4](./docs/tasks/E4-indicators-and-inspector.md)); today: instance-centric rail + panels + terminals |
| 🧹 **Clean** | Every spawned process owned to the end: verified leftovers surfaced, per-Account opt-in automatic cleanup, every kill audited | specified ([E5](./docs/tasks/E5-process-ownership-and-cleanup.md)) |
| 🎭 **Parallelism** | Accounts per Provider, isolated state dirs, one-click launches from projects or a folder picker | shipped |
| 🚀 **Smart onboarding** | Scans the machine (including other manager tools' profile roots), imports existing setups read-only, guides the Provider's own sign-in | shipped (Claude Code); cross-Provider generalisation specified ([E3](./docs/tasks/E3-smart-onboarding.md)) |
| 📊 **Usage (two clocks)** | Window clocks (5h/weekly) that trigger switching + user-entered subscription clocks | specified ([E6](./docs/tasks/E6-quota-cost-and-billing-research.md)) |
| 🖥️ **Machine health** | RAM/CPU/swap/disk monitoring, overload warnings, launch headroom advice | specified (M3/E4) |
| 🌐 **Fleet** | Your own machines, paired by code, one console | specified, at-launch beta ([E7](./docs/tasks/E7-fleet-at-launch.md)) |

**Honesty notes:** switching between *different* Providers is labelled
beta with per-pair fidelity badges — never marketed as lossless;
cross-Provider claims always carry the badge. Cross-machine switches
require matching binary versions and repo paths (surfaced in the
wizard).
| 🔗 **Fleet link** | Connect multiple computers over your network (pairing code, end-to-end secured) — see and manage every session across all of them from one console |
| 🧩 **Extensible by design** | A clean adapter architecture — Claude Code, Codex, and ZCode first, new harnesses without changing the core |

## Guiding principles

- **Works with your tools, never replaces them.** AI Juggler orchestrates harnesses you already have installed; it is not another agent.
- **Compliant by design.** It only manages accounts you own, never automates logins, never shares credentials, and respects each vendor's per-account limits and terms. See the [proposal](./docs/PROPOSAL.md) for the full compliance posture.
- **Local-first & private.** Your configs, transcripts, and usage data stay on your machine. No cloud, no telemetry unless you opt in.
- **Native performance.** A Rust core with a modern desktop UI — fast, reliable, resilient, self-updating.

## Supported harnesses

| Harness | Support |
| --- | --- |
| Claude Code | 🚧 Planned — launch target |
| Codex | 🚧 Planned |
| ZCode | 🚧 Planned |
| Others (Gemini CLI, Droid, Opencode, …) | 🔌 Via the adapter system, post-v1 |

## Documentation

- [Product proposal](./docs/PROPOSAL.md) — the why: problem, users, landscape, risks, compliance
- [Technical specification](./docs/SPEC.md) — the how: architecture, adapters, data model, roadmap
- [Execution plan](./docs/plan/EPICS.md) — epics, tasks, acceptance criteria, validation gates
- [Quality standards](./docs/plan/QUALITY.md) — engineering standards & the per-task Definition of Done
- [ADRs](./docs/adr/) — architecture decision records
- [Research notes](./docs/research/) — deep dives on Claude Code, Codex, ZCode, and the tooling landscape

## Roadmap

| Milestone | Scope | Status |
| --- | --- | --- |
| M0 | Repo bootstrap, spec review | 🔄 In progress |
| M1 | Profile manager + parallel launcher (single tool) | ⏳ |
| M2 | Multi-harness + session & resource tracking | ⏳ |
| M3 | Usage dashboards + quota alerts | ⏳ |
| M4 | Cross-tool session & project handover | ⏳ |
| M5 | Hardening, signed self-update, public v0.1 beta | ⏳ |
| M6 | Fleet link — one board across machines (v0.2) | ⏳ |
| M7 | Fleet control + aggregated usage (v0.3) | ⏳ |
| 1.0 | Stable release from beta feedback | ⏳ |

## Install

Nothing to install yet. The first release will be a **signed and notarized macOS app** (Universal binary, Developer ID via Vuxtra) with built-in auto-updates — drag it to `/Applications` and you're running. Early builds ship as `v0.1.0-beta.*` on the beta channel; watch [releases](https://github.com/fbrbovic/ai-juggler/releases) to catch the first one.

## Contributing

AI Juggler is open source (MIT) and contribution-friendly — see [CONTRIBUTING.md](./CONTRIBUTING.md). During the spec phase, the most valuable contribution is feedback: read the proposal and spec and open issues with your use cases, criticism, and wishes.

## License

[MIT](./LICENSE) © Faruk Brbovic and AI Juggler contributors
