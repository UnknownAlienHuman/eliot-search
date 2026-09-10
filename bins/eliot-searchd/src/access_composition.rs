//! Mandatory pre-retrieval access gate and live-barrier path (T20).
//!
//! Daemon composition over the accepted `search-access` kernel. Query and
//! expand paths must call [`admit_pre_retrieval`] before any provider
//! dispatch, [`recheck_before_emission`] before any result emission and
//! [`recheck_before_expansion`] before every handle/continuation expansion.
//! Deny barriers therefore precede scoring; a revocation invalidates every
//! rank leg it influenced via [`discard_contaminated_legs`], not merely the
//! displayed forbidden hits.
//!
//! What this module never does:
//!
//! - A local token, root registration or handle possession is never treated
//!   as authority (see [`deny_local_token_as_authority`]).
//! - No Qdrant filter, collection name or point ID crosses here. Live Qdrant
//!   scoring/IDF/count parity is an explicit T24 obligation, not a T20 claim
//!   (see [`QDRANT_LIVE_PARITY_DEFERRED_TO`]).
//! - Policy state is persisted as data by `search-control-redb`
//!   (`policy_codec`); this gate only decides admission against live state.
//!
//! Wiring (integration owner): add to `entry.rs`
//!
//! ```text
//! #[cfg(feature = "wave4-query")]
//! mod access_composition;
//! ```
//!
//! and call the gate from `provider_composition` query/expand paths. This
//! file assumes `wave4-query`; without that feature the module is absent and
//! recipe queries stay explicitly unavailable through the existing
//! `PROVIDER_*_UNAVAILABLE` capability gating.

use std::collections::BTreeSet;

use search_access::{
    AccessCheckpoint, AccessError, LiveSecurityState, PreRetrievalPlan, PreRetrievalRequest,
    RequestSecurityFence, classify_contaminated_legs, compile_pre_retrieval, recheck_live_access,
};

/// Live Qdrant scoring/IDF/count parity is deferred to T24, not claimed here.
pub const QDRANT_LIVE_PARITY_DEFERRED_TO: &str = "T24";

/// Maximum legs carried in one admission summary.
pub const MAX_ADMISSION_LEGS: usize = 64;

/// Pre-retrieval admission refused without a provider dispatch.
pub const ACCESS_PRE_RETRIEVAL_DENIED: &str = "ACCESS_PRE_RETRIEVAL_DENIED";
/// Live barrier fired at an emission/expansion checkpoint.
pub const ACCESS_LIVE_BARRIER_DENIED: &str = "ACCESS_LIVE_BARRIER_DENIED";
/// A local token or process identity was offered as authority.
pub const ACCESS_LOCAL_TOKEN_NOT_AUTHORITY: &str = "ACCESS_LOCAL_TOKEN_NOT_AUTHORITY";

/// Typed gate denial: stable daemon reason plus the kernel access code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessGateDenial {
    /// Daemon `ACCESS_*` reason code.
    pub reason: &'static str,
    /// Underlying `search-access` reason code.
    pub access_code: &'static str,
}

/// Content-free admission summary for one gated request.
///
/// Carries counts, generations and predicate digests only: no source bytes,
/// no grant material and no reusable authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreRetrievalAdmission {
    /// Number of admitted safe legs.
    pub leg_count: usize,
    /// Live security generation bound into the admission permit.
    pub live_generation: u64,
    /// Live snapshot digest bound into the admission permit.
    pub snapshot_digest: [u8; 32],
    /// Unified retrieval/IDF predicate digests, one per eligibility plan.
    pub predicate_digests: Vec<[u8; 32]>,
}

/// Admits one query/expand request through the mandatory pre-retrieval gate.
///
/// The kernel compiles grant, policy fence, scope, safe legs and the live
/// barrier in one fixed order; any failure denies before any provider call.
/// Fails closed with [`ACCESS_PRE_RETRIEVAL_DENIED`] on any kernel denial.
pub fn admit_pre_retrieval(
    request: PreRetrievalRequest<'_>,
) -> Result<PreRetrievalAdmission, AccessGateDenial> {
    let plan = compile_pre_retrieval(request).map_err(|error| AccessGateDenial {
        reason: ACCESS_PRE_RETRIEVAL_DENIED,
        access_code: error.code(),
    })?;
    admission_summary(&plan).ok_or(AccessGateDenial {
        reason: ACCESS_PRE_RETRIEVAL_DENIED,
        access_code: AccessError::RetrievalLegBudgetExceeded.code(),
    })
}

/// Rechecks the live barrier immediately before result emission.
///
/// Must be called with the admitted memberships and the current live state;
/// any new denial or purge fails with [`ACCESS_LIVE_BARRIER_DENIED`].
pub fn recheck_before_emission(
    memberships: &BTreeSet<search_contracts::SourceMembershipId>,
    planned_generation: u64,
    live: &LiveSecurityState,
) -> Result<(), AccessGateDenial> {
    recheck_barrier(
        memberships,
        planned_generation,
        live,
        AccessCheckpoint::BeforeResultEmission,
    )
}

/// Rechecks the live barrier before every handle/continuation expansion.
///
/// Expansion reauthorizes live state; handle possession alone admits nothing.
pub fn recheck_before_expansion(
    memberships: &BTreeSet<search_contracts::SourceMembershipId>,
    planned_generation: u64,
    live: &LiveSecurityState,
    continuation: bool,
) -> Result<(), AccessGateDenial> {
    let checkpoint = if continuation {
        AccessCheckpoint::ContinuationExpansion
    } else {
        AccessCheckpoint::HandleExpansion
    };
    recheck_barrier(memberships, planned_generation, live, checkpoint)
}

/// Discards every rank leg influenced by a live fence move.
///
/// Returns the exact contaminated leg IDs for whole-leg discard and replan.
/// An empty set means the in-flight work is unaffected.
#[must_use]
pub fn discard_contaminated_legs(
    execution: &[search_access::LegSecurityPopulation],
    previous: &LiveSecurityState,
    current: &LiveSecurityState,
) -> BTreeSet<usize> {
    match classify_contaminated_legs(execution, previous, current) {
        search_access::ContaminationDecision::Clean => BTreeSet::new(),
        search_access::ContaminationDecision::DiscardLegs(legs) => legs,
    }
}

/// Rejects a local token or process identity offered as authority.
///
/// Authenticating a local token establishes no per-request allowed scope.
/// Always denies; callers must present a server-validated grant instead.
#[must_use]
pub const fn deny_local_token_as_authority() -> AccessGateDenial {
    AccessGateDenial {
        reason: ACCESS_LOCAL_TOKEN_NOT_AUTHORITY,
        access_code: AccessError::LocalIdentityNotAuthority.code(),
    }
}

fn recheck_barrier(
    memberships: &BTreeSet<search_contracts::SourceMembershipId>,
    planned_generation: u64,
    live: &LiveSecurityState,
    checkpoint: AccessCheckpoint,
) -> Result<(), AccessGateDenial> {
    let fence = RequestSecurityFence {
        planned_generation,
        memberships: memberships.clone(),
    };
    recheck_live_access(&fence, live, checkpoint)
        .map(|_| ())
        .map_err(|error| AccessGateDenial {
            reason: ACCESS_LIVE_BARRIER_DENIED,
            access_code: error.code(),
        })
}

fn admission_summary(plan: &PreRetrievalPlan) -> Option<PreRetrievalAdmission> {
    if plan.legs.len() > MAX_ADMISSION_LEGS {
        return None;
    }
    Some(PreRetrievalAdmission {
        leg_count: plan.legs.len(),
        live_generation: plan.permit.live_generation,
        snapshot_digest: *plan.permit.live_snapshot_digest.as_bytes(),
        predicate_digests: plan
            .predicates
            .iter()
            .map(|predicates| predicates.retrieval_digest.0)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_access::{
        AccessModality, AuthoritativeAccessSnapshot, AuthoritativePolicyState, GrantClaims,
        GrantValidationContext, IndexedRouteFence, MembershipAccessBinding, NamespacePolicyFence,
        RequestedMembershipScope,
    };
    use search_contracts::{
        AccessPolicyRevision, BindingId, Blake3Digest32, CollectionGenerationId, Epoch, GrantId,
        InstallationId, InstallationIncarnationId, OpaqueId, OwnerEpoch, PurgeFenceRevision,
        RecipeIdV1, ShadowFenceRevision, SourceMembershipId, SourceNamespaceId,
        SourceOwnerGeneration,
    };
    use std::collections::BTreeMap;

    struct Bundle {
        claims: GrantClaims,
        context: GrantValidationContext,
        requested: RequestedMembershipScope,
        grant_scope: BTreeSet<SourceMembershipId>,
        authoritative: AuthoritativeAccessSnapshot,
        policy_fence: NamespacePolicyFence,
        authoritative_policy: AuthoritativePolicyState,
        route: IndexedRouteFence,
        live: LiveSecurityState,
    }

    fn oid(value: &str) -> OpaqueId {
        OpaqueId::new(value).expect("fixture id")
    }

    fn member(n: u128) -> SourceMembershipId {
        SourceMembershipId::from_bytes(n.to_be_bytes())
    }

    fn bundle() -> Bundle {
        let installation_id = InstallationId::from_bytes([1; 16]);
        let incarnation = InstallationIncarnationId::from_bytes([2; 16]);
        let binding_id = BindingId::from_bytes([3; 16]);
        let boot = oid("boot-gate-0001");
        let members = [member(1), member(2)];
        let bindings = members
            .into_iter()
            .map(|m| {
                (
                    m,
                    MembershipAccessBinding {
                        membership_id: m,
                        access_partition_digest: Blake3Digest32::from_bytes([0xA1; 32]),
                        scoring_partition_digest: Blake3Digest32::from_bytes([0xB2; 32]),
                        projection_membership_id: oid("projection-gate"),
                        active: true,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let namespace_id = SourceNamespaceId::from_bytes([0x20; 16]);
        let owner_generation = SourceOwnerGeneration::from_bytes([0x21; 32]);
        Bundle {
            claims: GrantClaims {
                grant_id: GrantId::from_bytes([9; 16]),
                binding_id,
                installation_id,
                installation_incarnation_id: incarnation,
                issued_boot_id: boot.clone(),
                issued_at_ms: 1_000,
                expires_at_ms: 2_000,
                nonce: oid("nonce-gate-0001"),
                revocation_generation: 4,
                allowed_recipes: BTreeSet::from([RecipeIdV1::FindText]),
                allowed_modalities: BTreeSet::from([AccessModality::Direct]),
                allowed_budget_classes: BTreeSet::from([oid("budget-gate")]),
                max_source_read_bytes: 1_024,
                max_result_bytes: 1_024,
            },
            context: GrantValidationContext {
                binding_id,
                installation_id,
                installation_incarnation_id: incarnation,
                boot_id: boot,
                now_ms: 1_500,
                current_revocation_generation: 4,
                signature_verified: true,
                pairing_verified: true,
                nonce_accepted: true,
            },
            requested: RequestedMembershipScope {
                memberships: BTreeSet::from(members),
            },
            grant_scope: BTreeSet::from(members),
            authoritative: AuthoritativeAccessSnapshot {
                generation: 7,
                source_catalog_generation: 3,
                membership_generation: 5,
                bindings,
                snapshot_digest: Blake3Digest32::from_bytes([0xCC; 32]),
            },
            policy_fence: NamespacePolicyFence {
                namespace_id,
                owner_generation,
                policy_revision: AccessPolicyRevision::new(11),
            },
            authoritative_policy: AuthoritativePolicyState {
                namespace_id,
                owner_generation,
                policy_revision: AccessPolicyRevision::new(11),
                shadow_revision: ShadowFenceRevision::new(6),
                purge_revision: PurgeFenceRevision::new(6),
            },
            route: IndexedRouteFence {
                collection_generation_id: CollectionGenerationId::from_bytes([0x10; 16]),
                visible_epoch: Epoch::new(12).expect("epoch"),
                route_generation: 2,
                owner_epoch: OwnerEpoch::new(1).expect("owner"),
            },
            live: LiveSecurityState {
                generation: 7,
                denied_memberships: BTreeSet::new(),
                purged_memberships: BTreeSet::new(),
                fail_closed: false,
                snapshot_digest: Blake3Digest32::from_bytes([0xDD; 32]),
            },
        }
    }

    fn admit(bundle: &Bundle) -> Result<PreRetrievalAdmission, AccessGateDenial> {
        admit_pre_retrieval(PreRetrievalRequest {
            claims: bundle.claims.clone(),
            validation_context: &bundle.context,
            recipe: RecipeIdV1::FindText,
            modality: AccessModality::Direct,
            requested_scope: &bundle.requested,
            grant_scope: &bundle.grant_scope,
            authoritative: &bundle.authoritative,
            policy_fence: &bundle.policy_fence,
            authoritative_policy: &bundle.authoritative_policy,
            route: bundle.route,
            live: &bundle.live,
            overlap_proof: None,
            max_legs: 8,
        })
    }

    #[test]
    fn clean_request_admits_with_predicate_digests() {
        let bundle = bundle();
        let admission = admit(&bundle).expect("clean request admits");
        assert_eq!(admission.leg_count, 2);
        assert_eq!(admission.live_generation, 7);
        assert_eq!(admission.predicate_digests.len(), 2);
    }

    #[test]
    fn revoked_grant_denies_before_provider_dispatch() {
        let mut bundle = bundle();
        bundle.context.current_revocation_generation = 99;
        let denial = admit(&bundle).expect_err("revoked grant denies");
        assert_eq!(denial.reason, ACCESS_PRE_RETRIEVAL_DENIED);
        assert_eq!(denial.access_code, AccessError::GrantRevoked.code());
    }

    #[test]
    fn live_purge_denies_admission() {
        let mut bundle = bundle();
        bundle.live.purged_memberships.insert(member(1));
        let denial = admit(&bundle).expect_err("purged scope denies");
        assert_eq!(denial.access_code, AccessError::LivePurge.code());
    }

    #[test]
    fn emission_recheck_fires_on_new_revocation() {
        let live = bundle().live;
        let memberships = BTreeSet::from([member(1), member(2)]);
        recheck_before_emission(&memberships, 7, &live).expect("clean live passes");
        let mut revoked = live;
        revoked.generation = 8;
        revoked.denied_memberships.insert(member(2));
        let denial = recheck_before_emission(&memberships, 7, &revoked)
            .expect_err("new revocation fires at emission");
        assert_eq!(denial.reason, ACCESS_LIVE_BARRIER_DENIED);
        assert_eq!(denial.access_code, AccessError::LiveRevocation.code());
    }

    #[test]
    fn expansion_reauthorizes_live_state() {
        let live = bundle().live;
        let memberships = BTreeSet::from([member(1)]);
        recheck_before_expansion(&memberships, 7, &live, false).expect("clean handle pass");
        recheck_before_expansion(&memberships, 7, &live, true).expect("clean continuation pass");
        let mut purged = live;
        purged.generation = 8;
        purged.purged_memberships.insert(member(1));
        let denial = recheck_before_expansion(&memberships, 7, &purged, false)
            .expect_err("purge fires at expansion");
        assert_eq!(denial.access_code, AccessError::LivePurge.code());
    }

    #[test]
    fn contaminated_legs_discarded_whole() {
        let bundle = bundle();
        let admission = admit(&bundle).expect("baseline admits");
        assert_eq!(admission.leg_count, 2);
        let plan = compile_pre_retrieval(PreRetrievalRequest {
            claims: bundle.claims.clone(),
            validation_context: &bundle.context,
            recipe: RecipeIdV1::FindText,
            modality: AccessModality::Direct,
            requested_scope: &bundle.requested,
            grant_scope: &bundle.grant_scope,
            authoritative: &bundle.authoritative,
            policy_fence: &bundle.policy_fence,
            authoritative_policy: &bundle.authoritative_policy,
            route: bundle.route,
            live: &bundle.live,
            overlap_proof: None,
            max_legs: 8,
        })
        .expect("baseline plan compiles");
        let execution = plan
            .legs
            .iter()
            .map(|leg| search_access::LegSecurityPopulation {
                leg_id: leg.leg_id,
                memberships: leg.memberships.clone(),
                security_generation: bundle.live.generation,
                idf_population_digest: None,
            })
            .collect::<Vec<_>>();
        let mut moved = bundle.live.clone();
        moved.generation = 8;
        moved.denied_memberships.insert(member(2));
        // Whole influenced leg is discarded, never sanitized in place.
        assert_eq!(
            discard_contaminated_legs(&execution, &bundle.live, &moved),
            BTreeSet::from([1])
        );
    }

    #[test]
    fn local_token_never_admits() {
        let denial = deny_local_token_as_authority();
        assert_eq!(denial.reason, ACCESS_LOCAL_TOKEN_NOT_AUTHORITY);
        assert_eq!(
            denial.access_code,
            AccessError::LocalIdentityNotAuthority.code()
        );
    }

    #[test]
    fn qdrant_parity_deferred() {
        assert_eq!(QDRANT_LIVE_PARITY_DEFERRED_TO, "T24");
        assert_eq!(
            QDRANT_LIVE_PARITY_DEFERRED_TO,
            search_access::QDRANT_LIVE_PARITY_DEFERRED_TO
        );
    }
}
