//! Total deterministic candidate ordering and tie-breaking.

use core::cmp::Ordering;

use search_contracts::{
    AssuranceClass, CandidateId, EvidenceRole, ObservationFreshnessState, ValidatedSearchCandidate,
};

use crate::DomainError;

/// Complete deterministic ordering key for one validated candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateOrderKey {
    /// Versioned fusion rank; lower ranks are preferred.
    pub fused_rank: u32,
    /// Stable source-local ordinal used before global identity.
    pub source_ordinal: u64,
    /// Canonical evidence role.
    pub evidence_role: EvidenceRole,
    /// Canonical evidence assurance.
    pub assurance: AssuranceClass,
    /// Canonical freshness state.
    pub freshness: ObservationFreshnessState,
    /// Final globally stable tie-breaker.
    pub candidate_id: CandidateId,
}

impl CandidateOrderKey {
    /// Builds a key only from a valid evidence-bearing candidate.
    ///
    /// # Errors
    ///
    /// Propagates forbidden candidate-reason validation failures.
    pub fn from_candidate(
        candidate: &ValidatedSearchCandidate,
        fused_rank: u32,
        source_ordinal: u64,
    ) -> Result<Self, DomainError> {
        candidate.validate().map_err(DomainError::from)?;
        Ok(Self {
            fused_rank,
            source_ordinal,
            evidence_role: candidate.evidence_role,
            assurance: candidate.assurance,
            freshness: candidate.freshness,
            candidate_id: candidate.candidate_id,
        })
    }
}

/// Total stable comparison for validated candidates.
#[must_use]
pub fn stable_candidate_order(left: &CandidateOrderKey, right: &CandidateOrderKey) -> Ordering {
    left.fused_rank
        .cmp(&right.fused_rank)
        .then_with(|| assurance_strength(right.assurance).cmp(&assurance_strength(left.assurance)))
        .then_with(|| freshness_strength(right.freshness).cmp(&freshness_strength(left.freshness)))
        .then_with(|| left.evidence_role.cmp(&right.evidence_role))
        .then_with(|| left.source_ordinal.cmp(&right.source_ordinal))
        .then_with(|| left.candidate_id.cmp(&right.candidate_id))
}

/// Sorts complete candidate keys deterministically.
pub fn stable_sort_candidates(values: &mut [CandidateOrderKey]) {
    values.sort_by(stable_candidate_order);
}

const fn assurance_strength(value: AssuranceClass) -> u8 {
    match value {
        AssuranceClass::ExactBytes => 4,
        AssuranceClass::MappedText => 3,
        AssuranceClass::LossyText => 2,
        AssuranceClass::DescriptiveOnly => 1,
    }
}

const fn freshness_strength(value: ObservationFreshnessState) -> u8 {
    match value {
        ObservationFreshnessState::CurrentConfirmed => 4,
        ObservationFreshnessState::ObservedWithAge => 3,
        ObservationFreshnessState::GapDetected => 2,
        ObservationFreshnessState::Unknown => 1,
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::{AssuranceClass, CandidateId, EvidenceRole, ObservationFreshnessState};

    use super::{CandidateOrderKey, stable_candidate_order, stable_sort_candidates};

    fn key(rank: u32, assurance: AssuranceClass, id: u8) -> CandidateOrderKey {
        CandidateOrderKey {
            fused_rank: rank,
            source_ordinal: 0,
            evidence_role: EvidenceRole::Definition,
            assurance,
            freshness: ObservationFreshnessState::CurrentConfirmed,
            candidate_id: CandidateId::from_bytes([id; 16]),
        }
    }

    #[test]
    fn final_identity_breaks_all_other_ties() {
        let a = key(1, AssuranceClass::ExactBytes, 1);
        let b = key(1, AssuranceClass::ExactBytes, 2);
        assert_eq!(stable_candidate_order(&a, &b), core::cmp::Ordering::Less);
        assert_eq!(stable_candidate_order(&b, &a), core::cmp::Ordering::Greater);
    }

    #[test]
    fn comparison_is_transitive_and_stable() {
        let a = key(1, AssuranceClass::ExactBytes, 3);
        let b = key(1, AssuranceClass::MappedText, 2);
        let c = key(2, AssuranceClass::ExactBytes, 1);
        assert!(stable_candidate_order(&a, &b).is_lt());
        assert!(stable_candidate_order(&b, &c).is_lt());
        assert!(stable_candidate_order(&a, &c).is_lt());

        let mut values = [c, b, a];
        stable_sort_candidates(&mut values);
        assert_eq!(values, [a, b, c]);
    }
}
