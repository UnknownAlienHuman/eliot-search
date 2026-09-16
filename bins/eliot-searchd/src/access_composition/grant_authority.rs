//! Session-bound standalone grant authority composition.
//!
//! A standalone grant may be minted only for one active mutually authenticated
//! provider session and two equal reads of the server-owned binding policy.
//! The policy source owns persistence/currentness; this module owns only the
//! fixed composition order and never treats a local token, ACL or locator as
//! authority.

#![allow(clippy::module_name_repetitions)]

use search_contracts::{SearchReadGrantClaims, protocol::PeerRole};
use search_provider_protocol::{BindingContext, BoundSession};

use super::grant::{
    AuthoritativeGrantPolicy, GrantMintError, StandaloneGrantIssuer,
    StandaloneGrantRequest, mint_standalone_grant,
};

/// Server-owned source for the current active policy of one authenticated
/// binding.
///
/// Implementations must return only a current active record for the supplied
/// binding and installation incarnation. Revoked, expired, missing, unreadable
/// or outcome-unknown state must return `Err`; it must never be reconstructed
/// from client request fields.
pub trait StandaloneGrantPolicySource {
    /// Package-specific source failure. Details are deliberately not exposed to
    /// the client-facing authority result.
    type Error;

    /// Reads one immutable authoritative binding-policy snapshot.
    ///
    /// # Errors
    ///
    /// Returns the source-specific failure when no current active snapshot can
    /// be proved for this binding.
    fn snapshot(
        &mut self,
        binding: &BindingContext,
    ) -> Result<AuthoritativeGrantPolicy, Self::Error>;
}

/// Closed session-bound grant authority failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantAuthorityError {
    /// The provider connection is no longer active.
    SessionInactive,
    /// This operation is standalone-only and the authenticated role differs.
    PeerRoleDenied,
    /// Current binding policy could not be read safely.
    PolicyUnavailable,
    /// The policy record is not bound to the authenticated session.
    PolicyBindingMismatch,
    /// Binding or policy state changed while the grant was being issued.
    PolicyChangedDuringIssuance,
    /// Exact grant intersection or issuance failed.
    Mint(GrantMintError),
}

impl GrantAuthorityError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::SessionInactive => "DAEMON_GRANT_SESSION_INACTIVE",
            Self::PeerRoleDenied => "DAEMON_GRANT_PEER_ROLE_DENIED",
            Self::PolicyUnavailable => "DAEMON_GRANT_POLICY_UNAVAILABLE",
            Self::PolicyBindingMismatch => "DAEMON_GRANT_POLICY_BINDING_MISMATCH",
            Self::PolicyChangedDuringIssuance => {
                "DAEMON_GRANT_POLICY_CHANGED_DURING_ISSUANCE"
            }
            Self::Mint(error) => error.code(),
        }
    }
}

impl From<GrantMintError> for GrantAuthorityError {
    fn from(error: GrantMintError) -> Self {
        Self::Mint(error)
    }
}

/// Fixed composition root for standalone grant issuance.
///
/// The authority owns an injected policy source and issuer. It does not own a
/// transport, binding database, source catalog or access policy. One successful
/// call requires an active standalone session, an exact pre-issuance policy
/// snapshot, successful non-widening minting and an equal post-issuance policy
/// snapshot before any claims are returned.
#[derive(Debug)]
pub struct SessionBoundGrantAuthority<P, I> {
    policy_source: P,
    issuer: I,
}

impl<P, I> SessionBoundGrantAuthority<P, I> {
    /// Creates one authority from explicit policy and issuer owners.
    #[must_use]
    pub const fn new(policy_source: P, issuer: I) -> Self {
        Self {
            policy_source,
            issuer,
        }
    }
}

impl<P, I> SessionBoundGrantAuthority<P, I>
where
    P: StandaloneGrantPolicySource,
    I: StandaloneGrantIssuer,
{
    /// Mints one grant for an active mutually authenticated standalone session.
    ///
    /// Policy is read both before and after issuer mutation. If it changes, the
    /// result is discarded and the operation identity remains consumed by the
    /// issuer, preventing a blind retry under changed authority.
    ///
    /// # Errors
    ///
    /// Returns [`GrantAuthorityError`] when the session, policy snapshots or
    /// exact non-widening mint cannot be validated.
    pub fn mint(
        &mut self,
        session: &BoundSession,
        request: &StandaloneGrantRequest,
    ) -> Result<SearchReadGrantClaims, GrantAuthorityError> {
        let binding = active_standalone_binding(session)?;
        let before = self
            .policy_source
            .snapshot(&binding)
            .map_err(|_| GrantAuthorityError::PolicyUnavailable)?;
        validate_policy_binding(&binding, &before)?;

        let claims = mint_standalone_grant(&mut self.issuer, request, &before)?;

        let after = self
            .policy_source
            .snapshot(&binding)
            .map_err(|_| GrantAuthorityError::PolicyUnavailable)?;
        validate_policy_binding(&binding, &after)?;
        if before != after {
            return Err(GrantAuthorityError::PolicyChangedDuringIssuance);
        }
        Ok(claims)
    }
}

fn active_standalone_binding(
    session: &BoundSession,
) -> Result<BindingContext, GrantAuthorityError> {
    if !session.is_active() {
        return Err(GrantAuthorityError::SessionInactive);
    }
    let binding = session.binding_context();
    if binding.role() != PeerRole::StandaloneCli {
        return Err(GrantAuthorityError::PeerRoleDenied);
    }
    Ok(binding)
}

fn validate_policy_binding(
    binding: &BindingContext,
    policy: &AuthoritativeGrantPolicy,
) -> Result<(), GrantAuthorityError> {
    if policy.binding_id != binding.binding_id()
        || policy.installation_incarnation_id != binding.incarnation()
    {
        return Err(GrantAuthorityError::PolicyBindingMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
