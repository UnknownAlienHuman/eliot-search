//! Optional document provider seam: gated, unselected, never executed.
//!
//! Baseline text/code materialization works with optional workers absent, and
//! provider removal always falls back explicitly to baseline behavior. No
//! provider is selected by baseline code or configuration; qualification
//! requires a future P17 ADR with exact provider, runtime, artifact, license,
//! Windows, no-execute, input/output, coordinate, loss, assurance, resource
//! and fuzz identities plus accepted P15 and independent review. Python, Node
//! and vendor types never enter this API.

use crate::MaterializationError;
use search_contracts::Blake3Digest32;

/// Opaque optional document provider descriptor. Baseline code records it
/// but never interprets, selects or executes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDescriptor {
    name: String,
}

impl ProviderDescriptor {
    /// Records a provider descriptor name without qualifying it.
    #[must_use]
    pub const fn new(name: String) -> Self {
        Self { name }
    }

    /// Recorded provider name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Validates an optional document provider descriptor.
///
/// Always fails in baseline: without a P17 ADR, accepted P15 and independent
/// review no provider is qualified. An empty descriptor is a malformed
/// qualification request.
pub const fn validate_provider_descriptor(
    descriptor: &ProviderDescriptor,
) -> Result<(), MaterializationError> {
    if descriptor.name.is_empty() {
        return Err(MaterializationError::RequestInvalid);
    }
    Err(MaterializationError::ProviderNotQualified)
}

/// Optional provider failure classification input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProviderFailureKind {
    /// No provider is configured or reachable.
    ProviderAbsent,
    /// A previously present provider was removed.
    ProviderRemoved,
    /// Provider output contradicts its loss evidence.
    ProviderOutputMismatch,
}

/// Explicit fallback decision. Baseline text/code materialization is always
/// available; nothing silently switches providers or relabels lossy output.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProviderFallbackDecision {
    /// Serve baseline text/code materialization explicitly.
    UseBaselineTextCode,
    /// Reject the request instead of degrading silently.
    RejectRequest,
}

/// Classifies an optional provider failure into an explicit fallback.
///
/// Absence and removal fall back to baseline text/code behavior; output that
/// contradicts its loss evidence rejects the request.
#[must_use]
pub const fn classify_provider_failure(kind: ProviderFailureKind) -> ProviderFallbackDecision {
    match kind {
        ProviderFailureKind::ProviderAbsent | ProviderFailureKind::ProviderRemoved => {
            ProviderFallbackDecision::UseBaselineTextCode
        }
        ProviderFailureKind::ProviderOutputMismatch => ProviderFallbackDecision::RejectRequest,
    }
}

/// Claimed optional provider output shape with its loss evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderOutputClaim {
    /// Whether the provider claims exact output.
    pub claimed_exact: bool,
    /// Loss records the provider itself reports.
    pub loss_records: u64,
}

/// Guards provider output claims against relabeling.
///
/// A claim of exact output alongside any reported loss is rejected with
/// [`MaterializationError::ProviderOutputInvalid`]. Honest claims pass this
/// shape check; materialization itself remains baseline-gated.
pub const fn verify_provider_output_claim(
    claim: &ProviderOutputClaim,
) -> Result<(), MaterializationError> {
    if claim.claimed_exact && claim.loss_records > 0 {
        return Err(MaterializationError::ProviderOutputInvalid);
    }
    Ok(())
}

/// Qualifies an optional document provider against fixture evidence.
///
/// Always fails in baseline with [`MaterializationError::ProviderNotQualified`];
/// a zero fixture digest is a malformed qualification request. This seam
/// exists so P17 can attach without changing baseline call sites.
pub fn qualify_provider(
    descriptor: &ProviderDescriptor,
    fixtures_digest: &Blake3Digest32,
) -> Result<(), MaterializationError> {
    if descriptor.name.is_empty() || *fixtures_digest == Blake3Digest32::from_bytes([0; 32]) {
        return Err(MaterializationError::RequestInvalid);
    }
    Err(MaterializationError::ProviderNotQualified)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest() -> Blake3Digest32 {
        Blake3Digest32::from_bytes([5; 32])
    }

    #[test]
    fn descriptors_stay_unqualified() {
        let descriptor = ProviderDescriptor::new("pdf-worker".to_string());
        assert_eq!(
            validate_provider_descriptor(&descriptor),
            Err(MaterializationError::ProviderNotQualified)
        );
        assert_eq!(
            qualify_provider(&descriptor, &digest()),
            Err(MaterializationError::ProviderNotQualified)
        );
    }

    #[test]
    fn empty_descriptors_and_zero_fixtures_are_malformed() {
        let empty = ProviderDescriptor::new(String::new());
        assert_eq!(
            validate_provider_descriptor(&empty),
            Err(MaterializationError::RequestInvalid)
        );
        let descriptor = ProviderDescriptor::new("pdf-worker".to_string());
        assert_eq!(
            qualify_provider(&descriptor, &Blake3Digest32::from_bytes([0; 32])),
            Err(MaterializationError::RequestInvalid)
        );
    }

    #[test]
    fn absence_and_removal_fall_back_to_baseline() {
        assert_eq!(
            classify_provider_failure(ProviderFailureKind::ProviderAbsent),
            ProviderFallbackDecision::UseBaselineTextCode
        );
        assert_eq!(
            classify_provider_failure(ProviderFailureKind::ProviderRemoved),
            ProviderFallbackDecision::UseBaselineTextCode
        );
        assert_eq!(
            classify_provider_failure(ProviderFailureKind::ProviderOutputMismatch),
            ProviderFallbackDecision::RejectRequest
        );
    }

    #[test]
    fn exact_claims_with_loss_are_rejected() {
        assert_eq!(
            verify_provider_output_claim(&ProviderOutputClaim {
                claimed_exact: true,
                loss_records: 2
            }),
            Err(MaterializationError::ProviderOutputInvalid)
        );
        verify_provider_output_claim(&ProviderOutputClaim {
            claimed_exact: false,
            loss_records: 2,
        })
        .expect("honest lossy claim passes shape check");
        verify_provider_output_claim(&ProviderOutputClaim {
            claimed_exact: true,
            loss_records: 0,
        })
        .expect("honest exact claim passes shape check");
    }
}
