//! Sparse vector validation and deterministic weighting.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::missing_errors_doc
)]

use crate::analyzer::LexicalAnalysis;
use crate::code_identifiers::CodeEncodingCheckpoint;

use super::SparseError;
use super::fingerprint::{SparseFingerprint, fingerprint_bytes, fingerprint_bytes_checked};
use super::mapping::SparseFeatureSet;
use super::numerical::NumericalParameters;
use super::profile::{
    DocumentTfWeighting, FrozenCorpusStatistics, IdfMode, QueryTfWeighting, SparseLimits,
    SparseProfile,
};

#[derive(Clone, Debug, PartialEq)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

impl SparseVector {
    pub fn validate(&self, profile: &SparseProfile) -> Result<(), SparseError> {
        self.validate_parameters(&NumericalParameters::from_profile(profile))
    }

    pub(crate) fn validate_parameters(
        &self,
        profile: &NumericalParameters,
    ) -> Result<(), SparseError> {
        self.validate_parameters_inner(profile, None)
    }

    pub(crate) fn validate_parameters_checked(
        &self,
        profile: &NumericalParameters,
        checkpoint: &mut CodeEncodingCheckpoint<'_>,
    ) -> Result<(), SparseError> {
        self.validate_parameters_inner(profile, Some(checkpoint))
    }

    fn validate_parameters_inner(
        &self,
        profile: &NumericalParameters,
        mut checkpoint: Option<&mut CodeEncodingCheckpoint<'_>>,
    ) -> Result<(), SparseError> {
        if self.indices.is_empty() || self.indices.len() != self.values.len() {
            return Err(SparseError::EmptyVector);
        }
        checkpoint_poll(&mut checkpoint)?;
        for position in 0..self.indices.len() {
            checkpoint_worked(&mut checkpoint)?;
            let index = self.indices[position];
            let value = self.values[position];
            if index >= profile.index_space
                || (position > 0 && self.indices[position - 1] >= index)
                || !value.is_finite()
                || value <= 0.0
            {
                return Err(SparseError::NonFiniteWeight);
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn fingerprint(&self) -> SparseFingerprint {
        self.fingerprint_inner(None)
            .unwrap_or(SparseFingerprint([0; 32]))
    }

    pub(crate) fn fingerprint_checked(
        &self,
        checkpoint: &mut CodeEncodingCheckpoint<'_>,
    ) -> Result<SparseFingerprint, SparseError> {
        self.fingerprint_inner(Some(checkpoint))
    }

    fn fingerprint_inner(
        &self,
        mut checkpoint: Option<&mut CodeEncodingCheckpoint<'_>>,
    ) -> Result<SparseFingerprint, SparseError> {
        checkpoint_poll(&mut checkpoint)?;
        let mut canonical = Vec::with_capacity(self.indices.len().saturating_mul(8));
        for (index, value) in self.indices.iter().zip(&self.values) {
            checkpoint_worked(&mut checkpoint)?;
            canonical.extend_from_slice(&index.to_be_bytes());
            canonical.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        checkpoint.map_or_else(
            || Ok(fingerprint_bytes(&canonical)),
            |checkpoint| fingerprint_bytes_checked(&canonical, checkpoint),
        )
    }
}

pub(super) fn weight_document(
    analysis: &LexicalAnalysis,
    features: &SparseFeatureSet,
    profile: &SparseProfile,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
) -> Result<SparseVector, SparseError> {
    weight_document_parameters(
        analysis,
        features,
        &NumericalParameters::from_profile(profile),
        statistics,
        limits,
    )
}

fn weight_document_parameters(
    analysis: &LexicalAnalysis,
    features: &SparseFeatureSet,
    profile: &NumericalParameters,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
) -> Result<SparseVector, SparseError> {
    weight_document_parameters_inner(analysis, features, profile, statistics, limits, None)
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn weight_document_parameters_checked(
    analysis: &LexicalAnalysis,
    features: &SparseFeatureSet,
    profile: &NumericalParameters,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
    checkpoint: &mut CodeEncodingCheckpoint<'_>,
) -> Result<SparseVector, SparseError> {
    weight_document_parameters_inner(
        analysis,
        features,
        profile,
        statistics,
        limits,
        Some(checkpoint),
    )
}

fn weight_document_parameters_inner(
    analysis: &LexicalAnalysis,
    features: &SparseFeatureSet,
    profile: &NumericalParameters,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
    mut checkpoint: Option<&mut CodeEncodingCheckpoint<'_>>,
) -> Result<SparseVector, SparseError> {
    if features.features.len() > limits.max_vector_values {
        return Err(SparseError::FeatureBudgetExceeded);
    }
    checkpoint_poll(&mut checkpoint)?;
    let document_length = analysis.receipt.emitted_token_count as f64;
    let mut indices = Vec::with_capacity(features.features.len());
    let mut values = Vec::with_capacity(features.features.len());
    for feature in &features.features {
        checkpoint_worked(&mut checkpoint)?;
        let frequency = feature.frequency as f64;
        let value = match profile.document_tf {
            DocumentTfWeighting::Raw => frequency,
            DocumentTfWeighting::Logarithmic => 1.0 + frequency.ln(),
            DocumentTfWeighting::Bm25 { k1, b } => {
                let average = statistics
                    .ok_or(SparseError::StatisticsRequired)?
                    .average_document_length;
                // `mul_add` fuses rounding to a single step, so the BM25 weight
                // bits may shift by 1 ulp versus separate multiply+add
                // (feeds `value.to_bits` fingerprint).
                let denominator =
                    k1.mul_add(b.mul_add(document_length / average, 1.0 - b), frequency);
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
    weight_query_parameters(
        features,
        &NumericalParameters::from_profile(profile),
        statistics,
        limits,
    )
}

fn weight_query_parameters(
    features: &SparseFeatureSet,
    profile: &NumericalParameters,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
) -> Result<SparseVector, SparseError> {
    weight_query_parameters_inner(features, profile, statistics, limits, None)
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn weight_query_parameters_checked(
    features: &SparseFeatureSet,
    profile: &NumericalParameters,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
    checkpoint: &mut CodeEncodingCheckpoint<'_>,
) -> Result<SparseVector, SparseError> {
    weight_query_parameters_inner(features, profile, statistics, limits, Some(checkpoint))
}

fn weight_query_parameters_inner(
    features: &SparseFeatureSet,
    profile: &NumericalParameters,
    statistics: Option<&FrozenCorpusStatistics>,
    limits: SparseLimits,
    mut checkpoint: Option<&mut CodeEncodingCheckpoint<'_>>,
) -> Result<SparseVector, SparseError> {
    if features.features.len() > limits.max_vector_values {
        return Err(SparseError::FeatureBudgetExceeded);
    }
    checkpoint_poll(&mut checkpoint)?;
    let mut indices = Vec::with_capacity(features.features.len());
    let mut values = Vec::with_capacity(features.features.len());
    for feature in &features.features {
        checkpoint_worked(&mut checkpoint)?;
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
                ((document_count - document_frequency + 0.5) / (document_frequency + 0.5)).ln_1p()
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

fn checkpoint_poll(
    checkpoint: &mut Option<&mut CodeEncodingCheckpoint<'_>>,
) -> Result<(), SparseError> {
    if let Some(checkpoint) = checkpoint.as_deref_mut() {
        checkpoint.poll()?;
    }
    Ok(())
}

fn checkpoint_worked(
    checkpoint: &mut Option<&mut CodeEncodingCheckpoint<'_>>,
) -> Result<(), SparseError> {
    if let Some(checkpoint) = checkpoint.as_deref_mut() {
        checkpoint.worked()?;
    }
    Ok(())
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
