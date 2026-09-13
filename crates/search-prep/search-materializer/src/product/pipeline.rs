//! End-to-end bounded baseline materialization pipeline.

use super::digest::{
    digest_coordinate_map, digest_loss_map, digest_representation, warnings_for,
};
use super::model::{MaterializationContext, MaterializationProduct, ResourceReceipt};
use super::read::{RevisionReadPort, open_exact_revision};
use crate::MaterializationError;
use crate::assurance::derive_assurance;
use crate::decode::{StepCounter, decode_text_or_code, detect_or_validate_encoding};
use crate::maps::{
    MapBundle, MapIdentities, build_coordinate_map, build_loss_map, validate_map_bundle,
};
use crate::normalize::normalize_representation;
use crate::profile::digest32;
use crate::request::ValidatedMaterializationRequest;
use search_contracts::Blake3Digest32;

/// Runs exact revision open, encoding decision, decode, normalization, map
/// construction, assurance derivation and canonical digest generation.
///
/// Cancellation or budget exhaustion never returns a successful complete
/// representation. No durable publication occurs here.
pub fn materialize_text_or_code(
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    context: &MaterializationContext<'_>,
) -> Result<MaterializationProduct, MaterializationError> {
    let budget = context.budget.validate()?;
    if context.cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    if request.profile().id() != context.profile.id() {
        return Err(MaterializationError::ProfileMismatch);
    }
    let profile = &context.profile;
    let guard = open_exact_revision(request, port, context.cancel)?;
    let bytes = guard.into_bytes();
    let decision = detect_or_validate_encoding(&bytes, request.declared_encoding(), profile)?;
    let mut steps = StepCounter::new(budget.effective_steps(profile.limits().max_steps));
    let decoded = decode_text_or_code(
        &bytes,
        &decision,
        profile,
        &budget,
        &mut steps,
        context.cancel,
    )?;
    if context.cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    let canonical =
        normalize_representation(&decoded, profile, &budget, &mut steps, context.cancel)?;
    let identities = MapIdentities {
        source_id: request.source_id().clone(),
        revision: request.revision(),
        profile_id: profile.id(),
    };
    let coordinate_map = build_coordinate_map(
        &decoded,
        &canonical,
        &identities,
        profile,
        &budget,
        &mut steps,
        context.cancel,
    )?;
    let loss_map = build_loss_map(
        &decoded,
        &canonical,
        &identities,
        profile,
        &budget,
        &mut steps,
        context.cancel,
    )?;
    let bundle = MapBundle::from_parts(coordinate_map, loss_map);
    let warnings = warnings_for(&bundle, decision.encoding());
    let warnings_count =
        u64::try_from(warnings.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    let assurance = derive_assurance(&bundle, warnings_count, profile)?;
    validate_map_bundle(&canonical, &bundle, profile)?;
    let canonical_digest = Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/canonical/v1",
        &[canonical.text().as_bytes()],
    ));
    let coordinate_digest = digest_coordinate_map(&bundle);
    let loss_digest = digest_loss_map(&bundle);
    let representation_id = digest_representation(
        request,
        decision.encoding(),
        canonical.text(),
        &coordinate_digest,
        &loss_digest,
    );
    let segments = u64::try_from(bundle.coordinate_map().segments().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let loss_records = u64::try_from(bundle.loss_map().records().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let output_bytes =
        u64::try_from(canonical.text().len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    Ok(MaterializationProduct {
        representation_id,
        source_id: request.source_id().clone(),
        revision: request.revision(),
        profile_id: profile.id(),
        encoding: decision.encoding(),
        input_digest: request.content_digest(),
        canonical_digest,
        coordinate_digest,
        loss_digest,
        canonical,
        maps: bundle,
        assurance,
        warnings,
        resource: ResourceReceipt {
            input_bytes: request.byte_count(),
            output_bytes,
            steps_used: steps.used(),
            segments,
            loss_records,
        },
    })
}
