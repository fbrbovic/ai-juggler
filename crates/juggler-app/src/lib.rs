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

/// Name of the 5 s resource tick (I-07/I-08 deltas, AJ-E401): one
/// coalesced payload per tick for every active Instance.
pub const EVENT_SESSIONS_RESOURCES: &str = "session://resources";

/// Name of the 30 s quota tick (I-05 deltas, AJ-E401): one coalesced
/// payload per tick; per-Instance `quota` stays `null` until a feed
/// exists (AJ-E606+).
pub const EVENT_SESSIONS_QUOTA: &str = "session://quota";

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

    spawn_lifecycle_forwarder(Arc::clone(&app_state));

    let attention_state = Arc::clone(&app_state);
    let heuristic_tick_state = Arc::clone(&app_state);
    let resource_tick_state = Arc::clone(&app_state);
    let quota_tick_state = Arc::clone(&app_state);

    tauri::Builder::default()
        // Registered first: a second launch defers to the running app
        // (focus its window) instead of stacking ghost instances.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::harnesses_list,
            commands::legacy_aliases::harnesses_list_alias,
            commands::accounts_list,
            commands::account_create,
            commands::account_login_state,
            commands::account_login_command,
            commands::environment_scan,
            commands::account_import,
            commands::login_open_web,
            commands::profiles_list,
            commands::legacy_aliases::profiles_list_alias,
            commands::profile_create,
            commands::profile_delete,
            commands::launch_session,
            commands::sessions_list,
            commands::legacy_aliases::sessions_list_alias,
            commands::projects_list,
            commands::projects_backfill,
            commands::session_handover,
            commands::stop_session,
            commands::dashboard_stats,
            commands::terminal_spawn_login,
            commands::terminal_attach,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_kill,
            commands::terminal_snapshot,
            commands::instance_quality,
        ])
        .setup(|app| {
            let _ = app.emit(EVENT_APP_READY, commands::AppInfo::current());
            spawn_read_model_ticks(
                app.handle().clone(),
                resource_tick_state,
                quota_tick_state,
                attention_state,
                heuristic_tick_state,
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("ai-juggler: fatal: {error}");
            std::process::exit(1);
        });
}

/// Read-model ticks (AJ-E401 Req.4): `session://updated` stays event-driven
/// (<300 ms via the lifecycle forwarder); these two add the 5 s resource
/// and 30 s quota refreshes as separate, coalesced emissions. Each tick
/// emits at most one payload and nothing at all while no Instance is
/// active, so an idle app costs the UI zero wakeups.
fn spawn_read_model_ticks(
    app: tauri::AppHandle,
    resource_state: Arc<state::AppState>,
    quota_state: Arc<state::AppState>,
    attention_state: Arc<state::AppState>,
    heuristic_state: Arc<state::AppState>,
) {
    // Event-driven recovery (AJ-E405 Req.5): drain fresh-output signals
    // at 200 ms — well inside the 300 ms budget.
    {
        let attention_app = app.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(200));
                let _ = commands::attention_recovery_drain(&attention_app, &attention_state);
            }
        });
    }
    // The 15 s heuristic cycle (SPEC §5.1): debounced Stuck/WaitingUser
    // evaluation; quota exits classify immediately at the forwarder.
    {
        let heuristic_app = app.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(15));
                let _ = commands::heuristic_tick(&heuristic_app, &heuristic_state);
            }
        });
    }
    let quota_app = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let _ = commands::emit_resources(&app, &resource_state);
        }
    });
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            let _ = commands::emit_quota(&quota_app, &quota_state);
        }
    });
}

/// The lifecycle forwarder: supervisor events → storage status
/// transitions, with AJ-E405 exit classification (rate-limit signature →
/// [`SessionStatus::QuotaBlocked`]) applied before the status mapping runs.
fn spawn_lifecycle_forwarder(state: Arc<state::AppState>) {
    std::thread::spawn(move || {
        while let Some(event) = state
            .supervisor
            .recv_timeout(std::time::Duration::from_millis(500))
        {
            let stopped_by_user = state.supervisor.was_stopped(event.session());
            // AJ-E405: classify non-zero exits against the captured
            // output tail BEFORE the status mapping runs — a rate-limit
            // signature is QuotaBlocked (immediate), not a crash.
            let quota_session = event.session().clone();
            let quota_blocked = match &event {
                juggler_supervisor::LifecycleEvent::Exited { kind, session, .. } => {
                    if stopped_by_user || kind.is_success() {
                        None
                    } else if let juggler_supervisor::ExitKind::Code(code) = kind {
                        let tail = state
                            .supervisor
                            .output_tail(session, 4096)
                            .unwrap_or_default();
                        state
                            .heuristics
                            .classify_exit(*code, &tail, now_millis(), None)
                    } else {
                        None
                    }
                }
                juggler_supervisor::LifecycleEvent::Started { .. } => None,
            };
            if let Ok(storage) = state.storage.lock() {
                drop(commands::apply_lifecycle_event(
                    &storage,
                    event,
                    stopped_by_user,
                    quota_blocked.as_ref().map(|transition| &transition.reason),
                ));
                // A quota exit ends the process — forget debounce.
                if quota_blocked.is_some() {
                    state.heuristics.forget(&quota_session);
                }
            }
        }
    });
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|dur| i64::try_from(dur.as_millis()).unwrap_or_default())
        .unwrap_or_default()
}
