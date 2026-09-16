use search_contracts::protocol::{HelloBody, PeerRole};
use search_contracts::{
    BindingId, InstallationIncarnationId, ProtocolRange, ProtocolVersion, RequestId,
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
