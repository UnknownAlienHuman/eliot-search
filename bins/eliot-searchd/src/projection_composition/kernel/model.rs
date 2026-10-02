//! Admitted typed projection inputs and content-free persisted observations.

use search_contracts::{
    Blake3Digest32, BoundedSymbolKey, EntityKind, Epoch, Modality,
    ProfileId, ProjectionMembershipId, RepositoryLineageId,
    ScoringDocumentId, SourceMembershipId, UnitId, UnitKind,
};
use search_projection_planner::{ExpectedUnit, NamedVector, ScopeExpectation};

/// Accepted T13 source/projection membership binding evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MembershipReceipt {
    /// Exact authoritative source membership.
    pub source_membership_id: SourceMembershipId,
    /// Exact projection membership stored in every point payload.
    pub projection_membership_id: ProjectionMembershipId,
}

/// Admitted per-unit receipt assembled from T16 preparation evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedUnitReceipt {
    /// Exact unit occurrence identity.
    pub unit_id: UnitId,
    /// Membership-independent scoring-document identity.
    pub scoring_document_id: ScoringDocumentId,
    /// Deterministic unit ordinal within the retained representation.
    pub unit_ordinal: u64,
    /// Inclusive exact source byte start.
    pub source_byte_start: u64,
    /// Exclusive exact source byte end.
    pub source_byte_end: u64,
    /// Exact unit bytes digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
    /// Admitted residency binding digest.
    pub residency_digest: Blake3Digest32,
    /// Unit kind stored in the S9.5 payload.
    pub unit_kind: UnitKind,
    /// Unit modality stored in the S9.5 payload.
    pub modality: Modality,
    /// Exact language/format profile.
    pub language_or_format: ProfileId,
    /// Optional entity classification.
    pub entity_kind: Option<EntityKind>,
    /// Optional normalized symbol key.
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    /// Optional repository lineage.
    pub repository_lineage_id: Option<RepositoryLineageId>,
}

/// One admitted unit with its T25-qualified named-vector encodings.
#[derive(Clone, Debug, PartialEq)]
pub struct ComposingUnit {
    /// Admitted per-unit receipt.
    pub receipt: AdmittedUnitReceipt,
    /// Complete named-vector set with canonical digests.
    pub vectors: Vec<NamedVector>,
}

/// Complete membership-scoped projection request.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositionRequest {
    /// Exact admitted typed scope.
    pub scope: ScopeExpectation,
    /// First epoch in which every staged point may be visible.
    pub visible_epoch: Epoch,
    /// Accepted T13 membership binding; must equal the scope pair.
    pub membership: MembershipReceipt,
    /// Admitted units with qualified encodings.
    pub units: Vec<ComposingUnit>,
    /// Independent complete unit set from the T16 unit manifest.
    pub expected_units: Vec<ExpectedUnit>,
}

/// Content-free control reference to one persisted projection manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionReference {
    /// Scope-binding key over the canonical typed scope preimage.
    pub scope_key: [u8; 32],
    /// BLAKE3 digest of the exact canonical manifest bytes.
    pub manifest_digest: [u8; 32],
    /// Exact canonical manifest byte length.
    pub manifest_bytes: u64,
    /// Exact point count.
    pub point_count: u64,
}

/// Persisted projection observation: content-free reference plus CAS locate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredProjection {
    /// Content-free control reference.
    pub reference: ProjectionReference,
    /// CAS object identifier (hex of the manifest digest).
    pub object_id_hex: String,
    /// Scope key hex used for the reference file name.
    pub scope_key_hex: String,
}
