//! Closed Source metadata state and immutable batch observations.
//!
//! Values reconstructed here carry no current admission, native access or mutation authority.

use crate::error::{JournalEntryKind, RegistryError, RegistryJournalEntry, RegistryLimits};
use crate::membership::{MembershipKey, MembershipRecord};
use crate::operation::{
    BatchItemOutcome, BatchItemStatus, RegistryBatch, RegistryBatchReceipt, RegistryOperation,
    RegistryReceipt,
};
use crate::root::RootRecord;
use crate::source::RegisteredSource;
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, SourceIdentity};
use search_source_admission::policy_v2::CheckpointFailureV2;
use std::collections::BTreeMap;

#[path = "batch_codec.rs"]
mod codec;
pub use codec::decode_batch_record;
pub use codec::{BatchCodecV4Error, OriginalInputResolver, decode_batch_record_v4};

/// Current closed immutable operation profile, without a currentness assertion.
pub const CURRENT_BATCH_PROFILE: &str = codec::CURRENT_BATCH_PROFILE;
/// Current closed metadata-state profile, without a currentness assertion.
pub const CURRENT_STATE_PROFILE: &str = codec::CURRENT_STATE_PROFILE;

/// Canonical fingerprint of complete immutable request metadata.
pub fn request_fingerprint(batch: &RegistryBatch) -> Result<Blake3Digest32, RegistryError> {
    codec::request_fingerprint(batch)
}
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
    /// Source namespace ownership key.
    Namespace(search_contracts::SourceNamespaceId),
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
    /// Namespace ownership and retained metadata.
    Namespace,
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
    /// Namespace ownership and retained permanent-v2 metadata.
    Namespace(crate::namespace::NamespaceCutoverState),
}
impl RegistryStateRecord {
    /// Stable owner bytes for one complete technical state value.
    pub fn encode(&self) -> Result<Vec<u8>, RegistryError> {
        if matches!(self, Self::Namespace(_)) {
            codec::encode_state_v4(self)
        } else {
            codec::encode_state(self)
        }
    }
    /// Encodes in the explicitly retained closed historical format.
    pub fn encode_with_profile(&self, profile: &str) -> Result<Vec<u8>, RegistryError> {
        if !matches!(
            profile,
            "eliot.registry-state.v2" | "eliot.registry-state.v3" | codec::STATE_PROFILE_V4
        ) {
            return Err(RegistryError::SnapshotInvalid);
        }
        if profile == codec::STATE_PROFILE_V4 {
            codec::encode_state_v4(self)
        } else {
            codec::encode_state_profile(self, profile)
        }
    }
    /// Exact family-qualified key.
    pub fn key(&self) -> RegistryStateKey {
        match self {
            Self::Root(root) => RegistryStateKey::Root(root.root_binding_id),
            Self::Source(v) => RegistryStateKey::Source(v.identity().clone()),
            Self::Membership(v) => RegistryStateKey::Membership(v.key().clone()),
            Self::Generation { corpus, .. } => RegistryStateKey::Generation(corpus.clone()),
            Self::Namespace(state) => {
                RegistryStateKey::Namespace(state.ownership.source_namespace_id)
            }
        }
    }
}
/// Decode complete target bytes, restoring admission through retained original inputs.
pub fn decode_registry_state(bytes: &[u8]) -> Result<RegistryStateRecord, RegistryError> {
    codec::decode_state(bytes)
}

/// Decode one v4 state value through the original-input owner and caller probe.
pub fn decode_registry_state_v4<R: OriginalInputResolver + ?Sized>(
    bytes: &[u8],
    resolver: &mut R,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<RegistryStateRecord, BatchCodecV4Error> {
    codec::decode_state_v4(bytes, resolver, probe)
}

impl RegistryStateKey {
    /// Closed family from an actual typed key, never from a journal tag.
    pub const fn family(&self) -> RegistryStateFamily {
        match self {
            Self::Root(_) => RegistryStateFamily::Root,
            Self::Source(_) => RegistryStateFamily::Source,
            Self::Membership(_) => RegistryStateFamily::Membership,
            Self::Generation(_) => RegistryStateFamily::Generation,
            Self::Namespace(_) => RegistryStateFamily::Namespace,
        }
    }
    /// Family-qualified canonical technical key; no string-built locator identity.
    pub fn encode(&self) -> Result<Vec<u8>, RegistryError> {
        if matches!(self, Self::Namespace(_)) {
            codec::encode_key_v4(self)
        } else {
            codec::encode_key(self)
        }
    }
}

/// Decode the complete canonical owner key, refusing unknown fields or families.
pub fn decode_registry_state_key(bytes: &[u8]) -> Result<RegistryStateKey, RegistryError> {
    codec::decode_key(bytes)
}

/// Decode the explicit v4 namespace key profile.
pub fn decode_registry_state_key_v4(bytes: &[u8]) -> Result<RegistryStateKey, RegistryError> {
    codec::decode_key_v4(bytes)
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

/// Restores exact key/value metadata and retains its closed wire profile.
pub fn decode_registry_state_entry_profiled(
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
    /// Constructs metadata only, requiring the complete before/after keys to agree.
    pub fn new(
        key: RegistryStateKey,
        before: Option<RegistryStateRecord>,
        after: RegistryStateRecord,
    ) -> Result<Self, RegistryError> {
        if after.key() != key || before.as_ref().is_some_and(|value| value.key() != key) {
            return Err(RegistryError::SnapshotInvalid);
        }
        Ok(Self { key, before, after })
    }
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
    /// Reconstitutes complete immutable metadata through the sole closed decoder.
    ///
    /// This validates representation and original provenance, not live state,
    /// current policy, native access or permission to commit a mutation.
    pub fn from_observed_parts(
        encoding_profile: &'static str,
        state_encoding_profile: &'static str,
        entry: RegistryJournalEntry,
        request_fingerprint: Blake3Digest32,
        change_count: usize,
        deltas: Vec<RegistryStateDelta>,
        limits: RegistryLimits,
    ) -> Result<Self, RegistryError> {
        if encoding_profile == codec::BATCH_PROFILE_V4
            || state_encoding_profile == codec::STATE_PROFILE_V4
        {
            return Err(RegistryError::SnapshotInvalid);
        }
        let observed = Self {
            encoding_profile,
            state_encoding_profile,
            entry,
            request_fingerprint,
            change_count,
            deltas,
        };
        let restored = codec::decode_batch_record(&codec::encode(&observed)?, limits)?;
        if restored != observed {
            return Err(RegistryError::SnapshotInvalid);
        }
        Ok(restored)
    }

    /// Constructs and exactly rehydrates one permanent-v2 batch through the
    /// original-input resolver and caller-owned checkpoint.
    pub fn from_observed_parts_v4<R: OriginalInputResolver + ?Sized>(
        entry: RegistryJournalEntry,
        request_fingerprint: Blake3Digest32,
        change_count: usize,
        deltas: Vec<RegistryStateDelta>,
        limits: RegistryLimits,
        resolver: &mut R,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Self, BatchCodecV4Error> {
        let observed = Self {
            encoding_profile: codec::BATCH_PROFILE_V4,
            state_encoding_profile: codec::STATE_PROFILE_V4,
            entry,
            request_fingerprint,
            change_count,
            deltas,
        };
        let encoded = codec::encode_batch_record_v4(&observed)?;
        let restored = codec::decode_batch_record_v4(&encoded, limits, resolver, probe)?;
        if restored != observed {
            return Err(RegistryError::SnapshotInvalid.into());
        }
        Ok(restored)
    }
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
        if self.encoding_profile == codec::BATCH_PROFILE_V4 {
            codec::encode_batch_record_v4(self)
        } else {
            codec::encode(self)
        }
    }
    /// Projects the retained operation receipt; the registry owns replay acceptance.
    pub fn receipt(&self, replayed: bool) -> RegistryBatchReceipt {
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
    /// Projects immutable receipt metadata without accepting a live mutation.
    pub fn registry_receipt(&self, replayed: bool) -> RegistryReceipt {
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
    /// Checks operation/revision coherence without establishing current authority.
    pub fn verify_operation(&self) -> Result<(), RegistryError> {
        if self.record.entry.kind != JournalEntryKind::BatchApply
            || self.record.entry.before_revision.checked_add(1)
                != Some(self.record.entry.after_revision)
            || self.revision < self.record.entry.after_revision
        {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
        Ok(())
    }
    /// Checks complete observed target equality at the retained operation revision.
    pub fn verify(&self) -> Result<(), RegistryError> {
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
