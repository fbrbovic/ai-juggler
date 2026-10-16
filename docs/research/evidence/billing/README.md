# Billing-source evidence (AJ-E601)

Redacted live captures backing the grades in
`docs/research/billing-source-protocol.md`. One file per capture, named
`<provider>-<surface>-<YYYYMMDD>.md`, using the protocol's §2 template.

Rules:

- Secrets NEVER land here: before any commit, the folder is grepped
  for the classic credential markers (API-key prefixes, auth-header
  tokens, cookie headers — spelled without literals here so the grep
  stays self-clean) — zero hits or the capture does not merge.
- Excerpts are minimal (the field path plus just enough shape to pin
  the type); account ids, session ids, and user paths are redacted.
- A capture is immutable once committed; a later Provider release gets
  a NEW capture file (the demotion rule's paper trail, protocol §6).

| Capture | Provider | Class | Grade | Verification |
|---|---|---|---|---|
| `claude-statusline-20261013.md` | Claude | quota | documented-sanctioned | candidate (live field observation pending the authenticated session — see the capture Note) |
