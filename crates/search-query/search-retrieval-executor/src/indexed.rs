//! Single pinned route/epoch indexed retrieval over one accepted contract.
//!
//! This module executes exactly one [`LegTicket`](super::LegTicket) whose leg
//! carries a [`SafeRetrievalLeg`] with a single eligibility plan. Retrieval
//! and IDF are rendered from the single [`EligibilityPredicates`] contract:
//! the caller passes `idf_scoped_to_retrieval = true` so the backend scores
//! with the retrieval filter as the IDF corpus (the `ScopedToRetrieval`
//! rendering in `search-qdrant-bridge::real`). Collection-wide IDF is
//! rejected with [`ExecuteError::PopulationMismatch`](super::ExecuteError)
//! because denied documents must never move permitted denominators
//! (invariant 5).
//!
//! Backend outputs are nominations only: point identifiers, memberships,
//! digests and finite scores. No payload bytes cross here and nothing
//! returned here is source evidence. Evidence is produced later by
//! `search-candidate-validator` through exact revision readback, followed by
//! a final live recheck before projection. Qdrant payload text is never
//! accepted as evidence.
//!
//! The exact filter-population denominator travels with the output so the
//! spine gate can prove that indexed top-k saturation never narrows an
//! exact-proof denominator (invariant 6): `nominations.len() >
//! exact_denominator` fails with `PopulationMismatch`, saturation yields
//! [`LegCompletion::PartialCandidateScope`](super::LegCompletion), and only
//! an exact length match yields `CompleteCandidateScope`.
//!
//! Unavailable stays distinct from failure: [`IndexedPortError::Unavailable`]
//! maps to [`ExecuteError::BackendUnavailable`](super::ExecuteError) while
//! [`IndexedPortError::Failure`] maps to `BackendFailure`. Callers must
//! handle `BackendUnavailable` explicitly; falling back to DIRECT success
//! silently is forbidden.

use std::collections::BTreeSet;

use search_access::{
    AccessCheckpoint, AccessError, EligibilityPredicates, LiveSecurityState, RequestSecurityFence,
    recheck_live_access,
};
use search_contracts::{
    Blake3Digest32, CollectionRouteRevision, LegKind, OpaqueId, SourceMembershipId,
};
use search_epoch_pins::RouteIdentity;

use super::{ExecuteError, LegCompletion, LegOutput, LegPinSet, LegTicket, RawNomination};

/// Maximum nominations returned by one indexed leg.
///
/// Matches the reference bridge baseline ceiling so the executor never asks
/// for more than any conforming backend can return.
pub const MAX_INDEXED_LIMIT: usize = 4096;

/// Maximum sparse query values accepted in one indexed dispatch.
pub const MAX_QUERY_VECTOR_VALUES: usize = 4096;

/// Maximum vector-name bytes accepted in one indexed dispatch.
pub const MAX_VECTOR_NAME_BYTES: usize = 64;

/// Closed indexed-port failure.
///
/// `Unavailable` (wrong route/generation, missing collection, missing
/// schema) stays distinct from `Failure` (transport loss, malformed
/// response, budget rejection) so callers can report indexed-unavailable
/// without relabelling it as DIRECT completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexedPortError {
    /// The pinned route is not being served: retry is possible, fallback to
    /// DIRECT success is forbidden.
    Unavailable,
    /// The dispatch itself failed: the leg outcome stays a typed failure.
    Failure,
}

impl IndexedPortError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "INDEXED_PORT_UNAVAILABLE",
            Self::Failure => "INDEXED_PORT_FAILURE",
        }
    }
}

/// One backend nomination.
///
/// Carries identity only: no payload bytes, no excerpts, no source text.
/// The nomination becomes evidence-bearing only after exact revision
/// readback in `search-candidate-validator`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexedNomination {
    /// Provider-neutral 128-bit point identifier.
    pub point_id: [u8; 16],
    /// Membership the backend attributes to the point.
    ///
    /// The daemon port resolves this through exact point readback
    /// (`readback_exact`), never by trusting the scored-query payload
    /// alone; a point whose readback membership falls outside the ticket
    /// memberships fails the whole leg.
    pub source_membership_id: SourceMembershipId,
    /// Identity digest hint; verified against readback and later against
    /// the exact source revision.
    pub identity_digest: Blake3Digest32,
    /// Payload digest hint; verified against readback and later against
    /// the exact source revision.
    pub payload_digest: Blake3Digest32,
    /// Finite backend score; comparable only inside the single scoring
    /// population named by the returned digest.
    pub raw_score: f32,
}

/// Vendor-neutral indexed retrieval seam.
///
/// Both methods render from the single predicate contract supplied by the
/// caller: `query_filtered` scores with the retrieval filter and the
/// retrieval filter as the IDF corpus, `count_exact` counts the same filter
/// population. A diverged second filter argument is unrepresentable.
pub trait IndexedRetrievalPort {
    /// Returns bounded filtered nominations for one already-authorized leg.
    fn query_filtered(
        &mut self,
        ticket: &LegTicket,
        predicates: &EligibilityPredicates,
        vector_name: &str,
        query_vector: &[(u32, f32)],
        limit: usize,
        idf_scoped_to_retrieval: bool,
    ) -> Result<Vec<IndexedNomination>, IndexedPortError>;

    /// Counts the exact already-authorized filter population.
    fn count_exact(
        &mut self,
        ticket: &LegTicket,
        predicates: &EligibilityPredicates,
    ) -> Result<usize, IndexedPortError>;
}

/// Indexed leg product: bounded nominations plus their exact denominator.
///
/// `output` carries nominations only. `exact_denominator` is the exact
/// filter-population count from the same single contract, so downstream
/// validation and projection can prove top-k never narrowed the proof.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexedLegOutput {
    /// Bounded leg output with nominations only.
    pub output: LegOutput,
    /// Exact count of the same filter population.
    pub exact_denominator: usize,
}

/// Executes one single-membership indexed leg against the pinned route/epoch.
///
/// Fixed order: ticket shape, single-plan/single-contract predicate match,
/// filtered-IDF scope, single-pin route/epoch binding, budget and query
/// shape, pre-dispatch live recheck (deny before scoring), backend query and
/// exact count from the same contract, post-leg live recheck against the
/// post-dispatch fence (restrictive moves contaminate the whole leg),
/// bounded nomination mapping with deterministic score/candidate ordering,
/// and denominator proof.
///
/// `live_before` is the fence observed at dispatch; `live_after` is the
/// fence re-read after the backend answered. Passing the same snapshot twice
/// is allowed only when no fence move was observed; a fence move between the
/// two must be surfaced by passing the fresh state as `live_after` so the
/// influenced leg contaminates instead of scoring on stale authority.
///
/// # Errors
///
/// Returns [`ExecuteError`] for stale tickets, diverged predicates,
/// unscoped IDF, missing or mismatched pins, exhausted budgets, malformed
/// queries, denied or contaminated live state, backend failures and
/// nomination/denominator divergence. Every failure is typed; no silent
/// fallback is possible.
pub fn execute_indexed_leg<P: IndexedRetrievalPort>(
    ticket: &LegTicket,
    pins: &LegPinSet,
    predicates: &EligibilityPredicates,
    vector_name: &str,
    query_vector: &[(u32, f32)],
    limit: usize,
    idf_scoped_to_retrieval: bool,
    request_security: &RequestSecurityFence,
    live_before: &LiveSecurityState,
    live_after: &LiveSecurityState,
    port: &mut P,
) -> Result<IndexedLegOutput, ExecuteError> {
    let safe_leg = ticket
        .leg
        .safe_index_leg
        .as_ref()
        .ok_or(ExecuteError::InvalidExecutionState)?;
    if ticket.leg.leg_kind != LegKind::Lexical {
        return Err(ExecuteError::InvalidExecutionState);
    }
    if safe_leg.eligibility_plans.len() != 1 {
        return Err(ExecuteError::PopulationMismatch);
    }
    if !predicates.is_consistent() {
        return Err(ExecuteError::PopulationMismatch);
    }
    let plan = &safe_leg.eligibility_plans[0];
    if *predicates != plan.predicates() {
        return Err(ExecuteError::PopulationMismatch);
    }
    if plan.collection_generation_id != safe_leg.route.collection_generation_id
        || plan.visible_epoch != safe_leg.route.visible_epoch
    {
        return Err(ExecuteError::PopulationMismatch);
    }
    if !idf_scoped_to_retrieval {
        return Err(ExecuteError::PopulationMismatch);
    }
    if pins.len() != 1 {
        return Err(ExecuteError::PinAcquisitionFailed);
    }
    let Some(guard) = pins.epoch_pins().first() else {
        return Err(ExecuteError::PinAcquisitionFailed);
    };
    let expected_route = RouteIdentity {
        collection_generation_id: safe_leg.route.collection_generation_id,
        route_revision: CollectionRouteRevision::new(safe_leg.route.route_generation),
    };
    if guard.route() != expected_route || guard.epoch() != safe_leg.route.visible_epoch {
        return Err(ExecuteError::PinAcquisitionFailed);
    }
    if ticket.leg.memberships != safe_leg.memberships {
        return Err(ExecuteError::InvalidExecutionState);
    }
    let budget_limit = usize::try_from(ticket.leg.budget.max_candidates)
        .map_err(|_| ExecuteError::BudgetExceeded)?;
    if limit == 0 || limit > MAX_INDEXED_LIMIT || limit > budget_limit {
        return Err(ExecuteError::BudgetExceeded);
    }
    validate_vector_name(vector_name)?;
    validate_query_vector(query_vector)?;
    recheck_live_access(
        request_security,
        live_before,
        AccessCheckpoint::BeforeLegDispatch,
    )?;
    let nominations = port
        .query_filtered(
            ticket,
            predicates,
            vector_name,
            query_vector,
            limit,
            idf_scoped_to_retrieval,
        )
        .map_err(map_port_error)?;
    if nominations.len() > limit {
        return Err(ExecuteError::InvalidNomination);
    }
    let denominator = port
        .count_exact(ticket, predicates)
        .map_err(map_port_error)?;
    recheck_live_access(
        request_security,
        live_after,
        AccessCheckpoint::AfterLegCompletion,
    )
    .map_err(map_post_leg_error)?;
    if nominations.len() > denominator {
        return Err(ExecuteError::PopulationMismatch);
    }
    let population = Blake3Digest32::from_bytes(predicates.retrieval_digest.0);
    let mut mapped = Vec::with_capacity(nominations.len());
    let mut ids = BTreeSet::new();
    for nomination in &nominations {
        if !nomination.raw_score.is_finite() {
            return Err(ExecuteError::InvalidNomination);
        }
        if !ticket
            .leg
            .memberships
            .contains(&nomination.source_membership_id)
        {
            return Err(ExecuteError::InvalidNomination);
        }
        let candidate_id = OpaqueId::new(format!(
            "candidate:{}:{}",
            ticket.leg.leg_id,
            hex_point(nomination.point_id)
        ))
        .map_err(|_| ExecuteError::InvalidNomination)?;
        if !ids.insert(candidate_id.clone()) {
            return Err(ExecuteError::InvalidNomination);
        }
        mapped.push(RawNomination {
            candidate_id,
            point_id: nomination.point_id,
            source_membership_id: nomination.source_membership_id,
            identity_digest: nomination.identity_digest,
            payload_digest: nomination.payload_digest,
            raw_score: nomination.raw_score,
            scoring_population_digest: population,
        });
    }
    mapped.sort_by(|left, right| {
        right
            .raw_score
            .partial_cmp(&left.raw_score)
            .unwrap_or(core::cmp::Ordering::Equal)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let completion = if mapped.len() == denominator {
        LegCompletion::CompleteCandidateScope
    } else {
        LegCompletion::PartialCandidateScope
    };
    Ok(IndexedLegOutput {
        output: LegOutput {
            leg_id: ticket.leg.leg_id,
            leg_kind: ticket.leg.leg_kind,
            memberships: ticket.leg.memberships.clone(),
            security_generation: live_after.generation,
            scoring_population_digest: Some(population),
            nominations: mapped,
            completion,
        },
        exact_denominator: denominator,
    })
}

const fn map_port_error(error: IndexedPortError) -> ExecuteError {
    match error {
        IndexedPortError::Unavailable => ExecuteError::BackendUnavailable,
        IndexedPortError::Failure => ExecuteError::BackendFailure,
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

fn validate_vector_name(name: &str) -> Result<(), ExecuteError> {
    if name.is_empty()
        || name.len() > MAX_VECTOR_NAME_BYTES
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(ExecuteError::InvalidNomination);
    }
    Ok(())
}

fn validate_query_vector(query: &[(u32, f32)]) -> Result<(), ExecuteError> {
    if query.is_empty() || query.len() > MAX_QUERY_VECTOR_VALUES {
        return Err(ExecuteError::InvalidNomination);
    }
    if query.iter().any(|(_, value)| !value.is_finite()) {
        return Err(ExecuteError::InvalidNomination);
    }
    if query.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(ExecuteError::InvalidNomination);
    }
    Ok(())
}

fn hex_point(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(32);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_access::{
        AccessCheckpoint, AccessPermit, IndexedRouteFence, MembershipAccessBinding,
    };
    use search_contracts::{CollectionGenerationId, Epoch, LegKind, OwnerEpoch, RequestId};
    use search_epoch_pins::{PinLimits, PinRegistry};
    use search_query_planner::{CancellationBoundary, LegBudget, PlannedLeg};

    struct StubPort {
        nominations: Vec<IndexedNomination>,
        denominator: usize,
        fail_query: Option<IndexedPortError>,
        fail_count: Option<IndexedPortError>,
        query_calls: usize,
        count_calls: usize,
        observed_scoped: Option<bool>,
    }

    impl StubPort {
        fn ready(nominations: Vec<IndexedNomination>, denominator: usize) -> Self {
            Self {
                nominations,
                denominator,
                fail_query: None,
                fail_count: None,
                query_calls: 0,
                count_calls: 0,
                observed_scoped: None,
            }
        }
    }

    impl IndexedRetrievalPort for StubPort {
        fn query_filtered(
            &mut self,
            _ticket: &LegTicket,
            _predicates: &EligibilityPredicates,
            _vector_name: &str,
            _query_vector: &[(u32, f32)],
            _limit: usize,
            idf_scoped_to_retrieval: bool,
        ) -> Result<Vec<IndexedNomination>, IndexedPortError> {
            self.query_calls += 1;
            self.observed_scoped = Some(idf_scoped_to_retrieval);
            if let Some(error) = self.fail_query {
                return Err(error);
            }
            Ok(self.nominations.clone())
        }

        fn count_exact(
            &mut self,
            _ticket: &LegTicket,
            _predicates: &EligibilityPredicates,
        ) -> Result<usize, IndexedPortError> {
            self.count_calls += 1;
            if let Some(error) = self.fail_count {
                return Err(error);
            }
            Ok(self.denominator)
        }
    }

    struct Fixture {
        ticket: LegTicket,
        predicates: EligibilityPredicates,
        request_security: RequestSecurityFence,
        live_security: LiveSecurityState,
        registry: PinRegistry,
        membership: SourceMembershipId,
    }

    fn membership(byte: u8) -> SourceMembershipId {
        SourceMembershipId::from_bytes([byte; 16])
    }

    fn binding(membership_id: SourceMembershipId) -> MembershipAccessBinding {
        MembershipAccessBinding {
            membership_id,
            access_partition_digest: Blake3Digest32::from_bytes([0xA1; 32]),
            scoring_partition_digest: Blake3Digest32::from_bytes([0xB2; 32]),
            projection_membership_id: OpaqueId::new("projection:test").expect("fixture id"),
            active: true,
        }
    }

    fn route() -> IndexedRouteFence {
        IndexedRouteFence {
            collection_generation_id: CollectionGenerationId::from_bytes([0x31; 16]),
            visible_epoch: Epoch::new(7).expect("fixture epoch"),
            route_generation: 3,
            owner_epoch: OwnerEpoch::new(1).expect("fixture owner epoch"),
        }
    }

    fn fixture() -> Fixture {
        fixture_with_membership(membership(0x11))
    }

    fn fixture_with_membership(membership_id: SourceMembershipId) -> Fixture {
        let fence = route();
        let plan = search_access::compile_base_eligibility(&binding(membership_id), fence, 9, 2, 2)
            .expect("fixture plan");
        let predicates = plan.predicates();
        let safe_leg = search_access::SafeRetrievalLeg {
            leg_id: 0,
            memberships: BTreeSet::from([membership_id]),
            eligibility_plans: vec![plan],
            route: fence,
            overlap_proof_digest: None,
        };
        let ticket = LegTicket {
            request_id: RequestId::from_bytes([0x21; 16]),
            plan_digest: search_query_planner::CompiledPlanDigest([0x33; 32]),
            owner_id: OpaqueId::new("owner:test").expect("fixture owner"),
            leg: PlannedLeg {
                leg_id: 0,
                leg_kind: LegKind::Lexical,
                depends_on: Vec::new(),
                memberships: BTreeSet::from([membership_id]),
                safe_index_leg: Some(safe_leg),
                budget: LegBudget {
                    deadline_ms: 1_000,
                    max_candidates: 8,
                    max_source_read_bytes: 1_048_576,
                    max_cpu_ms: 500,
                    max_memory_bytes: 67_108_864,
                },
                cancellation_boundary: CancellationBoundary::BeforeDispatch,
            },
            access_permit: AccessPermit {
                checkpoint: AccessCheckpoint::BeforeLegDispatch,
                live_generation: 9,
                live_snapshot_digest: Blake3Digest32::from_bytes([0x5A; 32]),
            },
            issued_at_tick: 100,
        };
        let request_security = RequestSecurityFence {
            planned_generation: 9,
            memberships: BTreeSet::from([membership_id]),
        };
        let live_security = LiveSecurityState {
            generation: 9,
            denied_memberships: BTreeSet::new(),
            purged_memberships: BTreeSet::new(),
            fail_closed: false,
            snapshot_digest: Blake3Digest32::from_bytes([0x5A; 32]),
        };
        let registry = PinRegistry::new(
            RouteIdentity {
                collection_generation_id: fence.collection_generation_id,
                route_revision: CollectionRouteRevision::new(fence.route_generation),
            },
            fence.visible_epoch,
            PinLimits::BASELINE,
        )
        .expect("fixture registry");
        Fixture {
            ticket,
            predicates,
            request_security,
            live_security,
            registry,
            membership: membership_id,
        }
    }

    fn nomination(
        point_byte: u8,
        membership_id: SourceMembershipId,
        score: f32,
    ) -> IndexedNomination {
        IndexedNomination {
            point_id: [point_byte; 16],
            source_membership_id: membership_id,
            identity_digest: Blake3Digest32::from_bytes([point_byte; 32]),
            payload_digest: Blake3Digest32::from_bytes([point_byte.wrapping_add(1); 32]),
            raw_score: score,
        }
    }

    fn pins(fixture: &Fixture) -> LegPinSet {
        super::super::acquire_leg_pins(&fixture.ticket, &fixture.registry, 1_000)
            .expect("fixture pins")
    }

    fn query() -> Vec<(u32, f32)> {
        vec![(0, 1.0), (3, 0.5)]
    }

    #[test]
    fn happy_path_returns_nominations_with_exact_denominator() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(
            vec![
                nomination(0x01, fixture.membership, 2.0),
                nomination(0x02, fixture.membership, 1.0),
            ],
            2,
        );
        let output = execute_indexed_leg(
            &fixture.ticket,
            &pin_set,
            &fixture.predicates,
            "lexical",
            &query(),
            8,
            true,
            &fixture.request_security,
            &fixture.live_security,
            &fixture.live_security,
            &mut port,
        )
        .expect("indexed leg executes");
        assert_eq!(output.exact_denominator, 2);
        assert_eq!(
            output.output.completion,
            LegCompletion::CompleteCandidateScope
        );
        assert_eq!(output.output.nominations.len(), 2);
        assert_eq!(
            output.output.scoring_population_digest,
            Some(Blake3Digest32::from_bytes(
                fixture.predicates.retrieval_digest.0
            ))
        );
        assert!(output.output.nominations[0].raw_score >= output.output.nominations[1].raw_score);
        assert_eq!(port.query_calls, 1);
        assert_eq!(port.count_calls, 1);
        assert_eq!(port.observed_scoped, Some(true));
    }

    #[test]
    fn wrong_scope_predicates_deny_before_backend() {
        let fixture = fixture();
        let other = fixture_with_membership(membership(0x22));
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(vec![], 0);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &other.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::PopulationMismatch)
        );
        assert_eq!(port.query_calls, 0);
        assert_eq!(port.count_calls, 0);
    }

    #[test]
    fn diverged_retrieval_idf_predicates_deny() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut diverged = fixture.predicates;
        diverged.idf_digest = search_access::EligibilityPlanDigest([0xFF; 32]);
        assert!(!diverged.is_consistent());
        let mut port = StubPort::ready(vec![], 0);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &diverged,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::PopulationMismatch)
        );
        assert_eq!(port.query_calls, 0);
    }

    #[test]
    fn global_idf_scope_denies() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(vec![], 0);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                false,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::PopulationMismatch)
        );
        assert_eq!(port.query_calls, 0);
    }

    #[test]
    fn grouped_multi_plan_leg_denies() {
        let mut fixture = fixture();
        let second =
            search_access::compile_base_eligibility(&binding(membership(0x22)), route(), 9, 2, 2)
                .expect("second plan");
        fixture
            .ticket
            .leg
            .safe_index_leg
            .as_mut()
            .expect("safe leg")
            .eligibility_plans
            .push(second);
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(vec![], 0);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::PopulationMismatch)
        );
        assert_eq!(port.query_calls, 0);
    }

    #[test]
    fn missing_pin_denies() {
        let fixture = fixture();
        let empty = LegPinSet {
            epoch_pins: Vec::new(),
        };
        let mut port = StubPort::ready(vec![], 0);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &empty,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::PinAcquisitionFailed)
        );
        assert_eq!(port.query_calls, 0);
    }

    #[test]
    fn revoked_membership_contaminates_whole_leg() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let live = LiveSecurityState {
            denied_memberships: BTreeSet::from([fixture.membership]),
            ..fixture.live_security.clone()
        };
        let mut port = StubPort::ready(vec![nomination(0x01, fixture.membership, 2.0)], 1);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &live,
                &mut port,
            ),
            Err(ExecuteError::ContaminatedLeg)
        );
    }

    #[test]
    fn purged_membership_contaminates_whole_leg() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let live = LiveSecurityState {
            purged_memberships: BTreeSet::from([fixture.membership]),
            ..fixture.live_security.clone()
        };
        let mut port = StubPort::ready(vec![], 0);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &live,
                &mut port,
            ),
            Err(ExecuteError::ContaminatedLeg)
        );
    }

    #[test]
    fn fail_closed_domain_contaminates_whole_leg() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let live = LiveSecurityState {
            fail_closed: true,
            ..fixture.live_security.clone()
        };
        let mut port = StubPort::ready(vec![], 0);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &live,
                &mut port,
            ),
            Err(ExecuteError::ContaminatedLeg)
        );
    }

    #[test]
    fn denied_population_never_scores() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let live = LiveSecurityState {
            generation: 10,
            denied_memberships: BTreeSet::from([fixture.membership]),
            purged_memberships: BTreeSet::new(),
            fail_closed: false,
            snapshot_digest: Blake3Digest32::from_bytes([0x5A; 32]),
        };
        let mut port = StubPort::ready(vec![nomination(0x01, fixture.membership, 9.0)], 1);
        let error = execute_indexed_leg(
            &fixture.ticket,
            &pin_set,
            &fixture.predicates,
            "lexical",
            &query(),
            8,
            true,
            &fixture.request_security,
            &fixture.live_security,
            &live,
            &mut port,
        )
        .expect_err("denied population must not score");
        assert_eq!(error, ExecuteError::ContaminatedLeg);
    }

    #[test]
    fn limit_ceiling_is_enforced() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(vec![], 0);
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                0,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::BudgetExceeded)
        );
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                9,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::BudgetExceeded)
        );
        assert_eq!(port.query_calls, 0);
    }

    #[test]
    fn oversized_backend_page_is_rejected() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(
            (0x01..=0x09)
                .map(|byte| nomination(byte, fixture.membership, 1.0))
                .collect(),
            9,
        );
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::InvalidNomination)
        );
    }

    #[test]
    fn topk_saturation_is_partial_never_complete() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(vec![nomination(0x01, fixture.membership, 2.0)], 2);
        let output = execute_indexed_leg(
            &fixture.ticket,
            &pin_set,
            &fixture.predicates,
            "lexical",
            &query(),
            8,
            true,
            &fixture.request_security,
            &fixture.live_security,
            &fixture.live_security,
            &mut port,
        )
        .expect("partial leg executes");
        assert_eq!(output.exact_denominator, 2);
        assert_eq!(
            output.output.completion,
            LegCompletion::PartialCandidateScope
        );
    }

    #[test]
    fn empty_population_completes_empty() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(Vec::new(), 0);
        let output = execute_indexed_leg(
            &fixture.ticket,
            &pin_set,
            &fixture.predicates,
            "lexical",
            &query(),
            8,
            true,
            &fixture.request_security,
            &fixture.live_security,
            &fixture.live_security,
            &mut port,
        )
        .expect("empty leg executes");
        assert_eq!(output.exact_denominator, 0);
        assert_eq!(
            output.output.completion,
            LegCompletion::CompleteCandidateScope
        );
        assert!(output.output.nominations.is_empty());
    }

    #[test]
    fn nominations_beyond_denominator_are_rejected() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(
            vec![
                nomination(0x01, fixture.membership, 2.0),
                nomination(0x02, fixture.membership, 1.0),
            ],
            1,
        );
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut port,
            ),
            Err(ExecuteError::PopulationMismatch)
        );
    }

    #[test]
    fn deterministic_tie_break_orders_by_candidate_id() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(
            vec![
                nomination(0x09, fixture.membership, 1.0),
                nomination(0x01, fixture.membership, 1.0),
            ],
            2,
        );
        let output = execute_indexed_leg(
            &fixture.ticket,
            &pin_set,
            &fixture.predicates,
            "lexical",
            &query(),
            8,
            true,
            &fixture.request_security,
            &fixture.live_security,
            &fixture.live_security,
            &mut port,
        )
        .expect("tied leg executes");
        let first = &output.output.nominations[0];
        let second = &output.output.nominations[1];
        assert!(first.candidate_id < second.candidate_id);
        assert_eq!(first.point_id, [0x01; 16]);
    }

    #[test]
    fn malformed_backend_shapes_are_rejected() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let foreign = membership(0x99);
        for (nominations, denominator) in [
            (
                vec![IndexedNomination {
                    raw_score: f32::NAN,
                    ..nomination(0x01, fixture.membership, 1.0)
                }],
                1,
            ),
            (
                vec![
                    nomination(0x01, fixture.membership, 1.0),
                    nomination(0x01, fixture.membership, 0.5),
                ],
                2,
            ),
            (vec![nomination(0x01, foreign, 1.0)], 1),
        ] {
            let mut port = StubPort::ready(nominations, denominator);
            assert_eq!(
                execute_indexed_leg(
                    &fixture.ticket,
                    &pin_set,
                    &fixture.predicates,
                    "lexical",
                    &query(),
                    8,
                    true,
                    &fixture.request_security,
                    &fixture.live_security,
                    &fixture.live_security,
                    &mut port,
                ),
                Err(ExecuteError::InvalidNomination)
            );
        }
    }

    #[test]
    fn unavailable_stays_distinct_from_failure() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut unavailable = StubPort {
            fail_query: Some(IndexedPortError::Unavailable),
            ..StubPort::ready(vec![], 0)
        };
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut unavailable,
            ),
            Err(ExecuteError::BackendUnavailable)
        );
        let mut failed = StubPort {
            fail_query: Some(IndexedPortError::Failure),
            ..StubPort::ready(vec![], 0)
        };
        assert_eq!(
            execute_indexed_leg(
                &fixture.ticket,
                &pin_set,
                &fixture.predicates,
                "lexical",
                &query(),
                8,
                true,
                &fixture.request_security,
                &fixture.live_security,
                &fixture.live_security,
                &mut failed,
            ),
            Err(ExecuteError::BackendFailure)
        );
    }

    #[test]
    fn malformed_query_shapes_are_rejected() {
        let fixture = fixture();
        let pin_set = pins(&fixture);
        let mut port = StubPort::ready(vec![], 0);
        for (name, vector) in [
            ("empty", "lexical"),
            ("blank", ""),
            ("badchars", "lex ical!"),
        ] {
            let _ = name;
            let query_vector: &[(u32, f32)] = if name == "empty" { &[] } else { &query() };
            assert_eq!(
                execute_indexed_leg(
                    &fixture.ticket,
                    &pin_set,
                    &fixture.predicates,
                    vector,
                    query_vector,
                    8,
                    true,
                    &fixture.request_security,
                    &fixture.live_security,
                    &fixture.live_security,
                    &mut port,
                ),
                Err(ExecuteError::InvalidNomination),
                "{name}"
            );
        }
        for bad_query in [
            vec![(3, 0.5), (0, 1.0)],
            vec![(0, f32::INFINITY)],
            vec![(1, 1.0), (1, 0.5)],
        ] {
            assert_eq!(
                execute_indexed_leg(
                    &fixture.ticket,
                    &pin_set,
                    &fixture.predicates,
                    "lexical",
                    &bad_query,
                    8,
                    true,
                    &fixture.request_security,
                    &fixture.live_security,
                    &fixture.live_security,
                    &mut port,
                ),
                Err(ExecuteError::InvalidNomination)
            );
        }
        assert_eq!(port.query_calls, 0);
    }

    #[test]
    fn port_error_codes_are_stable() {
        assert_eq!(
            IndexedPortError::Unavailable.code(),
            "INDEXED_PORT_UNAVAILABLE"
        );
        assert_eq!(IndexedPortError::Failure.code(), "INDEXED_PORT_FAILURE");
    }
}
