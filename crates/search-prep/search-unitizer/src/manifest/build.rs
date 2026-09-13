//! Deterministic unit occurrence and manifest construction.

use crate::{UnitizationError, UnitizationInput};
use search_contracts::{Blake3Digest32, DigestAlgorithm};

use super::codec::{encode_body, encoded_size};
use super::digest::digest32;
use super::model::{MaterializerProvenance, UnitDescriptor, UnitManifest};
use super::profile::{ValidatedUnitizerProfile, unitizer_profile_digest};
use super::spec::{MANIFEST_DOMAIN, UNIT_DOMAIN, UNIT_MANIFEST_DIGEST_ALGORITHM};

fn derive_unit_digest(
    input: &UnitizationInput,
    provenance: &MaterializerProvenance,
    profile: &ValidatedUnitizerProfile,
    ordinal: u64,
    span: &crate::UnitSpan,
) -> Result<Blake3Digest32, UnitizationError> {
    let start = u64::try_from(span.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
    let end = u64::try_from(span.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
    Ok(Blake3Digest32::from_bytes(digest32(
        UNIT_DOMAIN,
        &[
            input.source_id.as_str().as_bytes(),
            &input.revision.get().to_le_bytes(),
            provenance.representation_id.as_bytes(),
            profile.id().as_bytes(),
            &ordinal.to_le_bytes(),
            &start.to_le_bytes(),
            &end.to_le_bytes(),
            &span.logical_line_start.to_le_bytes(),
            &span.logical_line_end.to_le_bytes(),
            &[
                u8::from(span.starts_at_line_boundary),
                u8::from(span.ends_at_line_boundary),
            ],
        ],
    )))
}

pub(super) fn assemble_manifest(
    input: &UnitizationInput,
    provenance: &MaterializerProvenance,
    profile: &ValidatedUnitizerProfile,
    digest_algorithm: DigestAlgorithm,
) -> Result<UnitManifest, UnitizationError> {
    if digest_algorithm != UNIT_MANIFEST_DIGEST_ALGORITHM {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    if unitizer_profile_digest(profile) != profile.id() {
        return Err(UnitizationError::UnitizerProfileMismatch);
    }
    if input.is_empty() {
        return Err(UnitizationError::EmptyInput);
    }
    let spans = crate::unitize_text(input.text(), &input.lines, profile.limits())?;
    if spans.is_empty() {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let input_bytes = u64::try_from(input.len()).map_err(|_| UnitizationError::OffsetOverflow)?;
    let line_count =
        u64::try_from(input.lines.len()).map_err(|_| UnitizationError::OffsetOverflow)?;
    let mut units = Vec::with_capacity(spans.len());
    for (index, span) in spans.iter().enumerate() {
        let ordinal = u64::try_from(index).map_err(|_| UnitizationError::OffsetOverflow)?;
        let start =
            u64::try_from(span.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
        let end = u64::try_from(span.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
        let unit_digest = derive_unit_digest(input, provenance, profile, ordinal, span)?;
        units.push(UnitDescriptor {
            ordinal,
            source_start: start,
            source_end: end,
            logical_line_start: span.logical_line_start,
            logical_line_end: span.logical_line_end,
            starts_at_line_boundary: span.starts_at_line_boundary,
            ends_at_line_boundary: span.ends_at_line_boundary,
            unit_digest,
        });
    }
    let mut ordered: Vec<Blake3Digest32> = units.iter().map(UnitDescriptor::unit_digest).collect();
    ordered.sort();
    if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(UnitizationError::UnitizationNondeterministic);
    }
    let mut manifest = UnitManifest {
        source_id: input.source_id.clone(),
        revision: input.revision,
        content_digest: input.content_digest,
        representation_id: provenance.representation_id,
        materializer_profile_digest: provenance.materializer_profile_digest,
        canonical_digest: provenance.canonical_digest,
        coordinate_digest: provenance.coordinate_digest,
        loss_digest: provenance.loss_digest,
        unitizer_profile_id: profile.id(),
        unitizer_profile_revision: profile.revision(),
        unitizer_limits: profile.limits(),
        digest_algorithm,
        input_bytes,
        emitted_bytes: input_bytes,
        line_count,
        units,
        manifest_digest: Blake3Digest32::from_bytes([0; 32]),
    };
    let mut body = Vec::new();
    encode_body(&manifest, &mut body)?;
    manifest.manifest_digest = Blake3Digest32::from_bytes(digest32(MANIFEST_DOMAIN, &[&body]));
    Ok(manifest)
}

/// Builds every occurrence in canonical order with full binding.
///
/// Validates finite counts, unique unit identities, profile-compliant sizes,
/// complete accounting of the represented bytes and the exact source,
/// representation, materializer and unitizer binding.
///
/// Success contains immutable unit descriptors and digests, not source
/// bodies or ranking data. Cancellation and budget exhaustion surface as
/// typed errors through boundary scanning; they never yield a successful
/// complete manifest.
pub fn build_unit_manifest(
    input: &UnitizationInput,
    provenance: &MaterializerProvenance,
    profile: &ValidatedUnitizerProfile,
    digest_algorithm: DigestAlgorithm,
    max_encoded_bytes: usize,
) -> Result<UnitManifest, UnitizationError> {
    let manifest = assemble_manifest(input, provenance, profile, digest_algorithm)?;
    if encoded_size(manifest.source_id.as_str().len(), manifest.units.len())? > max_encoded_bytes {
        return Err(UnitizationError::InputTooLarge);
    }
    Ok(manifest)
}
