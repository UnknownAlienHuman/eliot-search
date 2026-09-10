//! Exactly-one terminal response per admitted request.
//!
//! Results are already bounded by recipe contracts; the protocol does not
//! truncate or reinterpret coverage. Partial or degraded outcomes keep their
//! own kinds and are never relabeled success.

use crate::error::ProtocolError;
use crate::progress::ProgressState;

/// Closed terminal response class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TerminalKind {
    /// Operation completed its declared work.
    Success,
    /// Operation completed with explicit partial coverage.
    Partial,
    /// Operation was cancelled before success.
    Cancelled,
    /// Operation failed before a verified success postcondition.
    Failed,
    /// A possible mutation requires authoritative readback.
    OutcomeUnknown,
}

/// Emits the single terminal response for one progress counter.
pub fn emit_terminal(
    state: &mut ProgressState,
    terminal: TerminalKind,
) -> Result<(), ProtocolError> {
    state.finish(terminal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROTOCOL_LIMITS;

    #[test]
    fn non_success_terminals_do_not_require_completion() {
        let mut progress = ProgressState::new(4, DEFAULT_PROTOCOL_LIMITS).expect("progress");
        progress.advance(1).expect("advance");
        emit_terminal(&mut progress, TerminalKind::Partial).expect("partial");
        assert_eq!(progress.terminal(), Some(TerminalKind::Partial));
    }

    #[test]
    fn terminal_kinds_are_closed_and_ordered() {
        assert!(TerminalKind::Success < TerminalKind::OutcomeUnknown);
        assert_ne!(TerminalKind::Cancelled, TerminalKind::Failed);
    }
}
