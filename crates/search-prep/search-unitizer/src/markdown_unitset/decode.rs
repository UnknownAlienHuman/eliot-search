//! Exact positional decoding produces proposals, never verified source authority.
use super::budget::{Meter, add, mul};
use super::build::{inline_form, kind, occurrence};
use super::codec::{body, encode, shape};
use super::model::{
    INLINE_STATUS, MarkdownInlineObservation, MarkdownManifestBody, MarkdownMappedValue,
    MarkdownNodeData, MarkdownStructuralUnit, MarkdownUnitManifest,
    MarkdownUnitizationError as Error, REFERENCE_STATUS,
};
use super::profile::FORMAT;
use super::{MarkdownUnitizationBudget, MarkdownUnitizerProfile};
use search_contracts::{
    Blake3Digest32, CanonicalValue, MAX_CANONICAL_BYTES, OpaqueId, RepresentationId, UnitId,
};
use std::mem::size_of;

struct Record(std::vec::IntoIter<CanonicalValue>);
impl Record {
    fn new(value: CanonicalValue, length: usize) -> Result<Self, Error> {
        let CanonicalValue::Array(values) = value else {
            return Err(Error::Codec);
        };
        if values.len() != length {
            return Err(Error::Codec);
        }
        Ok(Self(values.into_vec().into_iter()))
    }
    fn next(&mut self) -> Result<CanonicalValue, Error> {
        self.0.next().ok_or(Error::Codec)
    }
}
fn string(value: CanonicalValue) -> Result<String, Error> {
    if let CanonicalValue::Text(value) = value {
        Ok(value.into_string())
    } else {
        Err(Error::Codec)
    }
}
const fn unsigned(value: &CanonicalValue) -> Result<u64, Error> {
    if let CanonicalValue::U64(value) = value {
        Ok(*value)
    } else {
        Err(Error::Codec)
    }
}
fn optional(value: &CanonicalValue) -> Result<Option<u64>, Error> {
    if value == &CanonicalValue::Null {
        Ok(None)
    } else {
        unsigned(value).map(Some)
    }
}
fn fixed<const N: usize>(value: CanonicalValue) -> Result<[u8; N], Error> {
    let CanonicalValue::Bytes(value) = value else {
        return Err(Error::Codec);
    };
    value.as_slice().try_into().map_err(|_| Error::Codec)
}
fn digest(value: CanonicalValue) -> Result<Blake3Digest32, Error> {
    fixed(value).map(Blake3Digest32::from_stored_bytes)
}
fn mapped(
    value: CanonicalValue,
    metadata: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Option<MarkdownMappedValue>, Error> {
    if value == CanonicalValue::Null {
        return Ok(None);
    }
    let mut record = Record::new(value, 2)?;
    if string(record.next()?)? != "mapped-text" {
        return Err(Error::Codec);
    }
    let value = record.next()?;
    let CanonicalValue::Text(value) = value else {
        return Err(Error::Codec);
    };
    let length = value.as_str().len();
    *metadata = add(*metadata, length)?;
    if length > meter.budget.limits.metadata_bytes_each
        || *metadata > meter.budget.limits.metadata_bytes_total
    {
        return Err(Error::InvalidStructure);
    }
    meter.work(length)?;
    meter.owned(add(size_of::<MarkdownMappedValue>(), length)?)?;
    Ok(Some(MarkdownMappedValue {
        value: value.into_string(),
    }))
}
fn nodes(
    value: CanonicalValue,
    manifest: &MarkdownManifestBody,
    metadata: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Vec<MarkdownStructuralUnit>, Error> {
    let CanonicalValue::Array(values) = value else {
        return Err(Error::Codec);
    };
    if values.is_empty() || values.len() > meter.budget.limits.nodes {
        return Err(Error::InvalidStructure);
    }
    meter.work(mul(values.len(), 512)?)?;
    meter.owned(mul(
        values.len(),
        add(size_of::<MarkdownStructuralUnit>(), 256)?,
    )?)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| Error::AllocationFailed)?;
    for value in values.into_vec() {
        meter.poll()?;
        let mut record = Record::new(value, 12)?;
        let ordinal = unsigned(&record.next()?)?;
        let token = kind(&string(record.next()?)?)?.0;
        let heading_level =
            u8::try_from(unsigned(&record.next()?)?).map_err(|_| Error::InvalidStructure)?;
        let parent_section = optional(&record.next()?)?;
        let physical_parent = optional(&record.next()?)?;
        let heading_section = optional(&record.next()?)?;
        let start = unsigned(&record.next()?)?;
        let end = unsigned(&record.next()?)?;
        let code_info = mapped(record.next()?, metadata, meter)?;
        let id = UnitId::from_bytes(fixed(record.next()?)?);
        let content = digest(record.next()?)?;
        let identity = digest(record.next()?)?;
        let data = MarkdownNodeData {
            ordinal,
            kind: token,
            heading_level,
            parent_section,
            physical_parent,
            heading_section,
            start,
            end,
            code_info,
            content,
        };
        let occurrence = occurrence(
            id,
            manifest.structural,
            ordinal,
            token,
            (start, end),
            manifest.raw.provenance().input_digest(),
            identity,
        )?;
        output.push(MarkdownStructuralUnit {
            data,
            occurrence,
            identity,
        });
    }
    Ok(output)
}
fn inline(
    value: CanonicalValue,
    metadata: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Vec<MarkdownInlineObservation>, Error> {
    let CanonicalValue::Array(values) = value else {
        return Err(Error::Codec);
    };
    if values.len() > meter.budget.limits.inline_records {
        return Err(Error::InvalidStructure);
    }
    meter.work(mul(values.len(), 128)?)?;
    meter.owned(mul(values.len(), size_of::<MarkdownInlineObservation>())?)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| Error::AllocationFailed)?;
    for value in values.into_vec() {
        meter.poll()?;
        let mut record = Record::new(value, 7)?;
        output.push(MarkdownInlineObservation {
            owner: unsigned(&record.next()?)?,
            form: inline_form(&string(record.next()?)?)?,
            start: unsigned(&record.next()?)?,
            end: unsigned(&record.next()?)?,
            text: mapped(record.next()?, metadata, meter)?,
            destination: mapped(record.next()?, metadata, meter)?,
            title: mapped(record.next()?, metadata, meter)?,
        });
    }
    Ok(output)
}
fn decode(
    encoded: &[u8],
    profile: &MarkdownUnitizerProfile,
    meter: &mut Meter<'_>,
) -> Result<MarkdownUnitManifest, Error> {
    if encoded.is_empty() || encoded.len() > meter.budget.limits.encoded_bytes {
        return Err(Error::CanonicalLimit);
    }
    // At most one logical value per input byte; key/record allowance includes
    // decoded graph and retained copies. Shared parser internals/spare capacity
    // are excluded; its own finite global limits still apply.
    meter.owned(add(
        mul(encoded.len(), add(size_of::<CanonicalValue>(), 128)?)?,
        MAX_CANONICAL_BYTES,
    )?)?;
    meter.work(add(mul(encoded.len(), 8)?, MAX_CANONICAL_BYTES)?)?;
    let value = search_contracts::parse_canonical_cbor(encoded).map_err(|_| Error::Codec)?;
    meter.poll()?;
    let mut envelope = Record::new(value, 4)?;
    if string(envelope.next()?)? != FORMAT || unsigned(&envelope.next()?)? != 4 {
        return Err(Error::Codec);
    }
    let mut record = Record::new(envelope.next()?, 12)?;
    let stored_digest = digest(envelope.next()?)?;
    meter.rows(64, 2048)?;
    if record.next()? != profile.value()? || string(record.next()?)? != profile.id().as_str() {
        return Err(Error::ProfileMismatch);
    }
    let CanonicalValue::Bytes(raw) = record.next()? else {
        return Err(Error::Codec);
    };
    // The unchanged v3 decoder validates its profile and canonical digest and
    // reencodes; reserve both complete hash caps plus graph/record/output costs.
    meter.canonical(add(mul(MAX_CANONICAL_BYTES, 2)?, 128)?)?;
    meter.work(add(mul(MAX_CANONICAL_BYTES, 4)?, mul(raw.len(), 8)?)?)?;
    meter.owned(add(
        mul(raw.len(), add(size_of::<CanonicalValue>(), 128)?)?,
        mul(4096, 512)?,
    )?)?;
    let raw = crate::decode_unit_manifest(raw.as_slice(), MAX_CANONICAL_BYTES)?;
    meter.poll()?;
    let mut manifest = MarkdownManifestBody {
        profile: profile.clone(),
        raw,
        structural: RepresentationId::from_bytes(fixed(record.next()?)?),
        residency: OpaqueId::new(string(record.next()?)?).map_err(|_| Error::Codec)?,
        markdown_product: digest(record.next()?)?,
        markdown_nodes: digest(record.next()?)?,
        markdown_inline: digest(record.next()?)?,
        nodes: Vec::new(),
        inline: Vec::new(),
    };
    if string(record.next()?)? != INLINE_STATUS || string(record.next()?)? != REFERENCE_STATUS {
        return Err(Error::Codec);
    }
    let mut metadata = 0;
    manifest.nodes = nodes(record.next()?, &manifest, &mut metadata, meter)?;
    manifest.inline = inline(record.next()?, &mut metadata, meter)?;
    shape(&manifest, meter)?;
    let value = body(&manifest, meter)?;
    if meter.hash("eliot/cbor/md-unit-manifest/v4", value)? != stored_digest {
        return Err(Error::ManifestMismatch);
    }
    let manifest = MarkdownUnitManifest {
        body: manifest,
        digest: stored_digest,
    };
    meter.work(encoded.len())?;
    if encode(&manifest, meter)?.as_slice() != encoded {
        return Err(Error::ManifestMismatch);
    }
    Ok(manifest)
}
/// Decode closed canonical v4 bytes as a proposal; owning exact replay is mandatory.
pub fn decode_markdown_unit_manifest(
    encoded: &[u8],
    profile: &MarkdownUnitizerProfile,
    budget: &MarkdownUnitizationBudget<'_>,
) -> Result<MarkdownUnitManifest, Error> {
    let mut meter = Meter::new(profile, budget)?;
    let result = decode(encoded, profile, &mut meter);
    meter.poll()?;
    result
}
