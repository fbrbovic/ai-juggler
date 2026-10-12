//! Domain model, storage, and usage accounting for AI Juggler.
//!
//! This crate is the pure heart of the system: entities (Account,
//! Profile, `SessionRecord`, usage snapshots and windows), the SQLite
//! persistence layer, and quota/credit math. It contains no I/O beyond its
//! storage layer, knows nothing about specific harnesses, the app shell, or
//! the UI, and must stay fully unit-testable (architecture rules in
//! `docs/plan/QUALITY.md` §4; requirements in `docs/SPEC.md` §3, §6).

#![cfg_attr(
    test,
    allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)
)]
