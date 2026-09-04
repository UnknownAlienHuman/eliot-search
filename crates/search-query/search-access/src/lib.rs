//! Server-authoritative access validation and live restrictive security fences.
//!
//! This package exposes no Qdrant filter, collection name, point ID, or reusable
//! authorization decision. Every permit is bound to one captured request fence
//! and must be rechecked at each load-bearing checkpoint.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    BindingId, Blake3Digest32, CollectionGenerationId, Epoch, GrantId,
    InstallationId, InstallationIncarnationId, OpaqueId, OwnerEpoch, RecipeIdV1,
    SourceMembershipId,
};

/// Closed access failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessError {
    GrantSignatureInvalid,
    PairingInvalid,
    GrantBindingMismatch,
    InstallationMismatch,
    GrantBootMismatch,
    GrantExpired,
    GrantNonceInvalid,
    GrantRevoked,
    RecipeDenied,
    ModalityDenied,
    BudgetClassDenied,
    ScopeUnknown,
    ScopeUnauthorized,
    AuthorizedScopeEmpty,
    SnapshotStale,
    RouteMismatch,
    OverlapProofMissing,
    RetrievalLegBudgetExceeded,
    SecurityOperationConflict,
    SecurityGenerationRegression,
    SecurityFailClosed,
    LiveRevocation,
    LivePurge,
    SecurityFenceStale,
    ContaminatedExecution,
}

impl AccessError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::GrantSignatureInvalid => "ACCESS_GRANT_SIGNATURE_INVALID",
            Self::PairingInvalid => "ACCESS_PAIRING_INVALID",
            Self::GrantBindingMismatch => "ACCESS_GRANT_BINDING_MISMATCH",
            Self::InstallationMismatch => "ACCESS_INSTALLATION_MISMATCH",
            Self::GrantBootMismatch => "ACCESS_GRANT_BOOT_MISMATCH",
            Self::GrantExpired => "ACCESS_GRANT_EXPIRED",
            Self::GrantNonceInvalid => "ACCESS_GRANT_NONCE_INVALID",
            Self::GrantRevoked => "ACCESS_GRANT_REVOKED",
            Self::RecipeDenied => "ACCESS_RECIPE_DENIED",
            Self::ModalityDenied => "ACCESS_MODALITY_DENIED",
            Self::BudgetClassDenied => "ACCESS_BUDGET_CLASS_DENIED",
            Self::ScopeUnknown => "ACCESS_SCOPE_UNKNOWN",
            Self::ScopeUnauthorized => "ACCESS_SCOPE_UNAUTHORIZED",
            Self::AuthorizedScopeEmpty => "ACCESS_SCOPE_EMPTY",
            Self::SnapshotStale => "ACCESS_SNAPSHOT_STALE",
            Self::RouteMismatch => "ACCESS_ROUTE_MISMATCH",
            Self::OverlapProofMissing => "ACCESS_OVERLAP_PROOF_MISSING",
            Self::RetrievalLegBudgetExceeded => "ACCESS_LEG_BUDGET_EXCEEDED",
            Self::SecurityOperationConflict => "ACCESS_SECURITY_OPERATION_CONFLICT",
            Self::SecurityGenerationRegression => "ACCESS_SECURITY_GENERATION_REGRESSION",
            Self::SecurityFailClosed => "ACCESS_SECURITY_FAIL_CLOSED",
            Self::LiveRevocation => "ACCESS_LIVE_REVOKED",
            Self::LivePurge => "ACCESS_LIVE_PURGED",
            Self::SecurityFenceStale => "ACCESS_SECURITY_FENCE_STALE",
            Self::ContaminatedExecution => "ACCESS_EXECUTION_CONTAMINATED",
        }
    }
}

impl fmt::Display for AccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AccessError {}

/// Closed retrieval modality granted to a request.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessModality {
    Direct,
    Lexical,
    Exact,
    Code,
    Semantic,
    Document,
}

/// Signed/pairing-verified grant claims before local binding checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantClaims {
    pub grant_id: GrantId,
    pub binding_id: BindingId,
    pub installation_id: InstallationId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub issued_boot_id: OpaqueId,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub nonce: OpaqueId,
    pub revocation_generation: u64,
    pub allowed_recipes: BTreeSet<RecipeIdV1>,
    pub allowed_modalities: BTreeSet<AccessModality>,
    pub allowed_budget_classes: BTreeSet<OpaqueId>,
    pub max_source_read_bytes: u64,
    pub max_result_bytes: u64,
}

/// Current server-side binding and verification observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantValidationContext {
    pub binding_id: BindingId,
    pub installation_id: InstallationId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub boot_id: OpaqueId,
    pub now_ms: u64,
    pub current_revocation_generation: u64,
    pub signature_verified: bool,
    pub pairing_verified: bool,
    pub nonce_accepted: bool,
}

/// Exact locally accepted grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedGrant(GrantClaims);

impl ValidatedGrant {
    #[must_use]
    pub const fn claims(&self) -> &GrantClaims {
        &self.0
    }

    #[must_use]
    pub fn permits_recipe(&self, recipe: RecipeIdV1) -> bool {
        self.0.allowed_recipes.contains(&recipe)
    }

    #[must_use]
    pub fn permits_modality(&self, modality: AccessModality) -> bool {
        self.0.allowed_modalities.contains(&modality)
    }
}

/// Validates signature/pairing, local identity, expiry, nonce, and revocation.
pub fn validate_grant(
    claims: GrantClaims,
    context: &GrantValidationContext,
) -> Result<ValidatedGrant, AccessError> {
    if !context.signature_verified {
        return Err(AccessError::GrantSignatureInvalid);
    }
    if !context.pairing_verified {
        return Err(AccessError::PairingInvalid);
    }
    if claims.binding_id != context.binding_id {
        return Err(AccessError::GrantBindingMismatch);
    }
    if claims.installation_id != context.installation_id
        || claims.installation_incarnation_id != context.installation_incarnation_id
    {
        return Err(AccessError::InstallationMismatch);
    }
    if claims.issued_boot_id != context.boot_id {
        return Err(AccessError::GrantBootMismatch);
    }
    if claims.expires_at_ms <= claims.issued_at_ms
        || context.now_ms < claims.issued_at_ms
        || context.now_ms >= claims.expires_at_ms
    {
        return Err(AccessError::GrantExpired);
    }
    if !context.nonce_accepted {
        return Err(AccessError::GrantNonceInvalid);
    }
    if claims.revocation_generation != context.current_revocation_generation {
        return Err(AccessError::GrantRevoked);
    }
    if claims.allowed_recipes.is_empty()
        || claims.allowed_modalities.is_empty()
        || claims.allowed_budget_classes.is_empty()
        || claims.max_source_read_bytes == 0
        || claims.max_result_bytes == 0
    {
        return Err(AccessError::ScopeUnauthorized);
    }
    Ok(ValidatedGrant(claims))
}

/// One authoritative source-membership access binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipAccessBinding {
    pub membership_id: SourceMembershipId,
    pub access_partition_digest: Blake3Digest32,
    pub scoring_partition_digest: Blake3Digest32,
    pub projection_membership_id: OpaqueId,
    pub active: bool,
}

/// Immutable server registry snapshot used for scope authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritativeAccessSnapshot {
    pub generation: u64,
    pub source_catalog_generation: u64,
    pub membership_generation: u64,
    pub bindings: BTreeMap<SourceMembershipId, MembershipAccessBinding>,
    pub snapshot_digest: Blake3Digest32,
}

/// Requested membership scope after client syntax normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestedMembershipScope {
    pub memberships: BTreeSet<SourceMembershipId>,
}

/// Server-authorized non-empty scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedScope {
    pub memberships: BTreeMap<SourceMembershipId, MembershipAccessBinding>,
    pub access_snapshot_generation: u64,
    pub source_catalog_generation: u64,
    pub membership_generation: u64,
    pub snapshot_digest: Blake3Digest32,
}

/// Intersects requested IDs with an immutable authoritative snapshot.
///
/// Unknown, inactive, or foreign IDs are rejected instead of silently widening
/// or substituting an adjacent scope.
pub fn intersect_scope(
    requested: &RequestedMembershipScope,
    grant_scope: &BTreeSet<SourceMembershipId>,
    authoritative: &AuthoritativeAccessSnapshot,
) -> Result<AuthorizedScope, AccessError> {
    if requested.memberships.is_empty() {
        return Err(AccessError::AuthorizedScopeEmpty);
    }
    if !requested.memberships.is_subset(grant_scope) {
        return Err(AccessError::ScopeUnauthorized);
    }
    let mut memberships = BTreeMap::new();
    for membership in &requested.memberships {
        let binding = authoritative
            .bindings
            .get(membership)
            .ok_or(AccessError::ScopeUnknown)?;
        if !binding.active {
            return Err(AccessError::ScopeUnauthorized);
        }
        memberships.insert(*membership, binding.clone());
    }
    if memberships.is_empty() {
        return Err(AccessError::AuthorizedScopeEmpty);
    }
    Ok(AuthorizedScope {
        memberships,
        access_snapshot_generation: authoritative.generation,
        source_catalog_generation: authoritative.source_catalog_generation,
        membership_generation: authoritative.membership_generation,
        snapshot_digest: authoritative.snapshot_digest,
    })
}

/// Exact indexed route and visible epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedRouteFence {
    pub collection_generation_id: CollectionGenerationId,
    pub visible_epoch: Epoch,
    pub route_generation: u64,
    pub owner_epoch: OwnerEpoch,
}

/// Closed vendor-neutral base eligibility predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseEligibilityPlan {
    pub membership_id: SourceMembershipId,
    pub projection_membership_id: OpaqueId,
    pub access_partition_digest: Blake3Digest32,
    pub scoring_partition_digest: Blake3Digest32,
    pub collection_generation_id: CollectionGenerationId,
    pub visible_epoch: Epoch,
    pub live_security_generation: u64,
    pub shadow_generation: u64,
    pub purge_generation: u64,
    pub predicate_digest: EligibilityPlanDigest,
}

/// Frozen package-local deterministic predicate digest.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EligibilityPlanDigest(pub [u8; 32]);

/// Compiles one coherent membership predicate shared by retrieval and IDF.
pub fn compile_base_eligibility(
    binding: &MembershipAccessBinding,
    route: IndexedRouteFence,
    live_security_generation: u64,
    shadow_generation: u64,
    purge_generation: u64,
) -> Result<BaseEligibilityPlan, AccessError> {
    if !binding.active {
        return Err(AccessError::ScopeUnauthorized);
    }
    let predicate_digest = derive_predicate_digest(
        binding,
        route,
        live_security_generation,
        shadow_generation,
        purge_generation,
    );
    Ok(BaseEligibilityPlan {
        membership_id: binding.membership_id,
        projection_membership_id: binding.projection_membership_id.clone(),
        access_partition_digest: binding.access_partition_digest,
        scoring_partition_digest: binding.scoring_partition_digest,
        collection_generation_id: route.collection_generation_id,
        visible_epoch: route.visible_epoch,
        live_security_generation,
        shadow_generation,
        purge_generation,
        predicate_digest,
    })
}

/// Current proof allowing memberships to share one retrieval/IDF population.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlapFreeRouteProof {
    pub route: IndexedRouteFence,
    pub memberships: BTreeSet<SourceMembershipId>,
    pub access_snapshot_generation: u64,
    pub profile_digest: Blake3Digest32,
    pub proof_digest: Blake3Digest32,
}

/// One safe coherent retrieval leg.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeRetrievalLeg {
    pub leg_id: usize,
    pub memberships: BTreeSet<SourceMembershipId>,
    pub eligibility_plans: Vec<BaseEligibilityPlan>,
    pub route: IndexedRouteFence,
    pub overlap_proof_digest: Option<Blake3Digest32>,
}

/// Compiles finite safe legs. Without a current overlap proof, each membership
/// remains in its own independent scoring population.
pub fn compile_safe_legs(
    scope: &AuthorizedScope,
    route: IndexedRouteFence,
    live_security_generation: u64,
    shadow_generation: u64,
    purge_generation: u64,
    overlap_proof: Option<&OverlapFreeRouteProof>,
    max_legs: usize,
) -> Result<Vec<SafeRetrievalLeg>, AccessError> {
    if max_legs == 0 {
        return Err(AccessError::RetrievalLegBudgetExceeded);
    }
    let membership_ids = scope.memberships.keys().copied().collect::<BTreeSet<_>>();
    let can_group = overlap_proof.is_some_and(|proof| {
        proof.route == route
            && proof.memberships == membership_ids
            && proof.access_snapshot_generation == scope.access_snapshot_generation
    });

    if can_group {
        let proof = overlap_proof.expect("grouping requires proof");
        let plans = scope
            .memberships
            .values()
            .map(|binding| {
                compile_base_eligibility(
                    binding,
                    route,
                    live_security_generation,
                    shadow_generation,
                    purge_generation,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(vec![SafeRetrievalLeg {
            leg_id: 0,
            memberships: membership_ids,
            eligibility_plans: plans,
            route,
            overlap_proof_digest: Some(proof.proof_digest),
        }]);
    }

    if scope.memberships.len() > max_legs {
        return Err(AccessError::RetrievalLegBudgetExceeded);
    }
    scope
        .memberships
        .values()
        .enumerate()
        .map(|(leg_id, binding)| {
            Ok(SafeRetrievalLeg {
                leg_id,
                memberships: BTreeSet::from([binding.membership_id]),
                eligibility_plans: vec![compile_base_eligibility(
                    binding,
                    route,
                    live_security_generation,
                    shadow_generation,
                    purge_generation,
                )?],
                route,
                overlap_proof_digest: None,
            })
        })
        .collect()
}

/// Live security recheck checkpoint.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessCheckpoint {
    RequestAdmission,
    BeforeLegDispatch,
    AfterLegCompletion,
    BeforeSourceReadback,
    BeforeResultEmission,
    HandleExpansion,
    ContinuationExpansion,
}

/// Security fence captured by a request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestSecurityFence {
    pub planned_generation: u64,
    pub memberships: BTreeSet<SourceMembershipId>,
}

/// Current immutable live deny/purge state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveSecurityState {
    pub generation: u64,
    pub denied_memberships: BTreeSet<SourceMembershipId>,
    pub purged_memberships: BTreeSet<SourceMembershipId>,
    pub fail_closed: bool,
    pub snapshot_digest: Blake3Digest32,
}

/// Ephemeral permit valid only for one explicit checkpoint and live generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessPermit {
    pub checkpoint: AccessCheckpoint,
    pub live_generation: u64,
    pub live_snapshot_digest: Blake3Digest32,
}

/// Rechecks live restrictive state. New denial or purge overrides the planned
/// snapshot immediately; no cached permit is reusable at another checkpoint.
pub fn recheck_live_access(
    request: &RequestSecurityFence,
    live: &LiveSecurityState,
    checkpoint: AccessCheckpoint,
) -> Result<AccessPermit, AccessError> {
    if live.fail_closed {
        return Err(AccessError::SecurityFailClosed);
    }
    if !request.memberships.is_disjoint(&live.purged_memberships) {
        return Err(AccessError::LivePurge);
    }
    if !request.memberships.is_disjoint(&live.denied_memberships) {
        return Err(AccessError::LiveRevocation);
    }
    if live.generation < request.planned_generation {
        return Err(AccessError::SecurityFenceStale);
    }
    Ok(AccessPermit {
        checkpoint,
        live_generation: live.generation,
        live_snapshot_digest: live.snapshot_digest,
    })
}

/// Executed leg security population evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegSecurityPopulation {
    pub leg_id: usize,
    pub memberships: BTreeSet<SourceMembershipId>,
    pub security_generation: u64,
    pub idf_population_digest: Option<Blake3Digest32>,
}

/// Whole-leg contamination decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContaminationDecision {
    Clean,
    DiscardLegs(BTreeSet<usize>),
}

/// Discards a whole leg when its candidates, IDF, counts, diversity, or trace
/// may have been influenced by newly denied or purged material.
#[must_use]
pub fn classify_contaminated_legs(
    execution: &[LegSecurityPopulation],
    previous: &LiveSecurityState,
    current: &LiveSecurityState,
) -> ContaminationDecision {
    if current.fail_closed {
        return ContaminationDecision::DiscardLegs(
            execution.iter().map(|leg| leg.leg_id).collect(),
        );
    }
    let newly_restricted = current
        .denied_memberships
        .difference(&previous.denied_memberships)
        .chain(current.purged_memberships.difference(&previous.purged_memberships))
        .copied()
        .collect::<BTreeSet<_>>();
    let contaminated = execution
        .iter()
        .filter(|leg| {
            leg.security_generation < current.generation
                && !leg.memberships.is_disjoint(&newly_restricted)
        })
        .map(|leg| leg.leg_id)
        .collect::<BTreeSet<_>>();
    if contaminated.is_empty() {
        ContaminationDecision::Clean
    } else {
        ContaminationDecision::DiscardLegs(contaminated)
    }
}

fn derive_predicate_digest(
    binding: &MembershipAccessBinding,
    route: IndexedRouteFence,
    live_security_generation: u64,
    shadow_generation: u64,
    purge_generation: u64,
) -> EligibilityPlanDigest {
    let mut lanes = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
    ];
    mix(&mut lanes, binding.membership_id.as_bytes());
    mix(
        &mut lanes,
        binding.projection_membership_id.as_str().as_bytes(),
    );
    mix(&mut lanes, binding.access_partition_digest.as_bytes());
    mix(&mut lanes, binding.scoring_partition_digest.as_bytes());
    mix(&mut lanes, route.collection_generation_id.as_bytes());
    mix(&mut lanes, &route.visible_epoch.get().to_be_bytes());
    mix(&mut lanes, &route.route_generation.to_be_bytes());
    mix(&mut lanes, &route.owner_epoch.get().to_be_bytes());
    mix(&mut lanes, &live_security_generation.to_be_bytes());
    mix(&mut lanes, &shadow_generation.to_be_bytes());
    mix(&mut lanes, &purge_generation.to_be_bytes());
    let mut digest = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        digest[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    EligibilityPlanDigest(digest)
}

fn mix(lanes: &mut [u64; 4], bytes: &[u8]) {
    for (index, byte) in bytes.iter().copied().enumerate() {
        let lane = index % lanes.len();
        lanes[lane] ^= u64::from(byte);
        lanes[lane] = lanes[lane]
            .wrapping_mul(0x0000_0100_0000_01b3)
            .rotate_left(u32::try_from(13 + lane * 5).unwrap_or(13));
    }
}
