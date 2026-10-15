//! Closed registry failure surface, finite limits and vendor-neutral durability port.
//!
//! This module owns no registry state. It defines the typed failure codes
//! required by `FUNCTIONS.md`, the finite [`RegistryLimits`] dimensions and the
//! package-owned [`RegistryControlPort`] durability boundary. All durable
//! registry mutations persist their content-free receipt through the port; no
//! mutation writes directly to a concrete store, filesystem or in-memory map
//! without going through the port.

use std::collections::BTreeMap;

use search_contracts::OpaqueId;
use search_ports::{
    CancellationProbe, MutationIdentity, OperationContext, Port, PortErrorKind, PortReceipt,
    PortRetryability,
};

pub use search_source_records::error::{DEFAULT_REGISTRY_LIMITS, RegistryError, RegistryLimits};

pub use search_source_records::error::{JournalEntryKind, RegistryJournalEntry};

/// Vendor-neutral registry persistence boundary.
///
/// Batch implementations atomically persist complete bounded technical state
/// and its immutable operation receipt. Other mutations retain the existing
/// receipt-only boundary until their state transactions are implemented. Ports never see
/// source bytes, extracted text, vectors, Qdrant IDs or concrete store
/// handles. The registry never touches a concrete redb/filesystem handle; all
/// persistence flows through this trait.
pub trait RegistryControlPort: Port {
    /// Atomically compare the expected registry revision and every before value,
    /// write the complete bounded delta plus operation receipt, and return a
    /// coherent exact readback. Receipt-only persistence is not an implementation.
    ///
    /// `batch.expected_registry_revision()` is source owner state. Concrete
    /// adapters must independently compare their global journal generation.
    /// Reserved key classes must validate actual owner key/value bytes through
    /// the owner decoders; a journal tag alone provides no validation.
    fn commit_batch(
        &mut self,
        batch: &crate::recovery::PreparedBatch,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<crate::recovery::RegistryBatchReadback, Self::Error>;

    /// Read one immutable batch operation and every current target under one
    /// coherent store revision. `Absent` means proven atomic-operation absence
    /// at its returned revision, never an unavailable or failed read.
    /// Corrupt, partial, foreign or unavailable observations must be errors.
    fn load_batch(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<crate::recovery::RegistryBatchLookup, Self::Error>;

    /// Persists one legacy non-root, non-batch journal entry under a stable identity.
    /// Implementations must refuse batch and root mutation kinds; only
    /// `commit_batch` can mint their operation alongside complete state.
    fn persist_entry(
        &mut self,
        entry: &RegistryJournalEntry,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<PortReceipt, Self::Error>;

    /// Loads a previously persisted entry by exact operation identity.
    fn load_entry(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<RegistryJournalEntry>, Self::Error>;
}

pub use search_source_records::error::{
    RegistryPortError, cancelled_before_commit, registry_mutation, registry_port_error,
};

/// Bounded process-local fake implementing [`RegistryControlPort`].
///
/// Tests use this fake for atomic batch state/readback and legacy receipt
/// component evidence. It has no disk durability. A production adapter must
/// provide the required atomic batch transaction; the registry never branches
/// on a concrete store type.
#[derive(Clone, Debug)]
pub struct InMemoryRegistryJournal<C> {
    entries: BTreeMap<OpaqueId, (RegistryJournalEntry, MutationIdentity)>,
    max_entries: usize,
    revision: u64,
    batch_records: BTreeMap<OpaqueId, Vec<u8>>,
    state_records: BTreeMap<Vec<u8>, Vec<u8>>,
    marker: core::marker::PhantomData<fn() -> C>,
}

impl<C> InMemoryRegistryJournal<C> {
    /// Restores observed component state through the production owner factory.
    ///
    /// This process-local fake supplies actual retained key/value bytes and its
    /// independent registry revision. It proves neither disk durability nor live authority.
    pub fn restore_observed_registry(
        &self,
        limits: RegistryLimits,
        max_total_bytes: usize,
    ) -> Result<crate::recovery::ObservedRegistryRestore, RegistryError> {
        let states: Vec<_> = self
            .state_records
            .iter()
            .map(|(key, value)| crate::recovery::RegistryRestoreState { key, value })
            .collect();
        let operations: Vec<_> = self
            .batch_records
            .iter()
            .map(
                |(operation_id, value)| crate::recovery::RegistryRestoreOperation {
                    operation_id,
                    value,
                },
            )
            .collect();
        crate::recovery::restore_registry_state(
            self.revision,
            &states,
            &operations,
            limits,
            max_total_bytes,
        )
    }

    /// Creates a bounded fake journal.
    pub fn new(max_entries: usize) -> Result<Self, RegistryError> {
        if max_entries == 0 {
            return Err(RegistryError::InvalidLimits);
        }
        Ok(Self {
            entries: BTreeMap::new(),
            max_entries,
            revision: 0,
            batch_records: BTreeMap::new(),
            state_records: BTreeMap::new(),
            marker: core::marker::PhantomData,
        })
    }

    /// Number of retained entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the fake holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<C> Port for InMemoryRegistryJournal<C>
where
    C: CancellationProbe,
{
    type Error = RegistryPortError;
    type Cancellation = C;
}

impl<C> RegistryControlPort for InMemoryRegistryJournal<C>
where
    C: CancellationProbe,
{
    fn commit_batch(
        &mut self,
        batch: &crate::recovery::PreparedBatch,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<crate::recovery::RegistryBatchReadback, Self::Error> {
        use crate::recovery::RegistryBatchReadback;
        let record = batch.record();
        if context.preflight().is_err() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameIdentity,
                RegistryError::CancelledBeforeCommit,
                Some(mutation.operation_id.clone()),
            ));
        }
        if mutation.operation_id != record.entry().operation_id {
            return Err(registry_port_error(
                PortErrorKind::InvalidInput,
                PortRetryability::Never,
                RegistryError::OperationConflict,
                Some(mutation.operation_id.clone()),
            ));
        }
        if let crate::recovery::RegistryBatchLookup::Committed(existing) =
            self.load_batch(&mutation.operation_id, context)?
        {
            if existing.record() != record {
                return Err(registry_port_error(
                    PortErrorKind::Conflict,
                    PortRetryability::Never,
                    RegistryError::OperationConflict,
                    Some(mutation.operation_id.clone()),
                ));
            }
            return Ok(existing);
        }
        if self.entries.contains_key(&mutation.operation_id)
            || self.revision != record.entry().before_revision
            || batch.states().iter().any(|state| {
                self.state_records.get(state.key_bytes()).map(Vec::as_slice) != state.before_bytes()
            })
        {
            return Err(registry_port_error(
                PortErrorKind::CompareAndSwapRejected,
                PortRetryability::AfterReadback,
                RegistryError::RegistryRevisionConflict,
                Some(mutation.operation_id.clone()),
            ));
        }
        if self.entries.len() >= self.max_entries {
            return Err(registry_port_error(
                PortErrorKind::ResourceExhausted,
                PortRetryability::Never,
                RegistryError::DurabilityExhausted,
                Some(mutation.operation_id.clone()),
            ));
        }
        let states = record
            .deltas()
            .iter()
            .map(|d| d.after().clone())
            .collect::<Vec<_>>();
        let readback = RegistryBatchReadback::new(
            record.clone(),
            record.entry().after_revision,
            states,
            DEFAULT_REGISTRY_LIMITS,
        )
        .map_err(|reason| {
            registry_port_error(
                PortErrorKind::InvalidInput,
                PortRetryability::Never,
                reason,
                Some(mutation.operation_id.clone()),
            )
        })?;
        // This fake is process-local component evidence, not an installed DB.
        for state in batch.states() {
            self.state_records
                .insert(state.key_bytes().to_vec(), state.after_bytes().to_vec());
        }
        self.batch_records
            .insert(mutation.operation_id.clone(), batch.encoded().to_vec());
        self.entries.insert(
            mutation.operation_id.clone(),
            (record.entry().clone(), mutation.clone()),
        );
        self.revision = record.entry().after_revision;
        Ok(readback)
    }

    fn load_batch(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<crate::recovery::RegistryBatchLookup, Self::Error> {
        use crate::recovery::{
            RegistryBatchLookup, RegistryBatchReadback, decode_batch_record,
            decode_registry_state_entry,
        };
        if context.preflight().is_err() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                Some(operation_id.clone()),
            ));
        }
        let Some(bytes) = self.batch_records.get(operation_id) else {
            if self.entries.get(operation_id).is_some_and(|(entry, _)| {
                matches!(
                    entry.kind,
                    JournalEntryKind::RootRegistration
                        | JournalEntryKind::RootPolicyChange
                        | JournalEntryKind::RootUnbind
                )
            }) {
                return Err(registry_port_error(
                    PortErrorKind::Quarantined,
                    PortRetryability::Never,
                    RegistryError::RootRestorationUnavailable,
                    Some(operation_id.clone()),
                ));
            }
            if self.entries.contains_key(operation_id) {
                return Err(registry_port_error(
                    PortErrorKind::Quarantined,
                    PortRetryability::Never,
                    RegistryError::MutationOutcomeUnknown,
                    Some(operation_id.clone()),
                ));
            }
            return Ok(RegistryBatchLookup::Absent {
                revision: self.revision,
            });
        };
        let record = decode_batch_record(bytes, DEFAULT_REGISTRY_LIMITS).map_err(|reason| {
            registry_port_error(
                PortErrorKind::Quarantined,
                PortRetryability::Never,
                reason,
                Some(operation_id.clone()),
            )
        })?;
        let states = record
            .deltas()
            .iter()
            .map(|delta| {
                let key = delta.key().encode()?;
                let bytes = self
                    .state_records
                    .get(&key)
                    .ok_or(RegistryError::SnapshotInvalid)?;
                decode_registry_state_entry(&key, bytes)
            })
            .collect::<Result<Vec<_>, RegistryError>>()
            .map_err(|reason| {
                registry_port_error(
                    PortErrorKind::Quarantined,
                    PortRetryability::Never,
                    reason,
                    Some(operation_id.clone()),
                )
            })?;
        RegistryBatchReadback::new(record, self.revision, states, DEFAULT_REGISTRY_LIMITS)
            .map(RegistryBatchLookup::Committed)
            .map_err(|reason| {
                registry_port_error(
                    PortErrorKind::Quarantined,
                    PortRetryability::Never,
                    reason,
                    Some(operation_id.clone()),
                )
            })
    }

    fn persist_entry(
        &mut self,
        entry: &RegistryJournalEntry,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<PortReceipt, Self::Error> {
        if matches!(
            entry.kind,
            JournalEntryKind::BatchApply
                | JournalEntryKind::RootRegistration
                | JournalEntryKind::RootPolicyChange
                | JournalEntryKind::RootUnbind
        ) {
            return Err(registry_port_error(
                PortErrorKind::InvalidInput,
                PortRetryability::Never,
                RegistryError::DurabilityRejected,
                Some(mutation.operation_id.clone()),
            ));
        }
        if context.cancellation().is_cancelled() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                Some(mutation.operation_id.clone()),
            ));
        }
        if let Some((existing, _)) = self.entries.get(&entry.operation_id) {
            if existing.mutation_digest != entry.mutation_digest {
                return Err(registry_port_error(
                    PortErrorKind::Conflict,
                    PortRetryability::Never,
                    RegistryError::OperationConflict,
                    Some(mutation.operation_id.clone()),
                ));
            }
            return Ok(port_receipt_for(entry, true));
        }
        if self.entries.len() >= self.max_entries {
            return Err(registry_port_error(
                PortErrorKind::ResourceExhausted,
                PortRetryability::AfterReadback,
                RegistryError::DurabilityExhausted,
                Some(mutation.operation_id.clone()),
            ));
        }
        self.entries.insert(
            entry.operation_id.clone(),
            (entry.clone(), mutation.clone()),
        );
        self.revision = entry.after_revision;
        Ok(port_receipt_for(entry, false))
    }

    fn load_entry(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<RegistryJournalEntry>, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                Some(operation_id.clone()),
            ));
        }
        Ok(self
            .entries
            .get(operation_id)
            .map(|(entry, _)| entry.clone()))
    }
}

fn port_receipt_for(entry: &RegistryJournalEntry, replayed: bool) -> PortReceipt {
    use search_contracts::BoundedNonContentMetadata;
    use search_ports::{PortOutcome, ReceiptRetryability};
    let _ = replayed;
    PortReceipt {
        operation_id: entry.operation_id.clone(),
        dependency_generation_digest: entry.mutation_digest,
        outcome: PortOutcome::Complete,
        retryability: ReceiptRetryability::SameIdentity,
        bounded_metadata: BoundedNonContentMetadata::empty(),
    }
}
