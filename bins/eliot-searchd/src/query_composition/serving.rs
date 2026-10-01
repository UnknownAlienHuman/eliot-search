//! Native query-serving composition under one current authority owner.
//!
//! This module owns no planner, retrieval algorithm, source registry or output
//! semantics. It binds one accepted recipe task factory to the existing provider
//! serving loop and constructs the canonical standalone pre-retrieval admission
//! only while the authoritative policy, source/access snapshots, scope resolver,
//! live security domain and output validator are borrowed from one owner.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::task::Poll;

use search_access::{
    AccessCheckpoint, AccessError, AuthoritativeAccessSnapshot, AuthoritativePolicyState,
    IndexedRouteFence, LiveSecurityState, NamespacePolicyFence, OverlapFreeRouteProof,
    RequestSecurityFence, RequestedMembershipScope, check_policy_fence,
};
use search_contracts::{ProviderBodyV1, RequestBody, RequestedScope, SourceMembershipId};
use search_provider_protocol::{AdmittedProviderRequest, BindingContext, MonotonicMillis};

use crate::access_composition::{
    AuthoritativeGrantPolicy, GrantUseError, NativeSecurityDomain, StandaloneAdmissionRequest,
    StandaloneGrantTemplate, StandaloneMembershipSecuritySnapshot,
    StandalonePreRetrievalAdmission, StandaloneScopeResolver,
    compile_standalone_pre_retrieval,
};
use crate::provider_composition::{
    CanonicalRecipeHost, CanonicalRecipeTask, CanonicalServingAuthority,
    CanonicalServingError, CanonicalWorkBudget, CanonicalWorkOutput, monotonic_millis,
};

/// One live authority borrow supplied under the owner's actual mutation lock.
///
/// Values are references to current owner state, not detached permission caches.
/// The resolver and output validator are mutable borrows from that same owner and
/// cannot outlive the callback.
pub(crate) struct CanonicalQueryAuthority<'a> {
    pub policy: &'a AuthoritativeGrantPolicy,
    pub domain: &'a NativeSecurityDomain,
    pub access: &'a AuthoritativeAccessSnapshot,
    pub membership_security: &'a StandaloneMembershipSecuritySnapshot,
    pub planned_policy: NamespacePolicyFence,
    pub authoritative_policy: AuthoritativePolicyState,
    pub route: IndexedRouteFence,
    pub live: &'a LiveSecurityState,
    pub overlap_proof: Option<&'a OverlapFreeRouteProof>,
    pub max_legs: usize,
    pub resolver: &'a mut dyn StandaloneScopeResolver,
    pub validate_output:
        &'a mut dyn FnMut(&RequestBody, &ProviderBodyV1) -> Result<(), AccessError>,
    pub valid_until: MonotonicMillis,
}

/// Sole owner that can borrow the complete current query authority population.
///
/// Implementations hold their actual policy/source/security lock across
/// `operation`. They must not construct this value from request claims, endpoint
/// existence, a cached access permit or handle possession.
pub(crate) trait CanonicalQueryAuthorityOwner {
    fn with_current<R>(
        &mut self,
        binding: &BindingContext,
        request: &mut AdmittedProviderRequest,
        operation: impl FnOnce(
            CanonicalQueryAuthority<'_>,
            &mut AdmittedProviderRequest,
        ) -> Result<R, CanonicalServingError>,
    ) -> Result<R, CanonicalServingError>;
}

/// Accepted recipe factory used after transport/grant admission.
///
/// `prepare` may allocate only bounded request-local state. Source reads,
/// provider calls, IDF, counts, handle publication and result emission are
/// forbidden until [`CanonicalAdmittedRecipeTask::poll_authorized`] receives the
/// canonical admission.
pub(crate) trait CanonicalAdmittedRecipeFactory {
    type Task: CanonicalAdmittedRecipeTask;

    fn prepare(
        &mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
        budget: CanonicalWorkBudget,
    ) -> Result<Self::Task, CanonicalServingError>;
}

/// Existing recipe executor boundary after canonical pre-retrieval admission.
pub(crate) trait CanonicalAdmittedRecipeTask {
    fn poll_authorized(
        &mut self,
        admission: &StandalonePreRetrievalAdmission,
        output: &mut CanonicalWorkOutput<'_, '_>,
        budget: CanonicalWorkBudget,
    ) -> Result<Poll<()>, CanonicalServingError>;

    fn poll_cancel(
        &mut self,
        budget: CanonicalWorkBudget,
    ) -> Result<Poll<()>, CanonicalServingError>;

    fn abort(&mut self);
}

/// Concrete provider host joining one authority owner and one accepted executor.
pub(crate) struct CanonicalQueryHost<O, F> {
    authority: O,
    factory: F,
}

impl<O, F> CanonicalQueryHost<O, F> {
    #[must_use]
    pub(crate) const fn new(authority: O, factory: F) -> Self {
        Self { authority, factory }
    }

    #[must_use]
    pub(crate) fn into_parts(self) -> (O, F) {
        (self.authority, self.factory)
    }
}

/// One request task retaining a snapshot-bound plan but no reusable authority.
pub(crate) struct AuthorityBoundRecipeTask<T> {
    inner: T,
    admission: Option<StandalonePreRetrievalAdmission>,
}

impl<O, F> CanonicalRecipeHost for CanonicalQueryHost<O, F>
where
    O: CanonicalQueryAuthorityOwner,
    F: CanonicalAdmittedRecipeFactory,
{
    type Task = AuthorityBoundRecipeTask<F::Task>;

    fn prepare(
        &mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
        budget: CanonicalWorkBudget,
    ) -> Result<Self::Task, CanonicalServingError> {
        budget.check()?;
        let inner = self.factory.prepare(binding, request, budget)?;
        Ok(AuthorityBoundRecipeTask {
            inner,
            admission: None,
        })
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
        self.authority
            .with_current(binding, request, |current, request| {
                let CanonicalQueryAuthority {
                    policy,
                    domain,
                    access,
                    membership_security,
                    planned_policy,
                    authoritative_policy,
                    route,
                    live,
                    overlap_proof,
                    max_legs,
                    resolver,
                    validate_output,
                    valid_until,
                } = current;

                let requested_scope = request.body().recipe_request.requested_scope.clone();
                let resolved = resolver
                    .resolve(&requested_scope)
                    .map_err(CanonicalServingError::Access)?;
                if resolved.memberships.is_empty() {
                    return Err(CanonicalServingError::Access(
                        AccessError::AuthorizedScopeEmpty,
                    ));
                }
                validate_retained_admission(
                    task.admission.as_ref(),
                    &resolved,
                    access,
                    authoritative_policy,
                    route,
                    overlap_proof,
                    live.generation,
                )?;

                let fence = RequestSecurityFence {
                    planned_generation: access.generation,
                    memberships: resolved.memberships.clone(),
                };
                let exact_live = domain
                    .with_live_checkpoint(&fence, AccessCheckpoint::RequestAdmission, |permit| {
                        permit.live_generation == live.generation
                            && permit.live_snapshot_digest == live.snapshot_digest
                    })
                    .map_err(CanonicalServingError::Access)?;
                if !exact_live {
                    return Err(CanonicalServingError::Access(AccessError::SnapshotStale));
                }
                let request_deadline = request
                    .guard()
                    .deadline()
                    .ok_or(CanonicalServingError::DeadlineExpired)?;
                let valid_until = valid_until.min(request_deadline);
                if monotonic_millis() >= valid_until {
                    return Err(CanonicalServingError::DeadlineExpired);
                }

                let mut fixed_scope = FixedScopeResolver {
                    requested: requested_scope,
                    resolved: Some(resolved),
                };
                let mut compile_admission =
                    |body: &RequestBody, template: &StandaloneGrantTemplate| {
                        compile_standalone_pre_retrieval(
                            &mut fixed_scope,
                            StandaloneAdmissionRequest {
                                body,
                                template,
                                authoritative_access: access,
                                membership_security,
                                requested_policy: &planned_policy,
                                authoritative_policy: &authoritative_policy,
                                route,
                                live,
                                overlap_proof,
                                max_legs,
                            },
                        )
                    };
                let authority = CanonicalServingAuthority {
                    standalone_policy: Some(policy),
                    standalone_grant: None,
                    standalone_admission: Some(&mut compile_admission),
                    domain,
                    fence: &fence,
                    valid_until,
                    validate_output,
                };
                operation(authority, request, task)
            })
    }
}

impl<T: CanonicalAdmittedRecipeTask> CanonicalRecipeTask for AuthorityBoundRecipeTask<T> {
    fn poll(
        &mut self,
        output: &mut CanonicalWorkOutput<'_, '_>,
        budget: CanonicalWorkBudget,
    ) -> Result<Poll<()>, CanonicalServingError> {
        if self.admission.is_none() {
            self.admission = Some(output.admit_standalone_query()?);
        }
        let admission = self
            .admission
            .as_ref()
            .ok_or(CanonicalServingError::InvalidCompletion)?;
        self.inner.poll_authorized(admission, output, budget)
    }

    fn poll_cancel(
        &mut self,
        budget: CanonicalWorkBudget,
    ) -> Result<Poll<()>, CanonicalServingError> {
        self.inner.poll_cancel(budget)
    }

    fn abort(&mut self) {
        self.inner.abort();
    }
}

/// In-process authority owner for the canonical host.
///
/// All fields are private and move into one owner. A state change must construct
/// a new coherent owner or be applied by a future mutation method on this value;
/// no detached field refresh is exposed. Every work turn revalidates the complete
/// bundle before lending any reference to the host.
pub(crate) struct NativeQueryAuthorityOwner<R, V> {
    policy: AuthoritativeGrantPolicy,
    domain: NativeSecurityDomain,
    access: AuthoritativeAccessSnapshot,
    membership_security: StandaloneMembershipSecuritySnapshot,
    planned_policy: NamespacePolicyFence,
    authoritative_policy: AuthoritativePolicyState,
    route: IndexedRouteFence,
    live: LiveSecurityState,
    overlap_proof: Option<OverlapFreeRouteProof>,
    max_legs: usize,
    resolver: R,
    validate_output: V,
}

impl<R, V> NativeQueryAuthorityOwner<R, V>
where
    R: StandaloneScopeResolver,
    V: FnMut(&RequestBody, &ProviderBodyV1) -> Result<(), AccessError>,
{
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        policy: AuthoritativeGrantPolicy,
        domain: NativeSecurityDomain,
        access: AuthoritativeAccessSnapshot,
        membership_security: StandaloneMembershipSecuritySnapshot,
        planned_policy: NamespacePolicyFence,
        authoritative_policy: AuthoritativePolicyState,
        route: IndexedRouteFence,
        live: LiveSecurityState,
        overlap_proof: Option<OverlapFreeRouteProof>,
        max_legs: usize,
        resolver: R,
        validate_output: V,
    ) -> Result<Self, AccessError> {
        let owner = Self {
            policy,
            domain,
            access,
            membership_security,
            planned_policy,
            authoritative_policy,
            route,
            live,
            overlap_proof,
            max_legs,
            resolver,
            validate_output,
        };
        owner.validate_bundle()?;
        Ok(owner)
    }

    fn validate_bundle(&self) -> Result<(), AccessError> {
        if self.live.fail_closed {
            return Err(AccessError::SecurityFailClosed);
        }
        if self.max_legs == 0
            || self.policy.binding_generation == 0
            || self.policy.policy_generation == 0
            || self.policy.allowed_membership_ids.is_empty()
            || self.policy.allowed_access_partitions.is_empty()
            || self.policy.allowed_modalities.is_empty()
            || self.policy.permitted_recipe_families.is_empty()
            || self.policy.allowed_budget_classes.is_empty()
            || (self.policy.exact_scan_permission && !self.policy.source_read_permission)
        {
            return Err(AccessError::ScopeUnauthorized);
        }
        check_policy_fence(&self.planned_policy, &self.authoritative_policy)?;
        if self.membership_security.access_snapshot_digest != self.access.snapshot_digest
            || self.membership_security.memberships.len() != self.access.bindings.len()
            || !self
                .access
                .bindings
                .keys()
                .all(|id| self.membership_security.memberships.contains_key(id))
        {
            return Err(AccessError::SnapshotStale);
        }
        let probe = RequestSecurityFence {
            planned_generation: self.access.generation,
            memberships: BTreeSet::new(),
        };
        let permit = self
            .domain
            .with_live_checkpoint(&probe, AccessCheckpoint::RequestAdmission, |permit| permit)?;
        if permit.live_generation != self.live.generation
            || permit.live_snapshot_digest != self.live.snapshot_digest
        {
            return Err(AccessError::SnapshotStale);
        }
        for membership_id in &self.policy.allowed_membership_ids {
            let binding = self
                .access
                .bindings
                .get(membership_id)
                .ok_or(AccessError::ScopeUnknown)?;
            let security = self
                .membership_security
                .memberships
                .get(membership_id)
                .ok_or(AccessError::SnapshotStale)?;
            if !binding.active
                || binding.membership_id != *membership_id
                || binding.access_partition_digest != security.access_partition_digest
            {
                return Err(AccessError::SnapshotStale);
            }
            if !self
                .policy
                .allowed_access_partitions
                .contains(&security.access_partition_id)
            {
                return Err(AccessError::ScopeUnauthorized);
            }
            if !self.policy.allowed_modalities.contains(&security.modality) {
                return Err(AccessError::ModalityDenied);
            }
        }
        if let Some(proof) = &self.overlap_proof {
            if proof.route != self.route
                || proof.access_snapshot_generation != self.access.generation
                || proof.memberships.is_empty()
                || !proof
                    .memberships
                    .iter()
                    .all(|membership| self.access.bindings.contains_key(membership))
            {
                return Err(AccessError::OverlapProofMissing);
            }
        }
        Ok(())
    }
}

impl<R, V> CanonicalQueryAuthorityOwner for NativeQueryAuthorityOwner<R, V>
where
    R: StandaloneScopeResolver,
    V: FnMut(&RequestBody, &ProviderBodyV1) -> Result<(), AccessError>,
{
    fn with_current<T>(
        &mut self,
        binding: &BindingContext,
        request: &mut AdmittedProviderRequest,
        operation: impl FnOnce(
            CanonicalQueryAuthority<'_>,
            &mut AdmittedProviderRequest,
        ) -> Result<T, CanonicalServingError>,
    ) -> Result<T, CanonicalServingError> {
        self.validate_bundle()
            .map_err(CanonicalServingError::Access)?;
        if self.policy.binding_id != binding.binding_id()
            || self.policy.installation_incarnation_id != binding.incarnation()
        {
            return Err(CanonicalServingError::GrantRefused(
                GrantUseError::BindingMismatch,
            ));
        }
        let valid_until = request
            .guard()
            .deadline()
            .ok_or(CanonicalServingError::DeadlineExpired)?;
        if monotonic_millis() >= valid_until {
            return Err(CanonicalServingError::DeadlineExpired);
        }
        let authority = CanonicalQueryAuthority {
            policy: &self.policy,
            domain: &self.domain,
            access: &self.access,
            membership_security: &self.membership_security,
            planned_policy: self.planned_policy,
            authoritative_policy: self.authoritative_policy,
            route: self.route,
            live: &self.live,
            overlap_proof: self.overlap_proof.as_ref(),
            max_legs: self.max_legs,
            resolver: &mut self.resolver,
            validate_output: &mut self.validate_output,
            valid_until,
        };
        operation(authority, request)
    }
}

struct FixedScopeResolver {
    requested: RequestedScope,
    resolved: Option<RequestedMembershipScope>,
}

impl StandaloneScopeResolver for FixedScopeResolver {
    fn resolve(
        &mut self,
        requested: &RequestedScope,
    ) -> Result<RequestedMembershipScope, AccessError> {
        if requested != &self.requested {
            return Err(AccessError::ScopeUnauthorized);
        }
        self.resolved
            .take()
            .ok_or(AccessError::ScopeUnauthorized)
    }
}

fn validate_retained_admission(
    admission: Option<&StandalonePreRetrievalAdmission>,
    resolved: &RequestedMembershipScope,
    access: &AuthoritativeAccessSnapshot,
    policy: AuthoritativePolicyState,
    route: IndexedRouteFence,
    overlap_proof: Option<&OverlapFreeRouteProof>,
    live_generation: u64,
) -> Result<(), CanonicalServingError> {
    let Some(admission) = admission else {
        return Ok(());
    };
    let resolved_memberships: BTreeSet<SourceMembershipId> =
        resolved.memberships.iter().copied().collect();
    if admission.request_fence.memberships != resolved_memberships
        || admission.request_fence.planned_generation != access.generation
        || admission.scope.access_snapshot_generation != access.generation
        || admission.scope.source_catalog_generation != access.source_catalog_generation
        || admission.scope.membership_generation != access.membership_generation
        || admission.scope.snapshot_digest != access.snapshot_digest
        || admission.scope.memberships.keys().copied().collect::<BTreeSet<_>>()
            != resolved_memberships
    {
        return Err(CanonicalServingError::Access(AccessError::SnapshotStale));
    }
    for leg in &admission.legs {
        if leg.route != route {
            return Err(CanonicalServingError::Access(AccessError::RouteMismatch));
        }
        match leg.overlap_proof_digest.as_ref() {
            Some(digest) => {
                let proof = overlap_proof.ok_or(CanonicalServingError::Access(
                    AccessError::OverlapProofMissing,
                ))?;
                if proof.route != route
                    || &proof.proof_digest != digest
                    || proof.memberships != leg.memberships
                    || proof.access_snapshot_generation != access.generation
                {
                    return Err(CanonicalServingError::Access(
                        AccessError::OverlapProofMissing,
                    ));
                }
            }
            None if leg.memberships.len() > 1 => {
                return Err(CanonicalServingError::Access(
                    AccessError::OverlapProofMissing,
                ));
            }
            None => {}
        }
        for plan in &leg.eligibility_plans {
            if plan.shadow_generation != policy.shadow_revision.get()
                || plan.purge_generation != policy.purge_revision.get()
            {
                return Err(CanonicalServingError::Access(
                    AccessError::PolicyRevisionStale,
                ));
            }
            if plan.live_security_generation > live_generation {
                return Err(CanonicalServingError::Access(
                    AccessError::SecurityFenceStale,
                ));
            }
        }
    }
    Ok(())
}