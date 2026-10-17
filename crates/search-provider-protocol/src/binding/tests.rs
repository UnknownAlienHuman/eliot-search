use std::cell::RefCell;

use search_contracts::protocol::{HelloBody, PeerRole};
use search_contracts::{
    AccessPartitionId, BindingId, BoundedNonContentMetadata, BoundedProgressCounts, BoundedSet,
    DisclosureCeiling, ErrorBody, GrantId, InstallationId, InstallationIncarnationId, MessageKind,
    Modality, OpaqueId, OpaqueRef, ProfileId, ProtocolFailureCode, ProtocolRange,
    ProtocolRetryability, ProtocolVersion, ProviderBodyV1, ProviderEnvelope, RecipeBodyV1,
    RecipeIdV1, RequestBody, RequestId, RequestedScope, ScopeDomainId, SearchReadGrantClaims,
    SearchReasonCodeV1, SearchRecipeRequest, SensitivityClass, SourceMembershipId, SourceView,
    UtcTimestamp, WorkspaceId, WorkspaceViewRevisionId, WorkspaceViewSource,
    protocol::{ProgressBody, ProgressPhase},
};

use super::*;
use crate::cancel::CancelOutcome;
use crate::config::DEFAULT_PROTOCOL_LIMITS;
use crate::pairing::{
    ClientNonce, PairingChallenge, PairingMachine, ProofDigest, ServerNonce, SessionId,
    VerifiedPairing, verify_proof,
};
use crate::request::{
    AuthenticatedEnvelope, ControlCommand, MonotonicMillis, RequestStatus, seal_envelope,
};
use crate::terminal::TerminalKind;
use crate::frame::{ClientEnvelopeCodec, ServerEnvelopeCodec};

fn version(major: u16, minor: u16) -> ProtocolVersion {
    ProtocolVersion { major, minor }
}

fn local_range() -> ProtocolRange {
    ProtocolRange::new(version(1, 0), version(1, 2)).expect("range")
}

fn hello() -> HelloBody {
    HelloBody {
        peer_role: PeerRole::StandaloneCli,
        pairing_proof_ref: search_contracts::canonical::OpaqueRef::new("proof-ref")
            .expect("ref"),
        supported_protocol_range: local_range(),
        requested_capability_digest: None,
    }
}

fn peer() -> TransportPeer {
    TransportPeer {
        role: PeerRole::StandaloneCli,
        incarnation: InstallationIncarnationId::from_bytes([0x11; 16]),
        binding: BindingId::from_bytes([0x22; 16]),
    }
}

fn verified() -> VerifiedPairing {
    let binding = ProofDigest::from_bytes([9; 32]);
    let mut machine = PairingMachine::new(version(1, 0), binding);
    machine
        .issue_challenge(
            SessionId::from_bytes([1; 16]).expect("session"),
            ClientNonce::from_bytes([2; 16]).expect("nonce"),
            PairingChallenge::from_bytes([3; 32]).expect("challenge"),
        )
        .expect("challenge");
    machine
        .verify_client_proof(&binding, &binding)
        .expect("verify");
    machine
        .issue_provider_proof(ProofDigest::from_bytes([4; 32]))
        .expect("provider");
    machine.into_verified().expect("verified")
}

fn bound_session() -> BoundSession {
    let pairing = verified();
    let peer = peer();
    let context =
        authenticate_binding(&hello(), &pairing, &peer.incarnation, &peer).expect("binding");
    BoundSession::open(
        context,
        pairing,
        ServerNonce::from_bytes([0x44; 16]).expect("nonce"),
        DEFAULT_PROTOCOL_LIMITS,
    )
    .expect("open")
}

fn envelope(request_id: RequestId, body: ProofDigest, proof: ProofDigest) -> AuthenticatedEnvelope {
    seal_envelope(
        version(1, 0),
        ServerNonce::from_bytes([0x44; 16]).expect("nonce"),
        request_id,
        ControlCommand::Health,
        body,
        proof,
    )
}

#[test]
fn hello_negotiates_and_rejects_role_substitution() {
    let acceptor = BindingSession::new(local_range(), DEFAULT_PROTOCOL_LIMITS).expect("session");
    let negotiated = acceptor.accept_hello(&hello(), &peer()).expect("hello");
    assert_eq!(negotiated.version(), version(1, 2));
    let mut wrong_role = peer();
    wrong_role.role = PeerRole::Daemon;
    assert_eq!(
        acceptor.accept_hello(&hello(), &wrong_role),
        Err(ProtocolError::AuthenticationFailed)
    );
    let mut foreign = hello();
    foreign.supported_protocol_range =
        ProtocolRange::new(version(2, 0), version(2, 0)).expect("range");
    assert_eq!(
        acceptor.accept_hello(&foreign, &peer()),
        Err(ProtocolError::NoCompatibleVersion)
    );
}

#[test]
fn binding_requires_pairing_plus_incarnation_and_peer() {
    let pairing = verified();
    let peer = peer();
    let context =
        authenticate_binding(&hello(), &pairing, &peer.incarnation, &peer).expect("binding");
    assert_eq!(context.version(), version(1, 0));
    assert_eq!(context.binding_id(), peer.binding);
    let mut other_incarnation = peer;
    other_incarnation.incarnation = InstallationIncarnationId::from_bytes([0x33; 16]);
    assert_eq!(
        authenticate_binding(&hello(), &pairing, &other_incarnation.incarnation, &peer),
        Err(ProtocolError::AuthenticationFailed)
    );
    let mut narrow = hello();
    narrow.supported_protocol_range =
        ProtocolRange::new(version(1, 1), version(1, 2)).expect("range");
    assert_eq!(
        authenticate_binding(&narrow, &pairing, &peer.incarnation, &peer),
        Err(ProtocolError::NoCompatibleVersion)
    );
}

#[test]
fn open_binds_session_to_ceremony_proof() {
    let pairing = verified();
    let peer = peer();
    let context =
        authenticate_binding(&hello(), &pairing, &peer.incarnation, &peer).expect("binding");
    let session = BoundSession::open(
        context,
        pairing,
        ServerNonce::from_bytes([0x44; 16]).expect("nonce"),
        DEFAULT_PROTOCOL_LIMITS,
    )
    .expect("open");
    assert!(session.is_active());
    assert!(verify_proof(
        &pairing.provider_proof(),
        &pairing.provider_proof()
    ));
}

#[test]
fn body_mismatch_precedes_sequence_replay_and_in_flight_mutation() {
    let mut session = bound_session();
    let request_id = RequestId::from_bytes([0x51; 16]);
    let body = ProofDigest::from_bytes([0x61; 32]);
    let proof = ProofDigest::from_bytes([0x71; 32]);
    let envelope = envelope(request_id, body, proof);

    assert!(matches!(
        session.admit_body_bound(
            &envelope,
            &proof,
            &ProofDigest::from_bytes([0x62; 32]),
            1,
            MonotonicMillis::new(10),
        ),
        Err(ProtocolError::InvalidBody)
    ));
    assert_eq!(session.in_flight_len(), 0);

    let guard = session
        .admit_body_bound(
            &envelope,
            &proof,
            &body,
            1,
            MonotonicMillis::new(10),
        )
        .expect("same sequence remains admissible");
    assert_eq!(guard.request_id(), &request_id);
    assert!(session.is_request_in_flight(&request_id));
}

#[test]
fn guard_remains_owned_until_single_terminal_release() {
    let mut session = bound_session();
    let request_id = RequestId::from_bytes([0x52; 16]);
    let body = ProofDigest::from_bytes([0x63; 32]);
    let proof = ProofDigest::from_bytes([0x73; 32]);
    let envelope = envelope(request_id, body, proof);

    session
        .admit_body_bound(
            &envelope,
            &proof,
            &body,
            1,
            MonotonicMillis::new(20),
        )
        .expect("admit");
    assert_eq!(session.in_flight_len(), 1);
    assert_eq!(
        session.complete_request(&request_id, TerminalKind::Success),
        Ok(RequestStatus::Ok)
    );
    assert_eq!(session.in_flight_len(), 0);
    assert!(!session.is_request_in_flight(&request_id));
    assert_eq!(
        session.complete_request(&request_id, TerminalKind::Success),
        Err(ProtocolError::DuplicateTerminal)
    );
}

#[test]
fn cancelled_request_cannot_be_relabelled_success() {
    let mut session = bound_session();
    let request_id = RequestId::from_bytes([0x53; 16]);
    let body = ProofDigest::from_bytes([0x64; 32]);
    let proof = ProofDigest::from_bytes([0x74; 32]);
    let envelope = envelope(request_id, body, proof);

    session
        .admit_body_bound(
            &envelope,
            &proof,
            &body,
            1,
            MonotonicMillis::new(30),
        )
        .expect("admit");
    assert_eq!(
        session.cancel(&request_id),
        CancelOutcome::Cancelled { terminal: false }
    );
    assert_eq!(session.in_flight_len(), 0);
    assert_eq!(
        session.complete_request(&request_id, TerminalKind::Success),
        Err(ProtocolError::InvalidSessionTransition)
    );
    assert_eq!(
        session.complete_request(&request_id, TerminalKind::Cancelled),
        Ok(RequestStatus::Cancelled)
    );
    assert_eq!(
        session.complete_request(&request_id, TerminalKind::Cancelled),
        Err(ProtocolError::DuplicateTerminal)
    );
}

fn profile_request_body(request_id: RequestId, peer: TransportPeer) -> RequestBody {
    let recipe = RecipeIdV1::CorpusProfile;
    let workspace = WorkspaceId::from_bytes([0x56; 16]);
    let recipe_request = SearchRecipeRequest::new(
        request_id,
        recipe,
        SourceView::WorkingTreeCurrent(WorkspaceViewSource {
            workspace_instance_id: workspace,
            workspace_view_revision_ref: WorkspaceViewRevisionId::from_bytes([0x57; 16]),
        }),
        RequestedScope::ActiveWorkspace(workspace),
        ProfileId::new("interactive").expect("budget"),
        RecipeBodyV1::CorpusProfile(search_contracts::CorpusProfileRecipe {
            facets: BoundedSet::empty(),
        }),
    )
    .expect("valid profile request");
    RequestBody {
        grant: SearchReadGrantClaims {
            grant_id: GrantId::from_bytes([0x51; 16]),
            installation_id: InstallationId::from_bytes([0x52; 16]),
            installation_incarnation_id: peer.incarnation,
            binding_id: peer.binding,
            principal_opaque_id: OpaqueId::new("fixture-principal").expect("principal"),
            client_scope_ref: OpaqueRef::new("fixture-scope").expect("scope"),
            scope_domain_id: ScopeDomainId::from_bytes([0x53; 16]),
            allowed_membership_ids: BoundedSet::from_items([
                SourceMembershipId::from_bytes([0x54; 16]),
            ]).expect("membership"),
            allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
            reference_portfolio_revision: None,
            allowed_access_partitions: BoundedSet::from_items([
                AccessPartitionId::from_bytes([0x55; 16]),
            ]).expect("access partition"),
            allowed_modalities: BoundedSet::from_items([Modality::Text]).expect("modality"),
            permitted_recipe_families: BoundedSet::from_items([recipe]).expect("recipe"),
            maximum_budget_class: ProfileId::new("interactive").expect("budget"),
            sensitivity_ceiling: SensitivityClass::Project,
            disclosure_ceiling: DisclosureCeiling::LocalOnly,
            source_read_permission: false,
            exact_scan_permission: false,
            issued_boot_id: OpaqueId::new("fixture-boot").expect("boot"),
            issued_at: UtcTimestamp::parse("2099-01-01T00:00:00.000000Z").expect("issued"),
            expires_at: UtcTimestamp::parse("2099-01-01T00:00:30.000000Z").expect("expires"),
            nonce: OpaqueId::new("fixture-grant-nonce").expect("nonce"),
            revocation_generation: 0,
        },
        recipe_request,
    }
}

#[test]
fn prepared_provider_delivery_emits_exact_frame_and_reuses_preview_after_tamper() {
    let mut session = bound_session();
    let peer = peer();
    let request_id = RequestId::from_bytes([0x65; 16]);
    let version = version(1, 0);
    let versions = ProtocolRange::new(version, version).expect("version range");
    let request_envelope = ProviderEnvelope {
        protocol_major: version.major,
        protocol_minor: version.minor,
        installation_incarnation_id: peer.incarnation,
        binding_id: peer.binding,
        connection_sequence: 1,
        request_id,
        message_kind: MessageKind::Request,
        relative_deadline_ms: Some(1_000),
        body: ProviderBodyV1::Request(profile_request_body(request_id, peer)),
    };
    let request_frame = ClientEnvelopeCodec::encode(
        &request_envelope, DEFAULT_PROTOCOL_LIMITS, versions,
    ).expect("request frame");
    let proof = ProofDigest::from_bytes([0x75; 32]);
    let mut request_clock = || Ok(MonotonicMillis::new(10));
    let mut request = session.admit_provider_request(
        request_frame.as_slice(), &proof, 1_000, &mut request_clock, |_| Ok(proof),
    ).expect("admitted request");

    let event_body = |sequence| ProviderBodyV1::Progress(ProgressBody {
        event_sequence: sequence,
        phase: ProgressPhase::Planning,
        bounded_counts: BoundedProgressCounts {
            completed_legs: 0,
            total_planned_legs: 1,
            nominated_candidates: 0,
            validated_candidates: 0,
            omitted_or_failed_legs: 0,
        },
        degraded_reason_codes: BoundedSet::empty(),
    });
    let mut output_clock = || Ok(MonotonicMillis::new(11));
    let mut observed_sequences = Vec::new();
    let mut wrote_tampered = false;
    let tampered = session.deliver_provider_event_prepared(
        &mut request,
        None,
        &mut output_clock,
        |context| {
            observed_sequences.push((context.event_sequence(), context.connection_sequence()));
            let body = event_body(context.event_sequence());
            let mut envelope = context.envelope(body.clone());
            envelope.connection_sequence += 1;
            let frame = ServerEnvelopeCodec::encode(
                &envelope, context.limits(), context.supported_versions(),
            ).expect("tampered but well-formed frame");
            Ok::<_, &'static str>((body, frame))
        },
        |_| {
            wrote_tampered = true;
            Ok::<(), ()>(())
        },
    );
    assert!(matches!(
        tampered,
        Err(PreparedProviderDeliveryError::Protocol(ProtocolError::InvalidBody))
    ));
    assert!(!wrote_tampered);
    assert_eq!(request.next_event_sequence(), Some(1));
    assert_eq!(session.session.sequences().provider().next_expected(), Some(1));

    let expected_frame = RefCell::new(Vec::new());
    let recorded_frames = RefCell::new(Vec::new());
    session.deliver_provider_event_prepared(
        &mut request,
        None,
        &mut output_clock,
        |context| {
            observed_sequences.push((context.event_sequence(), context.connection_sequence()));
            let body = event_body(context.event_sequence());
            let envelope = context.envelope(body.clone());
            let frame = ServerEnvelopeCodec::encode(
                &envelope, context.limits(), context.supported_versions(),
            ).expect("canonical response frame");
            *expected_frame.borrow_mut() = frame.as_slice().to_vec();
            Ok::<_, &'static str>((body, frame))
        },
        |transcript| {
            assert_eq!(transcript.parts()[0], b"ELIOT-PROVIDER-FRAME-RESP-v1\0");
            assert_eq!(transcript.frame(), expected_frame.borrow().as_slice());
            recorded_frames.borrow_mut().push(transcript.frame().to_vec());
            Ok::<(), ()>(())
        },
    ).expect("exact prepared frame delivered");

    assert_eq!(observed_sequences, [(1, 1), (1, 1)]);
    assert_eq!(recorded_frames.borrow().len(), 1);
    let decoded = ServerEnvelopeCodec::decode(
        &recorded_frames.borrow()[0], DEFAULT_PROTOCOL_LIMITS, versions,
    ).expect("recorded frame decodes");
    assert_eq!(decoded.connection_sequence, 1);
    assert_eq!(decoded.request_id, request_id);
    assert!(matches!(decoded.body, ProviderBodyV1::Progress(ref body) if body.event_sequence == 1));
    assert_eq!(request.next_event_sequence(), Some(2));
    assert_eq!(session.session.sequences().provider().next_expected(), Some(2));

    let terminal_body = ProviderBodyV1::Error(ErrorBody {
        code: ProtocolFailureCode::Search(SearchReasonCodeV1::SecurityFailClosed),
        retryability: ProtocolRetryability::Never,
        message_template_id: OpaqueId::new("fixture-denial").expect("template"),
        bounded_metadata: BoundedNonContentMetadata::empty(),
    });
    let terminal_frame = RefCell::new(Vec::new());
    session.deliver_provider_event_prepared(
        &mut request,
        Some(TerminalKind::Failed),
        &mut output_clock,
        |context| {
            assert_eq!((context.event_sequence(), context.connection_sequence()), (2, 2));
            let envelope = context.envelope(terminal_body.clone());
            let frame = ServerEnvelopeCodec::encode(
                &envelope, context.limits(), context.supported_versions(),
            ).expect("terminal frame");
            *terminal_frame.borrow_mut() = frame.as_slice().to_vec();
            Ok::<_, &'static str>((terminal_body.clone(), frame))
        },
        |transcript| {
            assert_eq!(transcript.frame(), terminal_frame.borrow().as_slice());
            recorded_frames.borrow_mut().push(transcript.frame().to_vec());
            Ok::<(), ()>(())
        },
    ).expect("terminal frame delivered");
    assert_eq!(request.next_event_sequence(), None);
    assert_eq!(session.session.sequences().provider().next_expected(), Some(3));
    assert_eq!(session.in_flight_len(), 0);
    assert_eq!(
        session.complete_request(&request_id, TerminalKind::Failed),
        Err(ProtocolError::DuplicateTerminal)
    );

    let failed_request_id = RequestId::from_bytes([0x66; 16]);
    let mut second_request_envelope = request_envelope;
    second_request_envelope.connection_sequence = 2;
    second_request_envelope.request_id = failed_request_id;
    second_request_envelope.body = ProviderBodyV1::Request(
        profile_request_body(failed_request_id, peer),
    );
    let second_request_frame = ClientEnvelopeCodec::encode(
        &second_request_envelope, DEFAULT_PROTOCOL_LIMITS, versions,
    ).expect("second request frame");
    let mut second_request_clock = || Ok(MonotonicMillis::new(12));
    let mut failed_request = session.admit_provider_request(
        second_request_frame.as_slice(), &proof, 1_000, &mut second_request_clock, |_| Ok(proof),
    ).expect("second request admitted");
    let mut failed_output_clock = || Ok(MonotonicMillis::new(13));
    let ambiguous_frame = RefCell::new(Vec::new());
    let failed_output = session.deliver_provider_event_prepared(
        &mut failed_request,
        None,
        &mut failed_output_clock,
        |context| {
            assert_eq!((context.event_sequence(), context.connection_sequence()), (1, 3));
            let body = event_body(context.event_sequence());
            let envelope = context.envelope(body.clone());
            let frame = ServerEnvelopeCodec::encode(
                &envelope, context.limits(), context.supported_versions(),
            ).expect("frame before ambiguous write");
            *ambiguous_frame.borrow_mut() = frame.as_slice().to_vec();
            Ok::<_, &'static str>((body, frame))
        },
        |transcript| {
            // The recording seam models an ambiguous transport failure after
            // bytes have been offered; the protocol must poison the session.
            assert_eq!(transcript.frame(), ambiguous_frame.borrow().as_slice());
            Err::<(), _>("output outcome unknown")
        },
    );
    assert!(matches!(
        failed_output,
        Err(PreparedProviderDeliveryError::Output("output outcome unknown"))
    ));
    assert_eq!(session.session_state(), SessionState::Closed);
    assert_eq!(failed_request.next_event_sequence(), Some(1));
    assert_eq!(session.session.sequences().provider().next_expected(), Some(3));
}
