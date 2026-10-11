//! Authored synthetic fixture definitions. Their identities never assert a run verdict.

use super::{CanonicalValue, IdentifierError, array, text};

type ExpectedToken = (&'static str, u64, u64, u32, u64);

struct CompatibilityCase {
    input: &'static str,
    tokens: &'static [ExpectedToken],
    parents: u64,
    attempts: u64,
}

const CASES: &[CompatibilityCase] = &[
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CodeIdentifierLimits, CodeIdentifiersProfile, LexicalInput, SparseError, SparseLimits,
        SparseQualification, accept_code_identifiers_profile, encode_code_document,
        encode_code_query, validate_code_identifiers_pair,
    };
    use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

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

    fn qualification(profile: &CodeIdentifiersProfile) -> SparseQualification {
        SparseQualification {
            profile_id: profile.id().clone(),
            profile_revision: profile.revision(),
            profile_fingerprint: profile.fingerprint(),
            provider_artifact_digest: profile.artifact_set_identity(),
            compatibility_fixture_digest: profile.compatibility_fixture_identity(),
            collision_fixture_digest: profile.collision_fixture_identity(),
            accepted: true,
            qualification_receipt: ReceiptRef::new("receipt:synthetic-unexecuted-fixture")
                .expect("receipt"),
        }
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
        for selected in 0..4 {
            let mut raw = qualification(&profile);
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
        let accepted = accept_code_identifiers_profile(profile.clone(), qualification(&profile))
            .expect("synthetic matching only");
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
        assert_eq!(document.encoding().vector.values, vec![2.0]);
        assert_eq!(query.encoding().vector.values, vec![1.0]);
        validate_code_identifiers_pair(&document, &query, &profile).expect("same profile");
        assert_eq!(
            validate_code_identifiers_pair(&document, &document, &profile),
            Err(IdentifierError::ProfileMismatch)
        );
    }
}
