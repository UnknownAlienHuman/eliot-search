//! Immutable encrypted retained-revision storage semantics.
//!
//! This package performs no filesystem, database, encryption, or secret-store
//! I/O. Callers provide already encrypted finite payloads and exact backend
//! readback. The kernel enforces source-revision monotonicity, immutability,
//! replay fencing, unknown-outcome recovery, and content-free receipts. A
//! concrete object-store adapter must separately prove atomic write/readback and
//! encryption-at-rest behavior.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;
use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

/// Conservative finite retained-revision limits.
pub const DEFAULT_REVISION_STORE_LIMITS: RevisionStoreLimits = RevisionStoreLimits {
    max_plaintext_bytes: 8 * 1024 * 1024 * 1024,
    max_ciphertext_bytes: 8 * 1024 * 1024 * 1024 + 1_048_576,
    max_revisions: 4_000_000,
    max_operations: 8_000_000,
    max_nonce_bytes: 64,
};

/// Closed content-free revision-store failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RevisionStoreError {
    /// Store limits are zero or internally inconsistent.
    InvalidLimits,
    /// Plaintext byte count is zero or exceeds its finite ceiling.
    PlaintextSizeInvalid,
    /// Ciphertext is empty or exceeds its finite ceiling.
    CiphertextSizeInvalid,
    /// Nonce is empty or exceeds its finite ceiling.
    NonceInvalid,
    /// Source revision is absent or cannot advance exactly once.
    RevisionSequenceInvalid,
    /// Exact source/revision key already stores different immutable metadata.
    RevisionConflict,
    /// Exact revision is absent.
    RevisionNotFound,
    /// Operation identity was reused with another complete request digest.
    OperationConflict,
    /// Finite revision or operation capacity was exhausted.
    CapacityExceeded,
    /// Write confirmation does not match the prepared exact intent.
    ReadbackMismatch,
    /// Required authorization or durable readback evidence is absent.
    EvidenceMissing,
    /// Possible external write has unknown authoritative outcome.
    OutcomeUnknown,
    /// Contradictory object state requires quarantine.
    Quarantined,
    /// Backend failed before a verified outcome existed.
    BackendFailure,
    /// Backend returned malformed or contradictory data.
    BackendContractViolation,
    /// Shared revision cannot advance.
    ContractExhausted,
}

impl RevisionStoreError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "REVISION_STORE_INVALID_LIMITS",
            Self::PlaintextSizeInvalid => "REVISION_STORE_PLAINTEXT_SIZE_INVALID",
            Self::CiphertextSizeInvalid => "REVISION_STORE_CIPHERTEXT_SIZE_INVALID",
            Self::NonceInvalid => "REVISION_STORE_NONCE_INVALID",
            Self::RevisionSequenceInvalid => "REVISION_STORE_REVISION_SEQUENCE_INVALID",
            Self::RevisionConflict => "REVISION_STORE_REVISION_CONFLICT",
            Self::RevisionNotFound => "REVISION_STORE_REVISION_NOT_FOUND",
            Self::OperationConflict => "REVISION_STORE_OPERATION_CONFLICT",
            Self::CapacityExceeded => "REVISION_STORE_CAPACITY_EXCEEDED",
            Self::ReadbackMismatch => "REVISION_STORE_READBACK_MISMATCH",
            Self::EvidenceMissing => "REVISION_STORE_EVIDENCE_MISSING",
            Self::OutcomeUnknown => "REVISION_STORE_OUTCOME_UNKNOWN",
            Self::Quarantined => "REVISION_STORE_QUARANTINED",
            Self::BackendFailure => "REVISION_STORE_BACKEND_FAILURE",
            Self::BackendContractViolation => "REVISION_STORE_BACKEND_CONTRACT_VIOLATION",
            Self::ContractExhausted => "REVISION_STORE_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for RevisionStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RevisionStoreError {}

/// Finite retained-revision limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevisionStoreLimits {
    /// Maximum exact plaintext bytes represented by one retained revision.
    pub max_plaintext_bytes: u64,
    /// Maximum encrypted object bytes.
    pub max_ciphertext_bytes: u64,
    /// Maximum retained immutable revision records.
    pub max_revisions: usize,
    /// Maximum retained operation identities.
    pub max_operations: usize,
    /// Maximum authenticated-encryption nonce bytes.
    pub max_nonce_bytes: usize,
}

impl RevisionStoreLimits {
    /// Validates all finite dimensions and ciphertext capacity.
    pub const fn validate(self) -> Result<Self, RevisionStoreError> {
        if self.max_plaintext_bytes == 0
            || self.max_ciphertext_bytes == 0
            || self.max_ciphertext_bytes < self.max_plaintext_bytes
            || self.max_revisions == 0
            || self.max_operations == 0
            || self.max_nonce_bytes == 0
        {
            Err(RevisionStoreError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Immutable stable-source and retained-revision key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RevisionKey {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Monotone retained revision for that source.
    pub revision: NonZeroRevision,
}

/// Closed encrypted-object suite identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CipherSuite {
    /// Version-one authenticated-encryption adapter profile.
    AuthenticatedEncryptionV1,
}

/// Exact encryption-key binding without key material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptionBinding {
    /// Opaque secret reference resolved by the secret-owning adapter.
    pub key_reference: OpaqueId,
    /// Monotone key version.
    pub key_version: NonZeroRevision,
    /// Closed cipher-suite profile.
    pub cipher_suite: CipherSuite,
}

/// Finite already-encrypted retained-revision payload.
#[derive(Clone, Eq, PartialEq)]
pub struct EncryptedRevisionPayload {
    /// Exact plaintext content digest.
    pub plaintext_digest: Blake3Digest32,
    /// Exact plaintext byte count.
    pub plaintext_bytes: u64,
    /// Exact ciphertext digest.
    pub ciphertext_digest: Blake3Digest32,
    /// Exact authenticated-encryption nonce.
    nonce: Vec<u8>,
    /// Exact encrypted object bytes including authentication tag.
    ciphertext: Vec<u8>,
    /// Exact encryption-key binding.
    pub encryption: EncryptionBinding,
}

impl EncryptedRevisionPayload {
    /// Creates a finite non-empty encrypted payload.
    pub fn new(
        plaintext_digest: Blake3Digest32,
        plaintext_bytes: u64,
        ciphertext_digest: Blake3Digest32,
        nonce: Vec<u8>,
        ciphertext: Vec<u8>,
        encryption: EncryptionBinding,
        limits: RevisionStoreLimits,
    ) -> Result<Self, RevisionStoreError> {
        let limits = limits.validate()?;
        if plaintext_bytes == 0 || plaintext_bytes > limits.max_plaintext_bytes {
            return Err(RevisionStoreError::PlaintextSizeInvalid);
        }
        let ciphertext_len = u64::try_from(ciphertext.len())
            .map_err(|_| RevisionStoreError::CiphertextSizeInvalid)?;
        if ciphertext.is_empty() || ciphertext_len > limits.max_ciphertext_bytes {
            return Err(RevisionStoreError::CiphertextSizeInvalid);
        }
        if nonce.is_empty() || nonce.len() > limits.max_nonce_bytes {
            return Err(RevisionStoreError::NonceInvalid);
        }
        Ok(Self {
            plaintext_digest,
            plaintext_bytes,
            ciphertext_digest,
            nonce,
            ciphertext,
            encryption,
        })
    }

    /// Exact nonce bytes for the encryption/storage adapter.
    pub fn nonce(&self) -> &[u8] {
        &self.nonce
    }

    /// Exact encrypted object bytes.
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    /// Exact ciphertext byte length.
    pub fn ciphertext_len(&self) -> usize {
        self.ciphertext.len()
    }
}

impl fmt::Debug for EncryptedRevisionPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedRevisionPayload")
            .field("plaintext_digest", &self.plaintext_digest)
            .field("plaintext_bytes", &self.plaintext_bytes)
            .field("ciphertext_digest", &self.ciphertext_digest)
            .field("nonce", &format_args!("<{} bytes>", self.nonce.len()))
            .field(
                "ciphertext",
                &format_args!("<{} encrypted bytes>", self.ciphertext.len()),
            )
            .field("encryption", &self.encryption)
            .finish()
    }
}

/// Full-payload immutable revision-store operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionOperation {
    operation_id: OpaqueId,
    request_digest: Blake3Digest32,
}

impl RevisionOperation {
    /// Creates a replay-fenced operation.
    #[must_use]
    pub const fn new(operation_id: OpaqueId, request_digest: Blake3Digest32) -> Self {
        Self {
            operation_id,
            request_digest,
        }
    }

    /// Immutable operation identifier.
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    /// Digest of exact canonical operation payload.
    pub const fn request_digest(&self) -> Blake3Digest32 {
        self.request_digest
    }
}

/// Exact append intent supplied to the object-store adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionWriteIntent {
    /// Immutable source/revision key.
    pub key: RevisionKey,
    /// Exact source-binding revision observed by the safe-reader pipeline.
    pub source_binding_revision: NonZeroRevision,
    /// Exact encrypted payload.
    pub payload: EncryptedRevisionPayload,
    /// Opaque content-addressed storage object identity.
    pub storage_object_id: OpaqueId,
    /// Opaque residency/retention binding.
    pub residency_key: OpaqueId,
    /// Authorization receipt for retaining this source revision.
    pub authorization_receipt: Option<ReceiptRef>,
    /// Full-payload immutable operation.
    pub operation: RevisionOperation,
}

/// Exact durable object readback after a possible write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionObjectReadback {
    /// Immutable source/revision key.
    pub key: RevisionKey,
    /// Opaque storage object identity.
    pub storage_object_id: OpaqueId,
    /// Exact ciphertext digest.
    pub ciphertext_digest: Blake3Digest32,
    /// Exact ciphertext byte count.
    pub ciphertext_bytes: u64,
    /// Exact plaintext content digest stored in authenticated metadata.
    pub plaintext_digest: Blake3Digest32,
    /// Exact plaintext byte count stored in authenticated metadata.
    pub plaintext_bytes: u64,
    /// Exact encryption-key binding.
    pub encryption: EncryptionBinding,
    /// Whether authoritative durable readback completed.
    pub readback_verified: bool,
    /// Content-free durable object receipt.
    pub object_receipt: Option<ReceiptRef>,
}

/// Durable immutable retained-revision record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionRecord {
    /// Immutable source/revision key.
    pub key: RevisionKey,
    /// Source-binding revision used for the read.
    pub source_binding_revision: NonZeroRevision,
    /// Exact plaintext content digest.
    pub content_digest: Blake3Digest32,
    /// Exact plaintext byte count.
    pub plaintext_bytes: u64,
    /// Exact ciphertext digest.
    pub ciphertext_digest: Blake3Digest32,
    /// Exact ciphertext byte count.
    pub ciphertext_bytes: u64,
    /// Opaque storage object identity.
    pub storage_object_id: OpaqueId,
    /// Opaque residency/retention binding.
    pub residency_key: OpaqueId,
    /// Exact encryption-key binding.
    pub encryption: EncryptionBinding,
    /// Authorization receipt.
    pub authorization_receipt: ReceiptRef,
    /// Durable object readback receipt.
    pub object_receipt: ReceiptRef,
    /// Full-payload immutable operation.
    pub operation: RevisionOperation,
}

/// Pending or terminal state of one revision key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevisionState {
    /// Durable control intent exists; object write is not confirmed.
    Pending(RevisionWriteIntent),
    /// Possible object write has unknown authoritative outcome.
    OutcomeUnknown(RevisionWriteIntent),
    /// Exact durable object readback confirmed immutable storage.
    Active(RevisionRecord),
    /// Contradictory object state requires quarantine.
    Quarantined {
        /// Immutable revision key.
        key: RevisionKey,
        /// Full-payload operation that encountered contradiction.
        operation: RevisionOperation,
    },
}

/// Content-free append receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionStoreReceipt {
    /// Immutable revision key.
    pub key: RevisionKey,
    /// Full-payload operation.
    pub operation: RevisionOperation,
    /// Exact plaintext content digest.
    pub content_digest: Blake3Digest32,
    /// Exact ciphertext digest.
    pub ciphertext_digest: Blake3Digest32,
    /// Durable object receipt.
    pub object_receipt: ReceiptRef,
    /// Whether the receipt was replayed from an existing active revision.
    pub replayed: bool,
}

/// Result of preparing one append.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrepareAppendResult {
    /// New exact intent was installed and must be written by the backend.
    Prepared(RevisionWriteIntent),
    /// Exact immutable revision already exists.
    AlreadyStored(RevisionStoreReceipt),
}

/// Result of exact unknown-outcome recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryResult {
    /// Exact write was durably present and activated.
    Applied(RevisionStoreReceipt),
    /// Exact write is absent and the pending state was removed for retry.
    NotApplied,
    /// Readback contradicted the prepared intent and the key was quarantined.
    Quarantined,
}

/// Finite immutable revision-store state machine.
#[derive(Clone, Debug)]
pub struct RevisionStore {
    limits: RevisionStoreLimits,
    states: BTreeMap<RevisionKey, RevisionState>,
    operations: Vec<(OpaqueId, Blake3Digest32, RevisionStoreReceipt)>,
}

impl RevisionStore {
    /// Creates an empty finite store kernel.
    pub fn new(limits: RevisionStoreLimits) -> Result<Self, RevisionStoreError> {
        Ok(Self {
            limits: limits.validate()?,
            states: BTreeMap::new(),
            operations: Vec::new(),
        })
    }

    /// Returns the exact state of one source/revision key.
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

    /// Prepares one append-only revision intent or replays an exact active receipt.
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
            if *digest != intent.operation.request_digest() {
                return Err(RevisionStoreError::OperationConflict);
            }
            let mut replay = receipt.clone();
            replay.replayed = true;
            return Ok(PrepareAppendResult::AlreadyStored(replay));
        }
        if self.operations.len() >= self.limits.max_operations
            || self.states.len() >= self.limits.max_revisions
        {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        if let Some(existing) = self.states.get(&intent.key) {
            return match existing {
                RevisionState::Active(record) if exact_record_matches_intent(record, &intent) => {
                    Ok(PrepareAppendResult::AlreadyStored(RevisionStoreReceipt {
                        key: record.key.clone(),
                        operation: record.operation.clone(),
                        content_digest: record.content_digest,
                        ciphertext_digest: record.ciphertext_digest,
                        object_receipt: record.object_receipt.clone(),
                        replayed: true,
                    }))
                }
                RevisionState::Pending(existing) | RevisionState::OutcomeUnknown(existing)
                    if existing.operation == intent.operation =>
                {
                    Ok(PrepareAppendResult::Prepared(existing.clone()))
                }
                RevisionState::Pending(_)
                | RevisionState::OutcomeUnknown(_)
                | RevisionState::Active(_)
                | RevisionState::Quarantined { .. } => {
                    Err(RevisionStoreError::RevisionConflict)
                }
            };
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
                return Ok(RevisionStoreReceipt {
                    key: record.key.clone(),
                    operation: record.operation.clone(),
                    content_digest: record.content_digest,
                    ciphertext_digest: record.ciphertext_digest,
                    object_receipt: record.object_receipt.clone(),
                    replayed: true,
                });
            }
            RevisionState::Active(_) => return Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => return Err(RevisionStoreError::Quarantined),
        };
        let record = record_from_readback(&intent, readback)?;
        let receipt = receipt_from_record(&record, false);
        self.states
            .insert(key.clone(), RevisionState::Active(record));
        self.operations.push((
            operation.operation_id().clone(),
            operation.request_digest(),
            receipt.clone(),
        ));
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
        let Some(readback) = readback else {
            self.states.remove(key);
            return Ok(RecoveryResult::NotApplied);
        };
        match record_from_readback(&intent, readback) {
            Ok(record) => {
                let receipt = receipt_from_record(&record, false);
                self.states
                    .insert(key.clone(), RevisionState::Active(record));
                self.operations.push((
                    operation.operation_id().clone(),
                    operation.request_digest(),
                    receipt.clone(),
                ));
                Ok(RecoveryResult::Applied(receipt))
            }
            Err(RevisionStoreError::ReadbackMismatch)
            | Err(RevisionStoreError::EvidenceMissing)
            | Err(RevisionStoreError::BackendContractViolation) => {
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
    if ciphertext_bytes == 0 || ciphertext_bytes > limits.max_ciphertext_bytes {
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
    Ok(())
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
        authorization_receipt,
        object_receipt,
        operation: intent.operation.clone(),
    })
}

fn receipt_from_record(record: &RevisionRecord, replayed: bool) -> RevisionStoreReceipt {
    RevisionStoreReceipt {
        key: record.key.clone(),
        operation: record.operation.clone(),
        content_digest: record.content_digest,
        ciphertext_digest: record.ciphertext_digest,
        object_receipt: record.object_receipt.clone(),
        replayed,
    }
}

/// Concrete encrypted-object backend contract.
pub trait RevisionObjectBackend {
    /// Concrete backend error.
    type BackendError;

    /// Attempts one atomic immutable encrypted-object write.
    fn write_immutable(
        &mut self,
        intent: &RevisionWriteIntent,
    ) -> Result<(), Self::BackendError>;

    /// Reads exact content-free object metadata after write or unknown outcome.
    fn readback(
        &mut self,
        key: &RevisionKey,
        storage_object_id: &OpaqueId,
    ) -> Result<Option<RevisionObjectReadback>, Self::BackendError>;

    /// Reads exact encrypted bytes for an active record.
    fn read_encrypted(
        &mut self,
        record: &RevisionRecord,
        max_ciphertext_bytes: u64,
    ) -> Result<EncryptedRevisionPayload, Self::BackendError>;

    /// Maps a concrete error without including source or ciphertext bytes.
    fn map_backend_error(error: &Self::BackendError) -> RevisionStoreError;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(revision: u64) -> RevisionKey {
        RevisionKey {
            source_id: OpaqueId::new("source:test").expect("source"),
            revision: NonZeroRevision::new(revision).expect("revision"),
        }
    }

    fn operation(name: &str, digest: u8) -> RevisionOperation {
        RevisionOperation::new(
            OpaqueId::new(format!("revision-operation:{name}"))
                .expect("operation"),
            Blake3Digest32::from_bytes([digest; 32]),
        )
    }

    fn payload(content: u8) -> EncryptedRevisionPayload {
        EncryptedRevisionPayload::new(
            Blake3Digest32::from_bytes([content; 32]),
            3,
            Blake3Digest32::from_bytes([content + 1; 32]),
            vec![content; 12],
            vec![content; 32],
            EncryptionBinding {
                key_reference: OpaqueId::new("secret:revision-key")
                    .expect("key"),
                key_version: NonZeroRevision::new(1).expect("version"),
                cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
            },
            DEFAULT_REVISION_STORE_LIMITS,
        )
        .expect("payload")
    }

    fn intent(revision: u64, name: &str, content: u8) -> RevisionWriteIntent {
        RevisionWriteIntent {
            key: key(revision),
            source_binding_revision: NonZeroRevision::new(1).expect("revision"),
            payload: payload(content),
            storage_object_id: OpaqueId::new(format!("object:{name}"))
                .expect("object"),
            residency_key: OpaqueId::new("residency:local")
                .expect("residency"),
            authorization_receipt: Some(
                ReceiptRef::new(format!("receipt:authorization:{name}"))
                    .expect("receipt"),
            ),
            operation: operation(name, content),
        }
    }

    fn readback(intent: &RevisionWriteIntent) -> RevisionObjectReadback {
        RevisionObjectReadback {
            key: intent.key.clone(),
            storage_object_id: intent.storage_object_id.clone(),
            ciphertext_digest: intent.payload.ciphertext_digest,
            ciphertext_bytes: u64::try_from(intent.payload.ciphertext_len())
                .expect("length"),
            plaintext_digest: intent.payload.plaintext_digest,
            plaintext_bytes: intent.payload.plaintext_bytes,
            encryption: intent.payload.encryption.clone(),
            readback_verified: true,
            object_receipt: Some(
                ReceiptRef::new("receipt:object").expect("receipt"),
            ),
        }
    }

    #[test]
    fn encrypted_payload_debug_never_dumps_ciphertext() {
        let payload = payload(7);
        let debug = format!("{payload:?}");
        assert!(!debug.contains("[7, 7"));
        assert!(debug.contains("encrypted bytes"));
    }

    #[test]
    fn first_revision_must_be_one_and_revisions_are_sequential() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        assert_eq!(
            store.prepare_append(intent(2, "two", 2)),
            Err(RevisionStoreError::RevisionSequenceInvalid)
        );
        let first = intent(1, "one", 1);
        store.prepare_append(first.clone()).expect("prepare");
        store
            .confirm_append(&first.key, &first.operation, readback(&first))
            .expect("confirm");
        assert!(matches!(
            store.prepare_append(intent(2, "two", 2)),
            Ok(PrepareAppendResult::Prepared(_))
        ));
    }

    #[test]
    fn exact_active_revision_replays_without_rewrite() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .confirm_append(&intent.key, &intent.operation, readback(&intent))
            .expect("confirm");
        let PrepareAppendResult::AlreadyStored(receipt) = store
            .prepare_append(intent)
            .expect("replay")
        else {
            panic!("exact immutable revision must replay")
        };
        assert!(receipt.replayed);
    }

    #[test]
    fn same_revision_with_other_content_is_conflict() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let first = intent(1, "one", 1);
        store.prepare_append(first.clone()).expect("prepare");
        store
            .confirm_append(&first.key, &first.operation, readback(&first))
            .expect("confirm");
        assert_eq!(
            store.prepare_append(intent(1, "other", 9)),
            Err(RevisionStoreError::RevisionConflict)
        );
    }

    #[test]
    fn unknown_write_is_not_active_success() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .mark_outcome_unknown(&intent.key, &intent.operation)
            .expect("unknown");
        assert_eq!(
            store.active_record(&intent.key),
            Err(RevisionStoreError::OutcomeUnknown)
        );
    }

    #[test]
    fn exact_readback_recovers_unknown_write() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .mark_outcome_unknown(&intent.key, &intent.operation)
            .expect("unknown");
        assert!(matches!(
            store
                .recover_unknown(
                    &intent.key,
                    &intent.operation,
                    Some(readback(&intent)),
                )
                .expect("recover"),
            RecoveryResult::Applied(_)
        ));
        assert!(store.active_record(&intent.key).is_ok());
    }

    #[test]
    fn absent_readback_removes_unknown_intent_for_retry() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .mark_outcome_unknown(&intent.key, &intent.operation)
            .expect("unknown");
        assert_eq!(
            store
                .recover_unknown(&intent.key, &intent.operation, None)
                .expect("recover"),
            RecoveryResult::NotApplied
        );
        assert_eq!(
            store.state(&intent.key),
            Err(RevisionStoreError::RevisionNotFound)
        );
    }

    #[test]
    fn contradictory_readback_quarantines_revision() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .mark_outcome_unknown(&intent.key, &intent.operation)
            .expect("unknown");
        let mut wrong = readback(&intent);
        wrong.ciphertext_digest = Blake3Digest32::from_bytes([99; 32]);
        assert_eq!(
            store
                .recover_unknown(&intent.key, &intent.operation, Some(wrong))
                .expect("recover"),
            RecoveryResult::Quarantined
        );
        assert_eq!(
            store.active_record(&intent.key),
            Err(RevisionStoreError::Quarantined)
        );
    }

    #[test]
    fn operation_id_reuse_with_other_request_digest_is_rejected() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let first = intent(1, "same", 1);
        store.prepare_append(first.clone()).expect("prepare");
        store
            .confirm_append(&first.key, &first.operation, readback(&first))
            .expect("confirm");
        let mut second = intent(2, "second", 2);
        second.operation = RevisionOperation::new(
            first.operation.operation_id().clone(),
            Blake3Digest32::from_bytes([88; 32]),
        );
        assert_eq!(
            store.prepare_append(second),
            Err(RevisionStoreError::OperationConflict)
        );
    }
}
