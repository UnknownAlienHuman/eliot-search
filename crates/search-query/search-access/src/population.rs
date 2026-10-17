//! Canonical C00 population compilation from typed owner observations.
//!
//! This path deliberately does not translate the legacy opaque projection ID
//! or partition digests into C00 IDs. Owner-produced C00 IDs are input data;
//! this compiler intersects them with the current grant, scope and security
//! snapshots, then returns the single bounded predicate consumed by retrieval,
//! filtered IDF, count and scroll.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::indexed::{CollectionSchema, EligibilityPopulation};
use search_contracts::{
    AccessPartitionId, AccessPolicyRevision, AuthoritativeGrantPolicy, Blake3ExactInputLimit,
    CollectionGenerationId, CollectionRoute, GrantFence, InstallationIncarnationId,
    MAX_CANONICAL_BYTES, MAX_COLLECTION_ITEMS, Modality, ObservationFreshnessState,
    OverlayRevision, ProjectionMembershipId, ProjectionProfileSetId, PurgeFenceRevision,
    QuerySnapshotFence, ReceiptRef, RecipeIdV1, ScoringPartitionId, SearchReadGrantClaims,
    ShadowFenceRevision, SourceMembershipId, SourceNamespaceId, SourceView, blake3_exact_bytes,
};

use crate::{
    AccessError, AuthoritativeAccessSnapshot, AuthoritativePolicyState, LiveSecurityState,
    NamespacePolicyFence, RequestedMembershipScope, check_policy_fence, intersect_scope,
};

/// One projection owner's exact typed identity for a source membership.
///
/// These values are copied from the accepted projection and manifest owner.
/// They are never derived from a legacy digest or caller-provided alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalProjectionMembership {
    pub source_membership_id: SourceMembershipId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub collection_generation_id: CollectionGenerationId,
    pub projection_membership_id: ProjectionMembershipId,
    pub access_partition_id: AccessPartitionId,
    pub scoring_partition_id: ScoringPartitionId,
    pub projection_profile_set_id: ProjectionProfileSetId,
}

/// The current owner snapshot of live deny and purge state.
///
/// Revisions bind this data to a [`QuerySnapshotFence`]. The embedded live
/// generation is its own sequence and is never numerically compared with an
/// access-policy, shadow, purge or overlay revision. Publication shadows are
/// supplied by the C02 snapshot's separate keyed map.
pub struct CanonicalEligibilitySecuritySnapshot<'a> {
    pub live: &'a LiveSecurityState,
    pub namespace_id: SourceNamespaceId,
    pub access_policy_revision: AccessPolicyRevision,
    pub shadow_fence_revision: ShadowFenceRevision,
    pub purge_fence_revision: PurgeFenceRevision,
    pub overlay_revision: OverlayRevision,
}

/// Trusted owner adapter over the existing issuer and current typed registry.
///
/// This is a producer dependency, not a provider/client extension point. The
/// implementation must use the existing private Root/protocol-issued token and
/// original issuer ledger, keep the current policy/security lock throughout the
/// compiler call, and refuse unavailable or stale original fences. C18 provides
/// no implementation or constructor that verifies caller flags or digests.
pub trait CanonicalPopulationVerificationPort {
    /// Existing producer-owned, privately constructed issued-grant observation.
    /// It must retain the exact issuance/nonce and original native lifetime.
    type IssuedGrant: ?Sized;

    /// Revalidate the original issued token, current policy and native lifetime,
    /// returning every retained claim field, including grant ID and full nonce.
    /// Never return claims copied from the caller without issuer-ledger proof.
    fn verified_issued_claims(
        &mut self,
        issued: &Self::IssuedGrant,
    ) -> Result<SearchReadGrantClaims, AccessError>;

    /// Independently resolve the current typed C03/manifest-owner binding under
    /// the exact original query fence. No candidate row or legacy opaque/digest
    /// alias is supplied to this read. Missing typed ownership fails closed.
    /// A retained-revision view must match the actual frozen source revision;
    /// its filesystem observation freshness is not proof of that match.
    fn current_projection_binding(
        &mut self,
        issued: &Self::IssuedGrant,
        membership_id: SourceMembershipId,
        fence: &QuerySnapshotFence,
    ) -> Result<CanonicalProjectionMembership, AccessError>;
}

/// Compiled C00 data and its original fences; never an access/emission permit.
///
/// Fields are private and only the owner-port compiler constructs this result.
pub struct CompiledEligibilityPopulation {
    population: EligibilityPopulation,
    projection_memberships: BTreeMap<SourceMembershipId, CanonicalProjectionMembership>,
    snapshot_fence: QuerySnapshotFence,
    grant_fence: GrantFence,
}

impl CompiledEligibilityPopulation {
    /// The single C00 value shared by retrieval, IDF, count and scroll.
    #[must_use]
    pub const fn population(&self) -> &EligibilityPopulation {
        &self.population
    }

    /// Exact source/projection links independently resolved by the owner during
    /// this compilation. Retrieval must not substitute a caller mapping later.
    #[must_use]
    pub const fn projection_memberships(
        &self,
    ) -> &BTreeMap<SourceMembershipId, CanonicalProjectionMembership> {
        &self.projection_memberships
    }

    #[must_use]
    pub const fn snapshot_fence(&self) -> &QuerySnapshotFence {
        &self.snapshot_fence
    }

    #[must_use]
    pub const fn grant_fence(&self) -> &GrantFence {
        &self.grant_fence
    }
}

/// Inputs from the current grant, access, route, projection and security owners.
///
/// Presented claims, maps and candidate rows remain data. Independent issuer
/// and typed-binding observations come only through the owner port supplied to
/// the compiler. This record creates no permit, receipt or persistence.
pub struct CanonicalEligibilityPopulationRequest<'a, S, R = RecipeIdV1> {
    pub grant: &'a SearchReadGrantClaims,
    pub grant_policy: &'a AuthoritativeGrantPolicy,
    pub recipe: R,
    pub modality: Modality,
    pub requested_scope: &'a RequestedMembershipScope,
    pub authoritative_access: &'a AuthoritativeAccessSnapshot,
    pub namespace_policy_fence: &'a NamespacePolicyFence,
    pub authoritative_policy: &'a AuthoritativePolicyState,
    pub snapshot_fence: &'a QuerySnapshotFence,
    pub collection_schema: &'a CollectionSchema,
    pub collection_route: &'a CollectionRoute,
    /// Committed manifest map from one C02 `PublicationProjectionSnapshot`.
    /// Only keys are consumed; receipt references remain opaque.
    pub publication_manifests: &'a BTreeMap<ProjectionMembershipId, ReceiptRef>,
    /// Publication source-shadow map from that same C02 snapshot.
    /// Only membership-key presence is consumed; the C02 value remains opaque.
    pub publication_shadows: &'a BTreeMap<ProjectionMembershipId, S>,
    /// Typed projection/access/scoring/profile owner rows keyed by exact C00 ID.
    pub projection_memberships: &'a BTreeMap<ProjectionMembershipId, CanonicalProjectionMembership>,
    pub security: CanonicalEligibilitySecuritySnapshot<'a>,
}

/// Compile one exact C00 eligibility population from current typed owner data.
///
/// Every selected source membership must have one matching typed projection
/// row, and all rows must share one access partition, scoring partition and
/// profile set. A denied, purged or shadowed selected member fails the whole
/// compilation so this value cannot silently claim complete scope after
/// narrowing. The returned value is the same closed contract passed to all
/// retrieval, IDF, count and scroll consumers.
pub fn compile_canonical_eligibility_population<
    S,
    O: CanonicalPopulationVerificationPort + ?Sized,
>(
    owner: &mut O,
    issued: &O::IssuedGrant,
    request: &CanonicalEligibilityPopulationRequest<'_, S>,
) -> Result<CompiledEligibilityPopulation, AccessError> {
    validate_issued_claims(owner, issued, request.grant)?;
    validate_grant_policy(
        request.grant,
        request.grant_policy,
        request.recipe,
        request.modality,
    )?;
    compile_verified_population(owner, issued, request)
}

fn compile_verified_population<S, R, O: CanonicalPopulationVerificationPort + ?Sized>(
    owner: &mut O,
    issued: &O::IssuedGrant,
    request: &CanonicalEligibilityPopulationRequest<'_, S, R>,
) -> Result<CompiledEligibilityPopulation, AccessError> {
    validate_snapshot_owners(request)?;
    if request.requested_scope.memberships.len() > MAX_COLLECTION_ITEMS {
        return Err(AccessError::EligibilityPopulationTooLarge);
    }

    let grant_memberships = request
        .grant
        .allowed_membership_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let scope = intersect_scope(
        request.requested_scope,
        &grant_memberships,
        request.authoritative_access,
    )?;
    if scope.memberships.is_empty() {
        return Err(AccessError::AuthorizedScopeEmpty);
    }
    if scope.memberships.len() > MAX_COLLECTION_ITEMS {
        return Err(AccessError::EligibilityPopulationTooLarge);
    }

    validate_live_restrictions(&scope.memberships.keys().copied().collect(), request)?;
    let (population, projection_memberships) =
        compile_population_memberships(&scope.memberships, request, owner, issued)?;
    validate_issued_claims(owner, issued, request.grant)?;
    Ok(CompiledEligibilityPopulation {
        population,
        projection_memberships,
        snapshot_fence: request.snapshot_fence.clone(),
        grant_fence: GrantFence {
            grant_id: request.grant.grant_id,
            revocation_generation: request.grant.revocation_generation,
        },
    })
}

fn validate_issued_claims<O: CanonicalPopulationVerificationPort + ?Sized>(
    owner: &mut O,
    issued: &O::IssuedGrant,
    presented: &SearchReadGrantClaims,
) -> Result<(), AccessError> {
    if owner.verified_issued_claims(issued)? != *presented {
        return Err(AccessError::GrantBindingMismatch);
    }
    Ok(())
}

fn validate_grant_policy(
    grant: &SearchReadGrantClaims,
    policy: &AuthoritativeGrantPolicy,
    recipe: RecipeIdV1,
    modality: Modality,
) -> Result<(), AccessError> {
    validate_common_grant_policy(grant, policy)?;
    if grant.permitted_recipe_families.is_empty() {
        return Err(AccessError::ScopeUnauthorized);
    }
    if !grant.permitted_recipe_families.contains(&recipe)
        || !policy.permitted_recipe_families.contains(&recipe)
    {
        return Err(AccessError::RecipeDenied);
    }
    validate_modality(grant, policy, modality)
}

fn validate_common_grant_policy(
    grant: &SearchReadGrantClaims,
    policy: &AuthoritativeGrantPolicy,
) -> Result<(), AccessError> {
    grant
        .validate_shape()
        .map_err(|_| AccessError::ScopeUnauthorized)?;
    policy
        .validate_shape()
        .map_err(|_| AccessError::ScopeUnauthorized)?;

    if grant.binding_id != policy.binding_id {
        return Err(AccessError::GrantBindingMismatch);
    }
    if grant.installation_id != policy.installation_id
        || grant.installation_incarnation_id != policy.installation_incarnation_id
    {
        return Err(AccessError::InstallationMismatch);
    }
    if grant.issued_boot_id != policy.issued_boot_id {
        return Err(AccessError::GrantBootMismatch);
    }
    if grant.revocation_generation != policy.revocation_generation {
        return Err(AccessError::GrantRevoked);
    }

    if grant.principal_opaque_id != policy.principal_opaque_id
        || grant.client_scope_ref != policy.client_scope_ref
        || grant.scope_domain_id != policy.scope_domain_id
        || grant.allowed_membership_ids.is_empty()
        || grant.allowed_access_partitions.is_empty()
        || grant.allowed_modalities.is_empty()
        || !grant
            .allowed_membership_ids
            .iter()
            .all(|id| policy.allowed_membership_ids.contains(id))
        || !grant
            .allowed_corpus_or_portfolio_ids
            .iter()
            .all(|id| policy.allowed_corpus_or_portfolio_ids.contains(id))
        || !grant
            .allowed_access_partitions
            .iter()
            .all(|id| policy.allowed_access_partitions.contains(id))
        || !grant
            .allowed_modalities
            .iter()
            .all(|id| policy.allowed_modalities.contains(id))
        || !grant
            .permitted_recipe_families
            .iter()
            .all(|id| policy.permitted_recipe_families.contains(id))
        || !policy
            .allowed_budget_classes
            .contains(&grant.maximum_budget_class)
        || grant.reference_portfolio_revision != policy.reference_portfolio_revision
        || grant.sensitivity_ceiling > policy.sensitivity_ceiling
        || grant.disclosure_ceiling > policy.disclosure_ceiling
        || (grant.source_read_permission && !policy.source_read_permission)
        || (grant.exact_scan_permission && !policy.exact_scan_permission)
    {
        return Err(AccessError::ScopeUnauthorized);
    }
    Ok(())
}

fn validate_modality(
    grant: &SearchReadGrantClaims,
    policy: &AuthoritativeGrantPolicy,
    modality: Modality,
) -> Result<(), AccessError> {
    if !grant.allowed_modalities.contains(&modality)
        || !policy.allowed_modalities.contains(&modality)
    {
        return Err(AccessError::ModalityDenied);
    }
    Ok(())
}

fn validate_snapshot_owners(
    request: &CanonicalEligibilityPopulationRequest<'_, impl Sized, impl Sized>,
) -> Result<(), AccessError> {
    let fence = request.snapshot_fence;
    fence.validate().map_err(|_| AccessError::SnapshotStale)?;
    // S16.2 requires live observation continuity for current-workspace queries.
    // An explicit immutable retained revision instead uses the owner's exact
    // source/manifest binding. Keep its reported freshness unchanged rather
    // than inventing a current-worktree observation for a historical source.
    if fence.observation_freshness.state != ObservationFreshnessState::CurrentConfirmed
        && !matches!(fence.source_view, SourceView::RetainedRevision(_))
    {
        return Err(AccessError::SnapshotStale);
    }
    validate_snapshot_fingerprint(fence)?;
    request
        .collection_schema
        .validate()
        .map_err(|_| AccessError::RouteMismatch)?;
    check_policy_fence(request.namespace_policy_fence, request.authoritative_policy)?;
    if request.security.namespace_id != request.authoritative_policy.namespace_id {
        return Err(AccessError::NamespaceUnknown);
    }

    let schema = request.collection_schema;
    let route = request.collection_route;
    let visible_epoch = fence.visible_epoch.ok_or(AccessError::RouteMismatch)?;
    if fence.installation_incarnation_id != *schema.installation_incarnation_id()
        || fence.installation_incarnation_id != request.grant.installation_incarnation_id
        || fence.collection_generation_id != Some(*schema.collection_generation_id())
        || route.collection_generation_id != *schema.collection_generation_id()
        || route.committed_visible_epoch != visible_epoch
        || route.collection_route_revision != fence.collection_route_revision
    {
        return Err(AccessError::RouteMismatch);
    }
    let schema_identity = schema.identity().map_err(|_| AccessError::RouteMismatch)?;
    if schema_identity.as_digest() != &route.schema_identity_digest {
        return Err(AccessError::RouteMismatch);
    }
    if fence.catalog_revision.get() != request.authoritative_access.source_catalog_generation
        || fence.membership_revision.get() != request.authoritative_access.membership_generation
    {
        return Err(AccessError::SnapshotStale);
    }
    if fence.access_policy_revision != request.authoritative_policy.policy_revision
        || request.security.access_policy_revision != fence.access_policy_revision
    {
        return Err(AccessError::PolicyRevisionStale);
    }
    if fence.shadow_fence_revision != request.authoritative_policy.shadow_revision
        || request.security.shadow_fence_revision != fence.shadow_fence_revision
        || fence.purge_fence_revision != request.authoritative_policy.purge_revision
        || request.security.purge_fence_revision != fence.purge_fence_revision
        || request.security.overlay_revision != fence.overlay_revision
    {
        return Err(AccessError::SnapshotStale);
    }
    if fence.reference_portfolio_revision != request.grant.reference_portfolio_revision {
        return Err(AccessError::ScopeUnauthorized);
    }
    Ok(())
}

fn validate_snapshot_fingerprint(fence: &QuerySnapshotFence) -> Result<(), AccessError> {
    let canonical_input = fence
        .canonical_fingerprint_input()
        .map_err(|_| AccessError::SnapshotStale)?;
    let limit =
        Blake3ExactInputLimit::new(MAX_CANONICAL_BYTES).map_err(|_| AccessError::SnapshotStale)?;
    let computed = blake3_exact_bytes(canonical_input.as_slice(), limit)
        .map_err(|_| AccessError::SnapshotStale)?;
    if computed.as_bytes() != fence.snapshot_fingerprint.as_bytes() {
        return Err(AccessError::SnapshotStale);
    }
    Ok(())
}

fn validate_live_restrictions(
    memberships: &BTreeSet<SourceMembershipId>,
    request: &CanonicalEligibilityPopulationRequest<'_, impl Sized, impl Sized>,
) -> Result<(), AccessError> {
    let live = request.security.live;
    if live.fail_closed {
        return Err(AccessError::SecurityFailClosed);
    }
    if !memberships.is_disjoint(&live.purged_memberships) {
        return Err(AccessError::LivePurge);
    }
    if !memberships.is_disjoint(&live.denied_memberships) {
        return Err(AccessError::LiveRevocation);
    }
    Ok(())
}

fn compile_population_memberships<O: CanonicalPopulationVerificationPort + ?Sized>(
    authorized: &BTreeMap<SourceMembershipId, crate::MembershipAccessBinding>,
    request: &CanonicalEligibilityPopulationRequest<'_, impl Sized, impl Sized>,
    owner: &mut O,
    issued: &O::IssuedGrant,
) -> Result<(EligibilityPopulation, BTreeMap<SourceMembershipId, CanonicalProjectionMembership>), AccessError> {
    if request.projection_memberships.len() > MAX_COLLECTION_ITEMS {
        return Err(AccessError::EligibilityPopulationTooLarge);
    }
    let mut by_source = BTreeMap::new();
    for (projection_id, owner_membership) in request.projection_memberships {
        if projection_id != &owner_membership.projection_membership_id
            || !request.publication_manifests.contains_key(projection_id)
        {
            return Err(AccessError::SnapshotStale);
        }
        if request.publication_shadows.contains_key(projection_id) {
            return Err(AccessError::LiveShadowed);
        }
        if by_source
            .insert(owner_membership.source_membership_id, owner_membership)
            .is_some()
        {
            return Err(AccessError::SnapshotStale);
        }
    }
    if by_source.len() != authorized.len()
        || authorized.keys().any(|id| !by_source.contains_key(id))
    {
        return Err(AccessError::SnapshotStale);
    }

    let schema = request.collection_schema;
    let mut projection_ids = Vec::with_capacity(authorized.len());
    let mut unique_projection_ids = BTreeSet::new();
    let mut access_partition = None;
    let mut scoring_partition = None;
    let mut verified_bindings = BTreeMap::new();
    for (source_id, binding) in authorized {
        if binding.membership_id != *source_id || !binding.active {
            return Err(AccessError::SnapshotStale);
        }
        let owner_membership = by_source.get(source_id).ok_or(AccessError::SnapshotStale)?;
        let current_binding =
            owner.current_projection_binding(issued, *source_id, request.snapshot_fence)?;
        if current_binding.source_membership_id != *source_id
            || &current_binding != *owner_membership
        {
            return Err(AccessError::SnapshotStale);
        }
        verified_bindings.insert(*source_id, current_binding);
        if owner_membership.installation_incarnation_id != *schema.installation_incarnation_id()
            || owner_membership.collection_generation_id != *schema.collection_generation_id()
            || owner_membership.projection_profile_set_id != *schema.projection_profile_set_id()
        {
            return Err(AccessError::RouteMismatch);
        }
        if !request
            .grant
            .allowed_access_partitions
            .contains(&owner_membership.access_partition_id)
            || !request
                .grant_policy
                .allowed_access_partitions
                .contains(&owner_membership.access_partition_id)
        {
            return Err(AccessError::ScopeUnauthorized);
        }
        if !unique_projection_ids.insert(owner_membership.projection_membership_id) {
            return Err(AccessError::SnapshotStale);
        }
        if access_partition.is_some_and(|value| value != owner_membership.access_partition_id)
            || scoring_partition.is_some_and(|value| value != owner_membership.scoring_partition_id)
        {
            return Err(AccessError::EligibilityPopulationIncoherent);
        }
        access_partition = Some(owner_membership.access_partition_id);
        scoring_partition = Some(owner_membership.scoring_partition_id);
        projection_ids.push(owner_membership.projection_membership_id);
    }

    let population = EligibilityPopulation::new(
        *schema.installation_incarnation_id(),
        *schema.collection_generation_id(),
        &projection_ids,
        access_partition.ok_or(AccessError::AuthorizedScopeEmpty)?,
        scoring_partition.ok_or(AccessError::AuthorizedScopeEmpty)?,
        schema.projection_profile_set_id().clone(),
        request
            .snapshot_fence
            .visible_epoch
            .ok_or(AccessError::RouteMismatch)?,
    )
    .map_err(|_| AccessError::EligibilityPopulationIncoherent)?;
    population
        .validate_for_schema(schema)
        .map_err(|_| AccessError::RouteMismatch)?;
    Ok((population, verified_bindings))
}

#[cfg(test)]
mod tests;

mod evidence;
pub use evidence::{
    CanonicalEvidencePopulationRequest, CanonicalEvidencePopulationVerificationPort,
    EvidencePopulationRecipe, compile_canonical_evidence_population,
};
