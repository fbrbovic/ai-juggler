//! Tauri application shell (ADR-0001) — arrives with milestone M1.
//!
//! Will host the Tauri 2 commands and typed events that connect the webview UI
//! (React + `TypeScript`) to the Rust core, plus updater wiring and the desktop
//! entry point. The UI is a view: it holds no business logic; all state
//! originates in Rust and arrives as pre-aggregated, typed events
//! (`docs/plan/QUALITY.md` §4).

#![cfg_attr(
    test,
    allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)
)]
