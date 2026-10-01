//! Canonical standalone-grant admission before retrieval or filtered IDF.
//!
//! The provider serving owner supplies only an issuer-verified
//! [`StandaloneGrantTemplate`]. Scope syntax is resolved by a server-owned
//! resolver, then this adapter rechecks the exact grant ceilings against one
//! coherent authoritative access/security snapshot before delegating safe-leg
//! and live-barrier decisions to `search-access`.
//!
//! This is deliberately not a second access engine: it owns only the daemon
//! mapping from the canonical P00 grant to the existing package-owned access
//! primitives. No client-provided filter, route, partition, modality, point ID,
//! handle or local-process identity becomes authority here.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use search_access::{
    AccessCheckpoint, AccessError, AccessPermit, AuthoritativeAccessSnapshot,
    AuthoritativePolicyState, AuthorizedScope, BaseEligibilityPlan, EligibilityPredicates,
    IndexedRouteFence, LiveSecurityState, NamespacePolicyFence, OverlapFreeRouteProof,
    RequestSecurityFence, RequestedMembershipScope, SafeRetrievalLeg, check_policy_fence,
    compile_safe_legs, intersect_scope, recheck_live_access,
};
use search_contracts::{
    AccessPartitionId, Blake3Digest32, GrantFence, Modality, RequestBody, RequestedScope,
    SourceMembershipId,
};

use super::super::grant::StandaloneGrantTemplate;

/// Server-owned resolver for client scope syntax.
///
/// Implementations resolve corpus, portfolio, workspace and source-handle
/// locators from current authoritative owners. They must not use client claims
/// as registry facts. This adapter independently intersects every returned
/// membership with the exact issuer-verified grant and authoritative access
/// snapshot, so even a faulty resolver cannot widen the grant.
pub trait StandaloneScopeResolver {
    /// Resolve one requested scope to exact membership identities.
    fn resolve(
        &mut self,
        requested: &RequestedScope,
    ) -> Result<RequestedMembershipScope, AccessError>;
}

/// Canonical source-level ceilings not present in the older vendor-neutral
/// [`search_access::MembershipAccessBinding`].
///
/// The digest must equal the binding's access-partition digest. Keeping the ID,
/// digest and modality together prevents a stale or foreign metadata map from
/// authorizing an otherwise matching membership key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandaloneMembershipSecurity {
    pub access_partition_id: AccessPartitionId,
    pub access_partition_digest: Blake3Digest32,
    pub modality: Modality,
}

/// Source-security metadata captured with one exact authoritative access
/// snapshot. The snapshot digest is an equality fence, not a reusable permit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneMembershipSecuritySnapshot {
    pub access_snapshot_digest: Blake3Digest32,
    pub memberships: BTreeMap<SourceMembershipId, StandaloneMembershipSecurity>,
}

/// Captured inputs for one canonical standalone admission.
///
/// All references are immutable snapshots. The function performs no I/O,
/// persists nothing and returns no reusable authority. Callers must still hold
/// the native security-domain lock and repeat live checks at every execution,
/// readback and emission checkpoint.
pub struct StandaloneAdmissionRequest<'a> {
    pub body: &'a RequestBody,
    pub template: &'a StandaloneGrantTemplate,
    pub authoritative_access: &'a AuthoritativeAccessSnapshot,
    pub membership_security: &'a StandaloneMembershipSecuritySnapshot,
    pub requested_policy: &'a NamespacePolicyFence,
    pub authoritative_policy: &'a AuthoritativePolicyState,
    pub route: IndexedRouteFence,
    pub live: &'a LiveSecurityState,
    pub overlap_proof: Option<&'a OverlapFreeRouteProof>,
    pub max_legs: usize,
}

/// Canonical grant/scope admission plus the request fence that must be retained
/// through execution. The admission permit is valid only for
/// [`AccessCheckpoint::RequestAdmission`] and cannot authorize later work.
#[derive(Debug)]
pub struct StandalonePreRetrievalAdmission {
    pub grant_fence: GrantFence,
    pub scope: AuthorizedScope,
    pub legs: Vec<SafeRetrievalLeg>,
    pub predicates: Vec<EligibilityPredicates>,
    pub request_fence: RequestSecurityFence,
    pub admission_permit: AccessPermit,
}

/// Compile one issuer-verified standalone request to non-widening safe legs.
///
/// Fixed order: exact template/request correspondence, recipe and budget
/// ceiling, namespace/owner/policy fence, server scope resolution, explicit
/// scope equality, grant membership/partition/modality intersection,
/// authoritative access intersection, safe-leg compilation and the mandatory
/// live admission barrier. No source byte or provider call occurs before all
/// checks succeed.
pub fn compile_standalone_pre_retrieval<R: StandaloneScopeResolver>(
    resolver: &mut R,
    request: StandaloneAdmissionRequest<'_>,
) -> Result<StandalonePreRetrievalAdmission, AccessError> {
    validate_grant_template(request.body, request.template)?;
    check_policy_fence(request.requested_policy, request.authoritative_policy)?;

    let resolved = resolver.resolve(&request.body.recipe_request.requested_scope)?;
    validate_explicit_resolution(&request.body.recipe_request.requested_scope, &resolved)?;
    validate_membership_security(
        request.template,
        &resolved,
        request.authoritative_access,
        request.membership_security,
    )?;

    let allowed_memberships = request
        .template
        .allowed_membership_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let scope = intersect_scope(
        &resolved,
        &allowed_memberships,
        request.authoritative_access,
    )?;
    let legs = compile_safe_legs(
        &scope,
        request.route,
        request.live.generation,
        request.authoritative_policy.shadow_revision.get(),
        request.authoritative_policy.purge_revision.get(),
        request.overlap_proof,
        request.max_legs,
    )?;
    let request_fence = RequestSecurityFence {
        planned_generation: request.authoritative_access.generation,
        memberships: scope.memberships.keys().copied().collect(),
    };
    let admission_permit = recheck_live_access(
        &request_fence,
        request.live,
        AccessCheckpoint::RequestAdmission,
    )?;
    let predicates = legs
        .iter()
        .flat_map(|leg| {
            leg.eligibility_plans
                .iter()
                .map(BaseEligibilityPlan::predicates)
        })
        .collect();
    Ok(StandalonePreRetrievalAdmission {
        grant_fence: GrantFence {
            grant_id: request.body.grant.grant_id,
            revocation_generation: request.body.grant.revocation_generation,
        },
        scope,
        legs,
        predicates,
        request_fence,
        admission_permit,
    })
}

fn validate_grant_template(
    body: &RequestBody,
    template: &StandaloneGrantTemplate,
) -> Result<(), AccessError> {
    body.grant
        .validate_shape()
        .map_err(|_| AccessError::ScopeUnauthorized)?;
    let claims = &body.grant;
    if claims.binding_id != template.binding_id
        || claims.installation_id != template.installation_id
        || claims.installation_incarnation_id != template.installation_incarnation_id
        || claims.principal_opaque_id != template.principal_opaque_id
        || claims.client_scope_ref != template.client_scope_ref
        || claims.scope_domain_id != template.scope_domain_id
        || claims.issued_boot_id != template.issued_boot_id
        || claims.revocation_generation != template.revocation_generation
    {
        return Err(AccessError::GrantBindingMismatch);
    }
    if claims.allowed_membership_ids != template.allowed_membership_ids
        || claims.allowed_corpus_or_portfolio_ids
            != template.allowed_corpus_or_portfolio_ids
        || claims.reference_portfolio_revision != template.reference_portfolio_revision
        || claims.allowed_access_partitions != template.allowed_access_partitions
        || claims.allowed_modalities != template.allowed_modalities
        || claims.permitted_recipe_families != template.permitted_recipe_families
        || claims.maximum_budget_class != template.maximum_budget_class
        || claims.sensitivity_ceiling != template.sensitivity_ceiling
        || claims.disclosure_ceiling != template.disclosure_ceiling
        || claims.source_read_permission != template.source_read_permission
        || claims.exact_scan_permission != template.exact_scan_permission
    {
        return Err(AccessError::ScopeUnauthorized);
    }
    let recipe = body.recipe_request.recipe;
    if body.recipe_request.body.recipe_id() != recipe
        || !template.permitted_recipe_families.contains(&recipe)
    {
        return Err(AccessError::RecipeDenied);
    }
    if body.recipe_request.requested_budget_class != template.maximum_budget_class {
        return Err(AccessError::BudgetClassDenied);
    }
    Ok(())
}

fn validate_explicit_resolution(
    requested: &RequestedScope,
    resolved: &RequestedMembershipScope,
) -> Result<(), AccessError> {
    if resolved.memberships.is_empty() {
        return Err(AccessError::AuthorizedScopeEmpty);
    }
    if let RequestedScope::ExplicitMemberships(expected) = requested {
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        if expected != resolved.memberships {
            return Err(AccessError::ScopeUnauthorized);
        }
    }
    Ok(())
}

fn validate_membership_security(
    template: &StandaloneGrantTemplate,
    resolved: &RequestedMembershipScope,
    authoritative: &AuthoritativeAccessSnapshot,
    security: &StandaloneMembershipSecuritySnapshot,
) -> Result<(), AccessError> {
    if security.access_snapshot_digest != authoritative.snapshot_digest {
        return Err(AccessError::SnapshotStale);
    }
    for membership_id in &resolved.memberships {
        if !template.allowed_membership_ids.contains(membership_id) {
            return Err(AccessError::ScopeUnauthorized);
        }
        let binding = authoritative
            .bindings
            .get(membership_id)
            .ok_or(AccessError::ScopeUnknown)?;
        let metadata = security
            .memberships
            .get(membership_id)
            .ok_or(AccessError::SnapshotStale)?;
        if binding.membership_id != *membership_id
            || binding.access_partition_digest != metadata.access_partition_digest
        {
            return Err(AccessError::SnapshotStale);
        }
        if !template
            .allowed_access_partitions
            .contains(&metadata.access_partition_id)
        {
            return Err(AccessError::ScopeUnauthorized);
        }
        if !template.allowed_modalities.contains(&metadata.modality) {
            return Err(AccessError::ModalityDenied);
        }
    }
    Ok(())
}
