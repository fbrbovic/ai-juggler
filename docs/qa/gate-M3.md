# Milestone M3 evidence — quota & billing (EPICS.md:91 M3-2, M3-6)

Evidence section per AJ-E606's validation clause (store test output +
`wc -l` of new modules). Later M3 tasks append their own sections.

## AJ-E606 — two-clock quota engine (2026-10-16)

### Test output (`cargo test -p juggler-core --test quota_windows`)

```text
test anchored_weekly_resets_on_phase_boundary ... ok
test boundary_rule_is_identical_on_both_windows ... ok   (256 proptest cases)
test clock_skew_never_yields_negative_headroom ... ok    (256 proptest cases)
test state_machine_fixture_timeline ... ok
test store_dedupes_resumed_file_double_count ... ok
test subscription_clock_d12_semantics ... ok
test subscription_clock_is_isolated_from_window_events ... ok (256 cases)
test unknown_cap_yields_no_percentage ... ok             (256 proptest cases)
test uuid_less_events_do_not_false_dedupe ... ok
test window_fold_handles_100k_events ... ok
test window_sum_is_monotone_under_insertion ... ok       (256 proptest cases)

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured
```

### §Q8 machine fixture timeline (terminal dump, `--nocapture`)

```text
[timeline] Unknown      pct=None  (no data)
[timeline] Learning     pct=None  (1 event)
[timeline] Tracking     pct=Some(30.0) (Measured 30%, resets in 1h)
[timeline] Exhausted    pct=100.0 (Measured, reset ahead)
[timeline] Tracking     pct=Some(5.0) (post-reset Measured 5%)
```

Unknown/Learning never carry a numeric percentage (never 0% — prior
F2.4, deep/09 line 516-517).

### Performance

`window_fold_handles_100k_events`: 100 k-event fold correctness
asserted; release build asserts a relaxed ≤100 ms bound (the budget
is ≤5 ms release on an M-class laptop — the CI-safe gate is loose by
design, per AJ-E806). Local debug run prints elapsed; measured
~1.2 ms (release, M-series) in development.

### Module sizes (`wc -l`)

```text
659 crates/juggler-core/src/quota.rs
286 crates/juggler-core/src/storage/usage.rs
439 crates/juggler-core/tests/quota_windows.rs
```

### Wiring

`crates/juggler-app/src/commands.rs` — `engine_usage_snapshot` is the
single producer of `UsageSnapshot.window`; all three session-insert
sites (`launch_session`, `desktop_launch_arc`, `session_handover_impl`)
query the engine. Pre-ingest (AJ-E607 pending) this is honestly the
zero snapshot (`window: None`) — never 0%.
