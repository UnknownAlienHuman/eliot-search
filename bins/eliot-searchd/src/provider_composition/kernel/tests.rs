use super::*;
use search_contracts::{ProtocolRange, ProtocolVersion, RequestId};
use search_provider_protocol::request::{
    AuthenticatedEnvelope, ControlCommand as Command, MonotonicMillis as At, RequestStatus,
    envelope_transcript, seal_envelope,
};
use search_provider_protocol::{
    CancelOutcome, DEFAULT_PROTOCOL_LIMITS, ProofDigest, ProtocolError, ProtocolLimits,
    ServerNonce, TerminalKind,
};

const KEY: [u8; 32] = [0x2A; 32];
const OTHER_KEY: [u8; 32] = [0x3B; 32];

fn nonce(counter: u64) -> ServerNonce {
    derive_server_nonce(&KEY, counter).expect("nonce")
}

fn envelope(command: Command, request: [u8; 16], nonce: &ServerNonce) -> AuthenticatedEnvelope {
    let digest = ProofDigest::from_bytes([0x33; 32]);
    let stub = seal_envelope(
        PROVIDER_PROTOCOL_VERSION,
        *nonce,
        RequestId::from_bytes(request),
        command,
        digest,
        ProofDigest::from_bytes([0; 32]),
    );
    let proof = ProofDigest::from_bytes(
        *blake3::keyed_hash(&KEY, &envelope_transcript(&stub)).as_bytes(),
    );
    seal_envelope(
        PROVIDER_PROTOCOL_VERSION,
        *nonce,
        RequestId::from_bytes(request),
        command,
        digest,
        proof,
    )
}

fn router_with_nonce(counter: u64) -> (ProviderRouter, ServerNonce) {
    let nonce = nonce(counter);
    let router = ProviderRouter::open(
        &KEY,
        PROVIDER_PROTOCOL_VERSION,
        nonce,
        DEFAULT_PROTOCOL_LIMITS,
    )
    .expect("open");
    (router, nonce)
}

#[test]
fn registries_are_closed() {
    assert_eq!(ProviderOperation::ALL.len(), 8);
    assert_eq!(
        ProviderOperation::parse("reboot"),
        Err(PROVIDER_UNKNOWN_COMMAND)
    );
    for operation in ProviderOperation::ALL {
        assert_eq!(ProviderOperation::parse(operation.as_str()), Ok(*operation));
    }
    assert!(ProviderOperation::Health.requires_envelope());
    assert!(ProviderOperation::Shutdown.requires_envelope());
    assert!(!ProviderOperation::Status.requires_envelope());
    assert!(!ProviderOperation::Query.requires_envelope());
}

#[test]
fn hello_negotiates_exact_version_and_rejects_major_mismatch() {
    let range = ProtocolRange::new(PROVIDER_PROTOCOL_VERSION, PROVIDER_PROTOCOL_VERSION)
        .expect("range");
    assert_eq!(
        negotiate_connection_version(range),
        Ok(PROVIDER_PROTOCOL_VERSION)
    );
    let foreign = ProtocolRange::new(
        ProtocolVersion { major: 2, minor: 0 },
        ProtocolVersion { major: 2, minor: 0 },
    )
    .expect("range");
    assert_eq!(
        negotiate_connection_version(foreign),
        Err(ProtocolError::NoCompatibleVersion)
    );
    assert_eq!(
        parse_client_range("1.0-1.0").expect("range"),
        PROVIDER_PROTOCOL_RANGE
    );
    assert!(parse_client_range("2.0-2.0").is_ok());
    assert!(parse_client_range("1.1-1.0").is_err());
    assert!(parse_client_range("hello").is_err());
    assert!(parse_client_range("01.0-1.0").is_err());
}

#[test]
fn open_rejects_zero_key_and_foreign_version() {
    assert!(matches!(
        ProviderRouter::open(
            &[0; 32],
            PROVIDER_PROTOCOL_VERSION,
            nonce(1),
            DEFAULT_PROTOCOL_LIMITS,
        ),
        Err(ProtocolError::InvalidBindingKey)
    ));
    assert!(matches!(
        ProviderRouter::open(
            &KEY,
            ProtocolVersion { major: 2, minor: 0 },
            ServerNonce::from_bytes([1; 16]).expect("nonce"),
            DEFAULT_PROTOCOL_LIMITS,
        ),
        Err(ProtocolError::NoCompatibleVersion)
    ));
}

#[test]
fn admit_checks_version_nonce_proof_sequence_replay_and_ceiling() {
    let (mut router, nonce) = router_with_nonce(11);
    let now = At::new(1000);
    let first = envelope(Command::Health, [1; 16], &nonce);
    router.admit(&first, &KEY, 1, now, None).expect("admit");
    assert_eq!(router.in_flight_len(), 1);

    let second = envelope(Command::Health, [2; 16], &nonce);
    assert_eq!(
        router
            .admit(&second, &OTHER_KEY, 2, now, None)
            .expect_err("proof"),
        ProtocolError::AuthenticationFailed
    );

    let mut foreign_nonce = *nonce.as_bytes();
    foreign_nonce[0] ^= 0xFF;
    let foreign = ServerNonce::from_bytes(foreign_nonce).expect("nonce");
    let third = envelope(Command::Health, [3; 16], &foreign);
    assert_eq!(
        router.admit(&third, &KEY, 2, now, None).expect_err("nonce"),
        ProtocolError::AuthenticationFailed
    );

    assert_eq!(
        router
            .admit(&first, &KEY, 2, now, None)
            .expect_err("replay"),
        ProtocolError::ReplayDetected
    );

    let fourth = envelope(Command::Version, [4; 16], &nonce);
    assert_eq!(
        router.admit(&fourth, &KEY, 9, now, None).expect_err("gap"),
        ProtocolError::SequenceGap
    );

    let fifth = envelope(Command::Version, [5; 16], &nonce);
    assert_eq!(
        router
            .admit(&fifth, &KEY, 2, now, None)
            .expect_err("duplicate"),
        ProtocolError::DuplicateSequence
    );
    assert_eq!(
        router
            .admit(&fourth, &KEY, 1, now, None)
            .expect_err("regression"),
        ProtocolError::SequenceRegression
    );
    assert_eq!(
        router
            .admit(&fourth, &KEY, 2, now, Some(0))
            .expect_err("deadline"),
        ProtocolError::DeadlineExpired
    );
    assert_eq!(router.in_flight_len(), 1);
}

#[test]
fn terminal_is_unique_ordered_and_releases_exactly_once() {
    let (mut router, nonce) = router_with_nonce(21);
    let now = At::new(50);
    let first = envelope(Command::Health, [0xA1; 16], &nonce);
    let second = envelope(Command::Version, [0xA2; 16], &nonce);
    router.admit(&first, &KEY, 1, now, None).expect("first");
    router.admit(&second, &KEY, 2, now, None).expect("second");
    assert_eq!(
        router.note_terminal(second.request_id(), TerminalKind::Success),
        Err(ProtocolError::SequenceGap)
    );
    assert_eq!(router.in_flight_len(), 2);
    assert_eq!(
        router.note_terminal(first.request_id(), TerminalKind::Success),
        Ok((RequestStatus::Ok, 1))
    );
    assert_eq!(
        router.note_terminal(second.request_id(), TerminalKind::Failed),
        Ok((RequestStatus::Failed, 2))
    );
    assert_eq!(router.in_flight_len(), 0);
    assert_eq!(
        router.note_terminal(first.request_id(), TerminalKind::Success),
        Err(ProtocolError::DuplicateTerminal)
    );
    assert_eq!(
        router.note_terminal(&RequestId::from_bytes([0xFF; 16]), TerminalKind::Failed),
        Err(ProtocolError::InvalidSessionTransition)
    );
}

#[test]
fn in_flight_ceiling_fails_closed_at_canonical_bound() {
    let mut router = ProviderRouter::open(
        &KEY,
        PROVIDER_PROTOCOL_VERSION,
        nonce(31),
        ProtocolLimits {
            max_in_flight_requests: 2,
            ..DEFAULT_PROTOCOL_LIMITS
        },
    )
    .expect("open");
    let now = At::new(7);
    let nonce = *router.server_nonce();
    for (index, sequence) in [(0xB1_u8, 1_u64), (0xB2, 2)] {
        let env = envelope(Command::Health, [index; 16], &nonce);
        router
            .admit(&env, &KEY, sequence, now, None)
            .expect("admit");
    }
    let overflow = envelope(Command::Health, [0xB3; 16], &nonce);
    assert_eq!(
        router
            .admit(&overflow, &KEY, 3, now, None)
            .expect_err("ceiling"),
        ProtocolError::ResourceExhausted
    );
}

#[test]
fn cancel_is_idempotent_and_releases_once() {
    let (mut router, nonce) = router_with_nonce(41);
    let now = At::new(9);
    let env = envelope(Command::Health, [0xC1; 16], &nonce);
    router.admit(&env, &KEY, 1, now, None).expect("admit");
    assert!(matches!(
        router.cancel(env.request_id()),
        CancelOutcome::Cancelled { terminal: false }
    ));
    assert_eq!(router.in_flight_len(), 0);
    assert_eq!(
        router.cancel(env.request_id()),
        CancelOutcome::UnknownOrTerminal
    );
    assert_eq!(
        router.cancel(&RequestId::from_bytes([9; 16])),
        CancelOutcome::UnknownOrTerminal
    );
}

#[test]
fn capabilities_gate_recipes_but_never_the_shell() {
    let shell = CapabilityEvidence::from_parts(
        false,
        false,
        false,
        vec!["SEARCH_NOT_ACCEPTED", "INDEXED_NOT_ACCEPTED"],
    )
    .expect("shell evidence");
    let caps = negotiate_capabilities(&shell);
    assert!(caps.health_available);
    assert!(caps.status_available);
    assert!(caps.cancel_available);
    assert!(!caps.query_available);
    assert!(!caps.ingest_available);
    assert!(!caps.expand_available);
    for operation in [
        ProviderOperation::Health,
        ProviderOperation::Status,
        ProviderOperation::Version,
        ProviderOperation::Cancel,
        ProviderOperation::Shutdown,
    ] {
        assert_eq!(gate_operation(operation, &caps), Ok(()));
    }
    for (operation, reason) in [
        (ProviderOperation::Ingest, PROVIDER_INGEST_UNAVAILABLE),
        (ProviderOperation::Query, PROVIDER_QUERY_UNAVAILABLE),
        (ProviderOperation::Expand, PROVIDER_EXPAND_UNAVAILABLE),
    ] {
        let denial = gate_operation(operation, &caps).expect_err("gated");
        assert_eq!(denial.reason, reason);
        assert!(!denial.blockers.is_empty());
    }
    let open = CapabilityEvidence::from_parts(true, true, false, vec![]).expect("evidence");
    let caps = negotiate_capabilities(&open);
    assert!(caps.query_available);
    assert_eq!(gate_operation(ProviderOperation::Query, &caps), Ok(()));
}

#[test]
fn response_seal_binds_receipt_without_relabeling() {
    let nonce = nonce(51);
    let id = RequestId::from_bytes([0xD1; 16]);
    let response = seal_response_with_receipt(
        &KEY,
        PROVIDER_PROTOCOL_VERSION,
        nonce,
        id,
        RequestStatus::Ok,
        4,
    );
    assert_eq!(response.version(), PROVIDER_PROTOCOL_VERSION);
    assert_eq!(*response.request_id(), id);
    assert_eq!(response.status(), RequestStatus::Ok);
    let tampered_receipt =
        render_response_receipt(PROVIDER_PROTOCOL_VERSION, &id, RequestStatus::Ok, 5);
    let tampered_digest = ProofDigest::from_bytes(*blake3::hash(&tampered_receipt).as_bytes());
    assert_ne!(*response.body_digest(), tampered_digest);
    let other = seal_response_with_receipt(
        &OTHER_KEY,
        PROVIDER_PROTOCOL_VERSION,
        nonce,
        id,
        RequestStatus::Ok,
        4,
    );
    assert_ne!(response.proof(), other.proof());
    let frame = encode_response_frame(&response).expect("encode");
    let decoded = search_provider_protocol::decode_response(
        frame.as_slice(),
        DEFAULT_PROTOCOL_LIMITS,
        PROVIDER_PROTOCOL_RANGE,
    )
    .expect("decode");
    assert_eq!(decoded, response);
}

#[test]
fn envelope_frames_round_trip_with_prefix_validation() {
    let nonce = nonce(61);
    let sealed = envelope(Command::Shutdown, [0xE1; 16], &nonce);
    let frame = search_provider_protocol::request::encode_envelope(
        &sealed,
        DEFAULT_PROTOCOL_LIMITS,
    )
    .expect("encode");
    let frame = frame.as_slice().to_vec();
    assert_eq!(
        usize::try_from(u32::from_le_bytes(frame[..4].try_into().expect("prefix")))
            .expect("prefix")
            + 4,
        frame.len()
    );
    assert_eq!(decode_envelope_frame(&frame).expect("decode"), sealed);
    assert!(decode_envelope_frame(&frame[..frame.len() - 1]).is_err());
    let mut oversize = vec![0xFF; DEFAULT_PROTOCOL_LIMITS.max_frame_bytes + 1];
    oversize[0..4].copy_from_slice(&10_u32.to_le_bytes());
    assert_eq!(
        decode_envelope_frame(&oversize),
        Err(ProtocolError::FrameTooLarge)
    );
}

#[test]
fn line_parsing_is_strict_and_bounded() {
    assert!(parse_op_line("health").is_err());
    assert_eq!(parse_op_line("op\thealth"), Err(PROVIDER_ENVELOPE_REQUIRED));
    assert!(parse_op_line("op\tstatus").is_ok());
    assert!(parse_op_line("op\tstatus\textra").is_err());
    assert!(parse_op_line("op\treboot").is_err());
    assert!(parse_op_line("op\tcancel").is_err());
    let (op, arg) =
        parse_op_line("op\tcancel\t00112233445566778899aabbccddeeff").expect("cancel");
    assert_eq!(op, ProviderOperation::Cancel);
    assert!(matches!(arg, OpArgument::CancelTarget(_)));
    assert!(parse_op_line("op\tcancel\tZZ").is_err());
    assert!(parse_op_line("op\tquery").is_err());
    assert!(parse_op_line("op\tquery\tABCD").is_err());
    assert!(parse_op_line("op\tquery\tab").is_ok());
    let huge = format!("op\tquery\t{}", "ab".repeat(MAX_OP_ARG_HEX));
    assert_eq!(
        parse_op_line(&huge).expect_err("bounded"),
        protocol_reason(ProtocolError::FrameTooLarge)
    );
    assert_eq!(parse_hello_line("op\thello"), Ok(None));
    assert!(parse_hello_line("op\thello\t1.0-1.0").is_ok());
    assert!(parse_hello_line("op\thello\tnope").is_err());
    assert!(parse_hello_line("op\tstatus").is_err());
    let (sequence, _) = parse_envelope_line("envelope\t12\tabcd").expect("line");
    assert_eq!(sequence, 12);
    assert!(parse_envelope_line("envelope\tabcd").is_err());
    assert!(parse_envelope_line("envelope\t01\tabcd").is_err());
    assert!(parse_envelope_line("envelope\t1\tAB").is_err());
}

#[test]
fn nonce_draws_are_fresh() {
    let first = derive_server_nonce(&KEY, 1).expect("nonce");
    let second = derive_server_nonce(&KEY, 2).expect("nonce");
    assert_ne!(first, second);
}

#[test]
fn child_reply_mapping_never_relabels_the_unknown() {
    assert_eq!(
        status_for_reply(ChildReply::Complete),
        (RequestStatus::Ok, TerminalKind::Success)
    );
    assert_eq!(
        status_for_reply(ChildReply::Rejected),
        (RequestStatus::Failed, TerminalKind::Failed)
    );
    assert_eq!(
        status_for_reply(ChildReply::Shutdown),
        (RequestStatus::Ok, TerminalKind::Success)
    );
    assert_eq!(
        status_for_reply(ChildReply::Fatal),
        (RequestStatus::OutcomeUnknown, TerminalKind::OutcomeUnknown)
    );
    assert_eq!(child_command_for_envelope(Command::Health), "health");
    assert_eq!(child_command_for_envelope(Command::Shutdown), "shutdown");
}

#[test]
fn rendered_lines_are_bounded_json_with_blockers() {
    let shell = CapabilityEvidence::from_parts(
        false,
        false,
        false,
        vec!["SEARCH_NOT_ACCEPTED", "INDEXED_NOT_ACCEPTED"],
    )
    .expect("shell evidence");
    let caps = negotiate_capabilities(&shell);
    let hello = render_hello(PROVIDER_PROTOCOL_VERSION, &nonce(71), &caps, 0).expect("hello");
    assert!(hello.contains("\"event\":\"provider_hello\""));
    assert!(hello.contains("\"version\":\"1.0\""));
    assert!(hello.contains("SEARCH_NOT_ACCEPTED"));
    let denied = gate_operation(ProviderOperation::Query, &caps).expect_err("denied");
    let line = render_op_response(
        ProviderOperation::Query,
        OpStatus::Unavailable,
        denied.reason,
        &denied.blockers,
    )
    .expect("render");
    assert!(line.contains(PROVIDER_QUERY_UNAVAILABLE));
    assert!(line.contains("\"status\":\"unavailable\""));
    assert!(!line.contains("\"status\":\"ok\""));
}

#[test]
fn shim_key_derivation_rejects_short_and_empty_material() {
    assert!(shim_key_from_bytes(&[]).is_err());
    assert!(shim_key_from_bytes(&[0x41; 31]).is_err());
    let first = shim_key_from_bytes(&[0x41; 32]).expect("key");
    let second = shim_key_from_bytes(&[0x41; 32]).expect("key");
    assert_eq!(first, second);
    assert_ne!(first, shim_key_from_bytes(&[0x42; 32]).expect("key"));
    assert_ne!(first, [0; 32]);
}

#[test]
fn disconnect_reports_exact_counts_and_closes() {
    let (mut router, nonce) = router_with_nonce(81);
    let now = At::new(3);
    for index in [0xF1_u8, 0xF2] {
        let env = envelope(Command::Health, [index; 16], &nonce);
        router
            .admit(&env, &KEY, u64::from(index - 0xF0), now, None)
            .expect("admit");
    }
    let receipt = router.disconnect();
    assert_eq!(receipt.cancelled_requests(), 2);
    assert_eq!(receipt.released_guards(), 2);
    assert!(!router.is_active());
    let env = envelope(Command::Health, [0xF3; 16], &nonce);
    assert!(router.admit(&env, &KEY, 3, now, None).is_err());
    let again = router.disconnect();
    assert_eq!(again.cancelled_requests(), 0);
}

#[test]
fn gaps_empty_and_sync_block_proof_in_fixed_order() {
    assert!(!evaluate_current_workspace_proven(0, 0, true, true));
    assert_eq!(
        current_workspace_proven_reason(0, 0, true, true),
        ROOTS_EMPTY_BLOCKS_CURRENTNESS
    );
    assert!(!evaluate_current_workspace_proven(1, 1, true, true));
    assert_eq!(
        current_workspace_proven_reason(1, 1, true, true),
        ROOTS_GAP_BLOCKS_CURRENTNESS
    );
    assert!(!evaluate_current_workspace_proven(1, 0, false, true));
    assert_eq!(
        current_workspace_proven_reason(1, 0, false, true),
        ROOTS_SYNC_INCOMPLETE_BLOCKS_CURRENTNESS
    );
    assert!(!evaluate_current_workspace_proven(1, 0, true, false));
    assert_eq!(
        current_workspace_proven_reason(1, 0, true, false),
        ROOTS_INDEX_UNAVAILABLE_BLOCKS_CURRENTNESS
    );
    assert!(evaluate_current_workspace_proven(2, 0, true, true));
    assert_eq!(
        current_workspace_proven_reason(2, 0, true, true),
        ROOTS_CURRENT_WORKSPACE_PROVEN
    );
}
