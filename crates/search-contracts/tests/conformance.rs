use search_contracts::*;

fn profile(value: &str) -> ProfileId {
    ProfileId::new(value).expect("test profile")
}

fn timestamp(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("test timestamp")
}

fn opaque_id(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("test opaque id")
}

fn opaque_ref(value: &str) -> OpaqueRef {
    OpaqueRef::new(value).expect("test opaque ref")
}

fn receipt(value: &str) -> ReceiptRef {
    ReceiptRef::new(value).expect("test receipt")
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| {
            let text = core::str::from_utf8(pair).expect("ASCII hex");
            u8::from_str_radix(text, 16).expect("valid hex")
        })
        .collect()
}

const fn query_snapshot() -> QuerySnapshotFence {
    QuerySnapshotFence {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        collection_generation_id: None,
        visible_epoch: None,
        collection_route_revision: CollectionRouteRevision::new(2),
        catalog_revision: CatalogRevision::new(3),
        membership_revision: MembershipRevision::new(4),
        reference_portfolio_revision: Some(PortfolioRevision::new(5)),
        access_policy_revision: AccessPolicyRevision::new(6),
        shadow_fence_revision: ShadowFenceRevision::new(7),
        purge_fence_revision: PurgeFenceRevision::new(8),
        overlay_revision: OverlayRevision::new(9),
        observation_cursor_revision: ObservationCursorRevision::new(10),
        observation_freshness: ObservationFreshness {
            state: ObservationFreshnessState::CurrentConfirmed,
            observation_cursor_revision: ObservationCursorRevision::new(11),
            observed_age_ms: None,
        },
        source_view: SourceView::WorkingTreeCurrent(WorkspaceViewSource {
            workspace_instance_id: WorkspaceId::from_bytes([0x21; 16]),
            workspace_view_revision_ref: WorkspaceViewRevisionId::from_bytes([0x22; 16]),
        }),
        workspace_view_revision_ref: Some(WorkspaceViewRevisionId::from_bytes([0x22; 16])),
        lexical_profile_ids: BoundedList::empty(),
        snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([0x44; 32]),
    }
}

fn source_handle() -> SearchSourceHandle {
    SearchSourceHandle {
        handle_id: HandleId::from_bytes([0x31; 16]),
        handle_revision: NonZeroRevision::new(1).expect("nonzero"),
        handle_class: HandleClass::Ephemeral,
        expires_at: Some(timestamp("2026-09-02T12:00:00.000000Z")),
        opaque_token: OpaqueHandleToken::new(&[0x5a; 32]).expect("token"),
    }
}

#[test]
fn exact_recipe_registry_is_closed() {
    let expected = [
        "locate@1",
        "find_text@1",
        "inspect_entity@1",
        "compare_implementations@1",
        "explore_entity@1",
        "corpus_profile@1",
        "corpus_delta@1",
        "provenance@1",
        "compile_exact_scan@1",
        "execute_exact_scan@1",
        "expand_handle@1",
    ];
    assert_eq!(RecipeIdV1::ALL.len(), expected.len());
    for (recipe, expected_wire) in RecipeIdV1::ALL.into_iter().zip(expected) {
        assert_eq!(recipe.as_str(), expected_wire);
        assert_eq!(RecipeIdV1::parse_versioned(expected_wire), Ok(recipe));
    }
    for invalid in ["locate", "locate@2", "LOCATE@1", "vendor@1", ""] {
        assert!(RecipeIdV1::parse_versioned(invalid).is_err());
    }
}

#[test]
fn comparison_axis_registry_is_closed() {
    let expected = [
        "interface",
        "validation",
        "errors",
        "side_effects",
        "tests",
        "callers",
        "documentation",
    ];
    assert_eq!(ComparisonAxis::ALL.len(), expected.len());
    for (axis, expected_wire) in ComparisonAxis::ALL.into_iter().zip(expected) {
        assert_eq!(axis.as_str(), expected_wire);
        assert_eq!(ComparisonAxis::parse(expected_wire), Ok(axis));
    }
    assert!(ComparisonAxis::parse("performance").is_err());
}

#[test]
fn semantic_registries_are_exact_and_reject_unknowns() {
    assert_eq!(AssuranceClass::ALL.len(), 4);
    assert_eq!(ObservationFreshnessState::ALL.len(), 4);
    assert_eq!(EvidenceRole::ALL.len(), 6);
    assert_eq!(Modality::ALL.len(), 6);
    assert_eq!(EntityKind::ALL.len(), 19);
    assert_eq!(SensitivityClass::ALL.len(), 4);
    assert_eq!(DisclosureCeiling::ALL.len(), 3);

    for value in AssuranceClass::ALL {
        assert_eq!(AssuranceClass::parse(value.as_str()), Ok(*value));
    }
    for value in ObservationFreshnessState::ALL {
        assert_eq!(ObservationFreshnessState::parse(value.as_str()), Ok(*value));
    }
    for value in EvidenceRole::ALL {
        assert_eq!(EvidenceRole::parse(value.as_str()), Ok(*value));
    }
    for value in Modality::ALL {
        assert_eq!(Modality::parse(value.as_str()), Ok(*value));
    }
    for value in EntityKind::ALL {
        assert_eq!(EntityKind::parse(value.as_str()), Ok(*value));
    }
    assert!(EntityKind::parse("class").is_err());
    assert!(Modality::parse("video").is_err());
}

#[test]
fn reason_and_error_namespaces_are_closed() {
    assert_eq!(SearchReasonCodeV1::ALL.len(), 31);
    assert_eq!(ProtocolErrorCode::ALL.len(), 10);
    assert_eq!(ContractErrorCode::ALL.len(), 10);

    for reason in SearchReasonCodeV1::ALL {
        assert_eq!(SearchReasonCodeV1::parse(reason.as_str()), Ok(*reason));
    }
    for code in ProtocolErrorCode::ALL {
        assert_eq!(ProtocolErrorCode::parse(code.as_str()), Ok(*code));
    }
    assert!(SearchReasonCodeV1::parse("STALE_SOURCE").is_err());
    assert!(ProtocolErrorCode::parse("FRAME_TOO_BIG").is_err());

    assert!(SearchReasonCodeV1::Stale.is_candidate_forbidden());
    assert!(SearchReasonCodeV1::Unreadable.is_candidate_forbidden());
    assert!(SearchReasonCodeV1::AccessRevoked.is_candidate_forbidden());
    assert!(SearchReasonCodeV1::Purged.is_candidate_forbidden());
    assert!(SearchReasonCodeV1::SourceRevisionUnavailable.is_candidate_forbidden());
    assert!(!SearchReasonCodeV1::IncompleteCoverage.is_candidate_forbidden());
}

#[test]
fn p00_bound_table_is_exact_and_digest_bound() {
    let bounds = ContractBoundsV1::p00().expect("P00 bounds");
    assert_eq!(bounds.bounds_revision.get(), 1);
    assert_eq!(bounds.classes.len(), 24);
    assert_eq!(
        bounds.table_digest.to_string(),
        "8ab611006a1f8cdd5dec9a71433fbb61bd5e24cc2d12569a4fddf78d859f2f82"
    );

    let expected = [
        ("anchor_depth", None, None, Some(16)),
        ("behavior_signature", None, Some(16_384), None),
        ("bound_classes", Some(256), None, None),
        ("canonical", None, Some(8_388_608), None),
        ("collection", Some(4_096), None, None),
        ("display_name", None, Some(512), None),
        ("display_path", None, Some(32_768), None),
        ("expression", None, Some(16_384), None),
        ("facet_value", None, Some(4_096), None),
        ("frame", None, Some(8_388_608), None),
        ("json_depth", None, None, Some(64)),
        ("map", Some(1_024), None, None),
        ("metadata_entries", Some(64), None, None),
        ("metadata_key", None, Some(128), None),
        ("name", None, Some(1_024), None),
        ("observation", None, Some(65_536), None),
        ("opaque_id", None, Some(256), None),
        ("opaque_ref", None, Some(4_096), None),
        ("profile_id", None, Some(256), None),
        ("protocol_in_flight", Some(32), None, None),
        ("raw", None, Some(8_388_608), None),
        ("reason_codes", Some(64), None, None),
        ("set", Some(4_096), None, None),
        ("symbol_key", None, Some(4_096), None),
    ];
    for (name, items, bytes, depth) in expected {
        let class = bounds.class(&profile(name)).expect("registered class");
        assert_eq!(class.max_items, items);
        assert_eq!(class.max_bytes, bytes);
        assert_eq!(class.max_depth, depth);
        class.validate().expect("valid class");
    }
    assert!(bounds.class(&profile("unregistered")).is_err());
    assert!(
        LimitClass {
            max_items: None,
            max_bytes: None,
            max_depth: None,
        }
        .validate()
        .is_err()
    );
}

#[test]
fn bounded_collections_enforce_limits_and_duplicate_policy() {
    assert_eq!(BoundedText::<3>::new("abc").expect("fits").as_str(), "abc");
    assert_eq!(
        BoundedText::<3>::new("abcd").expect_err("too long").kind(),
        ContractErrorKind::TooLong
    );
    assert_eq!(
        BoundedText::<3>::new_non_empty("")
            .expect_err("empty")
            .kind(),
        ContractErrorKind::Empty
    );
    assert_eq!(
        BoundedBytes::<2>::new([1, 2, 3].to_vec())
            .expect_err("too many bytes")
            .kind(),
        ContractErrorKind::TooLong
    );

    let mut list = BoundedList::<u8, 2>::new(vec![1, 2]).expect("list");
    assert_eq!(
        list.try_push(3).expect_err("bounded").kind(),
        ContractErrorKind::TooManyItems
    );
    assert_eq!(
        BoundedSet::<u8, 2>::from_items([1, 1])
            .expect_err("duplicates rejected")
            .kind(),
        ContractErrorKind::Duplicate
    );
    assert_eq!(
        BoundedMap::<u8, u8, 2>::from_entries([(1, 1), (1, 2)])
            .expect_err("duplicate keys rejected")
            .kind(),
        ContractErrorKind::Duplicate
    );
}

#[test]
fn uuid_ids_require_lowercase_hyphenated_canonical_form() {
    let input = "00112233-4455-6677-8899-aabbccddeeff";
    let id = SourceId::parse(input).expect("canonical UUID");
    assert_eq!(id.to_string(), input);
    assert_eq!(
        id.as_bytes(),
        &decode_hex("00112233445566778899aabbccddeeff")[..]
    );
    assert!(SourceId::parse("00112233445566778899aabbccddeeff").is_err());
    assert!(SourceId::parse("00112233-4455-6677-8899-AABBCCDDEEFF").is_err());
    assert!(SourceId::parse("00112233_4455_6677_8899_aabbccddeeff").is_err());
}

#[test]
fn digest_wrappers_require_fixed_lowercase_hex() {
    let text = "11".repeat(32);
    let digest = Blake3Digest32::parse_hex(&text).expect("digest");
    assert_eq!(digest.to_string(), text);
    assert!(Blake3Digest32::parse_hex(&"11".repeat(31)).is_err());
    assert!(Blake3Digest32::parse_hex(&"AA".repeat(32)).is_err());
}

#[test]
fn epoch_and_revision_sentinels_fail_closed() {
    assert!(OwnerEpoch::new(0).is_err());
    assert_eq!(OwnerEpoch::new(1).expect("owner epoch").get(), 1);
    assert!(Epoch::new(-1).is_err());
    assert!(Epoch::new(i64::MAX).is_err());
    assert_eq!(Epoch::new(0).expect("epoch zero").get(), 0);
    assert!(
        Epoch::new(i64::MAX - 1)
            .expect("last epoch")
            .checked_next()
            .is_err()
    );
    assert!(NonZeroRevision::new(0).is_err());
    assert!(
        NonZeroRevision::new(u64::MAX)
            .expect("max revision")
            .checked_next()
            .is_err()
    );
}

#[test]
fn opaque_handle_tokens_are_bounded_canonical_and_redacted() {
    assert!(OpaqueHandleToken::new(&[0; 31]).is_err());
    assert!(OpaqueHandleToken::new(&[0; 65]).is_err());
    let token = OpaqueHandleToken::new(&[0xff; 32]).expect("token");
    let encoded = token.encoded();
    assert!(!encoded.contains('='));
    assert_eq!(
        OpaqueHandleToken::parse_base64url(&encoded),
        Ok(token.clone())
    );
    assert!(OpaqueHandleToken::parse_base64url(&(encoded.clone() + "=")).is_err());
    let debug = format!("{token:?}");
    assert!(debug.contains("<redacted>"));
    assert!(debug.contains("len: 32"));
    assert!(!debug.contains(&encoded));
}

#[test]
fn metadata_keys_are_closed_lowercase_tokens() {
    for valid in ["a", "source.kind", "a-b_c.9"] {
        assert_eq!(MetadataKey::parse(valid).expect("key").as_str(), valid);
    }
    for invalid in ["", "A", "9abc", "a/b", "a b", "å", "a="] {
        assert!(MetadataKey::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(MetadataKey::parse(format!("a{}", "b".repeat(128))).is_err());
}

#[test]
fn utc_timestamp_is_fixed_precision_and_calendar_valid() {
    let valid = "2024-02-29T23:59:59.123456Z";
    assert_eq!(timestamp(valid).as_str(), valid);
    for invalid in [
        "0000-01-01T00:00:00.000000Z",
        "2023-02-29T00:00:00.000000Z",
        "2024-13-01T00:00:00.000000Z",
        "2024-01-01T24:00:00.000000Z",
        "2024-01-01T00:60:00.000000Z",
        "2024-01-01T00:00:60.000000Z",
        "2024-01-01T00:00:00.00000Z",
        "2024-01-01T00:00:00.000000+00:00",
    ] {
        assert!(UtcTimestamp::parse(invalid).is_err(), "accepted {invalid}");
    }
}

#[test]
fn canonical_json_is_deterministic_injective_and_strict() {
    let object = CanonicalValue::Object(
        BoundedMap::from_entries([
            (
                CanonicalKey::new_non_empty("b").expect("key"),
                CanonicalValue::U64(2),
            ),
            (
                CanonicalKey::new_non_empty("a").expect("key"),
                CanonicalValue::U64(1),
            ),
        ])
        .expect("object"),
    );
    let encoded = to_canonical_json(&object).expect("JSON");
    assert_eq!(encoded.as_slice(), br#"{"a":1,"b":2}"#);
    assert_eq!(parse_canonical_json(encoded.as_slice()), Ok(object));

    let bytes = CanonicalValue::Bytes(BoundedBytes::new(vec![0, 1, 2, 255]).expect("bytes"));
    let bytes_json = to_canonical_json(&bytes).expect("bytes JSON");
    assert_eq!(bytes_json.as_slice(), br#"{"$bytes":"AAEC_w"}"#);
    assert_eq!(parse_canonical_json(bytes_json.as_slice()), Ok(bytes));

    let reserved = CanonicalValue::Object(
        BoundedMap::from_entries([(
            CanonicalKey::new_non_empty("$bytes").expect("key"),
            CanonicalValue::Text(CanonicalText::new("AA").expect("text")),
        )])
        .expect("map"),
    );
    assert_eq!(
        to_canonical_json(&reserved)
            .expect_err("reserved bytes tag must be unambiguous")
            .kind(),
        ContractErrorKind::NonCanonical
    );

    assert_eq!(
        parse_canonical_json(br#"{"b":2,"a":1}"#)
            .expect_err("key order")
            .kind(),
        ContractErrorKind::NonCanonical
    );
    assert!(parse_canonical_json(br#"{"a":1, "b":2}"#).is_err());
    assert!(parse_canonical_json(br#"{"a":1,"a":2}"#).is_err());
    assert!(parse_canonical_json(b"01").is_err());
    assert!(parse_canonical_json(b"1.0").is_err());
    assert!(parse_canonical_json(br#""\u0061""#).is_err());
    assert_eq!(
        CanonicalValue::I64(0)
            .validate()
            .expect_err("nonnegative I64 is noncanonical")
            .kind(),
        ContractErrorKind::NonCanonical
    );
}

#[test]
fn canonical_json_depth_is_bounded() {
    let mut value = CanonicalValue::Null;
    for _ in 0..=MAX_CANONICAL_DEPTH {
        value = CanonicalValue::Array(BoundedList::new(vec![value]).expect("array"));
    }
    assert_eq!(
        value.validate().expect_err("depth exceeded").kind(),
        ContractErrorKind::DepthExceeded
    );
}

#[test]
fn deterministic_cbor_uses_shortest_forms_and_rfc_key_order() {
    let object = CanonicalValue::Object(
        BoundedMap::from_entries([
            (
                CanonicalKey::new_non_empty("bb").expect("key"),
                CanonicalValue::U64(2),
            ),
            (
                CanonicalKey::new_non_empty("a").expect("key"),
                CanonicalValue::U64(1),
            ),
        ])
        .expect("object"),
    );
    let encoded = to_canonical_cbor(&object).expect("CBOR");
    assert_eq!(encoded.as_slice(), &decode_hex("a261610162626202"));
    assert_eq!(parse_canonical_cbor(encoded.as_slice()), Ok(object));

    assert_eq!(
        parse_canonical_cbor(&[0x18, 0x17])
            .expect_err("nonminimal integer")
            .kind(),
        ContractErrorKind::NonCanonical
    );
    assert!(parse_canonical_cbor(&[0x9f, 0x01, 0xff]).is_err());
    assert_eq!(
        parse_canonical_cbor(&decode_hex("a262626202616101"))
            .expect_err("noncanonical key order")
            .kind(),
        ContractErrorKind::NonCanonical
    );
}

#[test]
fn domain_separation_is_explicit_and_bounded() {
    let value = CanonicalValue::U64(7);
    let preimage = domain_separated_preimage("eliot-search/test/v1", &value).expect("preimage");
    assert_eq!(preimage.as_slice(), b"eliot-search/test/v1\0\x07");
    assert!(domain_separated_preimage("", &value).is_err());
    assert!(domain_separated_preimage("eliot-search/\u{2603}", &value).is_err());
}

#[test]
fn query_snapshot_fingerprint_input_matches_the_frozen_golden() {
    let preimage = query_snapshot()
        .canonical_fingerprint_input()
        .expect("snapshot preimage");
    let expected = concat!(
        "656c696f742d7365617263682f71756572792d736e617073686f742d66696e6765727072696e742f763100",
        "b06b736f757263655f76696577a174776f726b696e675f747265655f63757272656e74a275776f726b73706163655f696e7374616e63655f69645021212121212121212121212121212121781b776f726b73706163655f766965775f7265766973696f6e5f7265665022222222222222222222222222222222",
        "6d76697369626c655f65706f6368f6",
        "70636174616c6f675f7265766973696f6e03",
        "706f7665726c61795f7265766973696f6e09",
        "736c65786963616c5f70726f66696c655f69647380",
        "736d656d626572736869705f7265766973696f6e04",
        "7470757267655f66656e63655f7265766973696f6e08",
        "756f62736572766174696f6e5f66726573686e657373a36573746174657163757272656e745f636f6e6669726d65646f6f627365727665645f6167655f6d73f6781b6f62736572766174696f6e5f637572736f725f7265766973696f6e0b",
        "75736861646f775f66656e63655f7265766973696f6e07",
        "766163636573735f706f6c6963795f7265766973696f6e06",
        "7818636f6c6c656374696f6e5f67656e65726174696f6e5f6964f6",
        "7819636f6c6c656374696f6e5f726f7574655f7265766973696f6e02",
        "781b696e7374616c6c6174696f6e5f696e6361726e6174696f6e5f69645001010101010101010101010101010101",
        "781b6f62736572766174696f6e5f637572736f725f7265766973696f6e0a",
        "781b776f726b73706163655f766965775f7265766973696f6e5f7265665022222222222222222222222222222222",
        "781c7265666572656e63655f706f7274666f6c696f5f7265766973696f6e05"
    );
    assert_eq!(preimage.as_slice(), decode_hex(expected));
    assert_eq!(preimage.len(), 628);
}

#[test]
fn protocol_range_selects_highest_common_version() {
    let local = ProtocolRange::new(
        ProtocolVersion { major: 1, minor: 2 },
        ProtocolVersion { major: 2, minor: 5 },
    )
    .expect("range");
    let peer = ProtocolRange::new(
        ProtocolVersion { major: 1, minor: 4 },
        ProtocolVersion { major: 2, minor: 3 },
    )
    .expect("range");
    assert_eq!(
        local.negotiate(peer),
        Ok(ProtocolVersion { major: 2, minor: 3 })
    );
    assert!(
        ProtocolRange::new(
            ProtocolVersion { major: 2, minor: 0 },
            ProtocolVersion { major: 1, minor: 9 },
        )
        .is_err()
    );
    let no_overlap = ProtocolRange::new(
        ProtocolVersion { major: 3, minor: 0 },
        ProtocolVersion { major: 3, minor: 1 },
    )
    .expect("range");
    assert_eq!(
        local.negotiate(no_overlap),
        Err(ProtocolErrorCode::ProtocolVersionMismatch)
    );
}

#[test]
fn protocol_frame_is_little_endian_bounded_and_exact_length() {
    let payload = JsonFramePayload::new(br#"{"ok":true}"#.to_vec()).expect("payload");
    let frame = encode_json_frame(&payload).expect("frame");
    assert_eq!(&frame.as_slice()[..4], &11_u32.to_le_bytes());
    assert_eq!(decode_json_frame(frame.as_slice()), Ok(payload));

    assert_eq!(
        decode_json_frame(&[1, 0, 0]).expect_err("short frame"),
        ProtocolErrorCode::InvalidEnvelope
    );
    assert_eq!(
        decode_json_frame(&[2, 0, 0, 0, b'{']).expect_err("length mismatch"),
        ProtocolErrorCode::InvalidEnvelope
    );
    assert_eq!(
        JsonFramePayload::new(vec![0xff]).expect_err("UTF-8 required"),
        ProtocolErrorCode::InvalidEnvelope
    );
    assert_eq!(
        JsonFramePayload::new(vec![b'x'; MAX_FRAME_BYTES]).expect_err("frame cap includes prefix"),
        ProtocolErrorCode::FrameTooLarge
    );
}

#[test]
fn provider_envelope_rejects_tag_and_version_mismatch() {
    let body = ProviderBodyV1::Cancel(CancelBody {
        target_request_id: RequestId::from_bytes([7; 16]),
    });
    let mut envelope = ProviderEnvelope {
        protocol_major: 1,
        protocol_minor: 0,
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        binding_id: BindingId::from_bytes([2; 16]),
        connection_sequence: 1,
        request_id: RequestId::from_bytes([3; 16]),
        message_kind: MessageKind::Cancel,
        relative_deadline_ms: Some(100),
        body,
    };
    let supported = ProtocolRange::new(
        ProtocolVersion { major: 1, minor: 0 },
        ProtocolVersion { major: 1, minor: 1 },
    )
    .expect("range");
    envelope
        .validate_version_and_limits(supported)
        .expect("valid envelope");
    envelope.message_kind = MessageKind::Result;
    assert_eq!(
        envelope.validate().expect_err("tag mismatch").kind(),
        ContractErrorKind::InvalidTaggedVariant
    );
    envelope.message_kind = MessageKind::Cancel;
    envelope.protocol_major = 2;
    assert_eq!(
        envelope
            .validate_version_and_limits(supported)
            .expect_err("unsupported version")
            .code(),
        ContractErrorCode::ContractVersionMismatch
    );
}

#[test]
fn observation_and_progress_shapes_reject_contradictions() {
    ObservationFreshness {
        state: ObservationFreshnessState::ObservedWithAge,
        observation_cursor_revision: ObservationCursorRevision::new(1),
        observed_age_ms: Some(10),
    }
    .validate()
    .expect("aged observation");
    assert!(
        ObservationFreshness {
            state: ObservationFreshnessState::ObservedWithAge,
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observed_age_ms: None,
        }
        .validate()
        .is_err()
    );
    assert!(
        ObservationFreshness {
            state: ObservationFreshnessState::CurrentConfirmed,
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observed_age_ms: Some(0),
        }
        .validate()
        .is_err()
    );

    BoundedProgressCounts {
        completed_legs: 2,
        total_planned_legs: 3,
        nominated_candidates: 4,
        validated_candidates: 3,
        omitted_or_failed_legs: 1,
    }
    .validate()
    .expect("valid counts");
    assert!(
        BoundedProgressCounts {
            completed_legs: 3,
            total_planned_legs: 3,
            nominated_candidates: 1,
            validated_candidates: 2,
            omitted_or_failed_legs: 1,
        }
        .validate()
        .is_err()
    );
}

#[test]
fn grant_and_task_expiry_are_strictly_ordered() {
    let grant = SearchReadGrantClaims {
        grant_id: GrantId::from_bytes([1; 16]),
        installation_id: InstallationId::from_bytes([2; 16]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([3; 16]),
        binding_id: BindingId::from_bytes([4; 16]),
        principal_opaque_id: opaque_id("principal"),
        client_scope_ref: opaque_ref("scope"),
        scope_domain_id: ScopeDomainId::from_bytes([5; 16]),
        allowed_membership_ids: BoundedSet::empty(),
        allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
        reference_portfolio_revision: None,
        allowed_access_partitions: BoundedSet::empty(),
        allowed_modalities: BoundedSet::empty(),
        permitted_recipe_families: BoundedSet::empty(),
        maximum_budget_class: profile("interactive"),
        sensitivity_ceiling: SensitivityClass::Project,
        disclosure_ceiling: DisclosureCeiling::LocalOnly,
        source_read_permission: true,
        exact_scan_permission: false,
        issued_boot_id: opaque_id("boot"),
        issued_at: timestamp("2026-09-02T10:00:00.000000Z"),
        expires_at: timestamp("2026-09-02T10:00:01.000000Z"),
        nonce: opaque_id("nonce"),
        revocation_generation: 0,
    };
    grant.validate_shape().expect("grant");
    let mut invalid = grant;
    invalid.expires_at = invalid.issued_at.clone();
    assert!(invalid.validate_shape().is_err());
}

#[test]
fn snapshot_index_pair_and_lexical_profile_rules_are_closed() {
    let mut snapshot = query_snapshot();
    snapshot.validate().expect("direct-only snapshot");
    snapshot.visible_epoch = Some(Epoch::new(1).expect("epoch"));
    assert!(snapshot.validate().is_err());

    snapshot.collection_generation_id = Some(CollectionGenerationId::from_bytes([2; 16]));
    snapshot.validate().expect("indexed pair");
    snapshot.collection_generation_id = None;
    snapshot.visible_epoch = None;
    snapshot.lexical_profile_ids = BoundedList::new(vec![profile("lexical-v1")]).expect("list");
    assert!(snapshot.validate().is_err());
}

#[test]
fn native_anchor_validation_covers_ranges_coordinates_and_depth() {
    assert!(
        NativeAnchor::TextBytes(TextBytesAnchor {
            content_digest: Blake3Digest32::from_bytes([1; 32]),
            byte_start_0: 3,
            byte_end_exclusive_0: 2,
        })
        .validate()
        .is_err()
    );
    assert!(
        NativeAnchor::BufferRange(BufferRangeAnchor {
            buffer_snapshot_id: BufferSnapshotId::from_bytes([1; 16]),
            buffer_version: 1,
            position_encoding: PositionEncoding::Utf16CodeUnits,
            start_line_0: 2,
            start_character_0: 0,
            end_line_0: 1,
            end_character_0: 99,
        })
        .validate()
        .is_err()
    );
    assert!(FiniteF64::new(f64::NAN).is_err());
    assert!(FiniteF64::new(f64::INFINITY).is_err());
    assert!(
        NativeAnchor::PdfRegion(PdfRegionAnchor {
            source_revision_id: SourceRevisionId::from_bytes([1; 16]),
            page_1: 0,
            x0: FiniteF64::new(0.0).expect("finite"),
            y0: FiniteF64::new(0.0).expect("finite"),
            x1: FiniteF64::new(1.0).expect("finite"),
            y1: FiniteF64::new(1.0).expect("finite"),
        })
        .validate()
        .is_err()
    );

    let mut anchor = NativeAnchor::TextBytes(TextBytesAnchor {
        content_digest: Blake3Digest32::from_bytes([1; 32]),
        byte_start_0: 0,
        byte_end_exclusive_0: 1,
    });
    for depth in 0..=MAX_ANCHOR_DEPTH {
        if depth < MAX_ANCHOR_DEPTH {
            anchor = NativeAnchor::ArchiveMember(ArchiveMemberAnchor {
                archive_revision_id: SourceRevisionId::from_bytes([2; 16]),
                member_path_bytes: BoundedBytes::new(b"member".to_vec()).expect("path"),
                nested_anchor: Box::new(anchor),
            });
        }
    }
    anchor.validate().expect("maximum depth accepted");
    let too_deep = NativeAnchor::ArchiveMember(ArchiveMemberAnchor {
        archive_revision_id: SourceRevisionId::from_bytes([2; 16]),
        member_path_bytes: BoundedBytes::new(b"member".to_vec()).expect("path"),
        nested_anchor: Box::new(anchor),
    });
    assert_eq!(
        too_deep.validate().expect_err("depth exceeded").kind(),
        ContractErrorKind::DepthExceeded
    );
}

#[test]
fn exact_complete_negative_requires_a_complete_zero_failure_denominator() {
    let plan_ref = ExactScanPlanRef {
        plan_id: PlanId::from_bytes([1; 16]),
        plan_fingerprint: PlanFingerprint::from_bytes([2; 32]),
    };
    let mut report = ExactExecutionReport {
        plan_ref,
        matched_items: BoundedList::empty(),
        scanned_items: 2,
        scanned_bytes: 10,
        unreadable_items: BoundedList::empty(),
        changed_or_unavailable_items: BoundedList::empty(),
        timed_out: false,
        cancelled: false,
        scope_drifted: false,
        coverage: CoverageDenominatorKind::CompleteScope,
        conclusion: ExactConclusion::NoMatchInCompleteScope,
        receipt_ref: receipt("exact-receipt"),
    };
    report.validate().expect("complete negative");
    report.timed_out = true;
    assert!(report.validate().is_err());
    report.timed_out = false;
    report.matched_items = BoundedList::new(vec![ExactMatch {
        source_revision_ref: SourceRevisionRef {
            source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
            source_id: SourceId::from_bytes([2; 16]),
            revision_id: SourceRevisionId::from_bytes([3; 16]),
            content_digest: Blake3Digest32::from_bytes([4; 32]),
            byte_length: 10,
        },
        native_anchor: NativeAnchor::TextBytes(TextBytesAnchor {
            content_digest: Blake3Digest32::from_bytes([4; 32]),
            byte_start_0: 0,
            byte_end_exclusive_0: 1,
        }),
        match_digest: Blake3Digest32::from_bytes([5; 32]),
        matched_byte_length: 1,
        predicate_profile_id: profile("literal-v1"),
        assurance: AssuranceClass::ExactBytes,
        source_handle: source_handle(),
    }])
    .expect("matches");
    assert!(report.validate().is_err());
}

#[test]
fn validated_candidates_cannot_carry_gap_only_reasons() {
    let ranking = BoundedNonContentRankingTrace {
        fusion_profile_id: FusionProfileId::new("fusion-v1").expect("fusion profile"),
        fused_rank: 1,
        exact_or_entity_boost: ExactOrEntityBoost::None,
        evidence_role_priority: 1,
        portfolio_priority: 1,
        lineage_diversity_action: LineageDiversityAction::Retained,
        deterministic_tie_break_digest: Blake3Digest32::from_bytes([9; 32]),
    };
    let mut candidate = ValidatedSearchCandidate {
        candidate_id: CandidateId::from_bytes([1; 16]),
        source_handle: source_handle(),
        evidence_role: EvidenceRole::Definition,
        entity_kind: Some(EntityKind::Function),
        assurance: AssuranceClass::ExactBytes,
        freshness: ObservationFreshnessState::CurrentConfirmed,
        ranking_trace: ranking,
        reason_codes: BoundedSet::empty(),
        candidate_validation_receipt_ref: receipt("candidate-receipt"),
    };
    candidate.validate().expect("candidate");
    candidate.reason_codes = BoundedSet::from_items([SearchReasonCodeV1::Stale]).expect("reasons");
    assert_eq!(
        candidate
            .validate()
            .expect_err("stale is not evidence")
            .kind(),
        ContractErrorKind::ForbiddenCandidateReason
    );
}

#[test]
fn ambiguity_is_explicit_non_evidence_and_nonempty() {
    let candidate = AmbiguousSubjectCandidate {
        source_handle: source_handle(),
        entity_kind: EntityKind::Function,
        match_basis: MatchBasis::ExactName,
        disambiguation_summary: BoundedNonContentMetadata::empty(),
    };
    candidate.validate().expect("ambiguous candidate");
    let set = SubjectAmbiguitySet {
        requested_selector_digest: Blake3Digest32::from_bytes([1; 32]),
        candidates: BoundedList::new(vec![candidate]).expect("candidate list"),
        reason_code: SearchReasonCodeV1::AmbiguousSubject,
    };
    set.validate().expect("ambiguity set");
    assert!(
        SubjectAmbiguitySet {
            requested_selector_digest: Blake3Digest32::from_bytes([1; 32]),
            candidates: BoundedList::empty(),
            reason_code: SearchReasonCodeV1::AmbiguousSubject,
        }
        .validate()
        .is_err()
    );
    let explicit = AmbiguousSubjectCandidate {
        source_handle: source_handle(),
        entity_kind: EntityKind::Function,
        match_basis: MatchBasis::ExplicitHandle,
        disambiguation_summary: BoundedNonContentMetadata::empty(),
    };
    assert!(explicit.validate().is_err());
}

#[test]
fn secure_erase_claim_requires_evidence_and_nonclaim_forbids_it() {
    PhysicalSecureErase {
        status: SecureEraseStatus::NotGuaranteed,
        evidence_ref: None,
    }
    .validate()
    .expect("honest nonclaim");
    PhysicalSecureErase {
        status: SecureEraseStatus::EvidenceAvailable,
        evidence_ref: Some(receipt("erase-evidence")),
    }
    .validate()
    .expect("evidence claim");
    assert!(
        PhysicalSecureErase {
            status: SecureEraseStatus::EvidenceAvailable,
            evidence_ref: None,
        }
        .validate()
        .is_err()
    );
    assert!(
        PhysicalSecureErase {
            status: SecureEraseStatus::NotGuaranteed,
            evidence_ref: Some(receipt("contradictory")),
        }
        .validate()
        .is_err()
    );
}

#[test]
fn cutover_receipt_enforces_terminal_and_time_state() {
    let final_view = SourceViewRef {
        source_view_digest: Blake3Digest32::from_bytes([1; 32]),
        workspace_view_revision_ref: None,
    };
    let mut receipt_value = SourceOwnerCutoverReceipt {
        protocol: SourceOwnerCutoverProtocolV1,
        cutover: SourceOwnerCutover {
            cutover_id: CutoverId::from_bytes([1; 16]),
            source_namespace_id: SourceNamespaceId::from_bytes([2; 16]),
            identity_mapping_digest: Blake3Digest32::from_bytes([3; 32]),
            prepared_at: timestamp("2026-09-02T10:00:00.000000Z"),
            effective_at: timestamp("2026-09-02T10:00:01.000000Z"),
        },
        old_owner: OldSourceOwnerFence {
            owner_system_id: opaque_id("old-owner"),
            source_owner_generation_before_fence: SourceOwnerGeneration::from_bytes([4; 32]),
            fence_revision: NonZeroRevision::new(1).expect("revision"),
            final_source_view_ref: final_view,
            final_revision_set_digest: Blake3Digest32::from_bytes([5; 32]),
            terminal_status: NamespaceOwnershipStatus::Fenced,
        },
        new_owner: NewSourceOwnerActivation {
            owner_system_id: opaque_id("new-owner"),
            source_owner_generation_after_activation: SourceOwnerGeneration::from_bytes([6; 32]),
            activation_revision: NonZeroRevision::new(2).expect("revision"),
            admitted_revision_set_digest: Blake3Digest32::from_bytes([7; 32]),
            status: NamespaceOwnershipStatus::Active,
        },
        validation: CutoverValidation {
            compatibility_receipt_refs: BoundedList::empty(),
            integrity_receipt_refs: BoundedList::empty(),
            unresolved_sources_and_reasons: BoundedList::empty(),
        },
        authorization: CutoverAuthorization {
            old_owner_authorization_ref: opaque_ref("old-auth"),
            new_owner_authorization_ref: opaque_ref("new-auth"),
            issued_at: timestamp("2026-09-02T09:59:59.000000Z"),
        },
    };
    receipt_value.validate().expect("cutover");
    receipt_value.old_owner.terminal_status = NamespaceOwnershipStatus::Active;
    assert!(receipt_value.validate().is_err());
}

#[test]
fn unresolved_source_requires_machine_readable_reason() {
    let source_id = SourceId::from_bytes([1; 16]);
    assert!(UnresolvedSource::new(source_id, BoundedSet::empty()).is_err());
    let unresolved = UnresolvedSource::new(
        source_id,
        BoundedSet::from_items([SearchReasonCodeV1::SourceUnstable]).expect("reason"),
    )
    .expect("unresolved source");
    assert_eq!(unresolved.source_id, source_id);
}

#[test]
fn recipe_and_result_families_must_match_their_tags() {
    let body = RecipeBodyV1::Locate(LocateRecipe {
        subject: SubjectSelector::Path(PathSelector {
            workspace_id: WorkspaceId::from_bytes([1; 16]),
            display_path: BoundedDisplayPath::new("src/lib.rs").expect("path"),
        }),
        evidence_roles: BoundedSet::empty(),
    });
    assert!(
        SearchRecipeRequest::new(
            RequestId::from_bytes([1; 16]),
            RecipeIdV1::FindText,
            SourceView::RetainedRevision(SourceRevisionId::from_bytes([2; 16])),
            RequestedScope::ExplicitMemberships(BoundedList::empty()),
            profile("interactive"),
            body.clone(),
        )
        .is_err()
    );
    assert!(
        SearchRecipeRequest::new(
            RequestId::from_bytes([1; 16]),
            RecipeIdV1::Locate,
            SourceView::RetainedRevision(SourceRevisionId::from_bytes([2; 16])),
            RequestedScope::ExplicitMemberships(BoundedList::empty()),
            profile("interactive"),
            body,
        )
        .is_ok()
    );
}

#[test]
fn protocol_baseline_limits_disable_compression_and_fragmentation() {
    let limits = ProtocolLimits::p00().expect("limits");
    assert_eq!(limits.frame_bytes, 8_388_608);
    assert_eq!(limits.in_flight_requests, 32);
    assert!(!limits.compression_enabled);
    assert!(!limits.fragmented_assembly_enabled);
}

macro_rules! assert_wire_registry {
    ($name:ident { $($variant:path => $wire:literal),+ $(,)? }) => {{
        let expected = [$(($variant, $wire)),+];
        assert_eq!($name::ALL, expected.map(|(variant, _)| variant).as_slice());
        for (variant, wire) in expected {
            assert_eq!(variant.as_str(), wire);
            assert_eq!($name::parse(wire), Ok(variant));
        }
        let error = $name::parse("__unknown_wire_value__").expect_err("registry is closed");
        assert_eq!(error.kind(), ContractErrorKind::InvalidCharacter);
        assert_eq!(error.field(), stringify!($name));
    }};
}

#[test]
fn canonical_wire_registries_1_are_exact_and_closed() {
    assert_wire_registry!(
        ExactOrEntityBoost {
            ExactOrEntityBoost::None => "none",
            ExactOrEntityBoost::ExactName => "exact_name",
            ExactOrEntityBoost::QualifiedName => "qualified_name",
            ExactOrEntityBoost::EntityKind => "entity_kind",
        }
    );
    assert_wire_registry!(
        LineageDiversityAction {
            LineageDiversityAction::Retained => "retained",
            LineageDiversityAction::Collapsed => "collapsed",
            LineageDiversityAction::Capped => "capped",
        }
    );
}

#[test]
fn ids_wire_registries_1_are_exact_and_closed() {
    assert_wire_registry!(
        DigestAlgorithm {
            DigestAlgorithm::Blake3_256 => "blake3_256",
            DigestAlgorithm::Sha256 => "sha256",
        }
    );
}

#[test]
fn lifecycle_wire_registries_1_are_exact_and_closed() {
    assert_wire_registry!(
        LifecycleRecordStatus {
            LifecycleRecordStatus::Active => "ACTIVE",
            LifecycleRecordStatus::Revoked => "REVOKED",
            LifecycleRecordStatus::Expired => "EXPIRED",
        }
    );
    assert_wire_registry!(
        SecurityMutationPhase {
            SecurityMutationPhase::Acquired => "ACQUIRED",
            SecurityMutationPhase::DurableCommitted => "DURABLE_COMMITTED",
            SecurityMutationPhase::LiveSnapshotPublished => "LIVE_SNAPSHOT_PUBLISHED",
            SecurityMutationPhase::DependentsInvalidated => "DEPENDENTS_INVALIDATED",
            SecurityMutationPhase::Acknowledged => "ACKNOWLEDGED",
            SecurityMutationPhase::FailClosed => "FAIL_CLOSED",
        }
    );
    assert_wire_registry!(
        PublicationIntentState {
            PublicationIntentState::Prepared => "PREPARED",
            PublicationIntentState::IntentDurable => "INTENT_DURABLE",
            PublicationIntentState::NewPointsAcknowledged => "NEW_POINTS_ACKNOWLEDGED",
            PublicationIntentState::OldPointsClosedAcknowledged => "OLD_POINTS_CLOSED_ACKNOWLEDGED",
            PublicationIntentState::ReadbackVerified => "READBACK_VERIFIED",
            PublicationIntentState::ControlCommitted => "CONTROL_COMMITTED",
            PublicationIntentState::Reclaimable => "RECLAIMABLE",
            PublicationIntentState::Compensating => "COMPENSATING",
            PublicationIntentState::Aborted => "ABORTED",
            PublicationIntentState::InvalidationOnlyCommitted => "INVALIDATION_ONLY_COMMITTED",
            PublicationIntentState::PublicationBlocked => "PUBLICATION_BLOCKED",
        }
    );
    assert_wire_registry!(
        CompletionState {
            CompletionState::Pending => "pending",
            CompletionState::Complete => "complete",
            CompletionState::Failed => "failed",
        }
    );
    assert_wire_registry!(
        DeletionState {
            DeletionState::NotApplicable => "not_applicable",
            DeletionState::Pending => "pending",
            DeletionState::Complete => "complete",
            DeletionState::Partial => "partial",
            DeletionState::Failed => "failed",
        }
    );
}

#[test]
fn lifecycle_wire_registries_2_are_exact_and_closed() {
    assert_wire_registry!(
        BackupSnapshotStatus {
            BackupSnapshotStatus::NotPresent => "not_present",
            BackupSnapshotStatus::Pending => "pending",
            BackupSnapshotStatus::RetainedTombstone => "retained_tombstone",
            BackupSnapshotStatus::Unresolved => "unresolved",
        }
    );
    assert_wire_registry!(
        SecureEraseStatus {
            SecureEraseStatus::NotGuaranteed => "not_guaranteed",
            SecureEraseStatus::EvidenceAvailable => "evidence_available",
        }
    );
    assert_wire_registry!(
        RestoreState {
            RestoreState::RestorePendingRevalidation => "RESTORE_PENDING_REVALIDATION",
            RestoreState::DirectOnly => "DIRECT_ONLY",
            RestoreState::IndexedAdmitted => "INDEXED_ADMITTED",
            RestoreState::Quarantined => "QUARANTINED",
        }
    );
    assert_wire_registry!(
        OptionalProviderLifecycleState {
            OptionalProviderLifecycleState::Absent => "absent",
            OptionalProviderLifecycleState::Stopped => "stopped",
            OptionalProviderLifecycleState::Starting => "starting",
            OptionalProviderLifecycleState::Ready => "ready",
            OptionalProviderLifecycleState::Degraded => "degraded",
            OptionalProviderLifecycleState::Quarantined => "quarantined",
        }
    );
}

#[test]
fn protocol_wire_registries_1_are_exact_and_closed() {
    assert_wire_registry!(
        MessageKind {
            MessageKind::Hello => "hello",
            MessageKind::Request => "request",
            MessageKind::Progress => "progress",
            MessageKind::Result => "result",
            MessageKind::Error => "error",
            MessageKind::Cancel => "cancel",
            MessageKind::Cancelled => "cancelled",
        }
    );
    assert_wire_registry!(
        PeerRole {
            PeerRole::Daemon => "daemon",
            PeerRole::StandaloneCli => "standalone_cli",
            PeerRole::ClientAdapter => "client_adapter",
            PeerRole::Worker => "worker",
        }
    );
    assert_wire_registry!(
        ProgressPhase {
            ProgressPhase::Accepted => "accepted",
            ProgressPhase::Planning => "planning",
            ProgressPhase::Retrieving => "retrieving",
            ProgressPhase::Validating => "validating",
            ProgressPhase::Projecting => "projecting",
        }
    );
    assert_wire_registry!(
        ProtocolRetryability {
            ProtocolRetryability::Never => "never",
            ProtocolRetryability::SameRequest => "same_request",
            ProtocolRetryability::NewRequestAfterRefresh => "new_request_after_refresh",
        }
    );
    assert_wire_registry!(
        HandleClass {
            HandleClass::Ephemeral => "ephemeral",
            HandleClass::DurableSource => "durable_source",
        }
    );
}

#[test]
fn query_wire_registries_1_are_exact_and_closed() {
    assert_wire_registry!(
        LegKind {
            LegKind::Direct => "direct",
            LegKind::Exact => "exact",
            LegKind::Structural => "structural",
            LegKind::Lexical => "lexical",
            LegKind::Semantic => "semantic",
            LegKind::Rerank => "rerank",
        }
    );
    assert_wire_registry!(
        CoverageGapKind {
            CoverageGapKind::UnavailableMembership => "unavailable_membership",
            CoverageGapKind::FailedLeg => "failed_leg",
            CoverageGapKind::OmittedBudget => "omitted_budget",
            CoverageGapKind::ObservationGap => "observation_gap",
            CoverageGapKind::SourceUnreadable => "source_unreadable",
            CoverageGapKind::ValidationGap => "validation_gap",
            CoverageGapKind::AccessRevoked => "access_revoked",
            CoverageGapKind::Purge => "purge",
            CoverageGapKind::ProviderDegraded => "provider_degraded",
        }
    );
    assert_wire_registry!(
        Retryability {
            Retryability::Never => "never",
            Retryability::SameRequest => "same_request",
            Retryability::AfterRefresh => "after_refresh",
            Retryability::AfterReconcile => "after_reconcile",
        }
    );
    assert_wire_registry!(
        PriorityClass {
            PriorityClass::Interactive => "interactive",
            PriorityClass::Verification => "verification",
            PriorityClass::Background => "background",
        }
    );
    assert_wire_registry!(
        StateDependencyKind {
            StateDependencyKind::MaterializerProfile => "materializer_profile",
            StateDependencyKind::UnitizerProfile => "unitizer_profile",
            StateDependencyKind::EnricherProfile => "enricher_profile",
            StateDependencyKind::ProviderCapability => "provider_capability",
            StateDependencyKind::OverlapRouteProof => "overlap_route_proof",
            StateDependencyKind::RetentionLease => "retention_lease",
        }
    );
}

#[test]
fn query_wire_registries_2_are_exact_and_closed() {
    assert_wire_registry!(
        RequiredDenominator {
            RequiredDenominator::CandidateScope => "candidate_scope",
            RequiredDenominator::CompleteScope => "complete_scope",
            RequiredDenominator::UnknownAllowed => "unknown_allowed",
        }
    );
    assert_wire_registry!(
        PositionEncoding {
            PositionEncoding::Utf8Bytes => "utf8_bytes",
            PositionEncoding::Utf16CodeUnits => "utf16_code_units",
            PositionEncoding::Utf32Codepoints => "utf32_codepoints",
        }
    );
    assert_wire_registry!(
        ExactPredicateKind {
            ExactPredicateKind::Literal => "literal",
            ExactPredicateKind::Regex => "regex",
            ExactPredicateKind::QualifiedSymbol => "qualified_symbol",
            ExactPredicateKind::StructuralPattern => "structural_pattern",
            ExactPredicateKind::RecordField => "record_field",
        }
    );
    assert_wire_registry!(
        ExactInputDomain {
            ExactInputDomain::RawBytes => "raw_bytes",
            ExactInputDomain::DecodedText => "decoded_text",
            ExactInputDomain::StructuralIr => "structural_ir",
        }
    );
    assert_wire_registry!(
        ExactItemFailureKind {
            ExactItemFailureKind::Unreadable => "unreadable",
            ExactItemFailureKind::RevisionUnavailable => "revision_unavailable",
            ExactItemFailureKind::ScopeChanged => "scope_changed",
            ExactItemFailureKind::Timeout => "timeout",
            ExactItemFailureKind::Cancelled => "cancelled",
            ExactItemFailureKind::UnsupportedEncoding => "unsupported_encoding",
            ExactItemFailureKind::PredicateError => "predicate_error",
        }
    );
}

#[test]
fn query_wire_registries_3_are_exact_and_closed() {
    assert_wire_registry!(
        CoverageDenominatorKind {
            CoverageDenominatorKind::CandidateScope => "candidate_scope",
            CoverageDenominatorKind::CompleteScope => "complete_scope",
            CoverageDenominatorKind::Unknown => "unknown",
        }
    );
    assert_wire_registry!(
        ExactConclusion {
            ExactConclusion::MatchesFound => "matches_found",
            ExactConclusion::NoMatchInCompleteScope => "no_match_in_complete_scope",
            ExactConclusion::Incomplete => "incomplete",
        }
    );
    assert_wire_registry!(
        LegExecutionState {
            LegExecutionState::Completed => "completed",
            LegExecutionState::Partial => "partial",
            LegExecutionState::Cancelled => "cancelled",
            LegExecutionState::Failed => "failed",
            LegExecutionState::DiscardedContaminated => "discarded_contaminated",
        }
    );
}

#[test]
fn recipes_wire_registries_1_are_exact_and_closed() {
    assert_wire_registry!(
        RelationKind {
            RelationKind::Definition => "definition",
            RelationKind::Reference => "reference",
            RelationKind::Caller => "caller",
            RelationKind::Test => "test",
            RelationKind::Documentation => "documentation",
            RelationKind::Configuration => "configuration",
        }
    );
    assert_wire_registry!(
        CasePolicy {
            CasePolicy::Exact => "exact",
            CasePolicy::UnicodeCasefold => "unicode_casefold",
        }
    );
    assert_wire_registry!(
        CorpusFacetDimension {
            CorpusFacetDimension::Role => "role",
            CorpusFacetDimension::LanguageOrFormat => "language_or_format",
            CorpusFacetDimension::EntityKind => "entity_kind",
            CorpusFacetDimension::Lineage => "lineage",
            CorpusFacetDimension::Readiness => "readiness",
        }
    );
    assert_wire_registry!(
        CorpusDeltaDimension {
            CorpusDeltaDimension::Source => "source",
            CorpusDeltaDimension::Membership => "membership",
            CorpusDeltaDimension::Representation => "representation",
            CorpusDeltaDimension::Symbol => "symbol",
            CorpusDeltaDimension::Readiness => "readiness",
        }
    );
    assert_wire_registry!(
        HandleExpansionKind {
            HandleExpansionKind::Excerpt => "excerpt",
            HandleExpansionKind::SourceMetadata => "source_metadata",
            HandleExpansionKind::Provenance => "provenance",
            HandleExpansionKind::Continuation => "continuation",
        }
    );
}

#[test]
fn results_wire_registries_1_are_exact_and_closed() {
    assert_wire_registry!(
        CandidateValidationGapReason {
            CandidateValidationGapReason::Stale => "stale",
            CandidateValidationGapReason::Unreadable => "unreadable",
            CandidateValidationGapReason::AccessRevoked => "access_revoked",
            CandidateValidationGapReason::Purged => "purged",
            CandidateValidationGapReason::SourceRevisionUnavailable => "source_revision_unavailable",
        }
    );
    assert_wire_registry!(
        CandidateGapDisposition {
            CandidateGapDisposition::Dropped => "dropped",
            CandidateGapDisposition::ReplanRequested => "replan_requested",
            CandidateGapDisposition::GapReported => "gap_reported",
        }
    );
    assert_wire_registry!(
        MatchBasis {
            MatchBasis::ExplicitHandle => "explicit_handle",
            MatchBasis::EditorPosition => "editor_position",
            MatchBasis::QualifiedName => "qualified_name",
            MatchBasis::ExactName => "exact_name",
            MatchBasis::Signature => "signature",
            MatchBasis::Structural => "structural",
            MatchBasis::Lexical => "lexical",
            MatchBasis::Semantic => "semantic",
        }
    );
    assert_wire_registry!(
        CountAssurance {
            CountAssurance::ExactInventory => "exact_inventory",
            CountAssurance::FilteredIndex => "filtered_index",
            CountAssurance::Partial => "partial",
        }
    );
    assert_wire_registry!(
        CorpusChangeKind {
            CorpusChangeKind::SourceAdded => "source_added",
            CorpusChangeKind::SourceRemoved => "source_removed",
            CorpusChangeKind::RevisionChanged => "revision_changed",
            CorpusChangeKind::MembershipChanged => "membership_changed",
            CorpusChangeKind::RepresentationChanged => "representation_changed",
            CorpusChangeKind::SymbolChanged => "symbol_changed",
            CorpusChangeKind::ReadinessChanged => "readiness_changed",
        }
    );
}

#[test]
fn results_wire_registries_2_are_exact_and_closed() {
    assert_wire_registry!(
        ProvenanceStepKind {
            ProvenanceStepKind::SourceIdentity => "source_identity",
            ProvenanceStepKind::RevisionOccurrence => "revision_occurrence",
            ProvenanceStepKind::Materialization => "materialization",
            ProvenanceStepKind::Representation => "representation",
            ProvenanceStepKind::Unit => "unit",
            ProvenanceStepKind::Projection => "projection",
            ProvenanceStepKind::Export => "export",
            ProvenanceStepKind::OwnershipCutover => "ownership_cutover",
        }
    );
}

#[test]
fn source_wire_registries_1_are_exact_and_closed() {
    assert_wire_registry!(
        ActiveMode {
            ActiveMode::Standalone => "standalone",
            ActiveMode::ManagedClient => "managed_client",
        }
    );
    assert_wire_registry!(
        VcsKind {
            VcsKind::Git => "git",
            VcsKind::None => "none",
        }
    );
    assert_wire_registry!(
        NamespaceOwnershipStatus {
            NamespaceOwnershipStatus::Active => "ACTIVE",
            NamespaceOwnershipStatus::CutoverPrepared => "CUTOVER_PREPARED",
            NamespaceOwnershipStatus::Fenced => "FENCED",
            NamespaceOwnershipStatus::Retired => "RETIRED",
        }
    );
    assert_wire_registry!(
        SourceIdentityKind {
            SourceIdentityKind::NtfsFile => "ntfs_file",
            SourceIdentityKind::GitBlobLineage => "git_blob_lineage",
            SourceIdentityKind::ImportedObject => "imported_object",
            SourceIdentityKind::AdmittedVirtualSnapshot => "admitted_virtual_snapshot",
        }
    );
    assert_wire_registry!(
        AcquisitionKind {
            AcquisitionKind::Filesystem => "filesystem",
            AcquisitionKind::GitObject => "git_object",
            AcquisitionKind::Imported => "imported",
            AcquisitionKind::AdmittedIdeSnapshot => "admitted_ide_snapshot",
        }
    );
}

#[test]
fn source_wire_registries_2_are_exact_and_closed() {
    assert_wire_registry!(
        MembershipRole {
            MembershipRole::Source => "source",
            MembershipRole::Test => "test",
            MembershipRole::Documentation => "documentation",
            MembershipRole::Generated => "generated",
            MembershipRole::Vendor => "vendor",
            MembershipRole::Reference => "reference",
        }
    );
    assert_wire_registry!(
        PortfolioRoleFilter {
            PortfolioRoleFilter::Source => "source",
            PortfolioRoleFilter::Test => "test",
            PortfolioRoleFilter::Documentation => "documentation",
            PortfolioRoleFilter::Reference => "reference",
        }
    );
    assert_wire_registry!(
        UnitKind {
            UnitKind::File => "file",
            UnitKind::Section => "section",
            UnitKind::Symbol => "symbol",
            UnitKind::Reference => "reference",
            UnitKind::Test => "test",
            UnitKind::Doc => "doc",
            UnitKind::Table => "table",
            UnitKind::ImageRegion => "image_region",
        }
    );
}

#[test]
fn closed_canonical_objects_reject_unknown_and_missing_fields() {
    let known = CanonicalKey::new_non_empty("known").expect("known key");
    let extra = CanonicalKey::new_non_empty("extra").expect("extra key");

    let value = CanonicalValue::Object(
        BoundedMap::from_entries([
            (known, CanonicalValue::Bool(true)),
            (extra, CanonicalValue::Bool(false)),
        ])
        .expect("closed object fixture"),
    );
    let mut object = ClosedCanonicalObject::from_value(value, "fixture").expect("object");
    assert_eq!(
        object.take_required("known"),
        Ok(CanonicalValue::Bool(true))
    );
    let unknown = object.finish().expect_err("unknown field must fail closed");
    assert_eq!(unknown.kind(), ContractErrorKind::UnknownField);
    assert_eq!(unknown.code(), ContractErrorCode::UnknownLoadBearingField);

    let empty = CanonicalValue::Object(BoundedMap::empty());
    let mut object = ClosedCanonicalObject::from_value(empty, "fixture").expect("object");
    let missing = object
        .take_required("known")
        .expect_err("missing required field must fail");
    assert_eq!(missing.kind(), ContractErrorKind::MalformedPayload);
}

#[test]
fn query_snapshot_rejects_workspace_view_aliasing() {
    let mut snapshot = query_snapshot();
    snapshot.workspace_view_revision_ref = Some(WorkspaceViewRevisionId::from_bytes([0x33; 16]));
    let error = snapshot
        .validate()
        .expect_err("top-level and source-view revisions must agree");
    assert_eq!(error.kind(), ContractErrorKind::ContradictoryState);
}

#[test]
fn contaminated_candidate_gaps_require_replanning() {
    let leg = opaque_id("leg-1");
    let mut gap = CandidateValidationGap {
        nominated_candidate_ref: opaque_id("candidate-1"),
        source_revision_ref: None,
        reason: CandidateValidationGapReason::Stale,
        affected_leg_refs: BoundedList::new(vec![leg]).expect("affected leg"),
        contaminated_rank_leg: true,
        disposition: CandidateGapDisposition::Dropped,
    };
    assert_eq!(
        gap.validate()
            .expect_err("contaminated order cannot survive")
            .kind(),
        ContractErrorKind::ContradictoryState
    );
    gap.disposition = CandidateGapDisposition::ReplanRequested;
    gap.validate()
        .expect("replanning preserves the security boundary");

    gap.affected_leg_refs = BoundedList::empty();
    assert_eq!(
        gap.validate()
            .expect_err("gap must identify an affected leg")
            .kind(),
        ContractErrorKind::ContradictoryState
    );
}
