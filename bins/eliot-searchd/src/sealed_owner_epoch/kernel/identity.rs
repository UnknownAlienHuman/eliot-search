//! Fixed-width owner-epoch object and transaction identities.

use super::spec::{MAX_OWNER_EPOCH_RECORDS, OwnerEpochError};

pub(super) fn object_id(epoch: u64) -> String {
    format!("owner-epoch-{epoch:020}")
}

pub(super) fn transaction_id(epoch: u64) -> String {
    format!("owner-epoch-op-{epoch:020}")
}

pub(super) fn parse_epoch_object_id(
    value: &str,
) -> Result<u64, OwnerEpochError> {
    let digits = value
        .strip_prefix("owner-epoch-")
        .ok_or(OwnerEpochError::ChainInvalid)?;
    if digits.len() != 20 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(OwnerEpochError::ChainInvalid);
    }
    let epoch = digits
        .parse::<u64>()
        .map_err(|_| OwnerEpochError::ChainInvalid)?;
    if epoch == 0 || object_id(epoch) != value {
        return Err(OwnerEpochError::ChainInvalid);
    }
    Ok(epoch)
}

pub(super) const fn require_epoch_capacity(
    records: usize,
) -> Result<(), OwnerEpochError> {
    if records >= MAX_OWNER_EPOCH_RECORDS {
        Err(OwnerEpochError::EpochExhausted)
    } else {
        Ok(())
    }
}
