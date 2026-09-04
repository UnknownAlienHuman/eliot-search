//! Bounded current-workspace overlay with memory-only unsaved bytes.
//!
//! Precedence is fixed:
//!
//! `published base < saved revision awaiting publication < authenticated unsaved snapshot`.
//!
//! Saved overlays retain immutable revision references only. Unsaved bytes live
//! exclusively in process-owned memory, are not cloneable or serializable, are
//! redacted from formatting, and are overwritten before release. Query gaps
//! never unshadow an older base revision.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    AccessPolicyRevision, BindingId, Blake3Digest32, BoundedList, BufferSnapshotId,
    MAX_LIST_ITEMS, NonZeroRevision, OpaqueId, OpaqueRef, OverlayRevision,
    PositionEncoding, ProfileId, PurgeFenceRevision, ReceiptRef, SourceId,
    SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
    SourceRevisionRef, UtcTimestamp, WorkspaceViewRevisionId,
};
use search_ports::MutationIdentity;

/// Default maximum lifetime of one unsaved snapshot: fifteen minutes.
pub const DEFAULT_UNSAVED_TTL_MILLIS: u64 = 15 * 60 * 1_000;

/// Closed overlay failure surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OverlayError {
    /// Editor buffer was not represented by an exact authenticated observation.
    UnsavedBufferUnobserved,
    /// Editor session or client binding was not authenticated.
    UnsavedBufferUnauthenticated,
    /// Snapshot failed admission or current authorization.
    UnsavedSnapshotNotAdmitted,
    /// Buffer identity changed or version failed to advance.
    UnsavedVersionConflict,
    /// A finite count, byte, TTL, search, or lifecycle quota was exceeded.
    OverlayQuotaExceeded,
    /// Snapshot lifetime ended.
    OverlayExpired,
    /// Grant, binding, or access-policy authorization changed.
    OverlayAuthorizationLost,
    /// Source-owner generation changed.
    OverlayOwnerGenerationChanged,
    /// Purge fence invalidated the overlay.
    OverlayPurged,
    /// Query byte, step, result, or item budget was exhausted.
    OverlayBudgetExhausted,
    /// Query cancellation was observed.
    OverlayRetrievalCancelled,
    /// Exact source identity or precedence could not be established.
    OverlayPrecedenceUnknown,
    /// Observed saved revision does not correspond to the intended buffer.
    OverlaySaveConflict,
    /// A caller attempted to place unsaved bytes in durable state.
    DurableUnsavedForbidden,
    /// Operation identity was reused with another request digest.
    OverlayOperationConflict,
    /// Guard identity is stale or belongs to another binding/snapshot.
    OverlayGuardMismatch,
    /// A lifecycle transition is invalid from the current state.
    OverlayInvalidTransition,
    /// Shared revision or bounded contract construction failed.
    ContractViolation,
}

impl OverlayError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsavedBufferUnobserved => "UNSAVED_BUFFER_UNOBSERVED",
            Self::UnsavedBufferUnauthenticated => "UNSAVED_BUFFER_UNAUTHENTICATED",
            Self::UnsavedSnapshotNotAdmitted => "UNSAVED_SNAPSHOT_NOT_ADMITTED",
            Self::UnsavedVersionConflict => "UNSAVED_VERSION_CONFLICT",
            Self::OverlayQuotaExceeded => "OVERLAY_QUOTA_EXCEEDED",
            Self::OverlayExpired => "OVERLAY_EXPIRED",
            Self::OverlayAuthorizationLost => "OVERLAY_AUTHORIZATION_LOST",
            Self::OverlayOwnerGenerationChanged => "OVERLAY_OWNER_GENERATION_CHANGED",
            Self::OverlayPurged => "OVERLAY_PURGED",
            Self::OverlayBudgetExhausted => "OVERLAY_BUDGET_EXHAUSTED",
            Self::OverlayRetrievalCancelled => "OVERLAY_RETRIEVAL_CANCELLED",
            Self::OverlayPrecedenceUnknown => "OVERLAY_PRECEDENCE_UNKNOWN",
            Self::OverlaySaveConflict => "OVERLAY_SAVE_CONFLICT",
            Self::DurableUnsavedForbidden => "DURABLE_UNSAVED_FORBIDDEN",
            Self::OverlayOperationConflict => "OVERLAY_OPERATION_CONFLICT",
            Self::OverlayGuardMismatch => "OVERLAY_GUARD_MISMATCH",
            Self::OverlayInvalidTransition => "OVERLAY_INVALID_TRANSITION",
            Self::ContractViolation => "OVERLAY_CONTRACT_VIOLATION",
        }
    }
}

impl fmt::Display for OverlayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for OverlayError {}

/// Finite overlay resource policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayLimits {
    /// Maximum saved entries.
    pub max_saved_entries: usize,
    /// Maximum unsaved entries.
    pub max_unsaved_entries: usize,
    /// Maximum unsaved entries for one binding.
    pub max_unsaved_per_binding: usize,
    /// Maximum unsaved entries for one source membership.
    pub max_unsaved_per_membership: usize,
    /// Maximum bytes in one unsaved snapshot.
    pub max_unsaved_bytes_per_snapshot: usize,
    /// Maximum unsaved bytes across the store.
    pub max_unsaved_bytes_total: usize,
    /// Maximum unsaved snapshot TTL.
    pub max_unsaved_ttl_millis: u64,
    /// Maximum entries in one immutable overlay snapshot.
    pub max_snapshot_entries: usize,
    /// Maximum candidates returned by one overlay query.
    pub max_query_candidates: usize,
    /// Maximum records changed in one invalidation/config operation.
    pub max_lifecycle_batch: usize,
}

impl OverlayLimits {
    /// Conservative baseline limits.
    pub const BASELINE: Self = Self {
        max_saved_entries: 1_024,
        max_unsaved_entries: 256,
        max_unsaved_per_binding: 64,
        max_unsaved_per_membership: 1,
        max_unsaved_bytes_per_snapshot: 8 * 1024 * 1024,
        max_unsaved_bytes_total: 64 * 1024 * 1024,
        max_unsaved_ttl_millis: DEFAULT_UNSAVED_TTL_MILLIS,
        max_snapshot_entries: 1_280,
        max_query_candidates: 512,
        max_lifecycle_batch: 256,
    };

    /// Validates all finite dimensions.
    pub fn validate(self) -> Result<Self, OverlayError> {
        let valid = self.max_saved_entries > 0
            && self.max_saved_entries <= MAX_LIST_ITEMS
            && self.max_unsaved_entries > 0
            && self.max_unsaved_entries <= MAX_LIST_ITEMS
            && self.max_unsaved_per_binding > 0
            && self.max_unsaved_per_binding <= self.max_unsaved_entries
            && self.max_unsaved_per_membership == 1
            && self.max_unsaved_bytes_per_snapshot > 0
            && self.max_unsaved_bytes_total >= self.max_unsaved_bytes_per_snapshot
            && self.max_unsaved_ttl_millis > 0
            && self.max_snapshot_entries > 0
            && self.max_snapshot_entries <= MAX_LIST_ITEMS
            && self.max_query_candidates > 0
            && self.max_query_candidates <= MAX_LIST_ITEMS
            && self.max_lifecycle_batch > 0
            && self.max_lifecycle_batch <= MAX_LIST_ITEMS;
        if valid {
            Ok(self)
        } else {
            Err(OverlayError::OverlayQuotaExceeded)
        }
    }
}

/// Exact binding shared by saved and unsaved overlay entries.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OverlayBinding {
    /// Current authenticated client binding.
    pub binding_id: BindingId,
    /// Exact source namespace.
    pub source_namespace_id: SourceNamespaceId,
    /// Stable source identity.
    pub source_id: SourceId,
    /// Exact corpus membership whose base points are shadowed.
    pub source_membership_id: SourceMembershipId,
    /// Current source-owner generation.
    pub source_owner_generation: SourceOwnerGeneration,
    /// Current access policy revision.
    pub access_policy_revision: AccessPolicyRevision,
    /// Current purge fence revision.
    pub purge_fence_revision: PurgeFenceRevision,
    /// Current workspace view revision.
    pub workspace_view_revision_id: WorkspaceViewRevisionId,
}

/// Current live authorization/security observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayLiveState {
    /// Current authenticated binding.
    pub binding_id: BindingId,
    /// Current source-owner generation.
    pub source_owner_generation: SourceOwnerGeneration,
    /// Current access policy revision.
    pub access_policy_revision: AccessPolicyRevision,
    /// Current purge fence revision.
    pub purge_fence_revision: PurgeFenceRevision,
    /// Current workspace view revision.
    pub workspace_view_revision_id: WorkspaceViewRevisionId,
    /// Session is authenticated.
    pub session_authenticated: bool,
    /// Grant permits source and overlay reads.
    pub overlay_permitted: bool,
    /// No purge barrier covers the source.
    pub purge_clear: bool,
}

impl OverlayLiveState {
    fn validate_binding(self, binding: OverlayBinding) -> Result<(), OverlayError> {
        if !self.session_authenticated {
            return Err(OverlayError::UnsavedBufferUnauthenticated);
        }
        if !self.overlay_permitted || self.binding_id != binding.binding_id {
            return Err(OverlayError::OverlayAuthorizationLost);
        }
        if self.source_owner_generation != binding.source_owner_generation {
            return Err(OverlayError::OverlayOwnerGenerationChanged);
        }
        if !self.purge_clear || self.purge_fence_revision != binding.purge_fence_revision {
            return Err(OverlayError::OverlayPurged);
        }
        if self.access_policy_revision != binding.access_policy_revision {
            return Err(OverlayError::OverlayAuthorizationLost);
        }
        if self.workspace_view_revision_id != binding.workspace_view_revision_id {
            return Err(OverlayError::UnsavedVersionConflict);
        }
        Ok(())
    }
}

/// Durable lifecycle of a saved-overlay reference.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SavedOverlayStatus {
    /// Immutable revision is eligible and awaits publication.
    Active,
    /// Publication made the overlay redundant.
    Published,
    /// Access, purge, ownership, or residency invalidated the entry.
    Invalidated,
}

/// Reference-only saved overlay. It owns no source bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedOverlayEntry {
    /// Exact source/membership/security binding.
    pub binding: OverlayBinding,
    /// Admitted immutable saved revision.
    pub revision: SourceRevisionRef,
    /// Preparation profile.
    pub preparation_profile_id: ProfileId,
    /// Digest of accepted preparation/profile state.
    pub preparation_digest: Blake3Digest32,
    /// Admission/residency receipt.
    pub revision_receipt_ref: ReceiptRef,
    /// Overlay revision assigned by this store.
    pub overlay_revision: OverlayRevision,
    /// Current lifecycle.
    pub status: SavedOverlayStatus,
    /// Immutable admission operation.
    pub operation: MutationIdentity,
    /// Digest of exact canonical admission request.
    pub operation_request_digest: Blake3Digest32,
}

/// Exact unsaved editor snapshot descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsavedSnapshot {
    /// Exact source/membership/security binding.
    pub binding: OverlayBinding,
    /// Opaque editor buffer identity.
    pub buffer_id: OpaqueId,
    /// Random snapshot identity.
    pub buffer_snapshot_id: BufferSnapshotId,
    /// Strictly monotone editor buffer version.
    pub buffer_version: u64,
    /// Coordinate encoding used by editor anchors.
    pub position_encoding: PositionEncoding,
    /// Digest of exact unsaved bytes.
    pub content_digest: Blake3Digest32,
    /// Exact byte length.
    pub byte_length: u64,
    /// Admission time.
    pub created_at: UtcTimestamp,
    /// Hard expiration time.
    pub expires_at: UtcTimestamp,
    /// Non-reconstructive authenticated editor-session reference.
    pub session_ref: OpaqueRef,
}

/// Memory-only unsaved bytes.
///
/// This type is intentionally non-`Clone`, has redacted formatting, exposes no
/// serialization interface, and overwrites its allocation before release.
pub struct MemoryOnlyBytes(Vec<u8>);

impl MemoryOnlyBytes {
    fn new(bytes: Vec<u8>, maximum: usize) -> Result<Self, OverlayError> {
        if bytes.is_empty() || bytes.len() > maximum {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        Ok(Self(bytes))
    }

    fn as_slice(&self) -> &[u8] {
        &self.0
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for MemoryOnlyBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MemoryOnlyBytes")
            .field(&format_args!("<redacted:{} bytes>", self.0.len()))
            .finish()
    }
}

impl Drop for MemoryOnlyBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

/// Process-local authority for replacing or closing one unsaved snapshot.
///
/// The guard is not cloneable or serializable. Possession never replaces a
/// current live authorization check.
pub struct UnsavedBufferGuard {
    binding: OverlayBinding,
    buffer_id: OpaqueId,
    buffer_snapshot_id: BufferSnapshotId,
    buffer_version: u64,
    guard_digest: Blake3Digest32,
    overlay_revision: OverlayRevision,
}

impl UnsavedBufferGuard {
    /// Bound source membership.
    #[must_use]
    pub const fn source_membership_id(&self) -> SourceMembershipId {
        self.binding.source_membership_id
    }

    /// Bound snapshot identity.
    #[must_use]
    pub const fn buffer_snapshot_id(&self) -> BufferSnapshotId {
        self.buffer_snapshot_id
    }

    /// Bound editor version.
    #[must_use]
    pub const fn buffer_version(&self) -> u64 {
        self.buffer_version
    }

    /// Overlay revision observed when the guard was issued.
    #[must_use]
    pub const fn overlay_revision(&self) -> OverlayRevision {
        self.overlay_revision
    }
}

impl fmt::Debug for UnsavedBufferGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnsavedBufferGuard")
            .field("source_membership_id", &self.binding.source_membership_id)
            .field("buffer_snapshot_id", &self.buffer_snapshot_id)
            .field("buffer_version", &self.buffer_version)
            .field("overlay_revision", &self.overlay_revision)
            .field("guard_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Debug)]
struct UnsavedEntry {
    snapshot: UnsavedSnapshot,
    bytes: MemoryOnlyBytes,
    guard_digest: Blake3Digest32,
    overlay_revision: OverlayRevision,
}

/// Successful saved admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedAdmissionReceipt {
    /// Admitted saved entry.
    pub entry: SavedOverlayEntry,
    /// Whether the same operation/request was replayed.
    pub replayed: bool,
}

/// Successful unsaved replacement.
pub struct OverlayReplacementReceipt {
    /// Guard for the newly installed snapshot.
    pub guard: UnsavedBufferGuard,
    /// Retired snapshot identity.
    pub replaced_snapshot_id: BufferSnapshotId,
    /// New overlay revision.
    pub overlay_revision: OverlayRevision,
}

/// Invalidation cause.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OverlayInvalidationCause {
    /// Editor closed the buffer.
    EditorClose,
    /// Editor/client disconnected.
    Disconnect,
    /// Snapshot TTL elapsed.
    TtlExpired,
    /// Binding or grant was revoked.
    BindingRevoked,
    /// Source-owner generation changed.
    OwnerGenerationChanged,
    /// Purge fence invalidated the source.
    Purged,
    /// Live quota reduction expired excess state.
    QuotaReduction,
    /// Daemon is shutting down.
    DaemonShutdown,
    /// Saved revision was admitted and accepted as the replacement.
    SavedTransition,
}

/// Bounded invalidation selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverlayInvalidationScope {
    /// Every unsaved entry.
    All,
    /// Every entry for one binding.
    Binding(BindingId),
    /// One source membership.
    Membership(SourceMembershipId),
    /// One exact snapshot.
    Snapshot(BufferSnapshotId),
}

/// Content-free invalidation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlayInvalidationReceipt {
    /// Cause applied.
    pub cause: OverlayInvalidationCause,
    /// New overlay revision.
    pub overlay_revision: OverlayRevision,
    /// Invalidated snapshots in deterministic order.
    pub invalidated_snapshot_ids: BoundedList<BufferSnapshotId, MAX_LIST_ITEMS>,
    /// Whether another bounded invalidation pass is required.
    pub more_remaining: bool,
}

/// Precedence class in an immutable overlay view.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OverlayPrecedence {
    /// Published index/base source.
    Published = 0,
    /// Saved immutable revision awaiting publication.
    Saved = 1,
    /// Authenticated unsaved snapshot.
    Unsaved = 2,
}

/// Content-free immutable overlay entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverlayViewEntry {
    /// Saved immutable revision reference.
    Saved {
        /// Exact binding.
        binding: OverlayBinding,
        /// Exact saved revision.
        revision: SourceRevisionRef,
        /// Preparation profile.
        preparation_profile_id: ProfileId,
        /// Preparation digest.
        preparation_digest: Blake3Digest32,
        /// Overlay revision.
        overlay_revision: OverlayRevision,
    },
    /// Unsaved process-memory snapshot metadata; no bytes are exposed.
    Unsaved {
        /// Exact binding.
        binding: OverlayBinding,
        /// Snapshot identity.
        buffer_snapshot_id: BufferSnapshotId,
        /// Editor version.
        buffer_version: u64,
        /// Position encoding.
        position_encoding: PositionEncoding,
        /// Content digest.
        content_digest: Blake3Digest32,
        /// Exact byte length.
        byte_length: u64,
        /// Expiration.
        expires_at: UtcTimestamp,
        /// Overlay revision.
        overlay_revision: OverlayRevision,
    },
}

impl OverlayViewEntry {
    fn binding(&self) -> OverlayBinding {
        match self {
            Self::Saved { binding, .. } | Self::Unsaved { binding, .. } => *binding,
        }
    }

    fn precedence(&self) -> OverlayPrecedence {
        match self {
            Self::Saved { .. } => OverlayPrecedence::Saved,
            Self::Unsaved { .. } => OverlayPrecedence::Unsaved,
        }
    }

    fn overlay_revision(&self) -> OverlayRevision {
        match self {
            Self::Saved {
                overlay_revision, ..
            }
            | Self::Unsaved {
                overlay_revision, ..
            } => *overlay_revision,
        }
    }
}

/// Immutable bounded overlay snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlaySnapshot {
    /// Snapshot revision.
    pub overlay_revision: OverlayRevision,
    /// Exact current access policy revision.
    pub access_policy_revision: AccessPolicyRevision,
    /// Exact current purge fence revision.
    pub purge_fence_revision: PurgeFenceRevision,
    /// Exact current workspace view revision.
    pub workspace_view_revision_id: WorkspaceViewRevisionId,
    /// Current entries in deterministic order.
    pub entries: BoundedList<OverlayViewEntry, MAX_LIST_ITEMS>,
    /// Digest covering metadata and shadow-relevant identities.
    pub snapshot_digest: Blake3Digest32,
}

/// Exact published base identity eligible for shadow calculation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PublishedBase {
    /// Source membership.
    pub source_membership_id: SourceMembershipId,
    /// Published immutable revision.
    pub source_revision_ref: SourceRevisionRef,
}

/// Exact object hidden by a higher-precedence overlay.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ShadowedTarget {
    /// Published base revision.
    Published(PublishedBase),
    /// Saved overlay revision hidden by an unsaved snapshot.
    Saved {
        /// Source membership.
        source_membership_id: SourceMembershipId,
        /// Saved source revision.
        source_revision_ref: SourceRevisionRef,
        /// Saved overlay revision.
        overlay_revision: OverlayRevision,
    },
}

/// Deterministic shadow set. It is applied even when overlay retrieval is partial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlayShadowSet {
    /// Exact targets suppressed before fusion.
    pub targets: BoundedList<ShadowedTarget, MAX_LIST_ITEMS>,
    /// Memberships whose precedence could not be proven; every known base for
    /// these memberships must remain suppressed.
    pub fail_closed_memberships: BoundedList<SourceMembershipId, MAX_LIST_ITEMS>,
}

/// Query kind supported without a durable secondary index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlaySearchKind {
    /// Exact literal byte search.
    ExactLiteral,
    /// ASCII-delimited token equality.
    Token,
}

/// Bounded overlay query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlaySearchRequest {
    /// Search operation.
    pub kind: OverlaySearchKind,
    /// Non-empty exact pattern.
    pub pattern: Vec<u8>,
    /// Maximum bytes examined.
    pub max_bytes: u64,
    /// Maximum comparison steps.
    pub max_steps: u64,
    /// Maximum candidates returned.
    pub max_candidates: usize,
}

/// Byte range inside one exact unsaved snapshot.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OverlayMatchRange {
    /// Inclusive byte start.
    pub byte_start: u64,
    /// Exclusive byte end.
    pub byte_end: u64,
}

/// Candidate produced by the process-memory/direct overlay plane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverlayCandidate {
    /// Exact unsaved byte match.
    UnsavedMatch {
        /// Exact binding.
        binding: OverlayBinding,
        /// Snapshot identity.
        buffer_snapshot_id: BufferSnapshotId,
        /// Buffer version.
        buffer_version: u64,
        /// Exact match range.
        range: OverlayMatchRange,
        /// Unsaved content digest.
        content_digest: Blake3Digest32,
        /// Highest precedence.
        precedence: OverlayPrecedence,
    },
    /// Saved revision nomination requiring direct immutable readback.
    SavedReference {
        /// Exact binding.
        binding: OverlayBinding,
        /// Exact saved revision.
        source_revision_ref: SourceRevisionRef,
        /// Overlay revision.
        overlay_revision: OverlayRevision,
        /// Saved precedence.
        precedence: OverlayPrecedence,
    },
}

impl OverlayCandidate {
    fn binding(&self) -> OverlayBinding {
        match self {
            Self::UnsavedMatch { binding, .. } | Self::SavedReference { binding, .. } => *binding,
        }
    }

    fn precedence(&self) -> OverlayPrecedence {
        match self {
            Self::UnsavedMatch { precedence, .. }
            | Self::SavedReference { precedence, .. } => *precedence,
        }
    }
}

/// Explicit overlay retrieval gap.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OverlayGap {
    /// Saved bytes were not supplied by the immutable revision-read port.
    SavedReadRequired,
    /// Byte or step budget prevented complete in-memory retrieval.
    BudgetExhausted,
    /// Query was cancelled.
    Cancelled,
    /// Entry became unauthorized or stale.
    AuthorizationChanged,
}

/// Bounded overlay candidate result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlayCandidateSet {
    /// Candidates in stable precedence/source/range order.
    pub candidates: BoundedList<OverlayCandidate, MAX_LIST_ITEMS>,
    /// Explicit gaps.
    pub gaps: BoundedList<OverlayGap, MAX_LIST_ITEMS>,
    /// Bytes examined.
    pub scanned_bytes: u64,
    /// Whether every eligible in-memory entry completed.
    pub complete: bool,
}

/// Explicit cancellation observation.
pub trait OverlayCancellation {
    /// Whether cancellation is currently requested.
    fn is_cancelled(&self) -> bool;
}

/// Cancellation implementation that never cancels.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancelled;

impl OverlayCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Published/base nomination presented for overlay precedence merge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseNomination {
    /// Exact published identity.
    pub published: PublishedBase,
    /// Stable source-independent rank key from the validated base plane.
    pub rank_key: Blake3Digest32,
}

/// Input candidate after shadowing and precedence merge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidateInput {
    /// Unshadowed published nomination.
    Published(BaseNomination),
    /// Overlay candidate.
    Overlay(OverlayCandidate),
}

/// Proof that a saved revision corresponds to one unsaved version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlaySaveTransition {
    /// Unsaved snapshot being retired.
    pub buffer_snapshot_id: BufferSnapshotId,
    /// Unsaved editor version.
    pub buffer_version: u64,
    /// Matching admitted saved revision.
    pub saved_revision: SourceRevisionRef,
    /// Content-free admission receipt.
    pub saved_revision_receipt_ref: ReceiptRef,
}

/// Finite overlay store.
#[derive(Debug)]
pub struct OverlayStore {
    limits: OverlayLimits,
    revision: OverlayRevision,
    saved: BTreeMap<SourceMembershipId, SavedOverlayEntry>,
    unsaved: BTreeMap<SourceMembershipId, UnsavedEntry>,
    operations: BTreeMap<OpaqueId, Blake3Digest32>,
}

impl OverlayStore {
    /// Creates an empty bounded store.
    pub fn new(limits: OverlayLimits) -> Result<Self, OverlayError> {
        Ok(Self {
            limits: limits.validate()?,
            revision: OverlayRevision::new(0),
            saved: BTreeMap::new(),
            unsaved: BTreeMap::new(),
            operations: BTreeMap::new(),
        })
    }

    /// Current overlay revision.
    #[must_use]
    pub const fn revision(&self) -> OverlayRevision {
        self.revision
    }

    /// Current limits.
    #[must_use]
    pub const fn limits(&self) -> OverlayLimits {
        self.limits
    }

    /// Total process-memory unsaved bytes.
    #[must_use]
    pub fn unsaved_bytes(&self) -> usize {
        self.unsaved.values().map(|entry| entry.bytes.len()).sum()
    }

    /// Admits one immutable saved revision reference.
    pub fn admit_saved_overlay(
        &mut self,
        binding: OverlayBinding,
        revision: SourceRevisionRef,
        preparation_profile_id: ProfileId,
        preparation_digest: Blake3Digest32,
        revision_receipt_ref: ReceiptRef,
        operation: MutationIdentity,
        operation_request_digest: Blake3Digest32,
        live: OverlayLiveState,
    ) -> Result<SavedAdmissionReceipt, OverlayError> {
        live.validate_binding(binding)?;
        if revision.source_namespace_id != binding.source_namespace_id
            || revision.source_id != binding.source_id
        {
            return Err(OverlayError::OverlayPrecedenceUnknown);
        }
        if let Some(existing_digest) = self.operations.get(&operation.operation_id) {
            if *existing_digest != operation_request_digest {
                return Err(OverlayError::OverlayOperationConflict);
            }
            let existing = self
                .saved
                .get(&binding.source_membership_id)
                .filter(|entry| {
                    entry.operation.operation_id == operation.operation_id
                        && entry.operation_request_digest == operation_request_digest
                })
                .cloned()
                .ok_or(OverlayError::OverlayOperationConflict)?;
            return Ok(SavedAdmissionReceipt {
                entry: existing,
                replayed: true,
            });
        }
        if self.saved.len() >= self.limits.max_saved_entries
            && !self.saved.contains_key(&binding.source_membership_id)
        {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        let next_revision = self.next_revision()?;
        let entry = SavedOverlayEntry {
            binding,
            revision,
            preparation_profile_id,
            preparation_digest,
            revision_receipt_ref,
            overlay_revision: next_revision,
            status: SavedOverlayStatus::Active,
            operation: operation.clone(),
            operation_request_digest,
        };
        self.operations
            .insert(operation.operation_id, operation_request_digest);
        self.saved
            .insert(binding.source_membership_id, entry.clone());
        Ok(SavedAdmissionReceipt {
            entry,
            replayed: false,
        })
    }

    /// Attaches one authenticated memory-only unsaved snapshot.
    pub fn attach_unsaved_snapshot(
        &mut self,
        snapshot: UnsavedSnapshot,
        bytes: Vec<u8>,
        guard_digest: Blake3Digest32,
        declared_ttl_millis: u64,
        live: OverlayLiveState,
    ) -> Result<UnsavedBufferGuard, OverlayError> {
        live.validate_binding(snapshot.binding)?;
        self.validate_snapshot(&snapshot, &bytes, declared_ttl_millis)?;
        if self.unsaved.contains_key(&snapshot.binding.source_membership_id) {
            return Err(OverlayError::UnsavedVersionConflict);
        }
        self.validate_unsaved_capacity(snapshot.binding, bytes.len(), 0)?;
        let overlay_revision = self.next_revision()?;
        let entry = UnsavedEntry {
            snapshot: snapshot.clone(),
            bytes: MemoryOnlyBytes::new(bytes, self.limits.max_unsaved_bytes_per_snapshot)?,
            guard_digest,
            overlay_revision,
        };
        let guard = make_guard(&entry);
        self.unsaved
            .insert(snapshot.binding.source_membership_id, entry);
        Ok(guard)
    }

    /// Atomically replaces one unsaved snapshot with a strictly newer version.
    pub fn replace_unsaved_snapshot(
        &mut self,
        guard: &UnsavedBufferGuard,
        next_snapshot: UnsavedSnapshot,
        next_bytes: Vec<u8>,
        next_guard_digest: Blake3Digest32,
        declared_ttl_millis: u64,
        live: OverlayLiveState,
    ) -> Result<OverlayReplacementReceipt, OverlayError> {
        live.validate_binding(next_snapshot.binding)?;
        self.validate_snapshot(&next_snapshot, &next_bytes, declared_ttl_millis)?;
        let current = self
            .unsaved
            .get(&guard.binding.source_membership_id)
            .ok_or(OverlayError::OverlayGuardMismatch)?;
        verify_guard(guard, current)?;
        if next_snapshot.binding != current.snapshot.binding
            || next_snapshot.buffer_id != current.snapshot.buffer_id
            || next_snapshot.buffer_version <= current.snapshot.buffer_version
            || next_snapshot.buffer_snapshot_id == current.snapshot.buffer_snapshot_id
        {
            return Err(OverlayError::UnsavedVersionConflict);
        }
        self.validate_unsaved_capacity(
            next_snapshot.binding,
            next_bytes.len(),
            current.bytes.len(),
        )?;
        let replaced_snapshot_id = current.snapshot.buffer_snapshot_id;
        let overlay_revision = self.next_revision()?;
        let replacement = UnsavedEntry {
            snapshot: next_snapshot.clone(),
            bytes: MemoryOnlyBytes::new(
                next_bytes,
                self.limits.max_unsaved_bytes_per_snapshot,
            )?,
            guard_digest: next_guard_digest,
            overlay_revision,
        };
        let next_guard = make_guard(&replacement);
        let old = self
            .unsaved
            .insert(next_snapshot.binding.source_membership_id, replacement)
            .expect("verified current entry");
        drop(old);
        Ok(OverlayReplacementReceipt {
            guard: next_guard,
            replaced_snapshot_id,
            overlay_revision,
        })
    }

    /// Invalidates one deterministic bounded set of unsaved snapshots.
    pub fn close_or_invalidate_unsaved(
        &mut self,
        scope: &OverlayInvalidationScope,
        cause: OverlayInvalidationCause,
    ) -> Result<OverlayInvalidationReceipt, OverlayError> {
        let mut ids = self
            .unsaved
            .iter()
            .filter(|(_, entry)| scope_matches(scope, entry))
            .map(|(membership, entry)| (*membership, entry.snapshot.buffer_snapshot_id))
            .collect::<Vec<_>>();
        ids.sort_by_key(|(membership, snapshot)| (*membership, *snapshot));
        let more_remaining = ids.len() > self.limits.max_lifecycle_batch;
        ids.truncate(self.limits.max_lifecycle_batch);
        let overlay_revision = if ids.is_empty() {
            self.revision
        } else {
            self.next_revision()?
        };
        let mut removed = Vec::new();
        for (membership, snapshot) in ids {
            if self.unsaved.remove(&membership).is_some() {
                removed.push(snapshot);
            }
        }
        Ok(OverlayInvalidationReceipt {
            cause,
            overlay_revision,
            invalidated_snapshot_ids: bounded(removed)?,
            more_remaining,
        })
    }

    /// Creates an immutable metadata-only overlay view after bounded expiry and
    /// current authorization checks.
    pub fn snapshot_overlay_view(
        &mut self,
        live_by_membership: &BTreeMap<SourceMembershipId, OverlayLiveState>,
        now: &UtcTimestamp,
        blake3_256: impl Fn(&[u8]) -> [u8; 32],
    ) -> Result<OverlaySnapshot, OverlayError> {
        let expired = self
            .unsaved
            .iter()
            .filter(|(_, entry)| now >= &entry.snapshot.expires_at)
            .map(|(membership, _)| *membership)
            .collect::<Vec<_>>();
        if expired.len() > self.limits.max_lifecycle_batch {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        if !expired.is_empty() {
            self.next_revision()?;
            for membership in expired {
                self.unsaved.remove(&membership);
            }
        }

        let mut entries = Vec::new();
        for entry in self.saved.values() {
            if entry.status != SavedOverlayStatus::Active {
                continue;
            }
            let live = live_by_membership
                .get(&entry.binding.source_membership_id)
                .ok_or(OverlayError::OverlayAuthorizationLost)?;
            live.validate_binding(entry.binding)?;
            entries.push(OverlayViewEntry::Saved {
                binding: entry.binding,
                revision: entry.revision,
                preparation_profile_id: entry.preparation_profile_id.clone(),
                preparation_digest: entry.preparation_digest,
                overlay_revision: entry.overlay_revision,
            });
        }
        for entry in self.unsaved.values() {
            let live = live_by_membership
                .get(&entry.snapshot.binding.source_membership_id)
                .ok_or(OverlayError::OverlayAuthorizationLost)?;
            live.validate_binding(entry.snapshot.binding)?;
            entries.push(OverlayViewEntry::Unsaved {
                binding: entry.snapshot.binding,
                buffer_snapshot_id: entry.snapshot.buffer_snapshot_id,
                buffer_version: entry.snapshot.buffer_version,
                position_encoding: entry.snapshot.position_encoding,
                content_digest: entry.snapshot.content_digest,
                byte_length: entry.snapshot.byte_length,
                expires_at: entry.snapshot.expires_at.clone(),
                overlay_revision: entry.overlay_revision,
            });
        }
        if entries.len() > self.limits.max_snapshot_entries {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        entries.sort_by(|left, right| {
            left.binding()
                .source_membership_id
                .cmp(&right.binding().source_membership_id)
                .then_with(|| right.precedence().cmp(&left.precedence()))
                .then_with(|| left.overlay_revision().cmp(&right.overlay_revision()))
        });
        let first = entries.first().map(OverlayViewEntry::binding);
        let (access_policy_revision, purge_fence_revision, workspace_view_revision_id) = first
            .map(|binding| {
                (
                    binding.access_policy_revision,
                    binding.purge_fence_revision,
                    binding.workspace_view_revision_id,
                )
            })
            .unwrap_or((
                AccessPolicyRevision::new(0),
                PurgeFenceRevision::new(0),
                WorkspaceViewRevisionId::from_bytes([0; 16]),
            ));
        let digest_input = snapshot_digest_input(
            self.revision,
            access_policy_revision,
            purge_fence_revision,
            workspace_view_revision_id,
            &entries,
        )?;
        Ok(OverlaySnapshot {
            overlay_revision: self.revision,
            access_policy_revision,
            purge_fence_revision,
            workspace_view_revision_id,
            entries: bounded(entries)?,
            snapshot_digest: Blake3Digest32::from_bytes(blake3_256(&digest_input)),
        })
    }

    /// Computes exact fail-closed base/saved shadows.
    pub fn compute_shadow_set(
        &self,
        base: &[PublishedBase],
        snapshot: &OverlaySnapshot,
    ) -> Result<OverlayShadowSet, OverlayError> {
        if base.len() > MAX_LIST_ITEMS {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        let mut highest = BTreeMap::new();
        let mut ambiguous = BTreeSet::new();
        for entry in &snapshot.entries {
            let membership = entry.binding().source_membership_id;
            match highest.get(&membership) {
                None => {
                    highest.insert(membership, entry);
                }
                Some(current) if entry.precedence() > current.precedence() => {
                    highest.insert(membership, entry);
                }
                Some(current) if entry.precedence() == current.precedence() => {
                    ambiguous.insert(membership);
                }
                Some(_) => {}
            }
        }

        let mut targets = BTreeSet::new();
        for published in base {
            if highest.contains_key(&published.source_membership_id)
                || ambiguous.contains(&published.source_membership_id)
            {
                targets.insert(ShadowedTarget::Published(*published));
            }
        }
        for entry in self.saved.values() {
            let membership = entry.binding.source_membership_id;
            let unsaved_wins = snapshot.entries.iter().any(|value| {
                value.binding().source_membership_id == membership
                    && value.precedence() == OverlayPrecedence::Unsaved
            });
            if unsaved_wins || ambiguous.contains(&membership) {
                targets.insert(ShadowedTarget::Saved {
                    source_membership_id: membership,
                    source_revision_ref: entry.revision,
                    overlay_revision: entry.overlay_revision,
                });
            }
        }
        Ok(OverlayShadowSet {
            targets: bounded(targets.into_iter().collect())?,
            fail_closed_memberships: bounded(ambiguous.into_iter().collect())?,
        })
    }

    /// Retrieves bounded direct candidates from one immutable overlay view.
    ///
    /// Unsaved matches are evaluated directly against process-memory bytes.
    /// Saved references are emitted only as direct-read requirements; this
    /// package never persists or mirrors their bytes.
    pub fn retrieve_overlay(
        &self,
        request: &OverlaySearchRequest,
        snapshot: &OverlaySnapshot,
        cancellation: &dyn OverlayCancellation,
    ) -> Result<OverlayCandidateSet, OverlayError> {
        if request.pattern.is_empty()
            || request.pattern.len() > self.limits.max_unsaved_bytes_per_snapshot
            || request.max_bytes == 0
            || request.max_steps == 0
            || request.max_candidates == 0
            || request.max_candidates > self.limits.max_query_candidates
        {
            return Err(OverlayError::OverlayBudgetExhausted);
        }
        if snapshot.overlay_revision != self.revision {
            return Err(OverlayError::OverlayPrecedenceUnknown);
        }

        let mut candidates = Vec::new();
        let mut gaps = BTreeSet::new();
        let mut scanned_bytes = 0_u64;
        let mut steps = 0_u64;
        let mut complete = true;
        for view in &snapshot.entries {
            if cancellation.is_cancelled() {
                gaps.insert(OverlayGap::Cancelled);
                complete = false;
                break;
            }
            match view {
                OverlayViewEntry::Saved {
                    binding,
                    revision,
                    overlay_revision,
                    ..
                } => {
                    candidates.push(OverlayCandidate::SavedReference {
                        binding: *binding,
                        source_revision_ref: *revision,
                        overlay_revision: *overlay_revision,
                        precedence: OverlayPrecedence::Saved,
                    });
                    gaps.insert(OverlayGap::SavedReadRequired);
                    complete = false;
                }
                OverlayViewEntry::Unsaved {
                    binding,
                    buffer_snapshot_id,
                    buffer_version,
                    content_digest,
                    byte_length,
                    ..
                } => {
                    let entry = self
                        .unsaved
                        .get(&binding.source_membership_id)
                        .ok_or(OverlayError::OverlayPrecedenceUnknown)?;
                    if entry.snapshot.buffer_snapshot_id != *buffer_snapshot_id
                        || entry.snapshot.buffer_version != *buffer_version
                        || entry.snapshot.content_digest != *content_digest
                        || entry.snapshot.byte_length != *byte_length
                    {
                        return Err(OverlayError::OverlayPrecedenceUnknown);
                    }
                    let next_bytes = scanned_bytes
                        .checked_add(*byte_length)
                        .ok_or(OverlayError::OverlayBudgetExhausted)?;
                    if next_bytes > request.max_bytes {
                        gaps.insert(OverlayGap::BudgetExhausted);
                        complete = false;
                        continue;
                    }
                    let ranges = find_matches(
                        entry.bytes.as_slice(),
                        &request.pattern,
                        request.kind,
                        request.max_steps.saturating_sub(steps),
                        request.max_candidates.saturating_sub(candidates.len()),
                    )?;
                    steps = steps
                        .checked_add(ranges.steps)
                        .ok_or(OverlayError::OverlayBudgetExhausted)?;
                    scanned_bytes = next_bytes;
                    for range in ranges.matches {
                        if candidates.len() >= request.max_candidates {
                            gaps.insert(OverlayGap::BudgetExhausted);
                            complete = false;
                            break;
                        }
                        candidates.push(OverlayCandidate::UnsavedMatch {
                            binding: *binding,
                            buffer_snapshot_id: *buffer_snapshot_id,
                            buffer_version: *buffer_version,
                            range,
                            content_digest: *content_digest,
                            precedence: OverlayPrecedence::Unsaved,
                        });
                    }
                }
            }
        }
        candidates.sort_by(|left, right| {
            right
                .precedence()
                .cmp(&left.precedence())
                .then_with(|| {
                    left.binding()
                        .source_membership_id
                        .cmp(&right.binding().source_membership_id)
                })
                .then_with(|| candidate_range(left).cmp(&candidate_range(right)))
        });
        Ok(OverlayCandidateSet {
            candidates: bounded(candidates)?,
            gaps: bounded(gaps.into_iter().collect())?,
            scanned_bytes,
            complete,
        })
    }

    /// Applies shadowing before deterministic base/overlay fusion.
    pub fn merge_overlay_and_base(
        &self,
        base: Vec<BaseNomination>,
        overlay: &OverlayCandidateSet,
        shadows: &OverlayShadowSet,
    ) -> Result<BoundedList<CandidateInput, MAX_LIST_ITEMS>, OverlayError> {
        let shadowed_base = shadows
            .targets
            .iter()
            .filter_map(|target| match target {
                ShadowedTarget::Published(value) => Some(*value),
                ShadowedTarget::Saved { .. } => None,
            })
            .collect::<BTreeSet<_>>();
        let fail_closed = shadows
            .fail_closed_memberships
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut merged = base
            .into_iter()
            .filter(|candidate| {
                !shadowed_base.contains(&candidate.published)
                    && !fail_closed.contains(&candidate.published.source_membership_id)
            })
            .map(CandidateInput::Published)
            .chain(
                overlay
                    .candidates
                    .iter()
                    .cloned()
                    .map(CandidateInput::Overlay),
            )
            .collect::<Vec<_>>();
        if merged.len() > MAX_LIST_ITEMS {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        merged.sort_by(|left, right| candidate_input_key(left).cmp(&candidate_input_key(right)));
        bounded(merged)
    }

    /// Proves an observed saved revision corresponds to the intended buffer.
    ///
    /// This function never persists unsaved bytes. The source/revision-store
    /// owners create the immutable revision and receipt first.
    pub fn prepare_save_admission(
        &self,
        guard: &UnsavedBufferGuard,
        observed_saved_revision: SourceRevisionRef,
        saved_revision_receipt_ref: ReceiptRef,
        live: OverlayLiveState,
    ) -> Result<OverlaySaveTransition, OverlayError> {
        let entry = self
            .unsaved
            .get(&guard.binding.source_membership_id)
            .ok_or(OverlayError::OverlayGuardMismatch)?;
        verify_guard(guard, entry)?;
        live.validate_binding(entry.snapshot.binding)?;
        if observed_saved_revision.source_namespace_id
                != entry.snapshot.binding.source_namespace_id
            || observed_saved_revision.source_id != entry.snapshot.binding.source_id
            || observed_saved_revision.content_digest != entry.snapshot.content_digest
            || observed_saved_revision.byte_length != entry.snapshot.byte_length
        {
            return Err(OverlayError::OverlaySaveConflict);
        }
        Ok(OverlaySaveTransition {
            buffer_snapshot_id: entry.snapshot.buffer_snapshot_id,
            buffer_version: entry.snapshot.buffer_version,
            saved_revision: observed_saved_revision,
            saved_revision_receipt_ref,
        })
    }

    /// Applies a completed save transition by removing the exact unsaved entry.
    pub fn commit_save_transition(
        &mut self,
        guard: &UnsavedBufferGuard,
        transition: &OverlaySaveTransition,
    ) -> Result<OverlayInvalidationReceipt, OverlayError> {
        let entry = self
            .unsaved
            .get(&guard.binding.source_membership_id)
            .ok_or(OverlayError::OverlayGuardMismatch)?;
        verify_guard(guard, entry)?;
        if transition.buffer_snapshot_id != entry.snapshot.buffer_snapshot_id
            || transition.buffer_version != entry.snapshot.buffer_version
            || transition.saved_revision.content_digest != entry.snapshot.content_digest
        {
            return Err(OverlayError::OverlaySaveConflict);
        }
        self.close_or_invalidate_unsaved(
            &OverlayInvalidationScope::Snapshot(transition.buffer_snapshot_id),
            OverlayInvalidationCause::SavedTransition,
        )
    }

    /// Reconstructs saved overlays from immutable admitted revision receipts.
    ///
    /// Unsaved bytes, guards, and snapshots are intentionally absent from this
    /// recovery surface.
    pub fn recover_saved_overlay(
        &mut self,
        entries: Vec<SavedOverlayEntry>,
        live_by_membership: &BTreeMap<SourceMembershipId, OverlayLiveState>,
    ) -> Result<BoundedList<SourceMembershipId, MAX_LIST_ITEMS>, OverlayError> {
        if entries.len() > self.limits.max_saved_entries {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        let mut recovered = Vec::new();
        for entry in entries {
            if entry.status != SavedOverlayStatus::Active {
                continue;
            }
            let live = live_by_membership
                .get(&entry.binding.source_membership_id)
                .ok_or(OverlayError::OverlayAuthorizationLost)?;
            live.validate_binding(entry.binding)?;
            if self
                .saved
                .insert(entry.binding.source_membership_id, entry.clone())
                .is_some()
            {
                return Err(OverlayError::OverlayPrecedenceUnknown);
            }
            self.revision = self.revision.max(entry.overlay_revision);
            recovered.push(entry.binding.source_membership_id);
        }
        bounded(recovered)
    }

    /// Applies live quota reductions after invalidating excess state first.
    pub fn apply_live_limits(
        &mut self,
        limits: OverlayLimits,
    ) -> Result<OverlayInvalidationReceipt, OverlayError> {
        let limits = limits.validate()?;
        let mut ordered = self
            .unsaved
            .iter()
            .map(|(membership, entry)| {
                (
                    entry.snapshot.created_at.clone(),
                    *membership,
                    entry.bytes.len(),
                )
            })
            .collect::<Vec<_>>();
        ordered.sort();
        let mut keep_count = ordered.len();
        let mut keep_bytes = self.unsaved_bytes();
        let mut remove = Vec::new();
        for (_, membership, bytes) in &ordered {
            let over_count = keep_count > limits.max_unsaved_entries;
            let over_bytes = keep_bytes > limits.max_unsaved_bytes_total;
            let over_snapshot = *bytes > limits.max_unsaved_bytes_per_snapshot;
            if over_count || over_bytes || over_snapshot {
                remove.push(*membership);
                keep_count = keep_count.saturating_sub(1);
                keep_bytes = keep_bytes.saturating_sub(*bytes);
            }
        }
        if remove.len() > self.limits.max_lifecycle_batch {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        let revision = if remove.is_empty() {
            self.revision
        } else {
            self.next_revision()?
        };
        let mut snapshots = Vec::new();
        for membership in remove {
            if let Some(entry) = self.unsaved.remove(&membership) {
                snapshots.push(entry.snapshot.buffer_snapshot_id);
            }
        }
        self.limits = limits;
        Ok(OverlayInvalidationReceipt {
            cause: OverlayInvalidationCause::QuotaReduction,
            overlay_revision: revision,
            invalidated_snapshot_ids: bounded(snapshots)?,
            more_remaining: false,
        })
    }

    fn validate_snapshot(
        &self,
        snapshot: &UnsavedSnapshot,
        bytes: &[u8],
        declared_ttl_millis: u64,
    ) -> Result<(), OverlayError> {
        if snapshot.buffer_version == 0
            || snapshot.created_at >= snapshot.expires_at
            || declared_ttl_millis == 0
            || declared_ttl_millis > self.limits.max_unsaved_ttl_millis
            || bytes.is_empty()
            || bytes.len() > self.limits.max_unsaved_bytes_per_snapshot
            || u64::try_from(bytes.len()).ok() != Some(snapshot.byte_length)
        {
            return Err(OverlayError::UnsavedSnapshotNotAdmitted);
        }
        Ok(())
    }

    fn validate_unsaved_capacity(
        &self,
        binding: OverlayBinding,
        incoming_bytes: usize,
        replacing_bytes: usize,
    ) -> Result<(), OverlayError> {
        if replacing_bytes == 0 && self.unsaved.len() >= self.limits.max_unsaved_entries {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        let binding_count = self
            .unsaved
            .values()
            .filter(|entry| entry.snapshot.binding.binding_id == binding.binding_id)
            .count();
        if replacing_bytes == 0 && binding_count >= self.limits.max_unsaved_per_binding {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        let next_total = self
            .unsaved_bytes()
            .saturating_sub(replacing_bytes)
            .checked_add(incoming_bytes)
            .ok_or(OverlayError::OverlayQuotaExceeded)?;
        if next_total > self.limits.max_unsaved_bytes_total {
            return Err(OverlayError::OverlayQuotaExceeded);
        }
        Ok(())
    }

    fn next_revision(&mut self) -> Result<OverlayRevision, OverlayError> {
        self.revision = self
            .revision
            .checked_next()
            .map_err(|_| OverlayError::ContractViolation)?;
        Ok(self.revision)
    }
}

fn verify_guard(guard: &UnsavedBufferGuard, entry: &UnsavedEntry) -> Result<(), OverlayError> {
    let snapshot = &entry.snapshot;
    if guard.binding != snapshot.binding
        || guard.buffer_id != snapshot.buffer_id
        || guard.buffer_snapshot_id != snapshot.buffer_snapshot_id
        || guard.buffer_version != snapshot.buffer_version
        || guard.guard_digest != entry.guard_digest
        || guard.overlay_revision != entry.overlay_revision
    {
        Err(OverlayError::OverlayGuardMismatch)
    } else {
        Ok(())
    }
}

fn make_guard(entry: &UnsavedEntry) -> UnsavedBufferGuard {
    UnsavedBufferGuard {
        binding: entry.snapshot.binding,
        buffer_id: entry.snapshot.buffer_id.clone(),
        buffer_snapshot_id: entry.snapshot.buffer_snapshot_id,
        buffer_version: entry.snapshot.buffer_version,
        guard_digest: entry.guard_digest,
        overlay_revision: entry.overlay_revision,
    }
}

fn scope_matches(scope: &OverlayInvalidationScope, entry: &UnsavedEntry) -> bool {
    match scope {
        OverlayInvalidationScope::All => true,
        OverlayInvalidationScope::Binding(binding) => {
            entry.snapshot.binding.binding_id == *binding
        }
        OverlayInvalidationScope::Membership(membership) => {
            entry.snapshot.binding.source_membership_id == *membership
        }
        OverlayInvalidationScope::Snapshot(snapshot) => {
            entry.snapshot.buffer_snapshot_id == *snapshot
        }
    }
}

struct MatchSearchResult {
    matches: Vec<OverlayMatchRange>,
    steps: u64,
}

fn find_matches(
    input: &[u8],
    pattern: &[u8],
    kind: OverlaySearchKind,
    max_steps: u64,
    max_matches: usize,
) -> Result<MatchSearchResult, OverlayError> {
    if max_steps == 0 || max_matches == 0 {
        return Err(OverlayError::OverlayBudgetExhausted);
    }
    if pattern.len() > input.len() {
        return Ok(MatchSearchResult {
            matches: Vec::new(),
            steps: 0,
        });
    }
    let mut matches = Vec::new();
    let mut steps = 0_u64;
    for start in 0..=input.len() - pattern.len() {
        steps = steps
            .checked_add(1)
            .ok_or(OverlayError::OverlayBudgetExhausted)?;
        if steps > max_steps {
            return Err(OverlayError::OverlayBudgetExhausted);
        }
        let end = start + pattern.len();
        if &input[start..end] != pattern {
            continue;
        }
        let accepted = match kind {
            OverlaySearchKind::ExactLiteral => true,
            OverlaySearchKind::Token => {
                token_boundary(input, start) && token_boundary(input, end)
            }
        };
        if accepted {
            if matches.len() >= max_matches {
                return Err(OverlayError::OverlayBudgetExhausted);
            }
            matches.push(OverlayMatchRange {
                byte_start: u64::try_from(start)
                    .map_err(|_| OverlayError::OverlayBudgetExhausted)?,
                byte_end: u64::try_from(end)
                    .map_err(|_| OverlayError::OverlayBudgetExhausted)?,
            });
        }
    }
    Ok(MatchSearchResult { matches, steps })
}

fn token_boundary(input: &[u8], position: usize) -> bool {
    position == 0
        || position == input.len()
        || !input[position.saturating_sub(1)].is_ascii_alphanumeric()
        || !input[position].is_ascii_alphanumeric()
}

fn candidate_range(candidate: &OverlayCandidate) -> Option<OverlayMatchRange> {
    match candidate {
        OverlayCandidate::UnsavedMatch { range, .. } => Some(*range),
        OverlayCandidate::SavedReference { .. } => None,
    }
}

fn candidate_input_key(
    candidate: &CandidateInput,
) -> (core::cmp::Reverse<OverlayPrecedence>, SourceMembershipId, Blake3Digest32) {
    match candidate {
        CandidateInput::Published(value) => (
            core::cmp::Reverse(OverlayPrecedence::Published),
            value.published.source_membership_id,
            value.rank_key,
        ),
        CandidateInput::Overlay(value) => {
            let digest = match value {
                OverlayCandidate::UnsavedMatch { content_digest, .. } => *content_digest,
                OverlayCandidate::SavedReference {
                    source_revision_ref,
                    ..
                } => source_revision_ref.content_digest,
            };
            (
                core::cmp::Reverse(value.precedence()),
                value.binding().source_membership_id,
                digest,
            )
        }
    }
}

fn snapshot_digest_input(
    revision: OverlayRevision,
    access_policy_revision: AccessPolicyRevision,
    purge_fence_revision: PurgeFenceRevision,
    workspace_view_revision_id: WorkspaceViewRevisionId,
    entries: &[OverlayViewEntry],
) -> Result<Vec<u8>, OverlayError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/overlay-snapshot/v1")?;
    bytes.extend_from_slice(&revision.get().to_be_bytes());
    bytes.extend_from_slice(&access_policy_revision.get().to_be_bytes());
    bytes.extend_from_slice(&purge_fence_revision.get().to_be_bytes());
    bytes.extend_from_slice(workspace_view_revision_id.as_bytes());
    for entry in entries {
        let binding = entry.binding();
        bytes.extend_from_slice(binding.binding_id.as_bytes());
        bytes.extend_from_slice(binding.source_namespace_id.as_bytes());
        bytes.extend_from_slice(binding.source_id.as_bytes());
        bytes.extend_from_slice(binding.source_membership_id.as_bytes());
        bytes.extend_from_slice(binding.source_owner_generation.as_bytes());
        bytes.push(entry.precedence() as u8);
        bytes.extend_from_slice(&entry.overlay_revision().get().to_be_bytes());
        match entry {
            OverlayViewEntry::Saved {
                revision,
                preparation_profile_id,
                preparation_digest,
                ..
            } => {
                bytes.extend_from_slice(revision.revision_id.as_bytes());
                bytes.extend_from_slice(revision.content_digest.as_bytes());
                append(&mut bytes, preparation_profile_id.as_str().as_bytes())?;
                bytes.extend_from_slice(preparation_digest.as_bytes());
            }
            OverlayViewEntry::Unsaved {
                buffer_snapshot_id,
                buffer_version,
                content_digest,
                byte_length,
                expires_at,
                ..
            } => {
                bytes.extend_from_slice(buffer_snapshot_id.as_bytes());
                bytes.extend_from_slice(&buffer_version.to_be_bytes());
                bytes.extend_from_slice(content_digest.as_bytes());
                bytes.extend_from_slice(&byte_length.to_be_bytes());
                append(&mut bytes, expires_at.as_str().as_bytes())?;
            }
        }
    }
    Ok(bytes)
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), OverlayError> {
    let length = u64::try_from(value.len()).map_err(|_| OverlayError::OverlayQuotaExceeded)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    if output.len() > 8 * 1024 * 1024 {
        return Err(OverlayError::OverlayQuotaExceeded);
    }
    Ok(())
}

fn bounded<T>(values: Vec<T>) -> Result<BoundedList<T, MAX_LIST_ITEMS>, OverlayError> {
    BoundedList::new(values).map_err(|_| OverlayError::OverlayQuotaExceeded)
}
