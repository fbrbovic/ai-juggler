//! AJ-E309: the first-run policy hook (D-14/D-12). This module owns
//! the CHOICE and its STORAGE — never pack behaviour (the switching
//! epic consumes the policy and must display the destination
//! Provider's active policy before executing).
//!
//! D-14 law: the first run MUST ask; absence of a choice means "ask
//! before first pack" — never a silent default, never a backfill for
//! existing users.

use crate::storage::Storage;

type PolicyResult<T> = Result<T, crate::CoreError>;

/// The global pack secret policy (D-14). Plain-language options; the
/// user chooses or explicitly decides later.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackPolicy {
    /// Secrets stripped from context packs before any use.
    Strip,
    /// Secrets kept, with a visible warning on every use.
    KeepWarn,
    /// Secrets kept silently.
    KeepSilently,
}

impl PackPolicy {
    /// The wire/storage word.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PackPolicy::Strip => "strip",
            PackPolicy::KeepWarn => "keep_warn",
            PackPolicy::KeepSilently => "keep_silently",
        }
    }

    /// Parses the storage word; `None` for anything else (corrupt
    /// values read as "no choice" — the honest fallback).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "strip" => Some(PackPolicy::Strip),
            "keep_warn" => Some(PackPolicy::KeepWarn),
            "keep_silently" => Some(PackPolicy::KeepSilently),
            _ => None,
        }
    }

    /// The Summary restatement copy (one line, consequence stated).
    #[must_use]
    pub fn restatement(self) -> &'static str {
        match self {
            PackPolicy::Strip => "Packs: strip secrets (all Providers)",
            PackPolicy::KeepWarn => "Packs: keep secrets, with a warning each time",
            PackPolicy::KeepSilently => "Packs: keep secrets silently",
        }
    }
}

/// The global policy's settings key (the per-Provider override key is
/// namespaced by provider id).
const POLICY_KEY: &str = "pack_policy:global";

/// Per-Provider override key (`pack_policy:provider:<id>`).
#[must_use]
pub fn provider_policy_key(provider: &str) -> String {
    format!("pack_policy:provider:{provider}")
}

/// Reads the GLOBAL policy. `None` = no choice recorded (the honest
/// "ask before first pack" state — D-14's trap note: never backfill).
///
/// # Errors
///
/// Returns [`crate::CoreError`] on storage failure.
pub fn global_policy(storage: &Storage) -> PolicyResult<Option<PackPolicy>> {
    let raw = storage.app_settings().get(&POLICY_KEY)?;
    Ok(raw.as_deref().and_then(PackPolicy::parse))
}

/// Writes the GLOBAL policy — ONLY on an explicit user click
/// (onboarding never stages a default first).
///
/// # Errors
///
/// Returns [`crate::CoreError`] on storage failure.
pub fn set_global_policy(storage: &Storage, policy: PackPolicy) -> PolicyResult<()> {
    storage.app_settings().set(&POLICY_KEY, policy.as_str())
}

/// Reads a Provider's override; `None` = inherit the global choice.
///
/// # Errors
///
/// Returns [`crate::CoreError`] on storage failure.
pub fn provider_policy(storage: &Storage, provider: &str) -> PolicyResult<Option<PackPolicy>> {
    let raw = storage.app_settings().get(&provider_policy_key(provider))?;
    Ok(raw.as_deref().and_then(PackPolicy::parse))
}

/// Writes (or clears, with `None`) a Provider override.
///
/// # Errors
///
/// Returns [`crate::CoreError`] on storage failure.
pub fn set_provider_policy(
    storage: &Storage,
    provider: &str,
    policy: Option<PackPolicy>,
) -> PolicyResult<()> {
    match policy {
        Some(choice) => storage
            .app_settings()
            .set(&provider_policy_key(provider), choice.as_str()),
        // The settings repo's empty-set IS a delete — the override
        // clears back to inheriting the global choice.
        None => storage
            .app_settings()
            .set(&provider_policy_key(provider), ""),
    }
}

/// The ACTIVE policy for a provider — the override when present,
/// else the global choice, else `None` (ask before first pack).
///
/// # Errors
///
/// Returns [`crate::CoreError`] on storage failure.
pub fn active_policy(storage: &Storage, provider: &str) -> PolicyResult<Option<PackPolicy>> {
    provider_policy(storage, provider)
        .or_else(|error| Err(error))
        .and_then(|override_choice| {
            if override_choice.is_some() {
                Ok(override_choice)
            } else {
                global_policy(storage)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Storage {
        Storage::open_in_memory().unwrap()
    }

    #[test]
    fn no_choice_is_none_never_a_default() {
        // D-14's trap: absence means "ask", never a backfilled strip.
        let storage = store();
        assert_eq!(global_policy(&storage).unwrap(), None);
        assert_eq!(active_policy(&storage, "claude-code").unwrap(), None);
    }

    #[test]
    fn global_roundtrip_and_restatement() {
        let storage = store();
        set_global_policy(&storage, PackPolicy::KeepWarn).unwrap();
        assert_eq!(global_policy(&storage).unwrap(), Some(PackPolicy::KeepWarn));
        assert_eq!(
            active_policy(&storage, "claude-code").unwrap(),
            Some(PackPolicy::KeepWarn)
        );
        assert_eq!(
            PackPolicy::KeepWarn.restatement(),
            "Packs: keep secrets, with a warning each time"
        );
    }

    #[test]
    fn provider_override_wins_and_clearing_inherits() {
        let storage = store();
        set_global_policy(&storage, PackPolicy::Strip).unwrap();
        set_provider_policy(&storage, "claude-code", Some(PackPolicy::KeepSilently)).unwrap();
        assert_eq!(
            active_policy(&storage, "claude-code").unwrap(),
            Some(PackPolicy::KeepSilently)
        );
        assert_eq!(
            active_policy(&storage, "codex").unwrap(),
            Some(PackPolicy::Strip)
        );
        set_provider_policy(&storage, "claude-code", None).unwrap();
        assert_eq!(
            active_policy(&storage, "claude-code").unwrap(),
            Some(PackPolicy::Strip)
        );
    }

    #[test]
    fn corrupt_values_read_as_no_choice() {
        let storage = store();
        storage
            .app_settings()
            .set(&POLICY_KEY, "yes please")
            .unwrap();
        assert_eq!(global_policy(&storage).unwrap(), None);
    }
}
