//! Adapter from daemon observations to the canonical admission kernel.

use std::path::Path;

use search_source_admission as kernel;

use super::classifier;

pub(super) const BASELINE_POLICY_REVISION: u64 = 1;
pub(super) const BASELINE_MAX_FILE_BYTES: u64 = kernel::DEFAULT_MAX_FILE_BYTES;
pub(crate) const SOURCE_ADMISSION_DENIED: &str =
    kernel::AdmissionError::AdmissionDenied.code();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceAdmissionConfig {
    pub(crate) revision: u64,
    pub(crate) max_file_bytes: u64,
    pub(crate) allow_generated: bool,
    pub(crate) allow_vendor: bool,
    pub(crate) allow_binary: bool,
}

impl SourceAdmissionConfig {
    pub(crate) const fn baseline() -> Self {
        Self {
            revision: BASELINE_POLICY_REVISION,
            max_file_bytes: BASELINE_MAX_FILE_BYTES,
            allow_generated: false,
            allow_vendor: false,
            allow_binary: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdmissionPolicy {
    canonical: kernel::CanonicalAdmissionPolicy,
    fingerprint: String,
}

impl AdmissionPolicy {
    pub(crate) fn baseline() -> Self {
        Self::from_config(SourceAdmissionConfig::baseline())
            .expect("baseline admission configuration is canonical")
    }

    pub(crate) fn from_config(config: SourceAdmissionConfig) -> Result<Self, String> {
        let input = policy_input(config);
        let draft = kernel::validate_policy_schema(&input)
            .map_err(|error| error.code().to_owned())?;
        let canonical = kernel::normalize_policy(&draft)
            .map_err(|error| error.code().to_owned())?;
        let fingerprint = kernel::policy_fingerprint(&canonical).to_hex();
        Ok(Self {
            canonical,
            fingerprint,
        })
    }

    pub(super) fn canonical(&self) -> &kernel::CanonicalAdmissionPolicy {
        &self.canonical
    }

    pub(super) fn revision(&self) -> u64 {
        self.canonical.revision().get()
    }

    pub(super) fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

fn policy_input(config: SourceAdmissionConfig) -> kernel::UnvalidatedPolicyInput {
    let mut rules = vec![
        rule(
            "deny-credential",
            10,
            kernel::SourceClass::Credential,
            kernel::RuleEffect::Deny,
            kernel::AdmissionReasonCode::DenyCredentialClass,
        ),
        rule(
            "deny-private-key",
            20,
            kernel::SourceClass::PrivateKey,
            kernel::RuleEffect::Deny,
            kernel::AdmissionReasonCode::DenyPrivateKeyClass,
        ),
        rule(
            "deny-system",
            30,
            kernel::SourceClass::System,
            kernel::RuleEffect::Deny,
            kernel::AdmissionReasonCode::DenySystemLocation,
        ),
        rule(
            "deny-cache",
            40,
            kernel::SourceClass::Cache,
            kernel::RuleEffect::Deny,
            kernel::AdmissionReasonCode::DenyCacheArtifact,
        ),
        rule(
            "deny-build",
            50,
            kernel::SourceClass::BuildOutput,
            kernel::RuleEffect::Deny,
            kernel::AdmissionReasonCode::DenyBuildOutput,
        ),
        rule(
            "allow-regular",
            1_000,
            kernel::SourceClass::Regular,
            kernel::RuleEffect::Allow,
            kernel::AdmissionReasonCode::AllowExplicitRule,
        ),
        rule(
            "allow-test",
            1_010,
            kernel::SourceClass::Test,
            kernel::RuleEffect::Allow,
            kernel::AdmissionReasonCode::AllowExplicitRule,
        ),
        rule(
            "allow-documentation",
            1_020,
            kernel::SourceClass::Documentation,
            kernel::RuleEffect::Allow,
            kernel::AdmissionReasonCode::AllowExplicitRule,
        ),
    ];
    if config.allow_generated {
        rules.push(rule(
            "allow-generated",
            1_030,
            kernel::SourceClass::Generated,
            kernel::RuleEffect::Allow,
            kernel::AdmissionReasonCode::AllowExplicitRule,
        ));
    }
    if config.allow_vendor {
        rules.push(rule(
            "allow-vendor",
            1_040,
            kernel::SourceClass::Vendor,
            kernel::RuleEffect::Allow,
            kernel::AdmissionReasonCode::AllowExplicitRule,
        ));
    }
    if config.allow_binary {
        rules.push(rule(
            "allow-binary",
            1_050,
            kernel::SourceClass::Binary,
            kernel::RuleEffect::Allow,
            kernel::AdmissionReasonCode::AllowExplicitRule,
        ));
    }
    kernel::UnvalidatedPolicyInput {
        schema_version: kernel::POLICY_SCHEMA_VERSION,
        revision: config.revision,
        rules,
        max_file_bytes: config.max_file_bytes,
        allow_generated: config.allow_generated,
        allow_vendor: config.allow_vendor,
        allow_binary: config.allow_binary,
        unknown_fields: Vec::new(),
    }
}

fn rule(
    id: &str,
    precedence: u32,
    source_class: kernel::SourceClass,
    effect: kernel::RuleEffect,
    reason: kernel::AdmissionReasonCode,
) -> kernel::UnvalidatedRuleInput {
    kernel::UnvalidatedRuleInput {
        id: id.to_owned(),
        precedence,
        source_class: source_class.as_str().to_owned(),
        operator: kernel::RuleOperator::SourceClassIs.as_str().to_owned(),
        effect: effect.as_str().to_owned(),
        reason: reason.as_str().to_owned(),
        size_threshold: None,
        sensitivity_threshold: None,
        locator_class: None,
        pattern: None,
        unknown_fields: Vec::new(),
    }
}

pub(super) fn build_observation(
    path: &Path,
    bytes: &[u8],
    policy: &AdmissionPolicy,
) -> Result<kernel::AdmissionObservation, String> {
    let classified = classifier::classify(path)?;
    let byte_size = u64::try_from(bytes.len())
        .map_err(|_| kernel::AdmissionError::SourceTooLarge.code().to_owned())?;
    let input = kernel::UnvalidatedObservationInput {
        locator_class: kernel::LocatorClass::NormalizedFile.as_str().to_owned(),
        location_class: kernel::LocationClass::LocalFixed.as_str().to_owned(),
        source_kind: kernel::SourceKind::File.as_str().to_owned(),
        source_class: classified.source_class.as_str().to_owned(),
        byte_size: Some(byte_size),
        is_generated: Some(classified.is_generated),
        is_vendor: Some(classified.is_vendor),
        is_binary: Some(classified.is_binary),
        is_system: Some(classified.is_system),
        sensitivity: Some(classified.sensitivity.as_str().to_owned()),
        detector_id: Some("detector:daemon-path-v1".to_owned()),
        profile_id: Some(kernel::BASELINE_PROFILE.to_owned()),
        unavailable_fields: Vec::new(),
        unknown_fields: Vec::new(),
    };
    kernel::validate_observation(
        &input,
        policy.canonical(),
        kernel::DEFAULT_ADMISSION_LIMITS,
    )
    .map_err(|error| error.code().to_owned())
}

pub(super) fn issue_receipt(
    policy: &AdmissionPolicy,
    observation: &kernel::AdmissionObservation,
) -> Result<kernel::AdmissionReceipt, String> {
    let decision = kernel::evaluate(
        policy.canonical(),
        observation,
        kernel::AdmissionBudget::default_budget(),
        kernel::CancelFlag::live(),
    )
    .map_err(|error| error.code().to_owned())?;
    let receipt = kernel::issue_receipt(policy.canonical(), observation, &decision)
        .map_err(|error| error.code().to_owned())?;
    kernel::verify_receipt(&receipt, policy.canonical(), observation)
        .map_err(|error| error.code().to_owned())?;
    Ok(receipt)
}

pub(super) fn verify_receipt(
    receipt: &kernel::AdmissionReceipt,
    policy: &AdmissionPolicy,
    observation: &kernel::AdmissionObservation,
) -> Result<(), String> {
    kernel::verify_receipt(receipt, policy.canonical(), observation)
        .map(|_| ())
        .map_err(|error| error.code().to_owned())
}

pub(super) fn outcome(receipt: &kernel::AdmissionReceipt) -> kernel::AdmissionOutcome {
    receipt.outcome()
}
