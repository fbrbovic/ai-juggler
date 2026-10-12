//! The `HarnessAdapter` contract — the only place harness specifics are allowed
//! to live (ADR-0002).
//!
//! This crate defines the `HarnessAdapter` trait, capability flags,
//! discovery reports, launch specs, and session-watching interfaces that every
//! harness integration (Claude Code, Codex, `ZCode`, future community adapters)
//! implements. It is semver-protected public API: external adapters build
//! against it, so breaking changes are guarded by `cargo-semver-checks` in CI.
//! Requirements: `docs/SPEC.md` §4.

#![cfg_attr(
    test,
    allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)
)]
