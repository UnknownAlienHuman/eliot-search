//! Exact manifest verification and identity-only diff.

use crate::{UnitizationError, UnitizationInput};
use search_contracts::Blake3Digest32;

use super::build::assemble_manifest;
use super::model::{
    MaterializerProvenance, UnitDescriptor, UnitManifest, UnitManifestDiff,
    UnitManifestVerificationReceipt,
};
use super::profile::{ValidatedUnitizerProfile, unitizer_profile_digest};
use super::spec::UNIT_MANIFEST_DIGEST_ALGORITHM;

/// Domain-separated digest over the canonical manifest bytes.
#[must_use]
pub const fn manifest_digest(manifest: &UnitManifest) -> Blake3Digest32 {
    manifest.manifest_digest
}

/// Recomputes unit IDs, ordering, spans, binding and the manifest digest and
/// proves complete profile-defined accounting. It does not assert filesystem
/// currentness or indexed publication.
pub fn verify_unit_manifest(
    manifest: &UnitManifest,
    input: &UnitizationInput,
    provenance: &MaterializerProvenance,
    profile: &ValidatedUnitizerProfile,
) -> Result<UnitManifestVerificationReceipt, UnitizationError> {
    if unitizer_profile_digest(profile) != profile.id() {
        return Err(UnitizationError::UnitizerProfileMismatch);
    }
    if manifest.unitizer_profile_id != profile.id()
        || manifest.unitizer_profile_revision != profile.revision()
        || manifest.unitizer_limits != profile.limits()
    {
        return Err(UnitizationError::UnitizerProfileMismatch);
    }
    if manifest.digest_algorithm != UNIT_MANIFEST_DIGEST_ALGORITHM {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    if manifest.source_id != input.source_id
        || manifest.revision != input.revision
        || manifest.content_digest != input.content_digest
    {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    if manifest.representation_id != provenance.representation_id
        || manifest.materializer_profile_digest != provenance.materializer_profile_digest
        || manifest.canonical_digest != provenance.canonical_digest
        || manifest.coordinate_digest != provenance.coordinate_digest
        || manifest.loss_digest != provenance.loss_digest
    {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let expected = assemble_manifest(input, provenance, profile, manifest.digest_algorithm)?;
    if expected.units != manifest.units
        || expected.input_bytes != manifest.input_bytes
        || expected.emitted_bytes != manifest.emitted_bytes
        || expected.line_count != manifest.line_count
    {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    if expected.manifest_digest != manifest.manifest_digest {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    Ok(UnitManifestVerificationReceipt {
        source_id: manifest.source_id.clone(),
        revision: manifest.revision,
        representation_id: manifest.representation_id,
        unitizer_profile_id: manifest.unitizer_profile_id,
        materializer_profile_digest: manifest.materializer_profile_digest,
        unit_count: u64::try_from(manifest.units.len())
            .map_err(|_| UnitizationError::OffsetOverflow)?,
        manifest_digest: manifest.manifest_digest,
    })
}

/// Returns the exact identity difference between two manifests.
///
/// Reports created, retained and retired unit identities with changed
/// identity reasons carried by the two manifest digests. Retained requires
/// exact unit-identity digest equality; heuristic span or name similarity
/// can never retain a unit.
pub fn diff_unit_manifests(
    old: &UnitManifest,
    new: &UnitManifest,
) -> Result<UnitManifestDiff, UnitizationError> {
    if old.digest_algorithm != new.digest_algorithm {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    let mut old_sorted: Vec<Blake3Digest32> =
        old.units.iter().map(UnitDescriptor::unit_digest).collect();
    old_sorted.sort();
    let mut new_sorted: Vec<Blake3Digest32> =
        new.units.iter().map(UnitDescriptor::unit_digest).collect();
    new_sorted.sort();
    let mut retained = Vec::new();
    let mut created = Vec::new();
    for digest in new.units.iter().map(UnitDescriptor::unit_digest) {
        if old_sorted.binary_search(&digest).is_ok() {
            retained.push(digest);
        } else {
            created.push(digest);
        }
    }
    let mut retired = Vec::new();
    for digest in old.units.iter().map(UnitDescriptor::unit_digest) {
        if new_sorted.binary_search(&digest).is_err() {
            retired.push(digest);
        }
    }
    Ok(UnitManifestDiff {
        old_digest: old.manifest_digest,
        new_digest: new.manifest_digest,
        retained,
        created,
        retired,
    })
}
