//! One real V2 input -> C18 -> C23 -> C24 -> C28 -> C26 exchange.

use super::{
    ReadTurn,
    output::{self, CardSource},
    source::RetainedProjectionResolver,
};
use crate::access_composition::{
    BoundedStandaloneGrantIssuer, GrantValidationClock, NativeSecurityDomain,
    StandaloneScopeResolver, SystemGrantClock, VerifiedStandaloneGrant,
};
use crate::owner_composition::{
    NativeProcessRootCustody, QualifiedNativeProfiles, RetainedRevisionReadState,
};
use crate::provider_composition::{
    CanonicalServingError as Error, CanonicalTcpConnection, CanonicalWorkBudget, monotonic_millis,
};
use crate::query_live_authority::LiveNativeQueryAuthorityOwner;
use search_access::{
    AccessCheckpoint, CompiledEligibilityPopulation, LiveSecurityState, RequestSecurityFence,
    RequestedMembershipScope,
};
use search_contracts::{
    self as c,
    evidence::RetrieveEvidenceRequestV2,
    evidence_frames::{
        EvidenceAcceptedFrame, EvidenceCoverageSnapshotFrame, EvidenceFrameHeader,
        EvidenceFrameKind, EvidenceFrameV2,
    },
};
use search_provider_protocol::{
    AdmittedEvidenceProviderRequestV2, BindingContext, EvidenceResponseBodyV2, MonotonicMillis,
    frame::EvidenceRecipeRequestV2,
};
use search_result_projector::{
    CandidateProjectionInput, CandidatePublicMetadata, CandidateSetEnvelope,
};
use search_retrieval_executor::{LegTicket, acquire_leg_pins};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    task::Poll,
    time::{Duration, Instant},
};

/// Borrow the one runtime's resources. Construction issues no grant, root,
/// ledger, source pin registry, handle store, planner proof or backend client.
pub struct NativeEvidenceRuntime<'a> {
    pub(crate) transport: &'a mut CanonicalTcpConnection,
    pub(crate) plane: &'a search_qdrant_bridge::real::BlockingRealQueryPlane,
    pub(crate) route: search_qdrant_bridge::CollectionRoute,
    pub(crate) custody: &'a mut NativeProcessRootCustody,
    pub(crate) registry: &'a search_source_registry::SourceRegistry,
    pub(crate) profiles: &'a QualifiedNativeProfiles,
    pub(crate) handles: &'a mut search_handles::HandleStore,
    pub(crate) epoch_pins: &'a search_epoch_pins::PinRegistry,
    pub(crate) clock: &'a mut SystemGrantClock,
    pub(crate) config: &'a crate::config_composition::EffectiveDaemonConfig,
}

/// Actual ingress on the paired transport, including its common cancel route.
/// The runtime's C22 compiler runs synchronously *inside* the same C18 callback
/// with the actual admitted V2 request. It returns the real immutable leg ticket,
/// its plan ID and full original budget; there is no fallback compiler here.
#[allow(clippy::too_many_arguments)]
#[expect(
    clippy::too_many_lines,
    reason = "Keep admission, same-lock execution and delivery ordered in one borrowed owner turn"
)]
pub fn poll_native_evidence_v2<R: StandaloneScopeResolver, V, E, T: GrantValidationClock>(
    binding: &BindingContext,
    issuer: &mut BoundedStandaloneGrantIssuer<E, T>,
    authority: &LiveNativeQueryAuthorityOwner<R, V>,
    runtime: &mut NativeEvidenceRuntime<'_>,
    maximum_deadline_ms: u64,
    quantum: Duration,
    compile: impl FnOnce(
        &CompiledEligibilityPopulation,
        &RequestedMembershipScope,
        &RequestSecurityFence,
        &NativeSecurityDomain,
        &LiveSecurityState,
        &VerifiedStandaloneGrant<'_>,
        &AdmittedEvidenceProviderRequestV2,
    ) -> Result<(c::PlanId, c::QueryExecutionBudget, LegTicket), Error>,
) -> Result<Poll<()>, Error> {
    let Poll::Ready(request) = runtime
        .transport
        .poll_receive_evidence_v2(maximum_deadline_ms, quantum)
        .map_err(Error::Transport)?
    else {
        return Ok(Poll::Pending);
    };
    let Some(mut request) = request else {
        return Ok(Poll::Ready(()));
    };
    let result = authority.with_retrieve_evidence_population(
        binding,
        issuer,
        &mut request,
        |population, scope, fence, domain, live, issued, admitted| {
            let result = (|| {
                let EvidenceRecipeRequestV2::RetrieveEvidence(input) =
                    &admitted.envelope().recipe_request
                else {
                    return Err(Error::Backend("V2_RECIPE_UNSUPPORTED"));
                };
                let input = input.clone();
                supported(&input, runtime.profiles)?;
                let hard = admitted
                    .guard()
                    .deadline()
                    .ok_or(Error::DeadlineExpired)?
                    .min(issued.valid_until());
                // CPU work is conservatively bounded by elapsed time from the same
                // transport admission. C18 compilation does not restart this window.
                let cpu = admitted
                    .guard()
                    .admitted_at()
                    .get()
                    .checked_add(input.budgets.max_cpu_ms)
                    .map(MonotonicMillis::new)
                    .ok_or(Error::DeadlineExpired)?;
                let budget = CanonicalWorkBudget::new(hard.min(cpu), 25)?;
                let deadline = absolute_deadline(budget.deadline())?;
                let (plan_id, planned_budget, ticket) =
                    compile(population, scope, fence, domain, live, issued, admitted)?;
                if planned_budget != input.budgets
                    || ticket.request_id != *admitted.guard().request_id()
                    || scope.memberships
                        != population
                            .projection_memberships()
                            .keys()
                            .copied()
                            .collect()
                    || ticket.leg.memberships() != &scope.memberships
                    || ticket.leg.leg_kind() != c::LegKind::Lexical
                    || ticket.leg.budget().max_source_read_bytes()
                        > input.budgets.max_source_read_bytes
                    || ticket.leg.budget().max_memory_bytes() > input.budgets.max_memory_bytes
                    || ticket.leg.budget().max_cpu_ms() > input.budgets.max_cpu_ms
                    || ticket.leg.budget().deadline_ms() > input.budgets.deadline_ms
                    || runtime.route.generation
                        != population.population().collection_generation_id()
                {
                    return Err(Error::InvalidCompletion);
                }
                let turn = ReadTurn {
                    request: admitted,
                    issued,
                    population,
                    domain,
                    fence,
                    budget,
                };
                turn.check()?;
                if !population
                    .snapshot_fence()
                    .lexical_profile_ids
                    .iter()
                    .any(|id| {
                        id.as_str() == runtime.profiles.lexical_profile().profile().id().as_str()
                    })
                {
                    return Err(Error::Backend("V2_LEXICAL_PROFILE_MISMATCH"));
                }
                let query = encode_query(&input, &turn, runtime.profiles)?;
                let pins = acquire_leg_pins(&ticket, runtime.epoch_pins, monotonic_millis().get())
                    .map_err(|error| Error::Backend(error.code()))?;
                let mut state = RetainedRevisionReadState::new(input.budgets.max_source_read_bytes);
                let mut resolver = RetainedProjectionResolver {
                    custody: runtime.custody,
                    state: &mut state,
                    registry: runtime.registry,
                    turn: &turn,
                    profiles: runtime.profiles,
                    config: runtime.config,
                    cache: BTreeMap::new(),
                    memory_remaining: input.budgets.max_memory_bytes,
                };
                let cancellation: Arc<dyn search_ports::CancellationProbe + Send + Sync> =
                    Arc::new(admitted.guard().cancellation());
                let limit = usize::try_from(
                    input
                        .budgets
                        .max_prefetch_candidates_per_leg
                        .min(input.budgets.max_validated_candidates),
                )
                .map_err(|_| Error::InvalidConfiguration)?;
                let vector = c::indexed::VectorName::new("lex_code_v1")
                    .map_err(|_| Error::InvalidConfiguration)?;
                let mut nominated = domain
                    .with_live_checkpoint(fence, AccessCheckpoint::BeforeLegDispatch, |_| {
                        crate::query_composition::canonical_retrieval::execute_pinned_leg_canonical(
                            runtime.plane,
                            runtime.route.clone(),
                            &ticket,
                            &pins,
                            population,
                            &mut resolver,
                            &vector,
                            &query,
                            limit,
                            deadline,
                            cancellation,
                            fence,
                            live,
                            live,
                        )
                    })
                    .map_err(Error::Access)?
                    .map_err(|error| Error::Backend(error.code()))?;
                if !nominated.readback_missing_addresses.is_empty() {
                    return Err(Error::Backend("V2_CANONICAL_POINT_READBACK_GAP"));
                }
                // One exact scoring population only. Readback order cannot decide rank.
                nominated.nominations.sort_by(|a, b| {
                    b.raw_score
                        .total_cmp(&a.raw_score)
                        .then_with(|| a.point_address.cmp(&b.point_address))
                });
                if nominated.nominations.is_empty() {
                    return Err(Error::Backend("V2_NO_SOURCE_BACKED_CANDIDATES"));
                }
                let mut pending = runtime.handles.begin_pending_mints();
                let mut inputs = Vec::new();
                let mut sources = BTreeMap::new();
                for (ordinal, nomination) in nominated.nominations.iter().enumerate() {
                    turn.check()?;
                    let id = fresh_candidate_id(&turn)?;
                    let validated = resolver.validate(nomination, &ticket, id, ordinal, live)?;
                    let prepared = &resolver
                        .cache
                        .get(&nomination.projection_membership_id)
                        .ok_or(Error::InvalidCompletion)?
                        .prepared;
                    let unit = prepared
                        .units()
                        .units()
                        .iter()
                        .find(|unit| unit.unit_id() == validated.source().unit_id())
                        .ok_or(Error::InvalidCompletion)?;
                    sources.insert(
                        id,
                        CardSource {
                            source: c::evidence::PublicCardSource {
                                source_id: validated.source_revision_ref().source_id,
                                revision: validated.source_revision_ref(),
                                representation_profile: c::ProfileId::new(
                                    prepared.product().profile_id().to_string(),
                                )
                                .map_err(|_| Error::InvalidCompletion)?,
                            },
                            unit: unit.occurrence().clone(),
                        },
                    );
                    let handle =
                        crate::query_composition::retained_handle::mint_evidence_candidate_handle(
                            resolver.custody,
                            resolver.state,
                            resolver.registry,
                            &turn,
                            &mut pending,
                            &validated,
                            prepared,
                            runtime.clock,
                            deadline,
                            runtime.config,
                        )?;
                    let public = CandidatePublicMetadata {
                        candidate_id: id,
                        source_handle: handle,
                        evidence_role: c::EvidenceRole::Reference,
                        entity_kind: None,
                        assurance: c::AssuranceClass::ExactBytes,
                        freshness: population.snapshot_fence().observation_freshness.state,
                        ranking_trace: c::BoundedNonContentRankingTrace {
                            fusion_profile_id: c::FusionProfileId::new(
                                "native-v2-single-lexical-v1",
                            )
                            .map_err(|_| Error::InvalidCompletion)?,
                            fused_rank: u32::try_from(ordinal + 1)
                                .map_err(|_| Error::InvalidCompletion)?,
                            exact_or_entity_boost: c::ExactOrEntityBoost::None,
                            evidence_role_priority: 0,
                            portfolio_priority: 0,
                            lineage_diversity_action: c::LineageDiversityAction::Retained,
                            deterministic_tie_break_digest: nomination
                                .candidate
                                .identity
                                .full_digest(),
                        },
                        reason_codes: BTreeSet::new(),
                        candidate_validation_receipt_ref: validated
                            .candidate_validation_receipt_ref()
                            .clone(),
                    };
                    inputs.push(CandidateProjectionInput {
                        source_backed: validated,
                        public,
                    });
                }
                let coverage = coverage(
                    &input,
                    &ticket,
                    nominated.exact_denominator,
                    inputs.len(),
                    population,
                )?;
                let fingerprint = c::PlanFingerprint::from_bytes(*ticket.plan_digest.as_bytes());
                let permit = domain
                    .with_live_checkpoint(fence, AccessCheckpoint::BeforeResultEmission, |permit| {
                        permit
                    })
                    .map_err(Error::Access)?;
                let result_fence = output::result_fence(&turn, permit, fingerprint, &inputs)?;
                drop(resolver);
                emit_frame(
                    runtime.transport,
                    admitted,
                    issued,
                    domain,
                    fence,
                    budget,
                    EvidenceFrameV2::Accepted(EvidenceAcceptedFrame {
                        header: header(admitted, &input, EvidenceFrameKind::Accepted)?,
                        progressive: input.progressive,
                        declared_frame_budget: 4,
                    }),
                )?;
                emit_frame(
                    runtime.transport,
                    admitted,
                    issued,
                    domain,
                    fence,
                    budget,
                    EvidenceFrameV2::CoverageSnapshot(EvidenceCoverageSnapshotFrame {
                        header: header(admitted, &input, EvidenceFrameKind::CoverageSnapshot)?,
                        coverage: coverage.clone(),
                        complete_negative_possible: false,
                    }),
                )?;
                let envelope = CandidateSetEnvelope {
                    request_id: *admitted.guard().request_id(),
                    plan_id,
                    plan_fingerprint: fingerprint,
                    result_fence,
                    coverage,
                    continuation_handle: None,
                };
                let max_bytes = usize::try_from(input.budgets.max_materialized_result_bytes)
                    .map_err(|_| Error::InvalidConfiguration)?
                    .min(c::MAX_FRAME_BYTES);
                output::emit_projected(
                    runtime.transport,
                    admitted,
                    issued,
                    population,
                    domain,
                    fence,
                    budget,
                    envelope,
                    inputs,
                    &sources,
                    search_result_projector::ProjectionBudget {
                        max_candidates: limit,
                        max_total_validated_source_bytes: usize::try_from(
                            input.budgets.max_source_read_bytes,
                        )
                        .map_err(|_| Error::InvalidConfiguration)?,
                        max_source_bytes_per_candidate: usize::try_from(
                            input.budgets.max_source_read_bytes,
                        )
                        .map_err(|_| Error::InvalidConfiguration)?,
                        max_serialized_output_bytes: max_bytes,
                    },
                    pending,
                )
            })();
            if let Err(Error::Backend(code)) = &result {
                let cpu_ms = match &admitted.envelope().recipe_request {
                    EvidenceRecipeRequestV2::RetrieveEvidence(value) => value.budgets.max_cpu_ms,
                    EvidenceRecipeRequestV2::OrientScope(value) => value.budgets.max_cpu_ms,
                };
                let refusal_deadline = admitted
                    .guard()
                    .admitted_at()
                    .get()
                    .checked_add(cpu_ms)
                    .map(MonotonicMillis::new)
                    .ok_or(Error::DeadlineExpired)?
                    .min(admitted.guard().deadline().ok_or(Error::DeadlineExpired)?)
                    .min(issued.valid_until());
                let failure = EvidenceResponseBodyV2::Error(c::ErrorBody {
                    code: if code.ends_with("UNSUPPORTED") {
                        c::ProtocolFailureCode::Protocol(
                            c::ProtocolErrorCode::UnsupportedMessageKind,
                        )
                    } else {
                        c::ProtocolFailureCode::Search(c::SearchReasonCodeV1::IncompleteCoverage)
                    },
                    retryability: c::ProtocolRetryability::Never,
                    message_template_id: c::OpaqueId::new(*code)
                        .map_err(|_| Error::InvalidCompletion)?,
                    bounded_metadata: c::BoundedNonContentMetadata::new(Vec::new())
                        .map_err(|_| Error::InvalidCompletion)?,
                });
                domain
                    .with_live_checkpoint(fence, AccessCheckpoint::BeforeResultEmission, |_| {
                        issued
                            .verify_current_evidence_claims(&admitted.envelope().grant)
                            .map_err(Error::GrantRefused)?;
                        runtime
                            .transport
                            .deliver_evidence_event_before_v2(admitted, failure, refusal_deadline)
                            .map_err(Error::Transport)
                    })
                    .map_err(Error::Access)??;
            }
            result
        },
    );
    if result.is_err() {
        runtime.transport.close();
    }
    result.map(|()| Poll::Ready(()))
}

fn supported(
    input: &RetrieveEvidenceRequestV2,
    _profiles: &QualifiedNativeProfiles,
) -> Result<(), Error> {
    if !matches!(input.source_view, c::SourceView::RetainedRevision(_))
        || input
            .requested_roles
            .iter()
            .any(|role| *role != c::EvidenceRole::Reference)
        || !input.optional_hints.authorized_source_ids.is_empty()
        || !input.optional_hints.suggested_exact_handles.is_empty()
        || input.budgets.max_validated_candidates > 256
    {
        return Err(Error::Backend("V2_LEXICAL_SHAPE_UNSUPPORTED"));
    }
    Ok(())
}

fn absolute_deadline(deadline: MonotonicMillis) -> Result<Instant, Error> {
    let anchor = Instant::now();
    let millis = deadline
        .get()
        .checked_sub(monotonic_millis().get())
        .and_then(|millis| millis.checked_sub(1))
        .filter(|millis| *millis != 0)
        .ok_or(Error::DeadlineExpired)?;
    anchor
        .checked_add(Duration::from_millis(millis))
        .ok_or(Error::DeadlineExpired)
}

fn encode_query(
    input: &RetrieveEvidenceRequestV2,
    turn: &ReadTurn<'_, '_>,
    profiles: &QualifiedNativeProfiles,
) -> Result<c::indexed::VectorValue, Error> {
    turn.check()?;
    let query = search_lexical::LexicalInput::new(
        c::OpaqueId::new(format!("query:{}", turn.request.guard().request_id()))
            .map_err(|_| Error::InvalidCompletion)?,
        c::NonZeroRevision::new(turn.request.guard().sequence())
            .map_err(|_| Error::InvalidCompletion)?,
        0,
        0,
        u64::try_from(input.question.as_str().len()).map_err(|_| Error::InvalidCompletion)?,
        input.question.as_str().to_owned(),
    );
    let encoded = search_lexical::encode_code_query(
        &query,
        profiles.lexical_profile(),
        search_lexical::CodeIdentifierLimits::HARD,
        search_lexical::SparseLimits::BASELINE,
        turn.request.guard().is_cancelled(),
    )
    .map_err(|_| Error::Backend("V2_LEXICAL_ENCODING_REFUSED"))?;
    turn.check()?;
    Ok(c::indexed::VectorValue::Sparse {
        indices: encoded.encoding().vector.indices.clone(),
        values: encoded.encoding().vector.values.clone(),
    })
}

fn fresh_candidate_id(turn: &ReadTurn<'_, '_>) -> Result<c::CandidateId, Error> {
    turn.check()?;
    let mut bytes = [0; 16];
    crate::qualified_entropy::fill_qualified_entropy(&mut bytes)
        .map_err(|_| Error::Backend("V2_CANDIDATE_ID_UNAVAILABLE"))?;
    turn.check()?;
    Ok(c::CandidateId::from_bytes(bytes))
}

fn header(
    request: &AdmittedEvidenceProviderRequestV2,
    input: &RetrieveEvidenceRequestV2,
    kind: EvidenceFrameKind,
) -> Result<EvidenceFrameHeader, Error> {
    Ok(EvidenceFrameHeader {
        request_id: *request.guard().request_id(),
        kind,
        event_sequence: request
            .next_event_sequence()
            .ok_or(Error::InvalidCompletion)?,
        profile_id: input.profile_id.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_frame(
    transport: &mut CanonicalTcpConnection,
    request: &mut AdmittedEvidenceProviderRequestV2,
    issued: &VerifiedStandaloneGrant<'_>,
    domain: &NativeSecurityDomain,
    fence: &RequestSecurityFence,
    budget: CanonicalWorkBudget,
    frame: EvidenceFrameV2,
) -> Result<(), Error> {
    domain
        .with_live_checkpoint(fence, AccessCheckpoint::BeforeResultEmission, |_| {
            budget.check()?;
            if request.guard().is_cancelled() {
                return Err(Error::GrantRefused(
                    crate::access_composition::GrantUseError::RequestInactive,
                ));
            }
            issued
                .verify_current_evidence_claims(&request.envelope().grant)
                .map_err(Error::GrantRefused)?;
            transport
                .deliver_evidence_event_before_v2(
                    request,
                    EvidenceResponseBodyV2::Frame(frame),
                    budget.deadline().min(issued.valid_until()),
                )
                .map_err(Error::Transport)
        })
        .map_err(Error::Access)??;
    Ok(())
}

fn coverage(
    input: &RetrieveEvidenceRequestV2,
    ticket: &LegTicket,
    count: u64,
    validated: usize,
    population: &CompiledEligibilityPopulation,
) -> Result<c::Coverage, Error> {
    let leg = c::OpaqueId::new(format!("lexical:{}", ticket.leg.leg_id()))
        .map_err(|_| Error::InvalidCompletion)?;
    let execution = output::digest(
        "eliot/cbor/native-v2-lexical-execution/v1",
        vec![
            output::bytes(ticket.request_id.as_bytes())?,
            output::bytes(ticket.plan_digest.as_bytes())?,
            c::CanonicalValue::U64(count),
            c::CanonicalValue::U64(u64::try_from(validated).map_err(|_| Error::InvalidCompletion)?),
        ],
    )?;
    Ok(c::Coverage {
        requested_legs: c::BoundedList::new(vec![c::LegDescriptor {
            leg_ref: leg.clone(),
            leg_kind: c::LegKind::Lexical,
            scoring_partition_ref: None,
            profile_id: input.profile_id.clone(),
        }])
        .map_err(|_| Error::InvalidCompletion)?,
        executed_legs: c::BoundedList::new(vec![c::LegExecutionSummary {
            leg_ref: leg,
            state: c::LegExecutionState::Completed,
            nominated_count: u32::try_from(validated).map_err(|_| Error::InvalidCompletion)?,
            validated_count: u32::try_from(validated).map_err(|_| Error::InvalidCompletion)?,
            reason_codes: c::BoundedSet::empty(),
            receipt_ref: c::ReceiptRef::new(format!("native-v2-lexical-execution:v1:{execution}"))
                .map_err(|_| Error::InvalidCompletion)?,
        }])
        .map_err(|_| Error::InvalidCompletion)?,
        represented_memberships: c::BoundedSet::from_items(
            ticket.leg.memberships().iter().copied(),
        )
        .map_err(|_| Error::InvalidCompletion)?,
        represented_source_lineages: c::BoundedSet::empty(),
        omitted_or_failed_legs: c::BoundedList::empty(),
        candidate_validation_gaps: c::BoundedList::empty(),
        observation_freshness: population.snapshot_fence().observation_freshness,
        unknowns: c::BoundedList::empty(),
        denominator_kind: c::CoverageDenominatorKind::CandidateScope,
    })
}
