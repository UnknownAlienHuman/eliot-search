//! Inert Markdown preparation and owning exact-revision replay verification.
//! Every phase uses the original cancellation token and absolute deadline.

use std::fmt;
use std::mem::size_of;

use super::identity::{IdentityBudget, commit_markdown};
use super::model::{
    InlineMetadataStatus, InlineRecord, MarkdownDocument, MarkdownNode, MarkdownUsage,
    ReferenceMetadataStatus,
};
use super::parse::parse_markdown;
use super::profile::{
    MarkdownError, MarkdownMeter, MarkdownOperationLimits, MarkdownProfile, checkpoint,
    narrow_limits, validate_raw_profile,
};
use crate::api::{
    AssuranceCeiling, CanonicalLine, CoordinateSegment, LossRecord, MaterializationContext,
    MaterializationError, MaterializationProduct, MaterializationWarning, NewlinePolicy,
    RevisionReadPort, SourceEncoding, SourceKind, ValidatedMaterializationRequest,
    materialize_text_or_code,
};
use search_contracts::{Blake3Digest32, OpaqueId};
use std::time::Instant;

/// Immutable owner-produced Markdown preparation. It is neither a `UnitSet` nor
/// a durable/source-admission/currentness receipt. No unmetered public Clone.
#[derive(Eq, PartialEq)]
pub struct MarkdownProduct {
    raw: MaterializationProduct,
    residency: OpaqueId,
    profile: Blake3Digest32,
    nodes_digest: Blake3Digest32,
    inline_digest: Blake3Digest32,
    product_digest: Blake3Digest32,
    #[cfg(not(test))]
    inventory: MarkdownInventory,
    #[cfg(test)]
    pub(super) inventory: MarkdownInventory,
    canonical_bytes: usize,
    work_used: usize,
}

#[derive(Eq, PartialEq)]
pub(super) struct MarkdownInventory {
    pub(super) nodes: Vec<MarkdownNode>,
    pub(super) inline_records: Vec<InlineRecord>,
    pub(super) inline_metadata: InlineMetadataStatus,
    pub(super) reference_metadata: ReferenceMetadataStatus,
    pub(super) usage: MarkdownUsage,
}

impl MarkdownProduct {
    /// Unchanged raw baseline state, maps and assurance through a shared borrow.
    pub const fn raw_product(&self) -> &MaterializationProduct {
        &self.raw
    }
    /// Exact retained canonical bytes; no renderer or decoded text replacement.
    pub fn canonical_text(&self) -> &str {
        self.raw.canonical_text()
    }
    /// Verified retained residency identity, not a filesystem locator.
    pub const fn residency(&self) -> &OpaqueId {
        &self.residency
    }
    /// New closed Markdown profile equality commitment.
    pub const fn profile_id(&self) -> Blake3Digest32 {
        self.profile
    }
    /// Ordered complete structural observation commitment.
    pub const fn nodes_digest(&self) -> Blake3Digest32 {
        self.nodes_digest
    }
    /// Ordered complete yielded inline-observation commitment.
    pub const fn inline_digest(&self) -> Blake3Digest32 {
        self.inline_digest
    }
    /// New source/profile/raw/map/inventory-bound product commitment, not UUID.
    pub const fn product_digest(&self) -> Blake3Digest32 {
        self.product_digest
    }
    /// Borrowed immutable structural record views in donor preorder.
    pub fn nodes(&self) -> &[MarkdownNode] {
        &self.inventory.nodes
    }
    /// Borrowed immutable inline record views; spans are whole-event provenance.
    pub fn inline_records(&self) -> &[InlineRecord] {
        &self.inventory.inline_records
    }
    /// The selected profile observes spans plus selected mapped syntax values.
    pub const fn inline_metadata_status(&self) -> InlineMetadataStatus {
        self.inventory.inline_metadata
    }
    /// Donor reference-fuel fallback prevents an exhaustive-reference assertion.
    pub const fn reference_metadata_status(&self) -> ReferenceMetadataStatus {
        self.inventory.reference_metadata
    }
    /// Actual successful nodes+inline+product domain/NUL/CBOR byte usage.
    pub const fn canonical_bytes_used(&self) -> usize {
        self.canonical_bytes
    }
    /// Accounted work through product construction; not measured CPU time.
    pub const fn work_used(&self) -> usize {
        self.work_used
    }
    /// Donor next attempts include the terminal call returning None.
    pub const fn next_attempts(&self) -> usize {
        self.inventory.usage.next_attempts
    }
    /// Actual yielded events, independently counted from next attempts.
    pub const fn yielded_events(&self) -> usize {
        self.inventory.usage.yielded_events
    }
}
impl fmt::Debug for MarkdownProduct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarkdownProduct")
            .field("profile", &self.profile)
            .field("product_digest", &self.product_digest)
            .field("nodes", &self.inventory.nodes.len())
            .field("inline_records", &self.inventory.inline_records.len())
            .field("raw_bytes", &self.raw.canonical_text().len())
            .finish_non_exhaustive()
    }
}

/// Immutable source-replay result; no publication/UnitSet/native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkdownVerificationReceipt {
    profile: Blake3Digest32,
    product: Blake3Digest32,
    raw: Blake3Digest32,
    canonical_bytes: usize,
    work: usize,
}
impl MarkdownVerificationReceipt {
    /// Compared closed profile identity.
    pub const fn profile_id(self) -> Blake3Digest32 {
        self.profile
    }
    /// Compared source-backed full product commitment.
    pub const fn product_digest(self) -> Blake3Digest32 {
        self.product
    }
    /// Compared unchanged raw baseline commitment.
    pub const fn raw_digest(self) -> Blake3Digest32 {
        self.raw
    }
    /// Actual complete bytes consumed by this replay's three canonical frames.
    pub const fn canonical_bytes_used(self) -> usize {
        self.canonical_bytes
    }
    /// Accounted replay plus comparison work under one original allowance.
    pub const fn work_used(self) -> usize {
        self.work
    }
}

/// Preflights before the port, runs the real raw pipeline once, then parses and
/// commits original canonical bytes.
///
/// Original token/deadline are never renewed.
/// Refusal drops every provisional result; donor calls are cooperatively bracketed.
pub fn materialize_markdown(
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    base_context: &MaterializationContext<'_>,
    markdown_profile: &MarkdownProfile,
    operation_limits: MarkdownOperationLimits,
    deadline: Instant,
) -> Result<MarkdownProduct, MarkdownError> {
    checkpoint(base_context.cancel, deadline)?;
    let result = (|| {
        let mut meter = MarkdownMeter::operation(operation_limits, base_context.cancel, deadline)?;
        materialize_inner(
            request,
            port,
            base_context,
            markdown_profile,
            operation_limits,
            deadline,
            &mut meter,
        )
    })();
    checkpoint(base_context.cancel, deadline)?;
    result
}

/// Reopens the same retained revision through the original port and reconstructs
/// all raw/maps/records/identities.
///
/// Comparison work is charged before full equality;
/// original held inventory, replay and row temporaries share one storage ceiling.
pub fn verify_markdown(
    product: &MarkdownProduct,
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    base_context: &MaterializationContext<'_>,
    markdown_profile: &MarkdownProfile,
    operation_limits: MarkdownOperationLimits,
    deadline: Instant,
) -> Result<MarkdownVerificationReceipt, MarkdownError> {
    checkpoint(base_context.cancel, deadline)?;
    let result = (|| {
        let mut meter = MarkdownMeter::operation(operation_limits, base_context.cancel, deadline)?;
        check_inventory_counts(&product.inventory, operation_limits)?;
        meter.hold_original_inventory(
            product
                .inventory
                .usage
                .owned_live
                .checked_add(product.residency.as_str().len())
                .ok_or(MarkdownError::ArithmeticOverflow)?,
        )?;
        let rebuilt = materialize_inner(
            request,
            port,
            base_context,
            markdown_profile,
            operation_limits,
            deadline,
            &mut meter,
        )?;
        let held = comparison_work(product, &mut meter)?;
        let replay = comparison_work(&rebuilt, &mut meter)?;
        meter.charge_work(
            held.checked_add(replay)
                .ok_or(MarkdownError::ArithmeticOverflow)?,
        )?;
        let equal = product == &rebuilt;
        meter.poll()?;
        if !equal {
            return Err(MarkdownError::ProductMismatch);
        }
        Ok(MarkdownVerificationReceipt {
            profile: product.profile,
            product: product.product_digest,
            raw: product.raw.representation_id(),
            canonical_bytes: meter.canonical_used(),
            work: meter.work_used(),
        })
    })();
    checkpoint(base_context.cancel, deadline)?;
    result
}

fn materialize_inner(
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    base_context: &MaterializationContext<'_>,
    profile: &MarkdownProfile,
    limits: MarkdownOperationLimits,
    deadline: Instant,
    meter: &mut MarkdownMeter<'_>,
) -> Result<MarkdownProduct, MarkdownError> {
    meter.poll()?;
    let mut budget = base_context
        .budget
        .validate()
        .map_err(MarkdownError::Materialization)?;
    limits.validate()?;
    validate_raw_profile(&base_context.profile)?;
    narrow_limits(limits.parser, profile.limits())?;
    if request.profile().id() != base_context.profile.id()
        || profile.raw_profile_id() != base_context.profile.id()
        || request.declared_kind() != SourceKind::Text
        || request.declared_encoding() != SourceEncoding::Utf8
    {
        return Err(MarkdownError::ProfileMismatch);
    }
    let input =
        u64::try_from(limits.parser.input_bytes).map_err(|_| MarkdownError::ArithmeticOverflow)?;
    let raw_limits = base_context.profile.limits();
    budget.max_input_bytes = budget
        .max_input_bytes
        .min(raw_limits.max_input_bytes)
        .min(input);
    budget.max_output_bytes = budget
        .max_output_bytes
        .min(raw_limits.max_output_bytes)
        .min(input);
    if request.byte_count() == 0 {
        return Err(MarkdownError::EmptyInput);
    }
    if request.byte_count() > budget.max_input_bytes {
        return Err(MarkdownError::InputLimit);
    }
    if request.byte_count() > budget.max_output_bytes {
        return Err(MarkdownError::Materialization(
            MaterializationError::OutputTooLarge,
        ));
    }
    let narrowed = MaterializationContext {
        profile: base_context.profile.clone(),
        budget,
        cancel: base_context.cancel,
    };
    let raw_result = materialize_text_or_code(request, port, &narrowed);
    meter.poll()?;
    let raw = raw_result.map_err(MarkdownError::Materialization)?;
    // Extra raw verification is charged before its hash/map traversal. Legacy
    // pipeline StepCounter accounting remains separate and unchanged.
    meter.charge_work(raw_work(&raw)?)?;
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
        return Err(MarkdownError::ProductMismatch);
    }
    let parser_limits = meter.parser_limits()?;
    let parsed = parse_markdown(
        raw.canonical_text().as_bytes(),
        parser_limits,
        base_context.cancel,
        deadline,
    );
    meter.poll()?;
    let document = parsed?;
    meter.charge_work(document.usage.steps)?;
    meter.hold_retained(document.usage.owned_live)?;
    // The sole identity producer performs the owning raw verifier and charged
    // exact source equality, then three measured canonical frames. It accepts
    // the actual opaque wrapper profile, preserving its lower profile limits.
    let commitments = commit_markdown(
        &document,
        &raw,
        request,
        profile,
        limits,
        (base_context.cancel, deadline),
        meter,
    )?;
    meter.poll()?;
    let nodes_digest = commitments.nodes();
    let inline_digest = commitments.inline();
    let product_digest = commitments.product();
    meter.hold_retained(request.residency().as_str().len())?;
    meter.charge_work(request.residency().as_str().len())?;
    let residency = request.residency().clone();
    let MarkdownDocument {
        source: _,
        nodes,
        inline_records,
        inline_metadata,
        reference_metadata,
        usage,
    } = document;
    let inventory = MarkdownInventory {
        nodes,
        inline_records,
        inline_metadata,
        reference_metadata,
        usage,
    };
    let product = MarkdownProduct {
        raw,
        residency,
        profile: profile.id(),
        nodes_digest,
        inline_digest,
        product_digest,
        inventory,
        canonical_bytes: commitments.complete_preimage_bytes(),
        work_used: meter.work_used(),
    };
    meter.poll()?;
    Ok(product)
}

const fn check_inventory_counts(
    inventory: &MarkdownInventory,
    limits: MarkdownOperationLimits,
) -> Result<(), MarkdownError> {
    if inventory.nodes.len() > limits.parser.nodes {
        return Err(MarkdownError::NodeLimit);
    }
    if inventory.inline_records.len() > limits.parser.inline_records {
        return Err(MarkdownError::InlineLimit);
    }
    Ok(())
}
fn add(total: &mut usize, value: usize) -> Result<(), MarkdownError> {
    *total = total
        .checked_add(value)
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    Ok(())
}
fn array_work(total: &mut usize, count: usize, element: usize) -> Result<(), MarkdownError> {
    add(
        total,
        count
            .checked_mul(element)
            .ok_or(MarkdownError::ArithmeticOverflow)?,
    )
}
fn raw_work(raw: &MaterializationProduct) -> Result<usize, MarkdownError> {
    // Defined logical comparison/verification units, not an instruction bound or
    // encoded-length formula; raw buffers/maps are separately byte/count bounded.
    let mut cost = size_of::<MaterializationProduct>();
    add(&mut cost, raw.canonical_text().len())?;
    add(&mut cost, raw.source_id().as_str().len())?;
    add(
        &mut cost,
        raw.maps().coordinate_map().source_id().as_str().len(),
    )?;
    add(&mut cost, raw.maps().loss_map().source_id().as_str().len())?;
    array_work(
        &mut cost,
        raw.canonical().lines().len(),
        size_of::<CanonicalLine>(),
    )?;
    array_work(
        &mut cost,
        raw.maps().coordinate_map().segments().len(),
        size_of::<CoordinateSegment>(),
    )?;
    array_work(
        &mut cost,
        raw.maps().loss_map().records().len(),
        size_of::<LossRecord>(),
    )?;
    array_work(
        &mut cost,
        raw.warnings().len(),
        size_of::<MaterializationWarning>(),
    )?;
    Ok(cost)
}
fn comparison_work(
    product: &MarkdownProduct,
    meter: &mut MarkdownMeter<'_>,
) -> Result<usize, MarkdownError> {
    let inventory = &product.inventory;
    // Precharge cardinality before the bookkeeping loop; every selected value
    // contributes its actual byte length before the later Eq reads any payload.
    let count = inventory
        .nodes
        .len()
        .checked_add(inventory.inline_records.len())
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    meter.charge_work(count)?;
    let mut cost = raw_work(&product.raw)?;
    add(&mut cost, size_of::<MarkdownProduct>())?;
    add(&mut cost, product.residency.as_str().len())?;
    array_work(&mut cost, inventory.nodes.len(), size_of::<MarkdownNode>())?;
    array_work(
        &mut cost,
        inventory.inline_records.len(),
        size_of::<InlineRecord>(),
    )?;
    for node in &inventory.nodes {
        meter.poll()?;
        if let Some(value) = &node.code_info {
            add(&mut cost, value.value.len())?;
        }
    }
    for record in &inventory.inline_records {
        meter.poll()?;
        for value in [&record.text, &record.destination, &record.title]
            .into_iter()
            .flatten()
        {
            add(&mut cost, value.value.len())?;
        }
    }
    Ok(cost)
}
