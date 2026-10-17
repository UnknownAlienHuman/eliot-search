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
    ScopedControlRecordsReadback, control_mutation_identity,
};
use search_ports::{CancellationProbe, OperationContext};
use search_runtime_owner::OwnerError;
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_admission::{AcceptedCapabilities, AdmissionError, SectionPlatform};
use search_source_provenance::{
    ObservedOriginalInputPart, OriginalAdmissionV2, OriginalInputsLayoutV2, RetainedPolicyV2,
};
use search_source_registry::recovery::{
    BatchCodecV4Error, PreparedBatch, RegistryStateRecord, required_original_refs_from_batch_v4,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
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
#[derive(Clone)]
pub struct PreparedOriginalInputs {
    identity: JournalIdentity,
    command: Option<ConditionalControlMutation>,
}

impl PreparedOriginalInputs {
    fn matches(&self, other: &Self) -> bool {
        self.identity == other.identity && self.command == other.command
    }
}

struct SourceOriginalLayouts {
    policies: BTreeMap<Vec<u8>, OriginalInputsLayoutV2>,
    admissions: BTreeMap<Vec<u8>, (OriginalInputsLayoutV2, Vec<u8>)>,
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
    /// Retain and reconstitute every v4 original referenced by a Source batch.
    ///
    /// Each missing layout is prepared after the previous layout has committed,
    /// so its conditional command carries the then-current journal generation.
    /// The final coherent readback is passed through the actual policy and
    /// admission owners before the Source transaction is allowed to dispatch.
    pub(crate) fn retain_source_batch_originals(
        &mut self,
        batch: &search_source_registry::recovery::PreparedBatch,
        context: &OperationContext<C>,
    ) -> Result<(), OriginalInputsError> {
        if batch.record().encoding_profile() != "eliot.registry-batch.v4" {
            return Ok(());
        }
        let (deadline, work) = self.original_work(context, Work::Checkpoint)?;
        let owner_error = RefCell::new(None);
        let mut probe = make_probe(deadline, work, &owner_error);
        let layouts =
            preserve_owner_error(source_original_layouts(batch, &mut probe), &owner_error)?;
        if layouts.policies.is_empty() && layouts.admissions.is_empty() {
            return Ok(());
        }
        if self.original_v4.is_none() {
            return Err(OriginalInputsError::ContextUnavailable);
        }
        for layout in layouts
            .policies
            .values()
            .chain(layouts.admissions.values().map(|(layout, _)| layout))
        {
            let prepared = self.prepare_original_inputs(layout, context)?;
            self.commit_original_inputs(&prepared, context)?;
        }

        let references = layouts
            .policies
            .keys()
            .chain(layouts.admissions.keys())
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let readback = self.original_inputs_snapshot(&references, context)?;
        let original_v4 = self
            .original_v4
            .as_ref()
            .ok_or(OriginalInputsError::ContextUnavailable)?;
        let owner_error = RefCell::new(None);
        let mut probe = make_probe(deadline, work, &owner_error);
        let policies = preserve_owner_error(
            (|| {
                let mut policies = BTreeMap::new();
                for reference in layouts.policies.keys() {
                    let policy = readback
                        .policy(
                            reference,
                            &original_v4.platform,
                            &original_v4.capabilities,
                            &mut probe,
                        )
                        .map_err(|error| layout_error(error, None))?;
                    policies.insert(reference.clone(), Arc::new(policy));
                }
                Ok(policies)
            })(),
            &owner_error,
        )?;
        let owner_error = RefCell::new(None);
        let mut probe = make_probe(deadline, work, &owner_error);
        preserve_owner_error(
            (|| {
                for (reference, (_, policy_reference)) in &layouts.admissions {
                    let policy = policies
                        .get(policy_reference)
                        .ok_or(OriginalInputsError::InvalidMetadata)?;
                    readback
                        .admission(reference, Arc::clone(policy), &mut probe)
                        .map_err(|error| layout_error(error, None))?;
                }
                Ok(())
            })(),
            &owner_error,
        )
    }

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
                owner_error.borrow_mut().get_or_insert(error);
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
                self.original_pending = Some(prepared.clone());
            }
            result.map_err(OriginalInputsError::OriginalControl)?;
            self.original_pending = None;
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
        let pending = self
            .original_pending
            .as_ref()
            .ok_or(OriginalInputsError::InvalidMetadata)?;
        if !pending.matches(prepared) {
            return Err(OriginalInputsError::InvalidMetadata);
        }
        let (deadline, work) = self.original_work(context, Work::Checkpoint)?;
        let command = pending
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
            self.original_pending = None;
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

fn source_original_layouts<P: FnMut() -> Result<(), CheckpointFailureV2>>(
    batch: &PreparedBatch,
    probe: &mut P,
) -> Result<SourceOriginalLayouts, OriginalInputsError> {
    let mut policies = BTreeMap::new();
    let mut admissions = BTreeMap::new();
    for delta in batch.record().deltas() {
        let mut visit = |state: &RegistryStateRecord| -> Result<(), OriginalInputsError> {
            match state {
                RegistryStateRecord::Source(source) => {
                    let Some(permanent) = source.permanent_admission() else {
                        return Ok(());
                    };
                    let policy_layout = OriginalInputsLayoutV2::policy(
                        permanent.original().policy().as_ref(),
                        probe,
                    )
                    .map_err(|error| layout_error(error, None))?;
                    if policy_layout.reference() != permanent.policy_reference() {
                        return Err(OriginalInputsError::InvalidMetadata);
                    }
                    policies
                        .entry(permanent.policy_reference().to_vec())
                        .or_insert(policy_layout);
                    let admission_layout =
                        OriginalInputsLayoutV2::admission(permanent.original(), probe)
                            .map_err(|error| layout_error(error, None))?;
                    if admission_layout.reference() != permanent.admission_reference() {
                        return Err(OriginalInputsError::InvalidMetadata);
                    }
                    let admission_reference = permanent.admission_reference().to_vec();
                    let policy_reference = permanent.policy_reference().to_vec();
                    if let Some((_, existing_policy)) = admissions.get(&admission_reference)
                        && existing_policy != &policy_reference
                    {
                        return Err(OriginalInputsError::InvalidMetadata);
                    }
                    admissions
                        .entry(admission_reference)
                        .or_insert((admission_layout, policy_reference));
                }
                RegistryStateRecord::Namespace(namespace) => {
                    let Some(metadata) = namespace.permanent_metadata() else {
                        return Ok(());
                    };
                    let policy_layout =
                        OriginalInputsLayoutV2::policy(metadata.policy().as_ref(), probe)
                            .map_err(|error| layout_error(error, None))?;
                    if policy_layout.reference() != metadata.policy_reference() {
                        return Err(OriginalInputsError::InvalidMetadata);
                    }
                    policies
                        .entry(metadata.policy_reference().to_vec())
                        .or_insert(policy_layout);
                }
                RegistryStateRecord::Root(_)
                | RegistryStateRecord::Membership(_)
                | RegistryStateRecord::ProjectionMembership(_)
                | RegistryStateRecord::Generation { .. } => {}
            }
            Ok(())
        };
        if let Some(before) = delta.before() {
            visit(before)?;
        }
        visit(delta.after())?;
    }
    let expected = required_original_refs_from_batch_v4(batch.encoded(), probe)
        .map_err(batch_codec_error)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual = policies
        .keys()
        .chain(admissions.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(OriginalInputsError::InvalidMetadata);
    }
    Ok(SourceOriginalLayouts {
        policies,
        admissions,
    })
}

fn make_probe<'a>(
    deadline: Instant,
    work: &'a dyn Fn(Work) -> Result<(), OwnerError>,
    owner_error: &'a RefCell<Option<OwnerError>>,
) -> impl FnMut() -> Result<(), CheckpointFailureV2> + 'a {
    move || match work(Work::Checkpoint) {
        Ok(()) if Instant::now() < deadline => Ok(()),
        Ok(()) => Err(CheckpointFailureV2::DeadlineExceeded),
        Err(error) => {
            owner_error.borrow_mut().get_or_insert(error);
            Err(CheckpointFailureV2::Cancelled)
        }
    }
}

fn preserve_owner_error<T>(
    result: Result<T, OriginalInputsError>,
    owner_error: &RefCell<Option<OwnerError>>,
) -> Result<T, OriginalInputsError> {
    match (result, owner_error.borrow_mut().take()) {
        (Err(_), Some(error)) => Err(OriginalInputsError::Owner(error)),
        (result, None) => result,
        (Ok(_), Some(error)) => Err(OriginalInputsError::Owner(error)),
    }
}

fn batch_codec_error(error: BatchCodecV4Error) -> OriginalInputsError {
    match error {
        BatchCodecV4Error::Admission(error) => layout_error(error, None),
        BatchCodecV4Error::Registry(_) => OriginalInputsError::InvalidMetadata,
    }
}
const fn probe_error(error: CheckpointFailureV2) -> AdmissionError {
    match error {
        CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
        CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
    }
}

const fn layout_error(error: AdmissionError, owner: Option<OwnerError>) -> OriginalInputsError {
    if let Some(error) = owner {
        return OriginalInputsError::Owner(error);
    }
    match error {
        AdmissionError::Cancelled => OriginalInputsError::Control(ControlError::ReadCancelled),
        AdmissionError::BudgetExhausted => {
            OriginalInputsError::Control(ControlError::BudgetExceeded)
        }
        _ => OriginalInputsError::InvalidMetadata,
    }
}
