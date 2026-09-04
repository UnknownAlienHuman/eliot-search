//! Stable term-index mapping and measured collision handling.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::collections::{BTreeMap, BTreeSet};

use crate::analyzer::LexicalAnalysis;

use super::fingerprint::{SparseFingerprint, fingerprint_bytes};
use super::profile::{CollisionPolicy, SparseLimits, SparseProfile};
use super::SparseError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseFeature {
    pub index: u32,
    pub terms: Vec<String>,
    pub frequency: u64,
    pub first_position: u64,
    pub last_position: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollisionReport {
    pub distinct_terms: usize,
    pub distinct_indexes: usize,
    pub collided_indexes: usize,
    pub collision_pairs: usize,
    pub collision_rate_ppm: u32,
    pub threshold_ppm: u32,
    pub accepted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseFeatureSet {
    pub features: Vec<SparseFeature>,
    pub report: CollisionReport,
    pub feature_fingerprint: SparseFingerprint,
}

pub fn map_terms(
    analysis: &LexicalAnalysis,
    profile: &SparseProfile,
    limits: SparseLimits,
) -> Result<SparseFeatureSet, SparseError> {
    profile.validate()?;
    let limits = limits.validate()?;
    let mut by_index = BTreeMap::<u32, SparseFeature>::new();
    let mut collision_pairs = 0_usize;

    for term in &analysis.terms {
        let index = term_index(&term.term, profile.hash_seed, profile.index_space);
        match by_index.get_mut(&index) {
            Some(feature) => {
                if feature.terms.iter().any(|value| value == &term.term) {
                    return Err(SparseError::FeatureCollision);
                }
                if profile.collision_policy == CollisionPolicy::Reject {
                    return Err(SparseError::FeatureCollision);
                }
                if feature.terms.len() >= limits.max_terms_per_index {
                    return Err(SparseError::CollisionBudgetExceeded);
                }
                collision_pairs = collision_pairs
                    .checked_add(feature.terms.len())
                    .ok_or(SparseError::CollisionBudgetExceeded)?;
                if collision_pairs > limits.max_collision_pairs {
                    return Err(SparseError::CollisionBudgetExceeded);
                }
                feature.terms.push(term.term.clone());
                feature.terms.sort();
                feature.frequency = feature
                    .frequency
                    .checked_add(term.frequency)
                    .ok_or(SparseError::FeatureBudgetExceeded)?;
                feature.first_position = feature.first_position.min(term.first_position);
                feature.last_position = feature.last_position.max(term.last_position);
            }
            None => {
                if by_index.len() >= limits.max_features {
                    return Err(SparseError::FeatureBudgetExceeded);
                }
                by_index.insert(
                    index,
                    SparseFeature {
                        index,
                        terms: vec![term.term.clone()],
                        frequency: term.frequency,
                        first_position: term.first_position,
                        last_position: term.last_position,
                    },
                );
            }
        }
    }

    let distinct_terms = analysis.terms.len();
    let distinct_indexes = by_index.len();
    let collided_indexes = by_index
        .values()
        .filter(|feature| feature.terms.len() > 1)
        .count();
    let collision_rate_ppm = collision_rate(distinct_terms, distinct_indexes)?;
    let accepted = collision_rate_ppm <= profile.maximum_collision_rate_ppm;
    let report = CollisionReport {
        distinct_terms,
        distinct_indexes,
        collided_indexes,
        collision_pairs,
        collision_rate_ppm,
        threshold_ppm: profile.maximum_collision_rate_ppm,
        accepted,
    };
    if !accepted {
        return Err(SparseError::CollisionThresholdExceeded);
    }
    let features = by_index.into_values().collect::<Vec<_>>();
    let feature_fingerprint = fingerprint_features(&features)?;
    Ok(SparseFeatureSet {
        features,
        report,
        feature_fingerprint,
    })
}

#[must_use]
pub fn term_index(term: &str, seed: u64, index_space: u32) -> u32 {
    if index_space == 0 {
        return 0;
    }
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ seed.rotate_left(17);
    for byte in term.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        hash ^= seed.rotate_right(u32::from(*byte & 31));
    }
    u32::try_from(hash % u64::from(index_space)).unwrap_or(0)
}

pub fn measure_collision_terms(
    terms: impl IntoIterator<Item = String>,
    profile: &SparseProfile,
    limits: SparseLimits,
) -> Result<CollisionReport, SparseError> {
    profile.validate()?;
    let limits = limits.validate()?;
    let terms = terms.into_iter().collect::<BTreeSet<_>>();
    if terms.len() > limits.max_features {
        return Err(SparseError::FeatureBudgetExceeded);
    }
    let mut indexes = BTreeMap::<u32, usize>::new();
    let mut pairs = 0_usize;
    for term in &terms {
        let index = term_index(term, profile.hash_seed, profile.index_space);
        let prior = indexes.get(&index).copied().unwrap_or(0);
        if prior >= limits.max_terms_per_index {
            return Err(SparseError::CollisionBudgetExceeded);
        }
        pairs = pairs
            .checked_add(prior)
            .ok_or(SparseError::CollisionBudgetExceeded)?;
        if pairs > limits.max_collision_pairs {
            return Err(SparseError::CollisionBudgetExceeded);
        }
        indexes.insert(index, prior.saturating_add(1));
    }
    let distinct_terms = terms.len();
    let distinct_indexes = indexes.len();
    let collision_rate_ppm = collision_rate(distinct_terms, distinct_indexes)?;
    Ok(CollisionReport {
        distinct_terms,
        distinct_indexes,
        collided_indexes: indexes.values().filter(|count| **count > 1).count(),
        collision_pairs: pairs,
        collision_rate_ppm,
        threshold_ppm: profile.maximum_collision_rate_ppm,
        accepted: collision_rate_ppm <= profile.maximum_collision_rate_ppm,
    })
}

fn collision_rate(
    distinct_terms: usize,
    distinct_indexes: usize,
) -> Result<u32, SparseError> {
    if distinct_terms == 0 {
        return Ok(0);
    }
    let collided_terms = distinct_terms.saturating_sub(distinct_indexes);
    let numerator = u128::try_from(collided_terms)
        .map_err(|_| SparseError::FingerprintOverflow)?
        .checked_mul(1_000_000)
        .ok_or(SparseError::FingerprintOverflow)?;
    let denominator = u128::try_from(distinct_terms)
        .map_err(|_| SparseError::FingerprintOverflow)?;
    u32::try_from(numerator / denominator)
        .map_err(|_| SparseError::FingerprintOverflow)
}

fn fingerprint_features(
    features: &[SparseFeature],
) -> Result<SparseFingerprint, SparseError> {
    let mut canonical = Vec::new();
    for feature in features {
        append(&mut canonical, &feature.index.to_be_bytes())?;
        append(&mut canonical, &feature.frequency.to_be_bytes())?;
        append(&mut canonical, &feature.first_position.to_be_bytes())?;
        append(&mut canonical, &feature.last_position.to_be_bytes())?;
        let term_count = u64::try_from(feature.terms.len())
            .map_err(|_| SparseError::FingerprintOverflow)?;
        append(&mut canonical, &term_count.to_be_bytes())?;
        for term in &feature.terms {
            let length = u64::try_from(term.len())
                .map_err(|_| SparseError::FingerprintOverflow)?;
            append(&mut canonical, &length.to_be_bytes())?;
            append(&mut canonical, term.as_bytes())?;
        }
    }
    Ok(fingerprint_bytes(&canonical))
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), SparseError> {
    output
        .len()
        .checked_add(value.len())
        .ok_or(SparseError::FingerprintOverflow)?;
    output.extend_from_slice(value);
    Ok(())
}
