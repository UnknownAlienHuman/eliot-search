//! Finite append, recovery, tombstone, and exact-deletion state machine.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

use super::error::RevisionStoreError;
use super::limits::{
    ENVELOPE_BINDING_VERSION, MIN_CIPHERTEXT_BYTES, RevisionStoreLimits,
};
use super::model::{
    LifecycleDeletionPlan, ObjectDeletionReceipt, PrepareAppendResult,
    PurgeTombstone, PurgeTombstoneReceipt, RecoveryResult, RevisionKey,
    RevisionObjectReadback, RevisionOperation, RevisionRecord, RevisionState,
    RevisionStoreReceipt, RevisionWriteIntent, TombstoneScope,
};
use super::residency::ResidencyClosure;

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

    /// Prepares one append-only revision intent or replays an exact active receipt.
    ///
    /// Reuse across inequivalent residency closures is a typed
    /// [`RevisionStoreError::ResidencyMismatch`]; reuse inside one equivalent
    /// closure replays only after verifying exact bytes. A bound operation
    /// whose state was deleted falls through to honest re-admission with
    /// fresh readback instead of replaying a stale receipt.
    pub fn prepare_append(
        &mut self,
        intent: RevisionWriteIntent,
    ) -> Result<PrepareAppendResult, RevisionStoreError> {
        validate_intent(&intent, self.limits)?;
        if let Some((_, digest, receipt)) = self
            .operations
            .iter()
            .find(|(operation_id, _, _)| operation_id == intent.operation.operation_id())
        {
            let bound = *digest == intent.operation.request_digest()
                && receipt.key == intent.key
                && receipt.content_digest == intent.payload.plaintext_digest
                && receipt.ciphertext_digest == intent.payload.ciphertext_digest;
            if !bound {
                return Err(RevisionStoreError::OperationConflict);
            }
            if let Some(RevisionState::Active(record)) = self.states.get(&intent.key)
                && record.operation == intent.operation
                && exact_record_matches_intent(record, &intent)
            {
                let mut replay = receipt.clone();
                replay.replayed = true;
                return Ok(PrepareAppendResult::AlreadyStored(replay));
            }
        }
        if self
            .deletions
            .iter()
            .any(|(operation_id, _, _)| operation_id == intent.operation.operation_id())
            || self.tombstones.iter().any(|tombstone| {
                tombstone.operation.operation_id() == intent.operation.operation_id()
            })
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if self.is_fenced(&intent.key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        if let Some(bound) =
            occurrence_residency(&self.states, &intent.key.source_id, intent.key.revision)
            && bound != intent.key.residency
        {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.operation_count() >= self.limits.max_operations
            || self.states.len() >= self.limits.max_revisions
        {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        if let Some(existing) = self.states.get(&intent.key) {
            return match existing {
                RevisionState::Active(record) if exact_record_matches_intent(record, &intent) => {
                    Ok(PrepareAppendResult::AlreadyStored(receipt_from_record(
                        record, true,
                    )))
                }
                RevisionState::Pending(existing) | RevisionState::OutcomeUnknown(existing) => {
                    if existing == &intent {
                        Ok(PrepareAppendResult::Prepared(existing.clone()))
                    } else if existing.operation == intent.operation {
                        Err(RevisionStoreError::OperationConflict)
                    } else {
                        Err(RevisionStoreError::RevisionConflict)
                    }
                }
                RevisionState::Active(_) | RevisionState::Quarantined { .. } => {
                    Err(RevisionStoreError::RevisionConflict)
                }
            };
        }
        if let Some(conflict) = reuse_conflict(&self.states, &intent) {
            return Err(conflict);
        }
        validate_next_source_revision(&self.states, &intent.key)?;
        self.states
            .insert(intent.key.clone(), RevisionState::Pending(intent.clone()));
        Ok(PrepareAppendResult::Prepared(intent))
    }

    /// Marks a possible external object write as unresolved.
    pub fn mark_outcome_unknown(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
    ) -> Result<(), RevisionStoreError> {
        let state = self
            .states
            .get_mut(key)
            .ok_or(RevisionStoreError::RevisionNotFound)?;
        match state {
            RevisionState::Pending(intent) if &intent.operation == operation => {
                *state = RevisionState::OutcomeUnknown(intent.clone());
                Ok(())
            }
            RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_) => {
                Err(RevisionStoreError::OperationConflict)
            }
            RevisionState::Active(_) => Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => Err(RevisionStoreError::Quarantined),
        }
    }

    /// Confirms an exact prepared write after authoritative durable readback.
    pub fn confirm_append(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
        readback: RevisionObjectReadback,
    ) -> Result<RevisionStoreReceipt, RevisionStoreError> {
        let intent = match self.state(key)? {
            RevisionState::Pending(intent) | RevisionState::OutcomeUnknown(intent)
                if &intent.operation == operation => intent.clone(),
            RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_) => {
                return Err(RevisionStoreError::OperationConflict);
            }
            RevisionState::Active(record) if &record.operation == operation => {
                return Ok(receipt_from_record(record, true));
            }
            RevisionState::Active(_) => return Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => return Err(RevisionStoreError::Quarantined),
        };
        if self.is_fenced(key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        let record = record_from_readback(&intent, readback)?;
        let receipt = receipt_from_record(&record, false);
        self.states
            .insert(key.clone(), RevisionState::Active(record));
        push_operation(&mut self.operations, operation, receipt.clone());
        Ok(receipt)
    }

    /// Recovers a possible write by exact authoritative readback.
    pub fn recover_unknown(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
        readback: Option<RevisionObjectReadback>,
    ) -> Result<RecoveryResult, RevisionStoreError> {
        let intent = match self.state(key)? {
            RevisionState::OutcomeUnknown(intent) if &intent.operation == operation => {
                intent.clone()
            }
            RevisionState::OutcomeUnknown(_) | RevisionState::Pending(_) => {
                return Err(RevisionStoreError::OperationConflict);
            }
            RevisionState::Active(record) if &record.operation == operation => {
                return Ok(RecoveryResult::Applied(receipt_from_record(record, true)));
            }
            RevisionState::Active(_) => return Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => return Err(RevisionStoreError::Quarantined),
        };
        if self.is_fenced(key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        let Some(readback) = readback else {
            self.states.remove(key);
            return Ok(RecoveryResult::NotApplied);
        };
        match record_from_readback(&intent, readback) {
            Ok(record) => {
                let receipt = receipt_from_record(&record, false);
                self.states
                    .insert(key.clone(), RevisionState::Active(record));
                push_operation(&mut self.operations, operation, receipt.clone());
                Ok(RecoveryResult::Applied(receipt))
            }
            Err(
                RevisionStoreError::ReadbackMismatch
                | RevisionStoreError::EvidenceMissing
                | RevisionStoreError::BackendContractViolation,
            ) => {
                self.states.insert(
                    key.clone(),
                    RevisionState::Quarantined {
                        key: key.clone(),
                        operation: operation.clone(),
                    },
                );
                Ok(RecoveryResult::Quarantined)
            }
            Err(error) => Err(error),
        }
    }

    /// Installs a purge tombstone at the admission boundary.
    ///
    /// The same tombstone reinstalls idempotently. A conflicting receipt for
    /// the same scope and generation, or any operation-identity reuse with a
    /// different payload, fails closed. There is no removal operation.
    pub fn install_purge_tombstone(
        &mut self,
        tombstone: PurgeTombstone,
    ) -> Result<PurgeTombstoneReceipt, RevisionStoreError> {
        if self
            .operations
            .iter()
            .any(|(operation_id, _, _)| operation_id == tombstone.operation.operation_id())
            || self
                .deletions
                .iter()
                .any(|(operation_id, _, _)| operation_id == tombstone.operation.operation_id())
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if let Some(existing) = self.tombstones.iter().find(|candidate| {
            candidate.operation.operation_id() == tombstone.operation.operation_id()
        }) {
            if existing.operation.request_digest() != tombstone.operation.request_digest()
                || *existing != tombstone
            {
                return Err(RevisionStoreError::OperationConflict);
            }
            return Ok(tombstone_receipt(existing, true));
        }
        if let Some(existing) = self.tombstones.iter().find(|candidate| {
            candidate.scope == tombstone.scope && candidate.generation == tombstone.generation
        }) {
            if existing.tombstone_receipt != tombstone.tombstone_receipt {
                return Err(RevisionStoreError::RevisionConflict);
            }
            return Ok(tombstone_receipt(existing, true));
        }
        if self.operation_count() >= self.limits.max_operations {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        self.tombstones.push(tombstone.clone());
        Ok(tombstone_receipt(&tombstone, false))
    }

    /// Executes one exact bounded deletion under a lifecycle-owner plan.
    ///
    /// Only an `Active` record matching the exact target key and storage
    /// object is removed, and only with the plan receipt the lifecycle owner
    /// issued. Pending or unknown writes are never reported as deleted;
    /// the same plan replays idempotently while any operation reuse with a
    /// different plan conflicts.
    pub fn apply_exact_object_deletion(
        &mut self,
        plan: LifecycleDeletionPlan,
    ) -> Result<ObjectDeletionReceipt, RevisionStoreError> {
        if self
            .operations
            .iter()
            .any(|(operation_id, _, _)| operation_id == plan.operation.operation_id())
            || self.tombstones.iter().any(|tombstone| {
                tombstone.operation.operation_id() == plan.operation.operation_id()
            })
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if let Some((_, digest, receipt)) = self
            .deletions
            .iter()
            .find(|(operation_id, _, _)| operation_id == plan.operation.operation_id())
        {
            let bound = *digest == plan.operation.request_digest()
                && receipt.target == plan.target
                && receipt.target_storage_object_id == plan.target_storage_object_id
                && receipt.authority == plan.authority
                && receipt.plan_receipt == plan.plan_receipt;
            if !bound {
                return Err(RevisionStoreError::OperationConflict);
            }
            let mut replay = receipt.clone();
            replay.replayed = true;
            return Ok(replay);
        }
        if self.operation_count() >= self.limits.max_operations {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        let record = match self.states.get(&plan.target) {
            None => return Err(RevisionStoreError::RevisionNotFound),
            Some(RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_)) => {
                return Err(RevisionStoreError::OutcomeUnknown);
            }
            Some(RevisionState::Quarantined { .. }) => {
                return Err(RevisionStoreError::Quarantined);
            }
            Some(RevisionState::Active(record)) => record.clone(),
        };
        if record.key != plan.target || record.storage_object_id != plan.target_storage_object_id {
            return Err(RevisionStoreError::DeletionNotAuthorized);
        }
        self.states.remove(&plan.target);
        let receipt = ObjectDeletionReceipt {
            target: plan.target.clone(),
            target_storage_object_id: plan.target_storage_object_id.clone(),
            authority: plan.authority,
            plan_receipt: plan.plan_receipt.clone(),
            operation: plan.operation.clone(),
            replayed: false,
        };
        self.deletions.push((
            plan.operation.operation_id().clone(),
            plan.operation.request_digest(),
            receipt.clone(),
        ));
        Ok(receipt)
    }
}

fn validate_intent(
    intent: &RevisionWriteIntent,
    limits: RevisionStoreLimits,
) -> Result<(), RevisionStoreError> {
    let limits = limits.validate()?;
    if intent.payload.plaintext_bytes == 0
        || intent.payload.plaintext_bytes > limits.max_plaintext_bytes
    {
        return Err(RevisionStoreError::PlaintextSizeInvalid);
    }
    let ciphertext_bytes = u64::try_from(intent.payload.ciphertext_len())
        .map_err(|_| RevisionStoreError::CiphertextSizeInvalid)?;
    if ciphertext_bytes == 0
        || ciphertext_bytes > limits.max_ciphertext_bytes
        || intent.payload.ciphertext_len() < MIN_CIPHERTEXT_BYTES
    {
        return Err(RevisionStoreError::CiphertextSizeInvalid);
    }
    if intent.payload.nonce().is_empty()
        || intent.payload.nonce().len() > limits.max_nonce_bytes
    {
        return Err(RevisionStoreError::NonceInvalid);
    }
    if intent.authorization_receipt.is_none() {
        return Err(RevisionStoreError::EvidenceMissing);
    }
    if intent.envelope.version != ENVELOPE_BINDING_VERSION {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    if intent.envelope.plaintext_length != intent.payload.plaintext_bytes {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    if intent.envelope.key_generation != intent.payload.encryption.key_version {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    match &intent.legacy_migration {
        Some(migration) => {
            if intent.residency_key != migration.legacy_residency_key {
                return Err(RevisionStoreError::ResidencyMismatch);
            }
        }
        None => {
            if intent.residency_key != intent.key.residency.scope_id()? {
                return Err(RevisionStoreError::ResidencyMismatch);
            }
        }
    }
    Ok(())
}

/// Returns the residency bound to one occurrence, if any state names it.
///
/// Occurrence identity is `(source_id, revision)`; the map key additionally
/// carries the closure so this scan is the single binding check.
fn occurrence_residency(
    states: &BTreeMap<RevisionKey, RevisionState>,
    source_id: &OpaqueId,
    revision: NonZeroRevision,
) -> Option<ResidencyClosure> {
    states
        .keys()
        .find(|key| key.source_id == *source_id && key.revision == revision)
        .map(|key| key.residency)
}

/// Detects physical reuse across inequivalent residency closures.
///
/// Object identities, ciphertext digests, envelope binding digests, and
/// secret references must never cross residency boundaries. Inside one
/// equivalent closure, sharing one object identity for different bytes is an
/// immutable object conflict, never silent overwriting.
fn reuse_conflict(
    states: &BTreeMap<RevisionKey, RevisionState>,
    intent: &RevisionWriteIntent,
) -> Option<RevisionStoreError> {
    for state in states.values() {
        let (
            residency,
            storage_object_id,
            ciphertext_digest,
            key_reference,
            source_binding_digest,
            residency_binding_digest,
        ) = match state {
            RevisionState::Pending(existing) | RevisionState::OutcomeUnknown(existing) => (
                existing.key.residency,
                &existing.storage_object_id,
                existing.payload.ciphertext_digest,
                &existing.payload.encryption.key_reference,
                existing.envelope.source_revision_binding_digest,
                existing.envelope.residency_binding_digest,
            ),
            RevisionState::Active(record) => (
                record.key.residency,
                &record.storage_object_id,
                record.ciphertext_digest,
                &record.encryption.key_reference,
                record.envelope.source_revision_binding_digest,
                record.envelope.residency_binding_digest,
            ),
            RevisionState::Quarantined { .. } => continue,
        };
        if residency == intent.key.residency {
            if storage_object_id == &intent.storage_object_id
                && ciphertext_digest != intent.payload.ciphertext_digest
            {
                return Some(RevisionStoreError::RevisionConflict);
            }
            continue;
        }
        if storage_object_id == &intent.storage_object_id
            || ciphertext_digest == intent.payload.ciphertext_digest
            || key_reference == &intent.payload.encryption.key_reference
            || source_binding_digest == intent.envelope.source_revision_binding_digest
            || residency_binding_digest == intent.envelope.residency_binding_digest
        {
            return Some(RevisionStoreError::ResidencyMismatch);
        }
    }
    None
}

/// Records one append operation identity unless it is already indexed.
///
/// Re-admission of a deleted occurrence reuses its bound operation identity;
/// the index keeps the first entry so history never duplicates.
fn push_operation(
    operations: &mut Vec<(OpaqueId, Blake3Digest32, RevisionStoreReceipt)>,
    operation: &RevisionOperation,
    receipt: RevisionStoreReceipt,
) {
    if !operations
        .iter()
        .any(|(operation_id, _, _)| operation_id == operation.operation_id())
    {
        operations.push((
            operation.operation_id().clone(),
            operation.request_digest(),
            receipt,
        ));
    }
}

/// Builds a content-free tombstone install receipt.
fn tombstone_receipt(tombstone: &PurgeTombstone, replayed: bool) -> PurgeTombstoneReceipt {
    PurgeTombstoneReceipt {
        scope: tombstone.scope,
        generation: tombstone.generation,
        tombstone_receipt: tombstone.tombstone_receipt.clone(),
        operation: tombstone.operation.clone(),
        replayed,
    }
}

fn validate_next_source_revision(
    states: &BTreeMap<RevisionKey, RevisionState>,
    key: &RevisionKey,
) -> Result<(), RevisionStoreError> {
    let latest = states
        .keys()
        .filter(|existing| existing.source_id == key.source_id)
        .map(|existing| existing.revision)
        .max();
    match latest {
        None if key.revision.get() == 1 => Ok(()),
        Some(current)
            if current
                .checked_next()
                .map_err(|_| RevisionStoreError::ContractExhausted)?
                == key.revision =>
        {
            Ok(())
        }
        None | Some(_) => Err(RevisionStoreError::RevisionSequenceInvalid),
    }
}

fn exact_record_matches_intent(record: &RevisionRecord, intent: &RevisionWriteIntent) -> bool {
    record.key == intent.key
        && record.source_binding_revision == intent.source_binding_revision
        && record.content_digest == intent.payload.plaintext_digest
        && record.plaintext_bytes == intent.payload.plaintext_bytes
        && record.ciphertext_digest == intent.payload.ciphertext_digest
        && record.storage_object_id == intent.storage_object_id
        && record.residency_key == intent.residency_key
        && record.encryption == intent.payload.encryption
        && record.envelope == intent.envelope
        && record.ingest == intent.ingest
        && record.operation == intent.operation
}

fn record_from_readback(
    intent: &RevisionWriteIntent,
    readback: RevisionObjectReadback,
) -> Result<RevisionRecord, RevisionStoreError> {
    if !readback.readback_verified {
        return Err(RevisionStoreError::EvidenceMissing);
    }
    let object_receipt = readback
        .object_receipt
        .ok_or(RevisionStoreError::EvidenceMissing)?;
    let authorization_receipt = intent
        .authorization_receipt
        .clone()
        .ok_or(RevisionStoreError::EvidenceMissing)?;
    let expected_ciphertext_bytes = u64::try_from(intent.payload.ciphertext_len())
        .map_err(|_| RevisionStoreError::BackendContractViolation)?;
    if readback.key != intent.key
        || readback.storage_object_id != intent.storage_object_id
        || readback.ciphertext_digest != intent.payload.ciphertext_digest
        || readback.ciphertext_bytes != expected_ciphertext_bytes
        || readback.plaintext_digest != intent.payload.plaintext_digest
        || readback.plaintext_bytes != intent.payload.plaintext_bytes
        || readback.encryption != intent.payload.encryption
        || readback.envelope != intent.envelope
    {
        return Err(RevisionStoreError::ReadbackMismatch);
    }
    Ok(RevisionRecord {
        key: intent.key.clone(),
        source_binding_revision: intent.source_binding_revision,
        content_digest: intent.payload.plaintext_digest,
        plaintext_bytes: intent.payload.plaintext_bytes,
        ciphertext_digest: intent.payload.ciphertext_digest,
        ciphertext_bytes: expected_ciphertext_bytes,
        storage_object_id: intent.storage_object_id.clone(),
        residency_key: intent.residency_key.clone(),
        encryption: intent.payload.encryption.clone(),
        envelope: intent.envelope,
        ingest: intent.ingest.clone(),
        authorization_receipt,
        object_receipt,
        operation: intent.operation.clone(),
    })
}

fn receipt_from_record(record: &RevisionRecord, replayed: bool) -> RevisionStoreReceipt {
    RevisionStoreReceipt {
        key: record.key.clone(),
        residency: record.key.residency,
        operation: record.operation.clone(),
        content_digest: record.content_digest,
        ciphertext_digest: record.ciphertext_digest,
        object_receipt: record.object_receipt.clone(),
        replayed,
    }
}
