//! Mandatory unsafe-source admission audit.

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest};

use super::common::{HardBlocker, HardBlockerClass, bounded_reason};

/// Mandatory unsafe-admission scenario.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionScenario {
    /// Unknown requested source or membership.
    UnknownScope,
    /// Symbolic-link escape.
    SymlinkEscape,
    /// Windows reparse-point/junction escape.
    ReparseEscape,
    /// Remote, device, or disallowed root.
    RemoteRoot,
    /// Source exceeds finite byte policy.
    OversizedSource,
    /// Restricted source lacks an exact grant.
    RestrictedWithoutGrant,
    /// Runtime owner epoch is stale.
    StaleOwnerEpoch,
    /// Admission policy revision is stale.
    StalePolicyRevision,
    /// Live purge/security barrier denies the source.
    PurgeFenced,
    /// Virtual buffer lacks authenticated immutable attestation.
    VirtualWithoutAttestation,
}

impl AdmissionScenario {
    /// Baseline mandatory admission-denial matrix.
    pub const MANDATORY: [Self; 10] = [
        Self::UnknownScope,
        Self::SymlinkEscape,
        Self::ReparseEscape,
        Self::RemoteRoot,
        Self::OversizedSource,
        Self::RestrictedWithoutGrant,
        Self::StaleOwnerEpoch,
        Self::StalePolicyRevision,
        Self::PurgeFenced,
        Self::VirtualWithoutAttestation,
    ];
}

/// One immutable source-admission probe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionProbe {
    /// Stable probe identity.
    pub probe_id: OpaqueId,
    /// Unsafe scenario under test.
    pub scenario: AdmissionScenario,
    /// Whether the source was actually admitted.
    pub admitted: bool,
    /// Whether denial happened before source bytes entered preparation/indexing.
    pub denied_before_content_processing: bool,
    /// Closed observed reason identity.
    pub observed_reason: OpaqueId,
    /// Immutable raw evidence.
    pub evidence_ref: ReceiptRef,
}

/// Complete unsafe-source admission audit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionAudit {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Canonical probes.
    pub probes: Vec<AdmissionProbe>,
    /// Hard blockers for unsafe admission or late denial.
    pub blockers: Vec<HardBlocker>,
    /// Whether every mandatory scenario denied before content processing.
    pub passed: bool,
    /// Deterministic audit digest.
    pub audit_digest: Blake3Digest32,
}

/// Validates the complete mandatory unsafe-admission matrix.
pub fn audit_source_admission(
    run: &FrozenRunManifest,
    mut probes: Vec<AdmissionProbe>,
    limits: EvalLimits,
) -> Result<AdmissionAudit, EvalError> {
    let limits = limits.validate()?;
    if probes.len() > limits.max_audit_items {
        return Err(EvalError::BudgetExceeded);
    }
    probes.sort_by(|left, right| {
        (left.scenario, &left.probe_id).cmp(&(right.scenario, &right.probe_id))
    });
    let mut scenarios = BTreeSet::new();
    let mut blockers = Vec::new();
    for probe in &probes {
        if probe.evidence_ref.as_str().is_empty() || !scenarios.insert(probe.scenario) {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        if probe.admitted || !probe.denied_before_content_processing {
            blockers.push(HardBlocker {
                class: HardBlockerClass::SourceAdmission,
                check_id: probe.probe_id.clone(),
                reason: bounded_reason(if probe.admitted {
                    "UNSAFE_SOURCE_ADMITTED"
                } else {
                    "DENIAL_AFTER_CONTENT_PROCESSING"
                })?,
                evidence_ref: probe.evidence_ref.clone(),
            });
        }
    }
    if !AdmissionScenario::MANDATORY
        .into_iter()
        .all(|scenario| scenarios.contains(&scenario))
    {
        return Err(EvalError::ProductReportIncomplete);
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/admission-audit/v1");
    fingerprint.push_digest(run.run_digest());
    for probe in &probes {
        fingerprint.push_text(probe.probe_id.as_str());
        fingerprint.push_u64(admission_tag(probe.scenario));
        fingerprint.push_bool(probe.admitted);
        fingerprint.push_bool(probe.denied_before_content_processing);
        fingerprint.push_text(probe.observed_reason.as_str());
    }
    Ok(AdmissionAudit {
        run_digest: run.run_digest(),
        probes,
        passed: blockers.is_empty(),
        blockers,
        audit_digest: fingerprint.finish(),
    })
}

const fn admission_tag(value: AdmissionScenario) -> u64 {
    match value {
        AdmissionScenario::UnknownScope => 1,
        AdmissionScenario::SymlinkEscape => 2,
        AdmissionScenario::ReparseEscape => 3,
        AdmissionScenario::RemoteRoot => 4,
        AdmissionScenario::OversizedSource => 5,
        AdmissionScenario::RestrictedWithoutGrant => 6,
        AdmissionScenario::StaleOwnerEpoch => 7,
        AdmissionScenario::StalePolicyRevision => 8,
        AdmissionScenario::PurgeFenced => 9,
        AdmissionScenario::VirtualWithoutAttestation => 10,
    }
}
