//! Domain model (spec §3).
//!
//! These types are the vocabulary of the whole system; every layer above
//! storage speaks them. They are deliberately I/O-free and independent of any
//! specific harness.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

macro_rules! id_newtype {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Arc<str>);

        impl $name {
            /// Creates the identifier from any string-like value.
            #[must_use]
            pub fn new(value: impl Into<Arc<str>>) -> Self {
                Self(value.into())
            }

            /// Views the identifier as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.as_str())
            }
        }
    };
}

id_newtype! {
    /// Identifies a harness kind, e.g. `claude-code`, `codex`, `zcode`.
    HarnessId
}

/// ADR-0008 terminology: `Provider` is the user-facing noun for a tool
/// family. Additive alias — same type, same wire values, no migration.
pub type ProviderId = HarnessId;

impl HarnessId {
    /// Alias constructor matching the ADR-0008 vocabulary.
    #[must_use]
    pub fn provider(value: impl Into<std::sync::Arc<str>>) -> Self {
        Self::new(value)
    }
}
id_newtype! {
    /// Identifies a managed account (harness login) owned by the user.
    AccountId
}
id_newtype! {
    /// Identifies a launch profile (account + settings overlay).
    ProfileId
}
id_newtype! {
    /// Identifies a supervised session, as tracked by AI Juggler.
    SessionId
}
id_newtype! {
    /// Identifies a machine in the fleet (`docs/SPEC.md` §11); the local
    /// machine uses a stable default id.
    MachineId
}
id_newtype! {
    /// Identifies a tracked local project (directory) — the juggling
    /// anchor (ADR-0007).
    ProjectId
}
id_newtype! {
    /// The *native* conversation identifier inside an Instance
    /// The native conversation id of a Provider session. See amendment A-1.
    ConversationId
}

/// Unix epoch milliseconds — the crate's canonical timestamp representation.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Current wall-clock time; falls back to the epoch if the system clock
    /// is before 1970, and saturates if it is absurdly far in the future.
    #[must_use]
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        Self(millis)
    }

    /// Creates a timestamp from explicit epoch milliseconds.
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// Returns epoch milliseconds.
    #[must_use]
    pub const fn millis(self) -> i64 {
        self.0
    }
}

/// A vendor plan tier label, e.g. `max-20x`, `plus`, `glm-pro`.
///
/// Deliberately an opaque string: tiers are vendor-specific and churn often
/// (see the research notes); the usage engine interprets them per harness.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlanTier(String);

impl PlanTier {
    /// Creates a plan tier label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self(label.into())
    }

    /// Views the label as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlanTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A harness login the user owns, materialized as an isolated state directory
/// managed by AI Juggler (`docs/SPEC.md` §3).
#[derive(Clone, PartialEq, Debug)]
pub struct Account {
    /// Stable identifier.
    pub id: AccountId,
    /// Which harness this account logs into.
    pub harness: HarnessId,
    /// Human label, e.g. `Work Max`.
    pub label: String,
    /// Vendor plan tier, if known.
    pub plan: Option<PlanTier>,
    /// The isolated harness state directory AI Juggler manages for this
    /// account (`CLAUDE_CONFIG_DIR` / `CODEX_HOME` / `HOME` root).
    pub state_dir: PathBuf,
}

/// Settings applied on top of an account's defaults when launching a profile.
#[derive(Clone, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SettingsOverlay {
    /// Preferred model identifier, harness-interpreted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Extra environment variables injected into the launched process.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

/// A launchable configuration: account plus a settings overlay
/// (`docs/SPEC.md` §3).
#[derive(Clone, PartialEq, Debug)]
pub struct Profile {
    /// Stable identifier.
    pub id: ProfileId,
    /// Human name for the profile (e.g. `Opus pairing`).
    pub name: String,
    /// The account this profile launches under.
    pub account: AccountId,
    /// Settings layered over the account defaults.
    pub settings_overlay: SettingsOverlay,
    /// Working directory used when the user does not pick one.
    pub default_cwd: Option<PathBuf>,
}

/// Lifecycle state of a supervised session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionStatus {
    /// Process is starting up.
    Starting,
    /// Healthy and doing work.
    Running,
    /// Alive but blocked on a user decision (e.g. permission prompt).
    WaitingUser,
    /// Suspected hung (heuristic trip; `docs/SPEC.md` §5.1).
    Stuck,
    /// Exited after hitting an account quota wall.
    QuotaBlocked,
    /// Completed normally.
    Finished,
    /// Stopped by the user (our stop ladder) — not a failure.
    Stopped,
    /// Exited abnormally.
    Failed,
    /// Work continued elsewhere via handover; resumable.
    HandedOver,
}

impl SessionStatus {
    /// All variants in canonical order.
    #[must_use]
    pub const fn all() -> [SessionStatus; 9] {
        [
            SessionStatus::Starting,
            SessionStatus::Running,
            SessionStatus::WaitingUser,
            SessionStatus::Stuck,
            SessionStatus::QuotaBlocked,
            SessionStatus::Finished,
            SessionStatus::Stopped,
            SessionStatus::Failed,
            SessionStatus::HandedOver,
        ]
    }

    /// Stable machine-readable name (storage format).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            SessionStatus::Starting => "starting",
            SessionStatus::Running => "running",
            SessionStatus::WaitingUser => "waiting_user",
            SessionStatus::Stuck => "stuck",
            SessionStatus::QuotaBlocked => "quota_blocked",
            SessionStatus::Finished => "finished",
            SessionStatus::Stopped => "stopped",
            SessionStatus::Failed => "failed",
            SessionStatus::HandedOver => "handed_over",
        }
    }

    /// Parses a stored status name; returns `None` for unknown values
    /// (unknown statuses must degrade, never crash — QUALITY.md §1).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "starting" => Some(SessionStatus::Starting),
            "running" => Some(SessionStatus::Running),
            "waiting_user" => Some(SessionStatus::WaitingUser),
            "stuck" => Some(SessionStatus::Stuck),
            "quota_blocked" => Some(SessionStatus::QuotaBlocked),
            "finished" => Some(SessionStatus::Finished),
            "stopped" => Some(SessionStatus::Stopped),
            "failed" => Some(SessionStatus::Failed),
            "handed_over" => Some(SessionStatus::HandedOver),
            _ => None,
        }
    }
}

/// Token counters for a session snapshot (`docs/SPEC.md` §6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TokenCounts {
    /// Prompt tokens consumed.
    pub input: u64,
    /// Completion tokens consumed (thinking included, billed as output).
    pub output: u64,
    /// Tokens served from prompt cache.
    pub cache_read: u64,
    /// Tokens written to the prompt cache.
    pub cache_write: u64,
    /// Reasoning tokens (subset semantics per harness; kept separate for
    /// analytics).
    pub reasoning: u64,
}

/// A vendor quota window state at a point in time (5-hour or weekly pool).
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuotaWindow {
    /// Percent of the window's allowance already used (0–100).
    pub used_percentage: f64,
    /// When the window resets, if known.
    pub resets_at: Option<Timestamp>,
}

/// How trustworthy a usage number is (`docs/SPEC.md` §6 honesty rule).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Confidence {
    /// Observed directly from a vendor-provided feed.
    Measured,
    /// Deterministically computed from local data and published formulas.
    Computed,
    /// Inferred from token activity; approximate by nature. The default:
    /// never overstate trust.
    #[default]
    Estimated,
}

/// Usage state attached to a session record.
#[derive(Clone, PartialEq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct UsageSnapshot {
    /// Token counters.
    pub tokens: TokenCounts,
    /// Known quota window state, if any feed exists for the harness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<QuotaWindow>,
    /// Trust level of `window`, if present.
    pub confidence: Confidence,
}

/// Points at a harness-native transcript on disk (owned by the harness, read
/// only by AI Juggler). The owning harness is implied by the profile's
/// account.
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct TranscriptRef {
    /// Absolute path to the native transcript/rollout file.
    pub path: PathBuf,
    /// The harness's own session identifier, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
}

/// What kind of harness occurrence an instance is (ADR-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstanceKind {
    /// A CLI harness running in an embedded terminal under an isolated
    /// environment.
    #[default]
    Terminal,
    /// A desktop-class harness running as a windowed application
    /// (supervised process, no terminal surface).
    Desktop,
}

impl InstanceKind {
    /// Wire name (`terminal` / `desktop`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Desktop => "desktop",
        }
    }

    /// Parses a stored wire name; unknown values yield `None` (degrade,
    /// never panic).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "terminal" => Some(Self::Terminal),
            "desktop" => Some(Self::Desktop),
            _ => None,
        }
    }
}

/// Imported history enrichment for a project (parsed from transcripts that
/// predate AI Juggler — e.g. t3.codes profiles).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ProjectHistory {
    /// Account labels seen working in this project before AI Juggler.
    pub accounts: Vec<String>,
    /// Native sessions discovered in imported transcripts.
    pub native_sessions: u64,
}

/// A tracked local project — the juggling anchor (ADR-0007): the
/// directory instances work in, keyed by canonical root so account
/// switches and handovers have a stable pivot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRecord {
    /// Stable identifier assigned by AI Juggler.
    pub id: ProjectId,
    /// Canonical absolute root directory.
    pub root: PathBuf,
    /// Display name (directory name unless the user renames).
    pub name: String,
    /// When the project was first seen.
    pub created_at: Timestamp,
    /// Last time an instance ran in this project.
    pub last_activity: Option<Timestamp>,
    /// Pre-AI-Juggler history parsed from imported transcripts (v4).
    pub history: ProjectHistory,
}

/// ADR-0008 alias: wire strings unchanged; naming only.
pub type InstanceStatus = SessionStatus;

/// A supervised Instance as tracked by AI Juggler (`docs/SPEC.md` §3,
/// ADR-0007/0008). The conversation lives *inside*: see `conversation`.
#[derive(Clone, PartialEq, Debug)]
pub struct InstanceRecord {
    /// Stable identifier assigned by AI Juggler (the *Instance* id;
    /// supervisor/journal key — ADR-0008 A-1).
    pub id: SessionId,
    /// The native conversation inside this Instance, once discovered
    /// (None while Starting — never a fabricated id).
    pub conversation: Option<ConversationId>,
    /// Profile the session runs under.
    pub profile: ProfileId,
    /// Machine the session runs on (fleet dimension; local default
    /// otherwise).
    pub machine: MachineId,
    /// The kind of harness occurrence (terminal CLI vs desktop window).
    pub kind: InstanceKind,
    /// The tracked project this session's cwd belongs to (juggling
    /// anchor, ADR-0007).
    pub project: Option<ProjectId>,
    /// Working directory of the session.
    pub cwd: PathBuf,
    /// Current lifecycle status.
    pub status: SessionStatus,
    /// When the session started.
    pub started_at: Timestamp,
    /// Last observed activity, if any.
    pub last_activity: Option<Timestamp>,
    /// Latest usage snapshot.
    pub usage: UsageSnapshot,
    /// Where the native transcript lives, once discovered.
    pub transcript_ref: Option<TranscriptRef>,
}

/// ADR-0008: old name kept one minor version as a pure alias.
pub type SessionRecord = InstanceRecord;

impl InstanceRecord {
    /// Launch-settings accessor (field rename lands with AJ-E105).
    #[must_use]
    pub fn launch_settings(&self) -> &ProfileId {
        &self.profile
    }
}
