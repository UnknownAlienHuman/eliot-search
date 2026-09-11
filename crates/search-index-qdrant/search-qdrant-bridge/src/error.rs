use core::fmt;

/// Closed Qdrant bridge failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BridgeError {
    EndpointNotLoopback,
    AuthenticationInvalid,
    SupervisorReceiptMismatch,
    CapabilityProbeFailed,
    CapabilityReceiptMismatch,
    CollectionAlreadyExists,
    CollectionNotFound,
    CollectionSchemaMismatch,
    PayloadIndexMissing,
    NamedVectorMissing,
    VectorDimensionMismatch,
    StrictModeRequired,
    MutationTooLarge,
    DuplicatePointId,
    PointNotFound,
    OperationConflict,
    MutationOutcomeUnknown,
    ExactReadbackMismatch,
    UnexpectedPoint,
    InvalidFilter,
    UnindexedFilter,
    QueryBudgetExceeded,
    InvalidScore,
    /// The operation was cancelled before dispatch or between bounded pages.
    Cancelled,
    /// The transport failed without a possible external write (reads and
    /// pre-send connect failures). Mutations that may have committed after
    /// dispatch report [`BridgeError::MutationOutcomeUnknown`] instead.
    TransportFailed,
    /// The server returned a response that does not match the exact expected
    /// shape (missing identity, payload or vector fields, oversize page).
    MalformedResponse,
}

impl BridgeError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EndpointNotLoopback => "QDRANT_ENDPOINT_NOT_LOOPBACK",
            Self::AuthenticationInvalid => "QDRANT_AUTHENTICATION_INVALID",
            Self::SupervisorReceiptMismatch => "QDRANT_SUPERVISOR_RECEIPT_MISMATCH",
            Self::CapabilityProbeFailed => "QDRANT_CAPABILITY_PROBE_FAILED",
            Self::CapabilityReceiptMismatch => "QDRANT_CAPABILITY_RECEIPT_MISMATCH",
            Self::CollectionAlreadyExists => "QDRANT_COLLECTION_ALREADY_EXISTS",
            Self::CollectionNotFound => "QDRANT_COLLECTION_NOT_FOUND",
            Self::CollectionSchemaMismatch => "QDRANT_COLLECTION_SCHEMA_MISMATCH",
            Self::PayloadIndexMissing => "QDRANT_PAYLOAD_INDEX_MISSING",
            Self::NamedVectorMissing => "QDRANT_NAMED_VECTOR_MISSING",
            Self::VectorDimensionMismatch => "QDRANT_VECTOR_DIMENSION_MISMATCH",
            Self::StrictModeRequired => "QDRANT_STRICT_MODE_REQUIRED",
            Self::MutationTooLarge => "QDRANT_MUTATION_TOO_LARGE",
            Self::DuplicatePointId => "QDRANT_DUPLICATE_POINT_ID",
            Self::PointNotFound => "QDRANT_POINT_NOT_FOUND",
            Self::OperationConflict => "QDRANT_OPERATION_CONFLICT",
            Self::MutationOutcomeUnknown => "QDRANT_MUTATION_OUTCOME_UNKNOWN",
            Self::ExactReadbackMismatch => "QDRANT_EXACT_READBACK_MISMATCH",
            Self::UnexpectedPoint => "QDRANT_UNEXPECTED_POINT",
            Self::InvalidFilter => "QDRANT_INVALID_FILTER",
            Self::UnindexedFilter => "QDRANT_UNINDEXED_FILTER",
            Self::QueryBudgetExceeded => "QDRANT_QUERY_BUDGET_EXCEEDED",
            Self::InvalidScore => "QDRANT_INVALID_SCORE",
            Self::Cancelled => "QDRANT_OPERATION_CANCELLED",
            Self::TransportFailed => "QDRANT_TRANSPORT_FAILED",
            Self::MalformedResponse => "QDRANT_MALFORMED_RESPONSE",
        }
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for BridgeError {}
