//! Typed errors for the supervisor layer.

use std::path::PathBuf;

use juggler_core::model::SessionId;

/// Errors surfaced while supervising harness sessions.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SupervisorError {
    /// Spawning the harness process failed.
    #[error("spawn failed for {program}: {message}")]
    Spawn {
        /// The program that was supposed to start.
        program: PathBuf,
        /// What went wrong.
        message: String,
    },
    /// The session id is not currently supervised.
    #[error("session {0} is not supervised")]
    UnknownSession(SessionId),
    /// The operation needs a PTY session but this one runs piped.
    #[error("session {session} has no pty (piped/headless session)")]
    NotATty {
        /// The session without a terminal.
        session: SessionId,
    },
    /// The supervision journal could not be read or written.
    #[error("journal error at {path}: {message}")]
    Journal {
        /// Journal file path.
        path: PathBuf,
        /// What went wrong.
        message: String,
    },
    /// An I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
