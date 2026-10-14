//! Finite technical registry storage. Source semantics stay with the caller.
mod codec;
mod lookup;
#[cfg(test)]
mod tests;
mod transaction;

use crate::{
    ControlCommitReceipt, ControlError, ControlKey, ControlRecordClass, ControlValue,
    JournalIdentity, JournalLimits,
};
use search_contracts::{Blake3Digest32, OpaqueId};
use std::fmt;

pub(super) const PREFIX: &[u8] = b"eliot.control.registry.v1/";
pub(super) const REVISION: &[u8] = b"eliot.control.registry.v1/revision";

/// Finite storage families, not source admission or policy authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RegistryStateFamily {
    /// Owner-encoded registered source metadata.
    Source,
    /// Owner-encoded membership metadata.
    Membership,
    /// Owner-encoded active corpus generation.
    Generation,
    /// Owner-encoded complete root metadata; no native custody or admission authority.
    Root,
}
impl RegistryStateFamily {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::Source => 1,
            Self::Membership => 2,
            Self::Generation => 3,
            Self::Root => 4,
        }
    }
    pub(super) const fn class(self) -> ControlRecordClass {
        match self {
            Self::Source | Self::Root => ControlRecordClass::Identity,
            Self::Membership => ControlRecordClass::State,
            Self::Generation => ControlRecordClass::Revision,
        }
    }
}

/// Exact owner-encoded key wrapped in a finite storage family.
/// Construction validates storage shape only; the source owner validates meaning.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct RegistryStateKey {
    family: RegistryStateFamily,
    bytes: Vec<u8>,
}
impl RegistryStateKey {
    /// Retain complete canonical owner key bytes without hashing or truncation.
    ///
    /// # Errors
    /// Empty keys and keys exceeding the active wrapped-key limit refuse.
    pub fn new(
        family: RegistryStateFamily,
        bytes: Vec<u8>,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        let key = Self { family, bytes };
        codec::state_key(&key, limits)?;
        Ok(key)
    }
    /// Storage family; not a decoded source-state assertion.
    #[must_use]
    pub const fn family(&self) -> RegistryStateFamily {
        self.family
    }
    /// Original complete owner bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Complete exact before/after metadata supplied by the source owner.
#[derive(Clone, Eq, PartialEq)]
pub struct RegistryStateChange {
    key: RegistryStateKey,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
}
impl RegistryStateChange {
    /// Prepare one bounded replacement. Absence is explicit; no deletion is inferred.
    ///
    /// # Errors
    /// Empty or oversized wrapped values refuse before any database dispatch.
    pub fn new(
        key: RegistryStateKey,
        before: Option<Vec<u8>>,
        after: Vec<u8>,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        codec::state_value(key.family, &after, limits)?;
        if let Some(value) = &before {
            codec::state_value(key.family, value, limits)?;
        }
        Ok(Self { key, before, after })
    }
    /// Exact owner key.
    #[must_use]
    pub const fn key(&self) -> &RegistryStateKey {
        &self.key
    }
    /// Exact original bytes, or required absence.
    #[must_use]
    pub fn before(&self) -> Option<&[u8]> {
        self.before.as_deref()
    }
    /// Exact replacement bytes.
    #[must_use]
    pub fn after(&self) -> &[u8] {
        &self.after
    }
}

/// Immutable technical operation envelope. Its payload is owner-encoded metadata,
/// never source bodies; decoding this wrapper grants no admission authority.
#[derive(Clone, Eq, PartialEq)]
pub struct RegistryOperationEnvelope {
    operation_id: OpaqueId,
    journal_generation: u64,
    registry_revision: u64,
    command_digest: Blake3Digest32,
    owner_bytes: Vec<u8>,
}
impl RegistryOperationEnvelope {
    /// Complete original operation identity; the storage hash is only an index.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }
    /// Original journal generation used in the immutable transaction fingerprint.
    #[must_use]
    pub const fn journal_generation_before(&self) -> u64 {
        self.journal_generation
    }
    /// Independent source-registry revision before this operation.
    #[must_use]
    pub const fn registry_revision_before(&self) -> u64 {
        self.registry_revision
    }
    /// Exact owner-computed command digest supplied by the semantic adapter.
    #[must_use]
    pub const fn command_digest(&self) -> Blake3Digest32 {
        self.command_digest
    }
    /// Complete canonical operation/delta metadata for the source owner to decode.
    #[must_use]
    pub fn owner_bytes(&self) -> &[u8] {
        &self.owner_bytes
    }
}

/// Bounded technical command. Root supplies already validated owner bytes.
/// This type does not evaluate admission, source policy or native custody.
#[derive(Clone, Eq, PartialEq)]
pub struct RegistryMutationRequest {
    control_id: crate::MutationId,
    envelope: RegistryOperationEnvelope,
    changes: Vec<RegistryStateChange>,
}
impl RegistryMutationRequest {
    /// Existing backend operation identity. This is only an index over the full
    /// raw ID retained in the envelope, never a new source/admission identity.
    #[must_use]
    pub const fn control_operation_id(&self) -> crate::MutationId {
        self.control_id
    }
    /// Prepare one atomic command without opening a database or raising limits.
    ///
    /// # Errors
    /// Duplicate keys, empty changes, overflow, or any active item/value bound refuse.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_id: OpaqueId,
        journal_generation: u64,
        registry_revision: u64,
        command_digest: Blake3Digest32,
        owner_bytes: Vec<u8>,
        mut changes: Vec<RegistryStateChange>,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        limits.validate()?;
        if changes.is_empty()
            || changes
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(4))
                .is_none_or(|n| n > limits.max_mutation_items)
        {
            return Err(ControlError::BudgetExceeded);
        }
        journal_generation
            .checked_add(1)
            .ok_or(ControlError::GenerationExhausted)?;
        registry_revision
            .checked_add(1)
            .ok_or(ControlError::GenerationExhausted)?;
        changes.sort_by(|a, b| a.key.cmp(&b.key));
        if changes.windows(2).any(|w| w[0].key == w[1].key) {
            return Err(ControlError::DuplicateMutationKey);
        }
        let value = Self {
            control_id: codec::mutation_id(&operation_id),
            envelope: RegistryOperationEnvelope {
                operation_id,
                journal_generation,
                registry_revision,
                command_digest,
                owner_bytes,
            },
            changes,
        };
        // Validate against the executing limits again when the command is dispatched.
        value.command(limits)?;
        Ok(value)
    }
    /// Original immutable technical operation, including recovery generation.
    #[must_use]
    pub const fn envelope(&self) -> &RegistryOperationEnvelope {
        &self.envelope
    }
    /// Every exact target comparison and replacement.
    #[must_use]
    pub fn changes(&self) -> &[RegistryStateChange] {
        &self.changes
    }
}

/// One complete owner value observed in a coherent registry read.
#[derive(Clone, Eq, PartialEq)]
pub struct RegistryStoredState {
    key: RegistryStateKey,
    bytes: Vec<u8>,
}
impl RegistryStoredState {
    /// Exact original owner key.
    #[must_use]
    pub const fn key(&self) -> &RegistryStateKey {
        &self.key
    }
    /// Exact owner value; semantic decoding/authorization remains required.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Coherent technical recovery observations, not permission to serve sources.
#[derive(Clone, Eq, PartialEq)]
pub struct RegistryReadSnapshot {
    identity: JournalIdentity,
    journal_generation: u64,
    registry_revision: u64,
    states: Vec<RegistryStoredState>,
    operations: Vec<RegistryOperationEnvelope>,
}
impl RegistryReadSnapshot {
    /// Actual verified journal/root/schema identity.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity {
        self.identity
    }
    /// Actual global journal generation, independent of the registry revision.
    #[must_use]
    pub const fn journal_generation(&self) -> u64 {
        self.journal_generation
    }
    /// Actual registry fence. Zero means a verified empty technical namespace.
    #[must_use]
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }
    /// All finite current owner values from the same read transaction.
    #[must_use]
    pub fn states(&self) -> &[RegistryStoredState] {
        &self.states
    }
    /// All immutable owner operation metadata from that same read.
    #[must_use]
    pub fn operations(&self) -> &[RegistryOperationEnvelope] {
        &self.operations
    }
    /// Look up the complete raw operation ID; a digest match alone never suffices.
    #[must_use]
    pub fn operation(&self, id: &OpaqueId) -> Option<&RegistryOperationEnvelope> {
        self.operations
            .iter()
            .find(|operation| &operation.operation_id == id)
    }
}

/// Verified durable commit and coherent current state. Historical replay may
/// return later current values; it never reinstalls the old operation's values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryCommitReadback {
    receipt: ControlCommitReceipt,
    readback: RegistryTransactionReadback,
}

/// Proven operation absence or immutable operation metadata from one bounded read.
/// Neither variant supplies source admission, native custody or serving authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryOperationLookup {
    /// Both operation row and backend receipt are absent at these exact coordinates.
    Absent {
        /// Actual verified journal identity.
        identity: JournalIdentity,
        /// Actual global journal generation of this read.
        journal_generation: u64,
        /// Actual independent registry revision of this read.
        registry_revision: u64,
    },
    /// Exact immutable envelope and its current coherent observation coordinates.
    Committed(RegistryOperationObservation),
}

/// Current observation coordinates, kept separate from immutable original inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryOperationObservation {
    identity: JournalIdentity,
    journal_generation: u64,
    registry_revision: u64,
    envelope: RegistryOperationEnvelope,
}
impl RegistryOperationObservation {
    /// Actual journal identity observed with this envelope.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity {
        self.identity
    }
    /// Current journal generation, not the original operation generation.
    #[must_use]
    pub const fn journal_generation(&self) -> u64 {
        self.journal_generation
    }
    /// Current independent registry revision.
    #[must_use]
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }
    /// Immutable original envelope; current coordinates do not rewrite it.
    #[must_use]
    pub const fn envelope(&self) -> &RegistryOperationEnvelope {
        &self.envelope
    }
}

/// One coherent operation and its requested current targets, not a full catalog.
#[derive(Clone, Eq, PartialEq)]
pub struct RegistryTransactionReadback {
    identity: JournalIdentity,
    journal_generation: u64,
    registry_revision: u64,
    operation: RegistryOperationEnvelope,
    states: Vec<RegistryStoredState>,
}
impl RegistryTransactionReadback {
    /// Actual journal identity of this coherent read.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity {
        self.identity
    }
    /// Current journal generation, independent of the registry revision.
    #[must_use]
    pub const fn journal_generation(&self) -> u64 {
        self.journal_generation
    }
    /// Current source-registry revision fence.
    #[must_use]
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }
    /// Exact immutable operation envelope, retaining its original generation.
    #[must_use]
    pub const fn operation(&self) -> &RegistryOperationEnvelope {
        &self.operation
    }
    /// Every target requested by this transaction, from the same read.
    #[must_use]
    pub fn states(&self) -> &[RegistryStoredState] {
        &self.states
    }
}
impl RegistryCommitReadback {
    pub(in crate::persistent) fn into_receipt(self) -> ControlCommitReceipt {
        self.receipt
    }
    /// Actual backend operation receipt.
    #[must_use]
    pub const fn receipt(&self) -> &ControlCommitReceipt {
        &self.receipt
    }
    /// Current coherent technical state, requiring source-owner semantic validation.
    #[must_use]
    pub const fn readback(&self) -> &RegistryTransactionReadback {
        &self.readback
    }
}

macro_rules! redacted {
    ($($name:ident),+) => { $(impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(concat!(stringify!($name), "(<opaque metadata>)"))
        }
    })+ };
}
redacted!(
    RegistryStateKey,
    RegistryStateChange,
    RegistryOperationEnvelope,
    RegistryMutationRequest,
    RegistryStoredState,
    RegistryReadSnapshot,
    RegistryTransactionReadback
);

pub(super) fn reserved(key: &[u8]) -> bool {
    key.starts_with(PREFIX)
}

pub(super) fn validate_record(
    version: u32,
    key: &ControlKey,
    value: &ControlValue,
    limits: JournalLimits,
) -> Result<(), ControlError> {
    if !reserved(key.as_bytes()) {
        return Ok(());
    }
    if version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
        return Err(ControlError::SchemaMismatch);
    }
    codec::decode_record(key, value, limits).map(|_| ())
}

pub(super) use transaction::validate_snapshot;
