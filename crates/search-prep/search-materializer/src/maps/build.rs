//! Deterministic coordinate and loss map construction.

use crate::MaterializationError;
use crate::decode::{DecodedRepresentation, StepCounter};
use crate::normalize::CanonicalRepresentation;
use crate::profile::{CoordinateSpace, LossBehavior, ValidatedMaterializerProfile};
use crate::request::{CancellationToken, MaterializationBudget};

use super::model::{
    COORDINATE_MAP_VERSION, CoordinateMap, CoordinateSegment, LossKind, LossMap, LossRecord,
    MapIdentities, SegmentRelation,
};

fn check_profile_binding(
    identities: &MapIdentities,
    decoded: &DecodedRepresentation,
    canonical: &CanonicalRepresentation,
    profile: &ValidatedMaterializerProfile,
) -> Result<(), MaterializationError> {
    if identities.profile_id != profile.id()
        || decoded.profile_id() != profile.id()
        || canonical.profile_id() != profile.id()
    {
        return Err(MaterializationError::ProfileMismatch);
    }
    Ok(())
}

/// Builds a versioned bounded coordinate map from decode/normalize evidence.
///
/// Adjacent lines with the same relationship and contiguous ranges merge into
/// one segment, so exact ASCII input yields a single `Exact` segment while
/// transcoded or normalized regions keep explicit `Range` segments and
/// stripped prefixes keep `Unmapped` segments. Elementary steps accumulate
/// into the caller-provided shared step counter.
pub fn build_coordinate_map(
    decoded: &DecodedRepresentation,
    canonical: &CanonicalRepresentation,
    identities: &MapIdentities,
    profile: &ValidatedMaterializerProfile,
    budget: &MaterializationBudget,
    steps: &mut StepCounter,
    cancel: CancellationToken<'_>,
) -> Result<CoordinateMap, MaterializationError> {
    if cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    let budget = budget.validate()?;
    check_profile_binding(identities, decoded, canonical, profile)?;
    if decoded.lines().len() != canonical.lines().len() {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    let max_segments = budget.effective_segments(profile.limits().max_map_segments);
    let mut segments: Vec<CoordinateSegment> = Vec::new();
    if decoded.bom_stripped() && decoded.bom_len_bytes() > 0 {
        segments.push(CoordinateSegment {
            native_start: 0,
            native_end: decoded.bom_len_bytes(),
            decoded_start: 0,
            decoded_end: 0,
            canonical_start: 0,
            canonical_end: 0,
            relation: SegmentRelation::Unmapped,
        });
    }
    for (decoded_line, canonical_line) in decoded.lines().iter().zip(canonical.lines().iter()) {
        if decoded_line.decoded_start != canonical_line.decoded_start
            || decoded_line.decoded_end != canonical_line.decoded_end
        {
            return Err(MaterializationError::CoordinateMapInvalid);
        }
        let native_aligned = decoded_line.native_start == decoded_line.decoded_start
            && decoded_line.native_end == decoded_line.decoded_end;
        let canonical_aligned = decoded_line.decoded_start == canonical_line.canonical_start
            && decoded_line.decoded_end == canonical_line.canonical_end;
        let relation = if !decoded.transcoded()
            && !canonical_line.ending_changed()
            && native_aligned
            && canonical_aligned
        {
            SegmentRelation::Exact
        } else {
            SegmentRelation::Range
        };
        let segment = CoordinateSegment {
            native_start: decoded_line.native_start,
            native_end: decoded_line.native_end,
            decoded_start: decoded_line.decoded_start,
            decoded_end: decoded_line.decoded_end,
            canonical_start: canonical_line.canonical_start,
            canonical_end: canonical_line.canonical_end,
            relation,
        };
        let mergeable = segments.last().is_some_and(|last| {
            last.relation == relation
                && last.native_end == segment.native_start
                && last.decoded_end == segment.decoded_start
                && last.canonical_end == segment.canonical_start
        });
        if mergeable {
            let Some(last) = segments.last_mut() else {
                return Err(MaterializationError::CoordinateMapInvalid);
            };
            last.native_end = segment.native_end;
            last.decoded_end = segment.decoded_end;
            last.canonical_end = segment.canonical_end;
        } else {
            if u64::try_from(segments.len()).map_err(|_| MaterializationError::OffsetOverflow)?
                >= max_segments
            {
                return Err(MaterializationError::BudgetExhausted);
            }
            segments.push(segment);
        }
        steps.consume(1)?;
        if cancel.is_cancelled() {
            return Err(MaterializationError::Cancelled);
        }
    }
    steps.consume(1)?;
    if segments.is_empty() {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    Ok(CoordinateMap {
        version: COORDINATE_MAP_VERSION,
        basis: [
            CoordinateSpace::NativeBytes,
            CoordinateSpace::DecodedScalar,
            CoordinateSpace::CanonicalScalar,
        ],
        segments,
        native_len: decoded.native_len(),
        decoded_len: decoded.decoded_len_chars(),
        canonical_len: canonical.canonical_len_chars(),
        source_id: identities.source_id.clone(),
        revision: identities.revision,
        profile_id: identities.profile_id,
    })
}

/// Builds a bounded loss map from decode/normalize evidence.
///
/// Every BOM removal, transcoding and newline change becomes an explicit
/// record; the map never claims a reversible mapping where none exists. A
/// profile that forbids loss fails with [`MaterializationError::Loss`].
/// Elementary steps accumulate into the caller-provided shared step counter.
pub fn build_loss_map(
    decoded: &DecodedRepresentation,
    canonical: &CanonicalRepresentation,
    identities: &MapIdentities,
    profile: &ValidatedMaterializerProfile,
    budget: &MaterializationBudget,
    steps: &mut StepCounter,
    cancel: CancellationToken<'_>,
) -> Result<LossMap, MaterializationError> {
    if cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    let budget = budget.validate()?;
    check_profile_binding(identities, decoded, canonical, profile)?;
    if decoded.lines().len() != canonical.lines().len() {
        return Err(MaterializationError::LossMapInvalid);
    }
    let max_records = budget.effective_loss(profile.limits().max_loss_records);
    let mut records: Vec<LossRecord> = Vec::new();
    let mut push = |record: LossRecord| -> Result<(), MaterializationError> {
        if u64::try_from(records.len()).map_err(|_| MaterializationError::OffsetOverflow)?
            >= max_records
        {
            return Err(MaterializationError::BudgetExhausted);
        }
        records.push(record);
        Ok(())
    };
    if decoded.bom_stripped() && decoded.bom_len_bytes() > 0 {
        push(LossRecord {
            kind: LossKind::RemovedBom,
            native_start: 0,
            native_end: decoded.bom_len_bytes(),
            decoded_start: 0,
            decoded_end: 0,
            canonical_start: 0,
            canonical_end: 0,
        })?;
    }
    if decoded.transcoded() {
        push(LossRecord {
            kind: LossKind::TranscodedEncoding,
            native_start: decoded.bom_len_bytes(),
            native_end: decoded.native_len(),
            decoded_start: 0,
            decoded_end: decoded.decoded_len_chars(),
            canonical_start: 0,
            canonical_end: canonical.canonical_len_chars(),
        })?;
    }
    for (decoded_line, canonical_line) in decoded.lines().iter().zip(canonical.lines().iter()) {
        if canonical_line.ending_changed() {
            push(LossRecord {
                kind: LossKind::NormalizedNewline,
                native_start: decoded_line.native_start,
                native_end: decoded_line.native_end,
                decoded_start: decoded_line.decoded_start,
                decoded_end: decoded_line.decoded_end,
                canonical_start: canonical_line.canonical_start,
                canonical_end: canonical_line.canonical_end,
            })?;
        }
        steps.consume(1)?;
        if cancel.is_cancelled() {
            return Err(MaterializationError::Cancelled);
        }
    }
    steps.consume(1)?;
    if profile.loss_behavior() == LossBehavior::RejectOnAnyLoss && !records.is_empty() {
        return Err(MaterializationError::Loss);
    }
    Ok(LossMap {
        records,
        profile_id: identities.profile_id,
        source_id: identities.source_id.clone(),
        revision: identities.revision,
    })
}
