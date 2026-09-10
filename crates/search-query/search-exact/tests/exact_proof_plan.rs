//! T34 executable frozen-denominator exact proof plans.
//!
//! Executed evidence that the public `search-exact` operations prove an exact
//! predicate over a frozen authoritative denominator:
//!
//! - the denominator is captured from caller-supplied inventory, never from
//!   ranked/indexed top-k candidates or client file lists (invariant 6);
//! - every plan revalidates live access/purge/observation/profile barriers
//!   before any read (T31 currentness);
//! - partial, timeout and cancelled outcomes stay typed incomplete data and
//!   never become a complete negative (invariant 15);
//! - the literal primitive alone never mints completeness.
//!
//! Inventory and readback arrive as caller-constructed values behind
//! vendor-neutral shapes (the fake-port seam): no Qdrant, redb, filesystem,
//! network or vendor engine is touched by any test here. Indexed/Qdrant
//! behavior is out of scope for T34.

#![forbid(unsafe_code)]

use search_contracts::{
    AssuranceClass, Blake3Digest32, BoundedList, CatalogRevision, CoverageDenominatorKind,
    ExactCompletenessRequirements, ExactConclusion, ExactExecutionReport, ExactInputDomain,
    ExactItemFailureKind, ExactMatch, ExactPredicateKind, HandleClass, HandleId, NativeAnchor,
    NonZeroRevision, OpaqueHandleToken, PlanFingerprint, PlanId, ProfileId, ReceiptRef,
    SearchReasonCodeV1, SearchSourceHandle, SourceId, SourceNamespaceId, SourceRevisionId,
    SourceRevisionRef, TextBytesAnchor,
};
use search_exact::literal::{LiteralLimits, scan_chunks};
use search_exact::{
    CompiledExactPredicate, CompiledExactScan, ComplexityClass, DenominatorItem, ExactCoverage,
    ExactError, ExactExecutionBudget, ExactExecutionControl, ExactItemReadback,
    ExactMatchProjector, ExactPlanIdentity, ExactPredicateProfile, ExactProofLiveState,
    ExactProofRevalidation, InventoryCapture, InventoryCompleteness, MatchSpan,
    NormalizationPolicy, PlanCurrencyFence, PlanDisclosureFence, PlanExecutionPermit,
    PlanLiveState, PredicateCompileRequest, checkpoint_execution, classify_completeness,
    compile_exact_scan, compile_predicate, execute_exact_scan, freeze_denominator,
    resume_execution, revalidate_complete_negative, validate_plan_before_execution,
    validate_predicate_profile, verify_execution_report,
};

// ---------------------------------------------------------------------------
// Deterministic test-only digests and identifiers.
// ---------------------------------------------------------------------------

const fn uuid16(value: u128) -> [u8; 16] {
    value.to_be_bytes()
}

const fn digest_byte(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

/// Test-only deterministic digest: length-prefixed FNV-1a-64 fanned over 32
/// bytes. Distinct lengths can never collide. It makes no content-security
/// claim and never leaves the test process.
fn content_digest(bytes: &[u8]) -> Blake3Digest32 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let length = u64::try_from(bytes.len()).expect("test input fits in u64");
    let mut out = [0_u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        let lane = u64::try_from(index).expect("lane fits in u64");
        let mixed = hash
            .wrapping_add(length.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .wrapping_add(lane.wrapping_mul(0xBF58_476D_1CE4_E5B9));
        let shift = u32::try_from((index % 8) * 8).expect("shift fits in u32");
        *slot = u8::try_from((mixed >> shift) & 0xFF).expect("masked byte fits");
    }
    Blake3Digest32::from_bytes(out)
}

fn test_blake(bytes: &[u8]) -> [u8; 32] {
    *content_digest(bytes).as_bytes()
}

// ---------------------------------------------------------------------------
// Fake-port fixtures: inventory capture, predicate, plan, readback.
// ---------------------------------------------------------------------------

fn literal_profile(domain: ExactInputDomain) -> ExactPredicateProfile {
    ExactPredicateProfile {
        profile_id: ProfileId::new("test-literal-profile/1").expect("profile id"),
        kind: ExactPredicateKind::Literal,
        input_domain: domain,
        engine_and_version: ProfileId::new("test-literal-engine/1").expect("engine id"),
        complexity_profile_id: ProfileId::new("test-linear-ceiling/1").expect("ceiling id"),
        complexity: ComplexityClass::Linear,
        normalization: NormalizationPolicy::None,
        max_pattern_bytes: 64,
        max_input_bytes: 4096,
        max_matches_per_item: 16,
        max_steps_per_item: 1_000_000,
        max_structural_depth: 0,
        allows_backreferences: false,
        allows_lookaround: false,
        qualification_receipt_ref: ReceiptRef::new("test-qualification-receipt/1")
            .expect("receipt"),
        qualification_digest: digest_byte(0xA1),
    }
}

fn compile_literal(profile: ExactPredicateProfile, pattern: &[u8]) -> CompiledExactPredicate {
    let request = PredicateCompileRequest {
        kind: ExactPredicateKind::Literal,
        input_domain: profile.input_domain,
        serialized_form: pattern.to_vec(),
        normalization: profile.normalization,
    };
    compile_predicate(request, profile, test_blake).expect("literal compiles")
}

fn denominator_item(
    source: u128,
    revision: u128,
    domain: ExactInputDomain,
    text: &[u8],
    receipt_tag: &str,
) -> (DenominatorItem, Vec<u8>) {
    let bytes = text.to_vec();
    let item = DenominatorItem {
        revision: SourceRevisionRef {
            source_namespace_id: SourceNamespaceId::from_bytes(uuid16(1)),
            source_id: SourceId::from_bytes(uuid16(source)),
            revision_id: SourceRevisionId::from_bytes(uuid16(revision)),
            content_digest: content_digest(&bytes),
            byte_length: u64::try_from(bytes.len()).expect("test input fits"),
        },
        input_domain: domain,
        stable_or_retained: true,
        inventory_receipt_ref: ReceiptRef::new(receipt_tag).expect("receipt"),
    };
    (item, bytes)
}

const fn strict_requirements() -> ExactCompletenessRequirements {
    ExactCompletenessRequirements {
        require_every_denominator_item: true,
        require_stable_or_retained_revision: true,
        require_current_observation: true,
        include_authenticated_unsaved_buffers: false,
        fail_on_timeout: true,
        fail_on_cancellation: true,
        fail_on_scope_drift: true,
    }
}

const fn complete_observation() -> InventoryCompleteness {
    InventoryCompleteness {
        enumeration_complete: true,
        omitted_items: 0,
        unknown_items: 0,
        current_observation: true,
    }
}

const fn capture(
    items: Vec<DenominatorItem>,
    completeness: InventoryCompleteness,
    fence: u8,
) -> InventoryCapture {
    InventoryCapture {
        inventory_revision: CatalogRevision::new(7),
        items,
        inventory_digest: digest_byte(fence),
        source_view_digest: digest_byte(fence.wrapping_add(1)),
        owner_generation_digest: digest_byte(fence.wrapping_add(2)),
        security_fence_digest: digest_byte(fence.wrapping_add(3)),
        overlay_digest: digest_byte(fence.wrapping_add(4)),
        completeness,
    }
}

const fn plan_identity(plan_tag: u128) -> ExactPlanIdentity {
    ExactPlanIdentity {
        plan_id: PlanId::from_bytes(uuid16(plan_tag)),
        inclusion_policy_digest: digest_byte(0xC1),
        unsaved_buffer_snapshot_ids: Vec::new(),
        plan_fingerprint: PlanFingerprint::from_bytes([0xC2; 32]),
    }
}

struct ProofScope {
    capture: InventoryCapture,
    plan: CompiledExactScan,
    first: (DenominatorItem, Vec<u8>),
    second: (DenominatorItem, Vec<u8>),
}

fn proof_scope(
    first_text: &str,
    second_text: &str,
    pattern: &str,
    plan_tag: u128,
    fence: u8,
) -> ProofScope {
    let domain = ExactInputDomain::DecodedText;
    let first = denominator_item(
        11,
        21,
        domain,
        first_text.as_bytes(),
        "test-inventory-receipt/first",
    );
    let second = denominator_item(
        12,
        22,
        domain,
        second_text.as_bytes(),
        "test-inventory-receipt/second",
    );
    let frozen_capture = capture(
        vec![first.0.clone(), second.0.clone()],
        complete_observation(),
        fence,
    );
    let predicate = compile_literal(literal_profile(domain), pattern.as_bytes());
    let denominator = freeze_denominator(frozen_capture.clone(), strict_requirements(), test_blake)
        .expect("freeze");
    let plan = compile_exact_scan(
        plan_identity(plan_tag),
        predicate,
        denominator,
        strict_requirements(),
    )
    .expect("compile plan");
    ProofScope {
        capture: frozen_capture,
        plan,
        first,
        second,
    }
}

struct SingleScope {
    capture: InventoryCapture,
    plan: CompiledExactScan,
    item: DenominatorItem,
    bytes: Vec<u8>,
}

fn single_scope(
    domain: ExactInputDomain,
    bytes: &[u8],
    pattern: &[u8],
    plan_tag: u128,
    fence: u8,
) -> SingleScope {
    let (item, owned) = denominator_item(11, 21, domain, bytes, "test-inventory-receipt/only");
    let frozen_capture = capture(vec![item.clone()], complete_observation(), fence);
    let predicate = compile_literal(literal_profile(domain), pattern);
    let denominator = freeze_denominator(frozen_capture.clone(), strict_requirements(), test_blake)
        .expect("freeze");
    let plan = compile_exact_scan(
        plan_identity(plan_tag),
        predicate,
        denominator,
        strict_requirements(),
    )
    .expect("compile plan");
    SingleScope {
        capture: frozen_capture,
        plan,
        item,
        bytes: owned,
    }
}

const fn live_current(frozen_capture: &InventoryCapture) -> PlanLiveState {
    PlanLiveState {
        inventory_revision: frozen_capture.inventory_revision,
        inventory_digest: frozen_capture.inventory_digest,
        source_view_digest: frozen_capture.source_view_digest,
        owner_generation_digest: frozen_capture.owner_generation_digest,
        security_fence_digest: frozen_capture.security_fence_digest,
        overlay_digest: frozen_capture.overlay_digest,
        disclosure: PlanDisclosureFence {
            access_permitted: true,
            purge_clear: true,
        },
        currency: PlanCurrencyFence {
            current_observation: true,
            predicate_profile_current: true,
        },
    }
}

fn live_permit(scope: &ProofScope) -> PlanExecutionPermit {
    validate_plan_before_execution(&scope.plan, live_current(&scope.capture)).expect("permit")
}

fn healthy_readback(item: &DenominatorItem, bytes: Vec<u8>) -> ExactItemReadback {
    ExactItemReadback {
        revision: item.revision,
        observed_content_digest: content_digest(&bytes),
        bytes,
        readback_receipt_ref: ReceiptRef::new("test-readback-receipt/1").expect("receipt"),
        access_permitted: true,
        purge_clear: true,
        current: true,
    }
}

struct TestProjector {
    assurance: AssuranceClass,
}

impl ExactMatchProjector for TestProjector {
    fn project(
        &self,
        item: &DenominatorItem,
        readback: &ExactItemReadback,
        predicate: &CompiledExactPredicate,
        span: MatchSpan,
    ) -> Result<ExactMatch, ExactError> {
        let start = usize::try_from(span.byte_start)
            .map_err(|_| ExactError::ExactPredicateLimitExceeded)?;
        let end =
            usize::try_from(span.byte_end).map_err(|_| ExactError::ExactPredicateLimitExceeded)?;
        let matched = readback
            .bytes
            .get(start..end)
            .ok_or(ExactError::ExactReportInvalid)?;
        let length = span
            .byte_end
            .checked_sub(span.byte_start)
            .ok_or(ExactError::ExactReportInvalid)?;
        Ok(ExactMatch {
            source_revision_ref: item.revision,
            native_anchor: NativeAnchor::TextBytes(TextBytesAnchor {
                content_digest: readback.observed_content_digest,
                byte_start_0: span.byte_start,
                byte_end_exclusive_0: span.byte_end,
            }),
            match_digest: content_digest(matched),
            matched_byte_length: length,
            predicate_profile_id: predicate.profile().profile_id.clone(),
            assurance: self.assurance,
            source_handle: SearchSourceHandle {
                handle_id: HandleId::from_bytes(uuid16(0xBEEF)),
                handle_revision: NonZeroRevision::new(1)
                    .map_err(|_| ExactError::ContractViolation)?,
                handle_class: HandleClass::Ephemeral,
                expires_at: None,
                opaque_token: OpaqueHandleToken::new(&[0x5A; 32])
                    .map_err(|_| ExactError::ContractViolation)?,
            },
        })
    }
}

struct TestControl {
    cancelled: bool,
    expired: bool,
}

impl ExactExecutionControl for TestControl {
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn deadline_expired(&self) -> bool {
        self.expired
    }
}

const fn live_control() -> TestControl {
    TestControl {
        cancelled: false,
        expired: false,
    }
}

fn ample_budget() -> ExactExecutionBudget {
    ExactExecutionBudget {
        max_items: 16,
        max_bytes: 1 << 20,
        max_matches: 64,
    }
    .validate()
    .expect("budget")
}

fn execute_scope(
    scope: &ProofScope,
    readbacks: Vec<ExactItemReadback>,
    control: &TestControl,
    budget: ExactExecutionBudget,
) -> ExactExecutionReport {
    let permit = live_permit(scope);
    let projector = TestProjector {
        assurance: AssuranceClass::MappedText,
    };
    execute_exact_scan(
        &scope.plan,
        &permit,
        readbacks,
        budget,
        control,
        None,
        &projector,
        ReceiptRef::new("test-execution-receipt/1").expect("receipt"),
    )
    .expect("execute")
}

const fn proof_live(scope: &ProofScope) -> ExactProofLiveState {
    ExactProofLiveState {
        denominator_digest: scope.plan.denominator().denominator_digest(),
        predicate_digest: scope.plan.predicate().predicate_digest(),
        security_fence_digest: scope.capture.security_fence_digest,
        access_permitted: true,
        purge_clear: true,
        current_observation: true,
    }
}

// ---------------------------------------------------------------------------
// Required exit evidence.
// ---------------------------------------------------------------------------

#[test]
fn complete_literal_negative_requires_every_denominator_item() {
    let scope = proof_scope("alpha beta", "gamma delta", "needle", 0xA1, 0x10);
    let readbacks = vec![
        healthy_readback(&scope.first.0, scope.first.1.clone()),
        healthy_readback(&scope.second.0, scope.second.1.clone()),
    ];
    let report = execute_scope(&scope, readbacks, &live_control(), ample_budget());
    assert!(report.matched_items.is_empty());
    assert_eq!(report.conclusion, ExactConclusion::NoMatchInCompleteScope);
    assert_eq!(report.coverage, CoverageDenominatorKind::CompleteScope);
    assert_eq!(report.scanned_items, 2);
    assert!(!report.timed_out);
    assert!(!report.cancelled);
    assert!(!report.scope_drifted);
    assert_eq!(
        classify_completeness(&scope.plan, &report),
        ExactCoverage::NoMatchInCompleteScope
    );
    let receipt = verify_execution_report(&scope.plan, &report, test_blake).expect("verify");
    assert_eq!(receipt.coverage, ExactCoverage::NoMatchInCompleteScope);
    assert_eq!(
        revalidate_complete_negative(&receipt, proof_live(&scope)),
        ExactProofRevalidation::Current
    );

    // One explicitly omitted source forbids the complete negative: the same
    // zero-match execution stays typed incomplete data.
    let gapped = capture(
        vec![scope.first.0.clone(), scope.second.0.clone()],
        InventoryCompleteness {
            enumeration_complete: true,
            omitted_items: 1,
            unknown_items: 0,
            current_observation: true,
        },
        0x20,
    );
    let relaxed = ExactCompletenessRequirements {
        require_every_denominator_item: false,
        ..strict_requirements()
    };
    let denominator =
        freeze_denominator(gapped.clone(), relaxed, test_blake).expect("relaxed freeze");
    let predicate = compile_literal(literal_profile(ExactInputDomain::DecodedText), b"needle");
    let plan =
        compile_exact_scan(plan_identity(0xB2), predicate, denominator, relaxed).expect("plan");
    let permit =
        validate_plan_before_execution(&plan, live_current(&gapped)).expect("relaxed permit");
    let projector = TestProjector {
        assurance: AssuranceClass::MappedText,
    };
    let report = execute_exact_scan(
        &plan,
        &permit,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            healthy_readback(&scope.second.0, scope.second.1.clone()),
        ],
        ample_budget(),
        &live_control(),
        None,
        &projector,
        ReceiptRef::new("test-execution-receipt/relaxed").expect("receipt"),
    )
    .expect("relaxed execute");
    assert!(report.matched_items.is_empty());
    assert_eq!(report.conclusion, ExactConclusion::Incomplete);
    assert_eq!(report.coverage, CoverageDenominatorKind::CandidateScope);
    assert_eq!(
        classify_completeness(&plan, &report),
        ExactCoverage::IncompleteNoMatch
    );
    let receipt = verify_execution_report(&plan, &report, test_blake).expect("relaxed verify");
    let live = ExactProofLiveState {
        denominator_digest: plan.denominator().denominator_digest(),
        predicate_digest: plan.predicate().predicate_digest(),
        security_fence_digest: gapped.security_fence_digest,
        access_permitted: true,
        purge_clear: true,
        current_observation: true,
    };
    assert_eq!(
        revalidate_complete_negative(&receipt, live),
        ExactProofRevalidation::Invalid
    );
}

#[test]
fn unreadable_or_changed_item_blocks_complete_negative() {
    let scope = proof_scope("alpha beta", "gamma delta", "needle", 0xA2, 0x11);

    // Denied access on the second item: typed partial data, never complete.
    let mut denied = healthy_readback(&scope.second.0, scope.second.1.clone());
    denied.access_permitted = false;
    let report = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            denied,
        ],
        &live_control(),
        ample_budget(),
    );
    assert!(report.matched_items.is_empty());
    assert_eq!(report.conclusion, ExactConclusion::Incomplete);
    assert!(report.scope_drifted);
    assert_eq!(report.changed_or_unavailable_items.len(), 1);
    let failure = report
        .changed_or_unavailable_items
        .iter()
        .next()
        .expect("one failure");
    assert_eq!(failure.failure_kind, ExactItemFailureKind::ScopeChanged);
    assert!(
        failure
            .reason_codes
            .contains(&SearchReasonCodeV1::AccessRevoked)
    );
    assert_eq!(
        classify_completeness(&scope.plan, &report),
        ExactCoverage::IncompleteNoMatch
    );

    // Changed bytes (digest/length mismatch against the frozen revision).
    let mut changed = scope.second.1.clone();
    changed.extend_from_slice(b"!");
    let report = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            healthy_readback(&scope.second.0, changed),
        ],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(report.conclusion, ExactConclusion::Incomplete);
    let failure = report
        .changed_or_unavailable_items
        .iter()
        .next()
        .expect("one failure");
    assert_eq!(
        failure.failure_kind,
        ExactItemFailureKind::RevisionUnavailable
    );

    // Missing readback for the second item.
    let report = execute_scope(
        &scope,
        vec![healthy_readback(&scope.first.0, scope.first.1.clone())],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(report.scanned_items, 1);
    assert_eq!(report.conclusion, ExactConclusion::Incomplete);

    // A purge barrier over the second item.
    let mut purged = healthy_readback(&scope.second.0, scope.second.1.clone());
    purged.purge_clear = false;
    let report = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            purged,
        ],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(report.conclusion, ExactConclusion::Incomplete);
    let failure = report
        .changed_or_unavailable_items
        .iter()
        .next()
        .expect("one failure");
    assert!(failure.reason_codes.contains(&SearchReasonCodeV1::Purged));

    // Healthy-item matches are preserved as typed partial data, not dropped
    // and not promoted to a complete claim.
    let matched = proof_scope("has needle here", "gamma delta", "needle", 0xA3, 0x12);
    let mut denied = healthy_readback(&matched.second.0, matched.second.1.clone());
    denied.access_permitted = false;
    let report = execute_scope(
        &matched,
        vec![
            healthy_readback(&matched.first.0, matched.first.1.clone()),
            denied,
        ],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(report.matched_items.len(), 1);
    assert_eq!(report.conclusion, ExactConclusion::MatchesFound);
    assert_eq!(report.coverage, CoverageDenominatorKind::CandidateScope);
    assert_eq!(
        classify_completeness(&matched.plan, &report),
        ExactCoverage::MatchesFound
    );
}

#[test]
fn raw_bytes_and_decoded_text_semantics_are_distinct() {
    let bytes = vec![0xFF, 0xFE, b'a', b'b'];

    // Raw bytes scan opaque content; an absent pattern is a complete negative.
    let raw = single_scope(ExactInputDomain::RawBytes, &bytes, b"zz", 0xA4, 0x13);
    let permit =
        validate_plan_before_execution(&raw.plan, live_current(&raw.capture)).expect("permit");
    let projector = TestProjector {
        assurance: AssuranceClass::ExactBytes,
    };
    let report = execute_exact_scan(
        &raw.plan,
        &permit,
        vec![healthy_readback(&raw.item, raw.bytes.clone())],
        ample_budget(),
        &live_control(),
        None,
        &projector,
        ReceiptRef::new("test-execution-receipt/raw").expect("receipt"),
    )
    .expect("raw execute");
    assert_eq!(report.conclusion, ExactConclusion::NoMatchInCompleteScope);

    // The same bytes as decoded text are explicitly unsupported, never a
    // silent empty result promoted to a negative proof.
    let text = single_scope(ExactInputDomain::DecodedText, &bytes, b"zz", 0xA5, 0x14);
    let permit =
        validate_plan_before_execution(&text.plan, live_current(&text.capture)).expect("permit");
    let projector = TestProjector {
        assurance: AssuranceClass::MappedText,
    };
    let report = execute_exact_scan(
        &text.plan,
        &permit,
        vec![healthy_readback(&text.item, text.bytes.clone())],
        ample_budget(),
        &live_control(),
        None,
        &projector,
        ReceiptRef::new("test-execution-receipt/text").expect("receipt"),
    )
    .expect("text execute");
    assert_eq!(report.conclusion, ExactConclusion::Incomplete);
    let failure = report.unreadable_items.iter().next().expect("one failure");
    assert_eq!(
        failure.failure_kind,
        ExactItemFailureKind::UnsupportedEncoding
    );
    assert_eq!(
        classify_completeness(&text.plan, &report),
        ExactCoverage::IncompleteNoMatch
    );
}

#[test]
fn safe_regex_is_size_and_time_bounded() {
    // Backreferences, lookaround and non-linear complexity are never qualified.
    let backref = ExactPredicateProfile {
        kind: ExactPredicateKind::Regex,
        allows_backreferences: true,
        ..literal_profile(ExactInputDomain::DecodedText)
    };
    assert_eq!(
        validate_predicate_profile(&backref),
        Err(ExactError::ExactEngineNotQualified)
    );
    let lookaround = ExactPredicateProfile {
        kind: ExactPredicateKind::Regex,
        allows_lookaround: true,
        ..literal_profile(ExactInputDomain::DecodedText)
    };
    assert_eq!(
        validate_predicate_profile(&lookaround),
        Err(ExactError::ExactEngineNotQualified)
    );
    let unbounded = ExactPredicateProfile {
        kind: ExactPredicateKind::Regex,
        complexity: ComplexityClass::BoundedStructural,
        max_structural_depth: 2,
        ..literal_profile(ExactInputDomain::DecodedText)
    };
    assert_eq!(
        validate_predicate_profile(&unbounded),
        Err(ExactError::ExactEngineNotQualified)
    );
    // Regex over raw bytes is a domain mismatch, never an implicit decoding.
    let domain_mismatch = ExactPredicateProfile {
        kind: ExactPredicateKind::Regex,
        input_domain: ExactInputDomain::RawBytes,
        ..literal_profile(ExactInputDomain::DecodedText)
    };
    assert_eq!(
        validate_predicate_profile(&domain_mismatch),
        Err(ExactError::ExactRequestInvalid)
    );

    // A safe non-backtracking regex profile compiles, but without a pinned
    // qualified engine nothing executes: no unbounded backtracking runtime.
    let safe = ExactPredicateProfile {
        kind: ExactPredicateKind::Regex,
        ..literal_profile(ExactInputDomain::DecodedText)
    };
    validate_predicate_profile(&safe).expect("safe profile");
    let request = PredicateCompileRequest {
        kind: ExactPredicateKind::Regex,
        input_domain: ExactInputDomain::DecodedText,
        serialized_form: b"a+b".to_vec(),
        normalization: NormalizationPolicy::None,
    };
    let predicate = compile_predicate(request, safe, test_blake).expect("safe regex compiles");
    assert_eq!(
        search_exact::execute_predicate(
            &predicate,
            search_exact::ExactInput::DecodedText("aaab"),
            None
        ),
        Err(ExactError::ExactPredicateUnsupported)
    );
}

#[test]
fn cancellation_and_scope_drift_are_incomplete() {
    let scope = proof_scope("alpha needle", "beta needle", "needle", 0xA6, 0x15);
    let readbacks = || {
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            healthy_readback(&scope.second.0, scope.second.1.clone()),
        ]
    };

    // Cancellation before items: every item is an explicit Cancelled failure.
    let cancelled = TestControl {
        cancelled: true,
        expired: false,
    };
    let report = execute_scope(&scope, readbacks(), &cancelled, ample_budget());
    assert!(report.matched_items.is_empty());
    assert!(report.cancelled);
    assert_eq!(report.scanned_items, 0);
    assert_eq!(report.unreadable_items.len(), 2);
    assert_eq!(report.conclusion, ExactConclusion::Incomplete);
    assert_eq!(
        classify_completeness(&scope.plan, &report),
        ExactCoverage::IncompleteNoMatch
    );
    let receipt = verify_execution_report(&scope.plan, &report, test_blake).expect("verify");
    assert_eq!(
        revalidate_complete_negative(&receipt, proof_live(&scope)),
        ExactProofRevalidation::Invalid
    );

    // Expired deadline: explicit Timeout failures, never a negative proof.
    let expired = TestControl {
        cancelled: false,
        expired: true,
    };
    let report = execute_scope(&scope, readbacks(), &expired, ample_budget());
    assert!(report.timed_out);
    assert_eq!(report.unreadable_items.len(), 2);
    assert_eq!(report.conclusion, ExactConclusion::Incomplete);

    // A one-item budget spends the second item as an explicit Timeout gap.
    // The first item's match is preserved as typed partial data: MatchesFound
    // over a candidate scope, never a complete claim.
    let tight = ExactExecutionBudget {
        max_items: 1,
        max_bytes: 1 << 20,
        max_matches: 64,
    }
    .validate()
    .expect("budget");
    let report = execute_scope(&scope, readbacks(), &live_control(), tight);
    assert!(report.timed_out);
    assert_eq!(report.scanned_items, 1);
    assert_eq!(report.matched_items.len(), 1);
    assert_eq!(report.conclusion, ExactConclusion::MatchesFound);
    assert_eq!(report.coverage, CoverageDenominatorKind::CandidateScope);
    assert_eq!(
        classify_completeness(&scope.plan, &report),
        ExactCoverage::MatchesFound
    );
    let failure = report.unreadable_items.iter().next().expect("one failure");
    assert_eq!(failure.failure_kind, ExactItemFailureKind::Timeout);

    // Stale observation on one item drifts the scope and blocks completeness:
    // the healthy item's match stays typed partial data, never complete.
    let mut stale = healthy_readback(&scope.second.0, scope.second.1.clone());
    stale.current = false;
    let report = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            stale,
        ],
        &live_control(),
        ample_budget(),
    );
    assert!(report.scope_drifted);
    assert_eq!(report.conclusion, ExactConclusion::MatchesFound);
    assert_eq!(report.coverage, CoverageDenominatorKind::CandidateScope);
    assert_eq!(
        classify_completeness(&scope.plan, &report),
        ExactCoverage::MatchesFound
    );
}

#[test]
fn semantic_overclaim_rejected() {
    let scope_a = proof_scope("alpha beta", "gamma delta", "needle", 0xA7, 0x16);
    let report_a = execute_scope(
        &scope_a,
        vec![
            healthy_readback(&scope_a.first.0, scope_a.first.1.clone()),
            healthy_readback(&scope_a.second.0, scope_a.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(report_a.conclusion, ExactConclusion::NoMatchInCompleteScope);

    // A complete negative for literal "needle" is not an absence proof for a
    // different predicate: verification and classification fail closed.
    let scope_b = proof_scope("alpha beta", "gamma delta", "thread", 0xA8, 0x16);
    assert_eq!(
        verify_execution_report(&scope_b.plan, &report_a, test_blake),
        Err(ExactError::ExactReportInvalid)
    );
    assert_eq!(
        classify_completeness(&scope_b.plan, &report_a),
        ExactCoverage::ExecutionInvalid
    );
    let receipt_a =
        verify_execution_report(&scope_a.plan, &report_a, test_blake).expect("verify A");
    let live_b = ExactProofLiveState {
        predicate_digest: scope_b.plan.predicate().predicate_digest(),
        ..proof_live(&scope_a)
    };
    assert_eq!(
        revalidate_complete_negative(&receipt_a, live_b),
        ExactProofRevalidation::HistoricalOnly
    );

    // Positive reports do not transfer either.
    let needle_scope = proof_scope("has needle", "gamma delta", "needle", 0xA9, 0x17);
    let needle_report = execute_scope(
        &needle_scope,
        vec![
            healthy_readback(&needle_scope.first.0, needle_scope.first.1.clone()),
            healthy_readback(&needle_scope.second.0, needle_scope.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(needle_report.conclusion, ExactConclusion::MatchesFound);
    let other = proof_scope("has needle", "gamma delta", "thread", 0xAA, 0x17);
    assert_eq!(
        verify_execution_report(&other.plan, &needle_report, test_blake),
        Err(ExactError::ExactReportInvalid)
    );
}

#[test]
fn fake_inventory_and_readback_ports_prove_adapter_independence() {
    // The same source bytes through two independently built fake inventories
    // (different fences, receipts, plan identities) prove the same predicate
    // outcomes with no shared adapter state and no fixed execution order.
    let early = proof_scope("alpha needle beta", "gamma delta", "needle", 0xB1, 0x30);
    let late = proof_scope("alpha needle beta", "gamma delta", "needle", 0xB2, 0x40);
    assert_ne!(
        early.plan.denominator().denominator_digest(),
        late.plan.denominator().denominator_digest(),
        "distinct inventories must never alias one proof"
    );
    let late_report = execute_scope(
        &late,
        vec![
            healthy_readback(&late.first.0, late.first.1.clone()),
            healthy_readback(&late.second.0, late.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    let early_report = execute_scope(
        &early,
        vec![
            healthy_readback(&early.first.0, early.first.1.clone()),
            healthy_readback(&early.second.0, early.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    for report in [&early_report, &late_report] {
        assert_eq!(report.matched_items.len(), 1);
        assert_eq!(report.conclusion, ExactConclusion::MatchesFound);
    }
    let early_spans: Vec<(u64, u64)> = early_report
        .matched_items
        .iter()
        .map(|exact_match| match &exact_match.native_anchor {
            NativeAnchor::TextBytes(anchor) => (anchor.byte_start_0, anchor.byte_end_exclusive_0),
            _ => panic!("test projector emits text-byte anchors only"),
        })
        .collect();
    let late_spans: Vec<(u64, u64)> = late_report
        .matched_items
        .iter()
        .map(|exact_match| match &exact_match.native_anchor {
            NativeAnchor::TextBytes(anchor) => (anchor.byte_start_0, anchor.byte_end_exclusive_0),
            _ => panic!("test projector emits text-byte anchors only"),
        })
        .collect();
    assert_eq!(early_spans, late_spans);
    assert_eq!(early_spans, vec![(6, 12)]);
    verify_execution_report(&early.plan, &early_report, test_blake).expect("verify early");
    verify_execution_report(&late.plan, &late_report, test_blake).expect("verify late");

    // A narrowed inventory cannot freeze as complete under strict
    // requirements: a top-k style omission recorded truthfully is rejected
    // before any plan exists, so it can never become a complete claim.
    let narrowed = capture(
        vec![early.first.0, early.second.0],
        InventoryCompleteness {
            enumeration_complete: true,
            omitted_items: 1,
            unknown_items: 0,
            current_observation: true,
        },
        0x31,
    );
    assert_eq!(
        freeze_denominator(narrowed, strict_requirements(), test_blake),
        Err(ExactError::ExactDenominatorIncomplete)
    );
}

#[test]
fn literal_scan_alone_never_mints_completeness() {
    let limits = LiteralLimits {
        max_query_bytes: 64,
        max_input_bytes: 4096,
        max_chunks: 128,
        max_matches: 100,
    };
    // A complete scan over supplied chunks proves only those chunks.
    let chunks =
        scan_chunks(&["al", "pha needle be", "ta"], "needle", false, limits).expect("chunk scan");
    assert_eq!(chunks.matches.len(), 1);
    assert!(chunks.complete());

    // An empty complete scan over one item's bytes coexists with a proof that
    // finds the predicate elsewhere in the frozen denominator.
    let lone = scan_chunks(&["xx"], "needle", false, limits).expect("lone scan");
    assert!(lone.matches.is_empty());
    assert!(lone.complete());
    let scope = proof_scope("xx", "needle here", "needle", 0xB3, 0x32);
    let report = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            healthy_readback(&scope.second.0, scope.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(report.matched_items.len(), 1);
    assert_eq!(report.conclusion, ExactConclusion::MatchesFound);
    assert_eq!(
        classify_completeness(&scope.plan, &report),
        ExactCoverage::MatchesFound
    );
}

#[test]
fn timeout_cancel_partial_never_upgrade() {
    let scope = proof_scope("alpha needle", "beta", "needle", 0xB4, 0x33);
    let cancelled = TestControl {
        cancelled: true,
        expired: false,
    };
    let report = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            healthy_readback(&scope.second.0, scope.second.1.clone()),
        ],
        &cancelled,
        ample_budget(),
    );
    // A cancelled report is valid typed data, but verification cannot promote
    // it past incomplete and revalidation refuses it outright.
    let receipt = verify_execution_report(&scope.plan, &report, test_blake).expect("verify");
    assert_eq!(receipt.coverage, ExactCoverage::IncompleteNoMatch);
    assert_eq!(
        classify_completeness(&scope.plan, &report),
        ExactCoverage::IncompleteNoMatch
    );
    assert_eq!(
        revalidate_complete_negative(&receipt, proof_live(&scope)),
        ExactProofRevalidation::Invalid
    );

    // Relabelling an incomplete report as a complete negative fails closed at
    // verification and classifies as invalid, never as proven absence.
    let denied = proof_scope("alpha beta", "gamma delta", "needle", 0xB5, 0x34);
    let mut missing = healthy_readback(&denied.second.0, denied.second.1.clone());
    missing.access_permitted = false;
    let partial = execute_scope(
        &denied,
        vec![
            healthy_readback(&denied.first.0, denied.first.1.clone()),
            missing,
        ],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(partial.conclusion, ExactConclusion::Incomplete);
    let mut relabelled = partial.clone();
    relabelled.conclusion = ExactConclusion::NoMatchInCompleteScope;
    assert_eq!(
        verify_execution_report(&denied.plan, &relabelled, test_blake),
        Err(ExactError::ExactReportInvalid)
    );
    assert_eq!(
        classify_completeness(&denied.plan, &relabelled),
        ExactCoverage::ExecutionInvalid
    );
    let mut upgraded = partial;
    upgraded.coverage = CoverageDenominatorKind::CompleteScope;
    assert_eq!(
        verify_execution_report(&denied.plan, &upgraded, test_blake),
        Err(ExactError::ExactReportInvalid)
    );
}

#[test]
fn foreign_match_outside_denominator_is_rejected() {
    let scope = proof_scope("needle here", "also needle", "needle", 0xB6, 0x35);
    let report = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            healthy_readback(&scope.second.0, scope.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    assert_eq!(report.conclusion, ExactConclusion::MatchesFound);
    assert_eq!(report.matched_items.len(), 2);
    verify_execution_report(&scope.plan, &report, test_blake).expect("honest verify");

    // A match minted outside the frozen denominator is rejected, even though
    // report shape, counts and conclusion stay internally consistent.
    let foreign = ExactMatch {
        source_revision_ref: SourceRevisionRef {
            source_namespace_id: SourceNamespaceId::from_bytes(uuid16(9)),
            source_id: SourceId::from_bytes(uuid16(9)),
            revision_id: SourceRevisionId::from_bytes(uuid16(0xFFFF)),
            content_digest: digest_byte(9),
            byte_length: 3,
        },
        native_anchor: NativeAnchor::TextBytes(TextBytesAnchor {
            content_digest: digest_byte(9),
            byte_start_0: 0,
            byte_end_exclusive_0: 3,
        }),
        match_digest: digest_byte(9),
        matched_byte_length: 3,
        predicate_profile_id: scope.plan.predicate().profile().profile_id.clone(),
        assurance: AssuranceClass::MappedText,
        source_handle: SearchSourceHandle {
            handle_id: HandleId::from_bytes(uuid16(0xBEEF)),
            handle_revision: NonZeroRevision::new(1).expect("nonzero"),
            handle_class: HandleClass::Ephemeral,
            expires_at: None,
            opaque_token: OpaqueHandleToken::new(&[0x5A; 32]).expect("token"),
        },
    };
    let mut tampered = report.clone();
    tampered
        .matched_items
        .try_push(foreign)
        .expect("bounded push");
    assert_eq!(
        verify_execution_report(&scope.plan, &tampered, test_blake),
        Err(ExactError::ExactReportInvalid)
    );

    // So is a match with a known identity but substituted revision bytes: the
    // verified binding covers the exact frozen revision, not just its id.
    let rewritten: Vec<ExactMatch> = report
        .clone()
        .matched_items
        .into_vec()
        .into_iter()
        .map(|mut exact_match| {
            exact_match.source_revision_ref.content_digest = digest_byte(0xDD);
            exact_match
        })
        .collect();
    let mut digest_tampered = report;
    digest_tampered.matched_items = BoundedList::new(rewritten).expect("bounded");
    assert_eq!(
        verify_execution_report(&scope.plan, &digest_tampered, test_blake),
        Err(ExactError::ExactReportInvalid)
    );
}

#[test]
fn checkpoint_resume_restart_from_retained_bytes() {
    let scope = proof_scope("alpha needle", "beta", "needle", 0xB7, 0x36);
    let first_id = scope.first.0.revision.revision_id;
    let second_id = scope.second.0.revision.revision_id;

    let checkpoint = checkpoint_execution(
        &scope.plan,
        vec![first_id],
        vec![ReceiptRef::new("test-checkpoint-receipt/1").expect("receipt")],
        test_blake,
    )
    .expect("checkpoint");
    let remaining =
        resume_execution(&scope.plan, &checkpoint, checkpoint.checkpoint_digest).expect("resume");
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining.iter().next().copied(),
        Some(second_id),
        "resume continues in frozen order"
    );

    // Checkpoint forgery and accounting contradictions fail closed.
    assert_eq!(
        resume_execution(&scope.plan, &checkpoint, digest_byte(0xFF)),
        Err(ExactError::ExactReportInvalid)
    );
    assert!(
        checkpoint_execution(
            &scope.plan,
            vec![first_id, first_id],
            vec![
                ReceiptRef::new("test-checkpoint-receipt/1").expect("receipt"),
                ReceiptRef::new("test-checkpoint-receipt/2").expect("receipt"),
            ],
            test_blake,
        )
        .is_err()
    );
    assert!(
        checkpoint_execution(
            &scope.plan,
            vec![SourceRevisionId::from_bytes(uuid16(0xFFFF))],
            vec![ReceiptRef::new("test-checkpoint-receipt/1").expect("receipt")],
            test_blake,
        )
        .is_err()
    );
    assert!(
        checkpoint_execution(
            &scope.plan,
            vec![first_id],
            vec![
                ReceiptRef::new("test-checkpoint-receipt/1").expect("receipt"),
                ReceiptRef::new("test-checkpoint-receipt/2").expect("receipt"),
            ],
            test_blake,
        )
        .is_err()
    );

    // After a restart the same frozen inputs and retained bytes reconstruct
    // the identical proof: digests are byte-determined, never process-local.
    let rebuilt = proof_scope("alpha needle", "beta", "needle", 0xB7, 0x36);
    assert_eq!(
        rebuilt.plan.denominator().denominator_digest(),
        scope.plan.denominator().denominator_digest()
    );
    assert_eq!(
        rebuilt.plan.predicate().predicate_digest(),
        scope.plan.predicate().predicate_digest()
    );
    let report = execute_scope(
        &rebuilt,
        vec![
            healthy_readback(&rebuilt.first.0, rebuilt.first.1.clone()),
            healthy_readback(&rebuilt.second.0, rebuilt.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    let receipt = verify_execution_report(&rebuilt.plan, &report, test_blake).expect("verify");
    let original = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            healthy_readback(&scope.second.0, scope.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    let expected =
        verify_execution_report(&scope.plan, &original, test_blake).expect("verify original");
    assert_eq!(receipt.report_digest, expected.report_digest);
}

#[test]
fn live_barrier_revalidation_matrix() {
    let scope = proof_scope("alpha beta", "gamma delta", "needle", 0xB8, 0x37);
    let current = live_current(&scope.capture);
    validate_plan_before_execution(&scope.plan, current).expect("current permit");

    let denied = PlanLiveState {
        disclosure: PlanDisclosureFence {
            access_permitted: false,
            purge_clear: true,
        },
        ..current
    };
    assert_eq!(
        validate_plan_before_execution(&scope.plan, denied),
        Err(ExactError::ExactAccessRevoked)
    );
    let purged = PlanLiveState {
        disclosure: PlanDisclosureFence {
            access_permitted: true,
            purge_clear: false,
        },
        ..current
    };
    assert_eq!(
        validate_plan_before_execution(&scope.plan, purged),
        Err(ExactError::ExactPurged)
    );
    let stale_profile = PlanLiveState {
        currency: PlanCurrencyFence {
            current_observation: true,
            predicate_profile_current: false,
        },
        ..current
    };
    assert_eq!(
        validate_plan_before_execution(&scope.plan, stale_profile),
        Err(ExactError::ExactEngineNotQualified)
    );
    let gapped = PlanLiveState {
        currency: PlanCurrencyFence {
            current_observation: false,
            predicate_profile_current: true,
        },
        ..current
    };
    assert_eq!(
        validate_plan_before_execution(&scope.plan, gapped),
        Err(ExactError::ExactObservationGap)
    );
    for drifted in [
        PlanLiveState {
            inventory_digest: digest_byte(0xF0),
            ..current
        },
        PlanLiveState {
            source_view_digest: digest_byte(0xF1),
            ..current
        },
        PlanLiveState {
            owner_generation_digest: digest_byte(0xF2),
            ..current
        },
        PlanLiveState {
            security_fence_digest: digest_byte(0xF3),
            ..current
        },
        PlanLiveState {
            overlay_digest: digest_byte(0xF4),
            ..current
        },
        PlanLiveState {
            inventory_revision: CatalogRevision::new(8),
            ..current
        },
    ] {
        assert_eq!(
            validate_plan_before_execution(&scope.plan, drifted),
            Err(ExactError::ExactDenominatorDrift)
        );
    }

    // A permit minted for one plan never authorizes a different frozen plan.
    let other = proof_scope("alpha beta", "gamma delta", "needle", 0xB9, 0x37);
    let foreign_permit = live_permit(&other);
    let projector = TestProjector {
        assurance: AssuranceClass::MappedText,
    };
    assert_eq!(
        execute_exact_scan(
            &scope.plan,
            &foreign_permit,
            vec![
                healthy_readback(&scope.first.0, scope.first.1.clone()),
                healthy_readback(&scope.second.0, scope.second.1.clone()),
            ],
            ample_budget(),
            &live_control(),
            None,
            &projector,
            ReceiptRef::new("test-execution-receipt/misuse").expect("receipt"),
        ),
        Err(ExactError::ExactDenominatorDrift)
    );
}

#[test]
fn proof_revalidation_tracks_live_fences() {
    let scope = proof_scope("alpha beta", "gamma delta", "needle", 0xBA, 0x38);
    let report = execute_scope(
        &scope,
        vec![
            healthy_readback(&scope.first.0, scope.first.1.clone()),
            healthy_readback(&scope.second.0, scope.second.1.clone()),
        ],
        &live_control(),
        ample_budget(),
    );
    let receipt = verify_execution_report(&scope.plan, &report, test_blake).expect("verify");
    let live = proof_live(&scope);
    assert_eq!(
        revalidate_complete_negative(&receipt, live),
        ExactProofRevalidation::Current
    );
    assert_eq!(
        revalidate_complete_negative(
            &receipt,
            ExactProofLiveState {
                denominator_digest: digest_byte(0xE0),
                ..live
            }
        ),
        ExactProofRevalidation::HistoricalOnly
    );
    assert_eq!(
        revalidate_complete_negative(
            &receipt,
            ExactProofLiveState {
                predicate_digest: digest_byte(0xE1),
                ..live
            }
        ),
        ExactProofRevalidation::HistoricalOnly
    );
    assert_eq!(
        revalidate_complete_negative(
            &receipt,
            ExactProofLiveState {
                current_observation: false,
                ..live
            }
        ),
        ExactProofRevalidation::HistoricalOnly
    );
    assert_eq!(
        revalidate_complete_negative(
            &receipt,
            ExactProofLiveState {
                access_permitted: false,
                ..live
            }
        ),
        ExactProofRevalidation::AccessRevoked
    );
    assert_eq!(
        revalidate_complete_negative(
            &receipt,
            ExactProofLiveState {
                purge_clear: false,
                ..live
            }
        ),
        ExactProofRevalidation::Purged
    );

    // A historical frozen proof is internally valid but stale for current
    // scope claims once the denominator moves on.
    let partial_scope = proof_scope("alpha beta", "gamma delta", "needle", 0xBB, 0x39);
    let mut denied = healthy_readback(&partial_scope.second.0, partial_scope.second.1.clone());
    denied.access_permitted = false;
    let partial = execute_scope(
        &partial_scope,
        vec![
            healthy_readback(&partial_scope.first.0, partial_scope.first.1.clone()),
            denied,
        ],
        &live_control(),
        ample_budget(),
    );
    let partial_receipt =
        verify_execution_report(&partial_scope.plan, &partial, test_blake).expect("verify");
    assert_eq!(
        revalidate_complete_negative(&partial_receipt, proof_live(&partial_scope)),
        ExactProofRevalidation::Invalid
    );
}

#[test]
fn frozen_denominator_is_deterministic_and_topk_never_aliases() {
    let domain = ExactInputDomain::DecodedText;
    let (item_a, _) = denominator_item(11, 21, domain, b"alpha", "test-inventory-receipt/a");
    let (item_b, _) = denominator_item(12, 22, domain, b"beta", "test-inventory-receipt/b");

    // Input order never affects the frozen proof identity.
    let ordered = freeze_denominator(
        capture(
            vec![item_a.clone(), item_b.clone()],
            complete_observation(),
            0x50,
        ),
        strict_requirements(),
        test_blake,
    )
    .expect("ordered freeze");
    let reversed = freeze_denominator(
        capture(vec![item_b, item_a.clone()], complete_observation(), 0x50),
        strict_requirements(),
        test_blake,
    )
    .expect("reversed freeze");
    assert_eq!(ordered.denominator_digest(), reversed.denominator_digest());
    assert_eq!(ordered.contract().source_revision_ids.len(), 2);

    // Duplicate revision identity fails closed.
    assert_eq!(
        freeze_denominator(
            capture(
                vec![item_a.clone(), item_a.clone()],
                complete_observation(),
                0x50,
            ),
            strict_requirements(),
            test_blake,
        ),
        Err(ExactError::ExactReportInvalid)
    );
    // An empty authorized scope freezes to nothing advertised as complete.
    assert_eq!(
        freeze_denominator(
            capture(Vec::new(), complete_observation(), 0x50),
            strict_requirements(),
            test_blake,
        ),
        Err(ExactError::ExactScopeEmpty)
    );

    // A narrowed denominator (the top-k shape) never aliases the full proof.
    let narrowed = freeze_denominator(
        capture(vec![item_a], complete_observation(), 0x51),
        strict_requirements(),
        test_blake,
    )
    .expect("narrowed freeze");
    assert_ne!(narrowed.denominator_digest(), ordered.denominator_digest());
    assert_eq!(narrowed.contract().source_revision_ids.len(), 1);
}

#[test]
fn frozen_denominator_rejects_unproven_completeness() {
    let domain = ExactInputDomain::DecodedText;
    let (item_a, _) = denominator_item(11, 21, domain, b"alpha", "test-inventory-receipt/a");
    let (item_b, _) = denominator_item(12, 22, domain, b"beta", "test-inventory-receipt/b");

    // Unretained revisions cannot back a retained proof.
    let mut unstable = item_a.clone();
    unstable.stable_or_retained = false;
    assert_eq!(
        freeze_denominator(
            capture(vec![unstable, item_b.clone()], complete_observation(), 0x50),
            strict_requirements(),
            test_blake,
        ),
        Err(ExactError::ExactDenominatorIncomplete)
    );
    // An observation gap blocks a current proof at freeze time (T31).
    assert_eq!(
        freeze_denominator(
            capture(
                vec![item_a.clone(), item_b.clone()],
                InventoryCompleteness {
                    current_observation: false,
                    ..complete_observation()
                },
                0x50,
            ),
            strict_requirements(),
            test_blake,
        ),
        Err(ExactError::ExactObservationGap)
    );
    // Omitted or unknown inventory never freezes as a complete denominator.
    for completeness in [
        InventoryCompleteness {
            omitted_items: 1,
            ..complete_observation()
        },
        InventoryCompleteness {
            unknown_items: 1,
            ..complete_observation()
        },
        InventoryCompleteness {
            enumeration_complete: false,
            ..complete_observation()
        },
    ] {
        assert_eq!(
            freeze_denominator(
                capture(vec![item_a.clone(), item_b.clone()], completeness, 0x50),
                strict_requirements(),
                test_blake,
            ),
            Err(ExactError::ExactDenominatorIncomplete)
        );
    }
}

#[test]
fn predicate_profile_contracts() {
    validate_predicate_profile(&literal_profile(ExactInputDomain::DecodedText)).expect("valid");
    let unbounded = ExactPredicateProfile {
        max_pattern_bytes: 0,
        ..literal_profile(ExactInputDomain::DecodedText)
    };
    assert_eq!(
        validate_predicate_profile(&unbounded),
        Err(ExactError::ExactPredicateLimitExceeded)
    );
    let structural = ExactPredicateProfile {
        kind: ExactPredicateKind::StructuralPattern,
        input_domain: ExactInputDomain::StructuralIr,
        complexity: ComplexityClass::BoundedStructural,
        max_structural_depth: 4,
        ..literal_profile(ExactInputDomain::DecodedText)
    };
    validate_predicate_profile(&structural).expect("structural profile");
    let shallow = ExactPredicateProfile {
        kind: ExactPredicateKind::StructuralPattern,
        input_domain: ExactInputDomain::StructuralIr,
        complexity: ComplexityClass::BoundedStructural,
        max_structural_depth: 0,
        ..literal_profile(ExactInputDomain::DecodedText)
    };
    assert_eq!(
        validate_predicate_profile(&shallow),
        Err(ExactError::ExactEngineNotQualified)
    );

    // Predicate compilation rejects malformed requests before any inventory.
    let profile = literal_profile(ExactInputDomain::DecodedText);
    let empty = PredicateCompileRequest {
        kind: ExactPredicateKind::Literal,
        input_domain: ExactInputDomain::DecodedText,
        serialized_form: Vec::new(),
        normalization: NormalizationPolicy::None,
    };
    assert_eq!(
        compile_predicate(empty, profile.clone(), test_blake),
        Err(ExactError::ExactPredicateInvalid)
    );
    let oversized = PredicateCompileRequest {
        kind: ExactPredicateKind::Literal,
        input_domain: ExactInputDomain::DecodedText,
        serialized_form: vec![b'a'; 65],
        normalization: NormalizationPolicy::None,
    };
    assert_eq!(
        compile_predicate(oversized, profile.clone(), test_blake),
        Err(ExactError::ExactPredicateInvalid)
    );
    let mismatched = PredicateCompileRequest {
        kind: ExactPredicateKind::Regex,
        input_domain: ExactInputDomain::DecodedText,
        serialized_form: b"a".to_vec(),
        normalization: NormalizationPolicy::None,
    };
    assert_eq!(
        compile_predicate(mismatched, profile.clone(), test_blake),
        Err(ExactError::ExactPredicateInvalid)
    );

    // Equal canonical inputs produce equal predicate digests; any pattern
    // change produces a distinct proof identity.
    let first = compile_literal(profile.clone(), b"needle");
    let second = compile_literal(profile.clone(), b"needle");
    assert_eq!(first.predicate_digest(), second.predicate_digest());
    let changed = compile_literal(profile, b"thread");
    assert_ne!(first.predicate_digest(), changed.predicate_digest());
}
