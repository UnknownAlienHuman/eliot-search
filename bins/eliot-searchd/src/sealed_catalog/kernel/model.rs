//! Immutable sealed-catalog models and content-free receipts.

use core::fmt;

use crate::sealed_digest::Sha256Digest;
use crate::sealed_store::SensitiveBytes;
use crate::sealed_transaction::SealedTransactionReceipt;

/// Immutable content binding stored inside one sealed catalog object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedCatalogBinding {
    /// Stable source identity supplied by the source catalog owner.
    pub source_id: String,
    /// Immutable source revision identity.
    pub source_revision_id: String,
    /// Idempotent operation that created or reconciled the content object.
    pub content_operation_id: String,
    /// DPAPI-sealed immutable content object identity.
    pub content_object_id: String,
    /// SHA-256 of exact plaintext content bytes.
    pub content_sha256: Sha256Digest,
    /// Exact plaintext byte count.
    pub content_plaintext_bytes: u64,
    /// Exact DPAPI ciphertext byte count.
    pub content_ciphertext_bytes: u64,
    /// Strict catalog format version.
    pub catalog_format_version: u16,
}

/// Content-free result of binding a source revision to sealed content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedCatalogReceipt {
    /// Catalog object identity.
    pub catalog_object_id: String,
    /// Exact immutable binding.
    pub binding: SealedCatalogBinding,
    /// Content transaction terminal receipt.
    pub content_transaction: SealedTransactionReceipt,
    /// Catalog transaction terminal receipt.
    pub catalog_transaction: SealedTransactionReceipt,
    /// Fresh catalog decrypt/readback matched exact canonical manifest bytes.
    pub catalog_readback_verified: bool,
}

/// Authenticated catalog read containing short-lived plaintext content.
pub struct SealedCatalogRead {
    /// Catalog object identity.
    pub catalog_object_id: String,
    /// Exact immutable binding parsed from authenticated catalog bytes.
    pub binding: SealedCatalogBinding,
    /// Exact authenticated content bytes. The allocation wipes on drop.
    pub content: SensitiveBytes,
}

impl fmt::Debug for SealedCatalogRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SealedCatalogRead")
            .field("catalog_object_id", &self.catalog_object_id)
            .field("binding", &self.binding)
            .field("content", &"<redacted>")
            .finish()
    }
}

/// Content-free verification receipt for a catalog/content pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedCatalogVerifyReceipt {
    /// Catalog object identity.
    pub catalog_object_id: String,
    /// Stable source identity.
    pub source_id: String,
    /// Immutable source revision identity.
    pub source_revision_id: String,
    /// Content object identity.
    pub content_object_id: String,
    /// Exact plaintext SHA-256.
    pub content_sha256: Sha256Digest,
    /// Exact plaintext bytes.
    pub content_plaintext_bytes: u64,
    /// Exact ciphertext bytes.
    pub content_ciphertext_bytes: u64,
    /// Both DPAPI objects and the plaintext digest were verified.
    pub authenticated: bool,
}
