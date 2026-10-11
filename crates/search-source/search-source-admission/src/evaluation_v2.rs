//! Pure effective admission over immutable v2 policy and observation models.
//!
//! Decisions express policy meaning only, without receipt, native identity,
//! root, access or currentness authority. The original caller owns checkpoints.

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalBlake3Output, CanonicalDigestDomain,
    CanonicalText, CanonicalValue, ContractError, ContractErrorKind, DigestInputLimit,
    NonZeroRevision, blake3_canonical_array_with_usage,
};

use super::batch_v2::EvaluationMeterV2;
use super::observation_v2::{AdmissionObservationV2, ObservationFingerprintV2};
use super::policy_v2::{
    CanonicalRuleV2, CheckpointFailureV2, CompiledAdmissionPolicyV2, MatchBufferV2,
    PolicyFingerprintV2, PredicateV2, checkpoint,
};
use super::{
    AdmissionError, AdmissionObservation, AdmissionOutcome, AdmissionReasonCode, LocationClass,
    SensitivityLevel, SourceClass, ValidatedAdmissionConfig,
};

/// Distinct effective-decision schema; retained v1 is unchanged.
pub const DECISION_SCHEMA_VERSION: u32 = 2;
/// Fixed effective semantics and finite equality profile.
pub const DECISION_PROFILE: &str = "eliot-admission-effective@2";
/// Complete domain-plus-CBOR decision preimage domain.
pub const DECISION_DIGEST_DOMAIN: &str = "eliot/cbor/admission/decision/v2";
/// Fixed maximum canonical rules charged by one evaluation.
pub const MAX_RULE_EVALUATIONS: usize = 128;
/// Fixed maximum applicable rule IDs in one decision.
pub const MAX_MATCHED_RULE_IDS: usize = 128;
/// Fixed closed reason vocabulary, including retained v1's unused baseline tag.
pub const MAX_REASONS: usize = 23;
/// Fixed complete decision preimage byte ceiling.
pub const MAX_DECISION_PREIMAGE_BYTES: usize = 65_536;
/// Fixed complete observation preimage byte ceiling.
pub const MAX_OBSERVATION_PREIMAGE_BYTES: usize = 16_384;

const DECISION_ROWS: usize = 8;

/// Raw finite operation ceilings; validated by every evaluator call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvaluationLimitsV2 {
    /// Maximum canonical rules, including nonmatching and nonpath rules.
    pub max_rule_evaluations: usize,
    /// Maximum applicable IDs, charged before every clone.
    pub max_matched_rule_ids: usize,
    /// Maximum distinct contributions before winning-category filtering.
    pub max_reasons: usize,
    /// Maximum complete decision domain-plus-CBOR preimage bytes.
    pub max_encoded_bytes: usize,
    /// Maximum complete existing observation preimage bytes.
    pub max_observation_encoded_bytes: usize,
}

impl EvaluationLimitsV2 {
    pub(super) fn validate(self) -> Result<Self, AdmissionError> {
        for (value, maximum) in [
            (self.max_rule_evaluations, MAX_RULE_EVALUATIONS),
            (self.max_matched_rule_ids, MAX_MATCHED_RULE_IDS),
            (self.max_reasons, MAX_REASONS),
            (self.max_encoded_bytes, MAX_DECISION_PREIMAGE_BYTES),
            (
                self.max_observation_encoded_bytes,
                MAX_OBSERVATION_PREIMAGE_BYTES,
            ),
        ] {
            if value == 0 || value > maximum {
                return Err(AdmissionError::ObservationInvalid);
            }
        }
        Ok(self)
    }
}

/// Fixed maxima, not a measured resource or cumulative whole-batch claim.
pub const DEFAULT_EVALUATION_LIMITS: EvaluationLimitsV2 = EvaluationLimitsV2 {
    max_rule_evaluations: MAX_RULE_EVALUATIONS,
    max_matched_rule_ids: MAX_MATCHED_RULE_IDS,
    max_reasons: MAX_REASONS,
    max_encoded_bytes: MAX_DECISION_PREIMAGE_BYTES,
    max_observation_encoded_bytes: MAX_OBSERVATION_PREIMAGE_BYTES,
};

/// Internally computed decision equality bytes, never supplied proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecisionFingerprintV2(Blake3Digest32);

impl DecisionFingerprintV2 {
    /// Complete shared canonical CBOR/BLAKE3 identity bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

/// One immutable effective decision bound to the evaluated policy and observation.
#[derive(Clone, Eq, PartialEq)]
pub struct AdmissionDecisionV2 {
    outcome: AdmissionOutcome,
    reason_codes: Vec<AdmissionReasonCode>,
    matched_rule_ids: Vec<String>,
    sensitivity: SensitivityLevel,
    policy_revision: NonZeroRevision,
    policy_fingerprint: PolicyFingerprintV2,
    observation_fingerprint: ObservationFingerprintV2,
    fingerprint: DecisionFingerprintV2,
}

impl AdmissionDecisionV2 {
    /// Effective deny/unsupported/review/explicit-Allow result.
    #[must_use]
    pub const fn outcome(&self) -> AdmissionOutcome {
        self.outcome
    }
    /// Winning-category reasons in closed enum order.
    #[must_use]
    pub fn reason_codes(&self) -> &[AdmissionReasonCode] {
        &self.reason_codes
    }
    /// All applicable rule IDs in canonical precedence/stable-ID order.
    #[must_use]
    pub fn matched_rule_ids(&self) -> &[String] {
        &self.matched_rule_ids
    }
    /// Observed sensitivity, conservatively Confidential when unavailable.
    #[must_use]
    pub const fn sensitivity(&self) -> SensitivityLevel {
        self.sensitivity
    }
    /// Exact evaluated policy revision.
    #[must_use]
    pub const fn policy_revision(&self) -> NonZeroRevision {
        self.policy_revision
    }
    /// Exact immutable compiled policy identity, including compile ceilings.
    #[must_use]
    pub const fn policy_fingerprint(&self) -> PolicyFingerprintV2 {
        self.policy_fingerprint
    }
    /// Exact immutable observation identity.
    #[must_use]
    pub const fn observation_fingerprint(&self) -> ObservationFingerprintV2 {
        self.observation_fingerprint
    }
    /// Internally computed complete decision identity.
    #[must_use]
    pub const fn fingerprint(&self) -> DecisionFingerprintV2 {
        self.fingerprint
    }
}

impl fmt::Debug for AdmissionDecisionV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmissionDecisionV2")
            .field("outcome", &self.outcome)
            .field("reasons", &self.reason_codes.len())
            .field("matched_rules", &self.matched_rule_ids.len())
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

/// Evaluates all canonical class-scoped rules into one immutable bound decision.
///
/// The caller's original checkpoint is reused around synchronous work and at
/// every rule. No owned clock, renewed deadline or hard preemption is implied.
/// Success may retain internally regenerated classifier indexes in scratch.
///
/// # Errors
/// Invalid raw limits return `ObservationInvalid`; actual count or complete-byte
/// exhaustion returns `BudgetExhausted`. Missing locator refuses the whole call
/// with `ObservationIncomplete`, even for nonpath policies. Cross-policy scratch
/// returns `PolicyConflict`. Every refusal clears scratch and emits no decision.
pub fn evaluate_v2(
    policy: &CompiledAdmissionPolicyV2,
    observation: &AdmissionObservationV2,
    scratch: &mut MatchBufferV2,
    limits: EvaluationLimitsV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionDecisionV2, AdmissionError> {
    evaluate_v2_metered(policy, observation, scratch, limits, None, probe)
}

// The batch owner freshly creates the private meter for one whole call.
// Some means its sole private caller already validated these per-item limits.
// None is the existing public evaluator path, with unchanged raw validation.
pub(super) fn evaluate_v2_metered(
    policy: &CompiledAdmissionPolicyV2,
    observation: &AdmissionObservationV2,
    scratch: &mut MatchBufferV2,
    limits: EvaluationLimitsV2,
    mut meter: Option<&mut EvaluationMeterV2>,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionDecisionV2, AdmissionError> {
    scratch.clear();
    let result = (|| {
        checkpoint(probe)?;
        let limits = if meter.is_none() {
            limits.validate()?
        } else {
            // BatchLimitsV2::validate already checked these fresh call limits once.
            limits
        };
        if policy.rules().len() > limits.max_rule_evaluations {
            return Err(AdmissionError::BudgetExhausted);
        }
        if let Some(meter) = meter.as_deref_mut() {
            // Every canonical rule, including nonmatching/nonpath rules, is
            // charged before observation/matcher/evaluator work for this item.
            meter.charge_rules(policy.rules().len())?;
        }
        checkpoint(probe)?;
        if let Some(meter) = meter.as_deref_mut() {
            let ceiling = meter.encoded_limit(limits.max_observation_encoded_bytes)?;
            // Integration seam owned by observation_v2: same identity check and
            // rows, but returns the successful shared producer's actual usage.
            let output = observation.validate_encoded_limit_with_usage(ceiling)?;
            let _ = meter.charge_encoded(output)?;
        } else {
            observation.validate_encoded_limit(limits.max_observation_encoded_bytes)?;
        }
        checkpoint(probe)?;
        let path_matches = policy.match_path_rules(observation, scratch, probe)?;
        let metadata = observation.metadata();
        let mut contributions =
            baseline_reasons(metadata, policy.section(), limits.max_reasons, &mut meter)?;
        let mut matched_rule_ids = Vec::new();
        for (index, rule) in policy.rules().iter().enumerate() {
            checkpoint(probe)?;
            if rule_matches(rule, index, metadata, path_matches) {
                if matched_rule_ids.len() >= limits.max_matched_rule_ids {
                    return Err(AdmissionError::BudgetExhausted);
                }
                if let Some(meter) = meter.as_deref_mut() {
                    meter.charge_matched_rule_id()?;
                }
                matched_rule_ids.push(rule.id().to_owned());
                insert_reason_metered(
                    &mut contributions,
                    rule.reason(),
                    limits.max_reasons,
                    &mut meter,
                )?;
            }
        }
        let outcome = resolve(&contributions);
        if contributions.is_empty() {
            insert_reason_metered(
                &mut contributions,
                AdmissionReasonCode::DenyByDefault,
                limits.max_reasons,
                &mut meter,
            )?;
        }
        let reason_codes: Vec<_> = contributions
            .into_iter()
            .filter(|reason| reason_outcome(*reason) == outcome)
            .collect();
        let sensitivity = metadata
            .sensitivity
            .unwrap_or(SensitivityLevel::Confidential);
        let data = DecisionData {
            policy,
            observation,
            outcome,
            reasons: &reason_codes,
            matched_ids: &matched_rule_ids,
            sensitivity,
        };
        checkpoint(probe)?;
        let digest = if let Some(meter) = meter.as_deref_mut() {
            let ceiling = meter.encoded_limit(limits.max_encoded_bytes)?;
            let output = compute_decision_fingerprint_with_usage(&data, ceiling)
                .map_err(encoded_limit_error)?;
            meter.charge_encoded(output)?
        } else {
            compute_decision_fingerprint(&data, limits.max_encoded_bytes)
                .map_err(encoded_limit_error)?
        };
        let fingerprint = DecisionFingerprintV2(digest);
        checkpoint(probe)?;
        let decision = AdmissionDecisionV2 {
            outcome,
            reason_codes,
            matched_rule_ids,
            sensitivity,
            policy_revision: policy.revision(),
            policy_fingerprint: policy.fingerprint(),
            observation_fingerprint: observation.fingerprint(),
            fingerprint,
        };
        checkpoint(probe)?;
        Ok(decision)
    })();
    if result.is_err() {
        scratch.clear();
    }
    result
}

fn insert_reason_metered(
    reasons: &mut BTreeSet<AdmissionReasonCode>,
    reason: AdmissionReasonCode,
    maximum: usize,
    meter: &mut Option<&mut EvaluationMeterV2>,
) -> Result<(), AdmissionError> {
    if !reasons.contains(&reason) {
        // Local ceiling wins before the aggregate charge; both are exhaustion.
        if reasons.len() >= maximum {
            return Err(AdmissionError::BudgetExhausted);
        }
        if let Some(meter) = meter.as_deref_mut() {
            meter.charge_reason_contribution()?;
        }
    }
    // Duplicate reasons charge once per item. All categories remain visible
    // here, before resolve/filter, including baseline and fallback reasons.
    insert_reason(reasons, reason, maximum)
}

fn insert_reason(
    reasons: &mut BTreeSet<AdmissionReasonCode>,
    reason: AdmissionReasonCode,
    maximum: usize,
) -> Result<(), AdmissionError> {
    if reasons.len() >= maximum && !reasons.contains(&reason) {
        return Err(AdmissionError::BudgetExhausted);
    }
    reasons.insert(reason);
    Ok(())
}

fn baseline_reasons(
    metadata: &AdmissionObservation,
    section: &ValidatedAdmissionConfig,
    maximum: usize,
    meter: &mut Option<&mut EvaluationMeterV2>,
) -> Result<BTreeSet<AdmissionReasonCode>, AdmissionError> {
    use AdmissionReasonCode as R;
    let mut reasons = BTreeSet::new();
    for (applies, reason) in [
        (
            metadata.source_class == SourceClass::Credential,
            R::DenyCredentialClass,
        ),
        (
            metadata.source_class == SourceClass::PrivateKey,
            R::DenyPrivateKeyClass,
        ),
        (
            metadata
                .sensitivity
                .is_some_and(SensitivityLevel::is_unconditional_deny),
            R::DenySensitiveCredential,
        ),
        (
            metadata.source_class == SourceClass::System || metadata.is_system == Some(true),
            R::DenySystemLocation,
        ),
        (
            metadata.source_class == SourceClass::Cache,
            R::DenyCacheArtifact,
        ),
        (
            metadata.source_class == SourceClass::BuildOutput,
            R::DenyBuildOutput,
        ),
        (
            (metadata.source_class == SourceClass::Generated
                || metadata.is_generated == Some(true))
                && !section.allow_generated(),
            R::DenyGeneratedClass,
        ),
        (
            (metadata.source_class == SourceClass::Vendor || metadata.is_vendor == Some(true))
                && !section.allow_vendor(),
            R::DenyVendorClass,
        ),
        (
            (metadata.source_class == SourceClass::Binary || metadata.is_binary == Some(true))
                && !section.allow_binary(),
            R::DenyBinaryClass,
        ),
        (
            metadata.sensitivity == Some(SensitivityLevel::SecretCandidate),
            R::DenySensitiveClass,
        ),
        (metadata.byte_size == Some(0), R::DenyEmptySource),
        (
            metadata
                .byte_size
                .is_some_and(|size| size > section.max_file_bytes()),
            R::DenySizeExceeded,
        ),
        (
            metadata.location_class == LocationClass::Remote,
            R::DenyRemoteLocation,
        ),
        (
            !metadata.source_kind.is_admittable(),
            R::UnsupportedSourceKind,
        ),
        (metadata.detector_id.is_none(), R::ReviewDetectorUnavailable),
        (metadata.sensitivity.is_none(), R::ReviewSensitivityUnknown),
        (
            metadata.byte_size.is_none()
                || metadata.profile_id.is_none()
                || metadata.is_generated.is_none()
                || metadata.is_vendor.is_none()
                || metadata.is_binary.is_none()
                || metadata.is_system.is_none(),
            R::ReviewMetadataUnavailable,
        ),
    ] {
        if applies {
            insert_reason_metered(&mut reasons, reason, maximum, meter)?;
        }
    }
    Ok(reasons)
}

fn rule_matches(
    rule: &CanonicalRuleV2,
    index: usize,
    metadata: &AdmissionObservation,
    path_matches: &[usize],
) -> bool {
    if rule.source_class() != metadata.source_class {
        return false;
    }
    match rule.predicate() {
        PredicateV2::Always | PredicateV2::SourceClassIs => true,
        PredicateV2::SensitivityAtLeast(threshold) => metadata
            .sensitivity
            .is_some_and(|actual| actual >= *threshold),
        PredicateV2::SizeGreaterThan(threshold) => {
            metadata.byte_size.is_some_and(|actual| actual > *threshold)
        }
        PredicateV2::LocatorClassIs(expected) => metadata.locator_class == *expected,
        // Donor indexes are sorted/deduplicated, and the compiled mapping is
        // monotonic in canonical rule order. No new match vector is needed.
        PredicateV2::RelativePathGlobMatches(_) => path_matches.binary_search(&index).is_ok(),
    }
}

fn reason_outcome(reason: AdmissionReasonCode) -> AdmissionOutcome {
    use AdmissionReasonCode as R;
    match reason {
        R::AllowBaselineSource | R::AllowExplicitRule => AdmissionOutcome::Allow,
        R::DenyCredentialClass
        | R::DenyPrivateKeyClass
        | R::DenySensitiveCredential
        | R::DenySystemLocation
        | R::DenyCacheArtifact
        | R::DenyBuildOutput
        | R::DenyVendorClass
        | R::DenyGeneratedClass
        | R::DenyBinaryClass
        | R::DenySensitiveClass
        | R::DenySizeExceeded
        | R::DenyEmptySource
        | R::DenyRemoteLocation
        | R::DenyExplicitRule
        | R::DenyByDefault => AdmissionOutcome::Deny,
        R::ReviewDetectorUnavailable
        | R::ReviewSensitivityUnknown
        | R::ReviewMetadataUnavailable
        | R::ReviewExplicitRule => AdmissionOutcome::ReviewRequired,
        R::UnsupportedSourceKind | R::UnsupportedExplicitRule => AdmissionOutcome::Unsupported,
    }
}

fn resolve(reasons: &BTreeSet<AdmissionReasonCode>) -> AdmissionOutcome {
    for outcome in [
        AdmissionOutcome::Deny,
        AdmissionOutcome::Unsupported,
        AdmissionOutcome::ReviewRequired,
        AdmissionOutcome::Allow,
    ] {
        if reasons
            .iter()
            .any(|reason| reason_outcome(*reason) == outcome)
        {
            return outcome;
        }
    }
    AdmissionOutcome::Deny
}

struct DecisionData<'a> {
    policy: &'a CompiledAdmissionPolicyV2,
    observation: &'a AdmissionObservationV2,
    outcome: AdmissionOutcome,
    reasons: &'a [AdmissionReasonCode],
    matched_ids: &'a [String],
    sensitivity: SensitivityLevel,
}

fn compute_decision_fingerprint(
    data: &DecisionData<'_>,
    max_encoded_bytes: usize,
) -> Result<Blake3Digest32, ContractError> {
    compute_decision_fingerprint_with_usage(data, max_encoded_bytes)
        .map(CanonicalBlake3Output::digest)
}

fn compute_decision_fingerprint_with_usage(
    data: &DecisionData<'_>,
    max_encoded_bytes: usize,
) -> Result<CanonicalBlake3Output, ContractError> {
    let domain = CanonicalDigestDomain::parse(DECISION_DIGEST_DOMAIN)?;
    blake3_canonical_array_with_usage(
        &domain,
        DECISION_ROWS,
        (0..DECISION_ROWS).map(|index| decision_row(data, index)),
        DigestInputLimit::new(max_encoded_bytes)?,
    )
}

fn text(value: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(value).map(CanonicalValue::Text)
}

fn array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    BoundedList::new(values).map(CanonicalValue::Array)
}

fn integer(value: usize) -> Result<CanonicalValue, ContractError> {
    u64::try_from(value)
        .map(CanonicalValue::U64)
        .map_err(|_| ContractError::new(ContractErrorKind::InvalidRange, "admission_decision_v2"))
}

fn decision_row(data: &DecisionData<'_>, index: usize) -> Result<CanonicalValue, ContractError> {
    match index {
        0 => array(vec![
            CanonicalValue::U64(u64::from(DECISION_SCHEMA_VERSION)),
            text(DECISION_PROFILE)?,
            integer(MAX_RULE_EVALUATIONS)?,
            integer(MAX_MATCHED_RULE_IDS)?,
            integer(MAX_REASONS)?,
            integer(MAX_DECISION_PREIMAGE_BYTES)?,
            integer(MAX_OBSERVATION_PREIMAGE_BYTES)?,
        ]),
        1 => Ok(CanonicalValue::U64(data.policy.revision().get())),
        2 => BoundedBytes::new(data.policy.fingerprint().as_bytes().to_vec())
            .map(CanonicalValue::Bytes),
        3 => BoundedBytes::new(data.observation.fingerprint().as_bytes().to_vec())
            .map(CanonicalValue::Bytes),
        4 => text(data.outcome.as_str()),
        5 => array(
            data.reasons
                .iter()
                .map(|reason| text(reason.as_str()))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        6 => array(
            data.matched_ids
                .iter()
                .map(|id| text(id))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        7 => text(data.sensitivity.as_str()),
        _ => Err(ContractError::new(
            ContractErrorKind::InvalidRange,
            "admission_decision_v2_row",
        )),
    }
}

fn encoded_limit_error(error: ContractError) -> AdmissionError {
    if error.kind() == ContractErrorKind::OversizePayload
        && matches!(error.field(), "digest_preimage" | "canonical_sink")
    {
        AdmissionError::BudgetExhausted
    } else {
        AdmissionError::ObservationInvalid
    }
}

// Private causal fixtures: AUTHORED_NOT_RUN under #352/#416/#215.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuleEffect;
    use crate::observation_v2::{AdmissionObservationV2, AdmissionRelativeLocator};
    use crate::policy_v2::{
        CheckpointFailureV2, DEFAULT_COMPILE_LIMITS, PolicyInputV2, PredicateV2, RuleInputV2,
        compile_policy_v2,
    };
    use crate::{
        AcceptedCapabilities, DEFAULT_ADMISSION_LIMITS, SectionPlatform,
        UnvalidatedObservationInput, baseline_policy, compiled_defaults, validate_observation,
        validate_section,
    };

    // ---- shared helpers -------------------------------------------------

    fn platform() -> SectionPlatform {
        SectionPlatform {
            os: "windows".to_owned(),
            arch: "x64".to_owned(),
        }
    }

    fn capabilities() -> AcceptedCapabilities {
        AcceptedCapabilities {
            // No hardcoded profile: derive the accepted vocabulary from the
            // compiled baseline input, exactly as accepted observation fixtures do.
            profiles: vec![compiled_defaults().bootstrap_policy_profile.clone()],
        }
    }

    /// Immutable section from the existing sole section validator.
    fn section() -> crate::ValidatedAdmissionConfig {
        validate_section(&compiled_defaults(), &platform(), &capabilities())
            .expect("defaults validate")
    }

    /// Permissive section with one flag lifted, still through the sole validator.
    fn permissive(flag: &str) -> crate::ValidatedAdmissionConfig {
        let mut raw = compiled_defaults();
        match flag {
            "generated" => raw.allow_generated = true,
            "vendor" => raw.allow_vendor = true,
            "binary" => raw.allow_binary = true,
            _ => unreachable!("closed flag vocabulary"),
        }
        validate_section(&raw, &platform(), &capabilities()).expect("permissive validates")
    }

    fn rule(id: &str, precedence: u32, predicate: PredicateV2, effect: RuleEffect) -> RuleInputV2 {
        RuleInputV2 {
            id: id.to_owned(),
            precedence,
            source_class: SourceClass::Regular,
            predicate,
            effect,
            unknown_fields: Vec::new(),
        }
    }

    fn policy(rules: Vec<RuleInputV2>) -> crate::policy_v2::CompiledAdmissionPolicyV2 {
        let input = PolicyInputV2 {
            schema_version: crate::policy_v2::POLICY_SCHEMA_VERSION,
            revision: 7,
            section: section(),
            rules,
            unknown_fields: Vec::new(),
        };
        compile_policy_v2(&input, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).expect("compiles")
    }

    /// Observation through the existing validated constructors, never raw text.
    fn raw_observation() -> UnvalidatedObservationInput {
        UnvalidatedObservationInput {
            locator_class: "normalized-file".to_owned(),
            location_class: "local-fixed".to_owned(),
            source_kind: "file".to_owned(),
            source_class: "regular".to_owned(),
            byte_size: Some(64),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".to_owned()),
            detector_id: Some("detector:fixture".to_owned()),
            profile_id: Some(crate::BASELINE_PROFILE.to_owned()),
            unavailable_fields: Vec::new(),
            unknown_fields: Vec::new(),
        }
    }

    fn observation(path: Option<&[u8]>) -> AdmissionObservationV2 {
        model(&raw_observation(), path)
    }

    fn model(raw: &UnvalidatedObservationInput, path: Option<&[u8]>) -> AdmissionObservationV2 {
        let policy = baseline_policy(3, 16_777_216);
        let metadata = validate_observation(raw, &policy, DEFAULT_ADMISSION_LIMITS)
            .expect("bounded observation");
        let locator =
            path.map(|p| AdmissionRelativeLocator::from_bytes(p).expect("bounded locator"));
        AdmissionObservationV2::new(metadata, locator).expect("bounded model")
    }

    /// Probe that succeeds until call `fail_at`, then reports the original failure.
    fn probe_failing_at(fail_at: usize) -> impl FnMut() -> Result<(), CheckpointFailureV2> {
        let mut calls = 0_usize;
        move || {
            calls += 1;
            if calls == fail_at {
                return Err(CheckpointFailureV2::DeadlineExceeded);
            }
            Ok(())
        }
    }

    /// Records every probe call index so the envelope is measured, never guessed.
    fn recording_probe() -> (
        impl FnMut() -> Result<(), CheckpointFailureV2>,
        std::rc::Rc<std::cell::RefCell<Vec<usize>>>,
    ) {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = std::rc::Rc::clone(&seen);
        let probe = move || {
            let index = sink.borrow().len() + 1;
            sink.borrow_mut().push(index);
            Ok(())
        };
        (probe, seen)
    }

    // ---- group 1: actual glob effect and class scope -------------------

    // AUTHORED_NOT_RUN. Two observations differ only in relative bytes, so the
    // single path rule matches one and misses the other. The miss proves the
    // glob is load-bearing; a decorative matcher would return the same result.
    #[test]
    fn glob_effect_changes_outcome_on_path_match_and_miss() {
        let policy = policy(vec![rule(
            "allow-src",
            10,
            PredicateV2::RelativePathGlobMatches("src/*.rs".to_owned()),
            RuleEffect::Allow,
        )]);
        let hit = observation(Some(b"src/lib.rs"));
        let miss = observation(Some(b"docs/readme.md"));
        let mut scratch = policy.new_match_buffer();
        let allowed = evaluate_v2(
            &policy,
            &hit,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .expect("allowed under matching path rule");
        assert_eq!(allowed.outcome(), AdmissionOutcome::Allow);
        let mut scratch = policy.new_match_buffer();
        let denied = evaluate_v2(
            &policy,
            &miss,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .expect("denied by default when no rule matches");
        assert_eq!(denied.outcome(), AdmissionOutcome::Deny);
        assert!(
            denied
                .reason_codes()
                .contains(&AdmissionReasonCode::DenyByDefault)
        );
        assert!(denied.matched_rule_ids().is_empty());
    }

    // AUTHORED_NOT_RUN. Identical predicates under different class scopes, so
    // only the rule whose class equals the observation class can contribute.
    #[test]
    fn class_scope_restricts_which_rule_can_contribute() {
        let mut regular = rule("allow-regular", 10, PredicateV2::Always, RuleEffect::Allow);
        let mut vendor = rule("allow-vendor", 20, PredicateV2::Always, RuleEffect::Allow);
        regular.source_class = SourceClass::Regular;
        vendor.source_class = SourceClass::Vendor;
        let policy = policy(vec![regular, vendor]);
        let seen = observation(Some(b"src/lib.rs"));
        let mut scratch = policy.new_match_buffer();
        let decision = evaluate_v2(
            &policy,
            &seen,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .expect("scoped evaluation");
        assert_eq!(decision.matched_rule_ids(), &["allow-regular"]);
        assert_eq!(decision.outcome(), AdmissionOutcome::Allow);
    }

    // ---- group 2: canonical permutations ---------------------------------

    // AUTHORED_NOT_RUN. Caller order must not reach the outcome, the ordered
    // reasons, or the ordered matched IDs. Compilation establishes canonical order.
    #[test]
    fn caller_rule_order_does_not_change_any_decision_field() {
        let allow = rule("allow-a", 10, PredicateV2::Always, RuleEffect::Allow);
        let deny = rule("deny-b", 20, PredicateV2::Always, RuleEffect::Deny);
        let observed = observation(Some(b"src/lib.rs"));
        let forward = policy(vec![allow.clone(), deny.clone()]);
        let reverse = policy(vec![deny, allow]);
        let mut left = forward.new_match_buffer();
        let left = evaluate_v2(
            &forward,
            &observed,
            &mut left,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .expect("forward decision");
        let mut right = reverse.new_match_buffer();
        let right = evaluate_v2(
            &reverse,
            &observed,
            &mut right,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .expect("reverse decision");
        assert_eq!(left.outcome(), right.outcome());
        assert_eq!(left.reason_codes(), right.reason_codes());
        assert_eq!(left.matched_rule_ids(), right.matched_rule_ids());
        assert_eq!(left.fingerprint(), right.fingerprint());
    }

    // ---- group 3: no implicit allow, deny-over-allow ----------------------

    // AUTHORED_NOT_RUN. Deny dominates an allow at any precedence, so a low
    // precedence deny must still beat a high precedence allow. This is the
    // v1 shape at lib.rs:2323-2347, deliberately preserved.
    #[test]
    fn deny_dominates_allow_regardless_of_precedence() {
        let policy = policy(vec![
            rule("allow-early", 1, PredicateV2::Always, RuleEffect::Allow),
            rule("deny-late", 9_999, PredicateV2::Always, RuleEffect::Deny),
        ]);
        let seen = observation(Some(b"src/lib.rs"));
        let mut scratch = policy.new_match_buffer();
        let decision = evaluate_v2(
            &policy,
            &seen,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .expect("deny wins");
        assert_eq!(decision.outcome(), AdmissionOutcome::Deny);
        assert!(
            decision
                .reason_codes()
                .contains(&AdmissionReasonCode::DenyExplicitRule)
        );
        assert!(
            !decision
                .reason_codes()
                .contains(&AdmissionReasonCode::AllowExplicitRule)
        );
    }

    // AUTHORED_NOT_RUN. No applicable allow ever means DenyByDefault, never a
    // silent allow. Guards the v1 baseline-allow affordance staying out of v2.
    #[test]
    fn no_applicable_allow_is_deny_by_default() {
        let review_policy = policy(vec![rule(
            "review-only",
            10,
            PredicateV2::Always,
            RuleEffect::Review,
        )]);
        let seen = observation(Some(b"src/lib.rs"));
        let mut scratch = review_policy.new_match_buffer();
        let decision = evaluate_v2(
            &review_policy,
            &seen,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .expect("review outcome");
        assert_eq!(decision.outcome(), AdmissionOutcome::ReviewRequired);
        assert_eq!(
            decision.reason_codes(),
            &[AdmissionReasonCode::ReviewExplicitRule]
        );
        let empty = policy(Vec::new());
        let mut scratch = empty.new_match_buffer();
        let silent = evaluate_v2(
            &empty,
            &seen,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .expect("empty policy denies by default");
        assert_eq!(silent.outcome(), AdmissionOutcome::Deny);
        assert_eq!(silent.reason_codes(), &[AdmissionReasonCode::DenyByDefault]);
        assert!(silent.matched_rule_ids().is_empty());
    }

    fn custom_policy(
        section: ValidatedAdmissionConfig,
        revision: u64,
        rules: Vec<RuleInputV2>,
    ) -> CompiledAdmissionPolicyV2 {
        compile_policy_v2(
            &PolicyInputV2 {
                schema_version: 2,
                revision,
                section,
                rules,
                unknown_fields: Vec::new(),
            },
            DEFAULT_COMPILE_LIMITS,
            &mut || Ok(()),
        )
        .unwrap()
    }

    fn run(
        policy: &CompiledAdmissionPolicyV2,
        observation: &AdmissionObservationV2,
        limits: EvaluationLimitsV2,
    ) -> Result<AdmissionDecisionV2, AdmissionError> {
        evaluate_v2(
            policy,
            observation,
            &mut policy.new_match_buffer(),
            limits,
            &mut || Ok(()),
        )
    }

    #[test]
    fn every_predicate_is_class_scoped_and_nonpath_predicates_are_effective() {
        let observed = observation(Some(b"src/lib.rs"));
        for predicate in [
            PredicateV2::Always,
            PredicateV2::SourceClassIs,
            PredicateV2::SensitivityAtLeast(SensitivityLevel::Public),
            PredicateV2::SizeGreaterThan(1),
            PredicateV2::LocatorClassIs(crate::LocatorClass::NormalizedFile),
            PredicateV2::RelativePathGlobMatches("src/*.rs".into()),
        ] {
            let applicable = rule("applicable", 1, predicate.clone(), RuleEffect::Allow);
            let mut wrong_class = applicable.clone();
            wrong_class.source_class = SourceClass::Vendor;
            let right = run(
                &policy(vec![applicable]),
                &observed,
                DEFAULT_EVALUATION_LIMITS,
            )
            .unwrap();
            let wrong = run(
                &policy(vec![wrong_class]),
                &observed,
                DEFAULT_EVALUATION_LIMITS,
            )
            .unwrap();
            assert_eq!(right.outcome(), AdmissionOutcome::Allow);
            assert_eq!(wrong.outcome(), AdmissionOutcome::Deny);
            assert!(wrong.matched_rule_ids().is_empty());
        }
    }

    #[test]
    fn every_safety_fence_survives_an_explicit_regular_allow() {
        use AdmissionReasonCode as R;
        let policy = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        for scenario in 0..12 {
            let mut raw = raw_observation();
            let reason = match scenario {
                0 => {
                    raw.source_class = "credential".into();
                    R::DenyCredentialClass
                }
                1 => {
                    raw.source_class = "private-key".into();
                    R::DenyPrivateKeyClass
                }
                2 => {
                    raw.source_class = "system".into();
                    R::DenySystemLocation
                }
                3 => {
                    raw.source_class = "cache".into();
                    R::DenyCacheArtifact
                }
                4 => {
                    raw.source_class = "build-output".into();
                    R::DenyBuildOutput
                }
                5 => {
                    raw.sensitivity = Some("credential".into());
                    R::DenySensitiveCredential
                }
                6 => {
                    raw.sensitivity = Some("private-key".into());
                    R::DenySensitiveCredential
                }
                7 => {
                    raw.sensitivity = Some("secret-candidate".into());
                    R::DenySensitiveClass
                }
                8 => {
                    raw.location_class = "remote".into();
                    R::DenyRemoteLocation
                }
                9 => {
                    raw.byte_size = Some(0);
                    R::DenyEmptySource
                }
                10 => {
                    raw.byte_size = Some(16_777_217);
                    R::DenySizeExceeded
                }
                _ => {
                    raw.is_system = Some(true);
                    R::DenySystemLocation
                }
            };
            let decision = run(
                &policy,
                &model(&raw, Some(b"src/lib.rs")),
                DEFAULT_EVALUATION_LIMITS,
            )
            .unwrap();
            assert_eq!(decision.outcome(), AdmissionOutcome::Deny);
            assert!(decision.reason_codes().contains(&reason));
            assert!(!decision.reason_codes().contains(&R::AllowExplicitRule));
        }
    }

    #[test]
    fn class_or_positive_flag_permissions_remove_denial_without_minting_allow() {
        for flag in ["generated", "vendor", "binary"] {
            for class_signal in [false, true] {
                let mut raw = raw_observation();
                if class_signal {
                    raw.source_class = flag.into();
                } else {
                    match flag {
                        "generated" => raw.is_generated = Some(true),
                        "vendor" => raw.is_vendor = Some(true),
                        _ => raw.is_binary = Some(true),
                    }
                }
                let mut allow = rule("allow", 1, PredicateV2::Always, RuleEffect::Allow);
                allow.source_class = SourceClass::parse(&raw.source_class).unwrap();
                let observed = model(&raw, Some(b"src/lib.rs"));
                let restrictive = custom_policy(section(), 1, vec![allow.clone()]);
                let explicit = custom_policy(permissive(flag), 1, vec![allow]);
                let silent = custom_policy(permissive(flag), 1, Vec::new());
                assert_eq!(
                    run(&restrictive, &observed, DEFAULT_EVALUATION_LIMITS)
                        .unwrap()
                        .outcome(),
                    AdmissionOutcome::Deny
                );
                assert_eq!(
                    run(&explicit, &observed, DEFAULT_EVALUATION_LIMITS)
                        .unwrap()
                        .outcome(),
                    AdmissionOutcome::Allow
                );
                assert_eq!(
                    run(&silent, &observed, DEFAULT_EVALUATION_LIMITS)
                        .unwrap()
                        .reason_codes(),
                    &[AdmissionReasonCode::DenyByDefault]
                );
            }
        }
        let mut raw = raw_observation();
        raw.sensitivity = Some("confidential".into());
        let observed = model(&raw, Some(b"src/lib.rs"));
        let explicit = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        assert_eq!(
            run(&explicit, &observed, DEFAULT_EVALUATION_LIMITS)
                .unwrap()
                .outcome(),
            AdmissionOutcome::Allow
        );
        assert_eq!(
            run(&policy(Vec::new()), &observed, DEFAULT_EVALUATION_LIMITS)
                .unwrap()
                .outcome(),
            AdmissionOutcome::Deny
        );
    }

    #[test]
    fn each_missing_signal_requires_review_and_never_allow() {
        let policy = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        for field in crate::CLOSED_UNAVAILABLE_FIELDS {
            let mut raw = raw_observation();
            match field {
                "byte-size" => raw.byte_size = None,
                "sensitivity" => raw.sensitivity = None,
                "detector-id" => raw.detector_id = None,
                "profile-id" => raw.profile_id = None,
                "generated-flag" => raw.is_generated = None,
                "vendor-flag" => raw.is_vendor = None,
                "binary-flag" => raw.is_binary = None,
                "system-flag" => raw.is_system = None,
                _ => unreachable!(),
            }
            raw.unavailable_fields.push(field.into());
            let decision = run(
                &policy,
                &model(&raw, Some(b"src/lib.rs")),
                DEFAULT_EVALUATION_LIMITS,
            )
            .unwrap();
            assert_eq!(decision.outcome(), AdmissionOutcome::ReviewRequired);
            assert_eq!(decision.matched_rule_ids(), &["allow"]);
            let reason = match field {
                "detector-id" => AdmissionReasonCode::ReviewDetectorUnavailable,
                "sensitivity" => AdmissionReasonCode::ReviewSensitivityUnknown,
                _ => AdmissionReasonCode::ReviewMetadataUnavailable,
            };
            assert_eq!(decision.reason_codes(), &[reason]);
        }
    }

    #[test]
    fn full_outcome_lattice_keeps_all_ids_but_only_winning_reasons() {
        let observed = observation(Some(b"src/lib.rs"));
        let effects = [
            RuleEffect::Allow,
            RuleEffect::Review,
            RuleEffect::Unsupported,
            RuleEffect::Deny,
        ];
        for count in 1..=4 {
            let rules = effects[..count]
                .iter()
                .enumerate()
                .map(|(i, effect)| {
                    rule(
                        &format!("r{i}"),
                        u32::try_from(i + 1).unwrap(),
                        PredicateV2::Always,
                        *effect,
                    )
                })
                .collect();
            let decision = run(&policy(rules), &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
            assert_eq!(
                decision.outcome(),
                [
                    AdmissionOutcome::Allow,
                    AdmissionOutcome::ReviewRequired,
                    AdmissionOutcome::Unsupported,
                    AdmissionOutcome::Deny
                ][count - 1]
            );
            assert_eq!(
                decision.reason_codes(),
                &[[
                    AdmissionReasonCode::AllowExplicitRule,
                    AdmissionReasonCode::ReviewExplicitRule,
                    AdmissionReasonCode::UnsupportedExplicitRule,
                    AdmissionReasonCode::DenyExplicitRule
                ][count - 1]]
            );
            assert_eq!(
                decision.matched_rule_ids(),
                &(0..count).map(|i| format!("r{i}")).collect::<Vec<_>>()
            );
        }
        let mut raw = raw_observation();
        raw.source_kind = "device".into();
        let allow = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        let unsupported = run(
            &allow,
            &model(&raw, Some(b"src/lib.rs")),
            DEFAULT_EVALUATION_LIMITS,
        )
        .unwrap();
        assert_eq!(unsupported.outcome(), AdmissionOutcome::Unsupported);
        assert_eq!(
            unsupported.reason_codes(),
            &[AdmissionReasonCode::UnsupportedSourceKind]
        );
    }

    #[test]
    fn contribution_budget_precedes_filtering_and_duplicates_charge_once() {
        let observed = observation(Some(b"src/lib.rs"));
        let mixed = policy(vec![
            rule("allow", 1, PredicateV2::Always, RuleEffect::Allow),
            rule("deny", 2, PredicateV2::Always, RuleEffect::Deny),
        ]);
        let mut limits = DEFAULT_EVALUATION_LIMITS;
        limits.max_reasons = 1;
        assert_eq!(
            run(&mixed, &observed, limits),
            Err(AdmissionError::BudgetExhausted)
        );
        limits.max_reasons = 2;
        assert_eq!(
            run(&mixed, &observed, limits).unwrap().reason_codes(),
            &[AdmissionReasonCode::DenyExplicitRule]
        );
        let duplicate = policy(vec![
            rule("a", 1, PredicateV2::Always, RuleEffect::Allow),
            rule("b", 2, PredicateV2::Always, RuleEffect::Allow),
        ]);
        limits.max_reasons = 1;
        assert_eq!(
            run(&duplicate, &observed, limits)
                .unwrap()
                .matched_rule_ids(),
            &["a", "b"]
        );
        let mut raw = raw_observation();
        raw.is_system = Some(true);
        assert_eq!(
            run(&duplicate, &model(&raw, Some(b"src/lib.rs")), limits),
            Err(AdmissionError::BudgetExhausted)
        );
    }

    #[test]
    fn raw_limits_rule_work_and_matched_output_refuse_without_truncation() {
        let observed = observation(Some(b"src/lib.rs"));
        let policy = policy(vec![
            rule("a", 1, PredicateV2::Always, RuleEffect::Allow),
            rule("b", 2, PredicateV2::Always, RuleEffect::Allow),
        ]);
        for dimension in 0..5 {
            for above_max in [false, true] {
                let mut limits = DEFAULT_EVALUATION_LIMITS;
                let value = if above_max {
                    [129, 129, 24, 65_537, 16_385][dimension]
                } else {
                    0
                };
                match dimension {
                    0 => limits.max_rule_evaluations = value,
                    1 => limits.max_matched_rule_ids = value,
                    2 => limits.max_reasons = value,
                    3 => limits.max_encoded_bytes = value,
                    _ => limits.max_observation_encoded_bytes = value,
                }
                assert_eq!(
                    run(&policy, &observed, limits),
                    Err(AdmissionError::ObservationInvalid)
                );
            }
        }
        let mut limits = DEFAULT_EVALUATION_LIMITS;
        limits.max_rule_evaluations = 1;
        assert_eq!(
            run(&policy, &observed, limits),
            Err(AdmissionError::BudgetExhausted)
        );
        limits.max_rule_evaluations = 2;
        limits.max_matched_rule_ids = 1;
        assert_eq!(
            run(&policy, &observed, limits),
            Err(AdmissionError::BudgetExhausted)
        );
        limits.max_matched_rule_ids = 2;
        assert_eq!(
            run(&policy, &observed, limits).unwrap().matched_rule_ids(),
            &["a", "b"]
        );
    }

    #[test]
    fn original_checkpoint_envelope_clears_every_pre_and_post_match_failure() {
        let policy = policy(vec![
            rule(
                "glob",
                1,
                PredicateV2::RelativePathGlobMatches("src/**".into()),
                RuleEffect::Allow,
            ),
            rule(
                "second",
                2,
                PredicateV2::RelativePathGlobMatches("src/*.rs".into()),
                RuleEffect::Allow,
            ),
        ]);
        let observed = observation(Some(b"src/lib.rs"));
        let mut scratch = policy.new_match_buffer();
        let (mut probe, seen) = recording_probe();
        evaluate_v2(
            &policy,
            &observed,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut probe,
        )
        .unwrap();
        let total = seen.borrow().len();
        assert_eq!(total, 12); // entry + observation2 + matcher4 + rules2 + hash/return3
        for fail_at in 1..=total {
            evaluate_v2(
                &policy,
                &observed,
                &mut scratch,
                DEFAULT_EVALUATION_LIMITS,
                &mut || Ok(()),
            )
            .unwrap();
            assert!(format!("{scratch:?}").contains("matches: 2"));
            assert_eq!(
                evaluate_v2(
                    &policy,
                    &observed,
                    &mut scratch,
                    DEFAULT_EVALUATION_LIMITS,
                    &mut probe_failing_at(fail_at)
                ),
                Err(AdmissionError::BudgetExhausted)
            );
            assert!(format!("{scratch:?}").contains("matches: 0"));
            let mut calls = 0;
            assert_eq!(
                evaluate_v2(
                    &policy,
                    &observed,
                    &mut scratch,
                    DEFAULT_EVALUATION_LIMITS,
                    &mut || {
                        calls += 1;
                        if calls == fail_at {
                            Err(CheckpointFailureV2::Cancelled)
                        } else {
                            Ok(())
                        }
                    }
                ),
                Err(AdmissionError::Cancelled)
            );
            assert_eq!(calls, fail_at);
            assert!(format!("{scratch:?}").contains("matches: 0"));
        }
        let mut invalid = DEFAULT_EVALUATION_LIMITS;
        invalid.max_reasons = 0;
        assert_eq!(
            evaluate_v2(&policy, &observed, &mut scratch, invalid, &mut || Err(
                CheckpointFailureV2::Cancelled
            )),
            Err(AdmissionError::Cancelled)
        );
        for limits in [
            invalid,
            EvaluationLimitsV2 {
                max_rule_evaluations: 1,
                ..DEFAULT_EVALUATION_LIMITS
            },
            EvaluationLimitsV2 {
                max_matched_rule_ids: 1,
                ..DEFAULT_EVALUATION_LIMITS
            },
            EvaluationLimitsV2 {
                max_encoded_bytes: 1,
                ..DEFAULT_EVALUATION_LIMITS
            },
            EvaluationLimitsV2 {
                max_observation_encoded_bytes: 1,
                ..DEFAULT_EVALUATION_LIMITS
            },
        ] {
            evaluate_v2(
                &policy,
                &observed,
                &mut scratch,
                DEFAULT_EVALUATION_LIMITS,
                &mut || Ok(()),
            )
            .unwrap();
            assert!(evaluate_v2(&policy, &observed, &mut scratch, limits, &mut || Ok(())).is_err());
            assert!(format!("{scratch:?}").contains("matches: 0"));
        }
    }

    #[test]
    fn cross_policy_scratch_and_whole_call_missing_locator_remain_closed() {
        let original = policy(vec![rule(
            "a",
            1,
            PredicateV2::RelativePathGlobMatches("src/**".into()),
            RuleEffect::Allow,
        )]);
        let other = policy(vec![rule("b", 1, PredicateV2::Always, RuleEffect::Allow)]);
        let observed = observation(Some(b"src/lib.rs"));
        let mut scratch = original.new_match_buffer();
        evaluate_v2(
            &original,
            &observed,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(
            evaluate_v2(
                &other,
                &observed,
                &mut scratch,
                DEFAULT_EVALUATION_LIMITS,
                &mut || Ok(())
            ),
            Err(AdmissionError::PolicyConflict)
        );
        assert!(format!("{scratch:?}").contains("matches: 0"));
        evaluate_v2(
            &original,
            &observed,
            &mut scratch,
            DEFAULT_EVALUATION_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(
            evaluate_v2(
                &original,
                &observation(None),
                &mut scratch,
                DEFAULT_EVALUATION_LIMITS,
                &mut || Ok(())
            ),
            Err(AdmissionError::ObservationIncomplete)
        );
        assert!(format!("{scratch:?}").contains("matches: 0"));
        for policy in [original, other, policy(Vec::new())] {
            assert_eq!(
                run(&policy, &observation(None), DEFAULT_EVALUATION_LIMITS),
                Err(AdmissionError::ObservationIncomplete)
            );
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn independent_eight_row_golden_and_complete_byte_boundaries_preserve_identity() {
        use search_contracts::{blake3_raw, to_canonical_cbor};
        let policy = custom_policy(section(), 1, Vec::new());
        let mut raw = raw_observation();
        raw.byte_size = Some(10);
        raw.detector_id = Some("detector:baseline-v1".into());
        let observed = model(&raw, Some(b"src/lib.rs"));
        let decision = run(&policy, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let data = DecisionData {
            policy: &policy,
            observation: &observed,
            outcome: decision.outcome(),
            reasons: decision.reason_codes(),
            matched_ids: decision.matched_rule_ids(),
            sensitivity: decision.sensitivity(),
        };
        let rows = (0..8)
            .map(|index| decision_row(&data, index).unwrap())
            .collect();
        let cbor = to_canonical_cbor(&array(rows).unwrap()).unwrap();
        // Independent .NET descriptor with real accepted identity bytes in its
        // two32-byte slots: 144 CBOR +32 domain +NUL =177 complete bytes.
        let expected = format!(
            "888702781b656c696f742d61646d697373696f6e2d656666656374697665403218801880171a00010000194000015820{}5820{}6444454e59816f44454e595f42595f44454641554c5480667075626c6963",
            hex(policy.fingerprint().as_bytes()),
            hex(observed.fingerprint().as_bytes())
        );
        assert_eq!(cbor.as_slice().len(), 144);
        assert_eq!(hex(cbor.as_slice()), expected);
        assert_eq!(
            blake3_raw(
                &CanonicalDigestDomain::parse(DECISION_DIGEST_DOMAIN).unwrap(),
                cbor.as_slice(),
                DigestInputLimit::new(177).unwrap()
            )
            .unwrap()
            .as_bytes(),
            decision.fingerprint().as_bytes()
        );
        let exact = EvaluationLimitsV2 {
            max_rule_evaluations: 1,
            max_matched_rule_ids: 1,
            max_reasons: 1,
            max_encoded_bytes: 177,
            max_observation_encoded_bytes: 194,
        };
        assert_eq!(run(&policy, &observed, exact).unwrap(), decision);
        assert_eq!(
            run(
                &policy,
                &observed,
                EvaluationLimitsV2 {
                    max_encoded_bytes: 176,
                    ..exact
                }
            ),
            Err(AdmissionError::BudgetExhausted)
        );
        assert_eq!(
            run(
                &policy,
                &observed,
                EvaluationLimitsV2 {
                    max_observation_encoded_bytes: 193,
                    ..exact
                }
            ),
            Err(AdmissionError::BudgetExhausted)
        );
        for ceiling in [32, 33] {
            assert_eq!(
                run(
                    &policy,
                    &observed,
                    EvaluationLimitsV2 {
                        max_encoded_bytes: ceiling,
                        ..exact
                    }
                ),
                Err(AdmissionError::BudgetExhausted)
            );
        }
    }

    #[test]
    fn identity_binds_policy_observation_outcome_reasons_ids_and_sensitivity_and_debug_is_private()
    {
        let policy = custom_policy(
            section(),
            1,
            vec![rule(
                "private-rule-id",
                1,
                PredicateV2::RelativePathGlobMatches("src/*.rs".into()),
                RuleEffect::Allow,
            )],
        );
        let observed = observation(Some(b"src/private-locator.rs"));
        let decision = run(&policy, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        assert_eq!(decision.policy_revision(), policy.revision());
        assert_eq!(decision.policy_fingerprint(), policy.fingerprint());
        assert_eq!(decision.observation_fingerprint(), observed.fingerprint());
        let debug = format!("{decision:?}");
        for private in [
            "private-rule-id",
            "src/",
            "detector:fixture",
            "baseline-safe-v1",
        ] {
            assert!(!debug.contains(private));
        }
        let other_revision = custom_policy(
            section(),
            2,
            vec![rule(
                "private-rule-id",
                1,
                PredicateV2::RelativePathGlobMatches("src/*.rs".into()),
                RuleEffect::Allow,
            )],
        );
        assert_ne!(
            decision.fingerprint(),
            run(&other_revision, &observed, DEFAULT_EVALUATION_LIMITS)
                .unwrap()
                .fingerprint()
        );
        assert_ne!(
            decision.fingerprint(),
            run(
                &policy,
                &observation(Some(b"src/other.rs")),
                DEFAULT_EVALUATION_LIMITS
            )
            .unwrap()
            .fingerprint()
        );
        let winning = [AdmissionReasonCode::AllowExplicitRule];
        let changed_reason = [AdmissionReasonCode::ReviewExplicitRule];
        let ids = vec!["private-rule-id".to_owned()];
        let changed_ids = vec!["different-id".to_owned()];
        let data = DecisionData {
            policy: &policy,
            observation: &observed,
            outcome: AdmissionOutcome::Allow,
            reasons: &winning,
            matched_ids: &ids,
            sensitivity: SensitivityLevel::Public,
        };
        let baseline = compute_decision_fingerprint(&data, MAX_DECISION_PREIMAGE_BYTES).unwrap();
        for changed in [
            DecisionData {
                outcome: AdmissionOutcome::Deny,
                ..data
            },
            DecisionData {
                reasons: &changed_reason,
                ..data
            },
            DecisionData {
                matched_ids: &changed_ids,
                ..data
            },
            DecisionData {
                sensitivity: SensitivityLevel::Confidential,
                ..data
            },
        ] {
            assert_ne!(
                baseline,
                compute_decision_fingerprint(&changed, MAX_DECISION_PREIMAGE_BYTES).unwrap()
            );
        }
    }

    #[test]
    fn only_shared_preimage_or_sink_oversize_is_a_budget_failure() {
        for field in [
            "digest_preimage",
            "canonical_sink",
            "canonical_text",
            "decision_row",
        ] {
            for kind in [
                ContractErrorKind::OversizePayload,
                ContractErrorKind::InvalidRange,
            ] {
                let expected = if kind == ContractErrorKind::OversizePayload
                    && matches!(field, "digest_preimage" | "canonical_sink")
                {
                    AdmissionError::BudgetExhausted
                } else {
                    AdmissionError::ObservationInvalid
                };
                assert_eq!(
                    encoded_limit_error(ContractError::new(kind, field)),
                    expected
                );
            }
        }
    }
}
