//! Bounded lexical planning for `retrieve_evidence@1` over the owner-compiled C18 population.

use std::collections::BTreeSet;

use search_access::{
    CompiledEligibilityPopulation, RequestSecurityFence, RequestedMembershipScope,
};
use search_contracts::evidence::{PublicScopeSelector, RetrieveEvidenceRequestV2};
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue, ContractError, DigestInputLimit, MAX_CANONICAL_BYTES, MAX_COLLECTION_ITEMS,
    NonZeroRevision, OpaqueId, ProfileId, QueryExecutionBudget, QuerySnapshotFence, RequestId,
    blake3_canonical, blake3_canonical_array,
};
use search_lexical::{
    AcceptedCodeIdentifiersProfile, CodeIdentifierLimits, LexicalInput, SparseLimits, SparseVector,
    encode_code_query,
};

use crate::{
    LegBudget, PlanError, allocate_leg_budgets, execution_array, execution_bytes, execution_text,
    verify_canonical_snapshot,
};

const EVIDENCE_PLAN_DOMAIN: &str = "eliot/cbor/retrieve-evidence-plan/v2";
const REQUEST_BODY_DOMAIN: &str = "eliot/cbor/retrieve-evidence-request-body/v2";
const MEMBERSHIP_SET_DOMAIN: &str = "eliot/cbor/retrieve-evidence-memberships/v2";
const PROJECTION_BINDINGS_DOMAIN: &str = "eliot/cbor/retrieve-evidence-projections/v2";
const C00_FILTER_DOMAIN: &str = "eliot/cbor/retrieve-evidence-c00-filter/v2";
const REQUEST_SOURCE_HINTS_DOMAIN: &str = "eliot/cbor/retrieve-evidence-source-hints/v2";

/// A capability omitted by the lexical-only `retrieve_evidence@1` planner.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EvidenceOmittedCapabilityV2 {
    /// Structural retrieval is not compiled by this lexical producer.
    Structural,
    /// Dense or other semantic retrieval is not compiled by this lexical producer.
    Semantic,
}

/// One finite lexical leg using the exact C18 population and C11 query vector.
pub struct EvidenceLexicalLegV2 {
    budget: LegBudget,
    query_vector: SparseVector,
    vector_fingerprint: [u8; 32],
    profile_id: ProfileId,
    profile_fingerprint: Blake3Digest32,
}

impl EvidenceLexicalLegV2 {
    /// Existing planner allocation for this one bounded leg.
    #[must_use]
    pub const fn budget(&self) -> &LegBudget {
        &self.budget
    }

    /// C11-produced sparse query vector. The C18 population remains separate and shared.
    #[must_use]
    pub const fn query_vector(&self) -> &SparseVector {
        &self.query_vector
    }

    /// Exact accepted C11 profile identity used for query encoding.
    #[must_use]
    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    /// Full behavior fingerprint of the accepted C11 profile.
    #[must_use]
    pub const fn profile_fingerprint(&self) -> Blake3Digest32 {
        self.profile_fingerprint
    }

    /// Exact vector fingerprint emitted by the C11 encoder.
    #[must_use]
    pub const fn vector_fingerprint(&self) -> &[u8; 32] {
        &self.vector_fingerprint
    }
}

/// Domain-separated identity of one compiled V2 lexical plan.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EvidencePlanIdentityV2(Blake3Digest32);

impl EvidencePlanIdentityV2 {
    /// Deterministic identity over the request data, C18 proof outputs and C11 plan.
    #[must_use]
    pub const fn digest(self) -> Blake3Digest32 {
        self.0
    }
}

/// Finite lexical plan borrowed from the exact C18 compilation invocation.
///
/// The borrowed population is the only eligibility value for indexed retrieval,
/// filtered IDF and denominator work. This plan creates no access permit and
/// cannot outlive the C18 owner callback when that callback owns the borrow.
pub struct CompiledEvidencePlanV2<'a> {
    request_id: RequestId,
    request: &'a RetrieveEvidenceRequestV2,
    population: &'a CompiledEligibilityPopulation,
    scope: &'a RequestedMembershipScope,
    request_fence: &'a RequestSecurityFence,
    leg: EvidenceLexicalLegV2,
    omitted_capabilities: [EvidenceOmittedCapabilityV2; 2],
    identity: EvidencePlanIdentityV2,
}

impl CompiledEvidencePlanV2<'_> {
    /// Original request ID supplied by the live Root callback's admitted guard.
    #[must_use]
    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }

    /// Exact typed V2 request borrowed for this compilation.
    #[must_use]
    pub const fn request(&self) -> &RetrieveEvidenceRequestV2 {
        self.request
    }

    /// Opaque owner-compiled C18 proof; no replacement or translated filter is made.
    #[must_use]
    pub const fn compiled_population(&self) -> &CompiledEligibilityPopulation {
        self.population
    }

    /// The one C00 predicate shared by all indexed population operations.
    #[must_use]
    pub const fn population(&self) -> &search_contracts::indexed::EligibilityPopulation {
        self.population.population()
    }

    /// Resolved membership scope captured by the same C18 callback.
    #[must_use]
    pub const fn scope(&self) -> &RequestedMembershipScope {
        self.scope
    }

    /// Original request security fence captured by the same C18 callback.
    #[must_use]
    pub const fn request_fence(&self) -> &RequestSecurityFence {
        self.request_fence
    }

    /// The sole supported lexical leg.
    #[must_use]
    pub const fn lexical_leg(&self) -> &EvidenceLexicalLegV2 {
        &self.leg
    }

    /// Capabilities this producer explicitly leaves unavailable.
    #[must_use]
    pub const fn omitted_capabilities(&self) -> &[EvidenceOmittedCapabilityV2; 2] {
        &self.omitted_capabilities
    }

    /// Content-free immutable identity for the complete compiled plan.
    #[must_use]
    pub const fn identity(&self) -> EvidencePlanIdentityV2 {
        self.identity
    }
}

/// Compile one actual `retrieve_evidence@1` request into a finite lexical plan.
///
/// `request_id`, `population`, `scope` and `request_fence` must be passed from
/// the same `LiveNativeQueryAuthorityOwner::with_retrieve_evidence_population`
/// callback. The ID is ordinary request data; authority comes only from the
/// opaque C18 result and the callback's still-held Root/issuer lifetimes.
///
/// The caller supplies the accepted C11 profile captured by the same current
/// Root runtime that produced the C18 population. The planner runs the existing
/// C11 query encoder, allocates exactly one lexical leg with the existing finite
/// budget allocator, and keeps the original C00 value borrowed for all indexed
/// population consumers. Structural and semantic/dense branches are reported
/// as omitted. This is runtime planning, not release or scale qualification.
///
/// # Errors
/// Refuses mismatched scope, fence, snapshot, C00 filter, profile or budgets.
#[allow(clippy::too_many_arguments)]
pub fn compile_retrieve_evidence_v2<'a>(
    request_id: RequestId,
    request: &'a RetrieveEvidenceRequestV2,
    population: &'a CompiledEligibilityPopulation,
    scope: &'a RequestedMembershipScope,
    request_fence: &'a RequestSecurityFence,
    lexical_profile: &AcceptedCodeIdentifiersProfile,
) -> Result<CompiledEvidencePlanV2<'a>, PlanError> {
    request
        .validate()
        .map_err(|_| PlanError::CanonicalPlanInvalid)?;
    validate_c18_binding(request, population, scope, request_fence)?;
    crate::validate_global_budget(request.budgets)?;

    let lexical_profile_id = ProfileId::new(lexical_profile.profile().id().as_str())
        .map_err(|_| PlanError::EvidenceProfileMismatch)?;
    let snapshot_profiles = &population.snapshot_fence().lexical_profile_ids;
    if !snapshot_profiles.is_empty()
        && !snapshot_profiles
            .iter()
            .any(|profile| profile == &lexical_profile_id)
    {
        return Err(PlanError::EvidenceProfileMismatch);
    }

    let source_start = 0_u64;
    let source_end = u64::try_from(request.question.as_bytes().len())
        .map_err(|_| PlanError::QueryEncodingFailed)?;
    // C11 currently models query input with the source-shaped LexicalInput.
    // This reserved marker is constant query metadata only: neither request ID
    // nor any source ID is converted into a lexical source identity.
    let query_input = LexicalInput::new(
        OpaqueId::new("eliot/query/retrieve-evidence@1")
            .map_err(|_| PlanError::QueryEncodingFailed)?,
        NonZeroRevision::new(1).map_err(|_| PlanError::QueryEncodingFailed)?,
        0,
        source_start,
        source_end,
        request.question.as_str().to_owned(),
    );
    let encoded = encode_code_query(
        &query_input,
        lexical_profile,
        CodeIdentifierLimits::HARD,
        SparseLimits::BASELINE,
        false,
    )
    .map_err(|_| PlanError::QueryEncodingFailed)?;
    let query_encoding = encoded.encoding();
    let budget = allocate_leg_budgets(request.budgets, 1)?
        .into_iter()
        .next()
        .ok_or(PlanError::BudgetExceeded)?;
    let leg = EvidenceLexicalLegV2 {
        budget,
        query_vector: query_encoding.vector.clone(),
        vector_fingerprint: query_encoding.receipt.vector_fingerprint.0,
        profile_id: lexical_profile_id,
        profile_fingerprint: lexical_profile.profile().fingerprint(),
    };
    let identity = EvidencePlanIdentityV2(fingerprint_evidence_plan(
        request_id,
        request,
        population,
        scope,
        request_fence,
        &leg,
    )?);

    Ok(CompiledEvidencePlanV2 {
        request_id,
        request,
        population,
        scope,
        request_fence,
        leg,
        omitted_capabilities: [
            EvidenceOmittedCapabilityV2::Structural,
            EvidenceOmittedCapabilityV2::Semantic,
        ],
        identity,
    })
}

fn validate_c18_binding(
    request: &RetrieveEvidenceRequestV2,
    compiled: &CompiledEligibilityPopulation,
    scope: &RequestedMembershipScope,
    request_fence: &RequestSecurityFence,
) -> Result<(), PlanError> {
    let snapshot = compiled.snapshot_fence();
    verify_canonical_snapshot(snapshot)?;
    if request.source_view != snapshot.source_view
        || !selector_matches_snapshot(&request.scope, snapshot)
        || snapshot.collection_generation_id.is_none()
        || snapshot.visible_epoch.is_none()
    {
        return Err(PlanError::EvidenceAdmissionMismatch);
    }

    let selected = &scope.memberships;
    if selected.is_empty()
        || selected.len() > MAX_COLLECTION_ITEMS
        || request_fence.planned_generation == 0
        || selected != &request_fence.memberships
        || selected.len() != compiled.projection_memberships().len()
        || selected
            .iter()
            .any(|membership| !compiled.projection_memberships().contains_key(membership))
    {
        return Err(PlanError::ScopeMismatch);
    }

    let c00 = compiled.population();
    let owner_projection_ids = compiled
        .projection_memberships()
        .iter()
        .map(|(source_membership_id, binding)| {
            if binding.source_membership_id != *source_membership_id {
                return Err(PlanError::EvidenceAdmissionMismatch);
            }
            Ok(binding.projection_membership_id)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let c00_projection_ids = c00
        .allowed_projection_memberships()
        .into_iter()
        .collect::<BTreeSet<_>>();
    if owner_projection_ids.len() != compiled.projection_memberships().len()
        || owner_projection_ids != c00_projection_ids
    {
        return Err(PlanError::EvidenceAdmissionMismatch);
    }
    c00.to_canonical_value()
        .map_err(|_| PlanError::EvidenceAdmissionMismatch)?;
    Ok(())
}

fn selector_matches_snapshot(
    selector: &PublicScopeSelector,
    snapshot: &QuerySnapshotFence,
) -> bool {
    match selector {
        PublicScopeSelector::WorkspaceRevision(revision) => {
            snapshot.workspace_view_revision_ref == Some(*revision)
        }
        PublicScopeSelector::CorpusRevision {
            membership_revision,
            ..
        } => *membership_revision == snapshot.membership_revision,
        PublicScopeSelector::PortfolioRevision {
            portfolio_revision, ..
        } => snapshot.reference_portfolio_revision == Some(*portfolio_revision),
        PublicScopeSelector::OverlayRevision(revision) => snapshot.overlay_revision == *revision,
    }
}

fn fingerprint_evidence_plan(
    request_id: RequestId,
    request: &RetrieveEvidenceRequestV2,
    compiled: &CompiledEligibilityPopulation,
    scope: &RequestedMembershipScope,
    request_fence: &RequestSecurityFence,
    leg: &EvidenceLexicalLegV2,
) -> Result<Blake3Digest32, PlanError> {
    let limit =
        DigestInputLimit::new(MAX_CANONICAL_BYTES).map_err(|_| PlanError::IdentityEncoding)?;
    let scope_digest = fingerprint_memberships(&scope.memberships, MEMBERSHIP_SET_DOMAIN)?;
    let projection_digest = fingerprint_projection_bindings(compiled)?;
    let filter_value = compiled
        .population()
        .to_canonical_value()
        .map_err(|_| PlanError::EvidenceAdmissionMismatch)?;
    let filter_digest = blake3_canonical(
        &CanonicalDigestDomain::parse(C00_FILTER_DOMAIN)
            .map_err(|_| PlanError::IdentityEncoding)?,
        &filter_value,
        limit,
    )
    .map_err(|_| PlanError::IdentityEncoding)?;
    let request_body = request_body_value(request)?;
    let request_digest = blake3_canonical(
        &CanonicalDigestDomain::parse(REQUEST_BODY_DOMAIN)
            .map_err(|_| PlanError::IdentityEncoding)?,
        &request_body,
        limit,
    )
    .map_err(|_| PlanError::IdentityEncoding)?;
    let grant_fence = compiled.grant_fence();
    let request_fence_digest =
        fingerprint_memberships(&request_fence.memberships, MEMBERSHIP_SET_DOMAIN)?;
    let leg_budget = leg_budget_value(*leg.budget())?;
    let lexical_profile = execution_array(vec![
        execution_text(leg.profile_id.as_str())?,
        execution_bytes(leg.profile_fingerprint.as_bytes())?,
        execution_bytes(&leg.vector_fingerprint)?,
    ])?;
    let value = execution_array(vec![
        execution_text("retrieve_evidence@1")?,
        execution_bytes(request_id.as_bytes())?,
        execution_bytes(request_digest.as_bytes())?,
        execution_bytes(compiled.snapshot_fence().snapshot_fingerprint.as_bytes())?,
        execution_bytes(grant_fence.grant_id.as_bytes())?,
        CanonicalValue::U64(grant_fence.revocation_generation),
        CanonicalValue::U64(request_fence.planned_generation),
        execution_bytes(scope_digest.as_bytes())?,
        execution_bytes(request_fence_digest.as_bytes())?,
        execution_bytes(projection_digest.as_bytes())?,
        execution_bytes(filter_digest.as_bytes())?,
        lexical_profile,
        leg_budget,
    ])?;
    blake3_canonical(
        &CanonicalDigestDomain::parse(EVIDENCE_PLAN_DOMAIN)
            .map_err(|_| PlanError::IdentityEncoding)?,
        &value,
        limit,
    )
    .map_err(|_| PlanError::IdentityEncoding)
}

fn fingerprint_memberships(
    memberships: &BTreeSet<search_contracts::SourceMembershipId>,
    domain: &str,
) -> Result<Blake3Digest32, PlanError> {
    if memberships.is_empty() || memberships.len() > MAX_COLLECTION_ITEMS {
        return Err(PlanError::ScopeMismatch);
    }
    let domain = CanonicalDigestDomain::parse(domain).map_err(|_| PlanError::IdentityEncoding)?;
    let limit =
        DigestInputLimit::new(MAX_CANONICAL_BYTES).map_err(|_| PlanError::IdentityEncoding)?;
    blake3_canonical_array(
        &domain,
        memberships.len(),
        memberships.iter().map(|membership| {
            BoundedBytes::new(membership.as_bytes().to_vec()).map(CanonicalValue::Bytes)
        }),
        limit,
    )
    .map_err(|_| PlanError::IdentityEncoding)
}

fn fingerprint_projection_bindings(
    compiled: &CompiledEligibilityPopulation,
) -> Result<Blake3Digest32, PlanError> {
    let rows = compiled.projection_memberships();
    if rows.is_empty() || rows.len() > MAX_COLLECTION_ITEMS {
        return Err(PlanError::EvidenceAdmissionMismatch);
    }
    let domain = CanonicalDigestDomain::parse(PROJECTION_BINDINGS_DOMAIN)
        .map_err(|_| PlanError::IdentityEncoding)?;
    let limit =
        DigestInputLimit::new(MAX_CANONICAL_BYTES).map_err(|_| PlanError::IdentityEncoding)?;
    blake3_canonical_array(
        &domain,
        rows.len(),
        rows.iter().map(|(source_membership_id, binding)| {
            contract_array(vec![
                contract_bytes(source_membership_id.as_bytes())?,
                contract_bytes(binding.source_membership_id.as_bytes())?,
                contract_bytes(binding.installation_incarnation_id.as_bytes())?,
                contract_bytes(binding.collection_generation_id.as_bytes())?,
                contract_bytes(binding.projection_membership_id.as_bytes())?,
                contract_bytes(binding.access_partition_id.as_bytes())?,
                contract_bytes(binding.scoring_partition_id.as_bytes())?,
                contract_text(binding.projection_profile_set_id.as_str())?,
            ])
        }),
        limit,
    )
    .map_err(|_| PlanError::IdentityEncoding)
}

fn request_body_value(request: &RetrieveEvidenceRequestV2) -> Result<CanonicalValue, PlanError> {
    let (selector_tag, selector_value) = selector_identity(&request.scope)?;
    let roles = execution_array(
        request
            .requested_roles
            .iter()
            .map(|role| execution_text(role.as_str()))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let source_hints = fingerprint_source_hints(&request.optional_hints.authorized_source_ids)?;
    let handles = execution_array(
        request
            .optional_hints
            .suggested_exact_handles
            .iter()
            .map(|handle| {
                let class = match handle.handle_class {
                    search_contracts::HandleClass::Ephemeral => "ephemeral",
                    search_contracts::HandleClass::DurableSource => "durable_source",
                };
                let expires_at = match &handle.expires_at {
                    Some(value) => execution_text(value.as_str())?,
                    None => CanonicalValue::Null,
                };
                execution_array(vec![
                    execution_bytes(handle.handle_id.as_bytes())?,
                    CanonicalValue::U64(handle.handle_revision.get()),
                    execution_text(class)?,
                    expires_at,
                    execution_bytes(handle.opaque_token.as_bytes())?,
                ])
            })
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    execution_array(vec![
        execution_array(vec![selector_tag, selector_value])?,
        execution_text(request.question.as_str())?,
        execution_text(request.intent.as_str())?,
        roles,
        execution_bytes(source_hints.as_bytes())?,
        handles,
        execution_text(request.profile_id.as_str())?,
        budget_value(request.budgets)?,
        CanonicalValue::Bool(request.progressive),
    ])
}

fn fingerprint_source_hints(
    source_ids: &search_contracts::BoundedSet<
        search_contracts::SourceId,
        { search_contracts::MAX_SET_ITEMS },
    >,
) -> Result<Blake3Digest32, PlanError> {
    let domain = CanonicalDigestDomain::parse(REQUEST_SOURCE_HINTS_DOMAIN)
        .map_err(|_| PlanError::IdentityEncoding)?;
    let limit =
        DigestInputLimit::new(MAX_CANONICAL_BYTES).map_err(|_| PlanError::IdentityEncoding)?;
    blake3_canonical_array(
        &domain,
        source_ids.len(),
        source_ids.iter().map(|source_id| {
            BoundedBytes::new(source_id.as_bytes().to_vec()).map(CanonicalValue::Bytes)
        }),
        limit,
    )
    .map_err(|_| PlanError::IdentityEncoding)
}

fn contract_bytes(bytes: &[u8]) -> Result<CanonicalValue, ContractError> {
    BoundedBytes::new(bytes.to_vec()).map(CanonicalValue::Bytes)
}

fn contract_text(text: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(text.to_owned()).map(CanonicalValue::Text)
}

fn contract_array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    BoundedList::new(values).map(CanonicalValue::Array)
}

fn selector_identity(
    selector: &PublicScopeSelector,
) -> Result<(CanonicalValue, CanonicalValue), PlanError> {
    match selector {
        PublicScopeSelector::WorkspaceRevision(revision) => Ok((
            execution_text("workspace_revision")?,
            execution_bytes(revision.as_bytes())?,
        )),
        PublicScopeSelector::CorpusRevision {
            corpus_id,
            membership_revision,
        } => Ok((
            execution_text("corpus_revision")?,
            execution_array(vec![
                execution_bytes(corpus_id.as_bytes())?,
                CanonicalValue::U64(membership_revision.get()),
            ])?,
        )),
        PublicScopeSelector::PortfolioRevision {
            portfolio_id,
            portfolio_revision,
        } => Ok((
            execution_text("portfolio_revision")?,
            execution_array(vec![
                execution_bytes(portfolio_id.as_bytes())?,
                CanonicalValue::U64(portfolio_revision.get()),
            ])?,
        )),
        PublicScopeSelector::OverlayRevision(revision) => Ok((
            execution_text("overlay_revision")?,
            CanonicalValue::U64(revision.get()),
        )),
    }
}

fn budget_value(budget: QueryExecutionBudget) -> Result<CanonicalValue, PlanError> {
    let priority = match budget.priority_class {
        search_contracts::PriorityClass::Interactive => "interactive",
        search_contracts::PriorityClass::Verification => "verification",
        search_contracts::PriorityClass::Background => "background",
    };
    execution_array(vec![
        execution_text(priority)?,
        CanonicalValue::U64(budget.deadline_ms),
        CanonicalValue::U64(u64::from(budget.max_scoring_legs)),
        CanonicalValue::U64(u64::from(budget.max_prefetch_candidates_per_leg)),
        CanonicalValue::U64(u64::from(budget.max_validated_candidates)),
        CanonicalValue::U64(budget.max_source_read_bytes),
        CanonicalValue::U64(budget.max_exact_scan_items),
        CanonicalValue::U64(budget.max_exact_scan_bytes),
        CanonicalValue::U64(budget.max_materialized_result_bytes),
        CanonicalValue::U64(budget.max_cpu_ms),
        CanonicalValue::U64(budget.max_memory_bytes),
    ])
}

fn leg_budget_value(budget: LegBudget) -> Result<CanonicalValue, PlanError> {
    execution_array(vec![
        CanonicalValue::U64(budget.deadline_ms()),
        CanonicalValue::U64(u64::from(budget.max_candidates())),
        CanonicalValue::U64(budget.max_source_read_bytes()),
        CanonicalValue::U64(budget.max_cpu_ms()),
        CanonicalValue::U64(budget.max_memory_bytes()),
    ])
}
