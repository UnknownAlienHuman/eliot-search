//! Authenticated catalog/content readback and content-free verification.

use std::path::Path;

use crate::sealed_digest::sha256;
use crate::sealed_store::{open_sealed, verify_sealed};

use super::model::{SealedCatalogBinding, SealedCatalogRead, SealedCatalogVerifyReceipt};
use super::spec::{SealedCatalogError, validate_identifier};

/// Opens a catalog binding, authenticates content, and recomputes exact SHA-256.
pub fn read_revision(
    data_root: &Path,
    catalog_object_id: &str,
    expected_source_id: &str,
    expected_source_revision_id: &str,
) -> Result<SealedCatalogRead, SealedCatalogError> {
    validate_identifier(catalog_object_id)?;
    validate_identifier(expected_source_id)?;
    validate_identifier(expected_source_revision_id)?;
    let manifest = open_sealed(data_root, catalog_object_id)?;
    let binding = SealedCatalogBinding::decode(manifest.expose())?;
    if binding.source_id != expected_source_id
        || binding.source_revision_id != expected_source_revision_id
    {
        return Err(SealedCatalogError::SourceBindingMismatch);
    }
    let content_verification = verify_sealed(data_root, &binding.content_object_id)?;
    if content_verification.plaintext_bytes != binding.content_plaintext_bytes
        || content_verification.ciphertext_bytes != binding.content_ciphertext_bytes
    {
        return Err(SealedCatalogError::ContentLengthMismatch);
    }
    let content = open_sealed(data_root, &binding.content_object_id)?;
    if u64::try_from(content.len()).map_err(|_| SealedCatalogError::ContentLengthMismatch)?
        != binding.content_plaintext_bytes
    {
        return Err(SealedCatalogError::ContentLengthMismatch);
    }
    if sha256(content.expose())? != binding.content_sha256 {
        return Err(SealedCatalogError::ContentDigestMismatch);
    }
    Ok(SealedCatalogRead {
        catalog_object_id: catalog_object_id.to_owned(),
        binding,
        content,
    })
}

/// Verifies a catalog/content pair without returning plaintext to the caller.
pub fn verify_revision(
    data_root: &Path,
    catalog_object_id: &str,
    expected_source_id: &str,
    expected_source_revision_id: &str,
) -> Result<SealedCatalogVerifyReceipt, SealedCatalogError> {
    let read = read_revision(
        data_root,
        catalog_object_id,
        expected_source_id,
        expected_source_revision_id,
    )?;
    Ok(SealedCatalogVerifyReceipt {
        catalog_object_id: read.catalog_object_id,
        source_id: read.binding.source_id,
        source_revision_id: read.binding.source_revision_id,
        content_object_id: read.binding.content_object_id,
        content_sha256: read.binding.content_sha256,
        content_plaintext_bytes: read.binding.content_plaintext_bytes,
        content_ciphertext_bytes: read.binding.content_ciphertext_bytes,
        authenticated: true,
    })
}
