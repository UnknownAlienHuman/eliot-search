//! Existing raw owner followed by inert observations and complete owning replay.
use super::identity::{inventory_identity, product_identity};
use super::meter::Meter;
use super::model::{
    Inventory, JatsAttribute, JatsComment as Comment, JatsElement, JatsError as E,
    JatsObservationStatus, JatsSpan, JatsText,
};
use super::parse::parse;
use super::profile::{JatsOperationLimits, JatsProfile, validate_raw};
use crate::api::{
    AssuranceCeiling, CanonicalLine, CoordinateSegment, LossRecord, MaterializationContext,
    MaterializationProduct, MaterializationWarning, NewlinePolicy, RevisionReadPort,
    SourceEncoding, SourceKind, ValidatedMaterializationRequest, materialize_text_or_code,
    verify_materialization,
};
use search_contracts::{Blake3Digest32, OpaqueId};
use std::fmt;
use std::mem::size_of;
use std::time::Instant;

/// Immutable owner-produced preparation; no public constructor or Clone.
#[derive(Eq, PartialEq)]
pub struct JatsProduct {
    pub(super) raw: MaterializationProduct,
    pub(super) residency: OpaqueId,
    pub(super) profile: Blake3Digest32,
    pub(super) inventory_digest: Blake3Digest32,
    pub(super) digest: Blake3Digest32,
    pub(super) inventory: Inventory,
    pub(super) canonical_bytes: usize,
}
impl JatsProduct {
    /// Original raw baseline product, maps and exact assurance.
    pub const fn raw_product(&self) -> &MaterializationProduct {
        &self.raw
    }
    /// Retained original XML, never normalized display text.
    pub fn canonical_text(&self) -> &str {
        self.raw.canonical_text()
    }
    /// Exact residency observation, not a locator.
    pub const fn residency(&self) -> &OpaqueId {
        &self.residency
    }
    /// Owner-computed selected profile.
    pub const fn profile_id(&self) -> Blake3Digest32 {
        self.profile
    }
    /// Ordered complete inventory commitment.
    pub const fn inventory_digest(&self) -> Blake3Digest32 {
        self.inventory_digest
    }
    /// Source/raw/maps/residency/profile/inventory commitment.
    pub const fn product_digest(&self) -> Blake3Digest32 {
        self.digest
    }
    /// Complete flat elements in opening-event order.
    pub fn elements(&self) -> &[JatsElement] {
        &self.inventory.elements
    }
    /// Complete allowed attributes in source order, with whole-tag provenance.
    pub fn attributes(&self) -> &[JatsAttribute] {
        &self.inventory.attributes
    }
    /// Complete decoded content observations in event order.
    pub fn texts(&self) -> &[JatsText] {
        &self.inventory.texts
    }
    /// Complete accepted comment extent inventory; no content is copied.
    pub fn comments(&self) -> &[Comment] {
        &self.inventory.comments
    }
    /// Optional original declaration extent.
    pub const fn declaration_span(&self) -> Option<JatsSpan> {
        self.inventory.declaration
    }
    /// Explicit mapped/raw/subset assurance, never full schema conformance.
    pub const fn observation_status(&self) -> JatsObservationStatus {
        JatsObservationStatus::CompleteSubsetMappedValues
    }
    /// Actual successful complete inventory plus product frame bytes.
    pub const fn canonical_bytes_used(&self) -> usize {
        self.canonical_bytes
    }
    /// Parser calls include EOF and prolog/epilog whitespace events.
    pub const fn event_calls(&self) -> usize {
        self.inventory.events
    }
}
impl fmt::Debug for JatsProduct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JatsProduct")
            .field("profile", &self.profile)
            .field("digest", &self.digest)
            .field("elements", &self.inventory.elements.len())
            .field("attributes", &self.inventory.attributes.len())
            .field("texts", &self.inventory.texts.len())
            .finish_non_exhaustive()
    }
}
/// Owning full reconstruction result, without persistence/publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JatsVerificationReceipt {
    profile: Blake3Digest32,
    product: Blake3Digest32,
    raw: Blake3Digest32,
    canonical: usize,
    work: usize,
}
impl JatsVerificationReceipt {
    /// Compared exact profile commitment.
    pub const fn profile_id(self) -> Blake3Digest32 {
        self.profile
    }
    /// Compared whole product commitment.
    pub const fn product_digest(self) -> Blake3Digest32 {
        self.product
    }
    /// Compared existing raw representation commitment.
    pub const fn raw_digest(self) -> Blake3Digest32 {
        self.raw
    }
    /// Actual successful complete replay frame bytes.
    pub const fn canonical_bytes_used(self) -> usize {
        self.canonical
    }
    /// Cumulative admitted logical work, not CPU measurement.
    pub const fn work_used(self) -> usize {
        self.work
    }
}
fn operation<'a>(
    limits: JatsOperationLimits,
    context: &MaterializationContext<'a>,
    deadline: Instant,
) -> Result<Meter<'a>, E> {
    let meter = Meter {
        limits: limits.validate()?,
        operation: Some((context.cancel, deadline)),
        work: 0,
        owned: 0,
        canonical: 0,
    };
    meter.poll()?;
    Ok(meter)
}
/// Preflights before the source port and preserves original cancel/deadline.
pub fn materialize_jats(
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    context: &MaterializationContext<'_>,
    profile: &JatsProfile,
    limits: JatsOperationLimits,
    deadline: Instant,
) -> Result<JatsProduct, E> {
    let mut meter = operation(limits, context, deadline)?;
    let result = build(request, port, context, profile, &mut meter);
    meter.poll()?;
    result
}
/// Reopens the same retained revision and compares every raw/map/inventory field.
pub fn verify_jats(
    product: &JatsProduct,
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    context: &MaterializationContext<'_>,
    profile: &JatsProfile,
    limits: JatsOperationLimits,
    deadline: Instant,
) -> Result<JatsVerificationReceipt, E> {
    let mut meter = operation(limits, context, deadline)?;
    let result = (|| {
        let held = hold_inventory(&product.inventory, &mut meter)?;
        meter.own(product.residency.as_str().len())?;
        let rebuilt = build(request, port, context, profile, &mut meter)?;
        let replayed = inventory_bytes(&rebuilt.inventory)?;
        meter.work(held.checked_add(replayed).ok_or(E::Overflow)?)?;
        meter.work(
            raw_work(&product.raw)?
                .checked_add(raw_work(&rebuilt.raw)?)
                .ok_or(E::Overflow)?,
        )?;
        let equal = product == &rebuilt;
        #[cfg(test)]
        super::fixtures::terminal(&meter, true);
        meter.poll()?;
        if !equal {
            return Err(E::ProductMismatch);
        }
        Ok(JatsVerificationReceipt {
            profile: product.profile,
            product: product.digest,
            raw: product.raw.representation_id(),
            canonical: meter.canonical,
            work: meter.work,
        })
    })();
    meter.poll()?;
    result
}
fn build(
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    context: &MaterializationContext<'_>,
    profile: &JatsProfile,
    meter: &mut Meter<'_>,
) -> Result<JatsProduct, E> {
    validate_raw(&context.profile)?;
    meter.parser().narrow(profile.limits())?;
    if request.profile().id() != context.profile.id()
        || profile.raw_profile_id() != context.profile.id()
        || request.declared_kind() != SourceKind::Text
        || request.declared_encoding() != SourceEncoding::Utf8
    {
        return Err(E::ProfileMismatch);
    }
    let input = usize::try_from(request.byte_count()).map_err(|_| E::Overflow)?;
    if input == 0 || input > meter.parser().input_bytes {
        return Err(E::LimitExceeded);
    }
    let mut raw_budget = context.budget.validate().map_err(E::Materialization)?;
    let limit = u64::try_from(meter.parser().input_bytes).map_err(|_| E::Overflow)?;
    raw_budget.max_input_bytes = raw_budget
        .max_input_bytes
        .min(context.profile.limits().max_input_bytes)
        .min(limit);
    raw_budget.max_output_bytes = raw_budget
        .max_output_bytes
        .min(context.profile.limits().max_output_bytes)
        .min(limit);
    if request.byte_count() > raw_budget.max_input_bytes
        || request.byte_count() > raw_budget.max_output_bytes
    {
        return Err(E::LimitExceeded);
    }
    meter.work(input)?; // before port: independent additional raw-pass reservation
    let raw_context = MaterializationContext {
        profile: context.profile.clone(),
        budget: raw_budget,
        cancel: context.cancel,
    };
    let result = materialize_text_or_code(request, port, &raw_context);
    meter.poll()?;
    let raw = result.map_err(E::Materialization)?;
    meter.work(raw_work(&raw)?)?;
    verify_materialization(&raw, request, &context.profile).map_err(E::Materialization)?;
    meter.poll()?;
    if raw.encoding() != SourceEncoding::Utf8
        || raw.canonical().source_encoding() != SourceEncoding::Utf8
        || raw.canonical().bom_stripped()
        || raw.canonical().transcoded()
        || raw.canonical().newline_policy() != NewlinePolicy::PreserveExact
        || raw.canonical_digest() != raw.input_digest()
        || raw.resource_receipt().input_bytes != request.byte_count()
        || raw.resource_receipt().output_bytes != request.byte_count()
        || raw.canonical().native_len() != request.byte_count()
        || raw.canonical().canonical_len_bytes() != request.byte_count()
        || !raw.maps().loss_map().records().is_empty()
        || !raw.warnings().is_empty()
        || raw.assurance().ceiling() != AssuranceCeiling::ExactBytes
    {
        return Err(E::ProductMismatch);
    }
    let inventory = parse(raw.canonical_text(), meter)?;
    let inventory_digest = inventory_identity(&inventory, meter)?;
    let digest = product_identity(&raw, request, profile, inventory_digest, meter)?;
    meter.own(request.residency().as_str().len())?;
    meter.work(request.residency().as_str().len())?;
    let product = JatsProduct {
        raw,
        residency: request.residency().clone(),
        profile: profile.id(),
        inventory_digest,
        digest,
        inventory,
        canonical_bytes: meter.canonical,
    };
    #[cfg(test)]
    super::fixtures::terminal(meter, false);
    meter.poll()?;
    Ok(product)
}
fn add(total: &mut usize, value: usize) -> Result<(), E> {
    *total = total.checked_add(value).ok_or(E::Overflow)?;
    Ok(())
}
fn sized<T>(total: &mut usize, count: usize) -> Result<(), E> {
    add(total, count.checked_mul(size_of::<T>()).ok_or(E::Overflow)?)
}
fn raw_work(raw: &MaterializationProduct) -> Result<usize, E> {
    let mut bytes = raw.canonical_text().len();
    add(&mut bytes, raw.source_id().as_str().len())?;
    add(
        &mut bytes,
        raw.maps().coordinate_map().source_id().as_str().len(),
    )?;
    add(&mut bytes, raw.maps().loss_map().source_id().as_str().len())?;
    sized::<CanonicalLine>(&mut bytes, raw.canonical().lines().len())?;
    sized::<CoordinateSegment>(&mut bytes, raw.maps().coordinate_map().segments().len())?;
    sized::<LossRecord>(&mut bytes, raw.maps().loss_map().records().len())?;
    sized::<MaterializationWarning>(&mut bytes, raw.warnings().len())?;
    Ok(bytes)
}
fn inventory_bytes(out: &Inventory) -> Result<usize, E> {
    let mut bytes = size_of::<Inventory>();
    sized::<JatsElement>(&mut bytes, out.elements.len())?;
    sized::<JatsAttribute>(&mut bytes, out.attributes.len())?;
    sized::<JatsText>(&mut bytes, out.texts.len())?;
    sized::<Comment>(&mut bytes, out.comments.len())?;
    for value in &out.attributes {
        add(&mut bytes, value.value.len())?;
    }
    for value in &out.texts {
        add(&mut bytes, value.value.len())?;
    }
    Ok(bytes)
}
fn hold_inventory(out: &Inventory, meter: &mut Meter<'_>) -> Result<usize, E> {
    let limits = meter.parser();
    let count = out.count()?;
    if out.elements.len() > limits.elements
        || out.attributes.len() > limits.attributes
        || out.texts.len() > limits.texts
        || out.comments.len() > limits.comments
    {
        return Err(E::LimitExceeded);
    }
    meter.work(count)?;
    let bytes = inventory_bytes(out)?;
    meter.own(bytes)?;
    meter.poll()?;
    Ok(bytes)
}
