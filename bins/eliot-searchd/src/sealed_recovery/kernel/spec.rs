//! Closed recovery limits, errors, and issue classifications.

use core::fmt;

use crate::sealed_digest::DigestError;
use crate::sealed_store::SealedStoreError;
use crate::sealed_transaction::SealedTransactionError;

/// Maximum durable operation identities inspected during one startup.
pub const MAX_RECOVERY_OPERATIONS: usize = 1_000_000;
/// Maximum individual issue records retained in one report.
pub const MAX_RECOVERY_ISSUES: usize = 4_096;

/// Closed structural recovery failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedRecoveryError {
    #[cfg(not(windows))]
    /// Windows recovery is unavailable on the current platform.
    UnsupportedPlatform,
    /// Recovery was attempted without a live owner/root-lock guard.
    OwnerGuardRequired,
    /// Transaction directory or an entry is malformed or reparse-backed.
    TransactionDirectoryInvalid,
    /// Unknown or malformed transaction filename was observed.
    TransactionFilenameInvalid,
    /// Finite operation capacity was exceeded.
    OperationCapacityExceeded,
    /// Filesystem enumeration, readback, or cleanup failed.
    IoFailure,
    /// Windows CNG SHA-256 failed.
    Digest(DigestError),
    /// DPAPI sealed-object operation failed.
    SealedStore(SealedStoreError),
    /// V2 transaction inspection or replay failed.
    Transaction(SealedTransactionError),
}

impl SealedRecoveryError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => "SEALED_RECOVERY_UNSUPPORTED_PLATFORM",
            Self::OwnerGuardRequired => "SEALED_RECOVERY_OWNER_GUARD_REQUIRED",
            Self::TransactionDirectoryInvalid => {
                "SEALED_RECOVERY_TRANSACTION_DIRECTORY_INVALID"
            }
            Self::TransactionFilenameInvalid => {
                "SEALED_RECOVERY_TRANSACTION_FILENAME_INVALID"
            }
            Self::OperationCapacityExceeded => {
                "SEALED_RECOVERY_OPERATION_CAPACITY_EXCEEDED"
            }
            Self::IoFailure => "SEALED_RECOVERY_IO_FAILURE",
            Self::Digest(error) => error.code(),
            Self::SealedStore(error) => error.code(),
            Self::Transaction(error) => error.code(),
        }
    }
}

impl fmt::Display for SealedRecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedRecoveryError {}

impl From<DigestError> for SealedRecoveryError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

impl From<SealedStoreError> for SealedRecoveryError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

impl From<SealedTransactionError> for SealedRecoveryError {
    fn from(error: SealedTransactionError) -> Self {
        Self::Transaction(error)
    }
}

/// Non-success classification retained without source content.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RecoveryIssueCode {
    /// Durable intent exists but the external sealed object is absent.
    PreparedObjectMissing,
    /// A committed receipt exists but its sealed object is absent.
    CommittedObjectMissing,
    /// Intent and receipt coexist but do not bind the same exact request.
    TransactionConflict,
    /// DPAPI plaintext byte count differs from durable metadata.
    PlaintextLengthMismatch,
    /// DPAPI ciphertext byte count differs from the terminal receipt.
    CiphertextLengthMismatch,
    /// Exact plaintext SHA-256 differs from durable metadata.
    PlaintextDigestMismatch,
}

impl RecoveryIssueCode {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreparedObjectMissing => "PREPARED_OBJECT_MISSING",
            Self::CommittedObjectMissing => "COMMITTED_OBJECT_MISSING",
            Self::TransactionConflict => "TRANSACTION_CONFLICT",
            Self::PlaintextLengthMismatch => "PLAINTEXT_LENGTH_MISMATCH",
            Self::CiphertextLengthMismatch => "CIPHERTEXT_LENGTH_MISMATCH",
            Self::PlaintextDigestMismatch => "PLAINTEXT_DIGEST_MISMATCH",
        }
    }
}
