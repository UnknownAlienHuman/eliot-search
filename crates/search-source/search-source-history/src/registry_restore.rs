//! Bounded observed-state restoration, without native, live-policy or serving authority.
//!
//! This module owns the pure data side of registry restoration: decoding,
//! coherence validation, replay verification and structured history parts.
//! The I/O owner must establish read coherence/completeness and revalidate live
//! root custody, owner/security fences, policy and grants before service use.

use search_contracts::{CanonicalKey, CanonicalValue, OpaqueId};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_records::batch::{
    BatchCodecV4Error, DecodedBatch, OriginalInputResolver, RegistryStateKey, RegistryStateRecord,
    decode_batch_record, decode_batch_record_v4, decode_registry_state_entry_profiled,
    decode_batch_record_with_original_probe, decode_registry_state_entry_profiled_with_original_probe,
    decode_registry_state_key_v4, decode_registry_state_v4,
};
use search_source_records::error::{RegistryError, RegistryLimits};
use search_source_records::membership::MembershipLifecycle;
use search_source_records::namespace::NamespaceCutoverState;
use search_source_records::root::RootStatus;
use search_source_records::source::SourceLifecycle;
use std::collections::BTreeMap;

const STATE_PROFILE_V4: &str = "eliot.registry-state.v4";
const BATCH_PROFILE_V4: &str = "eliot.registry-batch.v4";

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

/// Plain public history data produced by bounded restore validation.
///
/// These values carry no current admission, native access or mutation authority.
/// The I/O owner must establish read coherence/completeness and revalidate live
/// root custody, owner/security fences, policy and grants before service use.
#[derive(Clone, Debug)]
pub struct RegistryHistoryParts {
    /// Finite limits used during validation.
    pub limits: RegistryLimits,
    /// Registry revision that must match the contiguous operation chain.
    pub revision: u64,
    /// Observed root records by binding identity.
    pub roots: BTreeMap<search_contracts::RootBindingId, search_source_records::root::RootRecord>,
    /// Observed source records by stable identity.
    pub sources:
        BTreeMap<search_contracts::SourceIdentity, search_source_records::source::RegisteredSource>,
    /// Observed membership records by corpus/source key.
    pub memberships: BTreeMap<
        search_source_records::membership::MembershipKey,
        search_source_records::membership::MembershipRecord,
    >,
    /// Reverse index from membership identity to key.
    pub reverse_memberships: BTreeMap<
        search_contracts::SourceMembershipId,
        search_source_records::membership::MembershipKey,
    >,
    /// Active corpus generations.
    pub active_generations: BTreeMap<OpaqueId, search_contracts::NonZeroRevision>,
    /// Observed namespace-owner records on the same semantic registry owner.
    pub namespaces: BTreeMap<search_contracts::SourceNamespaceId, NamespaceCutoverState>,
    /// Decoded immutable batch history in revision order.
    pub operations: Vec<DecodedBatch>,
    /// Closed state encoding profile for each observed state key.
    pub state_encoding_profiles: BTreeMap<RegistryStateKey, &'static str>,
}

/// Opaque validated history token; no public constructor.
///
/// Obtain via [`restore_registry_state`]. Consume with [`ObservedRegistryHistory::into_parts`] to
/// extract the validated data for owner composition. The token grants no
/// service authority and provides no disk durability proof.
#[derive(Clone, Debug)]
pub struct ObservedRegistryHistory {
    parts: RegistryHistoryParts,
}

impl ObservedRegistryHistory {
    /// Consumes the opaque token, transferring validated history parts.
    pub fn into_parts(self) -> RegistryHistoryParts {
        self.parts
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
    preflight_registry_restore_inputs_with_namespaces(
        limits,
        state_count,
        operation_count,
        total_encoded_bytes,
        max_total_bytes,
        false,
    )
}

fn preflight_registry_restore_inputs_with_namespaces(
    limits: RegistryLimits,
    state_count: usize,
    operation_count: usize,
    total_encoded_bytes: usize,
    max_total_bytes: usize,
    namespaces_allowed: bool,
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
        .and_then(|count| {
            if namespaces_allowed {
                count.checked_add(limits.max_namespaces)
            } else {
                Some(count)
            }
        })
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

trait RestoreDecoder {
    type Error: From<RegistryError>;

    fn decode_state(
        &mut self,
        key: &[u8],
        value: &[u8],
    ) -> Result<(RegistryStateRecord, &'static str), Self::Error>;

    fn decode_operation(
        &mut self,
        value: &[u8],
        limits: RegistryLimits,
    ) -> Result<DecodedBatch, Self::Error>;

    fn namespaces_allowed(&self) -> bool;
}

struct LegacyRestoreDecoder;

impl RestoreDecoder for LegacyRestoreDecoder {
    type Error = RegistryError;

    fn decode_state(
        &mut self,
        key: &[u8],
        value: &[u8],
    ) -> Result<(RegistryStateRecord, &'static str), Self::Error> {
        decode_registry_state_entry_profiled(key, value)
    }

    fn decode_operation(
        &mut self,
        value: &[u8],
        limits: RegistryLimits,
    ) -> Result<DecodedBatch, Self::Error> {
        decode_batch_record(value, limits)
    }

    fn namespaces_allowed(&self) -> bool {
        false
    }
}

struct V4RestoreDecoder<'a, R: ?Sized, P> {
    resolver: &'a mut R,
    probe: &'a mut P,
}

impl<R, P> RestoreDecoder for V4RestoreDecoder<'_, R, P>
where
    R: OriginalInputResolver + ?Sized,
    P: FnMut() -> Result<(), CheckpointFailureV2>,
{
    type Error = BatchCodecV4Error;

    fn decode_state(
        &mut self,
        key: &[u8],
        value: &[u8],
    ) -> Result<(RegistryStateRecord, &'static str), Self::Error> {
        if encoded_profile(value)?.as_deref() == Some(STATE_PROFILE_V4) {
            let key = decode_registry_state_key_v4(key).map_err(BatchCodecV4Error::Registry)?;
            let state = decode_registry_state_v4(value, self.resolver, self.probe)?;
            if state.key() != key {
                return Err(BatchCodecV4Error::Registry(RegistryError::SnapshotInvalid));
            }
            Ok((state, STATE_PROFILE_V4))
        } else {
            decode_registry_state_entry_profiled_with_original_probe(key, value, self.probe)
        }
    }

    fn decode_operation(
        &mut self,
        value: &[u8],
        limits: RegistryLimits,
    ) -> Result<DecodedBatch, Self::Error> {
        if encoded_profile(value)?.as_deref() == Some(BATCH_PROFILE_V4) {
            decode_batch_record_v4(value, limits, self.resolver, self.probe)
        } else {
            decode_batch_record_with_original_probe(value, limits, self.probe)
        }
    }

    fn namespaces_allowed(&self) -> bool {
        true
    }
}

fn encoded_profile(bytes: &[u8]) -> Result<Option<String>, RegistryError> {
    let value = search_contracts::parse_canonical_cbor(bytes)
        .map_err(|_| RegistryError::SnapshotInvalid)?;
    let CanonicalValue::Object(fields) = value else {
        return Err(RegistryError::SnapshotInvalid);
    };
    let profile_key = CanonicalKey::new("profile").map_err(|_| RegistryError::SnapshotInvalid)?;
    match fields.get(&profile_key) {
        Some(CanonicalValue::Text(profile)) => Ok(Some(profile.as_str().to_owned())),
        _ => Err(RegistryError::SnapshotInvalid),
    }
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
) -> Result<ObservedRegistryHistory, RegistryError> {
    let mut decoder = LegacyRestoreDecoder;
    restore_registry_state_with(
        registry_revision,
        states,
        operations,
        limits,
        max_total_bytes,
        &mut decoder,
    )
}

/// Restore complete state/history through the permanent-v2 decoder and the
/// actual original-input owner.
///
/// The resolver reads only from the caller's already coherent metadata view;
/// this function performs no storage, native access, current-policy or grant
/// work. The exact caller probe is forwarded through every decode and evaluation
/// step, including legacy members. Old state and operation profiles remain byte-preserving
/// members of a mixed history, while the legacy public factory still refuses
/// v4 rows because it has no resolver or probe.
pub fn restore_registry_state_v4<R, P>(
    registry_revision: u64,
    states: &[RegistryRestoreState<'_>],
    operations: &[RegistryRestoreOperation<'_>],
    limits: RegistryLimits,
    max_total_bytes: usize,
    resolver: &mut R,
    original_probe: &mut P,
) -> Result<ObservedRegistryHistory, BatchCodecV4Error>
where
    R: OriginalInputResolver + ?Sized,
    P: FnMut() -> Result<(), CheckpointFailureV2>,
{
    let mut decoder = V4RestoreDecoder {
        resolver,
        probe: original_probe,
    };
    restore_registry_state_with(
        registry_revision,
        states,
        operations,
        limits,
        max_total_bytes,
        &mut decoder,
    )
}

fn restore_registry_state_with<D>(
    registry_revision: u64,
    states: &[RegistryRestoreState<'_>],
    operations: &[RegistryRestoreOperation<'_>],
    limits: RegistryLimits,
    max_total_bytes: usize,
    decoder: &mut D,
) -> Result<ObservedRegistryHistory, D::Error>
where
    D: RestoreDecoder,
{
    preflight_registry_restore_inputs_with_namespaces(
        limits,
        states.len(),
        operations.len(),
        0,
        max_total_bytes,
        decoder.namespaces_allowed(),
    )?;
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
        let (state, profile) = decoder.decode_state(row.key, row.value)?;
        state_encoding_profiles.insert(state.key(), profile);
        check_counter(&state, registry_revision)?;
        if current.insert(state.key(), state).is_some() {
            return Err(RegistryError::SnapshotInvalid.into());
        }
    }
    check_counts(&current, limits)?;
    let mut history = BTreeMap::new();
    let mut identities = std::collections::BTreeSet::new();
    for row in operations {
        let operation = decoder.decode_operation(row.value, limits)?;
        if &operation.entry().operation_id != row.operation_id
            || !identities.insert(row.operation_id.clone())
        {
            return Err(RegistryError::OperationConflict.into());
        }
        let after = operation.registry_revision_after();
        if after > registry_revision || history.insert(after, operation).is_some() {
            return Err(RegistryError::SnapshotInvalid.into());
        }
    }
    let mut replayed = BTreeMap::new();
    let mut replayed_profiles = BTreeMap::new();
    let mut replay_counts = [0_usize; 5];
    let mut revision = 0_u64;
    for operation in history.values() {
        if operation.expected_registry_revision() != revision {
            return Err(RegistryError::MutationOutcomeUnknown.into());
        }
        for delta in operation.deltas() {
            if replayed.get(delta.key()) != delta.before() {
                return Err(RegistryError::SnapshotInvalid.into());
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
        return Err(RegistryError::MutationOutcomeUnknown.into());
    }
    if replayed != current || replayed_profiles != state_encoding_profiles {
        return Err(RegistryError::SnapshotInvalid.into());
    }
    let mut roots = BTreeMap::new();
    let mut sources = BTreeMap::new();
    let mut memberships = BTreeMap::new();
    let mut namespaces = BTreeMap::new();
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
                    return Err(RegistryError::MembershipCollision.into());
                }
                memberships.insert(member.key().clone(), member);
            }
            RegistryStateRecord::Generation { corpus, generation } => {
                generations.insert(corpus, generation);
            }
            RegistryStateRecord::Namespace(namespace) => {
                if !decoder.namespaces_allowed() {
                    // This legacy entry point has no original-input resolver or probe.
                    return Err(RegistryError::AdmissionRestorationUnavailable.into());
                }
                namespaces.insert(namespace.ownership.source_namespace_id, namespace);
            }
        }
    }
    let mut root_identities = std::collections::BTreeSet::new();
    for root in roots.values() {
        if !root_identities.insert(root.canonical_root_digest) {
            return Err(RegistryError::RootIdentityConflict.into());
        }
    }
    for source in sources.values() {
        if !roots.contains_key(&source.binding().root_binding_id()) {
            return Err(RegistryError::RootNotRegistered.into());
        }
        if !source.has_original_admission() {
            return Err(RegistryError::AdmissionRestorationUnavailable.into());
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
            return Err(RegistryError::SnapshotInvalid.into());
        }
    }
    for corpus in generations.keys() {
        if !member_corpora.contains(corpus) {
            return Err(RegistryError::MembershipNotFound.into());
        }
    }
    validate_namespace_relationships(&roots, &sources, &namespaces, registry_revision)?;
    Ok(ObservedRegistryHistory {
        parts: RegistryHistoryParts {
            limits,
            revision: registry_revision,
            roots,
            sources,
            memberships,
            reverse_memberships: reverse,
            active_generations: generations,
            namespaces,
            operations: history.into_values().collect(),
            state_encoding_profiles,
        },
    })
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
    let mut counts = [0_usize; 5];
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
        RegistryStateKey::Namespace(_) => 4,
    }
}
fn check_count_limits(counts: [usize; 5], limits: RegistryLimits) -> Result<(), RegistryError> {
    if counts[0] > limits.max_roots
        || counts[1] > limits.max_sources
        || counts[2] > limits.max_memberships
        || counts[3] > limits.max_memberships
        || counts[4] > limits.max_namespaces
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
        RegistryStateRecord::Namespace(namespace) => {
            namespace.permanent_metadata().is_some_and(|metadata| {
                metadata.registry_revision() > 0
                    && metadata.registry_revision() <= maximum
                    && namespace.ownership.ownership_record_revision.get()
                        <= metadata.registry_revision()
            })
        }
    };
    if !valid {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok(())
}

fn validate_namespace_relationships(
    roots: &BTreeMap<search_contracts::RootBindingId, search_source_records::root::RootRecord>,
    sources: &BTreeMap<
        search_contracts::SourceIdentity,
        search_source_records::source::RegisteredSource,
    >,
    namespaces: &BTreeMap<search_contracts::SourceNamespaceId, NamespaceCutoverState>,
    registry_revision: u64,
) -> Result<(), RegistryError> {
    for (namespace_id, state) in namespaces {
        if state.ownership.source_namespace_id != *namespace_id {
            return Err(RegistryError::SnapshotInvalid);
        }
        let metadata = state
            .permanent_metadata()
            .ok_or(RegistryError::SnapshotInvalid)?;
        if metadata.registry_revision() > registry_revision
            || state.ownership.ownership_record_revision.get() > metadata.registry_revision()
            || state.ownership.source_admission_policy_revision.get()
                != metadata.policy().compiled().revision().get()
        {
            return Err(RegistryError::SnapshotInvalid);
        }
        if !roots.contains_key(&metadata.root_binding_id()) {
            return Err(RegistryError::RootNotRegistered);
        }
    }

    for source in sources.values() {
        let Some(permanent) = source.permanent_admission() else {
            continue;
        };
        let namespace = namespaces
            .get(&source.identity().source_namespace_id)
            .ok_or(RegistryError::NamespaceMismatch)?;
        let metadata = namespace
            .permanent_metadata()
            .ok_or(RegistryError::NamespaceMismatch)?;
        let root = roots
            .get(&source.binding().root_binding_id())
            .ok_or(RegistryError::RootNotRegistered)?;
        if metadata.root_binding_id() != root.root_binding_id
            || permanent.policy_reference() != metadata.policy_reference()
            || permanent.root_record_revision().get() > root.record_revision.get()
            || permanent.namespace_record_revision().get()
                > namespace.ownership.ownership_record_revision.get()
            || permanent.owner_generation() != namespace.ownership.source_owner_generation
        {
            return Err(RegistryError::SnapshotInvalid);
        }
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
        (None, RegistryStateRecord::Namespace(namespace)) => {
            namespace.ownership.status == search_contracts::NamespaceOwnershipStatus::Active
                && namespace.permanent_metadata().is_some()
                && namespace.preparation.is_none()
                && !namespace.fenced
                && namespace.activation.is_none()
                && namespace
                    .permanent_metadata()
                    .is_some_and(|metadata| metadata.registry_revision() == revision)
        }
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
                && old.provenance() == new.provenance()
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
