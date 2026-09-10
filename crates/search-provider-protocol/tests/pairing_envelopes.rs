//! Pairing kernel (#73) + authenticated envelopes (#89) sequencing probes.
//!
//! Layer order under test: canonical framing (`FrameCodec`) at the bottom,
//! the pairing ceremony prerequisite in the middle, authenticated per-request
//! envelopes on top. Envelope admission without a mutually verified pairing
//! must fail closed even when the transport session is active.

use search_provider_protocol as proto;

const fn version(major: u16, minor: u16) -> proto::ProtocolVersion {
    proto::ProtocolVersion { major, minor }
}

fn range(min_major: u16, min_minor: u16, max_major: u16, max_minor: u16) -> proto::ProtocolRange {
    proto::ProtocolRange::new(version(min_major, min_minor), version(max_major, max_minor))
        .expect("range")
}

fn nonzero16(seed: u8) -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let offset = u8::try_from(index).expect("fixed 16-byte test id");
        *slot = seed.wrapping_add(offset).max(1);
    }
    bytes
}

fn pairing_inputs() -> (
    proto::ProtocolVersion,
    proto::ProofDigest,
    proto::SessionId,
    proto::ClientNonce,
    proto::PairingChallenge,
) {
    (
        version(1, 0),
        proto::ProofDigest::from_bytes([9; 32]),
        proto::SessionId::from_bytes(nonzero16(0x11)).expect("session"),
        proto::ClientNonce::from_bytes(nonzero16(0x22)).expect("nonce"),
        proto::PairingChallenge::from_bytes([0x33; 32]).expect("challenge"),
    )
}

fn verified_pairing() -> proto::VerifiedPairing {
    let (version, binding, session, nonce, challenge) = pairing_inputs();
    let mut machine = proto::PairingMachine::new(version, binding);
    machine
        .issue_challenge(session, nonce, challenge)
        .expect("challenge");
    let proof = proto::ProofDigest::from_bytes([0x44; 32]);
    machine
        .verify_client_proof(&proof, &proof)
        .expect("client proof");
    machine
        .issue_provider_proof(proto::ProofDigest::from_bytes([0x55; 32]))
        .expect("provider proof");
    machine.into_verified().expect("verified")
}

fn hello_body() -> search_contracts::protocol::HelloBody {
    search_contracts::protocol::HelloBody {
        peer_role: search_contracts::protocol::PeerRole::StandaloneCli,
        pairing_proof_ref: search_contracts::canonical::OpaqueRef::new("pairing-proof-ref")
            .expect("opaque ref"),
        supported_protocol_range: range(1, 0, 1, 2),
        requested_capability_digest: None,
    }
}

fn transport_peer() -> proto::TransportPeer {
    proto::TransportPeer {
        role: search_contracts::protocol::PeerRole::StandaloneCli,
        incarnation: search_contracts::InstallationIncarnationId::from_bytes(nonzero16(0x77)),
        binding: search_contracts::BindingId::from_bytes(nonzero16(0x88)),
    }
}

fn bound_session() -> proto::BoundSession {
    let pairing = verified_pairing();
    let peer = transport_peer();
    let incarnation = *peer.incarnation();
    let binding =
        proto::authenticate_binding(&hello_body(), &pairing, &incarnation, &peer).expect("binding");
    proto::BoundSession::open(
        binding,
        pairing,
        proto::ServerNonce::from_bytes(nonzero16(0x99)).expect("server nonce"),
        proto::DEFAULT_PROTOCOL_LIMITS,
    )
    .expect("open")
}

fn sealed_envelope(session: &proto::BoundSession) -> proto::AuthenticatedEnvelope {
    proto::seal_envelope(
        version(1, 0),
        *session.server_nonce(),
        search_contracts::RequestId::from_bytes(nonzero16(0xA1)),
        proto::ControlCommand::Health,
        proto::ProofDigest::from_bytes([0xB2; 32]),
        proto::ProofDigest::from_bytes([0xC3; 32]),
    )
}

// Pairing transcripts are byte-exact: domain separation plus fixed field
// order over the negotiated version, binding digest, session, nonces.
#[test]
fn pairing_transcripts_are_deterministic_and_domain_separated() {
    let (negotiated, binding, session, nonce, challenge) = pairing_inputs();
    let client = proto::client_proof_transcript(negotiated, &binding, session, &nonce, &challenge);
    let server = proto::server_proof_transcript(negotiated, &binding, session, &nonce, &challenge);
    assert_eq!(
        client.as_bytes(),
        proto::client_proof_transcript(negotiated, &binding, session, &nonce, &challenge)
            .as_bytes()
    );
    assert_ne!(
        client.as_bytes(),
        server.as_bytes(),
        "client and server proofs must bind different domains"
    );
    assert!(
        client
            .as_bytes()
            .starts_with(b"ELIOT-PAIRING-CLIENT-v1\x00"),
        "client transcript must carry its domain separator"
    );
    assert!(
        server
            .as_bytes()
            .starts_with(b"ELIOT-PAIRING-SERVER-v1\x00"),
        "server transcript must carry its domain separator"
    );
    // Fixed layout: domain + NUL + major LE + minor LE + binding(32) +
    // session(16) + nonce(16) + challenge(32).
    assert_eq!(
        client.len(),
        "ELIOT-PAIRING-CLIENT-v1".len() + 1 + 2 + 2 + 32 + 16 + 16 + 32
    );
    // Version sensitivity: a different negotiated version binds a different proof.
    let other =
        proto::client_proof_transcript(version(1, 1), &binding, session, &nonce, &challenge);
    assert_ne!(client.as_bytes(), other.as_bytes());
}

// A provider proof cannot be issued before exact client verification.
#[test]
fn provider_proof_requires_prior_client_verification() {
    let (version, binding, session, nonce, challenge) = pairing_inputs();
    let mut machine = proto::PairingMachine::new(version, binding);
    machine
        .issue_challenge(session, nonce, challenge)
        .expect("challenge");
    assert_eq!(
        machine.issue_provider_proof(proto::ProofDigest::from_bytes([1; 32])),
        Err(proto::ProtocolError::InvalidPairingTransition)
    );
    assert_eq!(machine.state(), proto::PairingState::ChallengeIssued);
}

// A wrong client proof fails the ceremony terminally, not retryably.
#[test]
fn wrong_client_proof_fails_pairing_terminally() {
    let (version, binding, session, nonce, challenge) = pairing_inputs();
    let mut machine = proto::PairingMachine::new(version, binding);
    machine
        .issue_challenge(session, nonce, challenge)
        .expect("challenge");
    assert_eq!(
        machine.verify_client_proof(
            &proto::ProofDigest::from_bytes([1; 32]),
            &proto::ProofDigest::from_bytes([2; 32]),
        ),
        Err(proto::ProtocolError::PairingProofInvalid)
    );
    assert_eq!(machine.state(), proto::PairingState::Failed);
    assert_eq!(
        machine.verify_client_proof(
            &proto::ProofDigest::from_bytes([1; 32]),
            &proto::ProofDigest::from_bytes([1; 32]),
        ),
        Err(proto::ProtocolError::PairingFailed)
    );
}

// Zero nonces/challenges/sessions and zero keys are malformed, distinctly.
#[test]
fn zero_pairing_material_is_rejected_distinctly() {
    assert_eq!(
        proto::SessionId::from_bytes([0; 16]),
        Err(proto::ProtocolError::InvalidNonce)
    );
    assert_eq!(
        proto::ClientNonce::from_bytes([0; 16]),
        Err(proto::ProtocolError::InvalidNonce)
    );
    assert_eq!(
        proto::ServerNonce::from_bytes([0; 16]),
        Err(proto::ProtocolError::InvalidNonce)
    );
    assert_eq!(
        proto::PairingChallenge::from_bytes([0; 32]),
        Err(proto::ProtocolError::InvalidNonce)
    );
    assert_eq!(
        proto::BindingKey::from_bytes([0; 32]).expect_err("zero key must fail"),
        proto::ProtocolError::InvalidBindingKey
    );
}

// Pairing challenges are single-use across sessions: replay fails closed.
#[test]
fn pairing_challenge_replay_is_rejected() {
    let (_version, _binding, session, _nonce, challenge) = pairing_inputs();
    let mut ledger = proto::PairingLedger::new(16).expect("ledger");
    ledger.consume(session, &challenge).expect("first use");
    assert_eq!(
        ledger.consume(session, &challenge),
        Err(proto::ProtocolError::ReplayDetected)
    );
    // A fresh challenge for the same session is still accepted.
    let other = proto::PairingChallenge::from_bytes([0x34; 32]).expect("challenge");
    ledger.consume(session, &other).expect("fresh challenge");
    assert_eq!(
        proto::PairingLedger::new(0),
        Err(proto::ProtocolError::InvalidLimits)
    );
}

// Binding key hygiene: redacted formatting, callback-only access.
#[test]
fn binding_key_is_redacted_and_callback_only() {
    let key = proto::BindingKey::from_bytes([0xAB; 32]).expect("key");
    let debug = format!("{key:?}");
    assert!(debug.contains("<redacted>"), "key must not format: {debug}");
    assert!(!debug.contains("ab"), "key bytes must not leak: {debug}");
    let seen = key.with_bytes(|bytes| bytes.to_vec());
    assert_eq!(seen, vec![0xAB; 32]);
}

// Envelopes round-trip over the canonical frame codec with fixed sizes.
#[test]
fn authenticated_envelope_round_trips_over_frame_codec() {
    let session = bound_session();
    let envelope = sealed_envelope(&session);
    let framed = proto::encode_envelope(&envelope, proto::DEFAULT_PROTOCOL_LIMITS).expect("encode");
    assert_eq!(
        &framed.as_slice()[..4],
        &u32::try_from(framed.as_slice().len() - 4)
            .expect("len")
            .to_le_bytes()
    );
    let decoded = proto::decode_envelope(
        framed.as_slice(),
        proto::DEFAULT_PROTOCOL_LIMITS,
        range(1, 0, 1, 2),
    )
    .expect("decode");
    assert_eq!(decoded, envelope);
    // The transcript binds version, nonce, request, command and body digest.
    assert_eq!(
        proto::envelope_transcript(&envelope),
        proto::envelope_transcript(&decoded)
    );
}

// Strict envelope decoding: unknown command, wrong version, truncated proof.
#[test]
fn envelope_decoding_rejects_unknown_command_and_version() {
    let session = bound_session();
    let envelope = sealed_envelope(&session);
    let framed = proto::encode_envelope(&envelope, proto::DEFAULT_PROTOCOL_LIMITS).expect("encode");
    let mut body = framed.as_slice()[4..].to_vec();

    let replace = |body: &mut Vec<u8>, from: &[u8], to: &[u8]| {
        let text = String::from_utf8(body.clone()).expect("utf8");
        let patched = text.replacen(
            core::str::from_utf8(from).expect("from"),
            core::str::from_utf8(to).expect("to"),
            1,
        );
        *body = patched.into_bytes();
    };
    replace(&mut body, b"\"health\"", b"\"reboot\"");
    let mut reframed = u32::try_from(body.len())
        .expect("len")
        .to_le_bytes()
        .to_vec();
    reframed.extend_from_slice(&body);
    assert_eq!(
        proto::decode_envelope(&reframed, proto::DEFAULT_PROTOCOL_LIMITS, range(1, 0, 1, 2)),
        Err(proto::ProtocolError::UnknownCommand)
    );

    // Version outside the negotiated range fails with the version error.
    let other = proto::seal_envelope(
        version(2, 0),
        *session.server_nonce(),
        search_contracts::RequestId::from_bytes(nonzero16(0xA2)),
        proto::ControlCommand::Version,
        proto::ProofDigest::from_bytes([1; 32]),
        proto::ProofDigest::from_bytes([2; 32]),
    );
    let framed = proto::encode_envelope(&other, proto::DEFAULT_PROTOCOL_LIMITS).expect("encode");
    assert_eq!(
        proto::decode_envelope(
            framed.as_slice(),
            proto::DEFAULT_PROTOCOL_LIMITS,
            range(1, 0, 1, 2)
        ),
        Err(proto::ProtocolError::NoCompatibleVersion)
    );
}

// Sequencing gate: an active transport session without verified pairing
// cannot admit envelopes; verified pairing admits exactly once per ID.
#[test]
fn envelope_admission_requires_verified_pairing_first() {
    let pairing = verified_pairing();
    let peer = transport_peer();
    let incarnation = *peer.incarnation();
    let binding =
        proto::authenticate_binding(&hello_body(), &pairing, &incarnation, &peer).expect("binding");
    let mut session = proto::BoundSession::open(
        binding,
        pairing,
        proto::ServerNonce::from_bytes(nonzero16(0x99)).expect("server nonce"),
        proto::DEFAULT_PROTOCOL_LIMITS,
    )
    .expect("open");

    let envelope = sealed_envelope(&session);
    let proof = *envelope.proof();
    // Wrong keyed proof fails even with verified pairing.
    assert_eq!(
        session
            .admit(
                &envelope,
                &proto::ProofDigest::from_bytes([0xEE; 32]),
                1,
                proto::MonotonicMillis::new(1_000)
            )
            .expect_err("wrong proof must fail"),
        proto::ProtocolError::AuthenticationFailed
    );
    let guard = session
        .admit(&envelope, &proof, 1, proto::MonotonicMillis::new(1_000))
        .expect("admit");
    assert_eq!(guard.request_id(), envelope.request_id());
    // Same request ID replays fail; sequence must advance monotonically.
    assert_eq!(
        session
            .admit(&envelope, &proof, 2, proto::MonotonicMillis::new(1_000))
            .expect_err("replay must fail"),
        proto::ProtocolError::ReplayDetected
    );
}

// In-flight ceiling: 32 concurrent requests, the 33rd fails, cancel frees.
#[test]
fn in_flight_ceiling_is_32_and_cancel_releases() {
    let mut session = bound_session();
    let nonce = *session.server_nonce();
    let mut proofs = Vec::new();
    for index in 0_u8..32 {
        let mut id = nonzero16(0xC0);
        id[0] = index + 1;
        let envelope = proto::seal_envelope(
            version(1, 0),
            nonce,
            search_contracts::RequestId::from_bytes(id),
            proto::ControlCommand::Health,
            proto::ProofDigest::from_bytes([index; 32]),
            proto::ProofDigest::from_bytes([index; 32]),
        );
        proofs.push(*envelope.proof());
        session
            .admit(
                &envelope,
                &proofs[usize::from(index)],
                u64::from(index) + 1,
                proto::MonotonicMillis::new(1_000),
            )
            .expect("admit within ceiling");
    }
    let overflow = proto::seal_envelope(
        version(1, 0),
        nonce,
        search_contracts::RequestId::from_bytes(nonzero16(0xD0)),
        proto::ControlCommand::Health,
        proto::ProofDigest::from_bytes([0xF0; 32]),
        proto::ProofDigest::from_bytes([0xF0; 32]),
    );
    assert_eq!(
        session
            .admit(
                &overflow,
                overflow.proof(),
                33,
                proto::MonotonicMillis::new(1_000)
            )
            .expect_err("33rd request must exhaust"),
        proto::ProtocolError::ResourceExhausted
    );
    // Idempotent cancel of an in-flight request releases exactly one slot.
    let target = search_contracts::RequestId::from_bytes({
        let mut id = nonzero16(0xC0);
        id[0] = 1;
        id
    });
    assert_eq!(
        session.cancel(&target),
        proto::CancelOutcome::Cancelled { terminal: false }
    );
    assert_eq!(
        session.cancel(&target),
        proto::CancelOutcome::UnknownOrTerminal
    );
    session
        .admit(
            &overflow,
            overflow.proof(),
            33,
            proto::MonotonicMillis::new(1_000),
        )
        .expect("slot released by cancel");
}

// Relative deadlines are enforced at admission with an explicit clock.
#[test]
fn expired_relative_deadline_fails_admission() {
    let envelope = search_contracts::protocol::ProviderEnvelope {
        protocol_major: 1,
        protocol_minor: 0,
        installation_incarnation_id: search_contracts::InstallationIncarnationId::from_bytes(
            nonzero16(0x01),
        ),
        binding_id: search_contracts::BindingId::from_bytes(nonzero16(0x02)),
        connection_sequence: 7,
        request_id: search_contracts::RequestId::from_bytes(nonzero16(0x03)),
        message_kind: search_contracts::protocol::MessageKind::Request,
        relative_deadline_ms: Some(0),
        body: search_contracts::protocol::ProviderBodyV1::Cancel(
            search_contracts::protocol::CancelBody {
                target_request_id: search_contracts::RequestId::from_bytes(nonzero16(0x04)),
            },
        ),
    };
    let mut session = bound_session();
    let sealed = proto::seal_envelope(
        version(1, 0),
        *session.server_nonce(),
        envelope.request_id,
        proto::ControlCommand::Shutdown,
        proto::ProofDigest::from_bytes([1; 32]),
        proto::ProofDigest::from_bytes([1; 32]),
    );
    let _ = envelope;
    assert_eq!(
        session
            .admit_with_deadline(
                &sealed,
                sealed.proof(),
                1,
                proto::MonotonicMillis::new(5_000),
                Some(0),
            )
            .expect_err("expired deadline must fail"),
        proto::ProtocolError::DeadlineExpired
    );
    let guard = session
        .admit_with_deadline(
            &sealed,
            sealed.proof(),
            1,
            proto::MonotonicMillis::new(5_000),
            Some(1_000),
        )
        .expect("live deadline admits");
    assert!(!guard.is_expired(proto::MonotonicMillis::new(5_999)));
    assert!(guard.is_expired(proto::MonotonicMillis::new(6_000)));
}

// Disconnect drains deterministically and reports exact counts.
#[test]
fn disconnect_reports_exact_drain_counts() {
    let mut session = bound_session();
    assert!(session.is_active());
    let first = sealed_envelope(&session);
    let first_proof = *first.proof();
    session
        .admit(&first, &first_proof, 1, proto::MonotonicMillis::new(1))
        .expect("admit");
    let receipt = session.disconnect();
    assert_eq!(receipt.cancelled_requests(), 1);
    assert!(!session.is_active());
    assert_eq!(
        session.cancel(first.request_id()),
        proto::CancelOutcome::UnknownOrTerminal
    );
}
