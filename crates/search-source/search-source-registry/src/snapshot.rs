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
use crate::root::RootRecord;
use crate::source::{RegisteredSource, SourceLifecycle};

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
    Ok(ValidatedRegistrySnapshot {
        inner: snapshot.clone(),
    })
}

/// Computes a domain-separated canonical digest over technical registry state
/// and immutable refs, excluding display paths and content.
#[must_use]
pub fn snapshot_digest(snapshot: &RegistrySnapshot) -> RegistrySnapshotDigest {
    let mut hasher = FnvDigest::new(b"eliot-search-registry-snapshot-v1");
    hasher.mix_u64(snapshot.revision);
    hasher.mix_usize(snapshot.roots.len());
    for (id, root) in &snapshot.roots {
        hasher.mix_bytes(id.as_bytes());
        hasher.mix_bytes(root.canonical_root_digest.as_bytes());
        hasher.mix_bytes(root.policy_fingerprint.as_bytes());
        hasher.mix_u64(root.policy_revision.get());
        hasher.mix_u64(root.record_revision.get());
        hasher.mix_u64(root.registry_revision);
    }
    hasher.mix_usize(snapshot.sources.len());
    for (identity, source) in &snapshot.sources {
        hasher.mix_bytes(identity.source_id.as_bytes());
        hasher.mix_bytes(identity.source_namespace_id.as_bytes());
        hasher.mix_u64(source.source_revision().get());
        hasher.mix_u64(source.registry_revision());
    }
    hasher.mix_usize(snapshot.memberships.len());
    for (key, membership) in &snapshot.memberships {
        hasher.mix_str(key.corpus_id.as_str());
        hasher.mix_bytes(key.source_identity.source_id.as_bytes());
        hasher.mix_u64(membership.generation().get());
        hasher.mix_u64(membership.membership_revision().get());
        hasher.mix_u64(membership.registry_revision());
    }
    hasher.mix_usize(snapshot.portfolios.len());
    for (id, record) in &snapshot.portfolios {
        hasher.mix_bytes(id.as_bytes());
        hasher.mix_u64(record.portfolio_revision.get());
        hasher.mix_u64(record.registry_revision);
    }
    hasher.mix_usize(snapshot.active_generations.len());
    for (corpus, generation) in &snapshot.active_generations {
        hasher.mix_str(corpus.as_str());
        hasher.mix_u64(generation.get());
    }
    RegistrySnapshotDigest(hasher.finish())
}

/// Deterministic FNV-1a based 32-byte digest builder.
struct FnvDigest {
    states: [u64; 4],
}

impl FnvDigest {
    fn new(domain: &[u8]) -> Self {
        let mut states = [
            0xcbf2_9ce4_8422_2325,
            0x8422_2325_cbf2_9ce4,
            0x9ce4_8422_2325_cbf2,
            0x2325_cbf2_9ce4_8422,
        ];
        for (index, byte) in domain.iter().enumerate() {
            let slot = index % states.len();
            states[slot] ^= u64::from(*byte);
            states[slot] = states[slot].wrapping_mul(0x1000_0000_01b3);
        }
        Self { states }
    }

    fn mix_bytes(&mut self, bytes: &[u8]) {
        for (index, byte) in bytes.iter().enumerate() {
            let slot = index % self.states.len();
            self.states[slot] ^= u64::from(*byte);
            self.states[slot] = self.states[slot].wrapping_mul(0x1000_0000_01b3);
        }
    }

    fn mix_str(&mut self, value: &str) {
        self.mix_bytes(value.as_bytes());
    }

    fn mix_u64(&mut self, value: u64) {
        self.mix_bytes(&value.to_le_bytes());
    }

    fn mix_usize(&mut self, value: usize) {
        self.mix_u64(value as u64);
    }

    fn finish(self) -> Blake3Digest32 {
        let mut out = [0_u8; 32];
        for (index, state) in self.states.iter().enumerate() {
            out[index * 8..(index + 1) * 8].copy_from_slice(&state.to_le_bytes());
        }
        Blake3Digest32::from_bytes(out)
    }
}
