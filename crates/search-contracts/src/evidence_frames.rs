//! Bounded additive progressive frames for the `retrieve_evidence@1` and
//! `orient_scope@1` recipes (#213).
//!
//! These frames extend the existing single-response provider plane, they do not
//! create a second server, transport, session or protocol. One complete exchange
//! still emits exactly one terminal frame, uses the same canonical framing and
//! preserves the existing terminal/progress semantics.
//!
//! Ordering is canonical and independent of leg completion: every frame carries
//! a monotone `event_sequence`, and a batch frame carries an explicit
//! `last_batch` marker. A batch is never reordered by whichever leg finished
//! first, and the terminal frame is always the final frame.
//!
//! Coverage and terminal metadata are content-free; card batches may include
//! bounded authorized previews and exact-check templates. No frame
//! carries raw absolute paths, collection names, point identifiers, filters or
//! vendor scoring internals.

use crate::bounds::{BoundedList, BoundedSet, MAX_LIST_ITEMS, MAX_REASON_CODES};
use crate::evidence::{
    EvidenceCardV1, EvidenceGroup, ExactCheckTemplate, OrientationCard, OrientationGroup,
};
use crate::ids::{ProfileId, RequestId};
use crate::protocol::SearchSourceHandle;
use crate::query::LegExecutionState;
use crate::reasons::SearchReasonCodeV1;
use crate::results::Coverage;
use crate::{ContractError, ContractErrorKind};
use std::collections::{BTreeMap, BTreeSet};

/// Closed ordered frame vocabulary of one additive progressive exchange.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EvidenceFrameKind {
    /// Request admitted; scope, profile and generation resolved. Never carries candidates.
    Accepted,
    /// Content-free coverage snapshot emitted before any candidate batch.
    CoverageSnapshot,
    /// Bounded batch of retrieved evidence cards.
    CardBatch,
    /// Bounded batch of orientation cards.
    OrientationBatch,
    /// Bounded batch of suggested exact checks.
    SuggestedCheckBatch,
    /// Exactly one terminal frame carrying the terminal classification.
    Terminal,
}

impl EvidenceFrameKind {
    /// Exact closed set.
    pub const ALL: [Self; 6] = [
        Self::Accepted,
        Self::CoverageSnapshot,
        Self::CardBatch,
        Self::OrientationBatch,
        Self::SuggestedCheckBatch,
        Self::Terminal,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::CoverageSnapshot => "coverage_snapshot",
            Self::CardBatch => "card_batch",
            Self::OrientationBatch => "orientation_batch",
            Self::SuggestedCheckBatch => "suggested_check_batch",
            Self::Terminal => "terminal",
        }
    }

    /// Parses one exact frame name; unknown or unversioned names are refused.
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| {
                ContractError::new(ContractErrorKind::InvalidToken, "evidence_frame_kind")
            })
    }
}

/// Terminal classification of one progressive exchange.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EvidenceTerminalKind {
    /// Declared work completed; coverage stays as classified.
    Success,
    /// Work stopped with explicitly bounded partial coverage.
    Partial,
    /// Work stopped before any verified postcondition.
    Failure,
}

impl EvidenceTerminalKind {
    /// Exact closed set.
    pub const ALL: [Self; 3] = [Self::Success, Self::Partial, Self::Failure];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "terminal_success",
            Self::Partial => "terminal_partial",
            Self::Failure => "terminal_failure",
        }
    }

    /// Parses one exact terminal name; unknown or unversioned names are refused.
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| {
                ContractError::new(ContractErrorKind::InvalidToken, "evidence_terminal_kind")
            })
    }
}
/// Maximum frame items in one batch.
pub const MAX_EVIDENCE_FRAME_ITEMS: usize = 256;
/// Maximum byte size of one encoded progressive frame.
pub const MAX_EVIDENCE_FRAME_BYTES: usize = 1024 * 1024;

/// Header shared by every additive progressive frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceFrameHeader {
    pub request_id: RequestId,
    pub kind: EvidenceFrameKind,
    pub event_sequence: u64,
    pub profile_id: ProfileId,
}

impl EvidenceFrameHeader {
    pub fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }
}

/// `accepted` frame: scope, profile and generation resolved before planning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceAcceptedFrame {
    pub header: EvidenceFrameHeader,
    /// True when the caller requested a progressive exchange.
    pub progressive: bool,
    /// Declared total frame budget for this exchange, including this frame.
    pub declared_frame_budget: u32,
}

impl EvidenceAcceptedFrame {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.header.validate()?;
        if self.header.kind != EvidenceFrameKind::Accepted {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "accepted_frame.kind",
            ));
        }
        if self.declared_frame_budget == 0 {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "accepted_frame.declared_frame_budget",
            ));
        }
        Ok(())
    }
}

/// Content-free coverage snapshot emitted before any candidate batch.
///
/// This is the only truthful way to distinguish an empty result set from an
/// exhausted one. A zero-card result with an incomplete snapshot is never a
/// complete negative, and the classification comes from the existing
/// `Coverage` type rather than a second coverage model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceCoverageSnapshotFrame {
    pub header: EvidenceFrameHeader,
    pub coverage: Coverage,
    /// Whether a complete-negative claim is even possible for this exchange.
    pub complete_negative_possible: bool,
}

impl EvidenceCoverageSnapshotFrame {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.header.validate()?;
        self.coverage.validate()?;
        if self.header.kind != EvidenceFrameKind::CoverageSnapshot {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "coverage_snapshot.kind",
            ));
        }
        // A complete-negative claim requires the exact complete-scope denominator.
        // These heuristic recipes carry no ExactExecutionReport, so a scope
        // classification alone cannot authorize a complete negative.
        if self.complete_negative_possible {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "coverage_snapshot.complete_negative_possible",
            ));
        }
        // Any known gap forbids a complete-negative claim.
        let gap_present = !self.coverage.omitted_or_failed_legs.is_empty()
            || !self.coverage.candidate_validation_gaps.is_empty()
            || !self.coverage.unknowns.is_empty()
            || self
                .coverage
                .executed_legs
                .iter()
                .any(|leg| leg.state != LegExecutionState::Completed);
        if gap_present && self.complete_negative_possible {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "coverage_snapshot.gap_with_complete_negative",
            ));
        }
        Ok(())
    }
}

/// Bounded batch of evidence cards with an explicit terminal-batch marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceCardBatchFrame {
    pub header: EvidenceFrameHeader,
    pub cards: BoundedList<EvidenceCardV1, MAX_EVIDENCE_FRAME_ITEMS>,
    pub groups: BoundedList<EvidenceGroup, MAX_LIST_ITEMS>,
    /// True only on the final batch of this frame kind.
    pub last_batch: bool,
    /// Monotone offset of the first item in this batch within the full result set.
    pub batch_offset: u64,
}

impl EvidenceCardBatchFrame {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.header.validate()?;
        if self.header.kind != EvidenceFrameKind::CardBatch {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "card_batch.kind",
            ));
        }
        for card in &self.cards {
            card.validate()?;
        }
        if self.cards.is_empty() && !self.last_batch {
            return Err(ContractError::new(
                ContractErrorKind::Empty,
                "card_batch.empty_not_last",
            ));
        }
        Ok(())
    }
}

/// Bounded batch of orientation cards with an explicit terminal-batch marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrientationBatchFrame {
    pub header: EvidenceFrameHeader,
    pub cards: BoundedList<OrientationCard, MAX_EVIDENCE_FRAME_ITEMS>,
    pub groups: BoundedList<OrientationGroup, MAX_LIST_ITEMS>,
    pub recommended_reading: BoundedList<SearchSourceHandle, MAX_LIST_ITEMS>,
    /// True only on the final batch of this frame kind.
    pub last_batch: bool,
    pub batch_offset: u64,
}

impl OrientationBatchFrame {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.header.validate()?;
        for card in &self.cards {
            card.validate()?;
        }
        if self.header.kind != EvidenceFrameKind::OrientationBatch {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "orientation_batch.kind",
            ));
        }
        if self.cards.is_empty() && !self.last_batch {
            return Err(ContractError::new(
                ContractErrorKind::Empty,
                "orientation_batch.empty_not_last",
            ));
        }
        Ok(())
    }
}

/// Bounded batch of suggested exact checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedCheckBatchFrame {
    pub header: EvidenceFrameHeader,
    pub checks: BoundedList<ExactCheckTemplate, MAX_EVIDENCE_FRAME_ITEMS>,
    /// True only on the final batch of this frame kind.
    pub last_batch: bool,
    pub batch_offset: u64,
}

impl SuggestedCheckBatchFrame {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.header.kind != EvidenceFrameKind::SuggestedCheckBatch {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "suggested_check_batch.kind",
            ));
        }
        if self.checks.is_empty() && !self.last_batch {
            return Err(ContractError::new(
                ContractErrorKind::Empty,
                "suggested_check_batch.empty_not_last",
            ));
        }
        Ok(())
    }
}

/// Exactly one terminal frame carrying the terminal classification and the
/// final content-free coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceTerminalFrame {
    pub header: EvidenceFrameHeader,
    pub terminal: EvidenceTerminalKind,
    pub coverage: Coverage,
    pub reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
    /// Set when the exchange stopped at its frame or item budget.
    pub budget_exhausted: bool,
}

impl EvidenceTerminalFrame {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.header.validate()?;
        self.coverage.validate()?;
        if self.header.kind != EvidenceFrameKind::Terminal {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "terminal_frame.kind",
            ));
        }
        // Summary presence is not completion: bind each summary to one unique
        // requested leg. Partial/failure may omit legs; success may not.
        let mut requested = BTreeSet::new();
        for leg in &self.coverage.requested_legs {
            if !requested.insert(&leg.leg_ref) {
                return Err(ContractError::new(
                    ContractErrorKind::Duplicate,
                    "terminal_frame.duplicate_requested_leg",
                ));
            }
        }
        let mut executed = BTreeSet::new();
        for summary in &self.coverage.executed_legs {
            if !requested.contains(&summary.leg_ref) {
                return Err(ContractError::new(
                    ContractErrorKind::FamilyMismatch,
                    "terminal_frame.foreign_executed_leg",
                ));
            }
            if !executed.insert(&summary.leg_ref) {
                return Err(ContractError::new(
                    ContractErrorKind::Duplicate,
                    "terminal_frame.duplicate_executed_leg",
                ));
            }
        }
        if self.terminal == EvidenceTerminalKind::Success && executed != requested {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "terminal_frame.success_with_missing_execution",
            ));
        }
        // A success terminal may never claim complete-negative coverage without
        // its exact denominator, and may never be labelled success when a gap
        // remains.
        let gap_present = !self.coverage.omitted_or_failed_legs.is_empty()
            || !self.coverage.candidate_validation_gaps.is_empty()
            || !self.coverage.unknowns.is_empty()
            || self
                .coverage
                .executed_legs
                .iter()
                .any(|leg| leg.state != LegExecutionState::Completed);
        if self.terminal == EvidenceTerminalKind::Success && gap_present {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "terminal_frame.success_with_gap",
            ));
        }
        if self.budget_exhausted && self.terminal == EvidenceTerminalKind::Success {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "terminal_frame.success_with_exhausted_budget",
            ));
        }
        Ok(())
    }
}

/// Closed union of the six additive progressive frames.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceFrameV2 {
    Accepted(EvidenceAcceptedFrame),
    CoverageSnapshot(EvidenceCoverageSnapshotFrame),
    CardBatch(EvidenceCardBatchFrame),
    OrientationBatch(OrientationBatchFrame),
    SuggestedCheckBatch(SuggestedCheckBatchFrame),
    Terminal(EvidenceTerminalFrame),
}

impl EvidenceFrameV2 {
    /// Shape-only exchange binding; a request ID is not session authorization.
    #[must_use]
    pub const fn header(&self) -> &EvidenceFrameHeader {
        match self {
            Self::Accepted(value) => &value.header,
            Self::CoverageSnapshot(value) => &value.header,
            Self::CardBatch(value) => &value.header,
            Self::OrientationBatch(value) => &value.header,
            Self::SuggestedCheckBatch(value) => &value.header,
            Self::Terminal(value) => &value.header,
        }
    }
    #[must_use]
    pub const fn kind(&self) -> EvidenceFrameKind {
        match self {
            Self::Accepted(_) => EvidenceFrameKind::Accepted,
            Self::CoverageSnapshot(_) => EvidenceFrameKind::CoverageSnapshot,
            Self::CardBatch(_) => EvidenceFrameKind::CardBatch,
            Self::OrientationBatch(_) => EvidenceFrameKind::OrientationBatch,
            Self::SuggestedCheckBatch(_) => EvidenceFrameKind::SuggestedCheckBatch,
            Self::Terminal(_) => EvidenceFrameKind::Terminal,
        }
    }

    #[must_use]
    pub const fn event_sequence(&self) -> u64 {
        match self {
            Self::Accepted(value) => value.header.event_sequence,
            Self::CoverageSnapshot(value) => value.header.event_sequence,
            Self::CardBatch(value) => value.header.event_sequence,
            Self::OrientationBatch(value) => value.header.event_sequence,
            Self::SuggestedCheckBatch(value) => value.header.event_sequence,
            Self::Terminal(value) => value.header.event_sequence,
        }
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Accepted(value) => value.validate(),
            Self::CoverageSnapshot(value) => value.validate(),
            Self::CardBatch(value) => value.validate(),
            Self::OrientationBatch(value) => value.validate(),
            Self::SuggestedCheckBatch(value) => value.validate(),
            Self::Terminal(value) => value.validate(),
        }
    }
}

/// Validates one complete progressive exchange against its ordering and
/// budget contract.
///
/// Enforced rules, in order:
/// 1. exactly one `accepted` frame, first;
/// 2. exactly one `coverage_snapshot` frame, before any batch frame;
/// 3. each batch frame kind repeats only contiguously, with monotone
///    `event_sequence` and contiguous `batch_offset`;
/// 4. every batch frame kind declares exactly one final batch, except that a
///    batch kind may be entirely absent;
/// 5. exactly one `terminal` frame, last;
/// 6. the total frame count never exceeds the budget declared in the
///    `accepted` frame;
/// 7. no frame appears after the terminal frame.
pub fn validate_evidence_exchange(
    frames: &BoundedList<EvidenceFrameV2, MAX_EVIDENCE_EXCHANGE_FRAMES>,
) -> Result<(), ContractError> {
    let mut iter = frames.iter();
    let Some(EvidenceFrameV2::Accepted(accepted)) = iter.next() else {
        return Err(ContractError::new(
            ContractErrorKind::ContradictoryState,
            "evidence_exchange.first_frame",
        ));
    };
    accepted.validate()?;
    if accepted.declared_frame_budget < u32::try_from(frames.len()).unwrap_or(u32::MAX) {
        return Err(ContractError::new(
            ContractErrorKind::InvalidRange,
            "evidence_exchange.frame_budget",
        ));
    }

    let mut previous_sequence = accepted.header.event_sequence;
    let mut saw_snapshot = false;
    let mut terminal_seen = false;
    let mut batch_state: BTreeMap<EvidenceFrameKind, (u64, bool)> = BTreeMap::new();
    let mut batch_tail: Option<EvidenceFrameKind> = None;

    for frame in iter {
        if terminal_seen {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "evidence_exchange.frame_after_terminal",
            ));
        }
        frame.validate()?;
        if frame.header().request_id != accepted.header.request_id
            || frame.header().profile_id != accepted.header.profile_id
        {
            return Err(ContractError::new(
                ContractErrorKind::FamilyMismatch,
                "evidence_exchange.binding",
            ));
        }
        if frame.event_sequence() <= previous_sequence {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "evidence_exchange.event_sequence",
            ));
        }
        previous_sequence = frame.event_sequence();

        match frame {
            EvidenceFrameV2::Accepted(_) => {
                return Err(ContractError::new(
                    ContractErrorKind::Duplicate,
                    "evidence_exchange.second_accepted",
                ));
            }
            EvidenceFrameV2::CoverageSnapshot(_) => {
                if saw_snapshot {
                    return Err(ContractError::new(
                        ContractErrorKind::Duplicate,
                        "evidence_exchange.second_snapshot",
                    ));
                }
                if !batch_state.is_empty() {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidTaggedVariant,
                        "evidence_exchange.snapshot_after_batch",
                    ));
                }
                saw_snapshot = true;
            }
            EvidenceFrameV2::CardBatch(_)
            | EvidenceFrameV2::OrientationBatch(_)
            | EvidenceFrameV2::SuggestedCheckBatch(_) => {
                if !saw_snapshot {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidTaggedVariant,
                        "evidence_exchange.batch_before_snapshot",
                    ));
                }
                let kind = frame.kind();
                // A batch kind must not reappear after a different kind started.
                let tail_open = matches!(batch_tail, Some(tail) if tail != kind);
                if tail_open
                    && batch_state.contains_key(&batch_tail.unwrap_or(kind))
                    && !batch_state[&batch_tail.unwrap_or(kind)].1
                {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidTaggedVariant,
                        "evidence_exchange.interleaved_batches",
                    ));
                }
                batch_tail = Some(kind);
                let (offset, last) = batch_offset_and_last(frame);
                let entry = batch_state.entry(kind).or_insert((0, false));
                if offset != entry.0 {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidRange,
                        "evidence_exchange.batch_offset",
                    ));
                }
                if entry.1 {
                    return Err(ContractError::new(
                        ContractErrorKind::Duplicate,
                        "evidence_exchange.batch_after_last",
                    ));
                }
                entry.0 = offset.checked_add(batch_item_count(frame)).ok_or_else(|| {
                    ContractError::new(
                        ContractErrorKind::InvalidRange,
                        "evidence_exchange.offset_overflow",
                    )
                })?;
                if last {
                    entry.1 = true;
                }
            }
            EvidenceFrameV2::Terminal(terminal) => {
                terminal_seen = true;
                // Every started batch kind must have declared its final batch.
                for (kind, (_, last)) in &batch_state {
                    if !last {
                        let _ = kind;
                        return Err(ContractError::new(
                            ContractErrorKind::InvalidTaggedVariant,
                            "evidence_exchange.unterminated_batch",
                        ));
                    }
                }
                let _ = terminal;
            }
        }
    }

    if !terminal_seen || !saw_snapshot {
        return Err(ContractError::new(
            ContractErrorKind::ContradictoryState,
            "evidence_exchange.missing_terminal",
        ));
    }
    Ok(())
}

/// Batch offset and last-batch marker for one batch frame.
fn batch_offset_and_last(frame: &EvidenceFrameV2) -> (u64, bool) {
    match frame {
        EvidenceFrameV2::CardBatch(batch) => (batch.batch_offset, batch.last_batch),
        EvidenceFrameV2::OrientationBatch(batch) => (batch.batch_offset, batch.last_batch),
        EvidenceFrameV2::SuggestedCheckBatch(batch) => (batch.batch_offset, batch.last_batch),
        _ => (0, false),
    }
}

fn batch_item_count(frame: &EvidenceFrameV2) -> u64 {
    match frame {
        EvidenceFrameV2::CardBatch(batch) => u64::try_from(batch.cards.len()).unwrap_or(u64::MAX),
        EvidenceFrameV2::OrientationBatch(batch) => {
            u64::try_from(batch.cards.len()).unwrap_or(u64::MAX)
        }
        EvidenceFrameV2::SuggestedCheckBatch(batch) => {
            u64::try_from(batch.checks.len()).unwrap_or(u64::MAX)
        }
        _ => 0,
    }
}

/// Maximum frames in one complete progressive exchange.
pub const MAX_EVIDENCE_EXCHANGE_FRAMES: usize = 4_096;
