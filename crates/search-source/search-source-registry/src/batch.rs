//! Current SourceRegistry preparation and installation over immutable record metadata.
use super::{
    RegistryBatch, RegistryChange, RegistryLimits, RetainedRegistryOperation, SourceRegistry,
    apply_change,
};
use crate::error::{JournalEntryKind, RegistryError, RegistryJournalEntry};
use crate::membership::MembershipKey;
use crate::source::RegisteredSource;
use search_contracts::{Blake3Digest32, ReceiptRef};
use search_source_records::batch as codec;
#[cfg(test)]
use search_source_records::batch::decode_registry_state_entry_profiled;
pub use search_source_records::batch::{
    DecodedBatch, RegistryBatchLookup, RegistryBatchReadback, RegistryStateDelta,
    RegistryStateFamily, RegistryStateKey, RegistryStateRecord, decode_batch_record,
    decode_registry_state, decode_registry_state_entry, decode_registry_state_key,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "batch_wire_profile_tests.rs"]
mod wire_profile_tests;
/// Prepared bounded transaction, minted only by current owner semantics.
///
/// A storage adapter must atomically compare before values and revision,
/// replace every after value, advance the revision and retain this operation.
#[derive(Clone, Debug)]
pub struct PreparedBatch {
    record: DecodedBatch,
    encoded: Vec<u8>,
    states: Vec<PreparedStateDelta>,
}
impl PreparedBatch {
    /// Expected registry revision; an adapter must separately guard journal generation.
    pub const fn expected_registry_revision(&self) -> u64 {
        self.record.expected_registry_revision()
    }
    /// Exact operation and expected registry revision.
    pub const fn record(&self) -> &DecodedBatch {
        &self.record
    }
    /// Complete canonical operation/delta bytes.
    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }
    /// Pre-encoded complete targets in the same exact order as `record().deltas()`.
    pub fn states(&self) -> &[PreparedStateDelta] {
        &self.states
    }
}

/// Owner-prepared canonical state key and complete before/after bytes.
///
/// Adapters consume these bytes without repeating source owner computations.
#[derive(Clone, Debug)]
pub struct PreparedStateDelta {
    key: RegistryStateKey,
    key_bytes: Vec<u8>,
    before_bytes: Option<Vec<u8>>,
    after_bytes: Vec<u8>,
}
impl PreparedStateDelta {
    /// Exact typed owner key used to classify the reserved storage family.
    pub const fn key(&self) -> &RegistryStateKey {
        &self.key
    }
    /// Complete canonical owner key bytes.
    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }
    /// Complete expected canonical value; None requires actual verified absence.
    pub fn before_bytes(&self) -> Option<&[u8]> {
        self.before_bytes.as_deref()
    }
    /// Complete canonical replacement value.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after_bytes
    }
}

fn state(registry: &SourceRegistry, key: &RegistryStateKey) -> Option<RegistryStateRecord> {
    match key {
        RegistryStateKey::Root(root) => registry
            .roots
            .get(root)
            .cloned()
            .map(RegistryStateRecord::Root),
        RegistryStateKey::Source(k) => registry
            .sources
            .get(k)
            .cloned()
            .map(RegistryStateRecord::Source),
        RegistryStateKey::Membership(k) => registry
            .memberships
            .get(k)
            .cloned()
            .map(RegistryStateRecord::Membership),
        RegistryStateKey::Namespace(namespace) => registry
            .namespaces
            .get(namespace)
            .cloned()
            .map(RegistryStateRecord::Namespace),
        RegistryStateKey::Generation(k) => {
            registry
                .active_generations
                .get(k)
                .map(|v| RegistryStateRecord::Generation {
                    corpus: k.clone(),
                    generation: *v,
                })
        }
    }
}
fn include(
    keys: &mut BTreeSet<RegistryStateKey>,
    key: RegistryStateKey,
    limits: RegistryLimits,
) -> Result<(), RegistryError> {
    keys.insert(key);
    if keys.len() > limits.max_batch_changes {
        return Err(RegistryError::CapacityExceeded);
    }
    Ok(())
}

/// Prepares only the bounded target closure, including implicit retirement/cutover effects.
pub(super) fn prepare(
    registry: &SourceRegistry,
    batch: &RegistryBatch,
    receipt: &ReceiptRef,
) -> Result<PreparedBatch, RegistryError> {
    let permanent_membership = batch.changes.iter().any(|change| {
        matches!(
            change,
            RegistryChange::BindPermanentMembershipV2 { .. }
                | RegistryChange::TransitionPermanentMembershipV2 { .. }
        )
    });
    prepare_profiled(
        registry,
        batch,
        receipt,
        if permanent_membership {
            "eliot.registry-batch.v5"
        } else {
            codec::CURRENT_BATCH_PROFILE
        },
        if permanent_membership {
            "eliot.registry-state.v5"
        } else {
            codec::CURRENT_STATE_PROFILE
        },
    )
}

/// Reproduces a decoded historical transaction in its own closed wire formats.
/// The caller still compares the entire reproduced record before installation.
pub(super) fn prepare_observed(
    registry: &SourceRegistry,
    batch: &RegistryBatch,
    observed: &DecodedBatch,
) -> Result<PreparedBatch, RegistryError> {
    prepare_profiled(
        registry,
        batch,
        &observed.entry().receipt,
        observed.encoding_profile(),
        observed.state_encoding_profile(),
    )
}

fn prepare_profiled(
    registry: &SourceRegistry,
    batch: &RegistryBatch,
    receipt: &ReceiptRef,
    encoding_profile: &'static str,
    state_encoding_profile: &'static str,
) -> Result<PreparedBatch, RegistryError> {
    batch.validate(registry.limits)?;
    if registry.revision != batch.expected_registry_revision {
        return Err(RegistryError::RegistryRevisionConflict);
    }
    if registry.operations.len() >= registry.limits.max_operations {
        return Err(RegistryError::OperationCapacityExceeded);
    }
    let after_revision = registry
        .revision
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let mut keys = BTreeSet::new();
    for change in &batch.changes {
        if let Some(root) = change.root_target() {
            include(&mut keys, RegistryStateKey::Root(root), registry.limits)?;
        }
        if let RegistryChange::RegisterRoot { request, .. } = change
            && registry.roots.values().any(|root| {
                root.root_binding_id != request.root_binding_id
                    && root.canonical_root_digest == request.canonical_root_digest
            })
        {
            return Err(RegistryError::RootIdentityConflict);
        }
        if let Some(source) = change.source_target() {
            include(
                &mut keys,
                RegistryStateKey::Source(source.clone()),
                registry.limits,
            )?;
        }
        if let Some(member) = change.membership_target() {
            include(
                &mut keys,
                RegistryStateKey::Membership(member.clone()),
                registry.limits,
            )?;
            include(
                &mut keys,
                RegistryStateKey::Source(member.source_identity.clone()),
                registry.limits,
            )?;
        }
        match change {
            RegistryChange::BindPermanentMembershipV2 {
                member,
                source_identity,
                ..
            } => {
                include(
                    &mut keys,
                    RegistryStateKey::Membership(super::membership_v2::state_key(
                        member,
                        source_identity.clone(),
                    )?),
                    registry.limits,
                )?;
            }
            RegistryChange::TransitionPermanentMembershipV2 { membership_id, .. } => {
                include(
                    &mut keys,
                    RegistryStateKey::Membership(super::membership_v2::key_for_id(
                        &registry.reverse_memberships,
                        *membership_id,
                    )?),
                    registry.limits,
                )?;
            }
            _ => {}
        }
        match change {
            RegistryChange::RetireSource { identity, .. } => {
                for key in registry
                    .memberships
                    .keys()
                    .filter(|k| &k.source_identity == identity)
                {
                    include(
                        &mut keys,
                        RegistryStateKey::Membership(key.clone()),
                        registry.limits,
                    )?;
                }
            }
            RegistryChange::AddMembership(v) => {
                include(
                    &mut keys,
                    RegistryStateKey::Generation(v.key.corpus_id.clone()),
                    registry.limits,
                )?;
            }
            RegistryChange::CutoverNamespace(v) => {
                include(
                    &mut keys,
                    RegistryStateKey::Generation(v.corpus_id.clone()),
                    registry.limits,
                )?;
                for key in registry
                    .memberships
                    .keys()
                    .filter(|k| k.corpus_id == v.corpus_id)
                {
                    include(
                        &mut keys,
                        RegistryStateKey::Membership(key.clone()),
                        registry.limits,
                    )?;
                }
                for identity in &v.frozen_inventory {
                    include(
                        &mut keys,
                        RegistryStateKey::Source(identity.clone()),
                        registry.limits,
                    )?;
                    include(
                        &mut keys,
                        RegistryStateKey::Membership(MembershipKey {
                            corpus_id: v.corpus_id.clone(),
                            source_identity: identity.clone(),
                        }),
                        registry.limits,
                    )?;
                }
            }
            _ => {}
        }
    }
    let mut sources = BTreeMap::new();
    let mut roots = BTreeMap::new();
    let mut original_bytes = 0_usize;
    for key in &keys {
        if let RegistryStateKey::Source(identity) = key
            && let Some(original) = registry
                .sources
                .get(identity)
                .and_then(RegisteredSource::original_admission)
        {
            original_bytes = original_bytes
                .checked_add(original.encoded().len())
                .ok_or(RegistryError::CapacityExceeded)?;
            if original_bytes > search_contracts::MAX_CANONICAL_BYTES {
                return Err(RegistryError::CapacityExceeded);
            }
        }
    }
    let mut memberships = BTreeMap::new();
    let mut generations = BTreeMap::new();
    let mut namespaces = BTreeMap::new();
    for key in &keys {
        match state(registry, key) {
            Some(RegistryStateRecord::Root(root)) => {
                roots.insert(root.root_binding_id, root);
            }
            Some(RegistryStateRecord::Source(v)) => {
                sources.insert(v.identity().clone(), v);
            }
            Some(RegistryStateRecord::Membership(v)) => {
                memberships.insert(v.key().clone(), v);
            }
            Some(RegistryStateRecord::Generation { corpus, generation }) => {
                generations.insert(corpus, generation);
            }
            Some(RegistryStateRecord::Namespace(namespace)) => {
                namespaces.insert(namespace.ownership.source_namespace_id, namespace);
            }
            None => {}
        }
    }
    let before_sources = sources.len();
    let before_roots = roots.len();
    let before_memberships = memberships.len();
    for change in &batch.changes {
        match change {
            RegistryChange::BindPermanentMembershipV2 {
                member,
                source_namespace_id,
                root_binding_id,
                owner,
                source_identity,
                expected_source_revision,
                expected_root_record_revision,
                expected_namespace_record_revision,
                expected_owner_generation,
                receipt,
            } => {
                let (key, record) = super::membership_v2::stage_bind(
                    registry,
                    member,
                    *source_namespace_id,
                    *root_binding_id,
                    owner,
                    source_identity,
                    *expected_source_revision,
                    *expected_root_record_revision,
                    *expected_namespace_record_revision,
                    *expected_owner_generation,
                    after_revision,
                    receipt,
                )?;
                memberships.insert(key, record);
            }
            RegistryChange::TransitionPermanentMembershipV2 {
                membership_id,
                expected_membership_revision,
                command,
                receipt,
            } => {
                let key = super::membership_v2::key_for_id(
                    &registry.reverse_memberships,
                    *membership_id,
                )?;
                let current = memberships
                    .get(&key)
                    .ok_or(RegistryError::MembershipNotFound)?;
                let updated = super::membership_v2::stage_transition(
                    current,
                    *expected_membership_revision,
                    *command,
                    after_revision,
                    receipt,
                )?;
                memberships.insert(key, updated);
            }
            _ => apply_change(
                change,
                after_revision,
                registry.limits,
                &mut roots,
                &mut sources,
                &mut memberships,
                &mut generations,
            )?,
        }
    }
    if registry.roots.len() - before_roots + roots.len() > registry.limits.max_roots
        || registry.sources.len() - before_sources + sources.len() > registry.limits.max_sources
        || registry.memberships.len() - before_memberships + memberships.len()
            > registry.limits.max_memberships
    {
        return Err(RegistryError::CapacityExceeded);
    }
    let mut deltas = Vec::with_capacity(keys.len());
    for key in keys {
        let after = match &key {
            RegistryStateKey::Root(root) => roots.get(root).cloned().map(RegistryStateRecord::Root),
            RegistryStateKey::Source(k) => sources.get(k).cloned().map(RegistryStateRecord::Source),
            RegistryStateKey::Membership(k) => memberships
                .get(k)
                .cloned()
                .map(RegistryStateRecord::Membership),
            RegistryStateKey::Namespace(key) => namespaces
                .get(key)
                .cloned()
                .map(RegistryStateRecord::Namespace),
            RegistryStateKey::Generation(k) => {
                generations.get(k).map(|v| RegistryStateRecord::Generation {
                    corpus: k.clone(),
                    generation: *v,
                })
            }
        }
        .ok_or(RegistryError::SnapshotInvalid)?;
        deltas.push(RegistryStateDelta::new(
            key.clone(),
            state(registry, &key),
            after,
        )?);
    }
    let entry = RegistryJournalEntry::new(
        batch.operation.operation_id().clone(),
        batch.operation.mutation_digest(),
        registry.revision,
        after_revision,
        JournalEntryKind::BatchApply,
        receipt.clone(),
    );
    let fingerprint = codec::request_fingerprint(batch)?;
    let record = if encoding_profile == "eliot.registry-batch.v5" {
        DecodedBatch::from_observed_parts_v5(
            entry,
            fingerprint,
            batch.changes.len(),
            deltas,
            batch.changes.clone(),
            registry.limits,
            &mut || Ok(()),
        )
        .map_err(|error| match error {
            codec::BatchCodecV4Error::Registry(error) => error,
            codec::BatchCodecV4Error::Admission(_) => {
                RegistryError::AdmissionRestorationUnavailable
            }
        })?
    } else {
        DecodedBatch::from_observed_parts(
            encoding_profile,
            state_encoding_profile,
            entry,
            fingerprint,
            batch.changes.len(),
            deltas,
            registry.limits,
        )?
    };
    let encoded = record.encode()?;
    let states = record
        .deltas()
        .iter()
        .map(|delta| {
            Ok(PreparedStateDelta {
                key: delta.key().clone(),
                key_bytes: delta.key().encode()?,
                before_bytes: delta
                    .before()
                    .map(|before| encode_existing_state(registry, before))
                    .transpose()?,
                after_bytes: delta
                    .after()
                    .encode_with_profile(record.state_encoding_profile())?,
            })
        })
        .collect::<Result<Vec<_>, RegistryError>>()?;
    Ok(PreparedBatch {
        record,
        encoded,
        states,
    })
}

fn encode_existing_state(
    registry: &SourceRegistry,
    state: &RegistryStateRecord,
) -> Result<Vec<u8>, RegistryError> {
    let profile = registry
        .state_encoding_profiles
        .get(&state.key())
        .ok_or(RegistryError::AdmissionRestorationUnavailable)?;
    state.encode_with_profile(profile)
}

// Only semantic producer modules inside this owner may mint a prepared v4 mutation.
// A DecodedBatch from public metadata APIs is never a mutation permit by itself.
pub(super) fn prepare_v4_record(
    registry: &SourceRegistry,
    record: DecodedBatch,
) -> Result<PreparedBatch, RegistryError> {
    if record.encoding_profile() != "eliot.registry-batch.v4"
        || record.state_encoding_profile() != "eliot.registry-state.v4"
        || record.expected_registry_revision() != registry.revision
    {
        return Err(RegistryError::RegistryRevisionConflict);
    }
    validate_install(registry, &record)?;
    let states = record
        .deltas()
        .iter()
        .map(|delta| {
            Ok(PreparedStateDelta {
                key: delta.key().clone(),
                key_bytes: delta.key().encode()?,
                before_bytes: delta
                    .before()
                    .map(|value| encode_existing_state(registry, value))
                    .transpose()?,
                after_bytes: delta
                    .after()
                    .encode_with_profile(record.state_encoding_profile())?,
            })
        })
        .collect::<Result<Vec<_>, RegistryError>>()?;
    Ok(PreparedBatch {
        encoded: record.encode()?,
        record,
        states,
    })
}

pub(super) fn validate_install(
    registry: &SourceRegistry,
    record: &DecodedBatch,
) -> Result<(), RegistryError> {
    let existing = registry.operations.get(&record.entry().operation_id);
    if existing.is_some_and(|operation| operation.digest() != record.entry().mutation_digest) {
        return Err(RegistryError::OperationConflict);
    }
    if existing.is_none() && registry.operations.len() >= registry.limits.max_operations {
        return Err(RegistryError::OperationCapacityExceeded);
    }
    if registry.revision == record.entry().after_revision {
        if record
            .deltas()
            .iter()
            .any(|d| state(registry, d.key()).as_ref() != Some(d.after()))
        {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
    } else if registry.revision != record.entry().before_revision
        || record
            .deltas()
            .iter()
            .any(|d| state(registry, d.key()).as_ref() != d.before())
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    let new_sources = record
        .deltas()
        .iter()
        .filter(|d| {
            matches!(d.key(), RegistryStateKey::Source(_)) && state(registry, d.key()).is_none()
        })
        .count();
    let new_roots = record
        .deltas()
        .iter()
        .filter(|delta| {
            matches!(delta.key(), RegistryStateKey::Root(_))
                && state(registry, delta.key()).is_none()
        })
        .count();
    let new_memberships = record
        .deltas()
        .iter()
        .filter(|d| {
            matches!(d.key(), RegistryStateKey::Membership(_)) && state(registry, d.key()).is_none()
        })
        .count();
    let new_namespaces = record
        .deltas()
        .iter()
        .filter(|delta| {
            matches!(delta.key(), RegistryStateKey::Namespace(_))
                && state(registry, delta.key()).is_none()
        })
        .count();
    if registry.roots.len().saturating_add(new_roots) > registry.limits.max_roots
        || registry.sources.len().saturating_add(new_sources) > registry.limits.max_sources
        || registry.memberships.len().saturating_add(new_memberships)
            > registry.limits.max_memberships
        || registry.namespaces.len().saturating_add(new_namespaces) > registry.limits.max_namespaces
    {
        return Err(RegistryError::CapacityExceeded);
    }
    let mut staged_memberships = BTreeMap::new();
    for delta in record.deltas() {
        if let RegistryStateRecord::Membership(member) = delta.after()
            && (registry
                .reverse_memberships
                .get(&member.membership_id())
                .is_some_and(|key| key != member.key())
                || staged_memberships
                    .insert(member.membership_id(), member.key())
                    .is_some_and(|key| key != member.key()))
        {
            return Err(RegistryError::MembershipCollision);
        }
    }
    Ok(())
}

pub(super) fn install(
    registry: &mut SourceRegistry,
    record: &DecodedBatch,
) -> Result<(), RegistryError> {
    validate_install(registry, record)?;
    install_validated(registry, record);
    Ok(())
}

// The public producer has validated this exact prepared record before dispatch.
// Its successful readback must match before this infallible memory installation.
pub(super) fn install_prepared(registry: &mut SourceRegistry, prepared: &PreparedBatch) {
    install_validated(registry, prepared.record());
}

fn install_validated(registry: &mut SourceRegistry, record: &DecodedBatch) {
    if registry.revision != record.entry().after_revision {
        for delta in record.deltas() {
            registry
                .state_encoding_profiles
                .insert(delta.key().clone(), record.state_encoding_profile());
            match delta.after() {
                RegistryStateRecord::Root(root) => {
                    registry.roots.insert(root.root_binding_id, root.clone());
                }
                RegistryStateRecord::Source(v) => {
                    registry.sources.insert(v.identity().clone(), v.clone());
                }
                RegistryStateRecord::Membership(v) => {
                    registry
                        .reverse_memberships
                        .insert(v.membership_id(), v.key().clone());
                    registry.memberships.insert(v.key().clone(), v.clone());
                }
                RegistryStateRecord::Generation { corpus, generation } => {
                    registry
                        .active_generations
                        .insert(corpus.clone(), *generation);
                }
                RegistryStateRecord::Namespace(namespace) => {
                    registry
                        .namespaces
                        .insert(namespace.ownership.source_namespace_id, namespace.clone());
                }
            }
        }
        registry.revision = record.entry().after_revision;
    }
    registry
        .operations
        .entry(record.entry().operation_id.clone())
        .or_insert_with(|| RetainedRegistryOperation::Committed(record.clone()));
    registry.identity_cache_v2 = None;
    registry.pending_batch = None;
}

pub(super) fn request_fingerprint(batch: &RegistryBatch) -> Result<Blake3Digest32, RegistryError> {
    codec::request_fingerprint(batch)
}
