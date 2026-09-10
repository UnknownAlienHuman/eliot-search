//! Frozen T25 lexical/sparse profiles (`code_v1`, `text_neutral_v1`).
//!
//! This module owns the exact frozen behavior identity for the two accepted
//! lexical legs. It stores no corpus and implements no inverted index: it only
//! pins analyzer parameters, sparse term-index parameters and the canonical
//! digest that binds them. Any tokenizer, normalization, mapping, weighting,
//! IDF, collision, artifact or fixture change yields a different digest and
//! requires a new collection generation.
//!
//! Profile fingerprints reuse the package-local deterministic
//! [`fingerprint_bytes`](crate::sparse::fingerprint_bytes) hash over
//! length-prefixed canonical bytes under the `eliot/lexical-profile/v1`
//! domain, stored as [`Blake3Digest32`]. The frozen constants below are the
//! pinned digests; [`verify_frozen_profile`] recomputes the digest from the
//! live parameters and fails closed on any drift.

#![allow(
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::too_many_lines
)]

use core::fmt;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

use crate::analyzer::{
    AnalyzerConfig, CaseNormalization, DEFAULT_LEXICAL_LIMITS, TokenCharacterPolicy,
};
use crate::sparse::{
    AcceptedSparseProfile, CollisionPolicy, DocumentTfWeighting, IdfMode, QueryTfWeighting,
    SparseProfile, SparseQualification, fingerprint_bytes,
};

/// Canonical profile digest domain (NUL-terminated in the preimage).
pub const FROZEN_PROFILE_DOMAIN: &[u8] = b"eliot/lexical-profile/v1";

/// Frozen code-leg profile identity.
pub const CODE_V1_PROFILE_ID: &str = "lexical:code_v1";
/// Frozen neutral-text-leg profile identity.
pub const TEXT_NEUTRAL_V1_PROFILE_ID: &str = "lexical:text_neutral_v1";
/// Frozen code-leg revision.
pub const CODE_V1_REVISION: u64 = 1;
/// Frozen neutral-text-leg revision.
pub const TEXT_NEUTRAL_V1_REVISION: u64 = 1;
/// Frozen sparse index space for both legs.
pub const FROZEN_INDEX_SPACE: u32 = 1_048_576;
/// Frozen hash seed for the code leg.
pub const CODE_V1_HASH_SEED: u64 = 0x9E37_79B9_7F4A_7C15;
/// Frozen hash seed for the neutral-text leg.
pub const TEXT_NEUTRAL_V1_HASH_SEED: u64 = 0xC2B2_AE3D_27D4_EB4F;
/// Frozen collision-rate ceiling (10 000 ppm = 1%).
pub const FROZEN_MAX_COLLISION_RATE_PPM: u32 = 10_000;
/// Frozen minimum token length (Unicode scalars) for both legs.
pub const FROZEN_MIN_TOKEN_CHARS: usize = 2;

/// Pinned code-leg profile digest (see module docs).
pub const CODE_V1_FINGERPRINT: Blake3Digest32 = Blake3Digest32::from_bytes([
    0xa3, 0x59, 0xc0, 0x2c, 0x8c, 0x74, 0xec, 0xe6, 0x78, 0xab, 0x34, 0x41, 0xd4, 0xc5, 0x8f, 0x4f,
    0xa1, 0xe5, 0xf7, 0x82, 0x07, 0x50, 0x83, 0x70, 0x1d, 0x6d, 0x5a, 0xc1, 0xb1, 0x00, 0x55, 0x9c,
]);
/// Pinned neutral-text-leg profile digest (see module docs).
pub const TEXT_NEUTRAL_V1_FINGERPRINT: Blake3Digest32 = Blake3Digest32::from_bytes([
    0x47, 0x7c, 0x2c, 0x10, 0x68, 0x8a, 0xdd, 0x3c, 0x5c, 0xc1, 0x71, 0x4e, 0x8f, 0x66, 0xfb, 0xd2,
    0xec, 0xe6, 0x25, 0x97, 0xc6, 0x7a, 0x48, 0xcc, 0xae, 0x37, 0xaa, 0x06, 0xe9, 0xe8, 0xdd, 0xef,
]);

/// Upper bound for canonical profile preimage bytes (fail-closed).
const MAX_CANONICAL_BYTES: usize = 1_048_576;

/// Closed frozen-profile failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrozenProfileError {
    /// A frozen identifier or revision is malformed.
    InvalidIdentity,
    /// The frozen analyzer parameters are internally inconsistent.
    InvalidAnalyzer(crate::analyzer::LexicalError),
    /// The frozen sparse parameters are internally inconsistent.
    InvalidProfile(crate::sparse::SparseError),
    /// Recomputed digest differs from the pinned fingerprint: the profile
    /// changed and requires a new collection generation.
    DigestMismatch,
    /// Canonical preimage growth overflowed its finite bound.
    CanonicalOverflow,
}

impl FrozenProfileError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidIdentity => "FROZEN_PROFILE_INVALID_IDENTITY",
            Self::InvalidAnalyzer(_) => "FROZEN_PROFILE_INVALID_ANALYZER",
            Self::InvalidProfile(_) => "FROZEN_PROFILE_INVALID_PROFILE",
            Self::DigestMismatch => "FROZEN_PROFILE_DIGEST_MISMATCH",
            Self::CanonicalOverflow => "FROZEN_PROFILE_CANONICAL_OVERFLOW",
        }
    }
}

impl fmt::Display for FrozenProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAnalyzer(error) => {
                write!(formatter, "{}:{}", self.code(), error.code())
            }
            Self::InvalidProfile(error) => {
                write!(formatter, "{}:{}", self.code(), error.code())
            }
            _ => formatter.write_str(self.code()),
        }
    }
}

impl std::error::Error for FrozenProfileError {}

/// Content-free frozen profile descriptor (no analyzer dictionary bytes).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenProfileDescriptor {
    /// Frozen profile identity.
    pub profile_id: OpaqueId,
    /// Frozen profile revision.
    pub revision: NonZeroRevision,
    /// Pinned profile digest.
    pub fingerprint: Blake3Digest32,
    /// Analyzer configuration fingerprint (equals profile digest when frozen).
    pub analyzer_fingerprint: Blake3Digest32,
    /// Frozen sparse index space.
    pub index_space: u32,
    /// Frozen hash seed.
    pub hash_seed: u64,
}

/// Builds the frozen code-leg analyzer configuration.
pub fn code_v1_analyzer() -> Result<AnalyzerConfig, FrozenProfileError> {
    frozen_analyzer(
        CODE_V1_PROFILE_ID,
        CODE_V1_REVISION,
        TokenCharacterPolicy::UnicodeAlphanumericAndUnderscore,
        CODE_V1_FINGERPRINT,
    )
}

/// Builds the frozen neutral-text-leg analyzer configuration.
pub fn text_neutral_v1_analyzer() -> Result<AnalyzerConfig, FrozenProfileError> {
    frozen_analyzer(
        TEXT_NEUTRAL_V1_PROFILE_ID,
        TEXT_NEUTRAL_V1_REVISION,
        TokenCharacterPolicy::UnicodeAlphanumeric,
        TEXT_NEUTRAL_V1_FINGERPRINT,
    )
}

/// Builds the frozen code-leg sparse profile.
pub fn code_v1_sparse_profile() -> Result<SparseProfile, FrozenProfileError> {
    frozen_sparse_profile(
        CODE_V1_PROFILE_ID,
        CODE_V1_REVISION,
        CODE_V1_HASH_SEED,
        CODE_V1_FINGERPRINT,
        code_v1_analyzer()?,
    )
}

/// Builds the frozen neutral-text-leg sparse profile.
pub fn text_neutral_v1_sparse_profile() -> Result<SparseProfile, FrozenProfileError> {
    frozen_sparse_profile(
        TEXT_NEUTRAL_V1_PROFILE_ID,
        TEXT_NEUTRAL_V1_REVISION,
        TEXT_NEUTRAL_V1_HASH_SEED,
        TEXT_NEUTRAL_V1_FINGERPRINT,
        text_neutral_v1_analyzer()?,
    )
}

/// Describes a frozen profile without exposing dictionary content.
pub fn describe_frozen_profile(profile: &SparseProfile) -> FrozenProfileDescriptor {
    FrozenProfileDescriptor {
        profile_id: profile.profile_id.clone(),
        revision: profile.revision,
        fingerprint: profile.fingerprint,
        analyzer_fingerprint: profile.analyzer.fingerprint(),
        index_space: profile.index_space,
        hash_seed: profile.hash_seed,
    }
}

/// Recomputes the canonical digest of a sparse profile.
pub fn frozen_profile_digest(
    profile: &SparseProfile,
) -> Result<Blake3Digest32, FrozenProfileError> {
    let canonical = frozen_canonical_bytes(profile)?;
    Ok(Blake3Digest32::from_bytes(fingerprint_bytes(&canonical).0))
}

/// Verifies a profile is byte-identical to the pinned frozen digest.
///
/// Any behavior change makes the recomputed digest differ from the stored
/// fingerprint and fails with [`FrozenProfileError::DigestMismatch`]; the
/// caller must then mint a new profile identity/revision (new collection
/// generation) instead of silently reusing the old one.
pub fn verify_frozen_profile(
    profile: &SparseProfile,
    expected: Blake3Digest32,
) -> Result<(), FrozenProfileError> {
    profile
        .validate()
        .map_err(FrozenProfileError::InvalidProfile)?;
    let recomputed = frozen_profile_digest(profile)?;
    if recomputed != profile.fingerprint
        || profile.fingerprint != expected
        || profile.analyzer.fingerprint() != profile.fingerprint
    {
        return Err(FrozenProfileError::DigestMismatch);
    }
    Ok(())
}

/// Accepts a frozen profile against a matching qualification receipt.
///
/// Enforces freeze self-consistency first (recomputed digest must equal the
/// stored fingerprint on both the sparse profile and its analyzer), then
/// delegates to [`validate_sparse_profile`](crate::sparse::validate_sparse_profile)
/// so `latest`, implicit defaults and partially specified profiles stay
/// rejected.
pub fn accept_frozen_profile(
    profile: SparseProfile,
    qualification: SparseQualification,
) -> Result<AcceptedSparseProfile, FrozenProfileError> {
    profile
        .validate()
        .map_err(FrozenProfileError::InvalidProfile)?;
    let recomputed = frozen_profile_digest(&profile)?;
    if recomputed != profile.fingerprint || profile.analyzer.fingerprint() != profile.fingerprint {
        return Err(FrozenProfileError::DigestMismatch);
    }
    crate::sparse::validate_sparse_profile(profile, qualification)
        .map_err(FrozenProfileError::InvalidProfile)
}

/// Canonical length-prefixed preimage for a sparse profile (excludes the
/// stored fingerprint itself so the digest is well-founded).
pub fn frozen_canonical_bytes(profile: &SparseProfile) -> Result<Vec<u8>, FrozenProfileError> {
    let mut output = Vec::new();
    append(&mut output, FROZEN_PROFILE_DOMAIN)?;
    append_prefixed(&mut output, profile.profile_id.as_str().as_bytes())?;
    append(&mut output, &profile.revision.get().to_be_bytes())?;
    append_prefixed(
        &mut output,
        profile.analyzer.analyzer_id.as_str().as_bytes(),
    )?;
    append(&mut output, &profile.analyzer.revision.get().to_be_bytes())?;
    let character_policy = match profile.analyzer.character_policy {
        TokenCharacterPolicy::UnicodeAlphanumericAndUnderscore => 0_u8,
        TokenCharacterPolicy::UnicodeAlphanumeric => 1_u8,
    };
    append(&mut output, &[character_policy])?;
    let case = match profile.analyzer.case_normalization {
        CaseNormalization::Preserve => 0_u8,
        CaseNormalization::UnicodeLowercase => 1_u8,
    };
    append(&mut output, &[case])?;
    let min_token_chars = u64::try_from(profile.analyzer.min_token_chars)
        .map_err(|_| FrozenProfileError::CanonicalOverflow)?;
    append(&mut output, &min_token_chars.to_be_bytes())?;
    append(
        &mut output,
        &[u8::from(profile.analyzer.preserve_stop_word_positions)],
    )?;
    let stop_words = profile.analyzer.stop_words().collect::<Vec<_>>();
    let stop_count =
        u64::try_from(stop_words.len()).map_err(|_| FrozenProfileError::CanonicalOverflow)?;
    append(&mut output, &stop_count.to_be_bytes())?;
    for stop_word in stop_words {
        append_prefixed(&mut output, stop_word.as_bytes())?;
    }
    append(&mut output, &profile.index_space.to_be_bytes())?;
    append(&mut output, &profile.hash_seed.to_be_bytes())?;
    let collision = match profile.collision_policy {
        CollisionPolicy::Reject => 0_u8,
        CollisionPolicy::MergeMeasured => 1_u8,
    };
    append(&mut output, &[collision])?;
    append(
        &mut output,
        &profile.maximum_collision_rate_ppm.to_be_bytes(),
    )?;
    match profile.document_tf {
        DocumentTfWeighting::Raw => append(&mut output, &[0_u8])?,
        DocumentTfWeighting::Logarithmic => append(&mut output, &[1_u8])?,
        DocumentTfWeighting::Bm25 { k1, b } => {
            append(&mut output, &[2_u8])?;
            append(&mut output, &k1.to_bits().to_be_bytes())?;
            append(&mut output, &b.to_bits().to_be_bytes())?;
        }
    }
    let query_tf = match profile.query_tf {
        QueryTfWeighting::Binary => 0_u8,
        QueryTfWeighting::Raw => 1_u8,
        QueryTfWeighting::Logarithmic => 2_u8,
    };
    append(&mut output, &[query_tf])?;
    let idf = match profile.idf_mode {
        IdfMode::None => 0_u8,
        IdfMode::DelegatedToQdrant => 1_u8,
        IdfMode::FrozenLocal => 2_u8,
    };
    append(&mut output, &[idf])?;
    append(&mut output, &[u8::from(profile.qdrant_idf_enabled)])?;
    Ok(output)
}

fn frozen_analyzer(
    id: &str,
    revision: u64,
    character_policy: TokenCharacterPolicy,
    fingerprint: Blake3Digest32,
) -> Result<AnalyzerConfig, FrozenProfileError> {
    let analyzer_id = OpaqueId::new(id).map_err(|_| FrozenProfileError::InvalidIdentity)?;
    let revision =
        NonZeroRevision::new(revision).map_err(|_| FrozenProfileError::InvalidIdentity)?;
    AnalyzerConfig::new(
        analyzer_id,
        revision,
        character_policy,
        CaseNormalization::UnicodeLowercase,
        FROZEN_MIN_TOKEN_CHARS,
        true,
        Vec::<String>::new(),
        fingerprint,
        DEFAULT_LEXICAL_LIMITS,
    )
    .map_err(FrozenProfileError::InvalidAnalyzer)
}

fn frozen_sparse_profile(
    id: &str,
    revision: u64,
    hash_seed: u64,
    fingerprint: Blake3Digest32,
    analyzer: AnalyzerConfig,
) -> Result<SparseProfile, FrozenProfileError> {
    let profile = SparseProfile {
        profile_id: OpaqueId::new(id).map_err(|_| FrozenProfileError::InvalidIdentity)?,
        revision: NonZeroRevision::new(revision)
            .map_err(|_| FrozenProfileError::InvalidIdentity)?,
        analyzer,
        index_space: FROZEN_INDEX_SPACE,
        hash_seed,
        collision_policy: CollisionPolicy::MergeMeasured,
        maximum_collision_rate_ppm: FROZEN_MAX_COLLISION_RATE_PPM,
        document_tf: DocumentTfWeighting::Logarithmic,
        query_tf: QueryTfWeighting::Logarithmic,
        idf_mode: IdfMode::DelegatedToQdrant,
        qdrant_idf_enabled: true,
        fingerprint,
    };
    profile
        .validate()
        .map_err(FrozenProfileError::InvalidProfile)?;
    Ok(profile)
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), FrozenProfileError> {
    if output
        .len()
        .checked_add(value.len())
        .is_none_or(|next| next > MAX_CANONICAL_BYTES)
    {
        return Err(FrozenProfileError::CanonicalOverflow);
    }
    output.extend_from_slice(value);
    Ok(())
}

fn append_prefixed(output: &mut Vec<u8>, value: &[u8]) -> Result<(), FrozenProfileError> {
    let length = u64::try_from(value.len()).map_err(|_| FrozenProfileError::CanonicalOverflow)?;
    append(output, &length.to_be_bytes())?;
    append(output, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{LexicalInput, LexicalLimits, analyze};
    use crate::sparse::{
        SparseLimits, SparseQualification, encode_document, encode_query, validate_sparse_profile,
    };
    use search_contracts::{OpaqueId, ReceiptRef};

    fn test_input(text: &str) -> LexicalInput {
        LexicalInput::new(
            OpaqueId::new("source:test").expect("source id"),
            NonZeroRevision::new(1).expect("revision"),
            0,
            500,
            500 + u64::try_from(text.len()).expect("text length"),
            text.to_owned(),
        )
    }

    fn accept(profile: &SparseProfile) -> AcceptedSparseProfile {
        let qualification = SparseQualification {
            profile_id: profile.profile_id.clone(),
            profile_revision: profile.revision,
            profile_fingerprint: profile.fingerprint,
            provider_artifact_digest: Blake3Digest32::from_bytes([7; 32]),
            compatibility_fixture_digest: Blake3Digest32::from_bytes([8; 32]),
            collision_fixture_digest: Blake3Digest32::from_bytes([9; 32]),
            accepted: true,
            qualification_receipt: ReceiptRef::new("test:qualification").expect("receipt"),
        };
        validate_sparse_profile(profile.clone(), qualification).expect("qualified")
    }

    fn hex(bytes: &[u8; 32]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len().saturating_mul(2));
        for byte in bytes {
            output.push(HEX[usize::from(byte >> 4)] as char);
            output.push(HEX[usize::from(byte & 15)] as char);
        }
        output
    }

    #[test]
    fn frozen_digests_match_pinned_constants() {
        let code = code_v1_sparse_profile().expect("code profile");
        let text = text_neutral_v1_sparse_profile().expect("text profile");
        assert_eq!(
            frozen_profile_digest(&code).expect("digest"),
            CODE_V1_FINGERPRINT,
            "code digest drifted: recomputed {}",
            hex(frozen_profile_digest(&code).expect("digest").as_bytes())
        );
        assert_eq!(
            frozen_profile_digest(&text).expect("digest"),
            TEXT_NEUTRAL_V1_FINGERPRINT,
            "text digest drifted: recomputed {}",
            hex(frozen_profile_digest(&text).expect("digest").as_bytes())
        );
        verify_frozen_profile(&code, CODE_V1_FINGERPRINT).expect("code frozen");
        verify_frozen_profile(&text, TEXT_NEUTRAL_V1_FINGERPRINT).expect("text frozen");
    }

    #[test]
    fn frozen_legs_differ() {
        let code = code_v1_sparse_profile().expect("code");
        let text = text_neutral_v1_sparse_profile().expect("text");
        assert_ne!(code.fingerprint, text.fingerprint);
        assert_ne!(code.hash_seed, text.hash_seed);
        assert_ne!(
            code.analyzer.character_policy,
            text.analyzer.character_policy
        );
    }

    #[test]
    fn profile_change_requires_new_generation() {
        let code = code_v1_sparse_profile().expect("code");
        // Any behavior change must break the frozen digest.
        let mut mutated = code.clone();
        mutated.hash_seed = mutated.hash_seed.wrapping_add(1);
        assert_ne!(
            frozen_profile_digest(&mutated).expect("digest"),
            frozen_profile_digest(&code).expect("digest")
        );
        assert_eq!(
            verify_frozen_profile(&mutated, CODE_V1_FINGERPRINT),
            Err(FrozenProfileError::DigestMismatch)
        );
        let mut mutated_space = code.clone();
        mutated_space.index_space -= 1;
        assert_eq!(
            verify_frozen_profile(&mutated_space, CODE_V1_FINGERPRINT),
            Err(FrozenProfileError::DigestMismatch)
        );
        let mut mutated_tf = code;
        mutated_tf.query_tf = QueryTfWeighting::Binary;
        assert_eq!(
            verify_frozen_profile(&mutated_tf, CODE_V1_FINGERPRINT),
            Err(FrozenProfileError::DigestMismatch)
        );
    }

    #[test]
    fn frozen_profiles_have_no_implicit_stopwords_or_stemming() {
        for profile in [
            code_v1_sparse_profile().expect("code"),
            text_neutral_v1_sparse_profile().expect("text"),
        ] {
            assert_eq!(profile.analyzer.stop_words().len(), 0);
            // Classic English stopwords must still be emitted: no implicit list.
            let analysis = analyze(
                test_input("the and is"),
                &profile.analyzer,
                DEFAULT_LEXICAL_LIMITS,
            )
            .expect("analyze");
            assert_eq!(analysis.tokens.len(), 3, "profile {}", profile.profile_id);
            // No stemming: three distinct surface forms stay distinct.
            let stem = analyze(
                test_input("running runs ran"),
                &profile.analyzer,
                DEFAULT_LEXICAL_LIMITS,
            )
            .expect("analyze");
            assert_eq!(stem.terms.len(), 3);
        }
    }

    #[test]
    fn multilingual_document_encoding_is_deterministic() {
        let code = code_v1_sparse_profile().expect("code");
        let accepted = accept(&code);
        let corpus = [
            "hello world search",
            "ПРИВЕТ Мир поиск",
            "你好 世界 搜索",
            "مرحبا بالعالم بحث",
            "get_user_by_id",
            "parseHTMLDocument",
            "std::collections::HashMap",
            "src/search/lexical.rs",
            "hello,world!foo-bar:baz",
        ];
        for text in corpus {
            let first = encode_document(
                test_input(text),
                &accepted,
                None,
                DEFAULT_LEXICAL_LIMITS,
                SparseLimits::BASELINE,
                false,
            )
            .unwrap_or_else(|error| panic!("encode document {text:?}: {error}"));
            let second = encode_document(
                test_input(text),
                &accepted,
                None,
                DEFAULT_LEXICAL_LIMITS,
                SparseLimits::BASELINE,
                false,
            )
            .expect("second encode");
            assert_eq!(first.vector, second.vector, "text {text:?}");
            assert_eq!(
                first.receipt.vector_fingerprint,
                second.receipt.vector_fingerprint
            );
            // Sorted, unique, in-bounds, finite.
            assert!(!first.vector.indices.is_empty(), "text {text:?}");
            assert!(
                first
                    .vector
                    .indices
                    .windows(2)
                    .all(|pair| pair[0] < pair[1]),
                "text {text:?}"
            );
            assert!(
                first
                    .vector
                    .indices
                    .iter()
                    .all(|index| *index < FROZEN_INDEX_SPACE),
                "text {text:?}"
            );
            assert!(
                first
                    .vector
                    .values
                    .iter()
                    .all(|value| value.is_finite() && *value > 0.0),
                "text {text:?}"
            );
            let query = encode_query(
                test_input(text),
                &accepted,
                None,
                DEFAULT_LEXICAL_LIMITS,
                SparseLimits::BASELINE,
                false,
            )
            .expect("encode query");
            let query_again = encode_query(
                test_input(text),
                &accepted,
                None,
                DEFAULT_LEXICAL_LIMITS,
                SparseLimits::BASELINE,
                false,
            )
            .expect("second query");
            assert_eq!(query.vector, query_again.vector);
        }
    }

    #[test]
    fn document_query_compatibility_for_singleton_terms() {
        let text_profile = text_neutral_v1_sparse_profile().expect("text");
        let accepted = accept(&text_profile);
        // Every term occurs once, so logarithmic document and query weights agree.
        let document = encode_document(
            test_input("alpha beta gamma"),
            &accepted,
            None,
            DEFAULT_LEXICAL_LIMITS,
            SparseLimits::BASELINE,
            false,
        )
        .expect("document");
        let query = encode_query(
            test_input("alpha beta gamma"),
            &accepted,
            None,
            DEFAULT_LEXICAL_LIMITS,
            SparseLimits::BASELINE,
            false,
        )
        .expect("query");
        assert_eq!(document.vector.indices, query.vector.indices);
        assert_eq!(document.vector.values, query.vector.values);
    }

    #[test]
    fn multilingual_golden_vectors_are_pinned() {
        let code = code_v1_sparse_profile().expect("code");
        let accepted = accept(&code);
        // Pinned vector fingerprints (hex of the 32-byte sparse fingerprint).
        let goldens: [(&str, &str); 4] = [
            (
                "hello world",
                "b6bca0f1445859b3e9f58e6baee3ceefb3d645b0920f3d310b131dea384dc483",
            ),
            (
                "привет мир",
                "b6bca0f1445859b3b8479b26d6790173099f5bde0737e0f31a3ac4b878f82373",
            ),
            (
                "get_user_by_id",
                "2c0d3adc42d464a235b49c22bb3ab43c5f4834f4bd30a679c7672e9e8dac37fb",
            ),
            (
                "std::collections::HashMap",
                "7307458608af5924a6ccf54f6c662a880e6f5857bac47c6cd1419ae7318babdf",
            ),
        ];
        for (text, expected) in goldens {
            let encoding = encode_document(
                test_input(text),
                &accepted,
                None,
                DEFAULT_LEXICAL_LIMITS,
                SparseLimits::BASELINE,
                false,
            )
            .unwrap_or_else(|error| panic!("golden {text:?}: {error}"));
            let actual = hex(&encoding.receipt.vector_fingerprint.0);
            assert_eq!(actual, expected, "golden drift for {text:?}");
        }
    }

    #[test]
    fn collision_policy_is_measurable_and_rejectable() {
        use crate::sparse::{CollisionPolicy, measure_collision_terms};
        let mut profile = code_v1_sparse_profile().expect("code");
        profile.index_space = 8;
        profile.maximum_collision_rate_ppm = 1_000_000;
        profile.collision_policy = CollisionPolicy::MergeMeasured;
        let terms = (0..64).map(|index| format!("collision_term_{index}"));
        let report =
            measure_collision_terms(terms, &profile, SparseLimits::BASELINE).expect("measure");
        assert!(report.collided_indexes > 0);
        assert!(report.collision_rate_ppm > 0);
        // Rejecting policy surfaces the same corpus as a typed collision.
        let mut rejecting = profile.clone();
        rejecting.collision_policy = CollisionPolicy::Reject;
        let analysis = analyze(
            test_input("alpha beta gamma"),
            &rejecting.analyzer,
            DEFAULT_LEXICAL_LIMITS,
        )
        .expect("analyze");
        assert!(!analysis.tokens.is_empty());
        let tiny_terms = (0..64).map(|index| format!("reject_term_{index}"));
        let reject_report = measure_collision_terms(tiny_terms, &rejecting, SparseLimits::BASELINE)
            .expect("reject measure still reports");
        assert!(reject_report.collided_indexes > 0);
        // Zero threshold turns any collision into a threshold failure at map time.
        let mut strict = profile;
        strict.maximum_collision_rate_ppm = 0;
        strict.collision_policy = CollisionPolicy::MergeMeasured;
        let strict_terms = (0..64).map(|index| format!("collision_term_{index}"));
        let strict_report = measure_collision_terms(strict_terms, &strict, SparseLimits::BASELINE)
            .expect("strict measure reports");
        assert!(!strict_report.accepted);
    }

    #[test]
    fn sparse_vector_validation_rejects_non_finite_and_unordered() {
        use crate::sparse::SparseVector;
        let profile = code_v1_sparse_profile().expect("code");
        let good = SparseVector {
            indices: vec![1, 5, 9],
            values: vec![1.0, 2.0, 3.0],
        };
        good.validate(&profile).expect("good vector");
        for bad in [
            SparseVector {
                indices: vec![],
                values: vec![],
            },
            SparseVector {
                indices: vec![3, 3],
                values: vec![1.0, 1.0],
            },
            SparseVector {
                indices: vec![9, 5],
                values: vec![1.0, 1.0],
            },
            SparseVector {
                indices: vec![1],
                values: vec![f32::NAN],
            },
            SparseVector {
                indices: vec![1],
                values: vec![f32::INFINITY],
            },
            SparseVector {
                indices: vec![1],
                values: vec![0.0],
            },
            SparseVector {
                indices: vec![FROZEN_INDEX_SPACE],
                values: vec![1.0],
            },
            SparseVector {
                indices: vec![1, 2],
                values: vec![1.0],
            },
        ] {
            assert!(bad.validate(&profile).is_err(), "vector {bad:?}");
        }
    }

    #[test]
    fn bounded_budgets_fail_closed() {
        let code = code_v1_sparse_profile().expect("code");
        let accepted = accept(&code);
        let tiny_lexical = LexicalLimits {
            max_tokens: 1,
            ..DEFAULT_LEXICAL_LIMITS
        };
        assert!(
            encode_document(
                test_input("alpha beta gamma"),
                &accepted,
                None,
                tiny_lexical,
                SparseLimits::BASELINE,
                false,
            )
            .is_err()
        );
        let tiny_sparse = SparseLimits {
            max_vector_values: 1,
            ..SparseLimits::BASELINE
        };
        assert!(
            encode_document(
                test_input("alpha beta gamma delta epsilon zeta"),
                &accepted,
                None,
                DEFAULT_LEXICAL_LIMITS,
                tiny_sparse,
                false,
            )
            .is_err()
        );
        assert!(
            encode_document(
                test_input("alpha"),
                &accepted,
                None,
                DEFAULT_LEXICAL_LIMITS,
                SparseLimits::BASELINE,
                true,
            )
            .is_err(),
            "cancelled encoding must not advertise a vector"
        );
    }

    #[test]
    fn frozen_error_codes_are_stable() {
        assert_eq!(
            FrozenProfileError::DigestMismatch.code(),
            "FROZEN_PROFILE_DIGEST_MISMATCH"
        );
        assert_eq!(
            FrozenProfileError::CanonicalOverflow.code(),
            "FROZEN_PROFILE_CANONICAL_OVERFLOW"
        );
    }

    #[test]
    fn content_is_redacted_in_debug() {
        let code = code_v1_sparse_profile().expect("code");
        let accepted = accept(&code);
        let encoding = encode_document(
            test_input("sensitive lexical content"),
            &accepted,
            None,
            DEFAULT_LEXICAL_LIMITS,
            SparseLimits::BASELINE,
            false,
        )
        .expect("encode");
        for debug in [
            format!("{:?}", encoding.analysis),
            format!("{:?}", encoding.features),
            format!("{encoding:?}"),
            format!("{:?}", encoding.features.features[0]),
            format!("{:?}", encoding.analysis.tokens[0]),
        ] {
            assert!(
                !debug.contains("sensitive"),
                "debug leaked content: {debug}"
            );
        }
        let stats_debug = format!(
            "{:?}",
            crate::sparse::FrozenCorpusStatistics {
                document_count: 1,
                average_document_length: 3.0,
                document_frequency: std::iter::once((1_u32, 1_u64)).collect(),
                statistics_digest: Blake3Digest32::from_bytes([0xAB; 32]),
            }
        );
        assert!(stats_debug.contains("frequency_entries"));
    }
}
