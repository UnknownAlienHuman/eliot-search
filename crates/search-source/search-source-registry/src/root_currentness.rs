//! Pure source-root observation currentness and bounded watcher state.
//!
//! This module owns content-free root observation states, watcher dirty hints,
//! explicit gaps, reconciliation generations and source/workspace currentness.
//! It performs no filesystem, path, source, index, clock or persistence I/O.

use core::fmt;

use crate::legacy_root_catalog::LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS;

/// Maximum watcher hints retained before overflow becomes an explicit gap.
pub const MAX_SOURCE_ROOT_WATCHER_HINTS: usize = 64;
/// Maximum explicit gaps returned in one currentness snapshot.
pub const MAX_SOURCE_ROOT_OBSERVATION_GAPS: usize = 40;

/// Result of one qualified observation of a configured root.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceRootState {
    /// The exact configured root is currently an admissible directory.
    Available,
    /// The configured root is absent.
    Missing,
    /// The locator resolves to a non-directory object.
    NotDirectory,
    /// The root is a symlink, reparse point, replacement or otherwise unsafe.
    Unsafe,
    /// The platform adapter could not establish a qualified observation.
    Unverifiable,
}

impl SourceRootState {
    /// Stable content-free diagnostic spelling.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Missing => "missing",
            Self::NotDirectory => "not_directory",
            Self::Unsafe => "unsafe",
            Self::Unverifiable => "unverifiable",
        }
    }
}

/// Closed watcher-hint kind. Hints are dirty markers only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatcherHintKind {
    /// A watched object may have changed.
    Modified,
    /// A watched object may have appeared.
    Created,
    /// A watched object may have disappeared.
    Removed,
    /// The adapter requests an authoritative rescan.
    Rescan,
    /// The watcher itself reported lost events.
    Overflow,
}

impl WatcherHintKind {
    /// Stable content-free diagnostic spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modified => "modified",
            Self::Created => "created",
            Self::Removed => "removed",
            Self::Rescan => "rescan",
            Self::Overflow => "overflow",
        }
    }
}

/// One bounded watcher hint inside one currentness lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatcherHint {
    /// Zero-based configured-root position.
    pub position: usize,
    /// Content-free dirty-hint kind.
    pub kind: WatcherHintKind,
    /// Monotone wrapping sequence within this in-memory lifetime.
    pub sequence: u64,
}

/// Typed observation-gap reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationGapReason {
    /// A configured root is absent.
    Missing,
    /// A configured root was replaced by a non-directory.
    NotDirectory,
    /// A configured root failed link/reparse/replacement policy.
    Unsafe,
    /// A qualified observation could not be established.
    Unverifiable,
    /// Watcher capacity overflowed and authoritative reconciliation is required.
    WatcherOverflow,
    /// Durable registration update outcome is unresolved and the catalog must reopen.
    UpdateOutcomeUnknown,
}

impl ObservationGapReason {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Missing => "OBSERVATION_GAP_MISSING",
            Self::NotDirectory => "OBSERVATION_GAP_NOT_DIRECTORY",
            Self::Unsafe => "OBSERVATION_GAP_UNSAFE",
            Self::Unverifiable => "OBSERVATION_GAP_UNVERIFIABLE",
            Self::WatcherOverflow => "OBSERVATION_GAP_WATCHER_OVERFLOW",
            Self::UpdateOutcomeUnknown => "OBSERVATION_GAP_UPDATE_OUTCOME_UNKNOWN",
        }
    }
}

/// One explicit observation gap blocking currentness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationGap {
    /// Zero-based configured-root position, or configured count for watcher overflow.
    pub position: usize,
    /// Why currentness is blocked.
    pub reason: ObservationGapReason,
    /// Last qualified state associated with the gap.
    pub state: SourceRootState,
}

/// Bounded reconciliation cursor snapshot for control-state diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationCursor {
    /// Current reconciliation generation.
    pub generation: u64,
    /// Number of retained dirty hints.
    pub pending_hints: usize,
    /// Whether watcher capacity overflowed.
    pub overflowed: bool,
    /// Last allocated watcher-hint sequence.
    pub hint_sequence: u64,
    /// Generation last proven fully synchronized, when any.
    pub last_synced_generation: Option<u64>,
}

/// Independent source/workspace truth snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentWorkspaceTruth {
    /// Number of configured roots.
    pub configured: usize,
    /// Number of roots currently observed available.
    pub available: usize,
    /// Number of configured roots not currently available.
    pub unavailable: usize,
    /// Number of explicit bounded gaps.
    pub gap_count: usize,
    /// Current reconciliation generation.
    pub reconciliation_generation: u64,
    /// Generation last proven fully synchronized, when any.
    pub last_synced_generation: Option<u64>,
    /// Whether every configured root is currently observable with no gap.
    pub source_current: bool,
    /// Whether source currentness is also sync-proven for this exact generation.
    pub workspace_current: bool,
}

/// Closed failure from source-root currentness mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceRootCurrentnessError {
    /// Configured-root capacity would exceed the frozen root ceiling.
    RootLimitExceeded,
    /// A supplied zero-based root position is outside the configured set.
    PositionOutOfRange,
    /// Registration outcome is unresolved and mutation must stop until reopen.
    UpdateOutcomeUnknown,
}

impl SourceRootCurrentnessError {
    /// Stable package-local reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RootLimitExceeded => "REGISTRY_ROOT_CURRENTNESS_LIMIT",
            Self::PositionOutOfRange => "REGISTRY_ROOT_CURRENTNESS_POSITION_INVALID",
            Self::UpdateOutcomeUnknown => "REGISTRY_ROOT_CURRENTNESS_OUTCOME_UNKNOWN",
        }
    }
}

impl fmt::Display for SourceRootCurrentnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SourceRootCurrentnessError {}

/// Pure owner of configured-root observation and synchronization currentness.
///
/// The state vector is position-aligned with caller-owned qualified locators.
/// Locators and source content never enter this type.
#[derive(Debug, Eq, PartialEq)]
pub struct SourceRootCurrentness {
    states: Vec<SourceRootState>,
    update_outcome_unknown: bool,
    watcher_sequence: u64,
    pending_hints: Vec<WatcherHint>,
    watcher_overflowed: bool,
    reconciliation_generation: u64,
    last_synced_generation: Option<u64>,
}

impl SourceRootCurrentness {
    /// Creates one currentness owner with every configured root unverifiable.
    pub fn new(configured_count: usize) -> Result<Self, SourceRootCurrentnessError> {
        if configured_count > LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS {
            return Err(SourceRootCurrentnessError::RootLimitExceeded);
        }
        Ok(Self {
            states: vec![SourceRootState::Unverifiable; configured_count],
            update_outcome_unknown: false,
            watcher_sequence: 0,
            pending_hints: Vec::new(),
            watcher_overflowed: false,
            reconciliation_generation: 0,
            last_synced_generation: None,
        })
    }

    /// Number of configured roots represented by the aligned state vector.
    #[must_use]
    pub fn configured_count(&self) -> usize {
        self.states.len()
    }

    /// Returns one root's last qualified observation.
    #[must_use]
    pub fn state(&self, position: usize) -> Option<SourceRootState> {
        self.states.get(position).copied()
    }

    /// Number of available roots, or zero while registration outcome is unknown.
    #[must_use]
    pub fn available_count(&self) -> usize {
        if self.update_outcome_unknown {
            return 0;
        }
        self.states
            .iter()
            .filter(|state| **state == SourceRootState::Available)
            .count()
    }

    /// Number of configured roots not currently available.
    #[must_use]
    pub fn unavailable_count(&self) -> usize {
        self.configured_count().saturating_sub(self.available_count())
    }

    /// Whether durable registration outcome is unresolved and reopen is required.
    #[must_use]
    pub const fn update_outcome_unknown(&self) -> bool {
        self.update_outcome_unknown
    }

    /// Permanently fails this in-memory owner closed until a fresh catalog reopen.
    pub fn mark_update_outcome_unknown(&mut self) {
        self.update_outcome_unknown = true;
        self.last_synced_generation = None;
    }

    /// Applies one complete authoritative root observation pass.
    ///
    /// A state-count mismatch fails this owner closed because locator/state
    /// alignment can no longer be proved. Dirty hints and overflow are consumed
    /// only by an exact-sized pass.
    pub fn reconcile(&mut self, observed: &[SourceRootState]) -> bool {
        if self.update_outcome_unknown {
            return false;
        }
        if observed.len() != self.states.len() {
            self.mark_update_outcome_unknown();
            return false;
        }
        let changed = self.states.as_slice() != observed;
        let had_hints = !self.pending_hints.is_empty();
        let had_overflow = self.watcher_overflowed;
        self.states.copy_from_slice(observed);
        self.pending_hints.clear();
        self.watcher_overflowed = false;
        if changed || had_hints || had_overflow {
            self.advance_generation();
        }
        changed || had_hints || had_overflow
    }

    /// Applies one qualified observation to an existing configured position.
    pub fn observe(
        &mut self,
        position: usize,
        state: SourceRootState,
    ) -> Result<bool, SourceRootCurrentnessError> {
        self.ensure_usable()?;
        let current = self
            .states
            .get_mut(position)
            .ok_or(SourceRootCurrentnessError::PositionOutOfRange)?;
        if *current == state {
            return Ok(false);
        }
        *current = state;
        self.advance_generation();
        Ok(true)
    }

    /// Inserts one newly persisted configured root at the matching sorted position.
    pub fn insert(
        &mut self,
        position: usize,
        state: SourceRootState,
    ) -> Result<(), SourceRootCurrentnessError> {
        self.ensure_usable()?;
        if self.states.len() >= LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS {
            return Err(SourceRootCurrentnessError::RootLimitExceeded);
        }
        if position > self.states.len() {
            return Err(SourceRootCurrentnessError::PositionOutOfRange);
        }
        self.states.insert(position, state);
        self.advance_generation();
        Ok(())
    }

    /// Removes one durably unregistered configured-root position.
    pub fn remove(
        &mut self,
        position: usize,
    ) -> Result<(), SourceRootCurrentnessError> {
        self.ensure_usable()?;
        if position >= self.states.len() {
            return Err(SourceRootCurrentnessError::PositionOutOfRange);
        }
        self.states.remove(position);
        self.advance_generation();
        Ok(())
    }

    /// Records one watcher notification as a bounded dirty hint only.
    pub fn note_watcher_hint(
        &mut self,
        position: usize,
        kind: WatcherHintKind,
    ) -> Result<bool, SourceRootCurrentnessError> {
        self.ensure_usable()?;
        if position >= self.states.len() {
            return Err(SourceRootCurrentnessError::PositionOutOfRange);
        }
        self.watcher_sequence = self.watcher_sequence.wrapping_add(1);
        if self.pending_hints.len() >= MAX_SOURCE_ROOT_WATCHER_HINTS {
            self.watcher_overflowed = true;
            return Ok(true);
        }
        self.pending_hints.push(WatcherHint {
            position,
            kind,
            sequence: self.watcher_sequence,
        });
        Ok(self.watcher_overflowed)
    }

    /// Drains retained watcher hints without treating them as authoritative truth.
    pub fn drain_watcher_hints(&mut self) -> Vec<WatcherHint> {
        core::mem::take(&mut self.pending_hints)
    }

    /// Whether watcher capacity overflowed since the last exact reconciliation.
    #[must_use]
    pub const fn watcher_overflowed(&self) -> bool {
        self.watcher_overflowed
    }

    /// Returns explicit bounded gaps blocking source/workspace currentness.
    #[must_use]
    pub fn observation_gaps(&self) -> Vec<ObservationGap> {
        let mut gaps = Vec::new();
        if self.update_outcome_unknown {
            gaps.push(ObservationGap {
                position: 0,
                reason: ObservationGapReason::UpdateOutcomeUnknown,
                state: SourceRootState::Unverifiable,
            });
            return gaps;
        }
        for (position, state) in self.states.iter().copied().enumerate() {
            let reason = match state {
                SourceRootState::Available => continue,
                SourceRootState::Missing => ObservationGapReason::Missing,
                SourceRootState::NotDirectory => ObservationGapReason::NotDirectory,
                SourceRootState::Unsafe => ObservationGapReason::Unsafe,
                SourceRootState::Unverifiable => ObservationGapReason::Unverifiable,
            };
            if gaps.len() >= MAX_SOURCE_ROOT_OBSERVATION_GAPS {
                break;
            }
            gaps.push(ObservationGap {
                position,
                reason,
                state,
            });
        }
        if self.watcher_overflowed
            && gaps.len() < MAX_SOURCE_ROOT_OBSERVATION_GAPS
        {
            gaps.push(ObservationGap {
                position: self.states.len(),
                reason: ObservationGapReason::WatcherOverflow,
                state: SourceRootState::Unverifiable,
            });
        }
        gaps
    }

    /// Returns one content-free reconciliation cursor snapshot.
    #[must_use]
    pub fn reconciliation_cursor(&self) -> ReconciliationCursor {
        ReconciliationCursor {
            generation: self.reconciliation_generation,
            pending_hints: self.pending_hints.len(),
            overflowed: self.watcher_overflowed,
            hint_sequence: self.watcher_sequence,
            last_synced_generation: self.last_synced_generation,
        }
    }

    /// Computes source/workspace truth without index truth.
    #[must_use]
    pub fn current_workspace_truth(&self) -> CurrentWorkspaceTruth {
        let gaps = self.observation_gaps();
        let configured = self.configured_count();
        let available = self.available_count();
        let unavailable = self.unavailable_count();
        let source_current = !self.update_outcome_unknown
            && configured > 0
            && gaps.is_empty()
            && available == configured;
        let workspace_current = source_current
            && self.last_synced_generation == Some(self.reconciliation_generation);
        CurrentWorkspaceTruth {
            configured,
            available,
            unavailable,
            gap_count: gaps.len(),
            reconciliation_generation: self.reconciliation_generation,
            last_synced_generation: self.last_synced_generation,
            source_current,
            workspace_current,
        }
    }

    /// Marks the current reconciliation generation as sync-proven.
    pub fn mark_reconciled_synced(&mut self) -> bool {
        if self.update_outcome_unknown
            || self.states.is_empty()
            || !self.observation_gaps().is_empty()
        {
            return false;
        }
        self.last_synced_generation = Some(self.reconciliation_generation);
        true
    }

    fn ensure_usable(&self) -> Result<(), SourceRootCurrentnessError> {
        if self.update_outcome_unknown {
            Err(SourceRootCurrentnessError::UpdateOutcomeUnknown)
        } else {
            Ok(())
        }
    }

    fn advance_generation(&mut self) {
        self.reconciliation_generation =
            self.reconciliation_generation.wrapping_add(1);
        self.last_synced_generation = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_reconcile_and_sync_proof_are_generation_bound() {
        let mut currentness = SourceRootCurrentness::new(2).expect("currentness");
        assert!(currentness.reconcile(&[
            SourceRootState::Available,
            SourceRootState::Available,
        ]));
        let generation = currentness.reconciliation_cursor().generation;
        assert!(currentness.mark_reconciled_synced());
        assert!(currentness.current_workspace_truth().workspace_current);
        assert!(!currentness.reconcile(&[
            SourceRootState::Available,
            SourceRootState::Available,
        ]));
        assert_eq!(
            currentness.reconciliation_cursor().generation,
            generation
        );
        assert!(
            currentness
                .observe(1, SourceRootState::Missing)
                .expect("observe")
        );
        assert!(!currentness.current_workspace_truth().source_current);
        assert_eq!(
            currentness.observation_gaps()[0].reason,
            ObservationGapReason::Missing
        );
        assert!(!currentness.mark_reconciled_synced());
    }

    #[test]
    fn watcher_hints_are_bounded_dirty_markers_only() {
        let mut currentness = SourceRootCurrentness::new(1).expect("currentness");
        assert!(currentness.reconcile(&[SourceRootState::Available]));
        for _ in 0..=MAX_SOURCE_ROOT_WATCHER_HINTS {
            currentness
                .note_watcher_hint(0, WatcherHintKind::Modified)
                .expect("hint");
        }
        assert!(currentness.watcher_overflowed());
        assert_eq!(currentness.available_count(), 1);
        assert!(currentness
            .observation_gaps()
            .iter()
            .any(|gap| gap.reason == ObservationGapReason::WatcherOverflow));
        assert!(!currentness.current_workspace_truth().source_current);
        assert!(currentness.reconcile(&[SourceRootState::Available]));
        assert!(!currentness.watcher_overflowed());
        assert!(currentness.observation_gaps().is_empty());
    }

    #[test]
    fn registration_mutations_keep_state_positions_aligned() {
        let mut currentness = SourceRootCurrentness::new(1).expect("currentness");
        assert!(currentness.reconcile(&[SourceRootState::Available]));
        currentness
            .insert(0, SourceRootState::Missing)
            .expect("insert");
        assert_eq!(currentness.state(0), Some(SourceRootState::Missing));
        assert_eq!(currentness.state(1), Some(SourceRootState::Available));
        currentness.remove(0).expect("remove");
        assert_eq!(currentness.state(0), Some(SourceRootState::Available));
    }

    #[test]
    fn state_count_mismatch_fails_owner_closed() {
        let mut currentness = SourceRootCurrentness::new(1).expect("currentness");
        assert!(!currentness.reconcile(&[]));
        assert!(currentness.update_outcome_unknown());
        assert_eq!(
            currentness.observation_gaps(),
            vec![ObservationGap {
                position: 0,
                reason: ObservationGapReason::UpdateOutcomeUnknown,
                state: SourceRootState::Unverifiable,
            }]
        );
        assert_eq!(currentness.available_count(), 0);
        assert_eq!(
            currentness.observe(0, SourceRootState::Available),
            Err(SourceRootCurrentnessError::UpdateOutcomeUnknown)
        );
    }

    #[test]
    fn empty_registration_is_never_current() {
        let mut currentness = SourceRootCurrentness::new(0).expect("currentness");
        assert!(currentness.observation_gaps().is_empty());
        assert!(!currentness.mark_reconciled_synced());
        let truth = currentness.current_workspace_truth();
        assert!(!truth.source_current);
        assert!(!truth.workspace_current);
    }

    #[test]
    fn spellings_and_bounds_are_stable() {
        assert_eq!(SourceRootState::NotDirectory.code(), "not_directory");
        assert_eq!(WatcherHintKind::Overflow.as_str(), "overflow");
        assert_eq!(
            ObservationGapReason::WatcherOverflow.code(),
            "OBSERVATION_GAP_WATCHER_OVERFLOW"
        );
        assert_eq!(MAX_SOURCE_ROOT_WATCHER_HINTS, 64);
        assert_eq!(MAX_SOURCE_ROOT_OBSERVATION_GAPS, 40);
    }
}
