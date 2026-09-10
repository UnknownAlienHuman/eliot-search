//! Closed protocol failure registry.
//!
//! Every failure is a typed variant with a stable machine-readable reason
//! code. Partial or degraded states are never relabeled as success; unknown
//! outcomes keep their own variants.

use core::fmt;

/// Closed protocol failure registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProtocolError {
    /// Complete frame exceeds the configured byte ceiling.
    FrameTooLarge,
    /// Length-prefixed JSON frame is malformed (truncated prefix, declared
    /// length mismatch, non-UTF-8 or non-JSON body).
    InvalidEnvelope,
    /// Version value or range is malformed.
    InvalidVersion,
    /// Client and provider version ranges do not overlap.
    NoCompatibleVersion,
    /// Immediately preceding sequence was repeated.
    DuplicateSequence,
    /// Sequence moved behind accepted history.
    SequenceRegression,
    /// Sequence skipped one or more expected values.
    SequenceGap,
    /// Direction-local sequence space is exhausted.
    SequenceExhausted,
    /// Request identity was already admitted.
    ReplayDetected,
    /// Finite replay ledger is full.
    ReplayCapacityExceeded,
    /// Progress moved backwards.
    ProgressRegression,
    /// Progress exceeded its declared total or configured ceiling.
    ProgressExceededTotal,
    /// More than one terminal response was attempted.
    DuplicateTerminal,
    /// Terminal success contradicts incomplete progress.
    IncompleteTerminalSuccess,
    /// Session transition is invalid.
    InvalidSessionTransition,
    /// Pairing or binding proof is absent or invalid.
    AuthenticationRequired,
    /// A supplied proof did not match the expected keyed proof.
    AuthenticationFailed,
    /// A pairing ceremony failed terminally and cannot continue.
    PairingFailed,
    /// A client pairing proof did not match the expected proof.
    PairingProofInvalid,
    /// A pairing operation was attempted in the wrong ceremony state.
    InvalidPairingTransition,
    /// A session, nonce or challenge value is zero or malformed.
    InvalidNonce,
    /// A binding key is zero, empty or malformed.
    InvalidBindingKey,
    /// A request names a command outside the closed W1 shell registry.
    UnknownCommand,
    /// A response names a status outside the closed status registry.
    InvalidStatus,
    /// An envelope body digest or body binding is invalid.
    InvalidBody,
    /// A finite in-flight, ledger or queue ceiling is exhausted.
    ResourceExhausted,
    /// A relative deadline expired before admission or completion.
    DeadlineExpired,
    /// New requests were attempted after draining began.
    SessionDraining,
    /// Session is closed.
    SessionClosed,
    /// Contradictory protocol state is quarantined.
    Quarantined,
    /// Protocol limits are internally inconsistent.
    InvalidLimits,
}

impl ProtocolError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::FrameTooLarge => "PROTOCOL_FRAME_TOO_LARGE",
            Self::InvalidEnvelope => "PROTOCOL_INVALID_ENVELOPE",
            Self::InvalidVersion => "PROTOCOL_INVALID_VERSION",
            Self::NoCompatibleVersion => "PROTOCOL_NO_COMPATIBLE_VERSION",
            Self::DuplicateSequence => "PROTOCOL_DUPLICATE_SEQUENCE",
            Self::SequenceRegression => "PROTOCOL_SEQUENCE_REGRESSION",
            Self::SequenceGap => "PROTOCOL_SEQUENCE_GAP",
            Self::SequenceExhausted => "PROTOCOL_SEQUENCE_EXHAUSTED",
            Self::ReplayDetected => "PROTOCOL_REPLAY_DETECTED",
            Self::ReplayCapacityExceeded => "PROTOCOL_REPLAY_CAPACITY_EXCEEDED",
            Self::ProgressRegression => "PROTOCOL_PROGRESS_REGRESSION",
            Self::ProgressExceededTotal => "PROTOCOL_PROGRESS_EXCEEDED_TOTAL",
            Self::DuplicateTerminal => "PROTOCOL_DUPLICATE_TERMINAL",
            Self::IncompleteTerminalSuccess => "PROTOCOL_INCOMPLETE_TERMINAL_SUCCESS",
            Self::InvalidSessionTransition => "PROTOCOL_INVALID_SESSION_TRANSITION",
            Self::AuthenticationRequired => "PROTOCOL_AUTHENTICATION_REQUIRED",
            Self::AuthenticationFailed => "PROTOCOL_AUTH_FAILED",
            Self::PairingFailed => "PROTOCOL_PAIRING_FAILED",
            Self::PairingProofInvalid => "PROTOCOL_PAIRING_PROOF_INVALID",
            Self::InvalidPairingTransition => "PROTOCOL_INVALID_PAIRING_TRANSITION",
            Self::InvalidNonce => "PROTOCOL_INVALID_NONCE",
            Self::InvalidBindingKey => "PROTOCOL_INVALID_BINDING_KEY",
            Self::UnknownCommand => "PROTOCOL_UNKNOWN_COMMAND",
            Self::InvalidStatus => "PROTOCOL_INVALID_STATUS",
            Self::InvalidBody => "PROTOCOL_INVALID_BODY",
            Self::ResourceExhausted => "PROTOCOL_RESOURCE_EXHAUSTED",
            Self::DeadlineExpired => "PROTOCOL_DEADLINE_EXPIRED",
            Self::SessionDraining => "PROTOCOL_SESSION_DRAINING",
            Self::SessionClosed => "PROTOCOL_SESSION_CLOSED",
            Self::Quarantined => "PROTOCOL_QUARANTINED",
            Self::InvalidLimits => "PROTOCOL_INVALID_LIMITS",
        }
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_codes_are_stable_and_content_free() {
        assert_eq!(
            ProtocolError::FrameTooLarge.code(),
            "PROTOCOL_FRAME_TOO_LARGE"
        );
        assert_eq!(
            ProtocolError::AuthenticationRequired.code(),
            "PROTOCOL_AUTHENTICATION_REQUIRED"
        );
        assert_eq!(
            ProtocolError::AuthenticationFailed.code(),
            "PROTOCOL_AUTH_FAILED"
        );
        assert_eq!(
            ProtocolError::PairingProofInvalid.code(),
            "PROTOCOL_PAIRING_PROOF_INVALID"
        );
        assert_eq!(
            ProtocolError::ResourceExhausted.code(),
            "PROTOCOL_RESOURCE_EXHAUSTED"
        );
        assert_eq!(
            ProtocolError::DeadlineExpired.code(),
            "PROTOCOL_DEADLINE_EXPIRED"
        );
        // Codes carry no key, nonce or proof material.
        for error in [
            ProtocolError::AuthenticationFailed,
            ProtocolError::PairingProofInvalid,
            ProtocolError::InvalidBindingKey,
            ProtocolError::InvalidNonce,
        ] {
            assert!(!error.code().contains("key"));
            assert!(!format!("{error:?}").contains("0x"));
        }
    }
}
