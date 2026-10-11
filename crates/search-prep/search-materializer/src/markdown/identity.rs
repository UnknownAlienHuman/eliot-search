//! Shared canonical commitments for the closed inert Markdown profile.
//! Only the shared canonical encoder and reviewed BLAKE3 primitives compute identities.

use core::mem::size_of;
use std::time::Instant;

use super::model::{
    HARD_LIMITS, InlineKind, InlineMetadataStatus, InlineRecord, MappedText, MarkdownDocument,
    MarkdownError, MarkdownKind, MarkdownLimits, MarkdownNode, ObservedLinkKind, RawSpan,
    ReferenceMetadataStatus,
};
use crate::product::{MaterializationProduct, verify_materialization};
use crate::profile::{SourceEncoding, SourceKind, ValidatedMaterializerProfile};
use crate::request::{CancellationToken, ValidatedMaterializationRequest};
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue, ContractError, ContractErrorKind, DigestInputLimit,
    blake3_canonical_array_with_usage, blake3_raw,
};

pub(super) const PROFILE_PREIMAGE_CAP: usize = 16_384;
pub(super) const SOURCE_PREIMAGE_CAP: usize = 131_072;
pub(super) const INVENTORY_PREIMAGE_CAP: usize = 2_097_152;
pub(super) const PRODUCT_PREIMAGE_CAP: usize = 16_384;
pub(super) const WHOLE_PREIMAGE_CAP: usize = 4_194_304;
pub(super) const FIXTURE_PREIMAGE_CAP: usize = 16_384;

const PROFILE_DOMAIN: &str = "eliot/cbor/markdown-profile/v1";
const NODES_DOMAIN: &str = "eliot/cbor/markdown-nodes/v1";
const INLINE_DOMAIN: &str = "eliot/cbor/markdown-inline/v1";
const PRODUCT_DOMAIN: &str = "eliot/cbor/markdown-product/v1";
const SOURCE_DOMAIN: &str = "eliot/raw/markdown-algorithm-source/v1";
const FIXTURE_DOMAIN: &str = "eliot/raw/markdown-fixture-definition/v1";

/// Implement on the wrapper's SAME operation meter; no reset at replay/hash boundaries.
pub(super) trait IdentityBudget {
    fn remaining_work(&self) -> usize;
    fn charge_work(&mut self, amount: usize) -> Result<(), MarkdownError>;
    /// Storage only: row construction work is prepaid independently below.
    fn hold_transient(&mut self, bytes: usize) -> Result<(), MarkdownError>;
    /// Called only after all owned values in the preceding row have been dropped.
    fn release_row(&mut self);
    fn remaining_canonical_bytes(&self) -> usize;
    fn charge_canonical_bytes(&mut self, actual: usize) -> Result<(), MarkdownError>;
}

// Pure construction has fixed work/storage bounds, without a synthetic deadline.
// Source/fixture RAW preimages have their own checked caps; the fixed profile
// rows are built once before hashing under their complete-preimage ceiling.
struct SetupBudget {
    work: usize,
    storage: usize,
    canonical: usize,
}

impl IdentityBudget for SetupBudget {
    fn remaining_work(&self) -> usize {
        512 * 1024 - self.work
    }
    fn charge_work(&mut self, amount: usize) -> Result<(), MarkdownError> {
        if amount > self.remaining_work() {
            return Err(MarkdownError::WorkLimit);
        }
        self.work += amount;
        Ok(())
    }
    fn hold_transient(&mut self, bytes: usize) -> Result<(), MarkdownError> {
        if bytes > PROFILE_PREIMAGE_CAP - self.storage {
            return Err(MarkdownError::StorageLimit);
        }
        self.storage += bytes;
        Ok(())
    }
    fn release_row(&mut self) {
        self.storage = 0;
    }
    fn remaining_canonical_bytes(&self) -> usize {
        PROFILE_PREIMAGE_CAP - self.canonical
    }
    fn charge_canonical_bytes(&mut self, actual: usize) -> Result<(), MarkdownError> {
        if actual > self.remaining_canonical_bytes() {
            return Err(MarkdownError::CanonicalEncoding);
        }
        self.canonical += actual;
        Ok(())
    }
}
struct Context<'a, 'b, B> {
    /// None is pure profile setup; Some always preserves the operation originals.
    operation: Option<(CancellationToken<'a>, Instant)>,
    budget: &'b mut B,
    /// Already debited construction units, NEVER an encoded-byte estimate.
    prepaid_construction: Option<usize>,
}

impl<B: IdentityBudget> Context<'_, '_, B> {
    fn checkpoint(&self) -> Result<(), MarkdownError> {
        if let Some((cancel, deadline)) = self.operation {
            super::parse::checkpoint(cancel, deadline)?;
        }
        Ok(())
    }

    fn construction_work(&mut self, amount: usize) -> Result<(), MarkdownError> {
        if let Some(remaining) = &mut self.prepaid_construction {
            *remaining = remaining
                .checked_sub(amount)
                .ok_or(MarkdownError::WorkLimit)?;
            Ok(())
        } else {
            self.budget.charge_work(amount)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MarkdownCommitments {
    nodes: Blake3Digest32,
    inline: Blake3Digest32,
    product: Blake3Digest32,
    complete_preimage_bytes: usize,
}

impl MarkdownCommitments {
    pub(super) const fn nodes(&self) -> Blake3Digest32 {
        self.nodes
    }
    pub(super) const fn inline(&self) -> Blake3Digest32 {
        self.inline
    }
    pub(super) const fn product(&self) -> Blake3Digest32 {
        self.product
    }
    pub(super) const fn complete_preimage_bytes(&self) -> usize {
        self.complete_preimage_bytes
    }
}

macro_rules! row {
    ($cx:expr; $($value:expr),* $(,)?) => {{
        let fields = [$($value),*];
        array($cx, fields)
    }};
}

fn num(value: usize) -> Result<CanonicalValue, MarkdownError> {
    u64::try_from(value)
        .map(CanonicalValue::U64)
        .map_err(|_| MarkdownError::ArithmeticOverflow)
}

fn optional_index(value: Option<usize>) -> Result<CanonicalValue, MarkdownError> {
    value
        .map(num)
        .transpose()
        .map(|value| value.unwrap_or(CanonicalValue::Null))
}

fn array<const N: usize, B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    fields: [CanonicalValue; N],
) -> Result<CanonicalValue, MarkdownError> {
    if N > 12 {
        return Err(MarkdownError::CanonicalEncoding);
    }
    let slots = N
        .checked_mul(size_of::<CanonicalValue>())
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    cx.budget.hold_transient(slots)?;
    cx.construction_work(N + 1)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(N)
        .map_err(|_| MarkdownError::AllocationFailed)?;
    values.extend(fields);
    BoundedList::new(values)
        .map(CanonicalValue::Array)
        .map_err(|_| MarkdownError::CanonicalEncoding)
}

fn text<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    value: &str,
) -> Result<CanonicalValue, MarkdownError> {
    cx.construction_work(value.len())?;
    cx.budget.hold_transient(value.len())?;
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| MarkdownError::AllocationFailed)?;
    copy.push_str(value);
    CanonicalText::new(copy)
        .map(CanonicalValue::Text)
        .map_err(|_| MarkdownError::CanonicalEncoding)
}

fn bytes<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    value: &[u8],
) -> Result<CanonicalValue, MarkdownError> {
    cx.construction_work(value.len())?;
    cx.budget.hold_transient(value.len())?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| MarkdownError::AllocationFailed)?;
    copy.extend_from_slice(value);
    BoundedBytes::new(copy)
        .map(CanonicalValue::Bytes)
        .map_err(|_| MarkdownError::CanonicalEncoding)
}

fn digest<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    value: Blake3Digest32,
) -> Result<CanonicalValue, MarkdownError> {
    bytes(cx, value.as_bytes())
}

fn mapped<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    value: Option<&MappedText>,
    metadata_cap: usize,
) -> Result<CanonicalValue, MarkdownError> {
    let Some(value) = value else {
        return Ok(CanonicalValue::Null);
    };
    if value.value.len() > metadata_cap {
        return Err(MarkdownError::MetadataLimit);
    }
    // 2 = metadataMapped; 1 = whole donor event provenance, never a decoded subrange.
    row!(cx; CanonicalValue::U64(2), CanonicalValue::U64(1), text(cx, &value.value)?)
}

const fn encoding_error(_: ContractError) -> MarkdownError {
    MarkdownError::CanonicalEncoding
}

/// Count+1 reads preserve checkpoints even for an empty array and terminal None.
fn stream<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    domain_name: &str,
    count: usize,
    cap: usize,
    construction_work: usize,
    mut make_row: impl FnMut(usize, &mut Context<'_, '_, B>) -> Result<CanonicalValue, MarkdownError>,
) -> Result<(Blake3Digest32, usize), MarkdownError> {
    cx.checkpoint()?;
    // Debit ACTUAL row construction units first. Subsequent copies/slot work
    // consume only this local credit, leaving encoder work stable throughout.
    cx.budget.charge_work(construction_work)?;
    cx.prepaid_construction = Some(construction_work);
    let limit = cap
        .min(cx.budget.remaining_canonical_bytes())
        .min(cx.budget.remaining_work());
    if cx.budget.remaining_work() == 0 {
        return Err(MarkdownError::WorkLimit);
    }
    if limit == 0 {
        return Err(MarkdownError::CanonicalEncoding);
    }
    let domain = CanonicalDigestDomain::parse(domain_name).map_err(encoding_error)?;
    let limit_value = DigestInputLimit::new(limit).map_err(encoding_error)?;
    let mut index = 0;
    let mut owner_error = None;
    let output = {
        let rows = std::iter::from_fn(|| {
            cx.budget.release_row();
            let row = (|| {
                cx.checkpoint()?;
                if index == count {
                    return Ok(None);
                }
                cx.construction_work(1)?;
                let value = make_row(index, cx)?;
                cx.checkpoint()?;
                index += 1;
                Ok(Some(value))
            })();
            match row {
                Ok(value) => value.map(Ok),
                Err(error) => {
                    owner_error = Some(error);
                    Some(Err(ContractError::new(
                        ContractErrorKind::MalformedPayload,
                        "markdown_row",
                    )))
                }
            }
        });
        blake3_canonical_array_with_usage(&domain, count, rows, limit_value)
    };
    // Shared helper has returned and dropped its last row/iterator on ALL paths.
    cx.budget.release_row();
    let unconsumed = cx.prepaid_construction.take();
    let output = match output {
        Ok(output) if owner_error.is_none() => output,
        result => {
            return Err(owner_error.unwrap_or_else(|| match result {
                Err(error) => encoding_error(error),
                Ok(_) => MarkdownError::CanonicalEncoding,
            }));
        }
    };
    if unconsumed != Some(0) {
        return Err(MarkdownError::CanonicalEncoding);
    }
    let actual = output.complete_preimage_bytes();
    cx.budget.charge_work(actual)?;
    cx.budget.charge_canonical_bytes(actual)?;
    cx.checkpoint()?;
    Ok((output.digest(), actual))
}

fn raw_definition<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    domain_name: &str,
    definition: &[u8],
    cap: usize,
) -> Result<Blake3Digest32, MarkdownError> {
    cx.checkpoint()?;
    // This is the shared documented RAW framing length, not a CBOR sizing formula.
    let complete = domain_name
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_add(definition.len()))
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    if complete > cap {
        return Err(MarkdownError::CanonicalEncoding);
    }
    cx.budget.charge_work(complete)?;
    let domain = CanonicalDigestDomain::parse(domain_name).map_err(encoding_error)?;
    let output = blake3_raw(
        &domain,
        definition,
        DigestInputLimit::new(cap).map_err(encoding_error)?,
    )
    .map_err(encoding_error)?;
    cx.checkpoint()?;
    Ok(output)
}

fn limit_row<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    limits: MarkdownLimits,
) -> Result<CanonicalValue, MarkdownError> {
    row!(cx; num(limits.input_bytes)?, num(limits.next_attempts)?, num(limits.stack_depth)?,
        num(limits.nodes)?, num(limits.inline_records)?, num(limits.metadata_bytes_each)?,
        num(limits.metadata_bytes_total)?, num(limits.owned_bytes)?, num(limits.work_steps)?)
}

fn donor_row<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    index: usize,
) -> Result<CanonicalValue, MarkdownError> {
    let (name, version, checksum) = match index {
        0 => (
            "pulldown-cmark",
            "0.13.1",
            "83c41efbf8f90ac44de7f3a868f0867851d261b56291732d0cbf7cceaaeb55a6",
        ),
        1 => (
            "bitflags",
            "2.13.2",
            "3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06",
        ),
        2 => (
            "memchr",
            "2.8.3",
            "cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98",
        ),
        3 => (
            "unicase",
            "2.10.0",
            "357cc3acc6a036009fd6c973ed009037c732d60d0b4f6c673e9041497482a28f",
        ),
        _ => return Err(MarkdownError::CanonicalEncoding),
    };
    let features = if index == 2 {
        row!(cx; text(cx, "alloc")?, text(cx, "default")?, text(cx, "std")?)?
    } else {
        row!(cx;)?
    };
    row!(cx; text(cx, name)?, text(cx, version)?, text(cx, checksum)?, features)
}

pub(super) fn digest_profile(
    baseline: &ValidatedMaterializerProfile,
    limits: MarkdownLimits,
) -> Result<Blake3Digest32, MarkdownError> {
    let mut pure_setup_budget = SetupBudget {
        work: 0,
        storage: 0,
        canonical: 0,
    };
    limits.validate()?;
    super::profile::validate_raw_profile(baseline)?;
    // Caller supplies a separate fixed pure setup budget, not an operation deadline.
    let mut cx = Context {
        operation: None,
        budget: &mut pure_setup_budget,
        prepaid_construction: None,
    };
    // EXACT source bytes; hashing include_bytes!("identity.rs") is not output recursion.
    let parse = raw_definition(
        &mut cx,
        SOURCE_DOMAIN,
        include_bytes!("parse.rs"),
        SOURCE_PREIMAGE_CAP,
    )?;
    let model = raw_definition(
        &mut cx,
        SOURCE_DOMAIN,
        include_bytes!("model.rs"),
        SOURCE_PREIMAGE_CAP,
    )?;
    let identity = raw_definition(
        &mut cx,
        SOURCE_DOMAIN,
        include_bytes!("identity.rs"),
        SOURCE_PREIMAGE_CAP,
    )?;
    let fixtures = raw_definition(
        &mut cx,
        FIXTURE_DOMAIN,
        include_bytes!("fixtures.rs"),
        FIXTURE_PREIMAGE_CAP,
    )?;
    // Only the fixed 11-row PROFILE is constructed once before its hash. No
    // document/node/inline inventory is collected or duplicated this way.
    let slots = 11_usize
        .checked_mul(size_of::<CanonicalValue>())
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    cx.budget.hold_transient(slots)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(11)
        .map_err(|_| MarkdownError::AllocationFailed)?;
    let built = (|| {
        for index in 0..11 {
            cx.construction_work(1)?;
            let value = match index {
                0 => {
                    row!(&mut cx; CanonicalValue::U64(1), text(&mut cx, "markdown-raw@1")?, text(&mut cx, "eliot.cbor.length-first.v1")?)
                }
                1 => bytes(&mut cx, baseline.id().as_bytes()),
                2 => {
                    row!(&mut cx; donor_row(&mut cx, 0)?, donor_row(&mut cx, 1)?, donor_row(&mut cx, 2)?, donor_row(&mut cx, 3)?)
                }
                3 => Ok(CanonicalValue::U64(0)), // Options::empty(); no optional dialect.
                4 => {
                    row!(&mut cx; text(&mut cx, "flat-donor-preorder")?, text(&mut cx, "transparent-lists")?,
                text(&mut cx, "global-heading-stack")?, text(&mut cx, "nearest-physical-parent")?,
                text(&mut cx, "stored-node-inline-order")?, text(&mut cx, "raw-root-full-coverage")?,
                text(&mut cx, "overlapping-observations-not-coverage")?)
                }
                5 => {
                    row!(&mut cx; text(&mut cx, "strict-nonempty-utf8-bom-reject")?, text(&mut cx, "exact-cr-lf-no-normalization")?,
                text(&mut cx, "no-render-fetch-callback")?, text(&mut cx, "rawExactBytes")?, text(&mut cx, "metadataMapped")?,
                text(&mut cx, "wholeEvent")?, text(&mut cx, "referencePotentiallyIncomplete")?,
                text(&mut cx, "ObservedSpansAndSelectedValues")?)
                }
                6 => row!(&mut cx; limit_row(&mut cx, HARD_LIMITS)?, limit_row(&mut cx, limits)?),
                7 => {
                    row!(&mut cx; text(&mut cx, "markdown-logical-work@1")?, text(&mut cx, "attempt-includes-terminal-none")?,
                text(&mut cx, "yielded-events-separate")?, text(&mut cx, "records-frames-selected-strings-plus-row-transients")?,
                text(&mut cx, "copies-and-shared-complete-preimage-byte-work")?,
                text(&mut cx, "original-inventory-held-during-replay")?,
                text(&mut cx, "excludes-borrowed-raw-existing-maps-spare-capacity-donor-internals")?,
                text(&mut cx, "synchronous-donor-not-hard-preemptible")?,
                text(&mut cx, "prepaid-construction-from-records-metadata-and-id-lengths")?)
                }
                8 => {
                    row!(&mut cx; num(PROFILE_PREIMAGE_CAP)?, num(INVENTORY_PREIMAGE_CAP)?, num(PRODUCT_PREIMAGE_CAP)?,
                num(WHOLE_PREIMAGE_CAP)?, num(FIXTURE_PREIMAGE_CAP)?, num(SOURCE_PREIMAGE_CAP)?)
                }
                9 => {
                    row!(&mut cx; digest(&mut cx, parse)?, digest(&mut cx, model)?, digest(&mut cx, identity)?)
                }
                10 => digest(&mut cx, fixtures),
                _ => Err(MarkdownError::CanonicalEncoding),
            }?;
            rows.push(value);
        }
        Ok::<(), MarkdownError>(())
    })();
    if let Err(error) = built {
        drop(rows);
        cx.budget.release_row();
        return Err(error);
    }
    let output = (|| {
        let domain = CanonicalDigestDomain::parse(PROFILE_DOMAIN).map_err(encoding_error)?;
        let limit = PROFILE_PREIMAGE_CAP.min(cx.budget.remaining_work());
        let limit = DigestInputLimit::new(limit).map_err(encoding_error)?;
        blake3_canonical_array_with_usage(&domain, 11, rows.into_iter().map(Ok), limit)
            .map_err(encoding_error)
    })();
    cx.budget.release_row();
    let output = output?;
    cx.budget.charge_work(output.complete_preimage_bytes())?;
    let fingerprint = output.digest();
    cx.budget
        .charge_canonical_bytes(output.complete_preimage_bytes())?;
    cx.checkpoint()?;
    Ok(fingerprint)
}

fn check_span(source: &str, span: RawSpan, nonempty: bool) -> Result<(), MarkdownError> {
    if span.start > span.end
        || (nonempty && span.start == span.end)
        || source.get(span.start..span.end).is_none()
    {
        return Err(MarkdownError::InvalidRange);
    }
    Ok(())
}

fn node_row<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    document: &MarkdownDocument<'_>,
    index: usize,
    node: &MarkdownNode,
    metadata_cap: usize,
) -> Result<CanonicalValue, MarkdownError> {
    check_span(document.source, node.span, true)?;
    if node.ordinal != index {
        return Err(MarkdownError::InvalidStructure);
    }
    if index == 0 {
        if node.kind != MarkdownKind::Root
            || node.physical_parent.is_some()
            || node.heading_section.is_some()
            || node.span.start != 0
            || node.span.end != document.source.len()
        {
            return Err(MarkdownError::InvalidStructure);
        }
    } else {
        let parent = node
            .physical_parent
            .ok_or(MarkdownError::InvalidStructure)?;
        if parent >= index
            || !document.nodes[parent].span.contains(node.span)
            || node.span.start < document.nodes[index - 1].span.start
        {
            return Err(MarkdownError::InvalidStructure);
        }
    }
    if let Some(section) = node.heading_section
        && (section > index
            || !matches!(document.nodes[section].kind, MarkdownKind::Heading { .. }))
    {
        return Err(MarkdownError::InvalidStructure);
    }
    let (kind, level, heading_parent) = match node.kind {
        MarkdownKind::Root if index == 0 => (0, None, None),
        MarkdownKind::Heading {
            level,
            parent_section,
        } => {
            if !(1..=6).contains(&level)
                || node.heading_section != Some(index)
                || parent_section.is_some_and(|p| {
                    p >= index || !matches!(document.nodes[p].kind, MarkdownKind::Heading { .. })
                })
            {
                return Err(MarkdownError::InvalidStructure);
            }
            (1, Some(usize::from(level)), parent_section)
        }
        MarkdownKind::Paragraph => (2, None, None),
        MarkdownKind::Item => (3, None, None),
        MarkdownKind::Quote => (4, None, None),
        MarkdownKind::FencedCode => (5, None, None),
        MarkdownKind::IndentedCode => (6, None, None),
        MarkdownKind::ThematicBreak => (7, None, None),
        MarkdownKind::RawHtmlBlock => (8, None, None),
        MarkdownKind::Root => return Err(MarkdownError::InvalidStructure),
    };
    if node.code_info.is_some() != (kind == 5) {
        return Err(MarkdownError::InvalidStructure);
    }
    row!(cx; num(index)?, CanonicalValue::U64(kind), num(node.span.start)?, num(node.span.end)?,
        optional_index(node.physical_parent)?, optional_index(node.heading_section)?, optional_index(level)?,
        optional_index(heading_parent)?, mapped(cx, node.code_info.as_ref(), metadata_cap)?)
}

const fn link_tag(kind: ObservedLinkKind) -> u64 {
    match kind {
        ObservedLinkKind::Inline => 0,
        ObservedLinkKind::Reference => 1,
        ObservedLinkKind::Collapsed => 2,
        ObservedLinkKind::Shortcut => 3,
        ObservedLinkKind::Autolink => 4,
        ObservedLinkKind::Email => 5,
    }
}

fn inline_row<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    document: &MarkdownDocument<'_>,
    index: usize,
    record: &InlineRecord,
    metadata_cap: usize,
) -> Result<CanonicalValue, MarkdownError> {
    check_span(document.source, record.span, false)?;
    if !document
        .nodes
        .get(record.owner)
        .is_some_and(|node| node.span.contains(record.span))
    {
        return Err(MarkdownError::InvalidStructure);
    }
    let (kind, link) = match record.kind {
        InlineKind::Emphasis => (0, None),
        InlineKind::Strong => (1, None),
        InlineKind::Code => (2, None),
        InlineKind::Text => (3, None),
        InlineKind::SoftBreak => (4, None),
        InlineKind::HardBreak => (5, None),
        InlineKind::Html => (6, None),
        InlineKind::Link(link) => (7, Some(link_tag(link))),
        InlineKind::Image(link) => (8, Some(link_tag(link))),
    };
    if record.text.is_some() != (kind == 2)
        || record.destination.is_some() != link.is_some()
        || record.title.is_some() != link.is_some()
    {
        return Err(MarkdownError::InvalidStructure);
    }
    row!(cx; num(index)?, num(record.owner)?, CanonicalValue::U64(kind),
        link.map_or(CanonicalValue::Null, CanonicalValue::U64), num(record.span.start)?, num(record.span.end)?,
        mapped(cx, record.text.as_ref(), metadata_cap)?, mapped(cx, record.destination.as_ref(), metadata_cap)?,
        mapped(cx, record.title.as_ref(), metadata_cap)?)
}

fn add_construction(total: usize, amount: usize) -> Result<usize, MarkdownError> {
    total
        .checked_add(amount)
        .ok_or(MarkdownError::ArithmeticOverflow)
}

fn metadata_construction(value: Option<&MappedText>, cap: usize) -> Result<usize, MarkdownError> {
    let Some(value) = value else {
        return Ok(0);
    };
    if value.value.len() > cap {
        return Err(MarkdownError::MetadataLimit);
    }
    // Three mapped-value slots plus one array operation, and copied text bytes.
    add_construction(4, value.value.len())
}

fn nodes_construction<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    document: &MarkdownDocument<'_>,
    cap: usize,
) -> Result<usize, MarkdownError> {
    let mut total = 0;
    for node in &document.nodes {
        cx.checkpoint()?;
        cx.budget.charge_work(1)?; // bounded preflight record inspection, paid separately.
        // One row operation + nine array slots + one array operation.
        total = add_construction(total, 11)?;
        total = add_construction(total, metadata_construction(node.code_info.as_ref(), cap)?)?;
    }
    cx.checkpoint()?;
    Ok(total)
}

fn inline_construction<B: IdentityBudget>(
    cx: &mut Context<'_, '_, B>,
    document: &MarkdownDocument<'_>,
    cap: usize,
) -> Result<usize, MarkdownError> {
    let mut total = 0;
    for record in &document.inline_records {
        cx.checkpoint()?;
        cx.budget.charge_work(1)?;
        total = add_construction(total, 11)?;
        for value in [
            record.text.as_ref(),
            record.destination.as_ref(),
            record.title.as_ref(),
        ] {
            total = add_construction(total, metadata_construction(value, cap)?)?;
        }
    }
    cx.checkpoint()?;
    Ok(total)
}

fn product_construction(request: &ValidatedMaterializationRequest) -> Result<usize, MarkdownError> {
    let mut total = 12; // Exact row operations, including scalar rows.
    for fields in [2, 2, 3, 3, 2, 2, 4] {
        total = add_construction(total, fields + 1)?;
    }
    total = add_construction(total, 8 * 32)?; // Eight existing fixed digest byte copies.
    total = add_construction(total, "markdown-product@1".len())?;
    total = add_construction(total, request.source_id().as_str().len())?;
    add_construction(total, request.residency().as_str().len())
}

/// Private wrapper calls this only for its complete parser-produced document.
/// The wrapper accounts baseline verifier work on its existing materialization meter.
pub(super) fn commit_markdown(
    document: &MarkdownDocument<'_>,
    baseline: &MaterializationProduct,
    request: &ValidatedMaterializationRequest,
    profile: &super::profile::MarkdownProfile,
    operation_limits: super::profile::MarkdownOperationLimits,
    operation: (CancellationToken<'_>, Instant),
    budget: &mut impl IdentityBudget,
) -> Result<MarkdownCommitments, MarkdownError> {
    operation_limits.validate()?;
    let per_call_preimage_limit = operation_limits.canonical_preimage_bytes;
    let whole_call_preimage_limit = operation_limits.total_canonical_bytes;
    let mut cx = Context {
        operation: Some(operation),
        budget,
        prepaid_construction: None,
    };
    cx.checkpoint()?;
    super::profile::validate_raw_profile(request.profile())?;
    if profile.raw_profile_id() != request.profile().id()
        || request.declared_kind() != SourceKind::Text
        || request.declared_encoding() != SourceEncoding::Utf8
    {
        return Err(MarkdownError::ProfileMismatch);
    }
    verify_materialization(baseline, request, request.profile())
        .map_err(MarkdownError::Materialization)?;
    cx.checkpoint()?;
    cx.budget.charge_work(document.source.len())?; // exact byte equality scan's admitted work.
    if document.source != baseline.canonical_text()
        || baseline.encoding() != SourceEncoding::Utf8
        || baseline.input_digest() != baseline.canonical_digest()
        || request.byte_count()
            != u64::try_from(document.source.len())
                .map_err(|_| MarkdownError::ArithmeticOverflow)?
    {
        return Err(MarkdownError::ProductMismatch);
    }
    let limits = profile.limits();
    if document.source.is_empty()
        || document.source.len() > limits.input_bytes
        || document.nodes.is_empty()
        || document.nodes.len() > limits.nodes
        || document.inline_records.len() > limits.inline_records
        || document.usage.next_attempts > limits.next_attempts
        || document.usage.steps > limits.work_steps
        || document.usage.metadata_bytes > limits.metadata_bytes_total
        || document.usage.owned_peak > limits.owned_bytes
    {
        return Err(MarkdownError::InvalidLimits);
    }
    if document.reference_metadata != ReferenceMetadataStatus::PotentiallyIncomplete
        || document.inline_metadata != InlineMetadataStatus::ObservedSpansAndSelectedValues
    {
        return Err(MarkdownError::ProductMismatch);
    }
    let nodes_work = nodes_construction(&mut cx, document, limits.metadata_bytes_each)?;
    let (nodes, nodes_bytes) = stream(
        &mut cx,
        NODES_DOMAIN,
        document.nodes.len(),
        INVENTORY_PREIMAGE_CAP
            .min(per_call_preimage_limit)
            .min(whole_call_preimage_limit),
        nodes_work,
        |index, cx| {
            node_row(
                cx,
                document,
                index,
                &document.nodes[index],
                limits.metadata_bytes_each,
            )
        },
    )?;
    let remaining = whole_call_preimage_limit
        .checked_sub(nodes_bytes)
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    let inline_work = inline_construction(&mut cx, document, limits.metadata_bytes_each)?;
    let (inline, inline_bytes) = stream(
        &mut cx,
        INLINE_DOMAIN,
        document.inline_records.len(),
        INVENTORY_PREIMAGE_CAP
            .min(per_call_preimage_limit)
            .min(remaining),
        inline_work,
        |index, cx| {
            inline_row(
                cx,
                document,
                index,
                &document.inline_records[index],
                limits.metadata_bytes_each,
            )
        },
    )?;
    let used = nodes_bytes
        .checked_add(inline_bytes)
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    let remaining = whole_call_preimage_limit
        .checked_sub(used)
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    cx.budget.charge_work(1)?;
    let product_work = product_construction(request)?;
    let (product, product_bytes) = stream(
        &mut cx,
        PRODUCT_DOMAIN,
        12,
        PRODUCT_PREIMAGE_CAP
            .min(per_call_preimage_limit)
            .min(remaining),
        product_work,
        |index, cx| match index {
            0 => row!(cx; CanonicalValue::U64(1), text(cx, "markdown-product@1")?),
            1 => digest(cx, profile.id()),
            2 => digest(cx, baseline.representation_id()),
            3 => text(cx, request.source_id().as_str()),
            4 => Ok(CanonicalValue::U64(request.revision().get())),
            5 => text(cx, request.residency().as_str()),
            6 => {
                row!(cx; digest(cx, baseline.input_digest())?, CanonicalValue::U64(request.byte_count()))
            }
            7 => {
                row!(cx; digest(cx, baseline.canonical_digest())?, num(document.source.len())?, CanonicalValue::U64(1))
            }
            8 => {
                row!(cx; digest(cx, baseline.coordinate_digest())?, digest(cx, baseline.loss_digest())?, CanonicalValue::U64(1))
            }
            9 => row!(cx; num(document.nodes.len())?, digest(cx, nodes)?),
            10 => row!(cx; num(document.inline_records.len())?, digest(cx, inline)?),
            11 => {
                row!(cx; CanonicalValue::U64(1), CanonicalValue::U64(1), CanonicalValue::U64(1), CanonicalValue::U64(2))
            }
            _ => Err(MarkdownError::CanonicalEncoding),
        },
    )?;
    let used = used
        .checked_add(product_bytes)
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    let output = MarkdownCommitments {
        nodes,
        inline,
        product,
        complete_preimage_bytes: used,
    };
    cx.checkpoint()?; // AFTER output construction: late cancellation yields no product.
    Ok(output)
}
