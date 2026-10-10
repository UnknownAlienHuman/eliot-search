//! Complete v3 construction with package-derived occurrence identities.
use super::codec::{anchor_value, body_value, canonicalize_unit_manifest, provenance_value};
use super::digest::{bytes, hash_cbor, hash_raw, object, text};
use super::input::{UnitSetInput, UnitizationBudget};
use super::model::{ManifestBody, UnitDescriptor, UnitManifest, VerifiedUnitSet};
use super::v3_profile::{ValidatedV3UnitizerProfile, validate_v3_unitizer_profile};
use crate::{UnitSpan, UnitizationError};
use search_contracts::{
    BoundedList, CanonicalValue, NativeAnchor, Representation, TextBytesAnchor, UnitId,
    UnitOccurrence,
};
use std::collections::{BTreeMap, btree_map::Entry};

pub(super) fn assemble_manifest(
    input: &UnitSetInput<'_>,
    profile: &ValidatedV3UnitizerProfile,
    budget: &UnitizationBudget<'_>,
) -> Result<UnitManifest, UnitizationError> {
    budget.validate(profile)?;
    if input.profile_id != *profile.id()
        || validate_v3_unitizer_profile(profile.descriptor())?.id() != profile.id()
    {
        return Err(UnitizationError::UnitizerProfileMismatch);
    }
    budget.check(input.prep_steps)?;
    let source = input.product.canonical_text();
    let spans = crate::layout::unitize_text_checked(
        source,
        &input.lines,
        profile.limits(),
        |unit_count| {
            let steps = u64::try_from(unit_count)
                .map_err(|_| UnitizationError::OffsetOverflow)?
                .checked_mul(32)
                .and_then(|steps| input.prep_steps.checked_add(steps))
                .ok_or(UnitizationError::OffsetOverflow)?;
            budget.check(steps)
        },
    )?;
    let used = input
        .prep_steps
        .checked_add(
            u64::try_from(spans.len())
                .map_err(|_| UnitizationError::OffsetOverflow)?
                .checked_mul(32)
                .ok_or(UnitizationError::OffsetOverflow)?,
        )
        .ok_or(UnitizationError::OffsetOverflow)?;
    budget.check(used)?;
    let provenance_digest = hash_cbor(
        "eliot/cbor/unit-provenance/v3",
        &provenance_value(&input.provenance)?,
        4096,
    )?;
    let mut units = Vec::with_capacity(spans.len());
    let mut ids = BTreeMap::new();
    let mut cursor = 0;
    for (index, span) in spans.iter().enumerate() {
        budget.check(used)?;
        if span.source_start != cursor {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        let descriptor = derive_occurrence(
            input,
            profile,
            span,
            u64::try_from(index).map_err(|_| UnitizationError::OffsetOverflow)?,
            provenance_digest,
        )?;
        record_identity(
            &mut ids,
            descriptor.occurrence.unit_id,
            descriptor.identity_digest,
        )?;
        cursor = span.source_end;
        units.push(descriptor);
    }
    if cursor != source.len() {
        return Err(UnitizationError::UnitCoverageMismatch);
    }
    let body = ManifestBody {
        provenance: input.provenance.clone(),
        profile: profile.descriptor().clone(),
        profile_id: profile.id().clone(),
        input_bytes: input.provenance.canonical_bytes,
        represented_bytes: input.provenance.canonical_bytes,
        omitted_bytes: 0,
        line_count: u64::try_from(input.lines.len())
            .map_err(|_| UnitizationError::OffsetOverflow)?,
        units,
    };
    // The body is a draft, not an authority object with a zero placeholder.
    let manifest_digest = hash_cbor(
        "eliot/cbor/unit-manifest/v3",
        &body_value(&body)?,
        profile
            .descriptor()
            .max_manifest_bytes
            .checked_add(128)
            .ok_or(UnitizationError::OffsetOverflow)?,
    )?;
    let manifest = UnitManifest {
        body,
        manifest_digest,
    };
    let encoded = canonicalize_unit_manifest(&manifest)?;
    if encoded.len() > budget.max_encoded_bytes {
        return Err(UnitizationError::InputTooLarge);
    }
    budget.check(used)?;
    Ok(manifest)
}

pub(super) fn record_identity(
    ids: &mut BTreeMap<UnitId, search_contracts::Blake3Digest32>,
    unit_id: UnitId,
    commitment: search_contracts::Blake3Digest32,
) -> Result<(), UnitizationError> {
    match ids.entry(unit_id) {
        Entry::Vacant(entry) => {
            entry.insert(commitment);
            Ok(())
        }
        Entry::Occupied(entry) if *entry.get() == commitment => {
            Err(UnitizationError::UnitizationNondeterministic)
        }
        Entry::Occupied(_) => Err(UnitizationError::IdentityCollision),
    }
}

fn derive_occurrence(
    input: &UnitSetInput<'_>,
    profile: &ValidatedV3UnitizerProfile,
    span: &UnitSpan,
    ordinal: u64,
    provenance_digest: search_contracts::Blake3Digest32,
) -> Result<UnitDescriptor, UnitizationError> {
    let descriptor = profile.descriptor();
    let content = input
        .product
        .canonical_text()
        .get(span.source_start..span.source_end)
        .ok_or(UnitizationError::InvalidUtf8Boundary)?;
    let line_count = span
        .logical_line_end
        .checked_sub(span.logical_line_start)
        .ok_or(UnitizationError::InvalidLineSpan)?;
    if content.len() < descriptor.min_unit_bytes
        || content.chars().count() > descriptor.max_unit_scalars
        || line_count
            > u64::try_from(descriptor.max_unit_lines)
                .map_err(|_| UnitizationError::OffsetOverflow)?
    {
        return Err(UnitizationError::UnitTooLarge);
    }
    let source_start =
        u64::try_from(span.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
    let source_end =
        u64::try_from(span.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
    let native_anchor = NativeAnchor::TextBytes(TextBytesAnchor {
        content_digest: input.provenance.input_digest,
        byte_start_0: source_start,
        byte_end_exclusive_0: source_end,
    });
    native_anchor
        .validate()
        .map_err(|_| UnitizationError::InvalidLineSpan)?;
    let unit_content_digest = hash_raw(
        "eliot/raw/unit-content/v3",
        content.as_bytes(),
        descriptor
            .limits
            .max_unit_bytes
            .checked_add(128)
            .ok_or(UnitizationError::OffsetOverflow)?,
    )?;
    let reference = object(vec![
        ("provenance", bytes(provenance_digest.as_bytes())?),
        ("anchor", anchor_value(&native_anchor)?),
        ("canonical_start", CanonicalValue::U64(source_start)),
        ("canonical_end", CanonicalValue::U64(source_end)),
    ])?;
    let reference_digest = hash_cbor("eliot/cbor/unit-reference/v3", &reference, 4096)?;
    let identity = object(vec![
        ("provenance", bytes(provenance_digest.as_bytes())?),
        ("profile", text(profile.id().as_str())?),
        ("kind", text(descriptor.unit_kind.as_str())?),
        ("ordinal", CanonicalValue::U64(ordinal)),
        ("canonical_start", CanonicalValue::U64(source_start)),
        ("canonical_end", CanonicalValue::U64(source_end)),
        ("line_start", CanonicalValue::U64(span.logical_line_start)),
        ("line_end", CanonicalValue::U64(span.logical_line_end)),
        (
            "starts_at_line_boundary",
            CanonicalValue::Bool(span.starts_at_line_boundary),
        ),
        (
            "ends_at_line_boundary",
            CanonicalValue::Bool(span.ends_at_line_boundary),
        ),
        ("anchor", anchor_value(&native_anchor)?),
        ("content", bytes(unit_content_digest.as_bytes())?),
        ("reference", bytes(reference_digest.as_bytes())?),
        ("structural_identity", CanonicalValue::Null),
        ("configuration_predicate", CanonicalValue::Null),
    ])?;
    let identity_digest = hash_cbor("eliot/cbor/unit-occurrence/v3", &identity, 4096)?;
    let mut id_bytes = [0_u8; 16];
    id_bytes.copy_from_slice(&identity_digest.as_bytes()[..16]);
    Ok(UnitDescriptor {
        occurrence: UnitOccurrence {
            unit_id: UnitId::from_bytes(id_bytes),
            representation_id: input.provenance.binding.representation_id,
            unit_kind: descriptor.unit_kind,
            ordinal,
            native_anchor,
            structural_identity: None,
            configuration_predicate: None,
        },
        source_start,
        source_end,
        logical_line_start: span.logical_line_start,
        logical_line_end: span.logical_line_end,
        starts_at_line_boundary: span.starts_at_line_boundary,
        ends_at_line_boundary: span.ends_at_line_boundary,
        unit_content_digest,
        reference_digest,
        identity_digest,
    })
}

pub(super) fn verified(manifest: UnitManifest) -> VerifiedUnitSet {
    let representation = Representation {
        representation_id: manifest.body.provenance.binding.representation_id,
        materialization_id: manifest.body.provenance.binding.materialization_id,
        unitizer_profile_id: manifest.body.profile_id.clone(),
        enrichment_profile_ids: BoundedList::empty(),
        unit_manifest_digest: manifest.manifest_digest,
    };
    VerifiedUnitSet {
        manifest,
        representation,
    }
}

/// Build the complete ordered v3 set; cancellation/budget failure emits no set.
/// Occurrence identities, commitments and the representation digest are derived.
pub fn build_unit_manifest(
    input: &UnitSetInput<'_>,
    profile: &ValidatedV3UnitizerProfile,
    budget: &UnitizationBudget<'_>,
) -> Result<VerifiedUnitSet, UnitizationError> {
    assemble_manifest(input, profile, budget).map(verified)
}
