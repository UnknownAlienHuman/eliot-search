//! Closed publication limits, failure vocabulary and commit shapes.

use search_control_redb::publication_codec::PublicationCodecError;

/// Maximum logically retired point IDs carried by one retired manifest.
pub const MAX_RETIRED_IDS: usize = 1_024;

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
    /// Ambiguous outcome after a possible write; only recovery resolves it.
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
            PublicationCodecError::AbandonFenceMissing => {
                Self::AbandonFenceMissing
            }
            PublicationCodecError::JournalConflict => Self::JournalConflict,
            PublicationCodecError::JournalCorrupt => Self::JournalCorrupt,
            PublicationCodecError::JournalOutcomeUnknown => {
                Self::JournalOutcomeUnknown
            }
            PublicationCodecError::BudgetExceeded
            | PublicationCodecError::InvalidJournalName => Self::BudgetExceeded,
        }
    }
}

/// Closed commit shape. No boolean converts into a commit shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitKind {
    /// Full publication: observed commit advances the visible floor.
    Full,
    /// Invalidation-only finalization: no visible epoch is published.
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
