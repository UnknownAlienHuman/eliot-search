//! Receipt-free and receipt-bound exact UTF-8 materialization.

use super::model::{
    MaterializationLimits, MaterializationReceipt, MaterializedRevision, MaterializedText,
    RetainedRevision,
};
use super::scan::{check_byte_limits, reject_binary_controls, scan_lines};
use crate::error::MaterializationError;

/// Prepares exact UTF-8 bytes without manufacturing a revision or receipt.
///
/// This is a pure transform. A storage adapter must independently establish the
/// provenance and integrity of bytes before using its output as search evidence.
pub fn materialize_utf8(
    bytes: Vec<u8>,
    limits: MaterializationLimits,
) -> Result<MaterializedText, MaterializationError> {
    let limits = limits.validate()?;
    check_byte_limits(bytes.len(), limits)?;
    reject_binary_controls(&bytes)?;
    let text = String::from_utf8(bytes).map_err(|_| MaterializationError::InvalidUtf8)?;
    let (lines, line_endings) = scan_lines(text.as_bytes(), limits)?;
    Ok(MaterializedText {
        text,
        lines,
        line_endings,
    })
}

/// Materializes a receipt-bound retained revision without normalization.
///
/// The supplied digest and revision receipt are retained, not invented or
/// cryptographically verified here. Storage readback owns those checks.
pub fn materialize(
    input: RetainedRevision,
    limits: MaterializationLimits,
) -> Result<MaterializedRevision, MaterializationError> {
    let limits = limits.validate()?;
    if input.is_empty() {
        return Err(MaterializationError::EmptyInput);
    }
    check_byte_limits(input.len(), limits)?;
    let exact_len = u64::try_from(input.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    if input.byte_count != exact_len {
        return Err(MaterializationError::InputLengthMismatch);
    }
    let content_digest = input
        .content_digest
        .ok_or(MaterializationError::MissingContentDigest)?;
    let revision_receipt = input
        .revision_receipt
        .ok_or(MaterializationError::MissingRevisionReceipt)?;
    let MaterializedText {
        text,
        lines,
        line_endings,
    } = materialize_utf8(input.bytes, limits)?;
    let line_count =
        u64::try_from(lines.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    let receipt = MaterializationReceipt {
        source_id: input.source_id.clone(),
        revision: input.revision,
        content_digest,
        input_bytes: exact_len,
        output_bytes: exact_len,
        line_count,
        line_endings,
        revision_receipt,
    };
    Ok(MaterializedRevision {
        source_id: input.source_id,
        revision: input.revision,
        content_digest,
        text,
        lines,
        receipt,
    })
}
