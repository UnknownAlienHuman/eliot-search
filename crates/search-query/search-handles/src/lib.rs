//! Opaque source-handle minting, resolution, revalidation, expansion, and invalidation.
//!
//! Public tokens are non-self-describing bearer locators only. Authority,
//! source identity, revision, anchor, residency, grant, and disclosure state are
//! held exclusively in server-owned records keyed by a token digest.

#![forbid(unsafe_code)]
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
    BindingId, Blake3Digest32, BufferSnapshotId, DisclosureCeiling, GrantId,
    HandleClass, HandleId, HandleTokenDigest, NonZeroRevision, OpaqueHandleToken,
    OpaqueId, OwnerEpoch, RepresentationId, SearchSourceHandle, SourceMembershipId,
    SourceRevisionId, UnitId, UtcTimestamp, WorkspaceViewRevisionId,
};

/// Closed handle failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HandleError {
    InvalidPolicy,
    TokenInvalid,
    TokenCollision,
    HandleNotFound,
    QuotaExceeded,
    DurableUnsavedDenied,
    RetainedRevisionRequired,
    BindingMismatch,
    GrantMismatch,
    OwnerEpochMismatch,
    SourceOwnerMismatch,
    WorkspaceViewMismatch,
    BufferSnapshotMismatch,
    ResidencyDenied,
    RetentionExpired,
    HandleExpired,
    HandleInvalidated,
    Purged,
    DisclosureDenied,
    RangeBudgetExceeded,
    ReadbackMismatch,
    SourceUnreadable,
    CancellationBeforeMint,
    MintOutcomeUnknown,
    ExpansionCancelled,
    InvalidationBudgetExceeded,
    InvalidTransition,
}

impl HandleError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "HANDLE_POLICY_INVALID",
            Self::TokenInvalid => "HANDLE_TOKEN_INVALID",
            Self::TokenCollision => "HANDLE_TOKEN_COLLISION",
            Self::HandleNotFound => "HANDLE_NOT_FOUND",
            Self::QuotaExceeded => "HANDLE_QUOTA_EXCEEDED",
            Self::DurableUnsavedDenied => "HANDLE_DURABLE_UNSAVED_DENIED",
            Self::RetainedRevisionRequired => "HANDLE_RETAINED_REVISION_REQUIRED",
            Self::BindingMismatch => "HANDLE_BINDING_MISMATCH",
            Self::GrantMismatch => "HANDLE_GRANT_MISMATCH",
            Self::OwnerEpochMismatch => "HANDLE_OWNER_EPOCH_MISMATCH",
            Self::SourceOwnerMismatch => "HANDLE_SOURCE_OWNER_MISMATCH",
            Self::WorkspaceViewMismatch => "HANDLE_WORKSPACE_VIEW_MISMATCH",
            Self::BufferSnapshotMismatch => "HANDLE_BUFFER_SNAPSHOT_MISMATCH",
            Self::ResidencyDenied => "HANDLE_RESIDENCY_DENIED",
            Self::RetentionExpired => "HANDLE_RETENTION_EXPIRED",
            Self::HandleExpired => "HANDLE_EXPIRED",
            Self::HandleInvalidated => "HANDLE_INVALIDATED",
            Self::Purged => "HANDLE_PURGED",
            Self::DisclosureDenied => "HANDLE_DISCLOSURE_DENIED",
            Self::RangeBudgetExceeded => "HANDLE_RANGE_BUDGET_EXCEEDED",
            Self::ReadbackMismatch => "HANDLE_READBACK_MISMATCH",
            Self::SourceUnreadable => "HANDLE_SOURCE_UNREADABLE",
            Self::CancellationBeforeMint => "HANDLE_CANCELLED_BEFORE_MINT",
            Self::MintOutcomeUnknown => "HANDLE_MINT_OUTCOME_UNKNOWN",
            Self::ExpansionCancelled => "HANDLE_EXPANSION_CANCELLED",
            Self::InvalidationBudgetExceeded => "HANDLE_INVALIDATION_BUDGET_EXCEEDED",
            Self::InvalidTransition => "HANDLE_TRANSITION_INVALID",
        }
    }
}

impl fmt::Display for HandleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for HandleError {}

/// Finite handle policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandlePolicy {
    pub max_ephemeral_records: usize,
    pub max_durable_records: usize,
    pub max_expire_batch: usize,
    pub max_invalidate_batch: usize,
    pub max_expansion_bytes: u64,
}

impl HandlePolicy {
    pub const BASELINE: Self = Self {
        max_ephemeral_records: 16_384,
        max_durable_records: 16_384,
        max_expire_batch: 1_024,
        max_invalidate_batch: 1_024,
        max_expansion_bytes: 8 * 1_024 * 1_024,
    };

    pub const fn validate(self) -> Result<Self, HandleError> {
        if self.max_ephemeral_records == 0
            || self.max_durable_records == 0
            || self.max_expire_batch == 0
            || self.max_invalidate_batch == 0
            || self.max_expansion_bytes == 0
        {
            Err(HandleError::InvalidPolicy)
        } else {
            Ok(self)
        }
    }
}

/// Exact retained immutable source target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedSourceTarget {
    pub source_membership_id: SourceMembershipId,
    pub source_owner_generation: Blake3Digest32,
    pub source_revision_id: SourceRevisionId,
    pub representation_id: RepresentationId,
    pub unit_id: UnitId,
    pub byte_start: u64,
    pub byte_end: u64,
    pub excerpt_digest: Blake3Digest32,
    pub content_digest: Blake3Digest32,
    pub profile_digest: Blake3Digest32,
    pub residency_digest: Blake3Digest32,
    pub retention_lease_ref: OpaqueId,
    pub retention_expires_at: Option<UtcTimestamp>,
}

/// Exact authenticated unsaved-buffer target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsavedBufferTarget {
    pub source_membership_id: SourceMembershipId,
    pub workspace_view_revision_id: WorkspaceViewRevisionId,
    pub buffer_snapshot_id: BufferSnapshotId,
    pub buffer_version: u64,
    pub byte_start: u64,
    pub byte_end: u64,
    pub excerpt_digest: Blake3Digest32,
    pub content_digest: Blake3Digest32,
    pub profile_digest: Blake3Digest32,
    pub residency_digest: Blake3Digest32,
}

/// Server-owned exact target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandleTarget {
    RetainedSource(RetainedSourceTarget),
    UnsavedBuffer(UnsavedBufferTarget),
}

impl HandleTarget {
    #[must_use]
    pub const fn source_membership_id(&self) -> SourceMembershipId {
        match self {
            Self::RetainedSource(target) => target.source_membership_id,
            Self::UnsavedBuffer(target) => target.source_membership_id,
        }
    }

    #[must_use]
    pub const fn range(&self) -> (u64, u64) {
        match self {
            Self::RetainedSource(target) => (target.byte_start, target.byte_end),
            Self::UnsavedBuffer(target) => (target.byte_start, target.byte_end),
        }
    }

    #[must_use]
    pub const fn residency_digest(&self) -> Blake3Digest32 {
        match self {
            Self::RetainedSource(target) => target.residency_digest,
            Self::UnsavedBuffer(target) => target.residency_digest,
        }
    }
}

/// Authority binding retained only in the server record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleBinding {
    pub binding_id: BindingId,
    pub grant_id: GrantId,
    pub owner_epoch: OwnerEpoch,
    pub disclosure_ceiling: DisclosureCeiling,
}

/// Handle lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleRecordState {
    Active,
    Invalidated,
    Expired,
}

/// Server-owned handle record keyed by token digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleRecord {
    pub handle_id: HandleId,
    pub handle_revision: NonZeroRevision,
    pub handle_class: HandleClass,
    pub token_digest: HandleTokenDigest,
    pub target: HandleTarget,
    pub binding: HandleBinding,
    pub created_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
    pub state: HandleRecordState,
    pub invalidation_generation: u64,
}

/// CSPRNG/token-digest output supplied by an injected crypto provider.
///
/// Debug output never includes plaintext token bytes.
pub struct MintMaterial {
    pub handle_id: HandleId,
    pub opaque_token: OpaqueHandleToken,
    pub token_digest: HandleTokenDigest,
    pub created_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
}

impl fmt::Debug for MintMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MintMaterial")
            .field("handle_id", &self.handle_id)
            .field("opaque_token", &"<redacted>")
            .field("token_digest", &self.token_digest)
            .field("created_at", &self.created_at)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Crypto seam for random token minting and dedicated-domain digest lookup.
pub trait HandleCryptoPort {
    type Error;

    fn mint_material(&mut self) -> Result<MintMaterial, Self::Error>;
    fn token_digest(
        &self,
        token: &OpaqueHandleToken,
    ) -> Result<HandleTokenDigest, Self::Error>;
}

/// Finite server-owned handle store.
#[derive(Debug)]
pub struct HandleStore {
    policy: HandlePolicy,
    records: BTreeMap<HandleTokenDigest, HandleRecord>,
    by_id: BTreeMap<HandleId, HandleTokenDigest>,
}

impl HandleStore {
    pub fn new(policy: HandlePolicy) -> Result<Self, HandleError> {
        Ok(Self {
            policy: policy.validate()?,
            records: BTreeMap::new(),
            by_id: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Mints a memory-only ephemeral handle.
    pub fn mint_ephemeral<C: HandleCryptoPort>(
        &mut self,
        target: HandleTarget,
        binding: HandleBinding,
        crypto: &mut C,
        cancelled: bool,
    ) -> Result<SearchSourceHandle, HandleError> {
        if cancelled {
            return Err(HandleError::CancellationBeforeMint);
        }
        self.enforce_quota(HandleClass::Ephemeral)?;
        let material = crypto
            .mint_material()
            .map_err(|_| HandleError::TokenInvalid)?;
        self.insert_record(target, binding, HandleClass::Ephemeral, material)
    }

    /// Mints a durable handle only for immutable retained source revisions.
    pub fn mint_durable_source<C: HandleCryptoPort>(
        &mut self,
        target: RetainedSourceTarget,
        binding: HandleBinding,
        crypto: &mut C,
        cancelled: bool,
    ) -> Result<SearchSourceHandle, HandleError> {
        if cancelled {
            return Err(HandleError::CancellationBeforeMint);
        }
        if target.byte_start >= target.byte_end {
            return Err(HandleError::RetainedRevisionRequired);
        }
        self.enforce_quota(HandleClass::DurableSource)?;
        let material = crypto
            .mint_material()
            .map_err(|_| HandleError::TokenInvalid)?;
        self.insert_record(
            HandleTarget::RetainedSource(target),
            binding,
            HandleClass::DurableSource,
            material,
        )
    }

    fn insert_record(
        &mut self,
        target: HandleTarget,
        binding: HandleBinding,
        class: HandleClass,
        material: MintMaterial,
    ) -> Result<SearchSourceHandle, HandleError> {
        let (start, end) = target.range();
        if start >= end || material.expires_at <= material.created_at {
            return Err(HandleError::InvalidPolicy);
        }
        if self.records.contains_key(&material.token_digest)
            || self.by_id.contains_key(&material.handle_id)
        {
            return Err(HandleError::TokenCollision);
        }
        let record = HandleRecord {
            handle_id: material.handle_id,
            handle_revision: NonZeroRevision::new(1)
                .map_err(|_| HandleError::InvalidTransition)?,
            handle_class: class,
            token_digest: material.token_digest,
            target,
            binding,
            created_at: material.created_at.clone(),
            expires_at: material.expires_at.clone(),
            state: HandleRecordState::Active,
            invalidation_generation: 0,
        };
        self.by_id.insert(record.handle_id, record.token_digest);
        self.records.insert(record.token_digest, record);
        Ok(SearchSourceHandle {
            handle_id: material.handle_id,
            handle_revision: NonZeroRevision::new(1)
                .map_err(|_| HandleError::InvalidTransition)?,
            handle_class: class,
            expires_at: Some(material.expires_at),
            opaque_token: material.opaque_token,
        })
    }

    /// Resolves a public token to its private server record.
    pub fn resolve<C: HandleCryptoPort>(
        &self,
        handle: &SearchSourceHandle,
        crypto: &C,
    ) -> Result<&HandleRecord, HandleError> {
        let digest = crypto
            .token_digest(&handle.opaque_token)
            .map_err(|_| HandleError::HandleNotFound)?;
        let record = self
            .records
            .get(&digest)
            .ok_or(HandleError::HandleNotFound)?;
        if record.handle_id != handle.handle_id
            || record.handle_revision != handle.handle_revision
            || record.handle_class != handle.handle_class
        {
            return Err(HandleError::HandleNotFound);
        }
        Ok(record)
    }

    /// Monotonically invalidates a bounded exact scope.
    pub fn invalidate(
        &mut self,
        scope: &HandleInvalidationScope,
        generation: u64,
    ) -> Result<InvalidationReceipt, HandleError> {
        let matching = self
            .records
            .iter()
            .filter(|(_, record)| scope.matches(record))
            .map(|(digest, _)| *digest)
            .take(self.policy.max_invalidate_batch.saturating_add(1))
            .collect::<Vec<_>>();
        if matching.len() > self.policy.max_invalidate_batch {
            return Err(HandleError::InvalidationBudgetExceeded);
        }
        let mut invalidated = 0_usize;
        for digest in matching {
            if let Some(record) = self.records.get_mut(&digest) {
                if record.state == HandleRecordState::Active {
                    record.state = HandleRecordState::Invalidated;
                    record.invalidation_generation = generation;
                    record.handle_revision = record
                        .handle_revision
                        .checked_next()
                        .map_err(|_| HandleError::InvalidTransition)?;
                    invalidated = invalidated.saturating_add(1);
                }
            }
        }
        Ok(InvalidationReceipt {
            generation,
            invalidated,
        })
    }

    /// Expires a bounded deterministic slice at caller-supplied time.
    pub fn expire(&mut self, now: &UtcTimestamp) -> ExpiryReceipt {
        let candidates = self
            .records
            .iter()
            .filter(|(_, record)| {
                record.state == HandleRecordState::Active && record.expires_at <= *now
            })
            .map(|(digest, _)| *digest)
            .take(self.policy.max_expire_batch)
            .collect::<Vec<_>>();
        let mut expired = 0_usize;
        for digest in candidates {
            if let Some(record) = self.records.get_mut(&digest) {
                record.state = HandleRecordState::Expired;
                expired = expired.saturating_add(1);
            }
        }
        let more_expired = self.records.values().any(|record| {
            record.state == HandleRecordState::Active && record.expires_at <= *now
        });
        ExpiryReceipt {
            expired,
            more_expired,
        }
    }

    fn enforce_quota(&self, class: HandleClass) -> Result<(), HandleError> {
        let used = self
            .records
            .values()
            .filter(|record| record.handle_class == class && record.state == HandleRecordState::Active)
            .count();
        let limit = match class {
            HandleClass::Ephemeral => self.policy.max_ephemeral_records,
            HandleClass::DurableSource => self.policy.max_durable_records,
        };
        if used >= limit {
            Err(HandleError::QuotaExceeded)
        } else {
            Ok(())
        }
    }
}

/// Current authority and lifecycle inputs for handle revalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleValidationContext {
    pub binding_id: BindingId,
    pub grant_id: GrantId,
    pub owner_epoch: OwnerEpoch,
    pub source_owner_generation: Option<Blake3Digest32>,
    pub workspace_view_revision_id: Option<WorkspaceViewRevisionId>,
    pub buffer_snapshot_id: Option<BufferSnapshotId>,
    pub allowed_residencies: BTreeSet<Blake3Digest32>,
    pub purged_memberships: BTreeSet<SourceMembershipId>,
    pub now: UtcTimestamp,
    pub requested_bytes: u64,
    pub disclosure_ceiling: DisclosureCeiling,
}

/// Ephemeral exact expansion permit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleExpansionPermit {
    pub handle_id: HandleId,
    pub handle_revision: NonZeroRevision,
    pub target: HandleTarget,
    pub maximum_bytes: u64,
    pub authorization_digest: Blake3Digest32,
}

/// Revalidates possession against current server authority.
pub fn revalidate(
    record: &HandleRecord,
    context: &HandleValidationContext,
    authorization_digest: Blake3Digest32,
) -> Result<HandleExpansionPermit, HandleError> {
    match record.state {
        HandleRecordState::Invalidated => return Err(HandleError::HandleInvalidated),
        HandleRecordState::Expired => return Err(HandleError::HandleExpired),
        HandleRecordState::Active => {}
    }
    if record.expires_at <= context.now {
        return Err(HandleError::HandleExpired);
    }
    if record.binding.binding_id != context.binding_id {
        return Err(HandleError::BindingMismatch);
    }
    if record.binding.grant_id != context.grant_id {
        return Err(HandleError::GrantMismatch);
    }
    if record.binding.owner_epoch != context.owner_epoch {
        return Err(HandleError::OwnerEpochMismatch);
    }
    if context
        .purged_memberships
        .contains(&record.target.source_membership_id())
    {
        return Err(HandleError::Purged);
    }
    if !context
        .allowed_residencies
        .contains(&record.target.residency_digest())
    {
        return Err(HandleError::ResidencyDenied);
    }
    if context.disclosure_ceiling < record.binding.disclosure_ceiling {
        return Err(HandleError::DisclosureDenied);
    }
    let (start, end) = record.target.range();
    let length = end.saturating_sub(start);
    if context.requested_bytes == 0
        || context.requested_bytes > length
        || context.requested_bytes > u64::MAX.min(length)
    {
        return Err(HandleError::RangeBudgetExceeded);
    }
    match &record.target {
        HandleTarget::RetainedSource(target) => {
            if context.source_owner_generation != Some(target.source_owner_generation) {
                return Err(HandleError::SourceOwnerMismatch);
            }
            if target
                .retention_expires_at
                .as_ref()
                .is_some_and(|expiry| expiry <= &context.now)
            {
                return Err(HandleError::RetentionExpired);
            }
        }
        HandleTarget::UnsavedBuffer(target) => {
            if context.workspace_view_revision_id != Some(target.workspace_view_revision_id) {
                return Err(HandleError::WorkspaceViewMismatch);
            }
            if context.buffer_snapshot_id != Some(target.buffer_snapshot_id) {
                return Err(HandleError::BufferSnapshotMismatch);
            }
        }
    }
    Ok(HandleExpansionPermit {
        handle_id: record.handle_id,
        handle_revision: record.handle_revision,
        target: record.target.clone(),
        maximum_bytes: context.requested_bytes,
        authorization_digest,
    })
}

/// Exact expansion readback result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleReadback {
    pub target: HandleTarget,
    pub content_digest: Blake3Digest32,
    pub excerpt_digest: Blake3Digest32,
    pub bytes: Vec<u8>,
}

/// Exact expansion readback seam.
pub trait HandleReadbackPort {
    type Error;

    fn read_exact(
        &mut self,
        permit: &HandleExpansionPermit,
    ) -> Result<HandleReadback, Self::Error>;
}

/// Bounded source-backed handle expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleExpansion {
    pub handle_id: HandleId,
    pub handle_revision: NonZeroRevision,
    pub target: HandleTarget,
    pub bytes: Vec<u8>,
    pub content_digest: Blake3Digest32,
    pub excerpt_digest: Blake3Digest32,
}

/// Expands only the exact revalidated target and performs a second authority check.
pub fn expand<P: HandleReadbackPort>(
    record: &HandleRecord,
    before: &HandleValidationContext,
    after: &HandleValidationContext,
    authorization_digest: Blake3Digest32,
    port: &mut P,
    cancelled_before_emission: bool,
) -> Result<HandleExpansion, HandleError> {
    let permit = revalidate(record, before, authorization_digest)?;
    let readback = port
        .read_exact(&permit)
        .map_err(|_| HandleError::SourceUnreadable)?;
    if readback.target != permit.target
        || readback.bytes.is_empty()
        || u64::try_from(readback.bytes.len()).unwrap_or(u64::MAX) > permit.maximum_bytes
    {
        return Err(HandleError::ReadbackMismatch);
    }
    let expected = match &permit.target {
        HandleTarget::RetainedSource(target) => {
            (target.content_digest, target.excerpt_digest)
        }
        HandleTarget::UnsavedBuffer(target) => {
            (target.content_digest, target.excerpt_digest)
        }
    };
    if (readback.content_digest, readback.excerpt_digest) != expected {
        return Err(HandleError::ReadbackMismatch);
    }
    if cancelled_before_emission {
        return Err(HandleError::ExpansionCancelled);
    }
    revalidate(record, after, authorization_digest)?;
    Ok(HandleExpansion {
        handle_id: record.handle_id,
        handle_revision: record.handle_revision,
        target: permit.target,
        bytes: readback.bytes,
        content_digest: readback.content_digest,
        excerpt_digest: readback.excerpt_digest,
    })
}

/// Monotone invalidation scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandleInvalidationScope {
    AllEphemeral,
    Binding(BindingId),
    Grant(GrantId),
    OwnerEpoch(OwnerEpoch),
    Membership(SourceMembershipId),
    WorkspaceView(WorkspaceViewRevisionId),
    BufferSnapshot(BufferSnapshotId),
    Residency(Blake3Digest32),
    RetentionLease(OpaqueId),
}

impl HandleInvalidationScope {
    fn matches(&self, record: &HandleRecord) -> bool {
        match self {
            Self::AllEphemeral => record.handle_class == HandleClass::Ephemeral,
            Self::Binding(value) => record.binding.binding_id == *value,
            Self::Grant(value) => record.binding.grant_id == *value,
            Self::OwnerEpoch(value) => record.binding.owner_epoch == *value,
            Self::Membership(value) => record.target.source_membership_id() == *value,
            Self::WorkspaceView(value) => matches!(
                &record.target,
                HandleTarget::UnsavedBuffer(target)
                    if target.workspace_view_revision_id == *value
            ),
            Self::BufferSnapshot(value) => matches!(
                &record.target,
                HandleTarget::UnsavedBuffer(target) if target.buffer_snapshot_id == *value
            ),
            Self::Residency(value) => record.target.residency_digest() == *value,
            Self::RetentionLease(value) => matches!(
                &record.target,
                HandleTarget::RetainedSource(target) if target.retention_lease_ref == *value
            ),
        }
    }
}

/// Content-free invalidation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidationReceipt {
    pub generation: u64,
    pub invalidated: usize,
}

/// Bounded expiry receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpiryReceipt {
    pub expired: usize,
    pub more_expired: bool,
}
