//! AJ-E309 Req.6/7: the LOCAL onboarding-funnel event store —
//! emitted locally only (telemetry opt-in only; no network call in
//! this epic), capped at 10 MB with rotation, and schema-guarded:
//! events NEVER carry paths, labels, or credential-adjacent values
//! (the forbidden-key list is enforced at insert).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Keys that must NEVER appear in event fields (pinned by test).
pub const FORBIDDEN_KEYS: &[&str] = &[
    "path", "label", "token", "dir", "source", "account", "email", "key",
];

/// The store cap: on breach the file rotates (`.1` replaces).
const CAP_BYTES: u64 = 10 * 1024 * 1024;

/// One funnel event — a kind, a timestamp, and string fields.
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct FunnelEvent {
    /// `scan_started`, `proposal_accepted`, `login_skipped`, …
    pub kind: String,
    /// Emission time, epoch ms.
    pub at_ms: i64,
    /// Aggregate-safe fields only (counts, durations, words).
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

/// Validates one event against the schema guardrail.
///
/// # Errors
///
/// Returns [`crate::CoreError::Validation`] naming the forbidden key.
pub fn validate(event: &FunnelEvent) -> Result<(), crate::CoreError> {
    for forbidden in FORBIDDEN_KEYS {
        if event.fields.contains_key(*forbidden) {
            return Err(crate::CoreError::Validation {
                subject: "funnel event",
                message: format!("field '{forbidden}' is forbidden (no paths/labels/credentials)"),
            });
        }
    }
    Ok(())
}

/// The local event log (JSONL, rotated at 10 MB).
pub struct FunnelLog {
    path: PathBuf,
}

impl FunnelLog {
    /// Opens (or creates) the store under the app-data directory.
    #[must_use]
    pub fn open(app_data: &Path) -> Self {
        FunnelLog {
            path: app_data.join("funnel-events.jsonl"),
        }
    }

    /// Appends one validated event; rotates first when the file
    /// exceeds the cap. Rotation drops the oldest file — the cap
    /// holds, and emission stays off any hot path.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError`] on validation or I/O failure.
    pub fn append(&self, event: &FunnelEvent) -> Result<(), crate::CoreError> {
        validate(event)?;
        if let Ok(meta) = std::fs::metadata(&self.path) {
            if meta.len() >= CAP_BYTES {
                let _ = std::fs::rename(&self.path, self.path.with_extension("jsonl.1"));
            }
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::CoreError::Io(e.to_string()))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| crate::CoreError::Io(e.to_string()))?;
        let line = serde_json::to_string(event).map_err(|e| crate::CoreError::Io(e.to_string()))?;
        writeln!(file, "{line}").map_err(|e| crate::CoreError::Io(e.to_string()))?;
        Ok(())
    }

    /// Reads every stored event (oldest first). A corrupt line is
    /// skipped, never fatal.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError`] on I/O failure.
    pub fn read(&self) -> Result<Vec<FunnelEvent>, crate::CoreError> {
        let Ok(raw) = std::fs::read_to_string(&self.path) else {
            return Ok(Vec::new());
        };
        Ok(raw
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }
}

/// Req.7: the inference-override rate per field from the stored
/// events — `inference_edit{field}` vs `inference_shown{field}`;
/// fields over 30% are flagged for retuning.
#[must_use]
pub fn override_rates(events: &[FunnelEvent]) -> Vec<(String, f64, bool)> {
    let mut shown: BTreeMap<String, u64> = BTreeMap::new();
    let mut edits: BTreeMap<String, u64> = BTreeMap::new();
    for event in events {
        if let Some(field) = event.fields.get("field") {
            match event.kind.as_str() {
                "inference_shown" => *shown.entry(field.clone()).or_insert(0) += 1,
                "inference_edit" => *edits.entry(field.clone()).or_insert(0) += 1,
                _ => {}
            }
        }
    }
    shown
        .into_iter()
        .map(|(field, shown_count)| {
            let edited = edits.get(&field).copied().unwrap_or(0);
            // Every edit implies a prior show — impressions are
            // shown-without-edit PLUS edits.
            let impressions = shown_count + edited;
            let rate = if impressions == 0 {
                0.0
            } else {
                edited as f64 / impressions as f64
            };
            let flagged = rate > 0.30;
            (field, rate, flagged)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> (tempfile::TempDir, FunnelLog) {
        let dir = tempfile::tempdir().unwrap();
        let store = FunnelLog::open(dir.path());
        (dir, store)
    }

    fn event(kind: &str, fields: &[(&str, &str)]) -> FunnelEvent {
        FunnelEvent {
            kind: kind.to_owned(),
            at_ms: 0,
            fields: fields
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn forbidden_keys_are_rejected_by_name() {
        for key in ["path", "label", "token", "account"] {
            let err = validate(&event("x", &[(key, "anything")])).unwrap_err();
            assert!(err.to_string().contains("forbidden"), "{key}");
        }
        assert!(validate(&event("scan_completed", &[("providers", "3")])).is_ok());
    }

    #[test]
    fn append_read_roundtrip_and_corrupt_skip() {
        let (dir, store) = log();
        store
            .append(&event("scan_started", &[("providers", "1")]))
            .unwrap();
        store
            .append(&event("login_skipped", &[("remaining", "1")]))
            .unwrap();
        assert_eq!(store.read().unwrap().len(), 2);
        // A corrupt line is skipped, never fatal.
        std::fs::write(dir.path().join("funnel-events.jsonl"), "not json\n").unwrap();
        assert_eq!(store.read().unwrap().len(), 0);
    }

    #[test]
    fn override_rate_flags_over_30_percent() {
        // 6 shown-without-edit + 4 edited = 40% override rate.
        let events: Vec<FunnelEvent> = (0..10)
            .map(|i| {
                if i < 4 {
                    event("inference_edit", &[("field", "purpose")])
                } else {
                    event("inference_shown", &[("field", "purpose")])
                }
            })
            .collect();
        let rates = override_rates(&events);
        assert_eq!(rates.len(), 1);
        assert!((rates[0].1 - 0.4).abs() < 1e-9);
        assert!(rates[0].2, "40% > 30% flags for retuning");
    }
}
