//! One complete representation-and-descriptor encoding.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_materializer::MaterializedText;
use search_source_registry::{CurrentSourceRecord, PreparationSettings};

use super::wire::{MAGIC, VERSION, encoded_size, put_usize};
use super::{PreparationLimits, check};
use crate::{PreparationError, UnitSpan};

pub(super) fn encode(
    source: &CurrentSourceRecord,
    settings: PreparationSettings,
    product: &MaterializedText,
    units: &[UnitSpan],
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, PreparationError> {
    check(deadline, cancelled)?;
    let size = encoded_size(product.text().len(), units.len())?;
    if size > limits.max_encoded_bytes {
        return Err(PreparationError::EncodedTooLarge);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(size)
        .map_err(|_| PreparationError::AllocationFailed)?;
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(source.source_namespace_id.as_bytes());
    out.extend_from_slice(source.source_id.as_bytes());
    out.extend_from_slice(&source.registration_revision.to_le_bytes());
    out.extend_from_slice(&source.object_ref.format_version.to_le_bytes());
    out.push(source.object_ref.kind.tag());
    out.extend_from_slice(source.object_ref.digest.as_bytes());
    out.extend_from_slice(&source.object_ref.byte_length.to_le_bytes());
    out.extend_from_slice(&settings.utf8_version.to_le_bytes());
    out.extend_from_slice(&settings.line_unitizer_version.to_le_bytes());
    out.extend_from_slice(&settings.preferred_unit_bytes.to_le_bytes());
    out.extend_from_slice(&settings.max_unit_bytes.to_le_bytes());
    put_usize(&mut out, product.text().len())?;
    put_usize(&mut out, units.len())?;
    for chunk in product.text().as_bytes().chunks(8192) {
        check(deadline, cancelled)?;
        out.extend_from_slice(chunk);
    }
    for unit in units {
        check(deadline, cancelled)?;
        put_usize(&mut out, unit.source_start)?;
        put_usize(&mut out, unit.source_end)?;
        out.extend_from_slice(&unit.logical_line_start.to_le_bytes());
        out.extend_from_slice(&unit.logical_line_end.to_le_bytes());
        out.push(u8::from(unit.starts_at_line_boundary));
        out.push(u8::from(unit.ends_at_line_boundary));
    }
    check(deadline, cancelled)?;
    Ok(out)
}
