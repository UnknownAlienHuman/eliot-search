//! Reference-grant verification against the issuing owner's existing ledger.

use search_contracts::{SearchReadGrantClaims, UtcTimestamp};
use search_provider_protocol::MonotonicMillis;

use super::{BoundedStandaloneGrantIssuer, IssuedGrantRecord, StandaloneGrantTemplate};

/// Redacted refusal of a boot-local grant use; no foreign grant-existence oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantUseError {
    /// Unknown, foreign, malformed or altered issuance material.
    InvalidGrant,
    /// Native lifetime evidence or a trustworthy clock observation is missing.
    ClockUnavailable,
    /// Either the original monotonic lifetime or canonical UTC window expired.
    Expired,
    /// The authenticated binding/role does not match this standalone request.
    BindingMismatch,
    /// The live policy source could not prove a current active binding.
    PolicyUnavailable,
    /// Policy identity, generations, ceilings or state changed.
    PolicyChanged,
    /// The recipe, requested scope, budget or permission is not granted.
    RequestDenied,
    /// The admitted request is cancelled, expired or already terminal.
    RequestInactive,
}

impl GrantUseError {
    /// Stable content-free diagnostic, not a new provider-wire reason registry.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidGrant => "DAEMON_GRANT_USE_INVALID",
            Self::ClockUnavailable => "DAEMON_GRANT_USE_CLOCK_UNAVAILABLE",
            Self::Expired => "DAEMON_GRANT_USE_EXPIRED",
            Self::BindingMismatch => "DAEMON_GRANT_USE_BINDING_MISMATCH",
            Self::PolicyUnavailable => "DAEMON_GRANT_USE_POLICY_UNAVAILABLE",
            Self::PolicyChanged => "DAEMON_GRANT_USE_POLICY_CHANGED",
            Self::RequestDenied => "DAEMON_GRANT_USE_REQUEST_DENIED",
            Self::RequestInactive => "DAEMON_GRANT_USE_REQUEST_INACTIVE",
        }
    }
}

impl core::fmt::Display for GrantUseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for GrantUseError {}

/// Trusted clock check over retained issuance data, never echoed client time.
pub trait GrantValidationClock {
    /// Check both UTC and the unchanged boot-local deadline. A missing,
    /// regressed or unrepresentable observation must refuse use. Return the
    /// earlier of the original deadline and the current UTC-derived deadline;
    /// a forward wall-clock move must shorten the actual output budget.
    /// No default implementation turns an issuance clock into lifetime evidence.
    fn check_grant_window(
        &mut self,
        issued_at: &UtcTimestamp,
        expires_at: &UtcTimestamp,
        monotonic_expiry: MonotonicMillis,
    ) -> Result<MonotonicMillis, GrantUseError>;
}

/// Borrowed evidence of an exact issued grant, not a source/disclosure permit.
///
/// The issuer cannot be mutated while this non-clonable view is borrowed.
/// Live policy and full resolved source influence still require their owners;
/// holding this value never pauses expiry or permits output after its deadline.
pub struct VerifiedStandaloneGrant<'a> {
    record: &'a IssuedGrantRecord,
    valid_until: MonotonicMillis,
}

impl VerifiedStandaloneGrant<'_> {
    /// Original deadline tightened by the current UTC observation, in the
    /// daemon's monotonic clock. Never later than the retained issuance expiry.
    #[must_use]
    pub const fn valid_until(&self) -> MonotonicMillis { self.valid_until }

    pub(crate) const fn template(&self) -> &StandaloneGrantTemplate { &self.record.template }

    pub(crate) const fn effective_ttl_ms(&self) -> u64 { self.record.material.effective_ttl_ms }
}

impl<E, T: GrantValidationClock> BoundedStandaloneGrantIssuer<E, T> {
    /// Compare every claim with the original immutable template and receipt.
    ///
    /// This authenticates a stateful reference grant, not a signature invented
    /// from the client's transport MAC. No entropy draw, remint, eviction or
    /// second grant registry occurs. Lookup is bounded by the existing issuer
    /// capacity; changed fields and unknown/foreign IDs share one refusal.
    /// Wall-only/custom issuance without a native deadline fails closed.
    pub fn verify_issued_claims(
        &mut self,
        claims: &SearchReadGrantClaims,
    ) -> Result<VerifiedStandaloneGrant<'_>, GrantUseError> {
        claims.validate_shape().map_err(|_| GrantUseError::InvalidGrant)?;
        let record = self.records.values().find(|record| {
            record.material.grant_id == claims.grant_id
                && record.template.binding_id == claims.binding_id
                && record.template.installation_incarnation_id == claims.installation_incarnation_id
        }).ok_or(GrantUseError::InvalidGrant)?;
        if !matches_issued(record, claims) { return Err(GrantUseError::InvalidGrant); }
        let original_expiry = record.monotonic_expiry.ok_or(GrantUseError::ClockUnavailable)?;
        let valid_until = self.time.check_grant_window(
            &record.material.issued_at, &record.material.expires_at, original_expiry,
        )?;
        if valid_until > original_expiry { return Err(GrantUseError::ClockUnavailable); }
        Ok(VerifiedStandaloneGrant { record, valid_until })
    }
}

fn matches_issued(record: &IssuedGrantRecord, claims: &SearchReadGrantClaims) -> bool {
    let template = &record.template;
    let material = &record.material;
    // Work over every byte of the retained nonce even on a mismatch. Do not
    // reveal a matched nonce prefix through an early-return string comparison.
    let expected = material.nonce.as_str().as_bytes();
    let observed = claims.nonce.as_str().as_bytes();
    let mut difference = u8::from(expected.len() != observed.len());
    for (index, byte) in expected.iter().enumerate() {
        difference |= *byte ^ observed.get(index).copied().unwrap_or(0);
    }
    macro_rules! same_template {
        ($($field:ident),+ $(,)?) => { $(template.$field == claims.$field)&&+ };
    }
    difference == 0
        && material.grant_id == claims.grant_id
        && material.issued_at == claims.issued_at
        && material.expires_at == claims.expires_at
        && same_template!(
            installation_id, installation_incarnation_id, binding_id,
            principal_opaque_id, client_scope_ref, scope_domain_id,
            allowed_membership_ids, allowed_corpus_or_portfolio_ids,
            reference_portfolio_revision, allowed_access_partitions,
            allowed_modalities, permitted_recipe_families, maximum_budget_class,
            sensitivity_ceiling, disclosure_ceiling, source_read_permission,
            exact_scan_permission, issued_boot_id, revocation_generation,
        )
}
