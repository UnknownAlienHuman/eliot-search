//! First Current selection through an actual backend dependency and the existing journal.
use super::{
    DirectRegistryOwner,
    command::{DirectOutput, published_metadata},
    dek_command as shared,
};
use crate::config_composition::{self as config, CliConfigArgs};
use search_contracts::CanonicalValue as V;
use search_control_redb::{
    self as db, BoundControlJournal, ConditionalControlMutation, ControlKey, ControlRecordClass,
    ControlRecordCondition, ControlValue, ControlWrite, JournalLimits,
    MigrationBootstrapWork as Work, MutationId, RevisionDekSelectionCommand as StoredCommand,
    ScopedControlRecordsReadback,
};
use search_os_secrets::{
    RevisionDekApplicationIntent as Intent, RevisionDekRecord,
    RevisionDekSelectionPolicy as Policy, RevisionDekSelectionRecord as Record,
    decode_revision_dek_selection_record as decode, encode_revision_dek_selection_record as encode,
};
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiWork};
use search_ports::CancellationProbe;
use search_runtime_owner::OwnerError;
use std::{
    ffi::OsString,
    io::{Read, Write},
    time::Instant,
};

pub(super) struct SelectionCommand {
    operation: [u8; 16],
    expected: [u8; 32],
    cli: CliConfigArgs,
}
impl SelectionCommand {
    pub(super) fn parse(selected: &[OsString], cli: CliConfigArgs) -> Result<Self, String> {
        if selected.len() != 4
            || selected[0] != "--select-revision-dek-v1"
            || !cli.overrides.is_empty()
        {
            return Err("REVISION_DEK_SELECTION_USAGE".into());
        }
        let operation = super::super::super::codec::parse_id16(selected[2].to_str())
            .map_err(|_| "REVISION_DEK_SELECTION_OPERATION_INVALID")?;
        if operation == [0; 16] {
            return Err("REVISION_DEK_SELECTION_OPERATION_INVALID".into());
        }
        let expected = super::super::super::codec::parse_digest32(selected[3].to_str())
            .map_err(|_| "REVISION_DEK_SELECTION_CONFIG_IDENTITY_INVALID")?;
        Ok(Self {
            operation,
            expected,
            cli,
        })
    }
}

// This first-selection input family is explicit File/defaults, like the existing
// DIRECT root command. No general activation or ambient environment is applied.
fn capture_candidate(
    input: &SelectionCommand,
    work: &impl Fn(Work) -> Result<(), OwnerError>,
) -> Result<config::EffectiveDaemonConfig, String> {
    let preparation = search_config::ConfigLimits::W1
        .max_canonical_bytes
        .checked_mul(8)
        .ok_or("REVISION_DEK_SELECTION_CONFIG_BOUNDS")?;
    for charge in [Work::Allocate(preparation), Work::Process(preparation)] {
        work(charge).map_err(|error| error.code().to_owned())?;
    }
    let document = if let Some(path) = &input.cli.config_file {
        let ceiling = config::MAX_CAPTURED_FILE_BYTES
            .checked_add(1)
            .ok_or("REVISION_DEK_SELECTION_CONFIG_BOUNDS")?;
        work(Work::Allocate(ceiling)).map_err(|error| error.code().to_owned())?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(ceiling)
            .map_err(|_| "REVISION_DEK_SELECTION_CONFIG_BOUNDS")?;
        // Reserve open, metadata and release before acquisition. Every read is
        // separately prepaid and checkpointed; one extra byte detects growth.
        for _ in 0..3 {
            work(Work::Open).map_err(|error| error.code().to_owned())?;
        }
        let mut file =
            std::fs::File::open(path).map_err(|_| "REVISION_DEK_SELECTION_CONFIG_UNAVAILABLE")?;
        work(Work::Checkpoint).map_err(|error| error.code().to_owned())?;
        if !file
            .metadata()
            .map_err(|_| "REVISION_DEK_SELECTION_CONFIG_UNAVAILABLE")?
            .is_file()
        {
            return Err("REVISION_DEK_SELECTION_CONFIG_UNAVAILABLE".into());
        }
        let mut chunk = [0; 4096];
        while bytes.len() < ceiling {
            let count = chunk.len().min(ceiling - bytes.len());
            work(Work::Read(count)).map_err(|error| error.code().to_owned())?;
            let count = file
                .read(&mut chunk[..count])
                .map_err(|_| "REVISION_DEK_SELECTION_CONFIG_UNAVAILABLE")?;
            work(Work::Checkpoint).map_err(|error| error.code().to_owned())?;
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
        drop(file);
        work(Work::Checkpoint).map_err(|error| error.code().to_owned())?;
        if bytes.len() > config::MAX_CAPTURED_FILE_BYTES {
            return Err("REVISION_DEK_SELECTION_CONFIG_BOUNDS".into());
        }
        Some(
            config::capture_file_document(&bytes, "config-file")
                .map_err(|_| "REVISION_DEK_SELECTION_CONFIG_INVALID")?,
        )
    } else {
        None
    };
    config::build_effective(document, None, None, "direct", "direct")
        .map_err(|_| "REVISION_DEK_SELECTION_CONFIG_INVALID".into())
}

fn stored(record: &Record) -> Result<ControlValue, String> {
    let bytes = encode(record).map_err(|error| error.code().to_owned())?;
    ControlValue::new(ControlRecordClass::State, bytes, JournalLimits::BASELINE)
        .map_err(|_| "REVISION_DEK_SELECTION_CONTROL_VALUE_INVALID".into())
}
fn load(view: &ScopedControlRecordsReadback, key: &ControlKey) -> Result<Option<Record>, String> {
    if let Some(value) = view.get(key) {
        if value.class() != ControlRecordClass::State {
            return Err("REVISION_DEK_SELECTION_CONTROL_CLASS_INVALID".into());
        }
        return decode(value.as_bytes())
            .map(Some)
            .map_err(|error| error.code().to_owned());
    }
    if !view.missing_keys().contains(key) {
        return Err("REVISION_DEK_SELECTION_READBACK_INCOMPLETE".into());
    }
    Ok(None)
}
fn phase_id(intent: &Intent, phase: u64) -> Result<MutationId, String> {
    let hash = shared::digest(
        "eliot/cbor/revision-dek-selection-operation/v1",
        vec![
            shared::bytes(&intent.candidate().intent().binding().binding_digest())?,
            shared::bytes(&intent.operation_id())?,
            V::U64(phase),
        ],
    )?;
    Ok(MutationId(*hash.as_bytes()))
}
fn command(
    intent: &Intent,
    phase: u64,
    generation: u64,
    value: ControlValue,
) -> Result<StoredCommand, String> {
    let original = shared::stored(&RevisionDekRecord::Intent(
        intent.candidate().intent().clone(),
    ))?;
    let candidate = shared::stored(&RevisionDekRecord::CandidateNonServing(
        intent.candidate().clone(),
    ))?;
    let application = stored(&Record::Intent(intent.clone()))?;
    let hash = shared::digest(
        "eliot/cbor/revision-dek-selection-command/v1",
        vec![
            shared::bytes(&phase_id(intent, phase)?.0)?,
            V::U64(generation),
            shared::bytes(value.as_bytes())?,
            shared::bytes(original.as_bytes())?,
            shared::bytes(candidate.as_bytes())?,
            if phase == 0 {
                V::Null
            } else {
                shared::bytes(application.as_bytes())?
            },
        ],
    )?;
    let application_key = shared::key(db::REVISION_DEK_APPLICATION_INTENT_KEY)?;
    let current_key = shared::key(db::REVISION_DEK_CURRENT_KEY)?;
    let conditions = vec![
        ControlRecordCondition::exact(shared::key(db::REVISION_DEK_INTENT_KEY)?, original),
        ControlRecordCondition::exact(shared::key(db::REVISION_DEK_CANDIDATE_KEY)?, candidate),
        if phase == 0 {
            ControlRecordCondition::absent(application_key.clone())
        } else {
            ControlRecordCondition::exact(application_key.clone(), application)
        },
        ControlRecordCondition::absent(current_key.clone()),
    ];
    let target = if phase == 0 {
        application_key
    } else {
        current_key
    };
    let mutation = db::ControlMutation::new(
        phase_id(intent, phase)?,
        hash,
        generation,
        vec![ControlWrite { key: target, value }],
        vec![],
    );
    StoredCommand::new(ConditionalControlMutation::new(mutation, conditions))
        .map_err(|_| "REVISION_DEK_SELECTION_CONTROL_COMMAND_INVALID".into())
}
fn commit<C: CancellationProbe>(
    port: &mut BoundControlJournal<'_, C>,
    command: &StoredCommand,
    deadline: Instant,
    work: &impl Fn(Work) -> Result<(), OwnerError>,
) -> Result<(), String> {
    let mutation = db::control_mutation_identity(command.command().mutation().id())
        .map_err(|_| "REVISION_DEK_SELECTION_CONTROL_IDENTITY_INVALID")?;
    if port
        .transact_revision_dek_selection_with_original_context(command, &mutation, deadline, work)
        .is_ok()
    {
        return Ok(());
    }
    match port
        .recover_revision_dek_selection_with_original_context(command, &mutation, deadline, work)
    {
        Ok(db::CommitRecoveryDecision::Committed(_)) => Ok(()),
        _ => Err("REVISION_DEK_SELECTION_OUTCOME_UNKNOWN".into()),
    }
}
fn verify_operation<C: CancellationProbe>(
    port: &mut BoundControlJournal<'_, C>,
    intent: &Intent,
    phase: u64,
    value: ControlValue,
    deadline: Instant,
    work: &impl Fn(Work) -> Result<(), OwnerError>,
) -> Result<(), String> {
    let receipt = port
        .revision_dek_selection_operation_with_original_context(
            phase_id(intent, phase)?,
            deadline,
            work,
        )
        .map_err(|_| "REVISION_DEK_SELECTION_OPERATION_READ_REFUSED")?
        .ok_or("REVISION_DEK_SELECTION_OPERATION_MISSING")?;
    let command = command(intent, phase, receipt.before_generation, value)?;
    let mutation = db::control_mutation_identity(command.command().mutation().id())
        .map_err(|_| "REVISION_DEK_SELECTION_CONTROL_IDENTITY_INVALID")?;
    match port
        .recover_revision_dek_selection_with_original_context(&command, &mutation, deadline, work)
    {
        Ok(db::CommitRecoveryDecision::Committed(actual))
            if actual.operation_id == receipt.operation_id
                && actual.command_digest == receipt.command_digest =>
        {
            Ok(())
        }
        _ => Err("REVISION_DEK_SELECTION_OPERATION_READBACK_REFUSED".into()),
    }
}
fn output(available: bool, committed: bool, historical: bool, reason: &str) -> DirectOutput {
    DirectOutput {
        current: available,
        line: format!(
            "{{\"event\":\"revision_dek_current\",\"durable_current_committed\":{committed},\"current_backend_validated\":{available},\"historical_commit\":{historical},\"application_scope\":\"revision_dek_selection_backend\",\"whole_daemon_config_applied\":false,\"ttl_enforced\":false,\"rotation_enforced\":false,\"reason\":\"{reason}\",\"key_use\":false,\"leases\":false,\"source_body_serving\":false,\"ready\":false}}"
        ),
    }
}

impl DirectRegistryOwner {
    pub(super) fn execute_revision_dek_selection(
        mut self,
        command: &SelectionCommand,
        writer: &mut impl Write,
    ) -> Result<(), String> {
        let result = self.select_revision_dek(command);
        self.complete_output(result, writer)
    }
    #[expect(clippy::too_many_lines, reason = "One borrowed Native custody and journal span both exact phases, recovery and final backend fences")]
    fn select_revision_dek(&mut self, input: &SelectionCommand) -> Result<DirectOutput, String> {
        let deadline = self.owner.request.credential_context().0;
        let mut read = super::super::read_child::direct_operation(&mut self.owner)
            .map_err(|error| error.code().to_owned())?;
        let native = read
            .fresh_root_context()
            .map_err(|error| error.to_string())?;
        let facts = native
            .revision_dek_root_facts()
            .map_err(|_| "REVISION_DEK_SELECTION_NATIVE_SCOPE_REFUSED")?;
        let poll = || native.work(Work::Checkpoint);
        let work = |work| match work {
            LegacyRevisionDpapiWork::NativeCall => native.revision_dek_native_attempt(),
            LegacyRevisionDpapiWork::Allocate(n) => native.work(Work::Allocate(n)),
            LegacyRevisionDpapiWork::Process(n) => native.work(Work::Process(n)),
        };
        let open = || native.work(Work::Open);
        let clear = |bytes| native.revision_dek_account_native_clear(bytes);
        let mut context = shared::NativeContext {
            deadline,
            poll: &poll,
            work: &work,
            open: &open,
            clear: &clear,
        };
        let user =
            os::observe_revision_user(&mut context).map_err(|error| error.code().to_owned())?;
        native
            .work(Work::Allocate(32 * 1024))
            .map_err(|error| error.code().to_owned())?;
        native
            .work(Work::Process(32 * 1024))
            .map_err(|error| error.code().to_owned())?;
        let callback = |work| native.work(work);
        let journal = self
            .control
            .journal
            .as_mut()
            .ok_or("REVISION_DEK_SELECTION_CONTROL_UNAVAILABLE")?;
        let mut port = shared::bind(journal, &mut self.control.publisher, native.context())?;
        let keys = [
            shared::key(db::REVISION_DEK_INTENT_KEY)?,
            shared::key(db::REVISION_DEK_CANDIDATE_KEY)?,
            shared::key(db::REVISION_DEK_APPLICATION_INTENT_KEY)?,
            shared::key(db::REVISION_DEK_CURRENT_KEY)?,
        ];
        let mut view = port
            .read_control_keys_with_original_context(&keys, deadline, &callback)
            .map_err(|_| "REVISION_DEK_SELECTION_CONTROL_READ_REFUSED")?;
        let Some(RevisionDekRecord::CandidateNonServing(candidate)) =
            shared::load(&view, &keys[1])?
        else {
            return Ok(output(
                false,
                false,
                false,
                "REVISION_DEK_SELECTION_CANDIDATE_REQUIRED",
            ));
        };
        if shared::load(&view, &keys[0])?
            != Some(RevisionDekRecord::Intent(candidate.intent().clone()))
        {
            return Err("REVISION_DEK_SELECTION_CANDIDATE_CONTRADICTION".into());
        }
        let binding = candidate.intent().binding();
        if binding.root_id() != facts.data_root
            || binding.installation_id() != facts.installation
            || binding.installation_incarnation_id() != facts.incarnation
            || binding.user_scope_digest() != user.user_scope_digest()
        {
            return Ok(output(
                false,
                false,
                false,
                "REVISION_DEK_SELECTION_SCOPE_MISMATCH",
            ));
        }
        let application = load(&view, &keys[2])?;
        let current = load(&view, &keys[3])?;
        let historical = current.is_some();
        let mut application_published_here = false;
        let intent = match application {
            Some(Record::Intent(intent)) => intent,
            Some(_) => return Err("REVISION_DEK_SELECTION_APPLICATION_CONTRADICTION".into()),
            None => {
                if current.is_some() {
                    return Err("REVISION_DEK_SELECTION_APPLICATION_MISSING".into());
                }
                native
                    .source_full_checkpoint()
                    .map_err(|_| "REVISION_DEK_SELECTION_NATIVE_SCOPE_REFUSED")?;
                let config = capture_candidate(input, &callback)?;
                native
                    .source_full_checkpoint()
                    .map_err(|_| "REVISION_DEK_SELECTION_NATIVE_SCOPE_REFUSED")?;
                if config.fingerprint().as_bytes() != &input.expected {
                    return Ok(output(
                        false,
                        false,
                        false,
                        "REVISION_DEK_SELECTION_CONFIG_MISMATCH",
                    ));
                }
                let policy = Policy::from_config(config.secrets_settings(), config.fingerprint())
                    .map_err(|error| error.code().to_owned())?;
                let intent = Intent::new(
                    candidate.clone(),
                    input.operation,
                    policy,
                    facts.creator_epoch,
                )
                .map_err(|error| error.code().to_owned())?;
                for phase in [0, 1] {
                    if port
                        .revision_dek_selection_operation_with_original_context(
                            phase_id(&intent, phase)?,
                            deadline,
                            &callback,
                        )
                        .map_err(|_| "REVISION_DEK_SELECTION_OPERATION_READ_REFUSED")?
                        .is_some()
                    {
                        return Err("REVISION_DEK_SELECTION_OPERATION_CONTRADICTION".into());
                    }
                }
                let command = command(
                    &intent,
                    0,
                    view.generation(),
                    stored(&Record::Intent(intent.clone()))?,
                )?;
                commit(&mut port, &command, deadline, &callback)?;
                application_published_here = true;
                view = port
                    .read_control_keys_with_original_context(&keys, deadline, &callback)
                    .map_err(|_| "REVISION_DEK_SELECTION_CONTROL_READ_REFUSED")?;
                intent
            }
        };
        if intent.operation_id() != input.operation
            || intent.candidate() != &candidate
            || intent.policy().config_fingerprint().as_ref() != input.expected.as_slice()
        {
            return Ok(output(
                false,
                false,
                false,
                "REVISION_DEK_SELECTION_OPERATION_CONFLICT",
            ));
        }
        if load(&view, &keys[2])? != Some(Record::Intent(intent.clone())) {
            return Err("REVISION_DEK_SELECTION_APPLICATION_READBACK_REFUSED".into());
        }
        if !application_published_here {
            verify_operation(
                &mut port,
                &intent,
                0,
                stored(&Record::Intent(intent.clone()))?,
                deadline,
                &callback,
            )?;
        }
        // A stored Current is only historical evidence after exact metadata and
        // operation recovery. Backend failure must never acknowledge mere presence.
        if let Some(current) = &current {
            let expected = Record::Current(
                search_os_secrets::RevisionDekCurrent::new(&intent)
                    .map_err(|error| error.code().to_owned())?,
            );
            if current != &expected {
                return Err("REVISION_DEK_SELECTION_CURRENT_CONTRADICTION".into());
            }
            verify_operation(&mut port, &intent, 1, stored(current)?, deadline, &callback)?;
        }
        let dependency =
            match os::construct_revision_dek_selection(intent.policy(), &candidate, &mut context) {
                Ok(dependency) => dependency,
                Err(error) => return Ok(output(false, historical, historical, error.code())),
            };
        let selected = dependency
            .prepare_current_record(&intent)
            .map_err(|error| error.code().to_owned())?;
        let record = Record::Current(selected);
        if let Some(current) = current {
            if current != record {
                return Err("REVISION_DEK_SELECTION_CURRENT_CONTRADICTION".into());
            }
        } else {
            native
                .source_full_checkpoint()
                .map_err(|_| "REVISION_DEK_SELECTION_NATIVE_SCOPE_REFUSED")?;
            if os::observe_revision_user(&mut context)
                .map_err(|error| error.code().to_owned())?
                .user_scope_digest()
                != binding.user_scope_digest()
            {
                return Ok(output(
                    false,
                    false,
                    false,
                    "REVISION_DEK_SELECTION_USER_SCOPE_MISMATCH",
                ));
            }
            let command = command(&intent, 1, view.generation(), stored(&record)?)?;
            commit(&mut port, &command, deadline, &callback)?;
        }
        let final_view = port
            .read_control_keys_with_original_context(&keys, deadline, &callback)
            .map_err(|_| "REVISION_DEK_SELECTION_CONTROL_READ_REFUSED")?;
        if shared::load(&final_view, &keys[0])?
            != Some(RevisionDekRecord::Intent(candidate.intent().clone()))
            || shared::load(&final_view, &keys[1])?
                != Some(RevisionDekRecord::CandidateNonServing(candidate.clone()))
            || load(&final_view, &keys[2])? != Some(Record::Intent(intent.clone()))
            || load(&final_view, &keys[3])? != Some(record.clone())
        {
            return Err("REVISION_DEK_SELECTION_FINAL_READBACK_REFUSED".into());
        }
        // Fresh commit/recovery already returns the C02 owner-verified receipt;
        // historical phase-1 recovery was mandatory before backend construction.
        // The final exact state readback does not repeat either transaction proof.
        let revalidated =
            match os::construct_revision_dek_selection(intent.policy(), &candidate, &mut context) {
                Ok(value) => value,
                Err(error) => return Ok(output(false, true, historical, error.code())),
            };
        if Record::Current(
            revalidated
                .prepare_current_record(&intent)
                .map_err(|error| error.code().to_owned())?,
        ) != record
        {
            return Ok(output(
                false,
                true,
                historical,
                "REVISION_DEK_SELECTION_BACKEND_READBACK_REFUSED",
            ));
        }
        native
            .source_full_checkpoint()
            .map_err(|_| "REVISION_DEK_SELECTION_NATIVE_SCOPE_REFUSED")?;
        if os::observe_revision_user(&mut context)
            .map_err(|error| error.code().to_owned())?
            .user_scope_digest()
            != binding.user_scope_digest()
        {
            return Ok(output(
                false,
                true,
                historical,
                "REVISION_DEK_SELECTION_USER_SCOPE_MISMATCH",
            ));
        }
        drop(dependency);
        drop(revalidated);
        let _published =
            published_metadata(journal, &mut self.control.publisher, deadline, &callback)?;
        Ok(output(
            true,
            true,
            historical,
            "REVISION_DEK_CURRENT_VERIFIED",
        ))
    }
}
