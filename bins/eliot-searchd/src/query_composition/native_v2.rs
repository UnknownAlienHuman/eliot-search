//! Ordinary V2 lexical serving over the retained Root and existing query owners.

pub mod handler;
mod output;
mod source;

use search_access::{CompiledEligibilityPopulation, RequestSecurityFence};
use search_provider_protocol::AdmittedEvidenceProviderRequestV2;

use crate::access_composition::{NativeSecurityDomain, VerifiedStandaloneGrant};
use crate::provider_composition::{CanonicalServingError, CanonicalWorkBudget, monotonic_millis};

/// Borrowed inputs to one locked C18 callback. This is no new authority: both
/// opaque owner values must be borrowed from that callback for every operation.
pub struct ReadTurn<'a, 'grant> {
    pub(crate) request: &'a AdmittedEvidenceProviderRequestV2,
    pub(crate) issued: &'a VerifiedStandaloneGrant<'grant>,
    pub(crate) population: &'a CompiledEligibilityPopulation,
    pub(crate) domain: &'a NativeSecurityDomain,
    pub(crate) fence: &'a RequestSecurityFence,
    pub(crate) budget: CanonicalWorkBudget,
}

impl ReadTurn<'_, '_> {
    pub(crate) fn check(&self) -> Result<(), CanonicalServingError> {
        self.budget.check()?;
        if self.request.guard().is_cancelled() {
            return Err(CanonicalServingError::GrantRefused(
                crate::access_composition::GrantUseError::RequestInactive,
            ));
        }
        self.issued
            .verify_current_evidence_claims(&self.request.envelope().grant)
            .map_err(CanonicalServingError::GrantRefused)?;
        if monotonic_millis() >= self.issued.valid_until() {
            return Err(CanonicalServingError::DeadlineExpired);
        }
        Ok(())
    }
}
