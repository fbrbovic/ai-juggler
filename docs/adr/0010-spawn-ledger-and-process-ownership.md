# ADR-0010: Spawn ledger and verified process ownership

**Status:** Accepted
**Date:** 2026-10-13
**Deciders:** Vuxtra engineering + maintainer (decision-log D-08)
**Implements:** D-08 (and SPEC §3.2, §5.2)

## Context

Verified failure mode: Instances exit and leave dev servers, MCP
transports, watchers and language servers running for days (ports +
RAM held). The original product invariant said *all* automation is
passive — but the maintainer's workflow explicitly asks for automatic
and manual cleanups "all with verifications"
(docs/expected-worfklow.md:18). D-08 is the narrow, written amendment.

## Decision

1. **Spawn ledger is the ownership authority**: intent recorded before
   spawn; row = pid, pgid, machine, instance, provider, account, argv
   hash, cwd, started_at, exit, reaped. Not-in-ledger ⇒ never touched
   (**fail closed**, AJ-E503).
2. **ProcessIdentity verify-before-kill** at click time (pid + pgid +
   start-time + argv hash re-verified; full command line, cwd, ports
   shown), then a staged TERM→grace→KILL ladder with per-process
   outcomes and port re-probe.
3. **Tiers, per Account (not per Provider):** Tier 0 notify-only
   (default); Tier 1 "Clean safe (this Account)" opt-in — automatic
   kill of *verified-owned* leftovers only, every kill audit-logged.
   Manual verified cleanup always available and always audited.
4. Preserved: no SIGSTOP; unowned processes untouchable in every tier;
   every ledger row carries MachineId (fleet-shaped, D-13).

## Alternatives considered

- **Per-Provider tiers** — rejected (D-08 wording is per-Account; a
  blanket Provider kill risks exactly the wrong-account failure).
- **Pure notify-only forever** — rejected by the maintainer's explicit
  ask; the amendment keeps the default passive anyway.
- **ppid-walk as ownership source** — rejected: racy after
  intermediate exits; the ledger is written with intent before spawn.

## Consequences

The §1 automation invariant now carries exactly two recorded
amendments (this and D-14's user-owned policy). E5 builds the ledger,
reconciler, and Cleanup Centre; zero-false-kill is a beta-exit metric.
