# Contributing to AI Juggler

Thanks for your interest in making AI Juggler better! 🤹

## Current phase: spec & design

The project is in active design. The highest-value contributions right now are:

1. **Read the [proposal](./docs/PROPOSAL.md) and [spec](./docs/SPEC.md)** and open issues with feedback — missing use cases, compliance concerns, architecture risks, UX wishes.
2. **Share your juggling setup.** How do you run multiple accounts/harnesses today? What breaks? Concrete stories become test cases for the design.
3. **Research corrections.** If any claim in [docs/research](./docs/research/) is outdated or wrong, PRs with sources are very welcome.

## Ways to contribute (once code lands)

- Bug reports and reproducible issues
- Features and fixes (please link an issue first for anything non-trivial)
- New harness adapters (the adapter guide will live in `docs/` when the API stabilizes)
- Docs, blog posts, and screencasts

## Development setup

The codebase (Rust + Tauri) does not exist yet — it arrives with milestone M0/M1. This section will be filled in with build instructions, architecture pointers, and test commands as soon as the scaffold lands.

Planned toolchain: stable Rust, pnpm/Node for the UI, and standard `cargo fmt` / `clippy` / `cargo test` gates in CI.

## Commit & PR conventions

- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/) — `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`.
- **PRs:** small and focused, described in terms of the user-facing change. Link the issue you're addressing. CI must pass.
- **Scope creep:** split it into another PR.

## Licensing

By contributing, you agree that your contributions will be licensed under the [MIT License](./LICENSE) that covers this repository.

## Code of conduct

Be excellent to each other. See [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md); violations should be reported privately to the maintainer via GitHub (@fbrbovic).
