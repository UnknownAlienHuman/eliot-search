//! Typed V2 entry through the existing live Root owner and original issuer.

use search_access::{
    CompiledEligibilityPopulation, LiveSecurityState, RequestSecurityFence,
    RequestedMembershipScope,
};
use search_provider_protocol::{AdmittedEvidenceProviderRequestV2, BindingContext};

use super::{LiveNativeQueryAuthorityOwner, LiveNativeQueryState, try_lock};
use crate::access_composition::{
    BoundedStandaloneGrantIssuer, GrantUseError, GrantValidationClock, NativeSecurityDomain,
    RecipeGrantUseError, StandaloneScopeResolver, VerifiedStandaloneGrant,
};
use crate::provider_composition::{CanonicalServingError, monotonic_millis};
use crate::query_serving_composition::canonical_population::{
    CanonicalPopulationLiveAuthority, compile_current_evidence_population,
};

impl<R: StandaloneScopeResolver, V> LiveNativeQueryAuthorityOwner<R, V> {
    /// Compile real lexical V2 population while holding the same live owner lock.
    /// The callback receives data/private C18 proof, not a new authority or ledger.
    /// Current lifetime, the original V2 guard and cancellation bracket all work.
    pub(crate) fn with_retrieve_evidence_population<E, T: GrantValidationClock, O>(
        &self,
        binding: &BindingContext,
        issuer: &mut BoundedStandaloneGrantIssuer<E, T>,
        request: &mut AdmittedEvidenceProviderRequestV2,
        operation: impl FnOnce(
            &CompiledEligibilityPopulation,
            &RequestedMembershipScope,
            &RequestSecurityFence,
            &NativeSecurityDomain,
            &LiveSecurityState,
            &VerifiedStandaloneGrant<'_>,
            &mut AdmittedEvidenceProviderRequestV2,
        ) -> Result<O, CanonicalServingError>,
    ) -> Result<O, CanonicalServingError> {
        let mut state = try_lock(&self.state).map_err(CanonicalServingError::Access)?;
        state
            .snapshot
            .validate_bundle()
            .map_err(CanonicalServingError::Access)?;
        if state.snapshot.publication_guards.profile_digest != self.profile_digest {
            return Err(CanonicalServingError::Access(
                search_access::AccessError::RouteMismatch,
            ));
        }
        if state.snapshot.policy.binding_id != binding.binding_id()
            || state.snapshot.policy.installation_incarnation_id != binding.incarnation()
        {
            return Err(CanonicalServingError::GrantRefused(
                GrantUseError::BindingMismatch,
            ));
        }
        let LiveNativeQueryState {
            snapshot, resolver, ..
        } = &mut *state;
        let result =
            issuer.with_locked_evidence_grant(
                binding,
                request,
                &snapshot.policy,
                |issued, request| {
                    let deadline = request
                        .guard()
                        .deadline()
                        .ok_or(CanonicalServingError::DeadlineExpired)?;
                    if monotonic_millis() >= deadline.min(issued.valid_until()) {
                        return Err(CanonicalServingError::DeadlineExpired);
                    }
                    let metadata = snapshot.canonical_population.as_ref().ok_or(
                        CanonicalServingError::Access(search_access::AccessError::SnapshotStale),
                    )?;
                    let current = CanonicalPopulationLiveAuthority::borrow_current(
                        &snapshot.policy,
                        &snapshot.access,
                        &snapshot.membership_security,
                        &snapshot.sources,
                        &snapshot.planned_policy,
                        &snapshot.authoritative_policy,
                        snapshot.route,
                        &snapshot.publication_visibility,
                        &snapshot.live,
                        &snapshot.domain,
                    );
                    let (population, scope, fence) = compile_current_evidence_population(
                        metadata, &current, request, &issued, resolver,
                    )
                    .map_err(CanonicalServingError::Access)?;
                    // Metadata compilation spends this same admitted lifetime. Do
                    // not enter backend/source work if it expired or was cancelled
                    // while compiling; the post-callback issuer check is too late.
                    if request.guard().is_cancelled() {
                        return Err(CanonicalServingError::GrantRefused(
                            GrantUseError::RequestInactive,
                        ));
                    }
                    if monotonic_millis() >= deadline.min(issued.valid_until()) {
                        return Err(CanonicalServingError::DeadlineExpired);
                    }
                    operation(
                        &population,
                        &scope,
                        &fence,
                        &snapshot.domain,
                        &snapshot.live,
                        &issued,
                        request,
                    )
                },
            );
        drop(state);
        result.map_err(|error| match error {
            RecipeGrantUseError::Refused(error) | RecipeGrantUseError::AfterOperation(error) => {
                CanonicalServingError::GrantRefused(error)
            }
            RecipeGrantUseError::Operation(error) => error,
        })
    }
}
