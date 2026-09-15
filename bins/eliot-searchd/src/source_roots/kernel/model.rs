//! Observation-root catalog models and content-free truth snapshots.

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceRootState {
    Available,
    Missing,
    NotDirectory,
    Unsafe,
    Unverifiable,
}

impl SourceRootState {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Missing => "missing",
            Self::NotDirectory => "not_directory",
            Self::Unsafe => "unsafe",
            Self::Unverifiable => "unverifiable",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SourceRootEntry {
    pub(super) configured_path: PathBuf,
    pub(super) state: SourceRootState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRootView {
    pub(crate) index: usize,
    pub(crate) path: String,
    pub(crate) state: SourceRootState,
}

/// Closed watcher-hint kind. Hints are dirty markers only.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatcherHintKind {
    Modified,
    Created,
    Removed,
    Rescan,
    Overflow,
}

impl WatcherHintKind {
    #[must_use]
    #[allow(dead_code)]
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

/// One bounded watcher hint inside one catalog lifetime.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatcherHint {
    pub(crate) position: usize,
    pub(crate) kind: WatcherHintKind,
    pub(crate) sequence: u64,
}

/// Typed observation-gap reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationGapReason {
    Missing,
    NotDirectory,
    Unsafe,
    Unverifiable,
    WatcherOverflow,
    UpdateOutcomeUnknown,
}

impl ObservationGapReason {
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
    pub(crate) position: usize,
    pub(crate) reason: ObservationGapReason,
    pub(crate) state: SourceRootState,
}

/// Bounded reconciliation cursor snapshot for control-state diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationCursor {
    pub(crate) generation: u64,
    pub(crate) pending_hints: usize,
    pub(crate) overflowed: bool,
    pub(crate) hint_sequence: u64,
    pub(crate) last_synced_generation: Option<u64>,
}

/// Independent source/workspace truth snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentWorkspaceTruth {
    pub(crate) configured: usize,
    pub(crate) available: usize,
    pub(crate) unavailable: usize,
    pub(crate) gap_count: usize,
    pub(crate) reconciliation_generation: u64,
    pub(crate) last_synced_generation: Option<u64>,
    pub(crate) source_current: bool,
    pub(crate) workspace_current: bool,
}
