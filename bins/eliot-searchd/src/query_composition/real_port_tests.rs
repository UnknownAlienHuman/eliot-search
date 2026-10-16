//! Pre-dispatch contract and original-budget proofs; no server or qualification is fabricated.
#[path = "../../../../crates/search-query/search-query-planner/tests/support/canonical_plan.rs"]
mod canonical_plan_fixture;

use super::*;
use search_access::{AccessCheckpoint, AccessPermit, IndexedRouteFence, MembershipAccessBinding};
use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, LegKind, OwnerEpoch, RequestId};
use search_ports::{CancellationProbe, PackageOpaque};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

struct SharedProbe(Arc<AtomicBool>);

impl fmt::Debug for SharedProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SharedProbe([redacted])")
    }
}

impl PackageOpaque for SharedProbe {
    fn owner_package(&self) -> &'static str {
        "eliot-searchd-tests"
    }
}

impl CancellationProbe for SharedProbe {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

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
    let compiled_plan = canonical_plan_fixture::compile_lexical_plan(
        RequestId::from_bytes([5; 16]),
        leg,
        canonical_plan_fixture::lexical_budget(100, 8, 1024, 100, 1024 * 1024),
    );
    let ticket = ticket_for(&compiled_plan);
    (ticket, CollectionRoute { generation: route.collection_generation_id,
        physical_name: OpaqueId::new("fixture:qualified-route").unwrap() }, predicates)
}

fn ticket_for(plan: &search_query_planner::CompiledSearchPlan) -> LegTicket {
    let leg = plan.legs().iter().find(|leg| leg.leg_kind() == LegKind::Lexical)
        .expect("canonical lexical leg").clone();
    LegTicket { request_id: plan.request_id(), plan_digest: plan.digest(),
        owner_id: OpaqueId::new("fixture:owner").unwrap(), issued_at_tick: 0,
        access_permit: AccessPermit { checkpoint: AccessCheckpoint::BeforeLegDispatch,
            live_generation: 1, live_snapshot_digest: Blake3Digest32::from_bytes([7; 32]) },
        leg }
}

#[test]
fn real_port_rejects_widened_scope_contract_and_route_before_rpc() {
    let (ticket, route, predicates) = fixture();
    let expected = ticket.leg.safe_index_leg().unwrap().clone();
    validate_real_ticket(&expected, &route, &ticket, &predicates).unwrap();
    let filter = render_eligibility_filter(&expected.eligibility_plans[0], ticket.leg.memberships()).unwrap();
    assert_eq!(filter.allowed_source_memberships.len(), ticket.leg.memberships().len());

    let other_membership = SourceMembershipId::from_bytes([99; 16]);
    let other_plan = search_access::compile_base_eligibility(
        &MembershipAccessBinding { membership_id: other_membership,
            access_partition_digest: Blake3Digest32::from_bytes([3; 32]),
            scoring_partition_digest: Blake3Digest32::from_bytes([4; 32]),
            projection_membership_id: OpaqueId::new("fixture:projection:other").unwrap(), active: true },
        IndexedRouteFence { collection_generation_id: route.generation,
            visible_epoch: Epoch::new(1).unwrap(), route_generation: 1, owner_epoch: OwnerEpoch::new(1).unwrap() },
        1, 1, 1).unwrap();
    let mut wider = expected.clone();
    wider.memberships.insert(other_membership);
    wider.eligibility_plans.push(other_plan);
    let wider_plan = canonical_plan_fixture::compile_lexical_plan(
        RequestId::from_bytes([8; 16]), wider,
        canonical_plan_fixture::lexical_budget(100, 8, 1024, 100, 1024 * 1024));
    let wider_ticket = ticket_for(&wider_plan);
    assert!(validate_real_ticket(&expected, &route, &wider_ticket, &predicates).is_err());

    let changed_binding = MembershipAccessBinding { membership_id: expected.memberships.iter().next().copied().unwrap(),
        access_partition_digest: Blake3Digest32::from_bytes([3; 32]),
        scoring_partition_digest: Blake3Digest32::from_bytes([98; 32]),
        projection_membership_id: OpaqueId::new("fixture:projection").unwrap(), active: true };
    let changed_plan = search_access::compile_base_eligibility(
        &changed_binding, IndexedRouteFence { collection_generation_id: route.generation,
            visible_epoch: Epoch::new(1).unwrap(), route_generation: 1, owner_epoch: OwnerEpoch::new(1).unwrap() },
        1, 1, 1).unwrap();
    let changed_leg = search_access::SafeRetrievalLeg { leg_id: 0,
        memberships: BTreeSet::from([changed_binding.membership_id]),
        eligibility_plans: vec![changed_plan],
        route: IndexedRouteFence { collection_generation_id: route.generation,
            visible_epoch: Epoch::new(1).unwrap(), route_generation: 1, owner_epoch: OwnerEpoch::new(1).unwrap() },
        overlap_proof_digest: None };
    let changed_compiled = canonical_plan_fixture::compile_lexical_plan(
        RequestId::from_bytes([9; 16]), changed_leg,
        canonical_plan_fixture::lexical_budget(100, 8, 1024, 100, 1024 * 1024));
    let changed_ticket = ticket_for(&changed_compiled);
    assert!(validate_real_ticket(&expected, &route, &changed_ticket, &predicates).is_err());

    let mut diverged = predicates;
    diverged.idf_digest = search_access::EligibilityPlanDigest([97; 32]);
    assert!(validate_real_ticket(&expected, &route, &ticket, &diverged).is_err());
    let mut foreign = route.clone();
    foreign.generation = CollectionGenerationId::from_bytes([96; 16]);
    assert!(validate_real_ticket(&expected, &foreign, &ticket, &predicates).is_err());
}

#[test]
fn real_rpc_budget_shrinks_and_port_context_retains_live_probe() {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(1);
    let signal = Arc::new(AtomicBool::new(false));
    let probe: Arc<dyn CancellationProbe + Send + Sync> =
        Arc::new(SharedProbe(Arc::clone(&signal)));
    let first = real_rpc_context(deadline, start, &probe).unwrap();
    let cloned = first.clone();
    let next = real_rpc_context(deadline, start + Duration::from_millis(400), &probe).unwrap();
    assert_eq!(first.deadline(), Duration::from_secs(1));
    assert_eq!(next.deadline(), Duration::from_millis(600));
    assert!(real_rpc_context(deadline, deadline, &probe).is_err());
    assert_eq!(first.check(), Ok(()));
    signal.store(true, Ordering::SeqCst);
    assert_eq!(first.check(), Err(BridgeError::Cancelled));
    assert_eq!(cloned.check(), Err(BridgeError::Cancelled));
    assert!(real_rpc_context(deadline, start, &probe).is_err());
}

#[test]
fn real_rpc_context_refuses_a_pre_cancelled_probe_before_dispatch() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let probe: Arc<dyn CancellationProbe + Send + Sync> =
        Arc::new(SharedProbe(Arc::new(AtomicBool::new(true))));
    assert!(real_rpc_context(deadline, Instant::now(), &probe).is_err());
}
