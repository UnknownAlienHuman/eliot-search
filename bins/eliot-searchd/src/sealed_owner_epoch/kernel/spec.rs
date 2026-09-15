//! Closed owner-epoch format, limits and failure taxonomy.

use core::fmt;

use crate::sealed_digest::DigestError;
use crate::sealed_root_lock::SealedRootLockError;
use crate::sealed_store::SealedStoreError;
use crate::sealed_transaction::SealedTransactionError;

/// Maximum historical owner epochs validated during one acquisition.
pub const MAX_OWNER_EPOCH_RECORDS: usize = 1_000_000;
pub(super) const OWNER_EPOCH_MAGIC: &str = "ELIOT-SEALED-OWNER-EPOCH-V1";
pub(super) const OWNER_EPOCH_FORMAT_VERSION: u16 = 1;
pub(super) const OWNER_EPOCH_FIELD_COUNT: usize = 5;
pub(super) const SEALED_DIRECTORY: &str = "sealed-revisions";
pub(super) const SEALED_SUFFIX: &str = ".els-dpapi";
pub(super) const ZERO_DIGEST_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Closed owner-epoch failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerEpochError {
    #[cfg(not(windows))]
    /// Windows owner-epoch adapter is unavailable on this platform.
    UnsupportedPlatform,
    /// Historical epoch filename, record, or ordering is malformed.
    ChainInvalid,
    /// At least one historical epoch is missing.
    ChainGap,
    /// A record belongs to another physical data-root identity.
    RootBindingMismatch,
    /// Record predecessor digest does not match the exact previous record.
    PredecessorMismatch,
    /// Epoch or history capacity is exhausted.
    EpochExhausted,
    /// Filesystem enumeration/readback failed.
    IoFailure,
    /// Windows CNG SHA-256 failed.
    Digest(DigestError),
    /// Exclusive data-root acquisition failed.
    RootLock(SealedRootLockError),
    /// DPAPI sealed-object operation failed.
    SealedStore(SealedStoreError),
    /// Idempotent transaction reconciliation failed.
    Transaction(SealedTransactionError),
}

impl OwnerEpochError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => "SEALED_OWNER_EPOCH_UNSUPPORTED_PLATFORM",
            Self::ChainInvalid => "SEALED_OWNER_EPOCH_CHAIN_INVALID",
            Self::ChainGap => "SEALED_OWNER_EPOCH_CHAIN_GAP",
            Self::RootBindingMismatch => {
                "SEALED_OWNER_EPOCH_ROOT_BINDING_MISMATCH"
            }
            Self::PredecessorMismatch => {
                "SEALED_OWNER_EPOCH_PREDECESSOR_MISMATCH"
            }
            Self::EpochExhausted => "SEALED_OWNER_EPOCH_EXHAUSTED",
            Self::IoFailure => "SEALED_OWNER_EPOCH_IO_FAILURE",
            Self::Digest(error) => error.code(),
            Self::RootLock(error) => error.code(),
            Self::SealedStore(error) => error.code(),
            Self::Transaction(error) => error.code(),
        }
    }
}

impl fmt::Display for OwnerEpochError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for OwnerEpochError {}

impl From<DigestError> for OwnerEpochError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

impl From<SealedRootLockError> for OwnerEpochError {
    fn from(error: SealedRootLockError) -> Self {
        Self::RootLock(error)
    }
}

impl From<SealedStoreError> for OwnerEpochError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

impl From<SealedTransactionError> for OwnerEpochError {
    fn from(error: SealedTransactionError) -> Self {
        Self::Transaction(error)
    }
}
