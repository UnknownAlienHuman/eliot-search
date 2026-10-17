//! V2 invocation of C18 under the same Root metadata and issued-grant borrow.

use std::collections::{BTreeMap, BTreeSet};

use search_access::{
    CanonicalEvidencePopulationVerificationPort, CompiledEligibilityPopulation,
    EvidencePopulationRecipe, RequestSecurityFence, RequestedMembershipScope,
    compile_canonical_evidence_population,
};
use search_contracts::QuerySnapshotFence;
use search_contracts::evidence::{PublicScopeSelector, RecipeIdV2};
use search_provider_protocol::{AdmittedEvidenceProviderRequestV2, frame::EvidenceRecipeRequestV2};

use super::{
    AccessError, CanonicalEligibilityPopulationRequest, CanonicalEligibilitySecuritySnapshot,
    CanonicalPopulationLiveAuthority, CanonicalPopulationSnapshot, CanonicalProjectionMembership,
    NativeCanonicalPopulationPort, VerifiedStandaloneGrant, canonical_projection_membership,
    checkpoint_live, map_grant_error,
};
use crate::access_composition::{StandaloneScopeResolver, validate_membership_security};

#[allow(clippy::too_many_arguments)]
pub fn compile_current_evidence_population(
    metadata: &CanonicalPopulationSnapshot,
    current: &CanonicalPopulationLiveAuthority<'_>,
    request: &AdmittedEvidenceProviderRequestV2,
    issued: &VerifiedStandaloneGrant<'_>,
    resolver: &mut dyn StandaloneScopeResolver,
) -> Result<
    (
        CompiledEligibilityPopulation,
        RequestedMembershipScope,
        RequestSecurityFence,
    ),
    AccessError,
> {
    let envelope = request.envelope();
    let EvidenceRecipeRequestV2::RetrieveEvidence(body) = &envelope.recipe_request else {
        return Err(AccessError::RecipeDenied);
    };
    issued
        .verify_current_evidence_claims(&envelope.grant)
        .map_err(map_grant_error)?;
    metadata.validate_live(current)?;
    let requested = resolver.resolve_evidence(&body.scope, &metadata.fence)?;
    if requested.memberships.len() > search_contracts::MAX_COLLECTION_ITEMS {
        return Err(AccessError::EligibilityPopulationTooLarge);
    }
    validate_membership_security(
        issued.template(),
        &requested,
        current.access,
        current.membership_security,
    )?;
    let fence = RequestSecurityFence {
        planned_generation: current.access.generation,
        memberships: requested.memberships.clone(),
    };
    checkpoint_live(current, &fence)?;
    let mut projections = BTreeMap::<_, CanonicalProjectionMembership>::new();
    let mut modality = None;
    for member in &requested.memberships {
        let row = metadata
            .rows
            .get(member)
            .ok_or(AccessError::SnapshotStale)?;
        let security = current
            .membership_security
            .memberships
            .get(member)
            .ok_or(AccessError::SnapshotStale)?;
        if modality.is_some_and(|kind| kind != security.modality) {
            return Err(AccessError::EligibilityPopulationIncoherent);
        }
        modality = Some(security.modality);
        let projection = canonical_projection_membership(row, &metadata.schema);
        if projection.source_membership_id != *member
            || projections
                .insert(projection.projection_membership_id, projection)
                .is_some()
        {
            return Err(AccessError::SnapshotStale);
        }
    }
    let mut port = NativeCanonicalPopulationPort {
        metadata,
        claims: &envelope.grant.claims,
        evidence: Some(&envelope.grant),
        issued,
        resolver: Some(resolver),
    };
    let population = compile_canonical_evidence_population(
        &mut port,
        issued,
        &CanonicalEligibilityPopulationRequest {
            grant: &envelope.grant.claims,
            grant_policy: current.policy,
            recipe: EvidencePopulationRecipe {
                request: body,
                permitted_recipes: &envelope.grant.permitted_evidence_recipes,
            },
            modality: modality.ok_or(AccessError::AuthorizedScopeEmpty)?,
            requested_scope: &requested,
            authoritative_access: current.access,
            namespace_policy_fence: current.planned_policy,
            authoritative_policy: current.authoritative_policy,
            snapshot_fence: &metadata.fence,
            collection_schema: &metadata.schema,
            collection_route: &metadata.route,
            publication_manifests: &metadata.manifests,
            publication_shadows: &metadata.shadows,
            projection_memberships: &projections,
            security: CanonicalEligibilitySecuritySnapshot {
                live: current.live,
                namespace_id: current.authoritative_policy.namespace_id,
                access_policy_revision: current.authoritative_policy.policy_revision,
                shadow_fence_revision: current.authoritative_policy.shadow_revision,
                purge_fence_revision: current.authoritative_policy.purge_revision,
                overlay_revision: metadata.fence.overlay_revision,
            },
        },
    )?;
    checkpoint_live(current, &fence)?;
    Ok((population, requested, fence))
}

impl CanonicalEvidencePopulationVerificationPort for NativeCanonicalPopulationPort<'_, '_, '_, '_> {
    fn verified_evidence_recipes(
        &mut self,
        issued: &Self::IssuedGrant,
    ) -> Result<BTreeSet<RecipeIdV2>, AccessError> {
        if !std::ptr::eq(issued, self.issued) {
            return Err(AccessError::GrantBindingMismatch);
        }
        let evidence = self.evidence.ok_or(AccessError::RecipeDenied)?;
        issued
            .verify_current_evidence_claims(evidence)
            .map_err(map_grant_error)?;
        Ok(issued
            .template()
            .permitted_evidence_recipes
            .iter()
            .copied()
            .collect())
    }

    fn resolve_evidence_scope(
        &mut self,
        issued: &Self::IssuedGrant,
        selector: &PublicScopeSelector,
        fence: &QuerySnapshotFence,
    ) -> Result<RequestedMembershipScope, AccessError> {
        let _ = self.verified_evidence_recipes(issued)?;
        if fence != self.metadata.fence() {
            return Err(AccessError::SnapshotStale);
        }
        self.resolver
            .as_deref_mut()
            .ok_or(AccessError::ScopeUnknown)?
            .resolve_evidence(selector, fence)
    }
}
