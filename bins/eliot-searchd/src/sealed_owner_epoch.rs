//! Monotone DPAPI-sealed owner epochs for one Windows data root.
//!
//! The epoch is not supplied by a client. Acquisition first holds the exact OS
//! root lock, validates the contiguous immutable epoch chain, reconciles every
//! deterministic epoch transaction, then appends exactly one new epoch record.
//! A returned guard keeps the root lock alive for its full authority lifetime.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;
use std::path::Path;

use crate::sealed_digest::{DigestError, Sha256Digest};
use crate::sealed_root_lock::{SealedRootLease, SealedRootLockError};
use crate::sealed_store::{SealedStoreError, SensitiveBytes};
use crate::sealed_transaction::SealedTransactionError;
use crate::sealed_transaction_guard::put_idempotent_verified;

/// Maximum historical owner epochs validated during one acquisition.
pub const MAX_OWNER_EPOCH_RECORDS: usize = 1_000_000;
const OWNER_EPOCH_MAGIC: &str = "ELIOT-SEALED-OWNER-EPOCH-V1";
const OWNER_EPOCH_FORMAT_VERSION: u16 = 1;
const OWNER_EPOCH_FIELD_COUNT: usize = 5;
const ZERO_DIGEST_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Closed owner-epoch failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerEpochError {
    /// Windows owner-epoch adapter is unavailable on this platform.
    UnsupportedPlatform,
    /// Historical epoch filename, record, or ordering is malformed.
    ChainInvalid,
    /// At least one historical epoch is missing.
    ChainGap,
    /// A record belongs to another physical data-root identity.
    RootBindingMismatch,
    /// Record predecessor digest does not match the exact previous record.
    PredecessorMismatch,
    /// Epoch or history capacity is exhausted.
    EpochExhausted,
    /// Filesystem enumeration/readback failed.
    IoFailure,
    /// Windows CNG SHA-256 failed.
    Digest(DigestError),
    /// Exclusive data-root acquisition failed.
    RootLock(SealedRootLockError),
    /// DPAPI sealed-object operation failed.
    SealedStore(SealedStoreError),
    /// Idempotent transaction reconciliation failed.
    Transaction(SealedTransactionError),
}

impl OwnerEpochError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_OWNER_EPOCH_UNSUPPORTED_PLATFORM",
            Self::ChainInvalid => "SEALED_OWNER_EPOCH_CHAIN_INVALID",
            Self::ChainGap => "SEALED_OWNER_EPOCH_CHAIN_GAP",
            Self::RootBindingMismatch => "SEALED_OWNER_EPOCH_ROOT_BINDING_MISMATCH",
            Self::PredecessorMismatch => "SEALED_OWNER_EPOCH_PREDECESSOR_MISMATCH",
            Self::EpochExhausted => "SEALED_OWNER_EPOCH_EXHAUSTED",
            Self::IoFailure => "SEALED_OWNER_EPOCH_IO_FAILURE",
            Self::Digest(error) => error.code(),
            Self::RootLock(error) => error.code(),
            Self::SealedStore(error) => error.code(),
            Self::Transaction(error) => error.code(),
        }
    }
}

impl fmt::Display for OwnerEpochError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for OwnerEpochError {}

impl From<DigestError> for OwnerEpochError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

impl From<SealedRootLockError> for OwnerEpochError {
    fn from(error: SealedRootLockError) -> Self {
        Self::RootLock(error)
    }
}

impl From<SealedStoreError> for OwnerEpochError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

impl From<SealedTransactionError> for OwnerEpochError {
    fn from(error: SealedTransactionError) -> Self {
        Self::Transaction(error)
    }
}

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
    fn validate(&self) -> Result<(), OwnerEpochError> {
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

    fn encode(&self) -> Result<String, OwnerEpochError> {
        self.validate()?;
        Ok(format!(
            concat!(
                "{OWNER_EPOCH_MAGIC}\n",
                "format_version={}\n",
                "epoch={}\n",
                "previous_epoch={}\n",
                "previous_record_sha256={}\n",
                "root_binding_sha256={}\n"
            ),
            self.format_version,
            self.epoch,
            self.previous_epoch,
            self.previous_record_sha256,
            self.root_binding_sha256,
        ))
    }

    fn decode(bytes: &[u8]) -> Result<Self, OwnerEpochError> {
        let value = core::str::from_utf8(bytes).map_err(|_| OwnerEpochError::ChainInvalid)?;
        if !value.ends_with('\n') {
            return Err(OwnerEpochError::ChainInvalid);
        }
        let mut lines = value.lines();
        if lines.next() != Some(OWNER_EPOCH_MAGIC) {
            return Err(OwnerEpochError::ChainInvalid);
        }
        let mut fields = std::collections::BTreeMap::new();
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
            format_version: parse_u16(take(&mut fields, "format_version")?)?,
            epoch: parse_u64(take(&mut fields, "epoch")?)?,
            previous_epoch: parse_u64(take(&mut fields, "previous_epoch")?)?,
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

/// Process-local owner authority. It is deliberately non-cloneable.
pub struct OwnerEpochGuard {
    root_lease: SealedRootLease,
    record: OwnerEpochRecord,
    record_sha256: Sha256Digest,
    object_id: String,
    transaction_id: String,
}

impl OwnerEpochGuard {
    /// Acquires the data root and advances its monotone epoch exactly once.
    pub fn acquire(data_root: &Path) -> Result<Self, OwnerEpochError> {
        platform::acquire(data_root)
    }

    /// Current non-zero owner epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.record.epoch
    }

    /// Current physical-root binding digest.
    #[must_use]
    pub const fn root_binding_sha256(&self) -> Sha256Digest {
        self.record.root_binding_sha256
    }

    /// SHA-256 of exact current epoch-record plaintext.
    #[must_use]
    pub const fn record_sha256(&self) -> Sha256Digest {
        self.record_sha256
    }

    /// Immutable sealed epoch object identity.
    #[must_use]
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// Idempotent epoch transaction identity.
    #[must_use]
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    /// Whether the exact OS data-root lock remains held.
    #[must_use]
    pub fn root_lock_held(&self) -> bool {
        self.root_lease.is_held()
    }
}

impl fmt::Debug for OwnerEpochGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnerEpochGuard")
            .field("epoch", &self.epoch())
            .field("root_binding_sha256", &self.root_binding_sha256())
            .field("record_sha256", &self.record_sha256())
            .field("object_id", &self.object_id)
            .field("transaction_id", &self.transaction_id)
            .field("root_lock_held", &self.root_lock_held())
            .finish()
    }
}

fn object_id(epoch: u64) -> String {
    format!("owner-epoch-{epoch:020}")
}

fn transaction_id(epoch: u64) -> String {
    format!("owner-epoch-op-{epoch:020}")
}

fn parse_epoch_object_id(value: &str) -> Result<u64, OwnerEpochError> {
    let digits = value
        .strip_prefix("owner-epoch-")
        .ok_or(OwnerEpochError::ChainInvalid)?;
    if digits.len() != 20 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(OwnerEpochError::ChainInvalid);
    }
    let epoch = parse_u64(digits.to_owned())?;
    if epoch == 0 || object_id(epoch) != value {
        return Err(OwnerEpochError::ChainInvalid);
    }
    Ok(epoch)
}

fn take(
    fields: &mut std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<String, OwnerEpochError> {
    fields.remove(key).ok_or(OwnerEpochError::ChainInvalid)
}

fn parse_u64(value: String) -> Result<u64, OwnerEpochError> {
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(OwnerEpochError::ChainInvalid);
    }
    value
        .parse::<u64>()
        .map_err(|_| OwnerEpochError::ChainInvalid)
}

fn parse_u16(value: String) -> Result<u16, OwnerEpochError> {
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(OwnerEpochError::ChainInvalid);
    }
    value
        .parse::<u16>()
        .map_err(|_| OwnerEpochError::ChainInvalid)
}

#[cfg(not(windows))]
mod platform {
    use super::{OwnerEpochError, OwnerEpochGuard};
    use std::path::Path;

    pub(super) fn acquire(_data_root: &Path) -> Result<OwnerEpochGuard, OwnerEpochError> {
        Err(OwnerEpochError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        MAX_OWNER_EPOCH_RECORDS, OWNER_EPOCH_FORMAT_VERSION, OwnerEpochError,
        OwnerEpochGuard, OwnerEpochRecord, Sha256Digest, SensitiveBytes,
        ZERO_DIGEST_HEX, object_id, parse_epoch_object_id, put_idempotent_verified,
        transaction_id,
    };
    use crate::sealed_digest::sha256;
    use crate::sealed_root_lock::SealedRootLease;
    use crate::sealed_store::open_sealed;
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::path::Path;

    const SEALED_DIRECTORY: &str = "sealed-revisions";
    const SEALED_SUFFIX: &str = ".els-dpapi";
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    pub(super) fn acquire(data_root: &Path) -> Result<OwnerEpochGuard, OwnerEpochError> {
        let root_lease = SealedRootLease::acquire(data_root)?;
        if !root_lease.is_held() {
            return Err(OwnerEpochError::IoFailure);
        }
        let root_binding = root_binding(data_root)?;
        let records = discover_epoch_objects(data_root)?;
        let mut previous_epoch = 0_u64;
        let mut previous_digest = Sha256Digest::from_hex(ZERO_DIGEST_HEX)?;

        for (index, (epoch, epoch_object_id)) in records.iter().enumerate() {
            let expected_epoch = u64::try_from(index)
                .map_err(|_| OwnerEpochError::EpochExhausted)?
                .checked_add(1)
                .ok_or(OwnerEpochError::EpochExhausted)?;
            if *epoch != expected_epoch {
                return Err(OwnerEpochError::ChainGap);
            }
            let plaintext = open_sealed(data_root, epoch_object_id)?;
            let record = OwnerEpochRecord::decode(plaintext.expose())?;
            if record.epoch != *epoch
                || record.previous_epoch != previous_epoch
                || record.root_binding_sha256 != root_binding
            {
                return Err(if record.root_binding_sha256 != root_binding {
                    OwnerEpochError::RootBindingMismatch
                } else {
                    OwnerEpochError::PredecessorMismatch
                });
            }
            if record.previous_record_sha256 != previous_digest {
                return Err(OwnerEpochError::PredecessorMismatch);
            }
            let canonical = record.encode()?;
            if canonical.as_bytes() != plaintext.expose() {
                return Err(OwnerEpochError::ChainInvalid);
            }
            let digest = sha256(plaintext.expose())?;
            let transaction = transaction_id(*epoch);
            let receipt = put_idempotent_verified(
                data_root,
                &transaction,
                epoch_object_id,
                SensitiveBytes::new(canonical.into_bytes())?,
            )?;
            if receipt.object_id != *epoch_object_id {
                return Err(OwnerEpochError::ChainInvalid);
            }
            previous_epoch = *epoch;
            previous_digest = digest;
        }

        let next_epoch = previous_epoch
            .checked_add(1)
            .ok_or(OwnerEpochError::EpochExhausted)?;
        let record = OwnerEpochRecord {
            format_version: OWNER_EPOCH_FORMAT_VERSION,
            epoch: next_epoch,
            previous_epoch,
            previous_record_sha256: previous_digest,
            root_binding_sha256: root_binding,
        };
        let encoded = record.encode()?;
        let record_sha256 = sha256(encoded.as_bytes())?;
        let object_id = object_id(next_epoch);
        let transaction_id = transaction_id(next_epoch);
        let receipt = put_idempotent_verified(
            data_root,
            &transaction_id,
            &object_id,
            SensitiveBytes::new(encoded.into_bytes())?,
        )?;
        if receipt.object_id != object_id
            || receipt.operation_id != transaction_id
            || receipt.plaintext_bytes == 0
        {
            return Err(OwnerEpochError::ChainInvalid);
        }
        Ok(OwnerEpochGuard {
            root_lease,
            record,
            record_sha256,
            object_id,
            transaction_id,
        })
    }

    fn discover_epoch_objects(
        data_root: &Path,
    ) -> Result<BTreeMap<u64, String>, OwnerEpochError> {
        let directory = data_root.join(SEALED_DIRECTORY);
        if !directory.exists() {
            return Ok(BTreeMap::new());
        }
        let metadata = fs::symlink_metadata(&directory).map_err(|_| OwnerEpochError::IoFailure)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(OwnerEpochError::ChainInvalid);
        }
        let mut records = BTreeMap::new();
        for entry in fs::read_dir(&directory).map_err(|_| OwnerEpochError::IoFailure)? {
            let entry = entry.map_err(|_| OwnerEpochError::IoFailure)?;
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            let Some(epoch_id) = file_name.strip_suffix(SEALED_SUFFIX) else {
                continue;
            };
            if !epoch_id.starts_with("owner-epoch-") {
                continue;
            }
            let epoch = parse_epoch_object_id(epoch_id)?;
            if records.insert(epoch, epoch_id.to_owned()).is_some()
                || records.len() > MAX_OWNER_EPOCH_RECORDS
            {
                return Err(OwnerEpochError::ChainInvalid);
            }
        }
        Ok(records)
    }

    fn root_binding(data_root: &Path) -> Result<Sha256Digest, OwnerEpochError> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(data_root)
            .map_err(|_| OwnerEpochError::IoFailure)?;
        let metadata = file.metadata().map_err(|_| OwnerEpochError::IoFailure)?;
        if !metadata.is_dir()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(OwnerEpochError::RootBindingMismatch);
        }
        let canonical = fs::canonicalize(data_root).map_err(|_| OwnerEpochError::IoFailure)?;
        let mut binding = b"eliot-search/sealed-root-binding/v1\0".to_vec();
        binding.extend_from_slice(&metadata.volume_serial_number().unwrap_or(0).to_be_bytes());
        binding.extend_from_slice(&metadata.file_index().unwrap_or(0).to_be_bytes());
        for unit in canonical.as_os_str().encode_wide() {
            binding.extend_from_slice(&unit.to_le_bytes());
        }
        sha256(&binding).map_err(OwnerEpochError::from)
    }
}
