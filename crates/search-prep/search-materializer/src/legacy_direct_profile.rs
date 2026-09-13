//! Canonical legacy DIRECT materializer profile construction.

use search_contracts::Blake3Digest32;

use crate::MaterializationError;
use crate::profile::{
    BomPolicy, CoordinateSpace, InvalidSequencePolicy, LossBehavior,
    MaterializationProfileLimits, MaterializerProfileDescriptor, NewlinePolicy,
    SourceEncoding, SourceKind, UnicodeNormalization, ValidatedMaterializerProfile,
    validate_materializer_profile,
};

/// Canonical legacy DIRECT materializer profile name.
pub const LEGACY_DIRECT_MATERIALIZER_NAME: &str = "direct-exact-utf8";
/// Canonical legacy DIRECT materializer profile revision.
pub const LEGACY_DIRECT_MATERIALIZER_REVISION: u64 = 1;

/// Builds the validated exact-UTF8 materializer profile used by legacy DIRECT.
///
/// The caller supplies the real BLAKE3 golden-fixture digest and the exact
/// bounded source-byte ceiling. All behavioral fields remain closed here:
/// UTF-8 only, BOM rejection, exact newline preservation, no Unicode
/// normalization and rejection of every lossy transform.
pub fn legacy_direct_materializer_profile(
    max_input_bytes: u64,
    golden_fixture_digest: Blake3Digest32,
) -> Result<ValidatedMaterializerProfile, MaterializationError> {
    let descriptor = MaterializerProfileDescriptor {
        profile_name: LEGACY_DIRECT_MATERIALIZER_NAME.to_owned(),
        profile_revision: LEGACY_DIRECT_MATERIALIZER_REVISION,
        source_kinds: vec![SourceKind::Text, SourceKind::Code],
        encodings: vec![SourceEncoding::Utf8],
        bom_policy: BomPolicy::RejectWhenPresent,
        invalid_sequence_policy: InvalidSequencePolicy::Reject,
        newline_policy: NewlinePolicy::PreserveExact,
        unicode_normalization: UnicodeNormalization::None,
        loss_behavior: LossBehavior::RejectOnAnyLoss,
        limits: MaterializationProfileLimits {
            max_input_bytes,
            max_output_bytes: max_input_bytes,
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
        golden_fixture_digest,
    };
    validate_materializer_profile(&descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_profile_is_closed_and_deterministic() {
        let golden = Blake3Digest32::from_bytes([7; 32]);
        let first = legacy_direct_materializer_profile(16 * 1024 * 1024, golden)
            .expect("profile");
        let second = legacy_direct_materializer_profile(16 * 1024 * 1024, golden)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(first.name(), LEGACY_DIRECT_MATERIALIZER_NAME);
        assert_eq!(first.revision(), LEGACY_DIRECT_MATERIALIZER_REVISION);
        assert_eq!(first.encodings(), &[SourceEncoding::Utf8]);
        assert_eq!(first.bom_policy(), BomPolicy::RejectWhenPresent);
        assert_eq!(first.newline_policy(), NewlinePolicy::PreserveExact);
        assert_eq!(first.loss_behavior(), LossBehavior::RejectOnAnyLoss);
        assert_eq!(first.limits().max_input_bytes, 16 * 1024 * 1024);
        assert_eq!(first.limits().max_output_bytes, 16 * 1024 * 1024);
    }

    #[test]
    fn invalid_bounds_or_golden_digest_fail_closed() {
        assert_eq!(
            legacy_direct_materializer_profile(0, Blake3Digest32::from_bytes([7; 32])),
            Err(MaterializationError::ProfileInvalid)
        );
        assert_eq!(
            legacy_direct_materializer_profile(
                16 * 1024 * 1024,
                Blake3Digest32::from_bytes([0; 32])
            ),
            Err(MaterializationError::ProfileInvalid)
        );
    }
}
