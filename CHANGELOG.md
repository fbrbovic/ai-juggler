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

### Changed

- Handover model upgraded from summary packs to tiered session sync:
  native resume (T1), cross-account session transplant (T2), cross-harness
  transcript replay (T3); context packs remain as preview/export format (T4).
- Explicit automation policy in spec and proposal: monitoring, alerts,
  suggestions, and staging are automated; every state-changing action is
  executed by an explicit user click.
