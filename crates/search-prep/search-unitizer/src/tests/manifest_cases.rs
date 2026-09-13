use super::*;

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

    let bytes = canonicalize_unit_manifest(&manifest).expect("canonical");
    let reopened = decode_unit_manifest(bytes.as_slice(), 1_048_576).expect("reopen");
    assert_eq!(reopened, manifest);
    verify_unit_manifest(&reopened, &manifest_input(text), &provenance, &profile)
        .expect("reverify");

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
    const HEX: &str = include_str!("../testdata/unit_manifest_v1.hex");
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
