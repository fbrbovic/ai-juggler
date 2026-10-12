//! Concrete harness adapters for AI Juggler.
//!
//! One module per harness — `claude_code`, `codex`, `zcode` — plus `fake`, the
//! scriptable fake harness that drives lifecycle, stuck-detection, and
//! crash-recovery tests across the whole system. Every adapter implements the
//! `juggler_adapter::HarnessAdapter` contract and confines itself to its
//! managed account directories (guardrails in ADR-0003).
//! Requirements: `docs/SPEC.md` §4.1–§4.3.

#![cfg_attr(
    test,
    allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)
)]
