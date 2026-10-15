//! One-coherent-snapshot permanent-v2 registry restoration.

use super::original_inputs::OriginalInputsReadback;
use super::v4_readback::{OriginalV4Context, V4ReadbackError};
use search_contracts::{parse_canonical_cbor, CanonicalKey, CanonicalValue};
use search_control_redb::{RegistryOperationEnvelope, RegistryReadSnapshot};
use search_runtime_owner::OwnerError;
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_registry::error::RegistryError;
use search_source_registry::recovery::{
    decode_batch_record_v4, decode_batch_record_with_original_probe, decode_registry_state_v4, required_original_refs_from_batch_v4,
    required_original_refs_from_state_v4, restore_registry_state_v4, BatchCodecV4Error,
    DecodedBatch, ObservedRegistryRestore, OriginalInputResolver as OriginalInputResolverTrait,
    RegistryRestoreOperation, RegistryRestoreState,
};
use std::{cell::RefCell, collections::BTreeSet};

/// Raw v4/legacy profile shape discovered from one coherent registry snapshot.
pub(crate) struct SnapshotShape {
    references: Vec<Vec<u8>>,
}

impl SnapshotShape {
    pub(crate) fn references(&self) -> &[Vec<u8>] {
        &self.references
    }
}

/// Recognizes only an explicit v4 owner profile. Unknown, malformed and old
/// profiles stay on their legacy decoder and are never redispatched as v4.
pub(crate) fn is_v4_batch(bytes: &[u8]) -> bool {
    encoded_profile(bytes).as_deref() == Some("eliot.registry-batch.v4")
}

/// Recognizes only an explicit v4 state value profile.
pub(crate) fn is_v4_state(bytes: &[u8]) -> bool {
    encoded_profile(bytes).as_deref() == Some("eliot.registry-state.v4")
}

fn encoded_profile(bytes: &[u8]) -> Option<String> {
    let CanonicalValue::Object(fields) = parse_canonical_cbor(bytes).ok()? else {
        return None;
    };
    let profile = fields.get(&CanonicalKey::new("profile").ok()?)?;
    let CanonicalValue::Text(profile) = profile else {
        return None;
    };
    Some(profile.as_str().to_owned())
}

/// Extracts all original references from the raw operation/state rows.
pub(crate) fn collect_snapshot_refs(
    snapshot: &RegistryReadSnapshot,
    context: &OriginalV4Context<'_>,
) -> Result<SnapshotShape, V4ReadbackError> {
    let owner = RefCell::new(None);
    let mut probe = make_probe(context, &owner);
    let result = (|| {
        let mut has_v4 = false;
        let mut references = BTreeSet::new();
        for operation in snapshot.operations() {
            if is_v4_batch(operation.owner_bytes()) {
                has_v4 = true;
                for reference in
                    required_original_refs_from_batch_v4(operation.owner_bytes(), &mut probe)?
                {
                    references.insert(reference);
                }
            }
        }
        for state in snapshot.states() {
            if is_v4_state(state.canonical_bytes()) {
                has_v4 = true;
                for reference in
                    required_original_refs_from_state_v4(state.canonical_bytes(), &mut probe)?
                {
                    references.insert(reference);
                }
            }
        }
        if has_v4 && references.is_empty() {
            validate_without_external_originals(snapshot, context.registry_limits, &mut probe)?;
        }
        Ok(SnapshotShape {
            references: references.into_iter().collect(),
        })
    })();
    drop(probe);
    if let Some(error) = owner.into_inner() {
        return Err(V4ReadbackError::Owner(error));
    }
    result.map_err(V4ReadbackError::Batch)
}

/// Decodes one retained operation with the exact original caller context. The
/// caller has already read every required original reference for the whole
/// coherent snapshot, so this path performs no storage access of its own.
pub(crate) fn decode_operation_for_root(
    envelope: &RegistryOperationEnvelope,
    originals: Option<&OriginalInputsReadback>,
    context: &OriginalV4Context<'_>,
) -> Result<DecodedBatch, V4ReadbackError> {
    let mut resolver: Box<dyn OriginalInputResolverTrait> = match originals {
        Some(readback) => Box::new(readback.v4_resolver(&context.platform, &context.capabilities)),
        None => Box::new(EmptyOriginalResolver),
    };
    let owner = RefCell::new(None);
    let mut probe = make_probe(context, &owner);
    let result = if is_v4_batch(envelope.owner_bytes()) {
        decode_batch_record_v4(envelope.owner_bytes(), context.registry_limits, resolver.as_mut(), &mut probe)
    } else {
        decode_batch_record_with_original_probe(envelope.owner_bytes(), context.registry_limits, &mut probe)
    };
    drop(probe);
    if let Some(error) = owner.into_inner() {
        return Err(V4ReadbackError::Owner(error));
    }
    let decoded = result.map_err(V4ReadbackError::Batch)?;
    super::verify_operation_envelope(envelope, &decoded)
        .map_err(|error| V4ReadbackError::Batch(BatchCodecV4Error::Registry(error)))?;
    Ok(decoded)
}

fn validate_without_external_originals(
    snapshot: &RegistryReadSnapshot,
    registry_limits: search_source_registry::error::RegistryLimits,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<(), BatchCodecV4Error> {
    let mut resolver = EmptyOriginalResolver;
    for state in snapshot.states() {
        if !is_v4_state(state.canonical_bytes()) {
            continue;
        }
        decode_registry_state_v4(state.canonical_bytes(), &mut resolver, probe)?;
    }
    for operation in snapshot.operations() {
        if !is_v4_batch(operation.owner_bytes()) {
            continue;
        }
        decode_batch_record_v4(
            operation.owner_bytes(),
            registry_limits,
            &mut resolver,
            probe,
        )?;
    }
    Ok(())
}

/// Restore the complete bounded state/history through the existing v4 owner.
pub(crate) fn restore_snapshot_v4(
    snapshot: &RegistryReadSnapshot,
    originals: Option<&OriginalInputsReadback>,
    context: &OriginalV4Context<'_>,
    max_total_bytes: usize,
) -> Result<ObservedRegistryRestore, V4ReadbackError> {
    for operation in snapshot.operations() {
        decode_operation_for_root(operation, originals, context)?;
    }
    let states = snapshot
        .states()
        .iter()
        .map(|state| RegistryRestoreState {
            key: state.key().canonical_bytes(),
            value: state.canonical_bytes(),
        })
        .collect::<Vec<_>>();
    let operations = snapshot
        .operations()
        .iter()
        .map(|operation| RegistryRestoreOperation {
            operation_id: operation.operation_id(),
            value: operation.owner_bytes(),
        })
        .collect::<Vec<_>>();
    let mut resolver: Box<dyn OriginalInputResolverTrait> = match originals {
        Some(readback) => Box::new(readback.v4_resolver(&context.platform, &context.capabilities)),
        None => Box::new(EmptyOriginalResolver),
    };
    let owner = RefCell::new(None);
    let mut probe = make_probe(context, &owner);
    let result = restore_registry_state_v4(
        snapshot.registry_revision(),
        &states,
        &operations,
        context.registry_limits,
        max_total_bytes,
        resolver.as_mut(),
        &mut probe,
    );
    drop(probe);
    if let Some(error) = owner.into_inner() {
        return Err(V4ReadbackError::Owner(error));
    }
    result.map_err(V4ReadbackError::Batch)
}

pub(super) struct EmptyOriginalResolver;

impl OriginalInputResolverTrait for EmptyOriginalResolver {
    fn resolve_policy(
        &mut self,
        _reference: &[u8],
        _probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<std::sync::Arc<search_source_provenance::RetainedPolicyV2>, BatchCodecV4Error> {
        Err(BatchCodecV4Error::Registry(
            RegistryError::AdmissionRestorationUnavailable,
        ))
    }

    fn resolve_admission(
        &mut self,
        _reference: &[u8],
        _policy: std::sync::Arc<search_source_provenance::RetainedPolicyV2>,
        _probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<search_source_provenance::OriginalAdmissionV2, BatchCodecV4Error> {
        Err(BatchCodecV4Error::Registry(
            RegistryError::AdmissionRestorationUnavailable,
        ))
    }
}

fn make_probe<'context, 'work>(
    context: &'context OriginalV4Context<'work>,
    owner: &'context RefCell<Option<OwnerError>>,
) -> impl FnMut() -> Result<(), CheckpointFailureV2> + 'context {
    move || match (context.work)(search_control_redb::MigrationBootstrapWork::Checkpoint) {
        Ok(()) if std::time::Instant::now() < context.deadline => Ok(()),
        Ok(()) => Err(CheckpointFailureV2::DeadlineExceeded),
        Err(error) => {
            owner.borrow_mut().get_or_insert(error);
            Err(CheckpointFailureV2::Cancelled)
        }
    }
}
