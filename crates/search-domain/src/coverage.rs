//! Truthful coverage classification over canonical result and exact-proof records.

use search_contracts::{
    Coverage, CoverageDenominatorKind, ExactConclusion, ExactExecutionReport, LegExecutionState,
};

use crate::{DomainError, DomainErrorKind};

/// Semantic coverage classification returned by the pure kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageClassification {
    /// Exact proof accounts for the complete frozen denominator.
    CompleteScope,
    /// The requested candidate scope is represented without material gaps.
    CandidateScope,
    /// A known denominator has one or more explicit gaps.
    Partial,
    /// No authoritative denominator was established.
    Unknown,
}

/// Inputs used to classify one result's coverage claim.
#[derive(Clone, Copy, Debug)]
pub struct CoverageEvidence<'a> {
    /// Candidate/result coverage record.
    pub coverage: &'a Coverage,
    /// Exact execution proof when complete-scope coverage is claimed.
    pub exact_report: Option<&'a ExactExecutionReport>,
}

/// Classifies coverage without converting gaps or unknowns into completeness.
///
/// # Errors
///
/// Returns a typed invariant failure when `complete_scope` is claimed without
/// a valid exact report over the complete denominator.
pub fn classify_coverage(
    evidence: CoverageEvidence<'_>,
) -> Result<CoverageClassification, DomainError> {
    evidence.coverage.validate().map_err(DomainError::from)?;

    let has_material_gap = !evidence.coverage.omitted_or_failed_legs.is_empty()
        || !evidence.coverage.candidate_validation_gaps.is_empty()
        || !evidence.coverage.unknowns.is_empty()
        || evidence
            .coverage
            .executed_legs
            .iter()
            .any(|leg| leg.state != LegExecutionState::Completed);

    match evidence.coverage.denominator_kind {
        CoverageDenominatorKind::Unknown => Ok(CoverageClassification::Unknown),
        CoverageDenominatorKind::CandidateScope if has_material_gap => {
            Ok(CoverageClassification::Partial)
        }
        CoverageDenominatorKind::CandidateScope => Ok(CoverageClassification::CandidateScope),
        CoverageDenominatorKind::CompleteScope => {
            let report = evidence.exact_report.ok_or_else(|| {
                DomainError::new(
                    DomainErrorKind::InvariantViolation,
                    "coverage.complete_scope.exact_report",
                )
            })?;
            report.validate().map_err(DomainError::from)?;
            if report.coverage != CoverageDenominatorKind::CompleteScope
                || report.conclusion == ExactConclusion::Incomplete
                || has_material_gap
            {
                return Err(DomainError::new(
                    DomainErrorKind::InvariantViolation,
                    "coverage.complete_scope",
                ));
            }
            Ok(CoverageClassification::CompleteScope)
        }
    }
}

/// Returns whether an exact report permits `NO_MATCH_IN_COMPLETE_SCOPE`.
///
/// # Errors
///
/// Propagates closed contract-shape failures.
pub fn proves_complete_negative(report: &ExactExecutionReport) -> Result<bool, DomainError> {
    report.validate().map_err(DomainError::from)?;
    Ok(report.coverage == CoverageDenominatorKind::CompleteScope
        && report.conclusion == ExactConclusion::NoMatchInCompleteScope)
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        BoundedList, BoundedSet, Coverage, CoverageDenominatorKind, ObservationCursorRevision,
        ObservationFreshness, ObservationFreshnessState,
    };

    use super::{CoverageClassification, CoverageEvidence, classify_coverage};
    use crate::DomainErrorKind;

    fn empty_coverage(denominator_kind: CoverageDenominatorKind) -> Coverage {
        Coverage {
            requested_legs: BoundedList::empty(),
            executed_legs: BoundedList::empty(),
            represented_memberships: BoundedSet::empty(),
            represented_source_lineages: BoundedSet::empty(),
            omitted_or_failed_legs: BoundedList::empty(),
            candidate_validation_gaps: BoundedList::empty(),
            observation_freshness: ObservationFreshness {
                state: ObservationFreshnessState::CurrentConfirmed,
                observation_cursor_revision: ObservationCursorRevision::new(1),
                observed_age_ms: None,
            },
            unknowns: BoundedList::empty(),
            denominator_kind,
        }
    }

    #[test]
    fn candidate_scope_without_gaps_is_not_complete_scope() {
        let coverage = empty_coverage(CoverageDenominatorKind::CandidateScope);
        assert_eq!(
            classify_coverage(CoverageEvidence {
                coverage: &coverage,
                exact_report: None,
            })
            .expect("valid coverage"),
            CoverageClassification::CandidateScope
        );
    }

    #[test]
    fn complete_scope_without_exact_proof_fails_closed() {
        let coverage = empty_coverage(CoverageDenominatorKind::CompleteScope);
        let error = classify_coverage(CoverageEvidence {
            coverage: &coverage,
            exact_report: None,
        })
        .expect_err("exact proof is mandatory");
        assert_eq!(error.kind(), DomainErrorKind::InvariantViolation);
    }
}
