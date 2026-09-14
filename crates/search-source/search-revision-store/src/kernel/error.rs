//! Closed revision-store failure taxonomy and stable reason codes.

use core::fmt;

/// Closed content-free revision-store failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RevisionStoreError {
    /// Store limits are zero or internally inconsistent.
    InvalidLimits,
    /// Plaintext byte count is zero or exceeds its finite ceiling.
    PlaintextSizeInvalid,
    /// Ciphertext is empty or exceeds its finite ceiling.
    CiphertextSizeInvalid,
    /// Nonce is empty or exceeds its finite ceiling.
    NonceInvalid,
    /// Source revision is absent or cannot advance exactly once.
    RevisionSequenceInvalid,
    /// Exact source/revision key already stores different immutable metadata.
    RevisionConflict,
    /// Exact revision is absent.
    RevisionNotFound,
    /// Operation identity was reused with another complete request digest.
    OperationConflict,
    /// Finite revision or operation capacity was exhausted.
    CapacityExceeded,
    /// Write confirmation does not match the prepared exact intent.
    ReadbackMismatch,
    /// Required authorization or durable readback evidence is absent.
    EvidenceMissing,
    /// Possible external write has unknown authoritative outcome.
    OutcomeUnknown,
    /// Contradictory object state requires quarantine.
    Quarantined,
    /// Backend failed before a verified outcome existed.
    BackendFailure,
    /// Backend returned malformed or contradictory data.
    BackendContractViolation,
    /// Shared revision cannot advance.
    ContractExhausted,
    /// Residency closures differ where exact equivalence was required, or
    /// physical bytes/keys were reused across inequivalent closures.
    ResidencyMismatch,
    /// Object address or residency scope identity is malformed or versioned
    /// incorrectly.
    AddressInvalid,
    /// Envelope binding version, length, or key-generation linkage is invalid.
    EnvelopeInvalid,
    /// Canonical T13 ingest binding carries a malformed receipt digest.
    IngestBindingInvalid,
    /// A purge tombstone prohibits this residency scope from entering state.
    Tombstoned,
    /// Deletion plan does not name this exact record and storage object.
    DeletionNotAuthorized,
}

impl RevisionStoreError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "REVISION_STORE_INVALID_LIMITS",
            Self::PlaintextSizeInvalid => "REVISION_STORE_PLAINTEXT_SIZE_INVALID",
            Self::CiphertextSizeInvalid => "REVISION_STORE_CIPHERTEXT_SIZE_INVALID",
            Self::NonceInvalid => "REVISION_STORE_NONCE_INVALID",
            Self::RevisionSequenceInvalid => "REVISION_STORE_REVISION_SEQUENCE_INVALID",
            Self::RevisionConflict => "REVISION_STORE_REVISION_CONFLICT",
            Self::RevisionNotFound => "REVISION_STORE_REVISION_NOT_FOUND",
            Self::OperationConflict => "REVISION_STORE_OPERATION_CONFLICT",
            Self::CapacityExceeded => "REVISION_STORE_CAPACITY_EXCEEDED",
            Self::ReadbackMismatch => "REVISION_STORE_READBACK_MISMATCH",
            Self::EvidenceMissing => "REVISION_STORE_EVIDENCE_MISSING",
            Self::OutcomeUnknown => "REVISION_STORE_OUTCOME_UNKNOWN",
            Self::Quarantined => "REVISION_STORE_QUARANTINED",
            Self::BackendFailure => "REVISION_STORE_BACKEND_FAILURE",
            Self::BackendContractViolation => "REVISION_STORE_BACKEND_CONTRACT_VIOLATION",
            Self::ContractExhausted => "REVISION_STORE_CONTRACT_EXHAUSTED",
            Self::ResidencyMismatch => "REVISION_STORE_RESIDENCY_DOMAIN_MISMATCH",
            Self::AddressInvalid => "REVISION_STORE_OBJECT_ADDRESS_INVALID",
            Self::EnvelopeInvalid => "REVISION_STORE_ENVELOPE_INVALID",
            Self::IngestBindingInvalid => "REVISION_STORE_INGEST_BINDING_INVALID",
            Self::Tombstoned => "REVISION_STORE_PURGE_TOMBSTONE_CONFLICT",
            Self::DeletionNotAuthorized => "REVISION_STORE_OBJECT_DELETE_NOT_AUTHORIZED",
        }
    }
}

impl fmt::Display for RevisionStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RevisionStoreError {}
