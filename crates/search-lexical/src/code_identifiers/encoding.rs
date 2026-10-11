//! Additive immutable-profile ingress over the one shared sparse numerical implementation.

use core::fmt;

use crate::sparse::{
    NumericalParameters, fingerprint_bytes, map_terms_parameters, weight_document_parameters,
    weight_query_parameters,
};
use crate::{
    SparseEncoding, SparseEncodingKind, SparseEncodingReceipt, SparseError, SparseLimits,
    SparseQualification,
};

use super::{CodeIdentifierLimits, CodeIdentifiersProfile, IdentifierError, LexicalInput};

/// Closed typed causes for the new route, preserving existing sparse errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeEncodingError {
    /// Original-span identifier admission or normalization failed.
    Identifier(IdentifierError),
    /// Sparse mapping, weighting, limit or cancellation failed.
    Sparse(SparseError),
}

impl fmt::Display for CodeEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier(error) => error.fmt(formatter),
            Self::Sparse(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for CodeEncodingError {}
impl From<IdentifierError> for CodeEncodingError {
    fn from(error: IdentifierError) -> Self {
        Self::Identifier(error)
    }
}
impl From<SparseError> for CodeEncodingError {
    fn from(error: SparseError) -> Self {
        Self::Sparse(error)
    }
}

/// A profile matched to the existing qualification contract, with no mutable ingress.
///
/// Matching a qualification record does not prove its execution or external acceptance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedCodeIdentifiersProfile {
    profile: CodeIdentifiersProfile,
    qualification: SparseQualification,
}

impl AcceptedCodeIdentifiersProfile {
    /// Read the internally constructed immutable profile.
    #[must_use]
    pub const fn profile(&self) -> &CodeIdentifiersProfile {
        &self.profile
    }
    /// Read the matched external qualification record.
    #[must_use]
    pub const fn qualification(&self) -> &SparseQualification {
        &self.qualification
    }
}

/// Match all profile, artifact-set and fixture-definition identities before encoding.
///
/// # Errors
/// Refuses unaccepted or mismatched qualification records.
pub fn accept_code_identifiers_profile(
    profile: CodeIdentifiersProfile,
    qualification: SparseQualification,
) -> Result<AcceptedCodeIdentifiersProfile, SparseError> {
    if !qualification.accepted {
        return Err(SparseError::ProfileUnqualified);
    }
    if qualification.profile_id != profile.id
        || qualification.profile_revision != profile.revision
        || qualification.profile_fingerprint != profile.fingerprint
        || qualification.provider_artifact_digest != profile.artifact_set
        || qualification.compatibility_fixture_digest != profile.compatibility_fixture
        || qualification.collision_fixture_digest != profile.collision_fixture
    {
        return Err(SparseError::QualificationMismatch);
    }
    Ok(AcceptedCodeIdentifiersProfile {
        profile,
        qualification,
    })
}

/// Complete owner-produced encoding; callers cannot replace its identity or analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeIdentifierEncoding {
    encoding: SparseEncoding,
    expansion_attempts: u64,
}

impl CodeIdentifierEncoding {
    /// Readonly source-backed analysis, mapped features, vector and receipt.
    #[must_use]
    pub const fn encoding(&self) -> &SparseEncoding {
        &self.encoding
    }
    /// Actual charged whole/component attempts, including filtered and duplicate terms.
    #[must_use]
    pub const fn expansion_attempts(&self) -> u64 {
        self.expansion_attempts
    }
}

/// Encode a document using the immutable fixed code profile.
///
/// # Errors
/// Refuses cancellation, bounds, empty vectors, collisions or invalid numerical output.
pub fn encode_code_document(
    input: &LexicalInput,
    profile: &AcceptedCodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    cancelled: bool,
) -> Result<CodeIdentifierEncoding, CodeEncodingError> {
    encode(
        SparseEncodingKind::Document,
        input,
        profile,
        lexical_limits,
        sparse_limits,
        cancelled,
    )
}

/// Encode a query through the same analyzer, mapping and identity as documents.
///
/// # Errors
/// Refuses cancellation, bounds, empty vectors, collisions or invalid numerical output.
pub fn encode_code_query(
    input: &LexicalInput,
    profile: &AcceptedCodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    cancelled: bool,
) -> Result<CodeIdentifierEncoding, CodeEncodingError> {
    encode(
        SparseEncodingKind::Query,
        input,
        profile,
        lexical_limits,
        sparse_limits,
        cancelled,
    )
}

fn encode(
    kind: SparseEncodingKind,
    input: &LexicalInput,
    accepted: &AcceptedCodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    cancelled: bool,
) -> Result<CodeIdentifierEncoding, CodeEncodingError> {
    if cancelled {
        return Err(SparseError::Cancelled.into());
    }
    let sparse_limits = sparse_limits.validate_code()?;
    let profile = &accepted.profile;
    let analyzed = profile.analyze(input, lexical_limits)?;
    // The complete input has passed byte/range/binary/original/NFC admission before hashing.
    let input_fingerprint = fingerprint_bytes(input.bytes());
    let parameters = NumericalParameters::CODE_IDENTIFIERS;
    let features = map_terms_parameters(&analyzed.analysis, &parameters, sparse_limits)?;
    if features.features.is_empty() {
        return Err(SparseError::EmptyVector.into());
    }
    let vector = match kind {
        SparseEncodingKind::Document => weight_document_parameters(
            &analyzed.analysis,
            &features,
            &parameters,
            None,
            sparse_limits,
        )?,
        SparseEncodingKind::Query => {
            weight_query_parameters(&features, &parameters, None, sparse_limits)?
        }
    };
    vector.validate_parameters(&parameters)?;
    let receipt = SparseEncodingReceipt {
        kind,
        profile_id: profile.id.clone(),
        profile_revision: profile.revision,
        profile_fingerprint: profile.fingerprint,
        analyzer_fingerprint: profile.fingerprint,
        input_fingerprint,
        feature_fingerprint: features.feature_fingerprint,
        vector_fingerprint: vector.fingerprint(),
        input_bytes: analyzed.analysis.receipt.input_bytes,
        emitted_tokens: analyzed.analysis.receipt.emitted_token_count,
        distinct_terms: features.report.distinct_terms,
        vector_values: vector.indices.len(),
        collision_report: features.report.clone(),
        qualification_receipt: accepted.qualification.qualification_receipt.clone(),
        statistics_digest: None,
    };
    Ok(CodeIdentifierEncoding {
        encoding: SparseEncoding {
            analysis: analyzed.analysis,
            features,
            vector,
            receipt,
        },
        expansion_attempts: analyzed.expansion_attempts,
    })
}

/// Verify a document/query pair against one owner-produced immutable profile identity.
/// Collection-generation ownership remains with the consuming projection owner.
///
/// # Errors
/// Refuses wrong kinds or any profile/analyzer identity mismatch.
pub fn validate_code_identifiers_pair(
    document: &CodeIdentifierEncoding,
    query: &CodeIdentifierEncoding,
    profile: &CodeIdentifiersProfile,
) -> Result<(), IdentifierError> {
    if document.encoding.receipt.kind != SparseEncodingKind::Document
        || query.encoding.receipt.kind != SparseEncodingKind::Query
    {
        return Err(IdentifierError::ProfileMismatch);
    }
    for receipt in [&document.encoding.receipt, &query.encoding.receipt] {
        if receipt.profile_id != profile.id
            || receipt.profile_revision != profile.revision
            || receipt.profile_fingerprint != profile.fingerprint
            || receipt.analyzer_fingerprint != profile.fingerprint
        {
            return Err(IdentifierError::ProfileMismatch);
        }
    }
    Ok(())
}
