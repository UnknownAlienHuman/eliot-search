use super::*;

fn input(text: &str, lines: Vec<SourceLineSpan>) -> UnitizationInput {
    UnitizationInput::new(
        OpaqueId::new("source:test").expect("source"),
        NonZeroRevision::new(1).expect("revision"),
        Blake3Digest32::from_bytes([1; 32]),
        text.to_owned(),
        lines,
        Some(ReceiptRef::new("receipt:materialization").expect("receipt")),
    )
}

fn simple_lines(text: &str) -> Vec<SourceLineSpan> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        let (content_end, end) = if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            (index, index + 2)
        } else if matches!(bytes[index], b'\n' | b'\r') {
            (index, index + 1)
        } else {
            index += 1;
            continue;
        };
        spans.push(SourceLineSpan {
            line_index: u64::try_from(spans.len()).unwrap(),
            source_start: u64::try_from(start).unwrap(),
            source_end: u64::try_from(end).unwrap(),
            content_end: u64::try_from(content_end).unwrap(),
        });
        start = end;
        index = end;
    }
    if start < bytes.len() {
        spans.push(SourceLineSpan {
            line_index: u64::try_from(spans.len()).unwrap(),
            source_start: u64::try_from(start).unwrap(),
            source_end: u64::try_from(bytes.len()).unwrap(),
            content_end: u64::try_from(bytes.len()).unwrap(),
        });
    }
    spans
}

fn limits(preferred: usize, maximum: usize) -> UnitizationLimits {
    UnitizationLimits {
        preferred_unit_bytes: preferred,
        max_unit_bytes: maximum,
        ..DEFAULT_UNITIZATION_LIMITS
    }
}

#[test]
fn exact_reconstruction_has_no_gaps_or_overlap() {
    let text = "alpha\r\nbeta\nγamma";
    let result = unitize(input(text, simple_lines(text)), limits(7, 12)).unwrap();
    assert_eq!(
        result.units.iter().map(TextUnit::text).collect::<String>(),
        text
    );
    assert_eq!(result.receipt.input_bytes, result.receipt.emitted_bytes);
}

#[test]
fn line_boundaries_are_preferred_when_line_fits_hard_limit() {
    let text = "one\ntwo\nthree\n";
    let result = unitize(input(text, simple_lines(text)), limits(5, 10)).unwrap();
    assert_eq!(result.units[0].text(), "one\n");
    assert!(result.units[0].ends_at_line_boundary);
    assert_eq!(result.units[1].text(), "two\n");
}

#[test]
fn overlong_unicode_line_splits_only_at_character_boundaries() {
    let text = "αβγδεζηθ";
    let result = unitize(input(text, simple_lines(text)), limits(5, 6)).unwrap();
    assert!(result.units.len() > 1);
    assert!(result.units.iter().all(|unit| unit.len() <= 6));
    assert_eq!(
        result.units.iter().map(TextUnit::text).collect::<String>(),
        text
    );
}

#[test]
fn crlf_is_never_split_when_the_line_fits_hard_limit() {
    let text = "abcd\r\nef";
    let result = unitize(input(text, simple_lines(text)), limits(5, 8)).unwrap();
    assert_eq!(result.units[0].text(), "abcd\r\n");
}

#[test]
fn output_is_deterministic() {
    let text = "a\nbb\nccc\ndddd";
    let first = unitize(input(text, simple_lines(text)), limits(4, 7)).unwrap();
    let second = unitize(input(text, simple_lines(text)), limits(4, 7)).unwrap();
    assert_eq!(first, second);
}

#[test]
fn malformed_line_coverage_is_rejected() {
    let lines = vec![SourceLineSpan {
        line_index: 0,
        source_start: 1,
        source_end: 3,
        content_end: 3,
    }];
    assert_eq!(
        unitize(input("abc", lines), limits(2, 3)),
        Err(UnitizationError::InvalidLineSpan)
    );
}

#[test]
fn line_inventory_must_cover_all_bytes() {
    let lines = vec![SourceLineSpan {
        line_index: 0,
        source_start: 0,
        source_end: 2,
        content_end: 2,
    }];
    assert_eq!(
        unitize(input("abc", lines), limits(2, 3)),
        Err(UnitizationError::LineCoverageMismatch)
    );
}

#[test]
fn finite_unit_limit_is_fail_closed() {
    let text = "abcdefgh";
    let cap = UnitizationLimits {
        max_units: 3,
        ..limits(2, 2)
    };
    assert_eq!(
        unitize(input(text, simple_lines(text)), cap),
        Err(UnitizationError::TooManyUnits)
    );
}

#[test]
fn debug_does_not_dump_source_or_unit_text() {
    let text = "sensitive source text";
    let input = input(text, simple_lines(text));
    assert!(!format!("{input:?}").contains(text));
    let result = unitize(input, limits(8, 16)).unwrap();
    assert!(!format!("{result:?}").contains(text));
}

#[test]
fn hidden_line_terminators_are_rejected() {
    let text = "a\nb";
    let lines = vec![SourceLineSpan {
        line_index: 0,
        source_start: 0,
        source_end: 3,
        content_end: 3,
    }];
    assert_eq!(
        unitize_text(text, &lines, limits(2, 4)),
        Err(UnitizationError::InvalidLineEnding)
    );
}

#[test]
fn invented_unterminated_middle_lines_are_rejected() {
    let lines = vec![
        SourceLineSpan {
            line_index: 0,
            source_start: 0,
            source_end: 1,
            content_end: 1,
        },
        SourceLineSpan {
            line_index: 1,
            source_start: 1,
            source_end: 3,
            content_end: 3,
        },
    ];
    assert_eq!(
        unitize_text("abc", &lines, limits(2, 4)),
        Err(UnitizationError::InvalidLineEnding)
    );
}

#[test]
fn split_crlf_line_inventory_is_rejected() {
    let lines = vec![
        SourceLineSpan {
            line_index: 0,
            source_start: 0,
            source_end: 2,
            content_end: 1,
        },
        SourceLineSpan {
            line_index: 1,
            source_start: 2,
            source_end: 3,
            content_end: 2,
        },
    ];
    assert_eq!(
        unitize_text("a\r\n", &lines, limits(2, 4)),
        Err(UnitizationError::InvalidLineEnding)
    );
}

#[test]
fn raw_ranges_and_receipt_bound_units_agree_for_all_small_sizes() {
    let many_lines = "a\n".repeat(30);
    for text in ["a\r\nb\nc\rd", "αβγδεζηθ", many_lines.as_str()] {
        let lines = simple_lines(text);
        for preferred in 1..=12 {
            let caps = limits(preferred, preferred.max(4));
            let raw = unitize_text(text, &lines, caps).unwrap();
            let bound = unitize(input(text, lines.clone()), caps).unwrap();
            assert_eq!(raw.len(), bound.units.len());
            let mut cursor = 0;
            for (span, unit) in raw.iter().zip(&bound.units) {
                assert_eq!(span.source_start, cursor);
                assert_eq!(&text[span.source_start..span.source_end], unit.text());
                assert_eq!(span.logical_line_start, unit.logical_line_start);
                assert_eq!(span.logical_line_end, unit.logical_line_end);
                assert_eq!(span.starts_at_line_boundary, unit.starts_at_line_boundary);
                assert_eq!(span.ends_at_line_boundary, unit.ends_at_line_boundary);
                cursor = span.source_end;
            }
            assert_eq!(cursor, text.len());
        }
    }
}

#[test]
fn empty_layout_is_not_a_receipt_bound_revision() {
    assert!(unitize_text("", &[], limits(2, 4)).unwrap().is_empty());
    assert_eq!(
        unitize(input("", vec![]), limits(2, 4)),
        Err(UnitizationError::EmptyInput)
    );
}

#[test]
fn hard_limit_smaller_than_one_scalar_fails_without_progress() {
    assert_eq!(
        unitize_text("𐀀", &simple_lines("𐀀"), limits(1, 3)),
        Err(UnitizationError::NoProgress)
    );
}

// --- T16 durable unit manifests: failing-first contract tests. ---

fn manifest_profile(revision: u64) -> ValidatedUnitizerProfile {
    let descriptor = UnitizerProfileDescriptor {
        profile_name: "test-unitizer-v1".to_owned(),
        profile_revision: revision,
        limits: limits(8, 16),
    };
    validate_unitizer_profile(&descriptor).expect("valid test profile")
}

fn manifest_provenance() -> MaterializerProvenance {
    MaterializerProvenance::new(
        [9; 32],
        Blake3Digest32::from_bytes([10; 32]),
        Blake3Digest32::from_bytes([11; 32]),
        Blake3Digest32::from_bytes([12; 32]),
        Blake3Digest32::from_bytes([13; 32]),
    )
}

fn manifest_input(text: &str) -> UnitizationInput {
    input(text, simple_lines(text))
}

#[test]
fn durable_manifest_binds_source_representation_and_profiles() {
    let text = "alpha\nbeta\ngamma\n";
    let profile = manifest_profile(1);
    let provenance = manifest_provenance();
    let manifest = build_unit_manifest(
        &manifest_input(text),
        &provenance,
        &profile,
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("manifest");
    assert_eq!(manifest.source_id().as_str(), "source:test");
    assert_eq!(manifest.revision().get(), 1);
    assert_eq!(
        manifest.representation_id(),
        Blake3Digest32::from_bytes([10; 32])
    );
    assert_eq!(manifest.materializer_profile_digest(), &[9; 32]);
    assert_eq!(manifest.unitizer_profile_id(), profile.id());
    assert_eq!(manifest.unitizer_profile_revision(), 1);
    assert_eq!(manifest.digest_algorithm(), UNIT_MANIFEST_DIGEST_ALGORITHM);
    assert_eq!(manifest.unit_count(), manifest.units().len());
    assert_eq!(manifest.emitted_bytes(), manifest.input_bytes());
    assert!(!format!("{manifest:?}").contains(text));
}

#[test]
fn same_input_yields_byte_identical_manifest() {
    let text = "a\nbb\nccc\ndddd\n";
    let profile = manifest_profile(3);
    let provenance = manifest_provenance();
    let first = build_unit_manifest(
        &manifest_input(text),
        &provenance,
        &profile,
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("first");
    let second = build_unit_manifest(
        &manifest_input(text),
        &provenance,
        &profile,
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("second");
    assert_eq!(first, second);
    assert_eq!(
        canonicalize_unit_manifest(&first)
            .expect("canonical")
            .as_slice(),
        canonicalize_unit_manifest(&second)
            .expect("canonical")
            .as_slice()
    );
    assert_eq!(manifest_digest(&first), manifest_digest(&second));
}

#[test]
fn manifest_canonical_bytes_and_digest_are_golden() {
    let text = "one\ntwo\nthree\n";
    let profile = manifest_profile(1);
    let provenance = manifest_provenance();
    let manifest = build_unit_manifest(
        &manifest_input(text),
        &provenance,
        &profile,
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("manifest");
    assert_eq!(
        manifest.unitizer_profile_id().to_string(),
        "da31031a7fd84d455e0cb4e37ca71979f12091e069eadcb70aaa4e028970044b"
    );
    assert_eq!(manifest.units().len(), 2);
    assert_eq!(
        manifest.units()[0].unit_digest().to_string(),
        "5be178cc7bbbc66e836333d0d6d5df12aee142a04a80fd561f21a802050ebb3a"
    );
    assert_eq!(
        manifest.units()[1].unit_digest().to_string(),
        "e9c489bfc08b96b065eb71cf48c024e46683be72663558fbddb8a3a976433f6d"
    );
    assert_eq!(
        manifest_digest(&manifest).to_string(),
        "e8ccef05ef1f0fd89c1244cbf4e5d1b2ac910d06cbe339a7caffe3469be605d0"
    );
    let canonical = canonicalize_unit_manifest(&manifest).expect("canonical");
    assert_eq!(canonical.len(), 514);
    assert_eq!(canonical.as_slice(), hex_manifest_golden().as_slice());
}

#[test]
fn changed_profile_revision_changes_manifest_identity() {
    let text = "alpha\nbeta\ngamma\n";
    let provenance = manifest_provenance();
    let old = build_unit_manifest(
        &manifest_input(text),
        &provenance,
        &manifest_profile(1),
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("old");
    let new = build_unit_manifest(
        &manifest_input(text),
        &provenance,
        &manifest_profile(2),
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("new");
    assert_ne!(old.unitizer_profile_id(), new.unitizer_profile_id());
    assert_ne!(manifest_digest(&old), manifest_digest(&new));
    assert_eq!(
        classify_unitizer_profile_change(&manifest_profile(1), &manifest_profile(2)),
        UnitizerProfileChange::ReunitizeAndReproject
    );
    assert_eq!(
        classify_unitizer_profile_change(&manifest_profile(1), &manifest_profile(1)),
        UnitizerProfileChange::Noop
    );
    let diff = diff_unit_manifests(&old, &new).expect("diff");
    assert_eq!(diff.retained().len(), 0);
    assert_eq!(diff.created().len(), new.units().len());
    assert_eq!(diff.retired().len(), old.units().len());
}

#[test]
fn changed_representation_changes_unit_identity() {
    let text = "alpha\nbeta\ngamma\n";
    let profile = manifest_profile(1);
    let old = build_unit_manifest(
        &manifest_input(text),
        &manifest_provenance(),
        &profile,
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("old");
    let altered = MaterializerProvenance::new(
        [9; 32],
        Blake3Digest32::from_bytes([77; 32]),
        Blake3Digest32::from_bytes([11; 32]),
        Blake3Digest32::from_bytes([12; 32]),
        Blake3Digest32::from_bytes([13; 32]),
    );
    let new = build_unit_manifest(
        &manifest_input(text),
        &altered,
        &profile,
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("new");
    assert_ne!(old.units()[0].unit_digest(), new.units()[0].unit_digest());
    assert_ne!(manifest_digest(&old), manifest_digest(&new));
}

#[test]
fn verify_accepts_exact_and_rejects_tamper() {
    let text = "alpha\nbeta\ngamma\n";
    let profile = manifest_profile(1);
    let provenance = manifest_provenance();
    let manifest = build_unit_manifest(
        &manifest_input(text),
        &provenance,
        &profile,
        UNIT_MANIFEST_DIGEST_ALGORITHM,
        1_048_576,
    )
    .expect("manifest");
    let receipt = verify_unit_manifest(&manifest, &manifest_input(text), &provenance, &profile)
        .expect("verify");
    assert_eq!(
        receipt.unit_count(),
        u64::try_from(manifest.units().len()).expect("count")
    );
    assert_eq!(receipt.manifest_digest(), manifest_digest(&manifest));

    // Restart lookup: decode durable bytes without source text, then verify.
    let bytes = canonicalize_unit_manifest(&manifest).expect("canonical");
    let reopened = decode_unit_manifest(bytes.as_slice(), 1_048_576).expect("reopen");
    assert_eq!(reopened, manifest);
    verify_unit_manifest(&reopened, &manifest_input(text), &provenance, &profile)
        .expect("reverify");

    // Wrong profile / provenance / representation fail closed.
    assert_eq!(
        verify_unit_manifest(
            &manifest,
            &manifest_input(text),
            &provenance,
            &manifest_profile(2)
        ),
        Err(UnitizationError::UnitizerProfileMismatch)
    );
    assert_eq!(
        verify_unit_manifest(
            &manifest,
            &manifest_input(text),
            &manifest_provenance_tampered(),
            &profile
        ),
        Err(UnitizationError::UnitManifestIncomplete)
    );
    assert_eq!(
        verify_unit_manifest(
            &manifest,
            &manifest_input("alpha\nbeta\nCHANGED\n"),
            &provenance,
            &profile
        ),
        Err(UnitizationError::UnitManifestDigestMismatch)
    );

    // Truncated / wrong-domain / wrong-profile durable bytes fail closed.
    assert_eq!(
        decode_unit_manifest(&bytes.as_slice()[..bytes.len() - 1], 1_048_576),
        Err(UnitizationError::UnitManifestIncomplete)
    );
    let mut bad_magic = bytes.as_slice().to_vec();
    bad_magic[0] ^= 0xFF;
    assert_eq!(
        decode_unit_manifest(&bad_magic, 1_048_576),
        Err(UnitizationError::UnitManifestIncomplete)
    );
    let mut bad_algorithm = bytes.as_slice().to_vec();
    bad_algorithm[10] ^= 0xFF;
    assert_eq!(
        decode_unit_manifest(&bad_algorithm, 1_048_576),
        Err(UnitizationError::UnitManifestDigestMismatch)
    );
}

#[test]
fn build_rejects_wrong_digest_algorithm_without_reinterpretation() {
    let text = "alpha\nbeta\n";
    assert_eq!(
        build_unit_manifest(
            &manifest_input(text),
            &manifest_provenance(),
            &manifest_profile(1),
            search_contracts::DigestAlgorithm::Sha256,
            1_048_576,
        ),
        Err(UnitizationError::UnitManifestDigestMismatch)
    );
}

#[test]
fn invalid_profile_descriptors_fail_closed() {
    let bad_name = UnitizerProfileDescriptor {
        profile_name: String::new(),
        profile_revision: 1,
        limits: limits(8, 16),
    };
    assert_eq!(
        validate_unitizer_profile(&bad_name),
        Err(UnitizationError::UnitizerProfileInvalid)
    );
    let bad_revision = UnitizerProfileDescriptor {
        profile_name: "test-unitizer-v1".to_owned(),
        profile_revision: 0,
        limits: limits(8, 16),
    };
    assert_eq!(
        validate_unitizer_profile(&bad_revision),
        Err(UnitizationError::UnitizerProfileInvalid)
    );
    let bad_limits = UnitizerProfileDescriptor {
        profile_name: "test-unitizer-v1".to_owned(),
        profile_revision: 1,
        limits: UnitizationLimits {
            preferred_unit_bytes: 32,
            max_unit_bytes: 8,
            ..DEFAULT_UNITIZATION_LIMITS
        },
    };
    assert_eq!(
        validate_unitizer_profile(&bad_limits),
        Err(UnitizationError::InvalidLimits)
    );
}

fn manifest_provenance_tampered() -> MaterializerProvenance {
    MaterializerProvenance::new(
        [8; 32],
        Blake3Digest32::from_bytes([10; 32]),
        Blake3Digest32::from_bytes([11; 32]),
        Blake3Digest32::from_bytes([12; 32]),
        Blake3Digest32::from_bytes([13; 32]),
    )
}

fn hex_manifest_golden() -> Vec<u8> {
    // Package-local golden fixture: exact durable bytes for "one\ntwo\nthree\n"
    // under profile "test-unitizer-v1"/rev 1 with the fixed test provenance.
    // Any boundary, encoding, digest-domain or layout change fails here.
    const HEX: &str = include_str!("testdata/unit_manifest_v1.hex");
    let bytes = HEX.trim().as_bytes();
    assert_eq!(bytes.len() % 2, 0);
    bytes
        .chunks(2)
        .map(|pair| (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]))
        .collect()
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("non-hex golden fixture"),
    }
}
