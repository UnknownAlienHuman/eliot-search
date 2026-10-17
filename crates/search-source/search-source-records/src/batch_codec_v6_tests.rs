use super::*;
use crate::projection_membership::{ProjectionMembershipBindRequest, ProjectionMembershipRecord};

#[test]
fn projection_v6_retains_actual_ids_and_refuses_conflicting_record_or_transition() {
    let rev = |n| NonZeroRevision::new(n).unwrap();
    let identity = c::SourceIdentity {
        source_namespace_id: c::SourceNamespaceId::from_bytes([1; 16]),
        source_id: c::SourceId::from_bytes([2; 16]),
        identity_kind: c::SourceIdentityKind::NtfsFile,
        stable_identity_components: c::OpaqueCanonicalBytes::from_validated(b"v6-fixture".to_vec())
            .unwrap(),
    };
    let provenance = crate::membership::PermanentMembershipProvenanceV2::from_stored_parts(
        identity.clone(),
        rev(2),
        c::RootBindingId::from_bytes([3; 16]),
        rev(1),
        identity.source_namespace_id,
        rev(1),
        c::SourceOwnerGeneration::from_bytes([4; 32]),
        b"policy".to_vec(),
        b"admission".to_vec(),
        rev(1),
    )
    .unwrap();
    let request = ProjectionMembershipBindRequest {
        binding: c::ProjectionMembership {
            projection_membership_id: c::ProjectionMembershipId::from_bytes([
                0x52, 0x83, 0x69, 0x12, 0x42, 0x21, 0x49, 0x08, 0x93, 0x23, 0x44, 0x54, 0x65, 0x76,
                0x87, 0x98,
            ]),
            source_membership_id: c::SourceMembershipId::from_bytes([5; 16]),
            representation_id: c::RepresentationId::from_bytes([6; 16]),
            access_partition_id: c::AccessPartitionId::from_bytes([7; 16]),
            scoring_partition_id: c::ScoringPartitionId::from_bytes([8; 16]),
            projection_schema_id: c::ProfileId::new("projection:raw:v1").unwrap(),
        },
        expected_membership_revision: c::MembershipRevision::new(1),
        expected_provenance: provenance,
    };
    let receipt = ReceiptRef::new("receipt:projection:v6").unwrap();
    let state = RegistryStateRecord::ProjectionMembership(
        ProjectionMembershipRecord::from_observed_parts(request.clone(), 3, receipt.clone())
            .unwrap(),
    );
    let key = state.key();
    let key_bytes = key.encode().unwrap();
    let state_bytes = state.encode().unwrap();
    assert_eq!(key.family(), crate::batch::RegistryStateFamily::Membership);
    assert_eq!(
        crate::batch::decode_registry_state_entry(&key_bytes, &state_bytes).unwrap(),
        state
    );
    assert!(state.encode_with_profile(STATE_PROFILE_V5).is_err());
    assert!(state.encode_with_profile(STATE_PROFILE_V4).is_err());
    assert!(encode_key(&key).is_err());
    let wrong_key =
        RegistryStateKey::ProjectionMembership(c::SourceMembershipId::from_bytes([9; 16]));
    assert!(
        crate::batch::decode_registry_state_entry(&wrong_key.encode().unwrap(), &state_bytes)
            .is_err()
    );
    let batch = RegistryBatch {
        expected_registry_revision: 2,
        operation: crate::operation::RegistryOperation::new(
            OpaqueId::new("projection:operation").unwrap(),
            Blake3Digest32::from_stored_bytes([10; 32]),
        ),
        changes: vec![RegistryChange::BindProjectionMembership {
            request: request.clone(),
            receipt: receipt.clone(),
        }],
    };
    let entry = RegistryJournalEntry::new(
        batch.operation.operation_id().clone(),
        batch.operation.mutation_digest(),
        2,
        3,
        JournalEntryKind::BatchApply,
        receipt,
    );
    let fingerprint = request_fingerprint(&batch).unwrap();
    let record = DecodedBatch::from_observed_parts_v6(
        entry.clone(),
        fingerprint,
        vec![RegistryStateDelta::new(key.clone(), None, state.clone()).unwrap()],
        batch.changes.clone(),
        crate::error::DEFAULT_REGISTRY_LIMITS,
    )
    .unwrap();
    let encoded = record.encode().unwrap();
    let restored = decode_batch_record(&encoded, crate::error::DEFAULT_REGISTRY_LIMITS).unwrap();
    assert_eq!(restored, record);
    assert_eq!(restored.replay_batch().unwrap(), batch);
    let mut changed = batch.clone();
    let RegistryChange::BindProjectionMembership { request: input, .. } = &mut changed.changes[0]
    else {
        unreachable!()
    };
    input.binding.projection_membership_id = c::ProjectionMembershipId::from_bytes([11; 16]);
    assert_ne!(request_fingerprint(&changed).unwrap(), fingerprint);
    assert!(
        DecodedBatch::from_observed_parts_v6(
            entry.clone(),
            fingerprint,
            vec![RegistryStateDelta::new(key.clone(), None, state.clone()).unwrap()],
            changed.changes,
            crate::error::DEFAULT_REGISTRY_LIMITS
        )
        .is_err()
    );
    assert!(
        DecodedBatch::from_observed_parts_v6(
            entry,
            fingerprint,
            vec![RegistryStateDelta::new(key, Some(state.clone()), state).unwrap()],
            batch.changes,
            crate::error::DEFAULT_REGISTRY_LIMITS
        )
        .is_err()
    );
}
