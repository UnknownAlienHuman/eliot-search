//! Mutating-ingestion composition of exact raw units, the code encoder and C13.
//!
//! This pure preparation step retains the actual encoded vectors for staging;
//! a manifest of vector digests alone cannot supply Qdrant's point values.
//! Membership/profile acceptance, immutable artifact storage and publication
//! remain with their existing owners. This does not authorize query-time work
//! or qualify a caller-supplied profile acceptance record.

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use search_contracts::indexed::{
    CollectionSchema, MAX_STORED_VECTOR_VALUES, VectorMode, VectorRequirement, VectorValue,
};
use search_contracts::{BoundedName, Modality, NonZeroRevision};
use search_lexical::{
    AcceptedCodeIdentifiersProfile, CodeEncodingError, CodeIdentifierLimits, LexicalInput,
    SparseLimits, encode_code_document,
};
use search_materializer::api::MaterializationProduct;
use search_projection_planner::{
    PlannedUnit, PlannedVector, PlanningScope, ProjectionBudget, ProjectionError, ProjectionPlan,
    plan_projection, validate_schema, validate_scope_binding,
};
use search_unitizer::{UnitDescriptor, V3RepresentationKind, VerifiedUnitSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CodeProjectionError {
    ContextMismatch,
    BudgetExceeded,
    Cancelled,
    DeadlineExceeded,
    Encoding(CodeEncodingError),
    Projection(ProjectionError),
}

fn check(cancelled: &AtomicBool, deadline: Instant) -> Result<(), CodeProjectionError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(CodeProjectionError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(CodeProjectionError::DeadlineExceeded);
    }
    Ok(())
}

fn verify_product_binding(
    product: &MaterializationProduct,
    units: &VerifiedUnitSet,
    schema: &CollectionSchema,
) -> Result<(), CodeProjectionError> {
    let provenance = units.manifest().provenance();
    if units.manifest().profile().representation_kind != V3RepresentationKind::Code
        || provenance.commitment() != product.representation_id()
        || provenance.input_digest() != product.input_digest()
        || provenance.canonical_digest() != product.canonical_digest()
        || provenance.coordinate_digest() != product.coordinate_digest()
        || provenance.loss_digest() != product.loss_digest()
        || provenance.binding().source_id.to_string() != product.source_id().as_str()
        || provenance.legacy_revision_sequence() != product.revision().get()
        || schema.mode() != VectorMode::SparseOnly
        || schema.vectors().len() != 1
        || schema.validate().is_err()
    {
        return Err(CodeProjectionError::ContextMismatch);
    }
    Ok(())
}

fn lexical_unit_input(
    product: &MaterializationProduct,
    revision: NonZeroRevision,
    unit: &UnitDescriptor,
) -> Result<LexicalInput, CodeProjectionError> {
    let start =
        usize::try_from(unit.source_start()).map_err(|_| CodeProjectionError::ContextMismatch)?;
    let end =
        usize::try_from(unit.source_end()).map_err(|_| CodeProjectionError::ContextMismatch)?;
    let text = product
        .canonical_text()
        .get(start..end)
        .ok_or(CodeProjectionError::ContextMismatch)?;
    Ok(LexicalInput::new(
        product.source_id().clone(),
        revision,
        unit.occurrence().ordinal,
        unit.source_start(),
        unit.source_end(),
        text.to_owned(),
    ))
}

/// Encode every owner-verified raw code unit and build one exact point plan.
///
/// The caller supplies the admitted generation's exact schema and accepted
/// lexical profile. Neither is inferred from a locator, a payload or a digest.
/// The returned units retain their real vectors for the canonical bridge's
/// stage operation; no synthetic vector or field value is introduced.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_raw_code_projection(
    product: &MaterializationProduct,
    units: &VerifiedUnitSet,
    scope: &PlanningScope,
    schema: &CollectionSchema,
    profile: &AcceptedCodeIdentifiersProfile,
    language_or_format: &BoundedName,
    lexical_limits: CodeIdentifierLimits,
    sparse_limits: SparseLimits,
    projection_budget: ProjectionBudget,
    max_input_bytes: usize,
    max_total_vector_values: usize,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Result<(ProjectionPlan, Vec<PlannedUnit>), CodeProjectionError> {
    check(cancelled, deadline)?;
    validate_scope_binding(units, scope, projection_budget)
        .map_err(CodeProjectionError::Projection)?;
    verify_product_binding(product, units, schema)?;
    let provenance = units.manifest().provenance();
    if max_input_bytes == 0
        || max_total_vector_values == 0
        || product.canonical_text().len() > max_input_bytes
    {
        return Err(CodeProjectionError::BudgetExceeded);
    }
    let requirement = &schema.vectors()[0];
    if !matches!(
        requirement.requirement(),
        VectorRequirement::Sparse {
            idf_enabled: true,
            ..
        }
    ) {
        return Err(CodeProjectionError::ContextMismatch);
    }
    let requirements = BTreeMap::from([(
        requirement.name().as_str().to_owned(),
        requirement.requirement(),
    )]);
    let revision = NonZeroRevision::new(provenance.legacy_revision_sequence())
        .map_err(|_| CodeProjectionError::ContextMismatch)?;
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(units.units().len())
        .map_err(|_| CodeProjectionError::BudgetExceeded)?;
    let mut total_values = 0_usize;
    let stored_values_limit = usize::try_from(MAX_STORED_VECTOR_VALUES)
        .map_err(|_| CodeProjectionError::BudgetExceeded)?;
    for unit in units.units() {
        check(cancelled, deadline)?;
        let input = lexical_unit_input(product, revision, unit)?;
        let remaining_values = max_total_vector_values
            .checked_sub(total_values)
            .filter(|remaining| *remaining > 0)
            .ok_or(CodeProjectionError::BudgetExceeded)?;
        let sparse_limits = SparseLimits {
            max_vector_values: sparse_limits
                .max_vector_values
                .min(remaining_values)
                .min(stored_values_limit),
            ..sparse_limits
        };
        let encoded = encode_code_document(
            &input,
            profile,
            lexical_limits,
            sparse_limits,
            cancelled.load(Ordering::Acquire),
        )
        .map_err(CodeProjectionError::Encoding)?;
        check(cancelled, deadline)?;
        let vector = &encoded.encoding().vector;
        total_values = total_values
            .checked_add(vector.indices.len())
            .filter(|total| *total <= max_total_vector_values)
            .ok_or(CodeProjectionError::BudgetExceeded)?;
        let value = VectorValue::Sparse {
            indices: vector.indices.clone(),
            values: vector.values.clone(),
        };
        value
            .validate(&requirement.requirement())
            .map_err(|_| CodeProjectionError::ContextMismatch)?;
        prepared.push(PlannedUnit {
            unit_id: unit.unit_id(),
            unit_kind: unit.occurrence().unit_kind,
            modality: Modality::Code,
            language_or_format: language_or_format.clone(),
            entity_kind: None,
            normalized_symbol_key: None,
            unit_digest: unit.unit_content_digest(),
            reference_digest: unit.reference_digest(),
            vectors: vec![PlannedVector {
                name: requirement.name().clone(),
                value,
            }],
        });
    }
    check(cancelled, deadline)?;
    let plan = plan_projection(units, scope, &prepared, &requirements, projection_budget)
        .map_err(CodeProjectionError::Projection)?;
    validate_schema(schema, &plan, projection_budget).map_err(CodeProjectionError::Projection)?;
    check(cancelled, deadline)?;
    Ok((plan, prepared))
}
