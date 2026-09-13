//! Baseline profile construction and descriptor validation.

use super::digest::{digest32, profile_digest};
use super::model::{
    BomPolicy, CoordinateSpace, DEFAULT_PROFILE_LIMITS, InvalidSequencePolicy, LossBehavior,
    MAX_PROFILE_NAME_BYTES, MaterializerProfileDescriptor, MaterializerProfileId, NewlinePolicy,
    SourceEncoding, SourceKind, UnicodeNormalization, ValidatedMaterializerProfile,
};
use crate::MaterializationError;
use search_contracts::Blake3Digest32;

/// Baseline profile descriptor: strict UTF-8 plus exact UTF-16 transcoding,
/// recorded BOM handling, exact newlines and recorded-loss behavior.
///
/// The golden fixture digest binds the profile name and revision, so equal
/// names at different revisions qualify as different profiles.
#[must_use]
pub fn baseline_profile_descriptor(name: &str, revision: u64) -> MaterializerProfileDescriptor {
    let golden = Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/golden/v1",
        &[name.as_bytes(), &revision.to_le_bytes()],
    ));
    MaterializerProfileDescriptor {
        profile_name: name.to_string(),
        profile_revision: revision,
        source_kinds: vec![SourceKind::Text, SourceKind::Code],
        encodings: vec![
            SourceEncoding::Utf8,
            SourceEncoding::Utf16Le,
            SourceEncoding::Utf16Be,
        ],
        bom_policy: BomPolicy::StripAndRecord,
        invalid_sequence_policy: InvalidSequencePolicy::Reject,
        newline_policy: NewlinePolicy::PreserveExact,
        unicode_normalization: UnicodeNormalization::None,
        loss_behavior: LossBehavior::RecordAndLowerAssurance,
        limits: DEFAULT_PROFILE_LIMITS,
        coordinate_spaces: vec![
            CoordinateSpace::NativeBytes,
            CoordinateSpace::DecodedScalar,
            CoordinateSpace::CanonicalScalar,
        ],
        golden_fixture_digest: golden,
    }
}

fn has_duplicate<T: PartialEq>(values: &[T]) -> bool {
    for (index, value) in values.iter().enumerate() {
        if values[..index].contains(value) {
            return true;
        }
    }
    false
}

/// Validates a profile descriptor and binds its canonical identity.
///
/// Rejects empty or unbounded names, zero revisions, empty or duplicated
/// kind/encoding sets, zero limits, partial coordinate bases and missing
/// golden fixture digests. Implicit locale/platform defaults can never pass:
/// every behavior is explicit in the descriptor.
pub fn validate_materializer_profile(
    descriptor: &MaterializerProfileDescriptor,
) -> Result<ValidatedMaterializerProfile, MaterializationError> {
    if descriptor.profile_name.is_empty()
        || descriptor.profile_name.len() > MAX_PROFILE_NAME_BYTES
        || !descriptor
            .profile_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(MaterializationError::ProfileInvalid);
    }
    if descriptor.profile_revision == 0 {
        return Err(MaterializationError::ProfileInvalid);
    }
    if descriptor.source_kinds.is_empty() || has_duplicate(&descriptor.source_kinds) {
        return Err(MaterializationError::ProfileInvalid);
    }
    if descriptor.encodings.is_empty() || has_duplicate(&descriptor.encodings) {
        return Err(MaterializationError::ProfileInvalid);
    }
    let limits = descriptor.limits.validate()?;
    if descriptor.coordinate_spaces.len() != 3
        || has_duplicate(&descriptor.coordinate_spaces)
        || !descriptor
            .coordinate_spaces
            .contains(&CoordinateSpace::NativeBytes)
        || !descriptor
            .coordinate_spaces
            .contains(&CoordinateSpace::DecodedScalar)
        || !descriptor
            .coordinate_spaces
            .contains(&CoordinateSpace::CanonicalScalar)
    {
        return Err(MaterializationError::ProfileInvalid);
    }
    if descriptor.golden_fixture_digest == Blake3Digest32::from_bytes([0; 32]) {
        return Err(MaterializationError::ProfileInvalid);
    }
    let mut kinds = descriptor.source_kinds.clone();
    kinds.sort();
    kinds.dedup();
    let mut encodings = descriptor.encodings.clone();
    encodings.sort_by_key(|encoding| encoding.tag());
    encodings.dedup();
    let mut spaces = descriptor.coordinate_spaces.clone();
    spaces.sort_by_key(|space| space.tag());
    spaces.dedup();
    let profile = ValidatedMaterializerProfile {
        name: descriptor.profile_name.clone(),
        revision: descriptor.profile_revision,
        kinds,
        encodings,
        bom_policy: descriptor.bom_policy,
        invalid_sequence_policy: descriptor.invalid_sequence_policy,
        newline_policy: descriptor.newline_policy,
        unicode_normalization: descriptor.unicode_normalization,
        loss_behavior: descriptor.loss_behavior,
        limits,
        spaces,
        golden: descriptor.golden_fixture_digest,
        id: MaterializerProfileId::from_bytes([0; 32]),
    };
    let id = profile_digest(&profile);
    Ok(ValidatedMaterializerProfile { id, ..profile })
}
