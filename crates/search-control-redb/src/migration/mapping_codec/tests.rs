use super::*;
use search_contracts::SourceNamespaceId;

// Frozen daemon v1 bytes and the pre-existing mapper's UUID known answers.
const HEADER: &str = concat!(r#"{"kind":"source_mapping_header","schema":"eliot.source-mapping.v1","target_namespace_id":"01010101-0101-0101-0101-010101010101","legacy_namespace_sha256":"0202020202020202020202020202020202020202020202020202020202020202","catalog_snapshot_sha256":"0909090909090909090909090909090909090909090909090909090909090909","mapping_profile_sha256":"dbd82192a4a9d5be14c9774cd208b77ccbb61c5d832a7e6f450813c9d73df349","expected_events":1,"expected_sources":1,"identity_kind":"imported_object","acquisition_kind":"imported","draft_only":true,"cutover_authorized":false}"#, "\n");
const EVENT: &str = concat!(r#"{"kind":"source_event_mapping","source_id":"1a484e7a-e3c2-8c81-aee8-a55ee5edbca0","revision_id":"da220f10-65d7-860d-b594-81ab4b63d170","occurrence_sequence":1,"previous_revision_id":null,"opens_source":true,"opens_revision":true,"retires_source":false,"legacy":{"event_sequence":1,"source_event_ordinal":1,"previous_event_sha256":"0000000000000000000000000000000000000000000000000000000000000000","previous_source_event_sha256":"0000000000000000000000000000000000000000000000000000000000000000","operation_id":"0707070707070707070707070707070707070707070707070707070707070707","state":"A","source_id":"0303030303030303030303030303030303030303030303030303030303030303","legacy_revision_id":"0505050505050505050505050505050505050505050505050505050505050505","content_sha256":"0606060606060606060606060606060606060606060606060606060606060606","byte_length":42,"file_identity_sha256":"0808080808080808080808080808080808080808080808080808080808080808","path_sha256":"0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a","identity_strength":"native","event_sha256":"0404040404040404040404040404040404040404040404040404040404040404"}}"#, "\n");
const END: &str = concat!(r#"{"kind":"source_mapping_end","events":1,"sources":1,"revision_occurrences":1,"retained_revision_events":0,"retirements":0,"required_import_inputs":["content_blake3_readback","import_observation_and_stability","residency_policy","root_and_membership_bindings","namespace_owner_cutover"],"canonical_records_materialized":false}"#, "\n");

fn header(events: u64) -> SourceMappingHeader {
    SourceMappingHeader::new(SourceNamespaceId::from_bytes([1; 16]), [2; 32], [9; 32], events, 1, 16).unwrap()
}

fn decoder() -> SourceMappingManifestDecoder {
    SourceMappingManifestDecoder::new(header(1), 16).unwrap()
}

#[test]
fn mapping_codec_frozen_daemon_bytes_and_existing_uuid_answers_match() {
    assert_eq!(encode_source_mapping_header(header(1)), HEADER.as_bytes());
    let mut decoder = decoder();
    assert_eq!(decoder.push(HEADER.as_bytes()).unwrap(), SourceMappingDecodedRow::Header);
    let SourceMappingDecodedRow::Event(row) = decoder.push(EVENT.as_bytes()).unwrap() else { panic!("event"); };
    assert_eq!(row.source.to_string(), "1a484e7a-e3c2-8c81-aee8-a55ee5edbca0");
    assert_eq!(row.revision.to_string(), "da220f10-65d7-860d-b594-81ab4b63d170");
    assert_eq!(row.source_bytes, 42);
    assert!(row.lifecycle.opens_source() && row.lifecycle.opens_revision());
    let SourceMappingDecodedRow::End(summary) = decoder.push(END.as_bytes()).unwrap() else { panic!("footer"); };
    assert_eq!(summary.occurrences, 1);
    assert_eq!(decoder.finish().unwrap(), summary);
}

#[test]
fn mapping_codec_tampered_fields_ids_grammar_and_order_are_sticky() {
    let mutations = [
        EVENT.replace("1a484e7a-e3c2-8c81-aee8-a55ee5edbca0", "00000000-0000-0000-0000-000000000000"),
        EVENT.replace("\"opens_source\":true,", "\"opens_source\":true,\"opens_source\":true,"),
        EVENT.replace("\"opens_source\":true,", ""),
        EVENT.replace("\"opens_source\":true,", "\"unknown\":true,"),
        EVENT.replace("\"event_sequence\":1", "\"event_sequence\":2"),
        EVENT.replace("\"source_event_ordinal\":1", "\"source_event_ordinal\":2"),
        EVENT.replace("\"byte_length\":42", "\"byte_length\":042"),
        EVENT.replace("0a0a", "0A0A"),
        EVENT.replace("\"state\":\"A\"", "\"state\":\"UNKNOWN\""),
        EVENT.trim_end_matches('\n').to_owned(),
    ];
    for mutated in mutations {
        let mut decoder = decoder();
        decoder.push(HEADER.as_bytes()).unwrap();
        let refusal = decoder.push(mutated.as_bytes()).unwrap_err();
        assert_eq!(decoder.push(EVENT.as_bytes()), Err(refusal));
        assert_eq!(decoder.finish(), Err(refusal));
    }
}

#[test]
fn mapping_codec_missing_duplicate_foreign_footer_and_surplus_refuse() {
    let mut incomplete = decoder();
    incomplete.push(HEADER.as_bytes()).unwrap();
    incomplete.push(EVENT.as_bytes()).unwrap();
    assert!(incomplete.finish().is_err());
    for wrong in [HEADER.to_owned(), END.to_owned(), "unknown\n".to_owned(),
        "x".repeat(MAX_SOURCE_IMPORT_ROW_BYTES + 1)] {
        let mut decoder = decoder();
        decoder.push(HEADER.as_bytes()).unwrap();
        assert!(decoder.push(wrong.as_bytes()).is_err());
        assert!(decoder.finish().is_err());
    }
    let mut decoder = decoder();
    decoder.push(HEADER.as_bytes()).unwrap();
    decoder.push(EVENT.as_bytes()).unwrap();
    decoder.push(END.as_bytes()).unwrap();
    assert!(decoder.push(EVENT.as_bytes()).is_err());
    assert!(decoder.finish().is_err());
    let mut foreign = SourceMappingManifestDecoder::new(header(1), 16).unwrap();
    assert!(foreign.push(HEADER.replace("eliot.source-mapping.v1", "eliot.source-mapping.v2").as_bytes()).is_err());
}

#[test]
fn mapping_codec_occurrences_and_both_predecessor_chains_remain_exact() {
    let mut planner = SourceMappingPlanner::new(header(3), 16).unwrap();
    let mut decoder = SourceMappingManifestDecoder::new(header(3), 16).unwrap();
    decoder.push(&encode_source_mapping_header(header(3))).unwrap();
    let mut first_revision = None;
    for sequence in 1..=3 {
        let byte = if sequence == 2 { 11 } else { 5 };
        let event = LegacySourceMappingEvent { sequence,
            operation: Sha256Digest32::from_stored_bytes([7; 32]), legacy_source: Sha256Digest32::from_stored_bytes([3; 32]),
            legacy_revision: Sha256Digest32::from_stored_bytes([byte; 32]), content: Sha256Digest32::from_stored_bytes([byte; 32]),
            file_identity: Sha256Digest32::from_stored_bytes([8; 32]), path: Sha256Digest32::from_stored_bytes([10; 32]),
            event: Sha256Digest32::from_stored_bytes([u8::try_from(sequence).unwrap(); 32]),
            previous_event: Sha256Digest32::from_stored_bytes([u8::try_from(sequence - 1).unwrap(); 32]),
            previous_source_event: (sequence > 1).then(|| Sha256Digest32::from_stored_bytes([u8::try_from(sequence - 1).unwrap(); 32])),
            source_bytes: 42, state: LegacySourceMappingState::Active, native_identity: true };
        let mapped = planner.map(event).unwrap();
        let SourceMappingDecodedRow::Event(row) = decoder.push(&encode_source_mapping_event(&mapped, &event)).unwrap() else { panic!("event"); };
        assert_eq!(row.occurrence, sequence);
        if sequence == 1 { first_revision = Some(row.revision); }
        if sequence == 3 { assert_ne!(first_revision, Some(row.revision)); }
    }
    let summary = planner.finish().unwrap();
    decoder.push(&encode_source_mapping_summary(summary)).unwrap();
    assert_eq!(decoder.finish().unwrap().occurrences, 3);
}
