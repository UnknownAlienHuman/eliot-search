//! Closed catalog format constants, identifiers, and error vocabulary.

use core::fmt;

use crate::sealed_digest::DigestError;
use crate::sealed_store::SealedStoreError;
use crate::sealed_transaction::SealedTransactionError;

/// Maximum source, revision, operation, or object identifier length.
pub const MAX_CATALOG_IDENTIFIER_BYTES: usize = 128;
pub(super) const CATALOG_MAGIC: &str = "ELIOT-SEALED-CATALOG-V1";
pub(super) const CATALOG_FORMAT_VERSION: u16 = 1;
pub(super) const CATALOG_FIELD_COUNT: usize = 8;

/// Closed catalog failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedCatalogError {
    /// A source, revision, operation, or object identifier is malformed.
    InvalidIdentifier,
    /// A sealed catalog manifest is malformed or contains unknown fields.
    ManifestInvalid,
    /// Requested source or revision differs from the immutable manifest.
    SourceBindingMismatch,
    /// Current content bytes differ from the immutable catalog digest.
    ContentDigestMismatch,
    /// Current content byte accounting differs from the immutable manifest.
    ContentLengthMismatch,
    /// The catalog manifest written through the transaction was not read back exactly.
    CatalogReadbackMismatch,
    /// Windows CNG digest adapter failed.
    Digest(DigestError),
    /// DPAPI sealed-object adapter failed.
    SealedStore(SealedStoreError),
    /// Idempotent transaction adapter failed.
    Transaction(SealedTransactionError),
}

impl SealedCatalogError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidIdentifier => "SEALED_CATALOG_IDENTIFIER_INVALID",
            Self::ManifestInvalid => "SEALED_CATALOG_MANIFEST_INVALID",
            Self::SourceBindingMismatch => "SEALED_CATALOG_SOURCE_BINDING_MISMATCH",
            Self::ContentDigestMismatch => "SEALED_CATALOG_CONTENT_DIGEST_MISMATCH",
            Self::ContentLengthMismatch => "SEALED_CATALOG_CONTENT_LENGTH_MISMATCH",
            Self::CatalogReadbackMismatch => "SEALED_CATALOG_READBACK_MISMATCH",
            Self::Digest(error) => error.code(),
            Self::SealedStore(error) => error.code(),
            Self::Transaction(error) => error.code(),
        }
    }
}

impl fmt::Display for SealedCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedCatalogError {}

impl From<DigestError> for SealedCatalogError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

impl From<SealedStoreError> for SealedCatalogError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

impl From<SealedTransactionError> for SealedCatalogError {
    fn from(error: SealedTransactionError) -> Self {
        Self::Transaction(error)
    }
}

pub(super) fn validate_identifier(value: &str) -> Result<(), SealedCatalogError> {
    if value.is_empty()
        || value.len() > MAX_CATALOG_IDENTIFIER_BYTES
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(SealedCatalogError::InvalidIdentifier);
    }
    Ok(())
}
