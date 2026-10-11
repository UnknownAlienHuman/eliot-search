//! Closed positional v4 records through the sole shared canonical codec.
use super::budget::{Meter, add, index, mul, number};
use super::model::{
    CanonicalMarkdownUnitManifestBytes, INLINE_STATUS, MarkdownInlineObservation,
    MarkdownManifestBody, MarkdownMappedValue, MarkdownNodeData, MarkdownStructuralUnit,
    MarkdownUnitManifest, MarkdownUnitizationError as Error, REFERENCE_STATUS,
};
use super::profile::FORMAT;
use super::{MarkdownUnitizationBudget, MarkdownUnitizerProfile};
use search_contracts::{BoundedBytes, BoundedList, CanonicalText, CanonicalValue};
use std::collections::BTreeMap;
use std::mem::size_of;

pub(super) fn array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, Error> {
    BoundedList::new(values)
        .map(CanonicalValue::Array)
        .map_err(|_| Error::Codec)
}
pub(super) fn text(value: &str) -> Result<CanonicalValue, Error> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| Error::Codec)
}
pub(super) fn bytes(value: &[u8]) -> Result<CanonicalValue, Error> {
    BoundedBytes::new(value.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| Error::Codec)
}
pub(super) fn optional(value: Option<u64>) -> CanonicalValue {
    value.map_or(CanonicalValue::Null, CanonicalValue::U64)
}
fn mapped(value: Option<&MarkdownMappedValue>) -> Result<CanonicalValue, Error> {
    value.map_or(Ok(CanonicalValue::Null), |value| {
        array(vec![text("mapped-text")?, text(value.value())?])
    })
}
pub(super) fn node_identity(
    unit: &MarkdownNodeData,
    meter: &mut Meter<'_>,
) -> Result<CanonicalValue, Error> {
    meter.rows(
        12,
        add(
            128,
            unit.code_info.as_ref().map_or(0, |value| value.value.len()),
        )?,
    )?;
    array(vec![
        CanonicalValue::U64(unit.ordinal),
        text(unit.kind)?,
        CanonicalValue::U64(u64::from(unit.heading_level)),
        optional(unit.parent_section),
        optional(unit.physical_parent),
        optional(unit.heading_section),
        CanonicalValue::U64(unit.start),
        CanonicalValue::U64(unit.end),
        mapped(unit.code_info.as_ref())?,
        bytes(unit.content.as_bytes())?,
    ])
}
fn node(unit: &MarkdownStructuralUnit, meter: &mut Meter<'_>) -> Result<CanonicalValue, Error> {
    let CanonicalValue::Array(fields) = node_identity(&unit.data, meter)? else {
        return Err(Error::Codec);
    };
    meter.rows(3, 80)?;
    let mut fields = fields.into_vec();
    let content = fields.pop().ok_or(Error::Codec)?;
    fields.push(bytes(unit.occurrence.unit_id.as_bytes())?);
    fields.push(content);
    fields.push(bytes(unit.identity.as_bytes())?);
    array(fields)
}
fn inline(
    record: &MarkdownInlineObservation,
    meter: &mut Meter<'_>,
) -> Result<CanonicalValue, Error> {
    let mut copied = 128;
    for value in [&record.text, &record.destination, &record.title]
        .into_iter()
        .flatten()
    {
        copied = add(copied, value.value.len())?;
    }
    meter.rows(13, copied)?;
    array(vec![
        CanonicalValue::U64(record.owner),
        text(record.form)?,
        CanonicalValue::U64(record.start),
        CanonicalValue::U64(record.end),
        mapped(record.text.as_ref())?,
        mapped(record.destination.as_ref())?,
        mapped(record.title.as_ref())?,
    ])
}
pub(super) fn shape(manifest: &MarkdownManifestBody, meter: &mut Meter<'_>) -> Result<(), Error> {
    let limits = meter.budget.limits;
    if manifest.nodes.is_empty()
        || manifest.nodes.len() > limits.nodes
        || manifest.inline.len() > limits.inline_records
        || manifest.raw.unit_count() > limits.raw_units
        || manifest.raw.input_bytes() > number(limits.input_bytes)?
        || manifest.raw.emitted_bytes() != manifest.raw.input_bytes()
        || manifest.raw.omitted_bytes() != 0
        || manifest.raw.provenance().binding().representation_id == manifest.structural
        || manifest.raw.profile_id() != &manifest.profile.raw_id
    {
        return Err(Error::InvalidStructure);
    }
    let entries = add(manifest.nodes.len(), manifest.raw.unit_count())?;
    meter.work(add(mul(entries, 2048)?, mul(manifest.inline.len(), 128)?)?)?;
    // Check every selected value against the LOWER operation limits before any
    // identity-row copies or collision/occurrence allocations.
    let mut metadata = 0;
    for value in manifest
        .nodes
        .iter()
        .flat_map(|unit| unit.data.code_info.iter())
        .chain(manifest.inline.iter().flat_map(|record| {
            [&record.text, &record.destination, &record.title]
                .into_iter()
                .flatten()
        }))
    {
        meter.poll()?;
        metadata = add(metadata, value.value.len())?;
        if value.value.len() > limits.metadata_bytes_each || metadata > limits.metadata_bytes_total
        {
            return Err(Error::InvalidStructure);
        }
    }
    meter.work(metadata)?;
    meter.owned(add(mul(entries, 128)?, mul(manifest.nodes.len(), 128)?)?)?;
    let mut ids = BTreeMap::new();
    for unit in manifest.raw.units() {
        super::build::record(&mut ids, unit.unit_id(), unit.identity_digest())?;
    }
    let context = super::build::context(manifest, meter)?;
    for (ordinal, unit) in manifest.nodes.iter().enumerate() {
        meter.poll()?;
        let data = &unit.data;
        let expected = super::build::occurrence(
            super::build::truncated(unit.identity),
            manifest.structural,
            number(ordinal)?,
            data.kind,
            (data.start, data.end),
            manifest.raw.provenance().input_digest(),
            unit.identity,
        )?;
        if data.ordinal != number(ordinal)?
            || data.start >= data.end
            || data.end > manifest.raw.input_bytes()
            || unit.occurrence != expected
            || super::build::identity(context, data, meter)? != unit.identity
            || (data.code_info.is_some() && data.kind != "fenced-code")
        {
            return Err(Error::InvalidStructure);
        }
        if ordinal == 0 {
            if data.kind != "root"
                || data.start != 0
                || data.end != manifest.raw.input_bytes()
                || data.physical_parent.is_some()
                || data.heading_section.is_some()
            {
                return Err(Error::InvalidStructure);
            }
        } else {
            let parent = data.physical_parent.ok_or(Error::InvalidStructure)?;
            if data.kind == "root" || parent >= data.ordinal {
                return Err(Error::InvalidStructure);
            }
            let parent = &manifest.nodes[index(parent)?].data;
            if parent.start > data.start || parent.end < data.end {
                return Err(Error::InvalidStructure);
            }
        }
        if data.kind == "heading" {
            if !(1..=6).contains(&data.heading_level) || data.heading_section != Some(data.ordinal)
            {
                return Err(Error::InvalidStructure);
            }
            if let Some(parent) = data.parent_section {
                if parent >= data.ordinal {
                    return Err(Error::InvalidStructure);
                }
                let parent = &manifest.nodes[index(parent)?].data;
                if parent.kind != "heading" || parent.heading_level >= data.heading_level {
                    return Err(Error::InvalidStructure);
                }
            }
        } else if data.heading_level != 0 || data.parent_section.is_some() {
            return Err(Error::InvalidStructure);
        }
        if let Some(section) = data.heading_section
            && (section > data.ordinal || manifest.nodes[index(section)?].data.kind != "heading")
        {
            return Err(Error::InvalidStructure);
        }
        super::build::record(&mut ids, unit.occurrence.unit_id, unit.identity)?;
    }
    for record in &manifest.inline {
        meter.poll()?;
        if record.start > record.end
            || record.end > manifest.raw.input_bytes()
            || record.owner >= number(manifest.nodes.len())?
        {
            return Err(Error::InvalidStructure);
        }
        super::build::inline_form(record.form)?;
        let owner = &manifest.nodes[index(record.owner)?].data;
        let link = record.form.starts_with("link-") || record.form.starts_with("image-");
        if owner.start > record.start
            || owner.end < record.end
            || (record.text.is_some() && record.form != "code")
            || (!link && (record.destination.is_some() || record.title.is_some()))
        {
            return Err(Error::InvalidStructure);
        }
    }
    meter.poll()
}
pub(super) fn body(
    manifest: &MarkdownManifestBody,
    meter: &mut Meter<'_>,
) -> Result<CanonicalValue, Error> {
    shape(manifest, meter)?;
    let cap = manifest.raw.profile().max_manifest_bytes;
    // Reserve the existing bounded v3 hash/encode closure before calling it.
    meter.work(add(mul(cap, 2)?, 4096)?)?;
    meter.canonical(add(cap, 128)?)?;
    meter.owned(add(
        mul(cap, 3)?,
        mul(
            add(mul(manifest.raw.unit_count(), 32)?, 128)?,
            size_of::<CanonicalValue>(),
        )?,
    )?)?;
    let raw = crate::canonicalize_unit_manifest(&manifest.raw)?;
    meter.poll()?;
    meter.rows(
        add(64, add(manifest.nodes.len(), manifest.inline.len())?)?,
        1024,
    )?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(manifest.nodes.len())
        .map_err(|_| Error::AllocationFailed)?;
    for unit in &manifest.nodes {
        nodes.push(node(unit, meter)?);
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(manifest.inline.len())
        .map_err(|_| Error::AllocationFailed)?;
    for record in &manifest.inline {
        records.push(inline(record, meter)?);
    }
    array(vec![
        manifest.profile.value()?,
        text(manifest.profile.id().as_str())?,
        CanonicalValue::Bytes(BoundedBytes::new(raw.bytes).map_err(|_| Error::Codec)?),
        bytes(manifest.structural.as_bytes())?,
        text(manifest.residency.as_str())?,
        bytes(manifest.markdown_product.as_bytes())?,
        bytes(manifest.markdown_nodes.as_bytes())?,
        bytes(manifest.markdown_inline.as_bytes())?,
        text(INLINE_STATUS)?,
        text(REFERENCE_STATUS)?,
        array(nodes)?,
        array(records)?,
    ])
}
pub(super) fn encode(
    manifest: &MarkdownUnitManifest,
    meter: &mut Meter<'_>,
) -> Result<CanonicalMarkdownUnitManifestBytes, Error> {
    meter.rows(4, 128)?;
    let body = body(&manifest.body, meter)?;
    meter.owned(search_contracts::MAX_CANONICAL_BYTES)?;
    meter.work(search_contracts::MAX_CANONICAL_BYTES)?;
    let envelope = array(vec![
        text(FORMAT)?,
        CanonicalValue::U64(4),
        body,
        bytes(manifest.digest.as_bytes())?,
    ])?;
    let encoded = search_contracts::to_canonical_cbor(&envelope).map_err(|_| Error::Codec)?;
    meter.poll()?;
    if encoded.as_slice().len() > meter.budget.limits.encoded_bytes {
        return Err(Error::CanonicalLimit);
    }
    Ok(CanonicalMarkdownUnitManifestBytes {
        bytes: encoded.into_vec(),
    })
}
/// Encode a closed proposed manifest; encoding does not verify source completeness.
pub fn canonicalize_markdown_unit_manifest(
    manifest: &MarkdownUnitManifest,
    profile: &MarkdownUnitizerProfile,
    budget: &MarkdownUnitizationBudget<'_>,
) -> Result<CanonicalMarkdownUnitManifestBytes, Error> {
    let mut meter = Meter::new(profile, budget)?;
    let result = (|| {
        if manifest.body.profile != *profile {
            return Err(Error::ProfileMismatch);
        }
        let value = body(&manifest.body, &mut meter)?;
        if meter.hash("eliot/cbor/md-unit-manifest/v4", value)? != manifest.digest {
            return Err(Error::ManifestMismatch);
        }
        encode(manifest, &mut meter)
    })();
    meter.poll()?;
    result
}
