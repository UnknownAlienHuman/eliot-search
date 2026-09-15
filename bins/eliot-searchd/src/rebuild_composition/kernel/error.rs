//! Closed rebuild-composition failure vocabulary.

use search_epoch_pins::PinError;
use search_index_reclaimer::ReclaimError;

/// Closed rebuild-composition failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RebuildError {
    /// A finite limit is zero or internally inconsistent.
    InvalidLimits,
    /// Retained points are duplicated, unordered, or empty where prohibited.
    ManifestNotCanonical,
    /// A manifest digest does not recompute from its exact content.
    DigestMismatch,
    /// The presented route does not own the operation state.
    StaleRoute,
    /// The presented epoch is not visible, or a revision is not the successor.
    StaleRevision,
    /// A manifest generation does not match the caller generation.
    GenerationMismatch,
    /// A rebuild proposed the same generation it replaces.
    GenerationReuse,
    /// Active route or epoch pins can still observe the retired state.
    StillPinned,
    /// Backend readback does not prove the full planned point set.
    ReadbackMismatch,
    /// A staged cutover and its proof name different plans.
    CutoverMismatch,
    /// A finite point or batch budget was exceeded.
    BudgetExceeded,
    /// A retired manifest differs from its committed publication proof.
    PublicationMismatch,
    /// A pin-registry failure with no narrower rebuild mapping.
    PinDenied(PinError),
    /// An exact-reclaim failure with no narrower rebuild mapping.
    ReclaimDenied(ReclaimError),
}

impl RebuildError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "REBUILD_INVALID_LIMITS",
            Self::ManifestNotCanonical => "REBUILD_MANIFEST_NOT_CANONICAL",
            Self::DigestMismatch => "REBUILD_DIGEST_MISMATCH",
            Self::StaleRoute => "REBUILD_STALE_ROUTE",
            Self::StaleRevision => "REBUILD_STALE_REVISION",
            Self::GenerationMismatch => "REBUILD_GENERATION_MISMATCH",
            Self::GenerationReuse => "REBUILD_GENERATION_REUSE",
            Self::StillPinned => "REBUILD_STILL_PINNED",
            Self::ReadbackMismatch => "REBUILD_READBACK_MISMATCH",
            Self::CutoverMismatch => "REBUILD_CUTOVER_MISMATCH",
            Self::BudgetExceeded => "REBUILD_BUDGET_EXCEEDED",
            Self::PublicationMismatch => "REBUILD_PUBLICATION_MISMATCH",
            Self::PinDenied(inner) => inner.code(),
            Self::ReclaimDenied(inner) => inner.code(),
        }
    }
}

impl core::fmt::Display for RebuildError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RebuildError {}
