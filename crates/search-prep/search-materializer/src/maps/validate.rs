//! Independent structural validation of coordinate/loss map evidence.

use crate::MaterializationError;
use crate::normalize::CanonicalRepresentation;
use crate::profile::ValidatedMaterializerProfile;

use super::model::{
    COORDINATE_MAP_VERSION, CoordinateMap, LossKind, MapBundle, MapValidationReceipt,
    SegmentRelation,
};

/// Validates one coordinate map against expected space lengths.
///
/// Checks format version, record bounds, full contiguous coverage of the
/// native space, tiling of the decoded/canonical spaces by mappable segments,
/// zero decoded/canonical extent for `Unmapped` segments and monotonic
/// non-overlapping layout.
pub fn validate_coordinate_map(
    map: &CoordinateMap,
    native_len: u64,
    decoded_len: u64,
    canonical_len: u64,
) -> Result<(), MaterializationError> {
    if map.version != COORDINATE_MAP_VERSION {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    if map.native_len != native_len
        || map.decoded_len != decoded_len
        || map.canonical_len != canonical_len
    {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    if map.segments.is_empty() {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    let mut native_cursor = 0_u64;
    let mut decoded_cursor = 0_u64;
    let mut canonical_cursor = 0_u64;
    for segment in &map.segments {
        if segment.native_start != native_cursor
            || segment.native_end < segment.native_start
            || segment.native_end > native_len
        {
            return Err(MaterializationError::CoordinateMapInvalid);
        }
        if segment.decoded_end < segment.decoded_start || segment.decoded_end > decoded_len {
            return Err(MaterializationError::CoordinateMapInvalid);
        }
        if segment.canonical_end < segment.canonical_start || segment.canonical_end > canonical_len
        {
            return Err(MaterializationError::CoordinateMapInvalid);
        }
        if segment.relation == SegmentRelation::Unmapped {
            if segment.decoded_start != segment.decoded_end
                || segment.canonical_start != segment.canonical_end
                || segment.decoded_start != decoded_cursor
                || segment.canonical_start != canonical_cursor
            {
                return Err(MaterializationError::CoordinateMapInvalid);
            }
        } else {
            if segment.decoded_start != decoded_cursor
                || segment.canonical_start != canonical_cursor
            {
                return Err(MaterializationError::CoordinateMapInvalid);
            }
            decoded_cursor = segment.decoded_end;
            canonical_cursor = segment.canonical_end;
        }
        native_cursor = segment.native_end;
    }
    if native_cursor != native_len
        || decoded_cursor != decoded_len
        || canonical_cursor != canonical_len
    {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    Ok(())
}

/// Validates a map bundle against its representation and profile.
///
/// Checks bounds, coverage, monotonic segments, absence of overlapping
/// contradictory mappings, revision/profile identities, loss record bounds
/// and cardinality, and that every loss is both evidenced by the
/// representation (transcoding, BOM, changed terminators) and consistent with
/// the returned assurance ceiling. Returns a content-free receipt.
pub fn validate_map_bundle(
    representation: &CanonicalRepresentation,
    bundle: &MapBundle,
    profile: &ValidatedMaterializerProfile,
) -> Result<MapValidationReceipt, MaterializationError> {
    if bundle.coordinate_map.profile_id != profile.id()
        || bundle.loss_map.profile_id != profile.id()
        || representation.profile_id() != profile.id()
    {
        return Err(MaterializationError::ProfileMismatch);
    }
    if bundle.coordinate_map.source_id != bundle.loss_map.source_id
        || bundle.coordinate_map.revision != bundle.loss_map.revision
    {
        return Err(MaterializationError::LossMapInvalid);
    }
    validate_coordinate_map(
        &bundle.coordinate_map,
        representation.native_len(),
        representation.decoded_len_chars(),
        representation.canonical_len_chars(),
    )?;
    let max_records = profile.limits().max_loss_records;
    let loss_count = u64::try_from(bundle.loss_map.records.len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    if loss_count > max_records {
        return Err(MaterializationError::LossMapInvalid);
    }
    for record in &bundle.loss_map.records {
        if record.native_end < record.native_start
            || record.native_end > representation.native_len()
            || record.decoded_end < record.decoded_start
            || record.decoded_end > representation.decoded_len_chars()
            || record.canonical_end < record.canonical_start
            || record.canonical_end > representation.canonical_len_chars()
        {
            return Err(MaterializationError::LossMapInvalid);
        }
    }
    let mut has_bom = false;
    let mut has_transcode = false;
    let mut newline_records = 0_u64;
    for record in &bundle.loss_map.records {
        match record.kind {
            LossKind::RemovedBom => {
                if has_bom {
                    return Err(MaterializationError::LossMapInvalid);
                }
                has_bom = true;
            }
            LossKind::TranscodedEncoding => {
                if has_transcode {
                    return Err(MaterializationError::LossMapInvalid);
                }
                has_transcode = true;
            }
            LossKind::NormalizedNewline => {
                newline_records = newline_records
                    .checked_add(1)
                    .ok_or(MaterializationError::OffsetOverflow)?;
            }
        }
    }
    if representation.bom_stripped() != has_bom {
        return Err(MaterializationError::LossMapInvalid);
    }
    if representation.transcoded() != has_transcode {
        return Err(MaterializationError::LossMapInvalid);
    }
    let mut changed_lines = 0_u64;
    for line in representation.lines() {
        if line.ending_changed() {
            changed_lines = changed_lines
                .checked_add(1)
                .ok_or(MaterializationError::OffsetOverflow)?;
        }
    }
    if changed_lines != newline_records {
        return Err(MaterializationError::LossMapInvalid);
    }
    let segments = u64::try_from(bundle.coordinate_map.segments.len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    Ok(MapValidationReceipt {
        profile_id: profile.id(),
        source_id: bundle.coordinate_map.source_id.clone(),
        revision: bundle.coordinate_map.revision,
        coordinate_segments: segments,
        loss_records: loss_count,
        assurance: crate::assurance::derive_assurance_ceiling(&bundle.loss_map),
    })
}
