//! Content-bearing continuation page and live-barrier models.

use crate::direct_store::{StoreGap, StoredMatch};

use super::spec::ContinuationError;

/// Current live-authority snapshot rechecked before every expansion.
///
/// A moved generation, revocation or purge drops the whole retained ranking;
/// possession of a token alone never authorizes expansion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveExpansionBarrier {
    /// The live generation moved past the admitted one.
    pub generation_moved: bool,
    /// The grant or security state now denies disclosure.
    pub access_revoked: bool,
    /// A purge barrier now covers the retained state.
    pub purged: bool,
}

impl LiveExpansionBarrier {
    /// Barrier observed clean: nothing moved since admission.
    #[must_use]
    pub const fn clean() -> Self {
        Self {
            generation_moved: false,
            access_revoked: false,
            purged: false,
        }
    }

    pub(super) const fn denial(self) -> Option<ContinuationError> {
        if self.purged {
            Some(ContinuationError::Purged)
        } else if self.access_revoked {
            Some(ContinuationError::AccessRevoked)
        } else if self.generation_moved {
            Some(ContinuationError::SourceFenceChanged)
        } else {
            None
        }
    }
}

/// Search-completeness flags carried by every page from one search execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageCompletion {
    pub(crate) corpus_complete: bool,
    pub(crate) match_limit_reached: bool,
}

/// Window and gap truncation flags carried by every page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageTruncation {
    pub(crate) candidate_window_truncated: bool,
    pub(crate) gap_details_truncated: bool,
}

/// Immutable coverage carried by every page from one search execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageCoverage {
    pub(crate) registered_sources: usize,
    pub(crate) active_sources: usize,
    pub(crate) searched_sources: usize,
    pub(crate) completion: PageCompletion,
    pub(crate) total_matches: usize,
    pub(crate) retained_matches: usize,
    pub(crate) gap_count: usize,
    pub(crate) truncation: PageTruncation,
}

impl PageCoverage {
    #[must_use]
    pub(crate) const fn complete(&self) -> bool {
        self.completion.corpus_complete
            && !self.truncation.candidate_window_truncated
    }
}

/// One page of deterministic source-backed matches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPage {
    pub(crate) matches: Vec<StoredMatch>,
    pub(crate) gaps: Vec<StoreGap>,
    pub(crate) coverage: PageCoverage,
    pub(crate) page_start: usize,
    pub(crate) page_end: usize,
    pub(crate) exhausted: bool,
    pub(crate) continuation_token: Option<String>,
    pub(crate) expires_in_ms: Option<u64>,
}
