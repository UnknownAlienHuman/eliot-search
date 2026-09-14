//! Finite append, recovery, tombstone, and exact-deletion state machine.
//!
//! The facade owns only finite in-memory state and read-only accessors. Append,
//! unknown-outcome recovery, lifecycle deletion, and shared validation helpers
//! live in bounded private modules so those responsibilities cannot collapse
//! back into one state-machine monolith.

mod append;
mod lifecycle;
mod recovery;
mod support;

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, OpaqueId};

use super::error::RevisionStoreError;
use super::limits::RevisionStoreLimits;
use super::model::{
    ObjectDeletionReceipt, PurgeTombstone, RevisionKey, RevisionRecord,
    RevisionState, RevisionStoreReceipt, TombstoneScope,
};

/// Finite immutable revision-store state machine.
#[derive(Clone, Debug)]
pub struct RevisionStore {
    limits: RevisionStoreLimits,
    states: BTreeMap<RevisionKey, RevisionState>,
    operations: Vec<(OpaqueId, Blake3Digest32, RevisionStoreReceipt)>,
    deletions: Vec<(OpaqueId, Blake3Digest32, ObjectDeletionReceipt)>,
    tombstones: Vec<PurgeTombstone>,
}

impl RevisionStore {
    /// Creates an empty finite store kernel.
    pub fn new(limits: RevisionStoreLimits) -> Result<Self, RevisionStoreError> {
        Ok(Self {
            limits: limits.validate()?,
            states: BTreeMap::new(),
            operations: Vec::new(),
            deletions: Vec::new(),
            tombstones: Vec::new(),
        })
    }

    /// Returns the exact state of one domain-qualified source/revision key.
    pub fn state(&self, key: &RevisionKey) -> Result<&RevisionState, RevisionStoreError> {
        self.states
            .get(key)
            .ok_or(RevisionStoreError::RevisionNotFound)
    }

    /// Returns one exact active immutable record.
    pub fn active_record(&self, key: &RevisionKey) -> Result<&RevisionRecord, RevisionStoreError> {
        match self.state(key)? {
            RevisionState::Active(record) => Ok(record),
            RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_) => {
                Err(RevisionStoreError::OutcomeUnknown)
            }
            RevisionState::Quarantined { .. } => Err(RevisionStoreError::Quarantined),
        }
    }

    /// Number of retained revision states.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Returns whether no revision state is retained.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Counts every retained operation identity across appends, deletions,
    /// and tombstone installs against the finite operation ceiling.
    fn operation_count(&self) -> usize {
        self.operations
            .len()
            .saturating_add(self.deletions.len())
            .saturating_add(self.tombstones.len())
    }

    /// Returns whether a purge tombstone fences this domain-qualified key.
    fn is_fenced(&self, key: &RevisionKey) -> bool {
        self.tombstones
            .iter()
            .any(|tombstone| match tombstone.scope {
                TombstoneScope::Residency(residency) => residency == key.residency,
                TombstoneScope::Scope(scope) => scope == key.residency.scope,
            })
    }
}
