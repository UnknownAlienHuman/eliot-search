//! Child command and terminal-outcome mapping.

use search_provider_protocol::request::{ControlCommand, RequestStatus};
use search_provider_protocol::TerminalKind;

/// Terminal child-reply class observed by the proxy exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildReply {
    /// Terminal frame fully consumed and forwarded.
    Complete,
    /// Ordinary rejection frame fully consumed and forwarded.
    Rejected,
    /// Shutdown terminal consumed; the child exited cleanly.
    Shutdown,
    /// Fatal service frame (possible effects without a success receipt).
    Fatal,
}

/// Maps a consumed child reply to its response status and terminal kind.
///
/// Fatal frames map to `outcome_unknown`: a possible mutation after dispatch
/// is never relabeled success or ordinary failure.
#[must_use]
pub const fn status_for_reply(reply: ChildReply) -> (RequestStatus, TerminalKind) {
    match reply {
        ChildReply::Complete | ChildReply::Shutdown => (RequestStatus::Ok, TerminalKind::Success),
        ChildReply::Rejected => (RequestStatus::Failed, TerminalKind::Failed),
        ChildReply::Fatal => (RequestStatus::OutcomeUnknown, TerminalKind::OutcomeUnknown),
    }
}

/// Maps an envelope command to the child tab command it dispatches.
#[must_use]
pub const fn child_command_for_envelope(command: ControlCommand) -> &'static str {
    match command {
        ControlCommand::Health => "health",
        ControlCommand::Version => "version",
        ControlCommand::Shutdown => "shutdown",
    }
}
