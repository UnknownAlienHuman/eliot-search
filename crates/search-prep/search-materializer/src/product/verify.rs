//! Materialization verification and publication planning.

use super::digest::{digest_coordinate_map, digest_loss_map};
use super::model::{
    MaterializationAdmissionPlan, MaterializationProduct, MaterializationVerificationReceipt,
};
use crate::MaterializationError;
use crate::assurance::derive_assurance;
use crate::maps::validate_map_bundle;
use crate::profile::{ValidatedMaterializerProfile, digest32};
use crate::request::ValidatedMaterializationRequest;
use search_contracts::{Blake3Digest32, OpaqueId};

/// Recomputes identities and digests, validates maps and assurance, and
/// proves the output belongs to the exact source revision and profile.
pub fn verify_materialization(
    product: &MaterializationProduct,
    request: &ValidatedMaterializationRequest,
    profile: &ValidatedMaterializerProfile,
) -> Result<MaterializationVerificationReceipt, MaterializationError> {
    if profile.id() != request.profile().id() || profile.id() != product.profile_id() {
        return Err(MaterializationError::ProfileMismatch);
    }
    if request.content_digest() != product.input_digest() {
        return Err(MaterializationError::RevisionDigestMismatch);
    }
    if request.revision() != product.revision() || *request.source_id() != *product.source_id() {
        return Err(MaterializationError::RevisionDigestMismatch);
    }
    let canonical_digest = Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/canonical/v1",
        &[product.canonical_text().as_bytes()],
    ));
    if canonical_digest != product.canonical_digest() {
        return Err(MaterializationError::RevisionDigestMismatch);
    }
    if digest_coordinate_map(product.maps()) != product.coordinate_digest()
        || digest_loss_map(product.maps()) != product.loss_digest()
    {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    let receipt = validate_map_bundle(product.canonical(), product.maps(), profile)?;
    let warnings = u64::try_from(product.warnings().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let assurance = derive_assurance(product.maps(), warnings, profile)?;
    if assurance.ceiling() != product.assurance().ceiling() {
        return Err(MaterializationError::AssuranceViolation);
    }
    Ok(MaterializationVerificationReceipt {
        representation_id: product.representation_id(),
        profile_id: product.profile_id(),
        coordinate_segments: receipt.coordinate_segments(),
        loss_records: receipt.loss_records(),
        assurance,
    })
}

/// Creates a content-addressed immutable publication plan without writing.
pub fn prepare_admission(
    product: &MaterializationProduct,
    operation_id: &OpaqueId,
    deadline_steps: u64,
) -> Result<MaterializationAdmissionPlan, MaterializationError> {
    if deadline_steps == 0 {
        return Err(MaterializationError::RequestInvalid);
    }
    Ok(MaterializationAdmissionPlan {
        representation_id: product.representation_id(),
        canonical_digest: product.canonical_digest(),
        profile_id: product.profile_id(),
        canonical_bytes_len: product.resource_receipt().output_bytes,
        operation_id: operation_id.clone(),
    })
}
