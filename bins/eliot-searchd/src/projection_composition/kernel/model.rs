//! Admitted projection inputs and content-free persisted observations.

use search_contracts::{Blake3Digest32, Epoch, OpaqueId};
use search_projection_planner::{ExpectedUnit, NamedVector, ScopeExpectation};

/// Accepted T13 membership binding evidence (shape only).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipReceipt {
    /// Exact source membership.
    pub source_membership_id: OpaqueId,
    /// Exact projection membership.
    pub projection_membership_id: OpaqueId,
}

/// Admitted per-unit receipt assembled from T16 preparation evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedUnitReceipt {
    /// Deterministic unit ordinal within the retained revision.
    pub unit_ordinal: u64,
    /// Inclusive exact source byte start.
    pub source_byte_start: u64,
    /// Exclusive exact source byte end.
    pub source_byte_end: u64,
    /// Exact unit bytes digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
    /// T16 canonical representation digest the unit was prepared under.
    pub representation_digest: Blake3Digest32,
    /// Admitted residency binding digest.
    pub residency_digest: Blake3Digest32,
    /// Exact access partition digest used for pre-scoring filtering.
    pub access_partition_digest: Blake3Digest32,
}

/// One admitted unit with its T25-qualified named-vector encodings.
#[derive(Clone, Debug, PartialEq)]
pub struct ComposingUnit {
    /// Admitted per-unit receipt.
    pub receipt: AdmittedUnitReceipt,
    /// Complete named-vector set with immutable digests.
    pub vectors: Vec<NamedVector>,
}

/// Complete membership-scoped projection request.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositionRequest {
    /// Exact admitted scope.
    pub scope: ScopeExpectation,
    /// Visible target epoch stamped into every minimal payload.
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
    /// Scope-binding key (`blake3` over the canonical scope preimage).
    pub scope_key: [u8; 32],
    /// `blake3` digest of the exact canonical manifest bytes.
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
