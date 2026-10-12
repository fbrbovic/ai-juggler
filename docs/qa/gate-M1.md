# Gate V1 — Milestone M1 acceptance (M1-7)

Status: **PREPARED — awaiting dogfood run + user confirmation**
Prepared: 2026-10-12

M1 scope: two (or more) Claude Code accounts running in parallel through
the real app, fully isolated, for a full working session each — the
founding pain point (EPICS.md M1).

## Dogfood environment (prepared, real)

The maintainer's machine carries **five t3.codes-managed Claude profiles**
under `~/Library/Application Support/ClaudeCodeProfiles/`. All five were
imported into AI Juggler as read-only copies (PR #37 operational runner):

| Account | Imported from | Size | Login state | Needs |
|---|---|---|---|---|
| bookauth-anela | t3.codes profile | 293 MB | logged out in copy | one guided re-login |
| bookauth-luka | t3.codes profile | 541 MB | logged out in copy | one guided re-login |
| bookauth-mia | t3.codes profile | 425 MB | logged out in copy | one guided re-login |
| bookauth-olivia | t3.codes profile | 6.7 MB | logged out in copy | one guided re-login |
| personal-max | t3.codes profile | 340 MB | logged out in copy | one guided re-login |

Why re-login: the originals authenticate through the **shared pre-2.1.56
keychain entry** (`Claude Code-credentials`), which is path-locked and
shared — copies cannot (and should not) carry it. The wizard's login step
performs this per account (Sign in inside AI Juggler → embedded terminal →
Anthropic OAuth). **t3.codes originals were never modified** (manifest
evidence in PR #37) and its live sessions kept running throughout.

## Acceptance checklist

- [ ] Two accounts re-logged-in via the in-app guided login
- [ ] Both launched in parallel on the **same project** (juggling anchor):
      distinct `CLAUDE_CONFIG_DIR`s, separate transcripts, no cross-talk
- [ ] A full working session in each (prompt → edit → verify) through the
      embedded terminals
- [ ] Projects rail shows the directory once with both accounts in history;
      each session's panel embeds its terminal
- [ ] Session statuses transition running → finished in the rail
- [ ] `~/.claude` and the t3 originals remain untouched after the run
      (re-run the manifest check)
- [ ] Guardrail suite green: `just verify` (isolation, write containment,
      import read-only, scanner honesty)
- [ ] Quality audit #2 (code review pass over M1 diff)
- [ ] **User confirmation** — M1 closes

## Out of scope (tracked, not blockers)

- Quota dashboards / usage timelines (M2–M4)
- Hand-over between accounts (data lineage already recorded: project ↔
  session ↔ account)
- Desktop-class harness adapters (Claude.app on this machine is the Claude
  chat desktop app, not Claude Code — separate product; adapter research
  is its own milestone with a ToS check)
