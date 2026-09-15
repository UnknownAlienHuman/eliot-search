//! Exact idempotent put and unknown-outcome reconciliation state machine.

use std::fs;
use std::path::Path;

use crate::sealed_digest::sha256;
use crate::sealed_store::{
    SealReceipt, SealedStoreError, SensitiveBytes, open_sealed,
    seal_immutable, verify_sealed,
};

use super::codec::{
    encode_intent, encode_receipt, read_intent, read_receipt,
};
use super::io::{
    acquire_operation_lock, ensure_transaction_directory, metadata_path,
    write_once,
};
use super::model::{Intent, Receipt};
use super::super::super::model::{
    PutDisposition, SealedTransactionReceipt,
};
use super::super::super::spec::{
    SealedTransactionError, validate_operation_id,
};

pub(crate) fn put_idempotent(
    data_root: &Path,
    operation_id: &str,
    object_id: &str,
    plaintext: &SensitiveBytes,
) -> Result<SealedTransactionReceipt, SealedTransactionError> {
    validate_operation_id(operation_id)?;
    let directory = ensure_transaction_directory(data_root, true)?;
    let _lock = acquire_operation_lock(&directory, operation_id)?;
    let expected_digest = sha256(plaintext.expose())?;
    let intent = Intent {
        operation_id: operation_id.to_owned(),
        object_id: object_id.to_owned(),
        plaintext_bytes: u64::try_from(plaintext.len())
            .map_err(|_| SealedTransactionError::IntentConflict)?,
        plaintext_sha256: expected_digest,
    };
    let intent_path = metadata_path(&directory, operation_id, "intent");
    let receipt_path = metadata_path(&directory, operation_id, "receipt");

    if receipt_path.exists() {
        if intent_path.exists() {
            return Err(SealedTransactionError::ReceiptConflict);
        }
        let receipt = read_receipt(&receipt_path)?;
        require_receipt_matches(&receipt, &intent)?;
        require_exact_plaintext(data_root, object_id, plaintext)?;
        return Ok(public_receipt(receipt, PutDisposition::Replay));
    }

    let disposition = if intent_path.exists() {
        let observed = read_intent(&intent_path)?;
        if observed != intent {
            return Err(SealedTransactionError::IntentConflict);
        }
        match open_sealed(data_root, object_id) {
            Ok(existing) => {
                if existing.expose() != plaintext.expose() {
                    return Err(
                        SealedTransactionError::ReplayContentMismatch,
                    );
                }
                PutDisposition::Reconciled
            }
            Err(SealedStoreError::ObjectNotFound) => PutDisposition::Created,
            Err(error) => {
                return Err(SealedTransactionError::SealedStore(error));
            }
        }
    } else {
        match open_sealed(data_root, object_id) {
            Ok(_) => return Err(SealedTransactionError::ObjectConflict),
            Err(SealedStoreError::ObjectNotFound) => {}
            Err(error) => {
                return Err(SealedTransactionError::SealedStore(error));
            }
        }
        write_once(&directory, &intent_path, encode_intent(&intent).as_bytes())?;
        let readback = read_intent(&intent_path)?;
        if readback != intent {
            return Err(SealedTransactionError::ReadbackMismatch);
        }
        PutDisposition::Created
    };

    let seal = if disposition == PutDisposition::Reconciled {
        let verified = verify_sealed(data_root, object_id)?;
        SealReceipt {
            object_id: verified.object_id,
            plaintext_bytes: verified.plaintext_bytes,
            ciphertext_bytes: verified.ciphertext_bytes,
            format_version: verified.format_version,
            protection_scope: verified.protection_scope,
            readback_verified: verified.authenticated,
        }
    } else {
        seal_immutable(data_root, object_id, plaintext)?
    };
    if seal.plaintext_bytes != intent.plaintext_bytes {
        return Err(SealedTransactionError::ReadbackMismatch);
    }
    let receipt = Receipt {
        operation_id: operation_id.to_owned(),
        object_id: object_id.to_owned(),
        plaintext_bytes: seal.plaintext_bytes,
        plaintext_sha256: expected_digest,
        ciphertext_bytes: seal.ciphertext_bytes,
    };
    write_once(
        &directory,
        &receipt_path,
        encode_receipt(&receipt).as_bytes(),
    )?;
    let receipt_readback = read_receipt(&receipt_path)?;
    if receipt_readback != receipt {
        return Err(SealedTransactionError::ReadbackMismatch);
    }
    fs::remove_file(&intent_path)
        .map_err(|_| SealedTransactionError::IoFailure)?;
    if intent_path.exists() {
        return Err(SealedTransactionError::ReadbackMismatch);
    }
    Ok(public_receipt(receipt, disposition))
}

fn public_receipt(
    receipt: Receipt,
    disposition: PutDisposition,
) -> SealedTransactionReceipt {
    SealedTransactionReceipt {
        operation_id: receipt.operation_id,
        object_id: receipt.object_id,
        plaintext_bytes: receipt.plaintext_bytes,
        plaintext_sha256: receipt.plaintext_sha256,
        ciphertext_bytes: receipt.ciphertext_bytes,
        disposition,
        sealed_readback_verified: true,
        receipt_readback_verified: true,
    }
}

fn require_receipt_matches(
    receipt: &Receipt,
    intent: &Intent,
) -> Result<(), SealedTransactionError> {
    if receipt.operation_id != intent.operation_id
        || receipt.object_id != intent.object_id
        || receipt.plaintext_bytes != intent.plaintext_bytes
        || receipt.plaintext_sha256 != intent.plaintext_sha256
    {
        return Err(SealedTransactionError::ReceiptConflict);
    }
    Ok(())
}

fn require_exact_plaintext(
    data_root: &Path,
    object_id: &str,
    expected: &SensitiveBytes,
) -> Result<(), SealedTransactionError> {
    let observed = open_sealed(data_root, object_id)?;
    if observed.expose() != expected.expose() {
        return Err(SealedTransactionError::ReplayContentMismatch);
    }
    Ok(())
}
