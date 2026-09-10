//! Canonical durable DIRECT spine denominator gate.
//!
//! The admitted registry freezes the entire denominator; ranked, indexed
//! top-k and client file lists never define or narrow it (invariant 6).
//! Partial and degraded outcomes stay typed incomplete, never success
//! (invariant 15). This module is mechanics only: it holds no source
//! registry, revision store, access authority or vendor client.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use core::fmt;

/// Entire-query corpus budget across all admitted sources.
///
/// Per-item limits alone are not a corpus budget: one query bounds total
/// items, total retained bytes and total emitted matches. Exhaustion yields
/// explicit typed incompleteness, never a narrowed denominator relabelled as
/// complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpineBudget {
    /// Maximum admitted items attempted by one query.
    pub max_items: usize,
    /// Maximum summed retained bytes attempted by one query.
    pub max_bytes: u64,
    /// Maximum emitted matches across every item.
    pub max_matches: usize,
}

impl SpineBudget {
    /// Validates non-zero finite ceilings.
    #[must_use]
    pub const fn validate(self) -> Option<Self> {
        if self.max_items == 0 || self.max_matches == 0 || self.max_bytes == 0 {
            None
        } else {
            Some(self)
        }
    }
}

/// Canonical entire-query budget mirroring the daemon spine gate: 100k items,
/// 1 GiB retained bytes and 100k matches.
pub const CANONICAL_SPINE_BUDGET: SpineBudget = SpineBudget {
    max_items: 100_000,
    max_bytes: 1_073_741_824,
    max_matches: 100_000,
};

/// Closed spine-denominator failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpineError {
    /// The explicitly authorized denominator is empty and the caller required
    /// a non-empty scope.
    ScopeEmpty,
    /// Duplicate revision identity under exact byte equality.
    DuplicateItem,
    /// Inventory capture cannot support the requested completeness claim.
    DenominatorIncomplete,
    /// A finite budget ceiling was exceeded.
    BudgetExhausted,
}

impl SpineError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ScopeEmpty => "SPINE_SCOPE_EMPTY",
            Self::DuplicateItem => "SPINE_DUPLICATE_ITEM",
            Self::DenominatorIncomplete => "SPINE_DENOMINATOR_INCOMPLETE",
            Self::BudgetExhausted => "SPINE_BUDGET_EXHAUSTED",
        }
    }
}

impl fmt::Display for SpineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SpineError {}

/// Frozen entire-query denominator over exact revision identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenSpineDenominator {
    /// Canonical ordered revision identities.
    pub items: Vec<String>,
    /// Explicitly omitted items; non-zero forbids a complete claim.
    pub omitted_items: u64,
    /// Items of unknown identity/availability; non-zero forbids complete.
    pub unknown_items: u64,
    /// Observation continuity is current for the declared scope.
    pub current_observation: bool,
}

impl FrozenSpineDenominator {
    /// Whether the frozen capture can represent a complete scope.
    #[must_use]
    pub const fn is_complete_capture(&self) -> bool {
        self.omitted_items == 0 && self.unknown_items == 0 && self.current_observation
    }
}

/// Freezes one authoritative denominator in deterministic order.
///
/// The admitted list is sorted and deduplicated under exact string equality;
/// duplicates fail closed. Ranked top-k subsets must never be passed here:
/// any omission is recorded explicitly and forbids a complete claim.
///
/// # Errors
/// Returns [`SpineError::DuplicateItem`] when two admitted identities are
/// exactly equal.
pub fn freeze_spine_denominator(
    mut admitted: Vec<String>,
    omitted_items: u64,
    unknown_items: u64,
    current_observation: bool,
) -> Result<FrozenSpineDenominator, SpineError> {
    admitted.sort();
    for pair in admitted.windows(2) {
        if pair[0] == pair[1] {
            return Err(SpineError::DuplicateItem);
        }
    }
    Ok(FrozenSpineDenominator {
        items: admitted,
        omitted_items,
        unknown_items,
        current_observation,
    })
}

/// Truthful spine completeness classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpineCompleteness {
    /// Every denominator item completed with zero failures and no truncation.
    CompleteScope,
    /// Matches found but the scope is not fully proven (still typed data).
    MatchesFound,
    /// No match proven only over an incomplete scope (never a complete negative).
    IncompleteNoMatch,
    /// Execution invalid: accounting contradiction.
    ExecutionInvalid,
}

/// Classifies one executed spine scan.
///
/// A complete negative requires the frozen capture to be complete, every item
/// completed, zero failures and no truncation. Anything else stays explicitly
/// incomplete; partial results are never relabelled success.
#[must_use]
pub const fn classify_spine_completeness(
    capture_complete: bool,
    denominator_items: usize,
    completed_items: usize,
    failed_items: usize,
    matched: usize,
    truncated: bool,
) -> SpineCompleteness {
    if failed_items > 0
        || truncated
        || !capture_complete
        || completed_items != denominator_items
    {
        return if matched > 0 {
            SpineCompleteness::MatchesFound
        } else {
            SpineCompleteness::IncompleteNoMatch
        };
    }
    if matched > 0 {
        SpineCompleteness::MatchesFound
    } else {
        SpineCompleteness::CompleteScope
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_denominator_is_deterministic_and_rejects_duplicates() {
        let first =
            freeze_spine_denominator(vec!["b".to_owned(), "a".to_owned()], 0, 0, true).unwrap();
        let second =
            freeze_spine_denominator(vec!["a".to_owned(), "b".to_owned()], 0, 0, true).unwrap();
        assert_eq!(first.items, vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(first, second);
        assert_eq!(
            freeze_spine_denominator(vec!["a".to_owned(), "a".to_owned()], 0, 0, true),
            Err(SpineError::DuplicateItem)
        );
    }

    #[test]
    fn topk_narrowing_never_becomes_a_complete_claim() {
        // A ranked subset that omits admitted items stays explicitly incomplete.
        let narrowed = freeze_spine_denominator(vec!["a".to_owned()], 2, 0, true).unwrap();
        assert!(!narrowed.is_complete_capture());
        assert_eq!(
            classify_spine_completeness(false, 1, 1, 0, 0, false),
            SpineCompleteness::IncompleteNoMatch
        );
        // Unknown items and observation gaps also forbid completeness.
        assert!(!freeze_spine_denominator(vec!["a".to_owned()], 0, 1, true)
            .unwrap()
            .is_complete_capture());
        assert!(!freeze_spine_denominator(vec!["a".to_owned()], 0, 0, false)
            .unwrap()
            .is_complete_capture());
    }

    #[test]
    fn partial_and_degraded_outcomes_are_never_success() {
        assert_eq!(
            classify_spine_completeness(true, 3, 2, 0, 0, false),
            SpineCompleteness::IncompleteNoMatch
        );
        assert_eq!(
            classify_spine_completeness(true, 3, 3, 1, 0, false),
            SpineCompleteness::IncompleteNoMatch
        );
        assert_eq!(
            classify_spine_completeness(true, 3, 3, 0, 0, true),
            SpineCompleteness::IncompleteNoMatch
        );
        assert_eq!(
            classify_spine_completeness(true, 3, 3, 0, 2, false),
            SpineCompleteness::MatchesFound
        );
        assert_eq!(
            classify_spine_completeness(true, 3, 3, 0, 0, false),
            SpineCompleteness::CompleteScope
        );
        // Partial matches stay typed matches, never a complete scope claim.
        assert_eq!(
            classify_spine_completeness(true, 3, 2, 0, 5, false),
            SpineCompleteness::MatchesFound
        );
    }

    #[test]
    fn canonical_budget_is_finite_and_valid() {
        assert!(CANONICAL_SPINE_BUDGET.validate().is_some());
        assert_eq!(CANONICAL_SPINE_BUDGET.max_items, 100_000);
        assert_eq!(CANONICAL_SPINE_BUDGET.max_matches, 100_000);
        assert!(SpineBudget { max_items: 0, ..CANONICAL_SPINE_BUDGET }.validate().is_none());
        assert!(SpineBudget { max_bytes: 0, ..CANONICAL_SPINE_BUDGET }.validate().is_none());
    }
}
