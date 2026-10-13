# ADR-0013: Fleet at launch

**Status:** Accepted
**Date:** 2026-10-13
**Deciders:** Vuxtra engineering + maintainer (decision-log D-13)
**Implements:** D-13 (and SPEC §11)

## Context

Pass-2 recommended schema-only fleet (defer build post-1.0). The
maintainer overruled (D-13): Fleet ships with the launch, in beta, so
multi-machine integrates cleanly rather than being retrofitted.

## Decision

1. Fleet is a **Phase-0 architecture constraint**: identity
   (`MachineId`) on every ledger row, Switch Graph node, and usage
   event; ownership/verification/audit per machine; offline and
   conflict behaviour designed in.
2. Launch scope (beta): pairing (6-digit/QR, identity pinning),
   gRPC/mTLS transport (ADR-0004 preserved), remote Inspector (view),
   remote switch and cleanup under per-machine permissions
   (`View`/`Control`/`NoAccess`).
3. **Never**, at any tier: credential sync, login automation, traffic
   proxying, cloud relay. The §1/§8 invariants hold on every node; a
   console never acts on a node's spawn ledger without that node's
   permission.
4. The fleet security review (STRIDE: spoofed mDNS, MITM pre-pinning,
   replayed commands, stolen identity keys, malicious paired peer) is
   a **beta-exit gate** (AJ-E710).

## Alternatives considered

- **Schema-only now, build post-1.0** — the pass-2 recommendation;
  rejected by D-13 as retrofit risk.
- **Full fleet parity at launch (equal to local)** — rejected: remote
  control under permissions is the honest beta scope; full parity
  follows validation.

## Consequences

E7 ships identity/transport/pairing early and remote surfaces later in
beta; local designs must stay fleet-shaped in review; scope and
estimates re-planned around D-13 (recorded in EPICS phases).
