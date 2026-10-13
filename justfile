# AI Juggler — developer tasks. Standards: docs/plan/QUALITY.md.
# `just verify` is the local gate; CI runs the same checks.

default: verify

# Full local gate — everything CI enforces.
verify:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo doc --workspace --no-deps
    just evidence

# Evidence-store gate (AJ-E801): link resolution + secret scan + index freshness.
evidence:
    ./scripts/evidence_check.sh

# UI gate (AJ-E806): typecheck + build. Visual (AJ-E803) and a11y (AJ-E804)
# gates attach to UI-changing PRs; perf (AJ-E805) at milestones.
ui:
    pnpm --dir ui build

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

doc:
    cargo doc --workspace --no-deps

# Run the desktop app in dev mode (UI hot-reload + Rust rebuild on change).
dev:
    cargo tauri dev

# Release-mode app bundle under target/release/bundle/ (unsigned dev build).
app:
    cargo tauri build
