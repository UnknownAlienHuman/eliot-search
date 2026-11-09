//! Adversarial wire boundaries for the actual one-query DTO and frame codec.

use crate::*;

fn hit(rank: u8, text: &str) -> SearchItem {
    let start = u64::from(rank - 1) * 65_536;
    SearchItem::Hit(SearchHit {
        rank,
        score: -0.25,
        evidence: EvidenceMetadata {
            generation: 1,
            point_id: format!("{:016x}{:016x}", 1_u64, u64::from(rank)),
            source_namespace_id: "00000000-0000-4000-8000-000000000001".into(),
            source_id: "00000000-0000-4000-8000-000000000002".into(),
            input: ObjectRefDto {
                version: 1,
                kind: ObjectKindDto::Source,
                digest: "ab".repeat(32),
                bytes: 8 * 1024 * 1024,
            },
            prepared: ObjectRefDto {
                version: 1,
                kind: ObjectKindDto::Prepared,
                digest: "cd".repeat(32),
                bytes: 8 * 1024 * 1024 + 4096,
            },
            unit: u64::from(rank - 1),
            start,
            end: start + u64::try_from(text.len()).expect("finite fixture"),
            line_start: 1,
            line_end: 2,
            starts_at_line_boundary: true,
            ends_at_line_boundary: false,
        },
        unit_text: text.into(),
        source_info: None,
    })
}

fn terminal(items: Vec<SearchItem>) -> Terminal {
    Terminal {
        request_id: 1,
        outcome: QueryOutcome::Complete {
            generation: 1,
            items,
            stop: StopReason::Complete,
        },
        summary: None,
    }
}

fn raw_frame(body: &[u8]) -> Vec<u8> {
    let mut frame = u32::try_from(body.len())
        .expect("bounded fixture")
        .to_le_bytes()
        .to_vec();
    frame.extend_from_slice(body);
    frame
}

#[test]
fn fragmented_frame_keeps_prefix_body_and_trailing_bytes_visible() {
    let message = ClientFrame::Query {
        request_id: 1,
        remaining_ms: 731,
        request: SearchRequest {
            query: "needle\r\n😀".into(),
            limit: 64,
        },
        include_summary: false,
        include_source_info: false,
    };
    let frame = encode_client_frame(&message).expect("encode");
    let mut partial = FrameBuffer::new(MAX_CLIENT_FRAME_BYTES).expect("buffer");
    for byte in &frame[..frame.len() - 1] {
        assert_eq!(partial.push(&[*byte]).expect("fragment"), 1);
        assert!(!partial.is_complete());
    }
    let final_and_trailing = [*frame.last().expect("last"), 17, 18];
    assert_eq!(partial.push(&final_and_trailing).expect("last fragment"), 1);
    assert_eq!(
        decode_client_frame(partial.frame().expect("complete")).expect("decode"),
        message
    );
    assert_eq!(partial.push(&[19]).expect("no second frame"), 0);
}

#[test]
fn declared_length_is_rejected_at_prefix_before_body_arrives() {
    let mut frame = FrameBuffer::new(MAX_FRAME_BYTES).expect("buffer");
    assert_eq!(
        frame.push(&u32::MAX.to_le_bytes()),
        Err(ProtocolError::FrameTooLarge)
    );
    assert!(frame.frame().is_none());
    let mut zero = FrameBuffer::new(MAX_FRAME_BYTES).expect("buffer");
    assert_eq!(
        zero.push(&0_u32.to_le_bytes()),
        Err(ProtocolError::InvalidFrame)
    );
    assert!(FrameBuffer::new(MAX_FRAME_BYTES + 1).is_err());
}

#[test]
fn incomplete_extra_and_invalid_utf8_frames_fail_closed() {
    let valid = encode_client_frame(&ClientFrame::Cancel { request_id: 1 }).expect("encode");
    for count in 0..valid.len() {
        assert!(decode_client_frame(&valid[..count]).is_err());
    }
    let mut extra = valid;
    extra.push(0);
    assert!(decode_client_frame(&extra).is_err());
    assert_eq!(
        decode_client_frame(&raw_frame(&[0xff])),
        Err(ProtocolError::InvalidMessage)
    );
}

#[test]
fn closed_schema_rejects_duplicate_unknown_and_wrong_direction_fields() {
    let bodies = [
        r#"{"type":"cancel","request_id":1,"request_id":1}"#,
        r#"{"type":"cancel","request_id":1,"query":"hidden"}"#,
        r#"{"type":"query","request_id":1,"remaining_ms":1,"request":{"query":"ok","limit":1,"extra":1}}"#,
        r#"{"type":"query","request_id":1,"remaining_ms":0,"request":{"query":"ok","limit":1}}"#,
        r#"{"type":"cancel","request_id":2}"#,
    ];
    for body in bodies {
        assert!(
            decode_client_frame(&raw_frame(body.as_bytes())).is_err(),
            "{body}"
        );
    }
    let cancelled = br#"{"request_id":1,"outcome":{"status":"cancelled","extra":1}}"#;
    assert!(decode_terminal_frame(&raw_frame(cancelled), 1).is_err());
    let terminal = encode_terminal_frame(&Terminal {
        request_id: 1,
        outcome: QueryOutcome::Cancelled,
        summary: None,
    })
    .expect("terminal");
    assert_eq!(
        decode_terminal_frame(&terminal, 1)
            .expect("valid cancelled")
            .outcome,
        QueryOutcome::Cancelled
    );
    assert!(decode_client_frame(&terminal).is_err());
}

#[test]
fn whole_worst_escaped_units_stop_at_largest_fitting_rank_prefix() {
    let text = "\u{1}".repeat(MAX_UNIT_TEXT_BYTES);
    let items = (1..=64).map(|rank| hit(rank, &text)).collect();
    let bytes = prepare_complete(1, 1, items, None).expect("bounded terminal");
    assert!(bytes.len() <= MAX_FRAME_BYTES);
    assert_eq!(
        usize::try_from(u32::from_le_bytes(bytes[..4].try_into().expect("prefix")))
            .expect("length")
            + 4,
        bytes.len()
    );
    let mut decoded = decode_terminal_frame(&bytes, 64).expect("complete closed terminal");
    let QueryOutcome::Complete { items, stop, .. } = &mut decoded.outcome else {
        panic!("complete");
    };
    assert_eq!(*stop, StopReason::OutputLimit);
    assert!(!items.is_empty() && items.len() < MAX_ITEMS);
    for item in items.iter() {
        let SearchItem::Hit(card) = item else {
            panic!("hit");
        };
        assert_eq!(card.unit_text, text);
    }
    let next_rank = u8::try_from(items.len() + 1).expect("bounded rank");
    items.push(hit(next_rank, &text));
    assert_eq!(
        encode_terminal_frame(&decoded),
        Err(ProtocolError::FrameTooLarge)
    );
}

#[test]
fn double_escaped_full_units_need_output_stop_without_shrinking_units() {
    let text = "\\".repeat(MAX_UNIT_TEXT_BYTES);
    let bytes = prepare_complete(1, 1, (1..=64).map(|rank| hit(rank, &text)).collect(), None)
        .expect("frame");
    let decoded = decode_terminal_frame(&bytes, 64).expect("terminal");
    let QueryOutcome::Complete { items, stop, .. } = decoded.outcome else {
        panic!("complete");
    };
    assert_eq!(stop, StopReason::OutputLimit);
    assert!(items.len() < 64);
    assert!(items.iter().all(
        |item| matches!(item, SearchItem::Hit(card) if card.unit_text.len() == MAX_UNIT_TEXT_BYTES)
    ));
}

#[test]
fn missing_object_is_a_ranked_gap_and_integrity_codes_cannot_masquerade_as_gaps() {
    let SearchItem::Hit(card) = hit(2, "second") else {
        panic!("hit");
    };
    let gap = SearchGap {
        rank: 2,
        score: card.score,
        evidence: card.evidence,
        code: FailureCode::new("SOURCE_OBJECT_MISSING").expect("code"),
        source_info: None,
    };
    let mut value = terminal(vec![hit(1, "first"), SearchItem::Gap(gap)]);
    let encoded = encode_terminal_frame(&value).expect("encode");
    assert_eq!(decode_terminal_frame(&encoded, 2).expect("decode"), value);
    assert!(decode_terminal_frame(&encoded, 1).is_err());
    let QueryOutcome::Complete { items, .. } = &mut value.outcome else {
        panic!("complete");
    };
    let SearchItem::Gap(gap) = &mut items[1] else {
        panic!("gap");
    };
    gap.code = FailureCode::new("OBJECT_CORRUPT").expect("code");
    assert_eq!(
        encode_terminal_frame(&value),
        Err(ProtocolError::InvalidMessage)
    );
}

#[test]
fn correlation_generation_rank_score_and_exact_utf8_byte_binding_are_required() {
    let good = terminal(vec![hit(1, "😀")]);
    assert!(encode_terminal_frame(&good).is_ok());
    let mut wrong = good.clone();
    wrong.request_id = 2;
    assert_eq!(
        encode_terminal_frame(&wrong),
        Err(ProtocolError::InvalidCorrelation)
    );
    for mutation in 0..6 {
        let mut wrong = good.clone();
        let QueryOutcome::Complete { items, .. } = &mut wrong.outcome else {
            panic!("complete");
        };
        let SearchItem::Hit(card) = &mut items[0] else {
            panic!("hit");
        };
        match mutation {
            0 => card.rank = 2,
            1 => card.score = f32::INFINITY,
            2 => card.evidence.generation = 2,
            3 => card.evidence.end = card.evidence.start + 1,
            4 => card.evidence.source_id = "00000000-0000-0000-0000-000000000000".into(),
            _ => card.evidence.input.bytes = 0,
        }
        assert!(encode_terminal_frame(&wrong).is_err());
    }
}

#[test]
fn invalid_late_item_is_rejected_before_any_output_prefix_is_prepared() {
    let text = "\u{1}".repeat(MAX_UNIT_TEXT_BYTES);
    let mut items: Vec<_> = (1..=64).map(|rank| hit(rank, &text)).collect();
    let SearchItem::Hit(last) = items.last_mut().expect("last") else {
        panic!("hit");
    };
    last.evidence.prepared.digest = "invalid".into();
    assert_eq!(
        prepare_complete(1, 1, items, None),
        Err(ProtocolError::InvalidMessage)
    );
}

#[test]
fn decoded_item_count_and_failure_code_are_bounded() {
    let oversized = terminal((1..=65).map(|rank| hit(rank, "x")).collect());
    let bytes = serde_json::to_vec(&oversized).expect("fixture JSON");
    assert!(decode_terminal_frame(&raw_frame(&bytes), 64).is_err());
    for bad in ["", "raw OS error", "LOWER_case", "CODE\nTEXT"] {
        assert!(FailureCode::new(bad).is_err());
    }
    assert!(FailureCode::new(&"X".repeat(65)).is_err());
}

fn raw_result(selected: u64, scanned: u64, matches: u64, negative: bool) -> RawExactResult {
    let missing = selected - scanned;
    RawExactResult {
        generation: 1,
        selected_sources: selected,
        scanned_sources: scanned,
        missing_sources: missing,
        scanned_bytes: if scanned == 0 { 0 } else { 4096 },
        matches,
        // This fixture emits no items, so a non-zero match/gap total is an
        // explicit presentation cut rather than a complete emitted prefix.
        items: Vec::new(),
        scan_complete: scanned == selected,
        complete_negative: negative,
        stop: if matches + missing == 0 {
            StopReason::Complete
        } else {
            StopReason::OutputLimit
        },
    }
}

#[test]
fn an_explicit_exact_selection_must_name_at_least_one_source() {
    for ids in [
        None,
        Some(vec!["00000000-0000-4000-8000-000000000002".to_owned()]),
    ] {
        assert_eq!(
            RawExactRequest {
                literal: "needle".to_owned(),
                source_ids: ids.clone(),
                limit: 1,
            }
            .validate(),
            Ok(())
        );
        assert_eq!(
            DecodedUtf8Request {
                literal: "needle".to_owned(),
                source_ids: ids,
                limit: 1,
            }
            .validate(),
            Ok(())
        );
    }
    // An empty selection has no scan population, so a vacuous complete scan
    // over nothing could be read as a proven absence. It is refused instead.
    assert_eq!(
        RawExactRequest {
            literal: "needle".to_owned(),
            source_ids: Some(Vec::new()),
            limit: 1,
        }
        .validate(),
        Err(ProtocolError::EmptySelection)
    );
    assert_eq!(
        DecodedUtf8Request {
            literal: "needle".to_owned(),
            source_ids: Some(Vec::new()),
            limit: 1,
        }
        .validate(),
        Err(ProtocolError::EmptySelection)
    );
    assert_eq!(
        ProtocolError::EmptySelection.code(),
        "EXACT_EMPTY_SELECTION"
    );
}

#[test]
fn a_complete_negative_needs_a_non_empty_scanned_population() {
    // Scanned everything, found nothing, over a real population: a negative.
    assert_eq!(raw_result(3, 3, 0, true).validate(1), Ok(()));
    assert_eq!(
        raw_result(3, 3, 0, false).validate(1),
        Err(ProtocolError::InvalidMessage)
    );
    // Zero selected sources: complete scanning is vacuous and proves nothing.
    assert_eq!(raw_result(0, 0, 0, false).validate(1), Ok(()));
    assert_eq!(
        raw_result(0, 0, 0, true).validate(1),
        Err(ProtocolError::InvalidMessage)
    );
    // An incomplete scan is never a negative, whatever the population size.
    assert_eq!(raw_result(3, 2, 0, false).validate(1), Ok(()));
    assert_eq!(
        raw_result(3, 2, 0, true).validate(1),
        Err(ProtocolError::InvalidMessage)
    );
}

#[test]
fn a_window_summary_cannot_claim_a_negative_over_an_empty_selection() {
    let corpus = |selected: u64| QuerySummary {
        published_wanted_revision: 7,
        published_sources: 3,
        published_preparation_gaps: 0,
        selected_sources: selected,
        selected_preparation_gaps: 0,
        control: ControlObservation::Known {
            wanted_revision: 7,
            known_control_lag: false,
        },
    };
    let page = |selected: Option<u64>, negative: bool| WindowPage {
        window_id: 1,
        start_index: 0,
        next_index: 0,
        has_more: false,
        items: Vec::new(),
        page_stop: StopReason::Complete,
        summary: WindowSummary {
            generation: 1,
            retained_items: 0,
            found_matches: Some(0),
            scan_complete: Some(true),
            complete_negative: Some(negative),
            result_stop: StopReason::Complete,
            corpus: selected.map(corpus),
        },
    };
    assert_eq!(page(Some(3), true).validate(), Ok(()));
    assert_eq!(
        page(Some(3), false).validate(),
        Err(ProtocolError::InvalidMessage)
    );
    assert_eq!(page(Some(0), false).validate(), Ok(()));
    assert_eq!(
        page(Some(0), true).validate(),
        Err(ProtocolError::InvalidMessage)
    );
    // Without the corpus facts only an unjustified true can be rejected.
    assert_eq!(page(None, true).validate(), Ok(()));
    assert_eq!(page(None, false).validate(), Ok(()));
}
