# Security Policy

AI Juggler is in the spec phase — there are no released binaries yet. This policy describes how we handle security from day one, and the security commitments the product design makes.

## Reporting a vulnerability

Please use **GitHub's private vulnerability reporting** on this repository (Security → Report a vulnerability). If that is not possible, contact the maintainer privately via GitHub (@fbrbovic).

- Do **not** open public issues for security problems.
- We will acknowledge reports within a few days and keep you informed of the fix.

## Scope

Once code ships, the following are in scope:

- The AI Juggler application (Rust core, UI, updater)
- Handling of credentials, tokens, and session data
- The self-update mechanism (signature validation, downgrade attacks)
- Anything that could compromise a user's harness accounts or projects

## Design commitments

These are binding goals of the architecture (see the [spec](./docs/SPEC.md)):

- **Credentials live in the OS keychain** (macOS Keychain, Windows Credential Manager, Linux Secret Service) — never in plaintext config files.
- **Local-first:** transcripts, usage data, and indexes stay on the user's machine. No cloud services. Any telemetry is strictly opt-in and documented.
- **Signed updates only:** self-updates are verified against a published signature before installation.
- **Least privilege:** the app manages only the accounts and paths the user explicitly configures.
