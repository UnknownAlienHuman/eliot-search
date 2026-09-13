//! Canonical durable unit-manifest codec.

use crate::{UnitizationError, UnitizationLimits};
use search_contracts::{Blake3Digest32, DigestAlgorithm, NonZeroRevision, OpaqueId};

use super::digest::digest32;
use super::model::{CanonicalUnitManifestBytes, UnitDescriptor, UnitManifest};
use super::profile::UnitizerProfileId;
use super::spec::{
    DIGEST_TRAILER_BYTES, MAGIC, MANIFEST_DOMAIN, UNIT_CODEC_BYTES,
    UNIT_MANIFEST_VERSION,
};

const fn digest_tag(algorithm: DigestAlgorithm) -> u8 {
    match algorithm {
        DigestAlgorithm::Blake3_256 => 1,
        DigestAlgorithm::Sha256 => 2,
    }
}

const fn parse_digest_tag(value: u8) -> Result<DigestAlgorithm, UnitizationError> {
    match value {
        1 => Ok(DigestAlgorithm::Blake3_256),
        2 => Ok(DigestAlgorithm::Sha256),
        _ => Err(UnitizationError::UnitManifestDigestMismatch),
    }
}

/// Serializes schema, profile, source, representation and map identities plus
/// the ordered unit descriptors and counts deterministically.
pub fn canonicalize_unit_manifest(
    manifest: &UnitManifest,
) -> Result<CanonicalUnitManifestBytes, UnitizationError> {
    let mut out = Vec::with_capacity(encoded_size(
        manifest.source_id.as_str().len(),
        manifest.units.len(),
    )?);
    encode_body(manifest, &mut out)?;
    out.extend_from_slice(manifest.manifest_digest.as_bytes());
    Ok(CanonicalUnitManifestBytes { bytes: out })
}

/// Parses durable manifest bytes without source text or profile state.
///
/// Structural defects (magic, version, lengths, counts, ordering, encoded
/// limits) fail closed; an unknown digest-algorithm tag or a mismatched
/// digest trailer fails as a digest mismatch instead of being reinterpreted.
/// Full provenance still requires [`super::verify_unit_manifest`].
pub fn decode_unit_manifest(
    bytes: &[u8],
    max_encoded_bytes: usize,
) -> Result<UnitManifest, UnitizationError> {
    if bytes.len() > max_encoded_bytes || bytes.len() < DIGEST_TRAILER_BYTES {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let (body, trailer) = bytes.split_at(bytes.len() - DIGEST_TRAILER_BYTES);
    if body.len() < minimum_body_len() {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let mut cursor = 0_usize;
    if take(body, &mut cursor, MAGIC.len())? != MAGIC {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let version = u16::from_le_bytes(
        take(body, &mut cursor, 2)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
    );
    if version != UNIT_MANIFEST_VERSION {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let digest_algorithm = parse_digest_tag(take(body, &mut cursor, 1)?[0])?;
    let source_len = usize::from(u16::from_le_bytes(
        take(body, &mut cursor, 2)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
    ));
    if source_len == 0 {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let source_bytes = take(body, &mut cursor, source_len)?;
    let source_text =
        core::str::from_utf8(source_bytes).map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let source_id =
        OpaqueId::new(source_text).map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let revision = u64::from_le_bytes(
        take(body, &mut cursor, 8)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
    );
    let revision =
        NonZeroRevision::new(revision).map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let digest_at = |cursor: &mut usize| -> Result<Blake3Digest32, UnitizationError> {
        let raw: [u8; 32] = take(body, cursor, 32)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
        Ok(Blake3Digest32::from_bytes(raw))
    };
    let content_digest = digest_at(&mut cursor)?;
    let representation_id = digest_at(&mut cursor)?;
    let materializer_profile_digest: [u8; 32] = take(body, &mut cursor, 32)?
        .try_into()
        .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let canonical_digest = digest_at(&mut cursor)?;
    let coordinate_digest = digest_at(&mut cursor)?;
    let loss_digest = digest_at(&mut cursor)?;
    let profile_id_raw: [u8; 32] = take(body, &mut cursor, 32)?
        .try_into()
        .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let unitizer_profile_id = UnitizerProfileId::from_bytes(profile_id_raw);
    let unitizer_profile_revision = u64::from_le_bytes(
        take(body, &mut cursor, 8)?
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
    );
    if unitizer_profile_revision == 0 {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let limit_at = |cursor: &mut usize| -> Result<usize, UnitizationError> {
        let raw = u64::from_le_bytes(
            take(body, cursor, 8)?
                .try_into()
                .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
        );
        usize::try_from(raw).map_err(|_| UnitizationError::OffsetOverflow)
    };
    let unitizer_limits = UnitizationLimits {
        max_input_bytes: limit_at(&mut cursor)?,
        preferred_unit_bytes: limit_at(&mut cursor)?,
        max_unit_bytes: limit_at(&mut cursor)?,
        max_lines: limit_at(&mut cursor)?,
        max_units: limit_at(&mut cursor)?,
    };
    unitizer_limits
        .validate()
        .map_err(|_| UnitizationError::InvalidLimits)?;
    let count_at = |cursor: &mut usize| -> Result<u64, UnitizationError> {
        Ok(u64::from_le_bytes(
            take(body, cursor, 8)?
                .try_into()
                .map_err(|_| UnitizationError::UnitManifestIncomplete)?,
        ))
    };
    let input_bytes = count_at(&mut cursor)?;
    let emitted_bytes = count_at(&mut cursor)?;
    let unit_count = count_at(&mut cursor)?;
    let line_count = count_at(&mut cursor)?;
    if emitted_bytes != input_bytes {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let unit_count_usize =
        usize::try_from(unit_count).map_err(|_| UnitizationError::OffsetOverflow)?;
    if unit_count_usize > unitizer_limits.max_units {
        return Err(UnitizationError::TooManyUnits);
    }
    if body.len() - cursor != unit_count_usize * UNIT_CODEC_BYTES {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    let mut units = Vec::with_capacity(unit_count_usize);
    let mut expected_start = 0_u64;
    for ordinal in 0..unit_count_usize {
        let stored_ordinal = count_at(&mut cursor)?;
        let source_start = count_at(&mut cursor)?;
        let source_end = count_at(&mut cursor)?;
        let logical_line_start = count_at(&mut cursor)?;
        let logical_line_end = count_at(&mut cursor)?;
        let flags = take(body, &mut cursor, 1)?[0];
        if flags > 3 {
            return Err(UnitizationError::UnitManifestIncomplete);
        }
        let unit_digest = digest_at(&mut cursor)?;
        let expected_ordinal =
            u64::try_from(ordinal).map_err(|_| UnitizationError::OffsetOverflow)?;
        if stored_ordinal != expected_ordinal
            || source_start != expected_start
            || source_end <= source_start
        {
            return Err(UnitizationError::UnitizationNondeterministic);
        }
        units.push(UnitDescriptor {
            ordinal: stored_ordinal,
            source_start,
            source_end,
            logical_line_start,
            logical_line_end,
            starts_at_line_boundary: flags & 1 == 1,
            ends_at_line_boundary: flags & 2 == 2,
            unit_digest,
        });
        expected_start = source_end;
    }
    let mut ordered: Vec<Blake3Digest32> =
        units.iter().map(UnitDescriptor::unit_digest).collect();
    ordered.sort();
    if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(UnitizationError::UnitizationNondeterministic);
    }
    let expected_trailer = digest32(MANIFEST_DOMAIN, &[body]);
    if trailer != expected_trailer {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    let manifest_digest = Blake3Digest32::from_bytes(
        trailer
            .try_into()
            .map_err(|_| UnitizationError::UnitManifestDigestMismatch)?,
    );
    Ok(UnitManifest {
        source_id,
        revision,
        content_digest,
        representation_id,
        materializer_profile_digest,
        canonical_digest,
        coordinate_digest,
        loss_digest,
        unitizer_profile_id,
        unitizer_profile_revision,
        unitizer_limits,
        digest_algorithm,
        input_bytes,
        emitted_bytes,
        line_count,
        units,
        manifest_digest,
    })
}

fn take<'body>(
    body: &'body [u8],
    cursor: &mut usize,
    count: usize,
) -> Result<&'body [u8], UnitizationError> {
    let end = cursor
        .checked_add(count)
        .ok_or(UnitizationError::OffsetOverflow)?;
    let slice = body
        .get(*cursor..end)
        .ok_or(UnitizationError::UnitManifestIncomplete)?;
    *cursor = end;
    Ok(slice)
}

pub(super) fn encoded_size(
    source_len: usize,
    unit_count: usize,
) -> Result<usize, UnitizationError> {
    let Some(body) = source_len.checked_add(fixed_body_len()).and_then(|base| {
        unit_count
            .checked_mul(UNIT_CODEC_BYTES)
            .and_then(|tail| base.checked_add(tail))
    }) else {
        return Err(UnitizationError::OffsetOverflow);
    };
    body.checked_add(DIGEST_TRAILER_BYTES)
        .ok_or(UnitizationError::OffsetOverflow)
}

const fn fixed_body_len() -> usize {
    // MAGIC + version + algorithm + source_len + revision + six digests +
    // unitizer profile id + profile revision + five limits + four counts.
    8 + 2 + 1 + 2 + 8 + 32 * 6 + 32 + 8 + 8 * 5 + 8 * 4
}

const fn minimum_body_len() -> usize {
    // Fixed body with an empty source name; decode rejects the empty name
    // after this length gate.
    fixed_body_len()
}

pub(super) fn encode_body(
    manifest: &UnitManifest,
    out: &mut Vec<u8>,
) -> Result<(), UnitizationError> {
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&UNIT_MANIFEST_VERSION.to_le_bytes());
    out.push(digest_tag(manifest.digest_algorithm));
    let source = manifest.source_id.as_str().as_bytes();
    out.extend_from_slice(
        &u16::try_from(source.len())
            .map_err(|_| UnitizationError::OffsetOverflow)?
            .to_le_bytes(),
    );
    out.extend_from_slice(source);
    out.extend_from_slice(&manifest.revision.get().to_le_bytes());
    out.extend_from_slice(manifest.content_digest.as_bytes());
    out.extend_from_slice(manifest.representation_id.as_bytes());
    out.extend_from_slice(&manifest.materializer_profile_digest);
    out.extend_from_slice(manifest.canonical_digest.as_bytes());
    out.extend_from_slice(manifest.coordinate_digest.as_bytes());
    out.extend_from_slice(manifest.loss_digest.as_bytes());
    out.extend_from_slice(manifest.unitizer_profile_id.as_bytes());
    out.extend_from_slice(&manifest.unitizer_profile_revision.to_le_bytes());
    for limit in [
        manifest.unitizer_limits.max_input_bytes,
        manifest.unitizer_limits.preferred_unit_bytes,
        manifest.unitizer_limits.max_unit_bytes,
        manifest.unitizer_limits.max_lines,
        manifest.unitizer_limits.max_units,
    ] {
        out.extend_from_slice(
            &u64::try_from(limit)
                .map_err(|_| UnitizationError::OffsetOverflow)?
                .to_le_bytes(),
        );
    }
    for count in [
        manifest.input_bytes,
        manifest.emitted_bytes,
        u64::try_from(manifest.units.len()).map_err(|_| UnitizationError::OffsetOverflow)?,
        manifest.line_count,
    ] {
        out.extend_from_slice(&count.to_le_bytes());
    }
    for unit in &manifest.units {
        out.extend_from_slice(&unit.ordinal.to_le_bytes());
        out.extend_from_slice(&unit.source_start.to_le_bytes());
        out.extend_from_slice(&unit.source_end.to_le_bytes());
        out.extend_from_slice(&unit.logical_line_start.to_le_bytes());
        out.extend_from_slice(&unit.logical_line_end.to_le_bytes());
        out.push(
            u8::from(unit.starts_at_line_boundary)
                | (u8::from(unit.ends_at_line_boundary) << 1),
        );
        out.extend_from_slice(unit.unit_digest.as_bytes());
    }
    Ok(())
}
