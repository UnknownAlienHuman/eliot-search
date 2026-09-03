//! Finite owner leases, process-local authorities, and content-free receipts.

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, BoundedSet, NonZeroRevision, OwnerEpoch, ReceiptRef};
use search_ports::MonotonicInstant;

use crate::{DataRootIdentity, OwnerBinding, OwnerError, OwnerOperation, RuntimeMode};

/// Maximum number of distinct owner-health reasons.
pub const MAX_OWNER_HEALTH_REASONS: usize = 16;

/// Finite process-monotonic lease window.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LeaseWindow {
    acquired_at: MonotonicInstant,
    heartbeat_at: MonotonicInstant,
    expires_at: MonotonicInstant,
}

impl LeaseWindow {
    /// Creates a strictly ordered lease window.
    ///
    /// # Errors
    ///
    /// Acquisition must not follow heartbeat and expiration must follow it.
    pub fn new(
        acquired_at: MonotonicInstant,
        heartbeat_at: MonotonicInstant,
        expires_at: MonotonicInstant,
    ) -> Result<Self, OwnerError> {
        if acquired_at > heartbeat_at || heartbeat_at >= expires_at {
            return Err(OwnerError::OwnerLeaseInvalid);
        }
        Ok(Self {
            acquired_at,
            heartbeat_at,
            expires_at,
        })
    }

    /// Acquisition instant.
    #[must_use]
    pub const fn acquired_at(self) -> MonotonicInstant {
        self.acquired_at
    }

    /// Last verified heartbeat instant.
    #[must_use]
    pub const fn heartbeat_at(self) -> MonotonicInstant {
        self.heartbeat_at
    }

    /// Finite expiration instant.
    #[must_use]
    pub const fn expires_at(self) -> MonotonicInstant {
        self.expires_at
    }

    /// Returns whether the lease is expired at an explicit observation instant.
    #[must_use]
    pub const fn is_expired_at(self, now: MonotonicInstant) -> bool {
        now.ticks() >= self.expires_at.ticks()
    }

    /// Creates a strictly monotone renewal.
    ///
    /// # Errors
    ///
    /// Rejects heartbeat regression and expiration that does not extend the
    /// previous lease strictly into the future.
    pub fn renew(
        self,
        heartbeat_at: MonotonicInstant,
        expires_at: MonotonicInstant,
    ) -> Result<Self, OwnerError> {
        if heartbeat_at < self.heartbeat_at
            || expires_at <= heartbeat_at
            || expires_at <= self.expires_at
        {
            return Err(OwnerError::OwnerHeartbeatRegression);
        }
        Ok(Self {
            acquired_at: self.acquired_at,
            heartbeat_at,
            expires_at,
        })
    }
}

/// Durable lifecycle of one exact owner record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerLifecycle {
    /// Ordinary work may be admitted.
    Active,
    /// New work is denied while dependencies drain.
    Draining,
}

/// Canonical durable owner record expected by platform adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerRecord {
    binding: OwnerBinding,
    token_digest: Blake3Digest32,
    lease: LeaseWindow,
    lifecycle: OwnerLifecycle,
    record_revision: NonZeroRevision,
    record_digest: Blake3Digest32,
    acquire_operation: OwnerOperation,
    last_operation: OwnerOperation,
}

impl OwnerRecord {
    /// Creates the first active record for a prepared acquisition.
    #[must_use]
    pub fn new_active(
        binding: OwnerBinding,
        token_digest: Blake3Digest32,
        lease: LeaseWindow,
        record_revision: NonZeroRevision,
        record_digest: Blake3Digest32,
        acquire_operation: OwnerOperation,
    ) -> Self {
        Self {
            binding,
            token_digest,
            lease,
            lifecycle: OwnerLifecycle::Active,
            record_revision,
            record_digest,
            last_operation: acquire_operation.clone(),
            acquire_operation,
        }
    }

    /// Exact root, owner, mode, and epoch binding.
    #[must_use]
    pub const fn binding(&self) -> OwnerBinding {
        self.binding
    }

    /// Digest of the process-local bearer token.
    #[must_use]
    pub const fn token_digest(&self) -> Blake3Digest32 {
        self.token_digest
    }

    /// Current finite lease.
    #[must_use]
    pub const fn lease(&self) -> LeaseWindow {
        self.lease
    }

    /// Current durable lifecycle.
    #[must_use]
    pub const fn lifecycle(&self) -> OwnerLifecycle {
        self.lifecycle
    }

    /// Monotone record revision.
    #[must_use]
    pub const fn record_revision(&self) -> NonZeroRevision {
        self.record_revision
    }

    /// Digest of exact durable record bytes.
    #[must_use]
    pub const fn record_digest(&self) -> Blake3Digest32 {
        self.record_digest
    }

    /// Immutable acquisition operation.
    #[must_use]
    pub const fn acquire_operation(&self) -> &OwnerOperation {
        &self.acquire_operation
    }

    /// Most recent durable mutation operation.
    #[must_use]
    pub const fn last_operation(&self) -> &OwnerOperation {
        &self.last_operation
    }

    /// Produces a renewed active record.
    ///
    /// # Errors
    ///
    /// Requires active lifecycle, a contiguous revision, replay-compatible
    /// operation identity, and a strictly extended lease.
    pub fn renewed(
        &self,
        operation: OwnerOperation,
        heartbeat_at: MonotonicInstant,
        expires_at: MonotonicInstant,
        next_revision: NonZeroRevision,
        next_record_digest: Blake3Digest32,
    ) -> Result<Self, OwnerError> {
        if self.lifecycle != OwnerLifecycle::Active {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        verify_next_revision(self.record_revision, next_revision)?;
        operation.verify_replay(&self.last_operation)?;
        Ok(Self {
            binding: self.binding,
            token_digest: self.token_digest,
            lease: self.lease.renew(heartbeat_at, expires_at)?,
            lifecycle: OwnerLifecycle::Active,
            record_revision: next_revision,
            record_digest: next_record_digest,
            acquire_operation: self.acquire_operation.clone(),
            last_operation: operation,
        })
    }

    /// Produces a draining record.
    ///
    /// # Errors
    ///
    /// Requires active lifecycle, a contiguous revision, and a
    /// replay-compatible operation identity.
    pub fn draining(
        &self,
        operation: OwnerOperation,
        next_revision: NonZeroRevision,
        next_record_digest: Blake3Digest32,
    ) -> Result<Self, OwnerError> {
        if self.lifecycle != OwnerLifecycle::Active {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        verify_next_revision(self.record_revision, next_revision)?;
        operation.verify_replay(&self.last_operation)?;
        Ok(Self {
            binding: self.binding,
            token_digest: self.token_digest,
            lease: self.lease,
            lifecycle: OwnerLifecycle::Draining,
            record_revision: next_revision,
            record_digest: next_record_digest,
            acquire_operation: self.acquire_operation.clone(),
            last_operation: operation,
        })
    }
}

fn verify_next_revision(
    current: NonZeroRevision,
    proposed: NonZeroRevision,
) -> Result<(), OwnerError> {
    let expected = current
        .checked_next()
        .map_err(|_| OwnerError::ContractExhausted)?;
    if proposed == expected {
        Ok(())
    } else {
        Err(OwnerError::OwnerEpochMismatch)
    }
}

/// Process-local ownership authority issued after exact readback.
///
/// This type deliberately implements neither `Clone` nor serialization.
pub struct OwnerGuard {
    binding: OwnerBinding,
    token_digest: Blake3Digest32,
    record_revision: NonZeroRevision,
}

impl OwnerGuard {
    pub(crate) const fn from_record(record: &OwnerRecord) -> Self {
        Self {
            binding: record.binding,
            token_digest: record.token_digest,
            record_revision: record.record_revision,
        }
    }

    /// Fenced owner epoch.
    #[must_use]
    pub const fn owner_epoch(&self) -> OwnerEpoch {
        self.binding.epoch()
    }

    /// Bound data-root identity.
    #[must_use]
    pub const fn data_root(&self) -> DataRootIdentity {
        self.binding.root()
    }

    /// Exclusive runtime mode.
    #[must_use]
    pub const fn mode(&self) -> RuntimeMode {
        self.binding.owner().mode()
    }

    /// Durable revision observed when the guard was issued.
    #[must_use]
    pub const fn record_revision(&self) -> NonZeroRevision {
        self.record_revision
    }

    pub(crate) fn verifies(&self, record: &OwnerRecord) -> bool {
        self.binding == record.binding
            && self.token_digest == record.token_digest
            && self.record_revision <= record.record_revision
    }
}

impl fmt::Debug for OwnerGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnerGuard")
            .field("data_root_id", &self.binding.root().data_root_id())
            .field("owner_epoch", &self.binding.epoch())
            .field("mode", &self.binding.owner().mode())
            .field("record_revision", &self.record_revision)
            .field("token_digest", &"<redacted>")
            .finish()
    }
}

/// Reason for entering draining lifecycle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DrainReason {
    /// Orderly daemon shutdown.
    Shutdown,
    /// Restart without a root change.
    Restart,
    /// Runtime mode or data-root change.
    ModeOrRootChange,
    /// Explicit maintenance drain.
    Maintenance,
}

/// Process-local proof that an exact owner entered draining state.
pub struct DrainToken {
    binding: OwnerBinding,
    operation: OwnerOperation,
    reason: DrainReason,
    record_revision: NonZeroRevision,
}

impl DrainToken {
    pub(crate) const fn new(
        binding: OwnerBinding,
        operation: OwnerOperation,
        reason: DrainReason,
        record_revision: NonZeroRevision,
    ) -> Self {
        Self {
            binding,
            operation,
            reason,
            record_revision,
        }
    }

    /// Bound owner epoch.
    #[must_use]
    pub const fn owner_epoch(&self) -> OwnerEpoch {
        self.binding.epoch()
    }

    /// Drain reason.
    #[must_use]
    pub const fn reason(&self) -> DrainReason {
        self.reason
    }

    /// Durable draining-record revision.
    #[must_use]
    pub const fn record_revision(&self) -> NonZeroRevision {
        self.record_revision
    }

    pub(crate) const fn binding(&self) -> OwnerBinding {
        self.binding
    }

    pub(crate) const fn operation(&self) -> &OwnerOperation {
        &self.operation
    }
}

impl fmt::Debug for DrainToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DrainToken")
            .field("data_root_id", &self.binding.root().data_root_id())
            .field("owner_epoch", &self.binding.epoch())
            .field("reason", &self.reason)
            .field("record_revision", &self.record_revision)
            .finish_non_exhaustive()
    }
}

/// Dependency that must stop before clean root release.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyComponent {
    /// Client/provider endpoint admission and transport.
    Endpoint,
    /// Source observation and preparation runtime.
    SourceRuntime,
    /// Index and optional child processes.
    IndexAndChildren,
    /// Durable control store.
    ControlStore,
}

impl DependencyComponent {
    /// Complete baseline shutdown set.
    pub const REQUIRED: [Self; 4] = [
        Self::Endpoint,
        Self::SourceRuntime,
        Self::IndexAndChildren,
        Self::ControlStore,
    ];
}

/// Content-free proof that one dependency stopped under an owner fence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyShutdownReceipt {
    /// Dependency whose shutdown completed.
    pub component: DependencyComponent,
    /// Exact owner binding used by the dependency.
    pub binding: OwnerBinding,
    /// External content-free receipt reference.
    pub receipt_ref: ReceiptRef,
}

/// Permit issued only after complete dependency shutdown proof.
pub struct ReleasePermit {
    binding: OwnerBinding,
    drain_operation: OwnerOperation,
    dependency_receipt_digest: Blake3Digest32,
    receipt_count: usize,
}

impl ReleasePermit {
    pub(crate) const fn new(
        binding: OwnerBinding,
        drain_operation: OwnerOperation,
        dependency_receipt_digest: Blake3Digest32,
        receipt_count: usize,
    ) -> Self {
        Self {
            binding,
            drain_operation,
            dependency_receipt_digest,
            receipt_count,
        }
    }

    /// Exact owner binding permitted to release.
    #[must_use]
    pub const fn binding(&self) -> OwnerBinding {
        self.binding
    }

    /// Digest of canonical dependency receipt identities.
    #[must_use]
    pub const fn dependency_receipt_digest(&self) -> Blake3Digest32 {
        self.dependency_receipt_digest
    }

    /// Number of validated dependency receipts.
    #[must_use]
    pub const fn receipt_count(&self) -> usize {
        self.receipt_count
    }

    pub(crate) const fn drain_operation(&self) -> &OwnerOperation {
        &self.drain_operation
    }
}

impl fmt::Debug for ReleasePermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReleasePermit")
            .field("data_root_id", &self.binding.root().data_root_id())
            .field("owner_epoch", &self.binding.epoch())
            .field("dependency_receipt_digest", &self.dependency_receipt_digest)
            .field("receipt_count", &self.receipt_count)
            .finish_non_exhaustive()
    }
}

/// Exact content-free owner verification receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerVerificationReceipt {
    /// Exact verified owner binding.
    pub binding: OwnerBinding,
    /// Durable record revision verified.
    pub record_revision: NonZeroRevision,
    /// Digest read back from the durable owner record.
    pub record_digest: Blake3Digest32,
    /// External observation receipt.
    pub observation_receipt: ReceiptRef,
}

/// Proof that an exact owner no longer claims the data root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerShutdownReceipt {
    /// Released owner binding.
    pub binding: OwnerBinding,
    /// Immutable release operation.
    pub operation: OwnerOperation,
    /// Last durable owner-record revision.
    pub final_record_revision: NonZeroRevision,
    /// Digest of dependency shutdown receipt identities.
    pub dependency_receipt_digest: Blake3Digest32,
    /// Exact release readback receipt.
    pub release_readback_receipt: ReceiptRef,
}

/// Bounded owner-health lifecycle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerHealthState {
    /// No owner currently claims the root.
    Vacant,
    /// Exact active owner is consistent.
    Active,
    /// Exact owner is draining.
    Draining,
    /// Acquisition or release requires exact readback.
    OutcomeUnknown,
    /// Contradictory ownership is quarantined.
    Quarantined,
}

/// Bounded owner-health reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerHealthReason {
    /// Durable and in-memory record digests differ.
    RecordDigestMismatch,
    /// Process creation identity differs.
    ProcessIdentityMismatch,
    /// Executable identity differs.
    ExecutableIdentityMismatch,
    /// Owner epoch differs.
    EpochMismatch,
    /// OS ownership primitive was not verified.
    OwnershipPrimitiveUnverified,
    /// Durable readback was incomplete.
    DurableReadbackIncomplete,
    /// State is explicitly quarantined.
    Quarantined,
}

/// Content-minimized owner-health snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerHealth {
    /// Lifecycle classification.
    pub state: OwnerHealthState,
    /// Owner epoch when known.
    pub owner_epoch: Option<OwnerEpoch>,
    /// Content-minimized root identity; no path text is exposed.
    pub root: Option<DataRootIdentity>,
    /// Distinct bounded reasons.
    pub reasons: BoundedSet<OwnerHealthReason, MAX_OWNER_HEALTH_REASONS>,
}

impl OwnerHealth {
    /// Creates a healthy state with no reasons.
    #[must_use]
    pub const fn healthy(
        state: OwnerHealthState,
        owner_epoch: Option<OwnerEpoch>,
        root: Option<DataRootIdentity>,
    ) -> Self {
        Self {
            state,
            owner_epoch,
            root,
            reasons: BoundedSet::empty(),
        }
    }

    /// Creates a bounded reason-bearing health snapshot.
    ///
    /// # Errors
    ///
    /// Duplicate or excessive reasons are rejected.
    pub fn with_reasons(
        state: OwnerHealthState,
        owner_epoch: Option<OwnerEpoch>,
        root: Option<DataRootIdentity>,
        reasons: impl IntoIterator<Item = OwnerHealthReason>,
    ) -> Result<Self, OwnerError> {
        let reasons =
            BoundedSet::from_items(reasons).map_err(|_| OwnerError::OwnerIdentityAmbiguous)?;
        Ok(Self {
            state,
            owner_epoch,
            root,
            reasons,
        })
    }
}

pub(crate) fn validate_dependency_receipts(
    binding: OwnerBinding,
    receipts: &[DependencyShutdownReceipt],
) -> Result<(), OwnerError> {
    if receipts.len() != DependencyComponent::REQUIRED.len() {
        return Err(OwnerError::OwnerReleasePreconditionMissing);
    }
    let mut observed = BTreeSet::new();
    for receipt in receipts {
        if receipt.binding != binding || !observed.insert(receipt.component) {
            return Err(OwnerError::OwnerReleasePreconditionMissing);
        }
    }
    let required = DependencyComponent::REQUIRED
        .into_iter()
        .collect::<BTreeSet<_>>();
    if observed == required {
        Ok(())
    } else {
        Err(OwnerError::OwnerReleasePreconditionMissing)
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        ArtifactDigest, Blake3Digest32, DataRootId, InstallationId, InstallationIncarnationId,
        NonZeroRevision, OpaqueId, OwnerEpoch,
    };
    use search_ports::{IdempotencyClass, MonotonicInstant, MutationIdentity};

    use super::{LeaseWindow, OwnerGuard, OwnerRecord};
    use crate::{
        DataRootIdentity, DataRootLocationClass, ExecutableIdentity, OwnerBinding, OwnerError,
        OwnerIdentity, OwnerOperation, ProcessCreationIdentity, RuntimeMode,
    };

    fn record() -> OwnerRecord {
        let root = DataRootIdentity::new(
            DataRootId::from_bytes([1; 16]),
            DataRootLocationClass::LocalFixed,
            Blake3Digest32::from_bytes([2; 32]),
            Blake3Digest32::from_bytes([3; 32]),
        );
        let owner = OwnerIdentity::new(
            InstallationId::from_bytes([5; 16]),
            InstallationIncarnationId::from_bytes([6; 16]),
            ProcessCreationIdentity::new(100, 200, Blake3Digest32::from_bytes([4; 32]))
                .expect("process"),
            ExecutableIdentity::new(
                ArtifactDigest::from_bytes([7; 32]),
                Blake3Digest32::from_bytes([8; 32]),
            ),
            RuntimeMode::Standalone,
        );
        let operation = OwnerOperation::new(
            MutationIdentity::new(
                OpaqueId::new("owner-operation:acquire").expect("id"),
                IdempotencyClass::RetrySameIdentity,
            ),
            Blake3Digest32::from_bytes([9; 32]),
        );
        OwnerRecord::new_active(
            OwnerBinding::new(root, owner, OwnerEpoch::new(1).expect("epoch")),
            Blake3Digest32::from_bytes([10; 32]),
            LeaseWindow::new(
                MonotonicInstant::from_ticks(10),
                MonotonicInstant::from_ticks(10),
                MonotonicInstant::from_ticks(20),
            )
            .expect("lease"),
            NonZeroRevision::new(1).expect("revision"),
            Blake3Digest32::from_bytes([11; 32]),
            operation,
        )
    }

    #[test]
    fn lease_renewal_is_strictly_monotone() {
        let lease = record().lease();
        assert_eq!(
            lease.renew(
                MonotonicInstant::from_ticks(9),
                MonotonicInstant::from_ticks(30),
            ),
            Err(OwnerError::OwnerHeartbeatRegression)
        );
        assert_eq!(
            lease.renew(
                MonotonicInstant::from_ticks(11),
                MonotonicInstant::from_ticks(20),
            ),
            Err(OwnerError::OwnerHeartbeatRegression)
        );
    }

    #[test]
    fn guard_debug_is_token_redacted() {
        let debug = format!("{:?}", OwnerGuard::from_record(&record()));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("10, 10, 10"));
    }
}
