//! Pre-dispatch contract and original-budget proofs; no server or qualification is fabricated.
use super::*;
use search_access::{AccessCheckpoint, AccessPermit, IndexedRouteFence, MembershipAccessBinding};
use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, LegKind, OwnerEpoch, RequestId};
use search_query_planner::{CancellationBoundary, CompiledPlanDigest, LegBudget, PlannedLeg};
use std::time::Duration;

fn fixture() -> (LegTicket, CollectionRoute, EligibilityPredicates) {
    let membership = SourceMembershipId::from_bytes([1; 16]);
    let route = IndexedRouteFence { collection_generation_id: CollectionGenerationId::from_bytes([2; 16]),
        visible_epoch: Epoch::new(1).unwrap(), route_generation: 1, owner_epoch: OwnerEpoch::new(1).unwrap() };
    let plan = search_access::compile_base_eligibility(&MembershipAccessBinding { membership_id: membership,
        access_partition_digest: Blake3Digest32::from_bytes([3; 32]),
        scoring_partition_digest: Blake3Digest32::from_bytes([4; 32]),
        projection_membership_id: OpaqueId::new("fixture:projection").unwrap(), active: true }, route, 1, 1, 1).unwrap();
    let predicates = plan.predicates();
    let leg = search_access::SafeRetrievalLeg { leg_id: 0, memberships: BTreeSet::from([membership]),
        eligibility_plans: vec![plan], route, overlap_proof_digest: None };
    let ticket = LegTicket { request_id: RequestId::from_bytes([5; 16]), plan_digest: CompiledPlanDigest([6; 32]),
        owner_id: OpaqueId::new("fixture:owner").unwrap(), issued_at_tick: 0,
        access_permit: AccessPermit { checkpoint: AccessCheckpoint::BeforeLegDispatch,
            live_generation: 1, live_snapshot_digest: Blake3Digest32::from_bytes([7; 32]) },
        leg: PlannedLeg { leg_id: 0, leg_kind: LegKind::Lexical, depends_on: Vec::new(),
            memberships: leg.memberships.clone(), safe_index_leg: Some(leg),
            budget: LegBudget { deadline_ms: 100, max_candidates: 8, max_source_read_bytes: 1024,
                max_cpu_ms: 100, max_memory_bytes: 1024 * 1024 }, cancellation_boundary: CancellationBoundary::BeforeDispatch } };
    (ticket, CollectionRoute { generation: route.collection_generation_id,
        physical_name: OpaqueId::new("fixture:qualified-route").unwrap() }, predicates)
}

#[test]
fn real_port_rejects_widened_scope_contract_and_route_before_rpc() {
    let (ticket, route, predicates) = fixture();
    let expected = ticket.leg.safe_index_leg.as_ref().unwrap();
    validate_real_ticket(expected, &route, &ticket, &predicates).unwrap();
    let filter = render_eligibility_filter(&expected.eligibility_plans[0], &ticket.leg.memberships).unwrap();
    assert_eq!(filter.allowed_source_memberships.len(), ticket.leg.memberships.len());
    let mut wider = ticket.clone();
    wider.leg.memberships.insert(SourceMembershipId::from_bytes([99; 16]));
    assert!(validate_real_ticket(expected, &route, &wider, &predicates).is_err());
    let mut changed = ticket.clone();
    changed.leg.safe_index_leg.as_mut().unwrap().eligibility_plans[0].scoring_partition_digest = Blake3Digest32::from_bytes([98; 32]);
    assert!(validate_real_ticket(expected, &route, &changed, &predicates).is_err());
    let mut diverged = predicates;
    diverged.idf_digest = search_access::EligibilityPlanDigest([97; 32]);
    assert!(validate_real_ticket(expected, &route, &ticket, &diverged).is_err());
    let mut foreign = route.clone();
    foreign.generation = CollectionGenerationId::from_bytes([96; 16]);
    assert!(validate_real_ticket(expected, &foreign, &ticket, &predicates).is_err());
}

#[test]
fn real_rpc_budget_shrinks_and_keeps_original_cancel_flag() {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(1);
    let flag = Arc::new(AtomicBool::new(false));
    let first = real_rpc_context(deadline, start, &flag).unwrap();
    let next = real_rpc_context(deadline, start + Duration::from_millis(400), &flag).unwrap();
    assert_eq!(first.deadline(), Duration::from_secs(1));
    assert_eq!(next.deadline(), Duration::from_millis(600));
    assert!(real_rpc_context(deadline, deadline, &flag).is_err());
    flag.store(true, Ordering::SeqCst);
    assert_eq!(first.check(), Err(BridgeError::Cancelled));
    assert!(real_rpc_context(deadline, start, &flag).is_err());
}
