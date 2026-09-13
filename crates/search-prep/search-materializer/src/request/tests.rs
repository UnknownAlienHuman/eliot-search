use super::*;
use crate::MaterializationError;
use crate::profile::{
    SourceEncoding, SourceKind, ValidatedMaterializerProfile, baseline_profile_descriptor,
    validate_materializer_profile,
};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

fn profile() -> ValidatedMaterializerProfile {
    validate_materializer_profile(&baseline_profile_descriptor("request-test", 1))
        .expect("profile")
}

fn request(profile: &ValidatedMaterializerProfile) -> MaterializationRequest {
    MaterializationRequest {
        source_id: OpaqueId::new("source:test").expect("source"),
        revision: NonZeroRevision::new(1).expect("revision"),
        residency: OpaqueId::new("residency:test").expect("residency"),
        content_digest: Blake3Digest32::from_bytes([7; 32]),
        byte_count: 4,
        declared_kind: SourceKind::Text,
        declared_encoding: SourceEncoding::Utf8,
        profile_id: profile.id(),
        operation_id: OpaqueId::new("operation:test").expect("operation"),
        from_unsaved_bytes: false,
        unsaved_snapshot_receipt: None,
    }
}

#[test]
fn valid_request_binds_profile() {
    let profile = profile();
    let accepted = AcceptedProfiles::new(vec![profile.clone()]);
    let validated = validate_materialization_request(
        &request(&profile),
        &accepted,
        &DEFAULT_MATERIALIZATION_BUDGET,
    )
    .expect("valid");
    assert_eq!(validated.profile().id(), profile.id());
    assert!(!validated.admitted_unsaved());
}

#[test]
fn unknown_profile_is_mismatch() {
    let profile = profile();
    let other =
        validate_materializer_profile(&baseline_profile_descriptor("other", 1)).expect("other");
    let accepted = AcceptedProfiles::new(vec![other]);
    assert_eq!(
        validate_materialization_request(
            &request(&profile),
            &accepted,
            &DEFAULT_MATERIALIZATION_BUDGET
        ),
        Err(MaterializationError::ProfileMismatch)
    );
}

#[test]
fn unsupported_kind_and_encoding_are_typed() {
    let mut narrow = baseline_profile_descriptor("narrow", 1);
    narrow.source_kinds = vec![SourceKind::Text];
    let narrow = validate_materializer_profile(&narrow).expect("narrow");
    let accepted_narrow = AcceptedProfiles::new(vec![narrow.clone()]);
    let mut kind_input = request(&narrow);
    kind_input.declared_kind = SourceKind::Code;
    assert_eq!(
        validate_materialization_request(
            &kind_input,
            &accepted_narrow,
            &DEFAULT_MATERIALIZATION_BUDGET
        ),
        Err(MaterializationError::Unsupported)
    );
    let mut limited = baseline_profile_descriptor("limited", 1);
    limited.encodings = vec![SourceEncoding::Utf8];
    let limited = validate_materializer_profile(&limited).expect("limited");
    let accepted_limited = AcceptedProfiles::new(vec![limited.clone()]);
    let mut limited_input = request(&limited);
    limited_input.declared_encoding = SourceEncoding::Utf16Be;
    assert_eq!(
        validate_materialization_request(
            &limited_input,
            &accepted_limited,
            &DEFAULT_MATERIALIZATION_BUDGET
        ),
        Err(MaterializationError::EncodingUnsupported)
    );
}

#[test]
fn zero_bytes_are_invalid_and_oversize_is_budget() {
    let profile = profile();
    let accepted = AcceptedProfiles::new(vec![profile.clone()]);
    let mut input = request(&profile);
    input.byte_count = 0;
    assert_eq!(
        validate_materialization_request(&input, &accepted, &DEFAULT_MATERIALIZATION_BUDGET),
        Err(MaterializationError::RequestInvalid)
    );
    input.byte_count = DEFAULT_MATERIALIZATION_BUDGET.max_input_bytes + 1;
    assert_eq!(
        validate_materialization_request(&input, &accepted, &DEFAULT_MATERIALIZATION_BUDGET),
        Err(MaterializationError::BudgetExhausted)
    );
}

#[test]
fn unsaved_bytes_require_snapshot_receipt() {
    let profile = profile();
    let accepted = AcceptedProfiles::new(vec![profile.clone()]);
    let mut input = request(&profile);
    input.from_unsaved_bytes = true;
    assert_eq!(
        validate_materialization_request(&input, &accepted, &DEFAULT_MATERIALIZATION_BUDGET),
        Err(MaterializationError::UnsavedSnapshotNotAdmitted)
    );
    input.unsaved_snapshot_receipt =
        Some(ReceiptRef::new("receipt:snapshot").expect("receipt"));
    let validated =
        validate_materialization_request(&input, &accepted, &DEFAULT_MATERIALIZATION_BUDGET)
            .expect("admitted");
    assert!(validated.admitted_unsaved());
}

#[test]
fn invalid_budgets_fail_closed() {
    let profile = profile();
    let accepted = AcceptedProfiles::new(vec![profile.clone()]);
    let budget = MaterializationBudget {
        max_steps: 0,
        ..DEFAULT_MATERIALIZATION_BUDGET
    };
    assert_eq!(
        validate_materialization_request(&request(&profile), &accepted, &budget),
        Err(MaterializationError::InvalidLimits)
    );
}
