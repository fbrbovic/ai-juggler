# Billing & renewal-date sources (AJ-E309, D-12)

> D-12 law: renewal dates are USER-ENTERED, optional, and
> research-gated — **no code may auto-populate a renewal date until a
> sanctioned source is verified live by the maintainer.** This note
> records the per-Provider evidence trail. "No source found" is an
> acceptable, honest entry.

| Provider | Sanctioned source? | Evidence | Verified live? |
|---|---|---|---|
| claude-code (Anthropic) | Candidate — vendor web console | The Anthropic Console (console.anthropic.com → Settings → Billing) shows plan/renewal for org accounts; no documented public API for the subscription renewal date. Claude Code CLI itself exposes no renewal data. | NO — maintainer to verify against their console (needs an org subscription) |
| codex (OpenAI) | Candidate — vendor billing page | platform.openai.com → Settings → Billing shows plan cycles; no public API for renewal dates on ChatGPT/Codex subscriptions. | NO — maintainer to verify |
| gemini (Google) | No source found | Gemini CLI has no documented billing/renewal surface accessible programmatically; Google One/Workspace renewal data is account-scoped and not exposed to the CLI. | n/a |
| antigravity | No source found | No documented billing API for this harness. | n/a |

Access date for all web evidence above (reviewed, not fetched
programmatically): 2026-10-18. Sources reviewed: vendor documentation
index pages and the CLI help output captured during AJ-E302's scan
research.

## Standing rule

Until a row above flips to "Verified live — YES" with a dated
maintainer confirmation, `renewal_date` remains strictly
user-entered: the onboarding Confirm row and the Accounts editor are
the only writers, empty persists as unknown, and no inference path
exists (pinned by D-12's absence in AJ-E303's inference fields).
