# AI Juggler — developer tasks. Standards: docs/plan/QUALITY.md.
# `just verify` is the local gate; CI runs the same checks.

default: verify

# Full local gate — everything CI enforces.
verify:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo doc --workspace --no-deps

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
