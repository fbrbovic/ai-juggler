# 🤹 AI Juggler

**One native desktop app to juggle all your AI coding agents.**

AI Juggler is a desktop app for developers who run multiple AI coding subscriptions and harnesses — **Claude Code**, **Codex**, **ZCode**, and more — and are tired of juggling them by hand.

> **Status: spec phase.** The product proposal and technical specification are being drafted for community review — see [docs](./docs). No releases yet. Watch the repo or join the discussion in GitHub Issues/Discussions.

---

## The problem

Modern AI-assisted development means running several coding agents in parallel:

- Multiple subscriptions of the same tool (e.g. two Claude Max accounts) that don't natively run side by side
- Different tools for different strengths (Claude Code, Codex, ZCode …) with separate logins, configs, and conventions
- Quota that runs out mid-task — and no smooth way to let another account or tool take over and continue
- Sessions scattered across tools, impossible to track, hand over, or monitor for stuck states

Every developer solves this with a pile of shell scripts, terminal tabs, config-dir hacks, and sticky notes. **AI Juggler replaces that pile with one app.**

## What it will do

| Pillar | Capability |
| --- | --- |
| 🎭 **Profiles & parallelism** | Define profiles (tool × account × settings) and launch them in parallel with one click — isolation handled for you |
| 🔀 **Handover & portability** | Import, sync, and export projects and chat sessions between tools; let a fresh account or different harness take over mid-task, and hand control back when a quota window resets |
| 📊 **Usage dashboards** | Visual, per-account and per-plan usage and quota tracking, so you always know what you have left before you start a task |
| 🩺 **Session health** | Live status of every running session, with stuck-session detection and safe lifecycle controls |
| 🖥️ **Machine health** | RAM/CPU/swap/disk monitoring, overload warnings, launch headroom advice ("can this machine take another session?"), and crash recovery |
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
- [Research notes](./docs/research/) — deep dives on Claude Code, Codex, ZCode, and the tooling landscape

## Roadmap

| Milestone | Scope | Status |
| --- | --- | --- |
| M0 | Repo bootstrap, spec review | 🔄 In progress |
| M1 | Profile manager + parallel launcher (single tool) | ⏳ |
| M2 | Multi-harness + session & resource tracking | ⏳ |
| M3 | Usage dashboards + quota alerts | ⏳ |
| M4 | Cross-tool session & project handover | ⏳ |
| M5 | Hardening, self-update, public v0.1 | ⏳ |

## Contributing

AI Juggler is open source (MIT) and contribution-friendly — see [CONTRIBUTING.md](./CONTRIBUTING.md). During the spec phase, the most valuable contribution is feedback: read the proposal and spec and open issues with your use cases, criticism, and wishes.

## License

[MIT](./LICENSE) © Faruk Brbovic and AI Juggler contributors
