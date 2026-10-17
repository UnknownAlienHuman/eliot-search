//! Additive immutable-profile ingress over the one shared sparse numerical implementation.

use core::fmt;
use std::time::Instant;

use crate::sparse::{
    NumericalParameters, SparseFeatureSet, SparseFingerprint, SparseVector,
    fingerprint_bytes_checked, map_terms_parameters_checked, weight_document_parameters_checked,
    weight_query_parameters_checked,
};
use crate::{
    SparseEncoding, SparseEncodingKind, SparseEncodingReceipt, SparseError, SparseLimits,
    SparseQualification,
};

use super::{
    CodeEncodingCheckpoint, CodeIdentifierAnalysis, CodeIdentifierLimits, CodeIdentifiersProfile,
    IdentifierError, LexicalInput,
};

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

/// Closed failures from the runtime owner-profile qualification operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeProfileQualificationError {
    /// A real owner analyzer/encoder rejected a frozen fixture or limit.
    Encoding(CodeEncodingError),
    /// Actual owner output differed from its frozen expected result.
    FixtureMismatch,
    /// The resulting owner qualification record failed its identity contract.
    Qualification(SparseError),
}

impl fmt::Display for CodeProfileQualificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding(error) => error.fmt(formatter),
            Self::FixtureMismatch => {
                formatter.write_str("CODE_IDENTIFIERS_QUALIFICATION_FIXTURE_MISMATCH")
            }
            Self::Qualification(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for CodeProfileQualificationError {}
impl From<CodeEncodingError> for CodeProfileQualificationError {
    fn from(error: CodeEncodingError) -> Self {
        Self::Encoding(error)
    }
}
impl From<IdentifierError> for CodeProfileQualificationError {
    fn from(error: IdentifierError) -> Self {
        Self::Encoding(CodeEncodingError::Identifier(error))
    }
}
impl From<SparseError> for CodeProfileQualificationError {
    fn from(error: SparseError) -> Self {
        Self::Encoding(CodeEncodingError::Sparse(error))
    }
}

/// Opaque code profile issued after runtime owner qualification or a matched record.
///
/// [`accept_code_identifiers_profile`] only matches a supplied record. Runtime
/// issuance is provided by [`qualify_code_identifiers_profile`], which executes
/// the frozen owner fixtures. Neither path is release or scale qualification.
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
    /// Read the owner-produced runtime or matched external qualification record.
    #[must_use]
    pub const fn qualification(&self) -> &SparseQualification {
        &self.qualification
    }
}

/// Match all profile, artifact-set and fixture-definition identities before encoding.
///
/// # Errors
/// Refuses unaccepted or mismatched qualification records. This matcher does
/// not execute fixtures; use [`qualify_code_identifiers_profile`] for runtime
/// owner qualification.
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

/// Run the owner's frozen compatibility and collision qualification fixtures.
///
/// The returned profile is opaque and is created only after the analyzer,
/// normalization, mapping, document/query weighting and collision outputs all
/// match their frozen owner expectations. The same absolute deadline and
/// checkpoint are used for the complete run. This is runtime profile
/// qualification; it does not claim release, scale or external qualification.
///
/// # Errors
/// Refuses any fixture drift, limit/cancellation/deadline failure or invalid
/// owner profile. No partially qualified profile is returned.
pub fn qualify_code_identifiers_profile(
    profile: CodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    deadline: Instant,
    checkpoint: &mut impl FnMut() -> Result<(), SparseError>,
) -> Result<AcceptedCodeIdentifiersProfile, CodeProfileQualificationError> {
    let qualification = super::fixtures::qualify(
        &profile,
        lexical_limits,
        sparse_limits,
        deadline,
        checkpoint,
    )?;
    let accepted = accept_code_identifiers_profile(profile, qualification)
        .map_err(CodeProfileQualificationError::Qualification)?;
    let mut final_checkpoint = CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
    final_checkpoint.poll()?;
    Ok(accepted)
}

/// Complete owner-produced encoding; callers cannot replace its identity or analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeIdentifierEncoding {
    encoding: SparseEncoding,
    expansion_attempts: u64,
}

/// Shared unqualified numerical result used only while the owner executes its
/// immutable qualification fixtures. It cannot escape the owner module.
pub(super) struct EncodingParts {
    pub(super) analyzed: CodeIdentifierAnalysis,
    pub(super) input_fingerprint: SparseFingerprint,
    pub(super) features: SparseFeatureSet,
    pub(super) vector: SparseVector,
    pub(super) vector_fingerprint: SparseFingerprint,
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
    let mut observer = || {
        if cancelled {
            Err(SparseError::Cancelled)
        } else {
            Ok(())
        }
    };
    encode(
        SparseEncodingKind::Document,
        input,
        profile,
        lexical_limits,
        sparse_limits,
        None,
        &mut observer,
    )
}

/// Encode a document while observing cancellation and one immutable absolute deadline.
///
/// The observer runs synchronously after at most 64 tracked work units and at aggregate
/// allocation/result boundaries. Deadline expiry returns `SparseError::Cancelled`; an observer
/// error propagates unchanged. The observer cannot change the absolute deadline.
///
/// # Errors
/// Refuses cancellation, deadline expiry, bounds, empty vectors, collisions or invalid numerical output.
pub fn encode_code_document_with_checkpoint(
    input: &LexicalInput,
    profile: &AcceptedCodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    deadline: Instant,
    checkpoint: &mut impl FnMut() -> Result<(), SparseError>,
) -> Result<CodeIdentifierEncoding, CodeEncodingError> {
    encode(
        SparseEncodingKind::Document,
        input,
        profile,
        lexical_limits,
        sparse_limits,
        Some(deadline),
        checkpoint,
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
    let mut observer = || {
        if cancelled {
            Err(SparseError::Cancelled)
        } else {
            Ok(())
        }
    };
    encode(
        SparseEncodingKind::Query,
        input,
        profile,
        lexical_limits,
        sparse_limits,
        None,
        &mut observer,
    )
}

fn encode(
    kind: SparseEncodingKind,
    input: &LexicalInput,
    accepted: &AcceptedCodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    deadline: Option<Instant>,
    observer: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<CodeIdentifierEncoding, CodeEncodingError> {
    let profile = &accepted.profile;
    let parts = encode_parts(
        kind,
        input,
        profile,
        lexical_limits,
        sparse_limits,
        deadline,
        observer,
    )?;
    let receipt = SparseEncodingReceipt {
        kind,
        profile_id: profile.id.clone(),
        profile_revision: profile.revision,
        profile_fingerprint: profile.fingerprint,
        analyzer_fingerprint: profile.fingerprint,
        input_fingerprint: parts.input_fingerprint,
        feature_fingerprint: parts.features.feature_fingerprint,
        vector_fingerprint: parts.vector_fingerprint,
        input_bytes: parts.analyzed.analysis.receipt.input_bytes,
        emitted_tokens: parts.analyzed.analysis.receipt.emitted_token_count,
        distinct_terms: parts.features.report.distinct_terms,
        vector_values: parts.vector.indices.len(),
        collision_report: parts.features.report.clone(),
        qualification_receipt: accepted.qualification.qualification_receipt.clone(),
        statistics_digest: None,
    };
    let mut final_checkpoint = CodeEncodingCheckpoint::new(deadline, observer);
    final_checkpoint.poll()?;
    Ok(CodeIdentifierEncoding {
        encoding: SparseEncoding {
            analysis: parts.analyzed.analysis,
            features: parts.features,
            vector: parts.vector,
            receipt,
        },
        expansion_attempts: parts.analyzed.expansion_attempts,
    })
}

pub(super) fn encode_parts(
    kind: SparseEncodingKind,
    input: &LexicalInput,
    profile: &CodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    deadline: Option<Instant>,
    observer: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<EncodingParts, CodeEncodingError> {
    let mut checkpoint = CodeEncodingCheckpoint::new(deadline, observer);
    checkpoint.poll()?;
    let sparse_limits = sparse_limits.validate_code()?;
    let analyzed = profile.analyze_checked(input, lexical_limits, &mut checkpoint)?;
    // The complete input has passed byte/range/binary/original/NFC admission before hashing.
    let input_fingerprint = fingerprint_bytes_checked(input.bytes(), &mut checkpoint)?;
    let parameters = NumericalParameters::CODE_IDENTIFIERS;
    let features = map_terms_parameters_checked(
        &analyzed.analysis,
        &parameters,
        sparse_limits,
        &mut checkpoint,
    )?;
    if features.features.is_empty() {
        return Err(SparseError::EmptyVector.into());
    }
    checkpoint.poll()?;
    let vector = match kind {
        SparseEncodingKind::Document => weight_document_parameters_checked(
            &analyzed.analysis,
            &features,
            &parameters,
            None,
            sparse_limits,
            &mut checkpoint,
        )?,
        SparseEncodingKind::Query => weight_query_parameters_checked(
            &features,
            &parameters,
            None,
            sparse_limits,
            &mut checkpoint,
        )?,
    };
    vector.validate_parameters_checked(&parameters, &mut checkpoint)?;
    let vector_fingerprint = vector.fingerprint_checked(&mut checkpoint)?;
    checkpoint.poll()?;
    Ok(EncodingParts {
        analyzed,
        input_fingerprint,
        features,
        vector,
        vector_fingerprint,
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
