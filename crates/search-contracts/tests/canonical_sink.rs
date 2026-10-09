//! Public-API fixtures for the sole `search-contracts` canonical encoder.
//!
//! These freeze the byte goldens accepted at launch base
//! `5d0435a55db8120d629d14d5167db6d737ea90c2` and cover the small set of
//! regressions the shared checked sink could introduce: map ordering,
//! shortest text-key heads, duplicate rejection and JSON escaping/base64
//! expansion crossing the output ceiling.
//!
//! `stream_canonical_cbor` is `pub(crate)`, so the sink is observed through the
//! public vector APIs that share it. Digest-domain and algorithm fixtures
//! belong to the foundation owner and are deliberately absent here.

use search_contracts::*;

fn key(value: &str) -> CanonicalKey {
    CanonicalKey::new_non_empty(value).expect("fixture key")
}

fn object(entries: Vec<(&str, CanonicalValue)>) -> CanonicalValue {
    let pairs = entries
        .into_iter()
        .map(|(name, value)| (key(name), value))
        .collect::<Vec<_>>();
    CanonicalValue::Object(BoundedMap::from_entries(pairs).expect("fixture object"))
}

#[test]
fn frozen_json_and_cbor_goldens_are_byte_identical() {
    let ordered = object(vec![
        ("a", CanonicalValue::U64(1)),
        ("b", CanonicalValue::U64(2)),
    ]);
    let json = to_canonical_json(&ordered).expect("JSON");
    assert_eq!(json.as_slice(), b"{\"a\":1,\"b\":2}");
    let encoded = to_canonical_cbor(&ordered).expect("CBOR");
    assert_eq!(encoded.as_slice(), b"\xa2\x61\x61\x01\x61\x62\x02");
    assert_eq!(parse_canonical_cbor(encoded.as_slice()), Ok(ordered));

    // RFC 8949 section 4.2.3 sorts "b" (two encoded bytes) before "aa" (three),
    // while JSON keeps raw UTF-8 byte order.
    let divergent = object(vec![
        ("aa", CanonicalValue::U64(2)),
        ("b", CanonicalValue::U64(1)),
    ]);
    let json = to_canonical_json(&divergent).expect("JSON");
    assert_eq!(json.as_slice(), b"{\"aa\":2,\"b\":1}");
    let cbor = to_canonical_cbor(&divergent).expect("CBOR");
    assert_eq!(cbor.as_slice(), b"\xa2\x61\x62\x01\x62\x61\x61\x02");
    assert_eq!(parse_canonical_cbor(cbor.as_slice()), Ok(divergent));
}

#[test]
fn map_permutation_is_stable() {
    let forward = object(vec![
        ("b", CanonicalValue::U64(5)),
        ("aa", CanonicalValue::U64(4)),
        ("z", CanonicalValue::U64(1)),
    ]);
    let reversed = object(vec![
        ("z", CanonicalValue::U64(1)),
        ("aa", CanonicalValue::U64(4)),
        ("b", CanonicalValue::U64(5)),
    ]);
    let encoded = to_canonical_cbor(&forward).expect("CBOR");
    assert_eq!(to_canonical_cbor(&reversed).expect("CBOR"), encoded);
    assert_eq!(
        encoded.as_slice(),
        b"\xa3\x61\x62\x05\x61\x7a\x01\x62\x61\x61\x04"
    );
    let json = to_canonical_json(&forward).expect("JSON");
    assert_eq!(json.as_slice(), b"{\"aa\":4,\"b\":5,\"z\":1}");
    assert_eq!(parse_canonical_cbor(encoded.as_slice()), Ok(forward));
}

#[test]
fn text_key_head_uses_the_shortest_length_form() {
    for (length, head) in [(23_usize, &[0x77_u8][..]), (24, &[0x78_u8, 0x18][..])] {
        let name = "k".repeat(length);
        let value = object(vec![(name.as_str(), CanonicalValue::Null)]);
        let cbor = to_canonical_cbor(&value).expect("CBOR");
        assert_eq!(cbor.len(), 1 + head.len() + length + 1);
        assert_eq!(&cbor.as_slice()[..1], b"\xa1");
        assert_eq!(&cbor.as_slice()[1..=head.len()], head);
        assert_eq!(
            &cbor.as_slice()[1 + head.len()..1 + head.len() + length],
            name.as_bytes()
        );
        assert_eq!(cbor.as_slice().last().copied(), Some(0xf6));
        assert_eq!(parse_canonical_cbor(cbor.as_slice()), Ok(value));
    }
}

#[test]
fn duplicate_map_key_is_rejected_rather_than_normalized() {
    let single = object(vec![("a", CanonicalValue::U64(1))]);
    let encoded = to_canonical_cbor(&single).expect("CBOR");
    assert_eq!(encoded.as_slice(), b"\xa1\x61\x61\x01");
    assert_eq!(
        parse_canonical_cbor(b"\xa2\x61\x61\x01\x61\x61\x02")
            .expect_err("duplicate key")
            .kind(),
        ContractErrorKind::Duplicate
    );
    assert_eq!(parse_canonical_cbor(encoded.as_slice()), Ok(single));
}

#[test]
fn json_escape_expansion_is_rejected_at_the_output_ceiling() {
    // One input byte becomes six output bytes, so this value is legal on its own
    // while its JSON form crosses MAX_CANONICAL_BYTES.
    let raw = "\u{1}".repeat(MAX_CANONICAL_BYTES / 6 + 1);
    assert!(raw.len() <= MAX_RAW_BYTES);
    let value = CanonicalValue::Text(CanonicalText::new(raw).expect("bounded text"));
    let error = to_canonical_json(&value).expect_err("escaped JSON exceeds the ceiling");
    assert_eq!(error.kind(), ContractErrorKind::OversizePayload);
    // The same value still encodes as CBOR, isolating the rejection to the JSON
    // expansion rather than the tree bound.
    let cbor = to_canonical_cbor(&value).expect("CBOR");
    assert_eq!(parse_canonical_cbor(cbor.as_slice()), Ok(value));
}

#[test]
fn bytes_base64_expansion_is_rejected_at_the_output_ceiling() {
    // Three input bytes become four base64url characters plus the `$bytes`
    // wrapper, so this payload is legal while its JSON form crosses the ceiling.
    let payload = vec![0xff; 3 * (MAX_CANONICAL_BYTES / 4 + 1)];
    assert!(payload.len() <= MAX_RAW_BYTES);
    let value = CanonicalValue::Bytes(BoundedBytes::new(payload).expect("bounded bytes"));
    let error = to_canonical_json(&value).expect_err("base64 JSON exceeds the ceiling");
    assert_eq!(error.kind(), ContractErrorKind::OversizePayload);
    let cbor = to_canonical_cbor(&value).expect("CBOR");
    assert_eq!(parse_canonical_cbor(cbor.as_slice()), Ok(value));
}
