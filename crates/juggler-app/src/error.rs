//! Typed errors for the application shell.

/// Errors surfaced by app commands. Serialized to the UI as their
/// human-readable message; the UI shows them verbatim next to the failing
/// action.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AppError {
    /// Domain/storage failure.
    #[error("{0}")]
    Core(#[from] juggler_core::CoreError),
    /// Supervision failure.
    #[error("{0}")]
    Supervisor(#[from] juggler_supervisor::SupervisorError),
    /// Adapter failure.
    #[error("{0}")]
    Adapter(#[from] juggler_adapter::AdapterError),
    /// A referenced entity does not exist.
    #[error("{entity} {id} not found")]
    NotFound {
        /// Entity kind.
        entity: &'static str,
        /// Missing identifier.
        id: String,
    },
    /// The requested harness has no registered adapter.
    #[error("no adapter for harness {0}")]
    UnknownHarness(String),
    /// Filesystem/OS failure with context.
    #[error("{context}: {message}")]
    Io {
        /// What was happening.
        context: &'static str,
        /// The failure.
        message: String,
    },
    /// State lock poisoned (a previous panic while holding it).
    #[error("internal state unavailable")]
    StatePoisoned,
}

impl AppError {
    /// Convenience constructor for IO errors with context.
    #[must_use]
    pub fn io(context: &'static str, error: &std::io::Error) -> Self {
        Self::Io {
            context,
            message: error.to_string(),
        }
    }
}

/// Commands hand the message string to the webview.
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Result alias used by all commands.
pub type AppResult<T> = Result<T, AppError>;
