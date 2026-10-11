//! Owning Markdown replay, unchanged raw v3 and complete structural reconstruction.
use super::budget::{Meter, add, index, mul, number};
use super::codec::{array, body, bytes, encode, node_identity, shape, text};
use super::model::{
    MarkdownInlineObservation, MarkdownManifestBody, MarkdownMappedValue, MarkdownNodeData,
    MarkdownSourceBinding, MarkdownStructuralUnit, MarkdownUnitManifest,
    MarkdownUnitizationError as Error, MarkdownUnitizationUsage, VerifiedMarkdownUnitSet,
};
use super::{MarkdownUnitizationBudget, MarkdownUnitizerProfile};
use crate::{UnitizationBudget, V3RepresentationKind, ValidatedV3UnitizerProfile};
use search_contracts::{
    Blake3Digest32, BoundedList, CanonicalValue, NativeAnchor, OpaqueId, Representation,
    RepresentationId, TextBytesAnchor, UnitId, UnitKind, UnitOccurrence,
};
use search_materializer::api::{
    CancellationToken, MarkdownError, MarkdownInlineKind, MarkdownKind, MarkdownMappedText,
    MarkdownObservedLinkKind, MarkdownOperationLimits, MarkdownProduct, MarkdownProfile,
    MaterializationBudget, MaterializationContext, RevisionReadPort,
    ValidatedMaterializationRequest, ValidatedMaterializerProfile, verify_markdown,
};
use std::collections::BTreeMap;
use std::mem::size_of;

/// Named untrusted borrowed ingress; only these boundaries expose peer types.
pub struct MarkdownUnitizationSource<'a> {
    /// Admission-supplied raw and distinct structural coordinates.
    pub binding: &'a MarkdownSourceBinding,
    /// Immutable Markdown preparation whose full inventory is replayed.
    pub product: &'a MarkdownProduct,
    /// Validated exact retained-source request.
    pub request: &'a ValidatedMaterializationRequest,
    /// Existing owning exact-revision read port, never a source store opened here.
    pub port: &'a dyn RevisionReadPort,
    /// Actual selected raw materializer profile.
    pub raw_profile: &'a ValidatedMaterializerProfile,
    /// Finite raw materializer allowance, independently reserved before replay.
    pub raw_budget: MaterializationBudget,
    /// Actual immutable selected Markdown profile.
    pub markdown_profile: &'a MarkdownProfile,
    /// Actual unchanged raw v3 unitizer profile.
    pub raw_unitizer_profile: &'a ValidatedV3UnitizerProfile,
    /// Lower original replay allowances; no reset from stage-local defaults.
    pub replay_limits: MarkdownOperationLimits,
}

pub(super) fn kind(token: &str) -> Result<(&'static str, UnitKind), Error> {
    match token {
        "root" => Ok(("root", UnitKind::File)),
        "heading" => Ok(("heading", UnitKind::Section)),
        "paragraph" => Ok(("paragraph", UnitKind::Doc)),
        "item" => Ok(("item", UnitKind::Doc)),
        "quote" => Ok(("quote", UnitKind::Doc)),
        "fenced-code" => Ok(("fenced-code", UnitKind::Doc)),
        "indented-code" => Ok(("indented-code", UnitKind::Doc)),
        "thematic-break" => Ok(("thematic-break", UnitKind::Doc)),
        "raw-html-block" => Ok(("raw-html-block", UnitKind::Doc)),
        _ => Err(Error::InvalidStructure),
    }
}
pub(super) fn occurrence(
    id: UnitId,
    representation: RepresentationId,
    ordinal: u64,
    token: &str,
    span: (u64, u64),
    raw: Blake3Digest32,
    identity: Blake3Digest32,
) -> Result<UnitOccurrence, Error> {
    let anchor = NativeAnchor::TextBytes(TextBytesAnchor {
        content_digest: raw,
        byte_start_0: span.0,
        byte_end_exclusive_0: span.1,
    });
    anchor.validate().map_err(|_| Error::InvalidStructure)?;
    Ok(UnitOccurrence {
        unit_id: id,
        representation_id: representation,
        unit_kind: kind(token)?.1,
        ordinal,
        native_anchor: anchor,
        structural_identity: Some(
            OpaqueId::new(format!("md-unit-v4-{identity}")).map_err(|_| Error::Codec)?,
        ),
        configuration_predicate: None,
    })
}
pub(super) fn context(
    manifest: &MarkdownManifestBody,
    meter: &mut Meter<'_>,
) -> Result<Blake3Digest32, Error> {
    meter.rows(5, 512)?;
    let value = array(vec![
        text(manifest.profile.id().as_str())?,
        bytes(manifest.structural.as_bytes())?,
        bytes(manifest.raw.manifest_digest().as_bytes())?,
        bytes(manifest.markdown_product.as_bytes())?,
        text(manifest.residency.as_str())?,
    ])?;
    meter.hash("eliot/cbor/md-unit-context/v4", value)
}
pub(super) fn identity(
    context: Blake3Digest32,
    unit: &MarkdownNodeData,
    meter: &mut Meter<'_>,
) -> Result<Blake3Digest32, Error> {
    meter.rows(2, 32)?;
    let value = array(vec![
        bytes(context.as_bytes())?,
        node_identity(unit, meter)?,
    ])?;
    meter.hash("eliot/cbor/md-unit-occurrence/v4", value)
}
pub(super) fn truncated(digest: Blake3Digest32) -> UnitId {
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    UnitId::from_bytes(bytes)
}
pub(super) fn record(
    ids: &mut BTreeMap<UnitId, Blake3Digest32>,
    id: UnitId,
    digest: Blake3Digest32,
) -> Result<(), Error> {
    match ids.entry(id) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(digest);
        }
        std::collections::btree_map::Entry::Occupied(_) => return Err(Error::IdentityCollision),
    }
    Ok(())
}
fn mapped(
    value: Option<&MarkdownMappedText>,
    metadata: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Option<MarkdownMappedValue>, Error> {
    let Some(value) = value else {
        return Ok(None);
    };
    *metadata = add(*metadata, value.value().len())?;
    if value.value().len() > meter.budget.limits.metadata_bytes_each
        || *metadata > meter.budget.limits.metadata_bytes_total
    {
        return Err(Error::InvalidStructure);
    }
    meter.work(value.value().len())?;
    meter.owned(add(size_of::<MarkdownMappedValue>(), value.value().len())?)?;
    let mut copied = String::new();
    copied
        .try_reserve_exact(value.value().len())
        .map_err(|_| Error::AllocationFailed)?;
    copied.push_str(value.value());
    Ok(Some(MarkdownMappedValue { value: copied }))
}
fn node_kind(kind: MarkdownKind) -> Result<(&'static str, u8, Option<u64>), Error> {
    Ok(match kind {
        MarkdownKind::Root => ("root", 0, None),
        MarkdownKind::Heading {
            level,
            parent_section,
        } => ("heading", level, parent_section.map(number).transpose()?),
        MarkdownKind::Paragraph => ("paragraph", 0, None),
        MarkdownKind::Item => ("item", 0, None),
        MarkdownKind::Quote => ("quote", 0, None),
        MarkdownKind::FencedCode => ("fenced-code", 0, None),
        MarkdownKind::IndentedCode => ("indented-code", 0, None),
        MarkdownKind::ThematicBreak => ("thematic-break", 0, None),
        MarkdownKind::RawHtmlBlock => ("raw-html-block", 0, None),
    })
}
pub(super) fn inline_form(token: &str) -> Result<&'static str, Error> {
    const FORMS: [&str; 19] = [
        "emphasis",
        "strong",
        "code",
        "text",
        "soft-break",
        "hard-break",
        "html",
        "link-inline",
        "link-reference",
        "link-collapsed",
        "link-shortcut",
        "link-autolink",
        "link-email",
        "image-inline",
        "image-reference",
        "image-collapsed",
        "image-shortcut",
        "image-autolink",
        "image-email",
    ];
    FORMS
        .into_iter()
        .find(|form| *form == token)
        .ok_or(Error::InvalidStructure)
}
fn observed_form(kind: MarkdownInlineKind) -> &'static str {
    let link = |kind, image| match (kind, image) {
        (MarkdownObservedLinkKind::Inline, false) => "link-inline",
        (MarkdownObservedLinkKind::Reference, false) => "link-reference",
        (MarkdownObservedLinkKind::Collapsed, false) => "link-collapsed",
        (MarkdownObservedLinkKind::Shortcut, false) => "link-shortcut",
        (MarkdownObservedLinkKind::Autolink, false) => "link-autolink",
        (MarkdownObservedLinkKind::Email, false) => "link-email",
        (MarkdownObservedLinkKind::Inline, true) => "image-inline",
        (MarkdownObservedLinkKind::Reference, true) => "image-reference",
        (MarkdownObservedLinkKind::Collapsed, true) => "image-collapsed",
        (MarkdownObservedLinkKind::Shortcut, true) => "image-shortcut",
        (MarkdownObservedLinkKind::Autolink, true) => "image-autolink",
        (MarkdownObservedLinkKind::Email, true) => "image-email",
    };
    match kind {
        MarkdownInlineKind::Emphasis => "emphasis",
        MarkdownInlineKind::Strong => "strong",
        MarkdownInlineKind::Code => "code",
        MarkdownInlineKind::Text => "text",
        MarkdownInlineKind::SoftBreak => "soft-break",
        MarkdownInlineKind::HardBreak => "hard-break",
        MarkdownInlineKind::Html => "html",
        MarkdownInlineKind::Link(kind) => link(kind, false),
        MarkdownInlineKind::Image(kind) => link(kind, true),
    }
}
fn reserve_raw<'a>(
    source: &MarkdownUnitizationSource<'_>,
    meter: &mut Meter<'a>,
) -> Result<UnitizationBudget<'a>, Error> {
    let descriptor = source.raw_unitizer_profile.descriptor();
    let units = source
        .product
        .canonical_text()
        .len()
        .min(descriptor.limits.max_units);
    if units > meter.budget.limits.raw_units {
        return Err(Error::InvalidStructure);
    }
    let canonical = add(
        add(
            add(search_contracts::MAX_CANONICAL_BYTES, 4096)?,
            add(source.product.canonical_text().len(), mul(units, 8320)?)?,
        )?,
        add(descriptor.max_manifest_bytes, 128)?,
    )?;
    meter.canonical(canonical)?;
    meter.work(add(index(descriptor.max_steps)?, canonical)?)?;
    meter.owned(add(
        mul(
            source.product.raw_product().canonical().lines().len(),
            size_of::<crate::SourceLineSpan>(),
        )?,
        add(
            mul(
                units,
                add(
                    size_of::<crate::UnitDescriptor>(),
                    add(size_of::<crate::UnitSpan>(), 48)?,
                )?,
            )?,
            add(
                mul(add(mul(units, 32)?, 128)?, size_of::<CanonicalValue>())?,
                mul(descriptor.max_manifest_bytes, 3)?,
            )?,
        )?,
    )?)?;
    Ok(UnitizationBudget {
        max_steps: descriptor.max_steps,
        max_encoded_bytes: descriptor.max_manifest_bytes,
        deadline: meter.budget.deadline,
        cancelled: meter.budget.cancelled,
    })
}
fn reconstruct(
    source: &MarkdownUnitizationSource<'_>,
    profile: &MarkdownUnitizerProfile,
    meter: &mut Meter<'_>,
) -> Result<VerifiedMarkdownUnitSet, Error> {
    meter.poll()?;
    let product = source.product;
    if profile.raw_id != *source.raw_unitizer_profile.id()
        || profile.markdown_id != source.markdown_profile.id()
        || product.profile_id() != profile.markdown_id
        || source.raw_profile.id() != source.markdown_profile.raw_profile_id()
        || source.raw_unitizer_profile.descriptor().representation_kind
            != V3RepresentationKind::Text
        || product.nodes().is_empty()
        || product.nodes().len() > meter.budget.limits.nodes
        || product.inline_records().len() > meter.budget.limits.inline_records
        || product.canonical_text().len() > meter.budget.limits.input_bytes
    {
        return Err(Error::ProfileMismatch);
    }
    meter.work(64)?;
    meter.owned(64)?;
    if source.request.source_id().as_str() != source.binding.raw.source_id.to_string()
        || source.binding.raw.representation_id == source.binding.structural
    {
        return Err(Error::BindingMismatch);
    }
    source
        .raw_budget
        .validate()
        .map_err(|_| Error::InvalidLimits)?;
    source
        .replay_limits
        .validate()
        .map_err(|_| Error::InvalidLimits)?;
    // Two independent admitted counters, checked before the port/context clone.
    meter.work(add(
        index(source.raw_budget.max_steps)?,
        source.replay_limits.parser.work_steps,
    )?)?;
    meter.canonical(source.replay_limits.total_canonical_bytes)?;
    meter.owned(source.replay_limits.parser.owned_bytes)?;
    let raw_context = MaterializationContext {
        profile: source.raw_profile.clone(),
        budget: source.raw_budget,
        cancel: CancellationToken::new(meter.budget.cancelled),
    };
    let replay = verify_markdown(
        product,
        source.request,
        source.port,
        &raw_context,
        source.markdown_profile,
        source.replay_limits,
        meter.budget.deadline,
    )
    .map_err(|error| match error {
        MarkdownError::Cancelled => Error::Cancelled,
        MarkdownError::DeadlineExceeded => Error::DeadlineExceeded,
        _ => Error::SourceReplayFailed,
    })?;
    meter.poll()?;
    let raw_budget = reserve_raw(source, meter)?;
    let input = crate::prepare_unit_set_input(
        source.binding.raw.clone(),
        product.raw_product(),
        source.request,
        source.raw_profile,
        source.raw_unitizer_profile,
        &raw_budget,
    )?;
    let raw = crate::build_unit_manifest(&input, source.raw_unitizer_profile, &raw_budget)?;
    meter.poll()?;
    meter.owned(2048)?;
    meter.work(2048)?;
    let mut manifest = MarkdownManifestBody {
        profile: profile.clone(),
        raw: raw.manifest,
        structural: source.binding.structural,
        residency: product.residency().clone(),
        markdown_product: product.product_digest(),
        markdown_nodes: product.nodes_digest(),
        markdown_inline: product.inline_digest(),
        nodes: Vec::new(),
        inline: Vec::new(),
    };
    let identity_context = context(&manifest, meter)?;
    meter.owned(mul(
        product.nodes().len(),
        add(size_of::<MarkdownStructuralUnit>(), 256)?,
    )?)?;
    manifest
        .nodes
        .try_reserve_exact(product.nodes().len())
        .map_err(|_| Error::AllocationFailed)?;
    let mut metadata = 0;
    for node in product.nodes() {
        meter.poll()?;
        meter.work(128)?;
        let (kind, heading_level, parent_section) = node_kind(node.kind())?;
        let start = number(node.span().start())?;
        let end = number(node.span().end())?;
        let content = product
            .canonical_text()
            .get(node.span().start()..node.span().end())
            .ok_or(Error::InvalidStructure)?;
        let content_digest = meter.hash_raw("eliot/raw/md-unit-content/v4", content.as_bytes())?;
        let data = MarkdownNodeData {
            ordinal: number(node.ordinal())?,
            kind,
            heading_level,
            parent_section,
            physical_parent: node.physical_parent().map(number).transpose()?,
            heading_section: node.heading_section().map(number).transpose()?,
            start,
            end,
            code_info: mapped(node.code_info(), &mut metadata, meter)?,
            content: content_digest,
        };
        let identity = identity(identity_context, &data, meter)?;
        let occurrence = occurrence(
            truncated(identity),
            manifest.structural,
            data.ordinal,
            kind,
            (start, end),
            manifest.raw.provenance().input_digest(),
            identity,
        )?;
        let unit = MarkdownStructuralUnit {
            data,
            occurrence,
            identity,
        };
        manifest.nodes.push(unit);
    }
    meter.owned(mul(
        product.inline_records().len(),
        size_of::<MarkdownInlineObservation>(),
    )?)?;
    manifest
        .inline
        .try_reserve_exact(product.inline_records().len())
        .map_err(|_| Error::AllocationFailed)?;
    for record in product.inline_records() {
        meter.poll()?;
        meter.work(128)?;
        manifest.inline.push(MarkdownInlineObservation {
            owner: number(record.owner())?,
            form: observed_form(record.kind()),
            start: number(record.span().start())?,
            end: number(record.span().end())?,
            text: mapped(record.text(), &mut metadata, meter)?,
            destination: mapped(record.destination(), &mut metadata, meter)?,
            title: mapped(record.title(), &mut metadata, meter)?,
        });
    }
    shape(&manifest, meter)?;
    let value = body(&manifest, meter)?;
    let digest = meter.hash("eliot/cbor/md-unit-manifest/v4", value)?;
    let manifest = MarkdownUnitManifest {
        body: manifest,
        digest,
    };
    let _encoded = encode(&manifest, meter)?;
    meter.poll()?;
    let representation = Representation {
        representation_id: manifest.body.structural,
        materialization_id: manifest.body.raw.provenance().binding().materialization_id,
        unitizer_profile_id: profile.id().clone(),
        enrichment_profile_ids: BoundedList::empty(),
        unit_manifest_digest: manifest.digest,
    };
    let usage = MarkdownUnitizationUsage {
        work: meter.work,
        canonical_bytes: meter.canonical,
        owned_bytes: meter.owned,
        markdown_replay_work: replay.work_used(),
        markdown_replay_canonical_bytes: replay.canonical_bytes_used(),
    };
    Ok(VerifiedMarkdownUnitSet {
        manifest,
        representation,
        usage,
    })
}
/// Reopen the exact revision and construct every structural/inline observation.
pub fn unitize_markdown(
    source: &MarkdownUnitizationSource<'_>,
    profile: &MarkdownUnitizerProfile,
    budget: &MarkdownUnitizationBudget<'_>,
) -> Result<VerifiedMarkdownUnitSet, Error> {
    let mut meter = Meter::new(profile, budget)?;
    let result = reconstruct(source, profile, &mut meter);
    meter.poll()?;
    result
}
/// Reconstruct and compare every proposed field; rehashed omissions cannot verify.
pub fn verify_markdown_unit_manifest(
    proposed: &MarkdownUnitManifest,
    source: &MarkdownUnitizationSource<'_>,
    profile: &MarkdownUnitizerProfile,
    budget: &MarkdownUnitizationBudget<'_>,
) -> Result<VerifiedMarkdownUnitSet, Error> {
    let mut meter = Meter::new(profile, budget)?;
    let result = (|| {
        if proposed.body.profile != *profile {
            return Err(Error::ProfileMismatch);
        }
        shape(&proposed.body, &mut meter)?;
        // Precharge the held proposal and every later comparison before replay/Eq.
        let bytes = encode(proposed, &mut meter)?;
        meter.work(bytes.len())?;
        meter.owned(add(
            bytes.len(),
            mul(
                add(
                    add(proposed.body.nodes.len(), proposed.body.inline.len())?,
                    proposed.body.raw.unit_count(),
                )?,
                512,
            )?,
        )?)?;
        let mut expected = reconstruct(source, profile, &mut meter)?;
        meter.work(add(
            bytes.len(),
            mul(
                expected.manifest.body.raw.unit_count(),
                size_of::<crate::UnitDescriptor>(),
            )?,
        )?)?;
        let equal = proposed == &expected.manifest;
        meter.poll()?;
        if !equal {
            return Err(Error::ManifestMismatch);
        }
        expected.usage.work = meter.work;
        expected.usage.canonical_bytes = meter.canonical;
        expected.usage.owned_bytes = meter.owned;
        Ok(expected)
    })();
    meter.poll()?;
    result
}
