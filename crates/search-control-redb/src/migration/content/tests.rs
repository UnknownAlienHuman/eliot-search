use super::*;

fn digest(byte: u8) -> Sha256Digest32 {
    Sha256Digest32::from_bytes([byte; 32])
}

#[test]
fn frozen_profile_and_rows_are_byte_exact() {
    assert_eq!(
        source_content_profile_digest().to_string(),
        "8d99f177a1cf8134710dc7b586c36f2e0229060755e33089e04e492f667af29f"
    );
    let mut encoder = SourceContentManifestEncoder::new(
        SourceContentManifestHeader {
            target_namespace: SourceNamespaceId::from_bytes([1; 16]),
            legacy_namespace: digest(2),
            catalog_snapshot: digest(3),
            source_plan: digest(4),
            expected_objects: 1,
        },
    )
    .expect("encoder");

    let header = String::from_utf8(encoder.header_row().expect("header"))
        .expect("utf8");
    assert_eq!(
        header,
        concat!(
            "{\"kind\":\"source_content_header\",\"schema\":\"eliot.source-content.v1\",",
            "\"target_namespace_id\":\"01010101-0101-0101-0101-010101010101\",",
            "\"legacy_namespace_sha256\":\"0202020202020202020202020202020202020202020202020202020202020202\",",
            "\"catalog_snapshot_sha256\":\"0303030303030303030303030303030303030303030303030303030303030303\",",
            "\"source_plan_chain_sha256\":\"0404040404040404040404040404040404040404040404040404040404040404\",",
            "\"content_profile_sha256\":\"8d99f177a1cf8134710dc7b586c36f2e0229060755e33089e04e492f667af29f\",",
            "\"expected_objects\":1,\"content_digest_algorithm\":\"blake3_256\",",
            "\"cutover_authorized\":false}\n"
        )
    );

    let object = String::from_utf8(
        encoder
            .object_row(SourceContentObjectReadback {
                legacy_source_id: digest(5),
                legacy_revision_id: digest(6),
                content_sha256: digest(7),
                byte_length: 42,
                content_blake3: Blake3Digest32::from_bytes([8; 32]),
            })
            .expect("object"),
    )
    .expect("utf8");
    assert_eq!(
        object,
        concat!(
            "{\"kind\":\"source_content_readback\",\"ordinal\":1,",
            "\"legacy_source_id\":\"0505050505050505050505050505050505050505050505050505050505050505\",",
            "\"legacy_revision_id\":\"0606060606060606060606060606060606060606060606060606060606060606\",",
            "\"content_sha256\":\"0707070707070707070707070707070707070707070707070707070707070707\",",
            "\"byte_length\":42,\"content_blake3\":\"0808080808080808080808080808080808080808080808080808080808080808\"}\n"
        )
    );

    let (end, summary) = encoder.finish().expect("finish");
    assert_eq!(
        String::from_utf8(end).expect("utf8"),
        concat!(
            "{\"kind\":\"source_content_end\",\"objects\":1,\"source_bytes\":42,",
            "\"legacy_sha256_verified\":true,\"blake3_computed_from_bytes\":true,",
            "\"stability_receipt_issued\":false,\"residency_authorized\":false}\n"
        )
    );
    assert_eq!(
        summary,
        SourceContentManifestSummary {
            objects: 1,
            source_bytes: 42,
        }
    );
}

#[test]
fn order_count_and_bounds_fail_closed() {
    let header = SourceContentManifestHeader {
        target_namespace: SourceNamespaceId::from_bytes([1; 16]),
        legacy_namespace: digest(2),
        catalog_snapshot: digest(3),
        source_plan: digest(4),
        expected_objects: 1,
    };
    let mut encoder = SourceContentManifestEncoder::new(header).expect("encoder");
    assert_eq!(
        encoder.finish(),
        Err(SourceContentManifestEncodingError::InvalidState)
    );
    encoder.header_row().expect("header");
    assert_eq!(
        encoder.finish(),
        Err(SourceContentManifestEncodingError::ObjectCountMismatch)
    );
    let object = SourceContentObjectReadback {
        legacy_source_id: digest(5),
        legacy_revision_id: digest(6),
        content_sha256: digest(7),
        byte_length: u64::MAX,
        content_blake3: Blake3Digest32::from_bytes([8; 32]),
    };
    encoder.object_row(object).expect("first object");
    assert_eq!(
        encoder.object_row(SourceContentObjectReadback {
            byte_length: 1,
            ..object
        }),
        Err(SourceContentManifestEncodingError::ObjectCountMismatch)
    );
    encoder.finish().expect("finish");
    assert_eq!(
        encoder.header_row(),
        Err(SourceContentManifestEncodingError::InvalidState)
    );
}
