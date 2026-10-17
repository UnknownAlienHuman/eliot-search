//! Typed lexical evidence admission using the same private C18 population compiler.

use std::collections::BTreeSet;

use search_contracts::evidence::{PublicScopeSelector, RecipeIdV2, RetrieveEvidenceRequestV2};
use search_contracts::{
    BoundedSet, CorpusOrPortfolioId, MAX_SET_ITEMS, Modality, QuerySnapshotFence,
};

use super::{
    AccessError, CanonicalEligibilityPopulationRequest, CanonicalPopulationVerificationPort,
    CompiledEligibilityPopulation, RequestedMembershipScope, compile_verified_population,
    validate_common_grant_policy, validate_issued_claims, validate_modality,
};

/// Actual typed evidence request and its presented V2 permission extension.
/// Both are untrusted data until independently compared with the original issuer.
pub struct EvidencePopulationRecipe<'a> {
    pub request: &'a RetrieveEvidenceRequestV2,
    pub permitted_recipes: &'a BoundedSet<RecipeIdV2, MAX_SET_ITEMS>,
}

/// Same owner/snapshot inputs as V1, with no V1 recipe alias or fabricated grant.
pub type CanonicalEvidencePopulationRequest<'a, S> =
    CanonicalEligibilityPopulationRequest<'a, S, EvidencePopulationRecipe<'a>>;

/// Existing issuance and immutable-scope owners borrowed during one live turn.
pub trait CanonicalEvidencePopulationVerificationPort: CanonicalPopulationVerificationPort {
    /// Return the exact retained V2 permissions after checking the same issuer
    /// token, original nonce and current lifetime. V1 permission is insufficient.
    fn verified_evidence_recipes(
        &mut self,
        issued: &Self::IssuedGrant,
    ) -> Result<BTreeSet<RecipeIdV2>, AccessError>;

    /// Resolve this exact immutable selector through current server-owned state.
    /// A request-supplied membership list, endpoint or handle is not a resolver.
    fn resolve_evidence_scope(
        &mut self,
        issued: &Self::IssuedGrant,
        selector: &PublicScopeSelector,
        fence: &QuerySnapshotFence,
    ) -> Result<RequestedMembershipScope, AccessError>;
}

/// Compile lexical `retrieve_evidence@1` to the existing opaque population.
///
/// Unsupported modalities refuse before binding reads. Scope/permissions are
/// observed before and after compilation, while all ordinary route, original
/// grant, deny/purge/shadow, manifest and owner checks use the unchanged compiler.
/// This creates neither a reusable source permit nor an output proof.
pub fn compile_canonical_evidence_population<
    S,
    O: CanonicalEvidencePopulationVerificationPort + ?Sized,
>(
    owner: &mut O,
    issued: &O::IssuedGrant,
    request: &CanonicalEvidencePopulationRequest<'_, S>,
) -> Result<CompiledEligibilityPopulation, AccessError> {
    request
        .recipe
        .request
        .validate()
        .map_err(|_| AccessError::ScopeUnauthorized)?;
    if !matches!(request.modality, Modality::Code | Modality::Text) {
        return Err(AccessError::ModalityDenied);
    }
    if request.requested_scope.memberships.len() > search_contracts::MAX_COLLECTION_ITEMS {
        return Err(AccessError::EligibilityPopulationTooLarge);
    }
    validate_issued_claims(owner, issued, request.grant)?;
    validate_common_grant_policy(request.grant, request.grant_policy)?;
    validate_modality(request.grant, request.grant_policy, request.modality)?;
    if !request.grant.source_read_permission || !request.grant_policy.source_read_permission {
        return Err(AccessError::ScopeUnauthorized);
    }
    if request.recipe.request.source_view != request.snapshot_fence.source_view {
        return Err(AccessError::SnapshotStale);
    }
    validate_evidence_owner(owner, issued, request)?;
    let result = compile_verified_population(owner, issued, request)?;
    validate_evidence_owner(owner, issued, request)?;
    Ok(result)
}

fn validate_evidence_owner<S, O: CanonicalEvidencePopulationVerificationPort + ?Sized>(
    owner: &mut O,
    issued: &O::IssuedGrant,
    request: &CanonicalEvidencePopulationRequest<'_, S>,
) -> Result<(), AccessError> {
    let presented = request.recipe.permitted_recipes;
    let retained = owner.verified_evidence_recipes(issued)?;
    if retained.len() != presented.len() || retained.iter().any(|id| !presented.contains(id)) {
        return Err(AccessError::GrantBindingMismatch);
    }
    if !retained.contains(&RecipeIdV2::RetrieveEvidence)
        || retained
            .iter()
            .any(|id| !request.grant_policy.permitted_evidence_recipes.contains(id))
    {
        return Err(AccessError::RecipeDenied);
    }
    let selector = &request.recipe.request.scope;
    let public_id = match selector {
        PublicScopeSelector::CorpusRevision { corpus_id, .. } => {
            Some(CorpusOrPortfolioId::Corpus(*corpus_id))
        }
        PublicScopeSelector::PortfolioRevision { portfolio_id, .. } => {
            Some(CorpusOrPortfolioId::Portfolio(*portfolio_id))
        }
        _ => None,
    };
    if public_id.is_some_and(|id| !request.grant.allowed_corpus_or_portfolio_ids.contains(&id)) {
        return Err(AccessError::ScopeUnauthorized);
    }
    match selector {
        PublicScopeSelector::WorkspaceRevision(id)
            if request.snapshot_fence.workspace_view_revision_ref != Some(*id) =>
        {
            return Err(AccessError::SnapshotStale);
        }
        PublicScopeSelector::CorpusRevision {
            membership_revision,
            ..
        } if *membership_revision != request.snapshot_fence.membership_revision => {
            return Err(AccessError::SnapshotStale);
        }
        PublicScopeSelector::PortfolioRevision {
            portfolio_revision, ..
        } if Some(*portfolio_revision) != request.snapshot_fence.reference_portfolio_revision => {
            return Err(AccessError::SnapshotStale);
        }
        PublicScopeSelector::OverlayRevision(revision)
            if *revision != request.snapshot_fence.overlay_revision =>
        {
            return Err(AccessError::SnapshotStale);
        }
        _ => {}
    }
    let resolved = owner.resolve_evidence_scope(issued, selector, request.snapshot_fence)?;
    if resolved != *request.requested_scope {
        return Err(AccessError::ScopeUnauthorized);
    }
    Ok(())
}
