//! Typed canonical indexed retrieval over one owner-compiled S10.3 population.
//!
//! The daemon supplies the real bridge port and a bounded point-identity
//! resolver backed by the current projection manifest. This kernel owns no
//! Qdrant types or client, creates no graph or query plan, and keeps point
//! addresses, projection memberships, and source memberships as distinct
//! typed values. A nomination remains non-evidence until the existing exact
//! source validator completes its own readback.

use std::collections::{BTreeMap, BTreeSet};

use search_access::{
    AccessCheckpoint, AccessError, CanonicalProjectionMembership, CompiledEligibilityPopulation,
    LiveSecurityState, RequestSecurityFence, recheck_live_access,
};
use search_contracts::indexed::{EligibilityPopulation, VectorName, VectorValue};
use search_contracts::{
    CollectionRouteRevision, LegKind, ProjectionMembershipId, SourceMembershipId,
};
use search_epoch_pins::RouteIdentity;

use crate::{ExecuteError, LegCompletion, LegPinSet, LegTicket};

#[cfg(test)]
#[path = "../../search-query-planner/tests/support/canonical_plan.rs"]
mod canonical_plan_fixture;

/// Byte-exact Qdrant point address. It is a locator, never an identity digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalPointAddress([u8; 16]);

impl CanonicalPointAddress {
    /// Preserve an address's exact stored bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Exact address bytes, without interpreting them as an identity digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// One nomination whose canonical point payload was read back exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalIndexedNomination<Candidate> {
    /// Exact Qdrant address returned by the canonical query.
    pub point_address: CanonicalPointAddress,
    /// Source membership resolved through the typed owner binding.
    pub source_membership_id: SourceMembershipId,
    /// Projection membership read from and validated against canonical payload.
    pub projection_membership_id: ProjectionMembershipId,
    /// Ranking score, comparable only within this one execution population.
    pub raw_score: f32,
    /// Bridge-owned exact point identity and payload pair, with no vendor data.
    pub candidate: Candidate,
}

/// Bridge readback row before the executor resolves its source membership.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalIndexedReadback<Candidate> {
    /// Exact Qdrant address returned by the canonical query.
    pub point_address: CanonicalPointAddress,
    /// Projection membership from the validated canonical payload.
    pub projection_membership_id: ProjectionMembershipId,
    /// Ranking score, comparable only within this one execution population.
    pub raw_score: f32,
    /// Bridge-owned exact point identity and payload pair, with no vendor data.
    pub candidate: Candidate,
}

/// Query addresses and exact-readback results for one population.
///
/// `nominated_addresses` accounts for every query result. Missing addresses
/// stay visible as a coverage gap instead of being silently discarded.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalIndexedReadbackBatch<Candidate> {
    /// Every address nominated by the bounded query, in query order.
    pub nominated_addresses: Vec<CanonicalPointAddress>,
    /// Addresses absent during exact canonical readback.
    pub readback_missing_addresses: Vec<CanonicalPointAddress>,
    /// Nominations with exact identity and payload readback.
    pub nominations: Vec<CanonicalIndexedReadback<Candidate>>,
}

/// Closed failure from the real canonical indexed adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalIndexedPortError {
    /// The pinned canonical route or admitted vector is unavailable.
    Unavailable,
    /// The original request cancellation probe fired.
    Cancelled,
    /// Transport, identity lookup, or canonical readback failed.
    Failure,
    /// The original absolute request deadline elapsed.
    DeadlineExceeded,
}

/// Vendor-neutral port bound to one exact population owner and request.
///
/// Implementations bind one opaque owner compilation and one shared request
/// context at construction. Query (including filtered IDF), point readback,
/// and exact count therefore cannot receive diverging filters or independently
/// refreshed deadlines.
pub trait CanonicalIndexedRetrievalPort {
    /// Typed exact-readback product selected by the daemon adapter.
    type Candidate;

    /// The opaque owner compilation retained for the complete request leg.
    fn compiled_population(&self) -> &CompiledEligibilityPopulation;

    /// Bounded query followed by exact S11 identity and payload readback.
    fn query_and_readback(
        &mut self,
        vector_name: &VectorName,
        query: &VectorValue,
        limit: usize,
    ) -> Result<CanonicalIndexedReadbackBatch<Self::Candidate>, CanonicalIndexedPortError>;

    /// Exact count from the same retained population.
    fn count_exact(&mut self) -> Result<u64, CanonicalIndexedPortError>;

    /// Recheck the same absolute deadline and cancellation observer.
    fn check_request(&self) -> Result<(), CanonicalIndexedPortError>;
}

/// Canonical indexed result with explicit point-readback gaps.
pub struct CanonicalIndexedLegOutput<'population, Candidate> {
    /// The same opaque owner compilation used for each query, readback and
    /// count call. It binds scores but is not a reusable permission.
    pub compiled_population: &'population CompiledEligibilityPopulation,
    /// Existing immutable leg identity.
    pub leg_id: usize,
    /// Existing immutable leg kind.
    pub leg_kind: LegKind,
    /// Source memberships from the exact ticket.
    pub memberships: BTreeSet<SourceMembershipId>,
    /// Live security generation observed after execution.
    pub security_generation: u64,
    /// Typed, exactly read-back nominations.
    pub nominations: Vec<CanonicalIndexedNomination<Candidate>>,
    /// Exact denominator counted from the same C00 population.
    pub exact_denominator: u64,
    /// Query results that disappeared before exact point readback.
    pub readback_missing_addresses: Vec<CanonicalPointAddress>,
    /// Candidate-scope completion only; never a complete-scope proof.
    pub completion: LegCompletion,
}

/// Execute one immutable ticket through its opaque C18 compilation.
///
/// The port must retain the exact compilation reference; the pointer check
/// prevents a second equal-looking owner result or caller mapping from taking
/// its place.
#[allow(clippy::too_many_arguments)]
pub fn execute_canonical_indexed_leg<'population, P: CanonicalIndexedRetrievalPort>(
    ticket: &LegTicket,
    pins: &LegPinSet,
    compiled_population: &'population CompiledEligibilityPopulation,
    vector_name: &VectorName,
    query: &VectorValue,
    limit: usize,
    request_security: &RequestSecurityFence,
    live_before: &LiveSecurityState,
    live_after: &LiveSecurityState,
    port: &mut P,
) -> Result<CanonicalIndexedLegOutput<'population, P::Candidate>, ExecuteError> {
    let safe_leg = ticket
        .leg
        .safe_index_leg()
        .ok_or(ExecuteError::InvalidExecutionState)?;
    if ticket.leg.leg_kind() != LegKind::Lexical
        || ticket.leg.memberships() != &safe_leg.memberships
        || safe_leg.memberships.is_empty()
    {
        return Err(ExecuteError::InvalidExecutionState);
    }
    if !same_owner(compiled_population, port.compiled_population()) {
        return Err(ExecuteError::PopulationMismatch);
    }
    let population = compiled_population.population();
    if safe_leg.route.collection_generation_id != population.collection_generation_id()
        || safe_leg.route.visible_epoch != population.visible_epoch()
        || safe_leg.eligibility_plans.is_empty()
        || safe_leg.eligibility_plans.iter().any(|plan| {
            plan.collection_generation_id != population.collection_generation_id()
                || plan.visible_epoch != population.visible_epoch()
        })
    {
        return Err(ExecuteError::PopulationMismatch);
    }
    let plan_memberships = safe_leg
        .eligibility_plans
        .iter()
        .map(|plan| plan.membership_id)
        .collect::<BTreeSet<_>>();
    if plan_memberships.len() != safe_leg.eligibility_plans.len()
        || plan_memberships != safe_leg.memberships
        || (safe_leg.memberships.len() > 1 && safe_leg.overlap_proof_digest.is_none())
    {
        return Err(ExecuteError::PopulationMismatch);
    }
    let source_by_projection = validate_owner_memberships(
        &safe_leg.memberships,
        population,
        compiled_population.projection_memberships(),
    )?;

    let expected_route = RouteIdentity {
        collection_generation_id: safe_leg.route.collection_generation_id,
        route_revision: CollectionRouteRevision::new(safe_leg.route.route_generation),
    };
    if pins.len() != 1 {
        return Err(ExecuteError::PinAcquisitionFailed);
    }
    let Some(pin) = pins.epoch_pins().first() else {
        return Err(ExecuteError::PinAcquisitionFailed);
    };
    if pin.route() != expected_route || pin.epoch() != safe_leg.route.visible_epoch {
        return Err(ExecuteError::PinAcquisitionFailed);
    }

    let budget_limit = usize::try_from(ticket.leg.budget().max_candidates())
        .map_err(|_| ExecuteError::BudgetExceeded)?;
    if limit == 0 || limit > crate::indexed::MAX_INDEXED_LIMIT || limit > budget_limit {
        return Err(ExecuteError::BudgetExceeded);
    }
    recheck_live_access(
        request_security,
        live_before,
        AccessCheckpoint::BeforeLegDispatch,
    )?;
    port.check_request().map_err(map_port_error)?;

    let batch = port
        .query_and_readback(vector_name, query, limit)
        .map_err(map_port_error)?;
    let CanonicalIndexedReadbackBatch {
        nominated_addresses,
        readback_missing_addresses,
        nominations: readbacks,
    } = batch;
    let nominations = validate_nominations(
        &nominated_addresses,
        &readback_missing_addresses,
        readbacks,
        &source_by_projection,
        &safe_leg.memberships,
        limit,
    )?;
    let exact_denominator = port.count_exact().map_err(map_port_error)?;
    port.check_request().map_err(map_port_error)?;
    recheck_live_access(
        request_security,
        live_after,
        AccessCheckpoint::AfterLegCompletion,
    )
    .map_err(map_post_leg_error)?;
    if u64::try_from(nominations.len()).map_err(|_| ExecuteError::PopulationMismatch)?
        > exact_denominator
    {
        return Err(ExecuteError::PopulationMismatch);
    }

    let completion = candidate_scope_completion(
        nominations.len(),
        exact_denominator,
        readback_missing_addresses.len(),
    );

    Ok(CanonicalIndexedLegOutput {
        compiled_population,
        leg_id: ticket.leg.leg_id(),
        leg_kind: ticket.leg.leg_kind(),
        memberships: safe_leg.memberships.clone(),
        security_generation: live_after.generation,
        nominations,
        exact_denominator,
        readback_missing_addresses,
        completion,
    })
}

fn same_owner<T>(expected: &T, actual: &T) -> bool {
    std::ptr::eq(expected, actual)
}

fn validate_owner_memberships(
    ticket_memberships: &BTreeSet<SourceMembershipId>,
    population: &EligibilityPopulation,
    owner_memberships: &BTreeMap<SourceMembershipId, CanonicalProjectionMembership>,
) -> Result<BTreeMap<ProjectionMembershipId, SourceMembershipId>, ExecuteError> {
    let population_memberships = population
        .allowed_projection_memberships()
        .into_iter()
        .collect::<BTreeSet<_>>();
    if population_memberships.is_empty()
        || owner_memberships.len() != population_memberships.len()
        || owner_memberships.len() != ticket_memberships.len()
    {
        return Err(ExecuteError::PopulationMismatch);
    }
    let mut source_by_projection = BTreeMap::new();
    for (source_membership_id, binding) in owner_memberships {
        if !ticket_memberships.contains(&binding.source_membership_id)
            || binding.source_membership_id != *source_membership_id
            || !population_memberships.contains(&binding.projection_membership_id)
            || binding.installation_incarnation_id != population.installation_incarnation_id()
            || binding.collection_generation_id != population.collection_generation_id()
            || binding.access_partition_id != population.access_partition_id()
            || binding.scoring_partition_id != population.scoring_partition_id()
            || binding.projection_profile_set_id != *population.projection_profile_set_id()
            || source_by_projection
                .insert(binding.projection_membership_id, *source_membership_id)
                .is_some()
        {
            return Err(ExecuteError::PopulationMismatch);
        }
    }
    let source_memberships = owner_memberships.keys().copied().collect::<BTreeSet<_>>();
    if &source_memberships != ticket_memberships
        || source_by_projection
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != population_memberships
    {
        return Err(ExecuteError::PopulationMismatch);
    }
    Ok(source_by_projection)
}

fn validate_nominations<Candidate>(
    nominated_addresses: &[CanonicalPointAddress],
    readback_missing_addresses: &[CanonicalPointAddress],
    readbacks: Vec<CanonicalIndexedReadback<Candidate>>,
    source_by_projection: &BTreeMap<ProjectionMembershipId, SourceMembershipId>,
    ticket_memberships: &BTreeSet<SourceMembershipId>,
    limit: usize,
) -> Result<Vec<CanonicalIndexedNomination<Candidate>>, ExecuteError> {
    if nominated_addresses.len() > limit {
        return Err(ExecuteError::InvalidNomination);
    }
    let nominated = nominated_addresses.iter().copied().collect::<BTreeSet<_>>();
    let missing = readback_missing_addresses
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let returned = readbacks
        .iter()
        .map(|nomination| nomination.point_address)
        .collect::<BTreeSet<_>>();
    if nominated.len() != nominated_addresses.len()
        || missing.len() != readback_missing_addresses.len()
        || returned.len() != readbacks.len()
        || missing.iter().any(|address| returned.contains(address))
        || missing.union(&returned).copied().collect::<BTreeSet<_>>() != nominated
    {
        return Err(ExecuteError::InvalidNomination);
    }

    let mut nominations = Vec::with_capacity(readbacks.len());
    for readback in readbacks {
        if !readback.raw_score.is_finite() {
            return Err(ExecuteError::InvalidNomination);
        }
        let source_membership_id = *source_by_projection
            .get(&readback.projection_membership_id)
            .ok_or(ExecuteError::InvalidNomination)?;
        if !ticket_memberships.contains(&source_membership_id) {
            return Err(ExecuteError::InvalidNomination);
        }
        nominations.push(CanonicalIndexedNomination {
            point_address: readback.point_address,
            source_membership_id,
            projection_membership_id: readback.projection_membership_id,
            raw_score: readback.raw_score,
            candidate: readback.candidate,
        });
    }
    nominations.sort_by(|left, right| {
        right
            .raw_score
            .partial_cmp(&left.raw_score)
            .unwrap_or(core::cmp::Ordering::Equal)
            .then_with(|| left.point_address.cmp(&right.point_address))
    });
    Ok(nominations)
}

fn candidate_scope_completion(
    nominations: usize,
    exact_denominator: u64,
    readback_missing: usize,
) -> LegCompletion {
    if readback_missing == 0 && u64::try_from(nominations).ok() == Some(exact_denominator) {
        LegCompletion::CompleteCandidateScope
    } else {
        LegCompletion::PartialCandidateScope
    }
}

const fn map_port_error(error: CanonicalIndexedPortError) -> ExecuteError {
    match error {
        CanonicalIndexedPortError::Unavailable => ExecuteError::BackendUnavailable,
        CanonicalIndexedPortError::Cancelled => ExecuteError::Cancelled,
        CanonicalIndexedPortError::Failure => ExecuteError::BackendFailure,
        CanonicalIndexedPortError::DeadlineExceeded => ExecuteError::DeadlineExceeded,
    }
}

const fn map_post_leg_error(error: AccessError) -> ExecuteError {
    match error {
        AccessError::LiveRevocation
        | AccessError::LivePurge
        | AccessError::SecurityFailClosed
        | AccessError::ContaminatedExecution => ExecuteError::ContaminatedLeg,
        _ => ExecuteError::AccessDenied,
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_plan_fixture;
    use super::{
        CanonicalIndexedPortError, CanonicalIndexedReadback, CanonicalIndexedReadbackBatch,
        CanonicalIndexedRetrievalPort, CanonicalPointAddress, candidate_scope_completion,
        execute_canonical_indexed_leg, same_owner, validate_owner_memberships,
    };
    use crate::{ExecuteError, LegCompletion, LegPinSet, LegTicket, acquire_leg_pins};
    use search_access::{
        AccessCheckpoint, AccessError, AccessPermit, AuthoritativeAccessSnapshot,
        AuthoritativePolicyState, CanonicalEligibilityPopulationRequest,
        CanonicalEligibilitySecuritySnapshot, CanonicalPopulationVerificationPort,
        CanonicalProjectionMembership, CompiledEligibilityPopulation, IndexedRouteFence,
        LiveSecurityState, MembershipAccessBinding, NamespacePolicyFence, RequestSecurityFence,
        RequestedMembershipScope, compile_base_eligibility,
        compile_canonical_eligibility_population,
    };
    use search_contracts::indexed::{
        CollectionSchema, EligibilityPopulation, NamedVectorRequirement, VectorName,
        VectorRequirement, VectorValue,
    };
    use search_contracts::{
        AccessPartitionId, AccessPolicyRevision, ArtifactDigest, AuthoritativeGrantPolicy,
        Blake3Digest32, Blake3ExactInputLimit, BoundedList, BoundedSet, CatalogRevision,
        CollectionGenerationId, CollectionRoute, CollectionRouteRevision, DisclosureCeiling, Epoch,
        GrantId, InstallationId, InstallationIncarnationId, MembershipRevision, Modality,
        ObservationCursorRevision, ObservationFreshness, ObservationFreshnessState, OpaqueId,
        OpaqueRef, OverlayRevision, OwnerEpoch, ProfileId, ProjectionMembershipId,
        ProjectionProfileSetId, PurgeFenceRevision, QuerySnapshotFence, QuerySnapshotFingerprint,
        ReceiptRef, RecipeIdV1, ScoringPartitionId, SearchReadGrantClaims, SensitivityClass,
        ShadowFenceRevision, SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
        SourceRevisionId, SourceView, UtcTimestamp, blake3_exact_bytes,
    };
    use search_epoch_pins::{PinLimits, PinRegistry, RouteIdentity};
    use std::cell::Cell;
    use std::collections::{BTreeMap, BTreeSet};

    fn population(memberships: &[ProjectionMembershipId]) -> EligibilityPopulation {
        EligibilityPopulation::new(
            InstallationIncarnationId::from_bytes([1; 16]),
            CollectionGenerationId::from_bytes([2; 16]),
            memberships,
            AccessPartitionId::from_bytes([3; 16]),
            ScoringPartitionId::from_bytes([4; 16]),
            ProjectionProfileSetId::new("lexical_baseline_v1").expect("profile"),
            Epoch::new(5).expect("epoch"),
        )
        .expect("population")
    }

    fn owner_membership(
        source_membership_id: SourceMembershipId,
        projection_membership_id: ProjectionMembershipId,
    ) -> CanonicalProjectionMembership {
        CanonicalProjectionMembership {
            source_membership_id,
            installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
            collection_generation_id: CollectionGenerationId::from_bytes([2; 16]),
            projection_membership_id,
            access_partition_id: AccessPartitionId::from_bytes([3; 16]),
            scoring_partition_id: ScoringPartitionId::from_bytes([4; 16]),
            projection_profile_set_id: ProjectionProfileSetId::new("lexical_baseline_v1")
                .expect("profile"),
        }
    }

    #[test]
    fn membership_binding_is_a_typed_bijection_for_the_ticket_and_population() {
        let source = SourceMembershipId::from_bytes([6; 16]);
        let projection = ProjectionMembershipId::from_bytes([7; 16]);
        let members = [source].into_iter().collect();
        let compiled = population(&[projection]);
        let owner_memberships = BTreeMap::from([(source, owner_membership(source, projection))]);

        let map = validate_owner_memberships(&members, &compiled, &owner_memberships)
            .expect("exact owner-verified binding");
        assert_eq!(map.get(&projection), Some(&source));

        let wrong_projection = ProjectionMembershipId::from_bytes([8; 16]);
        let wrong = BTreeMap::from([(source, owner_membership(source, wrong_projection))]);
        assert!(validate_owner_memberships(&members, &compiled, &wrong).is_err());
    }

    #[test]
    fn an_equal_copy_is_not_the_same_population_owner() {
        let projection = ProjectionMembershipId::from_bytes([7; 16]);
        let first = population(&[projection]);
        let second = first.clone();
        assert_eq!(first, second);
        assert!(!same_owner(&first, &second));
    }

    #[test]
    fn missing_exact_point_readback_never_completes_candidate_scope() {
        assert_eq!(
            candidate_scope_completion(4, 4, 0),
            LegCompletion::CompleteCandidateScope
        );
        assert_eq!(
            candidate_scope_completion(4, 4, 1),
            LegCompletion::PartialCandidateScope
        );
        assert_eq!(
            candidate_scope_completion(3, 4, 0),
            LegCompletion::PartialCandidateScope
        );
    }

    #[test]
    fn canonical_point_address_preserves_bytes_without_digest_conversion() {
        let bytes = [0xA5; 16];
        assert_eq!(CanonicalPointAddress::from_bytes(bytes).as_bytes(), &bytes);
    }

    struct IssuedFixture(SearchReadGrantClaims);

    struct OwnerFixture {
        claims: SearchReadGrantClaims,
        bindings: BTreeMap<SourceMembershipId, CanonicalProjectionMembership>,
        fence: QuerySnapshotFence,
    }

    impl CanonicalPopulationVerificationPort for OwnerFixture {
        type IssuedGrant = IssuedFixture;

        fn verified_issued_claims(
            &mut self,
            issued: &Self::IssuedGrant,
        ) -> Result<SearchReadGrantClaims, AccessError> {
            if issued.0 != self.claims {
                return Err(AccessError::GrantBindingMismatch);
            }
            Ok(issued.0.clone())
        }

        fn current_projection_binding(
            &mut self,
            issued: &Self::IssuedGrant,
            membership_id: SourceMembershipId,
            fence: &QuerySnapshotFence,
        ) -> Result<CanonicalProjectionMembership, AccessError> {
            if issued.0 != self.claims || fence != &self.fence {
                return Err(AccessError::SnapshotStale);
            }
            self.bindings
                .get(&membership_id)
                .cloned()
                .ok_or(AccessError::ScopeUnknown)
        }
    }

    struct PopulationFixture {
        grant: SearchReadGrantClaims,
        policy: AuthoritativeGrantPolicy,
        scope: RequestedMembershipScope,
        access: AuthoritativeAccessSnapshot,
        policy_fence: NamespacePolicyFence,
        policy_state: AuthoritativePolicyState,
        fence: QuerySnapshotFence,
        schema: CollectionSchema,
        route: CollectionRoute,
        rows: BTreeMap<ProjectionMembershipId, CanonicalProjectionMembership>,
        manifests: BTreeMap<ProjectionMembershipId, ReceiptRef>,
        live: LiveSecurityState,
    }

    impl PopulationFixture {
        fn new() -> Self {
            let membership = SourceMembershipId::from_bytes([0x11; 16]);
            let incarnation = InstallationIncarnationId::from_bytes([0x12; 16]);
            let generation = CollectionGenerationId::from_bytes([0x13; 16]);
            let projection = ProjectionMembershipId::from_bytes([0x14; 16]);
            let access_partition = AccessPartitionId::from_bytes([0x15; 16]);
            let scoring_partition = ScoringPartitionId::from_bytes([0x16; 16]);
            let profile = ProjectionProfileSetId::new("lexical-v1").expect("profile");
            let schema = CollectionSchema::initial(
                incarnation,
                generation,
                profile.clone(),
                &[NamedVectorRequirement {
                    name: VectorName::new("lex_code_v1").expect("vector name"),
                    requirement: VectorRequirement::Sparse {
                        index_ceiling: 1024,
                        idf_enabled: true,
                    },
                }],
            )
            .expect("schema");
            let row = CanonicalProjectionMembership {
                source_membership_id: membership,
                installation_incarnation_id: incarnation,
                collection_generation_id: generation,
                projection_membership_id: projection,
                access_partition_id: access_partition,
                scoring_partition_id: scoring_partition,
                projection_profile_set_id: profile,
            };
            let grant = SearchReadGrantClaims {
                grant_id: GrantId::from_bytes([0x17; 16]),
                installation_id: InstallationId::from_bytes([0x18; 16]),
                installation_incarnation_id: incarnation,
                binding_id: search_contracts::BindingId::from_bytes([0x19; 16]),
                principal_opaque_id: OpaqueId::new("principal").expect("principal"),
                client_scope_ref: OpaqueRef::new("scope").expect("scope"),
                scope_domain_id: search_contracts::ScopeDomainId::from_bytes([0x1A; 16]),
                allowed_membership_ids: BoundedSet::from_items([membership]).expect("members"),
                allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
                reference_portfolio_revision: None,
                allowed_access_partitions: BoundedSet::from_items([access_partition])
                    .expect("partitions"),
                allowed_modalities: BoundedSet::from_items([Modality::Code]).expect("modalities"),
                permitted_recipe_families: BoundedSet::from_items([RecipeIdV1::Locate])
                    .expect("recipes"),
                maximum_budget_class: ProfileId::new("interactive").expect("budget class"),
                sensitivity_ceiling: SensitivityClass::Confidential,
                disclosure_ceiling: DisclosureCeiling::LocalOnly,
                source_read_permission: true,
                exact_scan_permission: false,
                issued_boot_id: OpaqueId::new("boot").expect("boot"),
                issued_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").expect("issued at"),
                expires_at: UtcTimestamp::parse("2026-10-16T00:01:00.000000Z").expect("expires at"),
                nonce: OpaqueId::new("retained-nonce").expect("nonce"),
                revocation_generation: 1,
            };
            let policy = AuthoritativeGrantPolicy {
                binding_id: grant.binding_id,
                binding_generation: 1,
                policy_generation: 1,
                installation_id: grant.installation_id,
                installation_incarnation_id: incarnation,
                principal_opaque_id: grant.principal_opaque_id.clone(),
                client_scope_ref: grant.client_scope_ref.clone(),
                scope_domain_id: grant.scope_domain_id,
                allowed_membership_ids: grant.allowed_membership_ids.clone(),
                allowed_corpus_or_portfolio_ids: grant.allowed_corpus_or_portfolio_ids.clone(),
                reference_portfolio_revision: None,
                allowed_access_partitions: grant.allowed_access_partitions.clone(),
                allowed_modalities: grant.allowed_modalities.clone(),
                permitted_recipe_families: grant.permitted_recipe_families.clone(),
                allowed_budget_classes: BoundedSet::from_items([grant
                    .maximum_budget_class
                    .clone()])
                .expect("budget classes"),
                sensitivity_ceiling: grant.sensitivity_ceiling,
                disclosure_ceiling: grant.disclosure_ceiling,
                source_read_permission: true,
                exact_scan_permission: false,
                issued_boot_id: grant.issued_boot_id.clone(),
                revocation_generation: 1,
                maximum_ttl_ms: 60_000,
            };
            let policy_state = AuthoritativePolicyState {
                namespace_id: SourceNamespaceId::from_bytes([0x1B; 16]),
                owner_generation: SourceOwnerGeneration::from_bytes([0x1C; 32]),
                policy_revision: AccessPolicyRevision::new(1),
                shadow_revision: ShadowFenceRevision::new(1),
                purge_revision: PurgeFenceRevision::new(1),
            };
            let mut fence = QuerySnapshotFence {
                installation_incarnation_id: incarnation,
                collection_generation_id: Some(generation),
                visible_epoch: Some(Epoch::new(1).expect("epoch")),
                collection_route_revision: CollectionRouteRevision::new(1),
                catalog_revision: CatalogRevision::new(1),
                membership_revision: MembershipRevision::new(1),
                reference_portfolio_revision: None,
                access_policy_revision: policy_state.policy_revision,
                shadow_fence_revision: policy_state.shadow_revision,
                purge_fence_revision: policy_state.purge_revision,
                overlay_revision: OverlayRevision::new(1),
                observation_cursor_revision: ObservationCursorRevision::new(1),
                observation_freshness: ObservationFreshness {
                    state: ObservationFreshnessState::CurrentConfirmed,
                    observation_cursor_revision: ObservationCursorRevision::new(1),
                    observed_age_ms: None,
                },
                source_view: SourceView::RetainedRevision(SourceRevisionId::from_bytes([0x1D; 16])),
                workspace_view_revision_ref: None,
                lexical_profile_ids: BoundedList::empty(),
                snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([0; 32]),
            };
            seal_snapshot(&mut fence);
            let route = CollectionRoute {
                collection_generation_id: generation,
                physical_collection_name: OpaqueRef::new("fixture-collection").expect("collection"),
                schema_identity_digest: *schema.identity().expect("schema identity").as_digest(),
                qualified_qdrant_build: ArtifactDigest::from_bytes([0x1E; 32]),
                committed_visible_epoch: fence.visible_epoch.expect("visible epoch"),
                collection_route_revision: fence.collection_route_revision,
            };
            Self {
                grant,
                policy,
                scope: RequestedMembershipScope {
                    memberships: BTreeSet::from([membership]),
                },
                access: AuthoritativeAccessSnapshot {
                    generation: 217,
                    source_catalog_generation: 1,
                    membership_generation: 1,
                    bindings: BTreeMap::from([(
                        membership,
                        MembershipAccessBinding {
                            membership_id: membership,
                            access_partition_digest: Blake3Digest32::from_bytes([0x1F; 32]),
                            scoring_partition_digest: Blake3Digest32::from_bytes([0x20; 32]),
                            projection_membership_id: OpaqueId::new("legacy-alias-not-used")
                                .expect("legacy alias"),
                            active: true,
                        },
                    )]),
                    snapshot_digest: Blake3Digest32::from_bytes([0x21; 32]),
                },
                policy_fence: NamespacePolicyFence {
                    namespace_id: policy_state.namespace_id,
                    owner_generation: policy_state.owner_generation,
                    policy_revision: policy_state.policy_revision,
                },
                policy_state,
                fence,
                schema,
                route,
                rows: BTreeMap::from([(projection, row)]),
                manifests: BTreeMap::from([(
                    projection,
                    ReceiptRef::new("manifest-fixture").expect("manifest"),
                )]),
                live: LiveSecurityState {
                    generation: 217,
                    denied_memberships: BTreeSet::new(),
                    purged_memberships: BTreeSet::new(),
                    fail_closed: false,
                    snapshot_digest: Blake3Digest32::from_bytes([0x22; 32]),
                },
            }
        }

        fn compile(&self) -> CompiledEligibilityPopulation {
            let issued = IssuedFixture(self.grant.clone());
            let mut owner = OwnerFixture {
                claims: self.grant.clone(),
                bindings: self
                    .rows
                    .values()
                    .map(|row| (row.source_membership_id, row.clone()))
                    .collect(),
                fence: self.fence.clone(),
            };
            compile_canonical_eligibility_population(
                &mut owner,
                &issued,
                &CanonicalEligibilityPopulationRequest::<()> {
                    grant: &self.grant,
                    grant_policy: &self.policy,
                    recipe: RecipeIdV1::Locate,
                    modality: Modality::Code,
                    requested_scope: &self.scope,
                    authoritative_access: &self.access,
                    namespace_policy_fence: &self.policy_fence,
                    authoritative_policy: &self.policy_state,
                    snapshot_fence: &self.fence,
                    collection_schema: &self.schema,
                    collection_route: &self.route,
                    publication_manifests: &self.manifests,
                    publication_shadows: &BTreeMap::new(),
                    projection_memberships: &self.rows,
                    security: CanonicalEligibilitySecuritySnapshot {
                        live: &self.live,
                        namespace_id: self.policy_state.namespace_id,
                        access_policy_revision: self.policy_state.policy_revision,
                        shadow_fence_revision: self.policy_state.shadow_revision,
                        purge_fence_revision: self.policy_state.purge_revision,
                        overlay_revision: self.fence.overlay_revision,
                    },
                },
            )
            .expect("real C18 compiler fixture")
        }
    }

    fn seal_snapshot(snapshot: &mut QuerySnapshotFence) {
        let input = snapshot
            .canonical_fingerprint_input()
            .expect("snapshot fingerprint input");
        let digest = blake3_exact_bytes(
            input.as_slice(),
            Blake3ExactInputLimit::new(input.len()).expect("bounded fingerprint input"),
        )
        .expect("snapshot fingerprint");
        snapshot.snapshot_fingerprint = QuerySnapshotFingerprint::from_bytes(*digest.as_bytes());
    }

    struct ExecutorFixture {
        compiled: CompiledEligibilityPopulation,
        ticket: LegTicket,
        pins: LegPinSet,
        _registry: PinRegistry,
        request_security: RequestSecurityFence,
        live: LiveSecurityState,
        membership: SourceMembershipId,
        projection: ProjectionMembershipId,
        vector_name: VectorName,
        query: VectorValue,
    }

    impl ExecutorFixture {
        fn new() -> Self {
            let population_fixture = PopulationFixture::new();
            let compiled = population_fixture.compile();
            let membership = *population_fixture
                .scope
                .memberships
                .first()
                .expect("single C18 membership");
            let projection = compiled
                .projection_memberships()
                .get(&membership)
                .expect("owner-retained C18 link")
                .projection_membership_id;
            let route = IndexedRouteFence {
                collection_generation_id: compiled.population().collection_generation_id(),
                visible_epoch: compiled.population().visible_epoch(),
                route_generation: 1,
                owner_epoch: OwnerEpoch::new(1).expect("owner epoch"),
            };
            let access_binding = MembershipAccessBinding {
                membership_id: membership,
                access_partition_digest: Blake3Digest32::from_bytes([0xA1; 32]),
                scoring_partition_digest: Blake3Digest32::from_bytes([0xB2; 32]),
                projection_membership_id: OpaqueId::new("planner-binding-only")
                    .expect("planner binding"),
                active: true,
            };
            let safe_leg = search_access::SafeRetrievalLeg {
                leg_id: 0,
                memberships: BTreeSet::from([membership]),
                eligibility_plans: vec![
                    compile_base_eligibility(&access_binding, route, 217, 1, 1)
                        .expect("C15 eligibility plan"),
                ],
                route,
                overlap_proof_digest: None,
            };
            let compiled_plan = canonical_plan_fixture::compile_lexical_plan(
                search_contracts::RequestId::from_bytes([0x23; 16]),
                safe_leg,
                canonical_plan_fixture::lexical_budget(1_000, 4, 4096, 500, 1_048_576),
            );
            let leg = compiled_plan
                .legs()
                .iter()
                .find(|leg| leg.leg_kind() == search_contracts::LegKind::Lexical)
                .expect("C15 compiled lexical leg")
                .clone();
            let ticket = LegTicket {
                request_id: compiled_plan.request_id(),
                plan_digest: compiled_plan.digest(),
                owner_id: OpaqueId::new("canonical-executor-test").expect("owner"),
                leg,
                access_permit: AccessPermit {
                    checkpoint: AccessCheckpoint::BeforeLegDispatch,
                    live_generation: population_fixture.live.generation,
                    live_snapshot_digest: population_fixture.live.snapshot_digest,
                },
                issued_at_tick: 100,
            };
            let registry = PinRegistry::new(
                RouteIdentity {
                    collection_generation_id: route.collection_generation_id,
                    route_revision: CollectionRouteRevision::new(route.route_generation),
                },
                route.visible_epoch,
                PinLimits::BASELINE,
            )
            .expect("C15 route pin registry");
            let pins = acquire_leg_pins(&ticket, &registry, 1_000).expect("C15 route pin");
            Self {
                compiled,
                ticket,
                pins,
                _registry: registry,
                request_security: RequestSecurityFence {
                    planned_generation: population_fixture.live.generation,
                    memberships: BTreeSet::from([membership]),
                },
                live: population_fixture.live,
                membership,
                projection,
                vector_name: VectorName::new("lex_code_v1").expect("vector name"),
                query: VectorValue::Sparse {
                    indices: vec![0, 2],
                    values: vec![1.0, 0.5],
                },
            }
        }

        fn execute<'a>(
            &'a self,
            live_before: &LiveSecurityState,
            live_after: &LiveSecurityState,
            port: &mut RecordingPort<'a>,
        ) -> Result<super::CanonicalIndexedLegOutput<'a, u8>, ExecuteError> {
            execute_canonical_indexed_leg(
                &self.ticket,
                &self.pins,
                &self.compiled,
                &self.vector_name,
                &self.query,
                4,
                &self.request_security,
                live_before,
                live_after,
                port,
            )
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CanonicalCall {
        Query,
        ExactReadback,
        ExactCount,
    }

    struct RecordingPort<'population> {
        compiled: &'population CompiledEligibilityPopulation,
        batch: CanonicalIndexedReadbackBatch<u8>,
        denominator: u64,
        context_error: Option<CanonicalIndexedPortError>,
        request_checks: Cell<usize>,
        calls: Vec<(CanonicalCall, &'population CompiledEligibilityPopulation)>,
    }

    impl<'population> RecordingPort<'population> {
        fn new(
            compiled: &'population CompiledEligibilityPopulation,
            batch: CanonicalIndexedReadbackBatch<u8>,
            denominator: u64,
            context_error: Option<CanonicalIndexedPortError>,
        ) -> Self {
            Self {
                compiled,
                batch,
                denominator,
                context_error,
                request_checks: Cell::new(0),
                calls: Vec::new(),
            }
        }

        fn record(&mut self, call: CanonicalCall) {
            self.calls.push((call, self.compiled));
        }
    }

    impl CanonicalIndexedRetrievalPort for RecordingPort<'_> {
        type Candidate = u8;

        fn compiled_population(&self) -> &CompiledEligibilityPopulation {
            self.compiled
        }

        fn query_and_readback(
            &mut self,
            _vector_name: &VectorName,
            _query: &VectorValue,
            _limit: usize,
        ) -> Result<CanonicalIndexedReadbackBatch<Self::Candidate>, CanonicalIndexedPortError>
        {
            self.record(CanonicalCall::Query);
            self.record(CanonicalCall::ExactReadback);
            Ok(self.batch.clone())
        }

        fn count_exact(&mut self) -> Result<u64, CanonicalIndexedPortError> {
            self.record(CanonicalCall::ExactCount);
            Ok(self.denominator)
        }

        fn check_request(&self) -> Result<(), CanonicalIndexedPortError> {
            self.request_checks
                .set(self.request_checks.get().saturating_add(1));
            if let Some(error) = self.context_error {
                return Err(error);
            }
            Ok(())
        }
    }

    #[test]
    fn canonical_executor_uses_one_compiled_population_and_refuses_before_dispatch() {
        let fixture = ExecutorFixture::new();
        let returned = CanonicalPointAddress::from_bytes([0x31; 16]);
        let disappeared = CanonicalPointAddress::from_bytes([0x32; 16]);
        let batch = CanonicalIndexedReadbackBatch {
            nominated_addresses: vec![returned, disappeared],
            readback_missing_addresses: vec![disappeared],
            nominations: vec![CanonicalIndexedReadback {
                point_address: returned,
                projection_membership_id: fixture.projection,
                raw_score: 2.0,
                candidate: 0xAA,
            }],
        };
        let mut port = RecordingPort::new(&fixture.compiled, batch.clone(), 1, None);
        let output = fixture
            .execute(&fixture.live, &fixture.live, &mut port)
            .expect("canonical indexed execution");
        assert_eq!(
            port.calls.iter().map(|(call, _)| *call).collect::<Vec<_>>(),
            vec![
                CanonicalCall::Query,
                CanonicalCall::ExactReadback,
                CanonicalCall::ExactCount,
            ]
        );
        assert!(
            port.calls
                .iter()
                .all(|(_, population)| std::ptr::eq(*population, &fixture.compiled))
        );
        assert!(std::ptr::eq(output.compiled_population, &fixture.compiled));
        assert_eq!(
            output.compiled_population.population(),
            fixture.compiled.population()
        );
        assert_eq!(output.nominations.len(), 1);
        assert_eq!(
            output.nominations[0].source_membership_id,
            fixture.membership
        );
        assert_eq!(
            output.nominations[0].projection_membership_id,
            fixture.projection
        );
        assert_eq!(output.exact_denominator, 1);
        assert_eq!(output.readback_missing_addresses, vec![disappeared]);
        assert_eq!(output.completion, LegCompletion::PartialCandidateScope);
        assert_eq!(port.request_checks.get(), 2);

        for (context_error, expected) in [
            (
                CanonicalIndexedPortError::Cancelled,
                ExecuteError::Cancelled,
            ),
            (
                CanonicalIndexedPortError::DeadlineExceeded,
                ExecuteError::DeadlineExceeded,
            ),
        ] {
            let mut refused =
                RecordingPort::new(&fixture.compiled, batch.clone(), 1, Some(context_error));
            assert!(matches!(
                fixture.execute(&fixture.live, &fixture.live, &mut refused),
                Err(error) if error == expected
            ));
            assert!(
                refused.calls.is_empty(),
                "request refusal dispatched retrieval"
            );
            assert_eq!(refused.request_checks.get(), 1);
        }

        for (restriction, expected) in [
            (0, ExecuteError::AccessDenied),
            (1, ExecuteError::AccessDenied),
        ] {
            let mut live_before = fixture.live.clone();
            if restriction == 0 {
                live_before.denied_memberships.insert(fixture.membership);
            } else {
                live_before.purged_memberships.insert(fixture.membership);
            }
            let mut refused = RecordingPort::new(&fixture.compiled, batch.clone(), 1, None);
            assert!(matches!(
                fixture.execute(&live_before, &fixture.live, &mut refused),
                Err(error) if error == expected
            ));
            assert!(
                refused.calls.is_empty(),
                "live restriction dispatched retrieval"
            );
            assert_eq!(refused.request_checks.get(), 0);
        }
    }
}
