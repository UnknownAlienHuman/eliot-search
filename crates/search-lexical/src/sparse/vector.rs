//! Sparse vector validation and deterministic weighting.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::missing_errors_doc
)]

use crate::analyzer::LexicalAnalysis;

use super::fingerprint::{SparseFingerprint, fingerprint_bytes};
use super::mapping::SparseFeatureSet;
use super::profile::{
    DocumentTfWeighting, FrozenCorpusStatistics, IdfMode, QueryTfWeighting,
    SparseLimits, SparseProfile,
};
use super::SparseError;

#[derive(Clone, Debug, PartialEq)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

impl SparseVector {
    pub fn validate(&self, profile: &SparseProfile) -> Result<(), SparseError> {
        if self.indices.is_empty() || self.indices.len() != self.values.len() {
            return Err(SparseError::EmptyVector);
        }
        if self.indices.windows(2).any(|pair| pair[0] >= pair[1])
            || self.indices.iter().any(|index| *index >= profile.index_space)
            || self.values.iter().any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(SparseError::NonFiniteWeight);
        }
        Ok(())
    }

    #[must_use]
    pub fn fingerprint(&self) -> SparseFingerprint {
        let mut canonical = Vec::with_capacity(self.indices.len().saturating_mul(8));
        for (index, value) in self.indices.iter().zip(&self.values) {
            canonical.extend_from_slice(&index.to_be_bytes());
            canonical.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        fingerprint_bytes(&canonical)
    }
}

pub(super) fn weight_document(
    analysis: &LexicalAnalysis,
    features: &SparseFeatureSet,
    profile: &SparseProfile,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
) -> Result<SparseVector, SparseError> {
    if features.features.len() > limits.max_vector_values {
        return Err(SparseError::FeatureBudgetExceeded);
    }
    let document_length = analysis.receipt.emitted_token_count as f64;
    let mut indices = Vec::with_capacity(features.features.len());
    let mut values = Vec::with_capacity(features.features.len());
    for feature in &features.features {
        let frequency = feature.frequency as f64;
        let value = match profile.document_tf {
            DocumentTfWeighting::Raw => frequency,
            DocumentTfWeighting::Logarithmic => 1.0 + frequency.ln(),
            DocumentTfWeighting::Bm25 { k1, b } => {
                let average = statistics
                    .ok_or(SparseError::StatisticsRequired)?
                    .average_document_length;
                let denominator = frequency
                    + k1 * (1.0 - b + b * (document_length / average));
                frequency * (k1 + 1.0) / denominator
            }
        };
        push_weight(feature.index, value, &mut indices, &mut values)?;
    }
    Ok(SparseVector { indices, values })
}

pub(super) fn weight_query(
    features: &SparseFeatureSet,
    profile: &SparseProfile,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
) -> Result<SparseVector, SparseError> {
    if features.features.len() > limits.max_vector_values {
        return Err(SparseError::FeatureBudgetExceeded);
    }
    let mut indices = Vec::with_capacity(features.features.len());
    let mut values = Vec::with_capacity(features.features.len());
    for feature in &features.features {
        let frequency = feature.frequency as f64;
        let term_frequency = match profile.query_tf {
            QueryTfWeighting::Binary => 1.0,
            QueryTfWeighting::Raw => frequency,
            QueryTfWeighting::Logarithmic => 1.0 + frequency.ln(),
        };
        let inverse_document_frequency = match profile.idf_mode {
            IdfMode::None | IdfMode::DelegatedToQdrant => 1.0,
            IdfMode::FrozenLocal => {
                let statistics = statistics.ok_or(SparseError::StatisticsRequired)?;
                let document_frequency = statistics
                    .document_frequency
                    .get(&feature.index)
                    .copied()
                    .unwrap_or(0) as f64;
                let document_count = statistics.document_count as f64;
                (1.0 + (document_count - document_frequency + 0.5)
                    / (document_frequency + 0.5))
                    .ln()
            }
        };
        push_weight(
            feature.index,
            term_frequency * inverse_document_frequency,
            &mut indices,
            &mut values,
        )?;
    }
    Ok(SparseVector { indices, values })
}

fn push_weight(
    index: u32,
    value: f64,
    indices: &mut Vec<u32>,
    values: &mut Vec<f32>,
) -> Result<(), SparseError> {
    if !value.is_finite() || value <= 0.0 || value > f64::from(f32::MAX) {
        return Err(SparseError::NonFiniteWeight);
    }
    let value = value as f32;
    if !value.is_finite() || value <= 0.0 {
        return Err(SparseError::NonFiniteWeight);
    }
    indices.push(index);
    values.push(value);
    Ok(())
}
