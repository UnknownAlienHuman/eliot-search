//! Bounded observed-state restoration, without native, live-policy or serving authority.

use super::{
    RegistryStateKey, RegistryStateRecord, RetainedRegistryOperation, SourceRegistry,
    decode_batch_record,
};
use crate::error::{RegistryError, RegistryLimits};
use crate::membership::MembershipLifecycle;
use crate::root::RootStatus;
use crate::source::SourceLifecycle;
use search_contracts::OpaqueId;
use std::collections::BTreeMap;

/// Absolute combined encoded-input ceiling for one bounded restore observation.
pub const MAX_REGISTRY_RESTORE_BYTES: usize = 64 * 1024 * 1024;

/// Actual canonical state key/value bytes from one coherent owner read.
#[derive(Clone, Copy, Debug)]
pub struct RegistryRestoreState<'a> {
    /// Complete existing owner key bytes, including its closed family.
    pub key: &'a [u8],
    /// Complete existing owner value bytes.
    pub value: &'a [u8],
}

/// Actual immutable operation key/value bytes from that same owner read.
#[derive(Clone, Copy, Debug)]
pub struct RegistryRestoreOperation<'a> {
    /// Exact key identity independently read from the operation store.
    pub operation_id: &'a OpaqueId,
    /// Complete existing versioned operation/delta bytes, never a receipt-only row.
    pub value: &'a [u8],
}

/// Validated observed registry values and immutable operation history.
///
/// The I/O owner must establish read coherence/completeness and revalidate live
/// root custody, owner/security fences, policy and grants before service use.
/// This value grants none of those authorities and provides no disk durability proof.
#[derive(Clone, Debug)]
pub struct ObservedRegistryRestore {
    registry: SourceRegistry,
}
impl ObservedRegistryRestore {
    /// Read-only observed owner values; external live revalidation remains required.
    pub const fn registry(&self) -> &SourceRegistry {
        &self.registry
    }
    /// Transfer observed metadata into owner composition, without granting service authority.
    ///
    /// Root/security/current-policy revalidation remains mandatory outside this factory.
    pub fn into_registry(self) -> SourceRegistry {
        self.registry
    }
}

/// Preflight restore counts and combined encoded bytes before decoding or allocation.
///
/// `total_encoded_bytes` includes every state key/value and every operation's
/// UTF-8 identity/value. Callers must use checked addition when accumulating it.
/// This validates capacity only, not state, history, provenance or serving authority.
///
/// # Errors
/// Invalid limits return `InvalidLimits`; count/byte overflow or exceeded
/// capacity returns `CapacityExceeded`, using the same path as restoration.
pub fn preflight_registry_restore_inputs(
    limits: RegistryLimits,
    state_count: usize,
    operation_count: usize,
    total_encoded_bytes: usize,
    max_total_bytes: usize,
) -> Result<(), RegistryError> {
    let limits = limits.validate()?;
    if max_total_bytes == 0 || max_total_bytes > MAX_REGISTRY_RESTORE_BYTES {
        return Err(RegistryError::InvalidLimits);
    }
    let max_states = limits
        .max_roots
        .checked_add(limits.max_sources)
        .and_then(|count| count.checked_add(limits.max_memberships))
        .and_then(|count| count.checked_add(limits.max_memberships))
        .ok_or(RegistryError::CapacityExceeded)?;
    if state_count > max_states
        || operation_count > limits.max_operations
        || operation_count > limits.max_journal_entries
    {
        return Err(RegistryError::CapacityExceeded);
    }
    let mut bytes = 0;
    charge(&mut bytes, total_encoded_bytes, max_total_bytes)
}

/// Restore the complete four-family state and its committed batch history.
///
/// No format is added: existing owner key/value and operation decoders are used.
/// The independent revision must match a contiguous operation chain from genesis;
/// replayed exact before/after values must equal every supplied current state row.
/// Missing history, unknown/duplicate rows, unavailable original admission inputs,
/// inconsistent relationships and regressing counters refuse without partial state.
pub fn restore_registry_state(
    registry_revision: u64,
    states: &[RegistryRestoreState<'_>],
    operations: &[RegistryRestoreOperation<'_>],
    limits: RegistryLimits,
    max_total_bytes: usize,
) -> Result<ObservedRegistryRestore, RegistryError> {
    preflight_registry_restore_inputs(limits, states.len(), operations.len(), 0, max_total_bytes)?;
    // Incremental charging keeps early overflow/byte refusal while sharing the
    // public preflight's byte-bound implementation before any semantic decode.
    let mut bytes = 0_usize;
    for row in states {
        charge(&mut bytes, row.key.len(), max_total_bytes)?;
        charge(&mut bytes, row.value.len(), max_total_bytes)?;
    }
    for row in operations {
        charge(&mut bytes, row.operation_id.as_str().len(), max_total_bytes)?;
        charge(&mut bytes, row.value.len(), max_total_bytes)?;
    }
    let mut current = BTreeMap::new();
    let mut state_encoding_profiles = BTreeMap::new();
    for row in states {
        let (state, profile) =
            super::batch::decode_registry_state_entry_profiled(row.key, row.value)?;
        state_encoding_profiles.insert(state.key(), profile);
        check_counter(&state, registry_revision)?;
        if current.insert(state.key(), state).is_some() {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    check_counts(&current, limits)?;
    let mut history = BTreeMap::new();
    let mut identities = std::collections::BTreeSet::new();
    for row in operations {
        let operation = decode_batch_record(row.value, limits)?;
        if &operation.entry().operation_id != row.operation_id
            || !identities.insert(row.operation_id.clone())
        {
            return Err(RegistryError::OperationConflict);
        }
        let after = operation.registry_revision_after();
        if after > registry_revision || history.insert(after, operation).is_some() {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    let mut replayed = BTreeMap::new();
    let mut replayed_profiles = BTreeMap::new();
    let mut replay_counts = [0_usize; 4];
    let mut revision = 0_u64;
    for operation in history.values() {
        if operation.expected_registry_revision() != revision {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
        for delta in operation.deltas() {
            if replayed.get(delta.key()) != delta.before() {
                return Err(RegistryError::SnapshotInvalid);
            }
            check_counter(delta.after(), operation.registry_revision_after())?;
            check_transition(
                delta.before(),
                delta.after(),
                operation.registry_revision_after(),
            )?;
            if delta.before().is_none() {
                replay_counts[family_index(delta.key())] += 1;
            }
        }
        for delta in operation.deltas() {
            replayed.insert(delta.key().clone(), delta.after().clone());
            replayed_profiles.insert(delta.key().clone(), operation.state_encoding_profile());
        }
        check_count_limits(replay_counts, limits)?;
        revision = operation.registry_revision_after();
    }
    if revision != registry_revision {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    if replayed != current || replayed_profiles != state_encoding_profiles {
        return Err(RegistryError::SnapshotInvalid);
    }
    let mut roots = BTreeMap::new();
    let mut sources = BTreeMap::new();
    let mut memberships = BTreeMap::new();
    let mut generations = BTreeMap::new();
    let mut reverse = BTreeMap::new();
    for state in current.into_values() {
        match state {
            RegistryStateRecord::Root(root) => {
                roots.insert(root.root_binding_id, root);
            }
            RegistryStateRecord::Source(source) => {
                sources.insert(source.identity().clone(), source);
            }
            RegistryStateRecord::Membership(member) => {
                if reverse
                    .insert(member.membership_id(), member.key().clone())
                    .is_some()
                {
                    return Err(RegistryError::MembershipCollision);
                }
                memberships.insert(member.key().clone(), member);
            }
            RegistryStateRecord::Generation { corpus, generation } => {
                generations.insert(corpus, generation);
            }
        }
    }
    let mut root_identities = std::collections::BTreeSet::new();
    for root in roots.values() {
        if !root_identities.insert(root.canonical_root_digest) {
            return Err(RegistryError::RootIdentityConflict);
        }
    }
    for source in sources.values() {
        if !roots.contains_key(&source.binding().root_binding_id()) {
            return Err(RegistryError::RootNotRegistered);
        }
        if source.original_admission().is_none() {
            return Err(RegistryError::AdmissionRestorationUnavailable);
        }
    }
    let mut member_corpora = std::collections::BTreeSet::new();
    for member in memberships.values() {
        member_corpora.insert(member.key().corpus_id.clone());
        let source = sources
            .get(&member.key().source_identity)
            .ok_or(RegistryError::SourceNotFound)?;
        let generation = generations
            .get(&member.key().corpus_id)
            .ok_or(RegistryError::CutoverGenerationConflict)?;
        if member.generation() > *generation
            || (member.lifecycle() == MembershipLifecycle::Active
                && (member.generation() != *generation
                    || source.lifecycle() != SourceLifecycle::Active))
        {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    for corpus in generations.keys() {
        if !member_corpora.contains(corpus) {
            return Err(RegistryError::MembershipNotFound);
        }
    }
    // Supported operation bytes contain only these four state families. No
    // portfolio/namespace state can be silently reconstructed from receipts.
    // Any omitted receipt-only mutation creates a history/revision gap above.
    let registry = SourceRegistry {
        limits,
        revision: registry_revision,
        roots,
        sources,
        memberships,
        reverse_memberships: reverse,
        portfolios: BTreeMap::new(),
        portfolio_history: crate::portfolio_history::PortfolioHistory::new(),
        namespaces: BTreeMap::new(),
        active_generations: generations,
        operations: history
            .into_values()
            .map(RetainedRegistryOperation::Committed)
            .collect(),
        pending_batch: None,
        state_encoding_profiles,
    };
    Ok(ObservedRegistryRestore { registry })
}

fn charge(total: &mut usize, count: usize, maximum: usize) -> Result<(), RegistryError> {
    *total = total
        .checked_add(count)
        .ok_or(RegistryError::CapacityExceeded)?;
    if *total > maximum {
        return Err(RegistryError::CapacityExceeded);
    }
    Ok(())
}
fn check_counts(
    states: &BTreeMap<RegistryStateKey, RegistryStateRecord>,
    limits: RegistryLimits,
) -> Result<(), RegistryError> {
    let mut counts = [0_usize; 4];
    for key in states.keys() {
        counts[family_index(key)] += 1;
    }
    check_count_limits(counts, limits)
}
fn family_index(key: &RegistryStateKey) -> usize {
    match key {
        RegistryStateKey::Root(_) => 0,
        RegistryStateKey::Source(_) => 1,
        RegistryStateKey::Membership(_) => 2,
        RegistryStateKey::Generation(_) => 3,
    }
}
fn check_count_limits(counts: [usize; 4], limits: RegistryLimits) -> Result<(), RegistryError> {
    if counts[0] > limits.max_roots
        || counts[1] > limits.max_sources
        || counts[2] > limits.max_memberships
        || counts[3] > limits.max_memberships
    {
        return Err(RegistryError::CapacityExceeded);
    }
    Ok(())
}
fn check_counter(state: &RegistryStateRecord, maximum: u64) -> Result<(), RegistryError> {
    let valid = match state {
        RegistryStateRecord::Root(root) => {
            root.registry_revision > 0
                && root.registry_revision <= maximum
                && root.record_revision.get() <= root.registry_revision
        }
        RegistryStateRecord::Source(source) => {
            source.registry_revision() > 0
                && source.registry_revision() <= maximum
                && source.source_revision().get() <= source.registry_revision()
        }
        RegistryStateRecord::Membership(member) => {
            member.registry_revision() > 0
                && member.registry_revision() <= maximum
                && member.membership_revision().get()
                    <= member.registry_revision().saturating_mul(2)
        }
        RegistryStateRecord::Generation { .. } => true,
    };
    if !valid {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok(())
}
fn check_transition(
    before: Option<&RegistryStateRecord>,
    after: &RegistryStateRecord,
    revision: u64,
) -> Result<(), RegistryError> {
    if before == Some(after) {
        return Ok(());
    }
    let valid = match (before, after) {
        (None, RegistryStateRecord::Root(root)) => {
            root.record_revision.get() == 1
                && root.registry_revision == revision
                && root.status == RootStatus::Active
        }
        (None, RegistryStateRecord::Source(source)) => {
            source.source_revision().get() == 1
                && source.registry_revision() == revision
                && source.lifecycle() == SourceLifecycle::Active
        }
        (None, RegistryStateRecord::Membership(member)) => {
            member.membership_revision().get() == 1
                && member.registry_revision() == revision
                && member.lifecycle() == MembershipLifecycle::Active
        }
        (None, RegistryStateRecord::Generation { .. }) => true,
        (Some(RegistryStateRecord::Root(old)), RegistryStateRecord::Root(new)) => {
            old.root_binding_id == new.root_binding_id
                && old.canonical_root_digest == new.canonical_root_digest
                && old.owner_epoch == new.owner_epoch
                && old.status == RootStatus::Active
                && old.record_revision.checked_next().ok() == Some(new.record_revision)
                && new.registry_revision == revision
        }
        (Some(RegistryStateRecord::Source(old)), RegistryStateRecord::Source(new)) => {
            old.identity() == new.identity()
                && old.lifecycle() == SourceLifecycle::Active
                && old.source_revision().checked_next().ok() == Some(new.source_revision())
                && new.registry_revision() == revision
                && old.admission() == new.admission()
                && old.original_admission() == new.original_admission()
                && old.assignment() == new.assignment()
                && new.binding().binding_revision() >= old.binding().binding_revision()
                && new.binding().observation_revision() >= old.binding().observation_revision()
        }
        (Some(RegistryStateRecord::Membership(old)), RegistryStateRecord::Membership(new)) => {
            old.key() == new.key()
                && old.membership_id() == new.membership_id()
                && new.generation() >= old.generation()
                && new.membership_revision() > old.membership_revision()
                && new.membership_revision().get() - old.membership_revision().get() <= 2
                && new.registry_revision() == revision
        }
        (
            Some(RegistryStateRecord::Generation {
                corpus: old_id,
                generation: old,
            }),
            RegistryStateRecord::Generation {
                corpus: new_id,
                generation: new,
            },
        ) => old_id == new_id && new > old,
        _ => false,
    };
    if !valid {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok(())
}
