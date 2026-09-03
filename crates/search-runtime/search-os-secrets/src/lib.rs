//! Opaque secret references, encrypted-record lifecycle, and short-lived leases.
//!
//! The package performs no platform I/O and makes no encryption claim. A
//! Windows adapter supplies authenticated encrypted reads/writes/deletes through
//! the existing ports. Plaintext exists only in a non-clone finite lease.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::BTreeMap;

use search_contracts::{
    Blake3Digest32, InstallationId, InstallationIncarnationId, NonZeroRevision,
    OpaqueId, ReceiptRef,
};
use search_ports::{MonotonicInstant, MutationIdentity};

/// Conservative default finite limits for the secret catalog.
pub const DEFAULT_SECRET_LIMITS: SecretLimits = SecretLimits {
    max_records: 256,
    max_ciphertext_bytes: 1_048_576,
    max_plaintext_lease_bytes: 1_048_576,
    max_operations: 4_096,
};

/// Closed content-free secret failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SecretError {
    /// Secret reference is absent.
    NotFound,
    /// Secret reference already exists.
    AlreadyExists,
    /// User, installation, incarnation, or purpose binding differs.
    BindingMismatch,
    /// Secret is not active and cannot be leased.
    NotLeaseable,
    /// Mutation identity was reused with another request digest.
    OperationConflict,
    /// Version did not advance exactly once.
    VersionMismatch,
    /// Durable record revision did not advance exactly once.
    RecordRevisionMismatch,
    /// Finite catalog or operation capacity was exhausted.
    CapacityExceeded,
    /// Ciphertext or plaintext was empty.
    EmptySecret,
    /// Encrypted or plaintext material exceeds its finite limit.
    SecretTooLarge,
    /// Lease timestamps are invalid or the lease is expired.
    InvalidLeaseWindow,
    /// Durable write/readback differs from the prepared mutation.
    ReadbackMismatch,
    /// Required durable readback evidence is absent.
    RecoveryEvidenceMissing,
    /// A possible external mutation requires exact recovery.
    OutcomeUnknown,
    /// Contradictory state requires quarantine.
    Quarantined,
    /// Requested lifecycle transition is invalid.
    InvalidTransition,
    /// A shared version or revision cannot advance.
    ContractExhausted,
}

impl SecretError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotFound => "SECRET_NOT_FOUND",
            Self::AlreadyExists => "SECRET_ALREADY_EXISTS",
            Self::BindingMismatch => "SECRET_BINDING_MISMATCH",
            Self::NotLeaseable => "SECRET_NOT_LEASEABLE",
            Self::OperationConflict => "SECRET_OPERATION_CONFLICT",
            Self::VersionMismatch => "SECRET_VERSION_MISMATCH",
            Self::RecordRevisionMismatch => "SECRET_RECORD_REVISION_MISMATCH",
            Self::CapacityExceeded => "SECRET_CAPACITY_EXCEEDED",
            Self::EmptySecret => "SECRET_EMPTY",
            Self::SecretTooLarge => "SECRET_TOO_LARGE",
            Self::InvalidLeaseWindow => "SECRET_LEASE_INVALID",
            Self::ReadbackMismatch => "SECRET_READBACK_MISMATCH",
            Self::RecoveryEvidenceMissing => "SECRET_RECOVERY_EVIDENCE_MISSING",
            Self::OutcomeUnknown => "SECRET_OUTCOME_UNKNOWN",
            Self::Quarantined => "SECRET_QUARANTINED",
            Self::InvalidTransition => "SECRET_INVALID_TRANSITION",
            Self::ContractExhausted => "SECRET_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for SecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SecretError {}

/// Finite catalog and lease limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecretLimits {
    /// Maximum encrypted records.
    pub max_records: usize,
    /// Maximum encrypted bytes in one record.
    pub max_ciphertext_bytes: usize,
    /// Maximum plaintext bytes in one lease.
    pub max_plaintext_lease_bytes: usize,
    /// Maximum retained mutation identities.
    pub max_operations: usize,
}

impl SecretLimits {
    /// Validates that every finite dimension is non-zero.
    pub const fn validate(self) -> Result<Self, SecretError> {
        if self.max_records == 0
            || self.max_ciphertext_bytes == 0
            || self.max_plaintext_lease_bytes == 0
            || self.max_operations == 0
        {
            Err(SecretError::CapacityExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Exact authority tuple for one secret.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SecretBinding {
    installation_id: InstallationId,
    installation_incarnation_id: InstallationIncarnationId,
    user_scope_digest: Blake3Digest32,
    purpose: OpaqueId,
}

impl SecretBinding {
    /// Creates an exact user/install/incarnation/purpose binding.
    #[must_use]
    pub const fn new(
        installation_id: InstallationId,
        installation_incarnation_id: InstallationIncarnationId,
        user_scope_digest: Blake3Digest32,
        purpose: OpaqueId,
    ) -> Self {
        Self {
            installation_id,
            installation_incarnation_id,
            user_scope_digest,
            purpose,
        }
    }

    /// Stable installation identifier.
    #[must_use]
    pub const fn installation_id(&self) -> InstallationId {
        self.installation_id
    }

    /// Current installation-incarnation identifier.
    #[must_use]
    pub const fn installation_incarnation_id(&self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    /// Digest of the local user/principal scope.
    #[must_use]
    pub const fn user_scope_digest(&self) -> Blake3Digest32 {
        self.user_scope_digest
    }

    /// Closed capability-owned purpose identifier.
    #[must_use]
    pub const fn purpose(&self) -> &OpaqueId {
        &self.purpose
    }
}

/// Opaque bound secret reference with a monotone version.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SecretReference {
    id: OpaqueId,
    binding: SecretBinding,
    version: NonZeroRevision,
}

impl SecretReference {
    /// Creates an opaque bound reference.
    #[must_use]
    pub const fn new(
        id: OpaqueId,
        binding: SecretBinding,
        version: NonZeroRevision,
    ) -> Self {
        Self {
            id,
            binding,
            version,
        }
    }

    /// Opaque identifier; consumers must not parse it.
    #[must_use]
    pub const fn id(&self) -> &OpaqueId {
        &self.id
    }

    /// Exact authority binding.
    #[must_use]
    pub const fn binding(&self) -> &SecretBinding {
        &self.binding
    }

    /// Monotone secret version.
    #[must_use]
    pub const fn version(&self) -> NonZeroRevision {
        self.version
    }

    fn next_version(&self) -> Result<NonZeroRevision, SecretError> {
        self.version
            .checked_next()
            .map_err(|_| SecretError::ContractExhausted)
    }
}

/// Immutable mutation identity plus digest of canonical request bytes.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SecretOperation {
    mutation: MutationIdentity,
    request_digest: Blake3Digest32,
}

impl SecretOperation {
    /// Creates a replay fence.
    #[must_use]
    pub const fn new(
        mutation: MutationIdentity,
        request_digest: Blake3Digest32,
    ) -> Self {
        Self {
            mutation,
            request_digest,
        }
    }

    /// Shared immutable mutation identity.
    #[must_use]
    pub const fn mutation(&self) -> &MutationIdentity {
        &self.mutation
    }

    /// Digest of canonical request bytes.
    #[must_use]
    pub const fn request_digest(&self) -> Blake3Digest32 {
        self.request_digest
    }

    /// Returns whether both operation identity and request digest match.
    #[must_use]
    pub fn is_same_request(&self, other: &Self) -> bool {
        self == other
    }
}

/// Owned finite encrypted payload with redacted formatting.
#[derive(Clone, Eq, PartialEq)]
pub struct EncryptedPayload(Vec<u8>);

impl EncryptedPayload {
    /// Validates non-empty finite encrypted bytes.
    pub fn new(bytes: Vec<u8>, max_bytes: usize) -> Result<Self, SecretError> {
        if bytes.is_empty() {
            return Err(SecretError::EmptySecret);
        }
        if bytes.len() > max_bytes {
            return Err(SecretError::SecretTooLarge);
        }
        Ok(Self(bytes))
    }

    /// Exact encrypted bytes for the platform adapter.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Exact encrypted length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether the encrypted payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for EncryptedPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("EncryptedPayload")
            .field(&format_args!("<{} encrypted bytes>", self.0.len()))
            .finish()
    }
}

/// Prepared rotation retained across a platform mutation boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingRotation {
    operation: SecretOperation,
    replacement_reference: SecretReference,
    replacement_ciphertext: EncryptedPayload,
    replacement_ciphertext_digest: Blake3Digest32,
    replacement_record_revision: NonZeroRevision,
}

/// Prepared deletion retained across a platform mutation boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingDelete {
    operation: SecretOperation,
}

/// Unknown platform mutation retained for exact readback recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnknownSecretMutation {
    /// A replacement ciphertext may have been committed.
    Rotation(PendingRotation),
    /// Deletion may have completed.
    Delete(PendingDelete),
}

/// Durable encrypted-record lifecycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecretRecordState {
    /// Current encrypted version is active and leaseable.
    Active,
    /// Replacement write is prepared but unconfirmed.
    RotationPending(PendingRotation),
    /// Deletion is prepared but unconfirmed.
    DeletePending(PendingDelete),
    /// A platform mutation requires exact readback.
    OutcomeUnknown(UnknownSecretMutation),
    /// Reference is durably deleted and not leaseable.
    Deleted(SecretOperation),
    /// Contradictory state blocks access.
    Quarantined(SecretError),
}

/// Encrypted durable record. Plaintext is never stored here.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretRecord {
    reference: SecretReference,
    ciphertext: EncryptedPayload,
    ciphertext_digest: Blake3Digest32,
    record_revision: NonZeroRevision,
    state: SecretRecordState,
    last_operation: SecretOperation,
}

impl SecretRecord {
    /// Creates an active encrypted record.
    #[must_use]
    pub const fn new_active(
        reference: SecretReference,
        ciphertext: EncryptedPayload,
        ciphertext_digest: Blake3Digest32,
        record_revision: NonZeroRevision,
        operation: SecretOperation,
    ) -> Self {
        Self {
            reference,
            ciphertext,
            ciphertext_digest,
            record_revision,
            state: SecretRecordState::Active,
            last_operation: operation,
        }
    }

    /// Opaque bound reference.
    #[must_use]
    pub const fn reference(&self) -> &SecretReference {
        &self.reference
    }

    /// Encrypted payload.
    #[must_use]
    pub const fn ciphertext(&self) -> &EncryptedPayload {
        &self.ciphertext
    }

    /// Digest of exact encrypted bytes.
    #[must_use]
    pub const fn ciphertext_digest(&self) -> Blake3Digest32 {
        self.ciphertext_digest
    }

    /// Durable record revision.
    #[must_use]
    pub const fn record_revision(&self) -> NonZeroRevision {
        self.record_revision
    }

    /// Current lifecycle.
    #[must_use]
    pub const fn state(&self) -> &SecretRecordState {
        &self.state
    }

    /// Most recent mutation operation.
    #[must_use]
    pub const fn last_operation(&self) -> &SecretOperation {
        &self.last_operation
    }
}

impl fmt::Debug for SecretRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretRecord")
            .field("reference", &self.reference)
            .field("ciphertext", &self.ciphertext)
            .field("ciphertext_digest", &self.ciphertext_digest)
            .field("record_revision", &self.record_revision)
            .field("state", &self.state)
            .field("last_operation", &self.last_operation)
            .finish()
    }
}

/// Platform effect authorized by a prepared secret mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecretMutationEffect {
    /// Atomically replace encrypted bytes for the exact reference.
    WriteEncrypted {
        /// Replacement opaque reference.
        reference: SecretReference,
        /// Replacement encrypted bytes.
        ciphertext: EncryptedPayload,
        /// Replacement encrypted-byte digest.
        ciphertext_digest: Blake3Digest32,
        /// Immutable mutation operation.
        operation: SecretOperation,
    },
    /// Delete the exact reference.
    Delete {
        /// Exact reference to delete.
        reference: SecretReference,
        /// Immutable mutation operation.
        operation: SecretOperation,
    },
}

/// Exact durable write readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretWriteReadback {
    /// Opaque reference observed after the write.
    pub reference: SecretReference,
    /// Digest of exact encrypted bytes observed.
    pub ciphertext_digest: Blake3Digest32,
    /// Durable record revision observed.
    pub record_revision: NonZeroRevision,
    /// Immutable operation observed in durable metadata.
    pub operation: SecretOperation,
    /// Content-free durable readback receipt.
    pub durable_receipt: Option<ReceiptRef>,
}

/// Exact durable delete readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretDeleteReadback {
    /// Whether the exact reference is verified absent.
    pub reference_absent: bool,
    /// Immutable operation observed in durable tombstone or audit metadata.
    pub operation: SecretOperation,
    /// Content-free durable readback receipt.
    pub durable_receipt: Option<ReceiptRef>,
}

/// Content-free transition receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretTransitionReceipt {
    /// Opaque reference after the transition.
    pub reference: SecretReference,
    /// Immutable operation identity.
    pub operation: SecretOperation,
    /// Durable record revision.
    pub record_revision: NonZeroRevision,
    /// Platform readback receipt.
    pub durable_receipt: ReceiptRef,
}

/// Finite deterministic encrypted-record catalog.
#[derive(Clone, Debug)]
pub struct SecretCatalog {
    limits: SecretLimits,
    records: BTreeMap<OpaqueId, SecretRecord>,
    operations: BTreeMap<OpaqueId, Blake3Digest32>,
}

impl SecretCatalog {
    /// Creates an empty catalog with conservative limits.
    pub fn new() -> Result<Self, SecretError> {
        Self::with_limits(DEFAULT_SECRET_LIMITS)
    }

    /// Creates an empty catalog with explicit finite limits.
    pub fn with_limits(limits: SecretLimits) -> Result<Self, SecretError> {
        Ok(Self {
            limits: limits.validate()?,
            records: BTreeMap::new(),
            operations: BTreeMap::new(),
        })
    }

    /// Returns one exact encrypted record.
    pub fn get(&self, id: &OpaqueId) -> Result<&SecretRecord, SecretError> {
        self.records.get(id).ok_or(SecretError::NotFound)
    }

    /// Number of retained records, including tombstones.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether the catalog has no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Registers one newly created active encrypted record.
    pub fn create(&mut self, record: SecretRecord) -> Result<(), SecretError> {
        if self.records.contains_key(record.reference().id()) {
            return Err(SecretError::AlreadyExists);
        }
        if self.records.len() >= self.limits.max_records {
            return Err(SecretError::CapacityExceeded);
        }
        validate_ciphertext(&record.ciphertext, self.limits)?;
        self.register_operation(record.last_operation())?;
        self.records.insert(record.reference.id.clone(), record);
        Ok(())
    }

    /// Prepares an exact one-version rotation.
    pub fn prepare_rotation(
        &mut self,
        id: &OpaqueId,
        binding: &SecretBinding,
        replacement_version: NonZeroRevision,
        replacement_ciphertext: EncryptedPayload,
        replacement_ciphertext_digest: Blake3Digest32,
        replacement_record_revision: NonZeroRevision,
        operation: SecretOperation,
    ) -> Result<SecretMutationEffect, SecretError> {
        validate_ciphertext(&replacement_ciphertext, self.limits)?;
        let current = self.get(id)?.clone();
        if current.reference.binding() != binding {
            return Err(SecretError::BindingMismatch);
        }
        if !matches!(current.state(), SecretRecordState::Active) {
            return Err(SecretError::InvalidTransition);
        }
        if replacement_version != current.reference.next_version()? {
            return Err(SecretError::VersionMismatch);
        }
        verify_next_record_revision(current.record_revision, replacement_record_revision)?;
        self.register_operation(&operation)?;
        let replacement_reference = SecretReference::new(
            current.reference.id.clone(),
            current.reference.binding.clone(),
            replacement_version,
        );
        let pending = PendingRotation {
            operation: operation.clone(),
            replacement_reference: replacement_reference.clone(),
            replacement_ciphertext: replacement_ciphertext.clone(),
            replacement_ciphertext_digest,
            replacement_record_revision,
        };
        self.records
            .get_mut(id)
            .ok_or(SecretError::NotFound)?
            .state = SecretRecordState::RotationPending(pending);
        Ok(SecretMutationEffect::WriteEncrypted {
            reference: replacement_reference,
            ciphertext: replacement_ciphertext,
            ciphertext_digest: replacement_ciphertext_digest,
            operation,
        })
    }

    /// Confirms a prepared rotation by exact durable readback.
    pub fn confirm_rotation(
        &mut self,
        id: &OpaqueId,
        readback: &SecretWriteReadback,
    ) -> Result<SecretTransitionReceipt, SecretError> {
        self.finish_rotation(id, readback, false)
    }

    /// Recovers an outcome-unknown rotation by exact durable readback.
    pub fn recover_rotation(
        &mut self,
        id: &OpaqueId,
        readback: &SecretWriteReadback,
    ) -> Result<SecretTransitionReceipt, SecretError> {
        self.finish_rotation(id, readback, true)
    }

    fn finish_rotation(
        &mut self,
        id: &OpaqueId,
        readback: &SecretWriteReadback,
        require_unknown: bool,
    ) -> Result<SecretTransitionReceipt, SecretError> {
        let pending = match self.get(id)?.state() {
            SecretRecordState::RotationPending(pending) if !require_unknown => pending.clone(),
            SecretRecordState::OutcomeUnknown(UnknownSecretMutation::Rotation(pending))
                if require_unknown => pending.clone(),
            _ => return Err(SecretError::InvalidTransition),
        };
        verify_rotation_readback(&pending, readback)?;
        let durable_receipt = readback
            .durable_receipt
            .clone()
            .ok_or(SecretError::RecoveryEvidenceMissing)?;
        let record = self.records.get_mut(id).ok_or(SecretError::NotFound)?;
        record.reference = pending.replacement_reference.clone();
        record.ciphertext = pending.replacement_ciphertext;
        record.ciphertext_digest = pending.replacement_ciphertext_digest;
        record.record_revision = pending.replacement_record_revision;
        record.last_operation = pending.operation.clone();
        record.state = SecretRecordState::Active;
        Ok(SecretTransitionReceipt {
            reference: record.reference.clone(),
            operation: pending.operation,
            record_revision: record.record_revision,
            durable_receipt,
        })
    }

    /// Prepares exact deletion for an active reference.
    pub fn prepare_delete(
        &mut self,
        id: &OpaqueId,
        binding: &SecretBinding,
        operation: SecretOperation,
    ) -> Result<SecretMutationEffect, SecretError> {
        let current = self.get(id)?.clone();
        if current.reference.binding() != binding {
            return Err(SecretError::BindingMismatch);
        }
        if !matches!(current.state(), SecretRecordState::Active) {
            return Err(SecretError::InvalidTransition);
        }
        self.register_operation(&operation)?;
        self.records
            .get_mut(id)
            .ok_or(SecretError::NotFound)?
            .state = SecretRecordState::DeletePending(PendingDelete {
            operation: operation.clone(),
        });
        Ok(SecretMutationEffect::Delete {
            reference: current.reference,
            operation,
        })
    }

    /// Confirms exact deletion by durable absence readback.
    pub fn confirm_delete(
        &mut self,
        id: &OpaqueId,
        readback: &SecretDeleteReadback,
    ) -> Result<SecretTransitionReceipt, SecretError> {
        self.finish_delete(id, readback, false)
    }

    /// Recovers an outcome-unknown delete by durable absence readback.
    pub fn recover_delete(
        &mut self,
        id: &OpaqueId,
        readback: &SecretDeleteReadback,
    ) -> Result<SecretTransitionReceipt, SecretError> {
        self.finish_delete(id, readback, true)
    }

    fn finish_delete(
        &mut self,
        id: &OpaqueId,
        readback: &SecretDeleteReadback,
        require_unknown: bool,
    ) -> Result<SecretTransitionReceipt, SecretError> {
        let pending = match self.get(id)?.state() {
            SecretRecordState::DeletePending(pending) if !require_unknown => pending.clone(),
            SecretRecordState::OutcomeUnknown(UnknownSecretMutation::Delete(pending))
                if require_unknown => pending.clone(),
            _ => return Err(SecretError::InvalidTransition),
        };
        if !pending.operation.is_same_request(&readback.operation) {
            return Err(SecretError::ReadbackMismatch);
        }
        if !readback.reference_absent {
            return Err(SecretError::OutcomeUnknown);
        }
        let durable_receipt = readback
            .durable_receipt
            .clone()
            .ok_or(SecretError::RecoveryEvidenceMissing)?;
        let record = self.records.get_mut(id).ok_or(SecretError::NotFound)?;
        record.ciphertext.0.fill(0);
        record.ciphertext.0.clear();
        record.last_operation = pending.operation.clone();
        record.state = SecretRecordState::Deleted(pending.operation.clone());
        Ok(SecretTransitionReceipt {
            reference: record.reference.clone(),
            operation: pending.operation,
            record_revision: record.record_revision,
            durable_receipt,
        })
    }

    /// Marks the exact prepared mutation as outcome-unknown.
    pub fn mark_outcome_unknown(
        &mut self,
        id: &OpaqueId,
        operation: &SecretOperation,
    ) -> Result<(), SecretError> {
        let state = self.get(id)?.state().clone();
        let unknown = match state {
            SecretRecordState::RotationPending(pending)
                if pending.operation.is_same_request(operation) =>
            {
                UnknownSecretMutation::Rotation(pending)
            }
            SecretRecordState::DeletePending(pending)
                if pending.operation.is_same_request(operation) =>
            {
                UnknownSecretMutation::Delete(pending)
            }
            SecretRecordState::OutcomeUnknown(unknown) => {
                let expected = match &unknown {
                    UnknownSecretMutation::Rotation(pending) => &pending.operation,
                    UnknownSecretMutation::Delete(pending) => &pending.operation,
                };
                if expected.is_same_request(operation) {
                    return Ok(());
                }
                return Err(SecretError::OperationConflict);
            }
            _ => return Err(SecretError::InvalidTransition),
        };
        self.records
            .get_mut(id)
            .ok_or(SecretError::NotFound)?
            .state = SecretRecordState::OutcomeUnknown(unknown);
        Ok(())
    }

    /// Explicitly quarantines one reference.
    pub fn quarantine(
        &mut self,
        id: &OpaqueId,
        reason: SecretError,
    ) -> Result<(), SecretError> {
        self.records
            .get_mut(id)
            .ok_or(SecretError::NotFound)?
            .state = SecretRecordState::Quarantined(reason);
        Ok(())
    }

    fn register_operation(&mut self, operation: &SecretOperation) -> Result<(), SecretError> {
        let id = operation.mutation().operation_id.clone();
        if let Some(existing) = self.operations.get(&id) {
            if *existing == operation.request_digest() {
                return Ok(());
            }
            return Err(SecretError::OperationConflict);
        }
        if self.operations.len() >= self.limits.max_operations {
            return Err(SecretError::CapacityExceeded);
        }
        self.operations.insert(id, operation.request_digest());
        Ok(())
    }
}

fn validate_ciphertext(
    ciphertext: &EncryptedPayload,
    limits: SecretLimits,
) -> Result<(), SecretError> {
    if ciphertext.is_empty() {
        Err(SecretError::EmptySecret)
    } else if ciphertext.len() > limits.max_ciphertext_bytes {
        Err(SecretError::SecretTooLarge)
    } else {
        Ok(())
    }
}

fn verify_next_record_revision(
    current: NonZeroRevision,
    proposed: NonZeroRevision,
) -> Result<(), SecretError> {
    let expected = current
        .checked_next()
        .map_err(|_| SecretError::ContractExhausted)?;
    if proposed == expected {
        Ok(())
    } else {
        Err(SecretError::RecordRevisionMismatch)
    }
}

fn verify_rotation_readback(
    pending: &PendingRotation,
    readback: &SecretWriteReadback,
) -> Result<(), SecretError> {
    if pending.replacement_reference != readback.reference
        || pending.replacement_ciphertext_digest != readback.ciphertext_digest
        || pending.replacement_record_revision != readback.record_revision
        || !pending.operation.is_same_request(&readback.operation)
    {
        Err(SecretError::ReadbackMismatch)
    } else {
        Ok(())
    }
}

/// Short-lived plaintext lease.
///
/// The type is non-clone, redacts formatting, exposes bytes only to a callback,
/// and overwrites its owned buffer on drop.
pub struct SecretLease {
    reference: SecretReference,
    issued_at: MonotonicInstant,
    expires_at: MonotonicInstant,
    plaintext: Vec<u8>,
}

impl SecretLease {
    /// Issues a finite plaintext lease for an exact active record.
    pub fn issue(
        record: &SecretRecord,
        requested_binding: &SecretBinding,
        issued_at: MonotonicInstant,
        expires_at: MonotonicInstant,
        plaintext: Vec<u8>,
        limits: SecretLimits,
    ) -> Result<Self, SecretError> {
        if record.reference.binding() != requested_binding {
            return Err(SecretError::BindingMismatch);
        }
        if !matches!(record.state(), SecretRecordState::Active) {
            return Err(SecretError::NotLeaseable);
        }
        if plaintext.is_empty() {
            return Err(SecretError::EmptySecret);
        }
        if plaintext.len() > limits.max_plaintext_lease_bytes {
            return Err(SecretError::SecretTooLarge);
        }
        if expires_at <= issued_at {
            return Err(SecretError::InvalidLeaseWindow);
        }
        Ok(Self {
            reference: record.reference.clone(),
            issued_at,
            expires_at,
            plaintext,
        })
    }

    /// Opaque reference bound to this lease.
    #[must_use]
    pub const fn reference(&self) -> &SecretReference {
        &self.reference
    }

    /// Returns whether use at an explicit instant is permitted.
    #[must_use]
    pub const fn is_valid_at(&self, now: MonotonicInstant) -> bool {
        now.ticks() >= self.issued_at.ticks() && now.ticks() < self.expires_at.ticks()
    }

    /// Exposes plaintext only for the duration of the supplied callback.
    pub fn with_secret<T>(
        &self,
        now: MonotonicInstant,
        use_secret: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretError> {
        if !self.is_valid_at(now) {
            return Err(SecretError::InvalidLeaseWindow);
        }
        Ok(use_secret(&self.plaintext))
    }
}

impl fmt::Debug for SecretLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretLease")
            .field("reference", &self.reference)
            .field("issued_at", &self.issued_at)
            .field("expires_at", &self.expires_at)
            .field("plaintext", &"<redacted>")
            .finish()
    }
}

impl Drop for SecretLease {
    fn drop(&mut self) {
        self.plaintext.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_ports::IdempotencyClass;

    fn binding() -> SecretBinding {
        SecretBinding::new(
            InstallationId::from_bytes([1; 16]),
            InstallationIncarnationId::from_bytes([2; 16]),
            Blake3Digest32::from_bytes([3; 32]),
            OpaqueId::new("secret-purpose:provider-auth").expect("purpose"),
        )
    }

    fn operation(name: &str, digest: u8) -> SecretOperation {
        SecretOperation::new(
            MutationIdentity::new(
                OpaqueId::new(format!("secret-operation:{name}")).expect("operation"),
                IdempotencyClass::RetrySameIdentity,
            ),
            Blake3Digest32::from_bytes([digest; 32]),
        )
    }

    fn record() -> SecretRecord {
        SecretRecord::new_active(
            SecretReference::new(
                OpaqueId::new("secret:provider-auth").expect("reference"),
                binding(),
                NonZeroRevision::new(1).expect("version"),
            ),
            EncryptedPayload::new(vec![7, 8, 9], 64).expect("ciphertext"),
            Blake3Digest32::from_bytes([4; 32]),
            NonZeroRevision::new(1).expect("revision"),
            operation("create", 5),
        )
    }

    #[test]
    fn record_debug_does_not_dump_ciphertext() {
        let record = record();
        let debug = format!("{record:?}");
        assert!(!debug.contains("[7, 8, 9]"));
        assert!(debug.contains("encrypted bytes"));
    }

    #[test]
    fn lease_debug_does_not_dump_plaintext() {
        let record = record();
        let lease = SecretLease::issue(
            &record,
            &binding(),
            MonotonicInstant::from_ticks(1),
            MonotonicInstant::from_ticks(2),
            b"top-secret".to_vec(),
            DEFAULT_SECRET_LIMITS,
        )
        .expect("lease");
        let debug = format!("{lease:?}");
        assert!(!debug.contains("top-secret"));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn cross_binding_lease_is_denied() {
        let record = record();
        let other = SecretBinding::new(
            InstallationId::from_bytes([9; 16]),
            InstallationIncarnationId::from_bytes([2; 16]),
            Blake3Digest32::from_bytes([3; 32]),
            OpaqueId::new("secret-purpose:provider-auth").expect("purpose"),
        );
        assert!(matches!(
            SecretLease::issue(
                &record,
                &other,
                MonotonicInstant::from_ticks(1),
                MonotonicInstant::from_ticks(2),
                vec![1],
                DEFAULT_SECRET_LIMITS,
            ),
            Err(SecretError::BindingMismatch)
        ));
    }

    #[test]
    fn rotation_advances_version_and_record_revision_exactly_once() {
        let mut catalog = SecretCatalog::new().expect("catalog");
        let record = record();
        let id = record.reference().id().clone();
        catalog.create(record).expect("create");
        let operation = operation("rotate", 6);
        catalog
            .prepare_rotation(
                &id,
                &binding(),
                NonZeroRevision::new(2).expect("version"),
                EncryptedPayload::new(vec![10, 11], 64).expect("ciphertext"),
                Blake3Digest32::from_bytes([7; 32]),
                NonZeroRevision::new(2).expect("revision"),
                operation.clone(),
            )
            .expect("prepare");
        let receipt = catalog
            .confirm_rotation(
                &id,
                &SecretWriteReadback {
                    reference: SecretReference::new(
                        id.clone(),
                        binding(),
                        NonZeroRevision::new(2).expect("version"),
                    ),
                    ciphertext_digest: Blake3Digest32::from_bytes([7; 32]),
                    record_revision: NonZeroRevision::new(2).expect("revision"),
                    operation,
                    durable_receipt: Some(
                        ReceiptRef::new("receipt:rotation").expect("receipt"),
                    ),
                },
            )
            .expect("confirm");
        assert_eq!(receipt.reference.version().get(), 2);
        assert!(matches!(catalog.get(&id).expect("record").state(), SecretRecordState::Active));
    }

    #[test]
    fn mutation_timeout_is_not_reported_as_success() {
        let mut catalog = SecretCatalog::new().expect("catalog");
        let record = record();
        let id = record.reference().id().clone();
        catalog.create(record).expect("create");
        let operation = operation("delete", 8);
        catalog
            .prepare_delete(&id, &binding(), operation.clone())
            .expect("prepare");
        catalog
            .mark_outcome_unknown(&id, &operation)
            .expect("unknown");
        assert!(matches!(
            catalog.get(&id).expect("record").state(),
            SecretRecordState::OutcomeUnknown(UnknownSecretMutation::Delete(_))
        ));
    }

    #[test]
    fn delete_requires_exact_absence_readback() {
        let mut catalog = SecretCatalog::new().expect("catalog");
        let record = record();
        let id = record.reference().id().clone();
        catalog.create(record).expect("create");
        let operation = operation("delete", 8);
        catalog
            .prepare_delete(&id, &binding(), operation.clone())
            .expect("prepare");
        assert_eq!(
            catalog.confirm_delete(
                &id,
                &SecretDeleteReadback {
                    reference_absent: false,
                    operation,
                    durable_receipt: Some(
                        ReceiptRef::new("receipt:delete").expect("receipt"),
                    ),
                },
            ),
            Err(SecretError::OutcomeUnknown)
        );
    }

    #[test]
    fn operation_reuse_with_other_payload_is_rejected() {
        let mutation = MutationIdentity::new(
            OpaqueId::new("secret-operation:collision").expect("operation"),
            IdempotencyClass::RetrySameIdentity,
        );
        let mut catalog = SecretCatalog::new().expect("catalog");
        let first = SecretRecord::new_active(
            SecretReference::new(
                OpaqueId::new("secret:first").expect("reference"),
                binding(),
                NonZeroRevision::new(1).expect("version"),
            ),
            EncryptedPayload::new(vec![1], 64).expect("ciphertext"),
            Blake3Digest32::from_bytes([1; 32]),
            NonZeroRevision::new(1).expect("revision"),
            SecretOperation::new(mutation.clone(), Blake3Digest32::from_bytes([1; 32])),
        );
        catalog.create(first).expect("first");
        let second = SecretRecord::new_active(
            SecretReference::new(
                OpaqueId::new("secret:second").expect("reference"),
                binding(),
                NonZeroRevision::new(1).expect("version"),
            ),
            EncryptedPayload::new(vec![2], 64).expect("ciphertext"),
            Blake3Digest32::from_bytes([2; 32]),
            NonZeroRevision::new(1).expect("revision"),
            SecretOperation::new(mutation, Blake3Digest32::from_bytes([2; 32])),
        );
        assert_eq!(catalog.create(second), Err(SecretError::OperationConflict));
    }
}
