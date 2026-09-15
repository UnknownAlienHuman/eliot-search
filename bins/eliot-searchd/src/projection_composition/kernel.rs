//! Membership-scoped projection composition (T26).
//!
//! This module composes the pure projection planner
//! (`search-projection-planner`), private point identities
//! (`search-point-identity`) and admitted receipts (T13 membership bindings,
//! T16 canonical preparation/unit manifests, T25 frozen lexical profiles)
//! into complete reproducible projection plans with immutable manifests.
//!
//! What this module owns:
//!
//! - assembling exact [`search_projection_planner::ProjectionInput`] values
//!   from admitted unit receipts and one accepted membership binding, with a
//!   locally computed (never fabricated) minimal-payload digest;
//! - planning through [`search_projection_planner::plan_scoped_projection`],
//!   so completeness, residency, cross-domain and profile/generation handling
//!   keep the planner's typed failures;
//! - persisting immutable canonical manifest bytes in a scoped file CAS
//!   (`<root>/projection/objects/`) with no-clobber publication and exact
//!   readback, plus content-free control references (`<root>/projection/refs/`)
//!   that a redb journal can store: digests, lengths and counts only, never
//!   source bodies, unit text, paths or credentials;
//! - handing T24 the exact expected payload index set; payload index creation
//!   and every Qdrant call are gated on T24 and happen nowhere here.
//!
//! What this module never does: Qdrant transport, collection creation,
//! upserts, reads, counts or queries (no `search-qdrant-bridge` or
//! `search-qdrant-supervisor` dependency); source admission or access
//! decisions (T13/T20 owners); lexical encoding (T25 profiles arrive
//! accepted); broad-filter closure when exact point IDs exist.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use search_contracts::{Blake3Digest32, Epoch, OpaqueId};
use search_point_identity::{PointIdentityLimits, PointIdentityRegistry, ProjectionKind};
use search_projection_planner::{
    ExpectedUnit, NamedVector, ProjectionBudget, ProjectionError, ProjectionInput, ProjectionPlan,
    ProjectionProfiles, ScopeExpectation, expected_payload_indexes, plan_scoped_projection,
    verify_manifest_reconstruction,
};

/// Magic prefix of every persisted projection reference record.
const REFERENCE_MAGIC: &[u8; 8] = b"ELSPRJ01";
/// Exact byte length of one serialized [`ProjectionReference`]:
/// magic (8) + scope key (32) + manifest digest (32) + byte length (8) +
/// point count (8). The manifest digest is embedded so two manifests with
/// equal length under one scope never alias the same record.
const REFERENCE_BYTES: usize = 8 + 32 + 32 + 8 + 8;
/// File extension of immutable CAS manifest objects.
const MANIFEST_EXTENSION: &str = "pman";
/// Domain tag framing the minimal-payload digest preimage.
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"eliot-search/projection-payload/v1";
/// Domain tag framing the scope-binding key preimage.
const SCOPE_KEY_DOMAIN: &[u8] = b"eliot-search/projection-scope/v1";

/// Closed membership-scoped projection composition failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionCompositionError {
    /// A finite limit is zero or internally inconsistent.
    InvalidLimits,
    /// Membership or immutable source identity is empty or mismatched.
    MembershipMismatch,
    /// More than one membership was supplied where exactly one is required.
    MembershipArrayForbidden,
    /// A raw public/vendor collection identifier reached a scope boundary.
    RawCollectionIdForbidden,
    /// Inputs span inequivalent scoring/security/generation domains.
    ScopeMismatch,
    /// Unit residency does not match the admitted residency binding.
    ResidencyMismatch,
    /// An expected unit has no admitted receipt in the exact plan.
    MissingUnitReceipt,
    /// An input unit is outside the declared complete unit set.
    UnexpectedUnit,
    /// Two point specs resolve to the same compact identity.
    DuplicatePoint,
    /// A compact point identifier maps to another complete key.
    PointCollision,
    /// Point identity input is invalid.
    PointIdentityInvalid,
    /// A manifest does not reconstruct exactly its point specs.
    ManifestInvalid,
    /// A plan exceeds its point, vector, or byte budget.
    BudgetExceeded,
    /// The scoped CAS or its references are unavailable or unreadable.
    CasUnavailable,
    /// An immutable CAS object already exists with different bytes.
    CasConflict,
    /// A control reference already exists with a different manifest binding.
    ReferenceConflict,
}

impl ProjectionCompositionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "PROJECTION_COMPOSITION_INVALID_LIMITS",
            Self::MembershipMismatch => "PROJECTION_COMPOSITION_MEMBERSHIP_MISMATCH",
            Self::MembershipArrayForbidden => "PROJECTION_COMPOSITION_MEMBERSHIP_ARRAY_FORBIDDEN",
            Self::RawCollectionIdForbidden => "PROJECTION_COMPOSITION_RAW_COLLECTION_ID_FORBIDDEN",
            Self::ScopeMismatch => "PROJECTION_COMPOSITION_SCOPE_MISMATCH",
            Self::ResidencyMismatch => "PROJECTION_COMPOSITION_RESIDENCY_MISMATCH",
            Self::MissingUnitReceipt => "PROJECTION_COMPOSITION_MISSING_UNIT_RECEIPT",
            Self::UnexpectedUnit => "PROJECTION_COMPOSITION_UNEXPECTED_UNIT",
            Self::DuplicatePoint => "PROJECTION_COMPOSITION_DUPLICATE_POINT",
            Self::PointCollision => "PROJECTION_COMPOSITION_POINT_COLLISION",
            Self::PointIdentityInvalid => "PROJECTION_COMPOSITION_POINT_IDENTITY_INVALID",
            Self::ManifestInvalid => "PROJECTION_COMPOSITION_MANIFEST_INVALID",
            Self::BudgetExceeded => "PROJECTION_COMPOSITION_BUDGET_EXCEEDED",
            Self::CasUnavailable => "PROJECTION_COMPOSITION_CAS_UNAVAILABLE",
            Self::CasConflict => "PROJECTION_COMPOSITION_CAS_CONFLICT",
            Self::ReferenceConflict => "PROJECTION_COMPOSITION_REFERENCE_CONFLICT",
        }
    }
}

impl std::fmt::Display for ProjectionCompositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProjectionCompositionError {}

impl From<ProjectionError> for ProjectionCompositionError {
    fn from(error: ProjectionError) -> Self {
        match error {
            ProjectionError::InvalidLimits => Self::InvalidLimits,
            ProjectionError::MembershipMismatch => Self::MembershipMismatch,
            ProjectionError::MembershipArrayForbidden => Self::MembershipArrayForbidden,
            ProjectionError::RawCollectionIdForbidden => Self::RawCollectionIdForbidden,
            ProjectionError::ResidencyMismatch => Self::ResidencyMismatch,
            ProjectionError::MissingUnitReceipt => Self::MissingUnitReceipt,
            ProjectionError::UnexpectedUnit => Self::UnexpectedUnit,
            ProjectionError::DuplicatePointId | ProjectionError::DuplicateUnitRole => {
                Self::DuplicatePoint
            }
            ProjectionError::PointIdentity => Self::PointIdentityInvalid,
            ProjectionError::InvalidManifest => Self::ManifestInvalid,
            ProjectionError::BudgetExceeded | ProjectionError::ManifestTooLarge => {
                Self::BudgetExceeded
            }
            // Admitted encoder/profile drift and malformed units surface as
            // scope failures: the plan never covers an inequivalent domain.
            ProjectionError::ScopeMismatch
            | ProjectionError::InvalidUnitRange
            | ProjectionError::VectorSetMismatch
            | ProjectionError::DuplicateVectorName
            | ProjectionError::VectorDimensionMismatch
            | ProjectionError::InvalidVector
            | ProjectionError::CollectionVectorMissing
            | ProjectionError::CollectionVectorMismatch
            | ProjectionError::PayloadIndexMissing => Self::ScopeMismatch,
        }
    }
}

impl From<search_point_identity::PointIdentityError> for ProjectionCompositionError {
    fn from(error: search_point_identity::PointIdentityError) -> Self {
        match error {
            search_point_identity::PointIdentityError::DigestCollision => Self::PointCollision,
            search_point_identity::PointIdentityError::RegistryCapacityExceeded => {
                Self::BudgetExceeded
            }
            _ => Self::PointIdentityInvalid,
        }
    }
}

/// Accepted T13 membership binding evidence (shape only).
///
/// Acceptance itself is owned by the T13 membership path: the caller passes
/// the binding only after the accepted receipt check. This struct pins the
/// exact pair into the request so planning cannot drift to another binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipReceipt {
    /// Exact source membership.
    pub source_membership_id: OpaqueId,
    /// Exact projection membership.
    pub projection_membership_id: OpaqueId,
}

/// Admitted per-unit receipt assembled from T16 preparation evidence.
///
/// Per-unit representation/residency/access copies let composition detect a
/// single wrong-residency or re-bound unit instead of trusting a plan-level
/// claim. Scoring-partition and collection-generation bindings stay
/// scope-level: one exact plan covers one IDF/security domain and one
/// generation.
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
///
/// Vector bytes and digests arrive with the accepted lexical profile
/// qualification; composition validates their shape against the frozen
/// profile set through the planner and never re-encodes them.
#[derive(Clone, Debug, PartialEq)]
pub struct ComposingUnit {
    /// Admitted per-unit receipt.
    pub receipt: AdmittedUnitReceipt,
    /// Complete named-vector set with immutable digests.
    pub vectors: Vec<NamedVector>,
}

/// Complete membership-scoped projection request.
///
/// `expected_units` is the independent complete set from the T16 unit
/// manifest: it — not the arriving receipts — decides completeness, so a
/// missing receipt fails closed instead of narrowing the plan.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositionRequest {
    /// Exact admitted scope (membership pair, representation, scoring,
    /// generation, residency, projection fingerprint/schema).
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
///
/// Storable in a redb journal: scope key, manifest digest, byte length and
/// point count only. Source bodies, unit text, paths, membership arrays and
/// credentials are structurally unrepresentable here.
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

impl ProjectionReference {
    /// Serializes the reference to its exact 88-byte record form.
    #[must_use]
    pub fn to_bytes(self) -> [u8; REFERENCE_BYTES] {
        let mut output = [0_u8; REFERENCE_BYTES];
        output[..8].copy_from_slice(REFERENCE_MAGIC);
        output[8..40].copy_from_slice(&self.scope_key);
        output[40..72].copy_from_slice(&self.manifest_digest);
        output[72..80].copy_from_slice(&self.manifest_bytes.to_be_bytes());
        output[80..88].copy_from_slice(&self.point_count.to_be_bytes());
        output
    }

    /// Parses one exact reference record; malformed input fails closed.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionCompositionError::ManifestInvalid`] when the magic
    /// prefix or the fixed length is wrong.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProjectionCompositionError> {
        if bytes.len() != REFERENCE_BYTES || bytes[..8] != *REFERENCE_MAGIC {
            return Err(ProjectionCompositionError::ManifestInvalid);
        }
        let scope_key: [u8; 32] = bytes[8..40]
            .try_into()
            .map_err(|_| ProjectionCompositionError::ManifestInvalid)?;
        let manifest_digest: [u8; 32] = bytes[40..72]
            .try_into()
            .map_err(|_| ProjectionCompositionError::ManifestInvalid)?;
        let manifest_bytes = u64::from_be_bytes(
            bytes[72..80]
                .try_into()
                .map_err(|_| ProjectionCompositionError::ManifestInvalid)?,
        );
        let point_count = u64::from_be_bytes(
            bytes[80..88]
                .try_into()
                .map_err(|_| ProjectionCompositionError::ManifestInvalid)?,
        );
        Ok(Self {
            scope_key,
            manifest_digest,
            manifest_bytes,
            point_count,
        })
    }
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

/// Computes the minimal-payload digest for one composed point.
///
/// The digest covers exactly the filterable payload fields plus the unit and
/// representation bindings, under an explicit domain tag. It is computed —
/// never fabricated — from admitted values the caller already holds.
#[must_use]
pub fn compute_payload_digest(
    scope: &ScopeExpectation,
    visible_epoch: Epoch,
    receipt: &AdmittedUnitReceipt,
) -> Blake3Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PAYLOAD_DIGEST_DOMAIN);
    hasher.update(&[0]);
    append_text_hash(&mut hasher, scope.source_membership_id.as_str());
    append_text_hash(&mut hasher, scope.projection_membership_id.as_str());
    hasher.update(&scope.source_revision.get().to_be_bytes());
    hasher.update(&receipt.unit_ordinal.to_be_bytes());
    hasher.update(&visible_epoch.get().to_be_bytes());
    hasher.update(receipt.access_partition_digest.as_bytes());
    hasher.update(receipt.unit_digest.as_bytes());
    hasher.update(receipt.representation_digest.as_bytes());
    Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
}

/// Computes the scope-binding key identifying one exact plan scope.
#[must_use]
pub fn compute_scope_key(scope: &ScopeExpectation) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SCOPE_KEY_DOMAIN);
    hasher.update(&[0]);
    append_text_hash(&mut hasher, scope.namespace_id.as_str());
    append_text_hash(&mut hasher, scope.source_id.as_str());
    hasher.update(&scope.source_revision.get().to_be_bytes());
    append_text_hash(&mut hasher, scope.source_membership_id.as_str());
    append_text_hash(&mut hasher, scope.projection_membership_id.as_str());
    hasher.update(scope.projection_fingerprint.as_bytes());
    hasher.update(&scope.projection_schema_revision.get().to_be_bytes());
    hasher.update(scope.representation_digest.as_bytes());
    hasher.update(scope.scoring_partition_digest.as_bytes());
    hasher.update(scope.collection_generation_digest.as_bytes());
    hasher.update(scope.residency_digest.as_bytes());
    *hasher.finalize().as_bytes()
}

/// Composes one complete membership-scoped projection plan (pure, no I/O).
///
/// Builds exact planner inputs from admitted receipts, checks the accepted
/// membership binding against the scope pair, plans through the scoped
/// planner (completeness, residency, cross-domain and profile handling keep
/// their typed failures) and replays every derived identity through a finite
/// collision registry with full-key readback before returning.
///
/// # Errors
///
/// Returns the typed [`ProjectionCompositionError`] for scope drift, missing
/// or unexpected receipts, duplicates, collisions, invalid manifests and
/// exhausted budgets. Performs no Qdrant, CAS or redb I/O.
pub fn compose_scoped_projection(
    request: &CompositionRequest,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    point_identity_limits: PointIdentityLimits,
) -> Result<ProjectionPlan, ProjectionCompositionError> {
    let budget = budget
        .validate()
        .map_err(|_| ProjectionCompositionError::from(ProjectionError::InvalidLimits))?;
    point_identity_limits
        .validate()
        .map_err(ProjectionCompositionError::from)?;
    if request.membership.source_membership_id != request.scope.source_membership_id
        || request.membership.projection_membership_id != request.scope.projection_membership_id
    {
        return Err(ProjectionCompositionError::MembershipMismatch);
    }
    if request.units.is_empty() || request.units.len() > budget.max_points {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }
    let mut inputs = Vec::with_capacity(request.units.len());
    for unit in &request.units {
        if unit.receipt.source_byte_start >= unit.receipt.source_byte_end {
            return Err(ProjectionCompositionError::from(
                ProjectionError::InvalidUnitRange,
            ));
        }
        inputs.push(ProjectionInput {
            namespace_id: request.scope.namespace_id.clone(),
            source_id: request.scope.source_id.clone(),
            source_membership_id: request.scope.source_membership_id.clone(),
            projection_membership_id: request.scope.projection_membership_id.clone(),
            source_revision: request.scope.source_revision,
            unit_ordinal: unit.receipt.unit_ordinal,
            source_byte_start: unit.receipt.source_byte_start,
            source_byte_end: unit.receipt.source_byte_end,
            projection_kind: ProjectionKind::Lexical,
            projection_fingerprint: request.scope.projection_fingerprint,
            projection_schema_revision: request.scope.projection_schema_revision,
            visible_epoch: request.visible_epoch,
            access_partition_digest: unit.receipt.access_partition_digest,
            representation_digest: unit.receipt.representation_digest,
            scoring_partition_digest: request.scope.scoring_partition_digest,
            collection_generation_digest: request.scope.collection_generation_digest,
            residency_digest: unit.receipt.residency_digest,
            unit_digest: unit.receipt.unit_digest,
            reference_digest: unit.receipt.reference_digest,
            payload_digest: compute_payload_digest(
                &request.scope,
                request.visible_epoch,
                &unit.receipt,
            ),
            vectors: unit.vectors.clone(),
        });
    }
    let plan = plan_scoped_projection(
        inputs,
        &request.expected_units,
        &request.scope,
        profiles,
        budget,
        point_identity_limits,
    )
    .map_err(ProjectionCompositionError::from)?;
    verify_no_collisions(&plan, point_identity_limits)?;
    verify_manifest_reconstruction(&plan.manifest, &plan.points)
        .map_err(ProjectionCompositionError::from)?;
    Ok(plan)
}

/// Returns the exact filterable payload fields T24 must index.
///
/// No Qdrant call happens here: this is the authoritative handoff list so the
/// T24 bridge creates every required payload index before any publication.
/// It always equals the planner's required filter fields.
#[must_use]
pub const fn expected_payload_indexes_for_bridge() -> [&'static str; 6] {
    expected_payload_indexes()
}

/// Persists one composed manifest in the scoped CAS with a control reference.
///
/// The admitted `scope` is passed explicitly (it carries the residency
/// binding the plan no longer retains) and is checked against the plan's
/// identity keys before any write. Publication is no-clobber with exact
/// readback: an existing object with identical bytes replays idempotently; an
/// existing object or reference with different bytes fails closed with
/// [`ProjectionCompositionError::CasConflict`] or
/// [`ProjectionCompositionError::ReferenceConflict`] instead of overwriting.
/// Only canonical manifest bytes enter CAS; only the content-free
/// [`ProjectionReference`] is control state.
///
/// # Errors
///
/// Returns [`ProjectionCompositionError`] for scope drift, invalid manifests,
/// exhausted budgets and unavailable or conflicting CAS/reference state.
/// Performs no Qdrant I/O.
pub fn store_projection_manifest(
    root: &Path,
    plan: &ProjectionPlan,
    scope: &ScopeExpectation,
    budget: ProjectionBudget,
) -> Result<StoredProjection, ProjectionCompositionError> {
    let budget = budget
        .validate()
        .map_err(|_| ProjectionCompositionError::from(ProjectionError::InvalidLimits))?;
    verify_manifest_reconstruction(&plan.manifest, &plan.points)
        .map_err(ProjectionCompositionError::from)?;
    if plan.manifest.canonical_bytes.len() > budget.max_manifest_bytes {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }
    let manifest_digest = blake3::hash(&plan.manifest.canonical_bytes);
    let manifest_digest: [u8; 32] = *manifest_digest.as_bytes();
    let Some(first) = plan.points.first() else {
        return Err(ProjectionCompositionError::ManifestInvalid);
    };
    // The persisted scope must be the scope the plan was composed under;
    // residency itself was enforced at compose time and is rebound here.
    if first.identity.key.namespace_id != scope.namespace_id
        || first.identity.key.source_id != scope.source_id
        || first.identity.key.source_revision != scope.source_revision
        || first.identity.key.source_membership_id != scope.source_membership_id
        || first.identity.key.projection_membership_id != scope.projection_membership_id
        || first.identity.key.projection_fingerprint != scope.projection_fingerprint
        || first.identity.key.projection_schema_revision != scope.projection_schema_revision
        || first.identity.key.representation_digest != scope.representation_digest
        || first.identity.key.scoring_partition_digest != scope.scoring_partition_digest
        || first.identity.key.collection_generation_digest != scope.collection_generation_digest
    {
        return Err(ProjectionCompositionError::ScopeMismatch);
    }
    let scope_key = compute_scope_key(scope);
    let (reference_path, objects_dir) = cas_directories(root, &scope_key)?;
    let object_id_hex = hex(&manifest_digest);
    let object_path = objects_dir
        .join(&object_id_hex[..2])
        .join(format!("{object_id_hex}.{MANIFEST_EXTENSION}"));
    if let Some(parent) = object_path.parent() {
        fs::create_dir_all(parent).map_err(|_| ProjectionCompositionError::CasUnavailable)?;
    }
    write_new_or_replay(
        &object_path,
        &plan.manifest.canonical_bytes,
        ProjectionCompositionError::CasConflict,
    )?;
    let point_count =
        u64::try_from(plan.points.len()).map_err(|_| ProjectionCompositionError::BudgetExceeded)?;
    let manifest_bytes = u64::try_from(plan.manifest.canonical_bytes.len())
        .map_err(|_| ProjectionCompositionError::BudgetExceeded)?;
    let reference = ProjectionReference {
        scope_key,
        manifest_digest,
        manifest_bytes,
        point_count,
    };
    let mut record = [0_u8; REFERENCE_BYTES];
    record[..8].copy_from_slice(REFERENCE_MAGIC);
    record[8..40].copy_from_slice(&scope_key);
    record[40..72].copy_from_slice(&manifest_digest);
    record[72..80].copy_from_slice(&manifest_bytes.to_be_bytes());
    record[80..88].copy_from_slice(&point_count.to_be_bytes());
    write_new_or_replay(
        &reference_path,
        &record,
        ProjectionCompositionError::ReferenceConflict,
    )?;
    // Exact readback of both records precedes any success report.
    let reread_object = read_bounded(&object_path, budget.max_manifest_bytes)?;
    if reread_object != plan.manifest.canonical_bytes
        || blake3::hash(&reread_object).as_bytes() != &manifest_digest
    {
        return Err(ProjectionCompositionError::CasConflict);
    }
    let reread_reference = read_bounded(&reference_path, REFERENCE_BYTES)?;
    if reread_reference != record {
        return Err(ProjectionCompositionError::ReferenceConflict);
    }
    void_reference_roundtrip(&reference)?;
    Ok(StoredProjection {
        reference,
        object_id_hex,
        scope_key_hex: hex(&scope_key),
    })
}

/// Loads exact canonical manifest bytes for one control reference.
///
/// The bytes are digest-verified against the reference; structural
/// interpretation stays with the planner's reconstruction proof. Publication
/// flows recompose deterministically from admitted receipts and compare
/// bytes, so no second manifest parser can drift.
///
/// # Errors
///
/// Returns [`ProjectionCompositionError`] for unavailable CAS state, digest
/// mismatch or over-budget objects. Performs no Qdrant I/O.
pub fn load_projection_manifest_bytes(
    root: &Path,
    reference: &ProjectionReference,
    budget: ProjectionBudget,
) -> Result<Vec<u8>, ProjectionCompositionError> {
    let budget = budget
        .validate()
        .map_err(|_| ProjectionCompositionError::from(ProjectionError::InvalidLimits))?;
    if reference.manifest_bytes
        > u64::try_from(budget.max_manifest_bytes)
            .map_err(|_| ProjectionCompositionError::BudgetExceeded)?
    {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }
    let (reference_path, objects_dir) = cas_directories(root, &reference.scope_key)?;
    let reread_reference = read_bounded(&reference_path, REFERENCE_BYTES)?;
    let mut expected_record = [0_u8; REFERENCE_BYTES];
    expected_record[..8].copy_from_slice(REFERENCE_MAGIC);
    expected_record[8..40].copy_from_slice(&reference.scope_key);
    expected_record[40..72].copy_from_slice(&reference.manifest_digest);
    expected_record[72..80].copy_from_slice(&reference.manifest_bytes.to_be_bytes());
    expected_record[80..88].copy_from_slice(&reference.point_count.to_be_bytes());
    if reread_reference != expected_record {
        return Err(ProjectionCompositionError::ReferenceConflict);
    }
    let object_id_hex = hex(&reference.manifest_digest);
    let object_path = objects_dir
        .join(&object_id_hex[..2])
        .join(format!("{object_id_hex}.{MANIFEST_EXTENSION}"));
    let bytes = read_bounded(&object_path, budget.max_manifest_bytes)?;
    if bytes.len() as u64 != reference.manifest_bytes
        || blake3::hash(&bytes).as_bytes() != &reference.manifest_digest
    {
        return Err(ProjectionCompositionError::CasConflict);
    }
    Ok(bytes)
}

/// Verifies stored bytes equal a freshly composed plan's canonical bytes.
///
/// Recomposition from admitted receipts must reproduce the persisted bytes
/// exactly; any drift (profile change, reordered encoding, truncated write)
/// fails closed. This is the restart-safe reconstruction proof T24 consumes
/// alongside [`verify_manifest_reconstruction`].
///
/// # Errors
///
/// Returns [`ProjectionCompositionError`] for unavailable state or any byte,
/// digest or length inequality. Performs no Qdrant I/O.
pub fn verify_stored_projection(
    root: &Path,
    stored: &StoredProjection,
    plan: &ProjectionPlan,
    budget: ProjectionBudget,
) -> Result<(), ProjectionCompositionError> {
    let bytes = load_projection_manifest_bytes(root, &stored.reference, budget)?;
    if bytes != plan.manifest.canonical_bytes {
        return Err(ProjectionCompositionError::CasConflict);
    }
    verify_manifest_reconstruction(&plan.manifest, &plan.points)
        .map_err(ProjectionCompositionError::from)?;
    Ok(())
}

fn verify_no_collisions(
    plan: &ProjectionPlan,
    limits: PointIdentityLimits,
) -> Result<(), ProjectionCompositionError> {
    let registry_limits = PointIdentityLimits {
        max_registered_points: plan.points.len().max(1),
        ..limits
    };
    let mut registry = PointIdentityRegistry::new(registry_limits)?;
    for point in &plan.points {
        registry.register(point.identity.clone())?;
    }
    Ok(())
}

fn void_reference_roundtrip(
    reference: &ProjectionReference,
) -> Result<(), ProjectionCompositionError> {
    let bytes = reference.to_bytes();
    let parsed = ProjectionReference::from_bytes(&bytes)?;
    if &parsed != reference {
        return Err(ProjectionCompositionError::ManifestInvalid);
    }
    Ok(())
}

fn cas_directories(
    root: &Path,
    scope_key: &[u8; 32],
) -> Result<(PathBuf, PathBuf), ProjectionCompositionError> {
    if !root.is_dir() {
        return Err(ProjectionCompositionError::CasUnavailable);
    }
    let base = root.join("projection");
    let refs = base.join("refs");
    let objects = base.join("objects");
    for directory in [&base, &refs, &objects] {
        fs::create_dir_all(directory).map_err(|_| ProjectionCompositionError::CasUnavailable)?;
    }
    Ok((refs.join(format!("{}.ref", hex(scope_key))), objects))
}

fn write_new_or_replay(
    path: &Path,
    bytes: &[u8],
    conflict: ProjectionCompositionError,
) -> Result<(), ProjectionCompositionError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(bytes)
                .map_err(|_| ProjectionCompositionError::CasUnavailable)?;
            file.sync_all()
                .map_err(|_| ProjectionCompositionError::CasUnavailable)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = read_bounded(path, bytes.len().saturating_add(1))?;
            if existing == bytes {
                Ok(())
            } else {
                Err(conflict)
            }
        }
        Err(_) => Err(ProjectionCompositionError::CasUnavailable),
    }
}

fn read_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>, ProjectionCompositionError> {
    let file = fs::File::open(path).map_err(|_| ProjectionCompositionError::CasUnavailable)?;
    let mut output = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut output)
        .map_err(|_| ProjectionCompositionError::CasUnavailable)?;
    if output.len() > max_bytes {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }
    Ok(output)
}

fn append_text_hash(hasher: &mut blake3::Hasher, value: &str) {
    let bytes = value.as_bytes();
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(TABLE[usize::from(byte >> 4)]));
        output.push(char::from(TABLE[usize::from(byte & 0x0F)]));
    }
    output
}

/// Default composition budget for tests and small scopes (finite, explicit).
#[cfg(test)]
const TEST_BUDGET: ProjectionBudget = ProjectionBudget {
    max_points: 16,
    max_vectors_per_point: 4,
    max_vector_name_bytes: 64,
    max_stored_vector_values_per_point: 1_024,
    max_manifest_bytes: 1_048_576,
};

#[cfg(test)]
#[allow(clippy::too_many_lines)]
mod tests {
    use super::*;
    use search_contracts::NonZeroRevision;
    use search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
    use search_projection_planner::{VectorRequirement, VectorValue};
    use std::collections::BTreeMap;

    fn oid(value: &str) -> OpaqueId {
        OpaqueId::new(value).expect("opaque id")
    }

    fn digest(byte: u8) -> Blake3Digest32 {
        Blake3Digest32::from_bytes([byte; 32])
    }

    fn test_profiles() -> ProjectionProfiles {
        ProjectionProfiles {
            profile_set_id: oid("profile-set:test"),
            profile_set_digest: digest(0xA0),
            vectors: BTreeMap::from([(
                "lexical-sparse".to_owned(),
                VectorRequirement {
                    dimensions: 8,
                    sparse: true,
                },
            )]),
        }
    }

    fn test_vectors() -> Vec<NamedVector> {
        vec![NamedVector {
            name: "lexical-sparse".to_owned(),
            dimensions: 8,
            value: VectorValue::Sparse {
                indices: vec![1, 4],
                values: vec![1.0, 2.0],
            },
            digest: digest(0xD0),
        }]
    }

    fn test_scope(generation: u8) -> ScopeExpectation {
        ScopeExpectation {
            namespace_id: oid("namespace:test"),
            source_id: oid("source:test"),
            source_revision: NonZeroRevision::new(3).expect("revision"),
            source_membership_id: oid("membership:source:a"),
            projection_membership_id: oid("membership:projection:a"),
            projection_fingerprint: digest(0x07),
            projection_schema_revision: NonZeroRevision::new(2).expect("revision"),
            representation_digest: digest(0xB1),
            scoring_partition_digest: digest(0xB2),
            collection_generation_digest: Blake3Digest32::from_bytes([generation; 32]),
            residency_digest: digest(0xB4),
        }
    }

    fn test_unit(ordinal: u64) -> ComposingUnit {
        ComposingUnit {
            receipt: AdmittedUnitReceipt {
                unit_ordinal: ordinal,
                source_byte_start: ordinal * 100,
                source_byte_end: ordinal * 100 + 50,
                unit_digest: Blake3Digest32::from_bytes({
                    let mut bytes = [0xC0; 32];
                    bytes[0] = u8::try_from(ordinal).unwrap_or(u8::MAX);
                    bytes
                }),
                reference_digest: digest(0xC1),
                representation_digest: digest(0xB1),
                residency_digest: digest(0xB4),
                access_partition_digest: digest(0xB0),
            },
            vectors: test_vectors(),
        }
    }

    fn test_request(generation: u8, ordinals: &[u64]) -> CompositionRequest {
        let scope = test_scope(generation);
        let units = ordinals
            .iter()
            .map(|ordinal| test_unit(*ordinal))
            .collect::<Vec<_>>();
        let expected_units = units
            .iter()
            .map(|unit| ExpectedUnit {
                unit_ordinal: unit.receipt.unit_ordinal,
                unit_digest: unit.receipt.unit_digest,
            })
            .collect::<Vec<_>>();
        CompositionRequest {
            scope,
            visible_epoch: Epoch::new(7).expect("epoch"),
            membership: MembershipReceipt {
                source_membership_id: oid("membership:source:a"),
                projection_membership_id: oid("membership:projection:a"),
            },
            units,
            expected_units,
        }
    }

    fn test_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("eliot-search-t26-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test root");
        root
    }

    #[test]
    fn compose_persists_reloads_and_recomposes_identical_bytes() {
        let root = test_root("roundtrip");
        let profiles = test_profiles();
        let request = test_request(0xC0, &[0, 1]);
        let plan = compose_scoped_projection(
            &request,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("compose");
        assert_eq!(plan.points.len(), 2);
        let stored =
            store_projection_manifest(&root, &plan, &request.scope, TEST_BUDGET).expect("store");
        let loaded =
            load_projection_manifest_bytes(&root, &stored.reference, TEST_BUDGET).expect("load");
        assert_eq!(loaded, plan.manifest.canonical_bytes);
        // Restart-safe recomposition from the same admitted receipts.
        let recomposed = compose_scoped_projection(
            &request,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("recompose");
        assert_eq!(
            recomposed.manifest.canonical_bytes,
            plan.manifest.canonical_bytes
        );
        verify_stored_projection(&root, &stored, &recomposed, TEST_BUDGET).expect("verify");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn persist_is_idempotent_for_same_bytes_and_conflicts_on_divergence() {
        let root = test_root("immutable");
        let profiles = test_profiles();
        let request = test_request(0xC0, &[0]);
        let plan = compose_scoped_projection(
            &request,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("compose");
        let first = store_projection_manifest(&root, &plan, &request.scope, TEST_BUDGET)
            .expect("first store");
        let second = store_projection_manifest(&root, &plan, &request.scope, TEST_BUDGET)
            .expect("replay store");
        assert_eq!(first, second);
        // Same scope, different unit content: the reference must conflict,
        // never silently swap the manifest.
        let mut diverged = request;
        diverged.units[0].receipt.unit_digest = digest(0xEE);
        diverged.expected_units[0].unit_digest = digest(0xEE);
        let diverged_plan = compose_scoped_projection(
            &diverged,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("diverged compose");
        assert_eq!(
            store_projection_manifest(&root, &diverged_plan, &diverged.scope, TEST_BUDGET),
            Err(ProjectionCompositionError::ReferenceConflict)
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_wrong_residency_duplicate_propagate_typed_errors() {
        let profiles = test_profiles();
        // Missing receipt: expected set declares two units, one arrives.
        let mut missing = test_request(0xC0, &[0]);
        missing.expected_units.push(ExpectedUnit {
            unit_ordinal: 1,
            unit_digest: digest(0xE0),
        });
        assert_eq!(
            compose_scoped_projection(
                &missing,
                &profiles,
                TEST_BUDGET,
                DEFAULT_POINT_IDENTITY_LIMITS
            ),
            Err(ProjectionCompositionError::MissingUnitReceipt)
        );
        // Wrong residency on one unit.
        let mut residency = test_request(0xC0, &[0]);
        residency.units[0].receipt.residency_digest = digest(0xFF);
        assert_eq!(
            compose_scoped_projection(
                &residency,
                &profiles,
                TEST_BUDGET,
                DEFAULT_POINT_IDENTITY_LIMITS
            ),
            Err(ProjectionCompositionError::ResidencyMismatch)
        );
        // Duplicate unit role.
        let mut duplicate = test_request(0xC0, &[0]);
        duplicate.units.push(test_unit(0));
        assert_eq!(
            compose_scoped_projection(
                &duplicate,
                &profiles,
                TEST_BUDGET,
                DEFAULT_POINT_IDENTITY_LIMITS
            ),
            Err(ProjectionCompositionError::DuplicatePoint)
        );
        // Membership binding drift.
        let mut drifted = test_request(0xC0, &[0]);
        drifted.membership.projection_membership_id = oid("membership:projection:b");
        assert_eq!(
            compose_scoped_projection(
                &drifted,
                &profiles,
                TEST_BUDGET,
                DEFAULT_POINT_IDENTITY_LIMITS
            ),
            Err(ProjectionCompositionError::MembershipMismatch)
        );
    }

    #[test]
    fn one_source_two_memberships_yield_distinct_manifests() {
        let profiles = test_profiles();
        let first = compose_scoped_projection(
            &test_request(0xC0, &[0]),
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("first membership");
        let mut second_request = test_request(0xC0, &[0]);
        second_request.scope.source_membership_id = oid("membership:source:b");
        second_request.scope.projection_membership_id = oid("membership:projection:b");
        second_request.membership.source_membership_id = oid("membership:source:b");
        second_request.membership.projection_membership_id = oid("membership:projection:b");
        let second = compose_scoped_projection(
            &second_request,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("second membership");
        assert_ne!(
            first.points[0].point_id, second.points[0].point_id,
            "memberships must never alias one point set"
        );
        assert_ne!(
            first.manifest.canonical_bytes,
            second.manifest.canonical_bytes
        );
    }

    #[test]
    fn generation_change_replaces_manifest_and_conflicts_with_prior_reference() {
        let root = test_root("generation");
        let profiles = test_profiles();
        let baseline_request = test_request(0xC0, &[0]);
        let baseline = compose_scoped_projection(
            &baseline_request,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("baseline");
        let stored =
            store_projection_manifest(&root, &baseline, &baseline_request.scope, TEST_BUDGET)
                .expect("store baseline");
        let rotated_request = test_request(0xC1, &[0]);
        let rotated = compose_scoped_projection(
            &rotated_request,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("rotated");
        assert_ne!(
            baseline.points[0].point_id, rotated.points[0].point_id,
            "profile/generation change mints new identities"
        );
        // The rotated manifest lives under a new scope key; the prior
        // reference still reads back the exact baseline bytes.
        let rotated_stored =
            store_projection_manifest(&root, &rotated, &rotated_request.scope, TEST_BUDGET)
                .expect("store rotated");
        assert_ne!(
            stored.reference.scope_key,
            rotated_stored.reference.scope_key
        );
        verify_stored_projection(&root, &stored, &baseline, TEST_BUDGET).expect("baseline kept");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reordered_units_yield_identical_manifest_bytes() {
        let profiles = test_profiles();
        let mut forward = test_request(0xC0, &[0, 1]);
        forward.units.reverse();
        let forward_plan = compose_scoped_projection(
            &forward,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("forward");
        let backward_plan = compose_scoped_projection(
            &test_request(0xC0, &[0, 1]),
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("backward");
        assert_eq!(
            forward_plan.manifest.canonical_bytes,
            backward_plan.manifest.canonical_bytes
        );
    }

    #[test]
    fn reference_carries_no_source_bodies() {
        let root = test_root("reference");
        let profiles = test_profiles();
        let request = test_request(0xC0, &[0]);
        let plan = compose_scoped_projection(
            &request,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("compose");
        let stored =
            store_projection_manifest(&root, &plan, &request.scope, TEST_BUDGET).expect("store");
        let record = stored.reference.to_bytes();
        assert_eq!(record.len(), REFERENCE_BYTES);
        assert_eq!(&record[..8], REFERENCE_MAGIC);
        // The record is digests, lengths and counts only, and it round-trips
        // exactly.
        let parsed = ProjectionReference::from_bytes(&record).expect("parse");
        assert_eq!(parsed, stored.reference);
        assert_eq!(
            ProjectionReference::from_bytes(b"short"),
            Err(ProjectionCompositionError::ManifestInvalid)
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn payload_indexes_for_t24_are_exact_and_complete() {
        let indexes = expected_payload_indexes_for_bridge();
        assert_eq!(indexes.len(), 6);
        for field in [
            "source_membership_id",
            "projection_membership_id",
            "source_revision",
            "unit_ordinal",
            "visible_epoch",
            "access_partition_digest",
        ] {
            assert!(indexes.contains(&field), "missing payload index {field}");
        }
    }

    #[test]
    fn composition_error_codes_are_stable() {
        assert_eq!(
            ProjectionCompositionError::ScopeMismatch.code(),
            "PROJECTION_COMPOSITION_SCOPE_MISMATCH"
        );
        assert_eq!(
            ProjectionCompositionError::MissingUnitReceipt.code(),
            "PROJECTION_COMPOSITION_MISSING_UNIT_RECEIPT"
        );
        assert_eq!(
            ProjectionCompositionError::ReferenceConflict.code(),
            "PROJECTION_COMPOSITION_REFERENCE_CONFLICT"
        );
    }
}
