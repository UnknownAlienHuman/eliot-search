//! Strict owner-epoch plaintext codec.

use std::collections::BTreeMap;

use crate::sealed_digest::Sha256Digest;

use super::spec::{
    OWNER_EPOCH_FIELD_COUNT, OWNER_EPOCH_FORMAT_VERSION, OWNER_EPOCH_MAGIC,
    OwnerEpochError, ZERO_DIGEST_HEX,
};

/// Immutable sealed owner-epoch record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerEpochRecord {
    /// Strict record format version.
    pub format_version: u16,
    /// Monotone non-zero epoch.
    pub epoch: u64,
    /// Exact predecessor epoch, or zero for epoch one.
    pub previous_epoch: u64,
    /// SHA-256 of exact predecessor record plaintext, or zero digest for epoch one.
    pub previous_record_sha256: Sha256Digest,
    /// SHA-256 of native physical data-root identity.
    pub root_binding_sha256: Sha256Digest,
}

impl OwnerEpochRecord {
    pub(super) fn validate(&self) -> Result<(), OwnerEpochError> {
        if self.format_version != OWNER_EPOCH_FORMAT_VERSION || self.epoch == 0 {
            return Err(OwnerEpochError::ChainInvalid);
        }
        if self.epoch == 1 {
            if self.previous_epoch != 0
                || self.previous_record_sha256.to_hex() != ZERO_DIGEST_HEX
            {
                return Err(OwnerEpochError::PredecessorMismatch);
            }
        } else if self.previous_epoch != self.epoch - 1 {
            return Err(OwnerEpochError::PredecessorMismatch);
        }
        Ok(())
    }

    pub(super) fn encode(&self) -> Result<String, OwnerEpochError> {
        self.validate()?;
        Ok(format!(
            concat!(
                "{}\n",
                "format_version={}\n",
                "epoch={}\n",
                "previous_epoch={}\n",
                "previous_record_sha256={}\n",
                "root_binding_sha256={}\n"
            ),
            OWNER_EPOCH_MAGIC,
            self.format_version,
            self.epoch,
            self.previous_epoch,
            self.previous_record_sha256,
            self.root_binding_sha256,
        ))
    }

    /// Decodes the exact canonical plaintext record.
    pub fn decode(bytes: &[u8]) -> Result<Self, OwnerEpochError> {
        let value = core::str::from_utf8(bytes)
            .map_err(|_| OwnerEpochError::ChainInvalid)?;
        if !value.ends_with('\n') {
            return Err(OwnerEpochError::ChainInvalid);
        }
        let mut lines = value.lines();
        if lines.next() != Some(OWNER_EPOCH_MAGIC) {
            return Err(OwnerEpochError::ChainInvalid);
        }
        let mut fields = BTreeMap::new();
        for line in lines {
            let Some((key, field_value)) = line.split_once('=') else {
                return Err(OwnerEpochError::ChainInvalid);
            };
            if key.is_empty()
                || field_value.is_empty()
                || fields
                    .insert(key.to_owned(), field_value.to_owned())
                    .is_some()
            {
                return Err(OwnerEpochError::ChainInvalid);
            }
        }
        if fields.len() != OWNER_EPOCH_FIELD_COUNT {
            return Err(OwnerEpochError::ChainInvalid);
        }
        let record = Self {
            format_version: parse_u16(&take(&mut fields, "format_version")?)?,
            epoch: parse_u64(&take(&mut fields, "epoch")?)?,
            previous_epoch: parse_u64(&take(&mut fields, "previous_epoch")?)?,
            previous_record_sha256: Sha256Digest::from_hex(&take(
                &mut fields,
                "previous_record_sha256",
            )?)?,
            root_binding_sha256: Sha256Digest::from_hex(&take(
                &mut fields,
                "root_binding_sha256",
            )?)?,
        };
        if !fields.is_empty() {
            return Err(OwnerEpochError::ChainInvalid);
        }
        record.validate()?;
        Ok(record)
    }
}

fn take(
    fields: &mut BTreeMap<String, String>,
    key: &str,
) -> Result<String, OwnerEpochError> {
    fields.remove(key).ok_or(OwnerEpochError::ChainInvalid)
}

pub(super) fn parse_u64(value: &str) -> Result<u64, OwnerEpochError> {
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(OwnerEpochError::ChainInvalid);
    }
    value
        .parse::<u64>()
        .map_err(|_| OwnerEpochError::ChainInvalid)
}

fn parse_u16(value: &str) -> Result<u16, OwnerEpochError> {
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(OwnerEpochError::ChainInvalid);
    }
    value
        .parse::<u16>()
        .map_err(|_| OwnerEpochError::ChainInvalid)
}
