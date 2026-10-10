//! Closed v3 CBOR schema over the shared canonical codec. Decoding is not verification.
use super::digest::{array, bytes, hash_cbor, object, text};
use super::model::{
    CanonicalUnitManifestBytes, ManifestBody, MaterializerProvenance, UnitDescriptor, UnitManifest,
};
use super::spec::{UNIT_MANIFEST_FORMAT, UNIT_MANIFEST_VERSION};
use super::v3_profile::profile_value;
use crate::UnitizationError;
use search_contracts::{CanonicalValue, ClosedCanonicalObject, NativeAnchor};
mod decode;
pub use decode::decode_unit_manifest;

pub(super) fn anchor_value(anchor: &NativeAnchor) -> Result<CanonicalValue, UnitizationError> {
    let NativeAnchor::TextBytes(anchor) = anchor else {
        return Err(UnitizationError::UnitManifestIncomplete);
    };
    array(vec![
        text("text-bytes")?,
        bytes(anchor.content_digest.as_bytes())?,
        CanonicalValue::U64(anchor.byte_start_0),
        CanonicalValue::U64(anchor.byte_end_exclusive_0),
    ])
}

pub(super) fn provenance_value(
    p: &MaterializerProvenance,
) -> Result<CanonicalValue, UnitizationError> {
    let b = &p.binding;
    array(vec![
        array(vec![
            bytes(b.source_namespace_id.as_bytes())?,
            bytes(b.source_id.as_bytes())?,
            bytes(b.source_revision_id.as_bytes())?,
            bytes(b.representation_id.as_bytes())?,
            bytes(b.materialization_id.as_bytes())?,
        ])?,
        bytes(p.materializer_commitment.as_bytes())?,
        bytes(p.materializer_profile_digest.as_bytes())?,
        bytes(p.canonical_digest.as_bytes())?,
        bytes(p.coordinate_digest.as_bytes())?,
        bytes(p.loss_digest.as_bytes())?,
        bytes(p.input_digest.as_bytes())?,
        CanonicalValue::U64(p.native_bytes),
        CanonicalValue::U64(p.canonical_bytes),
        CanonicalValue::U64(p.legacy_revision_sequence),
    ])
}

pub(super) fn unit_value(unit: &UnitDescriptor) -> Result<CanonicalValue, UnitizationError> {
    let occurrence = &unit.occurrence;
    if occurrence.structural_identity.is_some() || occurrence.configuration_predicate.is_some() {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    array(vec![
        bytes(occurrence.unit_id.as_bytes())?,
        bytes(occurrence.representation_id.as_bytes())?,
        text(occurrence.unit_kind.as_str())?,
        CanonicalValue::U64(occurrence.ordinal),
        anchor_value(&occurrence.native_anchor)?,
        CanonicalValue::Null,
        CanonicalValue::Null,
        CanonicalValue::U64(unit.source_start),
        CanonicalValue::U64(unit.source_end),
        CanonicalValue::U64(unit.logical_line_start),
        CanonicalValue::U64(unit.logical_line_end),
        CanonicalValue::Bool(unit.starts_at_line_boundary),
        CanonicalValue::Bool(unit.ends_at_line_boundary),
        bytes(unit.unit_content_digest.as_bytes())?,
        bytes(unit.reference_digest.as_bytes())?,
        bytes(unit.identity_digest.as_bytes())?,
    ])
}

pub(super) fn body_value(body: &ManifestBody) -> Result<CanonicalValue, UnitizationError> {
    if body.units.len() > body.profile.limits.max_units {
        return Err(UnitizationError::TooManyUnits);
    }
    object(vec![
        ("provenance", provenance_value(&body.provenance)?),
        ("profile", profile_value(&body.profile)?),
        ("profile_id", text(body.profile_id.as_str())?),
        ("input_bytes", CanonicalValue::U64(body.input_bytes)),
        (
            "represented_bytes",
            CanonicalValue::U64(body.represented_bytes),
        ),
        ("omitted_bytes", CanonicalValue::U64(body.omitted_bytes)),
        ("line_count", CanonicalValue::U64(body.line_count)),
        (
            "units",
            array(
                body.units
                    .iter()
                    .map(unit_value)
                    .collect::<Result<Vec<_>, _>>()?,
            )?,
        ),
    ])
}

pub(super) fn envelope(
    body: CanonicalValue,
    digest: search_contracts::Blake3Digest32,
) -> Result<CanonicalValue, UnitizationError> {
    object(vec![
        ("format", text(UNIT_MANIFEST_FORMAT)?),
        (
            "version",
            CanonicalValue::U64(u64::from(UNIT_MANIFEST_VERSION)),
        ),
        ("body", body),
        ("digest", bytes(digest.as_bytes())?),
    ])
}

/// Encode one immutable proposed manifest with exact schema and digest parity.
/// Verification against the retained materialization remains mandatory.
pub fn canonicalize_unit_manifest(
    manifest: &UnitManifest,
) -> Result<CanonicalUnitManifestBytes, UnitizationError> {
    let body = body_value(&manifest.body)?;
    let limit = manifest
        .body
        .profile
        .max_manifest_bytes
        .checked_add(128)
        .ok_or(UnitizationError::OffsetOverflow)?;
    if hash_cbor("eliot/cbor/unit-manifest/v3", &body, limit)? != manifest.manifest_digest {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    let encoded = search_contracts::to_canonical_cbor(&envelope(body, manifest.manifest_digest)?)
        .map_err(|_| UnitizationError::InputTooLarge)?;
    if encoded.len() > manifest.body.profile.max_manifest_bytes {
        return Err(UnitizationError::InputTooLarge);
    }
    Ok(CanonicalUnitManifestBytes {
        bytes: encoded.as_slice().to_vec(),
    })
}

pub(super) fn closed(
    value: CanonicalValue,
    schema: &'static str,
) -> Result<ClosedCanonicalObject, UnitizationError> {
    ClosedCanonicalObject::from_value(value, schema)
        .map_err(|_| UnitizationError::UnitManifestIncomplete)
}
pub(super) fn take(
    fields: &mut ClosedCanonicalObject,
    name: &'static str,
) -> Result<CanonicalValue, UnitizationError> {
    fields
        .take_required(name)
        .map_err(|_| UnitizationError::UnitManifestIncomplete)
}
pub(super) fn finish(fields: ClosedCanonicalObject) -> Result<(), UnitizationError> {
    fields
        .finish()
        .map_err(|_| UnitizationError::UnitManifestIncomplete)
}
