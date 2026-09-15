//! Explicit fail-closed non-Windows platform boundary.

use std::path::Path;

use crate::sealed_store::SensitiveBytes;

use super::super::model::{
    SealedTransactionReceipt, TransactionObservation, TransactionStatus,
};
use super::super::spec::SealedTransactionError;

pub(crate) fn put_idempotent(
    _data_root: &Path,
    _operation_id: &str,
    _object_id: &str,
    _plaintext: &SensitiveBytes,
) -> Result<SealedTransactionReceipt, SealedTransactionError> {
    Err(SealedTransactionError::UnsupportedPlatform)
}

pub(crate) fn transaction_status(
    _data_root: &Path,
    _operation_id: &str,
) -> Result<TransactionStatus, SealedTransactionError> {
    Err(SealedTransactionError::UnsupportedPlatform)
}

pub(crate) fn inspect_transaction(
    _data_root: &Path,
    _operation_id: &str,
) -> Result<TransactionObservation, SealedTransactionError> {
    Err(SealedTransactionError::UnsupportedPlatform)
}
