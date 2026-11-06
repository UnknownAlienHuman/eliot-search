//! Pure complete-artifact preparation and borrowed saved-artifact acceptance.

use std::fmt;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_materializer::{LineSpan, MaterializationLimits, materialize_utf8};
use search_source_registry::{
    CurrentSourceRecord, MAX_SOURCE_OBJECT_BYTES, ObjectKind, PreparationSettings,
    SOURCE_OBJECT_FORMAT_VERSION,
};

use crate::{PreparationError, UnitSpan};

mod decode;
mod encode;
mod wire;

/// Fail-only ceilings, excluded from the successful artifact's transform identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparationLimits {
    /// Maximum retained source bytes, checked before hashing or transformation.
    pub max_input_bytes: usize,
    /// Maximum unchanged representation bytes.
    pub max_output_bytes: usize,
    /// Maximum logical lines and their bounded in-memory inventory.
    pub max_lines: usize,
    /// Maximum ordered descriptors and their bounded in-memory inventory.
    pub max_units: usize,
    /// Maximum complete serialized artifact bytes.
    pub max_encoded_bytes: usize,
}

impl PreparationLimits {
    fn validate(self) -> Result<(), PreparationError> {
        if self.max_input_bytes == 0
            || self.max_output_bytes == 0
            || self.max_lines == 0
            || self.max_units == 0
            || self.max_encoded_bytes == 0
        {
            return Err(PreparationError::InvalidLimits);
        }
        for bytes in [
            self.max_lines.checked_mul(size_of::<LineSpan>()),
            self.max_units.checked_mul(size_of::<UnitSpan>()),
        ] {
            bytes
                .filter(|size| *size <= isize::MAX.unsigned_abs())
                .ok_or(PreparationError::OffsetOverflow)?;
        }
        Ok(())
    }

    const fn check_text_length(self, length: usize) -> Result<(), PreparationError> {
        if length > self.max_input_bytes {
            return Err(PreparationError::InputTooLarge);
        }
        if length > self.max_output_bytes {
            return Err(PreparationError::OutputTooLarge);
        }
        Ok(())
    }
}

/// A checked view borrowing the saved representation, with bounded descriptors.
///
/// This value contains no object address. The caller keeps the verified object
/// buffer alive and owns its current-outcome and returned-object binding.
pub struct PreparedUtf8<'a> {
    text: &'a str,
    units: Vec<UnitSpan>,
}

impl PreparedUtf8<'_> {
    /// Borrows the exact unchanged representation without copying its text.
    #[must_use]
    pub const fn text(&self) -> &str {
        self.text
    }

    /// Borrows ordered descriptors; ordinal is position within this artifact.
    #[must_use]
    pub const fn units(&self) -> &[UnitSpan] {
        self.units.as_slice()
    }
}

impl fmt::Debug for PreparedUtf8<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedUtf8")
            .field("text_bytes", &self.text.len())
            .field("unit_count", &self.units.len())
            .finish()
    }
}

/// Produces one complete encoding from the exact retained input allocation.
///
/// The encoding binds source namespace/identity/registration, the input Source
/// `ObjectRef` and output-affecting settings. It contains representation bytes once
/// and ordered ranges, never its own subsequently returned `ObjectRef`.
/// Empty supported input produces a complete artifact with zero units.
///
/// # Errors
///
/// Returns distinct binary/UTF-8 refusals, exceeded execution ceilings, invalid
/// settings/source binding, input integrity failure, allocation/coordinate
/// failure, cancellation or expiration of the original deadline.
pub fn prepare_utf8(
    source: &CurrentSourceRecord,
    verified_bytes: Vec<u8>,
    settings: PreparationSettings,
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, PreparationError> {
    validate(source, settings, limits, deadline, cancelled)?;
    if u64::try_from(verified_bytes.len()).ok() != Some(source.object_ref.byte_length) {
        return Err(PreparationError::BindingMismatch);
    }
    verify_digest(source, &verified_bytes, deadline, cancelled)?;
    let product = materialize_utf8(
        verified_bytes,
        MaterializationLimits {
            max_input_bytes: limits.max_input_bytes,
            max_output_bytes: limits.max_output_bytes,
            max_lines: limits.max_lines,
        },
        deadline,
        cancelled,
    )?;
    let units = crate::layout::unitize_text(&product, settings, limits, deadline, cancelled)?;
    encode::encode(
        source, settings, &product, &units, limits, deadline, cancelled,
    )
}

/// Accepts a saved complete artifact against its expected source and settings.
///
/// The caller first verifies the Prepared `ObjectRef` through the shared object
/// reader. This function validates saved bytes directly; it does not rebuild an
/// expected materialization or unit inventory. The result borrows only artifact.
///
/// # Errors
///
/// Returns explicit framing/binding/integrity failures, invalid settings or
/// exceeded execution ceilings, allocation/coordinate failures, cancellation or
/// expiration of the original deadline.
pub fn decode_prepared_utf8<'a>(
    artifact: &'a [u8],
    expected_source: &CurrentSourceRecord,
    expected_settings: &PreparationSettings,
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PreparedUtf8<'a>, PreparationError> {
    validate(
        expected_source,
        *expected_settings,
        limits,
        deadline,
        cancelled,
    )?;
    decode::decode(
        artifact,
        expected_source,
        *expected_settings,
        limits,
        deadline,
        cancelled,
    )
}

fn validate(
    source: &CurrentSourceRecord,
    settings: PreparationSettings,
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), PreparationError> {
    check(deadline, cancelled)?;
    limits.validate()?;
    if !settings.is_supported() {
        return Err(PreparationError::InvalidSettings);
    }
    if source.registration_revision == 0
        || source.object_ref.kind != ObjectKind::Source
        || source.object_ref.format_version != SOURCE_OBJECT_FORMAT_VERSION
        || source.object_ref.byte_length > MAX_SOURCE_OBJECT_BYTES
    {
        return Err(PreparationError::BindingMismatch);
    }
    let length = usize::try_from(source.object_ref.byte_length)
        .map_err(|_| PreparationError::OffsetOverflow)?;
    limits.check_text_length(length)
}

pub fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), PreparationError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(PreparationError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(PreparationError::Deadline);
    }
    Ok(())
}

fn verify_digest(
    source: &CurrentSourceRecord,
    bytes: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), PreparationError> {
    let mut hasher = blake3::Hasher::new();
    for chunk in bytes.chunks(8192) {
        check(deadline, cancelled)?;
        hasher.update(chunk);
    }
    check(deadline, cancelled)?;
    if hasher.finalize().as_bytes() != source.object_ref.digest.as_bytes() {
        return Err(PreparationError::IntegrityFailure);
    }
    Ok(())
}
