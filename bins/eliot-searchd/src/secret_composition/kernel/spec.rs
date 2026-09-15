//! Closed pairing-secret constants and stable failure vocabulary.

use search_contracts::ProtocolVersion;
use search_os_secrets::{DEFAULT_SECRET_LIMITS, SecretError, SecretLimits};

/// Closed purpose bound into every loopback-pairing reference.
pub const PAIRING_SECRET_PURPOSE: &str = "secret-purpose:loopback-pairing";
/// Exact pairing-key size in bytes; anything else fails closed.
pub const PAIRING_KEY_BYTES: usize = 32;
/// Negotiated loopback-pairing protocol version bound into every transcript.
pub const PAIRING_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
/// Fixed role bound into the binding digest; substitution changes the digest.
pub const PAIRING_ROLE: &str = "loopback-operator";
/// Default finite lease lifetime in monotonic ticks (5 minutes at 1 tick/ms).
pub const DEFAULT_PAIRING_LEASE_TTL_TICKS: u64 = 300_000;
/// Maximum vault-blob bytes accepted into the catalog.
pub const MAX_VAULT_BLOB_BYTES: usize = 256;
/// Bounded recovery attempts after an ambiguous platform mutation.
pub const MAX_RECOVERY_ATTEMPTS: u8 = 3;

pub(super) const BINDING_DOMAIN: &[u8] = b"eliot-search/loopback-binding/v1\0";
pub(super) const OPERATION_DIGEST_DOMAIN: &[u8] = b"eliot-search/loopback-operation/v1\0";
pub(super) const TEST_RNG_DOMAIN: &[u8] = b"eliot-search/loopback-test-rng/v1\0";

/// Canonical finite secret limits used by the default composer.
pub(super) const DEFAULT_LIMITS: SecretLimits = DEFAULT_SECRET_LIMITS;

/// Closed pairing-secret composition failure with a stable machine code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretCompositionError {
    /// No pairing reference is provisioned.
    NotFound,
    /// Installation, incarnation, user-scope or purpose binding differs.
    BindingMismatch,
    /// Reference is not active and cannot be leased.
    NotLeaseable,
    /// Lease window is expired or otherwise unusable now.
    LeaseExpired,
    /// Key material is not exactly 32 bytes or is all zero.
    InvalidKeyMaterial,
    /// Requested lease lifetime is zero or overflows.
    InvalidTtl,
    /// No OS vault is available on this platform/configuration.
    VaultUnavailable,
    /// Durable vault write failed without a verifiable outcome.
    VaultWriteFailed,
    /// Durable vault read failed.
    VaultReadFailed,
    /// Expected vault evidence is absent while the record needs it.
    EvidenceMissing,
    /// Durable write/readback differs from the prepared mutation.
    ReadbackMismatch,
    /// A possible external mutation requires exact recovery.
    OutcomeUnknown,
    /// Requested lifecycle transition is invalid.
    InvalidTransition,
    /// Finite catalog or operation capacity was exhausted.
    CapacityExceeded,
    /// A shared version or revision cannot advance.
    ContractExhausted,
    /// Contradictory state requires quarantine.
    Quarantined,
    /// Ceremony material derivation produced a malformed value.
    EntropyInvalid,
}

impl SecretCompositionError {
    /// Stable machine-readable reason code; carries no secret material.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotFound => "PAIRING_SECRET_NOT_FOUND",
            Self::BindingMismatch => "PAIRING_SECRET_BINDING_MISMATCH",
            Self::NotLeaseable => "PAIRING_SECRET_NOT_LEASEABLE",
            Self::LeaseExpired => "PAIRING_SECRET_LEASE_EXPIRED",
            Self::InvalidKeyMaterial => "PAIRING_SECRET_KEY_INVALID",
            Self::InvalidTtl => "PAIRING_SECRET_TTL_INVALID",
            Self::VaultUnavailable => "PAIRING_SECRET_VAULT_UNAVAILABLE",
            Self::VaultWriteFailed => "PAIRING_SECRET_VAULT_WRITE_FAILED",
            Self::VaultReadFailed => "PAIRING_SECRET_VAULT_READ_FAILED",
            Self::EvidenceMissing => "PAIRING_SECRET_EVIDENCE_MISSING",
            Self::ReadbackMismatch => "PAIRING_SECRET_READBACK_MISMATCH",
            Self::OutcomeUnknown => "PAIRING_SECRET_OUTCOME_UNKNOWN",
            Self::InvalidTransition => "PAIRING_SECRET_INVALID_TRANSITION",
            Self::CapacityExceeded => "PAIRING_SECRET_CAPACITY_EXCEEDED",
            Self::ContractExhausted => "PAIRING_SECRET_CONTRACT_EXHAUSTED",
            Self::Quarantined => "PAIRING_SECRET_QUARANTINED",
            Self::EntropyInvalid => "PAIRING_SECRET_ENTROPY_INVALID",
        }
    }
}

impl core::fmt::Display for SecretCompositionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SecretCompositionError {}

impl From<SecretError> for SecretCompositionError {
    fn from(error: SecretError) -> Self {
        match error {
            SecretError::NotFound => Self::NotFound,
            SecretError::BindingMismatch => Self::BindingMismatch,
            SecretError::NotLeaseable => Self::NotLeaseable,
            SecretError::InvalidLeaseWindow => Self::LeaseExpired,
            SecretError::OperationConflict
            | SecretError::AlreadyExists
            | SecretError::VersionMismatch
            | SecretError::RecordRevisionMismatch
            | SecretError::InvalidTransition => Self::InvalidTransition,
            SecretError::CapacityExceeded => Self::CapacityExceeded,
            SecretError::ContractExhausted => Self::ContractExhausted,
            SecretError::EmptySecret | SecretError::SecretTooLarge => Self::InvalidKeyMaterial,
            SecretError::ReadbackMismatch => Self::ReadbackMismatch,
            SecretError::RecoveryEvidenceMissing => Self::EvidenceMissing,
            SecretError::OutcomeUnknown => Self::OutcomeUnknown,
            SecretError::Quarantined => Self::Quarantined,
        }
    }
}
