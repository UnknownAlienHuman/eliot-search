//! Frozen-denominator exact predicate compilation and execution.
//!
//! The exact plane never accepts Qdrant, lexical top-k, semantic candidates,
//! or client file lists as a complete denominator. Inventory and readback are
//! captured by accepted ports; this package validates those finite inputs,
//! executes bounded predicates, accounts for every frozen item, and permits a
//! complete-negative conclusion only when every required condition is proven.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, BoundedCanonicalBytes, BoundedList, BoundedNonContentMetadata,
    BoundedSet, BufferSnapshotId, CatalogRevision, CoverageDenominatorKind,
    ExactCompletenessRequirements, ExactConclusion, ExactExecutionReport,
    ExactInputDomain, ExactItemFailure, ExactItemFailureKind, ExactMatch,
    ExactPredicate, ExactPredicateKind, ExactScanDenominator, ExactScanPlan,
    ExactScanPlanRef, MAX_LIST_ITEMS, MAX_RAW_BYTES, MAX_REASON_CODES,
    PlanFingerprint, PlanId, ProfileId, ReceiptRef, SearchReasonCodeV1,
    SourceRevisionId, SourceRevisionRef,
};

/// Closed exact-plane failure surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExactError {
    /// Predicate request and profile are inconsistent.
    ExactRequestInvalid,
    /// The requested predicate kind or domain is not implemented by a qualified engine.
    ExactPredicateUnsupported,
    /// Predicate syntax or canonical serialized form is invalid.
    ExactPredicateInvalid,
    /// Engine identity or qualification evidence is incomplete.
    ExactEngineNotQualified,
    /// Pattern, input, match, item, byte, step, or checkpoint limit was exceeded.
    ExactPredicateLimitExceeded,
    /// The explicitly authorized denominator is empty.
    ExactScopeEmpty,
    /// Inventory capture cannot support the requested completeness claim.
    ExactDenominatorIncomplete,
    /// Current inventory or a load-bearing fence differs from the frozen plan.
    ExactDenominatorDrift,
    /// A planned source revision is missing or no longer exactly readable.
    SourceRevisionUnavailable,
    /// Exact source bytes could not be read.
    SourceUnreadable,
    /// Input bytes cannot be interpreted in the declared domain.
    ExactEncodingUnsupported,
    /// Structural input/profile identity does not match the compiled predicate.
    ExactStructuralProfileMismatch,
    /// Observation continuity is incomplete.
    ExactObservationGap,
    /// Current authorization no longer permits the item or result.
    ExactAccessRevoked,
    /// A purge barrier covers the item or result.
    ExactPurged,
    /// Finite execution budget was exhausted.
    ExactBudgetExhausted,
    /// Explicit deadline observation expired.
    ExactTimeout,
    /// Explicit cancellation was observed.
    ExactCancelled,
    /// Durable checkpoint outcome requires authoritative readback.
    ExactCheckpointOutcomeUnknown,
    /// Report, checkpoint, item accounting, or receipt is contradictory.
    ExactReportInvalid,
    /// A semantic absence claim was requested from an exact syntactic predicate.
    SemanticAbsenceClaimForbidden,
    /// Shared bounded contract construction failed.
    ContractViolation,
}

impl ExactError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ExactRequestInvalid => "EXACT_REQUEST_INVALID",
            Self::ExactPredicateUnsupported => "EXACT_PREDICATE_UNSUPPORTED",
            Self::ExactPredicateInvalid => "EXACT_PREDICATE_INVALID",
            Self::ExactEngineNotQualified => "EXACT_ENGINE_NOT_QUALIFIED",
            Self::ExactPredicateLimitExceeded => "EXACT_PREDICATE_LIMIT_EXCEEDED",
            Self::ExactScopeEmpty => "EXACT_SCOPE_EMPTY",
            Self::ExactDenominatorIncomplete => "EXACT_DENOMINATOR_INCOMPLETE",
            Self::ExactDenominatorDrift => "EXACT_DENOMINATOR_DRIFT",
            Self::SourceRevisionUnavailable => "SOURCE_REVISION_UNAVAILABLE",
            Self::SourceUnreadable => "SOURCE_UNREADABLE",
            Self::ExactEncodingUnsupported => "EXACT_ENCODING_UNSUPPORTED",
            Self::ExactStructuralProfileMismatch => "EXACT_STRUCTURAL_PROFILE_MISMATCH",
            Self::ExactObservationGap => "EXACT_OBSERVATION_GAP",
            Self::ExactAccessRevoked => "EXACT_ACCESS_REVOKED",
            Self::ExactPurged => "EXACT_PURGED",
            Self::ExactBudgetExhausted => "EXACT_BUDGET_EXHAUSTED",
            Self::ExactTimeout => "EXACT_TIMEOUT",
            Self::ExactCancelled => "EXACT_CANCELLED",
            Self::ExactCheckpointOutcomeUnknown => "EXACT_CHECKPOINT_OUTCOME_UNKNOWN",
            Self::ExactReportInvalid => "EXACT_REPORT_INVALID",
            Self::SemanticAbsenceClaimForbidden => "SEMANTIC_ABSENCE_CLAIM_FORBIDDEN",
            Self::ContractViolation => "EXACT_CONTRACT_VIOLATION",
        }
    }
}

impl fmt::Display for ExactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ExactError {}

/// Proven worst-case execution class for a predicate profile.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ComplexityClass {
    /// Single bounded pass over the input.
    Linear,
    /// Bounded structural traversal with explicit depth and node ceilings.
    BoundedStructural,
}

/// Declared normalization policy. Exact bytes and decoded text remain distinct.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NormalizationPolicy {
    /// No normalization; compare exact bytes or Unicode scalar sequence.
    None,
    /// ASCII-only case-insensitive literal comparison.
    AsciiCaseInsensitive,
}

/// Qualification evidence and hard limits for one exact predicate profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactPredicateProfile {
    /// Stable profile identifier.
    pub profile_id: ProfileId,
    /// Closed predicate kind.
    pub kind: ExactPredicateKind,
    /// Required input domain.
    pub input_domain: ExactInputDomain,
    /// Exact engine/provider name and version.
    pub engine_and_version: ProfileId,
    /// Profile identifier describing the proven complexity ceiling.
    pub complexity_profile_id: ProfileId,
    /// Proven complexity class.
    pub complexity: ComplexityClass,
    /// Exact normalization semantics.
    pub normalization: NormalizationPolicy,
    /// Maximum canonical predicate bytes.
    pub max_pattern_bytes: usize,
    /// Maximum bytes read for one item.
    pub max_input_bytes: usize,
    /// Maximum matches emitted for one item.
    pub max_matches_per_item: usize,
    /// Maximum bounded engine steps for one item.
    pub max_steps_per_item: u64,
    /// Maximum structural depth; zero outside structural profiles.
    pub max_structural_depth: u16,
    /// Whether regex backreferences are admitted by qualification.
    pub allows_backreferences: bool,
    /// Whether regex lookaround is admitted by qualification.
    pub allows_lookaround: bool,
    /// Exact source/license/golden qualification receipt.
    pub qualification_receipt_ref: ReceiptRef,
    /// Digest of exact qualification evidence.
    pub qualification_digest: Blake3Digest32,
}

/// Validates a profile without authorizing any source scope.
pub fn validate_predicate_profile(
    profile: &ExactPredicateProfile,
) -> Result<(), ExactError> {
    if profile.max_pattern_bytes == 0
        || profile.max_pattern_bytes > MAX_RAW_BYTES
        || profile.max_input_bytes == 0
        || profile.max_input_bytes > MAX_RAW_BYTES
        || profile.max_matches_per_item == 0
        || profile.max_matches_per_item > MAX_LIST_ITEMS
        || profile.max_steps_per_item == 0
    {
        return Err(ExactError::ExactPredicateLimitExceeded);
    }

    let domain_valid = match profile.kind {
        ExactPredicateKind::Literal => matches!(
            profile.input_domain,
            ExactInputDomain::RawBytes | ExactInputDomain::DecodedText
        ),
        ExactPredicateKind::Regex | ExactPredicateKind::QualifiedSymbol => {
            profile.input_domain == ExactInputDomain::DecodedText
        }
        ExactPredicateKind::StructuralPattern => {
            profile.input_domain == ExactInputDomain::StructuralIr
        }
        ExactPredicateKind::RecordField => matches!(
            profile.input_domain,
            ExactInputDomain::DecodedText | ExactInputDomain::StructuralIr
        ),
    };
    if !domain_valid {
        return Err(ExactError::ExactRequestInvalid);
    }

    if profile.kind == ExactPredicateKind::Regex
        && (profile.complexity != ComplexityClass::Linear
            || profile.allows_backreferences
            || profile.allows_lookaround)
    {
        return Err(ExactError::ExactEngineNotQualified);
    }
    if profile.input_domain == ExactInputDomain::StructuralIr
        && (profile.complexity != ComplexityClass::BoundedStructural
            || profile.max_structural_depth == 0)
    {
        return Err(ExactError::ExactEngineNotQualified);
    }
    if profile.input_domain != ExactInputDomain::StructuralIr
        && profile.max_structural_depth != 0
    {
        return Err(ExactError::ExactRequestInvalid);
    }
    Ok(())
}

/// Canonical predicate compilation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateCompileRequest {
    /// Closed predicate kind.
    pub kind: ExactPredicateKind,
    /// Declared input domain.
    pub input_domain: ExactInputDomain,
    /// Producer-canonical serialized form.
    pub serialized_form: Vec<u8>,
    /// Exact normalization policy requested by the client contract.
    pub normalization: NormalizationPolicy,
}

/// Compiled exact predicate plus qualification and identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledExactPredicate {
    contract: ExactPredicate,
    profile: ExactPredicateProfile,
    predicate_digest: Blake3Digest32,
    literal: Option<Vec<u8>>,
}

impl CompiledExactPredicate {
    /// Shared wire contract.
    #[must_use]
    pub const fn contract(&self) -> &ExactPredicate {
        &self.contract
    }

    /// Qualified profile.
    #[must_use]
    pub const fn profile(&self) -> &ExactPredicateProfile {
        &self.profile
    }

    /// Digest of exact canonical predicate/profile inputs.
    #[must_use]
    pub const fn predicate_digest(&self) -> Blake3Digest32 {
        self.predicate_digest
    }
}

/// Compiles a bounded predicate using a caller-supplied BLAKE3-256 operation.
///
/// The hash callback keeps this package independent from one crypto crate while
/// preserving the exact digest domain and bytes.
pub fn compile_predicate(
    request: PredicateCompileRequest,
    profile: ExactPredicateProfile,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<CompiledExactPredicate, ExactError> {
    validate_predicate_profile(&profile)?;
    if request.kind != profile.kind
        || request.input_domain != profile.input_domain
        || request.normalization != profile.normalization
        || request.serialized_form.is_empty()
        || request.serialized_form.len() > profile.max_pattern_bytes
    {
        return Err(ExactError::ExactPredicateInvalid);
    }
    if request.normalization == NormalizationPolicy::AsciiCaseInsensitive
        && request.serialized_form.iter().any(|byte| !byte.is_ascii())
    {
        return Err(ExactError::ExactPredicateInvalid);
    }

    let canonical = BoundedCanonicalBytes::<MAX_RAW_BYTES>::from_validated(
        request.serialized_form.clone(),
    )
    .map_err(|_| ExactError::ContractViolation)?;
    let contract = ExactPredicate {
        kind: request.kind,
        engine_and_version: profile.engine_and_version.clone(),
        serialized_form: canonical,
        input_domain: request.input_domain,
        worst_case_complexity_class: profile.complexity_profile_id.clone(),
    };
    let digest_input = predicate_digest_input(&request, &profile)?;
    let predicate_digest = Blake3Digest32::from_bytes(blake3_256(&digest_input));
    let literal = (request.kind == ExactPredicateKind::Literal)
        .then_some(request.serialized_form);
    Ok(CompiledExactPredicate {
        contract,
        profile,
        predicate_digest,
        literal,
    })
}

fn predicate_digest_input(
    request: &PredicateCompileRequest,
    profile: &ExactPredicateProfile,
) -> Result<Vec<u8>, ExactError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/exact-predicate/v1")?;
    bytes.push(predicate_kind_tag(request.kind));
    bytes.push(input_domain_tag(request.input_domain));
    bytes.push(normalization_tag(request.normalization));
    append(&mut bytes, profile.profile_id.as_str().as_bytes())?;
    append(&mut bytes, profile.engine_and_version.as_str().as_bytes())?;
    append(
        &mut bytes,
        profile.complexity_profile_id.as_str().as_bytes(),
    )?;
    append(&mut bytes, profile.qualification_digest.as_bytes())?;
    append(&mut bytes, &request.serialized_form)?;
    Ok(bytes)
}

/// One exact byte range returned by a predicate engine.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MatchSpan {
    /// Inclusive start byte.
    pub byte_start: u64,
    /// Exclusive end byte.
    pub byte_end: u64,
}

impl MatchSpan {
    /// Validates a non-empty range against one input length.
    pub fn validate(self, input_len: usize) -> Result<(), ExactError> {
        let end = usize::try_from(self.byte_end)
            .map_err(|_| ExactError::ExactPredicateLimitExceeded)?;
        if self.byte_start >= self.byte_end || end > input_len {
            Err(ExactError::ExactReportInvalid)
        } else {
            Ok(())
        }
    }
}

/// Borrowed predicate-engine input.
#[derive(Clone, Copy, Debug)]
pub enum ExactInput<'a> {
    /// Exact source bytes.
    RawBytes(&'a [u8]),
    /// Strictly decoded UTF-8 text; spans remain byte-based.
    DecodedText(&'a str),
    /// Qualified structural intermediate representation bytes.
    StructuralIr(&'a [u8]),
}

impl ExactInput<'_> {
    fn bytes(self) -> &[u8] {
        match self {
            Self::RawBytes(bytes) | Self::StructuralIr(bytes) => bytes,
            Self::DecodedText(text) => text.as_bytes(),
        }
    }
}

/// Limits supplied to a qualified non-literal engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PredicateExecutionLimits {
    /// Maximum input bytes.
    pub max_input_bytes: usize,
    /// Maximum emitted matches.
    pub max_matches: usize,
    /// Maximum bounded execution steps.
    pub max_steps: u64,
    /// Maximum structural depth.
    pub max_structural_depth: u16,
}

/// Qualified non-literal predicate engine boundary.
///
/// Implementations must be pinned and separately qualified. Raw vendor types
/// and engine state do not cross this interface.
pub trait QualifiedPredicateEngine {
    /// Exact engine/version identity.
    fn engine_and_version(&self) -> &ProfileId;

    /// Executes one already compiled predicate in its declared domain.
    fn execute(
        &self,
        predicate: &ExactPredicate,
        input: ExactInput<'_>,
        limits: PredicateExecutionLimits,
    ) -> Result<Vec<MatchSpan>, ExactError>;
}

/// Executes the compiled predicate over one finite input.
pub fn execute_predicate(
    predicate: &CompiledExactPredicate,
    input: ExactInput<'_>,
    engine: Option<&dyn QualifiedPredicateEngine>,
) -> Result<BoundedList<MatchSpan, MAX_LIST_ITEMS>, ExactError> {
    let input_domain = match input {
        ExactInput::RawBytes(_) => ExactInputDomain::RawBytes,
        ExactInput::DecodedText(_) => ExactInputDomain::DecodedText,
        ExactInput::StructuralIr(_) => ExactInputDomain::StructuralIr,
    };
    if input_domain != predicate.contract.input_domain {
        return Err(ExactError::ExactEncodingUnsupported);
    }
    let bytes = input.bytes();
    if bytes.len() > predicate.profile.max_input_bytes {
        return Err(ExactError::ExactPredicateLimitExceeded);
    }

    let mut spans = if let Some(literal) = &predicate.literal {
        find_literal(
            bytes,
            literal,
            predicate.profile.normalization,
            predicate.profile.max_matches_per_item,
            predicate.profile.max_steps_per_item,
        )?
    } else {
        let engine = engine.ok_or(ExactError::ExactPredicateUnsupported)?;
        if engine.engine_and_version() != &predicate.profile.engine_and_version {
            return Err(ExactError::ExactEngineNotQualified);
        }
        engine.execute(
            &predicate.contract,
            input,
            PredicateExecutionLimits {
                max_input_bytes: predicate.profile.max_input_bytes,
                max_matches: predicate.profile.max_matches_per_item,
                max_steps: predicate.profile.max_steps_per_item,
                max_structural_depth: predicate.profile.max_structural_depth,
            },
        )?
    };

    if spans.len() > predicate.profile.max_matches_per_item || spans.len() > MAX_LIST_ITEMS {
        return Err(ExactError::ExactPredicateLimitExceeded);
    }
    spans.sort_unstable();
    let mut previous = None;
    for span in &spans {
        span.validate(bytes.len())?;
        if previous == Some(*span) {
            return Err(ExactError::ExactReportInvalid);
        }
        previous = Some(*span);
    }
    BoundedList::new(spans).map_err(|_| ExactError::ExactPredicateLimitExceeded)
}

fn find_literal(
    input: &[u8],
    needle: &[u8],
    normalization: NormalizationPolicy,
    max_matches: usize,
    max_steps: u64,
) -> Result<Vec<MatchSpan>, ExactError> {
    if needle.is_empty() {
        return Err(ExactError::ExactPredicateInvalid);
    }
    if normalization == NormalizationPolicy::AsciiCaseInsensitive
        && (needle.iter().any(|byte| !byte.is_ascii())
            || input.iter().any(|byte| !byte.is_ascii()))
    {
        return Err(ExactError::ExactEncodingUnsupported);
    }
    if needle.len() > input.len() {
        return Ok(Vec::new());
    }

    let mut output = Vec::new();
    let mut steps = 0_u64;
    for start in 0..=input.len() - needle.len() {
        steps = steps
            .checked_add(1)
            .ok_or(ExactError::ExactPredicateLimitExceeded)?;
        if steps > max_steps {
            return Err(ExactError::ExactPredicateLimitExceeded);
        }
        let window = &input[start..start + needle.len()];
        let matched = match normalization {
            NormalizationPolicy::None => window == needle,
            NormalizationPolicy::AsciiCaseInsensitive => window
                .iter()
                .zip(needle)
                .all(|(left, right)| left.eq_ignore_ascii_case(right)),
        };
        if matched {
            if output.len() >= max_matches {
                return Err(ExactError::ExactPredicateLimitExceeded);
            }
            output.push(MatchSpan {
                byte_start: u64::try_from(start)
                    .map_err(|_| ExactError::ExactPredicateLimitExceeded)?,
                byte_end: u64::try_from(start + needle.len())
                    .map_err(|_| ExactError::ExactPredicateLimitExceeded)?,
            });
        }
    }
    Ok(output)
}

/// One exact source revision in an authoritative inventory capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenominatorItem {
    /// Exact immutable source revision.
    pub revision: SourceRevisionRef,
    /// Input domain required for this item.
    pub input_domain: ExactInputDomain,
    /// Whether exact bytes are stable or retained for the plan lifetime.
    pub stable_or_retained: bool,
    /// Source-backed inventory/admission receipt.
    pub inventory_receipt_ref: ReceiptRef,
}

/// Completeness state of one authoritative inventory capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryCompleteness {
    /// Inventory enumeration completed over its declared scope.
    pub enumeration_complete: bool,
    /// Number of explicitly omitted items.
    pub omitted_items: u64,
    /// Number of items whose identity or availability is unknown.
    pub unknown_items: u64,
    /// Observation continuity is current for the declared scope.
    pub current_observation: bool,
}

impl InventoryCompleteness {
    /// Returns whether the capture can represent a complete authoritative scope.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.enumeration_complete
            && self.omitted_items == 0
            && self.unknown_items == 0
            && self.current_observation
    }
}

/// Already captured, authorized source inventory.
///
/// The type has no field for ranked/indexed candidates, preventing accidental
/// substitution of top-k results for the exact denominator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryCapture {
    /// Exact authoritative inventory revision.
    pub inventory_revision: CatalogRevision,
    /// Finite inventory items.
    pub items: Vec<DenominatorItem>,
    /// Inventory manifest digest.
    pub inventory_digest: Blake3Digest32,
    /// Explicit source/workspace view digest.
    pub source_view_digest: Blake3Digest32,
    /// Exact source-owner generation set digest.
    pub owner_generation_digest: Blake3Digest32,
    /// Exact grant/access/live-deny/purge/shadow fence digest.
    pub security_fence_digest: Blake3Digest32,
    /// Exact authenticated overlay snapshot-set digest.
    pub overlay_digest: Blake3Digest32,
    /// Truthful inventory completeness.
    pub completeness: InventoryCompleteness,
}

/// Frozen exact denominator and every load-bearing fence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenDenominator {
    contract: ExactScanDenominator,
    items: BoundedList<DenominatorItem, MAX_LIST_ITEMS>,
    denominator_digest: Blake3Digest32,
    inventory_digest: Blake3Digest32,
    source_view_digest: Blake3Digest32,
    owner_generation_digest: Blake3Digest32,
    security_fence_digest: Blake3Digest32,
    overlay_digest: Blake3Digest32,
    completeness: InventoryCompleteness,
}

impl FrozenDenominator {
    /// Shared contract denominator.
    #[must_use]
    pub const fn contract(&self) -> &ExactScanDenominator {
        &self.contract
    }

    /// Ordered frozen items.
    #[must_use]
    pub const fn items(&self) -> &BoundedList<DenominatorItem, MAX_LIST_ITEMS> {
        &self.items
    }

    /// Digest covering the exact item list and every load-bearing fence.
    #[must_use]
    pub const fn denominator_digest(&self) -> Blake3Digest32 {
        self.denominator_digest
    }

    /// Truthful completeness captured at freeze time.
    #[must_use]
    pub const fn completeness(&self) -> InventoryCompleteness {
        self.completeness
    }
}

/// Freezes one authoritative denominator in deterministic revision order.
pub fn freeze_denominator(
    mut capture: InventoryCapture,
    requirements: ExactCompletenessRequirements,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<FrozenDenominator, ExactError> {
    if capture.items.is_empty() {
        return Err(ExactError::ExactScopeEmpty);
    }
    if capture.items.len() > MAX_LIST_ITEMS {
        return Err(ExactError::ExactPredicateLimitExceeded);
    }
    capture.items.sort_by_key(|item| item.revision.revision_id);
    let mut revision_ids = Vec::with_capacity(capture.items.len());
    let mut previous = None;
    for item in &capture.items {
        if previous == Some(item.revision.revision_id) {
            return Err(ExactError::ExactReportInvalid);
        }
        previous = Some(item.revision.revision_id);
        if requirements.require_stable_or_retained_revision && !item.stable_or_retained {
            return Err(ExactError::ExactDenominatorIncomplete);
        }
        revision_ids.push(item.revision.revision_id);
    }
    if requirements.require_current_observation && !capture.completeness.current_observation {
        return Err(ExactError::ExactObservationGap);
    }
    if requirements.require_every_denominator_item && !capture.completeness.is_complete() {
        return Err(ExactError::ExactDenominatorIncomplete);
    }

    let contract = ExactScanDenominator {
        source_revision_ids: BoundedList::new(revision_ids)
            .map_err(|_| ExactError::ContractViolation)?,
        inventory_revision: capture.inventory_revision,
    };
    let digest_input = denominator_digest_input(&capture)?;
    Ok(FrozenDenominator {
        contract,
        items: BoundedList::new(capture.items).map_err(|_| ExactError::ContractViolation)?,
        denominator_digest: Blake3Digest32::from_bytes(blake3_256(&digest_input)),
        inventory_digest: capture.inventory_digest,
        source_view_digest: capture.source_view_digest,
        owner_generation_digest: capture.owner_generation_digest,
        security_fence_digest: capture.security_fence_digest,
        overlay_digest: capture.overlay_digest,
        completeness: capture.completeness,
    })
}

fn denominator_digest_input(capture: &InventoryCapture) -> Result<Vec<u8>, ExactError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/exact-denominator/v1")?;
    bytes.extend_from_slice(&capture.inventory_revision.get().to_be_bytes());
    append(&mut bytes, capture.inventory_digest.as_bytes())?;
    append(&mut bytes, capture.source_view_digest.as_bytes())?;
    append(&mut bytes, capture.owner_generation_digest.as_bytes())?;
    append(&mut bytes, capture.security_fence_digest.as_bytes())?;
    append(&mut bytes, capture.overlay_digest.as_bytes())?;
    bytes.push(u8::from(capture.completeness.enumeration_complete));
    bytes.extend_from_slice(&capture.completeness.omitted_items.to_be_bytes());
    bytes.extend_from_slice(&capture.completeness.unknown_items.to_be_bytes());
    bytes.push(u8::from(capture.completeness.current_observation));
    for item in &capture.items {
        bytes.extend_from_slice(item.revision.source_namespace_id.as_bytes());
        bytes.extend_from_slice(item.revision.source_id.as_bytes());
        bytes.extend_from_slice(item.revision.revision_id.as_bytes());
        bytes.extend_from_slice(item.revision.content_digest.as_bytes());
        bytes.extend_from_slice(&item.revision.byte_length.to_be_bytes());
        bytes.push(input_domain_tag(item.input_domain));
        bytes.push(u8::from(item.stable_or_retained));
        append(&mut bytes, item.inventory_receipt_ref.as_str().as_bytes())?;
    }
    Ok(bytes)
}

/// Non-semantic identities supplied when compiling a shared exact scan plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactPlanIdentity {
    /// Plan identifier.
    pub plan_id: PlanId,
    /// Digest of exact authorized inclusion policy.
    pub inclusion_policy_digest: Blake3Digest32,
    /// Explicit authenticated unsaved snapshots.
    pub unsaved_buffer_snapshot_ids: Vec<BufferSnapshotId>,
    /// Precomputed canonical plan fingerprint.
    pub plan_fingerprint: PlanFingerprint,
}

/// Executable exact plan retaining server-owned denominator detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledExactScan {
    contract: ExactScanPlan,
    predicate: CompiledExactPredicate,
    denominator: FrozenDenominator,
}

impl CompiledExactScan {
    /// Shared exact scan plan.
    #[must_use]
    pub const fn contract(&self) -> &ExactScanPlan {
        &self.contract
    }

    /// Compiled predicate.
    #[must_use]
    pub const fn predicate(&self) -> &CompiledExactPredicate {
        &self.predicate
    }

    /// Frozen denominator.
    #[must_use]
    pub const fn denominator(&self) -> &FrozenDenominator {
        &self.denominator
    }

    /// Stable shared plan reference.
    #[must_use]
    pub const fn plan_ref(&self) -> ExactScanPlanRef {
        ExactScanPlanRef {
            plan_id: self.contract.plan_id,
            plan_fingerprint: self.contract.plan_fingerprint,
        }
    }
}

/// Compiles a shared exact scan plan from an already frozen denominator.
pub fn compile_exact_scan(
    identity: ExactPlanIdentity,
    predicate: CompiledExactPredicate,
    denominator: FrozenDenominator,
    requirements: ExactCompletenessRequirements,
) -> Result<CompiledExactScan, ExactError> {
    if denominator
        .items
        .iter()
        .any(|item| item.input_domain != predicate.contract.input_domain)
    {
        return Err(ExactError::ExactEncodingUnsupported);
    }
    if !requirements.include_authenticated_unsaved_buffers
        && !identity.unsaved_buffer_snapshot_ids.is_empty()
    {
        return Err(ExactError::ExactRequestInvalid);
    }
    if identity.unsaved_buffer_snapshot_ids.len() > MAX_LIST_ITEMS {
        return Err(ExactError::ExactPredicateLimitExceeded);
    }
    if requirements.require_every_denominator_item
        && !denominator.completeness.is_complete()
    {
        return Err(ExactError::ExactDenominatorIncomplete);
    }
    let contract = ExactScanPlan {
        plan_id: identity.plan_id,
        predicate: predicate.contract.clone(),
        denominator: denominator.contract.clone(),
        inclusion_policy_digest: identity.inclusion_policy_digest,
        unsaved_buffer_snapshot_ids: BoundedList::new(identity.unsaved_buffer_snapshot_ids)
            .map_err(|_| ExactError::ContractViolation)?,
        completeness_requirements: requirements,
        plan_fingerprint: identity.plan_fingerprint,
    };
    Ok(CompiledExactScan {
        contract,
        predicate,
        denominator,
    })
}

/// Current state required before reading any denominator item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanLiveState {
    /// Current inventory revision.
    pub inventory_revision: CatalogRevision,
    /// Current inventory manifest digest.
    pub inventory_digest: Blake3Digest32,
    /// Current source/workspace view digest.
    pub source_view_digest: Blake3Digest32,
    /// Current source-owner generation set digest.
    pub owner_generation_digest: Blake3Digest32,
    /// Current grant/access/live-deny/purge/shadow digest.
    pub security_fence_digest: Blake3Digest32,
    /// Current authenticated overlay-set digest.
    pub overlay_digest: Blake3Digest32,
    /// Current access permits execution.
    pub access_permitted: bool,
    /// No purge barrier covers the scope.
    pub purge_clear: bool,
    /// Observation continuity is sufficient.
    pub current_observation: bool,
    /// Predicate profile remains accepted.
    pub predicate_profile_current: bool,
}

/// Exact permit tying execution to one validated frozen state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanExecutionPermit {
    /// Shared plan reference.
    pub plan_ref: ExactScanPlanRef,
    /// Exact frozen denominator digest.
    pub denominator_digest: Blake3Digest32,
    /// Exact predicate digest.
    pub predicate_digest: Blake3Digest32,
    /// Security fence checked immediately before execution.
    pub security_fence_digest: Blake3Digest32,
}

/// Revalidates a plan before any read.
pub fn validate_plan_before_execution(
    plan: &CompiledExactScan,
    current: PlanLiveState,
) -> Result<PlanExecutionPermit, ExactError> {
    if !current.access_permitted {
        return Err(ExactError::ExactAccessRevoked);
    }
    if !current.purge_clear {
        return Err(ExactError::ExactPurged);
    }
    if !current.predicate_profile_current {
        return Err(ExactError::ExactEngineNotQualified);
    }
    if plan.contract.completeness_requirements.require_current_observation
        && !current.current_observation
    {
        return Err(ExactError::ExactObservationGap);
    }
    if current.inventory_revision != plan.denominator.contract.inventory_revision
        || current.inventory_digest != plan.denominator.inventory_digest
        || current.source_view_digest != plan.denominator.source_view_digest
        || current.owner_generation_digest != plan.denominator.owner_generation_digest
        || current.security_fence_digest != plan.denominator.security_fence_digest
        || current.overlay_digest != plan.denominator.overlay_digest
    {
        return Err(ExactError::ExactDenominatorDrift);
    }
    Ok(PlanExecutionPermit {
        plan_ref: plan.plan_ref(),
        denominator_digest: plan.denominator.denominator_digest,
        predicate_digest: plan.predicate.predicate_digest,
        security_fence_digest: current.security_fence_digest,
    })
}

/// Exact readback captured for one frozen item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactItemReadback {
    /// Exact revision read.
    pub revision: SourceRevisionRef,
    /// Exact bytes or qualified structural IR bytes.
    pub bytes: Vec<u8>,
    /// Digest computed over exact read bytes.
    pub observed_content_digest: Blake3Digest32,
    /// Exact readback receipt.
    pub readback_receipt_ref: ReceiptRef,
    /// Current access permits this item.
    pub access_permitted: bool,
    /// No purge barrier covers this item.
    pub purge_clear: bool,
    /// Source/view/owner observation remains current.
    pub current: bool,
}

/// Finite scan budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactExecutionBudget {
    /// Maximum denominator items attempted.
    pub max_items: usize,
    /// Maximum exact bytes read.
    pub max_bytes: u64,
    /// Maximum total projected matches.
    pub max_matches: usize,
}

impl ExactExecutionBudget {
    /// Validates non-zero finite limits.
    pub fn validate(self) -> Result<Self, ExactError> {
        if self.max_items == 0
            || self.max_items > MAX_LIST_ITEMS
            || self.max_bytes == 0
            || self.max_matches == 0
            || self.max_matches > MAX_LIST_ITEMS
        {
            Err(ExactError::ExactBudgetExhausted)
        } else {
            Ok(self)
        }
    }
}

/// Explicit cancellation/deadline observation boundary.
pub trait ExactExecutionControl {
    /// Whether cancellation is currently requested.
    fn is_cancelled(&self) -> bool;
    /// Whether the finite deadline is currently expired.
    fn deadline_expired(&self) -> bool;
}

/// Control implementation that never cancels or times out.
#[derive(Clone, Copy, Debug, Default)]
pub struct UninterruptedExecution;

impl ExactExecutionControl for UninterruptedExecution {
    fn is_cancelled(&self) -> bool {
        false
    }

    fn deadline_expired(&self) -> bool {
        false
    }
}

/// Converts exact byte spans to source-backed contract matches.
pub trait ExactMatchProjector {
    /// Projects one validated span using the exact item and readback receipt.
    fn project(
        &self,
        item: &DenominatorItem,
        readback: &ExactItemReadback,
        predicate: &CompiledExactPredicate,
        span: MatchSpan,
    ) -> Result<ExactMatch, ExactError>;
}

/// Complete outcome for one frozen denominator item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactItemExecution {
    /// Frozen source revision identity.
    pub source_revision_id: SourceRevisionId,
    /// Bytes successfully scanned before terminal outcome.
    pub scanned_bytes: u64,
    /// Source-backed matches.
    pub matches: BoundedList<ExactMatch, MAX_LIST_ITEMS>,
    /// Exactly one failure when the item did not complete.
    pub failure: Option<ExactItemFailure>,
}

impl ExactItemExecution {
    /// Returns whether the item completed predicate execution.
    #[must_use]
    pub fn completed(&self) -> bool {
        self.failure.is_none()
    }
}

/// Executes one exact item from captured readback data.
pub fn execute_item(
    plan: &CompiledExactScan,
    permit: &PlanExecutionPermit,
    item: &DenominatorItem,
    readback: &ExactItemReadback,
    engine: Option<&dyn QualifiedPredicateEngine>,
    projector: &dyn ExactMatchProjector,
) -> Result<ExactItemExecution, ExactError> {
    validate_permit(plan, permit)?;
    if readback.revision.revision_id != item.revision.revision_id
        || readback.revision != item.revision
    {
        return Ok(item_failure(
            item.revision.revision_id,
            ExactItemFailureKind::RevisionUnavailable,
            SearchReasonCodeV1::SourceRevisionUnavailable,
        )?);
    }
    if !readback.access_permitted {
        return Ok(item_failure(
            item.revision.revision_id,
            ExactItemFailureKind::ScopeChanged,
            SearchReasonCodeV1::AccessRevoked,
        )?);
    }
    if !readback.purge_clear {
        return Ok(item_failure(
            item.revision.revision_id,
            ExactItemFailureKind::ScopeChanged,
            SearchReasonCodeV1::Purged,
        )?);
    }
    if !readback.current {
        return Ok(item_failure(
            item.revision.revision_id,
            ExactItemFailureKind::ScopeChanged,
            SearchReasonCodeV1::Stale,
        )?);
    }
    if readback.observed_content_digest != item.revision.content_digest
        || u64::try_from(readback.bytes.len()).ok() != Some(item.revision.byte_length)
    {
        return Ok(item_failure(
            item.revision.revision_id,
            ExactItemFailureKind::RevisionUnavailable,
            SearchReasonCodeV1::ScopeChangedOrRevisionUnavailable,
        )?);
    }

    let input = match item.input_domain {
        ExactInputDomain::RawBytes => ExactInput::RawBytes(&readback.bytes),
        ExactInputDomain::DecodedText => match core::str::from_utf8(&readback.bytes) {
            Ok(text) => ExactInput::DecodedText(text),
            Err(_) => {
                return Ok(item_failure(
                    item.revision.revision_id,
                    ExactItemFailureKind::UnsupportedEncoding,
                    SearchReasonCodeV1::Unreadable,
                )?);
            }
        },
        ExactInputDomain::StructuralIr => ExactInput::StructuralIr(&readback.bytes),
    };

    let spans = match execute_predicate(&plan.predicate, input, engine) {
        Ok(spans) => spans,
        Err(ExactError::ExactEncodingUnsupported) => {
            return Ok(item_failure(
                item.revision.revision_id,
                ExactItemFailureKind::UnsupportedEncoding,
                SearchReasonCodeV1::Unreadable,
            )?);
        }
        Err(ExactError::ExactPredicateLimitExceeded) => {
            return Ok(item_failure(
                item.revision.revision_id,
                ExactItemFailureKind::PredicateError,
                SearchReasonCodeV1::ResourceExhausted,
            )?);
        }
        Err(_) => {
            return Ok(item_failure(
                item.revision.revision_id,
                ExactItemFailureKind::PredicateError,
                SearchReasonCodeV1::IncompleteCoverage,
            )?);
        }
    };

    let mut matches = Vec::with_capacity(spans.len());
    for span in spans {
        let exact_match = projector.project(item, readback, &plan.predicate, span)?;
        exact_match
            .validate()
            .map_err(|_| ExactError::ExactReportInvalid)?;
        matches.push(exact_match);
    }
    Ok(ExactItemExecution {
        source_revision_id: item.revision.revision_id,
        scanned_bytes: item.revision.byte_length,
        matches: BoundedList::new(matches).map_err(|_| ExactError::ExactBudgetExhausted)?,
        failure: None,
    })
}

/// Executes every frozen denominator item in canonical order over captured readbacks.
pub fn execute_exact_scan(
    plan: &CompiledExactScan,
    permit: &PlanExecutionPermit,
    readbacks: Vec<ExactItemReadback>,
    budget: ExactExecutionBudget,
    control: &dyn ExactExecutionControl,
    engine: Option<&dyn QualifiedPredicateEngine>,
    projector: &dyn ExactMatchProjector,
    receipt_ref: ReceiptRef,
) -> Result<ExactExecutionReport, ExactError> {
    validate_permit(plan, permit)?;
    let budget = budget.validate()?;
    let mut by_revision = BTreeMap::new();
    for readback in readbacks {
        if by_revision
            .insert(readback.revision.revision_id, readback)
            .is_some()
        {
            return Err(ExactError::ExactReportInvalid);
        }
    }

    let mut executions = Vec::new();
    let mut scanned_bytes = 0_u64;
    let mut matched_count = 0_usize;
    let mut cancelled = false;
    let mut timed_out = false;

    for item in plan.denominator.items.iter() {
        if control.is_cancelled() {
            cancelled = true;
            executions.push(item_failure(
                item.revision.revision_id,
                ExactItemFailureKind::Cancelled,
                SearchReasonCodeV1::Cancelled,
            )?);
            continue;
        }
        if control.deadline_expired() {
            timed_out = true;
            executions.push(item_failure(
                item.revision.revision_id,
                ExactItemFailureKind::Timeout,
                SearchReasonCodeV1::ResourceExhausted,
            )?);
            continue;
        }
        if executions.len() >= budget.max_items {
            timed_out = true;
            executions.push(item_failure(
                item.revision.revision_id,
                ExactItemFailureKind::Timeout,
                SearchReasonCodeV1::ResourceExhausted,
            )?);
            continue;
        }
        let Some(readback) = by_revision.remove(&item.revision.revision_id) else {
            executions.push(item_failure(
                item.revision.revision_id,
                ExactItemFailureKind::RevisionUnavailable,
                SearchReasonCodeV1::SourceRevisionUnavailable,
            )?);
            continue;
        };
        let proposed_bytes = scanned_bytes
            .checked_add(item.revision.byte_length)
            .ok_or(ExactError::ExactBudgetExhausted)?;
        if proposed_bytes > budget.max_bytes {
            timed_out = true;
            executions.push(item_failure(
                item.revision.revision_id,
                ExactItemFailureKind::Timeout,
                SearchReasonCodeV1::ResourceExhausted,
            )?);
            continue;
        }
        let execution = execute_item(
            plan,
            permit,
            item,
            &readback,
            engine,
            projector,
        )?;
        scanned_bytes = scanned_bytes
            .checked_add(execution.scanned_bytes)
            .ok_or(ExactError::ExactBudgetExhausted)?;
        matched_count = matched_count
            .checked_add(execution.matches.len())
            .ok_or(ExactError::ExactBudgetExhausted)?;
        if matched_count > budget.max_matches {
            return Err(ExactError::ExactBudgetExhausted);
        }
        executions.push(execution);
    }
    if !by_revision.is_empty() {
        return Err(ExactError::ExactReportInvalid);
    }
    assemble_execution_report(
        plan,
        executions,
        scanned_bytes,
        timed_out,
        cancelled,
        receipt_ref,
    )
}

/// Assembles and validates exact item accounting into the shared report shape.
pub fn assemble_execution_report(
    plan: &CompiledExactScan,
    executions: Vec<ExactItemExecution>,
    scanned_bytes: u64,
    timed_out: bool,
    cancelled: bool,
    receipt_ref: ReceiptRef,
) -> Result<ExactExecutionReport, ExactError> {
    if executions.len() != plan.denominator.items.len() {
        return Err(ExactError::ExactReportInvalid);
    }
    let denominator_ids = plan
        .denominator
        .items
        .iter()
        .map(|item| item.revision.revision_id)
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut matches = Vec::new();
    let mut unreadable = Vec::new();
    let mut changed = Vec::new();
    let mut scope_drifted = false;
    let mut completed_items = 0_u64;
    let mut accounted_bytes = 0_u64;

    for execution in executions {
        if !denominator_ids.contains(&execution.source_revision_id)
            || !seen.insert(execution.source_revision_id)
            || (execution.failure.is_some() && !execution.matches.is_empty())
        {
            return Err(ExactError::ExactReportInvalid);
        }
        accounted_bytes = accounted_bytes
            .checked_add(execution.scanned_bytes)
            .ok_or(ExactError::ExactReportInvalid)?;
        if execution.completed() {
            completed_items = completed_items
                .checked_add(1)
                .ok_or(ExactError::ExactReportInvalid)?;
            matches.extend(execution.matches.into_vec());
        } else if let Some(failure) = execution.failure {
            match failure.failure_kind {
                ExactItemFailureKind::RevisionUnavailable
                | ExactItemFailureKind::ScopeChanged => {
                    scope_drifted |= failure.failure_kind == ExactItemFailureKind::ScopeChanged;
                    changed.push(failure);
                }
                ExactItemFailureKind::Unreadable
                | ExactItemFailureKind::Timeout
                | ExactItemFailureKind::Cancelled
                | ExactItemFailureKind::UnsupportedEncoding
                | ExactItemFailureKind::PredicateError => unreadable.push(failure),
            }
        }
    }
    if seen != denominator_ids || accounted_bytes != scanned_bytes {
        return Err(ExactError::ExactReportInvalid);
    }
    let complete = plan.denominator.completeness.is_complete()
        && completed_items == u64::try_from(plan.denominator.items.len()).unwrap_or(u64::MAX)
        && unreadable.is_empty()
        && changed.is_empty()
        && !timed_out
        && !cancelled
        && !scope_drifted;
    let coverage = if complete {
        CoverageDenominatorKind::CompleteScope
    } else if plan.denominator.completeness.unknown_items > 0 {
        CoverageDenominatorKind::Unknown
    } else {
        CoverageDenominatorKind::CandidateScope
    };
    let conclusion = if matches.is_empty() {
        if complete {
            ExactConclusion::NoMatchInCompleteScope
        } else {
            ExactConclusion::Incomplete
        }
    } else {
        ExactConclusion::MatchesFound
    };
    let report = ExactExecutionReport {
        plan_ref: plan.plan_ref(),
        matched_items: BoundedList::new(matches)
            .map_err(|_| ExactError::ExactBudgetExhausted)?,
        scanned_items: completed_items,
        scanned_bytes,
        unreadable_items: BoundedList::new(unreadable)
            .map_err(|_| ExactError::ExactBudgetExhausted)?,
        changed_or_unavailable_items: BoundedList::new(changed)
            .map_err(|_| ExactError::ExactBudgetExhausted)?,
        timed_out,
        cancelled,
        scope_drifted,
        coverage,
        conclusion,
        receipt_ref,
    };
    report
        .validate()
        .map_err(|_| ExactError::ExactReportInvalid)?;
    Ok(report)
}

fn item_failure(
    source_revision_id: SourceRevisionId,
    failure_kind: ExactItemFailureKind,
    reason: SearchReasonCodeV1,
) -> Result<ExactItemExecution, ExactError> {
    Ok(ExactItemExecution {
        source_revision_id,
        scanned_bytes: 0,
        matches: BoundedList::empty(),
        failure: Some(ExactItemFailure {
            source_revision_id,
            failure_kind,
            reason_codes: BoundedSet::<SearchReasonCodeV1, MAX_REASON_CODES>::from_items([reason])
                .map_err(|_| ExactError::ContractViolation)?,
            bounded_metadata: BoundedNonContentMetadata::empty(),
        }),
    })
}

fn validate_permit(
    plan: &CompiledExactScan,
    permit: &PlanExecutionPermit,
) -> Result<(), ExactError> {
    if permit.plan_ref != plan.plan_ref()
        || permit.denominator_digest != plan.denominator.denominator_digest
        || permit.predicate_digest != plan.predicate.predicate_digest
        || permit.security_fence_digest != plan.denominator.security_fence_digest
    {
        Err(ExactError::ExactDenominatorDrift)
    } else {
        Ok(())
    }
}

/// Durable non-content checkpoint for a governed verification job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactCheckpoint {
    /// Exact plan reference.
    pub plan_ref: ExactScanPlanRef,
    /// Frozen denominator digest.
    pub denominator_digest: Blake3Digest32,
    /// Compiled predicate digest.
    pub predicate_digest: Blake3Digest32,
    /// Completed exact item identities.
    pub completed_items: BoundedList<SourceRevisionId, MAX_LIST_ITEMS>,
    /// Non-content result/failure receipt references.
    pub result_receipt_refs: BoundedList<ReceiptRef, MAX_LIST_ITEMS>,
    /// Digest of exact checkpoint state.
    pub checkpoint_digest: Blake3Digest32,
}

/// Builds a deterministic checkpoint after exact item accounting.
pub fn checkpoint_execution(
    plan: &CompiledExactScan,
    completed_items: Vec<SourceRevisionId>,
    result_receipt_refs: Vec<ReceiptRef>,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<ExactCheckpoint, ExactError> {
    if completed_items.len() != result_receipt_refs.len()
        || completed_items.len() > MAX_LIST_ITEMS
    {
        return Err(ExactError::ExactReportInvalid);
    }
    let denominator = plan
        .denominator
        .items
        .iter()
        .map(|item| item.revision.revision_id)
        .collect::<BTreeSet<_>>();
    let completed = completed_items.iter().copied().collect::<BTreeSet<_>>();
    if completed.len() != completed_items.len() || !completed.is_subset(&denominator) {
        return Err(ExactError::ExactReportInvalid);
    }
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/exact-checkpoint/v1")?;
    bytes.extend_from_slice(plan.contract.plan_id.as_bytes());
    bytes.extend_from_slice(plan.contract.plan_fingerprint.as_bytes());
    bytes.extend_from_slice(plan.denominator.denominator_digest.as_bytes());
    bytes.extend_from_slice(plan.predicate.predicate_digest.as_bytes());
    for (item, receipt) in completed_items.iter().zip(&result_receipt_refs) {
        bytes.extend_from_slice(item.as_bytes());
        append(&mut bytes, receipt.as_str().as_bytes())?;
    }
    Ok(ExactCheckpoint {
        plan_ref: plan.plan_ref(),
        denominator_digest: plan.denominator.denominator_digest,
        predicate_digest: plan.predicate.predicate_digest,
        completed_items: BoundedList::new(completed_items)
            .map_err(|_| ExactError::ContractViolation)?,
        result_receipt_refs: BoundedList::new(result_receipt_refs)
            .map_err(|_| ExactError::ContractViolation)?,
        checkpoint_digest: Blake3Digest32::from_bytes(blake3_256(&bytes)),
    })
}

/// Verifies a checkpoint and returns unfinished items in frozen order.
pub fn resume_execution(
    plan: &CompiledExactScan,
    checkpoint: &ExactCheckpoint,
    expected_checkpoint_digest: Blake3Digest32,
) -> Result<BoundedList<SourceRevisionId, MAX_LIST_ITEMS>, ExactError> {
    if checkpoint.plan_ref != plan.plan_ref()
        || checkpoint.denominator_digest != plan.denominator.denominator_digest
        || checkpoint.predicate_digest != plan.predicate.predicate_digest
        || checkpoint.checkpoint_digest != expected_checkpoint_digest
        || checkpoint.completed_items.len() != checkpoint.result_receipt_refs.len()
    {
        return Err(ExactError::ExactReportInvalid);
    }
    let completed = checkpoint
        .completed_items
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if completed.len() != checkpoint.completed_items.len() {
        return Err(ExactError::ExactReportInvalid);
    }
    let remaining = plan
        .denominator
        .items
        .iter()
        .map(|item| item.revision.revision_id)
        .filter(|id| !completed.contains(id))
        .collect::<Vec<_>>();
    BoundedList::new(remaining).map_err(|_| ExactError::ContractViolation)
}

/// Semantic classification after report verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactCoverage {
    /// At least one exact source-backed match exists.
    MatchesFound,
    /// Every authoritative denominator item completed with zero matches.
    NoMatchInCompleteScope,
    /// Zero matches, but one or more completeness conditions are absent.
    IncompleteNoMatch,
    /// Report shape or accounting is contradictory.
    ExecutionInvalid,
}

/// Classifies exact coverage without upgrading partial execution.
#[must_use]
pub fn classify_completeness(
    plan: &CompiledExactScan,
    report: &ExactExecutionReport,
) -> ExactCoverage {
    if report.plan_ref != plan.plan_ref() || report.validate().is_err() {
        return ExactCoverage::ExecutionInvalid;
    }
    match report.conclusion {
        ExactConclusion::MatchesFound => ExactCoverage::MatchesFound,
        ExactConclusion::NoMatchInCompleteScope
            if plan.denominator.completeness.is_complete()
                && report.scanned_items
                    == u64::try_from(plan.denominator.items.len()).unwrap_or(u64::MAX) =>
        {
            ExactCoverage::NoMatchInCompleteScope
        }
        ExactConclusion::NoMatchInCompleteScope => ExactCoverage::ExecutionInvalid,
        ExactConclusion::Incomplete => ExactCoverage::IncompleteNoMatch,
    }
}

/// Verified content-free execution receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactVerificationReceipt {
    /// Verified plan reference.
    pub plan_ref: ExactScanPlanRef,
    /// Exact denominator digest.
    pub denominator_digest: Blake3Digest32,
    /// Exact predicate digest.
    pub predicate_digest: Blake3Digest32,
    /// Digest of verified report accounting.
    pub report_digest: Blake3Digest32,
    /// Verified semantic coverage.
    pub coverage: ExactCoverage,
    /// Source-backed execution receipt.
    pub execution_receipt_ref: ReceiptRef,
}

/// Recomputes item accounting and issues a verification receipt.
pub fn verify_execution_report(
    plan: &CompiledExactScan,
    report: &ExactExecutionReport,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<ExactVerificationReceipt, ExactError> {
    report
        .validate()
        .map_err(|_| ExactError::ExactReportInvalid)?;
    if report.plan_ref != plan.plan_ref() {
        return Err(ExactError::ExactReportInvalid);
    }
    let denominator = plan
        .denominator
        .items
        .iter()
        .map(|item| item.revision.revision_id)
        .collect::<BTreeSet<_>>();
    let mut failed = BTreeSet::new();
    for failure in report
        .unreadable_items
        .iter()
        .chain(report.changed_or_unavailable_items.iter())
    {
        if !denominator.contains(&failure.source_revision_id)
            || !failed.insert(failure.source_revision_id)
        {
            return Err(ExactError::ExactReportInvalid);
        }
    }
    let accounted = report
        .scanned_items
        .checked_add(u64::try_from(failed.len()).unwrap_or(u64::MAX))
        .ok_or(ExactError::ExactReportInvalid)?;
    if accounted != u64::try_from(denominator.len()).unwrap_or(u64::MAX) {
        return Err(ExactError::ExactReportInvalid);
    }
    let coverage = classify_completeness(plan, report);
    if coverage == ExactCoverage::ExecutionInvalid {
        return Err(ExactError::ExactReportInvalid);
    }
    let report_digest = Blake3Digest32::from_bytes(blake3_256(
        &report_digest_input(plan, report)?,
    ));
    Ok(ExactVerificationReceipt {
        plan_ref: report.plan_ref,
        denominator_digest: plan.denominator.denominator_digest,
        predicate_digest: plan.predicate.predicate_digest,
        report_digest,
        coverage,
        execution_receipt_ref: report.receipt_ref.clone(),
    })
}

fn report_digest_input(
    plan: &CompiledExactScan,
    report: &ExactExecutionReport,
) -> Result<Vec<u8>, ExactError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/exact-report/v1")?;
    bytes.extend_from_slice(report.plan_ref.plan_id.as_bytes());
    bytes.extend_from_slice(report.plan_ref.plan_fingerprint.as_bytes());
    bytes.extend_from_slice(plan.denominator.denominator_digest.as_bytes());
    bytes.extend_from_slice(plan.predicate.predicate_digest.as_bytes());
    bytes.extend_from_slice(&report.scanned_items.to_be_bytes());
    bytes.extend_from_slice(&report.scanned_bytes.to_be_bytes());
    bytes.push(u8::from(report.timed_out));
    bytes.push(u8::from(report.cancelled));
    bytes.push(u8::from(report.scope_drifted));
    bytes.push(coverage_tag(report.coverage));
    bytes.push(conclusion_tag(report.conclusion));
    for exact_match in &report.matched_items {
        bytes.extend_from_slice(exact_match.source_revision_ref.revision_id.as_bytes());
        bytes.extend_from_slice(exact_match.match_digest.as_bytes());
        bytes.extend_from_slice(&exact_match.matched_byte_length.to_be_bytes());
    }
    for failure in report
        .unreadable_items
        .iter()
        .chain(report.changed_or_unavailable_items.iter())
    {
        bytes.extend_from_slice(failure.source_revision_id.as_bytes());
        bytes.push(failure_kind_tag(failure.failure_kind));
    }
    append(&mut bytes, report.receipt_ref.as_str().as_bytes())?;
    Ok(bytes)
}

/// Current-fence observation for revalidating a complete negative proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactProofLiveState {
    /// Current denominator digest.
    pub denominator_digest: Blake3Digest32,
    /// Current predicate/profile digest.
    pub predicate_digest: Blake3Digest32,
    /// Current security fence digest.
    pub security_fence_digest: Blake3Digest32,
    /// Current access permits emission.
    pub access_permitted: bool,
    /// No purge barrier covers the proof.
    pub purge_clear: bool,
    /// Current observation continuity is established.
    pub current_observation: bool,
}

/// Revalidation outcome for a previously complete negative proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactProofRevalidation {
    /// Proof remains current for its declared scope.
    Current,
    /// Exact historical frozen proof remains internally valid but is stale for current scope.
    HistoricalOnly,
    /// Restrictive access change invalidated disclosure.
    AccessRevoked,
    /// Purge invalidated disclosure.
    Purged,
    /// Receipt was not a complete negative proof.
    Invalid,
}

/// Revalidates a verified complete-negative receipt against current fences.
#[must_use]
pub fn revalidate_complete_negative(
    receipt: &ExactVerificationReceipt,
    live: ExactProofLiveState,
) -> ExactProofRevalidation {
    if receipt.coverage != ExactCoverage::NoMatchInCompleteScope {
        return ExactProofRevalidation::Invalid;
    }
    if !live.access_permitted {
        return ExactProofRevalidation::AccessRevoked;
    }
    if !live.purge_clear {
        return ExactProofRevalidation::Purged;
    }
    if live.denominator_digest != receipt.denominator_digest
        || live.predicate_digest != receipt.predicate_digest
        || !live.current_observation
    {
        return ExactProofRevalidation::HistoricalOnly;
    }
    ExactProofRevalidation::Current
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), ExactError> {
    let length = u64::try_from(value.len())
        .map_err(|_| ExactError::ExactPredicateLimitExceeded)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    if output.len() > MAX_RAW_BYTES {
        return Err(ExactError::ExactPredicateLimitExceeded);
    }
    Ok(())
}

const fn predicate_kind_tag(value: ExactPredicateKind) -> u8 {
    match value {
        ExactPredicateKind::Literal => 1,
        ExactPredicateKind::Regex => 2,
        ExactPredicateKind::QualifiedSymbol => 3,
        ExactPredicateKind::StructuralPattern => 4,
        ExactPredicateKind::RecordField => 5,
    }
}

const fn input_domain_tag(value: ExactInputDomain) -> u8 {
    match value {
        ExactInputDomain::RawBytes => 1,
        ExactInputDomain::DecodedText => 2,
        ExactInputDomain::StructuralIr => 3,
    }
}

const fn normalization_tag(value: NormalizationPolicy) -> u8 {
    match value {
        NormalizationPolicy::None => 1,
        NormalizationPolicy::AsciiCaseInsensitive => 2,
    }
}

const fn failure_kind_tag(value: ExactItemFailureKind) -> u8 {
    match value {
        ExactItemFailureKind::Unreadable => 1,
        ExactItemFailureKind::RevisionUnavailable => 2,
        ExactItemFailureKind::ScopeChanged => 3,
        ExactItemFailureKind::Timeout => 4,
        ExactItemFailureKind::Cancelled => 5,
        ExactItemFailureKind::UnsupportedEncoding => 6,
        ExactItemFailureKind::PredicateError => 7,
    }
}

const fn coverage_tag(value: CoverageDenominatorKind) -> u8 {
    match value {
        CoverageDenominatorKind::CandidateScope => 1,
        CoverageDenominatorKind::CompleteScope => 2,
        CoverageDenominatorKind::Unknown => 3,
    }
}

const fn conclusion_tag(value: ExactConclusion) -> u8 {
    match value {
        ExactConclusion::MatchesFound => 1,
        ExactConclusion::NoMatchInCompleteScope => 2,
        ExactConclusion::Incomplete => 3,
    }
}
