//! Coordinate and loss map production and bundle validation.
//!
//! The coordinate map relates native byte offsets, decoded scalar positions
//! and canonical scalar positions through versioned, bounded segments. The
//! loss map records every irreversible or offset-changing fact: removed BOMs,
//! transcoded encodings and normalized newlines. Maps never claim a
//! reversible mapping where none exists: stripped prefixes are `Unmapped`,
//! non-identity ranges are `Range`, and `Ambiguous` is representable but
//! never produced by the deterministic baseline builder.

use crate::MaterializationError;
use crate::decode::DecodedRepresentation;
use crate::decode::StepCounter;
use crate::normalize::CanonicalRepresentation;
use crate::profile::ValidatedMaterializerProfile;
use crate::request::CancellationToken;
use search_contracts::{NonZeroRevision, OpaqueId};

use crate::profile::MaterializerProfileId;

/// Coordinate map format version produced by this package.
pub const COORDINATE_MAP_VERSION: u16 = 1;

/// Relationship class of one coordinate segment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SegmentRelation {
    /// Offsets coincide numerically in all three spaces.
    Exact,
    /// Contiguous range with known endpoints but non-identity interior.
    Range,
    /// Several candidate counterparts; never produced by baseline decoding.
    Ambiguous,
    /// No counterpart exists (for example stripped BOM bytes).
    Unmapped,
}

impl SegmentRelation {
    /// Stable short name used in receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Range => "range",
            Self::Ambiguous => "ambiguous",
            Self::Unmapped => "unmapped",
        }
    }
}

/// One versioned coordinate segment across the three spaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoordinateSegment {
    /// Inclusive native byte start.
    pub native_start: u64,
    /// Exclusive native byte end.
    pub native_end: u64,
    /// Inclusive decoded scalar start.
    pub decoded_start: u64,
    /// Exclusive decoded scalar end.
    pub decoded_end: u64,
    /// Inclusive canonical scalar start.
    pub canonical_start: u64,
    /// Exclusive canonical scalar end.
    pub canonical_end: u64,
    /// Relationship class of this segment.
    pub relation: SegmentRelation,
}

/// Loss class of one irreversible or offset-changing fact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LossKind {
    /// A leading byte-order mark was consumed, not kept.
    RemovedBom,
    /// Bytes were transcoded from UTF-16 to UTF-8.
    TranscodedEncoding,
    /// A line terminator was normalized.
    NormalizedNewline,
}

impl LossKind {
    /// Stable short name used in receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RemovedBom => "removed-bom",
            Self::TranscodedEncoding => "transcoded-encoding",
            Self::NormalizedNewline => "normalized-newline",
        }
    }
}

/// One recorded loss with coordinates in all three spaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LossRecord {
    /// Loss class.
    pub kind: LossKind,
    /// Inclusive native byte start.
    pub native_start: u64,
    /// Exclusive native byte end.
    pub native_end: u64,
    /// Inclusive decoded scalar start.
    pub decoded_start: u64,
    /// Exclusive decoded scalar end.
    pub decoded_end: u64,
    /// Inclusive canonical scalar start.
    pub canonical_start: u64,
    /// Exclusive canonical scalar end.
    pub canonical_end: u64,
}

/// Identities stamped into every map at build time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapIdentities {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Exact retained revision.
    pub revision: NonZeroRevision,
    /// Canonical profile identity.
    pub profile_id: MaterializerProfileId,
}

/// Versioned bounded map across native, decoded and canonical spaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateMap {
    version: u16,
    basis: [crate::profile::CoordinateSpace; 3],
    segments: Vec<CoordinateSegment>,
    native_len: u64,
    decoded_len: u64,
    canonical_len: u64,
    source_id: OpaqueId,
    revision: NonZeroRevision,
    profile_id: MaterializerProfileId,
}

impl CoordinateMap {
    /// Map format version.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Coordinate basis in canonical order.
    #[must_use]
    pub const fn basis(&self) -> &[crate::profile::CoordinateSpace] {
        &self.basis
    }

    /// Map segments in native order.
    #[must_use]
    pub fn segments(&self) -> &[CoordinateSegment] {
        &self.segments
    }

    /// Covered native length in bytes.
    #[must_use]
    pub const fn native_len(&self) -> u64 {
        self.native_len
    }

    /// Covered decoded length in scalar values.
    #[must_use]
    pub const fn decoded_len(&self) -> u64 {
        self.decoded_len
    }

    /// Covered canonical length in scalar values.
    #[must_use]
    pub const fn canonical_len(&self) -> u64 {
        self.canonical_len
    }

    /// Bound source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Bound retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Bound profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }
}

/// Bounded loss record set for one materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LossMap {
    records: Vec<LossRecord>,
    profile_id: MaterializerProfileId,
    source_id: OpaqueId,
    revision: NonZeroRevision,
}

impl LossMap {
    /// Recorded losses in native order.
    #[must_use]
    pub fn records(&self) -> &[LossRecord] {
        &self.records
    }

    /// Bound profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }

    /// Bound source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Bound retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        records: Vec<LossRecord>,
        profile_id: MaterializerProfileId,
        source_id: OpaqueId,
        revision: NonZeroRevision,
    ) -> Self {
        Self {
            records,
            profile_id,
            source_id,
            revision,
        }
    }
}

/// Validated coordinate/loss pair travelling with one representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapBundle {
    coordinate_map: CoordinateMap,
    loss_map: LossMap,
}

impl MapBundle {
    /// Builds a bundle from its two maps. Structural validation happens in
    /// [`validate_map_bundle`], not here.
    #[must_use]
    pub const fn from_parts(coordinate_map: CoordinateMap, loss_map: LossMap) -> Self {
        Self {
            coordinate_map,
            loss_map,
        }
    }

    /// Coordinate map of this bundle.
    #[must_use]
    pub const fn coordinate_map(&self) -> &CoordinateMap {
        &self.coordinate_map
    }

    /// Loss map of this bundle.
    #[must_use]
    pub const fn loss_map(&self) -> &LossMap {
        &self.loss_map
    }
}

/// Content-free map validation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapValidationReceipt {
    profile_id: MaterializerProfileId,
    source_id: OpaqueId,
    revision: NonZeroRevision,
    coordinate_segments: u64,
    loss_records: u64,
    assurance: crate::assurance::AssuranceCeiling,
}

impl MapValidationReceipt {
    /// Validated profile identity.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }

    /// Validated source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Validated retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Number of validated coordinate segments.
    #[must_use]
    pub const fn coordinate_segments(&self) -> u64 {
        self.coordinate_segments
    }

    /// Number of validated loss records.
    #[must_use]
    pub const fn loss_records(&self) -> u64 {
        self.loss_records
    }

    /// Assurance ceiling consistent with the validated loss evidence.
    #[must_use]
    pub const fn assurance(&self) -> crate::assurance::AssuranceCeiling {
        self.assurance
    }
}

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
    budget: &crate::request::MaterializationBudget,
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
            crate::profile::CoordinateSpace::NativeBytes,
            crate::profile::CoordinateSpace::DecodedScalar,
            crate::profile::CoordinateSpace::CanonicalScalar,
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
/// Elementary steps accumulate into the shared step counter.
pub fn build_loss_map(
    decoded: &DecodedRepresentation,
    canonical: &CanonicalRepresentation,
    identities: &MapIdentities,
    profile: &ValidatedMaterializerProfile,
    budget: &crate::request::MaterializationBudget,
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
    if profile.loss_behavior() == crate::profile::LossBehavior::RejectOnAnyLoss
        && !records.is_empty()
    {
        return Err(MaterializationError::Loss);
    }
    Ok(LossMap {
        records,
        profile_id: identities.profile_id,
        source_id: identities.source_id.clone(),
        revision: identities.revision,
    })
}

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
    // Loss evidence must match the representation exactly: no unrecorded
    // transforms and no phantom records.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{decode_text_or_code, detect_or_validate_encoding};
    use crate::normalize::normalize_representation;
    use crate::profile::{
        SourceEncoding, baseline_profile_descriptor, validate_materializer_profile,
    };
    use crate::request::{DEFAULT_MATERIALIZATION_BUDGET, MaterializationBudget};

    fn profile() -> ValidatedMaterializerProfile {
        validate_materializer_profile(&baseline_profile_descriptor("maps-test", 1))
            .expect("profile")
    }

    fn identities(profile: &ValidatedMaterializerProfile) -> MapIdentities {
        MapIdentities {
            source_id: OpaqueId::new("source:test").expect("source"),
            revision: NonZeroRevision::new(1).expect("revision"),
            profile_id: profile.id(),
        }
    }

    fn decoded(
        bytes: &[u8],
        encoding: SourceEncoding,
        profile: &ValidatedMaterializerProfile,
    ) -> DecodedRepresentation {
        let decision = detect_or_validate_encoding(bytes, encoding, profile).expect("decision");
        decode_text_or_code(
            bytes,
            &decision,
            profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("decode")
    }

    fn bundle_for(
        bytes: &[u8],
        encoding: SourceEncoding,
        profile: &ValidatedMaterializerProfile,
    ) -> (CanonicalRepresentation, MapBundle) {
        let text = decoded(bytes, encoding, profile);
        let canonical = normalize_representation(
            &text,
            profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("normalize");
        let coordinate_map = build_coordinate_map(
            &text,
            &canonical,
            &identities(profile),
            profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("coordinate map");
        let loss_map = build_loss_map(
            &text,
            &canonical,
            &identities(profile),
            profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("loss map");
        (
            canonical,
            MapBundle {
                coordinate_map,
                loss_map,
            },
        )
    }

    #[test]
    fn exact_ascii_collapses_to_one_segment() {
        let profile = profile();
        let (canonical, bundle) = bundle_for(b"hello\nworld\n", SourceEncoding::Utf8, &profile);
        assert_eq!(bundle.coordinate_map.segments().len(), 1);
        assert_eq!(
            bundle.coordinate_map.segments()[0].relation,
            SegmentRelation::Exact
        );
        assert!(bundle.loss_map.records().is_empty());
        let receipt = validate_map_bundle(&canonical, &bundle, &profile).expect("valid");
        assert_eq!(receipt.coordinate_segments(), 1);
        assert_eq!(
            receipt.assurance(),
            crate::assurance::AssuranceCeiling::ExactBytes
        );
    }

    #[test]
    fn bom_yields_unmapped_segment_and_loss() {
        let profile = profile();
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"ab\n");
        let (canonical, bundle) = bundle_for(&bytes, SourceEncoding::Utf8, &profile);
        assert_eq!(
            bundle.coordinate_map.segments()[0].relation,
            SegmentRelation::Unmapped
        );
        assert_eq!(bundle.coordinate_map.segments()[0].native_end, 3);
        assert_eq!(bundle.loss_map.records().len(), 1);
        validate_map_bundle(&canonical, &bundle, &profile).expect("valid");
    }

    #[test]
    fn transcoded_lines_are_ranges_with_evidence() {
        let profile = profile();
        let mut bytes = Vec::new();
        for unit in "Aπ\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let (canonical, bundle) = bundle_for(&bytes, SourceEncoding::Utf16Le, &profile);
        assert!(
            bundle
                .coordinate_map
                .segments()
                .iter()
                .any(|segment| segment.relation == SegmentRelation::Range)
        );
        assert!(
            bundle
                .loss_map
                .records()
                .iter()
                .any(|record| record.kind == LossKind::TranscodedEncoding)
        );
        let receipt = validate_map_bundle(&canonical, &bundle, &profile).expect("valid");
        assert_eq!(
            receipt.assurance(),
            crate::assurance::AssuranceCeiling::ExactTranscoded
        );
    }

    #[test]
    fn bom_only_input_maps_to_unmapped_only() {
        let profile = profile();
        let (canonical, bundle) = bundle_for(b"\xEF\xBB\xBF", SourceEncoding::Utf8, &profile);
        assert_eq!(canonical.text(), "");
        assert_eq!(bundle.coordinate_map.segments().len(), 1);
        assert_eq!(
            bundle.coordinate_map.segments()[0].relation,
            SegmentRelation::Unmapped
        );
        assert_eq!(bundle.loss_map.records().len(), 1);
        let receipt = validate_map_bundle(&canonical, &bundle, &profile).expect("valid");
        assert_eq!(
            receipt.assurance(),
            crate::assurance::AssuranceCeiling::NormalizedWithRecordedLoss
        );
    }

    #[test]
    fn gapped_coverage_is_rejected() {
        let profile = profile();
        let (canonical, mut bundle) = bundle_for(b"ab\ncd\n", SourceEncoding::Utf8, &profile);
        bundle.coordinate_map.segments.clear();
        assert_eq!(
            validate_map_bundle(&canonical, &bundle, &profile),
            Err(MaterializationError::CoordinateMapInvalid)
        );
    }

    #[test]
    fn phantom_loss_is_rejected() {
        let profile = profile();
        let (canonical, mut bundle) = bundle_for(b"ab\n", SourceEncoding::Utf8, &profile);
        bundle.loss_map.records.push(LossRecord {
            kind: LossKind::RemovedBom,
            native_start: 0,
            native_end: 0,
            decoded_start: 0,
            decoded_end: 0,
            canonical_start: 0,
            canonical_end: 0,
        });
        assert_eq!(
            validate_map_bundle(&canonical, &bundle, &profile),
            Err(MaterializationError::LossMapInvalid)
        );
    }

    #[test]
    fn foreign_profile_binding_is_rejected() {
        let profile = profile();
        let other = validate_materializer_profile(&baseline_profile_descriptor("other-maps", 1))
            .expect("other");
        let (canonical, bundle) = bundle_for(b"ab\n", SourceEncoding::Utf8, &profile);
        assert_eq!(
            validate_map_bundle(&canonical, &bundle, &other),
            Err(MaterializationError::ProfileMismatch)
        );
    }

    #[test]
    fn tiny_segment_budget_is_exhausted() {
        let profile = profile();
        // BOM plus text forces two segments (Unmapped + Exact), so a budget
        // of one segment is exhausted instead of silently merging.
        let text = decoded(b"\xEF\xBB\xBFa\nb\n", SourceEncoding::Utf8, &profile);
        let canonical = normalize_representation(
            &text,
            &profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("normalize");
        let budget = MaterializationBudget {
            max_map_segments: 1,
            ..DEFAULT_MATERIALIZATION_BUDGET
        };
        assert_eq!(
            build_coordinate_map(
                &text,
                &canonical,
                &identities(&profile),
                &profile,
                &budget,
                &mut StepCounter::new(1 << 20),
                CancellationToken::never()
            ),
            Err(MaterializationError::BudgetExhausted)
        );
    }

    #[test]
    fn baseline_never_produces_ambiguous_segments() {
        let profile = profile();
        for bytes in [
            b"a\n".as_slice(),
            "αβ\n".as_bytes(),
            b"\xEF\xBB\xBFq\n".as_slice(),
        ] {
            let (_, bundle) = bundle_for(bytes, SourceEncoding::Utf8, &profile);
            assert!(
                bundle
                    .coordinate_map
                    .segments()
                    .iter()
                    .all(|segment| segment.relation != SegmentRelation::Ambiguous)
            );
        }
    }
}
