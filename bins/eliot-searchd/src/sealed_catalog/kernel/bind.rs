//! Transactional publication of one immutable sealed catalog binding.

use std::path::Path;

use crate::sealed_digest::sha256;
use crate::sealed_store::{SensitiveBytes, open_sealed, verify_sealed};
use crate::sealed_transaction_guard::put_idempotent_verified;

use super::model::{SealedCatalogBinding, SealedCatalogReceipt};
use super::spec::{
    CATALOG_FORMAT_VERSION, SealedCatalogError, validate_identifier,
};

/// Creates or exactly replays an immutable source-revision catalog binding.
#[allow(clippy::too_many_arguments)]
pub fn bind_revision(
    data_root: &Path,
    content_operation_id: &str,
    content_object_id: &str,
    catalog_operation_id: &str,
    catalog_object_id: &str,
    source_id: &str,
    source_revision_id: &str,
) -> Result<SealedCatalogReceipt, SealedCatalogError> {
    validate_identifier(content_operation_id)?;
    validate_identifier(content_object_id)?;
    validate_identifier(catalog_operation_id)?;
    validate_identifier(catalog_object_id)?;
    validate_identifier(source_id)?;
    validate_identifier(source_revision_id)?;

    let content = open_sealed(data_root, content_object_id)?;
    let content_sha256 = sha256(content.expose())?;
    let content_plaintext_bytes =
        u64::try_from(content.len()).map_err(|_| SealedCatalogError::ContentLengthMismatch)?;
    let content_transaction = put_idempotent_verified(
        data_root,
        content_operation_id,
        content_object_id,
        &content,
    )?;
    let content_verification = verify_sealed(data_root, content_object_id)?;
    if content_transaction.plaintext_bytes != content_plaintext_bytes
        || content_transaction.plaintext_bytes != content_verification.plaintext_bytes
        || content_transaction.ciphertext_bytes != content_verification.ciphertext_bytes
    {
        return Err(SealedCatalogError::ContentLengthMismatch);
    }

    let binding = SealedCatalogBinding {
        source_id: source_id.to_owned(),
        source_revision_id: source_revision_id.to_owned(),
        content_operation_id: content_operation_id.to_owned(),
        content_object_id: content_object_id.to_owned(),
        content_sha256,
        content_plaintext_bytes,
        content_ciphertext_bytes: content_verification.ciphertext_bytes,
        catalog_format_version: CATALOG_FORMAT_VERSION,
    };
    let encoded = binding.encode()?;
    let catalog_transaction = put_idempotent_verified(
        data_root,
        catalog_operation_id,
        catalog_object_id,
        &SensitiveBytes::new(encoded.as_bytes().to_vec())?,
    )?;
    let readback = open_sealed(data_root, catalog_object_id)?;
    if readback.expose() != encoded.as_bytes()
        || SealedCatalogBinding::decode(readback.expose())? != binding
    {
        return Err(SealedCatalogError::CatalogReadbackMismatch);
    }
    Ok(SealedCatalogReceipt {
        catalog_object_id: catalog_object_id.to_owned(),
        binding,
        content_transaction,
        catalog_transaction,
        catalog_readback_verified: true,
    })
}
