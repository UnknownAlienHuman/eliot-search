//! Durable client-binding and standalone grant-policy authority contracts.
//!
//! These records contain bounded authorization metadata only. They contain no
//! pairing key, credential bytes, source content, query text, path, Qdrant name
//! or reusable client secret. Construction is not authority: consumers still
//! require one committed, current, independently authenticated control snapshot.

use crate::bounds::{BoundedSet, MAX_SET_ITEMS};
use crate::canonical::{OpaqueId, OpaqueRef, UtcTimestamp};
use crate::ids::{
    AccessPartitionId, BindingId, Blake3Digest32, CorpusOrPortfolioId, InstallationId,
    InstallationIncarnationId, NonZeroRevision, PortfolioRevision, ProfileId, ScopeDomainId,
    SourceMembershipId,
};
use crate::protocol::PeerRole;
use crate::recipes::RecipeIdV1;
use crate::schema::{DisclosureCeiling, Modality, SensitivityClass};
use crate::{ContractError, ContractErrorKind};

/// Durable lifecycle of one authenticated provider binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProviderBindingStatus {
    /// Current registration; pairing, lifetime and policy still require checks.
    Active,
    /// Explicit terminal revocation.
    Revoked,
    /// Explicit terminal expiry.
    Expired,
}

/// Durable provider binding metadata.
///
/// The record identifies the peer registration and its permitted client
/// profiles. It is not a pairing proof, grant, access permit or secret lease.
#[derive(Clone, Eq, PartialEq)]
pub struct ProviderBindingRecord {
    pub binding_id: BindingId,
    pub installation_id: InstallationId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub peer_role: PeerRole,
    pub peer_identity_digest: Blake3Digest32,
    pub pairing_generation: NonZeroRevision,
    pub permitted_profile_ids: BoundedSet<ProfileId, MAX_SET_ITEMS>,
    pub disclosure_ceiling_ref: OpaqueRef,
    pub issued_at: UtcTimestamp,
    pub expires_at: Option<UtcTimestamp>,
    pub revocation_generation: NonZeroRevision,
    pub status: ProviderBindingStatus,
}

impl ProviderBindingRecord {
    /// Validate the closed binding-record shape without claiming currentness.
    pub fn validate_shape(&self) -> Result<(), ContractError> {
        if !matches!(self.peer_role, PeerRole::StandaloneCli | PeerRole::ClientAdapter) {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "provider_binding.peer_role",
            ));
        }
        if self
            .expires_at
            .as_ref()
            .is_some_and(|expires_at| expires_at <= &self.issued_at)
        {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "provider_binding.expires_at",
            ));
        }
        Ok(())
    }
}

impl core::fmt::Debug for ProviderBindingRecord {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ProviderBindingRecord")
            .field("status", &self.status)
            .field("pairing_generation", &self.pairing_generation)
            .finish_non_exhaustive()
    }
}

/// One immutable authoritative binding/policy capture.
///
/// Fixed principal, client-scope and security-domain identities come from the
/// authenticated control owner. A client request can only narrow this closure.
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

impl AuthoritativeGrantPolicy {
    /// Validate the closed policy shape without claiming publication/currentness.
    pub fn validate_shape(&self) -> Result<(), ContractError> {
        if self.binding_generation == 0
            || self.policy_generation == 0
            || self.revocation_generation == 0
            || self.maximum_ttl_ms <= 1
        {
            return Err(ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "authoritative_grant_policy.generation_or_ttl",
            ));
        }
        if self.allowed_membership_ids.is_empty()
            || self.allowed_access_partitions.is_empty()
            || self.allowed_modalities.is_empty()
            || self.permitted_recipe_families.is_empty()
            || self.allowed_budget_classes.is_empty()
        {
            return Err(ContractError::new(
                ContractErrorKind::Empty,
                "authoritative_grant_policy.required_set",
            ));
        }
        if self.exact_scan_permission && !self.source_read_permission {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "authoritative_grant_policy.exact_scan_permission",
            ));
        }
        if self.reference_portfolio_revision.is_none()
            && self
                .allowed_corpus_or_portfolio_ids
                .iter()
                .any(|id| matches!(id, CorpusOrPortfolioId::Portfolio(_)))
        {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "authoritative_grant_policy.reference_portfolio_revision",
            ));
        }
        Ok(())
    }
}

/// Durable lifecycle of one standalone grant-policy row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StandalonePolicyState {
    /// Current policy, subject to its independent lifetime and generations.
    Active,
    /// Explicit terminal revocation.
    Revoked,
    /// Explicit terminal expiry.
    Expired,
}

/// Durable standalone grant-policy metadata.
///
/// The row is useful only when read atomically with the exact current binding
/// and journal generation. Missing or terminal rows never imply default access.
#[derive(Clone, Eq, PartialEq)]
pub struct StandalonePolicyRecord {
    pub policy: AuthoritativeGrantPolicy,
    pub state: StandalonePolicyState,
    pub issued_at: UtcTimestamp,
    pub expires_at: Option<UtcTimestamp>,
}

impl StandalonePolicyRecord {
    /// Validate the closed policy-row shape without claiming currentness.
    pub fn validate_shape(&self) -> Result<(), ContractError> {
        self.policy.validate_shape()?;
        if self
            .expires_at
            .as_ref()
            .is_some_and(|expires_at| expires_at <= &self.issued_at)
        {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "standalone_policy.expires_at",
            ));
        }
        Ok(())
    }
}

impl core::fmt::Debug for StandalonePolicyRecord {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("StandalonePolicyRecord")
            .field("state", &self.state)
            .field("policy_generation", &self.policy.policy_generation)
            .finish_non_exhaustive()
    }
}
