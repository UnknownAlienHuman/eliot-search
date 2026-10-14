//! #213 public wire security and v1 compatibility fixtures.
//!
//! Shape checks never authorize scopes. Actual JSON/dispatch and fixed v1 byte
//! compatibility fixtures live in the protocol owner.

use search_contracts::evidence::{
    EvidenceHints, EvidenceIntent, PublicScopeSelector, RecipeIdV2, RetrieveEvidenceRequestV2,
};
use search_contracts::evidence_frames::{
    EvidenceAcceptedFrame, EvidenceFrameHeader, EvidenceFrameKind, EvidenceTerminalFrame,
    EvidenceTerminalKind,
};
use search_contracts::*;

const fn budget() -> QueryExecutionBudget {
    QueryExecutionBudget {
        priority_class: PriorityClass::Interactive,
        deadline_ms: 1_000,
        max_scoring_legs: 4,
        max_prefetch_candidates_per_leg: 64,
        max_validated_candidates: 32,
        max_source_read_bytes: 1 << 20,
        max_exact_scan_items: 1_000,
        max_exact_scan_bytes: 1 << 20,
        max_materialized_result_bytes: 1 << 20,
        max_cpu_ms: 1_000,
        max_memory_bytes: 1 << 20,
    }
}

const fn source_view() -> SourceView {
    SourceView::RetainedRevision(SourceRevisionId::from_bytes([0x51; 16]))
}

fn request(scope: PublicScopeSelector) -> RetrieveEvidenceRequestV2 {
    RetrieveEvidenceRequestV2 {
        scope,
        source_view: source_view(),
        question: BoundedText::new("where is the index owner").expect("question"),
        intent: EvidenceIntent::LocateImplementation,
        requested_roles: BoundedSet::empty(),
        optional_hints: EvidenceHints::default(),
        profile_id: ProfileId::new("lexical_baseline_v1").expect("profile"),
        budgets: budget(),
        progressive: true,
    }
}

// (A) Closed additive registry.
#[test]
fn v2_recipe_registry_is_closed_and_versioned() {
    assert_eq!(RecipeIdV2::ALL.len(), 2);
    assert_eq!(RecipeIdV2::RetrieveEvidence.as_str(), "retrieve_evidence@1");
    assert_eq!(RecipeIdV2::OrientScope.as_str(), "orient_scope@1");
    for recipe in RecipeIdV2::ALL {
        assert_eq!(RecipeIdV2::parse(recipe.as_str()), Ok(recipe));
    }
    for invalid in [
        "retrieve_evidence",
        "retrieve_evidence@2",
        "RETRIEVE_EVIDENCE@1",
        "",
    ] {
        assert!(
            RecipeIdV2::parse(invalid).is_err(),
            "{invalid} must be refused"
        );
    }
    // The v1 registry is a separate closed set; v2 names never enter it.
    for recipe in RecipeIdV2::ALL {
        assert!(RecipeIdV1::parse(recipe.as_str()).is_err());
    }
}

// (B) Closed progressive frame vocabulary.
#[test]
fn progressive_frame_vocabulary_is_closed_and_ordered() {
    assert_eq!(EvidenceFrameKind::ALL.len(), 6);
    let expected = [
        "accepted",
        "coverage_snapshot",
        "card_batch",
        "orientation_batch",
        "suggested_check_batch",
        "terminal",
    ];
    for (kind, wire) in EvidenceFrameKind::ALL.iter().zip(expected) {
        assert_eq!(kind.as_str(), wire);
        assert_eq!(EvidenceFrameKind::parse(wire), Ok(*kind));
    }
    for invalid in ["card", "batch", "terminal_success@1", ""] {
        assert!(
            EvidenceFrameKind::parse(invalid).is_err(),
            "{invalid} must be refused"
        );
    }
    assert_eq!(EvidenceTerminalKind::ALL.len(), 3);
    assert_eq!(EvidenceTerminalKind::Success.as_str(), "terminal_success");
    assert_eq!(EvidenceTerminalKind::Partial.as_str(), "terminal_partial");
    assert_eq!(EvidenceTerminalKind::Failure.as_str(), "terminal_failure");
    for invalid in ["success", "terminal_partial@2", ""] {
        assert!(
            EvidenceTerminalKind::parse(invalid).is_err(),
            "{invalid} must be refused"
        );
    }
}

// (C) Frame-kind invariant: the declared kind is the only accepted kind.
#[test]
fn accepted_frame_rejects_wrong_declared_kind() {
    let good = EvidenceAcceptedFrame {
        header: EvidenceFrameHeader {
            request_id: RequestId::from_bytes([0x11; 16]),
            kind: EvidenceFrameKind::Accepted,
            event_sequence: 1,
            profile_id: ProfileId::new("lexical_baseline_v1").expect("profile"),
        },
        progressive: true,
        declared_frame_budget: 8,
    };
    assert!(good.validate().is_ok());

    let mut zero_budget = good.clone();
    zero_budget.declared_frame_budget = 0;
    assert!(
        zero_budget.validate().is_err(),
        "a zero frame budget must be refused"
    );

    let mut wrong_kind = good;
    wrong_kind.header.kind = EvidenceFrameKind::Terminal;
    assert!(
        wrong_kind.validate().is_err(),
        "a mismatched declared kind must be refused"
    );
}

// (D) A success terminal may never hide a gap.
#[test]
fn terminal_success_rejects_incomplete_coverage() {
    let complete = EvidenceTerminalFrame {
        header: EvidenceFrameHeader {
            request_id: RequestId::from_bytes([0x11; 16]),
            kind: EvidenceFrameKind::Terminal,
            event_sequence: 9,
            profile_id: ProfileId::new("lexical_baseline_v1").expect("profile"),
        },
        terminal: EvidenceTerminalKind::Success,
        coverage: Coverage {
            requested_legs: BoundedList::empty(),
            executed_legs: BoundedList::empty(),
            represented_memberships: BoundedSet::empty(),
            represented_source_lineages: BoundedSet::empty(),
            omitted_or_failed_legs: BoundedList::empty(),
            candidate_validation_gaps: BoundedList::empty(),
            observation_freshness: ObservationFreshness {
                state: ObservationFreshnessState::CurrentConfirmed,
                observation_cursor_revision: ObservationCursorRevision::new(1),
                observed_age_ms: None,
            },
            unknowns: BoundedList::empty(),
            denominator_kind: CoverageDenominatorKind::CandidateScope,
        },
        reason_codes: BoundedSet::empty(),
        budget_exhausted: false,
    };
    assert!(complete.validate().is_ok());

    let mut exhausted = complete.clone();
    exhausted.budget_exhausted = true;
    assert!(
        exhausted.validate().is_err(),
        "success with an exhausted budget must be refused"
    );

    let mut gap = complete;
    gap.coverage.unknowns = BoundedList::new(vec![CoverageUnknown {
        unknown_ref: OpaqueId::new("gap").expect("gap"),
        description_template_id: OpaqueId::new("scope-unresolved").expect("template"),
        bounded_metadata: BoundedNonContentMetadata {
            entries: BoundedMap::empty(),
        },
    }])
    .expect("unknowns");
    assert!(
        gap.validate().is_err(),
        "success must not hide an unknown scope"
    );
}
// (E) Every scope selector is accepted on its own terms, and a caller may never
// present a bare membership list or a live path as an authorized scope.
#[test]
fn every_scope_selector_remains_an_ordinary_reference() {
    let selectors = [
        PublicScopeSelector::WorkspaceRevision(WorkspaceViewRevisionId::from_bytes([0x21; 16])),
        PublicScopeSelector::CorpusRevision {
            corpus_id: CorpusId::from_bytes([0x22; 16]),
            membership_revision: MembershipRevision::new(3),
        },
        PublicScopeSelector::PortfolioRevision {
            portfolio_id: ReferencePortfolioId::from_bytes([0x23; 16]),
            portfolio_revision: PortfolioRevision::new(4),
        },
        PublicScopeSelector::OverlayRevision(OverlayRevision::new(5)),
    ];
    for selector in &selectors {
        assert!(selector.validate().is_ok());
        assert!(request(selector.clone()).validate().is_ok());
    }
    // A selector value is never equal to a different selector kind.
    assert_ne!(selectors[0], selectors[1]);
    assert_ne!(selectors[2], selectors[3]);
}

// (F) An empty card set is incomplete coverage, never a complete negative.
#[test]
fn denominator_classification_alone_cannot_authorize_a_negative() {
    use search_contracts::evidence_frames::EvidenceCoverageSnapshotFrame;
    let mut snapshot = EvidenceCoverageSnapshotFrame {
        header: EvidenceFrameHeader {
            request_id: RequestId::from_bytes([1; 16]),
            kind: EvidenceFrameKind::CoverageSnapshot,
            event_sequence: 2,
            profile_id: ProfileId::new("lexical-v1").expect("profile"),
        },
        coverage: Coverage {
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
            denominator_kind: CoverageDenominatorKind::Unknown,
        },
        complete_negative_possible: false,
    };
    for kind in [
        CoverageDenominatorKind::Unknown,
        CoverageDenominatorKind::CandidateScope,
        CoverageDenominatorKind::CompleteScope,
    ] {
        snapshot.coverage.denominator_kind = kind;
        snapshot.complete_negative_possible = false;
        snapshot.validate().expect("classification preserved");
        snapshot.complete_negative_possible = true;
        assert!(snapshot.validate().is_err());
    }
}

// (G) v1 recipes are unchanged in count, spelling and parsing.
#[test]
fn v1_recipe_registry_spelling_is_unchanged() {
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
    for (recipe, wire) in RecipeIdV1::ALL.iter().zip(expected) {
        assert_eq!(recipe.as_str(), wire);
        assert_eq!(RecipeIdV1::parse_versioned(wire), Ok(*recipe));
    }
    for invalid in [
        "locate",
        "locate@2",
        "LOCATE@1",
        "retrieve_evidence@1",
        "orient_scope@1",
    ] {
        assert!(
            RecipeIdV1::parse_versioned(invalid).is_err(),
            "{invalid} stays refused"
        );
    }
}

fn terminal_requesting_one_leg() -> EvidenceTerminalFrame {
    EvidenceTerminalFrame {
        header: EvidenceFrameHeader {
            request_id: RequestId::from_bytes([1; 16]),
            kind: EvidenceFrameKind::Terminal,
            event_sequence: 3,
            profile_id: ProfileId::new("lexical-v1").expect("profile"),
        },
        terminal: EvidenceTerminalKind::Success,
        coverage: Coverage {
            requested_legs: BoundedList::new(vec![LegDescriptor {
                leg_ref: OpaqueId::new("L").expect("leg"),
                leg_kind: LegKind::Lexical,
                scoring_partition_ref: None,
                profile_id: ProfileId::new("lexical-v1").expect("profile"),
            }])
            .expect("requested"),
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
            denominator_kind: CoverageDenominatorKind::CandidateScope,
        },
        reason_codes: BoundedSet::empty(),
        budget_exhausted: false,
    }
}
fn completed_leg(name: &str) -> LegExecutionSummary {
    LegExecutionSummary {
        leg_ref: OpaqueId::new(name).expect("leg"),
        state: LegExecutionState::Completed,
        nominated_count: 0,
        validated_count: 0,
        reason_codes: BoundedSet::empty(),
        receipt_ref: ReceiptRef::new("fixture-only").expect("receipt"),
    }
}

#[test]
fn success_requires_execution_of_each_requested_leg_even_for_zero_results() {
    let mut terminal = terminal_requesting_one_leg();
    terminal
        .coverage
        .validate()
        .expect("generic coverage shape is valid");
    assert!(
        terminal.validate().is_err(),
        "no summaries must not mean work completed"
    );
    terminal.coverage.executed_legs = BoundedList::new(vec![completed_leg("L")]).expect("summary");
    terminal
        .validate()
        .expect("completed leg with zero candidates is legitimate success");
    for state in [
        LegExecutionState::Partial,
        LegExecutionState::Failed,
        LegExecutionState::Cancelled,
        LegExecutionState::DiscardedContaminated,
    ] {
        let mut incomplete = completed_leg("L");
        incomplete.state = state;
        terminal.coverage.executed_legs = BoundedList::new(vec![incomplete]).expect("summary");
        assert!(terminal.validate().is_err(), "{state:?} cannot be success");
    }
}

#[test]
fn terminal_leg_accounting_rejects_duplicate_or_foreign_ids() {
    for kind in EvidenceTerminalKind::ALL {
        let mut terminal = terminal_requesting_one_leg();
        terminal.terminal = kind;
        terminal.coverage.executed_legs =
            BoundedList::new(vec![completed_leg("L"), completed_leg("L")]).expect("summaries");
        assert!(
            terminal.validate().is_err(),
            "duplicate summaries cannot cover another leg"
        );
        terminal.coverage.executed_legs =
            BoundedList::new(vec![completed_leg("foreign")]).expect("summary");
        assert!(
            terminal.validate().is_err(),
            "foreign completed leg is not requested"
        );
        terminal.coverage.executed_legs =
            BoundedList::new(vec![completed_leg("L")]).expect("summary");
        let requested = terminal
            .coverage
            .requested_legs
            .iter()
            .next()
            .expect("leg")
            .clone();
        terminal.coverage.requested_legs =
            BoundedList::new(vec![requested.clone(), requested]).expect("legs");
        assert!(
            terminal.validate().is_err(),
            "duplicate requested IDs are malformed"
        );
    }
}

#[test]
fn partial_and_failure_may_report_missing_execution_without_claiming_success() {
    let mut terminal = terminal_requesting_one_leg();
    for kind in [EvidenceTerminalKind::Partial, EvidenceTerminalKind::Failure] {
        terminal.terminal = kind;
        terminal
            .validate()
            .expect("absence of execution remains a non-success outcome");
    }
}
