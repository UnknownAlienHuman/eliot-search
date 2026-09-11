//! T27 publication composition: guarded publisher over the control floor.
//!
//! This module wires three ordered steps for every publication attempt:
//! floor persistence through [`cas`], intent persistence through
//! [`FileJournal`], then exact Qdrant compensation through [`QdrantCompensate`].
//!
//! Guard vocabulary is owned by `search-contracts` and only re-exported here
//! as [`PublicationGuards`]: seven explicit fields, constructed explicitly,
//! with [`FakeGuards::fresh`] as the single deterministic harness source.
//! [`LiveGuardRead`] carries one observed guard snapshot. Neither guard holder
//! offers implicit construction.
//!
//! Qdrant compensation mirrors the existing bridge surface one-to-one:
//! [`QdrantCompensate::upsert_exact`], [`QdrantCompensate::close_exact`] and
//! [`QdrantCompensate::readback_exact`] forward to
//! `search_qdrant_bridge::real::RealDataPlane` methods of the same names and
//! explicit-ID semantics. There is no broad-filter close or delete on this
//! path: every mutation names explicit point IDs. Physical reclaim does not
//! exist here either; retirement is the logical [`RetiredManifest`] record,
//! reclaimed elsewhere through exact IDs only.
//!
//! Epoch discipline: every reservation, including an aborted one, consumes
//! exactly the successor of [`Publisher::last_reserved`]; anything else is
//! [`PublisherError::EpochMismatch`]. At most one commit stays active between
//! [`Publisher::propose`] and its control-commit observation, so crash
//! recovery through [`RecoveryDecision::decide`] always yields exactly one
//! head. Abandonment requires a complete [`MembershipFence`], otherwise
//! [`PublisherError::AbandonFenceMissing`]. Invalidation-only finalization
//! arrives only through [`CommitKind::invalidation_only`]; no boolean flag
//! selects the commit shape.

#![forbid(unsafe_code)]

pub use search_contracts::PublicationGuards;
use search_contracts::{Blake3Digest32, Epoch, OwnerEpoch};
pub use search_control_redb::publication_codec::{
    cas, FileJournal, JournalPersistOutcome, PublicationCodecError, PublicationFloor,
};

/// Maximum logically retired point IDs carried by one [`RetiredManifest`].
pub const MAX_RETIRED_IDS: usize = 1_024;

/// Guards observed at one control generation.
///
/// Compared field-for-field against the live snapshot inside
/// [`Publisher::propose`]; any concurrent rotation fails the proposal with
/// [`PublisherError::ControlConflict`] before any floor or journal write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGuardRead {
    guards: PublicationGuards,
    observed_generation: u64,
}

impl LiveGuardRead {
    /// Binds an explicitly observed guard snapshot to its control generation.
    #[must_use]
    pub const fn new(guards: PublicationGuards, observed_generation: u64) -> Self {
        Self {
            guards,
            observed_generation,
        }
    }

    /// Observed guard values.
    #[must_use]
    pub const fn guards(&self) -> PublicationGuards {
        self.guards
    }

    /// Control generation the observation was read at.
    #[must_use]
    pub const fn observed_generation(&self) -> u64 {
        self.observed_generation
    }
}

/// Deterministic harness guards with one explicit constructor.
///
/// There is deliberately no implicit construction: every harness builds
/// guards through [`FakeGuards::fresh`] and mutates the public fields
/// explicitly afterwards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeGuards {
    /// Explicit guard values; mutate fields directly for rotation scenarios.
    pub guards: PublicationGuards,
}

impl FakeGuards {
    /// Returns the single deterministic harness guard set.
    ///
    /// # Panics
    ///
    /// Never panics for the fixed constants below; the internal invariant is
    /// that the owner epoch stays non-zero.
    #[must_use]
    pub fn fresh() -> Self {
        let owner_epoch = OwnerEpoch::new(1).expect("fixed harness owner epoch is non-zero");
        Self {
            guards: PublicationGuards {
                owner_epoch,
                source_catalog_generation: 7,
                membership_generation: 5,
                access_generation: 3,
                shadow_generation: 2,
                purge_generation: 2,
                profile_digest: Blake3Digest32::from_bytes([0xA1; 32]),
            },
        }
    }

    /// Guard values held by this harness.
    #[must_use]
    pub const fn guards(&self) -> PublicationGuards {
        self.guards
    }
}

/// Closed publisher failure surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublisherError {
    /// A generation, guard or single-active-commit check failed.
    ControlConflict,
    /// A reserved epoch is not exactly one past the last reservation.
    EpochMismatch,
    /// An abandon request lacks a complete membership fence.
    AbandonFenceMissing,
    /// A foreign valid marker already occupies the journal slot.
    JournalConflict,
    /// Torn journal bytes; the slot quarantines.
    JournalCorrupt,
    /// Ambiguous outcome after a possible write; only a recovery read resolves.
    JournalOutcomeUnknown,
    /// A finite bound was exceeded.
    BudgetExceeded,
}

impl PublisherError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ControlConflict => "CONTROL_CONFLICT",
            Self::EpochMismatch => "EPOCH_MISMATCH",
            Self::AbandonFenceMissing => "ABANDON_FENCE_MISSING",
            Self::JournalConflict => "JOURNAL_CONFLICT",
            Self::JournalCorrupt => "JOURNAL_CORRUPT",
            Self::JournalOutcomeUnknown => "JOURNAL_OUTCOME_UNKNOWN",
            Self::BudgetExceeded => "PUBLISHER_BUDGET_EXCEEDED",
        }
    }
}

impl core::fmt::Display for PublisherError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PublisherError {}

impl From<PublicationCodecError> for PublisherError {
    fn from(error: PublicationCodecError) -> Self {
        match error {
            PublicationCodecError::ControlConflict => Self::ControlConflict,
            PublicationCodecError::EpochMismatch => Self::EpochMismatch,
            PublicationCodecError::AbandonFenceMissing => Self::AbandonFenceMissing,
            PublicationCodecError::JournalConflict => Self::JournalConflict,
            PublicationCodecError::JournalCorrupt => Self::JournalCorrupt,
            PublicationCodecError::JournalOutcomeUnknown => Self::JournalOutcomeUnknown,
            PublicationCodecError::BudgetExceeded | PublicationCodecError::InvalidJournalName => {
                Self::BudgetExceeded
            }
        }
    }
}

/// Closed commit shape. Constructed only through the two explicit
/// constructors below; no boolean converts into a commit shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitKind {
    /// Full publication: the observed commit advances the visible floor.
    Full,
    /// Invalidation-only finalization: no new visible epoch is published.
    InvalidationOnly,
}

impl CommitKind {
    /// Full publication shape.
    #[must_use]
    pub const fn full() -> Self {
        Self::Full
    }

    /// Invalidation-only finalization shape.
    #[must_use]
    pub const fn invalidation_only() -> Self {
        Self::InvalidationOnly
    }
}

/// Explicit point ID for exact compensation calls.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompensatePointId(pub [u8; 16]);

/// Immutable exact mutation identity for one compensation call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompensateMutation {
    /// Caller-assigned operation identity; retries reuse it byte-identical.
    pub operation_id: u64,
    /// Digest of the exact canonical mutation input.
    pub input_digest: [u8; 32],
}

/// Exact compensation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompensateReceipt {
    /// Explicit IDs the mutation applied to.
    pub affected: Vec<CompensatePointId>,
    /// Whether the receipt came from idempotent replay.
    pub replayed: bool,
}

/// Exact point readback with explicit present and missing sets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompensateReadback {
    /// Requested IDs found with exact payloads.
    pub present: Vec<CompensatePointId>,
    /// Requested IDs with no stored point.
    pub missing: Vec<CompensatePointId>,
}

/// Closed compensation failure, mirroring the bridge reason codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompensateError {
    /// The mutation may have committed; only an exact readback resolves it.
    Unknown,
    /// The same identity was reused with different input.
    Conflict,
    /// Readback or pre-dispatch state does not match the exact expectation.
    Mismatch,
}

impl CompensateError {
    /// Stable machine-readable reason code aligned with the bridge codes.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unknown => "QDRANT_MUTATION_OUTCOME_UNKNOWN",
            Self::Conflict => "QDRANT_OPERATION_CONFLICT",
            Self::Mismatch => "QDRANT_EXACT_READBACK_MISMATCH",
        }
    }
}

impl core::fmt::Display for CompensateError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for CompensateError {}

/// Exact-ID Qdrant compensation port.
///
/// Each method forwards to the same-named `RealDataPlane` operation with the
/// same explicit-ID semantics and the same unknown-until-readback contract:
/// `upsert_exact` stages new points, `close_exact` sets the exclusive upper
/// epoch on explicit IDs, `readback_exact` proves exact presence or absence.
/// Reads never report [`CompensateError::Unknown`]; mutations that may have
/// committed after dispatch always do.
pub trait QdrantCompensate {
    /// Upserts only the named point IDs.
    ///
    /// # Errors
    ///
    /// Returns [`CompensateError`] for bound violations, identity conflicts,
    /// or an unknown post-dispatch outcome.
    fn upsert_exact(
        &mut self,
        ids: Vec<CompensatePointId>,
        mutation: CompensateMutation,
    ) -> Result<CompensateReceipt, CompensateError>;

    /// Sets the exclusive upper epoch on only the named point IDs.
    ///
    /// # Errors
    ///
    /// Returns [`CompensateError`] for bound violations, missing or stale
    /// points, identity conflicts, or an unknown post-dispatch outcome.
    fn close_exact(
        &mut self,
        ids: Vec<CompensatePointId>,
        valid_until: Epoch,
        mutation: CompensateMutation,
    ) -> Result<CompensateReceipt, CompensateError>;

    /// Reads back exactly the named point IDs.
    ///
    /// # Errors
    ///
    /// Returns [`CompensateError`] for bound violations only; absence is data
    /// in [`CompensateReadback::missing`], never an error.
    fn readback_exact(
        &self,
        ids: Vec<CompensatePointId>,
    ) -> Result<CompensateReadback, CompensateError>;
}

/// Complete membership fence required to abandon an active commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MembershipFence {
    total: u64,
    fenced: u64,
}

impl MembershipFence {
    /// Builds a complete fence over `total` memberships.
    ///
    /// # Errors
    ///
    /// Returns [`PublisherError::BudgetExceeded`] for a zero membership count.
    pub const fn full(total: u64) -> Result<Self, PublisherError> {
        if total == 0 {
            return Err(PublisherError::BudgetExceeded);
        }
        Ok(Self {
            total,
            fenced: total,
        })
    }

    /// Builds a possibly partial fence.
    ///
    /// # Errors
    ///
    /// Returns [`PublisherError::BudgetExceeded`] for a zero membership count
    /// or for more fenced memberships than exist.
    pub const fn partial(fenced: u64, total: u64) -> Result<Self, PublisherError> {
        if total == 0 || fenced > total {
            return Err(PublisherError::BudgetExceeded);
        }
        Ok(Self { total, fenced })
    }

    /// Whether every membership is fenced.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.total > 0 && self.fenced == self.total
    }
}

/// Logical retirement record: exact point IDs retired by a publication.
///
/// This record never deletes anything; it only names the retired IDs so the
/// exact-ID reclaimer can act later. No physical cleaning exists on this path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredManifest {
    ids: Vec<CompensatePointId>,
}

impl RetiredManifest {
    /// Records the exact retired point IDs.
    ///
    /// # Errors
    ///
    /// Returns [`PublisherError::BudgetExceeded`] for an empty or oversized set.
    pub fn new(ids: Vec<CompensatePointId>) -> Result<Self, PublisherError> {
        if ids.is_empty() || ids.len() > MAX_RETIRED_IDS {
            return Err(PublisherError::BudgetExceeded);
        }
        Ok(Self { ids })
    }

    /// Exact retired point IDs.
    #[must_use]
    pub fn ids(&self) -> &[CompensatePointId] {
        &self.ids
    }
}

/// Durable crash-observation head for [`RecoveryDecision::decide`].
///
/// Five independent durable observations stay flat on purpose: folding them
/// into subgroups would hide crash-matrix rows instead of naming them.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryHead {
    /// A durable intent marker exists for the reservation.
    pub intent_durable: bool,
    /// The control commit for the reservation was observed.
    pub control_committed: bool,
    /// The committed snapshot was published after the control commit.
    pub snapshot_published: bool,
    /// Exact Qdrant readback verified the compensation effects.
    pub qdrant_verified: bool,
    /// The journal slot quarantined and denies normal access.
    pub quarantined: bool,
}

/// Single crash-recovery outcome. Recovery always yields exactly one head:
/// the publisher holds at most one active commit, so two competing recovery
/// targets cannot exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDecision {
    /// No pending work or verified durable work: resume proposing.
    Continue,
    /// Control committed but the snapshot is unpublished: publish it.
    PublishSnapshot,
    /// Intent durable without a verified compensation: compensate by exact
    /// IDs, then prove the effects through an exact readback.
    CompensateExact,
    /// Quarantined state denies every path until an operator intervenes.
    Blocked,
}

impl RecoveryDecision {
    /// Maps one crash-observation head to its single recovery outcome.
    #[must_use]
    pub const fn decide(head: RecoveryHead) -> Self {
        if head.quarantined {
            return Self::Blocked;
        }
        if head.control_committed && !head.snapshot_published {
            return Self::PublishSnapshot;
        }
        if head.control_committed {
            return Self::Continue;
        }
        if head.intent_durable && !head.qdrant_verified {
            return Self::CompensateExact;
        }
        Self::Continue
    }
}

/// One guarded publication proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposeRequest {
    /// Reserved epoch; must equal exactly one past the last reservation.
    pub epoch: Epoch,
    /// Guards observed by the proposer at `observed_generation`.
    pub guards: PublicationGuards,
    /// Control generation the guards were observed at.
    pub observed_generation: u64,
    /// Commit shape; invalidation-only never arrives as a boolean.
    pub kind: CommitKind,
    /// Opaque intent bytes persisted through the [`FileJournal`].
    pub intent_bytes: Vec<u8>,
}

/// Accepted proposal: the reserved epoch and its floor generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProposedCommit {
    epoch: Epoch,
    control_generation: u64,
}

impl ProposedCommit {
    /// Reserved epoch held by this proposal.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Floor generation reserved by this proposal; the control-commit
    /// observation must name exactly this generation.
    #[must_use]
    pub const fn control_generation(&self) -> u64 {
        self.control_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveCommit {
    epoch: Epoch,
    kind: CommitKind,
}

/// Guarded single-active-commit publisher.
///
/// Order per proposal: floor [`cas`] reservation, [`FileJournal`] intent
/// persistence, then exact Qdrant compensation driven by the caller through
/// [`QdrantCompensate`]. The visible floor advances only inside
/// [`Publisher::observe_control_commit`] for [`CommitKind::Full`]; an
/// uncommitted epoch is never visible, and every reservation, including an
/// abandoned one, stays consumed so skipped epochs are never reused.
pub struct Publisher {
    floor: PublicationFloor,
    last_reserved: Epoch,
    live: LiveGuardRead,
    active: Option<ActiveCommit>,
}

impl Publisher {
    /// Builds a publisher over an exact floor and live guard snapshot. The
    /// last reservation is the floor's own `floor_last_reserved`, so the
    /// first proposal must name exactly its successor.
    #[must_use]
    pub const fn new(floor: PublicationFloor, live: LiveGuardRead) -> Self {
        let last_reserved = floor.floor_last_reserved();
        Self {
            floor,
            last_reserved,
            live,
            active: None,
        }
    }

    /// Current floor generation.
    #[must_use]
    pub const fn floor_generation(&self) -> u64 {
        self.floor.generation()
    }

    /// Committed visible epoch; uncommitted reservations never appear here.
    #[must_use]
    pub const fn visible_epoch(&self) -> Epoch {
        self.floor.floor_visible()
    }

    /// Last consumed reservation, including abandoned epochs.
    #[must_use]
    pub const fn last_reserved(&self) -> Epoch {
        self.last_reserved
    }

    /// Whether exactly one commit awaits its control-commit observation.
    #[must_use]
    pub const fn has_active(&self) -> bool {
        self.active.is_some()
    }

    /// Current live guard snapshot.
    #[must_use]
    pub const fn live(&self) -> &LiveGuardRead {
        &self.live
    }

    /// Applies an externally verified guard rotation to the live snapshot.
    pub const fn rotate_live_guards(&mut self, live: LiveGuardRead) {
        self.live = live;
    }

    /// Reserves one epoch: single-active check, exact-successor epoch check,
    /// live-guard equality check, floor [`cas`], then [`FileJournal`]
    /// persistence. Failed checks consume nothing.
    ///
    /// The caller scopes `journal` to the reserved epoch (one marker slot per
    /// epoch): retries reuse the same slot with identical bytes and replay as
    /// [`JournalPersistOutcome::ReplayIdentical`], while the same epoch with
    /// different bytes reports [`PublisherError::JournalConflict`].
    ///
    /// # Errors
    ///
    /// Returns [`PublisherError::ControlConflict`] while another commit is
    /// active, when the observed guards differ from live state, or when the
    /// floor step fails; [`PublisherError::EpochMismatch`] for a skipped or
    /// reused epoch; the journal failures on persistence errors.
    pub fn propose(
        &mut self,
        journal: &FileJournal,
        request: ProposeRequest,
    ) -> Result<ProposedCommit, PublisherError> {
        let ProposeRequest {
            epoch,
            guards,
            observed_generation,
            kind,
            intent_bytes,
        } = request;
        if self.active.is_some() {
            return Err(PublisherError::ControlConflict);
        }
        let expected = self
            .last_reserved
            .checked_next()
            .map_err(|_| PublisherError::EpochMismatch)?;
        if epoch != expected {
            return Err(PublisherError::EpochMismatch);
        }
        if guards != self.live.guards || observed_generation != self.live.observed_generation {
            return Err(PublisherError::ControlConflict);
        }
        let next_generation = self
            .floor
            .generation()
            .checked_add(1)
            .ok_or(PublisherError::ControlConflict)?;
        let next = PublicationFloor::new(next_generation, self.floor.floor_visible(), epoch)?;
        let committed = cas(&self.floor, self.floor.generation(), &next)?;
        match journal.persist(&intent_bytes)? {
            JournalPersistOutcome::Persisted | JournalPersistOutcome::ReplayIdentical => {}
        }
        self.floor = committed;
        self.last_reserved = epoch;
        self.active = Some(ActiveCommit { epoch, kind });
        Ok(ProposedCommit {
            epoch,
            control_generation: committed.generation(),
        })
    }

    /// Observes the control commit for the single active proposal. A full
    /// commit advances the visible floor to the reserved epoch; an
    /// invalidation-only commit finalizes without publishing a new visible
    /// epoch. The observation must name exactly the reserved generation.
    ///
    /// # Errors
    ///
    /// Returns [`PublisherError::ControlConflict`] with no active commit or
    /// for a stale or foreign generation observation.
    pub fn observe_control_commit(
        &mut self,
        observed_generation: u64,
    ) -> Result<Epoch, PublisherError> {
        let active = self.active.ok_or(PublisherError::ControlConflict)?;
        if observed_generation != self.floor.generation() {
            return Err(PublisherError::ControlConflict);
        }
        if active.kind == CommitKind::Full {
            let next_generation = self
                .floor
                .generation()
                .checked_add(1)
                .ok_or(PublisherError::ControlConflict)?;
            let next = PublicationFloor::new(
                next_generation,
                active.epoch,
                self.floor.floor_last_reserved(),
            )?;
            let committed = cas(&self.floor, self.floor.generation(), &next)?;
            self.floor = committed;
        }
        self.active = None;
        Ok(active.epoch)
    }

    /// Abandons the active commit under a complete membership fence. The
    /// epoch stays consumed: it is never visible and never reused.
    ///
    /// # Errors
    ///
    /// Returns [`PublisherError::ControlConflict`] with no active commit, or
    /// [`PublisherError::AbandonFenceMissing`] for a partial fence.
    pub fn abandon(&mut self, fence: &MembershipFence) -> Result<Epoch, PublisherError> {
        let active = self.active.ok_or(PublisherError::ControlConflict)?;
        if !fence.is_full() {
            return Err(PublisherError::AbandonFenceMissing);
        }
        self.active = None;
        Ok(active.epoch)
    }
}
