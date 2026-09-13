//! DIRECT preparation profiles, bounds and digest-algorithm bindings.

use search_exact::literal::LiteralLimits;
use search_materializer::MaterializationLimits;
use search_unitizer::UnitizationLimits;

use crate::development::{
    MAX_SCAN_INPUT_BYTES, MAX_SCAN_MATCHES, MAX_SCAN_QUERY_BYTES,
};
use crate::sha256;

/// Maximum encoded exact layout bytes retained by the DIRECT adapter.
pub const MAX_LAYOUT_BYTES: usize = 64 * 1024 * 1024 - 512;

pub(super) const MATERIALIZATION: MaterializationLimits = MaterializationLimits {
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    max_output_bytes: MAX_SCAN_INPUT_BYTES,
    max_lines: 1_000_000,
};

pub(super) const UNITIZATION: UnitizationLimits = UnitizationLimits {
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    preferred_unit_bytes: 16 * 1024,
    max_unit_bytes: 64 * 1024,
    max_lines: 1_000_000,
    max_units: 1_000_000,
};

pub(super) const LITERAL: LiteralLimits = LiteralLimits {
    max_query_bytes: MAX_SCAN_QUERY_BYTES,
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    max_chunks: 1_000_000,
    max_matches: MAX_SCAN_MATCHES,
};

/// Binds every preparation algorithm and limit, not query options, into the disk key.
pub fn profile_digest() -> [u8; 32] {
    let mut settings = Vec::new();
    for value in [
        MATERIALIZATION.max_input_bytes,
        MATERIALIZATION.max_output_bytes,
        MATERIALIZATION.max_lines,
        UNITIZATION.max_input_bytes,
        UNITIZATION.preferred_unit_bytes,
        UNITIZATION.max_unit_bytes,
        UNITIZATION.max_lines,
        UNITIZATION.max_units,
        MAX_LAYOUT_BYTES,
    ] {
        settings.extend_from_slice(&(value as u64).to_be_bytes());
    }
    sha256::digest_parts(
        b"eliot-search/direct-preparation-profile/v1",
        &[
            b"utf8-exact;no-normalization;crlf-cr-lf;nul-denied;controls-ceil-1pct-min4/v1",
            UnitizationLimits::LAYOUT_FORMAT.as_bytes(),
            &settings,
        ],
    )
}

/// Canonical DIRECT materializer profile name: exact UTF-8 preservation only.
pub const CANONICAL_MATERIALIZER_NAME: &str = "direct-exact-utf8";
/// Canonical DIRECT materializer profile revision.
pub const CANONICAL_MATERIALIZER_REVISION: u64 = 1;
/// Canonical DIRECT unitizer profile name.
pub const CANONICAL_UNITIZER_NAME: &str = "direct-exact-units";
/// Canonical DIRECT unitizer profile revision.
pub const CANONICAL_UNITIZER_REVISION: u64 = 1;

/// BLAKE3-256 wire tag matching `search-contracts::DigestAlgorithm`.
pub const DIGEST_ALGORITHM_BLAKE3_256: u8 = 1;
/// SHA-256 wire tag matching `search-contracts::DigestAlgorithm`.
pub const DIGEST_ALGORITHM_SHA256: u8 = 2;

/// Source content uses SHA-256 hex identities from the existing DIRECT catalog.
pub const CONTENT_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_SHA256;
/// Representation identities use real BLAKE3, never SHA-256 relabelled.
pub const REPRESENTATION_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_BLAKE3_256;
/// Manifest-envelope digests retain the existing SHA-256 profile.
pub const MANIFEST_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_SHA256;

/// Builds the validated canonical materializer profile for DIRECT.
///
/// The profile accepts exact UTF-8 only, rejects BOM-bearing input, preserves
/// line endings and rejects every lossy transform. DIRECT has no coordinate
/// reprojection fallback.
pub fn canonical_materializer_profile()
-> Result<search_materializer::api::ValidatedMaterializerProfile, &'static str> {
    use search_contracts::Blake3Digest32;
    use search_materializer::api::{
        BomPolicy, CoordinateSpace, InvalidSequencePolicy, LossBehavior,
        MaterializationProfileLimits, MaterializerProfileDescriptor, NewlinePolicy,
        SourceEncoding, SourceKind, UnicodeNormalization, validate_materializer_profile,
    };

    let golden = Blake3Digest32::from_bytes(
        *blake3::hash(
            format!("{CANONICAL_MATERIALIZER_NAME}:{CANONICAL_MATERIALIZER_REVISION}").as_bytes(),
        )
        .as_bytes(),
    );
    let descriptor = MaterializerProfileDescriptor {
        profile_name: CANONICAL_MATERIALIZER_NAME.to_owned(),
        profile_revision: CANONICAL_MATERIALIZER_REVISION,
        source_kinds: vec![SourceKind::Text, SourceKind::Code],
        encodings: vec![SourceEncoding::Utf8],
        bom_policy: BomPolicy::RejectWhenPresent,
        invalid_sequence_policy: InvalidSequencePolicy::Reject,
        newline_policy: NewlinePolicy::PreserveExact,
        unicode_normalization: UnicodeNormalization::None,
        loss_behavior: LossBehavior::RejectOnAnyLoss,
        limits: MaterializationProfileLimits {
            max_input_bytes: u64::try_from(MAX_SCAN_INPUT_BYTES)
                .map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID")?,
            max_output_bytes: u64::try_from(MAX_SCAN_INPUT_BYTES)
                .map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID")?,
            max_lines: 1_000_000,
            max_map_segments: 1_000_032,
            max_loss_records: 1_000_032,
            max_steps: 64 * 1024 * 1024,
        },
        coordinate_spaces: vec![
            CoordinateSpace::NativeBytes,
            CoordinateSpace::DecodedScalar,
            CoordinateSpace::CanonicalScalar,
        ],
        golden_fixture_digest: golden,
    };
    validate_materializer_profile(&descriptor)
        .map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID")
}

/// Builds the validated canonical unitizer profile for DIRECT.
///
/// Its limits equal the live unitization bounds, so the digest binds the exact
/// boundary decisions used by layout encoding and decoding.
pub fn canonical_unitizer_profile()
-> Result<search_unitizer::ValidatedUnitizerProfile, &'static str> {
    use search_unitizer::{UnitizerProfileDescriptor, validate_unitizer_profile};

    let descriptor = UnitizerProfileDescriptor {
        profile_name: CANONICAL_UNITIZER_NAME.to_owned(),
        profile_revision: CANONICAL_UNITIZER_REVISION,
        limits: UNITIZATION,
    };
    validate_unitizer_profile(&descriptor)
        .map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID")
}

/// Canonical materializer profile digest bytes, not a receipt.
pub fn canonical_materializer_digest() -> Result<[u8; 32], &'static str> {
    use search_materializer::api::profile_digest;
    Ok(*profile_digest(&canonical_materializer_profile()?).as_bytes())
}

/// Canonical unitizer profile digest bytes, not a receipt.
pub fn canonical_unitizer_digest() -> Result<[u8; 32], &'static str> {
    use search_unitizer::unitizer_profile_digest;
    Ok(*unitizer_profile_digest(&canonical_unitizer_profile()?).as_bytes())
}
