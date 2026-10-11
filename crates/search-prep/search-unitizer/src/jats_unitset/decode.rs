//! Exact positional decoding creates proposals, never source verification authority.
use super::budget::{Meter, add, index, mul};
use super::codec::{CHUNK, attribute, body, element, encode, shape, text_form};
use super::model::JatsUnitizationError as Error;
use super::model::{
    JatsMappedValue, JatsStructuralUnit, JatsUnitAttribute, JatsUnitComment, JatsUnitManifest,
    JatsUnitSpan, JatsUnitText, ManifestBody, NodeData, STATUS,
};
use super::profile::FORMAT;
use super::{JatsUnitizationBudget, JatsUnitizerProfile};
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
fn span(value: CanonicalValue) -> Result<JatsUnitSpan, Error> {
    let mut row = Record::new(value, 2)?;
    Ok(JatsUnitSpan {
        start: unsigned(&row.next()?)?,
        end: unsigned(&row.next()?)?,
    })
}
fn optional_span(value: CanonicalValue) -> Result<Option<JatsUnitSpan>, Error> {
    if value == CanonicalValue::Null {
        Ok(None)
    } else {
        span(value).map(Some)
    }
}
fn mapped(
    value: CanonicalValue,
    total: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<JatsMappedValue, Error> {
    let mut row = Record::new(value, 2)?;
    if string(row.next()?)? != "mapped-text" {
        return Err(Error::Codec);
    }
    let CanonicalValue::Text(value) = row.next()? else {
        return Err(Error::Codec);
    };
    let length = value.as_str().len();
    *total = add(*total, length)?;
    if length > meter.budget.limits.metadata_bytes_each
        || *total > meter.budget.limits.metadata_bytes_total
    {
        return Err(Error::InvalidStructure);
    }
    meter.work(length)?;
    meter.owned(add(size_of::<JatsMappedValue>(), length)?)?;
    Ok(JatsMappedValue {
        value: value.into_string(),
    })
}
fn chunks<T, F>(
    value: CanonicalValue,
    count: usize,
    cap: usize,
    meter: &mut Meter<'_>,
    mut parse: F,
) -> Result<Vec<T>, Error>
where
    F: FnMut(CanonicalValue, &mut Meter<'_>) -> Result<T, Error>,
{
    if count > cap {
        return Err(Error::InvalidStructure);
    }
    let CanonicalValue::Array(chunks) = value else {
        return Err(Error::Codec);
    };
    if chunks.len() != count.div_ceil(CHUNK) {
        return Err(Error::Codec);
    }
    meter.work(mul(count, 512)?)?;
    meter.owned(mul(count, add(size_of::<T>(), 256)?)?)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    for chunk in chunks.into_vec() {
        meter.poll()?;
        let CanonicalValue::Array(rows) = chunk else {
            return Err(Error::Codec);
        };
        if rows.len()
            != count
                .checked_sub(result.len())
                .ok_or(Error::ArithmeticOverflow)?
                .min(CHUNK)
        {
            return Err(Error::Codec);
        }
        for row in rows.into_vec() {
            meter.poll()?;
            result.push(parse(row, meter)?);
        }
    }
    if result.len() != count {
        return Err(Error::Codec);
    }
    Ok(result)
}
fn node(
    value: CanonicalValue,
    m: &ManifestBody,
    meter: &mut Meter<'_>,
) -> Result<JatsStructuralUnit, Error> {
    meter.work(128)?;
    let mut row = Record::new(value, 11)?;
    let data = NodeData {
        ordinal: unsigned(&row.next()?)?,
        kind: element(&string(row.next()?)?)?,
        parent: optional(&row.next()?)?,
        event: unsigned(&row.next()?)?,
        close_event: unsigned(&row.next()?)?,
        opening: span(row.next()?)?,
        closing: optional_span(row.next()?)?,
        span: span(row.next()?)?,
        content: digest(row.next()?)?,
    };
    let id = UnitId::from_bytes(fixed(row.next()?)?);
    let identity = digest(row.next()?)?;
    let occurrence = super::build::occurrence(
        id,
        m.structural,
        &data,
        m.raw.provenance().input_digest(),
        identity,
    )?;
    Ok(JatsStructuralUnit {
        data,
        occurrence,
        identity,
    })
}
fn attr(
    value: CanonicalValue,
    total: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<JatsUnitAttribute, Error> {
    let mut row = Record::new(value, 4)?;
    Ok(JatsUnitAttribute {
        owner: unsigned(&row.next()?)?,
        kind: attribute(&string(row.next()?)?)?,
        span: span(row.next()?)?,
        value: mapped(row.next()?, total, meter)?,
    })
}
fn content(
    value: CanonicalValue,
    total: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<JatsUnitText, Error> {
    let mut row = Record::new(value, 5)?;
    Ok(JatsUnitText {
        owner: unsigned(&row.next()?)?,
        event: unsigned(&row.next()?)?,
        kind: text_form(&string(row.next()?)?)?,
        span: span(row.next()?)?,
        value: mapped(row.next()?, total, meter)?,
    })
}
fn comment(value: CanonicalValue, _meter: &mut Meter<'_>) -> Result<JatsUnitComment, Error> {
    let mut row = Record::new(value, 3)?;
    Ok(JatsUnitComment {
        owner: optional(&row.next()?)?,
        event: unsigned(&row.next()?)?,
        span: span(row.next()?)?,
    })
}
fn decode(
    encoded: &[u8],
    profile: &JatsUnitizerProfile,
    meter: &mut Meter<'_>,
) -> Result<JatsUnitManifest, Error> {
    if encoded.is_empty() || encoded.len() > meter.budget.limits.encoded_bytes {
        return Err(Error::CanonicalLimit);
    }
    // Bounded hostile shared-codec graph reservation before parsing; no RSS claim.
    meter.owned(add(
        mul(encoded.len(), add(size_of::<CanonicalValue>(), 128)?)?,
        MAX_CANONICAL_BYTES,
    )?)?;
    meter.work(add(mul(encoded.len(), 8)?, MAX_CANONICAL_BYTES)?)?;
    let value = search_contracts::parse_canonical_cbor(encoded).map_err(|_| Error::Codec)?;
    meter.poll()?;
    let mut envelope = Record::new(value, 4)?;
    if string(envelope.next()?)? != FORMAT || unsigned(&envelope.next()?)? != 5 {
        return Err(Error::Codec);
    }
    let mut row = Record::new(envelope.next()?, 18)?;
    let stored = digest(envelope.next()?)?;
    meter.rows(64, 2048)?;
    if row.next()? != profile.value()? || string(row.next()?)? != profile.id().as_str() {
        return Err(Error::ProfileMismatch);
    }
    let CanonicalValue::Bytes(raw) = row.next()? else {
        return Err(Error::Codec);
    };
    meter.canonical(add(mul(MAX_CANONICAL_BYTES, 2)?, 128)?)?;
    meter.work(add(mul(MAX_CANONICAL_BYTES, 4)?, mul(raw.len(), 8)?)?)?;
    meter.owned(add(
        mul(raw.len(), add(size_of::<CanonicalValue>(), 128)?)?,
        mul(4096, 512)?,
    )?)?;
    let raw = crate::decode_unit_manifest(raw.as_slice(), MAX_CANONICAL_BYTES)?;
    meter.poll()?;
    let mut m = ManifestBody {
        profile: profile.clone(),
        raw,
        structural: RepresentationId::from_bytes(fixed(row.next()?)?),
        residency: OpaqueId::new(string(row.next()?)?).map_err(|_| Error::Codec)?,
        product: digest(row.next()?)?,
        inventory: digest(row.next()?)?,
        events: unsigned(&row.next()?)?,
        declaration: optional_span(row.next()?)?,
        nodes: Vec::new(),
        attributes: Vec::new(),
        texts: Vec::new(),
        comments: Vec::new(),
    };
    if string(row.next()?)? != STATUS {
        return Err(Error::Codec);
    }
    let counts = [
        index(unsigned(&row.next()?)?)?,
        index(unsigned(&row.next()?)?)?,
        index(unsigned(&row.next()?)?)?,
        index(unsigned(&row.next()?)?)?,
    ];
    let l = meter.budget.limits;
    if counts[0] == 0
        || counts[0] > l.nodes
        || counts[1] > l.attributes
        || counts[2] > l.texts
        || counts[3] > l.comments
    {
        return Err(Error::InvalidStructure);
    }
    m.nodes = chunks(row.next()?, counts[0], l.nodes, meter, |v, meter| {
        node(v, &m, meter)
    })?;
    let mut metadata = 0;
    m.attributes = chunks(row.next()?, counts[1], l.attributes, meter, |v, meter| {
        attr(v, &mut metadata, meter)
    })?;
    m.texts = chunks(row.next()?, counts[2], l.texts, meter, |v, meter| {
        content(v, &mut metadata, meter)
    })?;
    m.comments = chunks(row.next()?, counts[3], l.comments, meter, comment)?;
    shape(&m, meter)?;
    let value = body(&m, meter)?;
    if meter.hash("eliot/cbor/jats-unit-manifest/v5", value)? != stored {
        return Err(Error::ManifestMismatch);
    }
    let manifest = JatsUnitManifest {
        body: m,
        digest: stored,
    };
    meter.work(encoded.len())?;
    if encode(&manifest, meter)?.as_slice() != encoded {
        return Err(Error::ManifestMismatch);
    }
    Ok(manifest)
}
/// Decode closed canonical v5 bytes as a proposal; complete owning replay is mandatory.
pub fn decode_jats_unit_manifest(
    encoded: &[u8],
    profile: &JatsUnitizerProfile,
    budget: &JatsUnitizationBudget<'_>,
) -> Result<JatsUnitManifest, Error> {
    let mut meter = Meter::new(profile, budget)?;
    let result = decode(encoded, profile, &mut meter);
    meter.poll()?;
    result
}
