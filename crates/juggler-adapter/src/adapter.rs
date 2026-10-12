//! The `HarnessAdapter` contract (spec §4, ADR-0002).

use std::path::Path;
use std::sync::Arc;

use crate::capabilities::Capabilities;
use crate::discovery::DiscoveryReport;
use crate::environment::{DiscoveredInstall, ImportOutcome, LoginUrl};
use crate::error::AdapterError;
use crate::launch::{EnvMap, LaunchMode, LaunchSpec};
use crate::model::{Account, HarnessId, Profile};
use crate::watcher::SessionWatcher;

/// The only place harness specifics are allowed to live.
///
/// Implementations:
/// - are **read-only** toward harness state except inside
///   [`HarnessAdapter::prepare_account`], the single writer to a managed
///   account directory;
/// - declare `verified_against` versions and capability-probe at runtime —
///   never assume unadvertised capabilities;
/// - never touch vendor credentials' contents.
///
/// The trait is public, semver-protected API (guarded by
/// `cargo-semver-checks` in CI once the first stable release ships).
pub trait HarnessAdapter: Send + Sync {
    /// The harness this adapter integrates, e.g. `claude-code`.
    fn id(&self) -> HarnessId;

    /// What this harness integration supports.
    fn capabilities(&self) -> Capabilities;

    /// Harness versions this adapter was validated against (spec §4
    /// version-pinning rule). Discovery compares against this list.
    fn verified_against(&self) -> &'static [&'static str];

    /// Probes the machine: is the harness installed, at what version, and is
    /// that version verified? Never fails — problems are carried in the
    /// report.
    #[must_use]
    fn discover(&self) -> DiscoveryReport;

    /// Prepares (idempotently) an isolated account state directory and
    /// returns the environment overlay that points the harness at it.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::AccountDir`] when the directory cannot be
    /// created or initialized.
    fn prepare_account(&self, state_dir: &Path) -> Result<EnvMap, AdapterError>;

    /// Builds the launch specification for one session. The account carries
    /// the managed state directory the adapter itself prepared earlier via
    /// [`HarnessAdapter::prepare_account`].
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::InvalidLaunch`] for unsupported
    /// profile/mode combinations.
    fn launch_spec(
        &self,
        account: &Account,
        profile: &Profile,
        cwd: &Path,
        mode: &LaunchMode,
    ) -> Result<LaunchSpec, AdapterError>;

    /// Creates a watcher over the account's transcript root.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Watch`] when the root cannot be observed.
    fn watcher(&self, transcript_root: &Path) -> Result<Box<dyn SessionWatcher>, AdapterError>;

    /// Scans the machine for existing installations of this harness
    /// (smart onboarding, spec §12). Metadata-only: presence, size,
    /// timestamps — never file contents. Adapters that cannot scan return
    /// an empty list; scanning never fails.
    #[must_use]
    fn scan_environment(&self) -> Vec<DiscoveredInstall> {
        Vec::new()
    }

    /// Copy-imports an existing config dir (found via
    /// [`HarnessAdapter::scan_environment`]) into a fresh managed state
    /// dir. Read-only toward `source`: a byte copy with rebuildable caches
    /// skipped; the source is never written to or marked.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Unsupported`] when this adapter cannot
    /// import, [`AdapterError::AccountDir`] for path/size problems and
    /// copy failures (message carries the plain-language reason).
    fn import_account(&self, source: &Path, dest: &Path) -> Result<ImportOutcome, AdapterError> {
        let _ = (source, dest);
        Err(AdapterError::Unsupported(
            "this adapter cannot import existing configurations".into(),
        ))
    }

    /// The vendor's own web sign-in page, when one exists (the maximum
    /// login aid: we open the page, the user completes the vendor's flow).
    #[must_use]
    fn login_url(&self) -> Option<LoginUrl> {
        None
    }

    /// Builds the launch spec for the vendor's own interactive login flow
    /// against a config dir — the guided-terminal login primitive (spec
    /// §4.1): AI Juggler spawns it in an embedded terminal; the user
    /// completes the OAuth flow; nothing is automated.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Unsupported`] when the harness has no
    /// terminal login flow, [`AdapterError::InvalidLaunch`] when the
    /// binary cannot be resolved.
    fn login_spec(&self, state_dir: &Path) -> Result<LaunchSpec, AdapterError> {
        let _ = state_dir;
        Err(AdapterError::Unsupported(
            "this harness has no terminal login flow".into(),
        ))
    }
}

/// Shared adapter handle type used by the registry and supervisor.
pub type SharedAdapter = Arc<dyn HarnessAdapter>;
