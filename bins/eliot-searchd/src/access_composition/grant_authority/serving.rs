//! Standalone grant enforcement inside the native serving host's lock scope.

use search_access::{AccessError, RequestSecurityFence};
use search_contracts::{RequestBody, RequestedScope, protocol::PeerRole};
use search_provider_protocol::{AdmittedProviderRequest, BindingContext};

use crate::provider_composition::{
    CanonicalRecipeHost, CanonicalServingAuthority, CanonicalServingError,
    CanonicalServingLimits, CanonicalServingOwner, CanonicalTcpConnection, CanonicalWorkBudget,
    monotonic_millis,
};
use super::RecipeGrantUseError;
use super::super::grant::{
    BoundedStandaloneGrantIssuer, GrantUseError, GrantValidationClock,
    VerifiedStandaloneGrant,
};

/// Native host plus the original standalone issuer, bound to one paired context.
///
/// Construct through [`CanonicalServingOwner::new_standalone`]. Durable policy
/// reads remain operation-scoped during grant issuance; this serving adapter
/// stores only the original boot-local issuer ledger. The source host still owns
/// current policy, scope resolution, live domain, admission and output under one
/// lock. It creates no task, source registry, lock or allow-all backend.
pub struct StandaloneGrantRecipeHost<E, T, H> {
    // Drop source resources before the existing issuance owner.
    host: H,
    issuer: BoundedStandaloneGrantIssuer<E, T>,
    binding: BindingContext,
}

impl<E, T, H> CanonicalServingOwner<StandaloneGrantRecipeHost<E, T, H>>
where
    T: GrantValidationClock,
    H: CanonicalRecipeHost,
{
    /// Connect a negotiated standalone transport to its original issuer ledger.
    ///
    /// Native bootstrap must transfer the issuer that actually minted the
    /// client's grant, not replace it with an empty or copied ledger. Current
    /// durable policy is supplied by the source host under its live authority
    /// lock for every work turn. Existing owner limits and fresh-session checks
    /// still apply. This method does not start a listener or manufacture missing
    /// source/executor owners.
    ///
    /// # Errors
    ///
    /// Rejects a closed/non-standalone connection or invalid owner limits/state.
    /// Construction failure drops the transferred transport and its key.
    pub fn new_standalone(
        transport: CanonicalTcpConnection,
        host: H,
        issuer: BoundedStandaloneGrantIssuer<E, T>,
        limits: CanonicalServingLimits,
    ) -> Result<Self, CanonicalServingError> {
        let binding = transport
            .session()
            .ok_or(CanonicalServingError::Closed)?
            .binding_context();
        if binding.role() != PeerRole::StandaloneCli {
            return Err(CanonicalServingError::GrantRefused(
                GrantUseError::BindingMismatch,
            ));
        }
        Self::new(
            transport,
            StandaloneGrantRecipeHost {
                host,
                issuer,
                binding,
            },
            limits,
        )
    }
}

impl<E, T, H> CanonicalRecipeHost for StandaloneGrantRecipeHost<E, T, H>
where
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
        // Ledger-only early refusal. This is never a current-policy permit;
        // with_current_authority repeats the full check under the source lock.
        self.issuer
            .preflight_issued_recipe_grant(binding, request)
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
            CanonicalServingAuthority<'_, '_>,
            &mut AdmittedProviderRequest,
            &mut Self::Task,
        ) -> Result<R, CanonicalServingError>,
    ) -> Result<R, CanonicalServingError> {
        self.require_binding(binding)?;
        let issuer = &mut self.issuer;
        self.host
            .with_current_authority(binding, request, task, |live, request, task| {
                let policy = live.standalone_policy.ok_or(
                    CanonicalServingError::GrantRefused(GrantUseError::PolicyUnavailable),
                )?;
                issuer
                    .with_locked_recipe_grant(binding, request, policy, |issued, request| {
                        validate_influence(&issued, request.body(), live.fence)?;
                        let valid_until = live.valid_until.min(issued.valid_until());
                        if monotonic_millis() >= valid_until {
                            return Err(CanonicalServingError::GrantRefused(
                                GrantUseError::Expired,
                            ));
                        }
                        // Move the non-clonable issuer-ledger proof into this one
                        // live authority turn. The task receives neither
                        // claims-derived authority nor the raw template.
                        let authorized = CanonicalServingAuthority {
                            standalone_policy: live.standalone_policy,
                            standalone_grant: Some(issued),
                            standalone_admission: live.standalone_admission,
                            domain: live.domain,
                            fence: live.fence,
                            valid_until,
                            validate_output: live.validate_output,
                        };
                        operation(authorized, request, task)
                    })
                    .map_err(map_grant_use)
            })
    }
}

impl<E, T, H> StandaloneGrantRecipeHost<E, T, H> {
    fn require_binding(&self, binding: &BindingContext) -> Result<(), CanonicalServingError> {
        // BindingContext equality includes the originating pairing ceremony.
        // This adapter cannot be transplanted to another connection's context.
        if binding != &self.binding {
            return Err(CanonicalServingError::GrantRefused(
                GrantUseError::BindingMismatch,
            ));
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
        return Err(CanonicalServingError::Access(
            AccessError::AuthorizedScopeEmpty,
        ));
    }
    // Check the whole authoritative influence population, not only returned
    // hits. Refuse before scanning a host-supplied set larger than the grant.
    if fence.memberships.len() > allowed.len()
        || !fence.memberships.iter().all(|id| allowed.contains(id))
    {
        return Err(CanonicalServingError::Access(
            AccessError::ScopeUnauthorized,
        ));
    }
    if let RequestedScope::ExplicitMemberships(requested) =
        &request.recipe_request.requested_scope
    {
        if !requested.iter().all(|id| fence.memberships.contains(id)) {
            return Err(CanonicalServingError::Access(
                AccessError::ScopeUnauthorized,
            ));
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
        RecipeGrantUseError::AfterOperation(error) => {
            CanonicalServingError::GrantAfterOperation(error)
        }
    }
}
