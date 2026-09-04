//! Final exact readback guard for sealed-object transaction receipts.
//!
//! The transaction journal is content-free, so its byte counts are never
//! trusted by themselves. Every successful create, reconciliation, or replay is
//! checked against an authenticated DPAPI object readback before it may escape
//! the concrete adapter boundary.

use std::path::Path;

use crate::sealed_store::{SensitiveBytes, verify_sealed};
use crate::sealed_transaction::{
    SealedTransactionError, SealedTransactionReceipt, put_idempotent,
};

/// Executes an idempotent put and binds its terminal receipt to the exact
/// authenticated sealed-object readback.
pub fn put_idempotent_verified(
    data_root: &Path,
    operation_id: &str,
    object_id: &str,
    plaintext: SensitiveBytes,
) -> Result<SealedTransactionReceipt, SealedTransactionError> {
    let receipt = put_idempotent(
        data_root,
        operation_id,
        object_id,
        plaintext,
    )?;
    let observed = verify_sealed(data_root, object_id)?;
    if receipt.operation_id != operation_id
        || receipt.object_id != object_id
        || receipt.plaintext_bytes != observed.plaintext_bytes
        || receipt.ciphertext_bytes != observed.ciphertext_bytes
        || !receipt.sealed_readback_verified
        || !receipt.receipt_readback_verified
        || !observed.authenticated
    {
        return Err(SealedTransactionError::ReceiptConflict);
    }
    Ok(receipt)
}
