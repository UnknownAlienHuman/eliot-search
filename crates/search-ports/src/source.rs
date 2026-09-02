//! Source admission, inventory, ownership, safe-read, revision, and residency ports.

use search_contracts::{
    CatalogRevision, SearchObjectResidencyKey, SourceNamespaceOwnership, SourceOwnerCutoverReceipt,
    SourceRevisionRef, WorkspaceViewRevision,
};

use crate::{BoundedPage, MutationIdentity, OperationContext, PackageOpaque, Port};

/// Pure source-admission policy boundary.
pub trait SourceAdmissionPort: Port {
    /// Capability-owned admission policy input.
    type Policy: Send + Sync + 'static;
    /// Canonical normalized policy.
    type CanonicalPolicy: Send + Sync + 'static;
    /// Bounded source observation.
    type Observation: Send + Sync + 'static;
    /// Closed admission decision.
    type AdmissionDecision: Send + Sync + 'static;
    /// Immutable admission receipt.
    type AdmissionReceipt: Send + Sync + 'static;

    /// Normalizes and validates one admission policy without I/O.
    fn normalize_policy(&self, policy: &Self::Policy)
    -> Result<Self::CanonicalPolicy, Self::Error>;

    /// Evaluates one bounded observation against a canonical policy.
    fn evaluate(
        &self,
        policy: &Self::CanonicalPolicy,
        observation: &Self::Observation,
    ) -> Result<Self::AdmissionDecision, Self::Error>;

    /// Verifies that a receipt still binds the exact policy and observation.
    fn verify_receipt(
        &self,
        receipt: &Self::AdmissionReceipt,
        policy: &Self::CanonicalPolicy,
        observation: &Self::Observation,
    ) -> Result<(), Self::Error>;
}

/// Authoritative source and workspace inventory boundary.
pub trait SourceInventoryPort: Port {
    /// Requested source-view selector.
    type SourceViewRequest: Send + Sync + 'static;
    /// Exact resolved source view.
    type ResolvedSourceView: Send + Sync + 'static;
    /// Workspace selector.
    type WorkspaceRequest: Send + Sync + 'static;
    /// Exact denominator scope.
    type DenominatorScope: Send + Sync + 'static;
    /// Opaque page continuation capability encoded by the owning package.
    type PageItem: Send + Sync + 'static;

    /// Resolves an exact source view.
    fn resolve_source_view(
        &self,
        request: &Self::SourceViewRequest,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ResolvedSourceView, Self::Error>;

    /// Resolves one coherent immutable workspace-view revision.
    fn resolve_workspace_view(
        &self,
        workspace: &Self::WorkspaceRequest,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<WorkspaceViewRevision, Self::Error>;

    /// Lists a finite authoritative denominator page.
    fn list_exact_denominator(
        &self,
        scope: &Self::DenominatorScope,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<BoundedPage<Self::PageItem>, Self::Error>;

    /// Returns the exact current admitted source head, when one exists.
    fn lookup_source_head(
        &self,
        source: &Self::SourceViewRequest,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevisionRef>, Self::Error>;

    /// Reads the current authoritative inventory revision.
    fn read_inventory_revision(
        &self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<CatalogRevision, Self::Error>;
}

/// Source-namespace ownership transition boundary.
pub trait SourceOwnershipPort: Port {
    /// Source namespace selector.
    type Namespace: Send + Sync + 'static;
    /// Closed ownership transition command.
    type TransitionCommand: Send + Sync + 'static;
    /// Content-free cutover verification receipt.
    type CutoverVerificationReceipt: Send + Sync + 'static;

    /// Reads the exact current namespace owner.
    fn read_namespace_owner(
        &self,
        namespace: &Self::Namespace,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<SourceNamespaceOwnership, Self::Error>;

    /// Applies one guarded namespace-owner transition.
    fn transition_namespace_owner(
        &mut self,
        command: &Self::TransitionCommand,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<SourceNamespaceOwnership, Self::Error>;

    /// Verifies one exact cutover receipt against authoritative state.
    fn verify_cutover_receipt(
        &self,
        receipt: &SourceOwnerCutoverReceipt,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::CutoverVerificationReceipt, Self::Error>;
}

/// Final-handle, stable-read, and no-execute Git read boundary.
pub trait SafeReaderPort: Port {
    /// Untrusted source locator admitted only after final-handle verification.
    type Locator: Send + Sync + 'static;
    /// Admitted root authority.
    type AdmittedRoot: Send + Sync + 'static;
    /// Process-local resolved final source capability.
    type ResolvedSource: PackageOpaque;
    /// Stable bounded read product.
    type StableRead: Send + Sync + 'static;
    /// Repository authority used by no-execute object reads.
    type Repository: Send + Sync + 'static;
    /// Exact native object identifier.
    type ObjectId: Send + Sync + 'static;
    /// Repository-relative path selector.
    type RepositoryPath: Send + Sync + 'static;

    /// Resolves and verifies final containment of an admitted source.
    fn resolve_final_source(
        &self,
        locator: &Self::Locator,
        admitted_root: &Self::AdmittedRoot,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ResolvedSource, Self::Error>;

    /// Reads exact bounded bytes with before/after stability checks.
    fn stable_read(
        &self,
        resolved: &Self::ResolvedSource,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::StableRead, Self::Error>;

    /// Reads an exact Git object without hooks, filters, shell, prompts, or network.
    fn read_git_object_no_execute(
        &self,
        repository: &Self::Repository,
        object_id: &Self::ObjectId,
        path: Option<&Self::RepositoryPath>,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::StableRead, Self::Error>;
}

/// Immutable revision-store boundary.
pub trait SourceRevisionStorePort: Port {
    /// Stable read accepted from the safe-reader owner.
    type StableRead: Send + Sync + 'static;
    /// Retention and encryption residency input.
    type Residency: Send + Sync + 'static;
    /// Immutable admission receipt.
    type RevisionReceipt: Send + Sync + 'static;
    /// Exact verified revision reopen product.
    type VerifiedRevision: Send + Sync + 'static;
    /// Closed retention cause.
    type RetentionCause: Send + Sync + 'static;
    /// Process-local or durable retention lease.
    type RetentionLease: PackageOpaque;
    /// Content-free release receipt.
    type ReleaseReceipt: Send + Sync + 'static;
    /// Snapshot used to enumerate exact mark roots.
    type MarkSnapshot: Send + Sync + 'static;
    /// One exact mark root.
    type MarkRoot: Send + Sync + 'static;

    /// Admits an immutable source revision under an exact mutation identity.
    fn admit_revision(
        &mut self,
        stable_read: &Self::StableRead,
        residency: &Self::Residency,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::RevisionReceipt, Self::Error>;

    /// Reopens and verifies the exact immutable revision.
    fn reopen_exact(
        &self,
        revision_ref: &SourceRevisionRef,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::VerifiedRevision, Self::Error>;

    /// Acquires a finite retention lease.
    fn retain(
        &mut self,
        revision_ref: &SourceRevisionRef,
        cause: &Self::RetentionCause,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::RetentionLease, Self::Error>;

    /// Releases the exact retention lease.
    fn release_retention(
        &mut self,
        lease: &Self::RetentionLease,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::ReleaseReceipt, Self::Error>;

    /// Enumerates a finite page of exact mark roots.
    fn enumerate_mark_roots(
        &self,
        snapshot: &Self::MarkSnapshot,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<BoundedPage<Self::MarkRoot>, Self::Error>;
}

/// Residency resolution and transition-authorization boundary.
pub trait ResidencyPolicyPort: Port {
    /// Profile selector.
    type ProfileRef: Send + Sync + 'static;
    /// Source or revision descriptor.
    type Source: Send + Sync + 'static;
    /// Target residency profile.
    type TargetProfile: Send + Sync + 'static;
    /// Authorized copy or re-encryption plan.
    type TransitionPlan: Send + Sync + 'static;
    /// External transition receipt.
    type ExternalReceipt: Send + Sync + 'static;
    /// Durable content-free transition receipt.
    type TransitionReceipt: Send + Sync + 'static;

    /// Resolves the exact residency key for an admitted source.
    fn resolve_residency(
        &self,
        profile_ref: &Self::ProfileRef,
        source: &Self::Source,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<SearchObjectResidencyKey, Self::Error>;

    /// Authorizes an explicit copy or re-encryption transition.
    fn authorize_copy_or_reencrypt(
        &self,
        source_key: &SearchObjectResidencyKey,
        target_profile: &Self::TargetProfile,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::TransitionPlan, Self::Error>;

    /// Records a verified completed transition.
    fn record_transition(
        &mut self,
        receipt: &Self::ExternalReceipt,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::TransitionReceipt, Self::Error>;
}
