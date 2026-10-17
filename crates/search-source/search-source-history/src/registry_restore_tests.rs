use super::registry_restore::{
    MAX_REGISTRY_RESTORE_BYTES, RegistryRestoreOperation, RegistryRestoreState,
    restore_registry_state, restore_registry_state_v4, validate_legacy_membership_generation,
};
use search_contracts::{
    Blake3Digest32, InstallationIncarnationId, NamespaceOwnershipStatus, NonZeroRevision, OpaqueId,
    OwnerEpoch, PolicyRevision, ReceiptRef, RootBindingId, SourceNamespaceId,
    SourceNamespaceOwnership, SourceOwnerGeneration,
};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_provenance::{OriginalAdmissionV2, RetainedPolicyV2};
use search_source_records::batch::{
    BatchCodecV4Error, DecodedBatch, OriginalInputResolver, RegistryStateDelta, RegistryStateKey,
    RegistryStateRecord,
};
use search_source_records::error::{DEFAULT_REGISTRY_LIMITS, JournalEntryKind, RegistryError};
use search_source_records::namespace::NamespaceCutoverState;
use search_source_records::root::{RootRecord, RootStatus};
use std::sync::Arc;

struct NoopResolver;

#[test]
fn v6_projection_batch_profile_survives_history_replay() {
    use search_contracts as c;
    use search_source_records::{
        batch::request_fingerprint,
        membership::PermanentMembershipProvenanceV2,
        operation::{RegistryBatch, RegistryChange, RegistryOperation},
        projection_membership::{ProjectionMembershipBindRequest, ProjectionMembershipRecord},
    };
    let rev = |value| NonZeroRevision::new(value).unwrap();
    let identity = c::SourceIdentity {
        source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
        source_id: c::SourceId::from_bytes([2; 16]),
        identity_kind: c::SourceIdentityKind::NtfsFile,
        stable_identity_components: c::OpaqueCanonicalBytes::from_validated(b"history-v6".to_vec()).unwrap(),
    };
    // Synthetic technical observations only. C03 separately requires the real
    // active member/source/root/namespace before this can become usable state.
    let request = ProjectionMembershipBindRequest {
        binding: c::ProjectionMembership {
            projection_membership_id: c::ProjectionMembershipId::from_bytes([3; 16]),
            source_membership_id: c::SourceMembershipId::from_bytes([4; 16]),
            representation_id: c::RepresentationId::from_bytes([5; 16]),
            access_partition_id: c::AccessPartitionId::from_bytes([6; 16]),
            scoring_partition_id: c::ScoringPartitionId::from_bytes([7; 16]),
            projection_schema_id: c::ProfileId::new("history-v6-profile").unwrap(),
        },
        expected_membership_revision: c::MembershipRevision::new(1),
        expected_provenance: PermanentMembershipProvenanceV2::from_stored_parts(
            identity.clone(), rev(1), RootBindingId::from_bytes([8; 16]), rev(1),
            identity.source_namespace_id, rev(1), SourceOwnerGeneration::from_bytes([9; 32]),
            b"policy-observation".to_vec(), b"admission-observation".to_vec(), rev(1),
        ).unwrap(),
        partition_semantics: None,
    };
    let operation_id = OpaqueId::new("history-v6-operation").unwrap();
    let receipt = ReceiptRef::new("history-v6-correlation").unwrap();
    let mutation = Blake3Digest32::from_bytes([10; 32]);
    let state = RegistryStateRecord::ProjectionMembership(
        ProjectionMembershipRecord::from_observed_parts(request.clone(), 1, receipt.clone()).unwrap(),
    );
    let key = state.key();
    let batch = RegistryBatch {
        expected_registry_revision: 0,
        operation: RegistryOperation::new(operation_id.clone(), mutation),
        changes: vec![RegistryChange::BindProjectionMembership { request, receipt: receipt.clone() }],
    };
    let operation = DecodedBatch::from_observed_parts_v6(
        search_source_records::error::RegistryJournalEntry::new(operation_id.clone(), mutation,
            0, 1, JournalEntryKind::BatchApply, receipt),
        request_fingerprint(&batch).unwrap(),
        vec![RegistryStateDelta::new(key.clone(), None, state.clone()).unwrap()],
        batch.changes, DEFAULT_REGISTRY_LIMITS,
    ).unwrap();
    let key_bytes = key.encode().unwrap();
    let state_bytes = state.encode().unwrap();
    let operation_bytes = operation.encode().unwrap();
    let restored = restore_registry_state(1,
        &[RegistryRestoreState { key: &key_bytes, value: &state_bytes }],
        &[RegistryRestoreOperation { operation_id: &operation_id, value: &operation_bytes }],
        DEFAULT_REGISTRY_LIMITS, MAX_REGISTRY_RESTORE_BYTES,
    ).unwrap().into_parts();
    assert_eq!(restored.operations, vec![operation.clone()]);
    assert_eq!(restored.operations[0].encode().unwrap(), operation_bytes);
    assert_eq!(restored.state_encoding_profiles.get(&key), Some(&"eliot.registry-state.v6"));
}

impl OriginalInputResolver for NoopResolver {
    fn resolve_policy(
        &mut self,
        _reference: &[u8],
        _probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Arc<RetainedPolicyV2>, BatchCodecV4Error> {
        Err(BatchCodecV4Error::Registry(RegistryError::SnapshotInvalid))
    }

    fn resolve_admission(
        &mut self,
        _reference: &[u8],
        _policy: Arc<RetainedPolicyV2>,
        _probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<OriginalAdmissionV2, BatchCodecV4Error> {
        Err(BatchCodecV4Error::Registry(RegistryError::SnapshotInvalid))
    }
}

fn root() -> RootRecord {
    RootRecord {
        root_binding_id: RootBindingId::from_bytes([1; 16]),
        canonical_root_digest: Blake3Digest32::from_bytes([2; 32]),
        policy_fingerprint: Blake3Digest32::from_bytes([3; 32]),
        policy_revision: NonZeroRevision::new(1).expect("policy revision"),
        owner_epoch: OwnerEpoch::new(1).expect("owner epoch"),
        record_revision: NonZeroRevision::new(1).expect("record revision"),
        registry_revision: 1,
        status: RootStatus::Active,
        last_receipt: ReceiptRef::new("restore:v4:root").expect("receipt"),
    }
}

fn ownership() -> SourceNamespaceOwnership {
    SourceNamespaceOwnership {
        source_namespace_id: SourceNamespaceId::from_bytes([4; 16]),
        owner_system_id: OpaqueId::new("legacy-namespace-owner").expect("owner"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([5; 16]),
        owner_epoch: OwnerEpoch::new(1).expect("owner epoch"),
        ownership_record_revision: NonZeroRevision::new(1).expect("record revision"),
        source_owner_generation: SourceOwnerGeneration::from_bytes([6; 32]),
        source_admission_policy_revision: PolicyRevision::new(1),
        status: NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: None,
    }
}

fn v4_batch(state: RegistryStateRecord) -> (Vec<u8>, Vec<u8>, OpaqueId) {
    let key = state.key();
    let delta = RegistryStateDelta::new(key.clone(), None, state).expect("delta");
    let operation_id = OpaqueId::new("restore:v4:operation").expect("operation");
    let operation = DecodedBatch::from_observed_parts_v4(
        search_source_records::error::RegistryJournalEntry::new(
            operation_id.clone(),
            Blake3Digest32::from_bytes([7; 32]),
            0,
            1,
            JournalEntryKind::BatchApply,
            ReceiptRef::new("restore:v4:operation").expect("receipt"),
        ),
        Blake3Digest32::from_bytes([8; 32]),
        1,
        vec![delta],
        DEFAULT_REGISTRY_LIMITS,
        &mut NoopResolver,
        &mut || Ok(()),
    )
    .expect("v4 operation");
    (
        key.encode().expect("key"),
        operation.encode().expect("operation"),
        operation_id,
    )
}

#[test]
fn v4_restore_forwards_probe_and_legacy_factory_refuses_v4_bytes() {
    let state = RegistryStateRecord::Root(root());
    let (key, operation, operation_id) = v4_batch(state.clone());
    let value = state
        .encode_with_profile("eliot.registry-state.v4")
        .expect("state");
    let mut resolver = NoopResolver;
    let mut probe_calls = 0;
    let restored = restore_registry_state_v4(
        1,
        &[RegistryRestoreState {
            key: &key,
            value: &value,
        }],
        &[RegistryRestoreOperation {
            operation_id: &operation_id,
            value: &operation,
        }],
        DEFAULT_REGISTRY_LIMITS,
        MAX_REGISTRY_RESTORE_BYTES,
        &mut resolver,
        &mut || {
            probe_calls += 1;
            Ok(())
        },
    )
    .expect("v4 restore");
    let parts = restored.into_parts();
    assert!(probe_calls > 0);
    assert_eq!(parts.roots.get(&root().root_binding_id), Some(&root()));
    assert_eq!(
        parts.operations[0].encoding_profile(),
        "eliot.registry-batch.v4"
    );
    assert_eq!(
        parts
            .state_encoding_profiles
            .get(&RegistryStateKey::Root(root().root_binding_id)),
        Some(&"eliot.registry-state.v4")
    );
    assert!(matches!(
        restore_registry_state(
            1,
            &[RegistryRestoreState {
                key: &key,
                value: &value
            }],
            &[RegistryRestoreOperation {
                operation_id: &operation_id,
                value: &operation
            }],
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES,
        ),
        Err(RegistryError::SnapshotInvalid)
    ));
}

#[test]
fn v4_restore_keeps_legacy_namespace_unbound_and_unrestorable() {
    let state = RegistryStateRecord::Namespace(NamespaceCutoverState::new(ownership()));
    let (key, operation, operation_id) = v4_batch(state);
    let value = RegistryStateRecord::Namespace(NamespaceCutoverState::new(ownership()))
        .encode_with_profile("eliot.registry-state.v4")
        .expect("state");
    let mut resolver = NoopResolver;
    let result = restore_registry_state_v4(
        1,
        &[RegistryRestoreState {
            key: &key,
            value: &value,
        }],
        &[RegistryRestoreOperation {
            operation_id: &operation_id,
            value: &operation,
        }],
        DEFAULT_REGISTRY_LIMITS,
        MAX_REGISTRY_RESTORE_BYTES,
        &mut resolver,
        &mut || Ok(()),
    );
    assert!(matches!(
        result,
        Err(BatchCodecV4Error::Registry(RegistryError::SnapshotInvalid))
    ));
}

fn legacy_root_observation() -> (Vec<u8>, Vec<u8>, Vec<u8>, OpaqueId) {
    let state = RegistryStateRecord::Root(root());
    let key = state.key().encode().expect("legacy key");
    let value = state.encode().expect("legacy value");
    let operation_id = OpaqueId::new("restore:legacy:operation").expect("operation");
    let batch = DecodedBatch::from_observed_parts(
        "eliot.registry-batch.v3",
        "eliot.registry-state.v3",
        search_source_records::error::RegistryJournalEntry::new(
            operation_id.clone(), Blake3Digest32::from_bytes([7; 32]), 0, 1,
            JournalEntryKind::BatchApply, root().last_receipt,
        ),
        Blake3Digest32::from_bytes([8; 32]),
        1,
        vec![RegistryStateDelta::new(state.key(), None, state).expect("delta")],
        DEFAULT_REGISTRY_LIMITS,
    ).expect("legacy batch");
    (key, value, batch.encode().expect("legacy operation"), operation_id)
}

#[test]
fn v4_restore_preserves_legacy_profiles_and_original_cancellation() {
    let (key, value, operation, operation_id) = legacy_root_observation();
    let states = [RegistryRestoreState { key: &key, value: &value }];
    let operations = [RegistryRestoreOperation { operation_id: &operation_id, value: &operation }];
    let restored = restore_registry_state_v4(
        1, &states, &operations, DEFAULT_REGISTRY_LIMITS, MAX_REGISTRY_RESTORE_BYTES,
        &mut NoopResolver, &mut || Ok(()),
    ).expect("legacy members of the context-aware history").into_parts();
    assert_eq!(restored.operations[0].encoding_profile(), "eliot.registry-batch.v3");
    assert_eq!(restored.state_encoding_profiles.get(&RegistryStateKey::Root(root().root_binding_id)), Some(&"eliot.registry-state.v3"));
    let cancelled = restore_registry_state_v4(
        1, &states, &operations, DEFAULT_REGISTRY_LIMITS, MAX_REGISTRY_RESTORE_BYTES,
        &mut NoopResolver, &mut || Err(CheckpointFailureV2::Cancelled),
    );
    assert!(matches!(cancelled, Err(BatchCodecV4Error::Admission(search_source_admission::AdmissionError::Cancelled))));
}

#[test]
fn legacy_membership_requires_its_exact_corpus_generation() {
    use search_source_records::membership::MembershipLifecycle;

    let corpus = OpaqueId::new("restore:legacy:corpus").expect("corpus");
    let foreign_corpus = OpaqueId::new("restore:foreign:corpus").expect("foreign corpus");
    let revision = NonZeroRevision::new(1).expect("generation");
    let empty = std::collections::BTreeMap::new();
    assert!(matches!(
        validate_legacy_membership_generation(
            &corpus,
            revision,
            MembershipLifecycle::Active,
            &empty,
        ),
        Err(RegistryError::CutoverGenerationConflict)
    ));

    let foreign = std::collections::BTreeMap::from([(foreign_corpus, revision)]);
    assert!(matches!(
        validate_legacy_membership_generation(
            &corpus,
            revision,
            MembershipLifecycle::Active,
            &foreign,
        ),
        Err(RegistryError::CutoverGenerationConflict)
    ));

    let exact = std::collections::BTreeMap::from([(corpus.clone(), revision)]);
    assert_eq!(
        validate_legacy_membership_generation(
            &corpus,
            revision,
            MembershipLifecycle::Active,
            &exact,
        ),
        Ok(())
    );
}

#[test]
fn legacy_state_and_operation_keep_the_probe_after_their_entry_checkpoint() {
    use search_source_records::batch::{
        decode_batch_record_with_original_probe,
        decode_registry_state_entry_profiled_with_original_probe,
    };
    let (key, value, operation, _) = legacy_root_observation();
    let mut entered = false;
    let state = decode_registry_state_entry_profiled_with_original_probe(&key, &value, &mut || {
        if std::mem::replace(&mut entered, true) { Err(CheckpointFailureV2::DeadlineExceeded) } else { Ok(()) }
    });
    assert!(matches!(state, Err(BatchCodecV4Error::Admission(search_source_admission::AdmissionError::BudgetExhausted))));
    let mut entered = false;
    let batch = decode_batch_record_with_original_probe(&operation, DEFAULT_REGISTRY_LIMITS, &mut || {
        if std::mem::replace(&mut entered, true) { Err(CheckpointFailureV2::DeadlineExceeded) } else { Ok(()) }
    });
    assert!(matches!(batch, Err(BatchCodecV4Error::Admission(search_source_admission::AdmissionError::BudgetExhausted))));
}
#[test]
fn v7_projection_partition_classes_survive_full_history_restore_and_conflicts_refuse() {
    use search_contracts as c;
    use search_source_records::{
        batch::{request_fingerprint, DecodedBatch, RegistryStateDelta, RegistryStateKey},
        membership::PermanentMembershipProvenanceV2,
        operation::{RegistryBatch, RegistryChange, RegistryOperation},
        projection_membership::{
            ProjectionMembershipBindRequest, ProjectionMembershipRecord, ProjectionPartitionSemantics,
        },
    };

    fn canonical(value: &str) -> Vec<u8> {
        c::to_canonical_cbor(&c::CanonicalValue::Text(
            c::CanonicalText::new(value).expect("text"),
        ))
        .expect("canonical bytes")
        .into_vec()
    }

    fn projection_operation(
        index: u8,
        before: u64,
        access_id: u8,
        scoring_id: u8,
        access_meaning: &str,
        scoring_meaning: &str,
    ) -> (RegistryStateKey, RegistryStateRecord, DecodedBatch) {
        let revision = NonZeroRevision::new(1).expect("revision");
        let namespace = SourceNamespaceId::from_bytes([1; 16]);
        let source = c::SourceId::from_bytes([index; 16]);
        let identity = c::SourceIdentity {
            source_namespace_id: namespace,
            source_id: source,
            identity_kind: c::SourceIdentityKind::NtfsFile,
            stable_identity_components: c::OpaqueCanonicalBytes::from_validated(vec![index])
                .expect("identity"),
        };
        let provenance = PermanentMembershipProvenanceV2::from_stored_parts(
            identity,
            revision,
            RootBindingId::from_bytes([8; 16]),
            revision,
            namespace,
            revision,
            SourceOwnerGeneration::from_bytes([9; 32]),
            b"policy".to_vec(),
            b"admission".to_vec(),
            revision,
        )
        .expect("provenance");
        let receipt = ReceiptRef::new(format!("restore:v7:{index}")).expect("receipt");
        let request = ProjectionMembershipBindRequest {
            binding: c::ProjectionMembership {
                projection_membership_id: c::ProjectionMembershipId::from_bytes([index + 20; 16]),
                source_membership_id: c::SourceMembershipId::from_bytes([index + 30; 16]),
                representation_id: c::RepresentationId::from_bytes([index + 40; 16]),
                access_partition_id: c::AccessPartitionId::from_bytes([access_id; 16]),
                scoring_partition_id: c::ScoringPartitionId::from_bytes([scoring_id; 16]),
                projection_schema_id: c::ProfileId::new("restore-v7-schema").expect("schema"),
            },
            expected_membership_revision: c::MembershipRevision::new(1),
            expected_provenance: provenance,
            partition_semantics: Some(
                ProjectionPartitionSemantics::new(
                    canonical(access_meaning),
                    canonical(scoring_meaning),
                )
                .expect("partition semantics"),
            ),
        };
        let state = RegistryStateRecord::ProjectionMembership(
            ProjectionMembershipRecord::from_observed_parts(
                request.clone(),
                before + 1,
                receipt.clone(),
            )
            .expect("projection record"),
        );
        let key = state.key();
        let operation_id = c::OpaqueId::new(format!("restore:v7:operation:{index}"))
            .expect("operation id");
        let mutation = Blake3Digest32::from_bytes([index + 60; 32]);
        let batch = RegistryBatch {
            expected_registry_revision: before,
            operation: RegistryOperation::new(operation_id.clone(), mutation),
            changes: vec![RegistryChange::BindProjectionMembership {
                request,
                receipt: receipt.clone(),
            }],
        };
        let operation = DecodedBatch::from_observed_parts_v7(
            search_source_records::error::RegistryJournalEntry::new(
                operation_id,
                mutation,
                before,
                before + 1,
                JournalEntryKind::BatchApply,
                receipt,
            ),
            request_fingerprint(&batch).expect("fingerprint"),
            vec![RegistryStateDelta::new(key.clone(), None, state.clone()).expect("delta")],
            batch.changes,
            DEFAULT_REGISTRY_LIMITS,
        )
        .expect("v7 operation");
        (key, state, operation)
    }

    fn restore_pair(
        first: (&RegistryStateKey, &RegistryStateRecord, &DecodedBatch),
        second: (&RegistryStateKey, &RegistryStateRecord, &DecodedBatch),
    ) -> Result<super::registry_restore::ObservedRegistryHistory, RegistryError> {
        let state_key_bytes = [
            first.0.encode().expect("first state key"),
            second.0.encode().expect("second state key"),
        ];
        let state_value_bytes = [
            first.1.encode_with_profile("eliot.registry-state.v7").expect("first v7 state"),
            second.1.encode_with_profile("eliot.registry-state.v7").expect("second v7 state"),
        ];
        let states = [
            RegistryRestoreState {
                key: &state_key_bytes[0],
                value: &state_value_bytes[0],
            },
            RegistryRestoreState {
                key: &state_key_bytes[1],
                value: &state_value_bytes[1],
            },
        ];
        let operation_bytes = [
            first.2.encode().expect("first operation"),
            second.2.encode().expect("second operation"),
        ];
        let operations = [
            RegistryRestoreOperation {
                operation_id: &first.2.entry().operation_id,
                value: &operation_bytes[0],
            },
            RegistryRestoreOperation {
                operation_id: &second.2.entry().operation_id,
                value: &operation_bytes[1],
            },
        ];
        restore_registry_state(
            2,
            &states,
            &operations,
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES,
        )
    }

    let first = projection_operation(1, 0, 6, 7, "access:read-v1", "scoring:code-v1");
    let second = projection_operation(2, 1, 6, 7, "access:read-v1", "scoring:code-v1");
    let restored = restore_pair(
        (&first.0, &first.1, &first.2),
        (&second.0, &second.1, &second.2),
    )
    .expect("shared canonical classes restore");
    let parts = restored.into_parts();
    assert_eq!(parts.operations.len(), 2);
    for (state, expected_access, expected_scoring) in [
        (&first.1, 6_u8, 7_u8),
        (&second.1, 6_u8, 7_u8),
    ] {
        let RegistryStateRecord::ProjectionMembership(record) = state else {
            panic!("projection state");
        };
        assert_eq!(
            record.binding().access_partition_id,
            c::AccessPartitionId::from_bytes([expected_access; 16])
        );
        assert_eq!(
            record.binding().scoring_partition_id,
            c::ScoringPartitionId::from_bytes([expected_scoring; 16])
        );
        assert_eq!(
            parts
                .state_encoding_profiles
                .get(&RegistryStateKey::ProjectionMembership(record.binding().source_membership_id)),
            Some(&"eliot.registry-state.v7")
        );
    }

    for (second_access_id, second_access_meaning) in [
        (6, "access:deny-v2"),
        (8, "access:read-v1"),
    ] {
        let second = projection_operation(
            2,
            1,
            second_access_id,
            7,
            second_access_meaning,
            "scoring:code-v1",
        );
        assert!(matches!(
            restore_pair(
                (&first.0, &first.1, &first.2),
                (&second.0, &second.1, &second.2),
            ),
            Err(RegistryError::ProjectionMembershipConflict)
        ));
    }
}
