//! Immutable admission receipt over one producer-derived v2 decision.
//!
//! The original caller owns the deadline and cancellation probe. This module binds
//! identities only: no native path, root registration, source revision, access or
//! currentness proof is created here, and no v2-to-v1 fallback is supplied. Source-only
//! receipt provenance comes solely from `evaluation_v2::evaluate_v2`.

use core::fmt;

use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue, ContractError, ContractErrorKind, DigestInputLimit, blake3_canonical_array,
};

use search_source_admission::AdmissionError;
use search_source_admission::evaluation_v2::{
    AdmissionDecisionV2, MAX_MATCHED_RULE_IDS, MAX_REASONS,
};
use search_source_admission::observation_v2::{
    AdmissionObservationV2, MAX_OBSERVATION_PREIMAGE_BYTES,
};
use search_source_admission::policy_v2::{CheckpointFailureV2, CompiledAdmissionPolicyV2};

// Preserve the original caller-owned checkpoint and closed failure translation.
fn checkpoint(
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<(), AdmissionError> {
    probe().map_err(|failure| match failure {
        CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
        CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
    })
}

/// Distinct receipt schema; the retained v1 receipt schema is unchanged.
pub const RECEIPT_SCHEMA_VERSION: u32 = 2;
/// Fixed receipt semantics and equality profile.
pub const RECEIPT_PROFILE: &str = "eliot-admission-receipt@2";
/// Complete domain-plus-CBOR receipt preimage domain.
pub const RECEIPT_DIGEST_DOMAIN: &str = "eliot/cbor/admission/receipt/v2";
/// Fixed complete receipt preimage byte ceiling.
pub const MAX_RECEIPT_PREIMAGE_BYTES: usize = 4_096;

const RECEIPT_ROWS: usize = 5;

/// Raw finite receipt ceilings; validated by every issuer and verifier call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiptLimitsV2 {
    /// Maximum complete receipt domain-plus-CBOR preimage bytes.
    pub max_encoded_bytes: usize,
    /// Maximum complete existing observation preimage bytes.
    pub max_observation_encoded_bytes: usize,
    /// Maximum reason count, checked before receipt-owned variable copies.
    pub max_reasons: usize,
    /// Maximum matched rule IDs, checked before receipt-owned variable copies.
    pub max_matched_rule_ids: usize,
}

impl ReceiptLimitsV2 {
    fn validate(self) -> Result<Self, AdmissionError> {
        for (value, maximum) in [
            (self.max_encoded_bytes, MAX_RECEIPT_PREIMAGE_BYTES),
            (
                self.max_observation_encoded_bytes,
                MAX_OBSERVATION_PREIMAGE_BYTES,
            ),
            (self.max_reasons, MAX_REASONS),
            (self.max_matched_rule_ids, MAX_MATCHED_RULE_IDS),
        ] {
            if value == 0 || value > maximum {
                return Err(AdmissionError::ObservationInvalid);
            }
        }
        Ok(self)
    }
}

/// Conservative fixed ceilings; not a measured resource claim.
pub const DEFAULT_RECEIPT_LIMITS: ReceiptLimitsV2 = ReceiptLimitsV2 {
    max_encoded_bytes: MAX_RECEIPT_PREIMAGE_BYTES,
    max_observation_encoded_bytes: MAX_OBSERVATION_PREIMAGE_BYTES,
    max_reasons: MAX_REASONS,
    max_matched_rule_ids: MAX_MATCHED_RULE_IDS,
};

/// Internally computed receipt equality bytes, never supplied proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiptFingerprintV2(Blake3Digest32);

impl ReceiptFingerprintV2 {
    /// Complete shared canonical CBOR/BLAKE3 identity bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

/// One immutable receipt binding the trusted decision to its exact owners.
///
/// The decision is embedded, never copied into a second shape, so a reader cannot
/// obtain a differing result view. `schema_version` has no public setter and is
/// writable only by the issuer, which refuses an unsupported schema by construction.
#[derive(Clone, Eq, PartialEq)]
pub struct AdmissionReceiptV2 {
    schema_version: u32,
    decision: AdmissionDecisionV2,
    fingerprint: ReceiptFingerprintV2,
}

impl AdmissionReceiptV2 {
    /// Frozen receipt schema.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Trusted immutable decision this receipt binds; no second result shape exists.
    #[must_use]
    pub const fn decision(&self) -> &AdmissionDecisionV2 {
        &self.decision
    }

    /// Internally computed complete receipt identity.
    #[must_use]
    pub const fn fingerprint(&self) -> ReceiptFingerprintV2 {
        self.fingerprint
    }
}

impl fmt::Debug for AdmissionReceiptV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Reason values, IDs, observation data and locator bytes stay absent.
        formatter
            .debug_struct("AdmissionReceiptV2")
            .field("schema", &self.schema_version)
            .field("outcome", &self.decision.outcome())
            .field("reasons", &self.decision.reason_codes().len())
            .field("matched_rules", &self.decision.matched_rule_ids().len())
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for VerifiedAdmissionReceiptV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("VerifiedAdmissionReceiptV2")
            .field(&self.receipt)
            .finish()
    }
}

/// Verified receipt accepted for one exact current policy and observation fence.
///
/// No public constructor exists. A verified receipt cannot be built from supplied
/// bytes or a supplied digest, so no codec or reader can mint trusted status.
#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedAdmissionReceiptV2 {
    receipt: AdmissionReceiptV2,
}

impl VerifiedAdmissionReceiptV2 {
    /// Accepted receipt.
    #[must_use]
    pub const fn receipt(&self) -> &AdmissionReceiptV2 {
        &self.receipt
    }
}

/// Issues one receipt binding a trusted decision to its exact policy and observation.
///
/// Fencing, counts and complete preimage charging precede the decision clone. Bounded
/// observation/canonical row copies may occur before sink charging. The decision is issued
/// solely by `evaluation_v2::evaluate_v2`; nothing here re-decides or relaxes it.
///
/// # Errors
/// Invalid raw ceilings return `ObservationInvalid`; identity disagreement returns
/// `ReceiptMismatch`; actual preimage or observation exhaustion returns `BudgetExhausted`;
/// checkpoint failures return `Cancelled`/`BudgetExhausted`.
pub fn issue_receipt_v2(
    policy: &CompiledAdmissionPolicyV2,
    observation: &AdmissionObservationV2,
    decision: &AdmissionDecisionV2,
    limits: ReceiptLimitsV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionReceiptV2, AdmissionError> {
    checkpoint(probe)?;
    let limits = limits.validate()?;
    // Exact identity fence. The trusted decision must belong to this policy and
    // this observation; receipt issuance never creates or widens that relation.
    if decision.policy_revision() != policy.revision()
        || decision.policy_fingerprint() != policy.fingerprint()
        || decision.observation_fingerprint() != observation.fingerprint()
    {
        return Err(AdmissionError::ReceiptMismatch);
    }
    guard_counts(decision, limits)?;
    checkpoint(probe)?;
    // Readonly revalidation of the complete observation preimage under the caller ceiling.
    observation.validate_encoded_limit(limits.max_observation_encoded_bytes)?;
    checkpoint(probe)?;
    let fingerprint = ReceiptFingerprintV2(
        compute_receipt_fingerprint(decision, limits.max_encoded_bytes)
            .map_err(encoded_limit_error)?,
    );
    checkpoint(probe)?;
    let receipt = AdmissionReceiptV2 {
        schema_version: RECEIPT_SCHEMA_VERSION,
        decision: decision.clone(),
        fingerprint,
    };
    checkpoint(probe)?;
    Ok(receipt)
}

/// Verifies one receipt against the current policy and observation fence.
///
/// Verification uses the stored trusted decision directly; it performs no reevaluation,
/// rebuilds no baseline deny set, and accepts no separately supplied decision, digest
/// or trusted wrapper. Only the current-policy comparison precedes observation matching,
/// so a stale fence is reported before an unrelated observation mismatch.
///
/// # Errors
/// `PolicySchemaUnsupported` for a wrong schema, `ReceiptStale` for a stale policy fence,
/// `ReceiptMismatch` for altered identities, `ObservationInvalid` for invalid raw ceilings,
/// `BudgetExhausted` for count or complete-byte
/// actual exhaustion, and `Cancelled`/`BudgetExhausted` for checkpoint failures.
pub fn verify_receipt_v2(
    receipt: &AdmissionReceiptV2,
    current_policy: &CompiledAdmissionPolicyV2,
    observation: &AdmissionObservationV2,
    limits: ReceiptLimitsV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<VerifiedAdmissionReceiptV2, AdmissionError> {
    checkpoint(probe)?;
    let limits = limits.validate()?;
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    let decision = receipt.decision();
    // Stale policy fence is decided before any observation comparison.
    if decision.policy_revision() != current_policy.revision()
        || decision.policy_fingerprint() != current_policy.fingerprint()
    {
        return Err(AdmissionError::ReceiptStale);
    }
    if decision.observation_fingerprint() != observation.fingerprint() {
        return Err(AdmissionError::ReceiptMismatch);
    }
    guard_counts(decision, limits)?;
    checkpoint(probe)?;
    observation.validate_encoded_limit(limits.max_observation_encoded_bytes)?;
    checkpoint(probe)?;
    let recomputed = ReceiptFingerprintV2(
        compute_receipt_fingerprint(decision, limits.max_encoded_bytes)
            .map_err(encoded_limit_error)?,
    );
    if recomputed != receipt.fingerprint() {
        return Err(AdmissionError::ReceiptMismatch);
    }
    checkpoint(probe)?;
    let verified = VerifiedAdmissionReceiptV2 {
        receipt: receipt.clone(),
    };
    checkpoint(probe)?;
    Ok(verified)
}

fn guard_counts(
    decision: &AdmissionDecisionV2,
    limits: ReceiptLimitsV2,
) -> Result<(), AdmissionError> {
    if decision.reason_codes().len() > limits.max_reasons
        || decision.matched_rule_ids().len() > limits.max_matched_rule_ids
    {
        return Err(AdmissionError::BudgetExhausted);
    }
    Ok(())
}

fn compute_receipt_fingerprint(
    decision: &AdmissionDecisionV2,
    max_encoded_bytes: usize,
) -> Result<Blake3Digest32, ContractError> {
    let domain = CanonicalDigestDomain::parse(RECEIPT_DIGEST_DOMAIN)?;
    blake3_canonical_array(
        &domain,
        RECEIPT_ROWS,
        (0..RECEIPT_ROWS).map(|index| receipt_row(decision, index)),
        DigestInputLimit::new(max_encoded_bytes)?,
    )
}

fn receipt_row(
    decision: &AdmissionDecisionV2,
    index: usize,
) -> Result<CanonicalValue, ContractError> {
    match index {
        0 => array(vec![
            CanonicalValue::U64(u64::from(RECEIPT_SCHEMA_VERSION)),
            text(RECEIPT_PROFILE)?,
            CanonicalValue::U64(4_096),
            CanonicalValue::U64(16_384),
            CanonicalValue::U64(23),
            CanonicalValue::U64(128),
        ]),
        1 => Ok(CanonicalValue::U64(decision.policy_revision().get())),
        2 => BoundedBytes::new(decision.policy_fingerprint().as_bytes().to_vec())
            .map(CanonicalValue::Bytes),
        3 => BoundedBytes::new(decision.observation_fingerprint().as_bytes().to_vec())
            .map(CanonicalValue::Bytes),
        4 => {
            BoundedBytes::new(decision.fingerprint().as_bytes().to_vec()).map(CanonicalValue::Bytes)
        }
        _ => Err(ContractError::new(
            ContractErrorKind::InvalidRange,
            "admission_receipt_v2_row",
        )),
    }
}

fn text(value: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(value).map(CanonicalValue::Text)
}

fn array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    BoundedList::new(values).map(CanonicalValue::Array)
}

/// Receipt byte exhaustion maps like the accepted observation contract, not the policy
/// one: only complete-preimage and sink overflow are budget failures.
fn encoded_limit_error(error: ContractError) -> AdmissionError {
    if error.kind() == ContractErrorKind::OversizePayload
        && matches!(error.field(), "digest_preimage" | "canonical_sink")
    {
        AdmissionError::BudgetExhausted
    } else {
        AdmissionError::ObservationInvalid
    }
}

// Private causal fixtures: AUTHORED_NOT_RUN under #352/#241/#405/#215.
#[cfg(test)]
mod tests {
    use super::*;
    use search_source_admission::evaluation_v2::{
        DEFAULT_EVALUATION_LIMITS, EvaluationLimitsV2, evaluate_v2,
    };
    use search_source_admission::observation_v2::AdmissionRelativeLocator;
    use search_source_admission::policy_v2::{
        DEFAULT_COMPILE_LIMITS, PolicyInputV2, PredicateV2, RuleInputV2, compile_policy_v2,
    };
    use search_source_admission::{
        AcceptedCapabilities, DEFAULT_ADMISSION_LIMITS, RuleEffect, SectionPlatform, SourceClass,
        UnvalidatedObservationInput, ValidatedAdmissionConfig, baseline_policy, compiled_defaults,
        validate_observation, validate_section,
    };

    fn section() -> ValidatedAdmissionConfig {
        let raw = compiled_defaults();
        validate_section(
            &raw,
            &SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            &AcceptedCapabilities {
                profiles: vec![raw.bootstrap_policy_profile.clone()],
            },
        )
        .unwrap()
    }

    fn rule(id: &str, precedence: u32, predicate: PredicateV2, effect: RuleEffect) -> RuleInputV2 {
        RuleInputV2 {
            id: id.into(),
            precedence,
            source_class: SourceClass::Regular,
            predicate,
            effect,
            unknown_fields: Vec::new(),
        }
    }

    fn custom_policy(
        section: ValidatedAdmissionConfig,
        revision: u64,
        rules: Vec<RuleInputV2>,
    ) -> CompiledAdmissionPolicyV2 {
        compile_policy_v2(
            &PolicyInputV2 {
                schema_version: search_source_admission::policy_v2::POLICY_SCHEMA_VERSION,
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

    fn policy(rules: Vec<RuleInputV2>) -> CompiledAdmissionPolicyV2 {
        custom_policy(section(), 7, rules)
    }

    fn raw_observation() -> UnvalidatedObservationInput {
        UnvalidatedObservationInput {
            locator_class: "normalized-file".into(),
            location_class: "local-fixed".into(),
            source_kind: "file".into(),
            source_class: "regular".into(),
            byte_size: Some(64),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".into()),
            detector_id: Some("detector:fixture".into()),
            profile_id: Some(search_source_admission::BASELINE_PROFILE.into()),
            unavailable_fields: Vec::new(),
            unknown_fields: Vec::new(),
        }
    }

    fn model(raw: &UnvalidatedObservationInput, path: Option<&[u8]>) -> AdmissionObservationV2 {
        let metadata = validate_observation(
            raw,
            &baseline_policy(3, 16_777_216),
            DEFAULT_ADMISSION_LIMITS,
        )
        .unwrap();
        AdmissionObservationV2::new(
            metadata,
            path.map(|p| AdmissionRelativeLocator::from_bytes(p).unwrap()),
        )
        .unwrap()
    }

    fn observation(path: Option<&[u8]>) -> AdmissionObservationV2 {
        model(&raw_observation(), path)
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
    fn lower_counts_and_observation_bytes_refuse_both_operations() {
        let bound = policy(vec![
            rule("a", 1, PredicateV2::Always, RuleEffect::Allow),
            rule("b", 2, PredicateV2::Always, RuleEffect::Allow),
        ]);
        let observed = observation(Some(b"src/lib.rs"));
        let decision = run(&bound, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        assert_eq!(decision.matched_rule_ids().len(), 2);
        let issued = issue_receipt_v2(
            &bound,
            &observed,
            &decision,
            DEFAULT_RECEIPT_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        for limits in [
            ReceiptLimitsV2 {
                max_matched_rule_ids: 1,
                ..DEFAULT_RECEIPT_LIMITS
            },
            ReceiptLimitsV2 {
                max_observation_encoded_bytes: 1,
                ..DEFAULT_RECEIPT_LIMITS
            },
        ] {
            assert_eq!(
                issue_receipt_v2(&bound, &observed, &decision, limits, &mut || Ok(())),
                Err(AdmissionError::BudgetExhausted)
            );
            assert_eq!(
                verify_receipt_v2(&issued, &bound, &observed, limits, &mut || Ok(())),
                Err(AdmissionError::BudgetExhausted)
            );
        }
        let mut raw = raw_observation();
        raw.source_class = "cache".into();
        raw.is_system = Some(true);
        let denied_observation = model(&raw, Some(b"cache/item"));
        let denied = run(&bound, &denied_observation, DEFAULT_EVALUATION_LIMITS).unwrap();
        assert!(denied.reason_codes().len() > 1);
        assert_eq!(
            issue_receipt_v2(
                &bound,
                &denied_observation,
                &denied,
                ReceiptLimitsV2 {
                    max_reasons: 1,
                    ..DEFAULT_RECEIPT_LIMITS
                },
                &mut || Ok(())
            ),
            Err(AdmissionError::BudgetExhausted)
        );
    }

    #[test]
    fn schema_stale_and_lower_byte_error_order_is_closed() {
        let bound = policy(vec![]);
        let observed = observation(Some(b"src/lib.rs"));
        let decision = run(&bound, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let mut receipt = issue_receipt_v2(
            &bound,
            &observed,
            &decision,
            DEFAULT_RECEIPT_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        let stale = custom_policy(section(), 8, vec![]);
        let different = observation(Some(b"src/other.rs"));
        assert_eq!(
            verify_receipt_v2(
                &receipt,
                &stale,
                &different,
                DEFAULT_RECEIPT_LIMITS,
                &mut || Ok(())
            ),
            Err(AdmissionError::ReceiptStale)
        );
        receipt.schema_version = 1; // Private test-only mutation, no public codec or constructor.
        assert_eq!(
            verify_receipt_v2(
                &receipt,
                &bound,
                &observed,
                DEFAULT_RECEIPT_LIMITS,
                &mut || Ok(())
            ),
            Err(AdmissionError::PolicySchemaUnsupported)
        );
        assert_eq!(
            verify_receipt_v2(
                &receipt,
                &bound,
                &observed,
                ReceiptLimitsV2 {
                    max_encoded_bytes: 0,
                    ..DEFAULT_RECEIPT_LIMITS
                },
                &mut || Ok(())
            ),
            Err(AdmissionError::ObservationInvalid)
        );
    }

    // Agent-authored cases follow; the manager adapts them to the frozen five-row API.
    // ---- AUTHORED_NOT_RUN v2 receipt causal groups ------------------

    // AUTHORED_NOT_RUN. Exact fence. The receipt mirrors the fencing decision on every
    // read-only field, and issuance against a mismatching policy is refused.
    #[test]
    fn receipt_binds_exact_policy_observation_decision_fence() {
        let bound = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        let observed = observation(Some(b"src/lib.rs"));
        let decision = run(&bound, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let divergent = policy(vec![rule("deny", 2, PredicateV2::Always, RuleEffect::Deny)]);
        assert_ne!(bound.fingerprint(), divergent.fingerprint());
        let issued = issue_receipt_v2(
            &bound,
            &observed,
            &decision,
            DEFAULT_RECEIPT_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(
            issued.decision().policy_revision(),
            decision.policy_revision()
        );
        assert_eq!(
            issued.decision().policy_fingerprint(),
            decision.policy_fingerprint()
        );
        assert_eq!(
            issued.decision().observation_fingerprint(),
            decision.observation_fingerprint()
        );
        assert_eq!(issued.decision().outcome(), decision.outcome());
        assert_eq!(issued.decision().reason_codes(), decision.reason_codes());
        assert_eq!(
            issued.decision().matched_rule_ids(),
            decision.matched_rule_ids()
        );
        assert_eq!(issued.decision().sensitivity(), decision.sensitivity());
        // Issuance against the divergent policy is a closed refusal, not a receipt.
        assert_eq!(
            issue_receipt_v2(
                &divergent,
                &observed,
                &decision,
                DEFAULT_RECEIPT_LIMITS,
                &mut || Ok(()),
            ),
            Err(AdmissionError::ReceiptMismatch)
        );
    }

    // AUTHORED_NOT_RUN. Verification distinguishes a stale policy fence from a mismatched
    // observation identity, and a same-identity round-trip verifies equal. Each
    // rejection case below asserts its own closed error.
    #[test]
    fn verify_separates_stale_policy_from_mismatched_observation() {
        let bound = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        let observed = observation(Some(b"src/lib.rs"));
        let decision = run(&bound, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let issued = issue_receipt_v2(
            &bound,
            &observed,
            &decision,
            DEFAULT_RECEIPT_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        // Same identity verifies to the identical receipt.
        let verified = verify_receipt_v2(
            &issued,
            &bound,
            &observed,
            DEFAULT_RECEIPT_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(verified.receipt(), &issued);
        // A newer policy revision is stale, not a mismatch.
        let newer = custom_policy(
            section(),
            bound.revision().get() + 1,
            vec![rule("allow", 1, PredicateV2::Always, RuleEffect::Allow)],
        );
        assert_ne!(bound.revision(), newer.revision());
        assert_eq!(
            verify_receipt_v2(
                &issued,
                &newer,
                &observed,
                DEFAULT_RECEIPT_LIMITS,
                &mut || Ok(()),
            ),
            Err(AdmissionError::ReceiptStale)
        );
        // A different observation is a mismatch, not stale.
        let shifted = observation(Some(b"src/main.rs"));
        assert_eq!(
            verify_receipt_v2(
                &issued,
                &bound,
                &shifted,
                DEFAULT_RECEIPT_LIMITS,
                &mut || Ok(()),
            ),
            Err(AdmissionError::ReceiptMismatch)
        );
    }

    // AUTHORED_NOT_RUN. An altered receipt is refused. The receipt binds only
    // owner-computed identities, so a caller-supplied digest cannot be introduced;
    // what is pinned is that verification rejects anything but the exact bytes.
    #[test]
    fn verify_refuses_altered_receipt_identity() {
        let bound = policy(vec![rule("deny", 1, PredicateV2::Always, RuleEffect::Deny)]);
        let observed = observation(Some(b"src/lib.rs"));
        let decision = run(&bound, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let issued = issue_receipt_v2(
            &bound,
            &observed,
            &decision,
            DEFAULT_RECEIPT_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        let allow = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        let allow_decision = run(&allow, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let foreign = issue_receipt_v2(
            &allow,
            &observed,
            &allow_decision,
            DEFAULT_RECEIPT_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        assert_ne!(issued.fingerprint(), foreign.fingerprint());
        let mut altered = issued.clone();
        altered.fingerprint = foreign.fingerprint();
        assert_eq!(
            verify_receipt_v2(
                &altered,
                &bound,
                &observed,
                DEFAULT_RECEIPT_LIMITS,
                &mut || Ok(()),
            ),
            Err(AdmissionError::ReceiptMismatch)
        );
    }

    // AUTHORED_NOT_RUN. The original caller checkpoint runs before raw-limit validation,
    // so a live cancellation wins over an invalid ceiling. Invalid raw limits with
    // a healthy probe return ObservationInvalid, and a zero ceiling is refused too.
    #[test]
    fn checkpoint_precedes_invalid_limit_and_maps_errors() {
        let bound = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        let observed = observation(Some(b"src/lib.rs"));
        let decision = run(&bound, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let invalid = ReceiptLimitsV2 {
            max_encoded_bytes: 0,
            ..DEFAULT_RECEIPT_LIMITS
        };
        // Healthy probe: the ceiling itself is the defect.
        assert_eq!(
            issue_receipt_v2(&bound, &observed, &decision, invalid, &mut || Ok(()),),
            Err(AdmissionError::ObservationInvalid)
        );
        // Cancelled caller beats the same invalid ceiling: checkpoint comes first.
        let mut seen = 0_usize;
        assert_eq!(
            issue_receipt_v2(&bound, &observed, &decision, invalid, &mut || {
                seen += 1;
                Err(CheckpointFailureV2::Cancelled)
            },),
            Err(AdmissionError::Cancelled)
        );
        assert_eq!(seen, 1);
    }

    // AUTHORED_NOT_RUN. A late refusal returns no receipt. Receipt issuance owns no scratch
    // of its own, so the assertion is that no partial value is produced and the
    // caller checkpoint is the only failure channel.
    #[test]
    fn late_refusal_returns_no_partial_receipt() {
        let bound = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        let observed = observation(Some(b"src/lib.rs"));
        let decision = run(&bound, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let generous = ReceiptLimitsV2 {
            max_encoded_bytes: DEFAULT_RECEIPT_LIMITS.max_encoded_bytes,
            ..DEFAULT_RECEIPT_LIMITS
        };
        assert!(issue_receipt_v2(&bound, &observed, &decision, generous, &mut || Ok(()),).is_ok());
        // Measure the healthy probe count, then fail at every position in turn.
        let mut healthy = 0_usize;
        let issued = issue_receipt_v2(&bound, &observed, &decision, generous, &mut || {
            healthy += 1;
            Ok(())
        })
        .unwrap();
        // A refusal at any checkpoint yields no partial receipt: the only
        // observable is the closed error, never a value.
        for (failure, expected) in [
            (
                CheckpointFailureV2::DeadlineExceeded,
                AdmissionError::BudgetExhausted,
            ),
            (CheckpointFailureV2::Cancelled, AdmissionError::Cancelled),
        ] {
            for fail_at in 1..=healthy {
                let mut seen = 0_usize;
                assert_eq!(
                    issue_receipt_v2(&bound, &observed, &decision, generous, &mut || {
                        seen += 1;
                        if seen == fail_at {
                            return Err(failure);
                        }
                        Ok(())
                    }),
                    Err(expected)
                );
                assert_eq!(seen, fail_at);
            }
        }
        let mut verified_calls = 0;
        verify_receipt_v2(&issued, &bound, &observed, generous, &mut || {
            verified_calls += 1;
            Ok(())
        })
        .unwrap();
        for (failure, expected) in [
            (
                CheckpointFailureV2::DeadlineExceeded,
                AdmissionError::BudgetExhausted,
            ),
            (CheckpointFailureV2::Cancelled, AdmissionError::Cancelled),
        ] {
            for fail_at in 1..=verified_calls {
                let mut seen = 0;
                assert_eq!(
                    verify_receipt_v2(&issued, &bound, &observed, generous, &mut || {
                        seen += 1;
                        if seen == fail_at {
                            return Err(failure);
                        }
                        Ok(())
                    }),
                    Err(expected)
                );
                assert_eq!(seen, fail_at);
            }
        }
    }

    // AUTHORED_NOT_RUN. Caller-selected complete encoded ceiling. The minimum accepted
    // complete domain-plus-CBOR size is pinned independently from the five-row descriptor.
    // At the inclusive minimum, the receipt
    // is identical to the default-ceiling receipt; one less exhausts. No invented
    // golden is asserted, because the digest is owner-computed shared BLAKE3.
    #[test]
    fn full_ceiling_boundary_parity_and_redacted_debug() {
        let bound = policy(vec![rule(
            "allow",
            1,
            PredicateV2::Always,
            RuleEffect::Allow,
        )]);
        let observed = observation(Some(b"src/lib.rs"));
        let decision = run(&bound, &observed, DEFAULT_EVALUATION_LIMITS).unwrap();
        let ceiling = DEFAULT_RECEIPT_LIMITS.max_encoded_bytes;
        let baseline = issue_receipt_v2(
            &bound,
            &observed,
            &decision,
            ReceiptLimitsV2 {
                max_encoded_bytes: ceiling,
                ..DEFAULT_RECEIPT_LIMITS
            },
            &mut || Ok(()),
        )
        .unwrap();
        // Domain+NUL32 + outer array1 + header38 + revision7 one byte + three34-byte digests.
        let minimum = 174;
        assert_eq!(
            issue_receipt_v2(
                &bound,
                &observed,
                &decision,
                ReceiptLimitsV2 {
                    max_encoded_bytes: minimum,
                    ..DEFAULT_RECEIPT_LIMITS
                },
                &mut || Ok(()),
            )
            .unwrap(),
            baseline.clone()
        );
        assert_eq!(
            issue_receipt_v2(
                &bound,
                &observed,
                &decision,
                ReceiptLimitsV2 {
                    max_encoded_bytes: minimum - 1,
                    ..DEFAULT_RECEIPT_LIMITS
                },
                &mut || Ok(()),
            ),
            Err(AdmissionError::BudgetExhausted)
        );
        // Private Debug stays redacted: no locator, detector or profile text.
        let rendered = format!("{baseline:?}");
        assert!(!rendered.contains("src/lib.rs"));
        assert!(!rendered.contains("detector:fixture"));
        assert!(!rendered.contains(search_source_admission::BASELINE_PROFILE));
        for id in decision.matched_rule_ids() {
            assert!(!rendered.contains(id));
        }
        assert_eq!(
            verify_receipt_v2(
                &baseline,
                &bound,
                &observed,
                ReceiptLimitsV2 {
                    max_encoded_bytes: minimum,
                    ..DEFAULT_RECEIPT_LIMITS
                },
                &mut || Ok(())
            )
            .unwrap()
            .receipt(),
            &baseline
        );
        assert_eq!(
            verify_receipt_v2(
                &baseline,
                &bound,
                &observed,
                ReceiptLimitsV2 {
                    max_encoded_bytes: minimum - 1,
                    ..DEFAULT_RECEIPT_LIMITS
                },
                &mut || Ok(())
            ),
            Err(AdmissionError::BudgetExhausted)
        );
    }
}
