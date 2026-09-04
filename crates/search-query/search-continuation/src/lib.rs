//! Bounded opaque continuation lifecycle.
//!
//! Client tokens are random locators only. Plan, result, access, route, view,
//! issued-candidate and epoch-pin state remains server-owned. No vendor cursor,
//! raw score, point ID, query text, path, source bytes, or unsaved bytes are
//! serialized into a continuation handle.

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
    BindingId, Blake3Digest32, BoundedList, ContinuationHandle, ContinuationId,
    ContinuationRecord, HandleTokenDigest, LifecycleRecordStatus, MAX_LIST_ITEMS,
    MAX_SET_ITEMS, NonZeroRevision, OpaqueHandleToken, OpaqueId, OpaqueRef,
    PlanFingerprint, ResultFence, UtcTimestamp,
};

/// Default maximum continuation lifetime: fifteen minutes.
pub const DEFAULT_MAX_TTL_MILLIS: u64 = 15 * 60 * 1_000;

/// Closed continuation error surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ContinuationError {
    /// A configured finite limit is invalid.
    InvalidLimits,
    /// Creation time, expiration, or declared TTL is invalid.
    InvalidTtl,
    /// A credential is missing, foreign, or mismatched.
    NotAuthorized,
    /// The immutable snapshot or dependency fence changed or expired.
    SnapshotExpired,
    /// Current grant or security state denies disclosure.
    AccessRevoked,
    /// A purge barrier invalidated the continuation.
    Purged,
    /// A finite store, binding, window, issued-set, or batch quota is full.
    ResourceExhausted,
    /// Continuation ID or token digest collides with another record.
    IdentityCollision,
    /// Candidate fingerprints are duplicated.
    DuplicateCandidate,
    /// An ephemeral continuation has no candidate window.
    EmptyCandidateWindow,
    /// Record and payload durability variants disagree.
    DurabilityMismatch,
    /// A process-local epoch pin is absent or stale.
    EpochPinUnavailable,
    /// An emission permit no longer matches the record revision.
    StalePermit,
    /// A lifecycle transition is invalid.
    InvalidTransition,
    /// An invalidation generation regressed.
    OperationConflict,
    /// A revision counter cannot advance.
    RevisionExhausted,
}

impl ContinuationError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "CONTINUATION_INVALID_LIMITS",
            Self::InvalidTtl => "CONTINUATION_INVALID_TTL",
            Self::NotAuthorized => "CONTINUATION_NOT_AUTHORIZED",
            Self::SnapshotExpired => "SNAPSHOT_EXPIRED",
            Self::AccessRevoked => "ACCESS_REVOKED",
            Self::Purged => "PURGED",
            Self::ResourceExhausted => "RESOURCE_EXHAUSTED",
            Self::IdentityCollision => "CONTINUATION_IDENTITY_COLLISION",
            Self::DuplicateCandidate => "CONTINUATION_DUPLICATE_CANDIDATE",
            Self::EmptyCandidateWindow => "CONTINUATION_EMPTY_WINDOW",
            Self::DurabilityMismatch => "CONTINUATION_DURABILITY_MISMATCH",
            Self::EpochPinUnavailable => "CONTINUATION_EPOCH_PIN_UNAVAILABLE",
            Self::StalePermit => "CONTINUATION_STALE_PERMIT",
            Self::InvalidTransition => "CONTINUATION_INVALID_TRANSITION",
            Self::OperationConflict => "CONTINUATION_OPERATION_CONFLICT",
            Self::RevisionExhausted => "CONTINUATION_REVISION_EXHAUSTED",
        }
    }
}

impl fmt::Display for ContinuationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ContinuationError {}

/// Finite continuation quotas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuationLimits {
    /// Maximum retained records, including terminal records awaiting compaction.
    pub max_records: usize,
    /// Maximum active ephemeral records for one binding.
    pub max_ephemeral_per_binding: usize,
    /// Maximum active durable records for one binding.
    pub max_durable_per_binding: usize,
    /// Maximum candidates retained by one ephemeral record.
    pub max_candidate_window: usize,
    /// Maximum issued fingerprints retained by one record.
    pub max_issued_candidates: usize,
    /// Maximum candidates returned by one expansion.
    pub max_expansion_items: usize,
    /// Maximum records changed by one lifecycle operation.
    pub max_lifecycle_batch: usize,
    /// Maximum declared TTL in milliseconds.
    pub max_ttl_millis: u64,
}

impl ContinuationLimits {
    /// Conservative baseline.
    pub const BASELINE: Self = Self {
        max_records: 1_024,
        max_ephemeral_per_binding: 128,
        max_durable_per_binding: 32,
        max_candidate_window: MAX_LIST_ITEMS,
        max_issued_candidates: MAX_SET_ITEMS,
        max_expansion_items: 256,
        max_lifecycle_batch: 256,
        max_ttl_millis: DEFAULT_MAX_TTL_MILLIS,
    };

    /// Validates every finite dimension.
    pub fn validate(self) -> Result<Self, ContinuationError> {
        let valid = self.max_records > 0
            && self.max_records <= MAX_LIST_ITEMS
            && self.max_ephemeral_per_binding > 0
            && self.max_ephemeral_per_binding <= self.max_records
            && self.max_durable_per_binding > 0
            && self.max_durable_per_binding <= self.max_records
            && self.max_candidate_window > 0
            && self.max_candidate_window <= MAX_LIST_ITEMS
            && self.max_issued_candidates > 0
            && self.max_issued_candidates <= MAX_SET_ITEMS
            && self.max_expansion_items > 0
            && self.max_expansion_items <= self.max_candidate_window
            && self.max_lifecycle_batch > 0
            && self.max_lifecycle_batch <= MAX_LIST_ITEMS
            && self.max_ttl_millis > 0;
        if valid {
            Ok(self)
        } else {
            Err(ContinuationError::InvalidLimits)
        }
    }
}

/// Plaintext token delivery material supplied by a CSPRNG owner.
///
/// The store retains only the digest. The plaintext token moves into the
/// returned [`ContinuationHandle`].
pub struct ContinuationTokenMaterial {
    /// Random continuation identity.
    pub continuation_id: ContinuationId,
    /// Opaque bearer locator delivered to the authenticated client.
    pub opaque_token: OpaqueHandleToken,
    /// Dedicated-domain digest of the plaintext token.
    pub token_digest: HandleTokenDigest,
}

impl fmt::Debug for ContinuationTokenMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContinuationTokenMaterial")
            .field("continuation_id", &self.continuation_id)
            .field("opaque_token", &"<redacted>")
            .field("token_digest", &self.token_digest)
            .finish()
    }
}

/// Content-free credential produced after hashing a presented handle token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationCredential {
    /// Client-visible continuation identity.
    pub continuation_id: ContinuationId,
    /// Expiration copied from the handle.
    pub expires_at: UtcTimestamp,
    /// Dedicated-domain digest of the presented token.
    pub token_digest: HandleTokenDigest,
    /// Current authenticated binding.
    pub binding_id: BindingId,
}

impl ContinuationCredential {
    /// Builds a credential from a handle and externally computed token digest.
    #[must_use]
    pub fn from_handle(
        handle: &ContinuationHandle,
        token_digest: HandleTokenDigest,
        binding_id: BindingId,
    ) -> Self {
        Self {
            continuation_id: handle.continuation_id,
            expires_at: handle.expires_at.clone(),
            token_digest,
            binding_id,
        }
    }
}

/// One server-owned candidate in an ephemeral window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateWindowItem {
    /// Opaque candidate reference, never a raw backend cursor.
    pub candidate_ref: OpaqueRef,
    /// Stable identity used for issued-candidate suppression.
    pub fingerprint: Blake3Digest32,
}

/// Server-owned continuation payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuationPayload {
    /// Restart-invalid in-memory candidate window with a process-local pin.
    Ephemeral {
        /// Process boot identity.
        boot_id: OpaqueId,
        /// Bounded deterministic candidate window.
        candidates: BoundedList<CandidateWindowItem, MAX_LIST_ITEMS>,
    },
    /// Durable immutable-data replan checkpoint; no process-local pin or bytes.
    DurableReplan,
}

/// Complete creation request.
pub struct CreateContinuationRequest {
    /// Shared server-owned contract record.
    pub record: ContinuationRecord,
    /// Payload whose durability must match the record variant.
    pub payload: ContinuationPayload,
    /// Random client token and its dedicated digest.
    pub token: ContinuationTokenMaterial,
    /// Explicit finite TTL for policy validation.
    pub ttl_millis: u64,
    /// Candidate fingerprints already emitted before durable restore/creation.
    pub issued_fingerprints: BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
}

/// Successful creation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedContinuation {
    /// Client-visible opaque handle.
    pub handle: ContinuationHandle,
    /// Server-owned record containing no plaintext token.
    pub record: ContinuationRecord,
}

/// Current authority and dependency observations used during resume.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveContinuationState {
    /// Current authenticated binding.
    pub binding_id: BindingId,
    /// Current exact plan fingerprint.
    pub plan_fingerprint: PlanFingerprint,
    /// Current exact result fence.
    pub result_fence: ResultFence,
    /// Grant remains active.
    pub grant_active: bool,
    /// Security state permits disclosure.
    pub security_permits: bool,
    /// No purge barrier covers the continuation.
    pub purge_clear: bool,
    /// Source-owner generation remains current.
    pub owner_generation_current: bool,
    /// Saved/workspace view remains current.
    pub view_current: bool,
    /// Collection route and visible epoch remain current.
    pub route_current: bool,
    /// Projection/profile identity remains current.
    pub profile_current: bool,
    /// Durable job remains authorized and present.
    pub durable_job_active: bool,
    /// Original process-local pin is present and exact.
    pub epoch_pin_valid: bool,
}

/// Permit binding an expansion to one exact process-local record revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationPermit {
    continuation_id: ContinuationId,
    record_revision: u64,
    binding_id: BindingId,
    plan_fingerprint: PlanFingerprint,
    result_fence: ResultFence,
}

impl ContinuationPermit {
    /// Continuation identity.
    #[must_use]
    pub const fn continuation_id(&self) -> ContinuationId {
        self.continuation_id
    }

    /// Exact record revision observed during validation.
    #[must_use]
    pub const fn record_revision(&self) -> u64 {
        self.record_revision
    }
}

/// External cleanup or renewal effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuationEffect {
    /// Renew the original pin without changing route or epoch.
    RenewEpochPin {
        /// Exact pin reference.
        epoch_pin_ref: OpaqueRef,
        /// Pin lifetime may not exceed this time.
        not_after: UtcTimestamp,
    },
    /// Release the exact process-local pin.
    ReleaseEpochPin {
        /// Exact pin reference.
        epoch_pin_ref: OpaqueRef,
    },
    /// Delete one durable checkpoint after terminal transition.
    DeleteDurableCheckpoint {
        /// Durable job reference.
        durable_job_ref: OpaqueRef,
        /// Exact replan checkpoint reference.
        replan_checkpoint_ref: OpaqueRef,
    },
}

/// Validated continuation expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResumePlan {
    /// Next bounded in-memory window. Nothing is marked issued yet.
    EphemeralWindow {
        /// Permit required when emission succeeds.
        permit: ContinuationPermit,
        /// Next unissued candidates.
        candidates: BoundedList<CandidateWindowItem, MAX_LIST_ITEMS>,
        /// Pin renewal bounded by original expiration.
        pin_effect: ContinuationEffect,
    },
    /// Replan from durable immutable state and suppress earlier emissions.
    DurableReplan {
        /// Permit required when emission succeeds.
        permit: ContinuationPermit,
        /// Durable job reference.
        durable_job_ref: OpaqueRef,
        /// Exact replan checkpoint reference.
        replan_checkpoint_ref: OpaqueRef,
        /// Fingerprints already emitted to this continuation.
        issued_fingerprints: BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
    },
    /// No unissued ephemeral candidate remains.
    Exhausted {
        /// Permit proving the terminal observation.
        permit: ContinuationPermit,
    },
}

/// Successful issued-candidate update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmissionReceipt {
    /// Updated continuation.
    pub continuation_id: ContinuationId,
    /// Number of newly issued candidates.
    pub emitted_count: usize,
    /// Total issued candidates after commit.
    pub issued_total: usize,
    /// Whether an ephemeral window is now exhausted.
    pub completed: bool,
    /// Cleanup required after exhaustion.
    pub cleanup_effect: Option<ContinuationEffect>,
}

/// Monotonic invalidation reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvalidationReason {
    /// Grant or binding was revoked.
    AccessRevoked,
    /// A purge barrier covers the record.
    Purged,
    /// Source-owner generation changed.
    OwnerGenerationChanged,
    /// Saved or workspace view changed.
    ViewChanged,
    /// Route or visible epoch changed.
    RouteChanged,
    /// Required profile changed.
    ProfileChanged,
    /// Durable job changed or disappeared.
    DurableJobChanged,
    /// Process restart invalidated an ephemeral record.
    Restart,
    /// Caller completed the continuation.
    Completed,
}

/// Bounded invalidation selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidationScope {
    /// Every active record.
    All,
    /// Every active ephemeral record.
    Ephemeral,
    /// One authenticated binding.
    Binding(BindingId),
    /// One plan fingerprint.
    Plan(PlanFingerprint),
    /// One exact result fence.
    ResultFence(ResultFence),
    /// One continuation identity.
    Continuation(ContinuationId),
    /// One durable job reference.
    DurableJob(OpaqueRef),
    /// One process-local pin reference.
    EpochPin(OpaqueRef),
}

/// Result of invalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidationReceipt {
    /// Monotonic invalidation generation.
    pub generation: NonZeroRevision,
    /// Records changed in identity order.
    pub invalidated: BoundedList<ContinuationId, MAX_LIST_ITEMS>,
    /// Required cleanup effects.
    pub effects: BoundedList<ContinuationEffect, MAX_LIST_ITEMS>,
}

/// Result of one bounded expiry sweep.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpiryReceipt {
    /// Records expired in deterministic order.
    pub expired: BoundedList<ContinuationId, MAX_LIST_ITEMS>,
    /// Required cleanup effects.
    pub effects: BoundedList<ContinuationEffect, MAX_LIST_ITEMS>,
    /// Whether another bounded sweep is required.
    pub more_remaining: bool,
}

/// Result of applying restrictive live limits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigApplyReceipt {
    /// Records expired to satisfy the new limits.
    pub expired: BoundedList<ContinuationId, MAX_LIST_ITEMS>,
    /// Required cleanup effects.
    pub effects: BoundedList<ContinuationEffect, MAX_LIST_ITEMS>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredContinuation {
    record: ContinuationRecord,
    payload: ContinuationPayload,
    issued: BTreeSet<Blake3Digest32>,
    revision: u64,
    terminal_reason: Option<InvalidationReason>,
    last_invalidation_generation: Option<NonZeroRevision>,
}

impl StoredContinuation {
    fn id(&self) -> ContinuationId {
        match &self.record {
            ContinuationRecord::EphemeralWindow(value) => value.continuation_id,
            ContinuationRecord::DurableReplanCheckpoint(value) => value.continuation_id,
        }
    }

    fn token_digest(&self) -> HandleTokenDigest {
        match &self.record {
            ContinuationRecord::EphemeralWindow(value) => value.token_digest,
            ContinuationRecord::DurableReplanCheckpoint(value) => value.token_digest,
        }
    }

    fn binding_id(&self) -> BindingId {
        match &self.record {
            ContinuationRecord::EphemeralWindow(value) => value.binding_id,
            ContinuationRecord::DurableReplanCheckpoint(value) => value.binding_id,
        }
    }

    fn plan_fingerprint(&self) -> PlanFingerprint {
        match &self.record {
            ContinuationRecord::EphemeralWindow(value) => value.plan_fingerprint,
            ContinuationRecord::DurableReplanCheckpoint(value) => value.plan_fingerprint,
        }
    }

    fn result_fence(&self) -> &ResultFence {
        match &self.record {
            ContinuationRecord::EphemeralWindow(value) => &value.result_fence,
            ContinuationRecord::DurableReplanCheckpoint(value) => &value.result_fence,
        }
    }

    fn created_at(&self) -> &UtcTimestamp {
        match &self.record {
            ContinuationRecord::EphemeralWindow(value) => &value.created_at,
            ContinuationRecord::DurableReplanCheckpoint(value) => &value.created_at,
        }
    }

    fn expires_at(&self) -> &UtcTimestamp {
        match &self.record {
            ContinuationRecord::EphemeralWindow(value) => &value.expires_at,
            ContinuationRecord::DurableReplanCheckpoint(value) => &value.expires_at,
        }
    }

    fn status(&self) -> LifecycleRecordStatus {
        match &self.record {
            ContinuationRecord::EphemeralWindow(value) => value.status,
            ContinuationRecord::DurableReplanCheckpoint(value) => value.status,
        }
    }

    fn set_status(&mut self, status: LifecycleRecordStatus) {
        match &mut self.record {
            ContinuationRecord::EphemeralWindow(value) => value.status = status,
            ContinuationRecord::DurableReplanCheckpoint(value) => value.status = status,
        }
    }

    fn is_ephemeral(&self) -> bool {
        matches!(&self.payload, ContinuationPayload::Ephemeral { .. })
    }

    fn is_active(&self) -> bool {
        self.status() == LifecycleRecordStatus::Active
    }

    fn matches_scope(&self, scope: &InvalidationScope) -> bool {
        match scope {
            InvalidationScope::All => true,
            InvalidationScope::Ephemeral => self.is_ephemeral(),
            InvalidationScope::Binding(value) => self.binding_id() == *value,
            InvalidationScope::Plan(value) => self.plan_fingerprint() == *value,
            InvalidationScope::ResultFence(value) => self.result_fence() == value,
            InvalidationScope::Continuation(value) => self.id() == *value,
            InvalidationScope::DurableJob(value) => matches!(
                &self.record,
                ContinuationRecord::DurableReplanCheckpoint(record)
                    if &record.durable_job_ref == value
            ),
            InvalidationScope::EpochPin(value) => matches!(
                &self.record,
                ContinuationRecord::EphemeralWindow(record)
                    if &record.epoch_pin_ref == value
            ),
        }
    }

    fn cleanup_effect(&self) -> ContinuationEffect {
        match &self.record {
            ContinuationRecord::EphemeralWindow(record) => {
                ContinuationEffect::ReleaseEpochPin {
                    epoch_pin_ref: record.epoch_pin_ref.clone(),
                }
            }
            ContinuationRecord::DurableReplanCheckpoint(record) => {
                ContinuationEffect::DeleteDurableCheckpoint {
                    durable_job_ref: record.durable_job_ref.clone(),
                    replan_checkpoint_ref: record.replan_checkpoint_ref.clone(),
                }
            }
        }
    }
}

/// Finite server-owned continuation catalog.
#[derive(Debug)]
pub struct ContinuationStore {
    limits: ContinuationLimits,
    records: BTreeMap<ContinuationId, StoredContinuation>,
    token_index: BTreeMap<HandleTokenDigest, ContinuationId>,
}

impl ContinuationStore {
    /// Creates an empty bounded store.
    pub fn new(limits: ContinuationLimits) -> Result<Self, ContinuationError> {
        Ok(Self {
            limits: limits.validate()?,
            records: BTreeMap::new(),
            token_index: BTreeMap::new(),
        })
    }

    /// Current limits.
    #[must_use]
    pub const fn limits(&self) -> ContinuationLimits {
        self.limits
    }

    /// Number of retained records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether no record is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Creates an ephemeral or durable continuation after complete validation.
    pub fn create(
        &mut self,
        request: CreateContinuationRequest,
    ) -> Result<CreatedContinuation, ContinuationError> {
        let id = record_id(&request.record);
        let digest = record_token_digest(&request.record);
        let binding = record_binding(&request.record);
        let created_at = record_created_at(&request.record);
        let expires_at = record_expires_at(&request.record);
        if id != request.token.continuation_id || digest != request.token.token_digest {
            return Err(ContinuationError::IdentityCollision);
        }
        self.validate_lifetime(created_at, expires_at, request.ttl_millis)?;
        if record_status(&request.record) != LifecycleRecordStatus::Active {
            return Err(ContinuationError::InvalidTransition);
        }
        self.validate_payload(&request.record, &request.payload)?;
        self.validate_capacity(
            binding,
            matches!(&request.payload, ContinuationPayload::Ephemeral { .. }),
        )?;
        self.validate_identity(id, digest)?;
        if request.issued_fingerprints.len() > self.limits.max_issued_candidates {
            return Err(ContinuationError::ResourceExhausted);
        }
        let issued = unique_fingerprints(request.issued_fingerprints.iter().copied())?;
        let ContinuationTokenMaterial {
            continuation_id,
            opaque_token,
            token_digest: _,
        } = request.token;
        let handle = ContinuationHandle {
            continuation_id,
            expires_at: expires_at.clone(),
            opaque_token,
        };
        let stored = StoredContinuation {
            record: request.record.clone(),
            payload: request.payload,
            issued,
            revision: 1,
            terminal_reason: None,
            last_invalidation_generation: None,
        };
        self.token_index.insert(digest, id);
        self.records.insert(id, stored);
        Ok(CreatedContinuation {
            handle,
            record: request.record,
        })
    }

    /// Resolves an authenticated credential without disclosing foreign records.
    pub fn resolve(
        &self,
        credential: &ContinuationCredential,
    ) -> Result<&ContinuationRecord, ContinuationError> {
        let stored = self.authorized(credential)?;
        match stored.status() {
            LifecycleRecordStatus::Active => Ok(&stored.record),
            LifecycleRecordStatus::Expired => Err(ContinuationError::SnapshotExpired),
            LifecycleRecordStatus::Revoked => Err(terminal_error(stored.terminal_reason)),
        }
    }

    /// Revalidates and produces the next bounded expansion plan.
    ///
    /// Returned candidates are not marked issued until [`Self::commit_emission`]
    /// is called after successful emission.
    pub fn resume(
        &self,
        credential: &ContinuationCredential,
        live: &LiveContinuationState,
        now: &UtcTimestamp,
        max_items: usize,
    ) -> Result<ResumePlan, ContinuationError> {
        if max_items == 0 || max_items > self.limits.max_expansion_items {
            return Err(ContinuationError::InvalidLimits);
        }
        let stored = self.authorized(credential)?;
        self.revalidate(stored, live, now)?;
        let permit = ContinuationPermit {
            continuation_id: stored.id(),
            record_revision: stored.revision,
            binding_id: stored.binding_id(),
            plan_fingerprint: stored.plan_fingerprint(),
            result_fence: stored.result_fence().clone(),
        };
        match (&stored.record, &stored.payload) {
            (
                ContinuationRecord::EphemeralWindow(record),
                ContinuationPayload::Ephemeral { candidates, .. },
            ) => {
                let selected = candidates
                    .iter()
                    .filter(|item| !stored.issued.contains(&item.fingerprint))
                    .take(max_items)
                    .cloned()
                    .collect::<Vec<_>>();
                if selected.is_empty() {
                    return Ok(ResumePlan::Exhausted { permit });
                }
                Ok(ResumePlan::EphemeralWindow {
                    permit,
                    candidates: bounded(selected)?,
                    pin_effect: ContinuationEffect::RenewEpochPin {
                        epoch_pin_ref: record.epoch_pin_ref.clone(),
                        not_after: record.expires_at.clone(),
                    },
                })
            }
            (
                ContinuationRecord::DurableReplanCheckpoint(record),
                ContinuationPayload::DurableReplan,
            ) => Ok(ResumePlan::DurableReplan {
                permit,
                durable_job_ref: record.durable_job_ref.clone(),
                replan_checkpoint_ref: record.replan_checkpoint_ref.clone(),
                issued_fingerprints: bounded(stored.issued.iter().copied().collect())?,
            }),
            _ => Err(ContinuationError::DurabilityMismatch),
        }
    }

    /// Marks fingerprints issued only after successful client emission.
    pub fn commit_emission(
        &mut self,
        permit: &ContinuationPermit,
        emitted: BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
    ) -> Result<EmissionReceipt, ContinuationError> {
        if emitted.is_empty() || emitted.len() > self.limits.max_expansion_items {
            return Err(ContinuationError::InvalidLimits);
        }
        let proposed = unique_fingerprints(emitted.iter().copied())?;
        let stored = self
            .records
            .get_mut(&permit.continuation_id)
            .ok_or(ContinuationError::StalePermit)?;
        validate_permit(stored, permit)?;
        if proposed.iter().any(|value| stored.issued.contains(value)) {
            return Err(ContinuationError::DuplicateCandidate);
        }
        if let ContinuationPayload::Ephemeral { candidates, .. } = &stored.payload {
            let available = candidates
                .iter()
                .map(|item| item.fingerprint)
                .collect::<BTreeSet<_>>();
            if !proposed.is_subset(&available) {
                return Err(ContinuationError::StalePermit);
            }
        }
        let next_total = stored
            .issued
            .len()
            .checked_add(proposed.len())
            .ok_or(ContinuationError::ResourceExhausted)?;
        if next_total > self.limits.max_issued_candidates {
            return Err(ContinuationError::ResourceExhausted);
        }
        stored.issued.extend(proposed);
        stored.revision = stored
            .revision
            .checked_add(1)
            .ok_or(ContinuationError::RevisionExhausted)?;
        let completed = match &stored.payload {
            ContinuationPayload::Ephemeral { candidates, .. } => candidates
                .iter()
                .all(|item| stored.issued.contains(&item.fingerprint)),
            ContinuationPayload::DurableReplan => false,
        };
        let cleanup_effect = if completed {
            stored.set_status(LifecycleRecordStatus::Revoked);
            stored.terminal_reason = Some(InvalidationReason::Completed);
            Some(stored.cleanup_effect())
        } else {
            None
        };
        Ok(EmissionReceipt {
            continuation_id: stored.id(),
            emitted_count: emitted.len(),
            issued_total: stored.issued.len(),
            completed,
            cleanup_effect,
        })
    }

    /// Explicitly completes a continuation and returns its cleanup effect.
    pub fn complete(
        &mut self,
        permit: &ContinuationPermit,
    ) -> Result<ContinuationEffect, ContinuationError> {
        let stored = self
            .records
            .get_mut(&permit.continuation_id)
            .ok_or(ContinuationError::StalePermit)?;
        validate_permit(stored, permit)?;
        stored.set_status(LifecycleRecordStatus::Revoked);
        stored.terminal_reason = Some(InvalidationReason::Completed);
        stored.revision = stored
            .revision
            .checked_add(1)
            .ok_or(ContinuationError::RevisionExhausted)?;
        Ok(stored.cleanup_effect())
    }

    /// Applies one bounded monotonic invalidation.
    pub fn invalidate(
        &mut self,
        scope: &InvalidationScope,
        reason: InvalidationReason,
        generation: NonZeroRevision,
    ) -> Result<InvalidationReceipt, ContinuationError> {
        let ids = self
            .records
            .iter()
            .filter(|(_, value)| value.is_active() && value.matches_scope(scope))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        self.invalidate_ids(ids, reason, generation)
    }

    /// Invalidates process-local windows from earlier boot identities.
    pub fn invalidate_restart(
        &mut self,
        current_boot_id: &OpaqueId,
        generation: NonZeroRevision,
    ) -> Result<InvalidationReceipt, ContinuationError> {
        let ids = self
            .records
            .iter()
            .filter_map(|(id, value)| match &value.payload {
                ContinuationPayload::Ephemeral { boot_id, .. }
                    if value.is_active() && boot_id != current_boot_id =>
                {
                    Some(*id)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        self.invalidate_ids(ids, InvalidationReason::Restart, generation)
    }

    /// Expires one deterministic bounded batch.
    pub fn expire(
        &mut self,
        now: &UtcTimestamp,
    ) -> Result<ExpiryReceipt, ContinuationError> {
        let mut pending = self
            .records
            .iter()
            .filter(|(_, value)| value.is_active() && now >= value.expires_at())
            .map(|(id, value)| (value.expires_at().clone(), *id))
            .collect::<Vec<_>>();
        pending.sort();
        let more_remaining = pending.len() > self.limits.max_lifecycle_batch;
        pending.truncate(self.limits.max_lifecycle_batch);
        let mut ids = Vec::new();
        let mut effects = Vec::new();
        for (_, id) in pending {
            let value = self.records.get_mut(&id).expect("collected record");
            value.set_status(LifecycleRecordStatus::Expired);
            value.revision = value
                .revision
                .checked_add(1)
                .ok_or(ContinuationError::RevisionExhausted)?;
            ids.push(id);
            effects.push(value.cleanup_effect());
        }
        Ok(ExpiryReceipt {
            expired: bounded(ids)?,
            effects: bounded(effects)?,
            more_remaining,
        })
    }

    /// Applies restrictive live limits and expires incompatible active records.
    pub fn apply_live_limits(
        &mut self,
        limits: ContinuationLimits,
    ) -> Result<ConfigApplyReceipt, ContinuationError> {
        let limits = limits.validate()?;
        let mut ordered = self
            .records
            .iter()
            .filter(|(_, value)| value.is_active())
            .map(|(id, value)| (value.created_at().clone(), *id))
            .collect::<Vec<_>>();
        ordered.sort();
        let mut expire_ids = BTreeSet::new();
        for (_, id) in &ordered {
            let value = self.records.get(id).expect("collected record");
            let oversized_window = matches!(
                &value.payload,
                ContinuationPayload::Ephemeral { candidates, .. }
                    if candidates.len() > limits.max_candidate_window
            );
            if oversized_window || value.issued.len() > limits.max_issued_candidates {
                expire_ids.insert(*id);
            }
        }
        let mut survivors = ordered
            .iter()
            .map(|(_, id)| *id)
            .filter(|id| !expire_ids.contains(id))
            .collect::<Vec<_>>();
        while survivors.len() > limits.max_records {
            expire_ids.insert(survivors.remove(0));
        }
        let bindings = survivors
            .iter()
            .filter_map(|id| self.records.get(id).map(StoredContinuation::binding_id))
            .collect::<BTreeSet<_>>();
        for binding in bindings {
            for ephemeral in [true, false] {
                let maximum = if ephemeral {
                    limits.max_ephemeral_per_binding
                } else {
                    limits.max_durable_per_binding
                };
                let matching = survivors
                    .iter()
                    .copied()
                    .filter(|id| {
                        self.records.get(id).is_some_and(|value| {
                            value.binding_id() == binding
                                && value.is_ephemeral() == ephemeral
                                && !expire_ids.contains(id)
                        })
                    })
                    .collect::<Vec<_>>();
                let excess = matching.len().saturating_sub(maximum);
                expire_ids.extend(matching.into_iter().take(excess));
            }
        }
        if expire_ids.len() > self.limits.max_lifecycle_batch {
            return Err(ContinuationError::ResourceExhausted);
        }
        let mut ids = Vec::new();
        let mut effects = Vec::new();
        for id in expire_ids {
            let value = self.records.get_mut(&id).expect("collected record");
            value.set_status(LifecycleRecordStatus::Expired);
            value.revision = value
                .revision
                .checked_add(1)
                .ok_or(ContinuationError::RevisionExhausted)?;
            ids.push(id);
            effects.push(value.cleanup_effect());
        }
        self.limits = limits;
        Ok(ConfigApplyReceipt {
            expired: bounded(ids)?,
            effects: bounded(effects)?,
        })
    }

    /// Removes terminal records after their cleanup effects were executed.
    pub fn compact_terminal(
        &mut self,
        max_items: usize,
    ) -> Result<BoundedList<ContinuationId, MAX_LIST_ITEMS>, ContinuationError> {
        if max_items == 0 || max_items > self.limits.max_lifecycle_batch {
            return Err(ContinuationError::InvalidLimits);
        }
        let ids = self
            .records
            .iter()
            .filter(|(_, value)| !value.is_active())
            .map(|(id, _)| *id)
            .take(max_items)
            .collect::<Vec<_>>();
        for id in &ids {
            if let Some(value) = self.records.remove(id) {
                self.token_index.remove(&value.token_digest());
            }
        }
        bounded(ids)
    }

    fn authorized(
        &self,
        credential: &ContinuationCredential,
    ) -> Result<&StoredContinuation, ContinuationError> {
        let value = self
            .records
            .get(&credential.continuation_id)
            .ok_or(ContinuationError::NotAuthorized)?;
        let indexed = self
            .token_index
            .get(&credential.token_digest)
            .copied()
            .ok_or(ContinuationError::NotAuthorized)?;
        if indexed != credential.continuation_id
            || !constant_time_equal(&value.token_digest(), &credential.token_digest)
            || value.binding_id() != credential.binding_id
            || value.expires_at() != &credential.expires_at
        {
            return Err(ContinuationError::NotAuthorized);
        }
        Ok(value)
    }

    fn revalidate(
        &self,
        value: &StoredContinuation,
        live: &LiveContinuationState,
        now: &UtcTimestamp,
    ) -> Result<(), ContinuationError> {
        match value.status() {
            LifecycleRecordStatus::Expired => return Err(ContinuationError::SnapshotExpired),
            LifecycleRecordStatus::Revoked => {
                return Err(terminal_error(value.terminal_reason));
            }
            LifecycleRecordStatus::Active => {}
        }
        if now >= value.expires_at() {
            return Err(ContinuationError::SnapshotExpired);
        }
        if value.binding_id() != live.binding_id {
            return Err(ContinuationError::NotAuthorized);
        }
        if !live.grant_active || !live.security_permits {
            return Err(ContinuationError::AccessRevoked);
        }
        if !live.purge_clear {
            return Err(ContinuationError::Purged);
        }
        if value.plan_fingerprint() != live.plan_fingerprint
            || value.result_fence() != &live.result_fence
            || !live.owner_generation_current
            || !live.view_current
            || !live.route_current
            || !live.profile_current
        {
            return Err(ContinuationError::SnapshotExpired);
        }
        match &value.payload {
            ContinuationPayload::Ephemeral { .. } if !live.epoch_pin_valid => {
                Err(ContinuationError::EpochPinUnavailable)
            }
            ContinuationPayload::DurableReplan if !live.durable_job_active => {
                Err(ContinuationError::SnapshotExpired)
            }
            _ => Ok(()),
        }
    }

    fn validate_lifetime(
        &self,
        created_at: &UtcTimestamp,
        expires_at: &UtcTimestamp,
        ttl_millis: u64,
    ) -> Result<(), ContinuationError> {
        if ttl_millis == 0
            || ttl_millis > self.limits.max_ttl_millis
            || created_at >= expires_at
        {
            Err(ContinuationError::InvalidTtl)
        } else {
            Ok(())
        }
    }

    fn validate_payload(
        &self,
        record: &ContinuationRecord,
        payload: &ContinuationPayload,
    ) -> Result<(), ContinuationError> {
        match (record, payload) {
            (
                ContinuationRecord::EphemeralWindow(_),
                ContinuationPayload::Ephemeral { candidates, .. },
            ) => {
                if candidates.is_empty() {
                    return Err(ContinuationError::EmptyCandidateWindow);
                }
                if candidates.len() > self.limits.max_candidate_window {
                    return Err(ContinuationError::ResourceExhausted);
                }
                unique_fingerprints(candidates.iter().map(|item| item.fingerprint))?;
                Ok(())
            }
            (
                ContinuationRecord::DurableReplanCheckpoint(_),
                ContinuationPayload::DurableReplan,
            ) => Ok(()),
            _ => Err(ContinuationError::DurabilityMismatch),
        }
    }

    fn validate_capacity(
        &self,
        binding: BindingId,
        ephemeral: bool,
    ) -> Result<(), ContinuationError> {
        if self.records.len() >= self.limits.max_records {
            return Err(ContinuationError::ResourceExhausted);
        }
        let count = self
            .records
            .values()
            .filter(|value| {
                value.is_active()
                    && value.binding_id() == binding
                    && value.is_ephemeral() == ephemeral
            })
            .count();
        let maximum = if ephemeral {
            self.limits.max_ephemeral_per_binding
        } else {
            self.limits.max_durable_per_binding
        };
        if count >= maximum {
            Err(ContinuationError::ResourceExhausted)
        } else {
            Ok(())
        }
    }

    fn validate_identity(
        &self,
        id: ContinuationId,
        digest: HandleTokenDigest,
    ) -> Result<(), ContinuationError> {
        if self.records.contains_key(&id) || self.token_index.contains_key(&digest) {
            Err(ContinuationError::IdentityCollision)
        } else {
            Ok(())
        }
    }

    fn invalidate_ids(
        &mut self,
        ids: Vec<ContinuationId>,
        reason: InvalidationReason,
        generation: NonZeroRevision,
    ) -> Result<InvalidationReceipt, ContinuationError> {
        if ids.len() > self.limits.max_lifecycle_batch {
            return Err(ContinuationError::ResourceExhausted);
        }
        for id in &ids {
            let value = self.records.get(id).ok_or(ContinuationError::NotAuthorized)?;
            if value
                .last_invalidation_generation
                .is_some_and(|previous| generation < previous)
            {
                return Err(ContinuationError::OperationConflict);
            }
        }
        let mut invalidated = Vec::new();
        let mut effects = Vec::new();
        for id in ids {
            let value = self.records.get_mut(&id).ok_or(ContinuationError::NotAuthorized)?;
            if !value.is_active() || value.last_invalidation_generation == Some(generation) {
                continue;
            }
            value.set_status(LifecycleRecordStatus::Revoked);
            value.terminal_reason = Some(reason);
            value.last_invalidation_generation = Some(generation);
            value.revision = value
                .revision
                .checked_add(1)
                .ok_or(ContinuationError::RevisionExhausted)?;
            invalidated.push(id);
            effects.push(value.cleanup_effect());
        }
        Ok(InvalidationReceipt {
            generation,
            invalidated: bounded(invalidated)?,
            effects: bounded(effects)?,
        })
    }
}

fn record_id(record: &ContinuationRecord) -> ContinuationId {
    match record {
        ContinuationRecord::EphemeralWindow(value) => value.continuation_id,
        ContinuationRecord::DurableReplanCheckpoint(value) => value.continuation_id,
    }
}

fn record_token_digest(record: &ContinuationRecord) -> HandleTokenDigest {
    match record {
        ContinuationRecord::EphemeralWindow(value) => value.token_digest,
        ContinuationRecord::DurableReplanCheckpoint(value) => value.token_digest,
    }
}

fn record_binding(record: &ContinuationRecord) -> BindingId {
    match record {
        ContinuationRecord::EphemeralWindow(value) => value.binding_id,
        ContinuationRecord::DurableReplanCheckpoint(value) => value.binding_id,
    }
}

fn record_created_at(record: &ContinuationRecord) -> &UtcTimestamp {
    match record {
        ContinuationRecord::EphemeralWindow(value) => &value.created_at,
        ContinuationRecord::DurableReplanCheckpoint(value) => &value.created_at,
    }
}

fn record_expires_at(record: &ContinuationRecord) -> &UtcTimestamp {
    match record {
        ContinuationRecord::EphemeralWindow(value) => &value.expires_at,
        ContinuationRecord::DurableReplanCheckpoint(value) => &value.expires_at,
    }
}

fn record_status(record: &ContinuationRecord) -> LifecycleRecordStatus {
    match record {
        ContinuationRecord::EphemeralWindow(value) => value.status,
        ContinuationRecord::DurableReplanCheckpoint(value) => value.status,
    }
}

fn validate_permit(
    value: &StoredContinuation,
    permit: &ContinuationPermit,
) -> Result<(), ContinuationError> {
    if !value.is_active()
        || value.revision != permit.record_revision
        || value.binding_id() != permit.binding_id
        || value.plan_fingerprint() != permit.plan_fingerprint
        || value.result_fence() != &permit.result_fence
    {
        Err(ContinuationError::StalePermit)
    } else {
        Ok(())
    }
}

fn unique_fingerprints(
    values: impl IntoIterator<Item = Blake3Digest32>,
) -> Result<BTreeSet<Blake3Digest32>, ContinuationError> {
    let mut output = BTreeSet::new();
    for value in values {
        if !output.insert(value) {
            return Err(ContinuationError::DuplicateCandidate);
        }
    }
    Ok(output)
}

fn bounded<T>(values: Vec<T>) -> Result<BoundedList<T, MAX_LIST_ITEMS>, ContinuationError> {
    BoundedList::new(values).map_err(|_| ContinuationError::ResourceExhausted)
}

fn terminal_error(reason: Option<InvalidationReason>) -> ContinuationError {
    match reason {
        Some(InvalidationReason::AccessRevoked) => ContinuationError::AccessRevoked,
        Some(InvalidationReason::Purged) => ContinuationError::Purged,
        _ => ContinuationError::SnapshotExpired,
    }
}

fn constant_time_equal(left: &HandleTokenDigest, right: &HandleTokenDigest) -> bool {
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (*left ^ *right)
        })
        == 0
}
