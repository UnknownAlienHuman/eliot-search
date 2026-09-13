//! Deterministic product warnings and identity digests.

use super::model::MaterializationWarning;
use crate::maps::{LossKind, MapBundle, SegmentRelation};
use crate::profile::{SourceEncoding, digest32};
use crate::request::ValidatedMaterializationRequest;
use search_contracts::Blake3Digest32;

pub(super) fn warnings_for(
    bundle: &MapBundle,
    encoding: SourceEncoding,
) -> Vec<MaterializationWarning> {
    let mut warnings = Vec::new();
    let mut newlines = 0_u64;
    for record in bundle.loss_map().records() {
        match record.kind {
            LossKind::RemovedBom => warnings.push(MaterializationWarning::BomStripped),
            LossKind::TranscodedEncoding => {
                warnings.push(MaterializationWarning::Transcoded(encoding));
            }
            LossKind::NormalizedNewline => {
                newlines = newlines.saturating_add(1);
            }
        }
    }
    if newlines > 0 {
        warnings.push(MaterializationWarning::NewlinesNormalized(newlines));
    }
    warnings
}

pub(super) fn digest_coordinate_map(bundle: &MapBundle) -> Blake3Digest32 {
    let mut bytes = Vec::with_capacity(bundle.coordinate_map().segments().len() * 49);
    for segment in bundle.coordinate_map().segments() {
        bytes.extend_from_slice(&segment.native_start.to_le_bytes());
        bytes.extend_from_slice(&segment.native_end.to_le_bytes());
        bytes.extend_from_slice(&segment.decoded_start.to_le_bytes());
        bytes.extend_from_slice(&segment.decoded_end.to_le_bytes());
        bytes.extend_from_slice(&segment.canonical_start.to_le_bytes());
        bytes.extend_from_slice(&segment.canonical_end.to_le_bytes());
        bytes.push(match segment.relation {
            SegmentRelation::Exact => 1,
            SegmentRelation::Range => 2,
            SegmentRelation::Ambiguous => 3,
            SegmentRelation::Unmapped => 4,
        });
    }
    Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/coordinates/v1",
        &[&bytes],
    ))
}

pub(super) fn digest_loss_map(bundle: &MapBundle) -> Blake3Digest32 {
    let mut bytes = Vec::with_capacity(bundle.loss_map().records().len() * 49);
    for record in bundle.loss_map().records() {
        bytes.push(match record.kind {
            LossKind::RemovedBom => 1,
            LossKind::TranscodedEncoding => 2,
            LossKind::NormalizedNewline => 3,
        });
        bytes.extend_from_slice(&record.native_start.to_le_bytes());
        bytes.extend_from_slice(&record.native_end.to_le_bytes());
        bytes.extend_from_slice(&record.decoded_start.to_le_bytes());
        bytes.extend_from_slice(&record.decoded_end.to_le_bytes());
        bytes.extend_from_slice(&record.canonical_start.to_le_bytes());
        bytes.extend_from_slice(&record.canonical_end.to_le_bytes());
    }
    Blake3Digest32::from_bytes(digest32(b"eliot-search/materializer/loss/v1", &[&bytes]))
}

pub(super) fn digest_representation(
    request: &ValidatedMaterializationRequest,
    encoding: SourceEncoding,
    canonical_text: &str,
    coordinate_digest: &Blake3Digest32,
    loss_digest: &Blake3Digest32,
) -> Blake3Digest32 {
    Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/product/v1",
        &[
            request.source_id().as_str().as_bytes(),
            &request.revision().get().to_le_bytes(),
            request.profile().id().as_bytes(),
            &[encoding_tag(encoding)],
            request.content_digest().as_bytes(),
            canonical_text.as_bytes(),
            coordinate_digest.as_bytes(),
            loss_digest.as_bytes(),
        ],
    ))
}

pub(super) const fn encoding_tag(encoding: SourceEncoding) -> u8 {
    match encoding {
        SourceEncoding::Utf8 => 1,
        SourceEncoding::Utf16Le => 2,
        SourceEncoding::Utf16Be => 3,
    }
}
