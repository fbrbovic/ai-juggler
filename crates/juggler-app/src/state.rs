//! Application state: storage, adapter registry, supervisor — the single
//! backend instance behind every Tauri command (spec §2).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use juggler_adapter::AdapterRegistry;
use juggler_core::model::SessionId;
use juggler_core::storage::Storage;
use juggler_supervisor::tree::LedgerStore as _;
use juggler_supervisor::{
    HeuristicEngine, HeuristicThresholds, LiveStatsCollector, OrphanTracker, Supervisor,
};

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
    /// Batched live process stats for the Instance read model (AJ-E401):
    /// owns the persistent process table so CPU% deltas survive ticks.
    /// Lock ordering: never held together with `storage`.
    pub resources: Mutex<LiveStatsCollector>,
    /// Exit snapshots → confirmed orphans + stop states (AJ-E410).
    /// Interior-mutex: safe to read without the collector lock.
    pub orphans: OrphanTracker,
    /// The persistent spawn ledger handle (AJ-E501): queries + prune.
    pub ledger: std::sync::Arc<juggler_supervisor::tree::CoreLedgerStore>,
    /// The one live preview session (AJ-E415 Req.5: one at a time).
    /// Lock ordering: never held together with `storage`.
    pub preview: Mutex<Option<crate::commands::PreviewSession>>,
    /// Per-Instance transcript parse offsets for running-model
    /// detection (AJ-E412): how many bytes each tail-scan has consumed,
    /// so the 2 s tick only ever parses NEW lines.
    pub model_offsets: Mutex<std::collections::HashMap<SessionId, u64>>,
    /// Stuck/waiting/quota heuristics (AJ-E405); thresholds user-tunable
    /// via `heuristics.json`.
    pub heuristics: HeuristicEngine,
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
        // AJ-E501: the persistent spawn ledger — its OWN connection to
        // the same WAL store (spawn threads + reconciler ticks write
        // without the app's storage mutex), attached to the supervisor
        // (intent-before-spawn, helper upserts, exit patches).
        let ledger = std::sync::Arc::new(juggler_supervisor::tree::CoreLedgerStore::new(
            juggler_core::storage::ProcessLedgerRepo::open(&data_dir.join("juggler.sqlite"))?,
        ));
        // Retention (Req.5): exited rows older than 7 days at startup;
        // running rows are never pruned. The recurring job rides the
        // E504 reconciler tick.
        let cutoff = juggler_core::model::Timestamp::now().millis() - 7 * 24 * 3_600_000;
        let _ = ledger.prune(cutoff);
        let supervisor =
            Supervisor::with_journal(&data_dir.join("supervision"))?.with_ledger(ledger.clone());
        // Orphan grace (AJ-E504 Req.3): the user-tunable default, from
        // settings; per-Provider overrides land with the Cleanup
        // Centre's settings surface (AJ-E506). 30 s when unset — the
        // false-alert budget, never lowered for demos.
        if let Some(secs) = storage
            .app_settings()
            .get("orphan_grace_secs")
            .ok()
            .flatten()
            .and_then(|value| value.parse::<u64>().ok())
        {
            supervisor.set_orphan_grace("", secs);
        }
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
            ledger,
            registry,
            storage: Mutex::new(storage),
            resources: Mutex::new(LiveStatsCollector::new()),
            orphans: OrphanTracker::new(),
            model_offsets: Mutex::new(std::collections::HashMap::new()),
            heuristics: HeuristicEngine::new(load_thresholds(&data_dir)),
            terminal_taps: Mutex::new(std::collections::HashSet::new()),
            preview: Mutex::new(None),
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

/// Loads the user-tunable heuristic thresholds from `heuristics.json`;
/// missing/corrupt files fall back to the SPEC §5.1 defaults (never a
/// crash — the Settings screen lands with its own task and writes here).
fn load_thresholds(data_dir: &Path) -> HeuristicThresholds {
    std::fs::read_to_string(data_dir.join("heuristics.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}
