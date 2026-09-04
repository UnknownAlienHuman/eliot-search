//! Qualified sparse profile and finite statistics.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

use crate::analyzer::AnalyzerConfig;

use super::SparseError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CollisionPolicy {
    Reject,
    MergeMeasured,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DocumentTfWeighting {
    Raw,
    Logarithmic,
    Bm25 { k1: f64, b: f64 },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum QueryTfWeighting {
    Binary,
    Raw,
    Logarithmic,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdfMode {
    None,
    DelegatedToQdrant,
    FrozenLocal,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SparseProfile {
    pub profile_id: OpaqueId,
    pub revision: NonZeroRevision,
    pub analyzer: AnalyzerConfig,
    pub index_space: u32,
    pub hash_seed: u64,
    pub collision_policy: CollisionPolicy,
    pub maximum_collision_rate_ppm: u32,
    pub document_tf: DocumentTfWeighting,
    pub query_tf: QueryTfWeighting,
    pub idf_mode: IdfMode,
    pub qdrant_idf_enabled: bool,
    pub fingerprint: Blake3Digest32,
}

impl SparseProfile {
    pub fn validate(&self) -> Result<(), SparseError> {
        if self.profile_id.as_str().is_empty()
            || self.index_space == 0
            || self.maximum_collision_rate_ppm > 1_000_000
        {
            return Err(SparseError::InvalidProfile);
        }
        match self.document_tf {
            DocumentTfWeighting::Raw | DocumentTfWeighting::Logarithmic => {}
            DocumentTfWeighting::Bm25 { k1, b }
                if k1.is_finite()
                    && b.is_finite()
                    && k1 > 0.0
                    && (0.0..=1.0).contains(&b) => {}
            DocumentTfWeighting::Bm25 { .. } => return Err(SparseError::InvalidProfile),
        }
        match (self.idf_mode, self.qdrant_idf_enabled) {
            (IdfMode::DelegatedToQdrant, true)
            | (IdfMode::FrozenLocal | IdfMode::None, false) => Ok(()),
            _ => Err(SparseError::DoubleIdf),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseQualification {
    pub profile_id: OpaqueId,
    pub profile_revision: NonZeroRevision,
    pub profile_fingerprint: Blake3Digest32,
    pub provider_artifact_digest: Blake3Digest32,
    pub compatibility_fixture_digest: Blake3Digest32,
    pub collision_fixture_digest: Blake3Digest32,
    pub accepted: bool,
    pub qualification_receipt: ReceiptRef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedSparseProfile {
    profile: SparseProfile,
    qualification: SparseQualification,
}

impl AcceptedSparseProfile {
    #[must_use]
    pub const fn profile(&self) -> &SparseProfile {
        &self.profile
    }

    #[must_use]
    pub const fn qualification(&self) -> &SparseQualification {
        &self.qualification
    }
}

pub fn validate_sparse_profile(
    profile: SparseProfile,
    qualification: SparseQualification,
) -> Result<AcceptedSparseProfile, SparseError> {
    profile.validate()?;
    if !qualification.accepted {
        return Err(SparseError::ProfileUnqualified);
    }
    if qualification.profile_id != profile.profile_id
        || qualification.profile_revision != profile.revision
        || qualification.profile_fingerprint != profile.fingerprint
    {
        return Err(SparseError::QualificationMismatch);
    }
    Ok(AcceptedSparseProfile {
        profile,
        qualification,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseLimits {
    pub max_features: usize,
    pub max_terms_per_index: usize,
    pub max_vector_values: usize,
    pub max_collision_pairs: usize,
}

impl SparseLimits {
    pub const BASELINE: Self = Self {
        max_features: 1_000_000,
        max_terms_per_index: 64,
        max_vector_values: 1_000_000,
        max_collision_pairs: 100_000,
    };

    pub const fn validate(self) -> Result<Self, SparseError> {
        if self.max_features == 0
            || self.max_terms_per_index == 0
            || self.max_vector_values == 0
            || self.max_collision_pairs == 0
        {
            Err(SparseError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrozenCorpusStatistics {
    pub document_count: u64,
    pub average_document_length: f64,
    pub document_frequency: BTreeMap<u32, u64>,
    pub statistics_digest: Blake3Digest32,
}

impl FrozenCorpusStatistics {
    pub fn validate(&self, profile: &SparseProfile) -> Result<(), SparseError> {
        if self.document_count == 0
            || !self.average_document_length.is_finite()
            || self.average_document_length <= 0.0
            || self
                .document_frequency
                .iter()
                .any(|(index, frequency)| {
                    *index >= profile.index_space
                        || *frequency == 0
                        || *frequency > self.document_count
                })
        {
            return Err(SparseError::StatisticsInvalid);
        }
        Ok(())
    }
}
