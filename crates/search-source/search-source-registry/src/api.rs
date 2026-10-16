//! Public entry module for `search-source-registry`.
//!
//! All cross-package operations enter through this module. It re-exports the
//! exact logical surface and composes the package-local states into single
//! revision-guarded durable mutations. Every durable mutation persists its
//! content-free receipt through the vendor-neutral [`crate::error::RegistryControlPort`].

pub use crate::recovery::source_namespace_v2::{
    NativeNamespaceRequestV2, NativeNamespacePreparationV2, PreparedNativeNamespaceV2,
    CommittedNativeNamespaceV2, prepare_native_namespace_v2, commit_native_namespace_v2,
};
pub use crate::recovery::source_v2::{
    PreparedSourceAdmissionV2, SourceAdmissionPreparationV2, SourceAdmissionRecoveryV2,
    SourceAdmissionV2Error, commit_source_admission_v2, prepare_source_admission_v2,
    recover_source_admission_v2,
};
pub use crate::recovery::{
    PreparedSourceMembershipV2, SourceMembershipReceiptV2, commit_source_membership_v2,
    prepare_source_membership_transition_v2, prepare_source_membership_v2,
};
pub use search_source_records::source_request_v2::{
    SourceAdmissionRequestV2, SourceNamespaceSelectionV2, SourceOwnerScopeV2,
};

pub use crate::cutover::{
    CutoverPreparation, CutoverRecoveryDecision, NamespaceCutoverState, NamespaceOwnershipCommand,
    OwnerFenceReceipt, SourceOwnerActivationReceipt, VerifiedCutoverReceipt,
    activate_new_owner as cutover_activate_new_owner, fence_old_owner as cutover_fence_old_owner,
    prepare_namespace_cutover as cutover_prepare, recover_cutover as cutover_recover,
    transition_namespace_owner as cutover_transition_owner,
    verify_cutover_receipt as cutover_verify_receipt,
};
pub use crate::error::{
    DEFAULT_REGISTRY_LIMITS, InMemoryRegistryJournal, JournalEntryKind, RegistryControlPort,
    RegistryError, RegistryJournalEntry, RegistryLimits, RegistryPortError,
};
pub use crate::membership::{
    BindMembershipRequest, MembershipCommand, MembershipKey, MembershipLifecycle,
    MembershipObligation, MembershipPolicies, MembershipRecord, MembershipTransitionReceipt,
    NewMembership, PermanentMembershipProvenanceV2, PermanentMembershipRecordV2,
    SourceMembershipReceipt, derive_membership_id,
};
pub use crate::portfolio::{
    PortfolioItem, PortfolioReceipt, PublishPortfolioRequest, ReferencePortfolioRecord,
};
pub use crate::recovery::{
    BatchItemOutcome, BatchItemStatus, NamespaceCutover, RegistryBatch, RegistryBatchReceipt,
    RegistryChange, RegistryMutationRecovery, RegistryOperation, RegistryReceipt, SourceRegistry,
    apply_admission_batch as recovery_apply_batch, recover_registry_mutation as recovery_recover,
};
pub use crate::revision::{
    SourceRevisionControlPort, SourceRevisionCurrentness, SourceRevisionHead, SourceRevisionIdPort,
    SourceRevisionMutation, SourceRevisionMutationReadback, SourceRevisionRegistrationReceipt,
    SourceRevisionRegistrationRequest, SourceRevisionSourceState,
    decode_source_revision_mutation_readback, decode_source_revision_record,
    encode_source_revision_mutation_readback, encode_source_revision_record,
    register_source_revision,
};
pub use crate::root::{
    RegisterRootRequest, RootPolicyChangeReceipt, RootPolicyObligation, RootRecord,
    RootRegistrationReceipt, RootStatus, RootUnbindReceipt,
};
pub use crate::snapshot::{
    OriginalAdmissionAvailability, RegistrySnapshot, RegistrySnapshotDigest,
    RegistrySnapshotDigestV5, RegistrySnapshotDigestV6, ValidatedRegistrySnapshot, snapshot_digest,
    snapshot_digest_v5, snapshot_digest_v6, validate_registry_snapshot,
};
pub use crate::source::{
    AdmissionBindingProof, AdmittedSourceReceipt, RegisteredSource, SourceAdmissionUpdateReceipt,
    SourceLifecycle, SourceRevalidationObligation,
};
pub use crate::view::{
    DenominatorScope, RedactedRegistryView, RegistryInventoryAdapter, ResolveSourceViewRequest,
    ResolveWorkspaceViewRequest, ResolvedSourceView, VerifiedRegistryView, WorkspaceViewResolution,
    redacted_registry_view, resolve_source_view as view_resolve_source,
    resolve_workspace_view as view_resolve_workspace, verify_view as view_verify,
};

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, InstallationIncarnationId, NonZeroRevision, OpaqueId, ReceiptRef,
    SourceIdentity, SourceMembership, SourceMembershipId, SourceNamespaceId,
    SourceNamespaceOwnership, SourceOwnerCutoverReceipt,
};
use search_ports::{CancellationProbe, OperationContext};

use crate::error::cancelled_before_commit;

// ---------------------------------------------------------------------------
// Snapshot helpers
// ---------------------------------------------------------------------------

/// Builds an immutable snapshot from live registry state.
#[must_use]
pub fn registry_snapshot(registry: &SourceRegistry) -> RegistrySnapshot {
    let ownerships = registry
        .namespaces()
        .iter()
        .map(|(namespace, state)| (*namespace, state.ownership.clone()))
        .collect();
    RegistrySnapshot {
        revision: registry.revision(),
        roots: registry.roots().clone(),
        sources: registry.sources().clone(),
        memberships: registry.memberships().clone(),
        portfolios: registry.portfolios().clone(),
        portfolio_history: registry.portfolio_history().clone(),
        active_generations: registry.active_generations_snapshot().clone(),
        ownerships,
        namespace_states: registry.namespaces().clone(),
    }
}

/// Validates the live registry snapshot.
pub fn validate_live_snapshot(
    registry: &SourceRegistry,
) -> Result<ValidatedRegistrySnapshot, RegistryError> {
    validate_registry_snapshot(&registry_snapshot(registry), registry.limits())
}

/// Computes the canonical digest of live registry state.
pub fn live_snapshot_digest(
    registry: &SourceRegistry,
) -> Result<RegistrySnapshotDigest, RegistryError> {
    snapshot_digest(&registry_snapshot(registry))
}

/// Computes the explicit version-6 digest including permanent typed memberships.
pub fn live_snapshot_digest_v6(
    registry: &SourceRegistry,
) -> Result<crate::snapshot::RegistrySnapshotDigestV6, RegistryError> {
    snapshot_digest_v6(&registry_snapshot(registry))
}

// ---------------------------------------------------------------------------
// Root operations
// ---------------------------------------------------------------------------

/// Registers one explicit root durably through the control port.
#[allow(clippy::too_many_arguments)]
pub fn register_root<C, P>(
    registry: &mut SourceRegistry,
    request: &RegisterRootRequest,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RootRegistrationReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    crate::root::register_root(
        registry,
        request,
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )
}

/// Commits a root policy fence durably through the control port.
#[allow(clippy::too_many_arguments)]
pub fn update_root_policy<C, P>(
    registry: &mut SourceRegistry,
    root_binding_id: search_contracts::RootBindingId,
    expected_record_revision: NonZeroRevision,
    new_policy_fingerprint: Blake3Digest32,
    new_policy_revision: NonZeroRevision,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RootPolicyChangeReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    crate::root::update_root_policy(
        registry,
        root_binding_id,
        expected_record_revision,
        new_policy_fingerprint,
        new_policy_revision,
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )
}

/// Fences a root durably through the control port.
pub fn unbind_root<C, P>(
    registry: &mut SourceRegistry,
    root_binding_id: search_contracts::RootBindingId,
    expected_record_revision: NonZeroRevision,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RootUnbindReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    crate::root::unbind_root(
        registry,
        root_binding_id,
        expected_record_revision,
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )
}

// ---------------------------------------------------------------------------
// Source operations
// ---------------------------------------------------------------------------

/// Admits one source durably through the control port.
#[allow(clippy::too_many_arguments)]
pub fn admit_source<C, P>(
    registry: &mut SourceRegistry,
    identity: &SourceIdentity,
    binding: &search_source_identity::SourceBinding,
    grant: &search_source_admission::AdmissionReceipt,
    assignment: &AdmissionBindingProof,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<AdmittedSourceReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let expected = registry.revision();
    let next = expected
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let limits = registry.limits();
    let (roots, sources) = registry.roots_and_sources_mut();
    let out = crate::source::admit_source(
        sources,
        roots,
        expected,
        next,
        identity,
        binding,
        grant,
        assignment,
        operation_id,
        mutation_digest,
        receipt,
        limits,
        control_port,
        context,
    )?;
    if !out.replayed {
        registry.set_revision(next);
    }
    Ok(out)
}

/// Revalidates one admitted source durably through the control port.
#[allow(clippy::too_many_arguments)]
pub fn revalidate_admitted_source<C, P>(
    registry: &mut SourceRegistry,
    identity: &SourceIdentity,
    expected_source_revision: NonZeroRevision,
    current_grant: &search_source_admission::AdmissionReceipt,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceAdmissionUpdateReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let expected = registry.revision();
    let next = expected
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let (roots, sources) = registry.roots_and_sources_mut();
    let out = crate::source::revalidate_admitted_source(
        sources,
        roots,
        identity,
        expected_source_revision,
        expected,
        next,
        current_grant,
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )?;
    if !out.replayed {
        registry.set_revision(next);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Membership operations
// ---------------------------------------------------------------------------

/// Binds one membership under the exact current verified admission receipt.
#[allow(clippy::too_many_arguments)]
pub fn bind_membership<C, P>(
    registry: &mut SourceRegistry,
    request: &BindMembershipRequest,
    admission_grant: &search_source_admission::AdmissionReceipt,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceMembershipReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let expected = registry.revision();
    let next = expected
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let limits = registry.limits();
    let (sources, memberships, reverse, generations) = registry.membership_parts_mut();
    let out = crate::membership::bind_membership(
        memberships,
        reverse,
        sources,
        generations,
        expected,
        next,
        request,
        admission_grant,
        operation_id,
        mutation_digest,
        receipt,
        limits,
        control_port,
        context,
    )?;
    if !out.replayed {
        registry.set_revision(next);
    }
    Ok(out)
}

/// Binds and commits the accepted permanent `SourceMembership` contract.
///
/// The UUID membership ID is preserved verbatim from the typed member. Root,
/// namespace, owner, source and admission-policy fences are checked before
/// preparation; before that fresh check, the existing C02 port resolves an
/// exact committed operation so retries survive restart and later lifecycle
/// changes without reinstalling historical membership state.
#[allow(clippy::too_many_arguments)]
pub fn bind_source_membership_v2<C, P>(
    registry: &mut SourceRegistry,
    member: &SourceMembership,
    root_binding_id: search_contracts::RootBindingId,
    expected_root_record_revision: NonZeroRevision,
    namespace: SourceNamespaceSelectionV2,
    owner: &SourceOwnerScopeV2,
    expected_source_revision: NonZeroRevision,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceMembershipReceiptV2, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    if let Some(replayed) = crate::recovery::replay_committed_source_membership_bind_v2(
        registry,
        member,
        root_binding_id,
        expected_root_record_revision,
        namespace,
        owner,
        expected_source_revision,
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )? {
        return Ok(replayed);
    }
    let prepared = crate::recovery::prepare_source_membership_v2(
        registry,
        member,
        root_binding_id,
        expected_root_record_revision,
        namespace,
        owner,
        expected_source_revision,
        operation_id,
        mutation_digest,
        receipt,
        context,
    )?;
    crate::recovery::commit_source_membership_v2(registry, &prepared, control_port, context)
}

/// Applies a policy-preserving permanent-membership lifecycle change.
#[allow(clippy::too_many_arguments)]
pub fn transition_source_membership_v2<C, P>(
    registry: &mut SourceRegistry,
    membership_id: SourceMembershipId,
    expected_membership_revision: search_contracts::MembershipRevision,
    command: crate::membership::MembershipCommand,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceMembershipReceiptV2, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    let prepared = crate::recovery::prepare_source_membership_transition_v2(
        registry,
        membership_id,
        expected_membership_revision,
        command,
        operation_id,
        mutation_digest,
        receipt,
        context,
    )?;
    crate::recovery::commit_source_membership_v2(registry, &prepared, control_port, context)
}

/// Resolves an active permanent member and exact source identity for a retained reader.
pub fn active_source_membership_v2(
    registry: &SourceRegistry,
    membership_id: SourceMembershipId,
) -> Result<(&SourceMembership, &SourceIdentity), RegistryError> {
    registry.active_source_membership_v2(membership_id)
}

/// Applies one closed membership transition durably through the control port.
#[allow(clippy::too_many_arguments)]
pub fn transition_membership<C, P>(
    registry: &mut SourceRegistry,
    key: &MembershipKey,
    command: crate::membership::MembershipCommand,
    expected_membership_revision: NonZeroRevision,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<MembershipTransitionReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let expected = registry.revision();
    let next = expected
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let out = crate::membership::transition_membership(
        registry.memberships_mut(),
        key,
        command,
        expected_membership_revision,
        expected,
        next,
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )?;
    if !out.replayed {
        registry.set_revision(next);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Portfolio operations
// ---------------------------------------------------------------------------

/// Publishes a guarded portfolio revision through the existing receipt port.
#[allow(clippy::too_many_arguments)]
pub fn publish_reference_portfolio<C, P>(
    registry: &mut SourceRegistry,
    request: &PublishPortfolioRequest,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<PortfolioReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let expected = registry.revision();
    let next = expected
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let limits = registry.limits();
    let (sources, memberships, reverse, portfolios, history) = registry.portfolio_parts_mut();
    let out = crate::portfolio::publish_reference_portfolio(
        portfolios,
        history,
        memberships,
        reverse,
        sources,
        expected,
        next,
        request,
        operation_id,
        mutation_digest,
        receipt,
        limits,
        control_port,
        context,
    )?;
    if !out.replayed {
        registry.set_revision(next);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// View operations (pure; no control port)
// ---------------------------------------------------------------------------

/// Resolves one coherent immutable source view.
pub fn resolve_source_view(
    registry: &SourceRegistry,
    request: &ResolveSourceViewRequest,
    access_allowed: &BTreeSet<SourceMembershipId>,
    currentness_generation: NonZeroRevision,
) -> Result<ResolvedSourceView, RegistryError> {
    let snapshot = registry_snapshot(registry);
    let validated = validate_registry_snapshot(&snapshot, registry.limits())?;
    view_resolve_source(
        request,
        &validated,
        access_allowed,
        currentness_generation,
        registry.limits(),
    )
}

/// Resolves one coherent workspace view.
pub fn resolve_workspace_view(
    registry: &SourceRegistry,
    request: &ResolveWorkspaceViewRequest,
) -> Result<WorkspaceViewResolution, RegistryError> {
    let snapshot = registry_snapshot(registry);
    let validated = validate_registry_snapshot(&snapshot, registry.limits())?;
    view_resolve_workspace(request, &validated)
}

/// Verifies a view against current snapshot and owner generations.
pub fn verify_view(
    view: &ResolvedSourceView,
    registry: &SourceRegistry,
    expected_owner_generations: &BTreeMap<
        search_contracts::SourceNamespaceId,
        search_contracts::SourceOwnerGeneration,
    >,
) -> Result<VerifiedRegistryView, RegistryError> {
    let snapshot = registry_snapshot(registry);
    let validated = validate_registry_snapshot(&snapshot, registry.limits())?;
    view_verify(view, &validated, expected_owner_generations)
}

/// Returns the redacted view for one corpus.
#[must_use]
pub fn redacted_view(
    registry: &SourceRegistry,
    corpus_id: &OpaqueId,
) -> Option<RedactedRegistryView> {
    let snapshot = registry_snapshot(registry);
    validate_registry_snapshot(&snapshot, registry.limits()).ok()?;
    let validated = validate_registry_snapshot(&snapshot, registry.limits()).ok()?;
    Some(redacted_registry_view(&validated, corpus_id))
}

// ---------------------------------------------------------------------------
// Cutover operations
// ---------------------------------------------------------------------------

/// Prepares one namespace cutover durably without activating the new owner.
#[allow(clippy::too_many_arguments)]
pub fn prepare_namespace_cutover<C, P>(
    registry: &mut SourceRegistry,
    namespace_id: SourceNamespaceId,
    proposed_owner: OpaqueId,
    proposed_incarnation: InstallationIncarnationId,
    affected_sources: Vec<SourceIdentity>,
    affected_memberships: Vec<SourceMembershipId>,
    evidence_digest: Blake3Digest32,
    cutover_id: search_contracts::CutoverId,
    cutover_receipt_ref: ReceiptRef,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<CutoverPreparation, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let expected = registry.revision();
    let next = expected
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let limits = registry.limits();
    let out = cutover_prepare(
        registry.namespaces_mut(),
        expected,
        next,
        namespace_id,
        proposed_owner,
        proposed_incarnation,
        affected_sources,
        affected_memberships,
        evidence_digest,
        cutover_id,
        cutover_receipt_ref,
        operation_id,
        mutation_digest,
        receipt,
        limits,
        control_port,
        context,
    )?;
    if !out.replayed {
        registry.set_revision(next);
    }
    Ok(out)
}

/// Fences the old owner durably before any new activation.
pub fn fence_old_owner<C, P>(
    registry: &mut SourceRegistry,
    preparation: &CutoverPreparation,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<OwnerFenceReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let expected = registry.revision();
    let next = expected
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let out = cutover_fence_old_owner(
        registry.namespaces_mut(),
        expected,
        next,
        preparation,
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )?;
    if !out.replayed {
        registry.set_revision(next);
    }
    Ok(out)
}

/// Activates the new owner after the durable fence.
#[allow(clippy::too_many_arguments)]
pub fn activate_new_owner<C, P>(
    registry: &mut SourceRegistry,
    preparation: &CutoverPreparation,
    fence: &OwnerFenceReceipt,
    cutover_receipt: &SourceOwnerCutoverReceipt,
    new_epoch: search_contracts::OwnerEpoch,
    operation_id: &OpaqueId,
    mutation_digest: Blake3Digest32,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceOwnerActivationReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let expected = registry.revision();
    let next = expected
        .checked_add(1)
        .ok_or(RegistryError::RegistryRevisionOverflow)?;
    let out = cutover_activate_new_owner(
        registry.namespaces_mut(),
        expected,
        next,
        preparation,
        fence,
        cutover_receipt,
        new_epoch,
        operation_id,
        mutation_digest,
        receipt,
        control_port,
        context,
    )?;
    if !out.replayed {
        registry.set_revision(next);
    }
    Ok(out)
}

/// Verifies a cutover receipt against old/new ownership and the snapshot.
pub fn verify_cutover_receipt(
    registry: &SourceRegistry,
    receipt: &SourceOwnerCutoverReceipt,
    old_state: &SourceNamespaceOwnership,
    new_state: &SourceNamespaceOwnership,
    expected_sources: usize,
    expected_memberships: usize,
) -> Result<VerifiedCutoverReceipt, RegistryError> {
    let snapshot = registry_snapshot(registry);
    let validated = validate_registry_snapshot(&snapshot, registry.limits())?;
    cutover_verify_receipt(
        receipt,
        old_state,
        new_state,
        &validated,
        expected_sources,
        expected_memberships,
    )
}

/// Recovers durable cutover state without activating anything.
pub fn recover_cutover<C, P>(
    registry: &SourceRegistry,
    namespace_id: SourceNamespaceId,
    operation_id: &OpaqueId,
    control_port: &P,
    context: &OperationContext<C>,
) -> Result<CutoverRecoveryDecision, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    cutover_recover(
        registry.namespaces(),
        namespace_id,
        operation_id,
        control_port,
        context,
    )
}

/// Applies one closed ownership command purely through `search-domain`.
pub fn transition_namespace_owner(
    current: &SourceNamespaceOwnership,
    command: &NamespaceOwnershipCommand,
    next_revision: NonZeroRevision,
    next_generation: search_contracts::SourceOwnerGeneration,
    next_status: search_contracts::NamespaceOwnershipStatus,
    next_owner: Option<(
        OpaqueId,
        InstallationIncarnationId,
        search_contracts::OwnerEpoch,
    )>,
    cutover_receipt: Option<&SourceOwnerCutoverReceipt>,
) -> Result<SourceNamespaceOwnership, RegistryError> {
    cutover_transition_owner(
        current,
        command,
        next_revision,
        next_generation,
        next_status,
        next_owner,
        cutover_receipt,
    )
}

// ---------------------------------------------------------------------------
// Recovery operations
// ---------------------------------------------------------------------------

/// Recovers an unknown mutation outcome by exact readback.
pub fn recover_registry_mutation<C, P>(
    registry: &SourceRegistry,
    operation_id: &OpaqueId,
    expected_digest: Blake3Digest32,
    control_port: &P,
    context: &OperationContext<C>,
) -> Result<RegistryMutationRecovery, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    recovery_recover(
        registry,
        operation_id,
        expected_digest,
        control_port,
        context,
    )
}

/// Applies one admission batch with one outcome per input.
pub fn apply_admission_batch<C, P>(
    registry: &mut SourceRegistry,
    batch: RegistryBatch,
    receipt: &ReceiptRef,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<RegistryBatchReceipt, RegistryError>
where
    C: CancellationProbe,
    P: RegistryControlPort<Cancellation = C>,
{
    recovery_apply_batch(registry, batch, receipt, control_port, context)
}
pub use search_source_records::source_request_v2::source_receipt;
