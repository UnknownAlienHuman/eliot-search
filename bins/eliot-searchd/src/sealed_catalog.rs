//! DPAPI-sealed source-revision catalog bindings.
//!
//! A catalog manifest is itself an immutable DPAPI object written through the
//! idempotent sealed transaction lifecycle. It binds one stable source and
//! revision identity to one exact content object, operation, SHA-256 digest, and
//! byte accounting. Reads authenticate both objects and recompute the digest.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;
use std::collections::BTreeMap;
use std::path::Path;

use crate::sealed_digest::{DigestError, Sha256Digest, sha256};
use crate::sealed_store::{
    SealedStoreError, SensitiveBytes, open_sealed, verify_sealed,
};
use crate::sealed_transaction::{
    SealedTransactionError, SealedTransactionReceipt,
};
use crate::sealed_transaction_guard::put_idempotent_verified;

/// Maximum source or revision identifier length in the sealed catalog.
pub const MAX_CATALOG_IDENTIFIER_BYTES: usize = 128;
const CATALOG_MAGIC: &str = "ELIOT-SEALED-CATALOG-V1";
const CATALOG_FORMAT_VERSION: u16 = 1;
const CATALOG_FIELD_COUNT: usize = 8;

/// Closed catalog failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedCatalogError {
    /// A source, revision, operation, or object identifier is malformed.
    InvalidIdentifier,
    /// A sealed catalog manifest is malformed or contains unknown fields.
    ManifestInvalid,
    /// Requested source or revision differs from the immutable manifest.
    SourceBindingMismatch,
    /// Content object or transaction identity differs from the manifest.
    ContentBindingMismatch,
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
            Self::ContentBindingMismatch => "SEALED_CATALOG_CONTENT_BINDING_MISMATCH",
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

impl SealedCatalogBinding {
    fn validate(&self) -> Result<(), SealedCatalogError> {
        validate_identifier(&self.source_id)?;
        validate_identifier(&self.source_revision_id)?;
        validate_identifier(&self.content_operation_id)?;
        validate_identifier(&self.content_object_id)?;
        if self.content_plaintext_bytes == 0
            || self.content_ciphertext_bytes == 0
            || self.catalog_format_version != CATALOG_FORMAT_VERSION
        {
            return Err(SealedCatalogError::ManifestInvalid);
        }
        Ok(())
    }

    fn encode(&self) -> Result<String, SealedCatalogError> {
        self.validate()?;
        Ok(format!(
            concat!(
                "{CATALOG_MAGIC}\n",
                "catalog_format_version={}\n",
                "source_id={}\n",
                "source_revision_id={}\n",
                "content_operation_id={}\n",
                "content_object_id={}\n",
                "content_sha256={}\n",
                "content_plaintext_bytes={}\n",
                "content_ciphertext_bytes={}\n"
            ),
            self.catalog_format_version,
            self.source_id,
            self.source_revision_id,
            self.content_operation_id,
            self.content_object_id,
            self.content_sha256,
            self.content_plaintext_bytes,
            self.content_ciphertext_bytes,
        ))
    }

    fn decode(bytes: &[u8]) -> Result<Self, SealedCatalogError> {
        let value = core::str::from_utf8(bytes)
            .map_err(|_| SealedCatalogError::ManifestInvalid)?;
        if !value.ends_with('\n') {
            return Err(SealedCatalogError::ManifestInvalid);
        }
        let mut lines = value.lines();
        if lines.next() != Some(CATALOG_MAGIC) {
            return Err(SealedCatalogError::ManifestInvalid);
        }
        let mut fields = BTreeMap::new();
        for line in lines {
            let Some((key, field_value)) = line.split_once('=') else {
                return Err(SealedCatalogError::ManifestInvalid);
            };
            if key.is_empty()
                || field_value.is_empty()
                || fields
                    .insert(key.to_owned(), field_value.to_owned())
                    .is_some()
            {
                return Err(SealedCatalogError::ManifestInvalid);
            }
        }
        if fields.len() != CATALOG_FIELD_COUNT {
            return Err(SealedCatalogError::ManifestInvalid);
        }
        let binding = Self {
            source_id: take_field(&mut fields, "source_id")?,
            source_revision_id: take_field(&mut fields, "source_revision_id")?,
            content_operation_id: take_field(&mut fields, "content_operation_id")?,
            content_object_id: take_field(&mut fields, "content_object_id")?,
            content_sha256: Sha256Digest::from_hex(&take_field(
                &mut fields,
                "content_sha256",
            )?)?,
            content_plaintext_bytes: parse_u64(&take_field(
                &mut fields,
                "content_plaintext_bytes",
            )?)?,
            content_ciphertext_bytes: parse_u64(&take_field(
                &mut fields,
                "content_ciphertext_bytes",
            )?)?,
            catalog_format_version: parse_u16(&take_field(
                &mut fields,
                "catalog_format_version",
            )?)?,
        };
        if !fields.is_empty() {
            return Err(SealedCatalogError::ManifestInvalid);
        }
        binding.validate()?;
        Ok(binding)
    }
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
        content,
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
        SensitiveBytes::new(encoded.as_bytes().to_vec())?,
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

fn validate_identifier(value: &str) -> Result<(), SealedCatalogError> {
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

fn take_field(
    fields: &mut BTreeMap<String, String>,
    key: &str,
) -> Result<String, SealedCatalogError> {
    fields
        .remove(key)
        .ok_or(SealedCatalogError::ManifestInvalid)
}

fn parse_u64(value: &str) -> Result<u64, SealedCatalogError> {
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(SealedCatalogError::ManifestInvalid);
    }
    value
        .parse::<u64>()
        .map_err(|_| SealedCatalogError::ManifestInvalid)
}

fn parse_u16(value: &str) -> Result<u16, SealedCatalogError> {
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(SealedCatalogError::ManifestInvalid);
    }
    value
        .parse::<u16>()
        .map_err(|_| SealedCatalogError::ManifestInvalid)
}
