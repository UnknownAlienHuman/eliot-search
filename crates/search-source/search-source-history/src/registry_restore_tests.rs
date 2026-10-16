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
