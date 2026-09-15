//! Closed operation-identity grammar and transaction failure vocabulary.

use core::fmt;

use crate::sealed_digest::DigestError;
use crate::sealed_store::SealedStoreError;

/// Maximum operation-identifier length.
pub const MAX_OPERATION_ID_BYTES: usize = 128;

/// Closed transaction failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedTransactionError {
    #[cfg(not(windows))]
    /// The current platform does not provide the required adapter.
    UnsupportedPlatform,
    /// Operation identity is malformed.
    InvalidOperationId,
    /// Operation is already executing in another process.
    OperationBusy,
    /// Durable intent is malformed or conflicts with the request.
    IntentConflict,
    /// Durable receipt is malformed or conflicts with the request.
    ReceiptConflict,
    /// An object exists without the exact operation intent/receipt.
    ObjectConflict,
    /// Existing decrypted bytes differ from retry input.
    ReplayContentMismatch,
    /// Filesystem operation failed.
    IoFailure,
    /// Exact metadata readback failed.
    ReadbackMismatch,
    /// Windows CNG SHA-256 failed.
    Digest(DigestError),
    /// Underlying sealed-store operation failed.
    SealedStore(SealedStoreError),
}

impl SealedTransactionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => "SEALED_TRANSACTION_UNSUPPORTED_PLATFORM",
            Self::InvalidOperationId => "SEALED_TRANSACTION_OPERATION_ID_INVALID",
            Self::OperationBusy => "SEALED_TRANSACTION_OPERATION_BUSY",
            Self::IntentConflict => "SEALED_TRANSACTION_INTENT_CONFLICT",
            Self::ReceiptConflict => "SEALED_TRANSACTION_RECEIPT_CONFLICT",
            Self::ObjectConflict => "SEALED_TRANSACTION_OBJECT_CONFLICT",
            Self::ReplayContentMismatch => {
                "SEALED_TRANSACTION_REPLAY_CONTENT_MISMATCH"
            }
            Self::IoFailure => "SEALED_TRANSACTION_IO_FAILURE",
            Self::ReadbackMismatch => "SEALED_TRANSACTION_READBACK_MISMATCH",
            Self::Digest(error) => error.code(),
            Self::SealedStore(error) => error.code(),
        }
    }
}

impl fmt::Display for SealedTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedTransactionError {}

impl From<DigestError> for SealedTransactionError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

impl From<SealedStoreError> for SealedTransactionError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

pub(crate) fn validate_operation_id(
    value: &str,
) -> Result<(), SealedTransactionError> {
    if value.is_empty()
        || value.len() > MAX_OPERATION_ID_BYTES
        || matches!(value, "." | "..")
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(SealedTransactionError::InvalidOperationId);
    }
    Ok(())
}
