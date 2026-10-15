//! Compose validated observed history into the semantic registry, without live authority.
use super::{RetainedRegistryOperation, SourceRegistry};
use crate::error::{RegistryError, RegistryLimits};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_records::batch::{BatchCodecV4Error, OriginalInputResolver};
use std::collections::BTreeMap;

pub use search_source_history::registry_restore::{
    MAX_REGISTRY_RESTORE_BYTES, RegistryRestoreOperation, RegistryRestoreState,
    preflight_registry_restore_inputs,
};

/// Observed values and immutable history; no native, live-policy or serving grant.
#[derive(Clone, Debug)]
pub struct ObservedRegistryRestore {
    registry: SourceRegistry,
}
impl ObservedRegistryRestore {
    /// Read-only observed owner values; external live revalidation remains required.
    pub const fn registry(&self) -> &SourceRegistry {
        &self.registry
    }
    /// Transfer observed metadata; root/security/current-policy revalidation stays mandatory.
    pub fn into_registry(self) -> SourceRegistry {
        self.registry
    }
}

/// Restore complete bounded state/history through its sole observed-history validator.
///
/// I/O coherence and current native/security/policy qualification remain external.
pub fn restore_registry_state(
    registry_revision: u64,
    states: &[RegistryRestoreState<'_>],
    operations: &[RegistryRestoreOperation<'_>],
    limits: RegistryLimits,
    max_total_bytes: usize,
) -> Result<ObservedRegistryRestore, RegistryError> {
    let observed = search_source_history::registry_restore::restore_registry_state(
        registry_revision,
        states,
        operations,
        limits,
        max_total_bytes,
    )?;
    let parts = observed.into_parts();
    let registry = SourceRegistry {
        limits: parts.limits,
        revision: parts.revision,
        roots: parts.roots,
        sources: parts.sources,
        memberships: parts.memberships,
        reverse_memberships: parts.reverse_memberships,
        portfolios: BTreeMap::new(),
        portfolio_history: crate::portfolio_history::PortfolioHistory::new(),
        namespaces: parts.namespaces,
        active_generations: parts.active_generations,
        operations: parts
            .operations
            .into_iter()
            .map(RetainedRegistryOperation::Committed)
            .collect(),
        pending_batch: None,
        state_encoding_profiles: parts.state_encoding_profiles,
    };
    Ok(ObservedRegistryRestore { registry })
}

/// Restore complete observed state/history through the permanent-v2 owner
/// resolver, without granting live root, policy, native or serving authority.
pub fn restore_registry_state_v4<R, P>(
    registry_revision: u64,
    states: &[RegistryRestoreState<'_>],
    operations: &[RegistryRestoreOperation<'_>],
    limits: RegistryLimits,
    max_total_bytes: usize,
    resolver: &mut R,
    original_probe: &mut P,
) -> Result<ObservedRegistryRestore, BatchCodecV4Error>
where
    R: OriginalInputResolver + ?Sized,
    P: FnMut() -> Result<(), CheckpointFailureV2>,
{
    let observed = search_source_history::registry_restore::restore_registry_state_v4(
        registry_revision,
        states,
        operations,
        limits,
        max_total_bytes,
        resolver,
        original_probe,
    )?;
    let parts = observed.into_parts();
    let registry = SourceRegistry {
        limits: parts.limits,
        revision: parts.revision,
        roots: parts.roots,
        sources: parts.sources,
        memberships: parts.memberships,
        reverse_memberships: parts.reverse_memberships,
        portfolios: BTreeMap::new(),
        portfolio_history: crate::portfolio_history::PortfolioHistory::new(),
        namespaces: parts.namespaces,
        active_generations: parts.active_generations,
        operations: parts
            .operations
            .into_iter()
            .map(RetainedRegistryOperation::Committed)
            .collect(),
        pending_batch: None,
        state_encoding_profiles: parts.state_encoding_profiles,
    };
    Ok(ObservedRegistryRestore { registry })
}
