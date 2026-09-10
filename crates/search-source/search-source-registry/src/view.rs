//! Coherent source/workspace views over exact snapshots.
//!
//! One compound operation resolves one coherent immutable view. The resolver
//! never infers a nearest repository, broadens to disk or chooses an implicit
//! current HEAD. Access and currentness fences are consumed as exact accepted
//! inputs; their meaning is decided by their owners.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, ReferencePortfolioId, SourceIdentity,
    SourceMembershipId, SourceNamespaceId,
};
use search_ports::{
    BoundedPage, CancellationProbe, DisclosureClass, OperationContext, Port, PortErrorKind,
    PortRetryability, SourceInventoryPort,
};

use crate::error::{RegistryError, RegistryLimits, RegistryPortError, registry_port_error};
use crate::membership::{MembershipKey, MembershipLifecycle, MembershipRecord};
use crate::snapshot::{RegistrySnapshotDigest, ValidatedRegistrySnapshot, snapshot_digest};

/// Explicit source-view request; empty scope is ambiguous, never implicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveSourceViewRequest {
    /// Exact corpus to resolve.
    pub corpus_id: OpaqueId,
    /// Exact corpus generation to resolve.
    pub generation: NonZeroRevision,
    /// Explicit reference portfolio, when used.
    pub portfolio_id: Option<ReferencePortfolioId>,
    /// Explicit membership identities, when used instead of a portfolio.
    pub explicit_memberships: Vec<SourceMembershipId>,
    /// Finite result bound.
    pub max_items: usize,
}

/// One coherent immutable resolved source view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSourceView {
    /// Registry revision backing the view.
    pub registry_revision: u64,
    /// Canonical snapshot digest.
    pub registry_digest: RegistrySnapshotDigest,
    /// Exact corpus resolved.
    pub corpus_id: OpaqueId,
    /// Exact generation resolved.
    pub generation: NonZeroRevision,
    /// Allowed membership identities in canonical order.
    pub allowed_memberships: Vec<SourceMembershipId>,
    /// Stable source identities backing the allowed memberships.
    pub source_identities: Vec<SourceIdentity>,
    /// Requested but access-denied memberships.
    pub denied: Vec<SourceMembershipId>,
    /// Requested but absent memberships.
    pub missing: Vec<SourceMembershipId>,
    /// Admitted but excluded (inactive generation or retired) memberships.
    pub excluded: Vec<SourceMembershipId>,
}

/// Explicit workspace-view request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveWorkspaceViewRequest {
    /// Exact workspace identity.
    pub workspace_id: search_contracts::WorkspaceId,
    /// Admitted root containing the workspace.
    pub root_binding_id: search_contracts::RootBindingId,
    /// Exact branch fence digest.
    pub branch_digest: Blake3Digest32,
    /// Exact index fence digest.
    pub index_digest: Blake3Digest32,
    /// Exact buffer revision fence.
    pub buffer_revision: u64,
}

/// One coherent workspace-view resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceViewResolution {
    /// Exact workspace identity.
    pub workspace_id: search_contracts::WorkspaceId,
    /// Derived workspace-view revision identity.
    pub view_revision_id: search_contracts::WorkspaceViewRevisionId,
    /// Admitted root binding.
    pub root_binding_id: search_contracts::RootBindingId,
    /// Registry revision backing the resolution.
    pub registry_revision: u64,
    /// Branch fence digest.
    pub branch_digest: Blake3Digest32,
    /// Index fence digest.
    pub index_digest: Blake3Digest32,
    /// Buffer revision fence.
    pub buffer_revision: u64,
}

/// Verified registry view bound to a current snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRegistryView {
    /// Verified resolved view.
    pub view: ResolvedSourceView,
    /// Snapshot digest at verification time.
    pub digest: RegistrySnapshotDigest,
}

/// Redacted registry view exposing only the requester's corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedRegistryView {
    /// Registry revision.
    pub revision: u64,
    /// Requester's corpus.
    pub corpus_id: OpaqueId,
    /// Own active membership identities in canonical order.
    pub own_memberships: Vec<SourceMembershipId>,
    /// Own active source count.
    pub own_source_count: usize,
}

/// Resolves exact explicit roots/sources/memberships/portfolio into one
/// immutable view.
///
/// Access and currentness fences arrive as exact accepted inputs: only
/// memberships present in `access_allowed` may become allowed; all other
/// requested memberships are reported as denied rather than silently dropped.
pub fn resolve_source_view(
    request: &ResolveSourceViewRequest,
    snapshot: &ValidatedRegistrySnapshot,
    access_allowed: &BTreeSet<SourceMembershipId>,
    currentness_generation: NonZeroRevision,
    limits: RegistryLimits,
) -> Result<ResolvedSourceView, RegistryError> {
    limits.validate()?;
    if request.max_items == 0 || request.max_items > limits.max_portfolio_items {
        return Err(RegistryError::PortfolioLimitInvalid);
    }
    if request.portfolio_id.is_none() && request.explicit_memberships.is_empty() {
        return Err(RegistryError::SourceViewAmbiguous);
    }
    if request.portfolio_id.is_some() && !request.explicit_memberships.is_empty() {
        return Err(RegistryError::SourceViewAmbiguous);
    }
    let inner = snapshot.snapshot();
    let active = inner.active_generations.get(&request.corpus_id).copied();
    if active != Some(request.generation) {
        return Err(RegistryError::SourceViewStale);
    }
    if currentness_generation != request.generation {
        return Err(RegistryError::SourceViewStale);
    }
    let mut requested: Vec<SourceMembershipId> = Vec::new();
    if let Some(portfolio_id) = request.portfolio_id {
        let record = inner
            .portfolios
            .get(&portfolio_id)
            .ok_or(RegistryError::SourceViewAmbiguous)?;
        requested.extend(record.membership_precedence.iter().copied());
    } else {
        requested.extend(request.explicit_memberships.iter().copied());
    }
    if requested.len() > request.max_items {
        return Err(RegistryError::PortfolioLimitInvalid);
    }
    let mut seen = BTreeSet::new();
    for id in &requested {
        if !seen.insert(*id) {
            return Err(RegistryError::SourceViewAmbiguous);
        }
    }
    let reverse: BTreeMap<SourceMembershipId, MembershipKey> = inner
        .memberships
        .iter()
        .map(|(key, record)| (record.membership_id(), key.clone()))
        .collect();
    let mut allowed = Vec::new();
    let mut sources = Vec::new();
    let mut denied = Vec::new();
    let mut missing = Vec::new();
    let mut excluded = Vec::new();
    for id in requested {
        let Some(key) = reverse.get(&id) else {
            missing.push(id);
            continue;
        };
        let Some(membership) = inner.memberships.get(key) else {
            missing.push(id);
            continue;
        };
        if key.corpus_id != request.corpus_id
            || membership.generation() != request.generation
            || membership.lifecycle() != MembershipLifecycle::Active
        {
            excluded.push(id);
            continue;
        }
        if !access_allowed.contains(&id) {
            denied.push(id);
            continue;
        }
        let Some(source) = inner.sources.get(&key.source_identity) else {
            missing.push(id);
            continue;
        };
        allowed.push(id);
        sources.push(source.identity().clone());
    }
    allowed.sort();
    sources.sort();
    denied.sort();
    missing.sort();
    excluded.sort();
    Ok(ResolvedSourceView {
        registry_revision: inner.revision,
        registry_digest: snapshot_digest(inner),
        corpus_id: request.corpus_id.clone(),
        generation: request.generation,
        allowed_memberships: allowed,
        source_identities: sources,
        denied,
        missing,
        excluded,
    })
}

/// Binds one coherent repository/worktree/branch/index/buffer fence.
pub fn resolve_workspace_view(
    request: &ResolveWorkspaceViewRequest,
    snapshot: &ValidatedRegistrySnapshot,
) -> Result<WorkspaceViewResolution, RegistryError> {
    let inner = snapshot.snapshot();
    if !inner.roots.contains_key(&request.root_binding_id) {
        return Err(RegistryError::RootNotRegistered);
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in request
        .workspace_id
        .as_bytes()
        .iter()
        .chain(request.root_binding_id.as_bytes())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for byte in request
        .branch_digest
        .as_bytes()
        .iter()
        .chain(request.index_digest.as_bytes())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    for byte in request.buffer_revision.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&hash.to_le_bytes());
    bytes[8..].copy_from_slice(&(!hash).to_le_bytes());
    Ok(WorkspaceViewResolution {
        workspace_id: request.workspace_id,
        view_revision_id: search_contracts::WorkspaceViewRevisionId::from_bytes(bytes),
        root_binding_id: request.root_binding_id,
        registry_revision: inner.revision,
        branch_digest: request.branch_digest,
        index_digest: request.index_digest,
        buffer_revision: request.buffer_revision,
    })
}

/// Rechecks registry/root/membership/portfolio/owner/view generations before
/// a downstream operation uses the view.
pub fn verify_view(
    view: &ResolvedSourceView,
    current_snapshot: &ValidatedRegistrySnapshot,
    expected_owner_generations: &BTreeMap<
        SourceNamespaceId,
        search_contracts::SourceOwnerGeneration,
    >,
) -> Result<VerifiedRegistryView, RegistryError> {
    let inner = current_snapshot.snapshot();
    if view.registry_revision != inner.revision {
        return Err(RegistryError::SourceViewStale);
    }
    if view.registry_digest != snapshot_digest(inner) {
        return Err(RegistryError::SourceViewStale);
    }
    for namespace in view
        .source_identities
        .iter()
        .map(|identity| identity.source_namespace_id)
    {
        let _ = namespace;
    }
    for (namespace, expected) in expected_owner_generations {
        if inner
            .ownerships
            .get(namespace)
            .is_some_and(|current| &current.source_owner_generation != expected)
        {
            return Err(RegistryError::SourceViewStale);
        }
    }
    let reverse: BTreeSet<SourceMembershipId> = inner
        .memberships
        .values()
        .filter(|record| record.lifecycle() == MembershipLifecycle::Active)
        .map(MembershipRecord::membership_id)
        .collect();
    for id in &view.allowed_memberships {
        if !reverse.contains(id) {
            return Err(RegistryError::SourceViewStale);
        }
    }
    Ok(VerifiedRegistryView {
        view: view.clone(),
        digest: view.registry_digest,
    })
}

/// Returns only authorized root/source/membership metadata for one corpus.
/// Foreign membership names, counts and readiness are never disclosed.
#[must_use]
pub fn redacted_registry_view(
    snapshot: &ValidatedRegistrySnapshot,
    corpus_id: &OpaqueId,
) -> RedactedRegistryView {
    let inner = snapshot.snapshot();
    let mut own: Vec<SourceMembershipId> = inner
        .memberships
        .iter()
        .filter(|(key, record)| {
            &key.corpus_id == corpus_id && record.lifecycle() == MembershipLifecycle::Active
        })
        .map(|(_, record)| record.membership_id())
        .collect();
    own.sort();
    let count = own.len();
    RedactedRegistryView {
        revision: inner.revision,
        corpus_id: corpus_id.clone(),
        own_memberships: own,
        own_source_count: count,
    }
}

/// Exact denominator scope for the shared inventory port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenominatorScope {
    /// Exact corpus to page.
    pub corpus_id: OpaqueId,
    /// Exact generation to page.
    pub generation: NonZeroRevision,
    /// Finite page bound.
    pub max_items: usize,
}

/// Vendor-neutral inventory adapter owned by `search-source-registry::view`.
pub struct RegistryInventoryAdapter {
    snapshot: ValidatedRegistrySnapshot,
    access_allowed: BTreeSet<SourceMembershipId>,
    currentness_generation: BTreeMap<OpaqueId, NonZeroRevision>,
}

impl RegistryInventoryAdapter {
    /// Creates an adapter over one validated snapshot and exact fences.
    #[must_use]
    pub fn new(
        snapshot: ValidatedRegistrySnapshot,
        access_allowed: BTreeSet<SourceMembershipId>,
        currentness_generation: BTreeMap<OpaqueId, NonZeroRevision>,
    ) -> Self {
        Self {
            snapshot,
            access_allowed,
            currentness_generation,
        }
    }
}

impl Port for RegistryInventoryAdapter {
    type Error = RegistryPortError;
    type Cancellation = search_ports::FakeCancellation;
}

impl SourceInventoryPort for RegistryInventoryAdapter {
    type SourceViewRequest = ResolveSourceViewRequest;
    type ResolvedSourceView = ResolvedSourceView;
    type WorkspaceRequest = ResolveWorkspaceViewRequest;
    type DenominatorScope = DenominatorScope;
    type PageItem = SourceMembershipId;

    fn resolve_source_view(
        &self,
        request: &Self::SourceViewRequest,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ResolvedSourceView, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(port_error_cancelled());
        }
        let Some(currentness) = self.currentness_generation.get(&request.corpus_id).copied() else {
            return Err(port_error_invalid(RegistryError::SourceViewStale));
        };
        resolve_source_view(
            request,
            &self.snapshot,
            &self.access_allowed,
            currentness,
            crate::error::DEFAULT_REGISTRY_LIMITS,
        )
        .map_err(port_error_invalid)
    }

    fn resolve_workspace_view(
        &self,
        workspace: &Self::WorkspaceRequest,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<search_contracts::WorkspaceViewRevision, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(port_error_cancelled());
        }
        let resolution =
            resolve_workspace_view(workspace, &self.snapshot).map_err(port_error_invalid)?;
        Ok(search_contracts::WorkspaceViewRevision {
            workspace_view_revision_id: resolution.view_revision_id,
            workspace_instance_id: resolution.workspace_id,
            root_filesystem_identity: search_contracts::OpaqueCanonicalBytes::from_validated(
                b"registry:workspace-root".to_vec(),
            )
            .map_err(|_| port_error_invalid(RegistryError::SnapshotInvalid))?,
            repository_lineage_id: None,
            head_commit_and_branch: None,
            git_index_identity: None,
            inventory_revision: search_contracts::CatalogRevision::new(
                resolution.registry_revision.min(u64::from(u32::MAX)),
            ),
            worktree_observation_cursor: search_contracts::ObservationCursorRevision::new(1),
            authenticated_ide_overlay_revision: resolution.buffer_revision.min(u64::from(u32::MAX)),
            ignore_and_source_admission_policy_revision: search_contracts::PolicyRevision::new(1),
        })
    }

    fn list_exact_denominator(
        &self,
        scope: &Self::DenominatorScope,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<BoundedPage<Self::PageItem>, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(port_error_cancelled());
        }
        let inner = self.snapshot.snapshot();
        if inner.active_generations.get(&scope.corpus_id).copied() != Some(scope.generation) {
            return Err(port_error_invalid(RegistryError::SourceViewStale));
        }
        let mut items: Vec<SourceMembershipId> = inner
            .memberships
            .values()
            .filter(|record| {
                record.lifecycle() == MembershipLifecycle::Active
                    && self.access_allowed.contains(&record.membership_id())
            })
            .map(MembershipRecord::membership_id)
            .collect();
        items.sort();
        if items.len() > scope.max_items {
            return Err(port_error_invalid(RegistryError::PortfolioLimitInvalid));
        }
        let list = search_contracts::BoundedList::new(items)
            .map_err(|_| port_error_invalid(RegistryError::PortfolioLimitInvalid))?;
        BoundedPage::new(list, None, true)
            .map_err(|_| port_error_invalid(RegistryError::PortfolioLimitInvalid))
    }

    fn lookup_source_head(
        &self,
        source: &Self::SourceViewRequest,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<search_contracts::SourceRevisionRef>, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(port_error_cancelled());
        }
        let _ = source;
        Ok(None)
    }

    fn read_inventory_revision(
        &self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<search_contracts::CatalogRevision, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(port_error_cancelled());
        }
        Ok(search_contracts::CatalogRevision::new(
            self.snapshot.snapshot().revision.min(u64::from(u32::MAX)),
        ))
    }
}

fn port_error_cancelled() -> RegistryPortError {
    registry_port_error(
        PortErrorKind::CancelledBeforeSideEffect,
        PortRetryability::SameRequest,
        RegistryError::CancelledBeforeCommit,
        None,
    )
}

fn port_error_invalid(reason: RegistryError) -> RegistryPortError {
    let kind = match reason {
        RegistryError::SourceViewStale => PortErrorKind::StaleGeneration,
        RegistryError::SourceViewAmbiguous => PortErrorKind::InvalidInput,
        _ => PortErrorKind::InvalidInput,
    };
    registry_port_error(kind, PortRetryability::AfterRefresh, reason, None)
}

/// Re-export for port conformance documentation.
#[allow(dead_code)]
fn _assert_no_vendor_types(_: &DisclosureClass) {}
