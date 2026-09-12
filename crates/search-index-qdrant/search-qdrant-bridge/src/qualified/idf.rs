//! Independent-IDF admission for the qualified sparse-vector profile.

use super::QualificationError;

/// Canonical base eligibility plan shared by retrieval and the IDF corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseEligibility {
    /// Access partition identifier.
    pub access_partition: String,
    /// Tenant identifier.
    pub tenant: String,
    /// Visible epoch ceiling.
    pub visible_epoch: u64,
}

/// Independent-IDF profile descriptor: TF-only vectors plus Qdrant-side IDF.
#[derive(Clone, Debug, PartialEq)]
pub struct IndependentIdfProfile {
    /// Sparse vector name.
    pub vector_name: String,
    /// Local IDF multiplier. Must be exactly `1.0`.
    pub local_idf_factor: f32,
    /// Whether local corpus statistics were supplied. Must be false.
    pub local_statistics_present: bool,
    /// Whether Qdrant sparse-vector modifier is IDF. Must be true.
    pub qdrant_modifier_idf: bool,
    /// Retrieval eligibility filter plan.
    pub retrieval_eligibility: BaseEligibility,
    /// IDF corpus population filter plan.
    pub idf_corpus_eligibility: BaseEligibility,
}

/// Accepted independent-IDF receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndependentIdfReceipt {
    /// Accepted sparse vector name.
    pub vector_name: String,
    /// Always true: admission proves double IDF is impossible.
    pub double_idf_impossible: bool,
}

/// Admits one independent-IDF profile.
///
/// Any local IDF contribution or divergence between retrieval and IDF corpus
/// eligibility fails closed.
///
/// # Errors
///
/// Returns a stable qualification error for empty vector name, double IDF,
/// missing Qdrant modifier or divergent corpus eligibility.
pub fn admit_independent_idf(
    profile: &IndependentIdfProfile,
) -> Result<IndependentIdfReceipt, QualificationError> {
    if profile.vector_name.is_empty() {
        return Err(QualificationError::InvalidVectorName);
    }
    #[allow(clippy::float_cmp)]
    if profile.local_idf_factor != 1.0 || profile.local_statistics_present {
        return Err(QualificationError::DoubleIdf);
    }
    if !profile.qdrant_modifier_idf {
        return Err(QualificationError::IdfModifierMissing);
    }
    if profile.retrieval_eligibility != profile.idf_corpus_eligibility {
        return Err(QualificationError::CorpusEligibilityDiverged);
    }
    Ok(IndependentIdfReceipt {
        vector_name: profile.vector_name.clone(),
        double_idf_impossible: true,
    })
}
