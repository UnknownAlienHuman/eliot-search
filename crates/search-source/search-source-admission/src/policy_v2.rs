//! Immutable policy equality and byte-path classification, without admission authority.
//!
//! The caller owns the original deadline and cancellation probe. Synchronous
//! donor work is checked before and after; it cannot be interrupted internally.

use core::fmt;

use globset::{Candidate, GlobBuilder, GlobSet, GlobSetBuilder};
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue, ContractError, ContractErrorKind, DigestInputLimit, NonZeroRevision,
    blake3_canonical_array,
};

use super::observation_v2::{AdmissionObservationV2, RELATIVE_LOCATOR_PROFILE};
use super::{
    AdmissionError, AdmissionPolicyChange, AdmissionReasonCode, LocatorClass, RuleEffect,
    SensitivityLevel, SourceClass, ValidatedAdmissionConfig, check_id,
};

/// Distinct policy schema, never the retained v1 schema.
pub const POLICY_SCHEMA_VERSION: u32 = 2;
/// Immutable donor/options/grammar profile.
pub const POLICY_PROFILE: &str = "eliot-admission-globset-portable@2";
/// Exact reviewed donor version, also bound into the fingerprint.
pub const DONOR_VERSION: &str = "globset/0.4.20";
/// Complete domain-plus-CBOR equality domain.
pub const POLICY_DIGEST_DOMAIN: &str = "eliot/cbor/admission/policy/v2";
/// Absolute rule count before copying or compilation.
pub const MAX_RULES: usize = 128;
/// Absolute UTF-8 pattern bytes per rule.
pub const MAX_PATTERN_BYTES: usize = 1_024;
/// Absolute sum of pattern bytes before compilation.
pub const MAX_TOTAL_PATTERN_BYTES: usize = 32_768;
/// Absolute complete domain-plus-CBOR preimage ceiling.
pub const MAX_POLICY_PREIMAGE_BYTES: usize = 131_072;

const MAX_COMPONENTS: usize = 128;
const MAX_RECURSIVE_PAIRS: usize = 8;
const MAX_STAR_RUN: usize = 2;
const MAX_BRACE_GROUPS: usize = 4;
const MAX_BRACE_ARMS: usize = 16;
const MAX_CLASSES: usize = 16;
const MAX_CLASS_INTERIOR_BYTES: usize = 128;
const HEADER_ROWS: usize = 10;
const OPTIONS: [bool; 5] = [false, true, false, false, false];
const GRAMMAR: [usize; 11] = [
    MAX_RULES,
    MAX_PATTERN_BYTES,
    MAX_TOTAL_PATTERN_BYTES,
    MAX_POLICY_PREIMAGE_BYTES,
    MAX_COMPONENTS,
    MAX_RECURSIVE_PAIRS,
    MAX_STAR_RUN,
    MAX_BRACE_GROUPS,
    MAX_BRACE_ARMS,
    MAX_CLASSES,
    MAX_CLASS_INTERIOR_BYTES,
];

/// Raw finite compile ceilings; every entry point validates these values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileLimitsV2 {
    /// Maximum policy rules.
    pub max_rules: usize,
    /// Maximum bytes in one pattern.
    pub max_pattern_bytes: usize,
    /// Maximum sum of pattern bytes.
    pub max_total_pattern_bytes: usize,
    /// Maximum complete domain-plus-CBOR preimage bytes.
    pub max_encoded_bytes: usize,
}

impl CompileLimitsV2 {
    fn validate(self) -> Result<Self, AdmissionError> {
        if self.max_rules == 0
            || self.max_rules > MAX_RULES
            || self.max_pattern_bytes == 0
            || self.max_pattern_bytes > MAX_PATTERN_BYTES
            || self.max_total_pattern_bytes == 0
            || self.max_total_pattern_bytes > MAX_TOTAL_PATTERN_BYTES
            || self.max_encoded_bytes == 0
            || self.max_encoded_bytes > MAX_POLICY_PREIMAGE_BYTES
        {
            Err(AdmissionError::PolicyConflict)
        } else {
            Ok(self)
        }
    }

    const fn values(self) -> [usize; 4] {
        [
            self.max_rules,
            self.max_pattern_bytes,
            self.max_total_pattern_bytes,
            self.max_encoded_bytes,
        ]
    }
}

/// Conservative fixed compile ceilings; not measured resource qualification.
pub const DEFAULT_COMPILE_LIMITS: CompileLimitsV2 = CompileLimitsV2 {
    max_rules: MAX_RULES,
    max_pattern_bytes: MAX_PATTERN_BYTES,
    max_total_pattern_bytes: MAX_TOTAL_PATTERN_BYTES,
    max_encoded_bytes: MAX_POLICY_PREIMAGE_BYTES,
};

/// Closed failures from the caller's original deadline/cancellation checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointFailureV2 {
    /// Live caller cancellation was observed.
    Cancelled,
    /// The original caller-owned deadline was exhausted.
    DeadlineExceeded,
}

pub(super) fn checkpoint(
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<(), AdmissionError> {
    probe().map_err(|failure| match failure {
        CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
        CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
    })
}

/// Closed raw predicate: only the path predicate carries a pattern.
#[derive(Clone, Eq, PartialEq)]
pub enum PredicateV2 {
    /// Matches the rule's declared source class.
    Always,
    /// Compares to the rule's declared source class, without a second operand.
    SourceClassIs,
    /// Compares available sensitivity to a threshold.
    SensitivityAtLeast(SensitivityLevel),
    /// Compares available size to a threshold.
    SizeGreaterThan(u64),
    /// Compares the closed locator class.
    LocatorClassIs(LocatorClass),
    /// Matches one bounded relative-byte glob pattern.
    RelativePathGlobMatches(String),
}

impl PredicateV2 {
    /// Exact profile tag, with no free-form operator fallback.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::SourceClassIs => "source-class-is",
            Self::SensitivityAtLeast(_) => "sensitivity-at-least",
            Self::SizeGreaterThan(_) => "size-greater-than",
            Self::LocatorClassIs(_) => "locator-class-is",
            Self::RelativePathGlobMatches(_) => "relative-path-glob-matches",
        }
    }

    fn pattern(&self) -> Option<&str> {
        match self {
            Self::RelativePathGlobMatches(pattern) => Some(pattern),
            _ => None,
        }
    }
}

impl fmt::Debug for PredicateV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PredicateV2")
            .field("operator", &self.as_str())
            .field("pattern_bytes", &self.pattern().map(str::len))
            .finish_non_exhaustive()
    }
}

/// Editable raw rule input, with explicit unknown-field capture.
#[derive(Clone, Eq, PartialEq)]
pub struct RuleInputV2 {
    /// Stable bounded rule identifier.
    pub id: String,
    /// Explicit unique precedence, in ascending evaluation order.
    pub precedence: u32,
    /// Source class that scopes every predicate.
    pub source_class: SourceClass,
    /// Exactly one closed predicate with its required operand.
    pub predicate: PredicateV2,
    /// Closed effect; its reason code is computed by the owner.
    pub effect: RuleEffect,
    /// Must remain empty.
    pub unknown_fields: Vec<String>,
}

/// Editable raw policy, composed with an already validated immutable section.
#[derive(Clone, Eq, PartialEq)]
pub struct PolicyInputV2 {
    /// Must equal the distinct v2 schema.
    pub schema_version: u32,
    /// Non-zero policy revision, validated before copying.
    pub revision: u64,
    /// Immutable section, produced by the existing sole section validator.
    pub section: ValidatedAdmissionConfig,
    /// Rules in caller order; compilation establishes canonical order.
    pub rules: Vec<RuleInputV2>,
    /// Must remain empty.
    pub unknown_fields: Vec<String>,
}

/// Immutable validated rule, with no writable fields or alternate producer.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalRuleV2 {
    id: String,
    precedence: u32,
    source_class: SourceClass,
    predicate: PredicateV2,
    effect: RuleEffect,
}

impl CanonicalRuleV2 {
    /// Stable validated rule identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Canonical precedence.
    #[must_use]
    pub const fn precedence(&self) -> u32 {
        self.precedence
    }
    /// Class scope; path matches alone do not apply this condition.
    #[must_use]
    pub const fn source_class(&self) -> SourceClass {
        self.source_class
    }
    /// Immutable predicate and any bounded operand.
    #[must_use]
    pub const fn predicate(&self) -> &PredicateV2 {
        &self.predicate
    }
    /// Declared effect, without evaluation or admission authority.
    #[must_use]
    pub const fn effect(&self) -> RuleEffect {
        self.effect
    }
    /// Owner-computed reason for the declared effect.
    #[must_use]
    pub const fn reason(&self) -> AdmissionReasonCode {
        reason(self.effect)
    }
}

impl fmt::Debug for CanonicalRuleV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalRuleV2")
            .field("precedence", &self.precedence)
            .field("source_class", &self.source_class)
            .field("predicate", &self.predicate)
            .field("effect", &self.effect)
            .finish_non_exhaustive()
    }
}

const fn reason(effect: RuleEffect) -> AdmissionReasonCode {
    match effect {
        RuleEffect::Allow => AdmissionReasonCode::AllowExplicitRule,
        RuleEffect::Deny => AdmissionReasonCode::DenyExplicitRule,
        RuleEffect::Review => AdmissionReasonCode::ReviewExplicitRule,
        RuleEffect::Unsupported => AdmissionReasonCode::UnsupportedExplicitRule,
    }
}

/// Internally computed equality identity, never a caller-accepted proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyFingerprintV2(Blake3Digest32);

impl PolicyFingerprintV2 {
    /// Complete shared CBOR/BLAKE3 equality bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

/// One immutable compiled policy and exact glob-to-canonical-rule mapping.
pub struct CompiledAdmissionPolicyV2 {
    revision: NonZeroRevision,
    section: ValidatedAdmissionConfig,
    limits: CompileLimitsV2,
    rules: Vec<CanonicalRuleV2>,
    fingerprint: PolicyFingerprintV2,
    matcher: GlobSet,
    glob_to_rule: Vec<usize>,
}

impl CompiledAdmissionPolicyV2 {
    /// Validated revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }
    /// Immutable section data, without applied-effect authority.
    #[must_use]
    pub const fn section(&self) -> &ValidatedAdmissionConfig {
        &self.section
    }
    /// Validated effective ceilings bound into this policy's identity.
    #[must_use]
    pub const fn limits(&self) -> CompileLimitsV2 {
        self.limits
    }
    /// Immutable canonical rule table.
    #[must_use]
    pub fn rules(&self) -> &[CanonicalRuleV2] {
        &self.rules
    }
    /// Complete internally computed policy identity.
    #[must_use]
    pub const fn fingerprint(&self) -> PolicyFingerprintV2 {
        self.fingerprint
    }

    /// Allocates one policy-bound scratch vector before any observation match.
    #[must_use]
    pub fn new_match_buffer(&self) -> MatchBufferV2 {
        // The exact donor has at most two strategy inserts per global index.
        // Each local strategy emits at most one push per index. This bound is
        // pre-dedup, not a post-hoc check that would allow Vec growth first.
        MatchBufferV2 {
            policy: self.fingerprint,
            matches: Vec::with_capacity(2 * self.glob_to_rule.len()),
        }
    }

    /// Selects path-rule indexes in canonical order using a reusable buffer.
    ///
    /// These are classification candidates only. A future evaluator must also
    /// enforce class scope and deny dominance; this call emits no verdict.
    /// The probe must observe the original caller deadline and cancellation.
    ///
    /// # Errors
    /// Missing locator returns `ObservationIncomplete`; cross-policy scratch or
    /// impossible mapping returns `PolicyConflict`. Checkpoint failures return
    /// `Cancelled`/`BudgetExhausted`. All failures leave the buffer empty.
    pub fn match_path_rules<'a>(
        &self,
        observation: &AdmissionObservationV2,
        into: &'a mut MatchBufferV2,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<&'a [usize], AdmissionError> {
        into.matches.clear();
        let result = (|| {
            checkpoint(probe)?;
            if into.policy != self.fingerprint {
                return Err(AdmissionError::PolicyConflict);
            }
            let locator = observation
                .relative_locator()
                .ok_or(AdmissionError::ObservationIncomplete)?;
            let candidate = Candidate::from_bytes(locator.as_bytes());
            checkpoint(probe)?;
            self.matcher
                .matches_candidate_into(&candidate, &mut into.matches);
            checkpoint(probe)?;
            for index in &mut into.matches {
                *index = *self
                    .glob_to_rule
                    .get(*index)
                    .ok_or(AdmissionError::PolicyConflict)?;
            }
            checkpoint(probe)?;
            Ok(())
        })();
        if let Err(error) = result {
            into.matches.clear();
            return Err(error);
        }
        Ok(&into.matches)
    }
}

impl fmt::Debug for CompiledAdmissionPolicyV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompiledAdmissionPolicyV2")
            .field("profile", &POLICY_PROFILE)
            .field("rules", &self.rules.len())
            .field("patterns", &self.glob_to_rule.len())
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

/// Policy-bound reusable match storage; no caller-shaped indexes are accepted.
pub struct MatchBufferV2 {
    policy: PolicyFingerprintV2,
    matches: Vec<usize>,
}

impl MatchBufferV2 {
    /// Clears stale classification on evaluator entry and every refusal.
    pub(super) fn clear(&mut self) {
        self.matches.clear();
    }
}

impl fmt::Debug for MatchBufferV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MatchBufferV2")
            .field("matches", &self.matches.len())
            .finish_non_exhaustive()
    }
}

/// Conservative change direction, without an unproved pattern-inclusion rule.
#[must_use]
pub fn classify_policy_change_v2(
    old: &CompiledAdmissionPolicyV2,
    new: &CompiledAdmissionPolicyV2,
) -> AdmissionPolicyChange {
    if old.fingerprint == new.fingerprint {
        AdmissionPolicyChange::Noop
    } else {
        AdmissionPolicyChange::MixedSecurityBarrierAndReconcile
    }
}

/// Validates borrowed inputs, computes equality and builds one immutable set.
///
/// The mandatory caller probe is checked around every synchronous donor build.
/// No lock or clock is owned here; an individual build cannot be preempted.
///
/// # Errors
/// Schema/unknown/conflict/override refusals use existing closed admission
/// errors. Donor failures are content-free `PolicyConflict`. A refused complete
/// preimage returns `BudgetExhausted`, never a partial accepted policy.
pub fn compile_policy_v2(
    input: &PolicyInputV2,
    limits: CompileLimitsV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<CompiledAdmissionPolicyV2, AdmissionError> {
    checkpoint(probe)?;
    if input.schema_version != POLICY_SCHEMA_VERSION {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    if !input.unknown_fields.is_empty() {
        return Err(AdmissionError::PolicyUnknownField);
    }
    let revision =
        NonZeroRevision::new(input.revision).map_err(|_| AdmissionError::PolicyConflict)?;
    let limits = limits.validate()?;
    if input.rules.len() > limits.max_rules {
        return Err(AdmissionError::PolicyConflict);
    }
    if input
        .rules
        .iter()
        .any(|rule| !rule.unknown_fields.is_empty())
    {
        return Err(AdmissionError::PolicyUnknownField);
    }
    let mut total_pattern_bytes = 0_usize;
    for (index, rule) in input.rules.iter().enumerate() {
        check_id(&rule.id, "rule.id").map_err(|_| AdmissionError::PolicyConflict)?;
        if let Some(pattern) = rule.predicate.pattern() {
            if pattern.len() > limits.max_pattern_bytes {
                return Err(AdmissionError::PolicyConflict);
            }
            check_pattern_grammar(pattern)?;
            total_pattern_bytes = total_pattern_bytes
                .checked_add(pattern.len())
                .ok_or(AdmissionError::PolicyConflict)?;
            if total_pattern_bytes > limits.max_total_pattern_bytes {
                return Err(AdmissionError::PolicyConflict);
            }
        }
        if input.rules[..index]
            .iter()
            .any(|old| old.id == rule.id || old.precedence == rule.precedence)
        {
            return Err(AdmissionError::PolicyConflict);
        }
        if rule.effect == RuleEffect::Allow
            && (rule.source_class.is_unconditional_deny()
                || rule.source_class == SourceClass::System)
        {
            return Err(AdmissionError::PolicyOverrideForbidden);
        }
    }
    checkpoint(probe)?;
    let mut sorted: Vec<_> = input.rules.iter().collect();
    sorted.sort_by(|left, right| {
        left.precedence
            .cmp(&right.precedence)
            .then_with(|| left.id.cmp(&right.id))
    });
    let domain = CanonicalDigestDomain::parse(POLICY_DIGEST_DOMAIN)
        .map_err(|_| AdmissionError::PolicyConflict)?;
    let limit = DigestInputLimit::new(limits.max_encoded_bytes)
        .map_err(|_| AdmissionError::PolicyConflict)?;
    let fingerprint = blake3_canonical_array(
        &domain,
        HEADER_ROWS + sorted.len(),
        (0..HEADER_ROWS + sorted.len()).map(|index| policy_row(input, limits, &sorted, index)),
        limit,
    )
    .map_err(|_| AdmissionError::BudgetExhausted)?;
    checkpoint(probe)?;
    let mut builder = GlobSetBuilder::new();
    let mut glob_to_rule = Vec::with_capacity(sorted.len());
    for (index, rule) in sorted.iter().enumerate() {
        if let Some(pattern) = rule.predicate.pattern() {
            checkpoint(probe)?;
            let compiled = GlobBuilder::new(pattern)
                .case_insensitive(OPTIONS[0])
                .literal_separator(OPTIONS[1])
                .backslash_escape(OPTIONS[2])
                .empty_alternates(OPTIONS[3])
                .allow_unclosed_class(OPTIONS[4])
                .build();
            checkpoint(probe)?;
            builder.add(compiled.map_err(|_| AdmissionError::PolicyConflict)?);
            glob_to_rule.push(index);
        }
    }
    checkpoint(probe)?;
    let matcher = builder.build();
    checkpoint(probe)?;
    let matcher = matcher.map_err(|_| AdmissionError::PolicyConflict)?;
    let rules = sorted
        .into_iter()
        .map(|rule| CanonicalRuleV2 {
            id: rule.id.clone(),
            precedence: rule.precedence,
            source_class: rule.source_class,
            predicate: rule.predicate.clone(),
            effect: rule.effect,
        })
        .collect();
    checkpoint(probe)?;
    Ok(CompiledAdmissionPolicyV2 {
        revision,
        section: input.section.clone(),
        limits,
        rules,
        fingerprint: PolicyFingerprintV2(fingerprint),
        matcher,
        glob_to_rule,
    })
}

fn check_pattern_grammar(pattern: &str) -> Result<(), AdmissionError> {
    if pattern.is_empty()
        || pattern.len() > MAX_PATTERN_BYTES
        || pattern.starts_with('/')
        || pattern.ends_with('/')
        || pattern.chars().any(char::is_control)
        || pattern.bytes().any(|byte| matches!(byte, b'\\' | b':'))
    {
        return Err(AdmissionError::PolicyConflict);
    }
    let mut components = 0_usize;
    for component in pattern.split('/') {
        components += 1;
        if components > MAX_COMPONENTS || component.is_empty() || matches!(component, "." | "..") {
            return Err(AdmissionError::PolicyConflict);
        }
    }
    let mut classes = 0;
    let mut class_bytes = 0;
    let mut in_class = false;
    let mut groups = 0;
    let mut arms = 0;
    let mut in_group = false;
    let mut arm_nonempty = false;
    let mut star_run = 0;
    let mut recursive_pairs = 0;
    for byte in pattern.bytes() {
        if in_class {
            if byte == b']' {
                if class_bytes == 0 {
                    return Err(AdmissionError::PolicyConflict);
                }
                in_class = false;
            } else {
                class_bytes += 1;
                if class_bytes > MAX_CLASS_INTERIOR_BYTES || matches!(byte, b'[' | b'/') {
                    return Err(AdmissionError::PolicyConflict);
                }
            }
            continue;
        }
        if byte != b'*' {
            star_run = 0;
        }
        match byte {
            b'[' => {
                classes += 1;
                if classes > MAX_CLASSES {
                    return Err(AdmissionError::PolicyConflict);
                }
                in_class = true;
                class_bytes = 0;
                if in_group {
                    arm_nonempty = true;
                }
            }
            b']' => return Err(AdmissionError::PolicyConflict),
            b'{' => {
                if in_group {
                    return Err(AdmissionError::PolicyConflict);
                }
                groups += 1;
                arms += 1;
                if groups > MAX_BRACE_GROUPS || arms > MAX_BRACE_ARMS {
                    return Err(AdmissionError::PolicyConflict);
                }
                in_group = true;
                arm_nonempty = false;
            }
            b',' if in_group => {
                if !arm_nonempty {
                    return Err(AdmissionError::PolicyConflict);
                }
                arms += 1;
                if arms > MAX_BRACE_ARMS {
                    return Err(AdmissionError::PolicyConflict);
                }
                arm_nonempty = false;
            }
            b'}' => {
                if !in_group || !arm_nonempty {
                    return Err(AdmissionError::PolicyConflict);
                }
                in_group = false;
            }
            b'*' => {
                star_run += 1;
                if star_run > MAX_STAR_RUN {
                    return Err(AdmissionError::PolicyConflict);
                }
                if star_run == 2 {
                    recursive_pairs += 1;
                    if recursive_pairs > MAX_RECURSIVE_PAIRS {
                        return Err(AdmissionError::PolicyConflict);
                    }
                }
                if in_group {
                    arm_nonempty = true;
                }
            }
            _ => {
                if in_group {
                    arm_nonempty = true;
                }
            }
        }
    }
    if in_class || in_group {
        Err(AdmissionError::PolicyConflict)
    } else {
        Ok(())
    }
}

fn text(value: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(value).map(CanonicalValue::Text)
}

fn integer(value: usize) -> Result<CanonicalValue, ContractError> {
    u64::try_from(value)
        .map(CanonicalValue::U64)
        .map_err(|_| ContractError::new(ContractErrorKind::InvalidRange, "admission_policy_v2"))
}

fn array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    BoundedList::new(values).map(CanonicalValue::Array)
}

fn integers(values: &[usize]) -> Result<CanonicalValue, ContractError> {
    array(
        values
            .iter()
            .copied()
            .map(integer)
            .collect::<Result<_, _>>()?,
    )
}

fn operand(predicate: &PredicateV2) -> Result<CanonicalValue, ContractError> {
    match predicate {
        PredicateV2::Always | PredicateV2::SourceClassIs => Ok(CanonicalValue::Null),
        PredicateV2::SensitivityAtLeast(value) => text(value.as_str()),
        PredicateV2::SizeGreaterThan(value) => Ok(CanonicalValue::U64(*value)),
        PredicateV2::LocatorClassIs(value) => text(value.as_str()),
        PredicateV2::RelativePathGlobMatches(value) => {
            BoundedBytes::new(value.as_bytes().to_vec()).map(CanonicalValue::Bytes)
        }
    }
}

fn policy_row(
    input: &PolicyInputV2,
    limits: CompileLimitsV2,
    rules: &[&RuleInputV2],
    index: usize,
) -> Result<CanonicalValue, ContractError> {
    match index {
        0 => Ok(CanonicalValue::U64(u64::from(POLICY_SCHEMA_VERSION))),
        1 => text(POLICY_PROFILE),
        2 => text(RELATIVE_LOCATOR_PROFILE),
        3 => text(DONOR_VERSION),
        4 => array(OPTIONS.into_iter().map(CanonicalValue::Bool).collect()),
        5 => integers(&GRAMMAR),
        6 => integers(&limits.values()),
        7 => Ok(CanonicalValue::U64(input.revision)),
        8 => array(vec![
            text(input.section.bootstrap_policy_profile())?,
            CanonicalValue::U64(input.section.max_file_bytes()),
            CanonicalValue::Bool(input.section.allow_generated()),
            CanonicalValue::Bool(input.section.allow_vendor()),
            CanonicalValue::Bool(input.section.allow_binary()),
        ]),
        9 => integer(rules.len()),
        _ => {
            let rule = rules[index - HEADER_ROWS];
            array(vec![
                text(&rule.id)?,
                CanonicalValue::U64(u64::from(rule.precedence)),
                text(rule.source_class.as_str())?,
                text(rule.predicate.as_str())?,
                operand(&rule.predicate)?,
                text(rule.effect.as_str())?,
                text(reason(rule.effect).as_str())?,
            ])
        }
    }
}

#[cfg(test)]
mod tests {
    // #412 causal cases: AUTHORED_NOT_RUN under #352/#241/#215.
    use super::*;
    use crate::{AcceptedCapabilities, SectionPlatform, compiled_defaults, validate_section};
    use search_contracts::to_canonical_cbor;

    fn input() -> PolicyInputV2 {
        let defaults = compiled_defaults();
        let section = validate_section(
            &defaults,
            &SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            &AcceptedCapabilities {
                profiles: vec![defaults.bootstrap_policy_profile.clone()],
            },
        )
        .unwrap();
        PolicyInputV2 {
            schema_version: 2,
            revision: 1,
            section,
            rules: Vec::new(),
            unknown_fields: Vec::new(),
        }
    }

    fn rule(id: &str, precedence: u32, pattern: &str) -> RuleInputV2 {
        RuleInputV2 {
            id: id.into(),
            precedence,
            source_class: SourceClass::Regular,
            predicate: PredicateV2::RelativePathGlobMatches(pattern.into()),
            effect: RuleEffect::Deny,
            unknown_fields: Vec::new(),
        }
    }

    fn compile(input: &PolicyInputV2) -> CompiledAdmissionPolicyV2 {
        compile_policy_v2(input, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap()
    }

    fn observation(path: Option<&[u8]>) -> AdmissionObservationV2 {
        let raw = crate::UnvalidatedObservationInput {
            locator_class: "normalized-file".into(),
            location_class: "local-fixed".into(),
            source_kind: "file".into(),
            source_class: "regular".into(),
            byte_size: Some(10),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".into()),
            detector_id: Some("detector:fixture".into()),
            profile_id: Some(crate::BASELINE_PROFILE.into()),
            unavailable_fields: Vec::new(),
            unknown_fields: Vec::new(),
        };
        let metadata = crate::validate_observation(
            &raw,
            &crate::baseline_policy(1, 1_024),
            crate::DEFAULT_ADMISSION_LIMITS,
        )
        .unwrap();
        let locator = path.map(|bytes| {
            super::super::observation_v2::AdmissionRelativeLocator::from_bytes(bytes).unwrap()
        });
        AdmissionObservationV2::new(metadata, locator).unwrap()
    }

    #[test]
    fn matching_preserves_case_separator_bytes_and_canonical_rule_mapping() {
        let mut raw = input();
        let mut non_path = rule("metadata", 15, "unused");
        non_path.predicate = PredicateV2::Always;
        raw.rules = vec![
            rule("recursive", 20, "src/**"),
            non_path,
            rule("shallow", 10, "src/*.rs"),
        ];
        let policy = compile(&raw);
        let mut buffer = policy.new_match_buffer();
        let capacity = buffer.matches.capacity();
        for (path, expected) in [
            ("src/lib.rs", &[0, 2][..]),
            ("src/deep/lib.rs", &[2][..]),
            ("Src/lib.rs", &[][..]),
            ("src/é.rs", &[0, 2][..]),
        ] {
            assert_eq!(
                policy
                    .match_path_rules(
                        &observation(Some(path.as_bytes())),
                        &mut buffer,
                        &mut || Ok(())
                    )
                    .unwrap(),
                expected
            );
            assert_eq!(buffer.matches.capacity(), capacity);
        }
        raw.rules = vec![rule("exact", 1, "src/é.rs")];
        let exact = compile(&raw);
        let mut buffer = exact.new_match_buffer();
        assert_eq!(
            exact
                .match_path_rules(
                    &observation(Some("src/é.rs".as_bytes())),
                    &mut buffer,
                    &mut || Ok(())
                )
                .unwrap(),
            &[0]
        );
        assert!(
            exact
                .match_path_rules(
                    &observation(Some("src/e\u{301}.rs".as_bytes())),
                    &mut buffer,
                    &mut || Ok(())
                )
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn missing_locator_and_cross_policy_reuse_clear_the_whole_buffer() {
        let mut raw = input();
        raw.rules = vec![rule("path", 1, "src/**")];
        let policy = compile(&raw);
        let mut buffer = policy.new_match_buffer();
        assert_eq!(
            policy
                .match_path_rules(&observation(Some(b"src/lib.rs")), &mut buffer, &mut || Ok(
                    ()
                ))
                .unwrap(),
            &[0]
        );
        assert_eq!(
            policy.match_path_rules(&observation(None), &mut buffer, &mut || Ok(())),
            Err(AdmissionError::ObservationIncomplete)
        );
        assert!(buffer.matches.is_empty());
        buffer.matches.push(0);
        raw.revision += 1;
        let other = compile(&raw);
        assert_eq!(
            other.match_path_rules(&observation(Some(b"src/lib.rs")), &mut buffer, &mut || Ok(
                ()
            )),
            Err(AdmissionError::PolicyConflict)
        );
        assert!(buffer.matches.is_empty());
        assert_eq!(
            policy
                .match_path_rules(&observation(Some(b"src/lib.rs")), &mut buffer, &mut || Ok(
                    ()
                ))
                .unwrap(),
            &[0]
        );
        assert!(
            policy
                .match_path_rules(&observation(Some(b"docs/lib.rs")), &mut buffer, &mut || Ok(
                    ()
                ))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn match_checkpoint_failure_before_or_after_work_clears_results() {
        let mut raw = input();
        raw.rules = vec![rule("path", 1, "src/**")];
        let policy = compile(&raw);
        let mut buffer = policy.new_match_buffer();
        for fail_at in 1..=4 {
            for (failure, expected) in [
                (CheckpointFailureV2::Cancelled, AdmissionError::Cancelled),
                (
                    CheckpointFailureV2::DeadlineExceeded,
                    AdmissionError::BudgetExhausted,
                ),
            ] {
                buffer.matches.push(0);
                let mut seen = 0;
                let result = policy.match_path_rules(
                    &observation(Some(b"src/lib.rs")),
                    &mut buffer,
                    &mut || {
                        seen += 1;
                        if seen == fail_at {
                            Err(failure)
                        } else {
                            Ok(())
                        }
                    },
                );
                assert_eq!(result, Err(expected));
                assert_eq!(seen, fail_at);
                assert!(buffer.matches.is_empty());
            }
        }
        assert_eq!(
            policy
                .match_path_rules(&observation(Some(b"src/lib.rs")), &mut buffer, &mut || Ok(
                    ()
                ))
                .unwrap(),
            &[0]
        );
    }

    #[test]
    fn portable_grammar_refuses_malformed_components_groups_classes_and_controls() {
        for accepted in [
            "src/é.rs",
            "src/e\u{301}.rs",
            "研究/資料.txt",
            "a,b.rs",
            "{a,b}",
            "{[a,b]}/x",
            "[{,*}]",
            "a**b",
            "*/**/*",
        ] {
            assert!(check_pattern_grammar(accepted).is_ok(), "{accepted:?}");
        }
        for rejected in [
            "",
            "/src",
            "src/",
            "src//x",
            "src/.",
            "src/..",
            "a/./b",
            "a/../b",
            "a\\b",
            "C:file",
            "a\0b",
            "a\u{85}b",
            "***",
            "a***b",
            "{,a}",
            "{a,}",
            "{}",
            "{a",
            "a}",
            "{a,{b,c}}",
            "[abc",
            "[]",
            "a]b",
            "[a[b]",
            "[a/b]",
        ] {
            assert_eq!(
                check_pattern_grammar(rejected),
                Err(AdmissionError::PolicyConflict),
                "{rejected:?}"
            );
        }
    }

    #[test]
    fn grammar_ceilings_accept_exact_limits_and_refuse_plus_one() {
        let at_limit = [
            "a".repeat(MAX_PATTERN_BYTES),
            vec!["a"; MAX_COMPONENTS].join("/"),
            vec!["**"; MAX_RECURSIVE_PAIRS].join("/"),
            "{a,b,c,d}{e,f,g,h}{i,j,k,l}{m,n,o,p}".into(),
            "[a]".repeat(MAX_CLASSES),
            format!("[{}]", "a".repeat(MAX_CLASS_INTERIOR_BYTES)),
        ];
        for pattern in at_limit {
            assert!(check_pattern_grammar(&pattern).is_ok(), "{pattern:?}");
        }
        let over_limit = [
            "a".repeat(MAX_PATTERN_BYTES + 1),
            vec!["a"; MAX_COMPONENTS + 1].join("/"),
            vec!["**"; MAX_RECURSIVE_PAIRS + 1].join("/"),
            "{a,b}{c,d}{e,f}{g,h}{i,j}".into(),
            "{a,b,c,d}{e,f,g,h}{i,j,k,l}{m,n,o,p,q}".into(),
            "[a]".repeat(MAX_CLASSES + 1),
            format!("[{}]", "a".repeat(MAX_CLASS_INTERIOR_BYTES + 1)),
            "界".repeat(512),
        ];
        for pattern in over_limit {
            assert_eq!(
                check_pattern_grammar(&pattern),
                Err(AdmissionError::PolicyConflict),
                "{pattern:?}"
            );
        }
    }

    #[test]
    fn identity_is_order_independent_and_binds_class_operand_effect_section_and_limits() {
        let mut original = input();
        original.rules = vec![rule("last", 2, "src/*.rs"), rule("first", 1, "docs/**")];
        let policy = compile(&original);
        let mut shuffled = original.clone();
        shuffled.rules.reverse();
        assert_eq!(policy.fingerprint(), compile(&shuffled).fingerprint());
        assert_eq!(policy.rules()[0].id(), "first");
        assert_eq!(
            classify_policy_change_v2(&policy, &compile(&shuffled)),
            AdmissionPolicyChange::Noop
        );
        for mutation in 0..8 {
            let mut changed = original.clone();
            match mutation {
                0 => changed.rules[0].source_class = SourceClass::Test,
                1 => {
                    changed.rules[0].predicate =
                        PredicateV2::RelativePathGlobMatches("src/*.txt".into())
                }
                2 => changed.rules[0].effect = RuleEffect::Review,
                3 => changed.revision += 1,
                4 => {
                    let mut raw = compiled_defaults();
                    raw.max_file_bytes = 1_024;
                    changed.section = validate_section(
                        &raw,
                        &SectionPlatform {
                            os: "windows".into(),
                            arch: "x64".into(),
                        },
                        &AcceptedCapabilities {
                            profiles: vec![raw.bootstrap_policy_profile.clone()],
                        },
                    )
                    .unwrap();
                }
                5 => changed.rules[0].id = "renamed".into(),
                6 => changed.rules[0].precedence = 3,
                _ => changed.rules[0].predicate = PredicateV2::SizeGreaterThan(17),
            }
            let changed = compile(&changed);
            assert_ne!(policy.fingerprint(), changed.fingerprint());
            assert_eq!(
                classify_policy_change_v2(&policy, &changed),
                AdmissionPolicyChange::MixedSecurityBarrierAndReconcile
            );
        }
        let limits = CompileLimitsV2 {
            max_rules: MAX_RULES - 1,
            ..DEFAULT_COMPILE_LIMITS
        };
        let changed = compile_policy_v2(&original, limits, &mut || Ok(())).unwrap();
        assert_ne!(policy.fingerprint(), changed.fingerprint());
        assert_eq!(
            policy.rules()[0].reason(),
            AdmissionReasonCode::DenyExplicitRule
        );
    }

    #[test]
    fn complete_preimage_lower_ceiling_and_one_less_refuse_without_a_policy() {
        let input = input();
        // Shared serializer counts the complete specified rows; this is a
        // budget boundary proof, not an independent codec/golden claim.
        let mut limits = CompileLimitsV2 {
            max_encoded_bytes: 255,
            ..DEFAULT_COMPILE_LIMITS
        };
        let complete_len = |limits| {
            let rows = (0..HEADER_ROWS)
                .map(|index| policy_row(&input, limits, &[], index))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            to_canonical_cbor(&array(rows).unwrap()).unwrap().len() + POLICY_DIGEST_DOMAIN.len() + 1
        };
        let bytes = complete_len(limits);
        assert!((24..=255).contains(&bytes));
        limits.max_encoded_bytes = bytes;
        assert_eq!(complete_len(limits), bytes); // same CBOR integer width
        assert!(compile_policy_v2(&input, limits, &mut || Ok(())).is_ok());
        limits.max_encoded_bytes = bytes - 1;
        assert_eq!(
            compile_policy_v2(&input, limits, &mut || Ok(())).unwrap_err(),
            AdmissionError::BudgetExhausted
        );
        limits.max_encoded_bytes = 1;
        assert_eq!(
            compile_policy_v2(&input, limits, &mut || Ok(())).unwrap_err(),
            AdmissionError::BudgetExhausted
        );
    }

    #[test]
    fn all_non_path_predicates_have_closed_operands_and_computed_reasons() {
        let mut raw = input();
        let predicates = [
            PredicateV2::Always,
            PredicateV2::SourceClassIs,
            PredicateV2::SensitivityAtLeast(SensitivityLevel::Confidential),
            PredicateV2::SizeGreaterThan(17),
            PredicateV2::LocatorClassIs(LocatorClass::NormalizedFile),
        ];
        for (index, predicate) in predicates.into_iter().enumerate() {
            let mut next = rule(
                &format!("r{index}"),
                u32::try_from(index).unwrap(),
                "unused",
            );
            next.predicate = predicate;
            raw.rules.push(next);
        }
        let policy = compile(&raw);
        assert!(policy.glob_to_rule.is_empty());
        assert_eq!(policy.rules().len(), 5);
        assert!(
            policy
                .rules()
                .iter()
                .all(|rule| rule.reason() == AdmissionReasonCode::DenyExplicitRule)
        );
    }

    #[test]
    fn checkpoint_refusals_at_entry_and_after_donor_work_never_return_a_policy() {
        let mut raw = input();
        raw.rules = vec![rule("a", 1, "src/*.rs"), rule("b", 2, "docs/**")];
        let mut calls = 0;
        compile_policy_v2(&raw, DEFAULT_COMPILE_LIMITS, &mut || {
            calls += 1;
            Ok(())
        })
        .unwrap();
        // Include entry, both sides of individual builds, the set build and
        // final materialization. This is cooperative refusal, not preemption.
        assert!(calls >= 2 * (raw.rules.len() + 1) + 2);
        for fail_at in 1..=calls {
            for (failure, expected) in [
                (CheckpointFailureV2::Cancelled, AdmissionError::Cancelled),
                (
                    CheckpointFailureV2::DeadlineExceeded,
                    AdmissionError::BudgetExhausted,
                ),
            ] {
                let mut seen = 0;
                let result = compile_policy_v2(&raw, DEFAULT_COMPILE_LIMITS, &mut || {
                    seen += 1;
                    if seen == fail_at {
                        Err(failure)
                    } else {
                        Ok(())
                    }
                });
                assert_eq!(result.unwrap_err(), expected);
                assert_eq!(seen, fail_at);
            }
        }
        raw.schema_version = 1;
        assert_eq!(
            compile_policy_v2(&raw, DEFAULT_COMPILE_LIMITS, &mut || Err(
                CheckpointFailureV2::Cancelled
            ))
            .unwrap_err(),
            AdmissionError::Cancelled
        );
        assert_eq!(
            compile_policy_v2(&raw, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap_err(),
            AdmissionError::PolicySchemaUnsupported
        );
    }

    #[test]
    fn malformed_inputs_and_safety_overrides_refuse_before_compiled_authority() {
        let mut raw = input();
        raw.rules = vec![rule("a", 1, "src/*.rs"), rule("b", 2, "docs/**")];
        for dimension in 0..4 {
            for excessive in [false, true] {
                let mut limits = DEFAULT_COMPILE_LIMITS;
                let value = match dimension {
                    0 => &mut limits.max_rules,
                    1 => &mut limits.max_pattern_bytes,
                    2 => &mut limits.max_total_pattern_bytes,
                    _ => &mut limits.max_encoded_bytes,
                };
                *value = if excessive { *value + 1 } else { 0 };
                assert_eq!(
                    compile_policy_v2(&raw, limits, &mut || Ok(())).unwrap_err(),
                    AdmissionError::PolicyConflict
                );
            }
        }
        for limits in [
            CompileLimitsV2 {
                max_rules: 1,
                ..DEFAULT_COMPILE_LIMITS
            },
            CompileLimitsV2 {
                max_pattern_bytes: 1,
                ..DEFAULT_COMPILE_LIMITS
            },
            CompileLimitsV2 {
                max_total_pattern_bytes: 1,
                ..DEFAULT_COMPILE_LIMITS
            },
        ] {
            assert_eq!(
                compile_policy_v2(&raw, limits, &mut || Ok(())).unwrap_err(),
                AdmissionError::PolicyConflict
            );
        }
        for mutation in 0..6 {
            let mut bad = raw.clone();
            match mutation {
                0 => bad.revision = 0,
                1 => bad.rules[1].id = bad.rules[0].id.clone(),
                2 => bad.rules[1].precedence = bad.rules[0].precedence,
                3 => bad.rules[0].id.clear(),
                4 => {
                    bad.rules[0].predicate =
                        PredicateV2::RelativePathGlobMatches("x".repeat(MAX_PATTERN_BYTES + 1))
                }
                _ => bad
                    .rules
                    .resize_with(MAX_RULES + 1, || rule("extra", 3, "*")),
            }
            assert_eq!(
                compile_policy_v2(&bad, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap_err(),
                AdmissionError::PolicyConflict
            );
        }
        for class in [
            SourceClass::Credential,
            SourceClass::PrivateKey,
            SourceClass::System,
        ] {
            let mut bad = raw.clone();
            bad.rules[0].source_class = class;
            bad.rules[0].effect = RuleEffect::Allow;
            assert_eq!(
                compile_policy_v2(&bad, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap_err(),
                AdmissionError::PolicyOverrideForbidden
            );
        }
        raw.rules[0].unknown_fields.push("untrusted".into());
        assert_eq!(
            compile_policy_v2(&raw, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap_err(),
            AdmissionError::PolicyUnknownField
        );
        // Over-count refuses without scanning even a malformed rule table.
        raw.rules
            .resize_with(MAX_RULES + 1, || rule("extra", 3, "*"));
        assert_eq!(
            compile_policy_v2(&raw, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap_err(),
            AdmissionError::PolicyConflict
        );
        raw.unknown_fields.push("untrusted".into());
        assert_eq!(
            compile_policy_v2(&raw, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap_err(),
            AdmissionError::PolicyUnknownField
        );
    }
}
