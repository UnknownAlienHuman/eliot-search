//! Retention leases, purge fencing, tombstones, reclaim, and restore lifecycle.
//!
//! This package is a pure coordinator. Concrete index, object-store, backup,
//! handle, continuation, and control-store adapters execute exact effects and
//! return readback receipts. Logical non-accessibility is distinct from
//! physical secure erasure, and restore never enters indexed serving without
//! complete revalidation.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, ObjectResidencyKeyDigest, OpaqueId,
    PurgeFenceRevision, ReceiptRef, SourceMembershipId, SourceRevisionId,
};

/// Closed retention/purge/restore failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RetentionError {
    /// Policy contains a zero or contradictory finite limit.
    InvalidPolicy,
    /// Lease target or interval is malformed.
    InvalidLease,
    /// Lease identity already exists with another request.
    LeaseConflict,
    /// Lease is not present.
    LeaseNotFound,
    /// Lease is expired, revoked, or released.
    LeaseInactive,
    /// Lease renewal does not advance expiry.
    LeaseRegression,
    /// Finite lease/tombstone/operation capacity was exceeded.
    CapacityExceeded,
    /// Operation identity was reused with another canonical request digest.
    OperationConflict,
    /// Purge target set is empty, duplicated, or exceeds the finite limit.
    InvalidPurgeManifest,
    /// Purge-fence revision is stale or non-contiguous.
    PurgeFenceMismatch,
    /// Purge phase transition is invalid.
    InvalidPurgeTransition,
    /// Required restrictive live-deny receipt is missing or mismatched.
    LiveDenyReceiptMissing,
    /// Handle/continuation invalidation evidence is incomplete.
    InvalidationIncomplete,
    /// Index deletion evidence is incomplete.
    IndexDeletionIncomplete,
    /// Cache deletion evidence is incomplete.
    CacheDeletionIncomplete,
    /// Search-owned object deletion evidence is incomplete.
    ObjectDeletionIncomplete,
    /// Backup/snapshot disposition is unresolved.
    BackupDispositionIncomplete,
    /// External mutation outcome requires exact readback.
    OutcomeUnknown,
    /// Active retention leases still protect at least one target.
    RetentionLeaseActive,
    /// Active epoch/route pins still protect at least one target.
    EpochPinActive,
    /// Reclaim manifest differs from exact retired objects.
    ReclaimManifestMismatch,
    /// Tombstone identity or generation differs.
    TombstoneMismatch,
    /// Restore manifest is malformed or unpaired.
    RestoreManifestInvalid,
    /// Restore has not completed exact object/control/index readback.
    RestoreRevalidationIncomplete,
    /// Restore attempted indexed admission without a validated publication.
    IndexedRestoreNotAdmitted,
    /// Physical secure-erasure claim lacks external evidence.
    SecureEraseEvidenceMissing,
    /// Contradictory state requires quarantine.
    Quarantined,
    /// Shared revision/epoch space is exhausted.
    ContractExhausted,
}

impl RetentionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "RETENTION_POLICY_INVALID",
            Self::InvalidLease => "RETENTION_LEASE_INVALID",
            Self::LeaseConflict => "RETENTION_LEASE_CONFLICT",
            Self::LeaseNotFound => "RETENTION_LEASE_NOT_FOUND",
            Self::LeaseInactive => "RETENTION_LEASE_INACTIVE",
            Self::LeaseRegression => "RETENTION_LEASE_REGRESSION",
            Self::CapacityExceeded => "RETENTION_CAPACITY_EXCEEDED",
            Self::OperationConflict => "RETENTION_OPERATION_CONFLICT",
            Self::InvalidPurgeManifest => "PURGE_MANIFEST_INVALID",
            Self::PurgeFenceMismatch => "PURGE_FENCE_MISMATCH",
            Self::InvalidPurgeTransition => "PURGE_TRANSITION_INVALID",
            Self::LiveDenyReceiptMissing => "PURGE_LIVE_DENY_RECEIPT_MISSING",
            Self::InvalidationIncomplete => "PURGE_INVALIDATION_INCOMPLETE",
            Self::IndexDeletionIncomplete => "PURGE_INDEX_DELETION_INCOMPLETE",
            Self::CacheDeletionIncomplete => "PURGE_CACHE_DELETION_INCOMPLETE",
            Self::ObjectDeletionIncomplete => "PURGE_OBJECT_DELETION_INCOMPLETE",
            Self::BackupDispositionIncomplete => "PURGE_BACKUP_DISPOSITION_INCOMPLETE",
            Self::OutcomeUnknown => "PURGE_OUTCOME_UNKNOWN",
            Self::RetentionLeaseActive => "RECLAIM_RETENTION_LEASE_ACTIVE",
            Self::EpochPinActive => "RECLAIM_EPOCH_PIN_ACTIVE",
            Self::ReclaimManifestMismatch => "RECLAIM_MANIFEST_MISMATCH",
            Self::TombstoneMismatch => "PURGE_TOMBSTONE_MISMATCH",
            Self::RestoreManifestInvalid => "RESTORE_MANIFEST_INVALID",
            Self::RestoreRevalidationIncomplete => "RESTORE_REVALIDATION_INCOMPLETE",
            Self::IndexedRestoreNotAdmitted => "RESTORE_INDEXED_NOT_ADMITTED",
            Self::SecureEraseEvidenceMissing => "PURGE_SECURE_ERASE_EVIDENCE_MISSING",
            Self::Quarantined => "RETENTION_QUARANTINED",
            Self::ContractExhausted => "RETENTION_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for RetentionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RetentionError {}

/// Finite retention subsystem policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    /// Maximum active and historical leases.
    pub max_leases: usize,
    /// Maximum purge targets in one transaction.
    pub max_purge_targets: usize,
    /// Maximum retained tombstones.
    pub max_tombstones: usize,
    /// Maximum operation identities retained for idempotency.
    pub max_operations: usize,
    /// Maximum lease duration.
    pub max_lease_duration_ms: u64,
}

impl RetentionPolicy {
    /// Conservative local baseline.
    pub const BASELINE: Self = Self {
        max_leases: 65_536,
        max_purge_targets: 100_000,
        max_tombstones: 100_000,
        max_operations: 100_000,
        max_lease_duration_ms: 365 * 24 * 60 * 60 * 1_000,
    };

    /// Validates finite non-zero limits.
    pub const fn validate(self) -> Result<Self, RetentionError> {
        if self.max_leases == 0
            || self.max_purge_targets == 0
            || self.max_tombstones == 0
            || self.max_operations == 0
            || self.max_lease_duration_ms == 0
        {
            Err(RetentionError::InvalidPolicy)
        } else {
            Ok(self)
        }
    }
}

/// Closed search-owned retention object kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RetainedObjectKind {
    SourceRevision,
    Materialization,
    UnitSet,
    Projection,
    Handle,
    Continuation,
    EvaluationArtifact,
}

/// Exact object protected by a lease or targeted by purge.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetainedObject {
    /// Stable search-owned object identity.
    pub object_id: OpaqueId,
    /// Closed object kind.
    pub kind: RetainedObjectKind,
    /// Residency domain digest.
    pub residency_digest: ObjectResidencyKeyDigest,
    /// Source membership when the object belongs to one membership.
    pub source_membership_id: Option<SourceMembershipId>,
    /// Source revision when immutable source bytes are involved.
    pub source_revision_id: Option<SourceRevisionId>,
    /// Collection generation for indexed projection objects.
    pub collection_generation_id: Option<CollectionGenerationId>,
    /// Last epoch in which the object may be visible.
    pub last_visible_epoch: Option<Epoch>,
    /// Digest of exact immutable object bytes/metadata.
    pub object_digest: Blake3Digest32,
}

/// Immutable operation identity and canonical request digest.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetentionOperation {
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Digest of exact canonical request bytes.
    pub request_digest: Blake3Digest32,
}

/// Retention lease lifecycle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RetentionLeaseState {
    Active,
    Released,
    Revoked,
    Expired,
}

/// Finite exact retention lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionLease {
    /// Lease identity.
    pub lease_id: OpaqueId,
    /// Principal/job/handle that owns the lease.
    pub owner_id: OpaqueId,
    /// Exact protected object.
    pub target: RetainedObject,
    /// Issue time.
    pub issued_at_ms: u64,
    /// Expiration time.
    pub expires_at_ms: u64,
    /// Current lifecycle.
    pub state: RetentionLeaseState,
    /// Monotone lease revision.
    pub revision: u64,
    /// Last mutation operation.
    pub last_operation: RetentionOperation,
    /// Durable lease receipt.
    pub lease_receipt: ReceiptRef,
}

/// Content-free lease mutation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseMutationReceipt {
    /// Lease identity.
    pub lease_id: OpaqueId,
    /// Revision after the mutation.
    pub revision: u64,
    /// Resulting state.
    pub state: RetentionLeaseState,
    /// Operation identity.
    pub operation_id: OpaqueId,
    /// Durable readback receipt.
    pub readback_receipt: ReceiptRef,
}

/// Finite retention lease catalog.
#[derive(Clone, Debug)]
pub struct RetentionCatalog {
    policy: RetentionPolicy,
    leases: BTreeMap<OpaqueId, RetentionLease>,
    operations: BTreeMap<OpaqueId, Blake3Digest32>,
}

impl RetentionCatalog {
    /// Creates an empty finite catalog.
    pub fn new(policy: RetentionPolicy) -> Result<Self, RetentionError> {
        Ok(Self {
            policy: policy.validate()?,
            leases: BTreeMap::new(),
            operations: BTreeMap::new(),
        })
    }

    /// Reads one lease.
    #[must_use]
    pub fn get(&self, lease_id: &OpaqueId) -> Option<&RetentionLease> {
        self.leases.get(lease_id)
    }

    /// Creates one exact active lease.
    pub fn create_lease(
        &mut self,
        lease_id: OpaqueId,
        owner_id: OpaqueId,
        target: RetainedObject,
        issued_at_ms: u64,
        expires_at_ms: u64,
        operation: RetentionOperation,
        lease_receipt: ReceiptRef,
    ) -> Result<&RetentionLease, RetentionError> {
        self.validate_interval(issued_at_ms, expires_at_ms)?;
        self.register_operation(&operation)?;
        if self.leases.contains_key(&lease_id) {
            let existing = self
                .leases
                .get(&lease_id)
                .ok_or(RetentionError::LeaseNotFound)?;
            if existing.last_operation == operation
                && existing.owner_id == owner_id
                && existing.target == target
                && existing.issued_at_ms == issued_at_ms
                && existing.expires_at_ms == expires_at_ms
            {
                return Ok(existing);
            }
            return Err(RetentionError::LeaseConflict);
        }
        if self.leases.len() >= self.policy.max_leases {
            return Err(RetentionError::CapacityExceeded);
        }
        self.leases.insert(
            lease_id.clone(),
            RetentionLease {
                lease_id: lease_id.clone(),
                owner_id,
                target,
                issued_at_ms,
                expires_at_ms,
                state: RetentionLeaseState::Active,
                revision: 1,
                last_operation: operation,
                lease_receipt,
            },
        );
        Ok(self.leases.get(&lease_id).expect("inserted lease"))
    }

    /// Renews one active lease strictly forward.
    pub fn renew_lease(
        &mut self,
        lease_id: &OpaqueId,
        now_ms: u64,
        new_expires_at_ms: u64,
        operation: RetentionOperation,
        readback_receipt: ReceiptRef,
    ) -> Result<LeaseMutationReceipt, RetentionError> {
        self.register_operation(&operation)?;
        let max_duration = self.policy.max_lease_duration_ms;
        let lease = self
            .leases
            .get_mut(lease_id)
            .ok_or(RetentionError::LeaseNotFound)?;
        if lease.state != RetentionLeaseState::Active || lease.expires_at_ms <= now_ms {
            return Err(RetentionError::LeaseInactive);
        }
        if new_expires_at_ms <= lease.expires_at_ms
            || new_expires_at_ms.saturating_sub(now_ms) > max_duration
        {
            return Err(RetentionError::LeaseRegression);
        }
        lease.expires_at_ms = new_expires_at_ms;
        lease.revision = lease
            .revision
            .checked_add(1)
            .ok_or(RetentionError::ContractExhausted)?;
        lease.last_operation = operation.clone();
        Ok(LeaseMutationReceipt {
            lease_id: lease.lease_id.clone(),
            revision: lease.revision,
            state: lease.state,
            operation_id: operation.operation_id,
            readback_receipt,
        })
    }

    /// Releases or revokes one lease monotonically.
    pub fn end_lease(
        &mut self,
        lease_id: &OpaqueId,
        target_state: RetentionLeaseState,
        operation: RetentionOperation,
        readback_receipt: ReceiptRef,
    ) -> Result<LeaseMutationReceipt, RetentionError> {
        if !matches!(
            target_state,
            RetentionLeaseState::Released | RetentionLeaseState::Revoked
        ) {
            return Err(RetentionError::InvalidLease);
        }
        self.register_operation(&operation)?;
        let lease = self
            .leases
            .get_mut(lease_id)
            .ok_or(RetentionError::LeaseNotFound)?;
        if lease.state == target_state && lease.last_operation == operation {
            return Ok(LeaseMutationReceipt {
                lease_id: lease.lease_id.clone(),
                revision: lease.revision,
                state: lease.state,
                operation_id: operation.operation_id,
                readback_receipt,
            });
        }
        if lease.state != RetentionLeaseState::Active {
            return Err(RetentionError::LeaseInactive);
        }
        lease.state = target_state;
        lease.revision = lease
            .revision
            .checked_add(1)
            .ok_or(RetentionError::ContractExhausted)?;
        lease.last_operation = operation.clone();
        Ok(LeaseMutationReceipt {
            lease_id: lease.lease_id.clone(),
            revision: lease.revision,
            state: lease.state,
            operation_id: operation.operation_id,
            readback_receipt,
        })
    }

    /// Expires a bounded deterministic set of leases.
    pub fn expire(&mut self, now_ms: u64, max_batch: usize) -> usize {
        let candidates = self
            .leases
            .iter()
            .filter(|(_, lease)| {
                lease.state == RetentionLeaseState::Active && lease.expires_at_ms <= now_ms
            })
            .map(|(lease_id, _)| lease_id.clone())
            .take(max_batch)
            .collect::<Vec<_>>();
        let mut expired = 0_usize;
        for lease_id in candidates {
            if let Some(lease) = self.leases.get_mut(&lease_id) {
                lease.state = RetentionLeaseState::Expired;
                lease.revision = lease.revision.saturating_add(1);
                expired = expired.saturating_add(1);
            }
        }
        expired
    }

    /// Returns whether an exact object is protected by any active unexpired lease.
    #[must_use]
    pub fn has_active_lease(&self, target: &RetainedObject, now_ms: u64) -> bool {
        self.leases.values().any(|lease| {
            lease.state == RetentionLeaseState::Active
                && lease.expires_at_ms > now_ms
                && lease.target == *target
        })
    }

    fn validate_interval(&self, issued: u64, expires: u64) -> Result<(), RetentionError> {
        if expires <= issued || expires.saturating_sub(issued) > self.policy.max_lease_duration_ms {
            Err(RetentionError::InvalidLease)
        } else {
            Ok(())
        }
    }

    fn register_operation(&mut self, operation: &RetentionOperation) -> Result<(), RetentionError> {
        if let Some(existing) = self.operations.get(&operation.operation_id) {
            return if *existing == operation.request_digest {
                Ok(())
            } else {
                Err(RetentionError::OperationConflict)
            };
        }
        if self.operations.len() >= self.policy.max_operations {
            return Err(RetentionError::CapacityExceeded);
        }
        self.operations
            .insert(operation.operation_id.clone(), operation.request_digest);
        Ok(())
    }
}

/// Exact purge target manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeManifest {
    /// Purge request identity.
    pub request_id: OpaqueId,
    /// Exact target objects in canonical order.
    pub targets: Vec<RetainedObject>,
    /// Digest of exact canonical target set.
    pub manifest_digest: Blake3Digest32,
    /// Purge generation.
    pub purge_generation: u64,
    /// Next purge-fence revision.
    pub purge_fence_revision: PurgeFenceRevision,
    /// Manifest preparation receipt.
    pub preparation_receipt: ReceiptRef,
}

impl PurgeManifest {
    /// Validates canonical, finite, duplicate-free targets.
    pub fn validate(&self, policy: RetentionPolicy) -> Result<(), RetentionError> {
        if self.purge_generation == 0
            || self.targets.is_empty()
            || self.targets.len() > policy.max_purge_targets
            || self.targets.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(RetentionError::InvalidPurgeManifest);
        }
        Ok(())
    }
}

/// Ordered purge phase.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PurgePhase {
    Prepared,
    LiveDenyCommitted,
    HandlesInvalidated,
    IndexDeleted,
    CacheDeleted,
    SearchObjectsDeleted,
    BackupDispositionRecorded,
    Complete,
    OutcomeUnknown,
    Quarantined,
}

/// Exact affected-object acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeLayerReceipt {
    /// Request identity.
    pub request_id: OpaqueId,
    /// Purge generation.
    pub purge_generation: u64,
    /// Exact acknowledged object IDs.
    pub acknowledged_objects: BTreeSet<OpaqueId>,
    /// Objects missing from exact readback.
    pub missing_objects: BTreeSet<OpaqueId>,
    /// Unexpected objects observed during readback.
    pub unexpected_objects: BTreeSet<OpaqueId>,
    /// Layer-specific exact readback digest.
    pub readback_digest: Blake3Digest32,
    /// Durable external receipt.
    pub receipt: ReceiptRef,
}

/// Backup/snapshot disposition.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BackupDisposition {
    NotPresent,
    Deleted,
    TombstoneRetained,
    LegalHoldRetained,
    Unresolved,
}

/// Physical erasure statement, distinct from logical non-accessibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalEraseEvidence {
    NotGuaranteed,
    EvidenceAvailable {
        /// External device/provider evidence.
        evidence_receipt: ReceiptRef,
    },
}

/// Immutable logical purge tombstone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeTombstone {
    /// Request identity.
    pub request_id: OpaqueId,
    /// Purge generation.
    pub purge_generation: u64,
    /// Purge fence revision.
    pub purge_fence_revision: PurgeFenceRevision,
    /// Exact purged manifest digest.
    pub manifest_digest: Blake3Digest32,
    /// Logical non-accessibility proof.
    pub logical_non_accessibility_receipt: ReceiptRef,
    /// Backup disposition.
    pub backup_disposition: BackupDisposition,
    /// Physical erasure statement.
    pub physical_erase: PhysicalEraseEvidence,
    /// Tombstone digest.
    pub tombstone_digest: Blake3Digest32,
}

/// Stateful single-transaction purge coordinator.
#[derive(Clone, Debug)]
pub struct PurgeCoordinator {
    policy: RetentionPolicy,
    phase: PurgePhase,
    manifest: PurgeManifest,
    expected_target_ids: BTreeSet<OpaqueId>,
    last_receipt: Option<PurgeLayerReceipt>,
    backup_disposition: Option<BackupDisposition>,
    tombstone: Option<PurgeTombstone>,
}

impl PurgeCoordinator {
    /// Creates a prepared purge transaction.
    pub fn new(
        policy: RetentionPolicy,
        manifest: PurgeManifest,
        previous_fence: PurgeFenceRevision,
    ) -> Result<Self, RetentionError> {
        let policy = policy.validate()?;
        manifest.validate(policy)?;
        let expected_fence = previous_fence
            .checked_next()
            .map_err(|_| RetentionError::ContractExhausted)?;
        if manifest.purge_fence_revision != expected_fence {
            return Err(RetentionError::PurgeFenceMismatch);
        }
        let expected_target_ids = manifest
            .targets
            .iter()
            .map(|target| target.object_id.clone())
            .collect::<BTreeSet<_>>();
        if expected_target_ids.len() != manifest.targets.len() {
            return Err(RetentionError::InvalidPurgeManifest);
        }
        Ok(Self {
            policy,
            phase: PurgePhase::Prepared,
            manifest,
            expected_target_ids,
            last_receipt: None,
            backup_disposition: None,
            tombstone: None,
        })
    }

    /// Current phase.
    #[must_use]
    pub const fn phase(&self) -> PurgePhase {
        self.phase
    }

    /// Exact manifest.
    #[must_use]
    pub const fn manifest(&self) -> &PurgeManifest {
        &self.manifest
    }

    /// Commits the restrictive live-deny barrier first.
    pub fn accept_live_deny(&mut self, receipt: PurgeLayerReceipt) -> Result<(), RetentionError> {
        self.accept_layer(
            PurgePhase::Prepared,
            PurgePhase::LiveDenyCommitted,
            receipt,
            RetentionError::LiveDenyReceiptMissing,
        )
    }

    /// Accepts handle and continuation invalidation.
    pub fn accept_invalidation(
        &mut self,
        receipt: PurgeLayerReceipt,
    ) -> Result<(), RetentionError> {
        self.accept_layer(
            PurgePhase::LiveDenyCommitted,
            PurgePhase::HandlesInvalidated,
            receipt,
            RetentionError::InvalidationIncomplete,
        )
    }

    /// Accepts exact index deletion/fencing.
    pub fn accept_index_deletion(
        &mut self,
        receipt: PurgeLayerReceipt,
    ) -> Result<(), RetentionError> {
        self.accept_layer(
            PurgePhase::HandlesInvalidated,
            PurgePhase::IndexDeleted,
            receipt,
            RetentionError::IndexDeletionIncomplete,
        )
    }

    /// Accepts exact cache deletion.
    pub fn accept_cache_deletion(
        &mut self,
        receipt: PurgeLayerReceipt,
    ) -> Result<(), RetentionError> {
        self.accept_layer(
            PurgePhase::IndexDeleted,
            PurgePhase::CacheDeleted,
            receipt,
            RetentionError::CacheDeletionIncomplete,
        )
    }

    /// Accepts deletion of search-owned revision/materialization/unit objects.
    pub fn accept_object_deletion(
        &mut self,
        receipt: PurgeLayerReceipt,
    ) -> Result<(), RetentionError> {
        self.accept_layer(
            PurgePhase::CacheDeleted,
            PurgePhase::SearchObjectsDeleted,
            receipt,
            RetentionError::ObjectDeletionIncomplete,
        )
    }

    /// Records explicit backup/snapshot/legal-hold disposition.
    pub fn record_backup_disposition(
        &mut self,
        disposition: BackupDisposition,
        receipt: PurgeLayerReceipt,
    ) -> Result<(), RetentionError> {
        if disposition == BackupDisposition::Unresolved {
            return Err(RetentionError::BackupDispositionIncomplete);
        }
        self.accept_layer(
            PurgePhase::SearchObjectsDeleted,
            PurgePhase::BackupDispositionRecorded,
            receipt,
            RetentionError::BackupDispositionIncomplete,
        )?;
        self.backup_disposition = Some(disposition);
        Ok(())
    }

    /// Completes logical purge and issues a tombstone.
    pub fn complete(
        &mut self,
        logical_non_accessibility_receipt: ReceiptRef,
        physical_erase: PhysicalEraseEvidence,
        tombstone_digest: Blake3Digest32,
    ) -> Result<PurgeTombstone, RetentionError> {
        if self.phase != PurgePhase::BackupDispositionRecorded {
            return Err(RetentionError::InvalidPurgeTransition);
        }
        let backup_disposition = self
            .backup_disposition
            .ok_or(RetentionError::BackupDispositionIncomplete)?;
        if matches!(
            physical_erase,
            PhysicalEraseEvidence::EvidenceAvailable { .. }
        ) && matches!(backup_disposition, BackupDisposition::LegalHoldRetained)
        {
            return Err(RetentionError::SecureEraseEvidenceMissing);
        }
        let tombstone = PurgeTombstone {
            request_id: self.manifest.request_id.clone(),
            purge_generation: self.manifest.purge_generation,
            purge_fence_revision: self.manifest.purge_fence_revision,
            manifest_digest: self.manifest.manifest_digest,
            logical_non_accessibility_receipt,
            backup_disposition,
            physical_erase,
            tombstone_digest,
        };
        self.phase = PurgePhase::Complete;
        self.tombstone = Some(tombstone.clone());
        Ok(tombstone)
    }

    /// Marks external mutation outcome unknown without advancing success.
    pub fn mark_outcome_unknown(&mut self) -> Result<(), RetentionError> {
        if matches!(self.phase, PurgePhase::Complete | PurgePhase::Quarantined) {
            return Err(RetentionError::InvalidPurgeTransition);
        }
        self.phase = PurgePhase::OutcomeUnknown;
        Ok(())
    }

    /// Quarantines contradictory purge state.
    pub fn quarantine(&mut self) {
        self.phase = PurgePhase::Quarantined;
    }

    fn accept_layer(
        &mut self,
        expected_phase: PurgePhase,
        next_phase: PurgePhase,
        receipt: PurgeLayerReceipt,
        incomplete_error: RetentionError,
    ) -> Result<(), RetentionError> {
        if self.phase != expected_phase {
            return Err(RetentionError::InvalidPurgeTransition);
        }
        if receipt.request_id != self.manifest.request_id
            || receipt.purge_generation != self.manifest.purge_generation
            || receipt.acknowledged_objects != self.expected_target_ids
            || !receipt.missing_objects.is_empty()
            || !receipt.unexpected_objects.is_empty()
        {
            return Err(incomplete_error);
        }
        self.last_receipt = Some(receipt);
        self.phase = next_phase;
        Ok(())
    }
}

/// Pin observation used for ordinary reclaim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReclaimFence {
    /// Minimum active pinned epoch, or none when no epoch is pinned.
    pub minimum_pinned_epoch: Option<Epoch>,
    /// Whether the exact route/collection generation is still current.
    pub route_current: bool,
    /// Whether purge fences allow ordinary reclaim.
    pub purge_permitted: bool,
}

/// Exact reclaim manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimManifest {
    /// Canonically ordered retired objects.
    pub objects: Vec<RetainedObject>,
    /// Digest of exact manifest.
    pub manifest_digest: Blake3Digest32,
    /// Reclaim operation identity.
    pub operation: RetentionOperation,
}

/// Determines whether an exact retired object may be reclaimed.
pub fn reclaim_eligible(
    object: &RetainedObject,
    catalog: &RetentionCatalog,
    fence: ReclaimFence,
    now_ms: u64,
) -> Result<(), RetentionError> {
    if catalog.has_active_lease(object, now_ms) {
        return Err(RetentionError::RetentionLeaseActive);
    }
    if !fence.route_current || !fence.purge_permitted {
        return Err(RetentionError::ReclaimManifestMismatch);
    }
    if let (Some(retired_epoch), Some(minimum_pin)) =
        (object.last_visible_epoch, fence.minimum_pinned_epoch)
    {
        if minimum_pin <= retired_epoch {
            return Err(RetentionError::EpochPinActive);
        }
    }
    Ok(())
}

/// Validates every object in one exact reclaim manifest.
pub fn validate_reclaim_manifest(
    manifest: &ReclaimManifest,
    catalog: &RetentionCatalog,
    fence: ReclaimFence,
    now_ms: u64,
    max_objects: usize,
) -> Result<(), RetentionError> {
    if manifest.objects.is_empty()
        || manifest.objects.len() > max_objects
        || manifest.objects.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(RetentionError::ReclaimManifestMismatch);
    }
    for object in &manifest.objects {
        reclaim_eligible(object, catalog, fence, now_ms)?;
    }
    Ok(())
}

/// Paired restore manifest for control and index state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreManifest {
    /// Restore operation identity.
    pub restore_id: OpaqueId,
    /// Control checkpoint digest.
    pub control_checkpoint_digest: Blake3Digest32,
    /// Index snapshot digest.
    pub index_snapshot_digest: Blake3Digest32,
    /// Collection generation restored.
    pub collection_generation_id: CollectionGenerationId,
    /// Visible epoch claimed by the pair.
    pub visible_epoch: Epoch,
    /// Purge tombstone generation included in the backup.
    pub purge_tombstone_generation: u64,
    /// Exact paired-manifest digest.
    pub paired_manifest_digest: Blake3Digest32,
    /// Backup provenance receipt.
    pub backup_receipt: ReceiptRef,
}

/// Restore lifecycle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RestorePhase {
    RestorePendingRevalidation,
    ControlReadbackVerified,
    ObjectsReadbackVerified,
    DirectOnly,
    IndexReadbackVerified,
    IndexedAdmitted,
    Quarantined,
}

/// Exact restore-layer receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreLayerReceipt {
    /// Restore identity.
    pub restore_id: OpaqueId,
    /// Paired manifest digest.
    pub paired_manifest_digest: Blake3Digest32,
    /// Layer readback digest.
    pub readback_digest: Blake3Digest32,
    /// External receipt.
    pub receipt: ReceiptRef,
}

/// Restore coordinator that defaults to non-serving revalidation.
#[derive(Clone, Debug)]
pub struct RestoreCoordinator {
    manifest: RestoreManifest,
    phase: RestorePhase,
    control_receipt: Option<RestoreLayerReceipt>,
    object_receipt: Option<RestoreLayerReceipt>,
    index_receipt: Option<RestoreLayerReceipt>,
}

impl RestoreCoordinator {
    /// Creates a pending-revalidation restore transaction.
    pub fn new(manifest: RestoreManifest) -> Result<Self, RetentionError> {
        if manifest.purge_tombstone_generation == 0 {
            return Err(RetentionError::RestoreManifestInvalid);
        }
        Ok(Self {
            manifest,
            phase: RestorePhase::RestorePendingRevalidation,
            control_receipt: None,
            object_receipt: None,
            index_receipt: None,
        })
    }

    /// Current restore phase.
    #[must_use]
    pub const fn phase(&self) -> RestorePhase {
        self.phase
    }

    /// Accepts exact control checkpoint readback.
    pub fn verify_control(&mut self, receipt: RestoreLayerReceipt) -> Result<(), RetentionError> {
        self.verify_receipt(&receipt)?;
        if self.phase != RestorePhase::RestorePendingRevalidation
            || receipt.readback_digest != self.manifest.control_checkpoint_digest
        {
            return Err(RetentionError::RestoreRevalidationIncomplete);
        }
        self.control_receipt = Some(receipt);
        self.phase = RestorePhase::ControlReadbackVerified;
        Ok(())
    }

    /// Accepts exact restored source/object readback.
    pub fn verify_objects(
        &mut self,
        receipt: RestoreLayerReceipt,
        all_objects_valid: bool,
    ) -> Result<(), RetentionError> {
        self.verify_receipt(&receipt)?;
        if self.phase != RestorePhase::ControlReadbackVerified || !all_objects_valid {
            return Err(RetentionError::RestoreRevalidationIncomplete);
        }
        self.object_receipt = Some(receipt);
        self.phase = RestorePhase::ObjectsReadbackVerified;
        Ok(())
    }

    /// Admits direct-only serving after control and source-object verification.
    pub fn admit_direct_only(&mut self) -> Result<(), RetentionError> {
        if self.phase != RestorePhase::ObjectsReadbackVerified {
            return Err(RetentionError::RestoreRevalidationIncomplete);
        }
        self.phase = RestorePhase::DirectOnly;
        Ok(())
    }

    /// Accepts exact index snapshot readback while remaining direct-only.
    pub fn verify_index(&mut self, receipt: RestoreLayerReceipt) -> Result<(), RetentionError> {
        self.verify_receipt(&receipt)?;
        if self.phase != RestorePhase::DirectOnly
            || receipt.readback_digest != self.manifest.index_snapshot_digest
        {
            return Err(RetentionError::RestoreRevalidationIncomplete);
        }
        self.index_receipt = Some(receipt);
        self.phase = RestorePhase::IndexReadbackVerified;
        Ok(())
    }

    /// Admits indexed serving only after a current publication/readiness receipt.
    pub fn admit_indexed(
        &mut self,
        publication_receipt: &ReceiptRef,
        current_visible_epoch: Epoch,
        current_collection_generation: CollectionGenerationId,
    ) -> Result<(), RetentionError> {
        if self.phase != RestorePhase::IndexReadbackVerified
            || publication_receipt.as_str().is_empty()
            || current_visible_epoch != self.manifest.visible_epoch
            || current_collection_generation != self.manifest.collection_generation_id
        {
            return Err(RetentionError::IndexedRestoreNotAdmitted);
        }
        self.phase = RestorePhase::IndexedAdmitted;
        Ok(())
    }

    /// Quarantines contradictory restored state.
    pub fn quarantine(&mut self) {
        self.phase = RestorePhase::Quarantined;
    }

    fn verify_receipt(&self, receipt: &RestoreLayerReceipt) -> Result<(), RetentionError> {
        if receipt.restore_id != self.manifest.restore_id
            || receipt.paired_manifest_digest != self.manifest.paired_manifest_digest
        {
            Err(RetentionError::RestoreManifestInvalid)
        } else {
            Ok(())
        }
    }
}
