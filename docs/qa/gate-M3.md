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

## AJ-E607 — ingest pipeline (2026-10-16)

### Fixture corpus (`crates/juggler-adapters/tests/fixtures/quota/`, shapes from the AJ-E602–E604 packets, ids redacted)

`claude-statusline.spool` (4 lines: 2 quota-bearing, 1 cost-only, 1
malformed) · `claude-transcript.jsonl` (keyed usage lines + one
no-uuid usage line + one resumed duplicate) · `codex-rollout.jsonl`
(session_meta + 2 `token_usage_record` with rollup fields + noise) ·
`zcode-model-usage.sql` (peak + off-peak + unknown-model + non-completed
rows) · `user-statusline.sh` (chaining marker fixture).

### Test output (`cargo test -p juggler-adapters --test quota_ingest`)

```text
test gemini_has_no_emitter_by_design ... ok
test install_chains_and_uninstall_restores_exactly ... ok
test install_over_nothing_uninstalls_to_nothing ... ok
test iso_timestamps_parse_strictly ... ok
test no_http_client_anywhere_in_the_adapters_crate ... ok
test re_ingesting_the_same_fixtures_inserts_zero_new_rows ... ok
test rollout_events_read_only_the_response_level_usage ... ok
test spool_parses_with_zero_dropped_valid_lines ... ok
test spool_rotates_at_threshold ... ok
test spool_tail_is_offset_incremental_and_truncation_aware ... ok
test statusline_wrapper_chains_user_script_byte_for_byte ... ok
test transcript_events_skip_unkeyed_and_double_usage ... ok
test zcode_events_compute_credits_with_peak_and_offpeak ... ok

test result: ok. 13 passed; 0 failed
```

Pipeline-level (`cargo test -p juggler-app --test usage_pipeline`):
`one_pass_ingests_all_sources_and_repeats_insert_nothing ... ok` — one
pass ingests spool+transcript (duplicate uuid absorbed), the wrapper
installs, the second pass inserts ZERO rows, health records both
sources with `last_event_at`.

### Idempotency proof (Req.3, COUNT-level)

Store-level: re-running the transcript fixture ingest twice →
`events_for_account` len identical (2: `a-1`, `a-2` once each) — the
resumed-file duplicate is absorbed by `UNIQUE(account, msg_uuid)`.
Pipeline-level: pass 2 with no file changes inserts zero rows
(offset-tracked tails).

### No-HTTP gate (Quality gate)

`no_http_client_anywhere_in_the_adapters_crate`: Cargo.toml carries no
HTTP client dependency (reqwest/ureq/isahc/curl/hyper) and no source
file constructs any network client (TcpStream/UdpSocket included) —
grep-pinned in CI on every run.

### Chaining gate (deep/09 §Q9)

`statusline_wrapper_chains_user_script_byte_for_byte`: the wrapper's
chained output is byte-identical to running the user statusline
directly on the same stdin bytes, while the spool captures the payload
— both outputs visible, no duplication.

### Pending live evidence (rides the maintainer login — gate-E5 open item)

The live-dogfood (one real authenticated Claude session ≥20 min where
the spool-derived 5h % matches `/usage` within ±5%; one terminal
screenshot of the chained statusline) requires an authenticated
session; the maintained login is pending re-auth (the same blocker as
the E5 authenticated-helper dogfood, gate-E5.md). Fixture gates above
pass; the live check is recorded as PENDING here and rides the next
authenticated session.
