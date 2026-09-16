//! Canonical bounded body for requesting one standalone read grant.
//!
//! This body carries requested ceilings only. It deliberately omits binding,
//! installation, principal, operation and issued-grant identities: the daemon
//! derives those from an already authenticated session and server-owned policy.
//! The exact canonical bytes are intended to be committed by the authenticated
//! envelope body digest before daemon composition.

use search_contracts::{
    AccessPartitionId, BoundedSet, CorpusOrPortfolioId, DisclosureCeiling, Modality, ProfileId,
    RecipeIdV1, SensitivityClass, SourceMembershipId, MAX_SET_ITEMS,
};

use crate::error::ProtocolError;

/// Versioned canonical body marker.
pub const STANDALONE_GRANT_REQUEST_VERSION: u16 = 1;
/// Independent body ceiling below the canonical 8 MiB frame ceiling.
pub const MAX_STANDALONE_GRANT_REQUEST_BYTES: usize = 1024 * 1024;

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

mod codec;

pub use codec::{
    decode_standalone_grant_request, encode_standalone_grant_request,
};

#[cfg(test)]
mod tests;
