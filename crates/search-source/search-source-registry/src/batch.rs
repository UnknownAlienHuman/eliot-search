//! Bounded batch state transactions. Decoded records are observations, not admission permits.
use super::{
    BatchItemOutcome, BatchItemStatus, RegistryBatch, RegistryBatchReceipt, RegistryChange,
    RegistryLimits, RegistryOperation, RegistryReceipt, RetainedRegistryOperation, SourceRegistry,
    apply_change,
};
use crate::error::{JournalEntryKind, RegistryError, RegistryJournalEntry};
use crate::membership::{MembershipKey, MembershipRecord};
use crate::root::RootRecord;
use crate::source::RegisteredSource;
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity};
use std::collections::{BTreeMap, BTreeSet};

#[path = "batch_codec.rs"]
mod codec;
pub use codec::decode_batch_record;

/// Exact technical state key; families cannot collide.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RegistryStateKey {
    /// Exact stable root binding identity.
    Root(search_contracts::RootBindingId),
    /// Stable source key.
    Source(SourceIdentity),
    /// Source/corpus key.
    Membership(MembershipKey),
    /// Corpus generation key.
    Generation(OpaqueId),
}

/// Closed owner state family, independent of adapter storage prefixes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryStateFamily {
    /// Complete root metadata; no physical custody or live grant proof.
    Root,
    /// Complete registered source metadata.
    Source,
    /// Complete source/corpus membership metadata.
    Membership,
    /// Active corpus generation.
    Generation,
}

/// Complete metadata value of an affected target, never source content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryStateRecord {
    /// Complete retained root record.
    Root(RootRecord),
    /// Retained source state.
    Source(RegisteredSource),
    /// Retained membership state.
    Membership(MembershipRecord),
    /// Corpus generation state.
    Generation {
        /// Exact corpus identity.
        corpus: OpaqueId,
        /// Active generation.
        generation: NonZeroRevision,
    },
}
impl RegistryStateRecord {
    /// Stable owner bytes for one complete technical state value.
    pub fn encode(&self) -> Result<Vec<u8>, RegistryError> {
        codec::encode_state(self)
    }
    /// Exact family-qualified key.
    pub fn key(&self) -> RegistryStateKey {
        match self {
            Self::Root(root) => RegistryStateKey::Root(root.root_binding_id),
            Self::Source(v) => RegistryStateKey::Source(v.identity().clone()),
            Self::Membership(v) => RegistryStateKey::Membership(v.key().clone()),
            Self::Generation { corpus, .. } => RegistryStateKey::Generation(corpus.clone()),
        }
    }
}
/// Decode complete target bytes, restoring admission through retained original inputs.
pub fn decode_registry_state(bytes: &[u8]) -> Result<RegistryStateRecord, RegistryError> {
    codec::decode_state(bytes)
}

impl RegistryStateKey {
    /// Closed family from an actual typed key, never from a journal tag.
    pub const fn family(&self) -> RegistryStateFamily {
        match self {
            Self::Root(_) => RegistryStateFamily::Root,
            Self::Source(_) => RegistryStateFamily::Source,
            Self::Membership(_) => RegistryStateFamily::Membership,
            Self::Generation(_) => RegistryStateFamily::Generation,
        }
    }
    /// Family-qualified canonical technical key; no string-built locator identity.
    pub fn encode(&self) -> Result<Vec<u8>, RegistryError> {
        codec::encode_key(self)
    }
}

/// Decode the complete canonical owner key, refusing unknown fields or families.
pub fn decode_registry_state_key(bytes: &[u8]) -> Result<RegistryStateKey, RegistryError> {
    codec::decode_key(bytes)
}

/// Decode actual owner key/value bytes and require their exact identities to match.
///
/// Storage prefixes and journal tags alone do not validate an owner record.
pub fn decode_registry_state_entry(
    key_bytes: &[u8],
    record_bytes: &[u8],
) -> Result<RegistryStateRecord, RegistryError> {
    decode_registry_state_entry_profiled(key_bytes, record_bytes).map(|(record, _)| record)
}

pub(super) fn decode_registry_state_entry_profiled(
    key_bytes: &[u8],
    record_bytes: &[u8],
) -> Result<(RegistryStateRecord, &'static str), RegistryError> {
    let key = decode_registry_state_key(key_bytes)?;
    let (record, profile) = codec::decode_state_profiled(record_bytes)?;
    if key != record.key() {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok((record, profile))
}

/// Exact before/after values. Tombstones stay records; no inferred deletions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryStateDelta {
    key: RegistryStateKey,
    before: Option<RegistryStateRecord>,
    after: RegistryStateRecord,
}
impl RegistryStateDelta {
    /// Exact target identity.
    pub const fn key(&self) -> &RegistryStateKey {
        &self.key
    }
    /// Expected original value; None is an absence precondition.
    pub const fn before(&self) -> Option<&RegistryStateRecord> {
        self.before.as_ref()
    }
    /// Exact committed replacement.
    pub const fn after(&self) -> &RegistryStateRecord {
        &self.after
    }
}

/// Owner-decoded immutable operation/delta record. Alone it authorizes no mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedBatch {
    encoding_profile: &'static str,
    state_encoding_profile: &'static str,
    entry: RegistryJournalEntry,
    request_fingerprint: Blake3Digest32,
    change_count: usize,
    deltas: Vec<RegistryStateDelta>,
}
impl DecodedBatch {
    /// Exact closed wire profile retained by decoding; never a currentness grant.
    pub const fn encoding_profile(&self) -> &'static str {
        self.encoding_profile
    }

    /// Exact state format written by this immutable operation's closed profile.
    pub const fn state_encoding_profile(&self) -> &'static str {
        self.state_encoding_profile
    }

    /// Expected source-registry owner revision, independent of journal generation.
    pub const fn expected_registry_revision(&self) -> u64 {
        self.entry.before_revision
    }
    /// Source-registry owner revision after this transaction.
    pub const fn registry_revision_after(&self) -> u64 {
        self.entry.after_revision
    }
    /// Original immutable operation receipt, including expected revision.
    pub const fn entry(&self) -> &RegistryJournalEntry {
        &self.entry
    }
    /// Actual owner-computed canonical input fingerprint, not the caller digest.
    pub const fn request_fingerprint(&self) -> Blake3Digest32 {
        self.request_fingerprint
    }
    /// Every affected or required source/membership/generation target.
    pub fn deltas(&self) -> &[RegistryStateDelta] {
        &self.deltas
    }
    /// Number of original business changes, distinct from affected target count.
    pub const fn change_count(&self) -> usize {
        self.change_count
    }
    /// Stable canonical record for the operation row.
    pub fn encode(&self) -> Result<Vec<u8>, RegistryError> {
        codec::encode(self)
    }
    pub(super) fn receipt(&self, replayed: bool) -> RegistryBatchReceipt {
        RegistryBatchReceipt {
            operation: RegistryOperation::new(
                self.entry.operation_id.clone(),
                self.entry.mutation_digest,
            ),
            before_revision: self.entry.before_revision,
            after_revision: self.entry.after_revision,
            items: (0..self.change_count)
                .map(|input_index| BatchItemOutcome {
                    input_index,
                    status: BatchItemStatus::Committed,
                    reason: None,
                })
                .collect(),
            receipt: self.entry.receipt.clone(),
            replayed,
        }
    }
    pub(super) fn registry_receipt(&self, replayed: bool) -> RegistryReceipt {
        RegistryReceipt {
            operation: RegistryOperation::new(
                self.entry.operation_id.clone(),
                self.entry.mutation_digest,
            ),
            before_revision: self.entry.before_revision,
            after_revision: self.entry.after_revision,
            change_count: self.change_count,
            mutation_digest: self.entry.mutation_digest,
            replayed,
        }
    }
}

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

/// One coherent owner read of an operation and all its current targets.
/// Historical operation verification is separate from current-target recovery.
#[derive(Clone, Debug)]
pub struct RegistryBatchReadback {
    record: DecodedBatch,
    revision: u64,
    states: Vec<RegistryStateRecord>,
}
/// Closed operation lookup. Absence is tied to a coherent current owner revision.
#[derive(Clone, Debug)]
pub enum RegistryBatchLookup {
    /// Operation absence was verified in the same coherent owner read.
    Absent {
        /// Current authoritative store revision.
        revision: u64,
    },
    /// Immutable committed operation and bounded target observations.
    Committed(RegistryBatchReadback),
}
impl RegistryBatchReadback {
    /// Constructs bounded read observations; exact comparison happens in the registry.
    pub fn new(
        record: DecodedBatch,
        revision: u64,
        states: Vec<RegistryStateRecord>,
        limits: RegistryLimits,
    ) -> Result<Self, RegistryError> {
        if states.len() > limits.max_batch_changes {
            return Err(RegistryError::CapacityExceeded);
        }
        Ok(Self {
            record,
            revision,
            states,
        })
    }
    /// Immutable decoded operation/delta record.
    pub const fn record(&self) -> &DecodedBatch {
        &self.record
    }
    /// Coherent registry owner revision of every observed target, not journal generation.
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// Full observed target values, not counts or digest assertions.
    pub fn states(&self) -> &[RegistryStateRecord] {
        &self.states
    }
    pub(super) fn verify_operation(&self) -> Result<(), RegistryError> {
        if self.record.entry.kind != JournalEntryKind::BatchApply
            || self.record.entry.before_revision.checked_add(1)
                != Some(self.record.entry.after_revision)
            || self.revision < self.record.entry.after_revision
        {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
        Ok(())
    }
    pub(super) fn verify(&self) -> Result<(), RegistryError> {
        self.verify_operation()?;
        if self.revision != self.record.entry.after_revision
            || self.states.len() != self.record.deltas.len()
        {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
        let mut states = BTreeMap::new();
        for state in &self.states {
            if states.insert(state.key(), state).is_some() {
                return Err(RegistryError::SnapshotInvalid);
            }
        }
        for delta in &self.record.deltas {
            if states.get(&delta.key).copied() != Some(&delta.after) {
                return Err(RegistryError::MutationOutcomeUnknown);
            }
        }
        Ok(())
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
    prepare_profiled(
        registry,
        batch,
        receipt,
        codec::CURRENT_BATCH_PROFILE,
        codec::CURRENT_STATE_PROFILE,
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
        &observed.entry.receipt,
        observed.encoding_profile,
        observed.state_encoding_profile,
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
            None => {}
        }
    }
    let before_sources = sources.len();
    let before_roots = roots.len();
    let before_memberships = memberships.len();
    for change in &batch.changes {
        apply_change(
            change,
            after_revision,
            registry.limits,
            &mut roots,
            &mut sources,
            &mut memberships,
            &mut generations,
        )?;
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
            RegistryStateKey::Generation(k) => {
                generations.get(k).map(|v| RegistryStateRecord::Generation {
                    corpus: k.clone(),
                    generation: *v,
                })
            }
        }
        .ok_or(RegistryError::SnapshotInvalid)?;
        deltas.push(RegistryStateDelta {
            before: state(registry, &key),
            key,
            after,
        });
    }
    let record = DecodedBatch {
        encoding_profile,
        state_encoding_profile,
        entry: RegistryJournalEntry::new(
            batch.operation.operation_id().clone(),
            batch.operation.mutation_digest(),
            registry.revision,
            after_revision,
            JournalEntryKind::BatchApply,
            receipt.clone(),
        ),
        request_fingerprint: codec::request_fingerprint(batch)?,
        change_count: batch.changes.len(),
        deltas,
    };
    let encoded = codec::encode(&record)?;
    let states = record
        .deltas
        .iter()
        .map(|delta| {
            Ok(PreparedStateDelta {
                key: delta.key.clone(),
                key_bytes: delta.key.encode()?,
                before_bytes: delta
                    .before
                    .as_ref()
                    .map(|before| encode_existing_state(registry, before))
                    .transpose()?,
                after_bytes: codec::encode_state_profile(
                    &delta.after,
                    record.state_encoding_profile,
                )?,
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
    codec::encode_state_profile(state, profile)
}

pub(super) fn validate_install(
    registry: &SourceRegistry,
    record: &DecodedBatch,
) -> Result<(), RegistryError> {
    let existing = registry
        .operations
        .iter()
        .find(|operation| operation.id() == &record.entry.operation_id);
    if existing.is_some_and(|operation| operation.digest() != record.entry.mutation_digest) {
        return Err(RegistryError::OperationConflict);
    }
    if existing.is_none() && registry.operations.len() >= registry.limits.max_operations {
        return Err(RegistryError::OperationCapacityExceeded);
    }
    if registry.revision == record.entry.after_revision {
        if record
            .deltas
            .iter()
            .any(|d| state(registry, &d.key).as_ref() != Some(&d.after))
        {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
    } else if registry.revision != record.entry.before_revision
        || record
            .deltas
            .iter()
            .any(|d| state(registry, &d.key).as_ref() != d.before.as_ref())
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    let new_sources = record
        .deltas
        .iter()
        .filter(|d| {
            matches!(d.key, RegistryStateKey::Source(_)) && state(registry, &d.key).is_none()
        })
        .count();
    let new_roots = record
        .deltas
        .iter()
        .filter(|delta| {
            matches!(delta.key, RegistryStateKey::Root(_)) && state(registry, &delta.key).is_none()
        })
        .count();
    let new_memberships = record
        .deltas
        .iter()
        .filter(|d| {
            matches!(d.key, RegistryStateKey::Membership(_)) && state(registry, &d.key).is_none()
        })
        .count();
    if registry.roots.len().saturating_add(new_roots) > registry.limits.max_roots
        || registry.sources.len().saturating_add(new_sources) > registry.limits.max_sources
        || registry.memberships.len().saturating_add(new_memberships)
            > registry.limits.max_memberships
    {
        return Err(RegistryError::CapacityExceeded);
    }
    let mut staged_memberships = BTreeMap::new();
    for delta in &record.deltas {
        if let RegistryStateRecord::Membership(member) = &delta.after
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
    if registry.revision != record.entry.after_revision {
        for delta in &record.deltas {
            registry
                .state_encoding_profiles
                .insert(delta.key.clone(), record.state_encoding_profile);
            match &delta.after {
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
            }
        }
        registry.revision = record.entry.after_revision;
    }
    if !registry
        .operations
        .iter()
        .any(|operation| operation.id() == &record.entry.operation_id)
    {
        registry
            .operations
            .push(RetainedRegistryOperation::Committed(record.clone()));
    }
    registry.pending_batch = None;
}

pub(super) fn request_fingerprint(batch: &RegistryBatch) -> Result<Blake3Digest32, RegistryError> {
    codec::request_fingerprint(batch)
}
