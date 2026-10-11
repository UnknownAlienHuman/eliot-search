//! Closed v5 records, full 256-record chunks and the sole shared canonical codec.
use super::budget::{Meter, add, index, mul, number};
use super::model::JatsUnitizationError as Error;
use super::model::{
    CanonicalJatsUnitManifestBytes, JatsMappedValue, JatsStructuralUnit, JatsUnitAttribute,
    JatsUnitComment, JatsUnitManifest, JatsUnitSpan, JatsUnitText, ManifestBody, NodeData, STATUS,
};
use super::profile::FORMAT;
use super::{JatsUnitizationBudget, JatsUnitizerProfile};
use search_contracts::{BoundedBytes, BoundedList, CanonicalText, CanonicalValue};
use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

pub(super) const CHUNK: usize = 256;
const ELEMENTS: [&str; 47] = [
    "article",
    "front",
    "article-meta",
    "journal-meta",
    "journal-id",
    "issn",
    "title-group",
    "article-title",
    "subtitle",
    "abstract",
    "body",
    "sec",
    "title",
    "p",
    "list",
    "list-item",
    "boxed-text",
    "disp-quote",
    "code",
    "preformat",
    "fig",
    "caption",
    "table-wrap",
    "table",
    "thead",
    "tbody",
    "tr",
    "th",
    "td",
    "ref-list",
    "ref",
    "xref",
    "ext-link",
    "back",
    "ack",
    "fn-group",
    "fn",
    "italic",
    "bold",
    "sup",
    "sub",
    "monospace",
    "underline",
    "named-content",
    "label",
    "mixed-citation",
    "element-citation",
];
const ATTRIBUTES: [&str; 26] = [
    "id",
    "rid",
    "ref-type",
    "xlink:href",
    "dtd-version",
    "article-type",
    "xml:lang",
    "xml:space",
    "sec-type",
    "list-type",
    "content-type",
    "specific-use",
    "position",
    "orientation",
    "colspan",
    "rowspan",
    "align",
    "valign",
    "scope",
    "headers",
    "publication-type",
    "xlink:type",
    "xmlns:xlink",
    "xmlns:xml",
    "journal-id-type",
    "publication-format",
];
pub(super) fn element(token: &str) -> Result<&'static str, Error> {
    ELEMENTS
        .into_iter()
        .find(|n| *n == token)
        .ok_or(Error::InvalidStructure)
}
pub(super) fn attribute(token: &str) -> Result<&'static str, Error> {
    ATTRIBUTES
        .into_iter()
        .find(|n| *n == token)
        .ok_or(Error::InvalidStructure)
}
pub(super) fn text_form(token: &str) -> Result<&'static str, Error> {
    ["text", "cdata", "reference"]
        .into_iter()
        .find(|n| *n == token)
        .ok_or(Error::InvalidStructure)
}
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
pub(super) fn span(value: JatsUnitSpan) -> Result<CanonicalValue, Error> {
    array(vec![
        CanonicalValue::U64(value.start),
        CanonicalValue::U64(value.end),
    ])
}
pub(super) fn optional_span(value: Option<JatsUnitSpan>) -> Result<CanonicalValue, Error> {
    value.map_or(Ok(CanonicalValue::Null), span)
}
pub(super) fn node_identity(n: &NodeData, meter: &mut Meter<'_>) -> Result<CanonicalValue, Error> {
    meter.rows(20, 128)?;
    array(vec![
        CanonicalValue::U64(n.ordinal),
        text(n.kind)?,
        optional(n.parent),
        CanonicalValue::U64(n.event),
        CanonicalValue::U64(n.close_event),
        span(n.opening)?,
        optional_span(n.closing)?,
        span(n.span)?,
        bytes(n.content.as_bytes())?,
    ])
}
fn node(n: &JatsStructuralUnit, meter: &mut Meter<'_>) -> Result<CanonicalValue, Error> {
    let CanonicalValue::Array(values) = node_identity(&n.data, meter)? else {
        return Err(Error::Codec);
    };
    meter.rows(2, 48)?;
    let mut values = values.into_vec();
    values.push(bytes(n.occurrence.unit_id.as_bytes())?);
    values.push(bytes(n.identity.as_bytes())?);
    array(values)
}
fn mapped(value: &JatsMappedValue) -> Result<CanonicalValue, Error> {
    array(vec![text("mapped-text")?, text(value.value())?])
}
fn attr(a: &JatsUnitAttribute, meter: &mut Meter<'_>) -> Result<CanonicalValue, Error> {
    meter.rows(10, add(128, a.value.value.len())?)?;
    array(vec![
        CanonicalValue::U64(a.owner),
        text(a.kind)?,
        span(a.span)?,
        mapped(&a.value)?,
    ])
}
fn content(t: &JatsUnitText, meter: &mut Meter<'_>) -> Result<CanonicalValue, Error> {
    meter.rows(11, add(128, t.value.value.len())?)?;
    array(vec![
        CanonicalValue::U64(t.owner),
        CanonicalValue::U64(t.event),
        text(t.kind)?,
        span(t.span)?,
        mapped(&t.value)?,
    ])
}
fn comment(c: &JatsUnitComment, meter: &mut Meter<'_>) -> Result<CanonicalValue, Error> {
    meter.rows(6, 0)?;
    array(vec![
        optional(c.owner),
        CanonicalValue::U64(c.event),
        span(c.span)?,
    ])
}
fn chunks<T>(
    records: &[T],
    encode: fn(&T, &mut Meter<'_>) -> Result<CanonicalValue, Error>,
    meter: &mut Meter<'_>,
) -> Result<CanonicalValue, Error> {
    let count = records.len().div_ceil(CHUNK);
    meter.rows(add(records.len(), count)?, 0)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    for chunk in records.chunks(CHUNK) {
        meter.poll()?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(chunk.len())
            .map_err(|_| Error::AllocationFailed)?;
        for record in chunk {
            rows.push(encode(record, meter)?);
        }
        output.push(array(rows)?);
    }
    array(output)
}
const fn checked_span(s: JatsUnitSpan, cap: u64, nonempty: bool) -> Result<(), Error> {
    if s.start > s.end || (nonempty && s.start == s.end) || s.end > cap {
        return Err(Error::InvalidStructure);
    }
    Ok(())
}
const fn contains(outer: JatsUnitSpan, inner: JatsUnitSpan) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}
fn event(events: &mut BTreeSet<u64>, event: u64, cap: u64) -> Result<(), Error> {
    if event >= cap.saturating_sub(1) || !events.insert(event) {
        return Err(Error::InvalidStructure);
    }
    Ok(())
}
fn owner(
    m: &ManifestBody,
    ordinal: u64,
    extent: JatsUnitSpan,
    event: Option<u64>,
) -> Result<&NodeData, Error> {
    let n = &m
        .nodes
        .get(index(ordinal)?)
        .ok_or(Error::InvalidStructure)?
        .data;
    let inner = JatsUnitSpan {
        start: n.opening.end,
        end: n.closing.map_or(n.opening.end, |c| c.start),
    };
    if !contains(inner, extent) || event.is_some_and(|e| e <= n.event || e >= n.close_event) {
        return Err(Error::InvalidStructure);
    }
    Ok(n)
}
pub(super) fn shape(m: &ManifestBody, meter: &mut Meter<'_>) -> Result<(), Error> {
    let l = meter.budget.limits;
    if m.nodes.is_empty()
        || m.nodes.len() > l.nodes
        || m.attributes.len() > l.attributes
        || m.texts.len() > l.texts
        || m.comments.len() > l.comments
        || m.events == 0
        || m.events > number(l.events)?
        || m.raw.unit_count() > l.raw_units
        || m.raw.input_bytes() > number(l.input_bytes)?
        || m.raw.emitted_bytes() != m.raw.input_bytes()
        || m.raw.omitted_bytes() != 0
        || m.raw.provenance().binding().representation_id == m.structural
        || m.raw.profile_id() != &m.profile.raw_id
    {
        return Err(Error::InvalidStructure);
    }
    let records = add(
        add(m.nodes.len(), m.attributes.len())?,
        add(m.texts.len(), m.comments.len())?,
    )?;
    meter.work(add(mul(records, 2048)?, mul(m.raw.unit_count(), 2048)?)?)?;
    meter.owned(mul(add(records, m.raw.unit_count())?, 512)?)?;
    let mut metadata = 0;
    for v in m
        .attributes
        .iter()
        .map(|a| &a.value)
        .chain(m.texts.iter().map(|t| &t.value))
    {
        meter.poll()?;
        metadata = add(metadata, v.value.len())?;
        if v.value.len() > l.metadata_bytes_each || metadata > l.metadata_bytes_total {
            return Err(Error::InvalidStructure);
        }
    }
    meter.work(metadata)?;
    let ctx = super::build::context(m, meter)?;
    let mut ids = BTreeMap::new();
    let mut events = BTreeSet::new();
    if m.declaration.is_some() {
        events.insert(0);
    }
    for u in m.raw.units() {
        super::build::record(&mut ids, u.unit_id(), u.identity_digest())?;
    }
    let mut previous = None;
    for (ordinal, u) in m.nodes.iter().enumerate() {
        meter.poll()?;
        let d = &u.data;
        element(d.kind)?;
        checked_span(d.span, m.raw.input_bytes(), true)?;
        checked_span(d.opening, m.raw.input_bytes(), true)?;
        if d.ordinal != number(ordinal)?
            || d.opening.start != d.span.start
            || !contains(d.span, d.opening)
            || previous.is_some_and(|p| d.event <= p)
            || d.close_event < d.event
        {
            return Err(Error::InvalidStructure);
        }
        previous = Some(d.event);
        event(&mut events, d.event, m.events)?;
        if let Some(c) = d.closing {
            checked_span(c, m.raw.input_bytes(), true)?;
            if c.end != d.span.end || c.start < d.opening.end || d.close_event <= d.event {
                return Err(Error::InvalidStructure);
            }
            event(&mut events, d.close_event, m.events)?;
        } else if d.span != d.opening || d.close_event != d.event {
            return Err(Error::InvalidStructure);
        }
        if ordinal == 0 {
            if d.kind != "article" || d.parent.is_some() {
                return Err(Error::InvalidStructure);
            }
        } else {
            let p = d.parent.ok_or(Error::InvalidStructure)?;
            if p >= d.ordinal || d.kind == "article" {
                return Err(Error::InvalidStructure);
            }
            let parent = &m.nodes[index(p)?].data;
            if !contains(parent.span, d.span)
                || parent.event >= d.event
                || parent.close_event <= d.close_event
                || d.opening.start < parent.opening.end
                || d.span.end > parent.closing.ok_or(Error::InvalidStructure)?.start
            {
                return Err(Error::InvalidStructure);
            }
        }
        let mut depth = 1;
        let mut ancestor = d.parent;
        while let Some(p) = ancestor {
            depth = add(depth, 1)?;
            if depth > l.depth {
                return Err(Error::InvalidStructure);
            }
            ancestor = m.nodes[index(p)?].data.parent;
        }
        if super::build::identity(ctx, d, meter)? != u.identity
            || super::build::occurrence(
                super::build::truncated(u.identity),
                m.structural,
                d,
                m.raw.provenance().input_digest(),
                u.identity,
            )? != u.occurrence
        {
            return Err(Error::InvalidStructure);
        }
        super::build::record(&mut ids, u.occurrence.unit_id, u.identity)?;
    }
    if let Some(d) = m.declaration {
        checked_span(d, m.raw.input_bytes(), true)?;
        if d.start != 0 || d.end > m.nodes[0].data.opening.start {
            return Err(Error::InvalidStructure);
        }
    }
    let mut attrs = BTreeSet::new();
    let mut previous = None;
    for a in &m.attributes {
        meter.poll()?;
        attribute(a.kind)?;
        let n = &m
            .nodes
            .get(index(a.owner)?)
            .ok_or(Error::InvalidStructure)?
            .data;
        if previous.is_some_and(|p| a.owner < p)
            || a.span != n.opening
            || !attrs.insert((a.owner, a.kind))
        {
            return Err(Error::InvalidStructure);
        }
        previous = Some(a.owner);
    }
    let mut previous = None;
    for t in &m.texts {
        meter.poll()?;
        text_form(t.kind)?;
        checked_span(t.span, m.raw.input_bytes(), false)?;
        if previous.is_some_and(|p| t.event <= p) {
            return Err(Error::InvalidStructure);
        }
        previous = Some(t.event);
        event(&mut events, t.event, m.events)?;
        owner(m, t.owner, t.span, Some(t.event))?;
    }
    let mut previous = None;
    for c in &m.comments {
        meter.poll()?;
        checked_span(c.span, m.raw.input_bytes(), true)?;
        if previous.is_some_and(|p| c.event <= p) {
            return Err(Error::InvalidStructure);
        }
        previous = Some(c.event);
        event(&mut events, c.event, m.events)?;
        if let Some(o) = c.owner {
            owner(m, o, c.span, Some(c.event))?;
        } else if c.span.start < m.nodes[0].data.span.end && c.span.end > m.nodes[0].data.span.start
        {
            return Err(Error::InvalidStructure);
        }
    }
    meter.poll()
}
pub(super) fn body(m: &ManifestBody, meter: &mut Meter<'_>) -> Result<CanonicalValue, Error> {
    shape(m, meter)?;
    let cap = m.raw.profile().max_manifest_bytes;
    meter.work(add(mul(cap, 2)?, 4096)?)?;
    meter.canonical(add(cap, 128)?)?;
    meter.owned(add(
        mul(cap, 3)?,
        mul(
            add(mul(m.raw.unit_count(), 32)?, 128)?,
            size_of::<CanonicalValue>(),
        )?,
    )?)?;
    let raw = crate::canonicalize_unit_manifest(&m.raw)?;
    meter.poll()?;
    meter.rows(64, 2048)?;
    array(vec![
        m.profile.value()?,
        text(m.profile.id().as_str())?,
        CanonicalValue::Bytes(BoundedBytes::new(raw.bytes).map_err(|_| Error::Codec)?),
        bytes(m.structural.as_bytes())?,
        text(m.residency.as_str())?,
        bytes(m.product.as_bytes())?,
        bytes(m.inventory.as_bytes())?,
        CanonicalValue::U64(m.events),
        optional_span(m.declaration)?,
        text(STATUS)?,
        CanonicalValue::U64(number(m.nodes.len())?),
        CanonicalValue::U64(number(m.attributes.len())?),
        CanonicalValue::U64(number(m.texts.len())?),
        CanonicalValue::U64(number(m.comments.len())?),
        chunks(&m.nodes, node, meter)?,
        chunks(&m.attributes, attr, meter)?,
        chunks(&m.texts, content, meter)?,
        chunks(&m.comments, comment, meter)?,
    ])
}
pub(super) fn encode(
    m: &JatsUnitManifest,
    meter: &mut Meter<'_>,
) -> Result<CanonicalJatsUnitManifestBytes, Error> {
    meter.rows(4, 128)?;
    let body = body(&m.body, meter)?;
    // Shared encoder's own global output cap is reserved before its bounded call.
    meter.owned(search_contracts::MAX_CANONICAL_BYTES)?;
    meter.work(search_contracts::MAX_CANONICAL_BYTES)?;
    let envelope = array(vec![
        text(FORMAT)?,
        CanonicalValue::U64(5),
        body,
        bytes(m.digest.as_bytes())?,
    ])?;
    let encoded = search_contracts::to_canonical_cbor(&envelope).map_err(|_| Error::Codec)?;
    meter.poll()?;
    if encoded.as_slice().len() > meter.budget.limits.encoded_bytes {
        return Err(Error::CanonicalLimit);
    }
    Ok(CanonicalJatsUnitManifestBytes {
        bytes: encoded.into_vec(),
    })
}
/// Encode a closed proposal without claiming exact-source completeness.
pub fn canonicalize_jats_unit_manifest(
    m: &JatsUnitManifest,
    profile: &JatsUnitizerProfile,
    budget: &JatsUnitizationBudget<'_>,
) -> Result<CanonicalJatsUnitManifestBytes, Error> {
    let mut meter = Meter::new(profile, budget)?;
    let result = (|| {
        if m.body.profile != *profile {
            return Err(Error::ProfileMismatch);
        }
        let value = body(&m.body, &mut meter)?;
        if meter.hash("eliot/cbor/jats-unit-manifest/v5", value)? != m.digest {
            return Err(Error::ManifestMismatch);
        }
        encode(m, &mut meter)
    })();
    meter.poll()?;
    result
}
