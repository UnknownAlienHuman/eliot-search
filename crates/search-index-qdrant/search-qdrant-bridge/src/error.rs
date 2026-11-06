//! Content-free boundary failures.

use std::fmt;

/// A definite refusal/read failure or an unresolved dispatched mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendError {
    /// The explicit endpoint is not an allowed loopback gRPC URL.
    InvalidEndpoint,
    /// Execution limits are zero or exceed the public ceilings.
    InvalidLimits,
    /// A collection, batch, identifier, query or credential is invalid.
    InvalidInput,
    /// The complete SDK request exceeds its byte/count allowance.
    RequestTooLarge,
    /// A decoded response exceeds its selected point/payload allowance.
    ResponseTooLarge,
    /// A bounded local allocation failed.
    AllocationFailed,
    /// The server refused the supplied authentication.
    Authentication,
    /// The observed server version is not the selected exact patch.
    UnsupportedServer,
    /// The collection or text index differs from the fixed physical profile.
    SchemaMismatch,
    /// Actual points differ from the exact fixed upload batch.
    PointMismatch,
    /// A response lacks required fields or has invalid routing data.
    MalformedResponse,
    /// Cancellation was observed before dispatch, or during a read.
    Cancelled,
    /// The absolute deadline expired before dispatch, or during a read.
    Deadline,
    /// A read or connection transport failed.
    Transport,
    /// The server returned a definite failure for a read.
    BackendRefused,
    /// A mutation or failed composite operation cannot be settled as no-effect.
    /// This includes any failure after successful collection creation.
    UnknownDispatch,
}

impl BackendError {
    /// Stable machine code without URL, credentials, query or SDK status text.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidEndpoint => "BACKEND_ENDPOINT_INVALID",
            Self::InvalidLimits => "BACKEND_LIMITS_INVALID",
            Self::InvalidInput => "BACKEND_INPUT_INVALID",
            Self::RequestTooLarge => "BACKEND_REQUEST_TOO_LARGE",
            Self::ResponseTooLarge => "BACKEND_RESPONSE_TOO_LARGE",
            Self::AllocationFailed => "BACKEND_ALLOCATION_FAILED",
            Self::Authentication => "BACKEND_AUTHENTICATION_FAILED",
            Self::UnsupportedServer => "BACKEND_VERSION_UNSUPPORTED",
            Self::SchemaMismatch => "BACKEND_SCHEMA_MISMATCH",
            Self::PointMismatch => "BACKEND_POINT_MISMATCH",
            Self::MalformedResponse => "BACKEND_RESPONSE_MALFORMED",
            Self::Cancelled => "BACKEND_CANCELLED",
            Self::Deadline => "BACKEND_DEADLINE",
            Self::Transport => "BACKEND_TRANSPORT_FAILED",
            Self::BackendRefused => "BACKEND_REFUSED",
            Self::UnknownDispatch => "BACKEND_UNKNOWN_DISPATCH",
        }
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for BackendError {}
