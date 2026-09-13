//! Versioned coordinate/loss map contracts and content-free receipts.

use search_contracts::{NonZeroRevision, OpaqueId};

use crate::profile::{CoordinateSpace, MaterializerProfileId};

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
    pub(super) version: u16,
    pub(super) basis: [CoordinateSpace; 3],
    pub(super) segments: Vec<CoordinateSegment>,
    pub(super) native_len: u64,
    pub(super) decoded_len: u64,
    pub(super) canonical_len: u64,
    pub(super) source_id: OpaqueId,
    pub(super) revision: NonZeroRevision,
    pub(super) profile_id: MaterializerProfileId,
}

impl CoordinateMap {
    /// Map format version.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Coordinate basis in canonical order.
    #[must_use]
    pub const fn basis(&self) -> &[CoordinateSpace] {
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
    pub(super) records: Vec<LossRecord>,
    pub(super) profile_id: MaterializerProfileId,
    pub(super) source_id: OpaqueId,
    pub(super) revision: NonZeroRevision,
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
    pub(super) coordinate_map: CoordinateMap,
    pub(super) loss_map: LossMap,
}

impl MapBundle {
    /// Builds a bundle from its two maps. Structural validation happens in
    /// [`super::validate_map_bundle`], not here.
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
    pub(super) profile_id: MaterializerProfileId,
    pub(super) source_id: OpaqueId,
    pub(super) revision: NonZeroRevision,
    pub(super) coordinate_segments: u64,
    pub(super) loss_records: u64,
    pub(super) assurance: crate::assurance::AssuranceCeiling,
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
