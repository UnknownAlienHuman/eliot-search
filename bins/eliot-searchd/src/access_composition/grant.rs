//! Daemon composition boundary for server-minted standalone read grants.
//!
//! The CLI request is never authority. This module narrows it against one
//! captured binding/policy snapshot, then delegates operation identity,
//! randomness and trusted time to an injected issuer. The issuer may retain
//! an idempotent process-incarnation receipt, but it cannot widen the template
//! supplied by composition.

mod issuer;

pub use issuer::{
    BoundedStandaloneGrantIssuer, GrantEntropySource, GrantTimeSource, GrantTimeWindow,
};

use search_contracts::{
    AccessPartitionId, BindingId, BoundedSet, CorpusOrPortfolioId, DisclosureCeiling, GrantId,
    InstallationId, InstallationIncarnationId, Modality, OpaqueId, OpaqueRef, PortfolioRevision,
    ProfileId, RecipeIdV1, ScopeDomainId, SearchReadGrantClaims, SensitivityClass,
    SourceMembershipId, UtcTimestamp, MAX_SET_ITEMS,
};

/// Bounded client request. Every field is a requested ceiling, never authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneGrantRequest {
    pub operation_id: OpaqueId,
    pub binding_id: BindingId,
    pub expected_binding_generation: u64,
    pub expected_policy_generation: u64,
    pub requested_membership_ids: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    pub requested_corpus_or_portfolio_ids: BoundedSet<CorpusOrPortfolioId, MAX_SET_ITEMS>,
    pub requested_access_partitions: BoundedSet<AccessPartitionId, MAX_SET_ITEMS>,
    pub requested_modalities: BoundedSet<Modality, MAX_SET_ITEMS>,
    pub requested_recipe_families: BoundedSet<RecipeIdV1, MAX_SET_ITEMS>,
    pub requested_budget_class: ProfileId,
    pub requested_sensitivity_ceiling: SensitivityClass,
    pub requested_disclosure_ceiling: DisclosureCeiling,
    pub requested_source_read_permission: bool,
    pub requested_exact_scan_permission: bool,
    pub requested_ttl_ms: u64,
}

/// One immutable authoritative binding/policy capture.
///
/// The fixed principal/scope/domain fields are supplied by the authenticated
/// binding owner. The request cannot replace them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritativeGrantPolicy {
    pub binding_id: BindingId,
    pub binding_generation: u64,
    pub policy_generation: u64,
    pub installation_id: InstallationId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub principal_opaque_id: OpaqueId,
    pub client_scope_ref: OpaqueRef,
    pub scope_domain_id: ScopeDomainId,
    pub allowed_membership_ids: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    pub allowed_corpus_or_portfolio_ids: BoundedSet<CorpusOrPortfolioId, MAX_SET_ITEMS>,
    pub reference_portfolio_revision: Option<PortfolioRevision>,
    pub allowed_access_partitions: BoundedSet<AccessPartitionId, MAX_SET_ITEMS>,
    pub allowed_modalities: BoundedSet<Modality, MAX_SET_ITEMS>,
    pub permitted_recipe_families: BoundedSet<RecipeIdV1, MAX_SET_ITEMS>,
    pub allowed_budget_classes: BoundedSet<ProfileId, MAX_SET_ITEMS>,
    pub sensitivity_ceiling: SensitivityClass,
    pub disclosure_ceiling: DisclosureCeiling,
    pub source_read_permission: bool,
    pub exact_scan_permission: bool,
    pub issued_boot_id: OpaqueId,
    pub revocation_generation: u64,
    pub maximum_ttl_ms: u64,
}

/// Exact non-widened template handed to the issuer.
///
/// This is content-free authorization metadata. It deliberately omits grant
/// identity, nonce and timestamps, which come only from the injected issuer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneGrantTemplate {
    pub operation_id: OpaqueId,
    pub binding_id: BindingId,
    pub binding_generation: u64,
    pub policy_generation: u64,
    pub installation_id: InstallationId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub principal_opaque_id: OpaqueId,
    pub client_scope_ref: OpaqueRef,
    pub scope_domain_id: ScopeDomainId,
    pub allowed_membership_ids: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    pub allowed_corpus_or_portfolio_ids: BoundedSet<CorpusOrPortfolioId, MAX_SET_ITEMS>,
    pub reference_portfolio_revision: Option<PortfolioRevision>,
    pub allowed_access_partitions: BoundedSet<AccessPartitionId, MAX_SET_ITEMS>,
    pub allowed_modalities: BoundedSet<Modality, MAX_SET_ITEMS>,
    pub permitted_recipe_families: BoundedSet<RecipeIdV1, MAX_SET_ITEMS>,
    pub maximum_budget_class: ProfileId,
    pub sensitivity_ceiling: SensitivityClass,
    pub disclosure_ceiling: DisclosureCeiling,
    pub source_read_permission: bool,
    pub exact_scan_permission: bool,
    pub issued_boot_id: OpaqueId,
    pub revocation_generation: u64,
    pub requested_ttl_ms: u64,
}

/// Trusted identity/time material returned by the exact issuer.
///
/// Echoed operation, binding and generation fields prevent a stale or foreign
/// receipt from being attached to a newer policy capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneGrantMaterial {
    pub operation_id: OpaqueId,
    pub binding_id: BindingId,
    pub binding_generation: u64,
    pub policy_generation: u64,
    pub grant_id: GrantId,
    pub nonce: OpaqueId,
    pub issued_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
    pub effective_ttl_ms: u64,
}

/// Closed issuer failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantIssuerError {
    Unavailable,
    CapacityExceeded,
    OperationConflict,
    OutcomeUnknown,
}

/// Injected owner of operation identity, CSPRNG identity and trusted time.
///
/// Implementations must reconstruct an equal receipt for the same operation
/// and equal template, and reject the same operation with a different template.
pub trait StandaloneGrantIssuer {
    fn issue(
        &mut self,
        template: &StandaloneGrantTemplate,
    ) -> Result<StandaloneGrantMaterial, GrantIssuerError>;
}

/// Closed composition failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantMintError {
    BindingMismatch,
    BindingGenerationStale,
    PolicyGenerationStale,
    PolicyInvalid,
    RequestedScopeEmpty,
    RequestedScopeUnauthorized,
    RequestedCeilingWidening,
    RequestedTtlInvalid,
    IssuerUnavailable,
    IssuerCapacityExceeded,
    IssuerOperationConflict,
    IssuerOutcomeUnknown,
    IssuerReceiptMismatch,
    IssuerReturnedInvalidGrant,
}

impl GrantMintError {
    /// Stable machine-readable reason.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::BindingMismatch => "DAEMON_GRANT_BINDING_MISMATCH",
            Self::BindingGenerationStale => "DAEMON_GRANT_BINDING_GENERATION_STALE",
            Self::PolicyGenerationStale => "DAEMON_GRANT_POLICY_GENERATION_STALE",
            Self::PolicyInvalid => "DAEMON_GRANT_POLICY_INVALID",
            Self::RequestedScopeEmpty => "DAEMON_GRANT_SCOPE_EMPTY",
            Self::RequestedScopeUnauthorized => "DAEMON_GRANT_SCOPE_UNAUTHORIZED",
            Self::RequestedCeilingWidening => "DAEMON_GRANT_CEILING_WIDENING",
            Self::RequestedTtlInvalid => "DAEMON_GRANT_TTL_INVALID",
            Self::IssuerUnavailable => "DAEMON_GRANT_ISSUER_UNAVAILABLE",
            Self::IssuerCapacityExceeded => "DAEMON_GRANT_ISSUER_CAPACITY_EXCEEDED",
            Self::IssuerOperationConflict => "DAEMON_GRANT_OPERATION_CONFLICT",
            Self::IssuerOutcomeUnknown => "DAEMON_GRANT_OUTCOME_UNKNOWN",
            Self::IssuerReceiptMismatch => "DAEMON_GRANT_RECEIPT_MISMATCH",
            Self::IssuerReturnedInvalidGrant => "DAEMON_GRANT_INVALID_ISSUER_RESULT",
        }
    }
}

/// Narrows one standalone request and asks the injected issuer for exact
/// identity/time material.
///
/// No client field can add a membership, corpus, partition, modality, recipe,
/// budget class, disclosure/sensitivity ceiling or permission absent from the
/// captured policy. Empty membership/recipe/modality scopes fail closed.
pub fn mint_standalone_grant<I: StandaloneGrantIssuer>(
    issuer: &mut I,
    request: &StandaloneGrantRequest,
    policy: &AuthoritativeGrantPolicy,
) -> Result<SearchReadGrantClaims, GrantMintError> {
    validate_request(request, policy)?;
    let template = build_template(request, policy);
    let material = issuer.issue(&template).map_err(map_issuer_error)?;
    validate_material(&template, policy, &material)?;

    let claims = SearchReadGrantClaims {
        grant_id: material.grant_id,
        installation_id: template.installation_id,
        installation_incarnation_id: template.installation_incarnation_id,
        binding_id: template.binding_id,
        principal_opaque_id: template.principal_opaque_id,
        client_scope_ref: template.client_scope_ref,
        scope_domain_id: template.scope_domain_id,
        allowed_membership_ids: template.allowed_membership_ids,
        allowed_corpus_or_portfolio_ids: template.allowed_corpus_or_portfolio_ids,
        reference_portfolio_revision: template.reference_portfolio_revision,
        allowed_access_partitions: template.allowed_access_partitions,
        allowed_modalities: template.allowed_modalities,
        permitted_recipe_families: template.permitted_recipe_families,
        maximum_budget_class: template.maximum_budget_class,
        sensitivity_ceiling: template.sensitivity_ceiling,
        disclosure_ceiling: template.disclosure_ceiling,
        source_read_permission: template.source_read_permission,
        exact_scan_permission: template.exact_scan_permission,
        issued_boot_id: template.issued_boot_id,
        issued_at: material.issued_at,
        expires_at: material.expires_at,
        nonce: material.nonce,
        revocation_generation: template.revocation_generation,
    };
    claims
        .validate_shape()
        .map_err(|_| GrantMintError::IssuerReturnedInvalidGrant)?;
    Ok(claims)
}

fn validate_request(
    request: &StandaloneGrantRequest,
    policy: &AuthoritativeGrantPolicy,
) -> Result<(), GrantMintError> {
    if policy.maximum_ttl_ms == 0
        || policy.binding_generation == 0
        || policy.policy_generation == 0
        || (policy.exact_scan_permission && !policy.source_read_permission)
        || (has_portfolio(&request.requested_corpus_or_portfolio_ids)
            && policy.reference_portfolio_revision.is_none())
    {
        return Err(GrantMintError::PolicyInvalid);
    }
    if request.binding_id != policy.binding_id {
        return Err(GrantMintError::BindingMismatch);
    }
    if request.expected_binding_generation != policy.binding_generation {
        return Err(GrantMintError::BindingGenerationStale);
    }
    if request.expected_policy_generation != policy.policy_generation {
        return Err(GrantMintError::PolicyGenerationStale);
    }
    if request.requested_membership_ids.is_empty()
        || request.requested_modalities.is_empty()
        || request.requested_recipe_families.is_empty()
    {
        return Err(GrantMintError::RequestedScopeEmpty);
    }
    if request.requested_ttl_ms == 0 || request.requested_ttl_ms > policy.maximum_ttl_ms {
        return Err(GrantMintError::RequestedTtlInvalid);
    }
    if !is_subset(
        &request.requested_membership_ids,
        &policy.allowed_membership_ids,
    ) || !is_subset(
        &request.requested_corpus_or_portfolio_ids,
        &policy.allowed_corpus_or_portfolio_ids,
    ) || !is_subset(
        &request.requested_access_partitions,
        &policy.allowed_access_partitions,
    ) || !is_subset(&request.requested_modalities, &policy.allowed_modalities)
        || !is_subset(
            &request.requested_recipe_families,
            &policy.permitted_recipe_families,
        )
        || !policy
            .allowed_budget_classes
            .contains(&request.requested_budget_class)
    {
        return Err(GrantMintError::RequestedScopeUnauthorized);
    }
    if request.requested_sensitivity_ceiling > policy.sensitivity_ceiling
        || request.requested_disclosure_ceiling > policy.disclosure_ceiling
        || (request.requested_source_read_permission && !policy.source_read_permission)
        || (request.requested_exact_scan_permission && !policy.exact_scan_permission)
        || (request.requested_exact_scan_permission
            && !request.requested_source_read_permission)
    {
        return Err(GrantMintError::RequestedCeilingWidening);
    }
    Ok(())
}

fn build_template(
    request: &StandaloneGrantRequest,
    policy: &AuthoritativeGrantPolicy,
) -> StandaloneGrantTemplate {
    StandaloneGrantTemplate {
        operation_id: request.operation_id.clone(),
        binding_id: policy.binding_id,
        binding_generation: policy.binding_generation,
        policy_generation: policy.policy_generation,
        installation_id: policy.installation_id,
        installation_incarnation_id: policy.installation_incarnation_id,
        principal_opaque_id: policy.principal_opaque_id.clone(),
        client_scope_ref: policy.client_scope_ref.clone(),
        scope_domain_id: policy.scope_domain_id,
        allowed_membership_ids: request.requested_membership_ids.clone(),
        allowed_corpus_or_portfolio_ids: request
            .requested_corpus_or_portfolio_ids
            .clone(),
        reference_portfolio_revision: if has_portfolio(
            &request.requested_corpus_or_portfolio_ids,
        ) {
            policy.reference_portfolio_revision
        } else {
            None
        },
        allowed_access_partitions: request.requested_access_partitions.clone(),
        allowed_modalities: request.requested_modalities.clone(),
        permitted_recipe_families: request.requested_recipe_families.clone(),
        maximum_budget_class: request.requested_budget_class.clone(),
        sensitivity_ceiling: request.requested_sensitivity_ceiling,
        disclosure_ceiling: request.requested_disclosure_ceiling,
        source_read_permission: request.requested_source_read_permission,
        exact_scan_permission: request.requested_exact_scan_permission,
        issued_boot_id: policy.issued_boot_id.clone(),
        revocation_generation: policy.revocation_generation,
        requested_ttl_ms: request.requested_ttl_ms,
    }
}

fn validate_material(
    template: &StandaloneGrantTemplate,
    policy: &AuthoritativeGrantPolicy,
    material: &StandaloneGrantMaterial,
) -> Result<(), GrantMintError> {
    if material.operation_id != template.operation_id
        || material.binding_id != template.binding_id
        || material.binding_generation != template.binding_generation
        || material.policy_generation != template.policy_generation
    {
        return Err(GrantMintError::IssuerReceiptMismatch);
    }
    if material.effective_ttl_ms == 0
        || material.effective_ttl_ms > template.requested_ttl_ms
        || material.effective_ttl_ms > policy.maximum_ttl_ms
    {
        return Err(GrantMintError::IssuerReturnedInvalidGrant);
    }
    if material.expires_at <= material.issued_at {
        return Err(GrantMintError::IssuerReturnedInvalidGrant);
    }
    Ok(())
}

fn map_issuer_error(error: GrantIssuerError) -> GrantMintError {
    match error {
        GrantIssuerError::Unavailable => GrantMintError::IssuerUnavailable,
        GrantIssuerError::CapacityExceeded => GrantMintError::IssuerCapacityExceeded,
        GrantIssuerError::OperationConflict => GrantMintError::IssuerOperationConflict,
        GrantIssuerError::OutcomeUnknown => GrantMintError::IssuerOutcomeUnknown,
    }
}

fn is_subset<T: Ord, const LIMIT: usize>(
    requested: &BoundedSet<T, LIMIT>,
    allowed: &BoundedSet<T, LIMIT>,
) -> bool {
    requested.iter().all(|item| allowed.contains(item))
}

fn has_portfolio<const LIMIT: usize>(values: &BoundedSet<CorpusOrPortfolioId, LIMIT>) -> bool {
    values
        .iter()
        .any(|item| matches!(item, CorpusOrPortfolioId::Portfolio(_)))
}

#[cfg(test)]
mod tests;
