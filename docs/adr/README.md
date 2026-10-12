# Architecture Decision Records

ADRs capture the *why* behind non-obvious decisions: context, decision, alternatives, consequences. They are immutable once accepted (superseded by a new ADR, never edited in place).

| # | Decision | Status |
|---|---|---|
| [0001](./0001-tauri-2-desktop-framework.md) | Desktop framework: Tauri 2 | Accepted |
| [0002](./0002-harness-adapter-architecture.md) | Harness integration via adapter trait + capabilities | Accepted |
| [0003](./0003-automation-policy-compliance-guardrails.md) | Automation policy & compliance guardrails | Accepted |
| [0004](./0004-fleet-transport-grpc-mtls.md) | Fleet transport: gRPC over mTLS + mDNS pairing | Accepted |
| [0005](./0005-mit-license.md) | License: MIT | Accepted |
| [0006 — Smart onboarding: environment scan and config import](0006-smart-onboarding-scan-and-import.md) | Accepted |

To add one: copy `0000-template.md`, number sequentially, PR it with the change that implements the decision.
