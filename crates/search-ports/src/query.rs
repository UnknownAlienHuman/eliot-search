//! Query access, overlay, exact-scan, and handle-store ports.

use search_contracts::{ExactExecutionReport, ExactScanPlan, SearchSourceHandle};

use crate::{BoundedStream, MutationIdentity, OperationContext, PackageOpaque, Port};

/// Non-widening grant, scope, safe-leg, and checkpoint compiler boundary.
pub trait AccessCompilerPort: Port {
    /// Untrusted signed/read grant claims.
    type GrantClaims: Send + Sync + 'static;
    /// Current authenticated client binding.
    type Binding: Send + Sync + 'static;
    /// Current authoritative access/security state.
    type LiveState: Send + Sync + 'static;
    /// Validated non-reusable grant decision.
    type ValidatedGrant: Send + Sync + 'static;
    /// Requested scope.
    type ScopeRequest: Send + Sync + 'static;
    /// Immutable query snapshot.
    type Snapshot: Send + Sync + 'static;
    /// Server-authoritative non-widened scope.
    type AuthorizedScope: Send + Sync + 'static;
    /// Route/filter proof set.
    type RouteProofs: Send + Sync + 'static;
    /// Finite safe-leg result.
    type SafeLegSet: Send + Sync + 'static;
    /// Security/currentness checkpoint descriptor.
    type Checkpoint: Send + Sync + 'static;
    /// Current security permit or contamination result.
    type SecurityPermit: Send + Sync + 'static;

    /// Validates grant claims against current binding and live state.
    fn validate_grant(
        &self,
        claims: &Self::GrantClaims,
        binding: &Self::Binding,
        live_state: &Self::LiveState,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ValidatedGrant, Self::Error>;

    /// Intersects requested scope with grant and server snapshot authority.
    fn intersect_scope(
        &self,
        request: &Self::ScopeRequest,
        grant: &Self::ValidatedGrant,
        snapshot: &Self::Snapshot,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::AuthorizedScope, Self::Error>;

    /// Compiles finite safe legs sharing canonical retrieval/IDF predicates.
    fn compile_safe_legs(
        &self,
        scope: &Self::AuthorizedScope,
        proofs: &Self::RouteProofs,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::SafeLegSet, Self::Error>;

    /// Revalidates live security/currentness immediately at a checkpoint.
    fn revalidate_checkpoint(
        &self,
        checkpoint: &Self::Checkpoint,
        live_state: &Self::LiveState,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::SecurityPermit, Self::Error>;
}

/// Saved/unsaved overlay snapshot and direct-candidate boundary.
pub trait OverlayPort: Port {
    /// Current workspace/source view.
    type View: Send + Sync + 'static;
    /// Validated grant.
    type ValidatedGrant: Send + Sync + 'static;
    /// Immutable process-local overlay snapshot.
    type OverlaySnapshot: Send + Sync + 'static;
    /// Bounded canonical shadowed-membership set.
    type ShadowedMemberships: Send + Sync + 'static;
    /// Direct overlay candidate request.
    type CandidateRequest: Send + Sync + 'static;
    /// One direct overlay candidate nomination.
    type OverlayCandidate: Send + Sync + 'static;
    /// Process-local finite candidate stream capability.
    type CandidateStreamRef: PackageOpaque;

    /// Captures one binding/grant/view-scoped immutable overlay snapshot.
    fn snapshot_overlay(
        &self,
        view: &Self::View,
        grant: &Self::ValidatedGrant,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::OverlaySnapshot, Self::Error>;

    /// Returns the exact memberships shadowed before retrieval, IDF, and counts.
    fn shadowed_memberships(
        &self,
        snapshot: &Self::OverlaySnapshot,
    ) -> Result<Self::ShadowedMemberships, Self::Error>;

    /// Produces a finite process-local stream of direct overlay nominations.
    fn direct_candidates(
        &self,
        snapshot: &Self::OverlaySnapshot,
        request: &Self::CandidateRequest,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<BoundedStream<Self::OverlayCandidate, Self::CandidateStreamRef>, Self::Error>;
}

/// Frozen-denominator exact scan boundary.
pub trait ExactScannerPort: Port {
    /// Exact-scan request.
    type ScanRequest: Send + Sync + 'static;
    /// Frozen authoritative inventory descriptor.
    type Inventory: Send + Sync + 'static;
    /// Exact retained-revision readback provider/capability.
    type Readback: Send + Sync + 'static;

    /// Compiles one finite exact plan over a frozen authoritative denominator.
    fn compile_exact_scan(
        &self,
        request: &Self::ScanRequest,
        inventory: &Self::Inventory,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<ExactScanPlan, Self::Error>;

    /// Executes every denominator item into exactly one outcome or explicit gap.
    fn execute_exact_scan(
        &self,
        plan: &ExactScanPlan,
        readback: &Self::Readback,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<ExactExecutionReport, Self::Error>;
}

/// Opaque source-handle lifecycle and expansion boundary.
pub trait HandleStorePort: Port {
    /// Exact handle subject descriptor.
    type Subject: Send + Sync + 'static;
    /// Authenticated binding descriptor.
    type Binding: Send + Sync + 'static;
    /// Finite handle limits.
    type Limits: Send + Sync + 'static;
    /// Retention receipt required for durable handles.
    type RetentionReceipt: Send + Sync + 'static;
    /// Handle expansion request.
    type ExpansionRequest: Send + Sync + 'static;
    /// Current authoritative access/currentness/lifecycle state.
    type LiveState: Send + Sync + 'static;
    /// Exact source-backed expansion result.
    type ExpansionResult: Send + Sync + 'static;
    /// Invalidation scope.
    type InvalidationScope: Send + Sync + 'static;
    /// Content-free invalidation receipt.
    type InvalidationReceipt: Send + Sync + 'static;
    /// Clock instant used for expiry.
    type ExpiryInstant: Send + Sync + 'static;
    /// Content-free expiry receipt.
    type ExpiryReceipt: Send + Sync + 'static;

    /// Mints a finite restart-invalid ephemeral handle.
    fn mint_ephemeral(
        &mut self,
        subject: &Self::Subject,
        binding: &Self::Binding,
        limits: &Self::Limits,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<SearchSourceHandle, Self::Error>;

    /// Mints a durable handle only for a retained immutable source revision.
    fn mint_durable(
        &mut self,
        subject: &Self::Subject,
        retention_receipt: &Self::RetentionReceipt,
        binding: &Self::Binding,
        limits: &Self::Limits,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<SearchSourceHandle, Self::Error>;

    /// Expands a handle only after current binding/grant/owner/view/residency/purge reauthorization.
    fn expand(
        &self,
        handle: &SearchSourceHandle,
        request: &Self::ExpansionRequest,
        live_state: &Self::LiveState,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ExpansionResult, Self::Error>;

    /// Invalidates all handles in one exact lifecycle/security scope.
    fn invalidate(
        &mut self,
        scope: &Self::InvalidationScope,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::InvalidationReceipt, Self::Error>;

    /// Expires finite handle state at one trusted clock instant.
    fn expire(
        &mut self,
        now: &Self::ExpiryInstant,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::ExpiryReceipt, Self::Error>;
}
