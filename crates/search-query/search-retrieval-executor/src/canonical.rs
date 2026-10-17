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
    AccessCheckpoint, AccessError, LiveSecurityState, RequestSecurityFence, recheck_live_access,
};
use search_contracts::indexed::{EligibilityPopulation, VectorName, VectorValue};
use search_contracts::{
    CollectionRouteRevision, LegKind, ProjectionMembershipId, SourceMembershipId,
};
use search_epoch_pins::RouteIdentity;

use crate::{ExecuteError, LegCompletion, LegPinSet, LegTicket};

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

/// Typed link supplied by the current projection owner for one leg.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CanonicalMembershipBinding {
    /// Membership identity already present in the immutable leg ticket.
    pub source_membership_id: SourceMembershipId,
    /// Exact C00 projection membership identity from the projection owner.
    pub projection_membership_id: ProjectionMembershipId,
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
/// Implementations bind one borrowed `EligibilityPopulation` and one shared
/// request context at construction. Query (including filtered IDF), point
/// readback, and exact count therefore cannot receive diverging filters or
/// independently refreshed deadlines.
pub trait CanonicalIndexedRetrievalPort {
    /// Typed exact-readback product selected by the daemon adapter.
    type Candidate;

    /// The one population value retained for the complete request leg.
    fn population(&self) -> &EligibilityPopulation;

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
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalIndexedLegOutput<'population, Candidate> {
    /// The same owner-compiled population used by the real adapter for every
    /// query, readback, and count call. This scopes the raw scores.
    pub population: &'population EligibilityPopulation,
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

/// Execute one immutable ticket through its canonical typed population.
///
/// The caller passes the population borrowed from the opaque C18 compilation
/// result. The port must retain that same reference; the pointer check prevents
/// a second equal-looking population from replacing the request owner.
#[allow(clippy::too_many_arguments)]
pub fn execute_canonical_indexed_leg<'population, P: CanonicalIndexedRetrievalPort>(
    ticket: &LegTicket,
    pins: &LegPinSet,
    population: &'population EligibilityPopulation,
    membership_bindings: &[CanonicalMembershipBinding],
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
    if !std::ptr::eq(population, port.population()) {
        return Err(ExecuteError::PopulationMismatch);
    }
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
    let source_by_projection =
        validate_membership_bindings(&safe_leg.memberships, population, membership_bindings)?;

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
        population,
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

fn validate_membership_bindings(
    ticket_memberships: &BTreeSet<SourceMembershipId>,
    population: &EligibilityPopulation,
    bindings: &[CanonicalMembershipBinding],
) -> Result<BTreeMap<ProjectionMembershipId, SourceMembershipId>, ExecuteError> {
    let population_memberships = population
        .allowed_projection_memberships()
        .into_iter()
        .collect::<BTreeSet<_>>();
    if population_memberships.is_empty()
        || bindings.len() != population_memberships.len()
        || bindings.len() != ticket_memberships.len()
    {
        return Err(ExecuteError::PopulationMismatch);
    }
    let mut source_by_projection = BTreeMap::new();
    let mut sources = BTreeSet::new();
    for binding in bindings {
        if !ticket_memberships.contains(&binding.source_membership_id)
            || !population_memberships.contains(&binding.projection_membership_id)
            || !sources.insert(binding.source_membership_id)
            || source_by_projection
                .insert(
                    binding.projection_membership_id,
                    binding.source_membership_id,
                )
                .is_some()
        {
            return Err(ExecuteError::PopulationMismatch);
        }
    }
    if &sources != ticket_memberships
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
    use super::{
        CanonicalMembershipBinding, CanonicalPointAddress, candidate_scope_completion,
        validate_membership_bindings,
    };
    use crate::LegCompletion;
    use search_contracts::indexed::EligibilityPopulation;
    use search_contracts::{
        AccessPartitionId, CollectionGenerationId, Epoch, InstallationIncarnationId,
        ProjectionMembershipId, ProjectionProfileSetId, ScoringPartitionId, SourceMembershipId,
    };

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

    #[test]
    fn membership_binding_is_a_typed_bijection_for_the_ticket_and_population() {
        let source = SourceMembershipId::from_bytes([6; 16]);
        let projection = ProjectionMembershipId::from_bytes([7; 16]);
        let members = [source].into_iter().collect();
        let compiled = population(&[projection]);
        let bindings = [CanonicalMembershipBinding {
            source_membership_id: source,
            projection_membership_id: projection,
        }];

        let map = validate_membership_bindings(&members, &compiled, &bindings)
            .expect("exact typed binding");
        assert_eq!(map.get(&projection), Some(&source));

        let wrong_projection = ProjectionMembershipId::from_bytes([8; 16]);
        let wrong = [CanonicalMembershipBinding {
            source_membership_id: source,
            projection_membership_id: wrong_projection,
        }];
        assert!(validate_membership_bindings(&members, &compiled, &wrong).is_err());
    }

    #[test]
    fn an_equal_copy_is_not_the_same_population_owner() {
        let projection = ProjectionMembershipId::from_bytes([7; 16]);
        let first = population(&[projection]);
        let second = first.clone();
        assert_eq!(first, second);
        assert!(!std::ptr::eq(&first, &second));
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
}
