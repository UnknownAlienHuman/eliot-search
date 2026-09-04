//! Closed ordinary-reclaim failures.

use core::fmt;

/// Failure returned by exact retired-point reclamation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReclaimError {
    /// Manifest contains no exact point identifiers.
    EmptyManifest,
    /// Point identifiers are duplicated or not in canonical order.
    InvalidPointSet,
    /// Manifest and committed publication proof differ.
    PublicationMismatch,
    /// Active route or epoch pins can still observe retired points.
    StillPinned,
    /// A finite point, batch, or byte limit was exceeded.
    BudgetExceeded,
    /// Requested batch index does not exist.
    BatchNotFound,
    /// Batch receipt does not match the exact plan.
    BatchReceiptMismatch,
    /// Delete may have committed but exact readback is unresolved.
    BatchOutcomeUnknown,
    /// Exact readback returned an unexpected point.
    UnexpectedReadback,
    /// Checkpoint belongs to another plan or manifest.
    CheckpointMismatch,
    /// Not every exact identifier is verified absent.
    IncompleteReclaim,
    /// Deterministic operation identity could not be represented.
    IdentityEncoding,
}

impl ReclaimError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyManifest => "RECLAIM_EMPTY_MANIFEST",
            Self::InvalidPointSet => "RECLAIM_INVALID_POINT_SET",
            Self::PublicationMismatch => "RECLAIM_PUBLICATION_MISMATCH",
            Self::StillPinned => "RECLAIM_STILL_PINNED",
            Self::BudgetExceeded => "RECLAIM_BUDGET_EXCEEDED",
            Self::BatchNotFound => "RECLAIM_BATCH_NOT_FOUND",
            Self::BatchReceiptMismatch => "RECLAIM_BATCH_RECEIPT_MISMATCH",
            Self::BatchOutcomeUnknown => "RECLAIM_BATCH_OUTCOME_UNKNOWN",
            Self::UnexpectedReadback => "RECLAIM_UNEXPECTED_READBACK",
            Self::CheckpointMismatch => "RECLAIM_CHECKPOINT_MISMATCH",
            Self::IncompleteReclaim => "RECLAIM_INCOMPLETE",
            Self::IdentityEncoding => "RECLAIM_IDENTITY_ENCODING_FAILED",
        }
    }
}

impl fmt::Display for ReclaimError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ReclaimError {}
