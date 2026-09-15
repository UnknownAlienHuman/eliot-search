//! Read-only durable transaction status and binding inspection.

use std::path::Path;

use super::codec::{read_intent, read_receipt};
use super::io::{ensure_transaction_directory, metadata_path};
use super::super::super::model::{
    TransactionBinding, TransactionObservation, TransactionStatus,
};
use super::super::super::spec::{
    SealedTransactionError, validate_operation_id,
};

pub(crate) fn transaction_status(
    data_root: &Path,
    operation_id: &str,
) -> Result<TransactionStatus, SealedTransactionError> {
    Ok(inspect_transaction(data_root, operation_id)?.status)
}

pub(crate) fn inspect_transaction(
    data_root: &Path,
    operation_id: &str,
) -> Result<TransactionObservation, SealedTransactionError> {
    validate_operation_id(operation_id)?;
    let directory = ensure_transaction_directory(data_root, false)?;
    let intent_path = metadata_path(&directory, operation_id, "intent");
    let receipt_path = metadata_path(&directory, operation_id, "receipt");
    let intent_exists = intent_path.exists();
    let receipt_exists = receipt_path.exists();
    match (intent_exists, receipt_exists) {
        (false, false) => Ok(TransactionObservation {
            status: TransactionStatus::Absent,
            binding: None,
        }),
        (true, false) => {
            let intent = read_intent(&intent_path)?;
            Ok(TransactionObservation {
                status: TransactionStatus::Prepared,
                binding: Some(TransactionBinding {
                    operation_id: intent.operation_id,
                    object_id: intent.object_id,
                    plaintext_bytes: intent.plaintext_bytes,
                    plaintext_sha256: intent.plaintext_sha256,
                    ciphertext_bytes: None,
                }),
            })
        }
        (false, true) => {
            let receipt = read_receipt(&receipt_path)?;
            Ok(TransactionObservation {
                status: TransactionStatus::Committed,
                binding: Some(TransactionBinding {
                    operation_id: receipt.operation_id,
                    object_id: receipt.object_id,
                    plaintext_bytes: receipt.plaintext_bytes,
                    plaintext_sha256: receipt.plaintext_sha256,
                    ciphertext_bytes: Some(receipt.ciphertext_bytes),
                }),
            })
        }
        (true, true) => {
            let intent = read_intent(&intent_path)?;
            let receipt = read_receipt(&receipt_path)?;
            if intent.operation_id == receipt.operation_id
                && intent.object_id == receipt.object_id
                && intent.plaintext_bytes == receipt.plaintext_bytes
                && intent.plaintext_sha256 == receipt.plaintext_sha256
            {
                Ok(TransactionObservation {
                    status: TransactionStatus::CommittedCleanupPending,
                    binding: Some(TransactionBinding {
                        operation_id: receipt.operation_id,
                        object_id: receipt.object_id,
                        plaintext_bytes: receipt.plaintext_bytes,
                        plaintext_sha256: receipt.plaintext_sha256,
                        ciphertext_bytes: Some(receipt.ciphertext_bytes),
                    }),
                })
            } else {
                Ok(TransactionObservation {
                    status: TransactionStatus::Conflicted,
                    binding: None,
                })
            }
        }
    }
}
