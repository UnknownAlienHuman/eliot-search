//! Compose validated observed history into the semantic registry, without live authority.
use super::{RetainedRegistryOperation, SourceRegistry};
use crate::error::{RegistryError, RegistryLimits};
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
        namespaces: BTreeMap::new(),
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
