//! Immutable original-input metadata on the same bound journal and context.

use super::{JournalIdentity, RegistryJournal};
#[path = "original_inputs_v4.rs"]
mod original_inputs_v4;
pub(crate) use original_inputs_v4::OriginalInputResolver;
use search_contracts::{CanonicalDigestDomain, DigestInputLimit, blake3_raw};
use search_control_redb::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlError, ControlKey, ControlMutation,
    ControlRecordClass, ControlRecordCondition, ControlValue, ControlWrite,
    MigrationBootstrapFailure, MigrationBootstrapWork as Work, MutationId,
    ScopedControlRecordsReadback,
    control_mutation_identity,
};
use search_ports::{CancellationProbe, OperationContext};
use search_runtime_owner::OwnerError;
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_admission::{AcceptedCapabilities, AdmissionError, SectionPlatform};
use search_source_provenance::{
    ObservedOriginalInputPart, OriginalAdmissionV2, OriginalInputsLayoutV2, RetainedPolicyV2,
};
use std::{
    cell::RefCell,
    collections::BTreeSet,
    sync::Arc,
    time::Instant,
};

#[derive(Debug)]
pub enum OriginalInputsError {
    Context(search_ports::PortError<search_ports::ContextReason>),
    ContextUnavailable,
    Owner(OwnerError),
    Control(ControlError),
    OriginalControl(MigrationBootstrapFailure<OwnerError>),
    InvalidMetadata,
}

/// The caller retains this exact command through an unknown outcome and recovery.
pub struct PreparedOriginalInputs {
    identity: JournalIdentity,
    command: Option<ConditionalControlMutation>,
}

/// One actual coherent journal observation, not permission to serve source bytes.
pub struct OriginalInputsReadback {
    snapshot: ScopedControlRecordsReadback,
    limits: search_control_redb::JournalLimits,
}
impl OriginalInputsReadback {
    pub(crate) const fn generation(&self) -> u64 {
        self.snapshot.generation()
    }
    pub(crate) const fn identity(&self) -> JournalIdentity {
        self.snapshot.identity()
    }
    pub(crate) fn v4_resolver(
        &self,
        platform: &SectionPlatform,
        capabilities: &AcceptedCapabilities,
    ) -> OriginalInputResolver<'_> {
        OriginalInputResolver::new(&self.snapshot, self.limits, platform, capabilities)
    }
    pub(crate) fn policy(
        &self,
        reference: &[u8],
        platform: &SectionPlatform,
        capabilities: &AcceptedCapabilities,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<RetainedPolicyV2, AdmissionError> {
        let parts = self.parts(reference, probe)?;
        OriginalInputsLayoutV2::restore_policy(reference, &parts, platform, capabilities, probe)
    }
    pub(crate) fn admission(
        &self,
        reference: &[u8],
        policy: Arc<RetainedPolicyV2>,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<OriginalAdmissionV2, AdmissionError> {
        let parts = self.parts(reference, probe)?;
        OriginalInputsLayoutV2::restore_admission(reference, &parts, policy, probe)
    }
    fn parts(
        &self,
        reference: &[u8],
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Vec<ObservedOriginalInputPart<'_>>, AdmissionError> {
        let keys = OriginalInputsLayoutV2::lookup_keys(reference, probe)?;
        let mut parts = Vec::with_capacity(keys.len());
        for key in keys {
            probe().map_err(probe_error)?;
            let key =
                ControlKey::new(key, self.limits).map_err(|_| AdmissionError::ReceiptMismatch)?;
            if self.snapshot.missing_keys().binary_search(&key).is_ok() {
                return Err(AdmissionError::ReceiptMismatch);
            }
            let index = self
                .snapshot
                .records()
                .binary_search_by(|(candidate, _)| candidate.cmp(&key))
                .map_err(|_| AdmissionError::ReceiptMismatch)?;
            let (actual_key, value) = &self.snapshot.records()[index];
            if value.class() != ControlRecordClass::State {
                return Err(AdmissionError::ReceiptMismatch);
            }
            parts.push(ObservedOriginalInputPart {
                key: actual_key.as_bytes(),
                value: value.as_bytes(),
            });
        }
        probe().map_err(probe_error)?;
        Ok(parts)
    }
}

impl<'journal, C: CancellationProbe> RegistryJournal<'journal, C> {
    pub(crate) fn original_inputs_snapshot(
        &self,
        references: &[&[u8]],
        context: &OperationContext<C>,
    ) -> Result<OriginalInputsReadback, OriginalInputsError> {
        let (deadline, work) = self.original_work(context, Work::Checkpoint)?;
        if references.is_empty() {
            return Err(OriginalInputsError::InvalidMetadata);
        }
        let owner_error = RefCell::new(None);
        let mut probe = || match work(Work::Checkpoint) {
            Ok(()) if Instant::now() < deadline => Ok(()),
            Ok(()) => Err(CheckpointFailureV2::DeadlineExceeded),
            Err(error) => {
                owner_error.borrow_mut().replace(error);
                Err(CheckpointFailureV2::Cancelled)
            }
        };
        let mut keys = BTreeSet::new();
        for reference in references {
            let projected = OriginalInputsLayoutV2::lookup_keys(reference, &mut probe)
                .map_err(|error| layout_error(error, owner_error.borrow_mut().take()))?;
            for key in projected {
                keys.insert(
                    ControlKey::new(key, self.journal.limits())
                        .map_err(OriginalInputsError::Control)?,
                );
            }
        }
        let keys = keys.into_iter().collect::<Vec<_>>();
        let snapshot = self
            .journal
            .read_control_keys_with_original_context(&keys, deadline, &work)
            .map_err(OriginalInputsError::OriginalControl)?;
        if snapshot.identity() != self.identity {
            return Err(OriginalInputsError::InvalidMetadata);
        }
        Ok(OriginalInputsReadback {
            snapshot,
            limits: self.journal.limits(),
        })
    }

    pub(crate) fn prepare_original_inputs(
        &self,
        layout: &OriginalInputsLayoutV2,
        context: &OperationContext<C>,
    ) -> Result<PreparedOriginalInputs, OriginalInputsError> {
        if self.requires_recovery() {
            return Err(OriginalInputsError::Control(
                ControlError::CommitOutcomeUnknown,
            ));
        }
        let references = [layout.reference()];
        let observed = self.original_inputs_snapshot(&references, context)?;
        let limits = self.journal.limits();
        let mut writes = Vec::with_capacity(layout.parts().len());
        let mut conditions = Vec::with_capacity(layout.parts().len());
        let mut present = 0;
        for part in layout.parts() {
            let bytes = part
                .key()
                .len()
                .checked_add(part.value().len())
                .ok_or(OriginalInputsError::InvalidMetadata)?;
            self.original_work(context, Work::Allocate(bytes))?;
            self.original_work(context, Work::Process(bytes))?;
            let key = ControlKey::new(part.key().to_vec(), limits)
                .map_err(OriginalInputsError::Control)?;
            let value = ControlValue::new(ControlRecordClass::State, part.value().to_vec(), limits)
                .map_err(OriginalInputsError::Control)?;
            if let Some(existing) = observed.snapshot.get(&key) {
                if existing != &value {
                    return Err(OriginalInputsError::InvalidMetadata);
                }
                present += 1;
            }
            conditions.push(ControlRecordCondition::absent(key.clone()));
            writes.push(ControlWrite { key, value });
        }
        if present != 0 && present != writes.len() {
            return Err(OriginalInputsError::InvalidMetadata);
        }
        let command = if present == writes.len() {
            None
        } else {
            self.original_work(context, Work::Process(layout.reference().len()))?;
            let domain = CanonicalDigestDomain::parse("eliot/raw/original-inputs-retain/v2")
                .map_err(|_| OriginalInputsError::InvalidMetadata)?;
            let digest = blake3_raw(
                &domain,
                layout.reference(),
                DigestInputLimit::new(1_024).map_err(|_| OriginalInputsError::InvalidMetadata)?,
            )
            .map_err(|_| OriginalInputsError::InvalidMetadata)?;
            Some(ConditionalControlMutation::new(
                ControlMutation::new(
                    MutationId(*digest.as_bytes()),
                    digest,
                    observed.snapshot.generation(),
                    writes,
                    vec![],
                ),
                conditions,
            ))
        };
        self.original_work(context, Work::Checkpoint)?;
        Ok(PreparedOriginalInputs {
            identity: self.identity,
            command,
        })
    }

    pub(crate) fn commit_original_inputs(
        &mut self,
        prepared: &PreparedOriginalInputs,
        context: &OperationContext<C>,
    ) -> Result<(), OriginalInputsError> {
        if self.requires_recovery() {
            return Err(OriginalInputsError::Control(
                ControlError::CommitOutcomeUnknown,
            ));
        }
        if prepared.identity != self.identity {
            return Err(OriginalInputsError::InvalidMetadata);
        }
        let (deadline, work) = self.original_work(context, Work::Checkpoint)?;
        if let Some(command) = &prepared.command {
            let identity = control_mutation_identity(command.mutation().id())
                .map_err(OriginalInputsError::Control)?;
            let result = self
                .journal
                .transact_records_with_original_context(command, &identity, deadline, &work);
            if result
                .as_ref()
                .err()
                .is_some_and(|error| error.control == ControlError::CommitOutcomeUnknown)
            {
                self.original_uncertain = true;
            }
            result.map_err(OriginalInputsError::OriginalControl)?;
        }
        Ok(())
    }

    pub(crate) fn recover_original_inputs(
        &mut self,
        prepared: &PreparedOriginalInputs,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, OriginalInputsError> {
        if prepared.identity != self.identity {
            return Err(OriginalInputsError::InvalidMetadata);
        }
        let (deadline, work) = self.original_work(context, Work::Checkpoint)?;
        let command = prepared
            .command
            .as_ref()
            .ok_or(OriginalInputsError::InvalidMetadata)?;
        let identity = control_mutation_identity(command.mutation().id())
            .map_err(OriginalInputsError::Control)?;
        let result = self
            .journal
            .recover_records_with_original_context(command, &identity, deadline, &work);
        if matches!(
            &result,
            Ok(CommitRecoveryDecision::Committed(_)
                | CommitRecoveryDecision::NotCommittedRetrySameOperation)
        ) {
            self.original_uncertain = false;
        }
        result.map_err(OriginalInputsError::OriginalControl)
    }

    fn original_work(
        &self,
        context: &OperationContext<C>,
        unit: Work,
    ) -> Result<(Instant, super::OriginalNativeWork<'journal>), OriginalInputsError> {
        context.preflight().map_err(OriginalInputsError::Context)?;
        let (deadline, work) = self
            .original
            .ok_or(OriginalInputsError::ContextUnavailable)?;
        work(unit).map_err(OriginalInputsError::Owner)?;
        if Instant::now() >= deadline {
            return Err(OriginalInputsError::Control(ControlError::BudgetExceeded));
        }
        Ok((deadline, work))
    }
}
const fn probe_error(error: CheckpointFailureV2) -> AdmissionError {
    match error {
        CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
        CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
    }
}

fn layout_error(error: AdmissionError, owner: Option<OwnerError>) -> OriginalInputsError {
    if let Some(error) = owner {
        return OriginalInputsError::Owner(error);
    }
    match error {
        AdmissionError::Cancelled => OriginalInputsError::Control(ControlError::ReadCancelled),
        AdmissionError::BudgetExhausted => OriginalInputsError::Control(ControlError::BudgetExceeded),
        _ => OriginalInputsError::InvalidMetadata,
    }
}
