//! End-to-end qualified document and query sparse encoding.

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

use crate::analyzer::{
    analyze, LexicalAnalysis, LexicalInput, LexicalLimits,
};

use super::fingerprint::{SparseFingerprint, fingerprint_bytes};
use super::mapping::{CollisionReport, SparseFeatureSet, map_terms};
use super::profile::{
    AcceptedSparseProfile, DocumentTfWeighting, FrozenCorpusStatistics, IdfMode,
    SparseLimits, SparseProfile,
};
use super::vector::{SparseVector, weight_document, weight_query};
use super::SparseError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SparseEncodingKind {
    Document,
    Query,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseEncodingReceipt {
    pub kind: SparseEncodingKind,
    pub profile_id: OpaqueId,
    pub profile_revision: NonZeroRevision,
    pub profile_fingerprint: Blake3Digest32,
    pub analyzer_fingerprint: Blake3Digest32,
    pub input_fingerprint: SparseFingerprint,
    pub feature_fingerprint: SparseFingerprint,
    pub vector_fingerprint: SparseFingerprint,
    pub input_bytes: u64,
    pub emitted_tokens: u64,
    pub distinct_terms: usize,
    pub vector_values: usize,
    pub collision_report: CollisionReport,
    pub qualification_receipt: ReceiptRef,
    pub statistics_digest: Option<Blake3Digest32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SparseEncoding {
    pub analysis: LexicalAnalysis,
    pub features: SparseFeatureSet,
    pub vector: SparseVector,
    pub receipt: SparseEncodingReceipt,
}

pub fn encode_document(
    input: LexicalInput,
    profile: &AcceptedSparseProfile,
    statistics: Option<&FrozenCorpusStatistics>,
    lexical_limits: LexicalLimits,
    sparse_limits: SparseLimits,
    cancelled: bool,
) -> Result<SparseEncoding, SparseError> {
    encode(
        SparseEncodingKind::Document,
        input,
        profile,
        statistics,
        lexical_limits,
        sparse_limits,
        cancelled,
    )
}

pub fn encode_query(
    input: LexicalInput,
    profile: &AcceptedSparseProfile,
    statistics: Option<&FrozenCorpusStatistics>,
    lexical_limits: LexicalLimits,
    sparse_limits: SparseLimits,
    cancelled: bool,
) -> Result<SparseEncoding, SparseError> {
    encode(
        SparseEncodingKind::Query,
        input,
        profile,
        statistics,
        lexical_limits,
        sparse_limits,
        cancelled,
    )
}

fn encode(
    kind: SparseEncodingKind,
    input: LexicalInput,
    accepted: &AcceptedSparseProfile,
    statistics: Option<&FrozenCorpusStatistics>,
    lexical_limits: LexicalLimits,
    sparse_limits: SparseLimits,
    cancelled: bool,
) -> Result<SparseEncoding, SparseError> {
    if cancelled {
        return Err(SparseError::Cancelled);
    }
    let profile = accepted.profile();
    profile.validate()?;
    let sparse_limits = sparse_limits.validate()?;
    let input_fingerprint = fingerprint_bytes(input.bytes());
    let analysis = analyze(input, &profile.analyzer, lexical_limits)?;
    if cancelled {
        return Err(SparseError::Cancelled);
    }
    let features = map_terms(&analysis, profile, sparse_limits)?;
    if features.features.is_empty() {
        return Err(SparseError::EmptyVector);
    }
    let statistics_digest = validate_statistics(kind, profile, statistics)?;
    let vector = match kind {
        SparseEncodingKind::Document => {
            weight_document(&analysis, &features, profile, statistics, sparse_limits)?
        }
        SparseEncodingKind::Query => {
            weight_query(&features, profile, statistics, sparse_limits)?
        }
    };
    vector.validate(profile)?;
    let vector_fingerprint = vector.fingerprint();
    Ok(SparseEncoding {
        receipt: SparseEncodingReceipt {
            kind,
            profile_id: profile.profile_id.clone(),
            profile_revision: profile.revision,
            profile_fingerprint: profile.fingerprint,
            analyzer_fingerprint: profile.analyzer.fingerprint(),
            input_fingerprint,
            feature_fingerprint: features.feature_fingerprint,
            vector_fingerprint,
            input_bytes: analysis.receipt.input_bytes,
            emitted_tokens: analysis.receipt.emitted_token_count,
            distinct_terms: features.report.distinct_terms,
            vector_values: vector.indices.len(),
            collision_report: features.report.clone(),
            qualification_receipt: accepted
                .qualification()
                .qualification_receipt
                .clone(),
            statistics_digest,
        },
        analysis,
        features,
        vector,
    })
}

fn validate_statistics(
    kind: SparseEncodingKind,
    profile: &SparseProfile,
    statistics: Option<&FrozenCorpusStatistics>,
) -> Result<Option<Blake3Digest32>, SparseError> {
    let bm25_needs_length = kind == SparseEncodingKind::Document
        && matches!(profile.document_tf, DocumentTfWeighting::Bm25 { .. });
    match profile.idf_mode {
        IdfMode::DelegatedToQdrant => {
            if statistics.is_some() && !bm25_needs_length {
                return Err(SparseError::StatisticsUnexpected);
            }
        }
        IdfMode::FrozenLocal if statistics.is_none() => {
            return Err(SparseError::StatisticsRequired);
        }
        IdfMode::None if statistics.is_some() && !bm25_needs_length => {
            return Err(SparseError::StatisticsUnexpected);
        }
        IdfMode::FrozenLocal | IdfMode::None => {}
    }
    if let Some(statistics) = statistics {
        statistics.validate(profile)?;
        Ok(Some(statistics.statistics_digest))
    } else if bm25_needs_length {
        Err(SparseError::StatisticsRequired)
    } else {
        Ok(None)
    }
}
