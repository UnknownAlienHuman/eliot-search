//! Complete canary-by-surface leakage audit.

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest};

use super::common::{HardBlocker, HardBlockerClass, bounded_reason};

/// Closed prohibited canary class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanaryClass {
    /// Source body or excerpt bytes.
    SourceContent,
    /// User query text.
    QueryText,
    /// Absolute or canonical path text.
    AbsolutePath,
    /// Plaintext secret or credential.
    Secret,
    /// Bearer token or opaque-handle plaintext.
    BearerToken,
    /// Private handle/continuation authority record.
    AuthorityRecord,
    /// Private evaluation oracle.
    Oracle,
    /// Forbidden vendor-specific internal metadata.
    VendorMetadata,
}

/// Closed observable leakage surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LeakageSurface {
    /// Process/application logs.
    Logs,
    /// Durable control store.
    ControlStore,
    /// Search-index payload.
    IndexPayload,
    /// Metrics, traces, or telemetry.
    Telemetry,
    /// Protocol error/result body.
    Protocol,
    /// Temporary filesystem objects.
    TemporaryFiles,
    /// Crash artifacts and dumps.
    CrashArtifacts,
    /// Optional model-provider input.
    ModelInput,
    /// Evaluation feedback visible to production ranking/training.
    EvaluationFeedback,
}

/// One immutable canary/surface observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeakageObservation {
    /// Stable observation identity.
    pub observation_id: OpaqueId,
    /// Canary class searched for.
    pub canary_class: CanaryClass,
    /// Exact observed surface.
    pub surface: LeakageSurface,
    /// Digest of the secret canary retained outside candidate-visible state.
    pub canary_digest: Blake3Digest32,
    /// Whether the canary or an explicitly forbidden derivative was detected.
    pub detected: bool,
    /// Whether the inspected surface inventory was complete.
    pub complete_surface: bool,
    /// Immutable raw audit evidence.
    pub evidence_ref: ReceiptRef,
}

/// Complete zero-tolerance leakage audit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeakageAudit {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Required canary classes.
    pub required_canaries: BTreeSet<CanaryClass>,
    /// Required observable surfaces.
    pub required_surfaces: BTreeSet<LeakageSurface>,
    /// Exact observations in canonical pair order.
    pub observations: Vec<LeakageObservation>,
    /// Hard blockers emitted for every detection or incomplete surface.
    pub blockers: Vec<HardBlocker>,
    /// Whether all required pairs were observed and no canary was detected.
    pub passed: bool,
    /// Deterministic audit digest.
    pub audit_digest: Blake3Digest32,
}

/// Audits the complete required canary-by-surface matrix.
pub fn audit_leakage(
    run: &FrozenRunManifest,
    required_canaries: BTreeSet<CanaryClass>,
    required_surfaces: BTreeSet<LeakageSurface>,
    mut observations: Vec<LeakageObservation>,
    limits: EvalLimits,
) -> Result<LeakageAudit, EvalError> {
    let limits = limits.validate()?;
    if required_canaries.is_empty()
        || required_surfaces.is_empty()
        || observations.len() > limits.max_audit_items
    {
        return Err(EvalError::InvalidLimits);
    }
    observations.sort_by(|left, right| {
        (left.canary_class, left.surface, &left.observation_id).cmp(&(
            right.canary_class,
            right.surface,
            &right.observation_id,
        ))
    });
    let mut observed_pairs = BTreeSet::new();
    let mut blockers = Vec::new();
    for observation in &observations {
        if !required_canaries.contains(&observation.canary_class)
            || !required_surfaces.contains(&observation.surface)
            || observation.evidence_ref.as_str().is_empty()
            || !observed_pairs.insert((observation.canary_class, observation.surface))
        {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        if observation.detected || !observation.complete_surface {
            blockers.push(HardBlocker {
                class: HardBlockerClass::Leakage,
                check_id: observation.observation_id.clone(),
                reason: bounded_reason(if observation.detected {
                    "CANARY_DETECTED"
                } else {
                    "SURFACE_INCOMPLETE"
                })?,
                evidence_ref: observation.evidence_ref.clone(),
            });
        }
    }
    for canary in &required_canaries {
        for surface in &required_surfaces {
            if !observed_pairs.contains(&(*canary, *surface)) {
                return Err(EvalError::ProductReportIncomplete);
            }
        }
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/leakage-audit/v1");
    fingerprint.push_digest(run.run_digest());
    for observation in &observations {
        fingerprint.push_text(observation.observation_id.as_str());
        fingerprint.push_u64(canary_tag(observation.canary_class));
        fingerprint.push_u64(surface_tag(observation.surface));
        fingerprint.push_digest(observation.canary_digest);
        fingerprint.push_bool(observation.detected);
        fingerprint.push_bool(observation.complete_surface);
    }
    Ok(LeakageAudit {
        run_digest: run.run_digest(),
        required_canaries,
        required_surfaces,
        observations,
        passed: blockers.is_empty(),
        blockers,
        audit_digest: fingerprint.finish(),
    })
}

const fn canary_tag(value: CanaryClass) -> u64 {
    match value {
        CanaryClass::SourceContent => 1,
        CanaryClass::QueryText => 2,
        CanaryClass::AbsolutePath => 3,
        CanaryClass::Secret => 4,
        CanaryClass::BearerToken => 5,
        CanaryClass::AuthorityRecord => 6,
        CanaryClass::Oracle => 7,
        CanaryClass::VendorMetadata => 8,
    }
}

const fn surface_tag(value: LeakageSurface) -> u64 {
    match value {
        LeakageSurface::Logs => 1,
        LeakageSurface::ControlStore => 2,
        LeakageSurface::IndexPayload => 3,
        LeakageSurface::Telemetry => 4,
        LeakageSurface::Protocol => 5,
        LeakageSurface::TemporaryFiles => 6,
        LeakageSurface::CrashArtifacts => 7,
        LeakageSurface::ModelInput => 8,
        LeakageSurface::EvaluationFeedback => 9,
    }
}
