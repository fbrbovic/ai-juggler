//! Lifecycle events observed by the supervisor (spec §5.1).

use juggler_core::model::SessionId;

/// How a supervised process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    /// Exited with a status code.
    Code(i32),
    /// Terminated by a signal; carries the signal number when known.
    Signaled(Option<i32>),
}

impl ExitKind {
    /// True only for a clean zero exit.
    #[must_use]
    pub fn is_success(self) -> bool {
        matches!(self, ExitKind::Code(0))
    }
}

/// Events emitted by the supervisor for every supervised session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// The process was spawned and is running under this supervisor.
    Started {
        /// Which session.
        session: SessionId,
        /// OS process id.
        pid: u32,
    },
    /// The process ended (normal exit, crash, or stop ladder).
    Exited {
        /// Which session.
        session: SessionId,
        /// OS process id.
        pid: u32,
        /// How it ended.
        kind: ExitKind,
    },
}

impl LifecycleEvent {
    /// The session this event belongs to.
    #[must_use]
    pub fn session(&self) -> &SessionId {
        match self {
            LifecycleEvent::Started { session, .. } | LifecycleEvent::Exited { session, .. } => {
                session
            }
        }
    }
}
