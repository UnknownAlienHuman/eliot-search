use search_contracts::{ProtocolRange, ProtocolVersion, RequestId};

use super::*;
use crate::config::DEFAULT_PROTOCOL_LIMITS;
use crate::error::ProtocolError;
use crate::pairing::{ProofDigest, ServerNonce};

fn version(major: u16, minor: u16) -> ProtocolVersion {
    ProtocolVersion { major, minor }
}

fn supported() -> ProtocolRange {
    ProtocolRange::new(version(1, 0), version(1, 2)).expect("range")
}

fn envelope() -> AuthenticatedStandaloneGrantEnvelope {
    seal_standalone_grant_envelope(
        version(1, 0),
        ServerNonce::from_bytes([0x11; 16]).expect("nonce"),
        RequestId::from_bytes([0x22; 16]),
        ProofDigest::from_bytes([0x33; 32]),
        ProofDigest::from_bytes([0x44; 32]),
    )
}

#[test]
fn canonical_json_and_frame_round_trip() {
    let envelope = envelope();
    let json = encode_standalone_grant_envelope_json(&envelope);
    assert!(json.starts_with(b"{\"v\":[1,0],\"nonce\":\"1111"));
    assert_eq!(
        decode_standalone_grant_envelope_json(&json, supported()).expect("json"),
        envelope
    );

    let frame = encode_standalone_grant_envelope(&envelope, DEFAULT_PROTOCOL_LIMITS)
        .expect("frame");
    assert_eq!(
        decode_standalone_grant_envelope(frame.as_slice(), DEFAULT_PROTOCOL_LIMITS, supported())
            .expect("decode frame"),
        envelope
    );
}

#[test]
fn transcript_binds_domain_version_nonce_request_and_body() {
    let base = envelope();
    let transcript = standalone_grant_envelope_transcript(&base);
    assert!(
        transcript
            .windows(STANDALONE_GRANT_ENVELOPE_DOMAIN.len())
            .any(|window| window == STANDALONE_GRANT_ENVELOPE_DOMAIN.as_bytes())
    );
    let changed = seal_standalone_grant_envelope(
        base.version(),
        *base.server_nonce(),
        *base.request_id(),
        ProofDigest::from_bytes([0x35; 32]),
        *base.proof(),
    );
    assert_ne!(
        standalone_grant_envelope_transcript(&base),
        standalone_grant_envelope_transcript(&changed)
    );
    assert!(verify_standalone_grant_envelope_proof(&base, base.proof()).is_ok());
    assert_eq!(
        verify_standalone_grant_envelope_proof(
            &base,
            &ProofDigest::from_bytes([0x99; 32])
        ),
        Err(ProtocolError::AuthenticationFailed)
    );
}

#[test]
fn noncanonical_and_unsupported_envelopes_fail_closed() {
    let canonical = encode_standalone_grant_envelope_json(&envelope());

    let mut trailing = canonical.clone();
    trailing.push(b' ');
    assert_eq!(
        decode_standalone_grant_envelope_json(&trailing, supported()),
        Err(ProtocolError::InvalidEnvelope)
    );

    let mut uppercase = canonical.clone();
    let body_marker = b"\"body\":\"";
    let body = uppercase
        .windows(body_marker.len())
        .position(|window| window == body_marker)
        .expect("body")
        + body_marker.len();
    uppercase[body] = b'A';
    assert_eq!(
        decode_standalone_grant_envelope_json(&uppercase, supported()),
        Err(ProtocolError::InvalidEnvelope)
    );

    let mut reordered = canonical;
    let nonce = b"\"nonce\"";
    let start = reordered
        .windows(nonce.len())
        .position(|window| window == nonce)
        .expect("nonce");
    reordered[start] = b'X';
    assert_eq!(
        decode_standalone_grant_envelope_json(&reordered, supported()),
        Err(ProtocolError::InvalidEnvelope)
    );

    let foreign = seal_standalone_grant_envelope(
        version(2, 0),
        ServerNonce::from_bytes([0x11; 16]).expect("nonce"),
        RequestId::from_bytes([0x22; 16]),
        ProofDigest::from_bytes([0x33; 32]),
        ProofDigest::from_bytes([0x44; 32]),
    );
    assert_eq!(
        decode_standalone_grant_envelope_json(
            &encode_standalone_grant_envelope_json(&foreign),
            supported(),
        ),
        Err(ProtocolError::NoCompatibleVersion)
    );
}

#[test]
fn zero_nonce_and_oversize_are_rejected() {
    let mut json = encode_standalone_grant_envelope_json(&envelope());
    let nonce_marker = b"\"nonce\":\"";
    let nonce = json
        .windows(nonce_marker.len())
        .position(|window| window == nonce_marker)
        .expect("nonce")
        + nonce_marker.len();
    json[nonce..nonce + 32].fill(b'0');
    assert_eq!(
        decode_standalone_grant_envelope_json(&json, supported()),
        Err(ProtocolError::InvalidNonce)
    );

    let oversized = vec![b'x'; MAX_STANDALONE_GRANT_ENVELOPE_JSON_BYTES + 1];
    assert_eq!(
        decode_standalone_grant_envelope_json(&oversized, supported()),
        Err(ProtocolError::FrameTooLarge)
    );
}
