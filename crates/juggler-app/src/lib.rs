//! Tauri application shell (ADR-0001).
//!
//! Hosts the Tauri 2 commands and typed events that connect the webview UI
//! (React + `TypeScript`) to the Rust core, plus updater wiring (M5) and the
//! desktop entry point. The UI is a view: it holds no business logic; all
//! state originates in Rust and arrives as pre-aggregated, typed events
//! (`docs/plan/QUALITY.md` §4).

#![cfg_attr(
    test,
    allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)
)]

mod commands;

use tauri::Emitter;

/// Name of the event emitted once the backend is ready; the payload is
/// the `AppInfo` value.
pub const EVENT_APP_READY: &str = "app://ready";

/// Builds and runs the desktop application.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .setup(|app| {
            let info = commands::AppInfo::current();
            let _ = app.emit(EVENT_APP_READY, &info);
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("ai-juggler: fatal: {error}");
            std::process::exit(1);
        });
}
