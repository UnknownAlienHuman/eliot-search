//! Revalidate a retained standalone grant around a canonical recipe work turn.

use search_contracts::{
    CorpusOrPortfolioId, HandleExpansionKind, RecipeBodyV1, ReferencePortfolioScope,
    RequestBody, RequestedScope, SearchReadGrantClaims, protocol::PeerRole,
};
use search_provider_protocol::{AdmittedProviderRequest, BindingContext, MonotonicMillis};

use crate::provider_composition::monotonic_millis;
use super::{SessionBoundGrantAuthority, StandaloneGrantPolicySource, validate_policy_binding};
use super::super::grant::{
    AuthoritativeGrantPolicy, BoundedStandaloneGrantIssuer, GrantUseError,
    GrantValidationClock, VerifiedStandaloneGrant,
};

/// Preserve whether failure occurred before or after the work callback ran.
/// An after-operation failure cannot retract output or prove mutation rollback.
#[derive(Debug)]
pub enum RecipeGrantUseError<E> {
    /// No callback was invoked and no source work was authorized here.
    Refused(GrantUseError),
    /// Preserve the actual callback error and its own effect classification.
    Operation(E),
    /// The callback returned, but current authority/time no longer validates.
    AfterOperation(GrantUseError),
}

impl<P, E, T> SessionBoundGrantAuthority<P, BoundedStandaloneGrantIssuer<E, T>>
where
    P: StandaloneGrantPolicySource,
    T: GrantValidationClock,
{
    /// Verify the client's exact issued claims and current policy around work.
    ///
    /// The host must retain the real policy/security-domain lock across this
    /// call. Snapshot equality is a change detector, not a replacement for that
    /// lock. The serving owner must have revalidated this exact request against
    /// its BoundSession. No local token, handle, echoed flag or MAC substitutes
    /// for the original issuer record. Verification never issues another grant.
    ///
    /// The callback receives the original grant deadline for every backend and
    /// socket boundary. It must still resolve workspace/handle/portfolio scope,
    /// intersect authoritative memberships, validate plan modalities and budgets,
    /// and run live domain/currentness/disclosure checks on the actual output.
    /// This layer validates a grant, not a source catalog, planner or disclosure.
    ///
    /// No native expiry is reconstructed from client timestamps. Both the
    /// original monotonic lifetime and current UTC must remain valid. On a
    /// callback/after-operation error or unwind, the serving owner must close
    /// the transport and retain its task cleanup; no output repair or retry is
    /// performed here. In particular, an error is not proof of rollback.
    pub fn with_current_recipe_grant<R, F>(
        &mut self,
        binding: &BindingContext,
        request: &mut AdmittedProviderRequest,
        operation: impl FnOnce(
            VerifiedStandaloneGrant<'_>,
            &mut AdmittedProviderRequest,
        ) -> Result<R, F>,
    ) -> Result<R, RecipeGrantUseError<F>> {
        use RecipeGrantUseError::{AfterOperation, Operation, Refused};

        validate_bound_request(binding, request).map_err(Refused)?;
        let before = self.policy_source.snapshot(binding)
            .map_err(|_| Refused(GrantUseError::PolicyUnavailable))?;
        let issued = self.verify_recipe_grant(binding, request, &before).map_err(Refused)?;

        let valid_until = issued.valid_until();
        let result = operation(issued, request).map_err(Operation)?;

        let after = self.policy_source.snapshot(binding)
            .map_err(|_| AfterOperation(GrantUseError::PolicyUnavailable))?;
        if before != after {
            return Err(AfterOperation(GrantUseError::PolicyChanged));
        }
        self.revalidate_after_operation(request, valid_until).map_err(AfterOperation)?;
        Ok(result)
    }

    /// Refuse invalid grants before allocating source-free task state. This
    /// produces no permit: execution must repeat validation under the host lock.
    pub(super) fn preflight_recipe_grant(
        &mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
    ) -> Result<(), GrantUseError> {
        validate_bound_request(binding, request)?;
        let policy = self.policy_source.snapshot(binding)
            .map_err(|_| GrantUseError::PolicyUnavailable)?;
        let _issued = self.verify_recipe_grant(binding, request, &policy)?;
        Ok(())
    }

    /// Use the current policy borrowed from the native host's held lock.
    ///
    /// Only the serving adapter calls this path. It does not call snapshot()
    /// again while that lock is held: a policy source may acquire the same
    /// non-reentrant lock. The host must borrow the actual active policy, not a
    /// detached cached copy, and hold its mutation lock through this entire call.
    /// The issuer's independent lifetime checks still bracket actual work/output.
    pub(super) fn with_locked_recipe_grant<R, F>(
        &mut self,
        binding: &BindingContext,
        request: &mut AdmittedProviderRequest,
        policy: &AuthoritativeGrantPolicy,
        operation: impl FnOnce(
            VerifiedStandaloneGrant<'_>,
            &mut AdmittedProviderRequest,
        ) -> Result<R, F>,
    ) -> Result<R, RecipeGrantUseError<F>> {
        use RecipeGrantUseError::{AfterOperation, Operation, Refused};

        let issued = self.verify_recipe_grant(binding, request, policy).map_err(Refused)?;
        let valid_until = issued.valid_until();
        let result = operation(issued, request).map_err(Operation)?;
        self.revalidate_after_operation(request, valid_until).map_err(AfterOperation)?;
        Ok(result)
    }

    // One validation owner for unlocked preflight, snapshot-based callers and
    // locked serving. Only the original issuance record supplies grant evidence.
    fn verify_recipe_grant<'a>(
        &'a mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
        policy: &AuthoritativeGrantPolicy,
    ) -> Result<VerifiedStandaloneGrant<'a>, GrantUseError> {
        validate_bound_request(binding, request)?;
        validate_policy_binding(binding, policy).map_err(|_| GrantUseError::BindingMismatch)?;
        let issued = self.issuer.verify_issued_claims(&request.body().grant)?;
        validate_policy(&request.body().grant, &issued, policy)?;
        validate_request(request.body())?;
        check_request(request)?;
        if monotonic_millis() >= issued.valid_until() { return Err(GrantUseError::Expired); }
        Ok(issued)
    }

    fn revalidate_after_operation(
        &mut self,
        request: &AdmittedProviderRequest,
        before_expiry: MonotonicMillis,
    ) -> Result<(), GrantUseError> {
        // Fresh UTC may shorten this turn's deadline, never lengthen the
        // expiry supplied before the callback, even below the original TTL.
        let issued = self.issuer.verify_issued_claims(&request.body().grant)?;
        let now = monotonic_millis();
        if now >= before_expiry.min(issued.valid_until()) { return Err(GrantUseError::Expired); }
        if now < request.guard().admitted_at() || request.guard().is_expired(now)
            || (request.next_event_sequence().is_some() && request.guard().is_cancelled())
        {
            return Err(GrantUseError::RequestInactive);
        }
        Ok(())
    }
}

fn validate_bound_request(
    binding: &BindingContext,
    request: &AdmittedProviderRequest,
) -> Result<(), GrantUseError> {
    let body = request.body();
    if binding.role() != PeerRole::StandaloneCli
        || body.grant.binding_id != binding.binding_id()
        || body.grant.installation_incarnation_id != binding.incarnation()
        || body.recipe_request.request_id != *request.guard().request_id()
    {
        return Err(GrantUseError::BindingMismatch);
    }
    check_request(request)
}

fn check_request(request: &AdmittedProviderRequest) -> Result<(), GrantUseError> {
    let now = monotonic_millis();
    if request.next_event_sequence().is_none() || request.guard().is_cancelled()
        || now < request.guard().admitted_at() || request.guard().is_expired(now)
    {
        return Err(GrantUseError::RequestInactive);
    }
    Ok(())
}

fn validate_policy(
    claims: &SearchReadGrantClaims,
    issued: &VerifiedStandaloneGrant<'_>,
    policy: &AuthoritativeGrantPolicy,
) -> Result<(), GrantUseError> {
    let template = issued.template();
    if policy.binding_generation == 0 || policy.policy_generation == 0
        || template.binding_generation != policy.binding_generation
        || template.policy_generation != policy.policy_generation
        || claims.installation_id != policy.installation_id
        || claims.issued_boot_id != policy.issued_boot_id
        || claims.principal_opaque_id != policy.principal_opaque_id
        || claims.client_scope_ref != policy.client_scope_ref
        || claims.scope_domain_id != policy.scope_domain_id
        || claims.revocation_generation != policy.revocation_generation
        || issued.effective_ttl_ms() == 0 || issued.effective_ttl_ms() > policy.maximum_ttl_ms
        || (policy.exact_scan_permission && !policy.source_read_permission)
    {
        return Err(GrantUseError::PolicyChanged);
    }
    // Recheck the actual ceilings even when a faulty source reused a generation.
    // Opaque budget profiles have no ordinal relationship: membership is exact.
    macro_rules! subset {
        ($field:ident) => { claims.$field.iter().all(|item| policy.$field.contains(item)) };
    }
    if !subset!(allowed_membership_ids) || !subset!(allowed_corpus_or_portfolio_ids)
        || !subset!(allowed_access_partitions) || !subset!(allowed_modalities)
        || !subset!(permitted_recipe_families)
        || !policy.allowed_budget_classes.contains(&claims.maximum_budget_class)
        || claims.sensitivity_ceiling > policy.sensitivity_ceiling
        || claims.disclosure_ceiling > policy.disclosure_ceiling
        || (claims.source_read_permission && !policy.source_read_permission)
        || (claims.exact_scan_permission && !policy.exact_scan_permission)
        || (claims.reference_portfolio_revision.is_some()
            && claims.reference_portfolio_revision != policy.reference_portfolio_revision)
    {
        return Err(GrantUseError::PolicyChanged);
    }
    Ok(())
}

fn validate_request(body: &RequestBody) -> Result<(), GrantUseError> {
    let request = &body.recipe_request;
    let claims = &body.grant;
    if request.body.recipe_id() != request.recipe
        || !claims.permitted_recipe_families.contains(&request.recipe)
        || request.requested_budget_class != claims.maximum_budget_class
    {
        return Err(GrantUseError::RequestDenied);
    }
    match &request.requested_scope {
        RequestedScope::ExplicitMemberships(memberships) => {
            if memberships.is_empty()
                || !memberships.iter().all(|id| claims.allowed_membership_ids.contains(id))
            {
                return Err(GrantUseError::RequestDenied);
            }
        }
        RequestedScope::Corpus(id) => {
            if !claims.allowed_corpus_or_portfolio_ids.contains(&CorpusOrPortfolioId::Corpus(*id)) {
                return Err(GrantUseError::RequestDenied);
            }
        }
        RequestedScope::ReferencePortfolio(scope) => validate_portfolio(claims, scope)?,
        // These locators are not resolved into authority here. The mandatory
        // host scope intersection must resolve them before any source access.
        RequestedScope::ActiveWorkspace(_) | RequestedScope::SourceHandle(_) => {}
    }
    match &request.body {
        RecipeBodyV1::CompareImplementations(value) => validate_portfolio(claims, &value.references)?,
        RecipeBodyV1::CompileExactScan { .. } | RecipeBodyV1::ExecuteExactScan(_) => {
            if !claims.source_read_permission || !claims.exact_scan_permission {
                return Err(GrantUseError::RequestDenied);
            }
        }
        RecipeBodyV1::ExpandHandle(value) if value.expansion == HandleExpansionKind::Excerpt => {
            if !claims.source_read_permission { return Err(GrantUseError::RequestDenied); }
        }
        _ => {}
    }
    Ok(())
}

fn validate_portfolio(
    claims: &SearchReadGrantClaims,
    scope: &ReferencePortfolioScope,
) -> Result<(), GrantUseError> {
    if claims.reference_portfolio_revision != Some(scope.portfolio_revision)
        || !claims.allowed_corpus_or_portfolio_ids
            .contains(&CorpusOrPortfolioId::Portfolio(scope.portfolio_id))
    {
        return Err(GrantUseError::RequestDenied);
    }
    Ok(())
}
