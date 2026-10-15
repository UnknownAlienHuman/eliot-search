//! Normal-path v4 operation/target readback over one coherent record batch.

use super::original_inputs::OriginalInputsReadback;
use super::OriginalNativeWork;
use search_control_redb::{MigrationBootstrapWork, RegistryOperationEnvelope,
    RegistryStoredState};
use search_runtime_owner::OwnerError;
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_admission::{AcceptedCapabilities, SectionPlatform};
use search_source_registry::error::RegistryError;
use search_source_registry::recovery::{
    BatchCodecV4Error, DecodedBatch, RegistryStateRecord, OriginalInputResolver,
    decode_batch_record_v4, decode_registry_state_v4, decode_registry_state_key_v4,
    decode_batch_record_with_original_probe, decode_registry_state_entry_profiled_with_original_probe,
    required_original_refs_from_batch_v4, required_original_refs_from_state_v4,
};
use std::{cell::RefCell, collections::BTreeSet, time::Instant};

/// Actual original context and semantic inputs for one normal v4 readback.
pub(crate) struct OriginalV4Context<'a> {
    pub(crate) deadline: Instant,
    pub(crate) work: OriginalNativeWork<'a>,
    pub(crate) platform: SectionPlatform,
    pub(crate) capabilities: AcceptedCapabilities,
    pub(crate) registry_limits: search_source_registry::error::RegistryLimits,
}

/// Bounded v4/legacy shape discovered from one raw operation and its targets.
pub(crate) struct TransactionShape {
    operation_v4: bool,
    state_v4: Vec<bool>,
    references: Vec<Vec<u8>>,
}

impl TransactionShape {
    pub(crate) const fn operation_v4(&self) -> bool {
        self.operation_v4
    }

    pub(crate) fn state_v4(&self) -> &[bool] {
        &self.state_v4
    }

    pub(crate) fn references(&self) -> &[Vec<u8>] {
        &self.references
    }

}

/// Closed readback failure retaining the exact owner/codec cause until the
/// daemon maps it to its vendor-neutral registry port error.
#[derive(Debug)]
pub(crate) enum V4ReadbackError {
    Owner(OwnerError),
    Batch(BatchCodecV4Error),
}

/// Extract references from the raw operation and exact current target bytes.
///
/// Legacy formats are accepted by their existing decoder and produce no v4
/// references. A v4 candidate is handed to the records-owned collector.
pub(crate) fn collect_transaction_refs(
    operation: &RegistryOperationEnvelope,
    states: &[RegistryStoredState],
    context: &OriginalV4Context<'_>,
) -> Result<TransactionShape, V4ReadbackError> {
    let owner = RefCell::new(None);
    let mut probe = make_probe(context, &owner);
    let result = (|| {
        let operation_v4 = super::v4_restore::is_v4_batch(operation.owner_bytes());
        if !operation_v4 {
            decode_batch_record_with_original_probe(operation.owner_bytes(), context.registry_limits, &mut probe)?;
        }
        let mut references = BTreeSet::new();
        if operation_v4 {
            for reference in required_original_refs_from_batch_v4(operation.owner_bytes(), &mut probe)? {
                references.insert(reference);
            }
        }
        let mut state_v4 = Vec::with_capacity(states.len());
        for state in states {
            let is_v4 = super::v4_restore::is_v4_state(state.canonical_bytes());
            if !is_v4 {
                decode_registry_state_entry_profiled_with_original_probe(
                    state.key().canonical_bytes(), state.canonical_bytes(), &mut probe,
                )?;
            }
            if is_v4 {
                for reference in required_original_refs_from_state_v4(state.canonical_bytes(), &mut probe)? {
                    references.insert(reference);
                }
            }
            state_v4.push(is_v4);
        }
        Ok(TransactionShape {
            operation_v4,
            state_v4,
            references: references.into_iter().collect(),
        })
    })();
    drop(probe);
    if let Some(error) = owner.into_inner() {
        return Err(V4ReadbackError::Owner(error));
    }
    result.map_err(V4ReadbackError::Batch)
}

/// Decode one operation and its exact targets using one resolver/cache view.
pub(crate) fn decode_transaction(
    shape: &TransactionShape,
    operation: &RegistryOperationEnvelope,
    states: &[RegistryStoredState],
    originals: Option<&OriginalInputsReadback>,
    context: &OriginalV4Context<'_>,
) -> Result<(DecodedBatch, Vec<RegistryStateRecord>), V4ReadbackError> {
    let mut resolver: Box<dyn OriginalInputResolver> = match originals {
        Some(readback) => Box::new(readback.v4_resolver(&context.platform, &context.capabilities)),
        None => Box::new(super::v4_restore::EmptyOriginalResolver),
    };
    let owner = RefCell::new(None);
    let mut probe = make_probe(context, &owner);
    let result = (|| {
        let decoded_operation = if shape.operation_v4() {
            decode_batch_record_v4(
                operation.owner_bytes(),
                context.registry_limits,
                resolver.as_mut(),
                &mut probe,
            )?
        } else {
            decode_batch_record_with_original_probe(operation.owner_bytes(), context.registry_limits, &mut probe)?
        };
        super::verify_operation_envelope(operation, &decoded_operation)?;
        let mut decoded_states = Vec::with_capacity(states.len());
        for (state, is_v4) in states.iter().zip(shape.state_v4()) {
            let decoded = if *is_v4 {
                decode_registry_state_v4(state.canonical_bytes(), resolver.as_mut(), &mut probe)?
            } else {
                decode_registry_state_entry_profiled_with_original_probe(
                    state.key().canonical_bytes(),
                    state.canonical_bytes(),
                    &mut probe,
                )
                .map(|(record, _)| record)?
            };
            if decoded.key() != decode_registry_state_key_v4(state.key().canonical_bytes())? {
                return Err(BatchCodecV4Error::Registry(RegistryError::SnapshotInvalid));
            }
            decoded_states.push(decoded);
        }
        Ok((decoded_operation, decoded_states))
    })();
    drop(probe);
    if let Some(error) = owner.into_inner() {
        return Err(V4ReadbackError::Owner(error));
    }
    result.map_err(V4ReadbackError::Batch)
}

fn make_probe<'context, 'work>(
    context: &'context OriginalV4Context<'work>,
    owner: &'context RefCell<Option<OwnerError>>,
) -> impl FnMut() -> Result<(), CheckpointFailureV2> + 'context {
    move || match (context.work)(MigrationBootstrapWork::Checkpoint) {
        Ok(()) if Instant::now() < context.deadline => Ok(()),
        Ok(()) => Err(CheckpointFailureV2::DeadlineExceeded),
        Err(error) => {
            owner.borrow_mut().get_or_insert(error);
            Err(CheckpointFailureV2::Cancelled)
        }
    }
}
