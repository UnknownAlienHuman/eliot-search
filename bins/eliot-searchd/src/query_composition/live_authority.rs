//! Live native query authority shared with one bounded update owner.
//!
//! The serving owner and updater serialize through one fail-closed mutex. A work
//! turn holds the lock across scope resolution, admission, backend work and output;
//! an update can therefore replace only the complete coherent authority bundle
//! between turns. No field-level refresh, cached permit or claims-derived state is
//! exposed.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

use search_access::{
    AccessCheckpoint, AccessError, AuthoritativeAccessSnapshot, AuthoritativePolicyState,
    IndexedRouteFence, LiveSecurityState, NamespacePolicyFence, OverlapFreeRouteProof,
    RequestSecurityFence, check_policy_fence,
};
use search_contracts::{ProviderBodyV1, RequestBody};
use search_provider_protocol::{AdmittedProviderRequest, BindingContext};

use crate::access_composition::{
    AuthoritativeGrantPolicy, GrantUseError, NativeSecurityDomain,
    StandaloneMembershipSecuritySnapshot, StandaloneScopeResolver,
};
use crate::provider_composition::{CanonicalServingError, monotonic_millis};
use crate::query_serving_composition::{
    CanonicalQueryAuthority, CanonicalQueryAuthorityOwner,
};

/// One complete current authority image accepted by the native query host.
///
/// The value owns the actual security domain together with every snapshot and
/// policy coordinate needed to prove that domain current. Construction validates
/// the whole bundle before it can enter the serving owner.
pub(crate) struct LiveNativeQuerySnapshot {
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
}

impl LiveNativeQuerySnapshot {
    /// Validate and retain one coherent native query-authority image.
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
    ) -> Result<Self, AccessError> {
        let snapshot = Self {
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
        };
        snapshot.validate_bundle()?;
        Ok(snapshot)
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

    fn validate_successor(&self, next: &Self) -> Result<(), AccessError> {
        next.validate_bundle()?;
        if next.policy.installation_id != self.policy.installation_id
            || next.policy.installation_incarnation_id
                != self.policy.installation_incarnation_id
        {
            return Err(AccessError::InstallationMismatch);
        }
        if next.policy.binding_id != self.policy.binding_id {
            return Err(AccessError::GrantBindingMismatch);
        }
        if next.policy.principal_opaque_id != self.policy.principal_opaque_id
            || next.policy.client_scope_ref != self.policy.client_scope_ref
            || next.policy.scope_domain_id != self.policy.scope_domain_id
            || next.policy.issued_boot_id != self.policy.issued_boot_id
        {
            return Err(AccessError::ScopeUnauthorized);
        }
        if next.policy.binding_generation < self.policy.binding_generation
            || next.policy.revocation_generation < self.policy.revocation_generation
        {
            return Err(AccessError::SecurityGenerationRegression);
        }
        if next.policy.policy_generation < self.policy.policy_generation {
            return Err(AccessError::PolicyRevisionStale);
        }
        if next.policy.policy_generation == self.policy.policy_generation
            && next.policy != self.policy
        {
            return Err(AccessError::PolicyRevisionStale);
        }
        if next.access.generation < self.access.generation
            || next.access.source_catalog_generation < self.access.source_catalog_generation
            || next.access.membership_generation < self.access.membership_generation
        {
            return Err(AccessError::SnapshotStale);
        }
        if next.access.generation == self.access.generation
            && (next.access != self.access
                || next.membership_security != self.membership_security)
        {
            return Err(AccessError::SnapshotStale);
        }
        if next.authoritative_policy.namespace_id
            != self.authoritative_policy.namespace_id
        {
            return Err(AccessError::NamespaceUnknown);
        }
        if next.authoritative_policy.owner_generation
            < self.authoritative_policy.owner_generation
        {
            return Err(AccessError::OwnerMismatch);
        }
        if next.authoritative_policy.policy_revision
            < self.authoritative_policy.policy_revision
            || next.authoritative_policy.shadow_revision
                < self.authoritative_policy.shadow_revision
            || next.authoritative_policy.purge_revision
                < self.authoritative_policy.purge_revision
        {
            return Err(AccessError::PolicyRevisionStale);
        }
        if next.route.owner_epoch < self.route.owner_epoch
            || next.route.route_generation < self.route.route_generation
            || (next.route.route_generation == self.route.route_generation
                && next.route != self.route)
        {
            return Err(AccessError::RouteMismatch);
        }
        if next.live.generation < self.live.generation {
            return Err(AccessError::SecurityGenerationRegression);
        }
        if next.live.generation == self.live.generation && next.live != self.live {
            return Err(AccessError::SnapshotStale);
        }
        if next.max_legs != self.max_legs {
            return Err(AccessError::RetrievalLegBudgetExceeded);
        }
        Ok(())
    }
}

struct LiveNativeQueryState<R, V> {
    snapshot: LiveNativeQuerySnapshot,
    resolver: R,
    validate_output: V,
}

/// Serving-side owner of one live native query authority bundle.
///
/// This type is intentionally non-clonable. Its companion updater is the sole
/// mutation capability; both share only the private mutex-protected state.
pub(crate) struct LiveNativeQueryAuthorityOwner<R, V> {
    state: Arc<Mutex<LiveNativeQueryState<R, V>>>,
}

/// Sole bounded update capability for [`LiveNativeQueryAuthorityOwner`].
///
/// Updates use `try_lock`: they never wait behind source work or output. A busy
/// turn is an explicit retryable conflict for the caller, while poisoning fails
/// closed. Replacement is all-or-nothing after full bundle and monotonicity
/// validation.
pub(crate) struct LiveNativeQueryAuthorityUpdater<R, V> {
    state: Arc<Mutex<LiveNativeQueryState<R, V>>>,
}

impl<R, V> LiveNativeQueryAuthorityOwner<R, V>
where
    R: StandaloneScopeResolver,
    V: FnMut(&RequestBody, &ProviderBodyV1) -> Result<(), AccessError>,
{
    /// Create exactly one serving owner and one non-clonable update capability.
    pub(crate) fn new(
        snapshot: LiveNativeQuerySnapshot,
        resolver: R,
        validate_output: V,
    ) -> (Self, LiveNativeQueryAuthorityUpdater<R, V>) {
        let state = Arc::new(Mutex::new(LiveNativeQueryState {
            snapshot,
            resolver,
            validate_output,
        }));
        (
            Self {
                state: Arc::clone(&state),
            },
            LiveNativeQueryAuthorityUpdater { state },
        )
    }
}

impl<R, V> LiveNativeQueryAuthorityUpdater<R, V>
where
    R: StandaloneScopeResolver,
    V: FnMut(&RequestBody, &ProviderBodyV1) -> Result<(), AccessError>,
{
    /// Replace the complete current authority image between serving turns.
    ///
    /// The resolver and output validator move with the snapshot so catalog,
    /// policy and rendering authority cannot be refreshed independently. On a
    /// busy turn no state changes and the caller may retry under its own budget.
    pub(crate) fn replace(
        &mut self,
        snapshot: LiveNativeQuerySnapshot,
        resolver: R,
        validate_output: V,
    ) -> Result<(), AccessError> {
        let mut current = try_lock(&self.state)?;
        current.snapshot.validate_successor(&snapshot)?;
        *current = LiveNativeQueryState {
            snapshot,
            resolver,
            validate_output,
        };
        Ok(())
    }
}

impl<R, V> CanonicalQueryAuthorityOwner for LiveNativeQueryAuthorityOwner<R, V>
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
        let mut state = try_lock(&self.state).map_err(CanonicalServingError::Access)?;
        state
            .snapshot
            .validate_bundle()
            .map_err(CanonicalServingError::Access)?;
        if state.snapshot.policy.binding_id != binding.binding_id()
            || state.snapshot.policy.installation_incarnation_id != binding.incarnation()
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

        let LiveNativeQueryState {
            snapshot,
            resolver,
            validate_output,
        } = &mut *state;
        let LiveNativeQuerySnapshot {
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
        } = snapshot;
        let authority = CanonicalQueryAuthority {
            policy,
            domain,
            access,
            membership_security,
            planned_policy: *planned_policy,
            authoritative_policy: *authoritative_policy,
            route: *route,
            live,
            overlap_proof: overlap_proof.as_ref(),
            max_legs: *max_legs,
            resolver,
            validate_output,
            valid_until,
        };
        operation(authority, request)
    }
}

fn try_lock<R, V>(
    state: &Arc<Mutex<LiveNativeQueryState<R, V>>>,
) -> Result<MutexGuard<'_, LiveNativeQueryState<R, V>>, AccessError> {
    match state.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::WouldBlock) => Err(AccessError::SecurityOperationConflict),
        Err(TryLockError::Poisoned(_)) => Err(AccessError::SecurityFailClosed),
    }
}
