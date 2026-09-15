//! Zeroizing plaintext ownership and content-free operation receipts.

use core::fmt;

use zeroize::Zeroize;

use super::spec::{MAX_PLAINTEXT_BYTES, SealedStoreError};

/// Plaintext owner that is overwritten before its allocation is released.
/// The type is deliberately non-`Clone`; `Debug` never exposes bytes.
pub struct SensitiveBytes(Vec<u8>);

impl SensitiveBytes {
    /// Creates a finite non-empty plaintext buffer.
    pub fn new(mut bytes: Vec<u8>) -> Result<Self, SealedStoreError> {
        if bytes.is_empty() {
            bytes.zeroize();
            return Err(SealedStoreError::EmptyPlaintext);
        }
        if bytes.len() > MAX_PLAINTEXT_BYTES {
            bytes.zeroize();
            return Err(SealedStoreError::PlaintextTooLarge);
        }
        Ok(Self(bytes))
    }

    /// Borrows plaintext for the shortest possible caller-owned interval.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// Plaintext byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the plaintext buffer is empty. A valid instance is never empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SensitiveBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SensitiveBytes")
            .field("bytes", &"<redacted>")
            .field("length", &self.0.len())
            .finish()
    }
}

impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

pub(crate) fn wipe(bytes: &mut [u8]) {
    bytes.zeroize();
}

/// Content-free successful immutable write receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealReceipt {
    pub object_id: String,
    pub plaintext_bytes: u64,
    pub ciphertext_bytes: u64,
    pub format_version: u16,
    pub protection_scope: &'static str,
    pub readback_verified: bool,
}

/// Content-free successful verification receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifyReceipt {
    pub object_id: String,
    pub plaintext_bytes: u64,
    pub ciphertext_bytes: u64,
    pub format_version: u16,
    pub protection_scope: &'static str,
    pub authenticated: bool,
}

/// Content-free logical-deletion receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteReceipt {
    pub object_id: String,
    pub logical_delete_complete: bool,
    pub physical_erasure_guaranteed: bool,
}
