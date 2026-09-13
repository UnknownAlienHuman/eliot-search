//! Immutable metric registry model and semantic validation.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, FingerprintBuilder};
use super::limits::EvalLimits;

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

const fn metric_direction_tag(value: MetricDirection) -> u64 {
    match value {
        MetricDirection::HigherIsBetter => 1,
        MetricDirection::LowerIsBetter => 2,
        MetricDirection::ZeroTolerance => 3,
    }
}

const fn metric_denominator_tag(value: MetricDenominator) -> u64 {
    match value {
        MetricDenominator::PerCase => 1,
        MetricDenominator::PerAttempt => 2,
        MetricDenominator::PerInputByte => 3,
        MetricDenominator::PerOutputByte => 4,
        MetricDenominator::PerMillisecond => 5,
    }
}

const fn missing_policy_tag(value: MissingValuePolicy) -> u64 {
    match value {
        MissingValuePolicy::FailRun => 1,
        MissingValuePolicy::CountAsFailure => 2,
        MissingValuePolicy::UnavailableAndIncomplete => 3,
    }
}
