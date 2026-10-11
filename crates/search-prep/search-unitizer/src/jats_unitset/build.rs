//! Complete owning JATS replay, unchanged raw v3 coverage and structural reconstruction.
use super::budget::{Meter, add, index, mul, number};
use super::codec::{array, body, bytes, element, encode, node_identity, shape, text};
use super::model::JatsUnitizationError as Error;
use super::model::{
    JatsMappedValue, JatsSourceBinding, JatsStructuralUnit, JatsUnitAttribute, JatsUnitComment,
    JatsUnitManifest, JatsUnitSpan, JatsUnitText, JatsUnitizationUsage, ManifestBody, NodeData,
    VerifiedJatsUnitSet,
};
use super::{JatsUnitizationBudget, JatsUnitizerProfile};
use crate::{UnitizationBudget, V3RepresentationKind, ValidatedV3UnitizerProfile};
use search_contracts::{
    Blake3Digest32, BoundedList, CanonicalValue, NativeAnchor, OpaqueId, Representation,
    RepresentationId, TextBytesAnchor, UnitId, UnitKind, UnitOccurrence,
};
use search_materializer::api::{
    CancellationToken, JatsAttribute, JatsError, JatsOperationLimits, JatsProduct, JatsProfile,
    JatsSpan, JatsText, JatsTextKind, MaterializationBudget, MaterializationContext,
    RevisionReadPort, ValidatedMaterializationRequest, ValidatedMaterializerProfile, verify_jats,
};
use std::collections::BTreeMap;
use std::mem::size_of;

/// Named untrusted borrowed ingress; peer types appear only at this public boundary.
pub struct JatsUnitizationSource<'a> {
    /// Admission-supplied raw and distinct structural coordinates.
    pub binding: &'a JatsSourceBinding,
    /// Immutable producer product, verified by complete owning reconstruction.
    pub product: &'a JatsProduct,
    /// Validated exact retained-source request.
    pub request: &'a ValidatedMaterializationRequest,
    /// Existing exact revision port; this owner opens no source store.
    pub port: &'a dyn RevisionReadPort,
    /// Actual selected strict raw profile.
    pub raw_profile: &'a ValidatedMaterializerProfile,
    /// Independently reserved finite original raw allowance.
    pub raw_budget: MaterializationBudget,
    /// Actual selected immutable producer profile.
    pub jats_profile: &'a JatsProfile,
    /// Actual unchanged raw v3 unitizer profile.
    pub raw_unitizer_profile: &'a ValidatedV3UnitizerProfile,
    /// Lower original producer replay allowances, never stage-local reset defaults.
    pub replay_limits: JatsOperationLimits,
}
pub(super) fn span(value: JatsSpan) -> Result<JatsUnitSpan, Error> {
    Ok(JatsUnitSpan {
        start: number(value.start())?,
        end: number(value.end())?,
    })
}
pub(super) fn occurrence(
    id: UnitId,
    representation: RepresentationId,
    data: &NodeData,
    raw: Blake3Digest32,
    identity: Blake3Digest32,
) -> Result<UnitOccurrence, Error> {
    let kind = element(data.kind)?;
    let anchor = NativeAnchor::TextBytes(TextBytesAnchor {
        content_digest: raw,
        byte_start_0: data.span.start,
        byte_end_exclusive_0: data.span.end,
    });
    anchor.validate().map_err(|_| Error::InvalidStructure)?;
    Ok(UnitOccurrence {
        unit_id: id,
        representation_id: representation,
        unit_kind: match kind {
            "sec" => UnitKind::Section,
            _ => UnitKind::Doc,
        },
        ordinal: data.ordinal,
        native_anchor: anchor,
        structural_identity: Some(
            OpaqueId::new(format!("jats-unit-v5-{identity}")).map_err(|_| Error::Codec)?,
        ),
        configuration_predicate: None,
    })
}
pub(super) fn context(m: &ManifestBody, meter: &mut Meter<'_>) -> Result<Blake3Digest32, Error> {
    meter.rows(6, 512)?;
    let value = array(vec![
        text(m.profile.id().as_str())?,
        bytes(m.structural.as_bytes())?,
        bytes(m.raw.manifest_digest().as_bytes())?,
        bytes(m.product.as_bytes())?,
        bytes(m.inventory.as_bytes())?,
        text(m.residency.as_str())?,
    ])?;
    meter.hash("eliot/cbor/jats-unit-context/v5", value)
}
pub(super) fn identity(
    context: Blake3Digest32,
    data: &NodeData,
    meter: &mut Meter<'_>,
) -> Result<Blake3Digest32, Error> {
    meter.rows(2, 32)?;
    let value = array(vec![
        bytes(context.as_bytes())?,
        node_identity(data, meter)?,
    ])?;
    meter.hash("eliot/cbor/jats-unit-occurrence/v5", value)
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
fn mapped(value: &str, total: &mut usize, meter: &mut Meter<'_>) -> Result<JatsMappedValue, Error> {
    *total = add(*total, value.len())?;
    if value.len() > meter.budget.limits.metadata_bytes_each
        || *total > meter.budget.limits.metadata_bytes_total
    {
        return Err(Error::InvalidStructure);
    }
    meter.work(value.len())?;
    meter.owned(add(size_of::<JatsMappedValue>(), value.len())?)?;
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| Error::AllocationFailed)?;
    copy.push_str(value);
    Ok(JatsMappedValue { value: copy })
}
fn reserve<T>(values: &mut Vec<T>, count: usize, meter: &mut Meter<'_>) -> Result<(), Error> {
    meter.owned(mul(count, add(size_of::<T>(), 128)?)?)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)
}
fn reconstruct(
    source: &JatsUnitizationSource<'_>,
    profile: &JatsUnitizerProfile,
    meter: &mut Meter<'_>,
) -> Result<VerifiedJatsUnitSet, Error> {
    meter.poll()?;
    let p = source.product;
    let l = meter.budget.limits;
    if profile.raw_id != *source.raw_unitizer_profile.id()
        || profile.jats_id != source.jats_profile.id()
        || p.profile_id() != profile.jats_id
        || source.raw_profile.id() != source.jats_profile.raw_profile_id()
        || source.raw_unitizer_profile.descriptor().representation_kind
            != V3RepresentationKind::Text
        || p.elements().is_empty()
        || p.elements().len() > l.nodes
        || p.attributes().len() > l.attributes
        || p.texts().len() > l.texts
        || p.comments().len() > l.comments
        || p.event_calls() > l.events
        || p.canonical_text().len() > l.input_bytes
    {
        return Err(Error::ProfileMismatch);
    }
    meter.work(64)?;
    meter.owned(4096)?;
    let mut preflight_metadata = 0;
    for value in p
        .attributes()
        .iter()
        .map(JatsAttribute::value)
        .chain(p.texts().iter().map(JatsText::value))
    {
        meter.poll()?;
        meter.work(1)?;
        preflight_metadata = add(preflight_metadata, value.len())?;
        if value.len() > l.metadata_bytes_each || preflight_metadata > l.metadata_bytes_total {
            return Err(Error::InvalidStructure);
        }
    }
    meter.work(preflight_metadata)?;
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
    // Original raw and additional JATS counters are independently reserved before clone/port.
    meter.work(add(
        index(source.raw_budget.max_steps)?,
        source.replay_limits.parser.work_steps,
    )?)?;
    meter.canonical(source.replay_limits.total_canonical_bytes)?;
    meter.owned(source.replay_limits.parser.owned_bytes)?;
    let context_raw = MaterializationContext {
        profile: source.raw_profile.clone(),
        budget: source.raw_budget,
        cancel: CancellationToken::new(meter.budget.cancelled),
    };
    let replay = verify_jats(
        p,
        source.request,
        source.port,
        &context_raw,
        source.jats_profile,
        source.replay_limits,
        meter.budget.deadline,
    )
    .map_err(|error| match error {
        JatsError::Cancelled => Error::Cancelled,
        JatsError::DeadlineExceeded => Error::DeadlineExceeded,
        _ => Error::SourceReplayFailed,
    })?;
    meter.poll()?;
    let raw_budget = reserve_raw(source, meter)?;
    let input = crate::prepare_unit_set_input(
        source.binding.raw.clone(),
        p.raw_product(),
        source.request,
        source.raw_profile,
        source.raw_unitizer_profile,
        &raw_budget,
    )?;
    let raw = crate::build_unit_manifest(&input, source.raw_unitizer_profile, &raw_budget)?;
    meter.poll()?;
    meter.owned(2048)?;
    meter.work(2048)?;
    let mut m = ManifestBody {
        profile: profile.clone(),
        raw: raw.manifest,
        structural: source.binding.structural,
        residency: p.residency().clone(),
        product: p.product_digest(),
        inventory: p.inventory_digest(),
        events: number(p.event_calls())?,
        declaration: p.declaration_span().map(span).transpose()?,
        nodes: Vec::new(),
        attributes: Vec::new(),
        texts: Vec::new(),
        comments: Vec::new(),
    };
    let ctx = context(&m, meter)?;
    reserve(&mut m.nodes, p.elements().len(), meter)?;
    for (ordinal, node) in p.elements().iter().enumerate() {
        meter.poll()?;
        meter.work(128)?;
        let content = p
            .canonical_text()
            .get(node.span().start()..node.span().end())
            .ok_or(Error::InvalidStructure)?;
        let data = NodeData {
            ordinal: number(ordinal)?,
            kind: element(node.kind().name())?,
            parent: node.parent().map(number).transpose()?,
            event: number(node.event())?,
            close_event: number(node.close_event())?,
            opening: span(node.opening())?,
            closing: node.closing().map(span).transpose()?,
            span: span(node.span())?,
            content: meter.hash_raw("eliot/raw/jats-unit-content/v5", content.as_bytes())?,
        };
        let identity = identity(ctx, &data, meter)?;
        let occurrence = occurrence(
            truncated(identity),
            m.structural,
            &data,
            m.raw.provenance().input_digest(),
            identity,
        )?;
        m.nodes.push(JatsStructuralUnit {
            data,
            occurrence,
            identity,
        });
    }
    let mut metadata = 0;
    reserve(&mut m.attributes, p.attributes().len(), meter)?;
    for a in p.attributes() {
        meter.poll()?;
        meter.work(128)?;
        m.attributes.push(JatsUnitAttribute {
            owner: number(a.owner())?,
            kind: super::codec::attribute(a.kind().name())?,
            span: span(a.span())?,
            value: mapped(a.value(), &mut metadata, meter)?,
        });
    }
    reserve(&mut m.texts, p.texts().len(), meter)?;
    for t in p.texts() {
        meter.poll()?;
        meter.work(128)?;
        let kind = match t.kind() {
            JatsTextKind::Text => "text",
            JatsTextKind::Cdata => "cdata",
            JatsTextKind::Reference => "reference",
        };
        m.texts.push(JatsUnitText {
            owner: number(t.owner())?,
            event: number(t.event())?,
            kind,
            span: span(t.span())?,
            value: mapped(t.value(), &mut metadata, meter)?,
        });
    }
    reserve(&mut m.comments, p.comments().len(), meter)?;
    for c in p.comments() {
        meter.poll()?;
        meter.work(128)?;
        m.comments.push(JatsUnitComment {
            owner: c.owner().map(number).transpose()?,
            event: number(c.event())?,
            span: span(c.span())?,
        });
    }
    shape(&m, meter)?;
    let value = body(&m, meter)?;
    let digest = meter.hash("eliot/cbor/jats-unit-manifest/v5", value)?;
    let manifest = JatsUnitManifest { body: m, digest };
    let _encoded = encode(&manifest, meter)?;
    meter.poll()?;
    let representation = Representation {
        representation_id: manifest.body.structural,
        materialization_id: manifest.body.raw.provenance().binding().materialization_id,
        unitizer_profile_id: profile.id().clone(),
        enrichment_profile_ids: BoundedList::empty(),
        unit_manifest_digest: digest,
    };
    let usage = JatsUnitizationUsage {
        work: meter.work,
        canonical_bytes: meter.canonical,
        owned_bytes: meter.owned,
        jats_replay_work: replay.work_used(),
        jats_replay_canonical_bytes: replay.canonical_bytes_used(),
    };
    Ok(VerifiedJatsUnitSet {
        manifest,
        representation,
        usage,
    })
}
/// Reopen the same revision and construct the entire structural/metadata inventory.
pub fn unitize_jats(
    source: &JatsUnitizationSource<'_>,
    profile: &JatsUnitizerProfile,
    budget: &JatsUnitizationBudget<'_>,
) -> Result<VerifiedJatsUnitSet, Error> {
    let mut meter = Meter::new(profile, budget)?;
    let result = reconstruct(source, profile, &mut meter);
    #[cfg(test)]
    if result.is_ok() {
        super::fixtures::terminal(&meter, false);
    }
    meter.poll()?;
    result
}
/// Reconstruct and compare every proposed field; rehashed omissions cannot verify.
pub fn verify_jats_unit_manifest(
    proposed: &JatsUnitManifest,
    source: &JatsUnitizationSource<'_>,
    profile: &JatsUnitizerProfile,
    budget: &JatsUnitizationBudget<'_>,
) -> Result<VerifiedJatsUnitSet, Error> {
    let mut meter = Meter::new(profile, budget)?;
    let result = (|| {
        if proposed.body.profile != *profile {
            return Err(Error::ProfileMismatch);
        }
        shape(&proposed.body, &mut meter)?;
        let bytes = encode(proposed, &mut meter)?;
        let records = add(
            add(proposed.body.nodes.len(), proposed.body.attributes.len())?,
            add(
                add(proposed.body.texts.len(), proposed.body.comments.len())?,
                proposed.body.raw.unit_count(),
            )?,
        )?;
        meter.work(bytes.len())?;
        meter.owned(add(bytes.len(), mul(records, 512)?)?)?;
        let mut expected = reconstruct(source, profile, &mut meter)?;
        meter.work(add(
            bytes.len(),
            mul(
                expected.manifest.body.raw.unit_count(),
                size_of::<crate::UnitDescriptor>(),
            )?,
        )?)?;
        let equal = proposed == &expected.manifest;
        #[cfg(test)]
        super::fixtures::terminal(&meter, true);
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
fn reserve_raw<'a>(
    source: &JatsUnitizationSource<'_>,
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
