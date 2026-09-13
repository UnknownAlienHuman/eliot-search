//! Deterministic content-free materialization encoding.

use super::digest::encoding_tag;
use super::model::{CanonicalMaterializationBytes, MaterializationProduct, MaterializationWarning};
use crate::MaterializationError;
use crate::maps::{LossKind, SegmentRelation};

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Serializes descriptors, maps and warnings deterministically with source
/// content referenced by digest rather than embedded.
pub fn canonicalize_materialization(
    product: &MaterializationProduct,
) -> Result<CanonicalMaterializationBytes, MaterializationError> {
    let mut out = Vec::new();
    out.extend_from_slice(b"ELIOT-MAT-V1");
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(product.profile_id().as_bytes());
    out.extend_from_slice(product.representation_id().as_bytes());
    out.extend_from_slice(product.input_digest().as_bytes());
    out.extend_from_slice(product.canonical_digest().as_bytes());
    out.extend_from_slice(product.coordinate_digest().as_bytes());
    out.extend_from_slice(product.loss_digest().as_bytes());
    out.push(encoding_tag(product.encoding()));
    out.push(product.assurance().ceiling().rank());
    let segments = u64::try_from(product.maps().coordinate_map().segments().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let losses = u64::try_from(product.maps().loss_map().records().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let warnings = u64::try_from(product.warnings().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    push_u64(&mut out, segments);
    push_u64(&mut out, losses);
    push_u64(&mut out, warnings);
    push_u64(&mut out, product.resource_receipt().output_bytes);
    for segment in product.maps().coordinate_map().segments() {
        push_u64(&mut out, segment.native_start);
        push_u64(&mut out, segment.native_end);
        push_u64(&mut out, segment.decoded_start);
        push_u64(&mut out, segment.decoded_end);
        push_u64(&mut out, segment.canonical_start);
        push_u64(&mut out, segment.canonical_end);
        out.push(match segment.relation {
            SegmentRelation::Exact => 1,
            SegmentRelation::Range => 2,
            SegmentRelation::Ambiguous => 3,
            SegmentRelation::Unmapped => 4,
        });
    }
    for record in product.maps().loss_map().records() {
        out.push(match record.kind {
            LossKind::RemovedBom => 1,
            LossKind::TranscodedEncoding => 2,
            LossKind::NormalizedNewline => 3,
        });
        push_u64(&mut out, record.native_start);
        push_u64(&mut out, record.native_end);
        push_u64(&mut out, record.decoded_start);
        push_u64(&mut out, record.decoded_end);
        push_u64(&mut out, record.canonical_start);
        push_u64(&mut out, record.canonical_end);
    }
    for warning in product.warnings() {
        match warning {
            MaterializationWarning::BomStripped => out.push(1),
            MaterializationWarning::Transcoded(encoding) => {
                out.push(2);
                out.push(encoding_tag(*encoding));
            }
            MaterializationWarning::NewlinesNormalized(count) => {
                out.push(3);
                push_u64(&mut out, *count);
            }
        }
    }
    Ok(CanonicalMaterializationBytes { bytes: out })
}
