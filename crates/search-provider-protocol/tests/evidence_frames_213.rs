//! #213 actual JSON round trips and adversarial wire fixtures.
use search_contracts::evidence::*;
use search_contracts::evidence_frames::*;
use search_contracts::*;
use search_provider_protocol as proto;

fn profile() -> ProfileId {
    ProfileId::new("lexical-v1").expect("profile")
}
const fn scope() -> PublicScopeSelector {
    PublicScopeSelector::WorkspaceRevision(WorkspaceViewRevisionId::from_bytes([0x21; 16]))
}
fn request(scope: PublicScopeSelector) -> RetrieveEvidenceRequestV2 {
    RetrieveEvidenceRequestV2 {
        scope,
        source_view: SourceView::RetainedRevision(SourceRevisionId::from_bytes([3; 16])),
        question: BoundedText::new("index owner").expect("question"),
        intent: EvidenceIntent::LocateImplementation,
        requested_roles: BoundedSet::empty(),
        optional_hints: EvidenceHints::default(),
        profile_id: profile(),
        progressive: true,
        budgets: QueryExecutionBudget {
            priority_class: PriorityClass::Interactive,
            deadline_ms: 1000,
            max_scoring_legs: 4,
            max_prefetch_candidates_per_leg: 64,
            max_validated_candidates: 32,
            max_source_read_bytes: 4096,
            max_exact_scan_items: 64,
            max_exact_scan_bytes: 4096,
            max_materialized_result_bytes: 4096,
            max_cpu_ms: 1000,
            max_memory_bytes: 4096,
        },
    }
}
fn orientation(scope: PublicScopeSelector) -> OrientScopeRequestV2 {
    let r = request(scope);
    OrientScopeRequestV2 {
        scope: r.scope,
        source_view: r.source_view,
        question: r.question,
        dimensions: BoundedSet::from_items([
            OrientationDimension::EntryPoints,
            OrientationDimension::RecommendedReading,
        ])
        .expect("dimensions"),
        optional_hints: OrientationHints::default(),
        profile_id: r.profile_id,
        budgets: r.budgets,
        progressive: true,
    }
}
const fn coverage(kind: CoverageDenominatorKind) -> Coverage {
    Coverage {
        requested_legs: BoundedList::empty(),
        executed_legs: BoundedList::empty(),
        represented_memberships: BoundedSet::empty(),
        represented_source_lineages: BoundedSet::empty(),
        omitted_or_failed_legs: BoundedList::empty(),
        candidate_validation_gaps: BoundedList::empty(),
        observation_freshness: ObservationFreshness {
            state: ObservationFreshnessState::Unknown,
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observed_age_ms: None,
        },
        unknowns: BoundedList::empty(),
        denominator_kind: kind,
    }
}
fn header(kind: EvidenceFrameKind, sequence: u64) -> EvidenceFrameHeader {
    EvidenceFrameHeader {
        request_id: RequestId::from_bytes([1; 16]),
        kind,
        event_sequence: sequence,
        profile_id: profile(),
    }
}
fn handle() -> SearchSourceHandle {
    SearchSourceHandle {
        handle_id: HandleId::from_bytes([8; 16]),
        handle_revision: NonZeroRevision::new(1).expect("revision"),
        handle_class: HandleClass::DurableSource,
        expires_at: None,
        opaque_token: OpaqueHandleToken::new(&[9; 32]).expect("token"),
    }
}
fn card() -> EvidenceCardV1 {
    let digest = Blake3Digest32::from_stored_bytes([4; 32]);
    EvidenceCardV1 {
        card_id: CandidateId::from_bytes([5; 16]),
        evidence_role: EvidenceRole::Definition,
        source: PublicCardSource {
            source_id: SourceId::from_bytes([2; 16]),
            revision: SourceRevisionRef {
                source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
                source_id: SourceId::from_bytes([2; 16]),
                revision_id: SourceRevisionId::from_bytes([3; 16]),
                content_digest: digest,
                byte_length: 32,
            },
            representation_profile: profile(),
        },
        unit: UnitOccurrence {
            unit_id: UnitId::from_bytes([6; 16]),
            representation_id: RepresentationId::from_bytes([7; 16]),
            unit_kind: UnitKind::Symbol,
            ordinal: 0,
            native_anchor: NativeAnchor::TextBytes(TextBytesAnchor {
                content_digest: digest,
                byte_start_0: 0,
                byte_end_exclusive_0: 12,
            }),
            structural_identity: None,
            configuration_predicate: None,
        },
        handle: handle(),
        bounded_preview: Some(BoundedText::new("index owner").expect("preview")),
        producer_assurance: AssuranceClass::ExactBytes,
        freshness: ObservationFreshnessState::Unknown,
        currentness: PublicCurrentnessState::Unknown,
        lineage: None,
        relations: BoundedList::empty(),
        ambiguity_group: None,
        score_profile: profile(),
        trace_ref: BoundedNonContentRankingTrace {
            fusion_profile_id: FusionProfileId::new("rank-v1").expect("fusion"),
            fused_rank: 1,
            exact_or_entity_boost: ExactOrEntityBoost::None,
            evidence_role_priority: 0,
            portfolio_priority: 0,
            lineage_diversity_action: LineageDiversityAction::Retained,
            deterministic_tie_break_digest: digest,
        },
    }
}
fn frames() -> Vec<EvidenceFrameV2> {
    let c = card();
    vec![
        EvidenceFrameV2::Accepted(EvidenceAcceptedFrame {
            header: header(EvidenceFrameKind::Accepted, 1),
            progressive: true,
            declared_frame_budget: 6,
        }),
        EvidenceFrameV2::CoverageSnapshot(EvidenceCoverageSnapshotFrame {
            header: header(EvidenceFrameKind::CoverageSnapshot, 2),
            coverage: coverage(CoverageDenominatorKind::CandidateScope),
            complete_negative_possible: false,
        }),
        EvidenceFrameV2::CardBatch(EvidenceCardBatchFrame {
            header: header(EvidenceFrameKind::CardBatch, 3),
            cards: BoundedList::new(vec![c.clone()]).expect("cards"),
            groups: BoundedList::empty(),
            last_batch: true,
            batch_offset: 0,
        }),
        EvidenceFrameV2::OrientationBatch(OrientationBatchFrame {
            header: header(EvidenceFrameKind::OrientationBatch, 4),
            cards: BoundedList::new(vec![OrientationCard {
                card_id: c.card_id,
                dimension: OrientationDimension::EntryPoints,
                source: c.source,
                unit: c.unit,
                handle: c.handle.clone(),
                producer_assurance: c.producer_assurance,
                freshness: c.freshness,
                currentness: c.currentness,
                rank_key: c.trace_ref,
            }])
            .expect("cards"),
            groups: BoundedList::empty(),
            recommended_reading: BoundedList::new(vec![c.handle]).expect("reading"),
            last_batch: true,
            batch_offset: 0,
        }),
        EvidenceFrameV2::SuggestedCheckBatch(SuggestedCheckBatchFrame {
            header: header(EvidenceFrameKind::SuggestedCheckBatch, 5),
            checks: BoundedList::new(vec![ExactCheckTemplate {
                template_id: Blake3Digest32::from_stored_bytes([1; 32]),
                rationale: BoundedText::new("verify definition").expect("rationale"),
                predicate: ExactPredicate {
                    kind: ExactPredicateKind::Literal,
                    engine_and_version: profile(),
                    serialized_form: BoundedCanonicalBytes::from_validated(b"index".to_vec())
                        .expect("predicate"),
                    input_domain: ExactInputDomain::DecodedText,
                    worst_case_complexity_class: profile(),
                },
                native_anchor: None,
            }])
            .expect("checks"),
            last_batch: true,
            batch_offset: 0,
        }),
        EvidenceFrameV2::Terminal(EvidenceTerminalFrame {
            header: header(EvidenceFrameKind::Terminal, 6),
            terminal: EvidenceTerminalKind::Success,
            coverage: coverage(CoverageDenominatorKind::CandidateScope),
            reason_codes: BoundedSet::empty(),
            budget_exhausted: false,
        }),
    ]
}
fn assert_json(bytes: &[u8]) {
    proto::FrameCodec::validate_payload(bytes, proto::DEFAULT_PROTOCOL_LIMITS).expect("valid JSON");
    let payload = JsonFramePayload::new(bytes.to_vec()).expect("payload");
    let framed =
        proto::encode_frame(&payload, proto::DEFAULT_PROTOCOL_LIMITS).expect("shared frame");
    assert_eq!(
        &framed.as_slice()[..4],
        &u32::try_from(bytes.len()).expect("length").to_le_bytes()
    );
    assert_eq!(
        proto::decode_frame(framed.as_slice(), proto::DEFAULT_PROTOCOL_LIMITS).expect("unframe"),
        payload
    );
}
fn mutate(text: &str, before: &str, after: &str) -> String {
    assert!(
        text.contains(before),
        "mutation must alter an existing field"
    );
    let changed = text.replacen(before, after, 1);
    assert_ne!(changed, text);
    changed
}

#[test]
fn all_four_selectors_round_trip_through_both_request_dispatches() {
    for selector in [
        scope(),
        PublicScopeSelector::CorpusRevision {
            corpus_id: CorpusId::from_bytes([2; 16]),
            membership_revision: MembershipRevision::new(3),
        },
        PublicScopeSelector::PortfolioRevision {
            portfolio_id: ReferencePortfolioId::from_bytes([3; 16]),
            portfolio_revision: PortfolioRevision::new(4),
        },
        PublicScopeSelector::OverlayRevision(OverlayRevision::new(5)),
    ] {
        let r = request(selector.clone());
        let wire = proto::encode_retrieve_evidence_request(&r).expect("encode");
        assert_json(&wire);
        assert_eq!(
            proto::decode_retrieve_evidence_request(&wire).expect("decode"),
            r
        );
        assert!(matches!(
            proto::decode_evidence_request(&wire).expect("dispatch"),
            proto::EvidenceRecipeRequestV2::RetrieveEvidence(_)
        ));
        assert!(proto::decode_orient_scope_request(&wire).is_err());
        let o = orientation(selector);
        let wire = proto::encode_orient_scope_request(&o).expect("encode");
        assert_json(&wire);
        assert_eq!(
            proto::decode_orient_scope_request(&wire).expect("decode"),
            o
        );
        assert!(matches!(
            proto::decode_evidence_request(&wire).expect("dispatch"),
            proto::EvidenceRecipeRequestV2::OrientScope(_)
        ));
        assert!(proto::decode_retrieve_evidence_request(&wire).is_err());
    }
}
#[test]
fn all_six_frames_and_three_terminals_use_valid_shared_json() {
    for f in frames() {
        let wire = proto::encode_evidence_frame(&f).expect("encode");
        assert_json(&wire);
        assert_eq!(proto::peek_evidence_frame(&wire).expect("peek"), f.kind());
        assert_eq!(proto::decode_evidence_frame(&wire).expect("decode"), f);
    }
    let EvidenceFrameV2::Terminal(mut terminal) = frames().pop().expect("terminal") else {
        panic!("terminal")
    };
    for kind in EvidenceTerminalKind::ALL {
        terminal.terminal = kind;
        let frame = EvidenceFrameV2::Terminal(terminal.clone());
        let wire = proto::encode_evidence_frame(&frame).expect("encode");
        assert_json(&wire);
        assert_eq!(proto::decode_evidence_frame(&wire).expect("decode"), frame);
    }
}
#[test]
fn strict_unknown_missing_duplicate_version_and_family_refusals() {
    let text = String::from_utf8(proto::encode_evidence_frame(&frames()[0]).expect("encode"))
        .expect("text");
    for (before, after) in [
        (r#""v":2"#, r#""v":3"#),
        (r#""v":2,"#, ""),
        (r#""v":2"#, r#""v":2,"v":2"#),
        (r#""frame":"accepted""#, r#""frame":"unknown""#),
        (r#""frame":"accepted""#, r#""frame":"terminal""#),
        (r#""progressive":true"#, r#""progressive":true,"extra":0"#),
        (r#""progressive":true,"#, ""),
        (
            r#""progressive":true"#,
            r#""progressive":true,"progressive":true"#,
        ),
    ] {
        assert!(
            proto::decode_evidence_frame(mutate(&text, before, after).as_bytes()).is_err(),
            "{before} -> {after}"
        );
    }
    for wire in [
        proto::encode_retrieve_evidence_request(&request(scope())).expect("retrieve"),
        proto::encode_orient_scope_request(&orientation(scope())).expect("orient"),
    ] {
        let text = String::from_utf8(wire).expect("text");
        for (before, after) in [
            (r#""v":2"#, r#""v":99"#),
            (r#""v":2,"#, ""),
            (r#""v":2"#, r#""v":2,"v":2"#),
            (
                r#""question":"index owner""#,
                r#""question":"index owner","raw_filter":{}"#,
            ),
            (r#""question":"index owner","#, ""),
            (
                r#""question":"index owner""#,
                r#""question":"index owner","question":"index owner""#,
            ),
            (r#""question":"index owner""#, r#""question":"""#),
        ] {
            assert!(
                proto::decode_evidence_request(mutate(&text, before, after).as_bytes()).is_err(),
                "{before} -> {after}"
            );
        }
        let declared = if text.contains(r#""recipe":"retrieve_evidence@1""#) {
            (
                r#""recipe":"retrieve_evidence@1""#,
                r#""recipe":"orient_scope@1""#,
            )
        } else {
            (
                r#""recipe":"orient_scope@1""#,
                r#""recipe":"retrieve_evidence@1""#,
            )
        };
        assert!(
            proto::decode_evidence_request(mutate(&text, declared.0, declared.1).as_bytes())
                .is_err()
        );
        assert!(
            proto::decode_evidence_request(
                mutate(&text, declared.0, r#""recipe":"retrieve_evidence@2""#).as_bytes()
            )
            .is_err()
        );
    }
}
#[test]
fn zero_result_coverage_is_preserved_and_never_promoted_to_negative() {
    for kind in [
        CoverageDenominatorKind::Unknown,
        CoverageDenominatorKind::CandidateScope,
        CoverageDenominatorKind::CompleteScope,
    ] {
        let frame = EvidenceFrameV2::CoverageSnapshot(EvidenceCoverageSnapshotFrame {
            header: header(EvidenceFrameKind::CoverageSnapshot, 2),
            coverage: coverage(kind),
            complete_negative_possible: false,
        });
        let wire = proto::encode_evidence_frame(&frame).expect("encode");
        assert_json(&wire);
        assert_eq!(proto::decode_evidence_frame(&wire).expect("decode"), frame);
        let forged = String::from_utf8(wire).expect("text").replace(
            r#""complete_negative_possible":false"#,
            r#""complete_negative_possible":true"#,
        );
        assert!(proto::decode_evidence_frame(forged.as_bytes()).is_err());
    }
}
#[test]
fn exchange_rejects_cross_request_regression_budget_and_missing_terminal() {
    validate_evidence_exchange(&BoundedList::new(frames()).expect("exchange"))
        .expect("valid exchange");
    let mut missing = frames();
    missing.pop();
    assert!(validate_evidence_exchange(&BoundedList::new(missing).expect("exchange")).is_err());
    let mut swapped = frames();
    swapped.swap(1, 2);
    assert!(validate_evidence_exchange(&BoundedList::new(swapped).expect("exchange")).is_err());
    let mut foreign = frames();
    if let EvidenceFrameV2::Terminal(t) = &mut foreign[5] {
        t.header.request_id = RequestId::from_bytes([2; 16]);
    }
    assert!(validate_evidence_exchange(&BoundedList::new(foreign).expect("exchange")).is_err());
    let mut exhausted = frames();
    if let EvidenceFrameV2::Accepted(a) = &mut exhausted[0] {
        a.declared_frame_budget = 5;
    }
    assert!(validate_evidence_exchange(&BoundedList::new(exhausted).expect("exchange")).is_err());
    let mut duplicate = frames();
    duplicate.push(duplicate[5].clone());
    assert!(validate_evidence_exchange(&BoundedList::new(duplicate).expect("exchange")).is_err());
}
#[test]
fn card_identity_and_range_inconsistency_are_refused() {
    let mut c = card();
    c.source.source_id = SourceId::from_bytes([99; 16]);
    assert!(c.validate().is_err());
    let mut c = card();
    if let NativeAnchor::TextBytes(a) = &mut c.unit.native_anchor {
        a.byte_end_exclusive_0 = 33;
    }
    assert!(c.validate().is_err());
}

fn result_fence() -> ResultFence {
    ResultFence {
        planned_snapshot: QuerySnapshotFence {
            installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
            collection_generation_id: None,
            visible_epoch: None,
            collection_route_revision: CollectionRouteRevision::new(1),
            catalog_revision: CatalogRevision::new(1),
            membership_revision: MembershipRevision::new(1),
            reference_portfolio_revision: None,
            access_policy_revision: AccessPolicyRevision::new(1),
            shadow_fence_revision: ShadowFenceRevision::new(1),
            purge_fence_revision: PurgeFenceRevision::new(1),
            overlay_revision: OverlayRevision::new(0),
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observation_freshness: coverage(CoverageDenominatorKind::Unknown).observation_freshness,
            source_view: request(scope()).source_view,
            workspace_view_revision_ref: None,
            lexical_profile_ids: BoundedList::empty(),
            snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([1; 32]),
        },
        emission_source_owner_fences: BoundedList::empty(),
        emission_security_fence: EmissionSecurityFence {
            access_policy_revision: AccessPolicyRevision::new(1),
            live_deny_generation: 1,
            shadow_fence_revision: ShadowFenceRevision::new(1),
            purge_fence_revision: PurgeFenceRevision::new(1),
            checked_at: UtcTimestamp::parse("2026-10-14T00:00:00.000000Z").expect("timestamp"),
            receipt_ref: ReceiptRef::new("fixture-only").expect("receipt"),
        },
        result_fingerprint: Blake3Digest32::from_stored_bytes([1; 32]),
    }
}
#[test]
fn both_result_families_round_trip_nonempty_and_zero_with_strict_refusals() {
    for empty in [false, true] {
        let c = card();
        let results = [
            proto::EvidenceRecipeResultV2::RetrieveEvidence(RetrieveEvidenceResultV2 {
                request_id: RequestId::from_bytes([1; 16]),
                plan_id: PlanId::from_bytes([1; 16]),
                plan_fingerprint: PlanFingerprint::from_bytes([1; 32]),
                result_fence: result_fence(),
                resolved_scope: scope(),
                cards: BoundedList::new(if empty { vec![] } else { vec![c.clone()] })
                    .expect("cards"),
                groups: BoundedList::empty(),
                suggested_exact_checks: BoundedList::empty(),
                coverage: coverage(if empty {
                    CoverageDenominatorKind::Unknown
                } else {
                    CoverageDenominatorKind::CandidateScope
                }),
                continuation: None,
            }),
            proto::EvidenceRecipeResultV2::OrientScope(OrientScopeResultV2 {
                request_id: RequestId::from_bytes([1; 16]),
                plan_id: PlanId::from_bytes([1; 16]),
                plan_fingerprint: PlanFingerprint::from_bytes([1; 32]),
                result_fence: result_fence(),
                resolved_scope: scope(),
                entry_points: BoundedList::new(if empty {
                    vec![]
                } else {
                    vec![OrientationCard {
                        card_id: c.card_id,
                        dimension: OrientationDimension::EntryPoints,
                        source: c.source,
                        unit: c.unit,
                        handle: c.handle,
                        producer_assurance: c.producer_assurance,
                        freshness: c.freshness,
                        currentness: c.currentness,
                        rank_key: c.trace_ref,
                    }]
                })
                .expect("cards"),
                groups: BoundedList::empty(),
                recommended_reading: BoundedList::empty(),
                suggested_exact_checks: BoundedList::empty(),
                ambiguities: BoundedList::empty(),
                coverage: coverage(if empty {
                    CoverageDenominatorKind::Unknown
                } else {
                    CoverageDenominatorKind::CandidateScope
                }),
                continuation: None,
            }),
        ];
        for result in results {
            let wire = proto::encode_evidence_result(&result).expect("encode");
            assert_json(&wire);
            assert_eq!(
                proto::decode_evidence_result(&wire).expect("decode"),
                result
            );
            let text = String::from_utf8(wire).expect("text");
            for (before, after) in [
                (r#""v":2"#, r#""v":3"#),
                (r#""v":2,"#, ""),
                (r#""v":2"#, r#""v":2,"v":2"#),
                (
                    r#""continuation":null"#,
                    r#""continuation":null,"point_id":1"#,
                ),
                (r#","continuation":null"#, ""),
                (
                    r#""continuation":null"#,
                    r#""continuation":null,"continuation":null"#,
                ),
            ] {
                assert!(
                    proto::decode_evidence_result(mutate(&text, before, after).as_bytes()).is_err()
                );
            }
            let (before, after) = match result.recipe_id() {
                RecipeIdV2::RetrieveEvidence => (
                    r#""recipe":"retrieve_evidence@1""#,
                    r#""recipe":"orient_scope@1""#,
                ),
                RecipeIdV2::OrientScope => (
                    r#""recipe":"orient_scope@1""#,
                    r#""recipe":"retrieve_evidence@1""#,
                ),
            };
            assert!(
                proto::decode_evidence_result(mutate(&text, before, after).as_bytes()).is_err()
            );
        }
    }
}

#[test]
fn zero_result_success_requires_exact_completed_leg_accounting_on_the_wire() {
    let mut terminal = EvidenceTerminalFrame {
        header: header(EvidenceFrameKind::Terminal, 3),
        terminal: EvidenceTerminalKind::Success,
        coverage: coverage(CoverageDenominatorKind::CandidateScope),
        reason_codes: BoundedSet::empty(),
        budget_exhausted: false,
    };
    terminal.coverage.requested_legs = BoundedList::new(vec![LegDescriptor {
        leg_ref: OpaqueId::new("L").expect("leg"),
        leg_kind: LegKind::Lexical,
        scoring_partition_ref: None,
        profile_id: profile(),
    }])
    .expect("requested");
    assert!(proto::encode_evidence_frame(&EvidenceFrameV2::Terminal(terminal.clone())).is_err());
    terminal.terminal = EvidenceTerminalKind::Partial;
    let partial = EvidenceFrameV2::Terminal(terminal.clone());
    let wire = proto::encode_evidence_frame(&partial).expect("honest partial");
    assert_json(&wire);
    assert_eq!(
        proto::decode_evidence_frame(&wire).expect("decode partial"),
        partial
    );

    terminal.terminal = EvidenceTerminalKind::Success;
    terminal.coverage.executed_legs = BoundedList::new(vec![LegExecutionSummary {
        leg_ref: OpaqueId::new("L").expect("leg"),
        state: LegExecutionState::Completed,
        nominated_count: 0,
        validated_count: 0,
        reason_codes: BoundedSet::empty(),
        receipt_ref: ReceiptRef::new("fixture-only").expect("receipt"),
    }])
    .expect("summary");
    let success = EvidenceFrameV2::Terminal(terminal);
    let wire = proto::encode_evidence_frame(&success).expect("zero matches after completed leg");
    assert_json(&wire);
    assert_eq!(
        proto::decode_evidence_frame(&wire).expect("decode success"),
        success
    );
    let text = String::from_utf8(wire).expect("text");
    let summary = r#"{"leg_ref":"L","state":"completed","nominated_count":0,"validated_count":0,"reason_codes":[],"receipt_ref":"fixture-only"}"#;
    let executed = format!(r#""executed_legs":[{summary}]"#);
    for replacement in [
        r#""executed_legs":[]"#.to_owned(),
        format!(r#""executed_legs":[{summary},{summary}]"#),
        format!(
            r#""executed_legs":[{}]"#,
            summary.replace(r#""leg_ref":"L""#, r#""leg_ref":"foreign""#)
        ),
        format!(
            r#""executed_legs":[{}]"#,
            summary.replace(r#""state":"completed""#, r#""state":"partial""#)
        ),
    ] {
        let forged = mutate(&text, &executed, &replacement);
        assert_json(forged.as_bytes());
        assert!(proto::decode_evidence_frame(forged.as_bytes()).is_err());
    }
}
