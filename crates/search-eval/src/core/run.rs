//! Frozen-run artifact selection, manifest binding, and deterministic digest.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, FingerprintBuilder};
use super::corpus::ValidatedControlCorpus;
use super::limits::EvalLimits;
use super::policy::ValidatedAcceptancePolicy;
use super::registry::ValidatedMetricRegistry;

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
