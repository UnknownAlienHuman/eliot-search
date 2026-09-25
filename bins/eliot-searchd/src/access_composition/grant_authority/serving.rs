//! Standalone grant enforcement inside the native serving host's lock scope.

use search_access::{AccessError, RequestSecurityFence};
use search_contracts::{RequestBody, RequestedScope, protocol::PeerRole};
use search_provider_protocol::{AdmittedProviderRequest, BindingContext};

use crate::provider_composition::{
    CanonicalRecipeHost, CanonicalServingAuthority, CanonicalServingError,
    CanonicalServingLimits, CanonicalServingOwner, CanonicalTcpConnection, CanonicalWorkBudget,
    monotonic_millis,
};
use super::{RecipeGrantUseError, SessionBoundGrantAuthority, StandaloneGrantPolicySource};
use super::super::grant::{
    BoundedStandaloneGrantIssuer, GrantUseError, GrantValidationClock, VerifiedStandaloneGrant,
};

/// Native host plus the original standalone issuer, bound to one paired context.
///
/// Construct through CanonicalServingOwner::new_standalone. The source host
/// still owns scope resolution, plan/currentness validation, disclosure and its
/// actual policy/domain lock. This adapter enforces the issued grant inside that
/// lock before the serving owner dispatches work or writes any result.
/// It creates no task, issuer, source registry, lock or allow-all backend.
/// Tasks remain H::Task and retain the existing owner's cancellation/cleanup.
pub struct StandaloneGrantRecipeHost<P, E, T, H> {
    // Drop source resources before the existing issuance owner.
    host: H,
    authority: SessionBoundGrantAuthority<P, BoundedStandaloneGrantIssuer<E, T>>,
    binding: BindingContext,
}

impl<P, E, T, H> CanonicalServingOwner<StandaloneGrantRecipeHost<P, E, T, H>>
where
    P: StandaloneGrantPolicySource,
    T: GrantValidationClock,
    H: CanonicalRecipeHost,
{
    /// Connect a negotiated standalone transport to its original grant authority.
    ///
    /// Native bootstrap must transfer the issuer that actually minted the
    /// client's grant, not replace it with an empty or copied ledger. The source
    /// host must return its current policy from under the same lock as the live
    /// domain, and must not do source/provider work outside its callback.
    /// Existing owner limits and fresh-session checks still apply. This method
    /// does not start a listener or manufacture missing source/executor owners.
    ///
    /// # Errors
    ///
    /// Rejects a closed/non-standalone connection or invalid owner limits/state.
    /// Construction failure drops the transferred transport and its key.
    pub fn new_standalone(
        transport: CanonicalTcpConnection,
        host: H,
        authority: SessionBoundGrantAuthority<P, BoundedStandaloneGrantIssuer<E, T>>,
        limits: CanonicalServingLimits,
    ) -> Result<Self, CanonicalServingError> {
        let binding = transport.session().ok_or(CanonicalServingError::Closed)?.binding_context();
        if binding.role() != PeerRole::StandaloneCli {
            return Err(CanonicalServingError::GrantRefused(GrantUseError::BindingMismatch));
        }
        Self::new(transport, StandaloneGrantRecipeHost { host, authority, binding }, limits)
    }
}

impl<P, E, T, H> CanonicalRecipeHost for StandaloneGrantRecipeHost<P, E, T, H>
where
    P: StandaloneGrantPolicySource,
    T: GrantValidationClock,
    H: CanonicalRecipeHost,
{
    type Task = H::Task;

    fn prepare(
        &mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
        budget: CanonicalWorkBudget,
    ) -> Result<Self::Task, CanonicalServingError> {
        self.require_binding(binding)?;
        budget.check()?;
        // This snapshot lookup is outside the source host's lock. It is only
        // an early refusal, never a permit to skip the locked check below.
        self.authority.preflight_recipe_grant(binding, request)
            .map_err(CanonicalServingError::GrantRefused)?;
        budget.check()?;
        // No fallible post-step here: the serving owner first retains the
        // returned task, then checks its budget, preserving cleanup on expiry.
        self.host.prepare(binding, request, budget)
    }

    fn with_current_authority<R>(
        &mut self,
        binding: &BindingContext,
        request: &mut AdmittedProviderRequest,
        task: &mut Self::Task,
        operation: impl FnOnce(
            CanonicalServingAuthority<'_>,
            &mut AdmittedProviderRequest,
            &mut Self::Task,
        ) -> Result<R, CanonicalServingError>,
    ) -> Result<R, CanonicalServingError> {
        self.require_binding(binding)?;
        let grants = &mut self.authority;
        self.host.with_current_authority(binding, request, task, |mut live, request, task| {
            let policy = live.standalone_policy
                .ok_or(CanonicalServingError::GrantRefused(GrantUseError::PolicyUnavailable))?;
            // Borrow policy from the host's actual lock, rather than re-entering
            // its snapshot source and potentially deadlocking on the same lock.
            grants.with_locked_recipe_grant(binding, request, policy, |issued, request| {
                validate_influence(&issued, request.body(), live.fence)?;
                live.valid_until = live.valid_until.min(issued.valid_until());
                if monotonic_millis() >= live.valid_until {
                    return Err(CanonicalServingError::GrantRefused(GrantUseError::Expired));
                }
                // The original output validator/domain borrow is passed through
                // intact. This expiry reaches task budgets and each TCP write.
                operation(live, request, task)
            }).map_err(map_grant_use)
        })
    }
}

impl<P, E, T, H> StandaloneGrantRecipeHost<P, E, T, H> {
    fn require_binding(&self, binding: &BindingContext) -> Result<(), CanonicalServingError> {
        // BindingContext equality includes the originating pairing ceremony.
        // This adapter cannot be transplanted to another connection's context.
        if binding != &self.binding {
            return Err(CanonicalServingError::GrantRefused(GrantUseError::BindingMismatch));
        }
        Ok(())
    }
}

fn validate_influence(
    issued: &VerifiedStandaloneGrant<'_>,
    request: &RequestBody,
    fence: &RequestSecurityFence,
) -> Result<(), CanonicalServingError> {
    let allowed = &issued.template().allowed_membership_ids;
    if fence.memberships.is_empty() {
        return Err(CanonicalServingError::Access(AccessError::AuthorizedScopeEmpty));
    }
    // Check the whole authoritative influence population, not only returned
    // hits. Refuse before scanning a host-supplied set larger than the grant.
    if fence.memberships.len() > allowed.len()
        || !fence.memberships.iter().all(|id| allowed.contains(id))
    {
        return Err(CanonicalServingError::Access(AccessError::ScopeUnauthorized));
    }
    if let RequestedScope::ExplicitMemberships(requested) = &request.recipe_request.requested_scope {
        if !requested.iter().all(|id| fence.memberships.contains(id)) {
            return Err(CanonicalServingError::Access(AccessError::ScopeUnauthorized));
        }
    }
    // Workspace, handle, corpus and portfolio expansion stays with the native
    // resolver. A granted subset alone does not prove correct scope resolution.
    Ok(())
}

fn map_grant_use(error: RecipeGrantUseError<CanonicalServingError>) -> CanonicalServingError {
    match error {
        RecipeGrantUseError::Refused(error) => CanonicalServingError::GrantRefused(error),
        RecipeGrantUseError::Operation(error) => error,
        RecipeGrantUseError::AfterOperation(error) => CanonicalServingError::GrantAfterOperation(error),
    }
}
