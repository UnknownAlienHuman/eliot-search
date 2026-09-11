//! T28 indexed-retrieval process test: single pinned route/epoch retrieval,
//! source validation and result projection.
//!
//! This harness drives the real composition units, not the served daemon:
//! the [`ProcessTestIndexedPort`] over the synchronous in-memory Qdrant
//! oracle (explicitly not live-server proof), the accepted
//! `search-retrieval-executor::indexed` kernel, `search-candidate-validator`
//! with an explicit revision-readback double, and
//! `search-result-projector`. Every test builds its state through
//! [`Chain`] and advances live fences only by constructing a new
//! [`LiveSecurityState`]; no hidden mutation exists.
//!
//! The Qdrant double below is a process-test double only: it exercises the
//! single-contract query/readback/count shape against the behavioral oracle.
//! It never manufactures a live receipt, spawns no process and performs no
//! broad-filter mutation. Live scoring/IDF/count parity stays a T24
//! acceptance obligation and is not asserted here.

#![cfg(feature = "wave4-query")]
#![forbid(unsafe_code)]

#[path = "../src/query_composition.rs"]
mod query_composition;

use std::collections::{BTreeMap, BTreeSet};

use query_composition::{
    ProcessTestIndexedPort, execute_pinned_leg, membership_opaque_id, render_eligibility_filter,
};
use search_access::{
    AccessCheckpoint, AccessPermit, BaseEligibilityPlan, IndexedRouteFence, LiveSecurityState,
    MembershipAccessBinding, RequestSecurityFence,
};
use search_candidate_validator::{
    CandidateAssurance, CandidateNomination, RevisionReadbackPort, SourceReadback,
    ValidationContext, ValidationOutcome,
};
use search_contracts::{
    AccessPolicyRevision, AssuranceClass, Blake3Digest32, BoundedList,
    BoundedNonContentRankingTrace, BoundedSet, CandidateId, CatalogRevision,
    CollectionGenerationId, CollectionRouteRevision, EntityKind, Epoch, EvidenceRole,
    ExactOrEntityBoost, FusionProfileId, HandleClass, HandleId, InstallationIncarnationId, LegKind,
    LineageDiversityAction, MembershipRevision, NonZeroRevision, ObservationCursorRevision,
    ObservationFreshness, ObservationFreshnessState, OpaqueHandleToken, OpaqueId, OverlayRevision,
    OwnerEpoch, PlanFingerprint, PlanId, ProfileId, PurgeFenceRevision, QuerySnapshotFence,
    QuerySnapshotFingerprint, ReceiptRef, RepresentationId, RequestId, SearchSourceHandle,
    ShadowFenceRevision, SourceMembershipId, SourceRevisionId, SourceView, UnitId,
};
use search_epoch_pins::{PinLimits, PinRegistry, RouteIdentity};
use search_qdrant_bridge::{
    AuthLeaseEvidence, BridgeEndpoint, BridgeLimits, BridgeMutation, CapabilityProbeResults,
    CollectionRoute, CollectionSchema, ConsistencyGates, EligibilityFilter, FilterGates,
    IndexGates, PointPayload, PointRecord, QdrantBridge, QdrantPointId, StrictnessFloors,
    SupervisorReceipt, TopologyGates, VectorSchema, probe_capabilities,
};
use search_query_planner::{CancellationBoundary, CompiledPlanDigest, LegBudget, PlannedLeg};
use search_result_projector::{
    CandidateProjectionInput, CandidatePublicMetadata, CandidateSetEnvelope, ProjectionBudget,
    project_candidate_set,
};
use search_retrieval_executor::{
    ExecuteError, LegCompletion, LegPinSet, LegTicket, acquire_leg_pins,
};

const VECTOR_NAME: &str = "lexical";
const VECTOR_DIMS: u32 = 8;

const fn membership(byte: u8) -> SourceMembershipId {
    SourceMembershipId::from_bytes([byte; 16])
}

const fn revision(byte: u8) -> SourceRevisionId {
    SourceRevisionId::from_bytes([byte; 16])
}

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn fence() -> IndexedRouteFence {
    IndexedRouteFence {
        collection_generation_id: CollectionGenerationId::from_bytes([0x31; 16]),
        visible_epoch: Epoch::new(7).expect("fixture epoch"),
        route_generation: 3,
        owner_epoch: OwnerEpoch::new(1).expect("fixture owner epoch"),
    }
}

fn plan_for(membership_id: SourceMembershipId) -> BaseEligibilityPlan {
    let binding = MembershipAccessBinding {
        membership_id,
        access_partition_digest: digest(0xA1),
        scoring_partition_digest: digest(0xB2),
        projection_membership_id: OpaqueId::new("projection:t28").expect("fixture projection"),
        active: true,
    };
    search_access::compile_base_eligibility(&binding, fence(), 9, 2, 2).expect("fixture plan")
}

const fn live_clean() -> LiveSecurityState {
    LiveSecurityState {
        generation: 9,
        denied_memberships: BTreeSet::new(),
        purged_memberships: BTreeSet::new(),
        fail_closed: false,
        snapshot_digest: digest(0x5A),
    }
}

fn request_fence(membership_id: SourceMembershipId) -> RequestSecurityFence {
    RequestSecurityFence {
        planned_generation: 9,
        memberships: BTreeSet::from([membership_id]),
    }
}

fn ticket_for(membership_id: SourceMembershipId, plan: &BaseEligibilityPlan) -> LegTicket {
    LegTicket {
        request_id: RequestId::from_bytes([0x21; 16]),
        plan_digest: CompiledPlanDigest([0x33; 32]),
        owner_id: OpaqueId::new("owner:t28").expect("fixture owner"),
        leg: PlannedLeg {
            leg_id: 0,
            leg_kind: LegKind::Lexical,
            depends_on: Vec::new(),
            memberships: BTreeSet::from([membership_id]),
            safe_index_leg: Some(search_access::SafeRetrievalLeg {
                leg_id: 0,
                memberships: BTreeSet::from([membership_id]),
                eligibility_plans: vec![plan.clone()],
                route: fence(),
                overlap_proof_digest: None,
            }),
            budget: LegBudget {
                deadline_ms: 5_000,
                max_candidates: 8,
                max_source_read_bytes: 1_048_576,
                max_cpu_ms: 2_000,
                max_memory_bytes: 67_108_864,
            },
            cancellation_boundary: CancellationBoundary::BeforeDispatch,
        },
        access_permit: AccessPermit {
            checkpoint: AccessCheckpoint::BeforeLegDispatch,
            live_generation: 9,
            live_snapshot_digest: digest(0x5A),
        },
        issued_at_tick: 100,
    }
}

fn registry_for() -> PinRegistry {
    let route = fence();
    PinRegistry::new(
        RouteIdentity {
            collection_generation_id: route.collection_generation_id,
            route_revision: CollectionRouteRevision::new(route.route_generation),
        },
        route.visible_epoch,
        PinLimits::BASELINE,
    )
    .expect("fixture registry")
}

fn test_bridge() -> QdrantBridge {
    let supervisor = SupervisorReceipt {
        owner_epoch: OwnerEpoch::new(1).expect("fixture owner epoch"),
        process_identity_digest: digest(0xB1),
        artifact_digest: digest(0xA2),
        endpoint_digest: digest(0xE3),
    };
    let endpoint = BridgeEndpoint {
        endpoint_digest: digest(0xE3),
        loopback_only: true,
    };
    let auth = AuthLeaseEvidence {
        reference_digest: digest(0x11),
        purpose_digest: digest(0x22),
        valid: true,
    };
    let capability = probe_capabilities(
        supervisor,
        digest(0xC4),
        CapabilityProbeResults {
            topology: TopologyGates {
                authenticated_health: true,
                single_shard: true,
                signed_i64_ranges: true,
            },
            filters: FilterGates {
                missing_upper_bound_must_not: true,
                sparse_idf: true,
                independent_idf_corpus: true,
            },
            indexes: IndexGates {
                strict_mode: true,
                payload_indexes: true,
                wait_for_mutations: true,
            },
            consistency: ConsistencyGates {
                strong_ordering: true,
                exact_count_and_readback: true,
                named_sparse_vectors: true,
            },
        },
    )
    .expect("fixture capability");
    QdrantBridge::connect(
        endpoint,
        auth,
        supervisor,
        capability,
        BridgeLimits::BASELINE,
    )
    .expect("fixture bridge")
}

fn collection_route() -> CollectionRoute {
    CollectionRoute {
        generation: CollectionGenerationId::from_bytes([0x31; 16]),
        physical_name: OpaqueId::new("t28e2e").expect("fixture collection"),
    }
}

fn test_schema() -> CollectionSchema {
    CollectionSchema {
        named_vectors: BTreeMap::from([(
            VECTOR_NAME.to_owned(),
            VectorSchema {
                dimensions: VECTOR_DIMS,
                sparse: true,
                idf_enabled: true,
            },
        )]),
        indexed_payload_fields: EligibilityFilter::INDEXED_FIELDS
            .iter()
            .map(|field| (*field).to_owned())
            .collect(),
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: digest(0xCC),
    }
}

fn point(
    point_byte: u8,
    membership_id: SourceMembershipId,
    valid_from: i64,
    valid_until: Option<i64>,
    score_values: Vec<(u32, f32)>,
    payload_byte: u8,
    identity_byte: u8,
) -> PointRecord {
    PointRecord {
        point_id: QdrantPointId([point_byte; 16]),
        payload: PointPayload {
            source_membership_id: membership_opaque_id(membership_id).expect("fixture member"),
            projection_membership_id: OpaqueId::new("projection:t28").expect("fixture projection"),
            access_partition_digest: digest(0xA1),
            source_revision: 1,
            unit_ordinal: 0,
            valid_from_epoch: Epoch::new(valid_from).expect("fixture from epoch"),
            valid_until_epoch_exclusive: valid_until
                .map(|until| Epoch::new(until).expect("fixture until epoch")),
            payload_digest: digest(payload_byte),
            identity_digest: digest(identity_byte),
        },
        vectors: BTreeMap::from([(
            VECTOR_NAME.to_owned(),
            search_qdrant_bridge::StoredVector {
                dimensions: VECTOR_DIMS,
                sparse: true,
                values: score_values,
                digest: digest(0xD0),
            },
        )]),
    }
}

/// Bounded revision fixture mirroring one indexed point.
struct RevisionFixture {
    profile_id: ProfileId,
    profile_digest: Blake3Digest32,
    revision_id: SourceRevisionId,
    representation_id: RepresentationId,
    unit_id: UnitId,
    residency_digest: Blake3Digest32,
    bytes: Vec<u8>,
}

fn revision_fixture() -> RevisionFixture {
    RevisionFixture {
        profile_id: ProfileId::new("t28e2e").expect("fixture profile"),
        profile_digest: digest(0xC4),
        revision_id: revision(0x60),
        representation_id: RepresentationId::from_bytes([0x61; 16]),
        unit_id: UnitId::from_bytes([0x62; 16]),
        residency_digest: digest(0xC3),
        bytes: b"0123456789".to_vec(),
    }
}

struct FixedReadback {
    readback: Option<SourceReadback>,
}

impl RevisionReadbackPort for FixedReadback {
    type Error = ();

    fn read_exact(
        &mut self,
        _request: &search_candidate_validator::ExactRevisionReadbackRequest,
    ) -> Result<SourceReadback, Self::Error> {
        self.readback.clone().ok_or(())
    }
}

fn readback_for(
    membership_id: SourceMembershipId,
    revision: &RevisionFixture,
    content: Blake3Digest32,
    unit: Blake3Digest32,
    excerpt: Blake3Digest32,
) -> SourceReadback {
    SourceReadback {
        source_membership_id: membership_id,
        source_revision_id: revision.revision_id,
        representation_id: revision.representation_id,
        unit_id: revision.unit_id,
        content_digest: content,
        unit_digest: unit,
        excerpt_digest: excerpt,
        profile_id: revision.profile_id.clone(),
        profile_digest: revision.profile_digest,
        residency_digest: revision.residency_digest,
        assurance: CandidateAssurance::Exact,
        source_byte_start: 0,
        source_byte_end: revision.bytes.len() as u64,
        coordinate_map_exact: true,
        residency_authorized: true,
        bytes: revision.bytes.clone(),
    }
}

/// Full retrieval chain state: bridge, ticket, pins and fences.
struct Chain {
    bridge: QdrantBridge,
    route: CollectionRoute,
    ticket: LegTicket,
    pins: LegPinSet,
    plan: BaseEligibilityPlan,
    membership: SourceMembershipId,
    request_security: RequestSecurityFence,
    live: LiveSecurityState,
}

impl Chain {
    fn seeded(points: Vec<PointRecord>) -> Self {
        let membership = membership(0x11);
        let plan = plan_for(membership);
        let ticket = ticket_for(membership, &plan);
        let registry = registry_for();
        let pins = acquire_leg_pins(&ticket, &registry, 1_000).expect("fixture pins");
        let mut bridge = test_bridge();
        let route = collection_route();
        bridge
            .create_candidate_collection(route.clone(), test_schema())
            .expect("fixture collection");
        if !points.is_empty() {
            bridge
                .upsert_exact(
                    &route,
                    points,
                    BridgeMutation {
                        operation_id: OpaqueId::new("op:t28e2e:1").expect("fixture op"),
                        canonical_input_digest: digest(0xDD),
                    },
                )
                .expect("fixture upsert");
        }
        Self {
            bridge,
            route,
            ticket,
            pins,
            plan,
            membership,
            request_security: request_fence(membership),
            live: live_clean(),
        }
    }

    fn port(&self) -> ProcessTestIndexedPort<'_> {
        let filter = render_eligibility_filter(&self.plan, &BTreeSet::from([self.membership]))
            .expect("fixture filter");
        let allowed = BTreeMap::from([(
            membership_opaque_id(self.membership).expect("fixture member"),
            self.membership,
        )]);
        ProcessTestIndexedPort::new(&self.bridge, self.route.clone(), filter, allowed)
    }

    fn query_vector() -> Vec<(u32, f32)> {
        vec![(0, 1.0), (3, 0.5)]
    }
}

fn standard_points(member: SourceMembershipId) -> Vec<PointRecord> {
    vec![
        point(0x01, member, 7, None, vec![(0, 1.0), (3, 0.5)], 0x70, 0x71),
        point(0x02, member, 7, None, vec![(0, 0.5)], 0x72, 0x73),
        point(0x03, member, 8, None, vec![(0, 2.0)], 0x74, 0x75),
    ]
}

fn validator_nomination(
    chain: &Chain,
    revision: &RevisionFixture,
    raw: &search_retrieval_executor::RawNomination,
    ordinal: usize,
) -> (CandidateNomination, ValidationContext) {
    let nomination = CandidateNomination {
        request_id: chain.ticket.request_id,
        plan_digest: chain.ticket.plan_digest.0,
        leg_id: 0,
        point_id: raw.point_id,
        source_membership_id: raw.source_membership_id,
        collection_generation_id: CollectionGenerationId::from_bytes([0x31; 16]),
        valid_from_epoch: Epoch::new(7).expect("fixture epoch"),
        valid_until_epoch_exclusive: None,
        profile_id: revision.profile_id.clone(),
        profile_digest: revision.profile_digest,
        source_revision_id: revision.revision_id,
        representation_id: revision.representation_id,
        unit_id: revision.unit_id,
        unit_byte_start: 0,
        unit_byte_end: revision.bytes.len() as u64,
        expected_content_digest: raw.payload_digest,
        expected_unit_digest: digest(0xC1),
        expected_excerpt_digest: digest(0xC2),
        expected_assurance: CandidateAssurance::Exact,
        expected_residency_digest: revision.residency_digest,
        raw_score: raw.raw_score,
    };
    let context = ValidationContext {
        request_id: chain.ticket.request_id,
        plan_digest: chain.ticket.plan_digest.0,
        collection_generation_id: CollectionGenerationId::from_bytes([0x31; 16]),
        visible_epoch: Epoch::new(7).expect("fixture epoch"),
        allowed_memberships: BTreeSet::from([chain.membership]),
        shadowed_units: BTreeSet::new(),
        max_candidates: 8,
        candidate_ordinal: ordinal,
        request_security: chain.request_security.clone(),
        live_security: chain.live.clone(),
        contaminated_legs: BTreeSet::new(),
    };
    (nomination, context)
}

fn validated_candidates(
    chain: &Chain,
    raws: &[search_retrieval_executor::RawNomination],
) -> Vec<search_candidate_validator::ValidatedSearchCandidate> {
    let revision = revision_fixture();
    raws.iter()
        .enumerate()
        .map(|(ordinal, raw)| {
            let (nomination, context) = validator_nomination(chain, &revision, raw, ordinal);
            let mut port = FixedReadback {
                readback: Some(readback_for(
                    chain.membership,
                    &revision,
                    raw.payload_digest,
                    digest(0xC1),
                    digest(0xC2),
                )),
            };
            match search_candidate_validator::validate(
                &nomination,
                &context,
                1_048_576,
                &mut port,
                &chain.live,
            ) {
                ValidationOutcome::Validated(candidate) => candidate,
                other => panic!("expected validated candidate, got {other:?}"),
            }
        })
        .collect()
}

fn source_handle(seed: u8) -> SearchSourceHandle {
    SearchSourceHandle {
        handle_id: HandleId::from_bytes([seed; 16]),
        handle_revision: NonZeroRevision::new(1).expect("fixture revision"),
        handle_class: HandleClass::DurableSource,
        expires_at: None,
        opaque_token: OpaqueHandleToken::new(&[0xA5; 32]).expect("fixture token"),
    }
}

const fn freshness() -> ObservationFreshness {
    ObservationFreshness {
        state: ObservationFreshnessState::CurrentConfirmed,
        observation_cursor_revision: ObservationCursorRevision::new(1),
        observed_age_ms: None,
    }
}

fn snapshot_fence() -> QuerySnapshotFence {
    QuerySnapshotFence {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([0x44; 16]),
        collection_generation_id: Some(CollectionGenerationId::from_bytes([0x31; 16])),
        visible_epoch: Some(Epoch::new(7).expect("fixture epoch")),
        collection_route_revision: CollectionRouteRevision::new(3),
        catalog_revision: CatalogRevision::new(5),
        membership_revision: MembershipRevision::new(6),
        reference_portfolio_revision: None,
        access_policy_revision: AccessPolicyRevision::new(3),
        shadow_fence_revision: ShadowFenceRevision::new(2),
        purge_fence_revision: PurgeFenceRevision::new(2),
        overlay_revision: OverlayRevision::new(4),
        observation_cursor_revision: ObservationCursorRevision::new(1),
        observation_freshness: freshness(),
        source_view: SourceView::RetainedRevision(revision(0x60)),
        workspace_view_revision_ref: None,
        lexical_profile_ids: BoundedList::new(Vec::new()).expect("fixture profiles"),
        snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([0xF0; 32]),
    }
}

fn envelope(nominated: u32, validated: u32, denominator_complete: bool) -> CandidateSetEnvelope {
    use search_contracts::{
        Coverage, EmissionSecurityFence, LegDescriptor, LegExecutionState, LegExecutionSummary,
    };
    use search_contracts::{CoverageDenominatorKind, UtcTimestamp};
    let member = membership(0x11);
    CandidateSetEnvelope {
        request_id: RequestId::from_bytes([0x21; 16]),
        plan_id: PlanId::from_bytes([0x22; 16]),
        plan_fingerprint: PlanFingerprint::from_bytes([0x23; 32]),
        result_fence: search_contracts::ResultFence {
            planned_snapshot: snapshot_fence(),
            emission_source_owner_fences: BoundedList::new(Vec::new()).expect("fixture fences"),
            emission_security_fence: EmissionSecurityFence {
                access_policy_revision: AccessPolicyRevision::new(3),
                live_deny_generation: 9,
                shadow_fence_revision: ShadowFenceRevision::new(2),
                purge_fence_revision: PurgeFenceRevision::new(2),
                checked_at: UtcTimestamp::parse("2026-09-11T00:00:00.000000Z")
                    .expect("fixture time"),
                receipt_ref: ReceiptRef::new("receipt:emission").expect("fixture receipt"),
            },
            result_fingerprint: digest(0xBF),
        },
        coverage: Coverage {
            requested_legs: BoundedList::new(vec![LegDescriptor {
                leg_ref: OpaqueId::new("leg:0").expect("fixture leg"),
                leg_kind: LegKind::Lexical,
                scoring_partition_ref: None,
                profile_id: ProfileId::new("t28e2e").expect("fixture profile"),
            }])
            .expect("fixture requested"),
            executed_legs: BoundedList::new(vec![LegExecutionSummary {
                leg_ref: OpaqueId::new("leg:0").expect("fixture leg"),
                state: if denominator_complete {
                    LegExecutionState::Completed
                } else {
                    LegExecutionState::Partial
                },
                nominated_count: nominated,
                validated_count: validated,
                reason_codes: BoundedSet::empty(),
                receipt_ref: ReceiptRef::new("receipt:leg").expect("fixture receipt"),
            }])
            .expect("fixture executed"),
            represented_memberships: BoundedSet::from_items([member]).expect("fixture members"),
            represented_source_lineages: BoundedSet::empty(),
            omitted_or_failed_legs: BoundedList::new(Vec::new()).expect("fixture omitted"),
            candidate_validation_gaps: BoundedList::new(Vec::new()).expect("fixture gaps"),
            observation_freshness: freshness(),
            unknowns: BoundedList::new(Vec::new()).expect("fixture unknowns"),
            denominator_kind: if denominator_complete {
                CoverageDenominatorKind::CompleteScope
            } else {
                CoverageDenominatorKind::CandidateScope
            },
        },
        continuation_handle: None,
        result_validation_receipt_ref: ReceiptRef::new("receipt:result").expect("fixture receipt"),
    }
}

fn projection_inputs(
    candidates: &[search_candidate_validator::ValidatedSearchCandidate],
) -> Vec<CandidateProjectionInput> {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let seed = u8::try_from(index).expect("fixture index fits in one byte");
            CandidateProjectionInput {
                source_backed: candidate.clone(),
                public: CandidatePublicMetadata {
                    candidate_id: CandidateId::from_bytes([0xB0 + seed; 16]),
                    source_handle: source_handle(0xC0 + seed),
                    evidence_role: EvidenceRole::Definition,
                    entity_kind: Some(EntityKind::Function),
                    assurance: AssuranceClass::ExactBytes,
                    freshness: ObservationFreshnessState::CurrentConfirmed,
                    ranking_trace: BoundedNonContentRankingTrace {
                        fusion_profile_id: FusionProfileId::new("t28e2e-rrf")
                            .expect("fixture fusion"),
                        fused_rank: u32::try_from(index + 1).expect("fixture rank fits in u32"),
                        exact_or_entity_boost: ExactOrEntityBoost::None,
                        evidence_role_priority: 1,
                        portfolio_priority: 1,
                        lineage_diversity_action: LineageDiversityAction::Retained,
                        deterministic_tie_break_digest: digest(0x71),
                    },
                    reason_codes: BTreeSet::new(),
                    candidate_validation_receipt_ref: ReceiptRef::new("receipt:candidate")
                        .expect("fixture receipt"),
                },
            }
        })
        .collect()
}

const fn projection_budget(max_candidates: usize) -> ProjectionBudget {
    ProjectionBudget {
        max_candidates,
        max_total_validated_source_bytes: 1_024,
        max_source_bytes_per_candidate: 512,
    }
}

#[test]
fn indexed_chain_validates_and_projects() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let mut port = chain.port();
    assert_eq!(port.filter().allowed_source_memberships.len(), 1);
    assert_eq!(
        port.filter().access_partition_digest,
        chain.plan.access_partition_digest
    );
    assert_eq!(port.filter().visible_epoch, chain.plan.visible_epoch);
    let output = execute_pinned_leg(
        &chain.ticket,
        &chain.pins,
        &chain
            .ticket
            .leg
            .safe_index_leg
            .as_ref()
            .expect("safe leg")
            .eligibility_plans[0]
            .predicates(),
        VECTOR_NAME,
        &Chain::query_vector(),
        8,
        &chain.request_security,
        &chain.live,
        &chain.live,
        &mut port,
    )
    .expect("indexed leg executes");
    assert_eq!(output.exact_denominator, 2);
    assert_eq!(
        output.output.completion,
        LegCompletion::CompleteCandidateScope
    );
    assert_eq!(output.output.nominations.len(), 2);
    assert!(output.output.nominations[0].raw_score > output.output.nominations[1].raw_score);

    let validated = validated_candidates(&chain, &output.output.nominations);
    assert_eq!(validated.len(), 2);
    for candidate in &validated {
        assert_eq!(
            candidate.emission_permit.access_permit.checkpoint,
            AccessCheckpoint::BeforeResultEmission
        );
    }

    let projected = project_candidate_set(
        envelope(2, 2, true),
        projection_inputs(&validated),
        projection_budget(8),
    )
    .expect("projection succeeds");
    assert_eq!(projected.result.candidates.len(), 2);
    assert!(projected.omissions.is_empty());
    assert_eq!(
        projected.result.candidates.as_slice()[0].source_handle,
        source_handle(0xC0)
    );
    assert_eq!(
        projected.result.candidates.as_slice()[0].assurance,
        AssuranceClass::ExactBytes
    );
}

#[test]
fn wrong_epoch_route_is_unavailable_not_direct() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let wrong_route = CollectionRoute {
        generation: CollectionGenerationId::from_bytes([0x99; 16]),
        physical_name: OpaqueId::new("t28e2e").expect("fixture collection"),
    };
    let filter =
        render_eligibility_filter(&chain.plan, &BTreeSet::from([member])).expect("fixture filter");
    let allowed = BTreeMap::from([(
        membership_opaque_id(member).expect("fixture member"),
        member,
    )]);
    let mut port = ProcessTestIndexedPort::new(&chain.bridge, wrong_route, filter, allowed);
    let predicates = chain.plan.predicates();
    let error = search_retrieval_executor::indexed::execute_indexed_leg(
        &chain.ticket,
        &chain.pins,
        &predicates,
        VECTOR_NAME,
        &Chain::query_vector(),
        8,
        true,
        &chain.request_security,
        &chain.live,
        &chain.live,
        &mut port,
    )
    .expect_err("wrong generation must not serve");
    assert_eq!(error, ExecuteError::BackendUnavailable);
    assert_eq!(error.code(), "EXECUTE_BACKEND_UNAVAILABLE");
}

#[test]
fn concurrent_revoke_contaminates_leg() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let mut port = chain.port();
    let predicates = chain.plan.predicates();
    let revoked = LiveSecurityState {
        generation: 10,
        denied_memberships: BTreeSet::from([member]),
        purged_memberships: BTreeSet::new(),
        fail_closed: false,
        snapshot_digest: digest(0x5A),
    };
    let error = search_retrieval_executor::indexed::execute_indexed_leg(
        &chain.ticket,
        &chain.pins,
        &predicates,
        VECTOR_NAME,
        &Chain::query_vector(),
        8,
        true,
        &chain.request_security,
        &chain.live,
        &revoked,
        &mut port,
    )
    .expect_err("revoked leg must contaminate");
    assert_eq!(error, ExecuteError::ContaminatedLeg);
}

#[test]
fn missing_revision_is_gap_not_success() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let mut port = chain.port();
    let predicates = chain.plan.predicates();
    let output = search_retrieval_executor::indexed::execute_indexed_leg(
        &chain.ticket,
        &chain.pins,
        &predicates,
        VECTOR_NAME,
        &Chain::query_vector(),
        8,
        true,
        &chain.request_security,
        &chain.live,
        &chain.live,
        &mut port,
    )
    .expect("indexed leg executes");
    let revision = revision_fixture();
    let (nomination, context) =
        validator_nomination(&chain, &revision, &output.output.nominations[0], 0);
    let mut readback = FixedReadback { readback: None };
    match search_candidate_validator::validate(
        &nomination,
        &context,
        1_048_576,
        &mut readback,
        &chain.live,
    ) {
        ValidationOutcome::Gap(gap) => assert_eq!(
            gap.reason,
            search_candidate_validator::ValidationError::SourceUnreadable
        ),
        other => panic!("missing revision must gap, got {other:?}"),
    }
}

#[test]
fn stale_point_is_gap() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let revision = revision_fixture();
    let raw = &search_retrieval_executor::RawNomination {
        candidate_id: OpaqueId::new("candidate:0:stale").expect("fixture candidate"),
        point_id: [0x41; 16],
        source_membership_id: member,
        identity_digest: digest(0x71),
        payload_digest: digest(0x70),
        raw_score: 1.0,
        scoring_population_digest: digest(0x01),
    };
    let (mut nomination, context) = validator_nomination(&chain, &revision, raw, 0);
    nomination.valid_until_epoch_exclusive = Some(Epoch::new(7).expect("fixture epoch"));
    match search_candidate_validator::precheck(&nomination, &context) {
        search_candidate_validator::ValidationPrecheck::Gap(reason) => assert_eq!(
            reason,
            search_candidate_validator::ValidationError::EpochInvalid
        ),
        other => panic!("stale point must gap, got {other:?}"),
    }
}

#[test]
fn denied_membership_nomination_is_gap() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let revision = revision_fixture();
    let foreign = membership(0x99);
    let raw = search_retrieval_executor::RawNomination {
        candidate_id: OpaqueId::new("candidate:0:foreign").expect("fixture candidate"),
        point_id: [0x42; 16],
        source_membership_id: foreign,
        identity_digest: digest(0x71),
        payload_digest: digest(0x70),
        raw_score: 1.0,
        scoring_population_digest: digest(0x01),
    };
    let (nomination, context) = validator_nomination(&chain, &revision, &raw, 0);
    match search_candidate_validator::precheck(&nomination, &context) {
        search_candidate_validator::ValidationPrecheck::Gap(reason) => assert_eq!(
            reason,
            search_candidate_validator::ValidationError::MembershipDenied
        ),
        other => panic!("foreign membership must gap, got {other:?}"),
    }
}

#[test]
fn empty_population_completes_empty_and_projects_empty() {
    let chain = Chain::seeded(Vec::new());
    let mut port = chain.port();
    let predicates = chain.plan.predicates();
    let output = search_retrieval_executor::indexed::execute_indexed_leg(
        &chain.ticket,
        &chain.pins,
        &predicates,
        VECTOR_NAME,
        &Chain::query_vector(),
        8,
        true,
        &chain.request_security,
        &chain.live,
        &chain.live,
        &mut port,
    )
    .expect("empty leg executes");
    assert_eq!(output.exact_denominator, 0);
    assert_eq!(
        output.output.completion,
        LegCompletion::CompleteCandidateScope
    );
    let projected = project_candidate_set(envelope(0, 0, true), Vec::new(), projection_budget(8))
        .expect("empty projection succeeds");
    assert!(projected.result.candidates.is_empty());
    assert!(projected.omissions.is_empty());
}

#[test]
fn partial_topk_never_claims_complete() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let mut port = chain.port();
    let predicates = chain.plan.predicates();
    let output = search_retrieval_executor::indexed::execute_indexed_leg(
        &chain.ticket,
        &chain.pins,
        &predicates,
        VECTOR_NAME,
        &Chain::query_vector(),
        1,
        true,
        &chain.request_security,
        &chain.live,
        &chain.live,
        &mut port,
    )
    .expect("partial leg executes");
    assert_eq!(output.exact_denominator, 2);
    assert_eq!(output.output.nominations.len(), 1);
    assert_eq!(
        output.output.completion,
        LegCompletion::PartialCandidateScope
    );
}

#[test]
fn final_live_revalidation_denies_revoked() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let mut port = chain.port();
    let predicates = chain.plan.predicates();
    let output = search_retrieval_executor::indexed::execute_indexed_leg(
        &chain.ticket,
        &chain.pins,
        &predicates,
        VECTOR_NAME,
        &Chain::query_vector(),
        8,
        true,
        &chain.request_security,
        &chain.live,
        &chain.live,
        &mut port,
    )
    .expect("indexed leg executes");
    let revision = revision_fixture();
    let (nomination, context) =
        validator_nomination(&chain, &revision, &output.output.nominations[0], 0);
    let revoked = LiveSecurityState {
        generation: 10,
        denied_memberships: BTreeSet::from([member]),
        purged_memberships: BTreeSet::new(),
        fail_closed: false,
        snapshot_digest: digest(0x5A),
    };
    let mut readback = FixedReadback {
        readback: Some(readback_for(
            member,
            &revision,
            output.output.nominations[0].payload_digest,
            digest(0xC1),
            digest(0xC2),
        )),
    };
    match search_candidate_validator::validate(
        &nomination,
        &context,
        1_048_576,
        &mut readback,
        &revoked,
    ) {
        ValidationOutcome::Gap(gap) => assert_eq!(
            gap.reason,
            search_candidate_validator::ValidationError::AccessRevoked
        ),
        other => panic!("revoked emission must gap, got {other:?}"),
    }
}

#[test]
fn projection_budget_ceiling_omits_explicitly() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let mut port = chain.port();
    let predicates = chain.plan.predicates();
    let output = search_retrieval_executor::indexed::execute_indexed_leg(
        &chain.ticket,
        &chain.pins,
        &predicates,
        VECTOR_NAME,
        &Chain::query_vector(),
        8,
        true,
        &chain.request_security,
        &chain.live,
        &chain.live,
        &mut port,
    )
    .expect("indexed leg executes");
    let validated = validated_candidates(&chain, &output.output.nominations);
    let projected = project_candidate_set(
        envelope(2, 2, true),
        projection_inputs(&validated),
        projection_budget(1),
    )
    .expect("capped projection succeeds");
    assert_eq!(projected.result.candidates.len(), 1);
    assert_eq!(projected.omissions.len(), 1);
    assert_eq!(
        projected.omissions[0].reason,
        search_result_projector::ProjectionError::CandidateBudgetExceeded
    );
}

#[test]
fn composition_error_codes_are_stable() {
    assert_eq!(
        query_composition::QueryCompositionError::EmptyMemberships.code(),
        "QUERY_COMPOSITION_MEMBERSHIPS_EMPTY"
    );
    assert_eq!(
        query_composition::QueryCompositionError::MembershipEncoding.code(),
        "QUERY_COMPOSITION_MEMBERSHIP_ENCODING"
    );
    assert_eq!(
        query_composition::QueryCompositionError::MembershipDecoding.code(),
        "QUERY_COMPOSITION_MEMBERSHIP_DECODING"
    );
}

#[test]
fn empty_membership_render_is_rejected() {
    let member = membership(0x11);
    let plan = plan_for(member);
    assert_eq!(
        render_eligibility_filter(&plan, &BTreeSet::new()),
        Err(query_composition::QueryCompositionError::EmptyMemberships)
    );
}

#[test]
fn projection_rejects_missing_emission_permit() {
    let member = membership(0x11);
    let chain = Chain::seeded(standard_points(member));
    let mut port = chain.port();
    let predicates = chain.plan.predicates();
    let output = search_retrieval_executor::indexed::execute_indexed_leg(
        &chain.ticket,
        &chain.pins,
        &predicates,
        VECTOR_NAME,
        &Chain::query_vector(),
        8,
        true,
        &chain.request_security,
        &chain.live,
        &chain.live,
        &mut port,
    )
    .expect("indexed leg executes");
    let mut validated = validated_candidates(&chain, &output.output.nominations);
    validated[0].emission_permit.access_permit.checkpoint = AccessCheckpoint::BeforeLegDispatch;
    let error = project_candidate_set(
        envelope(2, 2, true),
        projection_inputs(&validated),
        projection_budget(8),
    )
    .expect_err("forged permit must not project");
    assert_eq!(
        error,
        search_result_projector::ProjectionError::EmissionPermitMissing
    );
}
