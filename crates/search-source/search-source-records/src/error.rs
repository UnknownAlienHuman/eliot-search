//! Shared Source record failures and finite dimensions, without state or I/O.

use core::fmt;
use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};
use search_ports::{
    CancellationProbe, DisclosureClass, MutationIdentity, OperationContext, PortError,
    PortErrorKind, PortRetryability,
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
    /// Maximum retained immutable portfolio revisions across all identities.
    ///
    /// This ceiling bounds total history, not one latest record per identity.
    /// A single portfolio may hold many revisions while the aggregate stays
    /// beneath this bound.
    pub max_portfolio_revisions: usize,
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
            || self.max_portfolio_revisions == 0
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
    max_portfolio_revisions: 4_000_000,
    max_namespaces: 100_000,
};

/// Closed content-free registry failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RegistryError {
    /// Legacy root receipts lack the complete root record; no root authority can be restored.
    RootRestorationUnavailable,
    /// Original admission inputs were never retained; metadata/digest cannot restore provenance.
    AdmissionRestorationUnavailable,
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
    /// A projection binding does not exist for this source membership.
    ProjectionMembershipNotFound,
    /// An existing binding cannot be silently rebound to different coordinates.
    ProjectionMembershipConflict,
    /// A projection UUID already belongs to a different source membership.
    ProjectionMembershipIdCollision,
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
    /// Source revision data failed its canonical record validation.
    SourceRevisionRecordInvalid,
    /// The qualified source revision ID capability failed or returned a non-v4 UUID.
    RevisionIdUnavailable,
    /// A fresh source revision UUID already identifies another immutable occurrence.
    SourceRevisionIdCollision,
}

impl RegistryError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::RootRestorationUnavailable => "REGISTRY_ROOT_RESTORATION_UNAVAILABLE",
            Self::AdmissionRestorationUnavailable => "REGISTRY_ADMISSION_RESTORATION_UNAVAILABLE",
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
            Self::ProjectionMembershipNotFound => "PROJECTION_MEMBERSHIP_NOT_FOUND",
            Self::ProjectionMembershipConflict => "PROJECTION_MEMBERSHIP_CONFLICT",
            Self::ProjectionMembershipIdCollision => "PROJECTION_MEMBERSHIP_ID_COLLISION",
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
            Self::SourceRevisionRecordInvalid => "REGISTRY_SOURCE_REVISION_RECORD_INVALID",
            Self::RevisionIdUnavailable => "REGISTRY_REVISION_ID_UNAVAILABLE",
            Self::SourceRevisionIdCollision => "REGISTRY_SOURCE_REVISION_ID_COLLISION",
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RegistryError {}

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
