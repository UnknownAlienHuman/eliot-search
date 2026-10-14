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

fn fixture_header(expected_objects: u64) -> SourceContentManifestHeader {
    SourceContentManifestHeader {
        target_namespace: SourceNamespaceId::from_bytes([1; 16]),
        legacy_namespace: digest(2),
        catalog_snapshot: digest(3),
        source_plan: digest(4),
        expected_objects,
    }
}

fn fixture_object() -> SourceContentObjectReadback {
    SourceContentObjectReadback {
        legacy_source_id: digest(5),
        legacy_revision_id: digest(6),
        content_sha256: digest(7),
        byte_length: 42,
        content_blake3: Blake3Digest32::from_bytes([8; 32]),
    }
}

fn encoded_rows(
    header: SourceContentManifestHeader,
    object: Option<SourceContentObjectReadback>,
) -> (Vec<Vec<u8>>, SourceContentManifestSummary) {
    let mut encoder = SourceContentManifestEncoder::new(header).expect("encoder");
    let mut rows = vec![encoder.header_row().expect("header")];
    if let Some(object) = object {
        rows.push(encoder.object_row(object).expect("object"));
    }
    let (end, summary) = encoder.finish().expect("end");
    rows.push(end);
    (rows, summary)
}

fn replace_once(row: &[u8], from: &str, to: &str) -> Vec<u8> {
    String::from_utf8(row.to_vec())
        .expect("fixture utf8")
        .replacen(from, to, 1)
        .into_bytes()
}

#[test]
fn decoder_round_trips_empty_and_single_object_manifests() {
    for expected_objects in [0, 1] {
        let object = (expected_objects == 1).then(fixture_object);
        let (rows, expected_summary) = encoded_rows(fixture_header(expected_objects), object);
        let mut decoder = SourceContentManifestDecoder::new(fixture_header(expected_objects))
            .expect("decoder");

        assert_eq!(
            decoder.push(&rows[0]),
            Ok(SourceContentManifestDecodedRow::Header)
        );
        if let Some(object) = object {
            assert_eq!(
                decoder.push(&rows[1]),
                Ok(SourceContentManifestDecodedRow::Object(object))
            );
        }
        let end_index = if object.is_some() { 2 } else { 1 };
        assert_eq!(
            decoder.push(&rows[end_index]),
            Ok(SourceContentManifestDecodedRow::End(expected_summary))
        );
        assert_eq!(decoder.finish(), Ok(expected_summary));
    }
}

#[test]
fn decoder_rejects_noncanonical_rows_and_refuses_stickily() {
    let header = fixture_header(1);
    let object = fixture_object();
    let (rows, _) = encoded_rows(header, Some(object));
    let source = object.legacy_source_id.to_string();
    let revision = object.legacy_revision_id.to_string();
    let ordered = format!(
        "\"legacy_source_id\":\"{source}\",\"legacy_revision_id\":\"{revision}\""
    );
    let reordered = format!(
        "\"legacy_revision_id\":\"{revision}\",\"legacy_source_id\":\"{source}\""
    );
    let mut malformed_utf8 = rows[1].clone();
    let digest_start = malformed_utf8
        .windows(4)
        .position(|window| window == b"0505")
        .expect("source digest");
    malformed_utf8[digest_start] = 0xff;
    let missing_lf = rows[1][..rows[1].len() - 1].to_vec();
    let mut multiple_lf = rows[1].clone();
    multiple_lf.push(b'\n');

    let invalid_rows = [
        replace_once(&rows[1], "}\n", ",\"unknown\":0}\n"),
        replace_once(&rows[1], "\"ordinal\":1,", "\"ordinal\":1,\"ordinal\":1,"),
        replace_once(
            &rows[1],
            "\"legacy_source_id\":\"0",
            "\"legacy_source_id\":\"\\u0030",
        ),
        replace_once(&rows[1], &ordered, &reordered),
        replace_once(&rows[1], "\"byte_length\":42", "\"byte_length\":042"),
        malformed_utf8,
        missing_lf,
        multiple_lf,
        b"malformed\n".to_vec(),
    ];

    for invalid_row in invalid_rows {
        let mut decoder = SourceContentManifestDecoder::new(header).expect("decoder");
        decoder.push(&rows[0]).expect("header");
        let refusal = decoder.push(&invalid_row).expect_err("noncanonical row");
        assert_eq!(decoder.push(&rows[1]), Err(refusal));
        assert_eq!(decoder.finish(), Err(refusal));
    }

    let wrong_algorithm = replace_once(&rows[0], "blake3_256", "sha256");
    let mut decoder = SourceContentManifestDecoder::new(header).expect("decoder");
    assert_eq!(
        decoder.push(&wrong_algorithm),
        Err(SourceContentManifestEncodingError::InvalidHeader)
    );

    let other_header = SourceContentManifestHeader {
        target_namespace: SourceNamespaceId::from_bytes([9; 16]),
        ..fixture_header(1)
    };
    let (foreign_rows, _) = encoded_rows(other_header, Some(object));
    let mut decoder = SourceContentManifestDecoder::new(header).expect("decoder");
    assert_eq!(
        decoder.push(&foreign_rows[0]),
        Err(SourceContentManifestEncodingError::InvalidHeader)
    );
}

#[test]
fn decoder_rejects_ordinals_footers_trailing_rows_and_oversize() {
    let header = fixture_header(1);
    let object = fixture_object();
    let (rows, _) = encoded_rows(header, Some(object));

    let mut decoder = SourceContentManifestDecoder::new(header).expect("decoder");
    decoder.push(&rows[0]).expect("header");
    let wrong_ordinal = replace_once(&rows[1], "\"ordinal\":1", "\"ordinal\":2");
    assert_eq!(
        decoder.push(&wrong_ordinal),
        Err(SourceContentManifestEncodingError::ObjectCountMismatch)
    );

    let mut decoder = SourceContentManifestDecoder::new(header).expect("decoder");
    decoder.push(&rows[0]).expect("header");
    decoder.push(&rows[1]).expect("object");
    let wrong_footer = replace_once(&rows[2], "\"source_bytes\":42", "\"source_bytes\":43");
    assert_eq!(
        decoder.push(&wrong_footer),
        Err(SourceContentManifestEncodingError::InvalidState)
    );

    let mut decoder = SourceContentManifestDecoder::new(header).expect("decoder");
    decoder.push(&rows[0]).expect("header");
    decoder.push(&rows[1]).expect("object");
    decoder.push(&rows[2]).expect("end");
    assert_eq!(
        decoder.push(&rows[0]),
        Err(SourceContentManifestEncodingError::InvalidState)
    );

    let mut decoder = SourceContentManifestDecoder::new(header).expect("decoder");
    let oversized = vec![b'x'; super::super::MAX_SOURCE_IMPORT_ROW_BYTES + 1];
    assert_eq!(
        decoder.push(&oversized),
        Err(SourceContentManifestEncodingError::ObjectLimitExceeded)
    );
}
