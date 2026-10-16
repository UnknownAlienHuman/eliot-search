//! Explicit candidate-only DEK effect over the retained DIRECT owner and journal.
use super::{DirectRegistryOwner, command::{DirectOutput, published_metadata}};
use search_contracts::{self as c, CanonicalValue as V};
use search_control_redb::{self as db, BoundControlJournal, ConditionalControlMutation,
    ControlKey, ControlRecordClass, ControlRecordCondition, ControlValue, ControlWrite,
    JournalLimits, MigrationBootstrapWork as Work, MutationId, RevisionDekCommand as StoredCommand,
    ScopedControlRecordsReadback};
use search_os_secrets::{RevisionDekBinding, RevisionDekCandidate, RevisionDekIntent, RevisionDekRecord,
    decode_revision_dek_record, encode_revision_dek_record};
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiContext, LegacyRevisionDpapiWork};
use search_ports::{CancellationProbe, OperationContext};
use search_runtime_owner::OwnerError;
use std::{ffi::OsString, io::Write, time::Instant};

pub(super) struct RevisionDekCommand { operation: [u8; 16] }
impl RevisionDekCommand {
    pub(super) fn parse(selected: &[OsString]) -> Result<Self, String> {
        if selected.len() != 3 || selected[0] != "--prepare-revision-dek-v1" {
            return Err("REVISION_DEK_USAGE".into());
        }
        let operation = super::super::super::codec::parse_id16(selected[2].to_str())
            .map_err(|_| "REVISION_DEK_OPERATION_INVALID")?;
        if operation == [0; 16] { return Err("REVISION_DEK_OPERATION_INVALID".into()); }
        Ok(Self { operation })
    }
}

#[derive(Clone, Copy)]
struct InvocationRefusal { first: OwnerError, cleanup_exhausted: bool }
impl std::fmt::Debug for InvocationRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevisionDekInvocationRefusal").field("first", &self.first.code())
            .field("cleanup_exhausted", &self.cleanup_exhausted).finish()
    }
}
impl From<OwnerError> for InvocationRefusal {
    fn from(first: OwnerError) -> Self { Self { first, cleanup_exhausted: false } }
}
struct NativeContext<'a> {
    deadline: Instant,
    poll: &'a dyn Fn() -> Result<(), OwnerError>,
    work: &'a dyn Fn(LegacyRevisionDpapiWork) -> Result<(), OwnerError>,
    open: &'a dyn Fn() -> Result<(), OwnerError>,
    clear: &'a dyn Fn(usize) -> Result<(), (OwnerError, bool)>,
}
impl LegacyRevisionDpapiContext for NativeContext<'_> {
    type Error = InvocationRefusal;
    fn checkpoint(&mut self) -> Result<(), Self::Error> { (self.poll)().map_err(Into::into) }
    fn charge(&mut self, work: LegacyRevisionDpapiWork) -> Result<(), Self::Error> {
        (self.work)(work).map_err(Into::into)
    }
    fn account_native_clear(&mut self, bytes: usize) -> Result<(), Self::Error> {
        (self.clear)(bytes).map_err(|(first, cleanup_exhausted)| InvocationRefusal { first, cleanup_exhausted })
    }
}
impl os::RevisionDekContext for NativeContext<'_> {
    fn deadline(&self) -> Instant { self.deadline }
    fn charge_open(&mut self) -> Result<(), Self::Error> { (self.open)().map_err(Into::into) }
}

fn bind<'j, C: CancellationProbe>(journal: &'j mut db::PersistentControlJournal,
    publisher: &'j mut db::ControlSnapshotPublisher, _: &OperationContext<C>,
) -> Result<BoundControlJournal<'j, C>, String> {
    journal.bind_control_port(publisher).map_err(|_| "REVISION_DEK_CONTROL_UNAVAILABLE".into())
}
fn key(value: &[u8]) -> Result<ControlKey, String> {
    ControlKey::new(value.to_vec(), JournalLimits::BASELINE).map_err(|_| "REVISION_DEK_CONTROL_KEY_INVALID".into())
}
fn stored(record: &RevisionDekRecord) -> Result<ControlValue, String> {
    let bytes = encode_revision_dek_record(record).map_err(|error| error.code().to_owned())?;
    ControlValue::new(ControlRecordClass::State, bytes, JournalLimits::BASELINE)
        .map_err(|_| "REVISION_DEK_CONTROL_VALUE_INVALID".into())
}
fn load(snapshot: &ScopedControlRecordsReadback, key: &ControlKey) -> Result<Option<RevisionDekRecord>, String> {
    if let Some(value) = snapshot.get(key) {
        if value.class() != ControlRecordClass::State { return Err("REVISION_DEK_CONTROL_CLASS_INVALID".into()); }
        return decode_revision_dek_record(value.as_bytes()).map(Some).map_err(|error| error.code().to_owned());
    }
    if !snapshot.missing_keys().contains(key) { return Err("REVISION_DEK_CONTROL_READBACK_INCOMPLETE".into()); }
    Ok(None)
}
fn digest(domain: &str, fields: Vec<V>) -> Result<c::Blake3Digest32, String> {
    let value = c::BoundedList::new(fields).map(V::Array).map_err(|_| "REVISION_DEK_COMMAND_INVALID")?;
    c::blake3_canonical(&c::CanonicalDigestDomain::parse(domain).map_err(|_| "REVISION_DEK_COMMAND_INVALID")?,
        &value, c::DigestInputLimit::new(8192).map_err(|_| "REVISION_DEK_COMMAND_INVALID")?)
        .map_err(|_| "REVISION_DEK_COMMAND_INVALID".into())
}
fn bytes(value: &[u8]) -> Result<V, String> {
    c::BoundedBytes::new(value.to_vec()).map(V::Bytes).map_err(|_| "REVISION_DEK_COMMAND_INVALID".into())
}
fn phase_id(binding: &RevisionDekBinding, phase: u64) -> Result<MutationId, String> {
    let hash = digest("eliot/cbor/revision-dek-control-operation/v1", vec![
        bytes(&binding.binding_digest())?, V::U64(phase)])?;
    Ok(MutationId(*hash.as_bytes()))
}
fn command(binding: &RevisionDekBinding, phase: u64, generation: u64, value: ControlValue,
    intent: Option<ControlValue>,
) -> Result<StoredCommand, String> {
    let old = intent.as_ref().map_or(Ok(V::Null), |v| bytes(v.as_bytes()))?;
    let hash = digest("eliot/cbor/revision-dek-control-command/v1", vec![
        bytes(&binding.binding_digest())?, V::U64(phase), V::U64(generation), bytes(value.as_bytes())?, old])?;
    let intent_key = key(db::REVISION_DEK_INTENT_KEY)?;
    let candidate_key = key(db::REVISION_DEK_CANDIDATE_KEY)?;
    let current_key = key(db::REVISION_DEK_CURRENT_KEY)?;
    let conditions = vec![intent.map_or_else(|| ControlRecordCondition::absent(intent_key.clone()),
        |value| ControlRecordCondition::exact(intent_key.clone(), value)),
        ControlRecordCondition::absent(candidate_key.clone()), ControlRecordCondition::absent(current_key)];
    let target = if phase == 0 { intent_key } else { candidate_key };
    let mutation = db::ControlMutation::new(phase_id(binding, phase)?, hash, generation,
        vec![ControlWrite { key: target, value }], vec![]);
    StoredCommand::new(ConditionalControlMutation::new(mutation, conditions))
        .map_err(|_| "REVISION_DEK_CONTROL_COMMAND_INVALID".into())
}
fn commit<C: CancellationProbe>(port: &mut BoundControlJournal<'_, C>, command: &StoredCommand,
    deadline: Instant, work: &impl Fn(Work) -> Result<(), OwnerError>,
) -> Result<(), String> {
    let mutation = db::control_mutation_identity(command.command().mutation().id())
        .map_err(|_| "REVISION_DEK_CONTROL_IDENTITY_INVALID")?;
    if port.transact_revision_dek_with_original_context(command, &mutation, deadline, work).is_ok() { return Ok(()); }
    match port.recover_revision_dek_with_original_context(command, &mutation, deadline, work) {
        Ok(db::CommitRecoveryDecision::Committed(_)) => Ok(()),
        _ => Err("REVISION_DEK_CONTROL_OUTCOME_UNKNOWN".into()),
    }
}
fn output(prepared: bool, historical: bool, reason: &str) -> DirectOutput {
    // `current` qualifies this command's Native result, never a Current key role.
    DirectOutput { current: prepared, line: format!(
        "{{\"event\":\"revision_dek_candidate\",\"prepared\":{prepared},\"historical_commit\":{historical},\"role\":\"candidate_nonserving\",\"reason\":\"{reason}\",\"current_selected\":false,\"key_use\":false,\"source_body_serving\":false,\"ready\":false}}") }
}

impl DirectRegistryOwner {
    pub(super) fn execute_revision_dek(mut self, command: &RevisionDekCommand, writer: &mut impl Write) -> Result<(), String> {
        let result = self.prepare_revision_dek_output(command);
        self.complete_output(result, writer)
    }
    fn prepare_revision_dek_output(&mut self, command_input: &RevisionDekCommand) -> Result<DirectOutput, String> {
        let deadline = self.owner.request.credential_context().0;
        let mut read = super::super::read_child::direct_operation(&mut self.owner).map_err(|e| e.code().to_owned())?;
        let native = read.fresh_root_context().map_err(|e| e.to_string())?;
        let facts = native.revision_dek_root_facts().map_err(|_| "REVISION_DEK_NATIVE_SCOPE_REFUSED")?;
        let poll = || native.work(Work::Checkpoint);
        let work = |work| match work {
            LegacyRevisionDpapiWork::NativeCall => native.revision_dek_native_attempt(),
            LegacyRevisionDpapiWork::Allocate(n) => native.work(Work::Allocate(n)),
            LegacyRevisionDpapiWork::Process(n) => native.work(Work::Process(n)),
        };
        let open = || native.work(Work::Open);
        let clear = |bytes| native.revision_dek_account_native_clear(bytes);
        let mut context = NativeContext { deadline, poll: &poll, work: &work, open: &open, clear: &clear };
        let user = os::observe_revision_user(&mut context).map_err(|error| error.code().to_owned())?;
        native.work(Work::Allocate(8192)).map_err(|e| e.code().to_owned())?;
        native.work(Work::Process(8192)).map_err(|e| e.code().to_owned())?;
        let binding = RevisionDekBinding::new(facts.data_root, facts.installation, facts.incarnation,
            user.user_scope_digest(), command_input.operation).map_err(|error| error.code().to_owned())?;
        let callback = |work| native.work(work);
        let journal = self.control.journal.as_mut().ok_or("REVISION_DEK_CONTROL_UNAVAILABLE")?;
        let mut port = bind(journal, &mut self.control.publisher, native.context())?;
        let keys = [key(db::REVISION_DEK_INTENT_KEY)?, key(db::REVISION_DEK_CANDIDATE_KEY)?, key(db::REVISION_DEK_CURRENT_KEY)?];
        let mut snapshot = port.read_control_keys_with_original_context(&keys, deadline, &callback)
            .map_err(|_| "REVISION_DEK_CONTROL_READ_REFUSED")?;
        if snapshot.get(&keys[2]).is_some() { return Ok(output(false, false, "REVISION_DEK_CURRENT_EXISTS")); }
        let original = load(&snapshot, &keys[0])?;
        let candidate = load(&snapshot, &keys[1])?;
        let historical = candidate.is_some();
        let intent = match original {
            Some(RevisionDekRecord::Intent(intent)) if intent.binding() == &binding => intent,
            Some(_) => return Ok(output(false, false, "REVISION_DEK_OPERATION_CONFLICT")),
            None => {
                if candidate.is_some() { return Ok(output(false, false, "REVISION_DEK_CONTROL_CONTRADICTION")); }
                for phase in [0, 1] {
                    if port.revision_dek_operation_exists_with_original_context(phase_id(&binding, phase)?, deadline, &callback)
                        .map_err(|_| "REVISION_DEK_OPERATION_LOOKUP_REFUSED")? {
                        return Ok(output(false, false, "REVISION_DEK_CONTROL_CONTRADICTION"));
                    }
                }
                // Missing control history never adopts an existing credential.
                if os::read_revision_dek(&binding, &mut context).map_err(|error| error.code().to_owned())?.is_some() {
                    return Ok(output(false, false, "REVISION_DEK_UNOWNED_BACKEND_RECORD"));
                }
                let prepared = os::prepare_revision_dek(&binding, &mut context).map_err(|error| error.code().to_owned())?;
                let intent = RevisionDekIntent::new(binding.clone(), facts.creator_epoch, prepared.protected_bytes())
                    .map_err(|error| error.code().to_owned())?;
                native.source_full_checkpoint().map_err(|_| "REVISION_DEK_NATIVE_SCOPE_REFUSED")?;
                if os::observe_revision_user(&mut context).map_err(|error| error.code().to_owned())?.user_scope_digest()
                    != binding.user_scope_digest() { return Ok(output(false, false, "REVISION_DEK_USER_SCOPE_MISMATCH")); }
                let value = stored(&RevisionDekRecord::Intent(intent.clone()))?;
                let request = command(&binding, 0, snapshot.generation(), value, None)?;
                commit(&mut port, &request, deadline, &callback)?;
                native.source_full_checkpoint().map_err(|_| "REVISION_DEK_NATIVE_SCOPE_REFUSED")?;
                #[cfg(test)]
                native.revision_dek_track_test_target(&binding).map_err(|e| e.code().to_owned())?;
                if let Err(error) = os::write_revision_dek_once(&prepared, &mut context) {
                    return Ok(output(false, false, error.code()));
                }
                snapshot = port.read_control_keys_with_original_context(&keys, deadline, &callback)
                    .map_err(|_| "REVISION_DEK_CONTROL_READ_REFUSED")?;
                intent
            }
        };
        if let Some(candidate) = candidate {
            if !matches!(candidate, RevisionDekRecord::CandidateNonServing(ref value) if value.intent() == &intent) {
                return Ok(output(false, true, "REVISION_DEK_CONTROL_CONTRADICTION"));
            }
        }
        let observed = match os::read_revision_dek(&binding, &mut context) {
            Ok(Some(value)) => value,
            Ok(None) => return Ok(output(false, historical, "REVISION_DEK_PENDING_BACKEND_ABSENT")),
            Err(error) => return Ok(output(false, historical, error.code())),
        };
        let candidate = RevisionDekCandidate::new(&intent, &binding, observed.protected_bytes())
            .map_err(|error| error.code().to_owned())?;
        if !historical {
            native.source_full_checkpoint().map_err(|_| "REVISION_DEK_NATIVE_SCOPE_REFUSED")?;
            let user = os::observe_revision_user(&mut context).map_err(|error| error.code().to_owned())?;
            if user.user_scope_digest() != binding.user_scope_digest() { return Ok(output(false, false, "REVISION_DEK_USER_SCOPE_MISMATCH")); }
            let value = stored(&RevisionDekRecord::CandidateNonServing(candidate.clone()))?;
            let intent_value = stored(&RevisionDekRecord::Intent(intent.clone()))?;
            let request = command(&binding, 1, snapshot.generation(), value, Some(intent_value))?;
            commit(&mut port, &request, deadline, &callback)?;
        }
        let final_view = port.read_control_keys_with_original_context(&keys, deadline, &callback)
            .map_err(|_| "REVISION_DEK_CONTROL_READ_REFUSED")?;
        if load(&final_view, &keys[0])? != Some(RevisionDekRecord::Intent(intent))
            || load(&final_view, &keys[1])? != Some(RevisionDekRecord::CandidateNonServing(candidate))
            || final_view.get(&keys[2]).is_some() { return Err("REVISION_DEK_FINAL_READBACK_REFUSED".into()); }
        if os::observe_revision_user(&mut context).map_err(|error| error.code().to_owned())?.user_scope_digest()
            != binding.user_scope_digest() { return Ok(output(false, historical, "REVISION_DEK_USER_SCOPE_MISMATCH")); }
        drop(port);
        let _published = published_metadata(journal, &mut self.control.publisher, deadline, &callback)?;
        Ok(output(true, historical, "REVISION_DEK_CANDIDATE_VERIFIED"))
    }
}
