//! Application state: storage, adapter registry, supervisor — the single
//! backend instance behind every Tauri command (spec §2).

use std::path::PathBuf;
use std::sync::Mutex;

use juggler_adapter::AdapterRegistry;
use juggler_core::storage::Storage;
use juggler_supervisor::Supervisor;

/// Everything the commands operate on. `storage` is `Mutex`-wrapped:
/// rusqlite connections are `Send` but not `Sync`.
pub struct AppState {
    /// App data directory (databases, managed accounts, packs).
    pub data_dir: PathBuf,
    /// SQLite persistence (accounts, profiles, sessions).
    pub storage: Mutex<Storage>,
    /// Registered harness adapters.
    pub registry: AdapterRegistry,
    /// Process supervisor owning launched sessions.
    pub supervisor: Supervisor,
    /// Terminal ids with a live output forwarder (dedup: one forwarder per
    /// terminal no matter how often the UI re-attaches).
    pub terminal_taps: Mutex<std::collections::HashSet<String>>,
}

impl AppState {
    /// Builds the state rooted at `data_dir` with the built-in adapters.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::AppError`] when storage or the supervision
    /// journal cannot be opened.
    pub fn init(data_dir: PathBuf) -> Result<Self, crate::error::AppError> {
        Self::init_with_registry(data_dir, default_registry())
    }

    /// Builds the state with an explicit adapter registry (tests inject
    /// stub-backed adapters here).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::AppError`] when storage or the supervision
    /// journal cannot be opened.
    pub fn init_with_registry(
        data_dir: PathBuf,
        registry: AdapterRegistry,
    ) -> Result<Self, crate::error::AppError> {
        std::fs::create_dir_all(&data_dir)
            .map_err(|error| crate::error::AppError::io("create app data dir", &error))?;
        let storage = Storage::open(&data_dir.join("juggler.sqlite"))?;
        let supervisor = Supervisor::with_journal(&data_dir.join("supervision"))?;
        // Crash/restart reconciliation: sessions recorded running whose
        // processes did not survive the restart are marked stopped —
        // never a permanently fake "live" dot (survivors stay running).
        let adopted = supervisor.alive_adopted_sessions();
        {
            let records = storage.sessions().list(None).unwrap_or_default();
            for record in records {
                if matches!(
                    record.status,
                    juggler_core::model::SessionStatus::Starting
                        | juggler_core::model::SessionStatus::Running
                ) && !adopted.contains(&record.id)
                {
                    let mut reconciled = record;
                    reconciled.status = juggler_core::model::SessionStatus::Stopped;
                    reconciled.last_activity = Some(juggler_core::model::Timestamp::now());
                    let _ = storage.sessions().update(&reconciled);
                }
            }
        }
        Ok(Self {
            supervisor,
            registry,
            storage: Mutex::new(storage),
            terminal_taps: Mutex::new(std::collections::HashSet::new()),
            data_dir,
        })
    }

    /// Directory holding managed harness account state.
    #[must_use]
    pub fn accounts_dir(&self) -> PathBuf {
        self.data_dir.join("accounts")
    }
}

/// The built-in adapter set (community manifests arrive post-v1).
#[must_use]
pub fn default_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry.register(std::sync::Arc::new(
        juggler_adapters::ClaudeCodeAdapter::new(),
    ));
    registry
}

/// Resolves the per-platform app data directory (spec §11 layout).
///
/// # Errors
///
/// Returns a message string when no OS data directory can be determined.
pub fn resolve_data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|base| base.join("AI Juggler"))
        .ok_or_else(|| "no OS data directory available".to_string())
}
