//! Security purge barriers across every storage and result plane.
//!
//! This module decides barrier order and plane completeness. It never deletes
//! anything itself: concrete control/CAS/index/handle/cache adapters execute
//! exact effects and return readback receipts through vendor-neutral ports.
//!
//! Barrier order (invariant 7): a durable live deny fence commits before any
//! deletion layer runs. The fence stays effective through every later
//! cancellation, failure, unknown outcome and process restart, and overrides
//! ordinary query snapshots and pins immediately.
//!
//! Lifecycle separation (invariant 14): ordinary retired-point reclaim
//! ([`crate::sweep`] decisions executed through the T29 reclaim path) and
//! security/legal purge are distinct owners with distinct receipts. An
//! ordinary reclaim receipt is an observation only and can never satisfy a
//! purge layer.
//!
//! T37 reuse: CAS reachability and protection decisions stay in
//! [`crate::sweep`]; this module only checks that purge CAS targets never
//! include objects shared with unaffected memberships or holds. T14 parity:
//! deletion proves logical non-accessibility plus absence readback; physical
//! secure erasure is never claimed from unlink or delete.

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, OpaqueId, PurgeFenceRevision};

use crate::{
    PhysicalEraseEvidence, PurgeCoordinator, PurgeManifest, PurgePhase, RetentionError,
    RetentionOperation,
};

/// Closed purge plane: every storage and result surface one purge must fence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PurgePlane {
    LiveDeny,
    ResultHandles,
    ResultContinuations,
    ResultCacheRanking,
    ResultOverlays,
    StorageProjection,
    StorageCas,
    StorageControlRefs,
    BackupDisposition,
    ClientRevocation,
}

impl PurgePlane {
    /// All closed planes in canonical order.
    pub const ALL: &'static [Self] = &[
        Self::LiveDeny,
        Self::ResultHandles,
        Self::ResultContinuations,
        Self::ResultCacheRanking,
        Self::ResultOverlays,
        Self::StorageProjection,
        Self::StorageCas,
        Self::StorageControlRefs,
        Self::BackupDisposition,
        Self::ClientRevocation,
    ];

    /// Stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LiveDeny => "live_deny",
            Self::ResultHandles => "result_handles",
            Self::ResultContinuations => "result_continuations",
            Self::ResultCacheRanking => "result_cache_ranking",
            Self::ResultOverlays => "result_overlays",
            Self::StorageProjection => "storage_projection",
            Self::StorageCas => "storage_cas",
            Self::StorageControlRefs => "storage_control_refs",
            Self::BackupDisposition => "backup_disposition",
            Self::ClientRevocation => "client_revocation",
        }
    }

    /// Parses a closed plane; anything else is rejected.
    pub fn parse(value: &str) -> Result<Self, RetentionError> {
        Self::ALL
            .iter()
            .copied()
            .find(|plane| plane.as_str() == value)
            .ok_or(RetentionError::InvalidPurgeManifest)
    }
}

/// Authenticated authorized purge mutation binding operation to scope/domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeAuthority {
    /// Stable operation identity with its canonical request digest.
    pub operation: RetentionOperation,
    /// Digest of the exact canonical purge scope (affected memberships/set).
    pub scope_digest: Blake3Digest32,
    /// Digest of the exact residency/authority domain.
    pub domain_digest: Blake3Digest32,
    /// Whether the caller presented an authenticated authorized mutation.
    pub authorized: bool,
}

/// Validates purge authority without ever widening deletion.
///
/// A missing authority, a reused operation identity with a different request
/// digest, or a scope/domain mismatch fails closed; the caller deletes
/// nothing on any error path.
pub fn validate_purge_authority(
    authority: &PurgeAuthority,
    expected_operation: &RetentionOperation,
    expected_scope: &Blake3Digest32,
    expected_domain: &Blake3Digest32,
) -> Result<(), RetentionError> {
    if !authority.authorized {
        return Err(RetentionError::PurgeNotAuthorized);
    }
    if authority.operation != *expected_operation
        || authority.scope_digest != *expected_scope
        || authority.domain_digest != *expected_domain
    {
        return Err(RetentionError::PurgeScopeStale);
    }
    Ok(())
}

/// Proof that the durable live deny fence for one purge is committed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeBarrierProof {
    /// Purged request identity.
    pub request_id: OpaqueId,
    /// Purged generation.
    pub purge_generation: u64,
    /// Committed purge-fence revision.
    pub fence_revision: PurgeFenceRevision,
    /// Live security generation exposing the fence.
    pub live_generation: u64,
    /// Digest of the immutable live security snapshot exposing the fence.
    pub live_snapshot_digest: Blake3Digest32,
}

/// Requires the live deny fence before any deletion layer runs.
///
/// Succeeds only when the coordinator already committed
/// [`PurgePhase::LiveDenyCommitted`] (or a strictly later success phase) and
/// the barrier proof binds the coordinator's exact request, generation and
/// fence revision. Every other combination fails closed with
/// [`RetentionError::LiveDenyReceiptMissing`]; unknown-outcome and
/// quarantined phases never authorize deletion.
pub fn require_live_deny_before_delete(
    coordinator: &PurgeCoordinator,
    barrier: Option<&PurgeBarrierProof>,
) -> Result<(), RetentionError> {
    let barrier = barrier.ok_or(RetentionError::LiveDenyReceiptMissing)?;
    let manifest = coordinator.manifest();
    if barrier.request_id != manifest.request_id {
        return Err(RetentionError::LiveDenyReceiptMissing);
    }
    if barrier.purge_generation != manifest.purge_generation {
        return Err(RetentionError::LiveDenyReceiptMissing);
    }
    if barrier.fence_revision != manifest.purge_fence_revision {
        return Err(RetentionError::LiveDenyReceiptMissing);
    }
    if !matches!(
        coordinator.phase(),
        PurgePhase::LiveDenyCommitted
            | PurgePhase::HandlesInvalidated
            | PurgePhase::IndexDeleted
            | PurgePhase::CacheDeleted
            | PurgePhase::SearchObjectsDeleted
            | PurgePhase::BackupDispositionRecorded
    ) {
        return Err(RetentionError::LiveDenyReceiptMissing);
    }
    Ok(())
}

/// Content-free result-plane invalidation set for one purge.
///
/// Carries identities, generations and digests only — never content or
/// generic broad vendor filters. Handles, continuations, cached ranking and
/// overlay views are each revoked by their owning layer; this set only
/// records that every owner reported back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeInvalidationSet {
    /// Purged request identity.
    pub request_id: OpaqueId,
    /// Purged generation.
    pub purge_generation: u64,
    /// Handle/expansion invalidation reported by the handle owner.
    pub handles_revoked: bool,
    /// Continuation invalidation reported by the continuation owner.
    pub continuations_revoked: bool,
    /// Cached candidates, IDF populations, counts and traces dropped.
    pub cache_ranking_dropped: bool,
    /// Overlay views detached from purged bytes.
    pub overlays_detached: bool,
    /// Binding digest over the exact invalidation set.
    pub invalidation_digest: Blake3Digest32,
}

/// Verifies result-plane invalidation completeness.
///
/// Every result-plane owner must have reported; one missing or foreign report
/// keeps the purge fail-closed with
/// [`RetentionError::InvalidationIncomplete`].
pub fn verify_purge_invalidation(
    set: &PurgeInvalidationSet,
    manifest: &PurgeManifest,
) -> Result<(), RetentionError> {
    if set.request_id != manifest.request_id
        || set.purge_generation != manifest.purge_generation
        || !set.handles_revoked
        || !set.continuations_revoked
        || !set.cache_ranking_dropped
        || !set.overlays_detached
    {
        return Err(RetentionError::InvalidationIncomplete);
    }
    Ok(())
}

/// Rejects an ordinary reclaim receipt presented as purge index evidence.
///
/// Invariant 14: ordinary retired-point reclaim (T29 execute path) and
/// security purge are separate owners with separate receipts. The purge
/// projection layer requires the security-purge index-admin path with its own
/// purge receipt; `true` (an ordinary reclaim receipt was presented) always
/// fails with [`RetentionError::IndexDeletionIncomplete`].
pub const fn reject_ordinary_reclaim_as_purge(
    is_ordinary_reclaim_receipt: bool,
) -> Result<(), RetentionError> {
    if is_ordinary_reclaim_receipt {
        return Err(RetentionError::IndexDeletionIncomplete);
    }
    Ok(())
}

/// Checks purge CAS targets against objects shared with unaffected scopes.
///
/// Reuses T37 sweep protection semantics without duplicating mark logic: both
/// sets are exact [`OpaqueId`] sets, and any purge target that is also
/// reachable from an unaffected membership, hold, lease or tombstone stays
/// protected ([`RetentionError::SweepProtectedObjectConflict`]). Shared
/// objects remain inaccessible to the purged scope through the live deny
/// fence while staying alive for their remaining owners.
pub fn check_purge_cas_targets(
    target_ids: &BTreeSet<OpaqueId>,
    shared_protected: &BTreeSet<OpaqueId>,
) -> Result<(), RetentionError> {
    if target_ids.is_disjoint(shared_protected) {
        Ok(())
    } else {
        Err(RetentionError::SweepProtectedObjectConflict)
    }
}

/// Recovery directive for a purge interrupted mid-phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PurgeResumeDirective {
    /// Resume exactly the saved phase after exact readback; access stays denied.
    ResumeFrom(PurgePhase),
    /// The live fence no longer exposes the purge: re-fence before deletion.
    RefenceRequired,
    /// Contradictory or terminal state: quarantine, never resume deletion.
    Quarantine,
}

/// Directs purge recovery after a crash, cancellation or disconnect.
///
/// Terminal phases never resume. Generation drift quarantines instead of
/// ignoring the new root, hold, pin or publication state. A lost live fence
/// requires re-fencing before any further deletion; otherwise recovery
/// resumes the exact saved phase, with the deny fence still effective.
/// `OutcomeUnknown` resumes only into exact readback, never into success.
#[must_use]
pub const fn direct_purge_resume(
    saved_phase: PurgePhase,
    fence_still_live: bool,
    generation_matches: bool,
) -> PurgeResumeDirective {
    if matches!(saved_phase, PurgePhase::Complete | PurgePhase::Quarantined) {
        return PurgeResumeDirective::Quarantine;
    }
    if !generation_matches {
        return PurgeResumeDirective::Quarantine;
    }
    if !fence_still_live {
        return PurgeResumeDirective::RefenceRequired;
    }
    PurgeResumeDirective::ResumeFrom(saved_phase)
}

/// Per-plane resolution status for the terminal completion gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PurgePlaneStatus {
    /// Plane this status reports for.
    pub plane: PurgePlane,
    /// Whether the plane reported exact verified completion.
    pub resolved: bool,
    /// Whether the plane outcome is unknown (timeout/cancel/disconnect).
    pub outcome_unknown: bool,
}

/// Requires every required plane resolved before the terminal receipt.
///
/// No complete purge receipt exists while any required plane is missing,
/// unresolved or unknown: the gate fails with [`RetentionError::PurgePartial`]
/// and only explicitly allowed content-free unresolved-effect evidence is
/// retained. The status list is bounded by the closed plane registry; a
/// longer list is a forged manifest, never silently truncated.
pub fn require_all_planes_resolved(
    statuses: &[PurgePlaneStatus],
    required: &[PurgePlane],
) -> Result<(), RetentionError> {
    if statuses.len() > PurgePlane::ALL.len() || required.len() > PurgePlane::ALL.len() {
        return Err(RetentionError::InvalidPurgeManifest);
    }
    for plane in required {
        let mut matches = statuses.iter().filter(|status| status.plane == *plane);
        let Some(only) = matches.next() else {
            return Err(RetentionError::PurgePartial);
        };
        if matches.next().is_some() {
            return Err(RetentionError::InvalidPurgeManifest);
        }
        if !only.resolved || only.outcome_unknown {
            return Err(RetentionError::PurgePartial);
        }
    }
    Ok(())
}

/// Asserts the honest physical-erasure limitation (T14 parity).
///
/// Purge proves logical non-accessibility plus absence readback. Only
/// [`PhysicalEraseEvidence::NotGuaranteed`] passes; any
/// `EvidenceAvailable` claim derived from unlink or delete fails with
/// [`RetentionError::SecureEraseEvidenceMissing`] instead of relabeling a
/// partial outcome as success.
pub const fn assert_logical_only(physical: &PhysicalEraseEvidence) -> Result<(), RetentionError> {
    match physical {
        PhysicalEraseEvidence::NotGuaranteed => Ok(()),
        PhysicalEraseEvidence::EvidenceAvailable { .. } => {
            Err(RetentionError::SecureEraseEvidenceMissing)
        }
    }
}
