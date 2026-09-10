//! Closed registry failure surface, finite limits and vendor-neutral durability port.
//!
//! This module owns no registry state. It defines the typed failure codes
//! required by `FUNCTIONS.md`, the finite [`RegistryLimits`] dimensions and the
//! package-owned [`RegistryControlPort`] durability boundary. All durable
//! registry mutations persist their content-free receipt through the port; no
//! mutation writes directly to a concrete store, filesystem or in-memory map
//! without going through the port.

use core::fmt;
use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};
use search_ports::{
    CancellationProbe, DisclosureClass, MutationIdentity, OperationContext, Port, PortError,
    PortErrorKind, PortReceipt, PortRetryability,
};

/// Finite source-registry limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryLimits {
    /// Maximum retained stable sources.
    pub max_sources: usize,
    /// Maximum retained source/corpus membership records.
    pub max_memberships: usize,
    /// Maximum changes in one atomic batch.
    pub max_batch_changes: usize,
    /// Maximum retained full-payload operation identities.
    pub max_operations: usize,
    /// Maximum active portfolio results.
    pub max_portfolio_items: usize,
    /// Maximum stable identities in one namespace cutover inventory.
    pub max_cutover_inventory: usize,
    /// Maximum retained admitted roots.
    pub max_roots: usize,
    /// Maximum retained journal entries in the durability port.
    pub max_journal_entries: usize,
    /// Maximum retained reference portfolios.
    pub max_portfolios: usize,
    /// Maximum retained namespace ownership records.
    pub max_namespaces: usize,
}

impl RegistryLimits {
    /// Validates all finite dimensions as non-zero.
    pub const fn validate(self) -> Result<Self, RegistryError> {
        if self.max_sources == 0
            || self.max_memberships == 0
            || self.max_batch_changes == 0
            || self.max_operations == 0
            || self.max_portfolio_items == 0
            || self.max_cutover_inventory == 0
            || self.max_roots == 0
            || self.max_journal_entries == 0
            || self.max_portfolios == 0
            || self.max_namespaces == 0
        {
            Err(RegistryError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Conservative finite source-registry limits.
pub const DEFAULT_REGISTRY_LIMITS: RegistryLimits = RegistryLimits {
    max_sources: 1_000_000,
    max_memberships: 4_000_000,
    max_batch_changes: 4_096,
    max_operations: 2_000_000,
    max_portfolio_items: 100_000,
    max_cutover_inventory: 1_000_000,
    max_roots: 100_000,
    max_journal_entries: 2_000_000,
    max_portfolios: 100_000,
    max_namespaces: 100_000,
};

/// Closed content-free registry failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RegistryError {
    /// Registry limits are zero or internally inconsistent.
    InvalidLimits,
    /// Batch is empty or exceeds its finite change ceiling.
    BatchSizeInvalid,
    /// Expected registry revision is stale.
    RegistryRevisionConflict,
    /// Registry revision cannot advance.
    RegistryRevisionOverflow,
    /// Operation identity was reused with another full-payload digest.
    OperationConflict,
    /// Finite operation ledger is full.
    OperationCapacityExceeded,
    /// One batch touches the same source or membership more than once.
    DuplicateBatchTarget,
    /// Stable source identity is already registered.
    SourceAlreadyRegistered,
    /// Stable source identity is absent.
    SourceNotFound,
    /// Admission candidate and assignment proof differ.
    AdmissionBindingMismatch,
    /// Admission assignment receipt or exact binding digest is absent.
    AdmissionBindingEvidenceMissing,
    /// Existing source binding differs from the registration request.
    SourceBindingConflict,
    /// Source lifecycle or revision does not permit the mutation.
    SourceRevisionConflict,
    /// Membership is already active or conflicts with another generation.
    MembershipCollision,
    /// Membership is absent.
    MembershipNotFound,
    /// Membership revision differs.
    MembershipRevisionConflict,
    /// Source is retired and cannot enter an active portfolio.
    SourceRetired,
    /// Membership references another namespace.
    NamespaceMismatch,
    /// Cutover inventory is empty, duplicated, or exceeds its finite ceiling.
    CutoverInventoryInvalid,
    /// Cutover generation is stale, reused, or not strictly newer.
    CutoverGenerationConflict,
    /// Cutover authorization or authoritative readback is missing.
    CutoverEvidenceMissing,
    /// Cutover inventory contains an absent or retired source.
    CutoverSourceUnavailable,
    /// Portfolio request is zero or exceeds its finite ceiling.
    PortfolioLimitInvalid,
    /// Registry would exceed finite source or membership capacity.
    CapacityExceeded,
    /// Shared source or membership revision cannot advance.
    ContractExhausted,
    /// Registry snapshot is missing, incoherent or violates integrity.
    SnapshotInvalid,
    /// Root identity conflicts with an existing canonical root.
    RootIdentityConflict,
    /// Root is already registered under the same canonical identity.
    RootAlreadyRegistered,
    /// Root is not registered.
    RootNotRegistered,
    /// Root policy generation does not match the expected revision.
    RootPolicyGenerationMismatch,
    /// Source is not admitted.
    SourceNotAdmitted,
    /// Source is already admitted under a conflicting identity.
    SourceAlreadyAdmittedConflict,
    /// Admission receipt is stale against the current policy/owner fence.
    AdmissionReceiptStale,
    /// Admission receipt does not bind the exact source/policy/observation.
    AdmissionReceiptMismatch,
    /// Membership identity conflicts with an existing record.
    MembershipConflict,
    /// Membership generation does not match the expected revision.
    MembershipGenerationMismatch,
    /// Reference portfolio scope is empty without explicit permission.
    ReferenceScopeEmpty,
    /// Reference portfolio revision is invalid.
    ReferencePortfolioInvalid,
    /// Source view request is ambiguous across roots/memberships.
    SourceViewAmbiguous,
    /// Source view uses a stale registry/owner generation.
    SourceViewStale,
    /// Workspace view is incoherent across repository/worktree/branch/index.
    WorkspaceViewIncoherent,
    /// Source namespace has a conflicting active mutable owner.
    NamespaceOwnershipConflict,
    /// Operation requires a completed namespace owner cutover.
    CutoverRequired,
    /// Cutover receipt fails fence-before-activation or coverage proof.
    CutoverReceiptMismatch,
    /// Mutation outcome is unknown until exact readback recovery resolves it.
    MutationOutcomeUnknown,
    /// Mutation was cancelled before any durable commit.
    CancelledBeforeCommit,
    /// Durability port rejected the receipt persistence.
    DurabilityRejected,
    /// Durability port has no capacity for another journal entry.
    DurabilityExhausted,
}

impl RegistryError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "REGISTRY_INVALID_LIMITS",
            Self::BatchSizeInvalid => "REGISTRY_BATCH_SIZE_INVALID",
            Self::RegistryRevisionConflict => "REGISTRY_REVISION_CONFLICT",
            Self::RegistryRevisionOverflow => "REGISTRY_REVISION_OVERFLOW",
            Self::OperationConflict => "REGISTRY_OPERATION_CONFLICT",
            Self::OperationCapacityExceeded => "REGISTRY_OPERATION_CAPACITY_EXCEEDED",
            Self::DuplicateBatchTarget => "REGISTRY_DUPLICATE_BATCH_TARGET",
            Self::SourceAlreadyRegistered => "REGISTRY_SOURCE_ALREADY_REGISTERED",
            Self::SourceNotFound => "REGISTRY_SOURCE_NOT_FOUND",
            Self::AdmissionBindingMismatch => "REGISTRY_ADMISSION_BINDING_MISMATCH",
            Self::AdmissionBindingEvidenceMissing => "REGISTRY_ADMISSION_BINDING_EVIDENCE_MISSING",
            Self::SourceBindingConflict => "REGISTRY_SOURCE_BINDING_CONFLICT",
            Self::SourceRevisionConflict => "REGISTRY_SOURCE_REVISION_CONFLICT",
            Self::MembershipCollision => "REGISTRY_MEMBERSHIP_COLLISION",
            Self::MembershipNotFound => "REGISTRY_MEMBERSHIP_NOT_FOUND",
            Self::MembershipRevisionConflict => "REGISTRY_MEMBERSHIP_REVISION_CONFLICT",
            Self::SourceRetired => "REGISTRY_SOURCE_RETIRED",
            Self::NamespaceMismatch => "REGISTRY_NAMESPACE_MISMATCH",
            Self::CutoverInventoryInvalid => "REGISTRY_CUTOVER_INVENTORY_INVALID",
            Self::CutoverGenerationConflict => "REGISTRY_CUTOVER_GENERATION_CONFLICT",
            Self::CutoverEvidenceMissing => "REGISTRY_CUTOVER_EVIDENCE_MISSING",
            Self::CutoverSourceUnavailable => "REGISTRY_CUTOVER_SOURCE_UNAVAILABLE",
            Self::PortfolioLimitInvalid => "REGISTRY_PORTFOLIO_LIMIT_INVALID",
            Self::CapacityExceeded => "REGISTRY_CAPACITY_EXCEEDED",
            Self::ContractExhausted => "REGISTRY_CONTRACT_EXHAUSTED",
            Self::SnapshotInvalid => "REGISTRY_SNAPSHOT_INVALID",
            Self::RootIdentityConflict => "ROOT_IDENTITY_CONFLICT",
            Self::RootAlreadyRegistered => "ROOT_ALREADY_REGISTERED",
            Self::RootNotRegistered => "ROOT_NOT_REGISTERED",
            Self::RootPolicyGenerationMismatch => "ROOT_POLICY_GENERATION_MISMATCH",
            Self::SourceNotAdmitted => "SOURCE_NOT_ADMITTED",
            Self::SourceAlreadyAdmittedConflict => "SOURCE_ALREADY_ADMITTED_CONFLICT",
            Self::AdmissionReceiptStale => "ADMISSION_RECEIPT_STALE",
            Self::AdmissionReceiptMismatch => "ADMISSION_RECEIPT_MISMATCH",
            Self::MembershipConflict => "MEMBERSHIP_CONFLICT",
            Self::MembershipGenerationMismatch => "MEMBERSHIP_GENERATION_MISMATCH",
            Self::ReferenceScopeEmpty => "REFERENCE_SCOPE_EMPTY",
            Self::ReferencePortfolioInvalid => "REFERENCE_PORTFOLIO_INVALID",
            Self::SourceViewAmbiguous => "SOURCE_VIEW_AMBIGUOUS",
            Self::SourceViewStale => "SOURCE_VIEW_STALE",
            Self::WorkspaceViewIncoherent => "WORKSPACE_VIEW_INCOHERENT",
            Self::NamespaceOwnershipConflict => "SOURCE_NAMESPACE_OWNERSHIP_CONFLICT",
            Self::CutoverRequired => "SOURCE_OWNER_CUTOVER_REQUIRED",
            Self::CutoverReceiptMismatch => "CUTOVER_RECEIPT_MISMATCH",
            Self::MutationOutcomeUnknown => "REGISTRY_MUTATION_OUTCOME_UNKNOWN",
            Self::CancelledBeforeCommit => "REGISTRY_CANCELLED_BEFORE_COMMIT",
            Self::DurabilityRejected => "REGISTRY_DURABILITY_REJECTED",
            Self::DurabilityExhausted => "REGISTRY_DURABILITY_EXHAUSTED",
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RegistryError {}

/// Closed kind of one durable registry journal entry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum JournalEntryKind {
    /// Admitted root registration.
    RootRegistration,
    /// Root policy fence change.
    RootPolicyChange,
    /// Root unbind with invalidation work.
    RootUnbind,
    /// Source admission commit.
    SourceAdmission,
    /// Admitted source revalidation.
    SourceRevalidation,
    /// Source binding update.
    SourceBindingUpdate,
    /// Source retirement.
    SourceRetirement,
    /// Membership bind.
    MembershipBind,
    /// Membership transition.
    MembershipTransition,
    /// Membership retirement.
    MembershipRetirement,
    /// Reference portfolio publication.
    PortfolioPublish,
    /// Namespace cutover preparation.
    CutoverPrepare,
    /// Old owner fence.
    CutoverFence,
    /// New owner activation.
    CutoverActivate,
    /// Atomic batch commit.
    BatchApply,
}

/// Content-free durable journal entry persisted through the control port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryJournalEntry {
    /// Immutable operation identity.
    pub operation_id: OpaqueId,
    /// Digest of the complete canonical request payload.
    pub mutation_digest: Blake3Digest32,
    /// Registry revision before the mutation.
    pub before_revision: u64,
    /// Registry revision after the mutation.
    pub after_revision: u64,
    /// Closed entry kind.
    pub kind: JournalEntryKind,
    /// Content-free durability receipt reference.
    pub receipt: ReceiptRef,
}

impl RegistryJournalEntry {
    /// Creates a durable journal entry from validated parts.
    #[must_use]
    pub const fn new(
        operation_id: OpaqueId,
        mutation_digest: Blake3Digest32,
        before_revision: u64,
        after_revision: u64,
        kind: JournalEntryKind,
        receipt: ReceiptRef,
    ) -> Self {
        Self {
            operation_id,
            mutation_digest,
            before_revision,
            after_revision,
            kind,
            receipt,
        }
    }
}

/// Vendor-neutral durability boundary for registry receipts.
///
/// Implementations persist content-free journal entries only. They never see
/// source bytes, extracted text, vectors, Qdrant IDs or concrete store
/// handles. The registry never touches a concrete redb/filesystem handle; all
/// receipt persistence flows through this trait.
pub trait RegistryControlPort: Port {
    /// Persists one journal entry durably under a stable mutation identity.
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

/// Maps a cancelled operation context to a closed registry failure.
pub fn cancelled_before_commit<C: CancellationProbe>(
    context: &OperationContext<C>,
) -> Result<(), RegistryError> {
    context
        .preflight()
        .map_err(|_| RegistryError::CancelledBeforeCommit)
}

/// Builds the stable mutation identity for one registry operation.
#[must_use]
pub fn registry_mutation(operation_id: &OpaqueId) -> MutationIdentity {
    use search_ports::IdempotencyClass;
    MutationIdentity::new(operation_id.clone(), IdempotencyClass::RetrySameIdentity)
}

/// Portable redacted port error carrying a closed registry reason.
pub type RegistryPortError = PortError<RegistryError>;

/// Maps a closed registry failure into a redacted port error.
#[must_use]
pub fn registry_port_error(
    kind: PortErrorKind,
    retryability: PortRetryability,
    reason: RegistryError,
    operation_id: Option<OpaqueId>,
) -> RegistryPortError {
    PortError::new(
        kind,
        retryability,
        DisclosureClass::Redacted,
        reason,
        operation_id,
    )
}

/// Bounded process-local fake implementing [`RegistryControlPort`].
///
/// Tests use this fake to prove receipt persistence flows through the
/// vendor-neutral port. Production binds a durable implementation (for example
/// through the control journal); the registry itself never branches on the
/// concrete type.
#[derive(Clone, Debug)]
pub struct InMemoryRegistryJournal<C> {
    entries: BTreeMap<OpaqueId, (RegistryJournalEntry, MutationIdentity)>,
    max_entries: usize,
    marker: core::marker::PhantomData<fn() -> C>,
}

impl<C> InMemoryRegistryJournal<C> {
    /// Creates a bounded fake journal.
    pub fn new(max_entries: usize) -> Result<Self, RegistryError> {
        if max_entries == 0 {
            return Err(RegistryError::InvalidLimits);
        }
        Ok(Self {
            entries: BTreeMap::new(),
            max_entries,
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
    fn persist_entry(
        &mut self,
        entry: &RegistryJournalEntry,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<PortReceipt, Self::Error> {
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
