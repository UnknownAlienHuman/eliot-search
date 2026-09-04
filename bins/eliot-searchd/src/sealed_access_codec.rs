//! Strict canonical codec for append-only sealed access-fence records.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;
use std::collections::BTreeMap;

use crate::sealed_digest::{DigestError, Sha256Digest};

/// Maximum fence identifier bytes.
pub const MAX_FENCE_ID_BYTES: usize = 48;
/// Maximum other opaque identifier bytes.
pub const MAX_ACCESS_IDENTIFIER_BYTES: usize = 128;
/// Current strict access-fence record version.
pub const ACCESS_FENCE_FORMAT_VERSION: u16 = 1;
/// Canonical record magic.
pub const ACCESS_FENCE_MAGIC: &str = "ELIOT-SEALED-ACCESS-FENCE-V1";
const FIELD_COUNT: usize = 17;
const ZERO_DIGEST_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Closed current access disposition.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessFenceState {
    /// Exact bound source revision may be read under the current fence.
    Allow,
    /// Access is revoked. This state is terminal for one fence chain.
    Deny,
}

impl AccessFenceState {
    /// Stable canonical wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::Deny => "DENY",
        }
    }

    fn parse(value: &str) -> Result<Self, AccessCodecError> {
        match value {
            "ALLOW" => Ok(Self::Allow),
            "DENY" => Ok(Self::Deny),
            _ => Err(AccessCodecError::InvalidState),
        }
    }
}

/// Closed access-fence codec failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessCodecError {
    /// A bounded identifier is malformed.
    InvalidIdentifier,
    /// Record magic, fields, order-independent map, or canonical bytes are invalid.
    InvalidRecord,
    /// Numeric value is zero, non-canonical, or out of range.
    InvalidNumber,
    /// State is outside the closed `ALLOW | DENY` set.
    InvalidState,
    /// Predecessor fields are invalid for this generation.
    InvalidPredecessor,
    /// Windows CNG digest text is malformed.
    Digest(DigestError),
}

impl AccessCodecError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidIdentifier => "SEALED_ACCESS_IDENTIFIER_INVALID",
            Self::InvalidRecord => "SEALED_ACCESS_RECORD_INVALID",
            Self::InvalidNumber => "SEALED_ACCESS_NUMBER_INVALID",
            Self::InvalidState => "SEALED_ACCESS_STATE_INVALID",
            Self::InvalidPredecessor => "SEALED_ACCESS_PREDECESSOR_INVALID",
            Self::Digest(error) => error.code(),
        }
    }
}

impl fmt::Display for AccessCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AccessCodecError {}

impl From<DigestError> for AccessCodecError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

/// One immutable access-fence chain record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessFenceRecord {
    /// Strict record format version.
    pub format_version: u16,
    /// Stable fence-chain identity.
    pub fence_id: String,
    /// Contiguous non-zero record generation.
    pub generation: u64,
    /// Exact predecessor generation, or zero for generation one.
    pub previous_generation: u64,
    /// SHA-256 of exact predecessor record bytes, or zero digest for generation one.
    pub previous_record_sha256: Sha256Digest,
    /// Stable source identity.
    pub source_id: String,
    /// Immutable source revision identity.
    pub source_revision_id: String,
    /// Exact sealed catalog object identity.
    pub catalog_object_id: String,
    /// Stable scope authority identity.
    pub scope_id: String,
    /// Monotone non-zero scope revision.
    pub scope_revision: u64,
    /// Stable policy authority identity.
    pub policy_id: String,
    /// Monotone non-zero policy revision.
    pub policy_revision: u64,
    /// Contiguous non-zero access generation.
    pub access_generation: u64,
    /// Monotone purge-ledger generation.
    pub purge_generation: u64,
    /// Closed current disposition.
    pub state: AccessFenceState,
    /// Owner epoch that admitted this fence record.
    pub admitted_owner_epoch: u64,
    /// Native physical-root binding from that owner.
    pub owner_root_binding_sha256: Sha256Digest,
}

impl AccessFenceRecord {
    /// Validates local canonical invariants independent of predecessor state.
    pub fn validate(&self) -> Result<(), AccessCodecError> {
        if self.format_version != ACCESS_FENCE_FORMAT_VERSION
            || self.generation == 0
            || self.scope_revision == 0
            || self.policy_revision == 0
            || self.access_generation == 0
            || self.admitted_owner_epoch == 0
        {
            return Err(AccessCodecError::InvalidNumber);
        }
        validate_fence_id(&self.fence_id)?;
        for value in [
            &self.source_id,
            &self.source_revision_id,
            &self.catalog_object_id,
            &self.scope_id,
            &self.policy_id,
        ] {
            validate_identifier(value)?;
        }
        if self.generation == 1 {
            if self.previous_generation != 0
                || self.previous_record_sha256.to_hex() != ZERO_DIGEST_HEX
                || self.access_generation != 1
                || self.state != AccessFenceState::Allow
            {
                return Err(AccessCodecError::InvalidPredecessor);
            }
        } else if self.previous_generation != self.generation - 1 {
            return Err(AccessCodecError::InvalidPredecessor);
        }
        Ok(())
    }

    /// Encodes exact canonical UTF-8 bytes.
    pub fn encode(&self) -> Result<String, AccessCodecError> {
        self.validate()?;
        Ok(format!(
            concat!(
                "{ACCESS_FENCE_MAGIC}\n",
                "format_version={}\n",
                "fence_id={}\n",
                "generation={}\n",
                "previous_generation={}\n",
                "previous_record_sha256={}\n",
                "source_id={}\n",
                "source_revision_id={}\n",
                "catalog_object_id={}\n",
                "scope_id={}\n",
                "scope_revision={}\n",
                "policy_id={}\n",
                "policy_revision={}\n",
                "access_generation={}\n",
                "purge_generation={}\n",
                "state={}\n",
                "admitted_owner_epoch={}\n",
                "owner_root_binding_sha256={}\n"
            ),
            self.format_version,
            self.fence_id,
            self.generation,
            self.previous_generation,
            self.previous_record_sha256,
            self.source_id,
            self.source_revision_id,
            self.catalog_object_id,
            self.scope_id,
            self.scope_revision,
            self.policy_id,
            self.policy_revision,
            self.access_generation,
            self.purge_generation,
            self.state.as_str(),
            self.admitted_owner_epoch,
            self.owner_root_binding_sha256,
        ))
    }

    /// Parses strict canonical UTF-8 bytes and rejects unknown fields.
    pub fn decode(bytes: &[u8]) -> Result<Self, AccessCodecError> {
        let value = core::str::from_utf8(bytes)
            .map_err(|_| AccessCodecError::InvalidRecord)?;
        if !value.ends_with('\n') {
            return Err(AccessCodecError::InvalidRecord);
        }
        let mut lines = value.lines();
        if lines.next() != Some(ACCESS_FENCE_MAGIC) {
            return Err(AccessCodecError::InvalidRecord);
        }
        let mut fields = BTreeMap::new();
        for line in lines {
            let Some((key, field_value)) = line.split_once('=') else {
                return Err(AccessCodecError::InvalidRecord);
            };
            if key.is_empty()
                || field_value.is_empty()
                || fields
                    .insert(key.to_owned(), field_value.to_owned())
                    .is_some()
            {
                return Err(AccessCodecError::InvalidRecord);
            }
        }
        if fields.len() != FIELD_COUNT {
            return Err(AccessCodecError::InvalidRecord);
        }
        let record = Self {
            format_version: parse_u16(&take(&mut fields, "format_version")?)?,
            fence_id: take(&mut fields, "fence_id")?,
            generation: parse_u64(&take(&mut fields, "generation")?, false)?,
            previous_generation: parse_u64(
                &take(&mut fields, "previous_generation")?,
                true,
            )?,
            previous_record_sha256: Sha256Digest::from_hex(&take(
                &mut fields,
                "previous_record_sha256",
            )?)?,
            source_id: take(&mut fields, "source_id")?,
            source_revision_id: take(&mut fields, "source_revision_id")?,
            catalog_object_id: take(&mut fields, "catalog_object_id")?,
            scope_id: take(&mut fields, "scope_id")?,
            scope_revision: parse_u64(&take(&mut fields, "scope_revision")?, false)?,
            policy_id: take(&mut fields, "policy_id")?,
            policy_revision: parse_u64(
                &take(&mut fields, "policy_revision")?,
                false,
            )?,
            access_generation: parse_u64(
                &take(&mut fields, "access_generation")?,
                false,
            )?,
            purge_generation: parse_u64(
                &take(&mut fields, "purge_generation")?,
                true,
            )?,
            state: AccessFenceState::parse(&take(&mut fields, "state")?)?,
            admitted_owner_epoch: parse_u64(
                &take(&mut fields, "admitted_owner_epoch")?,
                false,
            )?,
            owner_root_binding_sha256: Sha256Digest::from_hex(&take(
                &mut fields,
                "owner_root_binding_sha256",
            )?)?,
        };
        if !fields.is_empty() {
            return Err(AccessCodecError::InvalidRecord);
        }
        record.validate()?;
        if record.encode()?.as_bytes() != bytes {
            return Err(AccessCodecError::InvalidRecord);
        }
        Ok(record)
    }
}

/// Exact all-zero predecessor digest for generation one.
pub fn zero_digest() -> Result<Sha256Digest, AccessCodecError> {
    Sha256Digest::from_hex(ZERO_DIGEST_HEX).map_err(AccessCodecError::from)
}

/// Validates one fence-chain identifier.
pub fn validate_fence_id(value: &str) -> Result<(), AccessCodecError> {
    validate_token(value, MAX_FENCE_ID_BYTES)
}

/// Validates one bounded opaque identifier.
pub fn validate_identifier(value: &str) -> Result<(), AccessCodecError> {
    validate_token(value, MAX_ACCESS_IDENTIFIER_BYTES)
}

fn validate_token(value: &str, max_bytes: usize) -> Result<(), AccessCodecError> {
    if value.is_empty()
        || value.len() > max_bytes
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Err(AccessCodecError::InvalidIdentifier)
    } else {
        Ok(())
    }
}

fn take(
    fields: &mut BTreeMap<String, String>,
    key: &str,
) -> Result<String, AccessCodecError> {
    fields.remove(key).ok_or(AccessCodecError::InvalidRecord)
}

fn parse_u64(value: &str, allow_zero: bool) -> Result<u64, AccessCodecError> {
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(AccessCodecError::InvalidNumber);
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| AccessCodecError::InvalidNumber)?;
    if !allow_zero && parsed == 0 {
        return Err(AccessCodecError::InvalidNumber);
    }
    Ok(parsed)
}

fn parse_u16(value: &str) -> Result<u16, AccessCodecError> {
    let value = parse_u64(value, false)?;
    u16::try_from(value).map_err(|_| AccessCodecError::InvalidNumber)
}
