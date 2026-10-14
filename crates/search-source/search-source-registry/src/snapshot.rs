//! Immutable registry snapshots, validation and canonical digests.
//!
//! Snapshots are the exact inputs to view resolution and cutover
//! verification. Validation fails closed on missing, duplicated or incoherent
//! records; digests cover technical state and immutable refs only.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, OpaqueId, ReferencePortfolioId, RootBindingId, SourceIdentity,
    SourceNamespaceId, SourceNamespaceOwnership,
};

use crate::error::{RegistryError, RegistryLimits};
use crate::membership::{MembershipKey, MembershipLifecycle, MembershipRecord};
use crate::portfolio::ReferencePortfolioRecord;
use crate::portfolio_history::PortfolioHistory;
use crate::root::RootRecord;
use crate::source::{RegisteredSource, SourceLifecycle};
#[path = "snapshot_canonical.rs"]
mod canonical;

/// Immutable registry snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistrySnapshot {
    /// Exact registry revision captured by the snapshot.
    pub revision: u64,
    /// Admitted roots.
    pub roots: BTreeMap<RootBindingId, RootRecord>,
    /// Admitted sources.
    pub sources: BTreeMap<SourceIdentity, RegisteredSource>,
    /// Source/corpus memberships.
    pub memberships: BTreeMap<MembershipKey, MembershipRecord>,
    /// Reference portfolios.
    pub portfolios: BTreeMap<ReferencePortfolioId, ReferencePortfolioRecord>,
    /// Immutable portfolio revisions and the guarded current pointer.
    ///
    /// Every retained revision stays readable after any pointer move, so an
    /// old query or handle keeps its exact interpretation.
    pub portfolio_history: PortfolioHistory,
    /// Active corpus generations.
    pub active_generations: BTreeMap<OpaqueId, NonZeroRevisionAlias>,
    /// Namespace ownership records.
    pub ownerships: BTreeMap<SourceNamespaceId, SourceNamespaceOwnership>,
}

/// Alias preserving the exact [`search_contracts::NonZeroRevision`] shape
/// without leaking a second revision newtype.
pub type NonZeroRevisionAlias = search_contracts::NonZeroRevision;

/// Validated immutable registry snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedRegistrySnapshot {
    inner: RegistrySnapshot,
}

impl ValidatedRegistrySnapshot {
    /// Exact validated snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &RegistrySnapshot {
        &self.inner
    }

    /// Exact registry revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.inner.revision
    }
}

/// Domain-separated canonical digest over technical registry state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegistrySnapshotDigest(pub Blake3Digest32);

/// Validates registry/root/source/membership/portfolio/owner generations,
/// uniqueness, referential integrity, closed states and bounded counts.
pub fn validate_registry_snapshot(
    snapshot: &RegistrySnapshot,
    limits: RegistryLimits,
) -> Result<ValidatedRegistrySnapshot, RegistryError> {
    limits.validate()?;
    if snapshot.roots.len() > limits.max_roots
        || snapshot.sources.len() > limits.max_sources
        || snapshot.memberships.len() > limits.max_memberships
        || snapshot.portfolios.len() > limits.max_portfolios
        || snapshot.ownerships.len() > limits.max_namespaces
    {
        return Err(RegistryError::CapacityExceeded);
    }
    for (id, root) in &snapshot.roots {
        if &root.root_binding_id != id {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    for (identity, source) in &snapshot.sources {
        if source.identity() != identity {
            return Err(RegistryError::SnapshotInvalid);
        }
        let root_id = source.binding().root_binding_id();
        if !snapshot.roots.contains_key(&root_id) {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    let mut reverse_ids = BTreeSet::new();
    for (key, membership) in &snapshot.memberships {
        if membership.key() != key {
            return Err(RegistryError::SnapshotInvalid);
        }
        if !reverse_ids.insert(membership.membership_id()) {
            return Err(RegistryError::SnapshotInvalid);
        }
        let source = snapshot
            .sources
            .get(&key.source_identity)
            .ok_or(RegistryError::SnapshotInvalid)?;
        if source.lifecycle() == SourceLifecycle::Retired
            && membership.lifecycle() == MembershipLifecycle::Active
        {
            return Err(RegistryError::SnapshotInvalid);
        }
        if let Some(active) = snapshot.active_generations.get(&key.corpus_id) {
            if membership.lifecycle() == MembershipLifecycle::Active
                && &membership.generation() != active
            {
                return Err(RegistryError::SnapshotInvalid);
            }
        } else if membership.lifecycle() == MembershipLifecycle::Active {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    for record in snapshot.portfolios.values() {
        if record.membership_precedence.len() > limits.max_portfolio_items {
            return Err(RegistryError::SnapshotInvalid);
        }
        let mut seen = BTreeSet::new();
        for membership_id in &record.membership_precedence {
            if !seen.insert(*membership_id) {
                return Err(RegistryError::SnapshotInvalid);
            }
        }
    }
    validate_portfolio_history(&snapshot.portfolio_history, limits)?;
    for (namespace, ownership) in &snapshot.ownerships {
        if ownership.source_namespace_id != *namespace {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    let current = snapshot.portfolio_history.current_view();
    if current.len() != snapshot.portfolios.len()
        || current.iter().any(|(id, projection)| {
            snapshot.portfolios.get(id).is_none_or(|record| {
                record.portfolio_id != *id
                    || record.portfolio_revision != projection.portfolio_revision
                    || record.membership_precedence != projection.membership_precedence
                    || record.registry_revision != projection.registry_revision
                    || record.last_receipt != projection.last_receipt
            })
        })
    {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok(ValidatedRegistrySnapshot {
        inner: snapshot.clone(),
    })
}

/// Validates the immutable portfolio history and its guarded current pointer.
///
/// Every retained revision is keyed by its own content and every current
/// pointer must name a retained revision. A pointer to an absent revision is
/// refused rather than treated as an empty scope, so a history gap stays
/// explicit instead of silently degrading to nothing.
fn validate_portfolio_history(
    history: &PortfolioHistory,
    limits: RegistryLimits,
) -> Result<(), RegistryError> {
    if history.len() > limits.max_portfolio_revisions {
        return Err(RegistryError::CapacityExceeded);
    }
    for record in history.revisions() {
        if record.membership_precedence().len() > limits.max_portfolio_items {
            return Err(RegistryError::SnapshotInvalid);
        }
        let mut seen = BTreeSet::new();
        for membership_id in record.membership_precedence() {
            if !seen.insert(*membership_id) {
                return Err(RegistryError::SnapshotInvalid);
            }
        }
    }
    for (portfolio_id, pointer) in history.current_pointers() {
        if pointer.target().portfolio_id != *portfolio_id
            || history.revision(&pointer.target()).is_none()
        {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    Ok(())
}

/// Computes the version-3 canonical digest of every snapshot field.
///
/// The flattened ownership projection does not persist cutover preparation,
/// fence or activation state. Those remain in NamespaceCutoverState.
///
/// # Errors
/// Returns no digest when the shared canonical item/byte ceilings are exceeded.
pub fn snapshot_digest(
    snapshot: &RegistrySnapshot,
) -> Result<RegistrySnapshotDigest, RegistryError> {
    canonical::digest(snapshot)
}
