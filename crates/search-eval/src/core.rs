//! Control corpus, metric registry, preregistered policy, and frozen runs.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, FingerprintBuilder};

/// Conservative finite evaluation limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvalLimits {
    /// Maximum cases in one control corpus.
    pub max_cases: usize,
    /// Maximum independent lineages.
    pub max_lineages: usize,
    /// Maximum registered metrics.
    pub max_metrics: usize,
    /// Maximum policy rules.
    pub max_policy_rules: usize,
    /// Maximum artifacts in one frozen run.
    pub max_artifacts: usize,
    /// Maximum repetitions per case/baseline.
    pub max_repetitions: u32,
    /// Maximum warm-up attempts per case/baseline.
    pub max_warmups: u32,
    /// Maximum UTF-8 bytes in one bounded identifier-like label.
    pub max_text_bytes: usize,
    /// Maximum immutable receipt references retained by one object.
    pub max_receipts: usize,
    /// Maximum resource samples retained by one attempt.
    pub max_resource_samples: usize,
    /// Maximum audit events or fault cells.
    pub max_audit_items: usize,
}

impl EvalLimits {
    /// Conservative baseline suitable for local Product Pulse runs.
    pub const BASELINE: Self = Self {
        max_cases: 100_000,
        max_lineages: 10_000,
        max_metrics: 4_096,
        max_policy_rules: 4_096,
        max_artifacts: 1_024,
        max_repetitions: 10_000,
        max_warmups: 1_000,
        max_text_bytes: 4_096,
        max_receipts: 100_000,
        max_resource_samples: 1_000_000,
        max_audit_items: 1_000_000,
    };

    /// Rejects zero or contradictory ceilings.
    pub const fn validate(self) -> Result<Self, EvalError> {
        if self.max_cases == 0
            || self.max_lineages < 8
            || self.max_metrics == 0
            || self.max_policy_rules == 0
            || self.max_artifacts == 0
            || self.max_repetitions == 0
            || self.max_warmups == 0
            || self.max_text_bytes == 0
            || self.max_receipts == 0
            || self.max_resource_samples == 0
            || self.max_audit_items == 0
        {
            Err(EvalError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Closed control-case family.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CaseFamily {
    /// Locate a bounded source/entity target.
    Locate,
    /// Literal or lexical text retrieval.
    FindText,
    /// Inspect one resolved entity.
    InspectEntity,
    /// Explore a bounded entity neighborhood.
    ExploreEntity,
    /// Compare independent implementations.
    CompareImplementations,
    /// Compile or execute an exact scan.
    ExactScan,
    /// Describe one frozen corpus.
    CorpusProfile,
    /// Compare two frozen corpus revisions.
    CorpusDelta,
    /// Produce source provenance.
    Provenance,
    /// Expand an opaque source handle.
    HandleExpansion,
    /// Fork relationship fixture.
    Fork,
    /// Mirror/copy relationship fixture.
    Mirror,
    /// Nested repository boundary fixture.
    NestedRepository,
    /// Submodule boundary fixture.
    Submodule,
    /// Restrictive access and purge fixture.
    Security,
    /// Crash and mutation-recovery fixture.
    Recovery,
    /// Framing/replay/cancellation fixture.
    Protocol,
    /// Latency and resource fixture.
    Performance,
}

impl CaseFamily {
    /// Topology families required by the architecture.
    pub const REQUIRED_TOPOLOGY: [Self; 4] = [
        Self::Fork,
        Self::Mirror,
        Self::NestedRepository,
        Self::Submodule,
    ];
}

/// One immutable oracle-bearing control case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCase {
    /// Stable case identity.
    pub case_id: OpaqueId,
    /// Closed case family.
    pub family: CaseFamily,
    /// Exact public fixture digest.
    pub fixture_digest: Blake3Digest32,
    /// Exact private oracle digest.
    pub oracle_digest: Blake3Digest32,
    /// Independent repository lineage represented by this case.
    pub lineage_id: OpaqueId,
    /// Whether fixture bytes are immutable/content-addressed.
    pub immutable: bool,
    /// Whether the oracle remains isolated from candidate-visible state.
    pub oracle_private: bool,
    /// Finite input ceiling.
    pub max_input_bytes: u64,
    /// Finite raw-output ceiling.
    pub max_output_bytes: u64,
    /// Content-free fixture evidence.
    pub fixture_receipt: ReceiptRef,
}

/// Exact registered control corpus manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCorpusManifest {
    /// Stable corpus identity.
    pub corpus_id: OpaqueId,
    /// Monotone corpus revision.
    pub revision: u64,
    /// Digest of the complete canonical corpus manifest.
    pub manifest_digest: Blake3Digest32,
    /// Digest of the exact fixture index.
    pub fixture_index_digest: Blake3Digest32,
    /// Digest of the disclosure policy.
    pub disclosure_policy_digest: Blake3Digest32,
    /// Every case in deterministic canonical order.
    pub cases: Vec<ControlCase>,
    /// Case families declared mandatory for this run profile.
    pub mandatory_families: BTreeSet<CaseFamily>,
    /// Whether the manifest itself is immutable.
    pub immutable: bool,
    /// Content-free manifest receipt.
    pub manifest_receipt: ReceiptRef,
}

/// Control corpus that passed structural, topology, lineage, and oracle checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedControlCorpus {
    manifest: ControlCorpusManifest,
    lineage_count: usize,
    family_counts: BTreeMap<CaseFamily, usize>,
    validation_digest: Blake3Digest32,
}

impl ValidatedControlCorpus {
    /// Exact accepted manifest.
    #[must_use]
    pub const fn manifest(&self) -> &ControlCorpusManifest {
        &self.manifest
    }

    /// Number of independent represented lineages.
    #[must_use]
    pub const fn lineage_count(&self) -> usize {
        self.lineage_count
    }

    /// Number of cases in one family.
    #[must_use]
    pub fn family_count(&self, family: CaseFamily) -> usize {
        self.family_counts.get(&family).copied().unwrap_or(0)
    }

    /// Deterministic validation fingerprint.
    #[must_use]
    pub const fn validation_digest(&self) -> Blake3Digest32 {
        self.validation_digest
    }
}

/// Validates the registered control corpus without executing a candidate.
pub fn validate_control_corpus(
    manifest: ControlCorpusManifest,
    limits: EvalLimits,
) -> Result<ValidatedControlCorpus, EvalError> {
    let limits = limits.validate()?;
    if manifest.revision == 0
        || !manifest.immutable
        || manifest.cases.is_empty()
        || manifest.cases.len() > limits.max_cases
        || manifest.mandatory_families.is_empty()
    {
        return Err(EvalError::CorpusInvalid);
    }

    let mut case_ids = BTreeSet::new();
    let mut lineages = BTreeSet::new();
    let mut family_counts = BTreeMap::new();
    for case in &manifest.cases {
        if !case_ids.insert(case.case_id.clone()) {
            return Err(EvalError::DuplicateCase);
        }
        if !case.immutable
            || !case.oracle_private
            || case.max_input_bytes == 0
            || case.max_output_bytes == 0
            || case.fixture_receipt.as_str().is_empty()
        {
            return Err(if case.oracle_private {
                EvalError::CorpusInvalid
            } else {
                EvalError::OracleContamination
            });
        }
        lineages.insert(case.lineage_id.clone());
        *family_counts.entry(case.family).or_insert(0_usize) += 1;
    }
    if lineages.len() < 8 || lineages.len() > limits.max_lineages {
        return Err(EvalError::InsufficientLineages);
    }
    for family in manifest
        .mandatory_families
        .iter()
        .copied()
        .chain(CaseFamily::REQUIRED_TOPOLOGY)
    {
        if family_counts.get(&family).copied().unwrap_or(0) == 0 {
            return Err(EvalError::CorpusIncomplete);
        }
    }
    if manifest
        .cases
        .windows(2)
        .any(|pair| pair[0].case_id >= pair[1].case_id)
    {
        return Err(EvalError::CorpusInvalid);
    }

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/corpus-validation/v1");
    fingerprint.push_text(manifest.corpus_id.as_str());
    fingerprint.push_u64(manifest.revision);
    fingerprint.push_digest(manifest.manifest_digest);
    fingerprint.push_digest(manifest.fixture_index_digest);
    fingerprint.push_digest(manifest.disclosure_policy_digest);
    for case in &manifest.cases {
        fingerprint.push_text(case.case_id.as_str());
        fingerprint.push_u64(case_family_tag(case.family));
        fingerprint.push_digest(case.fixture_digest);
        fingerprint.push_digest(case.oracle_digest);
        fingerprint.push_text(case.lineage_id.as_str());
        fingerprint.push_u64(case.max_input_bytes);
        fingerprint.push_u64(case.max_output_bytes);
    }
    Ok(ValidatedControlCorpus {
        manifest,
        lineage_count: lineages.len(),
        family_counts,
        validation_digest: fingerprint.finish(),
    })
}

/// Direction and zero-tolerance semantics of one metric.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MetricDirection {
    /// Larger values are better.
    HigherIsBetter,
    /// Smaller values are better.
    LowerIsBetter,
    /// Any non-zero value is a hard failure.
    ZeroTolerance,
}

/// Registered denominator meaning.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MetricDenominator {
    /// Every frozen case.
    PerCase,
    /// Every measured attempt.
    PerAttempt,
    /// Every source/input byte.
    PerInputByte,
    /// Every output/result byte.
    PerOutputByte,
    /// Every elapsed millisecond.
    PerMillisecond,
}

/// Missing-value behavior fixed before candidate results exist.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MissingValuePolicy {
    /// Missing evidence fails the run or gate.
    FailRun,
    /// Missing evidence contributes a registered failure value.
    CountAsFailure,
    /// Metric remains unavailable and the report remains incomplete.
    UnavailableAndIncomplete,
}

/// One preregistered metric definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricDefinition {
    /// Stable metric identity.
    pub metric_id: OpaqueId,
    /// Bounded unit identity.
    pub unit: OpaqueId,
    /// Improvement direction.
    pub direction: MetricDirection,
    /// Exact denominator meaning.
    pub denominator: MetricDenominator,
    /// Missing-value behavior.
    pub missing_value_policy: MissingValuePolicy,
    /// Whether this is a hard safety/correctness metric.
    pub safety_metric: bool,
    /// Minimum measured attempts before percentile-like aggregation is available.
    pub minimum_samples: u64,
}

/// Complete immutable metric registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricRegistry {
    /// Stable registry identity.
    pub registry_id: OpaqueId,
    /// Monotone registry revision.
    pub revision: u64,
    /// Digest of exact canonical definitions.
    pub registry_digest: Blake3Digest32,
    /// Definitions in canonical metric-id order.
    pub definitions: Vec<MetricDefinition>,
    /// Immutable registry evidence.
    pub registry_receipt: ReceiptRef,
}

/// Metric registry that passed semantic validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedMetricRegistry {
    registry: MetricRegistry,
    by_id: BTreeMap<OpaqueId, MetricDefinition>,
    validation_digest: Blake3Digest32,
}

impl ValidatedMetricRegistry {
    /// Exact accepted registry.
    #[must_use]
    pub const fn registry(&self) -> &MetricRegistry {
        &self.registry
    }

    /// Reads one metric definition.
    #[must_use]
    pub fn metric(&self, metric_id: &OpaqueId) -> Option<&MetricDefinition> {
        self.by_id.get(metric_id)
    }

    /// Deterministic validation fingerprint.
    #[must_use]
    pub const fn validation_digest(&self) -> Blake3Digest32 {
        self.validation_digest
    }
}

/// Validates metric identity, direction, denominator, and missing-value behavior.
pub fn validate_metric_registry(
    registry: MetricRegistry,
    limits: EvalLimits,
) -> Result<ValidatedMetricRegistry, EvalError> {
    let limits = limits.validate()?;
    if registry.revision == 0
        || registry.definitions.is_empty()
        || registry.definitions.len() > limits.max_metrics
        || registry.registry_receipt.as_str().is_empty()
        || registry
            .definitions
            .windows(2)
            .any(|pair| pair[0].metric_id >= pair[1].metric_id)
    {
        return Err(EvalError::MetricRegistryInvalid);
    }
    let mut by_id = BTreeMap::new();
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/metric-registry/v1");
    fingerprint.push_text(registry.registry_id.as_str());
    fingerprint.push_u64(registry.revision);
    fingerprint.push_digest(registry.registry_digest);
    for definition in &registry.definitions {
        if definition.unit.as_str().is_empty() || definition.minimum_samples == 0 {
            return Err(EvalError::MetricRegistryInvalid);
        }
        if definition.direction == MetricDirection::ZeroTolerance
            && !definition.safety_metric
        {
            return Err(EvalError::MetricRegistryInvalid);
        }
        if definition.safety_metric
            && definition.missing_value_policy != MissingValuePolicy::FailRun
        {
            return Err(EvalError::MetricRegistryInvalid);
        }
        if by_id
            .insert(definition.metric_id.clone(), definition.clone())
            .is_some()
        {
            return Err(EvalError::DuplicateMetric);
        }
        fingerprint.push_text(definition.metric_id.as_str());
        fingerprint.push_text(definition.unit.as_str());
        fingerprint.push_u64(metric_direction_tag(definition.direction));
        fingerprint.push_u64(metric_denominator_tag(definition.denominator));
        fingerprint.push_u64(missing_policy_tag(definition.missing_value_policy));
        fingerprint.push_bool(definition.safety_metric);
        fingerprint.push_u64(definition.minimum_samples);
    }
    Ok(ValidatedMetricRegistry {
        registry,
        by_id,
        validation_digest: fingerprint.finish(),
    })
}

/// One preregistered acceptance rule.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptanceRule {
    /// Registered metric identity.
    pub metric_id: OpaqueId,
    /// Absolute candidate acceptance threshold.
    pub candidate_threshold: f64,
    /// Maximum tolerated regression against the strongest applicable baseline.
    pub maximum_regression: f64,
    /// Minimum practical improvement needed to claim material gain.
    pub practical_effect: f64,
    /// Whether the metric is primary.
    pub primary: bool,
}

/// Complete acceptance policy fixed before candidate results.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptancePolicy {
    /// Stable policy identity.
    pub policy_id: OpaqueId,
    /// Monotone policy revision.
    pub revision: u64,
    /// Exact policy digest.
    pub policy_digest: Blake3Digest32,
    /// Evidence sequence at which the policy was registered.
    pub registered_sequence: u64,
    /// First candidate-result sequence, or zero if no result existed yet.
    pub first_candidate_result_sequence: u64,
    /// Evaluation producer identity.
    pub producer_id: OpaqueId,
    /// Independent policy approver identity.
    pub approver_id: OpaqueId,
    /// Registered metric rules in canonical order.
    pub rules: Vec<AcceptanceRule>,
    /// Whether every mandatory case family must have measured evidence.
    pub require_complete_case_families: bool,
    /// Whether all registered SLOs must pass.
    pub require_slo_success: bool,
    /// Whether final comparison must be DOMINATES or COMPLEMENTS.
    pub require_material_value: bool,
    /// Immutable policy approval receipt.
    pub approval_receipt: ReceiptRef,
}

/// Acceptance policy validated against an exact metric registry.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedAcceptancePolicy {
    policy: AcceptancePolicy,
    rules: BTreeMap<OpaqueId, AcceptanceRule>,
    validation_digest: Blake3Digest32,
}

impl ValidatedAcceptancePolicy {
    /// Exact accepted policy.
    #[must_use]
    pub const fn policy(&self) -> &AcceptancePolicy {
        &self.policy
    }

    /// Reads one registered rule.
    #[must_use]
    pub fn rule(&self, metric_id: &OpaqueId) -> Option<&AcceptanceRule> {
        self.rules.get(metric_id)
    }

    /// Deterministic policy-validation fingerprint.
    #[must_use]
    pub const fn validation_digest(&self) -> Blake3Digest32 {
        self.validation_digest
    }
}

/// Validates pre-registration, independent approval, and metric references.
pub fn validate_acceptance_policy(
    policy: AcceptancePolicy,
    metrics: &ValidatedMetricRegistry,
    limits: EvalLimits,
) -> Result<ValidatedAcceptancePolicy, EvalError> {
    let limits = limits.validate()?;
    if policy.revision == 0
        || policy.rules.is_empty()
        || policy.rules.len() > limits.max_policy_rules
        || policy.approval_receipt.as_str().is_empty()
        || policy.producer_id == policy.approver_id
        || policy
            .rules
            .windows(2)
            .any(|pair| pair[0].metric_id >= pair[1].metric_id)
    {
        return Err(if policy.producer_id == policy.approver_id {
            EvalError::IndependentReviewRequired
        } else {
            EvalError::AcceptancePolicyInvalid
        });
    }
    if policy.first_candidate_result_sequence != 0
        && policy.registered_sequence >= policy.first_candidate_result_sequence
    {
        return Err(EvalError::PolicyNotPreregistered);
    }

    let mut rules = BTreeMap::new();
    let mut primary_rules = 0_usize;
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/acceptance-policy/v1");
    fingerprint.push_text(policy.policy_id.as_str());
    fingerprint.push_u64(policy.revision);
    fingerprint.push_digest(policy.policy_digest);
    fingerprint.push_u64(policy.registered_sequence);
    for rule in &policy.rules {
        if !rule.candidate_threshold.is_finite()
            || !rule.maximum_regression.is_finite()
            || !rule.practical_effect.is_finite()
            || rule.maximum_regression < 0.0
            || rule.practical_effect < 0.0
            || metrics.metric(&rule.metric_id).is_none()
        {
            return Err(EvalError::AcceptancePolicyInvalid);
        }
        if rule.primary {
            primary_rules = primary_rules.saturating_add(1);
        }
        if rules.insert(rule.metric_id.clone(), rule.clone()).is_some() {
            return Err(EvalError::AcceptancePolicyInvalid);
        }
        fingerprint.push_text(rule.metric_id.as_str());
        fingerprint.push_f64(rule.candidate_threshold);
        fingerprint.push_f64(rule.maximum_regression);
        fingerprint.push_f64(rule.practical_effect);
        fingerprint.push_bool(rule.primary);
    }
    if primary_rules == 0 {
        return Err(EvalError::AcceptancePolicyInvalid);
    }
    fingerprint.push_bool(policy.require_complete_case_families);
    fingerprint.push_bool(policy.require_slo_success);
    fingerprint.push_bool(policy.require_material_value);
    Ok(ValidatedAcceptancePolicy {
        policy,
        rules,
        validation_digest: fingerprint.finish(),
    })
}

/// Exact immutable artifact/configuration identity included in one run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunArtifact {
    /// Artifact role identity.
    pub artifact_id: OpaqueId,
    /// Exact executable/model/index/parser artifact digest.
    pub artifact_digest: Blake3Digest32,
    /// Exact configuration digest.
    pub configuration_digest: Blake3Digest32,
    /// Exact runtime/profile digest.
    pub profile_digest: Blake3Digest32,
    /// Whether the role was explicitly selected rather than left floating.
    pub selected: bool,
}

/// Complete input used to freeze one A/B/C run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenRunInput {
    /// Stable run identity.
    pub run_id: OpaqueId,
    /// Exact repository commit identity.
    pub repository_commit: OpaqueId,
    /// Exact environment digest.
    pub environment_digest: Blake3Digest32,
    /// Exact source/view digest shared by A/B/C.
    pub source_view_digest: Blake3Digest32,
    /// Exact control-corpus validation digest.
    pub corpus_validation_digest: Blake3Digest32,
    /// Exact metric-registry validation digest.
    pub metric_registry_validation_digest: Blake3Digest32,
    /// Exact acceptance-policy validation digest.
    pub acceptance_policy_validation_digest: Blake3Digest32,
    /// Exact immutable raw-output store identity.
    pub raw_output_store_digest: Blake3Digest32,
    /// Deterministic run seed.
    pub seed: u64,
    /// Measured attempts per case and baseline.
    pub repetitions: u32,
    /// Warm-up attempts per case and baseline.
    pub warmups: u32,
    /// Every load-bearing selected artifact.
    pub artifacts: Vec<RunArtifact>,
    /// Whether network access is disabled for the run.
    pub network_disabled: bool,
    /// Whether oracle bytes/state are held outside candidate-visible state.
    pub oracle_store_separate: bool,
    /// Whether evaluation feedback is prohibited from production ranking/training.
    pub candidate_feedback_disabled: bool,
    /// Content-free environment capture receipt.
    pub environment_receipt: ReceiptRef,
}

/// Immutable, deterministic run manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenRunManifest {
    input: FrozenRunInput,
    run_digest: Blake3Digest32,
}

impl FrozenRunManifest {
    /// Complete frozen input.
    #[must_use]
    pub const fn input(&self) -> &FrozenRunInput {
        &self.input
    }

    /// Deterministic run digest.
    #[must_use]
    pub const fn run_digest(&self) -> Blake3Digest32 {
        self.run_digest
    }
}

/// Freezes a complete run manifest after all load-bearing selections exist.
pub fn freeze_run_manifest(
    mut input: FrozenRunInput,
    corpus: &ValidatedControlCorpus,
    metrics: &ValidatedMetricRegistry,
    policy: &ValidatedAcceptancePolicy,
    limits: EvalLimits,
) -> Result<FrozenRunManifest, EvalError> {
    let limits = limits.validate()?;
    if input.repository_commit.as_str().is_empty()
        || input.seed == 0
        || input.repetitions == 0
        || input.repetitions > limits.max_repetitions
        || input.warmups > limits.max_warmups
        || input.artifacts.is_empty()
        || input.artifacts.len() > limits.max_artifacts
        || !input.network_disabled
        || !input.oracle_store_separate
        || !input.candidate_feedback_disabled
        || input.environment_receipt.as_str().is_empty()
        || input.corpus_validation_digest != corpus.validation_digest()
        || input.metric_registry_validation_digest != metrics.validation_digest()
        || input.acceptance_policy_validation_digest != policy.validation_digest()
    {
        return Err(if input.oracle_store_separate {
            EvalError::FrozenRunInvalid
        } else {
            EvalError::OracleContamination
        });
    }
    input.artifacts.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    if input
        .artifacts
        .windows(2)
        .any(|pair| pair[0].artifact_id == pair[1].artifact_id)
        || input.artifacts.iter().any(|artifact| !artifact.selected)
    {
        return Err(EvalError::FrozenRunInvalid);
    }

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/frozen-run/v1");
    fingerprint.push_text(input.run_id.as_str());
    fingerprint.push_text(input.repository_commit.as_str());
    fingerprint.push_digest(input.environment_digest);
    fingerprint.push_digest(input.source_view_digest);
    fingerprint.push_digest(input.corpus_validation_digest);
    fingerprint.push_digest(input.metric_registry_validation_digest);
    fingerprint.push_digest(input.acceptance_policy_validation_digest);
    fingerprint.push_digest(input.raw_output_store_digest);
    fingerprint.push_u64(input.seed);
    fingerprint.push_u64(u64::from(input.repetitions));
    fingerprint.push_u64(u64::from(input.warmups));
    for artifact in &input.artifacts {
        fingerprint.push_text(artifact.artifact_id.as_str());
        fingerprint.push_digest(artifact.artifact_digest);
        fingerprint.push_digest(artifact.configuration_digest);
        fingerprint.push_digest(artifact.profile_digest);
    }
    Ok(FrozenRunManifest {
        input,
        run_digest: fingerprint.finish(),
    })
}

/// Role of one compared implementation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BaselineRole {
    /// Baseline A.
    A,
    /// Baseline B.
    B,
    /// Candidate C.
    C,
}

/// Exact qualified baseline/candidate descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaselineDescriptor {
    /// Stable baseline identity.
    pub baseline_id: OpaqueId,
    /// A/B/C role.
    pub role: BaselineRole,
    /// Exact source or release identity.
    pub source_identity: OpaqueId,
    /// Exact version identity; `latest` is forbidden.
    pub version_identity: OpaqueId,
    /// Exact artifact digest.
    pub artifact_digest: Blake3Digest32,
    /// Exact configuration digest.
    pub configuration_digest: Blake3Digest32,
    /// Exact invocation-driver digest.
    pub driver_digest: Blake3Digest32,
    /// Exact declared scope capability digest.
    pub scope_digest: Blake3Digest32,
    /// Frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Whether artifact qualification succeeded.
    pub qualified: bool,
    /// Whether network access is disabled.
    pub no_network: bool,
    /// Whether hidden patches are absent.
    pub unmodified_artifact: bool,
    /// Qualification evidence.
    pub qualification_receipt: ReceiptRef,
}

/// Baseline accepted for one exact frozen run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedBaseline(BaselineDescriptor);

impl ValidatedBaseline {
    /// Exact accepted descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &BaselineDescriptor {
        &self.0
    }
}

/// Validates exact artifact, version, scope, driver, and run binding.
pub fn validate_baseline_descriptor(
    descriptor: BaselineDescriptor,
    run: &FrozenRunManifest,
) -> Result<ValidatedBaseline, EvalError> {
    let version = descriptor.version_identity.as_str();
    if !descriptor.qualified
        || !descriptor.no_network
        || !descriptor.unmodified_artifact
        || descriptor.run_digest != run.run_digest()
        || descriptor.source_identity.as_str().is_empty()
        || version.is_empty()
        || version.eq_ignore_ascii_case("latest")
        || descriptor.qualification_receipt.as_str().is_empty()
    {
        return Err(EvalError::BaselineUnqualified);
    }
    Ok(ValidatedBaseline(descriptor))
}

fn case_family_tag(value: CaseFamily) -> u64 {
    match value {
        CaseFamily::Locate => 1,
        CaseFamily::FindText => 2,
        CaseFamily::InspectEntity => 3,
        CaseFamily::ExploreEntity => 4,
        CaseFamily::CompareImplementations => 5,
        CaseFamily::ExactScan => 6,
        CaseFamily::CorpusProfile => 7,
        CaseFamily::CorpusDelta => 8,
        CaseFamily::Provenance => 9,
        CaseFamily::HandleExpansion => 10,
        CaseFamily::Fork => 11,
        CaseFamily::Mirror => 12,
        CaseFamily::NestedRepository => 13,
        CaseFamily::Submodule => 14,
        CaseFamily::Security => 15,
        CaseFamily::Recovery => 16,
        CaseFamily::Protocol => 17,
        CaseFamily::Performance => 18,
    }
}

fn metric_direction_tag(value: MetricDirection) -> u64 {
    match value {
        MetricDirection::HigherIsBetter => 1,
        MetricDirection::LowerIsBetter => 2,
        MetricDirection::ZeroTolerance => 3,
    }
}

fn metric_denominator_tag(value: MetricDenominator) -> u64 {
    match value {
        MetricDenominator::PerCase => 1,
        MetricDenominator::PerAttempt => 2,
        MetricDenominator::PerInputByte => 3,
        MetricDenominator::PerOutputByte => 4,
        MetricDenominator::PerMillisecond => 5,
    }
}

fn missing_policy_tag(value: MissingValuePolicy) -> u64 {
    match value {
        MissingValuePolicy::FailRun => 1,
        MissingValuePolicy::CountAsFailure => 2,
        MissingValuePolicy::UnavailableAndIncomplete => 3,
    }
}
