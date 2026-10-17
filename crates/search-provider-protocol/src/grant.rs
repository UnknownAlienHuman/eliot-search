//! Canonical standalone read-grant request and response protocol.
//!
//! The request body carries requested ceilings only. It deliberately omits
//! binding, installation, principal, operation and issued-grant identities: the
//! daemon derives those from an already authenticated session and server-owned
//! policy. The dedicated authenticated request envelope binds the exact canonical
//! body digest under a grant-specific domain without extending the W1 shell
//! command registry. Responses reuse the existing authenticated response envelope
//! and add only one strict status-consistent typed body.

use search_contracts::{
    AccessPartitionId, BoundedSet, CorpusOrPortfolioId, DisclosureCeiling, Modality, ProfileId,
    RecipeIdV1, SearchReadGrantClaims, SensitivityClass,
    SourceMembershipId, MAX_SET_ITEMS,
};
use search_contracts::evidence::RecipeIdV2;

use crate::error::ProtocolError;

/// Versioned canonical body marker.
pub const STANDALONE_GRANT_REQUEST_VERSION: u16 = 1;
/// Independent body ceiling below the canonical 8 MiB frame ceiling.
pub const MAX_STANDALONE_GRANT_REQUEST_BYTES: usize = 1024 * 1024;
/// Versioned canonical body marker for explicit evidence recipe grants.
pub const STANDALONE_EVIDENCE_GRANT_REQUEST_VERSION: u16 = 2;

/// Client-supplied ceilings for one standalone grant.
///
/// Every set and permission is intersected with authoritative server policy.
/// This structure is not a grant and contains no reusable authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneGrantRequestV1 {
    /// Binding generation the client observed while planning.
    pub expected_binding_generation: u64,
    /// Policy generation the client observed while planning.
    pub expected_policy_generation: u64,
    /// Requested source memberships; must be non-empty.
    pub requested_membership_ids: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    /// Requested corpus/reference-portfolio identities.
    pub requested_corpus_or_portfolio_ids: BoundedSet<CorpusOrPortfolioId, MAX_SET_ITEMS>,
    /// Requested access partitions.
    pub requested_access_partitions: BoundedSet<AccessPartitionId, MAX_SET_ITEMS>,
    /// Requested modalities; must be non-empty.
    pub requested_modalities: BoundedSet<Modality, MAX_SET_ITEMS>,
    /// Requested recipe families; must be non-empty.
    pub requested_recipe_families: BoundedSet<RecipeIdV1, MAX_SET_ITEMS>,
    /// Requested maximum budget class.
    pub requested_budget_class: ProfileId,
    /// Requested sensitivity ceiling.
    pub requested_sensitivity_ceiling: SensitivityClass,
    /// Requested disclosure ceiling.
    pub requested_disclosure_ceiling: DisclosureCeiling,
    /// Whether exact source readback is requested.
    pub requested_source_read_permission: bool,
    /// Whether exact-scan execution is requested.
    pub requested_exact_scan_permission: bool,
    /// Requested finite TTL ceiling in milliseconds.
    pub requested_ttl_ms: u64,
}

impl StandaloneGrantRequestV1 {
    /// Validates request shape without consulting authority.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidBody`] for zero generations/TTL,
    /// required empty scopes or exact-scan permission without source-read
    /// permission.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.expected_binding_generation == 0
            || self.expected_policy_generation == 0
            || self.requested_membership_ids.is_empty()
            || self.requested_modalities.is_empty()
            || self.requested_recipe_families.is_empty()
            || self.requested_ttl_ms == 0
            || (self.requested_exact_scan_permission
                && !self.requested_source_read_permission)
        {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(())
    }
}

/// Client-supplied ceilings for an explicit evidence/orientation grant.
///
/// Its distinct versioned body can request only the two exact `RecipeIdV2`
/// values. Shared scope ceilings are still narrowed by the server policy and
/// issuer; this structure is not a grant or a source-access decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneGrantRequestV2 {
    /// Binding generation the client observed while planning.
    pub expected_binding_generation: u64,
    /// Policy generation the client observed while planning.
    pub expected_policy_generation: u64,
    /// Requested source memberships; must be non-empty.
    pub requested_membership_ids: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    /// Requested corpus/reference-portfolio identities.
    pub requested_corpus_or_portfolio_ids: BoundedSet<CorpusOrPortfolioId, MAX_SET_ITEMS>,
    /// Requested access partitions.
    pub requested_access_partitions: BoundedSet<AccessPartitionId, MAX_SET_ITEMS>,
    /// Requested modalities; must be non-empty.
    pub requested_modalities: BoundedSet<Modality, MAX_SET_ITEMS>,
    /// Exact requested evidence/orientation recipe permissions; must be non-empty.
    pub requested_evidence_recipes: BoundedSet<RecipeIdV2, MAX_SET_ITEMS>,
    /// Requested maximum budget class.
    pub requested_budget_class: ProfileId,
    /// Requested sensitivity ceiling.
    pub requested_sensitivity_ceiling: SensitivityClass,
    /// Requested disclosure ceiling.
    pub requested_disclosure_ceiling: DisclosureCeiling,
    /// Whether exact source readback is requested.
    pub requested_source_read_permission: bool,
    /// Whether exact-scan execution is requested.
    pub requested_exact_scan_permission: bool,
    /// Requested finite TTL ceiling in milliseconds.
    pub requested_ttl_ms: u64,
}

impl StandaloneGrantRequestV2 {
    /// Validates request shape without consulting server authority.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.expected_binding_generation == 0
            || self.expected_policy_generation == 0
            || self.requested_membership_ids.is_empty()
            || self.requested_modalities.is_empty()
            || self.requested_evidence_recipes.is_empty()
            || !self.requested_source_read_permission
            || self.requested_exact_scan_permission
            || self.requested_ttl_ms == 0
        {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(())
    }
}

/// One V2 grant claim envelope: unchanged shared claims plus explicit V2 recipe
/// permissions verified against the original bounded issuer ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchReadGrantClaimsV2 {
    /// Common issued claims, retaining their exact v1 field semantics.
    pub claims: SearchReadGrantClaims,
    /// Recipes explicitly permitted by the issued V2 grant.
    pub permitted_evidence_recipes: BoundedSet<RecipeIdV2, MAX_SET_ITEMS>,
}

impl SearchReadGrantClaimsV2 {
    /// Checks only the closed wire shape; the issuer ledger remains authoritative.
    pub fn validate_shape(&self) -> Result<(), ProtocolError> {
        self.claims
            .validate_shape()
            .map_err(|_| ProtocolError::InvalidBody)?;
        if !self.claims.source_read_permission
            || self.permitted_evidence_recipes.is_empty()
            || self
                .permitted_evidence_recipes
                .iter()
                .any(|recipe| !RecipeIdV2::ALL.contains(recipe))
        {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(())
    }
}

mod codec;
mod envelope;
mod response;

pub use codec::{
    decode_standalone_grant_request, decode_standalone_grant_request_v2,
    encode_standalone_grant_request, encode_standalone_grant_request_v2,
};
pub use envelope::{
    AuthenticatedStandaloneGrantEnvelope, MAX_STANDALONE_GRANT_ENVELOPE_JSON_BYTES,
    STANDALONE_GRANT_ENVELOPE_DOMAIN, decode_standalone_grant_envelope,
    decode_standalone_grant_envelope_json, encode_standalone_grant_envelope,
    encode_standalone_grant_envelope_json, seal_standalone_grant_envelope,
    standalone_grant_envelope_transcript, verify_standalone_grant_envelope_proof,
};
pub use response::{
    MAX_STANDALONE_GRANT_RESPONSE_BYTES, STANDALONE_GRANT_RESPONSE_VERSION,
    STANDALONE_EVIDENCE_GRANT_RESPONSE_VERSION,
    StandaloneGrantResponseBodyV1, decode_standalone_grant_response_body,
    StandaloneGrantResponseBodyV2, decode_standalone_grant_response_body_v2,
    encode_standalone_grant_response_body, encode_standalone_grant_response_body_v2,
};

#[cfg(test)]
mod tests;
