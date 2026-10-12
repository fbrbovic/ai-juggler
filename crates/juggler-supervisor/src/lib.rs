//! Process supervision and resource management (spec §5).
//!
//! Owns the lifecycle of every managed harness session: PTY spawning with
//! per-profile environments, the lifecycle event bus, process-tree tracking,
//! stuck-detection heuristics, the resource manager (system + per-session
//! monitoring, admission advisor, overload alerts), and crash recovery.
//! It talks to harnesses only through `juggler_adapter` types.

#![cfg_attr(
    test,
    allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)
)]
