//! Exact durable intent/receipt metadata codec.

use std::collections::BTreeMap;
use std::path::Path;

use crate::sealed_digest::Sha256Digest;

use super::io::read_small;
use super::model::{Intent, Receipt};
use super::super::super::spec::{
    SealedTransactionError, validate_operation_id,
};

pub(super) const INTENT_MAGIC: &str = "ELIOT-SEALED-INTENT-V1";
pub(super) const RECEIPT_MAGIC: &str = "ELIOT-SEALED-RECEIPT-V1";

#[must_use]
pub(super) fn encode_intent(intent: &Intent) -> String {
    format!(
        concat!(
            "{}\noperation={}\nobject={}\nplaintext_bytes={}\n",
            "plaintext_sha256={}\n"
        ),
        INTENT_MAGIC,
        intent.operation_id,
        intent.object_id,
        intent.plaintext_bytes,
        intent.plaintext_sha256.to_hex(),
    )
}

pub(super) fn read_intent(path: &Path) -> Result<Intent, SealedTransactionError> {
    let fields = parse_metadata(&read_small(path)?, INTENT_MAGIC)?;
    let operation_id = field(&fields, "operation")?.to_owned();
    validate_operation_id(&operation_id)?;
    let object_id = field(&fields, "object")?.to_owned();
    let plaintext_bytes = parse_u64(field(&fields, "plaintext_bytes")?)?;
    let plaintext_sha256 =
        Sha256Digest::from_hex(field(&fields, "plaintext_sha256")?)
            .map_err(|_| SealedTransactionError::IntentConflict)?;
    if fields.len() != 4 || plaintext_bytes == 0 {
        return Err(SealedTransactionError::IntentConflict);
    }
    Ok(Intent {
        operation_id,
        object_id,
        plaintext_bytes,
        plaintext_sha256,
    })
}

#[must_use]
pub(super) fn encode_receipt(receipt: &Receipt) -> String {
    format!(
        concat!(
            "{}\noperation={}\nobject={}\n",
            "plaintext_bytes={}\nplaintext_sha256={}\n",
            "ciphertext_bytes={}\n"
        ),
        RECEIPT_MAGIC,
        receipt.operation_id,
        receipt.object_id,
        receipt.plaintext_bytes,
        receipt.plaintext_sha256.to_hex(),
        receipt.ciphertext_bytes,
    )
}

pub(super) fn read_receipt(path: &Path) -> Result<Receipt, SealedTransactionError> {
    let fields = parse_metadata(&read_small(path)?, RECEIPT_MAGIC)?;
    let operation_id = field(&fields, "operation")?.to_owned();
    validate_operation_id(&operation_id)?;
    let object_id = field(&fields, "object")?.to_owned();
    let plaintext_bytes = parse_u64(field(&fields, "plaintext_bytes")?)?;
    let plaintext_sha256 =
        Sha256Digest::from_hex(field(&fields, "plaintext_sha256")?)
            .map_err(|_| SealedTransactionError::ReceiptConflict)?;
    let ciphertext_bytes = parse_u64(field(&fields, "ciphertext_bytes")?)?;
    if fields.len() != 5 || plaintext_bytes == 0 || ciphertext_bytes == 0 {
        return Err(SealedTransactionError::ReceiptConflict);
    }
    Ok(Receipt {
        operation_id,
        object_id,
        plaintext_bytes,
        plaintext_sha256,
        ciphertext_bytes,
    })
}

pub(super) fn parse_metadata(
    value: &str,
    magic: &str,
) -> Result<BTreeMap<String, String>, SealedTransactionError> {
    if !value.ends_with('\n') {
        return Err(SealedTransactionError::ReadbackMismatch);
    }
    let mut lines = value.lines();
    if lines.next() != Some(magic) {
        return Err(SealedTransactionError::ReadbackMismatch);
    }
    let mut fields = BTreeMap::new();
    for line in lines {
        let Some((key, field_value)) = line.split_once('=') else {
            return Err(SealedTransactionError::ReadbackMismatch);
        };
        if key.is_empty()
            || field_value.is_empty()
            || fields
                .insert(key.to_owned(), field_value.to_owned())
                .is_some()
        {
            return Err(SealedTransactionError::ReadbackMismatch);
        }
    }
    Ok(fields)
}

pub(super) fn field<'a>(
    fields: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, SealedTransactionError> {
    fields
        .get(key)
        .map(String::as_str)
        .ok_or(SealedTransactionError::ReadbackMismatch)
}

pub(super) fn parse_u64(value: &str) -> Result<u64, SealedTransactionError> {
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(SealedTransactionError::ReadbackMismatch);
    }
    value
        .parse::<u64>()
        .map_err(|_| SealedTransactionError::ReadbackMismatch)
}
