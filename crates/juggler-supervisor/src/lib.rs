//! Process supervision and resource management (spec §5).
//!
//! Owns the lifecycle of every managed harness session: PTY spawning with
//! per-profile environments, the lifecycle event bus, process-tree tracking,
//! the stop ladder, output capture, the supervision journal for crash
//! re-adoption, and (from M2) stuck-detection heuristics and the resource
//! manager. It talks to harnesses only through `juggler_adapter` types.

#![cfg_attr(
    test,
    allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)
)]

pub mod error;
pub mod group;
pub mod heuristics;
pub mod journal;
pub mod lifecycle;
pub mod orphans;
pub mod ring;
pub mod supervisor;
pub mod tree;

pub use error::SupervisorError;
pub use heuristics::{
    AttentionTransition, HeuristicEngine, HeuristicInputs, HeuristicThresholds, prompt_alive,
    quota_signature,
};
pub use journal::{AdoptedProcess, Journal};
pub use lifecycle::{ExitKind, LifecycleEvent};
pub use orphans::{
    AUTO_CLEANUP_GRACE_MS, DescendantRow, ExitSnapshot, ORPHAN_GRACE_MS, Orphan, OrphanTracker,
    OwnerState, StopOutcome, process_alive, stop_pid_ladder, verify_owner,
};
pub use ring::OutputRing;
pub use supervisor::{
    LiveProcessStats, LiveStatsCollector, ProcessRow, SpawnedProcess, Supervisor, TreeSample,
};
