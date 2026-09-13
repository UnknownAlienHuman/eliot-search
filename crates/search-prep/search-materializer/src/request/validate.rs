//! Request validation against accepted profiles and finite budgets.

use super::budget::MaterializationBudget;
use super::model::{
    AcceptedProfiles, MaterializationRequest, ValidatedMaterializationRequest,
};
use crate::MaterializationError;

/// Validates a materialization request against accepted profiles and budgets.
///
/// Unsaved bytes are rejected unless they carry an explicit authenticated
/// durable snapshot-admission receipt. A path or current file cannot
/// substitute for the revision: the request type cannot express one.
pub fn validate_materialization_request(
    request: &MaterializationRequest,
    accepted: &AcceptedProfiles,
    budget: &MaterializationBudget,
) -> Result<ValidatedMaterializationRequest, MaterializationError> {
    let budget = budget.validate()?;
    let Some(profile) = accepted.find(&request.profile_id) else {
        return Err(MaterializationError::ProfileMismatch);
    };
    if !profile.source_kinds().contains(&request.declared_kind) {
        return Err(MaterializationError::Unsupported);
    }
    if !profile.encodings().contains(&request.declared_encoding) {
        return Err(MaterializationError::EncodingUnsupported);
    }
    if request.byte_count == 0 {
        return Err(MaterializationError::RequestInvalid);
    }
    let max_input = budget.effective_input(profile.limits().max_input_bytes);
    if request.byte_count > max_input {
        return Err(MaterializationError::BudgetExhausted);
    }
    if request.from_unsaved_bytes && request.unsaved_snapshot_receipt.is_none() {
        return Err(MaterializationError::UnsavedSnapshotNotAdmitted);
    }
    Ok(ValidatedMaterializationRequest {
        source_id: request.source_id.clone(),
        revision: request.revision,
        residency: request.residency.clone(),
        content_digest: request.content_digest,
        byte_count: request.byte_count,
        declared_kind: request.declared_kind,
        declared_encoding: request.declared_encoding,
        profile: profile.clone(),
        operation_id: request.operation_id.clone(),
        admitted_unsaved: request.from_unsaved_bytes,
    })
}
