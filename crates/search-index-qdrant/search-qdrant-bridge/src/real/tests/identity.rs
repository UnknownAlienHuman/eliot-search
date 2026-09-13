use super::*;

#[test]
fn collection_name_binds_physical_and_generation() {
    let left = CollectionRoute {
        generation: search_contracts::CollectionGenerationId::from_bytes([
            0x11;
            16
        ]),
        physical_name: OpaqueId::new("t24_unit").expect("name"),
    };
    let right = CollectionRoute {
        generation: search_contracts::CollectionGenerationId::from_bytes([
            0x12;
            16
        ]),
        physical_name: OpaqueId::new("t24_unit").expect("name"),
    };
    let renamed = CollectionRoute {
        generation: search_contracts::CollectionGenerationId::from_bytes([
            0x11;
            16
        ]),
        physical_name: OpaqueId::new("t24_other").expect("name"),
    };
    let left_name = collection_name(&left).expect("left");
    assert_eq!(left_name.len(), 28);
    assert!(left_name.starts_with("t24c"));
    assert_eq!(collection_name(&left).expect("deterministic"), left_name);
    assert_ne!(
        collection_name(&right).expect("generation bound"),
        left_name
    );
    assert_ne!(
        collection_name(&renamed).expect("physical bound"),
        left_name
    );
}

#[test]
fn uuid_round_trip_preserves_128_bits() {
    let id = QdrantPointId([
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA,
        0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
    ]);
    let text = uuid_string(&id);
    assert_eq!(text, "00112233-4455-6677-8899-aabbccddeeff");
    assert_eq!(parse_uuid(&text).expect("round trip"), id);
    assert_eq!(
        parse_uuid("not-a-uuid").expect_err("rejects garbage"),
        BridgeError::MalformedResponse
    );
    assert_eq!(
        parse_uuid("00112233-4455-6677-8899-aabbccddeefg")
            .expect_err("rejects non-hex"),
        BridgeError::MalformedResponse
    );
}

#[test]
fn hex_32_round_trip() {
    let bytes = [0xABu8; 32];
    let text = hex_from_32(&bytes);
    assert_eq!(text.len(), 64);
    assert_eq!(hex_to_32(&text).expect("round trip"), bytes);
    assert_eq!(
        hex_to_32("short").expect_err("rejects short"),
        BridgeError::MalformedResponse
    );
}
