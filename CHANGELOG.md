# Changelog

All notable changes to AI Juggler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Repository bootstrap: license (MIT), contribution guide, code of conduct,
  security policy, issue/PR templates, placeholder CI.
- Deep-dive research notes for Claude Code, Codex, ZCode, and the tooling
  landscape (`docs/research/`).
- Product proposal (`docs/PROPOSAL.md`) and technical specification
  (`docs/SPEC.md`) — published for review.
- CONTRIBUTING guidance: only final artifacts belong in the repo (no
  conversation logs or exploration transcripts).
- Machine health subsystem (spec §5): system + per-session resource
  monitoring, overload alerts, launch admission advisor, crash recovery,
  and optimization recommendations.
- Fleet / multi-machine architecture (spec §11): mDNS discovery, code
  pairing, gRPC over mutual TLS, telemetry/control sync with
  credentials-never-leave-the-machine invariant; fleet milestones M6/M7
  toward 1.0.
- Release & platform policy: macOS-first signed + notarized builds
  (Apple Developer via Vuxtra), strict semver with 0.x beta channel,
  stable channel at 1.0.
- Execution plan (docs/plan/EPICS.md): epics → tasks → acceptance
  criteria for M0 through 1.0, per-milestone validation gates with
  committed gate reports and user confirmation.
- Quality standards (docs/plan/QUALITY.md): engineering standards and
  the per-task Definition of Done applied to every requirement.
- Architecture decision records (docs/adr/): Tauri 2, adapter
  architecture, automation policy, fleet transport, MIT license.
- Autonomous build runbook (docs/plan/RUNBOOK.md): the operating
  contract for unattended build runs — PR flow, DoD, gate stops, hard
  rails.

### Added (M0-1)

- Cargo workspace with the seven `juggler-*` crates; workspace lint
  baseline (clippy pedantic subset, `unwrap/expect/panic/todo` warnings,
  `missing_docs` deny, `unsafe_code` forbid); pinned toolchain
  (1.99.0, MSRV 1.99); cargo-deny policy; `just` task runner
  (`just verify` = the CI gate, locally).

### Added (M0-2)

- CI pipeline: `verify` job (fmt, clippy `-D warnings`, tests, docs with
  `-D warnings`, cargo-deny, llvm-cov coverage artifact) plus a
  cross-platform build matrix (macOS arm64 + x86_64 cross-check, Linux,
  Windows); required status checks enforced on `main`.

### Added (M0-3)

- Domain model in `juggler-core`: typed identifiers, `Account`, `Profile`,
  `SessionRecord`, session statuses (incl. `QuotaBlocked`), usage snapshots
  with confidence levels, quota windows, transcript refs; typed
  `CoreError`.
- SQLite storage: WAL + foreign keys, forward-only versioned migrations
  (newer-schema databases rejected, corrupt files degrade to
  `NotADatabase`), repositories for accounts/profiles/sessions with
  cascade deletes.
- Crash-safe `atomic_write` helper.
- Property-based round-trip tests (proptest) for every entity plus
  migration, corruption, cascade, and atomic-write edge cases.

### Changed

- Handover model upgraded from summary packs to tiered session sync:
  native resume (T1), cross-account session transplant (T2), cross-harness
  transcript replay (T3); context packs remain as preview/export format (T4).
- Explicit automation policy in spec and proposal: monitoring, alerts,
  suggestions, and staging are automated; every state-changing action is
  executed by an explicit user click.
