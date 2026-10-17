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

mod invalidation;

pub use invalidation::{
    HandleInvalidationProgress, HandleSecurityInvalidation, HandleSecurityInvalidationReceipt,
};

use core::fmt;
use std::collections::BTreeMap;
use std::time::Instant;

use search_contracts::lifecycle::{
    EphemeralSourceHandleRecord, LifecycleRecordStatus, SearchSourceHandleRecord,
};
use search_contracts::{
    BindingId, Blake3Digest32, Blake3ExactInputLimit, BoundedList, BufferSnapshotId,
    DisclosureCeiling, GrantId, HandleClass, HandleId, HandleTokenDigest, MAX_BLAKE3_EXACT_BYTES,
    NativeAnchor, NonZeroRevision, ObjectResidencyKeyDigest, OpaqueHandleToken, OpaqueRef,
    OwnerEpoch, RepresentationId, SearchSourceHandle, SourceMembershipId, SourceView, UnitId,
    UtcTimestamp, WorkspaceViewRevisionId, blake3_exact_bytes,
};
use search_ports::PackageOpaque;

pub use search_contracts::lifecycle::{
    EphemeralHandleTarget, RetainedSourceTarget, UnsavedBufferTarget,
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
    SourceProtectionUnavailable,
    SourceProtectionMismatch,
    AuthorizationDenied,
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
            Self::SourceProtectionUnavailable => "HANDLE_SOURCE_PROTECTION_UNAVAILABLE",
            Self::SourceProtectionMismatch => "HANDLE_SOURCE_PROTECTION_MISMATCH",
            Self::AuthorizationDenied => "HANDLE_AUTHORIZATION_DENIED",
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

/// Source membership and projection coordinates omitted from the shared C00
/// lifecycle target. These remain private store metadata, never a second target
/// contract or an authority source.
struct HandleSourceBinding {
    membership: SourceMembershipId,
    representation: Option<RepresentationId>,
    unit: Option<UnitId>,
}

/// One private stored entry. C00 lifecycle records are the canonical persisted
/// shape; these extra fields exist only for package-local invalidation,
/// reauthorization and the process-local source protection guard.
struct StoredHandle {
    record: SearchSourceHandleRecord,
    source: HandleSourceBinding,
    owner_epoch: OwnerEpoch,
    disclosure_ceiling: DisclosureCeiling,
    invalidation_generation: u64,
    monotonic_deadline: Option<Instant>,
    source_protection: Option<Box<dyn RetainedSourceProtectionGuard>>,
}

impl StoredHandle {
    pub(crate) const fn record(&self) -> &SearchSourceHandleRecord {
        &self.record
    }

    pub(crate) const fn handle_id(&self) -> HandleId {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => record.handle_id,
            SearchSourceHandleRecord::DurableSource(record) => record.handle_id,
        }
    }

    pub(crate) const fn handle_revision(&self) -> NonZeroRevision {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => record.handle_revision,
            SearchSourceHandleRecord::DurableSource(record) => record.handle_revision,
        }
    }

    pub(crate) const fn token_digest(&self) -> HandleTokenDigest {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => record.token_digest,
            SearchSourceHandleRecord::DurableSource(record) => record.token_digest,
        }
    }

    pub(crate) const fn class(&self) -> HandleClass {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(_) => HandleClass::Ephemeral,
            SearchSourceHandleRecord::DurableSource(_) => HandleClass::DurableSource,
        }
    }

    pub(crate) const fn status(&self) -> LifecycleRecordStatus {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => record.status,
            SearchSourceHandleRecord::DurableSource(record) => record.status,
        }
    }

    pub(crate) const fn expires_at(&self) -> Option<&UtcTimestamp> {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => Some(&record.expires_at),
            SearchSourceHandleRecord::DurableSource(record) => record.retention_expiry.as_ref(),
        }
    }

    pub(crate) fn set_revision_status(
        &mut self,
        revision: NonZeroRevision,
        status: LifecycleRecordStatus,
        generation: u64,
    ) {
        match &mut self.record {
            SearchSourceHandleRecord::Ephemeral(record) => {
                record.handle_revision = revision;
                record.status = status;
            }
            SearchSourceHandleRecord::DurableSource(record) => {
                record.handle_revision = revision;
                record.status = status;
            }
        }
        self.invalidation_generation = generation;
        if status != LifecycleRecordStatus::Active {
            self.source_protection = None;
            self.monotonic_deadline = None;
        }
    }

    fn expired_at(&self, now: Instant) -> bool {
        self.monotonic_deadline
            .is_none_or(|deadline| now >= deadline)
    }

    fn require_unexpired(&mut self, now: Instant) -> Result<(), HandleError> {
        if self.expired_at(now) {
            self.set_revision_status(
                self.handle_revision(),
                LifecycleRecordStatus::Expired,
                self.invalidation_generation,
            );
            return Err(HandleError::HandleExpired);
        }
        Ok(())
    }

    pub(crate) const fn target(&self) -> Option<&EphemeralHandleTarget> {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => Some(&record.target),
            SearchSourceHandleRecord::DurableSource(_) => None,
        }
    }

    pub(crate) const fn workspace_view_revision_id(&self) -> Option<WorkspaceViewRevisionId> {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => match &record.target {
                EphemeralHandleTarget::RetainedSource(target) => target.workspace_view_revision_ref,
                EphemeralHandleTarget::UnsavedBuffer(target) => {
                    Some(target.workspace_view_revision_ref)
                }
            },
            SearchSourceHandleRecord::DurableSource(record) => record.workspace_view_revision_ref,
        }
    }

    pub(crate) const fn buffer_snapshot_id(&self) -> Option<BufferSnapshotId> {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => match &record.target {
                EphemeralHandleTarget::UnsavedBuffer(target) => Some(target.buffer_snapshot_id),
                EphemeralHandleTarget::RetainedSource(_) => None,
            },
            SearchSourceHandleRecord::DurableSource(_) => None,
        }
    }

    pub(crate) const fn residency(&self) -> Option<ObjectResidencyKeyDigest> {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(record) => match &record.target {
                EphemeralHandleTarget::RetainedSource(target) => {
                    Some(target.object_residency_key_digest)
                }
                EphemeralHandleTarget::UnsavedBuffer(_) => None,
            },
            SearchSourceHandleRecord::DurableSource(record) => {
                Some(record.object_residency_key_digest)
            }
        }
    }

    pub(crate) const fn retention_lease_ref(&self) -> Option<&OpaqueRef> {
        match &self.record {
            SearchSourceHandleRecord::Ephemeral(_) => None,
            SearchSourceHandleRecord::DurableSource(record) => Some(&record.retention_lease_ref),
        }
    }

    pub(crate) const fn source_membership_id(&self) -> SourceMembershipId {
        self.source.membership
    }
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
    /// Process-local expiry captured from the original mint window; never serialized.
    pub monotonic_deadline: Instant,
}

impl fmt::Debug for MintMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MintMaterial")
            .field("handle_id", &self.handle_id)
            .field("opaque_token", &"<redacted>")
            .field("token_digest", &self.token_digest)
            .field("lifetime", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Crypto seam for random token minting and dedicated-domain digest lookup.
pub trait HandleCryptoPort {
    type Error;

    fn mint_material(&mut self) -> Result<MintMaterial, Self::Error>;
    fn token_digest(&self, token: &OpaqueHandleToken) -> Result<HandleTokenDigest, Self::Error>;
}

/// Source-owner-created protection held for the full ephemeral handle lifetime.
/// Drop must release this exact process-local guard. It is not a receipt or a
/// source of authorization.
pub trait RetainedSourceProtectionGuard: PackageOpaque {
    fn target(&self) -> &RetainedSourceTarget;
    fn expires_at(&self) -> &UtcTimestamp;
    /// The exact process-local deadline supplied to `protect`.
    fn monotonic_deadline(&self) -> Instant;
    fn revalidate_current(&self) -> Result<(), HandleError>;
}

/// Narrow source protection seam. A retained-source handle cannot be minted
/// unless an actual source owner returns a guard for its exact C00 target.
pub trait RetainedSourceProtectionPort: PackageOpaque {
    type Guard: RetainedSourceProtectionGuard;

    fn protect(
        &mut self,
        target: &RetainedSourceTarget,
        handle_expires_at: &UtcTimestamp,
        handle_monotonic_deadline: Instant,
    ) -> Result<Self::Guard, HandleError>;
}

/// Checkpoint phases required by exact handle expansion.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HandleAuthorizationPhase {
    BeforeSourceReadback,
    BeforeOutput,
}

/// Opaque current C18 authorization checkpoint held through its operation.
pub trait HandleAuthorizationCheckpoint: PackageOpaque {
    fn revalidate(
        &self,
        record: &SearchSourceHandleRecord,
        source_membership_id: Option<SourceMembershipId>,
        representation_id: Option<RepresentationId>,
        unit_id: Option<UnitId>,
        owner_epoch: OwnerEpoch,
        disclosure_ceiling: DisclosureCeiling,
    ) -> Result<(), HandleError>;
}

/// Owner port that creates a fresh opaque C18 checkpoint for each phase.
pub trait HandleReauthorizationPort: PackageOpaque {
    type Checkpoint: HandleAuthorizationCheckpoint;

    fn checkpoint(
        &mut self,
        record: &SearchSourceHandleRecord,
        source_membership_id: Option<SourceMembershipId>,
        representation_id: Option<RepresentationId>,
        unit_id: Option<UnitId>,
        owner_epoch: OwnerEpoch,
        disclosure_ceiling: DisclosureCeiling,
        phase: HandleAuthorizationPhase,
    ) -> Result<Self::Checkpoint, HandleError>;
}

/// Package-created readback permit. Its fields and constructors are private;
/// the token, selected C00 target and exact original anchor stay store-owned.
pub struct HandleExpansionPermit {
    target: EphemeralHandleTarget,
    source_membership_id: Option<SourceMembershipId>,
    representation_id: Option<RepresentationId>,
    unit_id: Option<UnitId>,
    maximum_bytes: u64,
}

impl HandleExpansionPermit {
    #[must_use]
    pub const fn target(&self) -> &EphemeralHandleTarget {
        &self.target
    }

    #[must_use]
    pub const fn source_membership_id(&self) -> Option<SourceMembershipId> {
        self.source_membership_id
    }

    #[must_use]
    pub const fn representation_id(&self) -> Option<RepresentationId> {
        self.representation_id
    }

    #[must_use]
    pub const fn unit_id(&self) -> Option<UnitId> {
        self.unit_id
    }

    #[must_use]
    pub const fn maximum_bytes(&self) -> u64 {
        self.maximum_bytes
    }
}

/// Exact source bytes returned by the immutable source owner. Digests are
/// computed by `search-handles`; the port cannot authorize by echoing them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleReadback {
    pub target: EphemeralHandleTarget,
    pub bytes: Vec<u8>,
}

/// Exact immutable source readback seam.
pub trait HandleReadbackPort {
    type Error;
    type OwnerObservation: PackageOpaque;

    fn read_exact(
        &mut self,
        permit: &HandleExpansionPermit,
    ) -> Result<(HandleReadback, Self::OwnerObservation), Self::Error>;

    fn verify_owner_observation(
        &self,
        observation: &Self::OwnerObservation,
        permit: &HandleExpansionPermit,
    ) -> Result<(), Self::Error>;
}

/// Bounded source-backed expansion delivered while the store and current
/// authorization checkpoint remain borrowed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleExpansion {
    pub handle_id: HandleId,
    pub handle_revision: NonZeroRevision,
    pub target: EphemeralHandleTarget,
    pub bytes: Vec<u8>,
    pub excerpt_digest: Blake3Digest32,
}

/// Handle-side failure or caller output failure.
#[derive(Debug)]
pub enum HandleDeliveryError<E> {
    Handle(HandleError),
    Output(E),
}

/// Finite server-owned handle store. The primary map is keyed only by the
/// dedicated-domain token digest; C00 records and protection guards never
/// contain the plaintext bearer token.
pub struct HandleStore {
    policy: HandlePolicy,
    records: BTreeMap<HandleTokenDigest, StoredHandle>,
    by_id: BTreeMap<HandleId, HandleTokenDigest>,
}

impl fmt::Debug for HandleStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandleStore")
            .field("policy", &self.policy)
            .field("records", &self.records.len())
            .field("by_id", &self.by_id.len())
            .finish_non_exhaustive()
    }
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

    /// Mints a memory-only unsaved-buffer handle. Retained-source targets must
    /// use `mint_ephemeral_retained_source`, which requires a source-owner guard.
    /// `source_membership_id` must be resolved by the authenticated buffer owner;
    /// it is never inferred from a source ID, workspace or buffer snapshot.
    pub fn mint_ephemeral<C: HandleCryptoPort>(
        &mut self,
        target: EphemeralHandleTarget,
        source_membership_id: SourceMembershipId,
        binding_id: BindingId,
        grant_id: GrantId,
        owner_epoch: OwnerEpoch,
        disclosure_ceiling: DisclosureCeiling,
        crypto: &mut C,
        cancelled: bool,
    ) -> Result<SearchSourceHandle, HandleError> {
        let EphemeralHandleTarget::UnsavedBuffer(buffer) = target else {
            return Err(HandleError::SourceProtectionUnavailable);
        };
        if cancelled {
            return Err(HandleError::CancellationBeforeMint);
        }
        self.enforce_quota(HandleClass::Ephemeral)?;
        let (material, token_digest) = verified_mint_material(crypto)?;
        let source = HandleSourceBinding {
            membership: source_membership_id,
            representation: None,
            unit: None,
        };
        self.insert_ephemeral(
            EphemeralHandleTarget::UnsavedBuffer(buffer),
            source,
            binding_id,
            grant_id,
            owner_epoch,
            disclosure_ceiling,
            material,
            token_digest,
            None,
        )
    }

    /// Mints a retained-source handle only while the exact source owner holds a
    /// protection guard through the handle's expiry. There is intentionally no
    /// fallback when the C17 source-protection producer is unavailable.
    #[allow(clippy::too_many_arguments)]
    pub fn mint_ephemeral_retained_source<C, P>(
        &mut self,
        target: RetainedSourceTarget,
        source_membership_id: SourceMembershipId,
        representation_id: RepresentationId,
        unit_id: UnitId,
        binding_id: BindingId,
        grant_id: GrantId,
        owner_epoch: OwnerEpoch,
        disclosure_ceiling: DisclosureCeiling,
        protection: &mut P,
        crypto: &mut C,
        cancelled: bool,
    ) -> Result<SearchSourceHandle, HandleError>
    where
        C: HandleCryptoPort,
        P: RetainedSourceProtectionPort,
    {
        if cancelled {
            return Err(HandleError::CancellationBeforeMint);
        }
        validate_retained_target(&target)?;
        self.enforce_quota(HandleClass::Ephemeral)?;
        let (material, token_digest) = verified_mint_material(crypto)?;
        let guard =
            protection.protect(&target, &material.expires_at, material.monotonic_deadline)?;
        if guard.target() != &target
            || guard.expires_at() < &material.expires_at
            || guard.monotonic_deadline() != material.monotonic_deadline
        {
            return Err(HandleError::SourceProtectionMismatch);
        }
        guard.revalidate_current()?;
        self.insert_ephemeral(
            EphemeralHandleTarget::RetainedSource(target),
            HandleSourceBinding {
                membership: source_membership_id,
                representation: Some(representation_id),
                unit: Some(unit_id),
            },
            binding_id,
            grant_id,
            owner_epoch,
            disclosure_ceiling,
            material,
            token_digest,
            Some(Box::new(guard)),
        )
    }

    fn insert_ephemeral(
        &mut self,
        target: EphemeralHandleTarget,
        source: HandleSourceBinding,
        binding_id: BindingId,
        grant_id: GrantId,
        owner_epoch: OwnerEpoch,
        disclosure_ceiling: DisclosureCeiling,
        material: MintMaterial,
        token_digest: HandleTokenDigest,
        source_protection: Option<Box<dyn RetainedSourceProtectionGuard>>,
    ) -> Result<SearchSourceHandle, HandleError> {
        if Instant::now() >= material.monotonic_deadline {
            return Err(HandleError::HandleExpired);
        }
        if material.expires_at <= material.created_at
            || self.records.contains_key(&token_digest)
            || self.by_id.contains_key(&material.handle_id)
        {
            return Err(
                if self.records.contains_key(&token_digest)
                    || self.by_id.contains_key(&material.handle_id)
                {
                    HandleError::TokenCollision
                } else {
                    HandleError::InvalidPolicy
                },
            );
        }
        let handle_revision =
            NonZeroRevision::new(1).map_err(|_| HandleError::InvalidTransition)?;
        let invalidation_refs = BoundedList::new(Vec::<OpaqueRef>::new())
            .map_err(|_| HandleError::InvalidTransition)?;
        let record = SearchSourceHandleRecord::Ephemeral(EphemeralSourceHandleRecord {
            handle_id: material.handle_id,
            handle_revision,
            token_digest,
            binding_id,
            grant_id,
            target,
            created_at: material.created_at.clone(),
            expires_at: material.expires_at.clone(),
            invalidation_refs,
            status: LifecycleRecordStatus::Active,
        });
        let stored = StoredHandle {
            record,
            source,
            owner_epoch,
            disclosure_ceiling,
            invalidation_generation: 0,
            monotonic_deadline: Some(material.monotonic_deadline),
            source_protection,
        };
        self.by_id.insert(material.handle_id, token_digest);
        self.records.insert(token_digest, stored);
        Ok(SearchSourceHandle {
            handle_id: material.handle_id,
            handle_revision,
            handle_class: HandleClass::Ephemeral,
            expires_at: Some(material.expires_at),
            opaque_token: material.opaque_token,
        })
    }

    /// Resolves the public bearer token inside this store, reauthorizes twice,
    /// verifies exact owner readback bytes, and invokes delivery while the
    /// current C18 checkpoint and mutable store borrow remain live. The only
    /// expansion request is the original C00 anchor; callers cannot supply a
    /// wider range or a record clone.
    pub fn expand<C, A, P, F, T, E>(
        &mut self,
        handle: &SearchSourceHandle,
        requested_max_bytes: u64,
        crypto: &C,
        authorization: &mut A,
        readback: &mut P,
        deliver: F,
    ) -> Result<T, HandleDeliveryError<E>>
    where
        C: HandleCryptoPort,
        A: HandleReauthorizationPort,
        P: HandleReadbackPort,
        F: FnOnce(&HandleExpansion) -> Result<T, E>,
    {
        self.expand_with_clock(
            handle,
            requested_max_bytes,
            crypto,
            authorization,
            readback,
            deliver,
            Instant::now,
        )
    }

    // The clock seam is private: public callers never supply time authority.
    fn expand_with_clock<C, A, P, F, T, E, N>(
        &mut self,
        handle: &SearchSourceHandle,
        requested_max_bytes: u64,
        crypto: &C,
        authorization: &mut A,
        readback: &mut P,
        deliver: F,
        mut now: N,
    ) -> Result<T, HandleDeliveryError<E>>
    where
        C: HandleCryptoPort,
        A: HandleReauthorizationPort,
        P: HandleReadbackPort,
        F: FnOnce(&HandleExpansion) -> Result<T, E>,
        N: FnMut() -> Instant,
    {
        let token_digest = crypto
            .token_digest(&handle.opaque_token)
            .map_err(|_| HandleDeliveryError::Handle(HandleError::HandleNotFound))?;
        let store_limit = self.policy.max_expansion_bytes;
        if requested_max_bytes == 0 || requested_max_bytes > store_limit {
            return Err(HandleDeliveryError::Handle(
                HandleError::RangeBudgetExceeded,
            ));
        }
        let stored = self
            .records
            .get_mut(&token_digest)
            .ok_or(HandleDeliveryError::Handle(HandleError::HandleNotFound))?;
        if stored.handle_id() != handle.handle_id
            || stored.handle_revision() != handle.handle_revision
            || stored.class() != handle.handle_class
            || stored.token_digest() != token_digest
            || stored.expires_at() != handle.expires_at.as_ref()
        {
            return Err(HandleDeliveryError::Handle(HandleError::HandleNotFound));
        }
        match stored.status() {
            LifecycleRecordStatus::Active => {}
            LifecycleRecordStatus::Revoked => {
                return Err(HandleDeliveryError::Handle(HandleError::HandleInvalidated));
            }
            LifecycleRecordStatus::Expired => {
                return Err(HandleDeliveryError::Handle(HandleError::HandleExpired));
            }
        }
        stored
            .require_unexpired(now())
            .map_err(HandleDeliveryError::Handle)?;
        let target = stored.target().cloned().ok_or(HandleDeliveryError::Handle(
            HandleError::RetainedRevisionRequired,
        ))?;
        let hash_ceiling = u64::try_from(MAX_BLAKE3_EXACT_BYTES)
            .map_err(|_| HandleDeliveryError::Handle(HandleError::RangeBudgetExceeded))?;
        let permitted_bytes = store_limit.min(requested_max_bytes).min(hash_ceiling);
        let selected_byte_length = anchor_byte_length(target_anchor(&target));
        if selected_byte_length.is_some_and(|length| length == 0 || length > permitted_bytes) {
            return Err(HandleDeliveryError::Handle(
                HandleError::RangeBudgetExceeded,
            ));
        }
        let permit = HandleExpansionPermit {
            target: target.clone(),
            source_membership_id: Some(stored.source.membership),
            representation_id: stored.source.representation,
            unit_id: stored.source.unit,
            maximum_bytes: selected_byte_length.unwrap_or(permitted_bytes),
        };

        let before_read = authorization
            .checkpoint(
                stored.record(),
                Some(stored.source.membership),
                stored.source.representation,
                stored.source.unit,
                stored.owner_epoch,
                stored.disclosure_ceiling,
                HandleAuthorizationPhase::BeforeSourceReadback,
            )
            .map_err(HandleDeliveryError::Handle)?;
        before_read
            .revalidate(
                stored.record(),
                Some(stored.source.membership),
                stored.source.representation,
                stored.source.unit,
                stored.owner_epoch,
                stored.disclosure_ceiling,
            )
            .map_err(HandleDeliveryError::Handle)?;
        stored
            .require_unexpired(now())
            .map_err(HandleDeliveryError::Handle)?;
        if let Some(guard) = &stored.source_protection {
            guard
                .revalidate_current()
                .map_err(HandleDeliveryError::Handle)?;
        } else if matches!(target, EphemeralHandleTarget::RetainedSource(_)) {
            return Err(HandleDeliveryError::Handle(
                HandleError::SourceProtectionUnavailable,
            ));
        }
        drop(before_read);
        stored
            .require_unexpired(now())
            .map_err(HandleDeliveryError::Handle)?;

        let (readback_value, owner_observation) = readback
            .read_exact(&permit)
            .map_err(|_| HandleDeliveryError::Handle(HandleError::SourceUnreadable))?;
        let actual_byte_length = u64::try_from(readback_value.bytes.len())
            .map_err(|_| HandleDeliveryError::Handle(HandleError::ReadbackMismatch))?;
        if readback_value.target != permit.target
            || actual_byte_length == 0
            || actual_byte_length > permit.maximum_bytes
            || selected_byte_length.is_some_and(|length| actual_byte_length != length)
        {
            return Err(HandleDeliveryError::Handle(HandleError::ReadbackMismatch));
        }
        readback
            .verify_owner_observation(&owner_observation, &permit)
            .map_err(|_| HandleDeliveryError::Handle(HandleError::ReadbackMismatch))?;
        let digest_limit = Blake3ExactInputLimit::new(readback_value.bytes.len())
            .map_err(|_| HandleDeliveryError::Handle(HandleError::RangeBudgetExceeded))?;
        let excerpt_digest = blake3_exact_bytes(&readback_value.bytes, digest_limit)
            .map_err(|_| HandleDeliveryError::Handle(HandleError::ReadbackMismatch))?;
        let expected_excerpt_digest = target_excerpt_digest(&permit.target);
        if excerpt_digest != expected_excerpt_digest {
            return Err(HandleDeliveryError::Handle(HandleError::ReadbackMismatch));
        }

        let expansion = HandleExpansion {
            handle_id: stored.handle_id(),
            handle_revision: stored.handle_revision(),
            target: permit.target.clone(),
            bytes: readback_value.bytes,
            excerpt_digest,
        };
        let before_output = authorization
            .checkpoint(
                stored.record(),
                Some(stored.source.membership),
                stored.source.representation,
                stored.source.unit,
                stored.owner_epoch,
                stored.disclosure_ceiling,
                HandleAuthorizationPhase::BeforeOutput,
            )
            .map_err(HandleDeliveryError::Handle)?;
        before_output
            .revalidate(
                stored.record(),
                Some(stored.source.membership),
                stored.source.representation,
                stored.source.unit,
                stored.owner_epoch,
                stored.disclosure_ceiling,
            )
            .map_err(HandleDeliveryError::Handle)?;
        stored
            .require_unexpired(now())
            .map_err(HandleDeliveryError::Handle)?;
        if let Some(guard) = &stored.source_protection {
            guard
                .revalidate_current()
                .map_err(HandleDeliveryError::Handle)?;
        } else if matches!(target, EphemeralHandleTarget::RetainedSource(_)) {
            return Err(HandleDeliveryError::Handle(
                HandleError::SourceProtectionUnavailable,
            ));
        }
        stored
            .require_unexpired(now())
            .map_err(HandleDeliveryError::Handle)?;
        let output = deliver(&expansion).map_err(HandleDeliveryError::Output);
        drop(before_output);
        output
    }

    /// Monotonically invalidates a bounded exact scope. Terminal records keep
    /// their token-digest index but immediately drop any source protection guard.
    pub fn invalidate(
        &mut self,
        scope: &HandleInvalidationScope,
        generation: u64,
    ) -> Result<InvalidationReceipt, HandleError> {
        let mut prepared = Vec::new();
        for (digest, record) in &self.records {
            if !scope.matches(record) || record.status() != LifecycleRecordStatus::Active {
                continue;
            }
            if prepared.len() == self.policy.max_invalidate_batch {
                return Err(HandleError::InvalidationBudgetExceeded);
            }
            if let Some(revision) = invalidation::prepare_revision(record, generation)? {
                prepared.push((*digest, revision));
            }
        }
        let invalidated = prepared.len();
        invalidation::apply_prepared(self, &prepared, generation);
        Ok(InvalidationReceipt {
            generation,
            invalidated,
        })
    }

    /// Expires a bounded deterministic slice using the actual process clock.
    /// UTC wire timestamps never authorize a longer process-local lifetime.
    pub fn expire(&mut self) -> ExpiryReceipt {
        self.expire_at(Instant::now())
    }

    fn expire_at(&mut self, now: Instant) -> ExpiryReceipt {
        let candidates = self
            .records
            .iter()
            .filter(|(_, record)| {
                record.status() == LifecycleRecordStatus::Active && record.expired_at(now)
            })
            .map(|(digest, _)| *digest)
            .take(self.policy.max_expire_batch)
            .collect::<Vec<_>>();
        let mut expired = 0_usize;
        for digest in candidates {
            if let Some(record) = self.records.get_mut(&digest) {
                record.set_revision_status(
                    record.handle_revision(),
                    LifecycleRecordStatus::Expired,
                    record.invalidation_generation,
                );
                expired = expired.saturating_add(1);
            }
        }
        let more_expired = self.records.values().any(|record| {
            record.status() == LifecycleRecordStatus::Active && record.expired_at(now)
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
            .filter(|record| {
                record.class() == class && record.status() == LifecycleRecordStatus::Active
            })
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

fn verified_mint_material<C: HandleCryptoPort>(
    crypto: &mut C,
) -> Result<(MintMaterial, HandleTokenDigest), HandleError> {
    let material = crypto
        .mint_material()
        .map_err(|_| HandleError::TokenInvalid)?;
    if material.expires_at <= material.created_at {
        return Err(HandleError::InvalidPolicy);
    }
    if Instant::now() >= material.monotonic_deadline {
        return Err(HandleError::HandleExpired);
    }
    let computed = crypto
        .token_digest(&material.opaque_token)
        .map_err(|_| HandleError::TokenInvalid)?;
    if computed != material.token_digest {
        return Err(HandleError::TokenInvalid);
    }
    Ok((material, computed))
}

fn validate_retained_target(target: &RetainedSourceTarget) -> Result<(), HandleError> {
    if target.source_namespace_id != target.source_revision_ref.source_namespace_id
        || target.native_anchor.validate().is_err()
        || target.source_view.validate().is_err()
    {
        return Err(HandleError::RetainedRevisionRequired);
    }
    if let SourceView::RetainedRevision(view_revision) = &target.source_view
        && *view_revision != target.source_revision_ref.revision_id
    {
        return Err(HandleError::RetainedRevisionRequired);
    }
    match &target.native_anchor {
        NativeAnchor::TextBytes(anchor) => {
            if anchor.content_digest != target.source_revision_ref.content_digest
                || anchor.byte_start_0 >= anchor.byte_end_exclusive_0
                || anchor.byte_end_exclusive_0 > target.source_revision_ref.byte_length
            {
                return Err(HandleError::RetainedRevisionRequired);
            }
        }
        NativeAnchor::GitBlobBytes(anchor) => {
            if anchor.byte_start_0 >= anchor.byte_end_exclusive_0
                || anchor.byte_end_exclusive_0 > target.source_revision_ref.byte_length
            {
                return Err(HandleError::RetainedRevisionRequired);
            }
        }
        NativeAnchor::PdfRegion(anchor) => {
            if anchor.source_revision_id != target.source_revision_ref.revision_id {
                return Err(HandleError::RetainedRevisionRequired);
            }
        }
        NativeAnchor::ArchiveMember(anchor) => {
            if anchor.archive_revision_id != target.source_revision_ref.revision_id {
                return Err(HandleError::RetainedRevisionRequired);
            }
        }
        NativeAnchor::BufferRange(_) => return Err(HandleError::RetainedRevisionRequired),
    }
    Ok(())
}

fn anchor_byte_range(anchor: &NativeAnchor) -> Option<(u64, u64)> {
    match anchor {
        NativeAnchor::TextBytes(anchor) => Some((anchor.byte_start_0, anchor.byte_end_exclusive_0)),
        NativeAnchor::GitBlobBytes(anchor) => {
            Some((anchor.byte_start_0, anchor.byte_end_exclusive_0))
        }
        NativeAnchor::ArchiveMember(anchor) => anchor_byte_range(&anchor.nested_anchor),
        NativeAnchor::BufferRange(_) | NativeAnchor::PdfRegion(_) => None,
    }
}

fn anchor_byte_length(anchor: &NativeAnchor) -> Option<u64> {
    anchor_byte_range(anchor).and_then(|(start, end)| end.checked_sub(start))
}

const fn target_anchor(target: &EphemeralHandleTarget) -> &NativeAnchor {
    match target {
        EphemeralHandleTarget::RetainedSource(target) => &target.native_anchor,
        EphemeralHandleTarget::UnsavedBuffer(target) => &target.native_anchor,
    }
}

const fn target_excerpt_digest(target: &EphemeralHandleTarget) -> Blake3Digest32 {
    match target {
        EphemeralHandleTarget::RetainedSource(target) => target.excerpt_digest,
        EphemeralHandleTarget::UnsavedBuffer(target) => target.excerpt_digest,
    }
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
    Residency(ObjectResidencyKeyDigest),
    RetentionLease(OpaqueRef),
}

impl HandleInvalidationScope {
    fn matches(&self, record: &StoredHandle) -> bool {
        match self {
            Self::AllEphemeral => record.class() == HandleClass::Ephemeral,
            Self::Binding(value) => record_binding_id(record) == *value,
            Self::Grant(value) => record_grant_id(record) == *value,
            Self::OwnerEpoch(value) => record.owner_epoch == *value,
            Self::Membership(value) => record.source_membership_id() == *value,
            Self::WorkspaceView(value) => record.workspace_view_revision_id() == Some(*value),
            Self::BufferSnapshot(value) => record.buffer_snapshot_id() == Some(*value),
            Self::Residency(value) => record.residency() == Some(*value),
            Self::RetentionLease(value) => record.retention_lease_ref() == Some(value),
        }
    }
}

const fn record_binding_id(record: &StoredHandle) -> BindingId {
    match record.record() {
        SearchSourceHandleRecord::Ephemeral(record) => record.binding_id,
        SearchSourceHandleRecord::DurableSource(record) => record.binding_id,
    }
}

const fn record_grant_id(record: &StoredHandle) -> GrantId {
    match record.record() {
        SearchSourceHandleRecord::Ephemeral(record) => record.grant_id,
        SearchSourceHandleRecord::DurableSource(record) => record.grant_id,
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

#[cfg(test)]
mod tests;
