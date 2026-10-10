//! Strict positional records and closed object decoding; no source authority.
use super::super::model::{
    ManifestBody, MaterializerProvenance, UnitDescriptor, UnitManifest, V3SourceBinding,
};
use super::super::spec::{UNIT_MANIFEST_FORMAT, UNIT_MANIFEST_VERSION};
use super::super::v3_profile::decode_profile;
use super::{canonicalize_unit_manifest, closed, finish, take};
use crate::UnitizationError;
use search_contracts::{
    Blake3Digest32, CanonicalValue, MaterializationId, NativeAnchor, ProfileId, RepresentationId,
    SourceId, SourceNamespaceId, SourceRevisionId, TextBytesAnchor, UnitId, UnitKind,
    UnitOccurrence,
};

const fn invalid() -> UnitizationError {
    UnitizationError::UnitManifestIncomplete
}
const fn number(value: &CanonicalValue) -> Result<u64, UnitizationError> {
    if let CanonicalValue::U64(n) = value {
        Ok(*n)
    } else {
        Err(invalid())
    }
}
const fn boolean(value: &CanonicalValue) -> Result<bool, UnitizationError> {
    if let CanonicalValue::Bool(n) = value {
        Ok(*n)
    } else {
        Err(invalid())
    }
}
fn string(value: CanonicalValue) -> Result<String, UnitizationError> {
    if let CanonicalValue::Text(n) = value {
        Ok(n.into_string())
    } else {
        Err(invalid())
    }
}
fn fixed<const N: usize>(value: CanonicalValue) -> Result<[u8; N], UnitizationError> {
    if let CanonicalValue::Bytes(n) = value {
        n.as_slice().try_into().map_err(|_| invalid())
    } else {
        Err(invalid())
    }
}
fn digest(value: CanonicalValue) -> Result<Blake3Digest32, UnitizationError> {
    fixed(value).map(Blake3Digest32::from_stored_bytes)
}

struct Record(std::vec::IntoIter<CanonicalValue>);
impl Record {
    fn new(value: CanonicalValue, length: usize) -> Result<Self, UnitizationError> {
        let CanonicalValue::Array(values) = value else {
            return Err(invalid());
        };
        if values.len() != length {
            return Err(invalid());
        }
        Ok(Self(values.into_vec().into_iter()))
    }
    fn next(&mut self) -> Result<CanonicalValue, UnitizationError> {
        self.0.next().ok_or_else(invalid)
    }
}

fn provenance(value: CanonicalValue) -> Result<MaterializerProvenance, UnitizationError> {
    let mut record = Record::new(value, 10)?;
    let mut binding = Record::new(record.next()?, 5)?;
    let binding = V3SourceBinding {
        source_namespace_id: SourceNamespaceId::from_bytes(fixed(binding.next()?)?),
        source_id: SourceId::from_bytes(fixed(binding.next()?)?),
        source_revision_id: SourceRevisionId::from_bytes(fixed(binding.next()?)?),
        representation_id: RepresentationId::from_bytes(fixed(binding.next()?)?),
        materialization_id: MaterializationId::from_bytes(fixed(binding.next()?)?),
    };
    Ok(MaterializerProvenance {
        binding,
        materializer_commitment: digest(record.next()?)?,
        materializer_profile_digest: digest(record.next()?)?,
        canonical_digest: digest(record.next()?)?,
        coordinate_digest: digest(record.next()?)?,
        loss_digest: digest(record.next()?)?,
        input_digest: digest(record.next()?)?,
        native_bytes: number(&record.next()?)?,
        canonical_bytes: number(&record.next()?)?,
        legacy_revision_sequence: number(&record.next()?)?,
    })
}

fn anchor(value: CanonicalValue) -> Result<NativeAnchor, UnitizationError> {
    let mut record = Record::new(value, 4)?;
    if string(record.next()?)? != "text-bytes" {
        return Err(invalid());
    }
    let anchor = NativeAnchor::TextBytes(TextBytesAnchor {
        content_digest: digest(record.next()?)?,
        byte_start_0: number(&record.next()?)?,
        byte_end_exclusive_0: number(&record.next()?)?,
    });
    anchor.validate().map_err(|_| invalid())?;
    Ok(anchor)
}

fn unit(value: CanonicalValue) -> Result<UnitDescriptor, UnitizationError> {
    let mut record = Record::new(value, 16)?;
    let unit_id = UnitId::from_bytes(fixed(record.next()?)?);
    let representation_id = RepresentationId::from_bytes(fixed(record.next()?)?);
    let unit_kind = UnitKind::parse(&string(record.next()?)?).map_err(|_| invalid())?;
    let ordinal = number(&record.next()?)?;
    let native_anchor = anchor(record.next()?)?;
    if record.next()? != CanonicalValue::Null || record.next()? != CanonicalValue::Null {
        return Err(invalid());
    }
    Ok(UnitDescriptor {
        occurrence: UnitOccurrence {
            unit_id,
            representation_id,
            unit_kind,
            ordinal,
            native_anchor,
            structural_identity: None,
            configuration_predicate: None,
        },
        source_start: number(&record.next()?)?,
        source_end: number(&record.next()?)?,
        logical_line_start: number(&record.next()?)?,
        logical_line_end: number(&record.next()?)?,
        starts_at_line_boundary: boolean(&record.next()?)?,
        ends_at_line_boundary: boolean(&record.next()?)?,
        unit_content_digest: digest(record.next()?)?,
        reference_digest: digest(record.next()?)?,
        identity_digest: digest(record.next()?)?,
    })
}

/// Decode bounded canonical v3 bytes into a proposed manifest, never a verified set.
/// Legacy binary v1/v2 bytes receive an explicit rebuild error.
pub fn decode_unit_manifest(
    encoded: &[u8],
    max_encoded_bytes: usize,
) -> Result<UnitManifest, UnitizationError> {
    if max_encoded_bytes == 0 || max_encoded_bytes > search_contracts::MAX_CANONICAL_BYTES {
        return Err(UnitizationError::InvalidLimits);
    }
    if encoded.len() > max_encoded_bytes {
        return Err(UnitizationError::InputTooLarge);
    }
    if encoded.starts_with(b"ELSUMF01") || encoded.starts_with(b"ELSUMF02") {
        return Err(UnitizationError::UnitManifestLegacyUnsupported);
    }
    let value = search_contracts::parse_canonical_cbor(encoded).map_err(|_| invalid())?;
    let mut envelope = closed(value, "unit_manifest_v3")?;
    if string(take(&mut envelope, "format")?)? != UNIT_MANIFEST_FORMAT
        || number(&take(&mut envelope, "version")?)? != u64::from(UNIT_MANIFEST_VERSION)
    {
        return Err(UnitizationError::UnitManifestLegacyUnsupported);
    }
    let mut body = closed(take(&mut envelope, "body")?, "unit_manifest_body_v3")?;
    let manifest_digest = digest(take(&mut envelope, "digest")?)?;
    finish(envelope)?;
    let provenance = provenance(take(&mut body, "provenance")?)?;
    let profile = decode_profile(take(&mut body, "profile")?)?;
    if encoded.len() > profile.descriptor().max_manifest_bytes {
        return Err(UnitizationError::InputTooLarge);
    }
    let profile_id =
        ProfileId::new(string(take(&mut body, "profile_id")?)?).map_err(|_| invalid())?;
    if profile_id != *profile.id() {
        return Err(UnitizationError::UnitizerProfileMismatch);
    }
    let input_bytes = number(&take(&mut body, "input_bytes")?)?;
    let represented_bytes = number(&take(&mut body, "represented_bytes")?)?;
    let omitted_bytes = number(&take(&mut body, "omitted_bytes")?)?;
    let line_count = number(&take(&mut body, "line_count")?)?;
    let CanonicalValue::Array(values) = take(&mut body, "units")? else {
        return Err(invalid());
    };
    if values.len() > profile.limits().max_units {
        return Err(UnitizationError::TooManyUnits);
    }
    let units = values
        .into_vec()
        .into_iter()
        .map(unit)
        .collect::<Result<Vec<_>, _>>()?;
    finish(body)?;
    let manifest = UnitManifest {
        body: ManifestBody {
            provenance,
            profile: profile.descriptor().clone(),
            profile_id,
            input_bytes,
            represented_bytes,
            omitted_bytes,
            line_count,
            units,
        },
        manifest_digest,
    };
    if canonicalize_unit_manifest(&manifest)?.as_slice() != encoded {
        return Err(UnitizationError::UnitizationNondeterministic);
    }
    Ok(manifest)
}
