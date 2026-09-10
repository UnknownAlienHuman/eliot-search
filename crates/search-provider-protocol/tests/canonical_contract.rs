//! Canonical contract probes: framing is `u32`-LE length plus canonical
//! UTF-8 JSON (`search-contracts/src/protocol.rs:302-336`), and
//! `ProtocolVersion` / `ProtocolRange` / `RequestId` are owned by
//! `search-contracts` (no duplicates, norm #89).

use search_provider_protocol as proto;

fn range(min_major: u16, min_minor: u16, max_major: u16, max_minor: u16) -> proto::ProtocolRange {
    proto::ProtocolRange::new(
        proto::ProtocolVersion {
            major: min_major,
            minor: min_minor,
        },
        proto::ProtocolVersion {
            major: max_major,
            minor: max_minor,
        },
    )
    .expect("range")
}

// (A) Framing must be u32-LE length + canonical UTF-8 JSON.
#[test]
fn framing_is_canonical_u32_le_json() {
    let payload = search_contracts::protocol::JsonFramePayload::new(br#"{"ok":true}"#.to_vec())
        .expect("canonical payload");
    let canonical =
        search_contracts::protocol::encode_json_frame(&payload).expect("canonical frame");
    assert_eq!(
        &canonical.as_slice()[..4],
        &11_u32.to_le_bytes(),
        "canonical frame must start with u32-LE length"
    );

    let encoded = proto::FrameCodec::encode(&payload, proto::DEFAULT_PROTOCOL_LIMITS)
        .expect("package encode");
    assert_eq!(
        encoded.as_slice(),
        canonical.as_slice(),
        "package framing must equal canonical u32-LE + JSON framing"
    );
    assert_eq!(
        proto::FrameCodec::decode(encoded.as_slice(), proto::DEFAULT_PROTOCOL_LIMITS)
            .expect("package decode"),
        payload
    );
    // Free-function aliases required by FUNCTIONS.md must agree.
    let free = proto::encode_frame(&payload, proto::DEFAULT_PROTOCOL_LIMITS).expect("encode");
    assert_eq!(free.as_slice(), canonical.as_slice());
    assert_eq!(
        proto::decode_frame(free.as_slice(), proto::DEFAULT_PROTOCOL_LIMITS).expect("decode"),
        payload
    );
}

#[test]
fn framing_rejects_oversize_malformed_without_unbounded_buffering() {
    // Oversize rejected before body allocation.
    let oversize = vec![0_u8; proto::DEFAULT_PROTOCOL_LIMITS.max_frame_bytes + 1];
    assert_eq!(
        proto::FrameCodec::decode(&oversize, proto::DEFAULT_PROTOCOL_LIMITS),
        Err(proto::ProtocolError::FrameTooLarge)
    );
    // Truncated prefix and declared-length mismatch fail closed.
    assert_eq!(
        proto::FrameCodec::decode(&[1, 0, 0], proto::DEFAULT_PROTOCOL_LIMITS),
        Err(proto::ProtocolError::InvalidEnvelope)
    );
    assert_eq!(
        proto::FrameCodec::decode(&[2, 0, 0, 0, b'{'], proto::DEFAULT_PROTOCOL_LIMITS),
        Err(proto::ProtocolError::InvalidEnvelope)
    );
    // Non-UTF-8 bodies are rejected by the canonical payload constructor.
    assert!(
        search_contracts::protocol::JsonFramePayload::new(vec![0xff]).is_err(),
        "non-UTF-8 payload must be rejected"
    );
}

// (B) Canonical ownership: package types must BE the search-contracts types.
#[test]
fn protocol_types_are_canonical() {
    assert_eq!(
        std::any::type_name::<proto::ProtocolVersion>(),
        std::any::type_name::<search_contracts::protocol::ProtocolVersion>(),
        "ProtocolVersion must be the canonical search-contracts type"
    );
    assert_eq!(
        std::any::type_name::<proto::ProtocolRange>(),
        std::any::type_name::<search_contracts::protocol::ProtocolRange>(),
        "ProtocolRange must be the canonical search-contracts type"
    );
    assert_eq!(
        std::any::type_name::<proto::RequestId>(),
        std::any::type_name::<search_contracts::RequestId>(),
        "RequestId must be the canonical search-contracts type"
    );
}

// (B) Negotiation is major/minor per FUNCTIONS.md:20-23: same major selects
// the highest minor, major mismatch fails, minor-disjoint ranges fail.
#[test]
fn negotiate_uses_major_minor_semantics() {
    let selected =
        proto::negotiate_hello(range(1, 0, 1, 2), range(1, 1, 1, 5)).expect("minor overlap");
    assert_eq!(selected, proto::ProtocolVersion { major: 1, minor: 2 });

    assert_eq!(
        proto::negotiate_hello(range(1, 0, 1, 1), range(2, 0, 2, 0)),
        Err(proto::ProtocolError::NoCompatibleVersion),
        "major mismatch must fail"
    );
    assert_eq!(
        proto::negotiate_hello(range(1, 0, 1, 1), range(1, 2, 1, 3)),
        Err(proto::ProtocolError::NoCompatibleVersion),
        "minor-disjoint ranges must fail"
    );
    // Alias preserved for intra-package callers must agree.
    assert_eq!(
        proto::negotiate_version(range(1, 0, 1, 2), range(1, 1, 1, 5)).expect("alias"),
        selected
    );
}
