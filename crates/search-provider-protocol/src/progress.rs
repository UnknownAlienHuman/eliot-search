//! Monotone progress counters: counts, phases and reasons only.
//!
//! Progress carries no source or query content and is never terminal; the
//! exactly-one terminal response lives in [`crate::terminal`].

use crate::config::ProtocolLimits;
use crate::error::ProtocolError;

/// Monotone progress towards a declared finite total.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressState {
    total: u64,
    completed: u64,
    terminal: Option<crate::terminal::TerminalKind>,
}

impl ProgressState {
    /// Creates a finite progress counter within limits.
    pub fn new(total: u64, limits: ProtocolLimits) -> Result<Self, ProtocolError> {
        let limits = limits.validate()?;
        if total > limits.max_progress_total {
            return Err(ProtocolError::ProgressExceededTotal);
        }
        Ok(Self {
            total,
            completed: 0,
            terminal: None,
        })
    }

    /// Declared total work units.
    pub const fn total(self) -> u64 {
        self.total
    }

    /// Monotone completed work units.
    pub const fn completed(self) -> u64 {
        self.completed
    }

    /// Terminal response when already emitted.
    pub const fn terminal(self) -> Option<crate::terminal::TerminalKind> {
        self.terminal
    }

    /// Advances progress monotonically and within its denominator.
    pub fn advance(&mut self, completed: u64) -> Result<(), ProtocolError> {
        if self.terminal.is_some() {
            return Err(ProtocolError::DuplicateTerminal);
        }
        if completed < self.completed {
            return Err(ProtocolError::ProgressRegression);
        }
        if completed > self.total {
            return Err(ProtocolError::ProgressExceededTotal);
        }
        self.completed = completed;
        Ok(())
    }

    /// Records exactly one terminal response.
    pub fn finish(&mut self, terminal: crate::terminal::TerminalKind) -> Result<(), ProtocolError> {
        if self.terminal.is_some() {
            return Err(ProtocolError::DuplicateTerminal);
        }
        if terminal == crate::terminal::TerminalKind::Success && self.completed != self.total {
            return Err(ProtocolError::IncompleteTerminalSuccess);
        }
        self.terminal = Some(terminal);
        Ok(())
    }
}

/// Advances monotone progress for one admitted request guard.
pub fn emit_progress(state: &mut ProgressState, completed: u64) -> Result<(), ProtocolError> {
    state.advance(completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROTOCOL_LIMITS;
    use crate::terminal::TerminalKind;

    #[test]
    fn progress_rejects_regression_and_overflow() {
        let mut progress = ProgressState::new(2, DEFAULT_PROTOCOL_LIMITS).expect("progress");
        progress.advance(1).expect("advance");
        assert_eq!(progress.advance(0), Err(ProtocolError::ProgressRegression));
        assert_eq!(
            progress.advance(3),
            Err(ProtocolError::ProgressExceededTotal)
        );
        assert_eq!(
            ProgressState::new(u64::MAX, DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::ProgressExceededTotal)
        );
    }

    #[test]
    fn success_requires_complete_progress_and_terminal_is_unique() {
        let mut progress = ProgressState::new(2, DEFAULT_PROTOCOL_LIMITS).expect("progress");
        progress.advance(1).expect("advance");
        assert_eq!(
            progress.finish(TerminalKind::Success),
            Err(ProtocolError::IncompleteTerminalSuccess)
        );
        progress.advance(2).expect("complete");
        emit_progress(
            &mut ProgressState::new(2, DEFAULT_PROTOCOL_LIMITS).expect("p"),
            2,
        )
        .expect("emit");
        progress.finish(TerminalKind::Success).expect("finish");
        assert_eq!(
            progress.finish(TerminalKind::Failed),
            Err(ProtocolError::DuplicateTerminal)
        );
    }
}
