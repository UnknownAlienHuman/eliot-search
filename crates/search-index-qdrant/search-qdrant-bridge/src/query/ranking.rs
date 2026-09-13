//! Bounded reference top-k: worst retained candidate at the heap root.

use core::cmp::Ordering;
use std::collections::BinaryHeap;

use super::CandidateNomination;
use crate::BridgeError;

#[derive(Debug)]
pub(super) struct TopCandidates {
    limit: usize,
    heap: BinaryHeap<RankedCandidate>,
}

impl TopCandidates {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            limit,
            heap: BinaryHeap::new(),
        }
    }

    pub(super) fn consider(
        &mut self,
        candidate: CandidateNomination,
    ) -> Result<(), BridgeError> {
        // Validate even a candidate that would not enter the retained top-k.
        if !candidate.score.is_finite() {
            return Err(BridgeError::InvalidScore);
        }
        let candidate = RankedCandidate(candidate);
        if self.heap.len() < self.limit {
            self.heap.push(candidate);
        } else if let Some(mut worst) = self.heap.peek_mut()
            && candidate < *worst
        {
            *worst = candidate;
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Vec<CandidateNomination> {
        self.heap
            .into_sorted_vec()
            .into_iter()
            .map(|candidate| candidate.0)
            .collect()
    }
}

// Only finite scores enter this private wrapper. Ordering is exactly the old
// full-sort order: descending score, ascending point ID, with -0.0 and +0.0
// tied. Do not use total_cmp, which would change the signed-zero tie break.
#[derive(Debug)]
struct RankedCandidate(CandidateNomination);

impl Ord for RankedCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .score
            .partial_cmp(&self.0.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.0.point_id.cmp(&other.0.point_id))
    }
}

impl PartialOrd for RankedCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for RankedCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for RankedCandidate {}

#[cfg(test)]
mod tests {
    use search_contracts::Blake3Digest32;

    use super::*;
    use crate::QdrantPointId;

    fn candidate(id: u32, score: f32) -> CandidateNomination {
        let mut bytes = [0_u8; 16];
        bytes[12..].copy_from_slice(&id.to_be_bytes());
        CandidateNomination {
            point_id: QdrantPointId(bytes),
            score,
            payload_digest: Blake3Digest32::from_bytes([1; 32]),
            identity_digest: Blake3Digest32::from_bytes([2; 32]),
        }
    }

    fn full_sort(mut values: Vec<CandidateNomination>, limit: usize) -> Vec<CandidateNomination> {
        values.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap()
                .then_with(|| left.point_id.cmp(&right.point_id))
        });
        values.truncate(limit);
        values
    }

    #[test]
    fn heap_matches_full_sort_and_never_exceeds_limit() {
        let original: Vec<_> = (0..257_u32)
            .map(|id| candidate(id, f32::from(i16::try_from(id % 17).unwrap() - 8)))
            .collect();
        for limit in [1, 2, 7, 32, 256, 257, 300] {
            let expected = full_sort(original.clone(), limit);
            for order in 0..4 {
                let mut input = original.clone();
                match order {
                    1 => input.reverse(),
                    2 => input.rotate_left(93),
                    3 => {
                        for index in 0..input.len() {
                            input.swap(index, (index * 73 + 19) % original.len());
                        }
                    }
                    _ => {}
                }
                let mut top = TopCandidates::new(limit);
                for value in input {
                    top.consider(value).unwrap();
                    assert!(top.heap.len() <= limit);
                }
                assert_eq!(top.finish(), expected, "limit={limit}, order={order}");
            }
        }
    }

    #[test]
    fn signed_zero_ties_use_point_id_not_float_bits() {
        let input = vec![
            candidate(4, 0.0),
            candidate(3, -0.0),
            candidate(2, 0.0),
            candidate(1, -0.0),
        ];
        let mut top = TopCandidates::new(2);
        for value in input.clone() {
            top.consider(value).unwrap();
        }
        let actual = top.finish();
        assert_eq!(actual, full_sort(input, 2));
        assert_eq!(actual[0].point_id, candidate(1, 0.0).point_id);
        assert_eq!(actual[1].point_id, candidate(2, 0.0).point_id);
    }

    #[test]
    fn nonfinite_candidates_fail_even_when_the_heap_is_full() {
        let expected = candidate(1, 1.0);
        let mut top = TopCandidates::new(1);
        top.consider(expected.clone()).unwrap();
        for score in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                top.consider(candidate(2, score)),
                Err(BridgeError::InvalidScore)
            );
            assert_eq!(top.heap.len(), 1);
        }
        assert_eq!(top.finish(), vec![expected]);
    }

    #[test]
    fn empty_population_returns_no_candidates() {
        assert!(TopCandidates::new(4).finish().is_empty());
    }
}
