//! Bounded transactional control journal and immutable snapshot publication.
//!
//! This package owns content-free control state only. Source bodies, query text,
//! excerpts, postings, embeddings, vectors, and ordinary query history have no
//! representable record class here. The in-memory journal is also the reference
//! model for a concrete durable `redb` adapter.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use search_contracts::{
    Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch, ReceiptRef,
};

/// Closed failure surface for control-journal operations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlError {
    /// The journal is unavailable to the current owner.
    StoreUnavailable,
    /// Persistent or reconstructed state is malformed.
    StoreCorrupt,
    /// Normal access is blocked by an explicit quarantine.
    StoreQuarantined,
    /// Root, installation, path, schema, or owner identity differs.
    IdentityMismatch,
    /// The schema family or version is unsupported.
    SchemaUnsupported,
    /// The observed schema differs from the expected schema.
    SchemaMismatch,
    /// A migration exists but has not been independently verified.
    MigrationUnverified,
    /// The expected journal generation is stale.
    TransactionConflict,
    /// An entity generation guard does not match current state.
    GenerationMismatch,
    /// One operation identity was reused with another command digest.
    OperationConflict,
    /// A possible commit requires authoritative readback.
    CommitOutcomeUnknown,
    /// A side-effect-free read was cancelled.
    ReadCancelled,
    /// A finite item, byte, record, or operation ceiling was exceeded.
    BudgetExceeded,
    /// A coherent immutable snapshot could not be reconstructed.
    SnapshotRebuildFailed,
    /// An in-memory snapshot could not be published after durable commit.
    SnapshotPublicationFailed,
    /// A key or value attempted to encode forbidden search content.
    ForbiddenControlPayload,
    /// A key is empty or exceeds the configured ceiling.
    InvalidKey,
    /// A value is empty or exceeds the configured ceiling.
    InvalidValue,
    /// One mutation contains the same key more than once.
    DuplicateMutationKey,
    /// The journal generation cannot advance.
    GenerationExhausted,
    /// The operation ledger is full and cannot preserve recovery semantics.
    IdempotencyCapacityExceeded,
}

impl ControlError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::StoreUnavailable => "CONTROL_STORE_UNAVAILABLE",
            Self::StoreCorrupt => "CONTROL_STORE_CORRUPT",
            Self::StoreQuarantined => "CONTROL_STORE_QUARANTINED",
            Self::IdentityMismatch => "CONTROL_IDENTITY_MISMATCH",
            Self::SchemaUnsupported => "CONTROL_SCHEMA_UNSUPPORTED",
            Self::SchemaMismatch => "CONTROL_SCHEMA_MISMATCH",
            Self::MigrationUnverified => "CONTROL_MIGRATION_UNVERIFIED",
            Self::TransactionConflict => "CONTROL_TRANSACTION_CONFLICT",
            Self::GenerationMismatch => "CONTROL_GENERATION_MISMATCH",
            Self::OperationConflict => "CONTROL_OPERATION_CONFLICT",
            Self::CommitOutcomeUnknown => "CONTROL_COMMIT_OUTCOME_UNKNOWN",
            Self::ReadCancelled => "CONTROL_READ_CANCELLED",
            Self::BudgetExceeded => "CONTROL_BUDGET_EXCEEDED",
            Self::SnapshotRebuildFailed => "SNAPSHOT_REBUILD_FAILED",
            Self::SnapshotPublicationFailed => "SNAPSHOT_PUBLICATION_FAILED",
            Self::ForbiddenControlPayload => "FORBIDDEN_CONTROL_PAYLOAD",
            Self::InvalidKey => "CONTROL_INVALID_KEY",
            Self::InvalidValue => "CONTROL_INVALID_VALUE",
            Self::DuplicateMutationKey => "CONTROL_DUPLICATE_MUTATION_KEY",
            Self::GenerationExhausted => "CONTROL_GENERATION_EXHAUSTED",
            Self::IdempotencyCapacityExceeded => "CONTROL_IDEMPOTENCY_CAPACITY_EXCEEDED",
        }
    }
}

impl fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ControlError {}

/// Closed semantic class for a control record.
///
/// The absence of source/query/vector variants is an API-level storage guard.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlRecordClass {
    /// Installation, root, owner, source, or membership identity metadata.
    Identity,
    /// Monotone generation, route, revision, epoch, or cursor metadata.
    Revision,
    /// Closed lifecycle, readiness, admission, or security state.
    State,
    /// Immutable content-free receipt or evidence reference.
    Receipt,
    /// Idempotency and unresolved-operation metadata.
    Operation,
    /// Immutable snapshot or manifest reference.
    Snapshot,
    /// Schema and migration metadata.
    Migration,
}

/// Exact journal identity bound to one installation, root, owner epoch, path,
/// schema family, and schema version.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct JournalIdentity {
    /// Installation incarnation that owns the journal.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Canonical data-root identity.
    pub data_root_id: DataRootId,
    /// Current runtime-owner epoch.
    pub owner_epoch: OwnerEpoch,
    /// Digest of the exact canonical local journal path identity.
    pub path_identity_digest: Blake3Digest32,
    /// Digest of the schema family and table contract.
    pub schema_family_digest: Blake3Digest32,
    /// Non-zero schema version.
    pub schema_version: u32,
}

impl JournalIdentity {
    /// Validates a journal identity.
    pub fn validate(self) -> Result<Self, ControlError> {
        if self.schema_version == 0 {
            return Err(ControlError::SchemaUnsupported);
        }
        Ok(self)
    }
}

/// Finite journal resource limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalLimits {
    /// Maximum key length in bytes.
    pub max_key_bytes: usize,
    /// Maximum single value length in bytes.
    pub max_value_bytes: usize,
    /// Maximum number of live records.
    pub max_records: usize,
    /// Maximum sum of live value bytes.
    pub max_total_value_bytes: usize,
    /// Maximum writes plus deletes in one transaction.
    pub max_mutation_items: usize,
    /// Maximum retained operation receipts.
    pub max_operation_records: usize,
}

impl JournalLimits {
    /// A conservative local baseline.
    pub const BASELINE: Self = Self {
        max_key_bytes: 4_096,
        max_value_bytes: 64 * 1_024,
        max_records: 65_536,
        max_total_value_bytes: 64 * 1_024 * 1_024,
        max_mutation_items: 4_096,
        max_operation_records: 65_536,
    };

    /// Validates that every dimension is finite and non-zero.
    pub const fn validate(self) -> Result<Self, ControlError> {
        if self.max_key_bytes == 0
            || self.max_value_bytes == 0
            || self.max_records == 0
            || self.max_total_value_bytes == 0
            || self.max_mutation_items == 0
            || self.max_operation_records == 0
        {
            return Err(ControlError::BudgetExceeded);
        }
        Ok(self)
    }
}

/// Finite opaque key. Debug output exposes only its byte length.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ControlKey(Vec<u8>);

impl ControlKey {
    /// Validates and owns a key.
    pub fn new(bytes: Vec<u8>, limits: JournalLimits) -> Result<Self, ControlError> {
        if bytes.is_empty() || bytes.len() > limits.max_key_bytes {
            return Err(ControlError::InvalidKey);
        }
        Ok(Self(bytes))
    }

    /// Exact key bytes for a concrete backend.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for ControlKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ControlKey")
            .field(&format_args!("<{} bytes>", self.0.len()))
            .finish()
    }
}

/// Finite canonical control value. Debug output never includes its bytes.
#[derive(Clone, Eq, PartialEq)]
pub struct ControlValue {
    class: ControlRecordClass,
    bytes: Vec<u8>,
}

impl ControlValue {
    /// Validates and owns a control value.
    pub fn new(
        class: ControlRecordClass,
        bytes: Vec<u8>,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        if bytes.is_empty() || bytes.len() > limits.max_value_bytes {
            return Err(ControlError::InvalidValue);
        }
        Ok(Self { class, bytes })
    }

    /// Closed semantic record class.
    #[must_use]
    pub const fn class(&self) -> ControlRecordClass {
        self.class
    }

    /// Exact canonical bytes for a concrete backend.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Encoded byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the value is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl fmt::Debug for ControlValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlValue")
            .field("class", &self.class)
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .finish()
    }
}

/// Fixed-size immutable operation identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MutationId(pub [u8; 32]);

/// One key/value replacement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlWrite {
    /// Exact key.
    pub key: ControlKey,
    /// Exact canonical value.
    pub value: ControlValue,
}

/// One guarded finite mutation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlMutation {
    id: MutationId,
    command_digest: Blake3Digest32,
    expected_generation: u64,
    writes: Vec<ControlWrite>,
    deletes: Vec<ControlKey>,
}

impl ControlMutation {
    /// Creates a mutation. Structural and capacity validation occurs before the
    /// journal stages any state.
    #[must_use]
    pub fn new(
        id: MutationId,
        command_digest: Blake3Digest32,
        expected_generation: u64,
        writes: Vec<ControlWrite>,
        deletes: Vec<ControlKey>,
    ) -> Self {
        Self {
            id,
            command_digest,
            expected_generation,
            writes,
            deletes,
        }
    }

    /// Immutable operation identity.
    #[must_use]
    pub const fn id(&self) -> MutationId {
        self.id
    }

    /// Digest of exact canonical command bytes.
    #[must_use]
    pub const fn command_digest(&self) -> Blake3Digest32 {
        self.command_digest
    }

    /// Expected journal generation.
    #[must_use]
    pub const fn expected_generation(&self) -> u64 {
        self.expected_generation
    }

    /// Finite writes.
    #[must_use]
    pub fn writes(&self) -> &[ControlWrite] {
        &self.writes
    }

    /// Finite deletes.
    #[must_use]
    pub fn deletes(&self) -> &[ControlKey] {
        &self.deletes
    }
}

/// Content-free exact transaction receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCommitReceipt {
    /// Immutable operation identity.
    pub operation_id: MutationId,
    /// Digest of exact canonical command bytes.
    pub command_digest: Blake3Digest32,
    /// Journal generation before the transaction.
    pub before_generation: u64,
    /// Journal generation after the transaction.
    pub after_generation: u64,
    /// Deterministically ordered changed keys.
    pub changed_keys: Vec<ControlKey>,
    /// Whether the receipt came from idempotency readback.
    pub replayed: bool,
}

/// Side-effect-free coherent read snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalReadSnapshot {
    /// Exact journal identity.
    pub identity: JournalIdentity,
    /// Exact committed generation.
    pub generation: u64,
    /// Deterministically ordered technical records.
    pub records: Vec<(ControlKey, ControlValue)>,
}

impl JournalReadSnapshot {
    /// Looks up one exact key without mutating journal state.
    #[must_use]
    pub fn get(&self, key: &ControlKey) -> Option<&ControlValue> {
        self.records
            .binary_search_by(|(candidate, _)| candidate.cmp(key))
            .ok()
            .map(|index| &self.records[index].1)
    }
}

/// Immutable bounded control snapshot used by request admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlSnapshot {
    /// Exact journal identity.
    pub identity: JournalIdentity,
    /// Exact committed generation represented by this snapshot.
    pub generation: u64,
    /// Deterministically ordered technical records.
    pub records: Vec<(ControlKey, ControlValue)>,
}

/// Exact recovery classification for a transaction with an unknown outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommitRecoveryDecision {
    /// Exact command was committed.
    Committed(ControlCommitReceipt),
    /// No record exists; retry is permitted only with the same operation.
    NotCommittedRetrySameOperation,
    /// Operation identity exists with another command digest.
    ConflictingInput,
    /// State is quarantined or internally contradictory.
    PartialOrCorruptQuarantine,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OperationRecord {
    command_digest: Blake3Digest32,
    receipt: ControlCommitReceipt,
}

/// Finite deterministic reference journal.
///
/// A concrete `redb` backend must preserve the same atomicity, idempotency, and
/// readback semantics. This model deliberately performs no filesystem I/O.
#[derive(Clone, Debug)]
pub struct ControlJournal {
    identity: JournalIdentity,
    generation: u64,
    limits: JournalLimits,
    records: BTreeMap<ControlKey, ControlValue>,
    operations: BTreeMap<MutationId, OperationRecord>,
    quarantine: Option<ControlError>,
}

impl ControlJournal {
    /// Creates an empty journal for one exact identity.
    pub fn open_or_create(
        identity: JournalIdentity,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        Ok(Self {
            identity: identity.validate()?,
            generation: 0,
            limits: limits.validate()?,
            records: BTreeMap::new(),
            operations: BTreeMap::new(),
            quarantine: None,
        })
    }

    /// Exact identity.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity {
        self.identity
    }

    /// Exact committed generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether the journal is quarantined.
    #[must_use]
    pub const fn is_quarantined(&self) -> bool {
        self.quarantine.is_some()
    }

    /// Reads one coherent snapshot without creating an operation row or write.
    pub fn read_snapshot(&self) -> Result<JournalReadSnapshot, ControlError> {
        self.ensure_available()?;
        Ok(JournalReadSnapshot {
            identity: self.identity,
            generation: self.generation,
            records: self
                .records
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        })
    }

    /// Executes one atomic guarded transaction.
    ///
    /// The journal stages a complete clone, validates every bound, then swaps
    /// the staged map into place. On every error, committed state is unchanged.
    pub fn transact(
        &mut self,
        mutation: ControlMutation,
    ) -> Result<ControlCommitReceipt, ControlError> {
        self.ensure_available()?;

        if let Some(existing) = self.operations.get(&mutation.id) {
            if existing.command_digest != mutation.command_digest {
                return Err(ControlError::OperationConflict);
            }
            let mut receipt = existing.receipt.clone();
            receipt.replayed = true;
            return Ok(receipt);
        }

        if mutation.expected_generation != self.generation {
            return Err(ControlError::TransactionConflict);
        }
        if self.operations.len() >= self.limits.max_operation_records {
            return Err(ControlError::IdempotencyCapacityExceeded);
        }

        let item_count = mutation
            .writes
            .len()
            .checked_add(mutation.deletes.len())
            .ok_or(ControlError::BudgetExceeded)?;
        if item_count == 0 || item_count > self.limits.max_mutation_items {
            return Err(ControlError::BudgetExceeded);
        }

        let mut touched = BTreeSet::new();
        for write in &mutation.writes {
            self.validate_key_value(&write.key, &write.value)?;
            if !touched.insert(write.key.clone()) {
                return Err(ControlError::DuplicateMutationKey);
            }
        }
        for key in &mutation.deletes {
            self.validate_key(key)?;
            if !touched.insert(key.clone()) {
                return Err(ControlError::DuplicateMutationKey);
            }
        }

        let mut staged = self.records.clone();
        for key in &mutation.deletes {
            staged.remove(key);
        }
        for write in &mutation.writes {
            staged.insert(write.key.clone(), write.value.clone());
        }
        self.validate_staged_records(&staged)?;

        let after_generation = self
            .generation
            .checked_add(1)
            .ok_or(ControlError::GenerationExhausted)?;
        let receipt = ControlCommitReceipt {
            operation_id: mutation.id,
            command_digest: mutation.command_digest,
            before_generation: self.generation,
            after_generation,
            changed_keys: touched.into_iter().collect(),
            replayed: false,
        };

        self.records = staged;
        self.generation = after_generation;
        self.operations.insert(
            mutation.id,
            OperationRecord {
                command_digest: mutation.command_digest,
                receipt: receipt.clone(),
            },
        );
        Ok(receipt)
    }

    /// Resolves an unknown transaction by exact idempotency readback.
    #[must_use]
    pub fn recover_transaction(
        &self,
        operation_id: MutationId,
        expected_command_digest: Blake3Digest32,
    ) -> CommitRecoveryDecision {
        if self.quarantine.is_some() {
            return CommitRecoveryDecision::PartialOrCorruptQuarantine;
        }
        match self.operations.get(&operation_id) {
            Some(record) if record.command_digest == expected_command_digest => {
                let mut receipt = record.receipt.clone();
                receipt.replayed = true;
                CommitRecoveryDecision::Committed(receipt)
            }
            Some(_) => CommitRecoveryDecision::ConflictingInput,
            None => CommitRecoveryDecision::NotCommittedRetrySameOperation,
        }
    }

    /// Deletes old idempotency rows while preserving explicitly protected
    /// unresolved operations.
    pub fn prune_idempotency(
        &mut self,
        retain_from_generation: u64,
        protected_operations: &BTreeSet<MutationId>,
        max_prune: usize,
    ) -> Result<PruneReceipt, ControlError> {
        self.ensure_available()?;
        if max_prune == 0 {
            return Err(ControlError::BudgetExceeded);
        }

        let candidates = self
            .operations
            .iter()
            .filter(|(id, record)| {
                record.receipt.after_generation < retain_from_generation
                    && !protected_operations.contains(id)
            })
            .map(|(id, _)| *id)
            .take(max_prune)
            .collect::<Vec<_>>();
        for id in &candidates {
            self.operations.remove(id);
        }
        Ok(PruneReceipt {
            removed_operations: candidates.len(),
            retained_operations: self.operations.len(),
            protected_operations: protected_operations.len(),
        })
    }

    /// Places the journal in fail-closed quarantine.
    pub fn quarantine(&mut self, reason: ControlError) -> QuarantineReceipt {
        self.quarantine = Some(reason);
        QuarantineReceipt {
            identity: self.identity,
            generation: self.generation,
            reason,
        }
    }

    /// Returns bounded content-free health.
    #[must_use]
    pub fn journal_health(&self) -> ControlStoreHealth {
        ControlStoreHealth {
            identity: self.identity,
            generation: self.generation,
            record_count: self.records.len(),
            operation_count: self.operations.len(),
            total_value_bytes: self.records.values().map(ControlValue::len).sum(),
            state: self
                .quarantine
                .map_or(ControlStoreState::Ready, |_| ControlStoreState::Quarantined),
            reason: self.quarantine,
        }
    }

    fn ensure_available(&self) -> Result<(), ControlError> {
        match self.quarantine {
            Some(_) => Err(ControlError::StoreQuarantined),
            None => Ok(()),
        }
    }

    fn validate_key(&self, key: &ControlKey) -> Result<(), ControlError> {
        if key.as_bytes().is_empty() || key.as_bytes().len() > self.limits.max_key_bytes {
            return Err(ControlError::InvalidKey);
        }
        Ok(())
    }

    fn validate_key_value(
        &self,
        key: &ControlKey,
        value: &ControlValue,
    ) -> Result<(), ControlError> {
        self.validate_key(key)?;
        if value.is_empty() || value.len() > self.limits.max_value_bytes {
            return Err(ControlError::InvalidValue);
        }
        Ok(())
    }

    fn validate_staged_records(
        &self,
        staged: &BTreeMap<ControlKey, ControlValue>,
    ) -> Result<(), ControlError> {
        if staged.len() > self.limits.max_records {
            return Err(ControlError::BudgetExceeded);
        }
        let total = staged.values().try_fold(0_usize, |sum, value| {
            sum.checked_add(value.len())
                .ok_or(ControlError::BudgetExceeded)
        })?;
        if total > self.limits.max_total_value_bytes {
            return Err(ControlError::BudgetExceeded);
        }
        Ok(())
    }
}

/// Purely rebuilds a coherent immutable snapshot from committed read state.
pub fn rebuild_control_snapshot(
    read_snapshot: JournalReadSnapshot,
    expected_identity: JournalIdentity,
    limits: JournalLimits,
) -> Result<ControlSnapshot, ControlError> {
    let limits = limits.validate()?;
    if read_snapshot.identity != expected_identity {
        return Err(ControlError::IdentityMismatch);
    }
    if read_snapshot.records.len() > limits.max_records {
        return Err(ControlError::SnapshotRebuildFailed);
    }

    let mut previous: Option<&ControlKey> = None;
    let mut total = 0_usize;
    for (key, value) in &read_snapshot.records {
        if previous.is_some_and(|previous_key| previous_key >= key) {
            return Err(ControlError::SnapshotRebuildFailed);
        }
        if key.as_bytes().is_empty()
            || key.as_bytes().len() > limits.max_key_bytes
            || value.is_empty()
            || value.len() > limits.max_value_bytes
        {
            return Err(ControlError::SnapshotRebuildFailed);
        }
        total = total
            .checked_add(value.len())
            .ok_or(ControlError::SnapshotRebuildFailed)?;
        previous = Some(key);
    }
    if total > limits.max_total_value_bytes {
        return Err(ControlError::SnapshotRebuildFailed);
    }

    Ok(ControlSnapshot {
        identity: read_snapshot.identity,
        generation: read_snapshot.generation,
        records: read_snapshot.records,
    })
}

/// Process-local immutable control-snapshot publisher.
#[derive(Clone, Debug, Default)]
pub struct ControlSnapshotPublisher {
    current: Option<Arc<ControlSnapshot>>,
}

impl ControlSnapshotPublisher {
    /// Creates an empty publisher.
    #[must_use]
    pub const fn new() -> Self {
        Self { current: None }
    }

    /// Current coherent snapshot.
    #[must_use]
    pub fn current(&self) -> Option<Arc<ControlSnapshot>> {
        self.current.clone()
    }

    /// Publishes only a snapshot that exactly matches the committed receipt.
    pub fn publish_snapshot_after_commit(
        &mut self,
        commit: &ControlCommitReceipt,
        snapshot: ControlSnapshot,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        if snapshot.generation != commit.after_generation {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        let receipt = SnapshotPublishReceipt {
            identity: snapshot.identity,
            generation: snapshot.generation,
            operation_id: Some(commit.operation_id),
        };
        self.current = Some(Arc::new(snapshot));
        Ok(receipt)
    }

    /// Republishes exact current durable state without replaying its mutation.
    pub fn recover_snapshot_publication(
        &mut self,
        journal: &ControlJournal,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        let snapshot =
            rebuild_control_snapshot(journal.read_snapshot()?, journal.identity(), journal.limits)?;
        let receipt = SnapshotPublishReceipt {
            identity: snapshot.identity,
            generation: snapshot.generation,
            operation_id: None,
        };
        self.current = Some(Arc::new(snapshot));
        Ok(receipt)
    }
}

/// Content-free snapshot publication receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotPublishReceipt {
    /// Exact journal identity.
    pub identity: JournalIdentity,
    /// Published journal generation.
    pub generation: u64,
    /// Commit operation when publication immediately followed a transaction.
    pub operation_id: Option<MutationId>,
}

/// Content-free idempotency-pruning receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PruneReceipt {
    /// Number of operation rows removed.
    pub removed_operations: usize,
    /// Number of operation rows retained.
    pub retained_operations: usize,
    /// Number of caller-protected operation identities.
    pub protected_operations: usize,
}

/// Content-free quarantine receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuarantineReceipt {
    /// Exact quarantined journal identity.
    pub identity: JournalIdentity,
    /// Last coherent generation.
    pub generation: u64,
    /// Closed reason.
    pub reason: ControlError,
}

/// Control-store lifecycle visible to the daemon.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlStoreState {
    /// Journal is available for guarded reads and mutations.
    Ready,
    /// Journal is quarantined and denies normal access.
    Quarantined,
}

/// Bounded path-free control-store health.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlStoreHealth {
    /// Exact journal identity.
    pub identity: JournalIdentity,
    /// Exact committed generation.
    pub generation: u64,
    /// Number of live technical records.
    pub record_count: usize,
    /// Number of retained operation receipts.
    pub operation_count: usize,
    /// Total live value bytes.
    pub total_value_bytes: usize,
    /// Store lifecycle.
    pub state: ControlStoreState,
    /// Closed quarantine reason, if any.
    pub reason: Option<ControlError>,
}

/// Validated migration descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationPlan {
    /// Current schema version.
    pub from_version: u32,
    /// Strictly newer target schema version.
    pub to_version: u32,
    /// Digest of exact ordered migration steps.
    pub plan_digest: Blake3Digest32,
    /// Immutable fixture/evidence reference.
    pub fixture_ref: ReceiptRef,
}

impl MigrationPlan {
    /// Validates a strictly forward migration.
    pub fn validate(self) -> Result<Self, ControlError> {
        if self.from_version == 0 || self.to_version <= self.from_version {
            return Err(ControlError::SchemaUnsupported);
        }
        Ok(self)
    }
}

/// Explicit migration lifecycle. No unverified state can become ready.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationState {
    /// No migration is in progress.
    Stable {
        /// Current verified schema version.
        version: u32,
    },
    /// Durable intent exists.
    Planned(MigrationPlan),
    /// External mutation may have applied.
    OutcomeUnknown(MigrationPlan),
    /// Exact post-migration schema and fixtures were verified.
    Verified(MigrationPlan),
    /// Contradictory state is quarantined.
    Quarantined(ControlError),
}

/// Pure migration state machine used by a concrete backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationMachine {
    state: MigrationState,
}

impl MigrationMachine {
    /// Creates a stable non-zero schema state.
    pub fn new(version: u32) -> Result<Self, ControlError> {
        if version == 0 {
            return Err(ControlError::SchemaUnsupported);
        }
        Ok(Self {
            state: MigrationState::Stable { version },
        })
    }

    /// Current migration state.
    #[must_use]
    pub const fn state(&self) -> &MigrationState {
        &self.state
    }

    /// Records a forward migration plan.
    pub fn plan(&mut self, plan: MigrationPlan) -> Result<(), ControlError> {
        let plan = plan.validate()?;
        match &self.state {
            MigrationState::Stable { version } if *version == plan.from_version => {
                self.state = MigrationState::Planned(plan);
                Ok(())
            }
            MigrationState::Stable { .. } => Err(ControlError::SchemaMismatch),
            _ => Err(ControlError::MigrationUnverified),
        }
    }

    /// Marks the external migration outcome unresolved.
    pub fn mark_outcome_unknown(&mut self) -> Result<(), ControlError> {
        let plan = match &self.state {
            MigrationState::Planned(plan) => plan.clone(),
            _ => return Err(ControlError::MigrationUnverified),
        };
        self.state = MigrationState::OutcomeUnknown(plan);
        Ok(())
    }

    /// Accepts exact verification of the planned target and fixtures.
    pub fn verify(
        &mut self,
        observed_version: u32,
        observed_plan_digest: Blake3Digest32,
        fixture_verified: bool,
    ) -> Result<(), ControlError> {
        let plan = match &self.state {
            MigrationState::Planned(plan) | MigrationState::OutcomeUnknown(plan) => plan.clone(),
            _ => return Err(ControlError::MigrationUnverified),
        };
        if observed_version != plan.to_version
            || observed_plan_digest != plan.plan_digest
            || !fixture_verified
        {
            self.state = MigrationState::Quarantined(ControlError::MigrationUnverified);
            return Err(ControlError::MigrationUnverified);
        }
        self.state = MigrationState::Verified(plan);
        Ok(())
    }

    /// Commits verified migration state as stable.
    pub fn commit_verified(&mut self) -> Result<(), ControlError> {
        let version = match &self.state {
            MigrationState::Verified(plan) => plan.to_version,
            _ => return Err(ControlError::MigrationUnverified),
        };
        self.state = MigrationState::Stable { version };
        Ok(())
    }
}

#[cfg(test)]
mod migration_tests {
    use search_contracts::{Blake3Digest32, ReceiptRef};

    use super::{MigrationMachine, MigrationPlan, MigrationState};

    fn plan() -> MigrationPlan {
        MigrationPlan {
            from_version: 1,
            to_version: 2,
            plan_digest: Blake3Digest32::from_bytes([7; 32]),
            fixture_ref: ReceiptRef::new("receipt:migration-fixture").expect("receipt"),
        }
    }

    #[test]
    fn non_copy_plan_survives_unknown_readback_and_commit() {
        let plan = plan();
        let mut machine = MigrationMachine::new(1).expect("machine");
        machine.plan(plan.clone()).expect("plan");
        machine.mark_outcome_unknown().expect("unknown");
        machine.verify(2, plan.plan_digest, true).expect("verify");
        machine.commit_verified().expect("commit");
        assert_eq!(machine.state(), &MigrationState::Stable { version: 2 });
    }

    #[test]
    fn failed_fixture_verification_quarantines_migration() {
        let plan = plan();
        let mut machine = MigrationMachine::new(1).expect("machine");
        machine.plan(plan.clone()).expect("plan");
        assert!(machine.verify(2, plan.plan_digest, false).is_err());
        assert!(matches!(machine.state(), MigrationState::Quarantined(_)));
    }
}
