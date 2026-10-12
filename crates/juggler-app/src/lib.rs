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

pub mod commands;
pub mod error;
pub mod state;
pub mod terminal;

use std::sync::Arc;

use tauri::Emitter;

/// Name of the event emitted once the backend is ready; the payload is
/// the `AppInfo` value.
pub const EVENT_APP_READY: &str = "app://ready";

/// Name of the event carrying the refreshed session list.
pub const EVENT_SESSIONS_UPDATED: &str = "session://updated";

/// Builds and runs the desktop application.
pub fn run() {
    let data_dir = state::resolve_data_dir().unwrap_or_else(|message| {
        eprintln!("ai-juggler: cannot resolve data dir: {message}");
        std::process::exit(1);
    });
    if let Err(message) = std::fs::create_dir_all(&data_dir) {
        eprintln!(
            "ai-juggler: cannot create {}: {message}",
            data_dir.display()
        );
        std::process::exit(1);
    }
    let app_state = Arc::new(state::AppState::init(data_dir).unwrap_or_else(|error| {
        eprintln!("ai-juggler: fatal: {error}");
        std::process::exit(1);
    }));

    // Lifecycle forwarder: supervisor events → storage status transitions.
    // The UI refreshes sessions from command results and `session://updated`.
    {
        let state = Arc::clone(&app_state);
        std::thread::spawn(move || {
            while let Some(event) = state
                .supervisor
                .recv_timeout(std::time::Duration::from_millis(500))
            {
                if let Ok(storage) = state.storage.lock() {
                    drop(commands::apply_lifecycle_event(&storage, event));
                }
            }
        });
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::harnesses_list,
            commands::accounts_list,
            commands::account_create,
            commands::account_login_state,
            commands::account_login_command,
            commands::environment_scan,
            commands::account_import,
            commands::login_open_web,
            commands::profiles_list,
            commands::profile_create,
            commands::profile_delete,
            commands::launch_session,
            commands::sessions_list,
            commands::stop_session,
            commands::dashboard_stats,
            commands::terminal_spawn_login,
            commands::terminal_attach,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_kill,
            commands::terminal_snapshot,
        ])
        .setup(|app| {
            let _ = app.emit(EVENT_APP_READY, commands::AppInfo::current());
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("ai-juggler: fatal: {error}");
            std::process::exit(1);
        });
}
