//! Stable sealed-transaction operations delegating to the platform owner.

use std::path::Path;

use crate::sealed_store::SensitiveBytes;

use super::model::{
    SealedTransactionReceipt, TransactionObservation, TransactionStatus,
};
use super::platform;
use super::spec::SealedTransactionError;

/// Creates or exactly reconciles one immutable sealed object.
pub fn put_idempotent(
    data_root: &Path,
    operation_id: &str,
    object_id: &str,
    plaintext: &SensitiveBytes,
) -> Result<SealedTransactionReceipt, SealedTransactionError> {
    platform::put_idempotent(data_root, operation_id, object_id, plaintext)
}

/// Reads content-free durable transaction state.
pub fn transaction_status(
    data_root: &Path,
    operation_id: &str,
) -> Result<TransactionStatus, SealedTransactionError> {
    platform::transaction_status(data_root, operation_id)
}

/// Inspects one operation identity and returns its durable expected binding.
pub fn inspect_transaction(
    data_root: &Path,
    operation_id: &str,
) -> Result<TransactionObservation, SealedTransactionError> {
    platform::inspect_transaction(data_root, operation_id)
}
