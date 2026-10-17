//! Causal V2 output cases over the actual codec and bound-session state owner.
use super::*;
use crate::frame::{EvidenceResponseBodyV2 as Body, EvidenceResponseEnvelopeV2};
use search_contracts::evidence_frames::*;
use search_contracts::{
    BoundedList, Coverage, CoverageDenominatorKind, ObservationCursorRevision,
    ObservationFreshness, ObservationFreshnessState,
};

fn admitted(session: &mut BoundSession, budget: u64) -> AdmittedEvidenceProviderRequestV2 {
    let mut envelope = evidence_request_envelope_v2(RequestId::from_bytes([0x81; 16]), 1, peer());
    let crate::frame::EvidenceRecipeRequestV2::RetrieveEvidence(value) =
        &mut envelope.recipe_request
    else {
        panic!("retrieve fixture");
    };
    value.budgets.max_materialized_result_bytes = budget;
    value.budgets.deadline_ms = 500;
    let frame = ClientEnvelopeCodec::encode_evidence_request_v2(
        &envelope,
        DEFAULT_PROTOCOL_LIMITS,
        local_range(),
    )
    .expect("request frame");
    let proof = ProofDigest::from_bytes([0x82; 32]);
    session
        .admit_evidence_provider_request_v2(
            frame.as_slice(),
            &proof,
            1_000,
            &mut || Ok(MonotonicMillis::new(10)),
            |_| Ok(proof),
        )
        .expect("admission")
}

fn header(kind: EvidenceFrameKind, sequence: u64) -> EvidenceFrameHeader {
    EvidenceFrameHeader {
        request_id: RequestId::from_bytes([0x81; 16]),
        kind,
        event_sequence: sequence,
        profile_id: ProfileId::new("lexical-v1").expect("profile"),
    }
}

fn coverage() -> Coverage {
    Coverage {
        requested_legs: BoundedList::empty(),
        executed_legs: BoundedList::empty(),
        represented_memberships: BoundedSet::empty(),
        represented_source_lineages: BoundedSet::empty(),
        omitted_or_failed_legs: BoundedList::empty(),
        candidate_validation_gaps: BoundedList::empty(),
        observation_freshness: ObservationFreshness {
            state: ObservationFreshnessState::Unknown,
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observed_age_ms: None,
        },
        unknowns: BoundedList::empty(),
        denominator_kind: CoverageDenominatorKind::CandidateScope,
    }
}

fn accepted(budget: u32) -> Body {
    Body::Frame(EvidenceFrameV2::Accepted(EvidenceAcceptedFrame {
        header: header(EvidenceFrameKind::Accepted, 1),
        progressive: true,
        declared_frame_budget: budget,
    }))
}

fn snapshot(sequence: u64) -> Body {
    Body::Frame(EvidenceFrameV2::CoverageSnapshot(
        EvidenceCoverageSnapshotFrame {
            header: header(EvidenceFrameKind::CoverageSnapshot, sequence),
            coverage: coverage(),
            complete_negative_possible: false,
        },
    ))
}

fn terminal(sequence: u64) -> EvidenceTerminalFrame {
    EvidenceTerminalFrame {
        header: header(EvidenceFrameKind::Terminal, sequence),
        terminal: EvidenceTerminalKind::Success,
        coverage: coverage(),
        reason_codes: BoundedSet::empty(),
        budget_exhausted: false,
    }
}

fn emit(
    session: &mut BoundSession,
    request: &mut AdmittedEvidenceProviderRequestV2,
    body: Body,
) -> Result<(), ProviderDeliveryError<()>> {
    session.deliver_evidence_provider_event_v2(
        request,
        body,
        &mut || Ok(MonotonicMillis::new(11)),
        |transcript| {
            let decoded = ServerEnvelopeCodec::decode_evidence_response_v2(
                transcript.frame(),
                DEFAULT_PROTOCOL_LIMITS,
                local_range(),
            )
            .expect("actual V2 response decode");
            assert_eq!(decoded.request_id, *request_id());
            Ok(())
        },
    )
}

fn request_id() -> &'static RequestId {
    const ID: RequestId = RequestId::from_bytes([0x81; 16]);
    &ID
}

#[test]
fn events_finish_once_and_preserve_exact_authenticated_bytes() {
    let mut session = bound_session();
    let mut request = admitted(&mut session, 65_536);
    assert_eq!(request.guard().deadline(), Some(MonotonicMillis::new(510)));
    let mut emitted = Vec::new();
    for body in [
        accepted(3),
        snapshot(2),
        Body::Frame(EvidenceFrameV2::Terminal(terminal(3))),
    ] {
        session
            .deliver_evidence_provider_event_prepared_v2(
                &mut request,
                &mut || Ok(MonotonicMillis::new(11)),
                |context| {
                    let envelope = context.envelope_v2(body.clone());
                    let bytes = ServerEnvelopeCodec::encode_evidence_response_v2(
                        &envelope,
                        context.limits(),
                        context.supported_versions(),
                    )
                    .expect("encode");
                    Ok::<_, ()>((body, bytes))
                },
                |transcript| {
                    let decoded = ServerEnvelopeCodec::decode_evidence_response_v2(
                        transcript.frame(),
                        DEFAULT_PROTOCOL_LIMITS,
                        local_range(),
                    )
                    .expect("round trip");
                    assert_eq!(decoded.connection_sequence, decoded.event_sequence);
                    let encoded = ServerEnvelopeCodec::encode_evidence_response_v2(
                        &decoded,
                        DEFAULT_PROTOCOL_LIMITS,
                        local_range(),
                    )
                    .expect("exact encode");
                    assert_eq!(transcript.frame(), encoded.as_slice());
                    emitted.push(decoded);
                    Ok::<_, ()>(())
                },
            )
            .expect("actual prepared delivery");
    }
    assert_eq!(emitted.len(), 3);
    assert_eq!(session.in_flight_len(), 0);
    assert_eq!(request.next_event_sequence(), None);
    let duplicate = session.finish_evidence_provider_request_v2(
        &mut request,
        terminal(4),
        &mut || Ok(MonotonicMillis::new(12)),
        |_| panic!("duplicate must not write"),
    );
    assert!(matches!(
        duplicate,
        Err(ProviderDeliveryError::<()>::Protocol(
            ProtocolError::DuplicateTerminal
        ))
    ));
    assert!(matches!(
        session.complete_request(request_id(), TerminalKind::Success),
        Err(ProtocolError::DuplicateTerminal)
    ));
}

#[test]
fn foreign_custody_tamper_and_wrong_profile_refuse_before_output() {
    let mut owner = bound_session();
    let mut request = admitted(&mut owner, 65_536);
    let mut foreign = bound_session();
    let _foreign_request = admitted(&mut foreign, 65_536);
    assert!(matches!(
        emit(&mut foreign, &mut request, accepted(4)),
        Err(ProviderDeliveryError::Protocol(
            ProtocolError::InvalidSessionTransition
        ))
    ));
    let result = owner.deliver_evidence_provider_event_prepared_v2(
        &mut request,
        &mut || Ok(MonotonicMillis::new(11)),
        |context| {
            let body = accepted(4);
            let mut envelope = context.envelope_v2(body.clone());
            envelope.binding_id = BindingId::from_bytes([0x99; 16]);
            let frame = ServerEnvelopeCodec::encode_evidence_response_v2(
                &envelope,
                context.limits(),
                context.supported_versions(),
            )
            .expect("well-shaped foreign frame");
            Ok::<_, ()>((body, frame))
        },
        |_| panic!("tampered frame must not write"),
    );
    assert!(matches!(
        result,
        Err(PreparedProviderDeliveryError::<(), ()>::Protocol(
            ProtocolError::InvalidBody
        ))
    ));
    let Body::Frame(EvidenceFrameV2::Accepted(mut wrong)) = accepted(4) else {
        unreachable!()
    };
    wrong.header.profile_id = ProfileId::new("foreign").expect("profile");
    assert!(matches!(
        emit(
            &mut owner,
            &mut request,
            Body::Frame(EvidenceFrameV2::Accepted(wrong))
        ),
        Err(ProviderDeliveryError::Protocol(ProtocolError::InvalidBody))
    ));
    assert!(matches!(
        emit(&mut owner, &mut request, snapshot(1)),
        Err(ProviderDeliveryError::Protocol(ProtocolError::InvalidBody))
    ));
    assert_eq!(request.next_event_sequence(), Some(1));
    emit(&mut owner, &mut request, accepted(4))
        .expect("refusals preserve event and provider sequence");
}

#[test]
fn cumulative_and_declared_budgets_preserve_cancel_terminal_capacity() {
    let mut session = bound_session();
    let mut request = admitted(&mut session, 1);
    assert!(matches!(
        emit(&mut session, &mut request, accepted(3)),
        Err(ProviderDeliveryError::Protocol(
            ProtocolError::ResourceExhausted
        ))
    ));
    assert_eq!(request.next_event_sequence(), Some(1));
    let cancel = ProviderEnvelope {
        protocol_major: 1,
        protocol_minor: 0,
        installation_incarnation_id: peer().incarnation,
        binding_id: peer().binding,
        connection_sequence: 2,
        request_id: RequestId::from_bytes([0x83; 16]),
        message_kind: MessageKind::Cancel,
        relative_deadline_ms: Some(100),
        body: ProviderBodyV1::Cancel(search_contracts::CancelBody {
            target_request_id: *request_id(),
        }),
    };
    let frame = ClientEnvelopeCodec::encode(&cancel, DEFAULT_PROTOCOL_LIMITS, local_range())
        .expect("cancel control");
    let proof = ProofDigest::from_bytes([0x84; 32]);
    let outcome = session
        .cancel_provider_request(
            frame.as_slice(),
            &proof,
            100,
            &mut || Ok(MonotonicMillis::new(11)),
            |_| Ok(proof),
            |transcript, _| {
                let ack = ServerEnvelopeCodec::decode(
                    transcript.frame(),
                    DEFAULT_PROTOCOL_LIMITS,
                    local_range(),
                )
                .expect("existing cancel control response");
                assert_eq!(ack.request_id, cancel.request_id);
                assert!(matches!(ack.body, ProviderBodyV1::Cancelled(value) if !value.terminal));
                Ok::<_, ()>(())
            },
        )
        .expect("authenticated cancellation shares V1 ledger");
    assert!(matches!(
        outcome,
        CancelOutcome::Cancelled { terminal: false }
    ));
    let replay = session.cancel_provider_request(
        frame.as_slice(),
        &proof,
        100,
        &mut || Ok(MonotonicMillis::new(11)),
        |_| Ok(proof),
        |_, _| panic!("replay must not write"),
    );
    assert!(matches!(
        replay,
        Err(ProviderDeliveryError::<()>::Protocol(
            ProtocolError::DuplicateSequence
        ))
    ));
    session
        .finish_evidence_provider_cancelled_v2(
            &mut request,
            &mut || Ok(MonotonicMillis::new(11)),
            |transcript| {
                let decoded = ServerEnvelopeCodec::decode_evidence_response_v2(
                    transcript.frame(),
                    DEFAULT_PROTOCOL_LIMITS,
                    local_range(),
                )
                .expect("cancelled frame");
                assert!(matches!(decoded.body, Body::Cancelled(value) if value.terminal));
                Ok::<_, ()>(())
            },
        )
        .expect("one reserved terminal despite data budget");
    assert_eq!(request.next_event_sequence(), None);
    assert_eq!(session.in_flight_len(), 0);

    let mut byte_budget = 0;
    for (event_sequence, body) in [(1, accepted(3)), (2, snapshot(2))] {
        let envelope = EvidenceResponseEnvelopeV2 {
            protocol_major: 1,
            protocol_minor: 0,
            installation_incarnation_id: peer().incarnation,
            binding_id: peer().binding,
            connection_sequence: event_sequence,
            request_id: *request_id(),
            event_sequence,
            body,
        };
        let length = ServerEnvelopeCodec::encode_evidence_response_v2(
            &envelope,
            DEFAULT_PROTOCOL_LIMITS,
            local_range(),
        )
        .expect("individual frame fits")
        .len();
        byte_budget = byte_budget.max(u64::try_from(length).expect("frame length"));
    }
    let mut session = bound_session();
    let mut request = admitted(&mut session, byte_budget);
    emit(&mut session, &mut request, accepted(3)).expect("first individually fitting frame");
    assert!(matches!(
        emit(&mut session, &mut request, snapshot(2)),
        Err(ProviderDeliveryError::Protocol(
            ProtocolError::ResourceExhausted
        ))
    ));
    assert_eq!(request.next_event_sequence(), Some(2));
    session.cancel(request_id());
    session
        .finish_evidence_provider_cancelled_v2(
            &mut request,
            &mut || Ok(MonotonicMillis::new(11)),
            |_| Ok::<_, ()>(()),
        )
        .expect("reserved terminal after cumulative exhaustion");

    let mut session = bound_session();
    let mut request = admitted(&mut session, 65_536);
    emit(&mut session, &mut request, accepted(3)).expect("accepted");
    emit(&mut session, &mut request, snapshot(2)).expect("snapshot");
    let progress = Body::Progress(ProgressBody {
        event_sequence: 3,
        phase: ProgressPhase::Projecting,
        bounded_counts: BoundedProgressCounts {
            completed_legs: 0,
            total_planned_legs: 0,
            nominated_candidates: 0,
            validated_candidates: 0,
            omitted_or_failed_legs: 0,
        },
        degraded_reason_codes: BoundedSet::empty(),
    });
    assert!(matches!(
        emit(&mut session, &mut request, progress),
        Err(ProviderDeliveryError::Protocol(
            ProtocolError::ResourceExhausted
        ))
    ));
    session.cancel(request_id());
    assert!(matches!(
        emit(
            &mut session,
            &mut request,
            Body::Frame(EvidenceFrameV2::Terminal(terminal(3)))
        ),
        Err(ProviderDeliveryError::Protocol(
            ProtocolError::InvalidSessionTransition
        ))
    ));
    session
        .finish_evidence_provider_cancelled_v2(
            &mut request,
            &mut || Ok(MonotonicMillis::new(11)),
            |_| Ok::<_, ()>(()),
        )
        .expect("cancel terminal");
}

#[test]
fn result_recipe_scope_and_source_view_remain_bound_to_admitted_request() {
    use search_contracts::*;
    let mut session = bound_session();
    let mut request = admitted(&mut session, 65_536);
    emit(&mut session, &mut request, accepted(4)).expect("accepted");
    emit(&mut session, &mut request, snapshot(2)).expect("snapshot");
    let crate::frame::EvidenceRecipeRequestV2::RetrieveEvidence(input) =
        &request.envelope().recipe_request
    else {
        unreachable!()
    };
    let fence = ResultFence {
        planned_snapshot: QuerySnapshotFence {
            installation_incarnation_id: peer().incarnation,
            collection_generation_id: None,
            visible_epoch: None,
            collection_route_revision: CollectionRouteRevision::new(1),
            catalog_revision: CatalogRevision::new(1),
            membership_revision: MembershipRevision::new(1),
            reference_portfolio_revision: None,
            access_policy_revision: AccessPolicyRevision::new(1),
            shadow_fence_revision: ShadowFenceRevision::new(1),
            purge_fence_revision: PurgeFenceRevision::new(1),
            overlay_revision: OverlayRevision::new(0),
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observation_freshness: coverage().observation_freshness,
            source_view: input.source_view.clone(),
            workspace_view_revision_ref: None,
            lexical_profile_ids: BoundedList::empty(),
            snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([1; 32]),
        },
        emission_source_owner_fences: BoundedList::empty(),
        emission_security_fence: EmissionSecurityFence {
            access_policy_revision: AccessPolicyRevision::new(1),
            live_deny_generation: 1,
            shadow_fence_revision: ShadowFenceRevision::new(1),
            purge_fence_revision: PurgeFenceRevision::new(1),
            checked_at: UtcTimestamp::parse("2026-10-17T00:00:00.000000Z").expect("timestamp"),
            receipt_ref: ReceiptRef::new("fixture-only").expect("receipt"),
        },
        result_fingerprint: Blake3Digest32::from_stored_bytes([1; 32]),
    };
    let result = search_contracts::evidence::RetrieveEvidenceResultV2 {
        request_id: *request_id(),
        plan_id: PlanId::from_bytes([1; 16]),
        plan_fingerprint: PlanFingerprint::from_bytes([1; 32]),
        result_fence: fence,
        resolved_scope: input.scope.clone(),
        cards: BoundedList::empty(),
        groups: BoundedList::empty(),
        suggested_exact_checks: BoundedList::empty(),
        coverage: coverage(),
        continuation: None,
    };
    let body = |result| {
        Body::Result(crate::frame::EvidenceRecipeResultV2::RetrieveEvidence(
            result,
        ))
    };
    let mut wrong = result.clone();
    wrong.resolved_scope =
        PublicScopeSelector::WorkspaceRevision(WorkspaceViewRevisionId::from_bytes([0x91; 16]));
    assert!(matches!(
        emit(&mut session, &mut request, body(wrong)),
        Err(ProviderDeliveryError::Protocol(ProtocolError::InvalidBody))
    ));
    let mut wrong = result.clone();
    wrong.result_fence.planned_snapshot.source_view =
        SourceView::RetainedRevision(SourceRevisionId::from_bytes([0x92; 16]));
    assert!(matches!(
        emit(&mut session, &mut request, body(wrong)),
        Err(ProviderDeliveryError::Protocol(ProtocolError::InvalidBody))
    ));
    let opposite = Body::Result(crate::frame::EvidenceRecipeResultV2::OrientScope(
        search_contracts::evidence::OrientScopeResultV2 {
            request_id: result.request_id,
            plan_id: result.plan_id,
            plan_fingerprint: result.plan_fingerprint,
            result_fence: result.result_fence.clone(),
            resolved_scope: result.resolved_scope.clone(),
            entry_points: BoundedList::empty(),
            groups: BoundedList::empty(),
            recommended_reading: BoundedList::empty(),
            suggested_exact_checks: BoundedList::empty(),
            ambiguities: BoundedList::empty(),
            coverage: coverage(),
            continuation: None,
        },
    ));
    assert!(matches!(
        emit(&mut session, &mut request, opposite),
        Err(ProviderDeliveryError::Protocol(ProtocolError::InvalidBody))
    ));
    emit(&mut session, &mut request, body(result)).expect("matching result remains nonterminal");
    assert_eq!(session.in_flight_len(), 1);
    assert_eq!(request.next_event_sequence(), Some(4));
    session
        .finish_evidence_provider_request_v2(
            &mut request,
            terminal(4),
            &mut || Ok(MonotonicMillis::new(11)),
            |_| Ok::<_, ()>(()),
        )
        .expect("sole terminal releases slot");
}

#[test]
fn output_failure_closes_original_session_without_terminal_success() {
    let mut session = bound_session();
    let mut request = admitted(&mut session, 65_536);
    emit(&mut session, &mut request, accepted(3)).expect("accepted");
    emit(&mut session, &mut request, snapshot(2)).expect("snapshot");
    let result = session.finish_evidence_provider_request_v2(
        &mut request,
        terminal(3),
        &mut || Ok(MonotonicMillis::new(11)),
        |_| Err("write failed"),
    );
    assert!(matches!(
        result,
        Err(ProviderDeliveryError::Output("write failed"))
    ));
    assert_eq!(session.session_state(), SessionState::Closed);
    assert_eq!(session.in_flight_len(), 0);
    assert!(request.guard().is_cancelled());
    assert_eq!(request.next_event_sequence(), Some(3));
}

#[test]
fn strict_response_codec_rejects_alternate_sequence_and_unknown_schema() {
    let envelope = EvidenceResponseEnvelopeV2 {
        protocol_major: 1,
        protocol_minor: 0,
        installation_incarnation_id: peer().incarnation,
        binding_id: peer().binding,
        connection_sequence: 1,
        request_id: *request_id(),
        event_sequence: 1,
        body: accepted(3),
    };
    let frame = ServerEnvelopeCodec::encode_evidence_response_v2(
        &envelope,
        DEFAULT_PROTOCOL_LIMITS,
        local_range(),
    )
    .expect("encode");
    let json = core::str::from_utf8(&frame.as_slice()[4..]).expect("JSON");
    for altered in [
        json.replacen("\"event_sequence\":1", "\"event_sequence\":2", 1),
        json.replacen("\"v\":2", "\"v\":3", 1),
        json.replacen("\"v\":2", "\"v\":2,\"extra\":0", 1),
    ] {
        let payload =
            search_contracts::JsonFramePayload::new(altered.into_bytes()).expect("payload");
        let bytes = FrameCodec::encode(&payload, DEFAULT_PROTOCOL_LIMITS).expect("frame");
        assert!(
            ServerEnvelopeCodec::decode_evidence_response_v2(
                bytes.as_slice(),
                DEFAULT_PROTOCOL_LIMITS,
                local_range()
            )
            .is_err()
        );
    }
}
