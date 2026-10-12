//! Environment scanning and config import for smart onboarding (spec §12,
//! ADR-0006).
//!
//! The onboarding wizard assumes the user knows nothing about how harness
//! config directories work: adapters scan the machine for existing
//! installations so the wizard can offer "import what you already have"
//! instead of a blank form.
//!
//! Guardrails (ADR-0003/0006): scanning is **metadata-only** — file
//! presence, sizes, and timestamps, never contents; importing is
//! **read-only toward the source** — a plain byte copy into a fresh managed
//! dir, with the source left untouched and unmarked.

use std::path::PathBuf;

use crate::model::HarnessId;

/// Login evidence for a discovered install, probed without reading
/// credential contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoginProbe {
    /// Credentials are present for this config dir.
    LoggedIn,
    /// No credentials found for this config dir.
    #[default]
    LoggedOut,
}

/// How a discovered install's location came to exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOrigin {
    /// The harness's own default location (e.g. `~/.claude`).
    VendorDefault,
    /// Pointed at by the harness's env override (e.g. `$CLAUDE_CONFIG_DIR`)
    /// — typically another manager tool or a manual setup.
    EnvOverride,
    /// Matched a config-naming pattern in the user's home — created
    /// manually, by another tool, or by an AI assistant.
    HomePattern,
}

impl InstallOrigin {
    /// Wire name for the UI (`vendor-default` / `env-override` /
    /// `home-pattern`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VendorDefault => "vendor-default",
            Self::EnvOverride => "env-override",
            Self::HomePattern => "home-pattern",
        }
    }
}

/// One existing harness installation found on this machine.
#[derive(Debug, Clone)]
pub struct DiscoveredInstall {
    /// Harness this install belongs to.
    pub harness: HarnessId,
    /// Config dir root (e.g. `~/.claude`).
    pub path: PathBuf,
    /// How the location came to exist.
    pub origin: InstallOrigin,
    /// Login evidence for this exact path (metadata-only probe).
    pub login: LoginProbe,
    /// Approximate bytes on disk (bounded walk; caches excluded).
    pub bytes: u64,
    /// Most recent mtime among top-level entries, unix seconds.
    pub last_used_secs: Option<u64>,
    /// True when the dir already carries the AI Juggler management marker
    /// (a previous import or managed setup).
    pub managed: bool,
    /// Human-facing hints the wizard shows verbatim.
    pub notes: Vec<String>,
}

/// Result of importing an existing config dir into a managed account dir.
#[derive(Debug, Clone)]
pub struct ImportOutcome {
    /// Files copied into the managed dir.
    pub files_copied: u64,
    /// Bytes copied into the managed dir.
    pub bytes_copied: u64,
    /// Relative paths skipped as rebuildable cache/noise, with reasons.
    pub skipped: Vec<String>,
    /// Login state of the **destination** after the copy.
    pub login: LoginProbe,
    /// When login did not carry over, the plain-language reason and what
    /// the user should do (e.g. macOS keychain path-locking).
    pub login_note: Option<String>,
}

/// The vendor's own web sign-in page for this harness, when one exists.
/// Opening it is the maximum login aid allowed: the user completes the
/// vendor's flow; AI Juggler never touches credentials.
pub type LoginUrl = &'static str;
