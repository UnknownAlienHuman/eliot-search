//! Guarded single-active publication state machine over the control floor.

use search_contracts::{Epoch, PublicationGuards};
use search_control_redb::publication_codec::{
    FileJournal, JournalPersistOutcome, PublicationFloor, cas,
};

use super::guards::LiveGuardRead;
use super::retirement::MembershipFence;
use super::spec::{CommitKind, PublisherError};

/// One guarded publication proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposeRequest {
    /// Reserved epoch; must be exactly one past the last reservation.
    pub epoch: Epoch,
    /// Guards observed by the proposer at `observed_generation`.
    pub guards: PublicationGuards,
    /// Control generation at which the guards were observed.
    pub observed_generation: u64,
    /// Explicit full or invalidation-only commit shape.
    pub kind: CommitKind,
    /// Opaque intent bytes persisted through the file journal.
    pub intent_bytes: Vec<u8>,
}

/// Accepted proposal: reserved epoch and exact floor generation.
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

    /// Floor generation required by the control-commit observation.
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
/// Proposal order is control-floor reservation through `cas`, durable intent
/// persistence through `FileJournal`, and only then caller-owned exact
/// compensation. Visible floor advancement occurs only after an exact control
/// commit observation. Every reservation remains consumed, including abandon.
pub struct Publisher {
    floor: PublicationFloor,
    last_reserved: Epoch,
    live: LiveGuardRead,
    active: Option<ActiveCommit>,
}

impl Publisher {
    /// Builds a publisher over an exact floor and live guard observation.
    #[must_use]
    pub const fn new(
        floor: PublicationFloor,
        live: LiveGuardRead,
    ) -> Self {
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

    /// Whether exactly one commit awaits its control observation.
    #[must_use]
    pub const fn has_active(&self) -> bool {
        self.active.is_some()
    }

    /// Current live guard observation.
    #[must_use]
    pub const fn live(&self) -> &LiveGuardRead {
        &self.live
    }

    /// Applies an externally verified guard rotation.
    pub const fn rotate_live_guards(&mut self, live: LiveGuardRead) {
        self.live = live;
    }

    /// Reserves exactly one successor epoch and persists its exact intent.
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
        if guards != self.live.guards()
            || observed_generation != self.live.observed_generation()
        {
            return Err(PublisherError::ControlConflict);
        }
        let next_generation = self
            .floor
            .generation()
            .checked_add(1)
            .ok_or(PublisherError::ControlConflict)?;
        let next = PublicationFloor::new(
            next_generation,
            self.floor.floor_visible(),
            epoch,
        )?;
        let committed = cas(&self.floor, self.floor.generation(), &next)?;
        match journal.persist(&intent_bytes)? {
            JournalPersistOutcome::Persisted
            | JournalPersistOutcome::ReplayIdentical => {}
        }
        self.floor = committed;
        self.last_reserved = epoch;
        self.active = Some(ActiveCommit { epoch, kind });
        Ok(ProposedCommit {
            epoch,
            control_generation: committed.generation(),
        })
    }

    /// Observes the exact control commit for the active proposal.
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
            let committed =
                cas(&self.floor, self.floor.generation(), &next)?;
            self.floor = committed;
        }
        self.active = None;
        Ok(active.epoch)
    }

    /// Abandons the active commit only under a complete membership fence.
    pub fn abandon(
        &mut self,
        fence: &MembershipFence,
    ) -> Result<Epoch, PublisherError> {
        let active = self.active.ok_or(PublisherError::ControlConflict)?;
        if !fence.is_full() {
            return Err(PublisherError::AbandonFenceMissing);
        }
        self.active = None;
        Ok(active.epoch)
    }
}
