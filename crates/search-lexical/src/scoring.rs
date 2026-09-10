//! Explicit scoring-document accounting for the IDF denominator.
//!
//! A logical scoring document may be split into several indexed units (or
//! share one [`ProjectionMembership`](search_contracts::ProjectionMembershipId)
//! with equivalent memberships). Counting raw units would silently multiply
//! one logical document in an IDF leg. This builder counts each distinct
//! [`ScoringDocumentId`] exactly once per sparse index: callers merge all
//! units of one scoring document via [`ScoringCorpusBuilder::add_unit`] (index
//! union plus checked token-length sum) and the finished
//! [`FrozenCorpusStatistics`](crate::sparse::FrozenCorpusStatistics) records
//! precisely which denominator was declared.
//!
//! The builder stores no postings, no BM25 index and no corpus text — only
//! hashed sparse indexes, per-document token counts and content-free digests.
//! IDF application itself stays delegated to the qualified Qdrant sparse path;
//! this module only makes the declared denominator explicit and bounded.

#![allow(
    clippy::cast_precision_loss,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ScoringDocumentId};

use crate::sparse::{FrozenCorpusStatistics, SparseProfile, fingerprint_bytes};

/// Canonical scoring-corpus digest domain (NUL-terminated in the preimage).
pub const SCORING_CORPUS_DOMAIN: &[u8] = b"eliot/lexical-scoring/v1";

/// Finite scoring-corpus limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoringLimits {
    /// Maximum distinct scoring documents in one declared denominator.
    pub max_scoring_documents: usize,
    /// Maximum distinct sparse indexes tracked in document frequency.
    pub max_frequency_entries: usize,
}

impl ScoringLimits {
    /// Conservative finite baseline.
    pub const BASELINE: Self = Self {
        max_scoring_documents: 1_000_000,
        max_frequency_entries: 1_048_576,
    };

    /// Validates every finite dimension as non-zero.
    pub const fn validate(self) -> Result<Self, ScoringError> {
        if self.max_scoring_documents == 0 || self.max_frequency_entries == 0 {
            Err(ScoringError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Closed scoring-accounting failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoringError {
    /// Limits are zero.
    InvalidLimits,
    /// No scoring document was declared.
    EmptyCorpus,
    /// Distinct scoring-document ceiling exceeded.
    TooManyScoringDocuments,
    /// Distinct frequency-index ceiling exceeded.
    TooManyFrequencyEntries,
    /// A sparse index is outside the bound profile index space.
    IndexOutOfBounds,
    /// A per-index document count overflowed or exceeds the denominator.
    FrequencyOverflow,
    /// A token-length sum overflowed.
    LengthOverflow,
    /// The average document length is non-finite or non-positive.
    NonFiniteAverage,
    /// Supplied indexes are not strictly increasing.
    UnsortedIndexes,
    /// Supplied indexes contain a duplicate within one call.
    DuplicateIndex,
    /// [`ScoringCorpusBuilder::add_document`] saw one scoring document twice;
    /// merge multi-unit documents with `add_unit` instead.
    DuplicateDocument,
}

impl ScoringError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "SCORING_LIMITS_INVALID",
            Self::EmptyCorpus => "SCORING_CORPUS_EMPTY",
            Self::TooManyScoringDocuments => "SCORING_TOO_MANY_DOCUMENTS",
            Self::TooManyFrequencyEntries => "SCORING_TOO_MANY_FREQUENCY_ENTRIES",
            Self::IndexOutOfBounds => "SCORING_INDEX_OUT_OF_BOUNDS",
            Self::FrequencyOverflow => "SCORING_FREQUENCY_OVERFLOW",
            Self::LengthOverflow => "SCORING_LENGTH_OVERFLOW",
            Self::NonFiniteAverage => "SCORING_AVERAGE_NON_FINITE",
            Self::UnsortedIndexes => "SCORING_INDEXES_UNSORTED",
            Self::DuplicateIndex => "SCORING_DUPLICATE_INDEX",
            Self::DuplicateDocument => "SCORING_DUPLICATE_DOCUMENT",
        }
    }
}

impl fmt::Display for ScoringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ScoringError {}

/// Content-free receipt naming exactly which denominator was declared.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoringAccountingReceipt {
    /// Bound lexical profile identity.
    pub profile_id: OpaqueId,
    /// Bound lexical profile revision.
    pub profile_revision: NonZeroRevision,
    /// Bound lexical profile fingerprint.
    pub profile_fingerprint: Blake3Digest32,
    /// Number of distinct scoring documents counted.
    pub document_count: u64,
    /// Sum of per-document token counts.
    pub total_token_count: u64,
    /// Mean tokens per scoring document.
    pub average_document_length: f64,
    /// Number of distinct sparse indexes with non-zero document frequency.
    pub frequency_entries: usize,
    /// Canonical digest over the declared denominator.
    pub statistics_digest: Blake3Digest32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DocumentAccum {
    indexes: BTreeSet<u32>,
    token_len: u64,
}

/// Explicit per-scoring-document denominator builder.
///
/// [`ScoringDocumentId`] keys are held only for the duration of the build to
/// deduplicate units; [`finish`](ScoringCorpusBuilder::finish) emits just
/// aggregated counts and a digest, never the membership list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoringCorpusBuilder {
    profile_id: OpaqueId,
    profile_revision: NonZeroRevision,
    profile_fingerprint: Blake3Digest32,
    index_space: u32,
    limits: ScoringLimits,
    documents: BTreeMap<ScoringDocumentId, DocumentAccum>,
}

impl ScoringCorpusBuilder {
    /// Binds a builder to one exact frozen profile and finite limits.
    pub fn new(profile: &SparseProfile, limits: ScoringLimits) -> Result<Self, ScoringError> {
        let limits = limits.validate()?;
        profile
            .validate()
            .map_err(|_| ScoringError::InvalidLimits)?;
        Ok(Self {
            profile_id: profile.profile_id.clone(),
            profile_revision: profile.revision,
            profile_fingerprint: profile.fingerprint,
            index_space: profile.index_space,
            limits,
            documents: BTreeMap::new(),
        })
    }

    /// Declares one whole scoring document exactly once.
    ///
    /// Fails with [`ScoringError::DuplicateDocument`] when the same scoring
    /// document was already declared: multi-unit documents must use
    /// [`add_unit`](Self::add_unit) so split units merge instead of
    /// multiplying the denominator.
    pub fn add_document(
        &mut self,
        scoring_id: ScoringDocumentId,
        sorted_unique_indexes: &[u32],
        token_count: u64,
    ) -> Result<(), ScoringError> {
        validate_indexes(sorted_unique_indexes, self.index_space)?;
        if self.documents.contains_key(&scoring_id) {
            return Err(ScoringError::DuplicateDocument);
        }
        if self.documents.len() >= self.limits.max_scoring_documents {
            return Err(ScoringError::TooManyScoringDocuments);
        }
        if sorted_unique_indexes.len() > self.limits.max_frequency_entries {
            return Err(ScoringError::TooManyFrequencyEntries);
        }
        self.documents.insert(
            scoring_id,
            DocumentAccum {
                indexes: sorted_unique_indexes.iter().copied().collect(),
                token_len: token_count,
            },
        );
        Ok(())
    }

    /// Merges one unit into its logical scoring document.
    ///
    /// Repeated calls with the same [`ScoringDocumentId`] union their sparse
    /// indexes and sum their token counts with checked arithmetic, so a
    /// document split into units declares the same denominator as the whole
    /// document encoded at once.
    pub fn add_unit(
        &mut self,
        scoring_id: ScoringDocumentId,
        sorted_unique_indexes: &[u32],
        token_count: u64,
    ) -> Result<(), ScoringError> {
        validate_indexes(sorted_unique_indexes, self.index_space)?;
        if let Some(existing) = self.documents.get_mut(&scoring_id) {
            let mut merged_len = existing.indexes.len();
            for index in sorted_unique_indexes {
                if !existing.indexes.contains(index) {
                    merged_len = merged_len
                        .checked_add(1)
                        .ok_or(ScoringError::TooManyFrequencyEntries)?;
                    if merged_len > self.limits.max_frequency_entries {
                        return Err(ScoringError::TooManyFrequencyEntries);
                    }
                }
            }
            existing
                .indexes
                .extend(sorted_unique_indexes.iter().copied());
            existing.token_len = existing
                .token_len
                .checked_add(token_count)
                .ok_or(ScoringError::LengthOverflow)?;
            return Ok(());
        }
        if self.documents.len() >= self.limits.max_scoring_documents {
            return Err(ScoringError::TooManyScoringDocuments);
        }
        if sorted_unique_indexes.len() > self.limits.max_frequency_entries {
            return Err(ScoringError::TooManyFrequencyEntries);
        }
        self.documents.insert(
            scoring_id,
            DocumentAccum {
                indexes: sorted_unique_indexes.iter().copied().collect(),
                token_len: token_count,
            },
        );
        Ok(())
    }

    /// Number of distinct scoring documents declared so far.
    #[must_use]
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Finishes the declared denominator into frozen statistics plus receipt.
    pub fn finish(
        self,
    ) -> Result<(FrozenCorpusStatistics, ScoringAccountingReceipt), ScoringError> {
        if self.documents.is_empty() {
            return Err(ScoringError::EmptyCorpus);
        }
        let document_count =
            u64::try_from(self.documents.len()).map_err(|_| ScoringError::FrequencyOverflow)?;
        let mut total_tokens = 0_u64;
        for document in self.documents.values() {
            total_tokens = total_tokens
                .checked_add(document.token_len)
                .ok_or(ScoringError::LengthOverflow)?;
        }
        let average = total_tokens as f64 / document_count as f64;
        if !average.is_finite() || average <= 0.0 {
            return Err(ScoringError::NonFiniteAverage);
        }
        let mut frequency = BTreeMap::<u32, u64>::new();
        for document in self.documents.values() {
            for index in &document.indexes {
                if *index >= self.index_space {
                    return Err(ScoringError::IndexOutOfBounds);
                }
                let next = frequency
                    .get(index)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or(ScoringError::FrequencyOverflow)?;
                if next > document_count {
                    return Err(ScoringError::FrequencyOverflow);
                }
                frequency.insert(*index, next);
            }
        }
        if frequency.len() > self.limits.max_frequency_entries {
            return Err(ScoringError::TooManyFrequencyEntries);
        }
        let statistics_digest = scoring_digest(document_count, average, &frequency)?;
        let statistics = FrozenCorpusStatistics {
            document_count,
            average_document_length: average,
            document_frequency: frequency,
            statistics_digest,
        };
        let frequency_entries = statistics.document_frequency.len();
        let receipt = ScoringAccountingReceipt {
            profile_id: self.profile_id,
            profile_revision: self.profile_revision,
            profile_fingerprint: self.profile_fingerprint,
            document_count,
            total_token_count: total_tokens,
            average_document_length: average,
            frequency_entries,
            statistics_digest,
        };
        Ok((statistics, receipt))
    }
}

fn validate_indexes(indexes: &[u32], index_space: u32) -> Result<(), ScoringError> {
    let mut previous: Option<u32> = None;
    for index in indexes {
        if *index >= index_space {
            return Err(ScoringError::IndexOutOfBounds);
        }
        if let Some(previous) = previous {
            if *index == previous {
                return Err(ScoringError::DuplicateIndex);
            }
            if *index < previous {
                return Err(ScoringError::UnsortedIndexes);
            }
        }
        previous = Some(*index);
    }
    Ok(())
}

fn scoring_digest(
    document_count: u64,
    average: f64,
    frequency: &BTreeMap<u32, u64>,
) -> Result<Blake3Digest32, ScoringError> {
    let pairs = frequency.len();
    let capacity = SCORING_CORPUS_DOMAIN
        .len()
        .checked_add(8)
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(8))
        .and_then(|value| {
            value.checked_add(
                pairs
                    .checked_mul(12)
                    .ok_or(ScoringError::FrequencyOverflow)
                    .ok()?,
            )
        })
        .ok_or(ScoringError::FrequencyOverflow)?;
    let mut canonical = Vec::with_capacity(capacity);
    canonical.extend_from_slice(SCORING_CORPUS_DOMAIN);
    canonical.extend_from_slice(&[0_u8]);
    canonical.extend_from_slice(&document_count.to_be_bytes());
    canonical.extend_from_slice(&average.to_bits().to_be_bytes());
    let pair_count = u64::try_from(pairs).map_err(|_| ScoringError::FrequencyOverflow)?;
    canonical.extend_from_slice(&pair_count.to_be_bytes());
    for (index, count) in frequency {
        canonical.extend_from_slice(&index.to_be_bytes());
        canonical.extend_from_slice(&count.to_be_bytes());
    }
    Ok(Blake3Digest32::from_bytes(fingerprint_bytes(&canonical).0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frozen::{code_v1_sparse_profile, text_neutral_v1_sparse_profile};

    fn scoring_id(byte: u8) -> ScoringDocumentId {
        ScoringDocumentId::from_bytes([byte; 16])
    }

    fn profile() -> SparseProfile {
        code_v1_sparse_profile().expect("frozen code profile")
    }

    #[test]
    fn split_units_merge_to_whole_document_denominator() {
        let profile = profile();
        // Whole document at once.
        let mut whole =
            ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
        whole
            .add_document(scoring_id(1), &[3, 7, 11], 9)
            .expect("whole");
        let (whole_stats, whole_receipt) = whole.finish().expect("finish whole");
        // Same logical document split into two units merges identically.
        let mut split =
            ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
        split.add_unit(scoring_id(1), &[3, 7], 4).expect("unit one");
        split
            .add_unit(scoring_id(1), &[7, 11], 5)
            .expect("unit two");
        let (split_stats, split_receipt) = split.finish().expect("finish split");
        assert_eq!(whole_stats, split_stats);
        assert_eq!(
            whole_receipt.statistics_digest,
            split_receipt.statistics_digest
        );
        assert_eq!(whole_receipt.document_count, 1);
        assert_eq!(whole_receipt.total_token_count, 9);
        // One logical document contributes once per index even with overlap.
        assert_eq!(split_stats.document_frequency.get(&7), Some(&1));
    }

    #[test]
    fn shared_index_counts_documents_not_occurrences() {
        let profile = profile();
        let mut builder =
            ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
        builder
            .add_document(scoring_id(1), &[5, 9], 4)
            .expect("doc one");
        builder
            .add_document(scoring_id(2), &[5, 13], 6)
            .expect("doc two");
        let (statistics, receipt) = builder.finish().expect("finish");
        assert_eq!(statistics.document_count, 2);
        assert_eq!(statistics.document_frequency.get(&5), Some(&2));
        assert_eq!(statistics.document_frequency.get(&9), Some(&1));
        assert_eq!(receipt.frequency_entries, 3);
        assert_eq!(receipt.total_token_count, 10);
        assert!((receipt.average_document_length - 5.0).abs() < f64::EPSILON);
        statistics
            .validate(&profile)
            .expect("statistics validate against bound profile");
    }

    #[test]
    fn duplicate_document_fails_instead_of_multiplying_denominator() {
        let profile = profile();
        let mut builder =
            ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
        builder.add_document(scoring_id(1), &[5], 2).expect("first");
        assert_eq!(
            builder.add_document(scoring_id(1), &[5], 2),
            Err(ScoringError::DuplicateDocument)
        );
    }

    #[test]
    fn receipt_binds_exact_frozen_profile() {
        let profile = text_neutral_v1_sparse_profile().expect("text profile");
        let mut builder =
            ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
        builder
            .add_document(scoring_id(9), &[1, 2], 3)
            .expect("doc");
        let (_, receipt) = builder.finish().expect("finish");
        assert_eq!(receipt.profile_id, profile.profile_id);
        assert_eq!(receipt.profile_revision, profile.revision);
        assert_eq!(receipt.profile_fingerprint, profile.fingerprint);
    }

    #[test]
    fn scoring_digest_is_deterministic() {
        let profile = profile();
        let build = || {
            let mut builder =
                ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
            builder
                .add_document(scoring_id(2), &[11, 30], 5)
                .expect("doc two");
            builder
                .add_document(scoring_id(1), &[7, 11], 3)
                .expect("doc one");
            builder.finish().expect("finish")
        };
        let (first_stats, first_receipt) = build();
        let (second_stats, second_receipt) = build();
        assert_eq!(first_stats, second_stats);
        assert_eq!(first_receipt, second_receipt);
    }

    #[test]
    fn scoring_failures_are_typed_and_bounded() {
        let profile = profile();
        // Empty corpus.
        let empty = ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
        assert_eq!(empty.finish().map(|_| ()), Err(ScoringError::EmptyCorpus));
        // Document ceiling.
        let tiny = ScoringLimits {
            max_scoring_documents: 1,
            max_frequency_entries: 1_048_576,
        };
        let mut builder = ScoringCorpusBuilder::new(&profile, tiny).expect("builder");
        builder.add_document(scoring_id(1), &[1], 1).expect("first");
        assert_eq!(
            builder.add_document(scoring_id(2), &[2], 1),
            Err(ScoringError::TooManyScoringDocuments)
        );
        // Index bounds, ordering and intra-call duplicates.
        let mut builder =
            ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
        assert_eq!(
            builder.add_document(scoring_id(1), &[profile.index_space], 1),
            Err(ScoringError::IndexOutOfBounds)
        );
        assert_eq!(
            builder.add_document(scoring_id(1), &[9, 4], 1),
            Err(ScoringError::UnsortedIndexes)
        );
        assert_eq!(
            builder.add_document(scoring_id(1), &[4, 4], 1),
            Err(ScoringError::DuplicateIndex)
        );
        // Zero limits rejected.
        assert_eq!(
            ScoringLimits {
                max_scoring_documents: 0,
                max_frequency_entries: 1,
            }
            .validate()
            .map(|_| ()),
            Err(ScoringError::InvalidLimits)
        );
        // All-empty documents make the average non-positive, not a silent zero.
        let mut builder =
            ScoringCorpusBuilder::new(&profile, ScoringLimits::BASELINE).expect("builder");
        builder
            .add_document(scoring_id(1), &[], 0)
            .expect("empty doc");
        assert_eq!(
            builder.finish().map(|_| ()),
            Err(ScoringError::NonFiniteAverage)
        );
    }

    #[test]
    fn scoring_error_codes_are_stable() {
        assert_eq!(ScoringError::EmptyCorpus.code(), "SCORING_CORPUS_EMPTY");
        assert_eq!(
            ScoringError::DuplicateDocument.code(),
            "SCORING_DUPLICATE_DOCUMENT"
        );
        assert_eq!(
            ScoringError::IndexOutOfBounds.code(),
            "SCORING_INDEX_OUT_OF_BOUNDS"
        );
    }
}
