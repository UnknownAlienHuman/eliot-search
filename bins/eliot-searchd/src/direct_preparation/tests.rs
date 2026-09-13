use super::*;
use super::layout::{prepare_and_scan, scan_with_limits};
use super::profile::{LITERAL, MATERIALIZATION, UNITIZATION};

use search_exact::literal::LiteralLimits;
use search_materializer::MaterializationLimits;
use search_unitizer::UnitizationLimits;

use crate::development::{MAX_SCAN_MATCHES, ScanMatch, ScanResult};

fn small(text: &str, query: &str) -> ScanResult {
    scan_with_limits(
        text.to_owned(),
        query,
        false,
        MATERIALIZATION,
        UnitizationLimits {
            preferred_unit_bytes: 2,
            max_unit_bytes: 4,
            ..UNITIZATION
        },
        LITERAL,
    )
    .unwrap()
}

#[test]
fn query_longer_than_units_still_matches_once() {
    let result = small("a123456789z", "123456789");
    assert_eq!(result.matches.len(), 1);
    assert_eq!(
        (result.matches[0].byte_start, result.matches[0].byte_end),
        (1, 10)
    );
    assert!(result.coverage.complete);
}

#[test]
fn newline_and_unicode_coordinates_are_source_byte_coordinates() {
    let text = "a\r\nβ\rc\n𐀀 target";
    let result = small(text, "target");
    let start = text.find("target").unwrap();
    assert_eq!(
        result.matches,
        vec![ScanMatch {
            byte_start: start,
            byte_end: start + 6,
            line: 3,
            column_bytes: 5
        }]
    );
    let crossing = small("a\r\nβ", "\r\nβ");
    assert_eq!(
        (crossing.matches[0].byte_start, crossing.matches[0].line),
        (1, 0)
    );
}

#[test]
fn repeated_matches_across_units_are_not_duplicated_or_lost() {
    let result = small("aaaaa", "aaa");
    assert_eq!(
        result
            .matches
            .iter()
            .map(|item| item.byte_start)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn preparation_failure_is_not_complete_empty_success() {
    assert_eq!(
        prepare_and_scan("a\0b".to_owned(), "missing", false),
        Err("MATERIALIZATION_BINARY_CONTENT")
    );
    assert!(small("", "missing").coverage.complete);
}

#[test]
fn limits_propagate_without_claiming_full_coverage() {
    let result = scan_with_limits(
        "aaaa".to_owned(),
        "aa",
        false,
        MATERIALIZATION,
        UNITIZATION,
        LiteralLimits {
            max_matches: 1,
            ..LITERAL
        },
    )
    .unwrap();
    assert_eq!(result.matches.len(), 1);
    assert!(!result.coverage.complete);
    assert!(result.coverage.match_limit_reached);
    assert_eq!(
        scan_with_limits(
            "a\nb".to_owned(),
            "b",
            false,
            MaterializationLimits {
                max_lines: 1,
                ..MATERIALIZATION
            },
            UNITIZATION,
            LITERAL
        ),
        Err("MATERIALIZATION_TOO_MANY_LINES")
    );
}

fn canonical_ids() -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32], [4; 32])
}

#[test]
fn canonical_profiles_validate_with_deterministic_digests() {
    let materializer = canonical_materializer_profile().expect("materializer");
    let unitizer = canonical_unitizer_profile().expect("unitizer");
    assert_eq!(materializer.revision(), CANONICAL_MATERIALIZER_REVISION);
    assert_eq!(unitizer.revision(), CANONICAL_UNITIZER_REVISION);
    assert_eq!(
        canonical_materializer_digest().expect("digest"),
        *search_materializer::api::profile_digest(&materializer).as_bytes()
    );
    assert_eq!(
        canonical_unitizer_digest().expect("digest"),
        *search_unitizer::unitizer_profile_digest(&unitizer).as_bytes()
    );
    assert_eq!(
        canonical_materializer_digest().expect("first"),
        canonical_materializer_digest().expect("second")
    );
    assert_eq!(
        canonical_unitizer_digest().expect("first"),
        canonical_unitizer_digest().expect("second")
    );
}

#[test]
fn digest_algorithm_tags_are_explicit_and_distinct() {
    assert_eq!(CONTENT_DIGEST_ALGORITHM, DIGEST_ALGORITHM_SHA256);
    assert_eq!(REPRESENTATION_DIGEST_ALGORITHM, DIGEST_ALGORITHM_BLAKE3_256);
    assert_eq!(MANIFEST_DIGEST_ALGORITHM, DIGEST_ALGORITHM_SHA256);
    assert_ne!(
        REPRESENTATION_DIGEST_ALGORITHM, CONTENT_DIGEST_ALGORITHM,
        "BLAKE3 representation must never relabel SHA-256 content"
    );
}

#[test]
fn leading_bom_is_an_explicit_gap_not_a_silent_layout() {
    let (namespace, source, revision, content) = canonical_ids();
    let mut with_bom = vec![0xEF, 0xBB, 0xBF];
    with_bom.extend_from_slice(b"needle");
    let (representation, body) =
        encode_canonical_preparation(&with_bom, &namespace, &source, &revision, &content, 9)
            .expect("bom");
    assert_eq!(body, vec![6]);
    assert_eq!(preparation_gap(&body), Ok(Some("DIRECT_REVISION_HAS_BOM")));
    verify_canonical_representation(
        &representation,
        &with_bom,
        &namespace,
        &source,
        &revision,
        &content,
        9,
    )
    .expect("verify bom");
    let (plain_repr, plain_body) =
        encode_canonical_preparation(b"needle", &namespace, &source, &revision, &content, 6)
            .expect("plain");
    assert!(preparation_gap(&plain_body).expect("gap").is_none());
    assert_ne!(representation, plain_repr);
}

#[test]
fn representation_binds_bytes_profiles_and_rejects_tamper() {
    let (namespace, source, revision, content) = canonical_ids();
    let (first, _) =
        encode_canonical_preparation(b"same", &namespace, &source, &revision, &content, 4)
            .expect("first");
    let (second, _) =
        encode_canonical_preparation(b"same", &namespace, &source, &revision, &content, 4)
            .expect("second");
    assert_eq!(first, second, "deterministic across restarts");
    let (changed, _) =
        encode_canonical_preparation(b"same!", &namespace, &source, &revision, &content, 5)
            .expect("changed");
    assert_ne!(first, changed);
    assert_eq!(
        verify_canonical_representation(
            &first,
            b"tampered",
            &namespace,
            &source,
            &revision,
            &content,
            8
        ),
        Err("DIRECT_PREPARATION_BINDING_MISMATCH")
    );
    assert_eq!(
        verify_canonical_representation(
            &first, b"same", &namespace, &source, &[9; 32], &content, 4
        ),
        Err("DIRECT_PREPARATION_BINDING_MISMATCH")
    );
}

#[test]
fn spine_gate_accepts_complete_denominator_and_rejects_narrowing() {
    assert!(verify_spine_gate(3, 3, true, true, false).is_ok());
    assert!(verify_spine_gate(0, 0, true, true, false).is_ok());
    assert!(verify_spine_gate(3, 2, false, false, false).is_ok());
    assert!(verify_spine_gate(3, 2, true, false, true).is_ok());
    assert_eq!(
        verify_spine_gate(3, 3, false, true, false),
        Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID")
    );
    assert_eq!(
        verify_spine_gate(3, 2, true, true, false),
        Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID")
    );
    assert_eq!(
        verify_spine_gate(3, 3, true, true, true),
        Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID")
    );
}

#[test]
fn corpus_budget_never_widens_shared_per_query_limits() {
    const {
        assert!(CANONICAL_CORPUS_BUDGET.max_sources > 0);
        assert!(CANONICAL_CORPUS_BUDGET.max_source_bytes > 0);
    }
    assert_eq!(CANONICAL_CORPUS_BUDGET.max_matches, MAX_SCAN_MATCHES);
    assert_eq!(CANONICAL_CORPUS_BUDGET.max_gaps, 100_000);
}

#[test]
fn source_backed_match_validation_rejects_unproven_ranges() {
    assert!(validate_source_backed_match("needle here", "needle", false, 0, 6).is_ok());
    assert!(validate_source_backed_match("xNEEDLE", "needle", true, 1, 7).is_ok());
    assert!(validate_source_backed_match("needle here", "needle", false, 1, 7).is_err());
    assert!(validate_source_backed_match("needle", "needle", false, 0, 7).is_err());
    assert!(validate_source_backed_match("needle", "needle", false, 0, 0).is_err());
    assert!(validate_source_backed_match("needle", "needle", false, 2, 2).is_err());
    assert!(validate_source_backed_match("βγ", "β", false, 0, 1).is_err());
    assert!(validate_source_backed_match("needle", "NEEDLE", false, 0, 6).is_err());
}

#[test]
fn truncated_and_reordered_bodies_fail_closed() {
    assert_eq!(preparation_gap(&[]), Err("DIRECT_PREPARATION_INVALID"));
    assert_eq!(preparation_gap(&[0]), Err("DIRECT_PREPARATION_INVALID"));
    assert_eq!(preparation_gap(&[7]), Err("DIRECT_PREPARATION_INVALID"));
    assert_eq!(preparation_gap(&[1, 0]), Err("DIRECT_PREPARATION_INVALID"));
    assert_eq!(preparation_gap(&[6, 6]), Err("DIRECT_PREPARATION_INVALID"));
}
