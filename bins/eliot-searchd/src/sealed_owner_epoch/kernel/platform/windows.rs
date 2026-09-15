//! Windows owner-epoch chain validation, reconciliation and append.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use crate::sealed_digest::{Sha256Digest, sha256};
use crate::sealed_root_lock::SealedRootLease;
use crate::sealed_store::{SensitiveBytes, open_sealed};
use crate::sealed_transaction_guard::put_idempotent_verified;

use super::super::codec::OwnerEpochRecord;
use super::super::identity::{
    object_id, parse_epoch_object_id, require_epoch_capacity, transaction_id,
};
use super::super::model::OwnerEpochGuard;
use super::super::spec::{
    MAX_OWNER_EPOCH_RECORDS, OWNER_EPOCH_FORMAT_VERSION, OwnerEpochError,
    SEALED_DIRECTORY, SEALED_SUFFIX, ZERO_DIGEST_HEX,
};

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// Reads and authenticates only the sealed epoch head.
pub(super) fn latest_head(
    data_root: &Path,
) -> Result<Option<OwnerEpochRecord>, OwnerEpochError> {
    let records = discover_epoch_objects(data_root)?;
    let Some((epoch, epoch_object_id)) = records.last_key_value() else {
        return Ok(None);
    };
    let plaintext = open_sealed(data_root, epoch_object_id)?;
    let record = OwnerEpochRecord::decode(plaintext.expose())?;
    if record.epoch != *epoch {
        return Err(OwnerEpochError::ChainInvalid);
    }
    Ok(Some(record))
}

pub(super) fn acquire(
    data_root: &Path,
) -> Result<OwnerEpochGuard, OwnerEpochError> {
    let root_lease = SealedRootLease::acquire(data_root)?;
    if !root_lease.is_held() {
        return Err(OwnerEpochError::IoFailure);
    }
    let root_binding = root_binding(data_root)?;
    let records = discover_epoch_objects(data_root)?;
    require_epoch_capacity(records.len())?;
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
            return Err(if record.root_binding_sha256 == root_binding {
                OwnerEpochError::PredecessorMismatch
            } else {
                OwnerEpochError::RootBindingMismatch
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
            &SensitiveBytes::new(canonical.into_bytes())?,
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
        &SensitiveBytes::new(encoded.into_bytes())?,
    )?;
    if receipt.object_id != object_id
        || receipt.operation_id != transaction_id
        || receipt.plaintext_bytes == 0
    {
        return Err(OwnerEpochError::ChainInvalid);
    }
    Ok(OwnerEpochGuard::new(
        root_lease,
        record,
        record_sha256,
        object_id,
        transaction_id,
    ))
}

fn discover_epoch_objects(
    data_root: &Path,
) -> Result<BTreeMap<u64, String>, OwnerEpochError> {
    let directory = data_root.join(SEALED_DIRECTORY);
    if !directory.exists() {
        return Ok(BTreeMap::new());
    }
    let metadata = fs::symlink_metadata(&directory)
        .map_err(|_| OwnerEpochError::IoFailure)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(OwnerEpochError::ChainInvalid);
    }
    let mut records = BTreeMap::new();
    for entry in fs::read_dir(&directory)
        .map_err(|_| OwnerEpochError::IoFailure)?
    {
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
    let metadata = file
        .metadata()
        .map_err(|_| OwnerEpochError::IoFailure)?;
    if !metadata.is_dir()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(OwnerEpochError::RootBindingMismatch);
    }
    let canonical = fs::canonicalize(data_root)
        .map_err(|_| OwnerEpochError::IoFailure)?;
    let observed = eliot_searchd::native_file::observe(&file)
        .map_err(|_| OwnerEpochError::IoFailure)?;
    let mut binding = b"eliot-search/sealed-root-binding/v1\0".to_vec();
    binding.extend_from_slice(&observed.legacy_identity_bytes());
    for unit in canonical.as_os_str().encode_wide() {
        binding.extend_from_slice(&unit.to_le_bytes());
    }
    sha256(&binding).map_err(OwnerEpochError::from)
}
