//! Frozen code-identifier qualification fixtures and expected owner outputs.

use super::{
    CanonicalValue, CodeIdentifierLimits, CodeIdentifiersProfile, CodeProfileQualificationError,
    IdentifierError, LexicalInput, array, text,
};
use crate::sparse::{SparseEncodingKind, SparseError, SparseLimits, SparseQualification};
use search_contracts::{NonZeroRevision, OpaqueId, ReceiptRef};
use std::time::Instant;

type ExpectedToken = (&'static str, u64, u64, u32, u64);

#[derive(Clone, Copy)]
struct CompatibilityCase {
    input: &'static str,
    tokens: &'static [ExpectedToken],
    parents: u64,
    attempts: u64,
}

#[derive(Clone, Copy)]
struct QualificationEvidence {
    checked_cases: usize,
    document_query_match: bool,
    collision_accepted: bool,
}

impl QualificationEvidence {
    const fn accepted(self) -> bool {
        self.checked_cases == CASES.len() && self.document_query_match && self.collision_accepted
    }
}

const DOCUMENT_QUERY_CASE: CompatibilityCase = CompatibilityCase {
    input: "name name",
    tokens: &[("name", 0, 4, 4, 0), ("name", 5, 9, 4, 1)],
    parents: 2,
    attempts: 2,
};

const CASES: &[CompatibilityCase] = &[
    DOCUMENT_QUERY_CASE,
    CompatibilityCase {
        input: "_ __",
        tokens: &[("_", 0, 1, 1, 0), ("__", 2, 4, 2, 1)],
        parents: 2,
        attempts: 2,
    },
    CompatibilityCase {
        input: "XMLParser_name",
        tokens: &[
            ("XMLParser_name", 0, 14, 14, 0),
            ("XML", 0, 3, 3, 0),
            ("Parser", 3, 9, 6, 0),
            ("name", 10, 14, 4, 0),
        ],
        parents: 1,
        attempts: 4,
    },
    CompatibilityCase {
        input: "a_a a",
        tokens: &[("a_a", 0, 3, 3, 0), ("a", 0, 1, 1, 0), ("a", 4, 5, 1, 1)],
        parents: 2,
        attempts: 4,
    },
    CompatibilityCase {
        input: "e\u{301}\r\né",
        tokens: &[("é", 0, 3, 2, 0), ("é", 5, 7, 1, 1)],
        parents: 2,
        attempts: 2,
    },
    CompatibilityCase {
        input: "9abc \u{301}abc _x",
        tokens: &[("_x", 11, 13, 2, 0), ("x", 12, 13, 1, 0)],
        parents: 1,
        attempts: 2,
    },
    CompatibilityCase {
        input: "foo-bar/baz\\qux.mod:name",
        tokens: &[
            ("foo", 0, 3, 3, 0),
            ("bar", 4, 7, 3, 1),
            ("baz", 8, 11, 3, 2),
            ("qux", 12, 15, 3, 3),
            ("mod", 16, 19, 3, 4),
            ("name", 20, 24, 4, 5),
        ],
        parents: 6,
        attempts: 6,
    },
    CompatibilityCase {
        input: "Ж α 中",
        tokens: &[("Ж", 0, 2, 1, 0), ("α", 3, 5, 1, 1), ("中", 6, 9, 1, 2)],
        parents: 3,
        attempts: 3,
    },
];

const COLLISION_TERMS: &[&str] = &[
    "_",
    "alpha",
    "beta",
    "XMLParser",
    "Parser",
    "name",
    "Ж",
    "α",
    "中",
];

const NAME_TERM_INDEX: u32 = 982_240;
const COLLISION_EXPECTED_FEATURES: &[(u32, &[&str])] = &[
    (81_294, &["_"]),
    (251_866, &["Ж"]),
    (434_149, &["Parser"]),
    (663_571, &["XMLParser"]),
    (678_026, &["alpha"]),
    (751_348, &["beta"]),
    (798_921, &["中"]),
    (832_349, &["XML"]),
    (982_240, &["name"]),
    (1_025_952, &["α"]),
];

pub(super) fn compatibility_definition() -> Result<CanonicalValue, IdentifierError> {
    let cases = CASES
        .iter()
        .map(|case| {
            let tokens = case
                .tokens
                .iter()
                .map(|(term, start, end, chars, position)| {
                    array(vec![
                        text(term)?,
                        CanonicalValue::U64(*start),
                        CanonicalValue::U64(*end),
                        CanonicalValue::U64(u64::from(*chars)),
                        CanonicalValue::U64(*position),
                    ])
                })
                .collect::<Result<Vec<_>, _>>()?;
            array(vec![
                text(case.input)?,
                CanonicalValue::U64(100),
                array(tokens)?,
                CanonicalValue::U64(case.parents),
                CanonicalValue::U64(case.attempts),
            ])
        })
        .collect::<Result<Vec<_>, _>>()?;
    array(vec![
        text(
            "authored-code-compatibility@1;term-original-start/end/scalars/parent-position;source-start100",
        )?,
        array(cases)?,
    ])
}

pub(super) fn collision_definition() -> Result<CanonicalValue, IdentifierError> {
    array(vec![
        text("authored-code-collision-corpus@1;terms-only;measurement-required-by-qualification")?,
        array(
            COLLISION_TERMS
                .iter()
                .map(|term| text(term))
                .collect::<Result<Vec<_>, _>>()?,
        )?,
    ])
}

fn qualification_input(
    value: &str,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<LexicalInput, CodeProfileQualificationError> {
    let mut bounded = super::CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
    bounded.poll()?;
    let source = OpaqueId::new("source:code-profile-qualification")
        .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?;
    let revision =
        NonZeroRevision::new(1).map_err(|_| CodeProfileQualificationError::FixtureMismatch)?;
    let end = 100_u64
        .checked_add(
            u64::try_from(value.len())
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        )
        .ok_or(CodeProfileQualificationError::FixtureMismatch)?;
    let owned_value = value.to_owned();
    bounded.poll()?;
    Ok(LexicalInput::new(
        source,
        revision,
        0,
        100,
        end,
        owned_value,
    ))
}

fn compatibility_case_matches(
    result: &super::CodeIdentifierAnalysis,
    case: &CompatibilityCase,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<bool, CodeProfileQualificationError> {
    let analysis = result.analysis();
    if analysis.tokens.len() != case.tokens.len()
        || analysis.receipt.candidate_count != case.parents
        || analysis.receipt.position_span != case.parents
        || result.expansion_attempts() != case.attempts
        || analysis
            .terms
            .iter()
            .map(|term| term.frequency)
            .sum::<u64>()
            != analysis.receipt.emitted_token_count
    {
        return Ok(false);
    }
    let mut bounded = super::CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
    for (actual, expected) in analysis.tokens.iter().zip(case.tokens) {
        bounded.poll()?;
        if actual.term != expected.0
            || actual.unit_byte_start != expected.1
            || actual.unit_byte_end != expected.2
            || actual.original_char_count != expected.3
            || actual.position != expected.4
            || actual.source_byte_start != 100 + expected.1
            || actual.source_byte_end != 100 + expected.2
        {
            return Ok(false);
        }
    }
    bounded.poll()?;
    Ok(true)
}

fn check_compatibility_case(
    profile: &CodeIdentifiersProfile,
    case: &CompatibilityCase,
    limits: CodeIdentifierLimits,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<(), CodeProfileQualificationError> {
    let input = qualification_input(case.input, deadline, checkpoint)?;
    let result = {
        let mut bounded = super::CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
        profile.analyze_checked(&input, limits, &mut bounded)?
    };
    if !compatibility_case_matches(&result, case, deadline, checkpoint)? {
        return Err(CodeProfileQualificationError::FixtureMismatch);
    }
    Ok(())
}

fn verify_compatibility_cases(
    profile: &CodeIdentifiersProfile,
    cases: &[CompatibilityCase],
    limits: CodeIdentifierLimits,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<usize, CodeProfileQualificationError> {
    let mut verified = 0_usize;
    for case in cases {
        check_compatibility_case(profile, case, limits, deadline, checkpoint)?;
        verified = verified
            .checked_add(1)
            .ok_or(CodeProfileQualificationError::FixtureMismatch)?;
    }
    Ok(verified)
}

fn collision_matches_expected(parts: &super::encoding::EncodingParts) -> bool {
    let report = &parts.features.report;
    if report.distinct_terms != 10
        || report.distinct_indexes != 10
        || report.collided_indexes != 0
        || report.collision_pairs != 0
        || report.collision_rate_ppm != 0
        || report.threshold_ppm != 10_000
        || !report.accepted
        || parts.features.features.len() != COLLISION_EXPECTED_FEATURES.len()
    {
        return false;
    }
    parts
        .features
        .features
        .iter()
        .zip(COLLISION_EXPECTED_FEATURES)
        .all(|(actual, (index, terms))| {
            actual.index == *index
                && actual.terms.len() == terms.len()
                && actual
                    .terms
                    .iter()
                    .map(String::as_str)
                    .eq(terms.iter().copied())
        })
}

fn verify_fixture_identities(
    profile: &CodeIdentifiersProfile,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<(), CodeProfileQualificationError> {
    let mut bounded = super::CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
    bounded.poll()?;
    let compatibility = compatibility_definition()?;
    bounded.poll()?;
    let compatibility_identity =
        super::identity("eliot/cbor/lexical-code-compatibility/v1", &compatibility)?;
    bounded.poll()?;
    let collisions = collision_definition()?;
    bounded.poll()?;
    let collision_identity = super::identity("eliot/cbor/lexical-code-collisions/v1", &collisions)?;
    bounded.poll()?;
    if compatibility_identity != profile.compatibility_fixture_identity()
        || collision_identity != profile.collision_fixture_identity()
    {
        return Err(CodeProfileQualificationError::FixtureMismatch);
    }
    Ok(())
}

fn verify_document_query(
    profile: &CodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<
    (
        super::encoding::EncodingParts,
        super::encoding::EncodingParts,
    ),
    CodeProfileQualificationError,
> {
    let pair_input = qualification_input("name name", deadline, checkpoint)?;
    let document = super::encoding::encode_parts(
        SparseEncodingKind::Document,
        &pair_input,
        profile,
        lexical_limits,
        sparse_limits,
        Some(deadline),
        checkpoint,
    )?;
    let query = super::encoding::encode_parts(
        SparseEncodingKind::Query,
        &pair_input,
        profile,
        lexical_limits,
        sparse_limits,
        Some(deadline),
        checkpoint,
    )?;
    let document_query_match = compatibility_case_matches(
        &document.analyzed,
        &DOCUMENT_QUERY_CASE,
        deadline,
        checkpoint,
    )? && compatibility_case_matches(
        &query.analyzed,
        &DOCUMENT_QUERY_CASE,
        deadline,
        checkpoint,
    )? && document.analyzed.analysis == query.analyzed.analysis
        && document.vector.indices == query.vector.indices
        && document.vector.indices == [NAME_TERM_INDEX]
        && document.vector.values == [2.0]
        && query.vector.values == [1.0]
        && document.features.features.len() == 1
        && query.features.features.len() == 1
        && document.features.features[0].index == NAME_TERM_INDEX
        && query.features.features[0].index == NAME_TERM_INDEX
        && document.features.features[0]
            .terms
            .iter()
            .map(String::as_str)
            .eq(["name"])
        && query.features.features[0]
            .terms
            .iter()
            .map(String::as_str)
            .eq(["name"])
        && document.features.features[0].frequency == 2
        && query.features.features[0].frequency == 2
        && document.features.features[0].first_position == 0
        && query.features.features[0].first_position == 0
        && document.features.features[0].last_position == 1
        && query.features.features[0].last_position == 1
        && document.features.report.distinct_terms == 1
        && query.features.report.distinct_terms == 1
        && document.features.report.distinct_indexes == 1
        && query.features.report.distinct_indexes == 1
        && document.features.report.collided_indexes == 0
        && query.features.report.collided_indexes == 0
        && document.features.report.accepted
        && query.features.report.accepted;
    if !document_query_match {
        return Err(CodeProfileQualificationError::FixtureMismatch);
    }
    Ok((document, query))
}

fn verify_collision_corpus(
    profile: &CodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<super::encoding::EncodingParts, CodeProfileQualificationError> {
    let mut bounded = super::CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
    bounded.poll()?;
    let capacity = COLLISION_TERMS.iter().try_fold(0_usize, |total, term| {
        total
            .checked_add(term.len())
            .and_then(|next| next.checked_add(usize::from(total > 0)))
            .ok_or(CodeProfileQualificationError::FixtureMismatch)
    })?;
    let mut corpus = String::with_capacity(capacity);
    for term in COLLISION_TERMS {
        bounded.poll()?;
        if !corpus.is_empty() {
            corpus.push(' ');
        }
        corpus.push_str(term);
    }
    bounded.poll()?;
    let collision_input = qualification_input(&corpus, deadline, checkpoint)?;
    let collision = super::encoding::encode_parts(
        SparseEncodingKind::Document,
        &collision_input,
        profile,
        lexical_limits,
        sparse_limits,
        Some(deadline),
        checkpoint,
    )?;
    if !collision_matches_expected(&collision) {
        return Err(CodeProfileQualificationError::FixtureMismatch);
    }
    Ok(collision)
}

fn qualification_receipt(
    profile: &CodeIdentifiersProfile,
    checked_cases: usize,
    document: &super::encoding::EncodingParts,
    query: &super::encoding::EncodingParts,
    collision: &super::encoding::EncodingParts,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<ReceiptRef, CodeProfileQualificationError> {
    let mut bounded = super::CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
    bounded.poll()?;
    let receipt_value = array(vec![
        text("lexical-code-runtime-qualification@1")?,
        CanonicalValue::Bytes(
            search_contracts::BoundedBytes::new(profile.fingerprint().as_bytes().to_vec())
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        ),
        CanonicalValue::U64(
            u64::try_from(checked_cases)
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        ),
        CanonicalValue::U64(
            u64::try_from(collision.features.report.distinct_terms)
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        ),
        CanonicalValue::U64(
            u64::try_from(collision.features.report.distinct_indexes)
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        ),
        CanonicalValue::U64(
            u64::try_from(collision.features.report.collided_indexes)
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        ),
        CanonicalValue::U64(
            u64::try_from(collision.features.report.collision_pairs)
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        ),
        CanonicalValue::U64(u64::from(collision.features.report.collision_rate_ppm)),
        array(
            collision
                .features
                .features
                .iter()
                .map(|feature| {
                    array(vec![
                        CanonicalValue::U64(u64::from(feature.index)),
                        array(
                            feature
                                .terms
                                .iter()
                                .map(|term| text(term))
                                .collect::<Result<_, _>>()?,
                        )?,
                    ])
                })
                .collect::<Result<Vec<_>, _>>()?,
        )?,
        CanonicalValue::Bytes(
            search_contracts::BoundedBytes::new(document.vector_fingerprint.0.to_vec())
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        ),
        CanonicalValue::Bytes(
            search_contracts::BoundedBytes::new(query.vector_fingerprint.0.to_vec())
                .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?,
        ),
    ])?;
    bounded.poll()?;
    let receipt_digest =
        super::identity("eliot/cbor/lexical-code-qualification/v1", &receipt_value)?;
    bounded.poll()?;
    let receipt = ReceiptRef::new(format!(
        "receipt:lexical-code-runtime-qualification:v1:{}",
        full_hex(receipt_digest.as_bytes())
    ))
    .map_err(|_| CodeProfileQualificationError::FixtureMismatch)?;
    bounded.poll()?;
    Ok(receipt)
}

pub(super) fn qualify(
    profile: &CodeIdentifiersProfile,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    deadline: Instant,
    checkpoint: &mut dyn FnMut() -> Result<(), SparseError>,
) -> Result<SparseQualification, CodeProfileQualificationError> {
    {
        let mut boundary = super::CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
        boundary.poll()?;
    }
    verify_fixture_identities(profile, deadline, checkpoint)?;
    let checked_cases =
        verify_compatibility_cases(profile, CASES, lexical_limits, deadline, checkpoint)?;
    let (document, query) =
        verify_document_query(profile, lexical_limits, sparse_limits, deadline, checkpoint)?;
    let collision =
        verify_collision_corpus(profile, lexical_limits, sparse_limits, deadline, checkpoint)?;
    let evidence = QualificationEvidence {
        checked_cases,
        document_query_match: true,
        collision_accepted: collision.features.report.accepted,
    };
    let receipt = qualification_receipt(
        profile,
        checked_cases,
        &document,
        &query,
        &collision,
        deadline,
        checkpoint,
    )?;
    {
        let mut boundary = super::CodeEncodingCheckpoint::new(Some(deadline), checkpoint);
        boundary.poll()?;
    }
    Ok(SparseQualification {
        profile_id: profile.id().clone(),
        profile_revision: profile.revision(),
        profile_fingerprint: profile.fingerprint(),
        provider_artifact_digest: profile.artifact_set_identity(),
        compatibility_fixture_digest: profile.compatibility_fixture_identity(),
        collision_fixture_digest: profile.collision_fixture_identity(),
        accepted: evidence.accepted(),
        qualification_receipt: receipt,
    })
}

fn full_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CodeIdentifierLimits, CodeIdentifiersProfile, LexicalInput, SparseError, SparseLimits,
        accept_code_identifiers_profile, encode_code_document,
        encode_code_document_with_checkpoint, encode_code_query, qualify_code_identifiers_profile,
        validate_code_identifiers_pair,
    };
    use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    fn input(value: &str) -> LexicalInput {
        LexicalInput::new(
            OpaqueId::new("source:identifier-fixture").expect("source"),
            NonZeroRevision::new(1).expect("revision"),
            0,
            100,
            100 + u64::try_from(value.len()).expect("length"),
            value.into(),
        )
    }

    fn qualified(
        profile: CodeIdentifiersProfile,
    ) -> super::super::encoding::AcceptedCodeIdentifiersProfile {
        qualify_code_identifiers_profile(
            profile,
            CodeIdentifierLimits::HARD,
            SparseLimits::BASELINE,
            Instant::now() + Duration::from_secs(30),
            &mut || Ok(()),
        )
        .expect("owner fixtures execute and qualify the fixed profile")
    }

    #[test]
    fn authored_golden_spans_parents_and_attempts() {
        let profile = CodeIdentifiersProfile::new().expect("profile");
        for case in CASES {
            let result = profile
                .analyze(&input(case.input), CodeIdentifierLimits::HARD)
                .expect("analysis");
            let analysis = result.analysis();
            let actual = analysis
                .tokens
                .iter()
                .map(|token| {
                    (
                        token.term.as_str(),
                        token.unit_byte_start,
                        token.unit_byte_end,
                        token.original_char_count,
                        token.position,
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, case.tokens);
            assert_eq!(analysis.receipt.candidate_count, case.parents);
            assert_eq!(analysis.receipt.position_span, case.parents);
            assert_eq!(result.expansion_attempts(), case.attempts);
            assert_eq!(
                analysis
                    .terms
                    .iter()
                    .map(|term| term.frequency)
                    .sum::<u64>(),
                analysis.receipt.emitted_token_count
            );
            for token in &analysis.tokens {
                assert_eq!(token.source_byte_start, 100 + token.unit_byte_start);
                assert_eq!(token.source_byte_end, 100 + token.unit_byte_end);
            }
        }
    }

    #[test]
    fn filtered_components_and_dedup_do_not_refund_work() {
        let profile = CodeIdentifiersProfile::new().expect("profile");
        let result = profile
            .analyze(&input("a_1 a_\u{301}"), CodeIdentifierLimits::HARD)
            .expect("analysis");
        assert_eq!(result.expansion_attempts(), 6);
        assert_eq!(result.analysis().tokens.len(), 4);
        assert_eq!(
            result
                .analysis()
                .terms
                .iter()
                .find(|term| term.term == "a")
                .expect("a")
                .frequency,
            2
        );
        let limits = CodeIdentifierLimits {
            total_expansions: 2,
            term_bytes: 1,
            ..CodeIdentifierLimits::HARD
        };
        assert_eq!(
            profile.analyze(&input("a_a"), limits),
            Err(IdentifierError::ExpansionsExceeded)
        );
    }

    #[test]
    fn original_and_normalized_limits_are_independent() {
        let profile = CodeIdentifiersProfile::new().expect("profile");
        let value = input("e\u{301}");
        assert_eq!(
            profile.analyze(
                &value,
                CodeIdentifierLimits {
                    original_bytes: 2,
                    ..CodeIdentifierLimits::HARD
                }
            ),
            Err(IdentifierError::OriginalBytesExceeded)
        );
        assert_eq!(
            profile.analyze(
                &value,
                CodeIdentifierLimits {
                    raw_scalars: 1,
                    ..CodeIdentifierLimits::HARD
                }
            ),
            Err(IdentifierError::TokenScalarsExceeded)
        );
        assert_eq!(
            profile.analyze(
                &value,
                CodeIdentifierLimits {
                    term_bytes: 1,
                    ..CodeIdentifierLimits::HARD
                }
            ),
            Err(IdentifierError::TermBytesExceeded)
        );
        let expanded = format!("_{}", "\u{344}".repeat(64));
        assert_eq!(
            profile.analyze(&input(&expanded), CodeIdentifierLimits::HARD),
            Err(IdentifierError::TokenScalarsExceeded)
        );
    }

    #[test]
    fn limits_fail_without_truncation_or_sibling_position_drift() {
        let profile = CodeIdentifiersProfile::new().expect("profile");
        assert_eq!(
            profile.analyze(
                &input("a_b_c"),
                CodeIdentifierLimits {
                    components: 2,
                    ..CodeIdentifierLimits::HARD
                }
            ),
            Err(IdentifierError::ComponentsExceeded)
        );
        assert_eq!(
            profile.analyze(
                &input("a_b_c"),
                CodeIdentifierLimits {
                    expansions: 3,
                    ..CodeIdentifierLimits::HARD
                }
            ),
            Err(IdentifierError::ExpansionsExceeded)
        );
        assert_eq!(
            profile.analyze(
                &input("a a b"),
                CodeIdentifierLimits {
                    tokens: 2,
                    ..CodeIdentifierLimits::HARD
                }
            ),
            Err(IdentifierError::TokensExceeded)
        );
        assert_eq!(
            profile.analyze(
                &input("a a b"),
                CodeIdentifierLimits {
                    unique_terms: 1,
                    ..CodeIdentifierLimits::HARD
                }
            ),
            Err(IdentifierError::UniqueTermsExceeded)
        );
        assert_eq!(
            profile
                .analyze(&input("aB\u{301}"), CodeIdentifierLimits::HARD)
                .expect("analysis")
                .analysis()
                .tokens[2]
                .term,
            "B\u{301}"
        );
        assert_eq!(
            profile
                .analyze(&input("a\u{301}B"), CodeIdentifierLimits::HARD)
                .expect("analysis")
                .analysis()
                .tokens
                .len(),
            1
        );
    }

    #[test]
    fn internal_identities_are_repeatable_and_qualification_mismatches_refuse() {
        let profile = CodeIdentifiersProfile::new().expect("profile");
        assert_eq!(profile, CodeIdentifiersProfile::new().expect("profile"));
        let accepted = qualified(profile.clone());
        for selected in 0..4 {
            let mut raw = accepted.qualification().clone();
            match selected {
                0 => raw.profile_fingerprint = Blake3Digest32::from_bytes([9; 32]),
                1 => raw.provider_artifact_digest = Blake3Digest32::from_bytes([9; 32]),
                2 => raw.compatibility_fixture_digest = Blake3Digest32::from_bytes([9; 32]),
                _ => raw.collision_fixture_digest = Blake3Digest32::from_bytes([9; 32]),
            }
            assert_eq!(
                accept_code_identifiers_profile(profile.clone(), raw),
                Err(SparseError::QualificationMismatch)
            );
        }
    }

    #[test]
    fn query_and_document_share_terms_identity_and_mapping_with_distinct_tf() {
        let profile = CodeIdentifiersProfile::new().expect("profile");
        let accepted = qualified(profile.clone());
        let document = encode_code_document(
            &input("name name"),
            &accepted,
            CodeIdentifierLimits::HARD,
            SparseLimits::BASELINE,
            false,
        )
        .expect("document");
        let query = encode_code_query(
            &input("name name"),
            &accepted,
            CodeIdentifierLimits::HARD,
            SparseLimits::BASELINE,
            false,
        )
        .expect("query");
        assert_eq!(document.encoding().analysis, query.encoding().analysis);
        assert_eq!(
            document.encoding().vector.indices,
            query.encoding().vector.indices
        );
        assert_eq!(document.encoding().vector.indices, [NAME_TERM_INDEX]);
        assert_eq!(document.encoding().vector.values, vec![2.0]);
        assert_eq!(query.encoding().vector.values, vec![1.0]);
        validate_code_identifiers_pair(&document, &query, &profile).expect("same profile");
        assert_eq!(
            validate_code_identifiers_pair(&document, &document, &profile),
            Err(IdentifierError::ProfileMismatch)
        );
    }

    #[test]
    fn owner_qualification_refuses_fixture_mismatch_cancellation_and_limits() {
        let profile = CodeIdentifiersProfile::new().expect("profile");
        let mut mismatched = DOCUMENT_QUERY_CASE;
        mismatched.parents += 1;
        assert_eq!(
            check_compatibility_case(
                &profile,
                &mismatched,
                CodeIdentifierLimits::HARD,
                Instant::now() + Duration::from_secs(30),
                &mut || Ok(()),
            ),
            Err(crate::CodeProfileQualificationError::FixtureMismatch)
        );

        let checks = AtomicUsize::new(0);
        let mut cancellation = || {
            if checks.fetch_add(1, Ordering::Relaxed) + 1 >= 5 {
                Err(SparseError::Cancelled)
            } else {
                Ok(())
            }
        };
        assert_eq!(
            qualify_code_identifiers_profile(
                profile.clone(),
                CodeIdentifierLimits::HARD,
                SparseLimits::BASELINE,
                Instant::now() + Duration::from_secs(30),
                &mut cancellation,
            ),
            Err(crate::CodeProfileQualificationError::Encoding(
                crate::CodeEncodingError::Sparse(SparseError::Cancelled)
            ))
        );

        assert_eq!(
            qualify_code_identifiers_profile(
                profile.clone(),
                CodeIdentifierLimits::HARD,
                SparseLimits::BASELINE,
                Instant::now().checked_sub(Duration::from_secs(1)).expect("fixture deadline"),
                &mut || Ok(()),
            ),
            Err(crate::CodeProfileQualificationError::Encoding(
                crate::CodeEncodingError::Sparse(SparseError::Cancelled)
            ))
        );

        assert_eq!(
            qualify_code_identifiers_profile(
                profile,
                CodeIdentifierLimits {
                    tokens: 1,
                    ..CodeIdentifierLimits::HARD
                },
                SparseLimits::BASELINE,
                Instant::now() + Duration::from_secs(30),
                &mut || Ok(()),
            ),
            Err(crate::CodeProfileQualificationError::Encoding(
                crate::CodeEncodingError::Identifier(IdentifierError::TokensExceeded)
            ))
        );
    }

    #[test]
    fn document_encoder_observes_cancellation_during_analysis_work() {
        let profile = CodeIdentifiersProfile::new().expect("profile");
        let accepted = qualified(profile);
        let value = "alpha ".repeat(128);
        let checks = AtomicUsize::new(0);
        let mut cancellation = || {
            let current = checks.fetch_add(1, Ordering::Relaxed) + 1;
            if current >= 16 {
                Err(SparseError::Cancelled)
            } else {
                Ok(())
            }
        };

        let result = encode_code_document_with_checkpoint(
            &input(&value),
            &accepted,
            CodeIdentifierLimits::HARD,
            SparseLimits::BASELINE,
            Instant::now() + Duration::from_secs(30),
            &mut cancellation,
        );

        assert_eq!(
            result,
            Err(crate::CodeEncodingError::Sparse(SparseError::Cancelled))
        );
        assert_eq!(checks.load(Ordering::Relaxed), 16);
    }
}
