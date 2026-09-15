//! Strict canonical manifest codec for one sealed catalog binding.

use std::collections::BTreeMap;

use crate::sealed_digest::Sha256Digest;

use super::model::SealedCatalogBinding;
use super::spec::{
    CATALOG_FIELD_COUNT, CATALOG_FORMAT_VERSION, CATALOG_MAGIC,
    SealedCatalogError, validate_identifier,
};

impl SealedCatalogBinding {
    pub(super) fn validate(&self) -> Result<(), SealedCatalogError> {
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

    pub(super) fn encode(&self) -> Result<String, SealedCatalogError> {
        self.validate()?;
        Ok(format!(
            concat!(
                "{}\n",
                "catalog_format_version={}\n",
                "source_id={}\n",
                "source_revision_id={}\n",
                "content_operation_id={}\n",
                "content_object_id={}\n",
                "content_sha256={}\n",
                "content_plaintext_bytes={}\n",
                "content_ciphertext_bytes={}\n"
            ),
            CATALOG_MAGIC,
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

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, SealedCatalogError> {
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
