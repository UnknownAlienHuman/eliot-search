//! Content-free failures for the selected local wire.

/// A bounded frame or closed DTO failed validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    /// Invalid, incomplete or trailing frame bytes.
    InvalidFrame,
    /// Declared or serialized bytes exceed the complete-frame ceiling.
    FrameTooLarge,
    /// Closed JSON schema or a field invariant was rejected.
    InvalidMessage,
    /// The sole request identity does not match.
    InvalidCorrelation,
    /// A bounded allocation could not be reserved.
    ResourceExhausted,
    /// A validated value could not be serialized.
    Serialization,
}

impl ProtocolError {
    /// Stable content-free diagnostic.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidFrame => "QUERY_FRAME_INVALID",
            Self::FrameTooLarge => "QUERY_FRAME_TOO_LARGE",
            Self::InvalidMessage => "QUERY_MESSAGE_INVALID",
            Self::InvalidCorrelation => "QUERY_RESPONSE_MISMATCH",
            Self::ResourceExhausted => "QUERY_ALLOCATION_FAILED",
            Self::Serialization => "QUERY_SERIALIZATION_FAILED",
        }
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for ProtocolError {}
