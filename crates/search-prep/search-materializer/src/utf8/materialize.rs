//! The selected exact UTF-8 transform.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::model::{MaterializationLimits, MaterializedText};
use super::scan::{check, reject_binary_controls, scan_lines};
use crate::MaterializationError;

/// Moves verified retained bytes into unchanged UTF-8 text and exact line ranges.
///
/// Empty input succeeds with zero lines. The caller owns retained-object
/// verification; this transform performs no source-path or object I/O.
///
/// # Errors
///
/// Returns a content refusal for binary controls or invalid UTF-8, or an
/// operation failure for invalid/exceeded ceilings, allocation/coordinate
/// failure, cancellation, or expiration of the original deadline.
pub fn materialize_utf8(
    bytes: Vec<u8>,
    limits: MaterializationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<MaterializedText, MaterializationError> {
    check(deadline, cancelled)?;
    limits.validate()?;
    if bytes.len() > limits.max_input_bytes {
        return Err(MaterializationError::InputTooLarge);
    }
    if bytes.len() > limits.max_output_bytes {
        return Err(MaterializationError::OutputTooLarge);
    }
    reject_binary_controls(&bytes, deadline, cancelled)?;
    let text = String::from_utf8(bytes).map_err(|_| MaterializationError::InvalidUtf8)?;
    check(deadline, cancelled)?;
    let lines = scan_lines(text.as_bytes(), limits, deadline, cancelled)?;
    Ok(MaterializedText { text, lines })
}
