//! Finite schema rows feed the existing shared CBOR/BLAKE3 stream once.
use super::meter::Meter;
use super::model::{Inventory, JatsError as E, JatsLimits, JatsSpan, JatsTextKind};
use super::profile::{
    JatsOperationLimits, JatsProfile, MAX_JATS_LIMITS, MAX_JATS_PREIMAGE_BYTES,
    MAX_JATS_TOTAL_CANONICAL_BYTES,
};
use crate::api::{
    MaterializationProduct, ValidatedMaterializationRequest, ValidatedMaterializerProfile,
};
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue as V, ContractError, ContractErrorKind, DigestInputLimit,
    blake3_canonical_array_with_usage, blake3_raw,
};
use std::mem::size_of;

const DONOR: &str = "quick-xml=0.42.0;MIT;edition2024;msrv1.86;build=false;defaults=false;features=[];sha256=41b1177fdf999d2321d3fb46ff47159d9c1fb9ad66a4879f8c50a0b504615e9b;memchr=2.8.3;features=alloc,default,std;sha256=cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98";
const POLICY: &str = "Publishing1.4/2024-10-31;driver-sha256=ea004da4d29f90ce42398989e6cad975ce20c4eba2fe6df3dfe563c9754df5f5;elements-sha256=5985f6551a2235ca5ffa694c019e5368328374fd9c45d5951f3e06f6f92ac514;not-complete-schema-closure;XML1.0;UTF8-only;strict-positive-input;BOM-reject;raw-preserve;closed-element-attribute-parent-tables;unknown-reject;front-journal-meta-with-journal-id-and-issn-required;title-group-title-then-subtitles;root-only-literal-xlink-and-xml-prefixes;no-default-namespace-or-rebinding;DTD-DOCTYPE-PI-custom-entity-reject;predefined-numeric-only;explicit-resolve-xml-entity;comments-inert;no-full-schema-validity-claim;mapped-XML10-text-and-attributes;whole-event-provenance;raw-XML-exact-evidence;all-event-ordinals-including-EOF;overlapping-extents-not-coverage;256-record-canonical-chunks-complete-except-last;logical-cumulative-storage-no-refunds;512-construction-units-per-schema-record-plus-copied-value-bytes;raw-budget-independent;original-cancel-and-deadline;VERSIONED_REBUILD";
const OPTIONS: &str = "allow_dangling_amp=false;allow_unmatched_ends=false;check_comments=true;check_end_names=true;expand_empty_elements=false;trim_text_start=false;trim_text_end=false;trim_markup_names_in_closing_tags=true;Reader::from_str;borrowed-pull-events;no-encoding-feature";
const DEFINITIONS: &str = concat!(
    include_str!("../jats.rs"),
    include_str!("model.rs"),
    include_str!("profile.rs"),
    include_str!("meter.rs"),
    include_str!("parse.rs"),
    include_str!("identity.rs"),
    include_str!("pipeline.rs"),
);

struct Rows<'a, 'b> {
    meter: &'a mut Meter<'b>,
    prepaid: usize,
}
impl Rows<'_, '_> {
    fn units(&mut self, amount: usize) -> Result<(), E> {
        self.meter.poll()?;
        self.prepaid = self.prepaid.checked_sub(amount).ok_or(E::LimitExceeded)?;
        Ok(())
    }
    fn array<const N: usize>(&mut self, values: [V; N]) -> Result<V, E> {
        self.units(N + 1)?;
        self.meter
            .own(N.checked_mul(size_of::<V>()).ok_or(E::Overflow)?)?;
        let mut out = Vec::new();
        out.try_reserve_exact(N).map_err(|_| E::AllocationFailed)?;
        out.extend(values);
        BoundedList::new(out)
            .map(V::Array)
            .map_err(|_| E::CanonicalEncoding)
    }
    fn text(&mut self, value: &str) -> Result<V, E> {
        self.units(value.len())?;
        self.meter.own(value.len())?;
        let mut out = String::new();
        out.try_reserve_exact(value.len())
            .map_err(|_| E::AllocationFailed)?;
        out.push_str(value);
        CanonicalText::new(out)
            .map(V::Text)
            .map_err(|_| E::CanonicalEncoding)
    }
    fn bytes(&mut self, value: &[u8]) -> Result<V, E> {
        self.units(value.len())?;
        self.meter.own(value.len())?;
        let mut out = Vec::new();
        out.try_reserve_exact(value.len())
            .map_err(|_| E::AllocationFailed)?;
        out.extend_from_slice(value);
        BoundedBytes::new(out)
            .map(V::Bytes)
            .map_err(|_| E::CanonicalEncoding)
    }
    fn span(&mut self, value: JatsSpan) -> Result<V, E> {
        self.array([num(value.start)?, num(value.end)?])
    }
    fn optional_span(&mut self, value: Option<JatsSpan>) -> Result<V, E> {
        value.map_or(Ok(V::Null), |value| self.span(value))
    }
}
macro_rules! row {
    ($cx:expr; $($value:expr),* $(,)?) => {{
        let fields = [$($value),*];
        $cx.array(fields)
    }};
}
fn num(value: usize) -> Result<V, E> {
    u64::try_from(value).map(V::U64).map_err(|_| E::Overflow)
}
fn optional(value: Option<usize>) -> Result<V, E> {
    value.map(num).transpose().map(|v| v.unwrap_or(V::Null))
}
fn frame(
    meter: &mut Meter<'_>,
    domain: &str,
    count: usize,
    copied: usize,
    construction_rows: usize,
    mut make: impl FnMut(usize, &mut Rows<'_, '_>) -> Result<V, E>,
) -> Result<Blake3Digest32, E> {
    // Logical construction reservation, not an encoded-size formula.
    let reservation = construction_rows
        .checked_mul(512)
        .and_then(|v| v.checked_add(copied))
        .ok_or(E::Overflow)?;
    meter.work(reservation)?;
    let cap = meter.canonical_cap();
    let mut rows = Rows {
        meter,
        prepaid: reservation,
    };
    let mut index = 0;
    let mut refusal = None;
    let stream = std::iter::from_fn(|| {
        let result = (|| {
            rows.meter.poll()?;
            if index == count {
                return Ok(None);
            }
            let row = make(index, &mut rows)?;
            rows.meter.poll()?;
            index += 1;
            Ok(Some(row))
        })();
        match result {
            Ok(row) => row.map(Ok),
            Err(error) => {
                refusal = Some(error);
                Some(Err(ContractError::new(
                    ContractErrorKind::MalformedPayload,
                    "jats_row",
                )))
            }
        }
    });
    let output = blake3_canonical_array_with_usage(
        &CanonicalDigestDomain::parse(domain).map_err(|_| E::CanonicalEncoding)?,
        count,
        stream,
        DigestInputLimit::new(cap).map_err(|_| E::CanonicalEncoding)?,
    );
    if let Some(error) = refusal {
        return Err(error);
    }
    let output = output.map_err(|_| E::CanonicalEncoding)?;
    meter.complete(output.complete_preimage_bytes())?;
    meter.poll()?;
    Ok(output.digest())
}
fn definition(bytes: &[u8], fixture: bool, meter: &mut Meter<'_>) -> Result<Blake3Digest32, E> {
    let domain = if fixture {
        "eliot/raw/jats-fixtures/v1"
    } else {
        "eliot/raw/jats-source/v1"
    };
    let cap = if fixture { 32_768 } else { 131_072 };
    if bytes.len() > cap - domain.len() - 1 {
        return Err(E::LimitExceeded);
    }
    meter.work(
        bytes
            .len()
            .checked_add(domain.len() + 1)
            .ok_or(E::Overflow)?,
    )?;
    blake3_raw(
        &CanonicalDigestDomain::parse(domain).map_err(|_| E::CanonicalEncoding)?,
        bytes,
        DigestInputLimit::new(cap).map_err(|_| E::CanonicalEncoding)?,
    )
    .map_err(|_| E::CanonicalEncoding)
}
pub(super) fn profile_identity(
    raw: &ValidatedMaterializerProfile,
    limits: JatsLimits,
) -> Result<Blake3Digest32, E> {
    let mut meter = Meter {
        limits: JatsOperationLimits {
            parser: MAX_JATS_LIMITS,
            canonical_preimage_bytes: MAX_JATS_PREIMAGE_BYTES,
            total_canonical_bytes: MAX_JATS_TOTAL_CANONICAL_BYTES,
        },
        operation: None,
        work: 0,
        owned: 0,
        canonical: 0,
    };
    let source = definition(DEFINITIONS.as_bytes(), false, &mut meter)?;
    let fixture = definition(include_bytes!("fixtures.rs"), true, &mut meter)?;
    let copied = DONOR.len() + POLICY.len() + OPTIONS.len();
    frame(
        &mut meter,
        "eliot/cbor/jats-profile/v1",
        8,
        copied,
        8,
        |i, cx| match i {
            0 => {
                row!(cx; V::U64(1), cx.text("jats-publishing14-subset@1")?, cx.text("eliot.cbor.length-first.v1")?)
            }
            1 => cx.bytes(raw.id().as_bytes()),
            2 => cx.text(DONOR),
            3 => cx.text(POLICY),
            4 => cx.text(OPTIONS),
            5 => {
                let mut values: [V; 19] = std::array::from_fn(|_| V::Null);
                for (slot, value) in values.iter_mut().zip(limits.dimensions()) {
                    *slot = num(value)?;
                }
                cx.array(values)
            }
            6 => {
                let mut values: [V; 19] = std::array::from_fn(|_| V::Null);
                for (slot, value) in values.iter_mut().zip(MAX_JATS_LIMITS.dimensions()) {
                    *slot = num(value)?;
                }
                cx.array(values)
            }
            7 => row!(cx; cx.bytes(source.as_bytes())?, cx.bytes(fixture.as_bytes())?, V::U64(1)),
            _ => Err(E::CanonicalEncoding),
        },
    )
}
pub(super) fn inventory_identity(
    out: &Inventory,
    meter: &mut Meter<'_>,
) -> Result<Blake3Digest32, E> {
    const CHUNK: usize = 256;
    let count = out.count()?;
    meter.work(count)?;
    let mut copied = 0_usize;
    for value in &out.attributes {
        meter.poll()?;
        copied = copied.checked_add(value.value.len()).ok_or(E::Overflow)?;
    }
    for value in &out.texts {
        meter.poll()?;
        copied = copied.checked_add(value.value.len()).ok_or(E::Overflow)?;
    }
    let records = count.checked_add(3).ok_or(E::Overflow)?;
    let chunks = records.div_ceil(CHUNK);
    let reservation = records.checked_add(chunks + 1).ok_or(E::Overflow)?;
    // Reserve every inner row's construction before the shared stream's cap.
    frame(
        meter,
        "eliot/cbor/jats-inventory/v1",
        chunks + 1,
        copied,
        reservation,
        |chunk, cx| {
            if chunk == 0 {
                return row!(cx; V::U64(1), num(records)?, num(CHUNK)?, V::U64(1));
            }
            let first = (chunk - 1).checked_mul(CHUNK).ok_or(E::Overflow)?;
            let last = first.checked_add(CHUNK).ok_or(E::Overflow)?.min(records);
            let mut values = Vec::new();
            cx.units(last - first + 1)?;
            cx.meter.own(
                (last - first)
                    .checked_mul(size_of::<V>())
                    .ok_or(E::Overflow)?,
            )?;
            values
                .try_reserve_exact(last - first)
                .map_err(|_| E::AllocationFailed)?;
            for index in first..last {
                cx.meter.poll()?;
                values.push(inventory_row(out, index, cx)?);
            }
            BoundedList::new(values)
                .map(V::Array)
                .map_err(|_| E::CanonicalEncoding)
        },
    )
}
fn inventory_row(out: &Inventory, index: usize, cx: &mut Rows<'_, '_>) -> Result<V, E> {
    if index == 0 {
        return row!(cx; V::U64(1), V::U64(1), cx.optional_span(out.declaration)?);
    }
    if index == 1 {
        return row!(cx; num(out.events)?, num(out.elements.len())?, num(out.attributes.len())?,
            num(out.texts.len())?, num(out.comments.len())?, num(out.mapped)?, num(out.namespaces)?);
    }
    if index == 2 {
        return row!(cx; cx.text("mapped-whole-event")?, cx.text("raw-exact-XML")?, V::U64(1));
    }
    let mut i = index - 3;
    if let Some(value) = out.elements.get(i) {
        return row!(cx; V::U64(0), num(i)?, cx.text(value.kind.name())?, optional(value.parent)?,
                num(value.event)?, num(value.close_event)?, cx.span(value.opening)?,
                cx.optional_span(value.closing)?, cx.span(value.span)?);
    }
    i -= out.elements.len();
    if let Some(value) = out.attributes.get(i) {
        return row!(cx; V::U64(1), num(i)?, num(value.owner)?, cx.text(value.kind.name())?,
                cx.span(value.span)?, cx.text(&value.value)?, V::U64(2), V::U64(1));
    }
    i -= out.attributes.len();
    if let Some(value) = out.texts.get(i) {
        let kind = match value.kind {
            JatsTextKind::Text => 0,
            JatsTextKind::Cdata => 1,
            JatsTextKind::Reference => 2,
        };
        return row!(cx; V::U64(2), num(i)?, num(value.owner)?, num(value.event)?, V::U64(kind),
                cx.span(value.span)?, cx.text(&value.value)?, V::U64(2), V::U64(1));
    }
    i -= out.texts.len();
    if let Some(value) = out.comments.get(i) {
        return row!(cx; V::U64(3), num(i)?, optional(value.owner)?, num(value.event)?, cx.span(value.span)?);
    }
    Err(E::CanonicalEncoding)
}
pub(super) fn product_identity(
    raw: &MaterializationProduct,
    request: &ValidatedMaterializationRequest,
    profile: &JatsProfile,
    inventory: Blake3Digest32,
    meter: &mut Meter<'_>,
) -> Result<Blake3Digest32, E> {
    let copied = request
        .source_id()
        .as_str()
        .len()
        .checked_add(request.residency().as_str().len())
        .ok_or(E::Overflow)?;
    frame(
        meter,
        "eliot/cbor/jats-product/v1",
        7,
        copied,
        7,
        |index, cx| match index {
            0 => row!(cx; V::U64(1), cx.text("jats-materialization@1")?, V::U64(1)),
            1 => {
                row!(cx; cx.bytes(profile.id().as_bytes())?, cx.bytes(profile.raw_profile_id().as_bytes())?)
            }
            2 => {
                row!(cx; cx.text(request.source_id().as_str())?, V::U64(request.revision().get()), cx.text(request.residency().as_str())?)
            }
            3 => {
                row!(cx; cx.bytes(raw.input_digest().as_bytes())?, cx.bytes(raw.canonical_digest().as_bytes())?, V::U64(request.byte_count()))
            }
            4 => {
                row!(cx; cx.bytes(raw.representation_id().as_bytes())?, cx.bytes(raw.coordinate_digest().as_bytes())?, cx.bytes(raw.loss_digest().as_bytes())?)
            }
            5 => cx.bytes(inventory.as_bytes()),
            6 => row!(cx; V::U64(1), V::U64(2), V::U64(1), V::U64(1)),
            _ => Err(E::CanonicalEncoding),
        },
    )
}
