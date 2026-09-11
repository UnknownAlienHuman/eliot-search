//! T29 route-rebuild, epoch-pin and safe-reclamation composition.
//!
//! This module composes three accepted pieces into one rebuild story without
//! owning any of them: canonical retained manifests (T26 projection output,
//! carried here as [`RetainedManifest`]), the process-local pin registry
//! (`search-epoch-pins`), and exact-ID ordinary reclaim
//! (`search-index-reclaimer`). It performs no transport of its own: the
//! caller moves bytes through the real data plane and hands back exact
//! readback views for verification.
//!
//! The ordered story per rebuild is:
//!
//! 1. [`propose_rebuild`] plans a new collection generation from a validated
//!    retained manifest. The source manifest is borrowed read-only, so a loss
//!    can never delete source truth through this module.
//! 2. The caller recreates the generation on the backend and upserts the
//!    retained points through its own data plane.
//! 3. [`verify_full_readback`] proves every retained point present with
//!    matching digests and nothing unexpected before anything cuts over.
//! 4. [`stage_cutover`] plus [`commit_cutover`] publish exactly one route
//!    cutover. A staged cutover alone never commits: after an interruption or
//!    restart the caller must re-verify the backend and commit with a fresh
//!    proof. Orphan backend state is never consulted for currentness.
//! 5. [`authorize_reclaim`] plans ordinary reclaim of retired points only
//!    when the pin watermark proves no active query can observe them. Purge
//!    is a separate lifecycle path: [`check_ordinary_receipt`] accepts only
//!    the ordinary reclaim kind and fails closed on anything else.
//!
//! Wiring (integration owner): add to `entry.rs`
//!
//! ```text
//! #[cfg(feature = "wave3-index")]
//! mod rebuild_composition;
//! ```
//!
//! What this module never does:
//!
//! - No broad-filter deletion: reclaim travels only through
//!   `search-index-reclaimer` exact-ID batches.
//! - No security purge: there is no purge constructor, plan, or receipt here.
//! - No second search database: the only indexed store is Qdrant.
//! - No durable query pins: the registry stays process-local by construction.
//! - No silent fallback: every contradiction is a typed [`RebuildError`].

#![forbid(unsafe_code)]

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, CollectionRouteRevision, Epoch, OpaqueId,
};
use search_epoch_pins::{
    EpochPinGuard, EpochPinPurpose, ExpiryReceipt, PinError, PinRegistry, PinReleaseReceipt,
    ReclamationWatermark, RetiredVisibilityFence, RouteIdentity, compute_reclamation_watermark,
};
use search_index_reclaimer::{
    CommittedRetiredManifest, PublicationCommitProof, ReclaimBudget, ReclaimError, ReclaimPlan,
    ReclaimReceipt, ReclaimReceiptKind, ReclaimSettings, RetiredPointManifest,
};

/// Closed rebuild-composition failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RebuildError {
    /// A finite limit is zero or internally inconsistent.
    InvalidLimits,
    /// Retained points are empty-handed where content is required, duplicated,
    /// or not in canonical order.
    ManifestNotCanonical,
    /// A manifest digest does not recompute from its exact content.
    DigestMismatch,
    /// The presented route is not the active route, or a reclaim caller named
    /// a route that does not own the retired manifest.
    StaleRoute,
    /// The presented epoch is not visible, or a revision is not the exact
    /// successor of the route it replaces.
    StaleRevision,
    /// A manifest generation does not match the caller generation.
    GenerationMismatch,
    /// A rebuild proposed the same generation it replaces.
    GenerationReuse,
    /// Active route or epoch pins can still observe the retired state.
    StillPinned,
    /// A backend readback does not prove the full planned point set.
    ReadbackMismatch,
    /// A staged cutover and its commit proof name different plans.
    CutoverMismatch,
    /// A finite point or batch budget was exceeded.
    BudgetExceeded,
    /// A retired manifest does not match its committed publication proof.
    PublicationMismatch,
    /// A pin-registry failure with no narrower rebuild mapping.
    PinDenied(PinError),
    /// An exact-reclaim failure with no narrower rebuild mapping.
    ReclaimDenied(ReclaimError),
}

impl RebuildError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "REBUILD_INVALID_LIMITS",
            Self::ManifestNotCanonical => "REBUILD_MANIFEST_NOT_CANONICAL",
            Self::DigestMismatch => "REBUILD_DIGEST_MISMATCH",
            Self::StaleRoute => "REBUILD_STALE_ROUTE",
            Self::StaleRevision => "REBUILD_STALE_REVISION",
            Self::GenerationMismatch => "REBUILD_GENERATION_MISMATCH",
            Self::GenerationReuse => "REBUILD_GENERATION_REUSE",
            Self::StillPinned => "REBUILD_STILL_PINNED",
            Self::ReadbackMismatch => "REBUILD_READBACK_MISMATCH",
            Self::CutoverMismatch => "REBUILD_CUTOVER_MISMATCH",
            Self::BudgetExceeded => "REBUILD_BUDGET_EXCEEDED",
            Self::PublicationMismatch => "REBUILD_PUBLICATION_MISMATCH",
            Self::PinDenied(inner) => inner.code(),
            Self::ReclaimDenied(inner) => inner.code(),
        }
    }
}

impl core::fmt::Display for RebuildError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RebuildError {}

/// One canonically retained index point: identity plus both content digests.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetainedPoint {
    /// Exact 128-bit point identifier.
    pub id: [u8; 16],
    /// Digest of the exact stored payload.
    pub payload_digest: Blake3Digest32,
    /// Digest of the exact point identity.
    pub identity_digest: Blake3Digest32,
}

/// Canonical retained manifest a rebuild replays.
///
/// The manifest is the source truth stand-in for one collection generation:
/// [`propose_rebuild`] and [`verify_full_readback`] borrow it read-only, so
/// backend loss can never mutate or delete it through this module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedManifest {
    /// Physical collection generation the points were projected for.
    pub generation: CollectionGenerationId,
    /// Logical route revision the points were projected for.
    pub route_revision: CollectionRouteRevision,
    /// Digest over the exact canonical point encoding.
    pub manifest_digest: Blake3Digest32,
    /// Canonically ordered retained points.
    pub points: Vec<RetainedPoint>,
}

/// Computes the exact canonical manifest digest with real BLAKE3.
///
/// Canonical encoding: domain tag, point count as little-endian `u64`, then
/// per point the identifier, payload digest, and identity digest in order.
#[must_use]
pub fn retained_manifest_digest(points: &[RetainedPoint]) -> Blake3Digest32 {
    let mut input = Vec::with_capacity(32 + 8 + points.len().saturating_mul(16 + 32 + 32));
    input.extend_from_slice(b"eliot-search/rebuild-manifest/v1\x00");
    input.extend_from_slice(&points.len().to_le_bytes());
    for point in points {
        input.extend_from_slice(&point.id);
        input.extend_from_slice(point.payload_digest.as_bytes());
        input.extend_from_slice(point.identity_digest.as_bytes());
    }
    Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes())
}

/// Validates canonical order, uniqueness, and digest of a retained manifest.
///
/// # Errors
///
/// Returns [`RebuildError::ManifestNotCanonical`] for duplicated or unordered
/// points, or [`RebuildError::DigestMismatch`] when the digest does not
/// recompute from the exact content.
pub fn validate_retained_manifest(manifest: &RetainedManifest) -> Result<(), RebuildError> {
    if manifest
        .points
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(RebuildError::ManifestNotCanonical);
    }
    if retained_manifest_digest(&manifest.points) != manifest.manifest_digest {
        return Err(RebuildError::DigestMismatch);
    }
    Ok(())
}

/// Finite rebuild tuning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RebuildBudget {
    /// Maximum retained points in one rebuild plan.
    pub max_points: usize,
    /// Maximum exact-ID batches in one rebuild plan.
    pub max_batches: usize,
}

/// One exact rebuild plan: the new generation and its deterministic batches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebuildPlan {
    /// Route being replaced.
    pub old_route: RouteIdentity,
    /// Fresh physical collection generation under construction.
    pub new_generation: CollectionGenerationId,
    /// Exact successor of the replaced route revision.
    pub new_revision: CollectionRouteRevision,
    /// Retained-manifest digest under replay.
    pub manifest_digest: Blake3Digest32,
    /// Deterministic exact-ID batches in canonical order.
    pub batches: Vec<Vec<[u8; 16]>>,
    /// Digest binding the old route, new generation, manifest, and batching.
    pub plan_digest: Blake3Digest32,
}

fn rebuild_plan_digest(
    old_route: RouteIdentity,
    new_generation: CollectionGenerationId,
    new_revision: CollectionRouteRevision,
    manifest_digest: Blake3Digest32,
    batch_size: usize,
) -> Result<Blake3Digest32, RebuildError> {
    let batch_size = u64::try_from(batch_size).map_err(|_| RebuildError::BudgetExceeded)?;
    let mut input = Vec::with_capacity(32 + 16 + 8 + 16 + 8 + 32 + 8);
    input.extend_from_slice(b"eliot-search/rebuild-plan/v1\x00");
    input.extend_from_slice(old_route.collection_generation_id.as_bytes());
    input.extend_from_slice(&old_route.route_revision.get().to_le_bytes());
    input.extend_from_slice(new_generation.as_bytes());
    input.extend_from_slice(&new_revision.get().to_le_bytes());
    input.extend_from_slice(manifest_digest.as_bytes());
    input.extend_from_slice(&batch_size.to_le_bytes());
    Ok(Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes()))
}

/// Plans a new collection generation from a validated retained manifest.
///
/// The manifest is borrowed read-only: planning never mutates source truth.
/// The new revision must be exactly the successor of the replaced route, and
/// the new generation must differ from the old one.
///
/// # Errors
///
/// Returns [`RebuildError::GenerationReuse`] for a recycled generation,
/// [`RebuildError::StaleRevision`] for a skipped or reused revision,
/// [`RebuildError::GenerationMismatch`] when the manifest belongs to another
/// generation, or the manifest validation failures for bad content.
pub fn propose_rebuild(
    old_route: RouteIdentity,
    new_generation: CollectionGenerationId,
    new_revision: CollectionRouteRevision,
    manifest: &RetainedManifest,
    batch_size: usize,
    budget: RebuildBudget,
) -> Result<RebuildPlan, RebuildError> {
    if batch_size == 0 || budget.max_points == 0 || budget.max_batches == 0 {
        return Err(RebuildError::InvalidLimits);
    }
    if new_generation == old_route.collection_generation_id {
        return Err(RebuildError::GenerationReuse);
    }
    let expected_revision = old_route
        .route_revision
        .checked_next()
        .map_err(|_| RebuildError::StaleRevision)?;
    if new_revision != expected_revision {
        return Err(RebuildError::StaleRevision);
    }
    if manifest.generation != old_route.collection_generation_id {
        return Err(RebuildError::GenerationMismatch);
    }
    if manifest.route_revision != old_route.route_revision {
        return Err(RebuildError::GenerationMismatch);
    }
    validate_retained_manifest(manifest)?;
    if manifest.points.len() > budget.max_points {
        return Err(RebuildError::BudgetExceeded);
    }
    let batch_count = manifest.points.len().div_ceil(batch_size);
    if batch_count > budget.max_batches {
        return Err(RebuildError::BudgetExceeded);
    }
    let mut batches = Vec::with_capacity(batch_count);
    for chunk in manifest.points.chunks(batch_size) {
        batches.push(chunk.iter().map(|point| point.id).collect());
    }
    let plan_digest = rebuild_plan_digest(
        old_route,
        new_generation,
        new_revision,
        manifest.manifest_digest,
        batch_size,
    )?;
    Ok(RebuildPlan {
        old_route,
        new_generation,
        new_revision,
        manifest_digest: manifest.manifest_digest,
        batches,
        plan_digest,
    })
}

/// Caller-observed backend state over the planned point set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexReadbackView {
    /// Points found with their live digests.
    pub present: Vec<RetainedPoint>,
    /// Planned identifiers with no stored point.
    pub missing: Vec<[u8; 16]>,
    /// Points returned for unplanned identifiers.
    pub unexpected: Vec<[u8; 16]>,
}

/// Proof that the backend holds exactly the planned generation content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullReadbackProof {
    /// Plan the proof was verified against.
    pub plan_digest: Blake3Digest32,
    /// Number of exactly verified points.
    pub verified_points: usize,
}

/// Verifies the backend holds exactly the retained point set.
///
/// Every planned identifier must be present with matching payload and
/// identity digests; any missing or unexpected identifier fails closed. The
/// resulting proof is the only value [`stage_cutover`] and [`commit_cutover`]
/// accept.
///
/// # Errors
///
/// Returns [`RebuildError::ReadbackMismatch`] for a foreign manifest, any
/// missing or unexpected identifier, or a digest contradiction.
pub fn verify_full_readback(
    plan: &RebuildPlan,
    manifest: &RetainedManifest,
    readback: &IndexReadbackView,
) -> Result<FullReadbackProof, RebuildError> {
    if manifest.manifest_digest != plan.manifest_digest {
        return Err(RebuildError::ReadbackMismatch);
    }
    if !readback.missing.is_empty() || !readback.unexpected.is_empty() {
        return Err(RebuildError::ReadbackMismatch);
    }
    let mut expected: Vec<RetainedPoint> = manifest.points.clone();
    expected.sort();
    let mut observed = readback.present.clone();
    observed.sort();
    if expected != observed {
        return Err(RebuildError::ReadbackMismatch);
    }
    Ok(FullReadbackProof {
        plan_digest: plan.plan_digest,
        verified_points: expected.len(),
    })
}

/// Staged but uncommitted route cutover.
///
/// A staged value authorizes nothing on its own: [`commit_cutover`] still
/// requires a fresh [`FullReadbackProof`], so an interrupted cutover or a
/// restart can never publish without re-verifying the backend. The token is
/// deliberately not `Copy`: dropping it models an interruption, and only an
/// explicit fresh proof commits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedCutover {
    /// Plan the cutover was staged for.
    pub plan_digest: Blake3Digest32,
    /// Route being replaced.
    pub old_route: RouteIdentity,
    /// Fresh generation under construction.
    pub new_generation: CollectionGenerationId,
    /// Exact successor revision under construction.
    pub new_revision: CollectionRouteRevision,
    /// Digest binding the staged cutover record.
    pub staged_digest: Blake3Digest32,
}

/// Committed route cutover: the single verified publication of one rebuild.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommittedCutover {
    /// Plan the cutover committed.
    pub plan_digest: Blake3Digest32,
    /// Published generation.
    pub new_generation: CollectionGenerationId,
    /// Published route revision.
    pub new_revision: CollectionRouteRevision,
    /// Digest binding the commit record to its readback proof.
    pub commit_digest: Blake3Digest32,
}

/// Stages one cutover after a full backend readback.
///
/// # Errors
///
/// Returns [`RebuildError::CutoverMismatch`] when the proof was verified
/// against another plan.
pub fn stage_cutover(
    plan: &RebuildPlan,
    proof: &FullReadbackProof,
) -> Result<StagedCutover, RebuildError> {
    if proof.plan_digest != plan.plan_digest {
        return Err(RebuildError::CutoverMismatch);
    }
    let mut input = Vec::with_capacity(32 + 32 + 16 + 16 + 8);
    input.extend_from_slice(b"eliot-search/rebuild-staged/v1\x00");
    input.extend_from_slice(plan.plan_digest.as_bytes());
    input.extend_from_slice(plan.old_route.collection_generation_id.as_bytes());
    input.extend_from_slice(plan.new_generation.as_bytes());
    input.extend_from_slice(&plan.new_revision.get().to_le_bytes());
    Ok(StagedCutover {
        plan_digest: plan.plan_digest,
        old_route: plan.old_route,
        new_generation: plan.new_generation,
        new_revision: plan.new_revision,
        staged_digest: Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes()),
    })
}

/// Commits a staged cutover under a fresh full-readback proof.
///
/// Staging and committing take the same proof type on purpose: after an
/// interruption or restart the caller re-runs [`verify_full_readback`] and
/// commits with the new proof, never with the pre-crash memory.
///
/// # Errors
///
/// Returns [`RebuildError::CutoverMismatch`] when the proof names another
/// plan than the staged cutover.
pub fn commit_cutover(
    staged: &StagedCutover,
    proof: &FullReadbackProof,
) -> Result<CommittedCutover, RebuildError> {
    if proof.plan_digest != staged.plan_digest {
        return Err(RebuildError::CutoverMismatch);
    }
    let verified =
        u64::try_from(proof.verified_points).map_err(|_| RebuildError::BudgetExceeded)?;
    let mut input = Vec::with_capacity(32 + 32 + 8);
    input.extend_from_slice(b"eliot-search/rebuild-commit/v1\x00");
    input.extend_from_slice(staged.staged_digest.as_bytes());
    input.extend_from_slice(&verified.to_le_bytes());
    Ok(CommittedCutover {
        plan_digest: staged.plan_digest,
        new_generation: staged.new_generation,
        new_revision: staged.new_revision,
        commit_digest: Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes()),
    })
}

/// Exact query session: owner, route, and epoch presented together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuerySession {
    /// Request, connection, or continuation identity owning the pin.
    pub owner: OpaqueId,
    /// Route the caller believes is active.
    pub route: RouteIdentity,
    /// Epoch the caller wants to observe.
    pub epoch: Epoch,
}

/// Acquires an epoch pin for one query session.
///
/// A session presenting a rotated (stale) route or a non-visible epoch is
/// denied with a typed error; the caller releases the returned guard by drop
/// on every terminal path (success, cancellation, error, disconnect) or
/// explicitly through [`release_owner_pins_of`].
///
/// # Errors
///
/// Returns [`RebuildError::StaleRoute`] for a rotated route,
/// [`RebuildError::StaleRevision`] for a non-visible epoch,
/// [`RebuildError::BudgetExceeded`] for exhausted pin capacity, or
/// [`RebuildError::PinDenied`] for any other registry failure.
pub fn begin_pinned_query(
    registry: &PinRegistry,
    session: &QuerySession,
    purpose: EpochPinPurpose,
    now_ms: u64,
) -> Result<EpochPinGuard, RebuildError> {
    registry
        .acquire_epoch_pin(
            session.route,
            session.epoch,
            session.owner.clone(),
            purpose,
            now_ms,
        )
        .map_err(|error| match error {
            PinError::RouteNotActive => RebuildError::StaleRoute,
            PinError::EpochNotVisible => RebuildError::StaleRevision,
            PinError::RegistryCapacityExceeded | PinError::OwnerCapacityExceeded => {
                RebuildError::BudgetExceeded
            }
            other => RebuildError::PinDenied(other),
        })
}

/// Idempotently releases every pin owned by one cancelled or disconnected
/// session.
///
/// # Errors
///
/// Returns [`RebuildError::PinDenied`] when the registry lock is poisoned;
/// reclamation then stays fail-closed.
pub fn release_owner_pins_of(
    registry: &PinRegistry,
    owner: &OpaqueId,
) -> Result<PinReleaseReceipt, RebuildError> {
    registry
        .release_owner_pins(owner)
        .map_err(RebuildError::PinDenied)
}

/// Expires bounded continuation pins at an explicit caller-supplied time.
///
/// The sweep is bounded by `max_expirations`: expiry never loops unboundedly.
///
/// # Errors
///
/// Returns [`RebuildError::InvalidLimits`] for a zero sweep bound, or
/// [`RebuildError::PinDenied`] when the registry lock is poisoned.
pub fn expire_continuation_pins_bounded(
    registry: &PinRegistry,
    now_ms: u64,
    max_expirations: usize,
) -> Result<ExpiryReceipt, RebuildError> {
    registry
        .expire_continuation_pins(now_ms, max_expirations)
        .map_err(|error| match error {
            PinError::InvalidLimits => RebuildError::InvalidLimits,
            other => RebuildError::PinDenied(other),
        })
}

/// Finite reclaim tuning for one ordinary reclaim authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReclaimTuning {
    /// Exact point identifiers per delete/readback batch.
    pub batch_size: usize,
    /// Maximum identifiers in one plan.
    pub max_points: usize,
    /// Maximum batches in one plan.
    pub max_batches: usize,
}

/// Authorizes ordinary reclaim of a committed retired manifest.
///
/// The caller route must own the manifest, the caller generation must match
/// the manifest generation, and the live pin watermark must prove no active
/// query can observe the retired points. Security purge never reaches this
/// function: there is no purge intent value to pass.
///
/// # Errors
///
/// Returns [`RebuildError::StaleRoute`] for a foreign caller route,
/// [`RebuildError::GenerationMismatch`] for a foreign caller generation,
/// [`RebuildError::StillPinned`] while any pin observes the retired state,
/// or the manifest, publication, and budget failures for bad content.
pub fn authorize_reclaim(
    manifest: RetiredPointManifest,
    publication: &PublicationCommitProof,
    caller_route: RouteIdentity,
    caller_generation: CollectionGenerationId,
    registry: &PinRegistry,
    tuning: ReclaimTuning,
) -> Result<ReclaimPlan, RebuildError> {
    if manifest.route != caller_route {
        return Err(RebuildError::StaleRoute);
    }
    if manifest.collection_generation_id != caller_generation {
        return Err(RebuildError::GenerationMismatch);
    }
    let committed: CommittedRetiredManifest =
        search_index_reclaimer::validate_retired_manifest(manifest, publication).map_err(
            |error| match error {
                ReclaimError::PublicationMismatch => RebuildError::PublicationMismatch,
                ReclaimError::EmptyManifest | ReclaimError::InvalidPointSet => {
                    RebuildError::ManifestNotCanonical
                }
                other => RebuildError::ReclaimDenied(other),
            },
        )?;
    let snapshot = registry.snapshot().map_err(RebuildError::PinDenied)?;
    let fence = RetiredVisibilityFence {
        route: committed.manifest().route,
        retirement_epoch_exclusive: committed.manifest().retirement_epoch_exclusive,
    };
    let watermark: ReclamationWatermark = compute_reclamation_watermark(fence, &snapshot);
    if tuning.batch_size == 0 || tuning.max_points == 0 || tuning.max_batches == 0 {
        return Err(RebuildError::InvalidLimits);
    }
    search_index_reclaimer::plan(
        committed,
        watermark,
        ReclaimSettings {
            batch_size: tuning.batch_size,
        },
        ReclaimBudget {
            max_points: tuning.max_points,
            max_batches: tuning.max_batches,
        },
    )
    .map_err(|error| match error {
        ReclaimError::StillPinned => RebuildError::StillPinned,
        ReclaimError::BudgetExceeded => RebuildError::BudgetExceeded,
        other => RebuildError::ReclaimDenied(other),
    })
}

/// Reports whether a receipt is ordinary retired-point reclaim.
///
/// The match names the single accepted variant without a wildcard, so adding
/// any future receipt kind (including a purge acknowledgement) fails to
/// compile here instead of silently passing as ordinary reclaim.
#[must_use]
pub const fn is_ordinary_reclaim_receipt(receipt: &ReclaimReceipt) -> bool {
    match receipt.kind {
        ReclaimReceiptKind::OrdinaryRetiredPointReclaim => true,
    }
}
