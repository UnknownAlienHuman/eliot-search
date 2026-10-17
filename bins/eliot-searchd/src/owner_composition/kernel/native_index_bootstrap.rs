//! One retained intended C15 collection in the existing C02 journal; never visibility.
#![allow(dead_code)] // Parent native_index_runtime owns actual create/readback/adoption.
use search_contracts::{Blake3Digest32, CollectionGenerationId, indexed::CollectionSchema};
use search_control_redb::{self as db, ConditionalControlMutation, ControlKey, ControlValue,
    ControlRecordClass, ControlRecordCondition, ControlWrite, JournalIdentity, JournalLimits,
    JournalOperatingProfile, MutationId, MigrationBootstrapWork as Work};
use search_runtime_owner::OwnerError;
use crate::registry_control::SourceIdentityIdsV2;
use super::QualifiedNativeProfiles;
use super::super::read_child::fresh_root::{FreshRootContext, FreshRootCancellation};

const KEY: &[u8] = b"eliot.root.native-index-bootstrap.v1";
const MAGIC: &[u8; 8] = b"ELIDXB01";
const REFUSED: &str = "NATIVE_INDEX_BOOTSTRAP_REFUSED";
const UNKNOWN: &str = "NATIVE_INDEX_BOOTSTRAP_OUTCOME_UNKNOWN";
const READBACK: &str = "NATIVE_INDEX_CREATE_READBACK_REQUIRED";
type Port<'a> = db::BoundControlJournal<'a, FreshRootCancellation>;

/// Technical disposition only. Dispatched/Indexed never authorize another create.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IndexBootstrapDisposition { Prepared, Dispatched, Indexed, OutcomeUnknown }

/// Original durable coordinates; private construction grants no C15/adoption readiness.
pub(super) struct NativeIndexBootstrap {
    schema: CollectionSchema,
    disposition: IndexBootstrapDisposition,
    row: Option<ControlValue>,
    identity: JournalIdentity,
    pending: Option<ConditionalControlMutation>,
}
impl NativeIndexBootstrap {
    pub(super) const fn schema(&self) -> &CollectionSchema { &self.schema }
    pub(super) const fn generation(&self) -> CollectionGenerationId { *self.schema.collection_generation_id() }
    pub(super) const fn disposition(&self) -> IndexBootstrapDisposition {
        if self.pending.is_some() { IndexBootstrapDisposition::OutcomeUnknown } else { self.disposition }
    }
}

// Fixed-width content-free metadata. Both CAS predecessors are actual C02 reads.
struct Row {
    generation: CollectionGenerationId, schema: Blake3Digest32, operation: MutationId,
    prepare_before: u64, dispatch_before: u64, dispatched: bool,
}
impl Row {
    fn value(&self) -> Result<ControlValue, &'static str> {
        let mut out = Vec::with_capacity(105);
        out.extend_from_slice(MAGIC); out.push(u8::from(self.dispatched));
        out.extend_from_slice(self.generation.as_bytes()); out.extend_from_slice(self.schema.as_bytes());
        out.extend_from_slice(&self.operation.0); out.extend_from_slice(&self.prepare_before.to_be_bytes());
        out.extend_from_slice(&self.dispatch_before.to_be_bytes());
        ControlValue::new(ControlRecordClass::State, out, JournalLimits::BASELINE).map_err(|_| REFUSED)
    }
    fn decode(value: &ControlValue) -> Result<Self, &'static str> {
        let bytes = value.as_bytes();
        if value.class() != ControlRecordClass::State || bytes.len() != 105 || &bytes[..8] != MAGIC
            || bytes[8] > 1 { return Err(REFUSED); }
        let row = Self {
            generation: CollectionGenerationId::from_bytes(bytes[9..25].try_into().map_err(|_| REFUSED)?),
            schema: Blake3Digest32::from_stored_bytes(bytes[25..57].try_into().map_err(|_| REFUSED)?),
            operation: MutationId(bytes[57..89].try_into().map_err(|_| REFUSED)?),
            prepare_before: u64::from_be_bytes(bytes[89..97].try_into().map_err(|_| REFUSED)?),
            dispatch_before: u64::from_be_bytes(bytes[97..105].try_into().map_err(|_| REFUSED)?),
            dispatched: bytes[8] == 1,
        };
        if (!row.dispatched && row.dispatch_before != 0)
            || (row.dispatched && row.dispatch_before <= row.prepare_before)
            || row.value()? != *value { return Err(REFUSED); }
        Ok(row)
    }
}
fn key() -> Result<ControlKey, &'static str> {
    ControlKey::new(KEY.to_vec(), JournalLimits::BASELINE).map_err(|_| REFUSED)
}
fn operation(identity: JournalIdentity, generation: CollectionGenerationId, schema: Blake3Digest32) -> MutationId {
    let mut hash = blake3::Hasher::new(); hash.update(b"eliot/root/index-bootstrap-operation/v1\0");
    hash.update(identity.data_root_id.as_bytes()); hash.update(identity.installation_incarnation_id.as_bytes());
    hash.update(generation.as_bytes()); hash.update(schema.as_bytes()); MutationId(*hash.finalize().as_bytes())
}
fn command(row: &Row, old: Option<&ControlValue>, before: u64)
    -> Result<ConditionalControlMutation, &'static str> {
    let value = row.value()?;
    let mut hash = blake3::Hasher::new(); hash.update(b"eliot/root/index-bootstrap-command/v1\0");
    hash.update(value.as_bytes()); hash.update(&before.to_be_bytes());
    if let Some(old) = old { hash.update(old.as_bytes()); }
    let digest = Blake3Digest32::from_bytes(*hash.finalize().as_bytes());
    let id = if row.dispatched { MutationId(*blake3::hash(digest.as_bytes()).as_bytes()) } else { row.operation };
    let key = key()?;
    let condition = old.map_or_else(|| ControlRecordCondition::absent(key.clone()),
        |value| ControlRecordCondition::exact(key.clone(), value.clone()));
    Ok(ConditionalControlMutation::new(db::ControlMutation::new(id, digest, before,
        vec![ControlWrite { key, value }], vec![]), vec![condition]))
}
fn recover(port: &mut Port<'_>, native: &FreshRootContext<'_, '_>, command: &ConditionalControlMutation)
    -> Result<(), &'static str> {
    let identity = db::control_mutation_identity(command.mutation().id()).map_err(|_| REFUSED)?;
    match port.recover_records_with_original_context(command, &identity, native.original_deadline(), &|unit| native.work(unit)) {
        Ok(db::CommitRecoveryDecision::Committed(_)) => Ok(()), _ => Err(UNKNOWN),
    }
}
fn commit_once(port: &mut Port<'_>, native: &FreshRootContext<'_, '_>, command: &ConditionalControlMutation)
    -> Result<(), &'static str> {
    let identity = db::control_mutation_identity(command.mutation().id()).map_err(|_| REFUSED)?;
    match port.transact_records_with_original_context(command, &identity, native.original_deadline(), &|unit| native.work(unit)) {
        Ok(_) => Ok(()),
        Err(error) if error.control == db::ControlError::CommitOutcomeUnknown => recover(port, native, command),
        Err(_) => Err(REFUSED),
    }
}
fn root_identity(native: &FreshRootContext<'_, '_>, identity: JournalIdentity) -> Result<(), &'static str> {
    let root = native.revision_dek_root_facts().map_err(|_| REFUSED)?;
    if identity.data_root_id != root.data_root || identity.installation_incarnation_id != root.incarnation
        || identity.owner_epoch != root.creator_epoch { return Err(REFUSED); }
    native.work(Work::Allocate(16 * 1024)).and_then(|()| native.work(Work::Process(16 * 1024))).map_err(|_| REFUSED)
}

/// Read the original row, actual adoption and current route before any new RNG.
/// DIRECT absence is established by C02's checked DIRECT snapshot: that owner
/// rejects publication/visibility rows in DIRECT. No epoch-zero state is built here.
pub(super) fn prepare_index_bootstrap(native: &FreshRootContext<'_, '_>,
    journal: &mut db::PersistentControlJournal, publisher: &mut db::ControlSnapshotPublisher,
    profiles: &QualifiedNativeProfiles) -> Result<NativeIndexBootstrap, &'static str> {
    let identity = journal.identity(); root_identity(native, identity)?;
    let work = |unit| native.work(unit);
    let mut port = journal.bind_control_port_with_original_context::<FreshRootCancellation, OwnerError>(
        publisher, native.original_deadline(), &work).map_err(|_| REFUSED)?;
    let key = key()?;
    let first = port.read_control_keys_with_original_context(std::slice::from_ref(&key), native.original_deadline(), &work).map_err(|_| UNKNOWN)?;
    let adoption = port.read_indexed_profile_adoption_with_original_context(native.original_deadline(), &work).map_err(|_| UNKNOWN)?;
    let mode = port.read_operating_profile_with_original_context(native.original_deadline(), &work).map_err(|_| UNKNOWN)?;
    let route = match mode {
        JournalOperatingProfile::Indexed => Some(*port.read_publication_checkpoint_with_original_context(
            native.original_deadline(), &work).map_err(|_| UNKNOWN)?.ok_or(REFUSED)?.visibility()),
        JournalOperatingProfile::Direct if adoption.is_none() => None,
        _ => return Err(REFUSED),
    };
    let last = port.read_control_keys_with_original_context(std::slice::from_ref(&key), native.original_deadline(), &work).map_err(|_| UNKNOWN)?;
    if first.identity() != identity || last.identity() != identity || first.generation() != last.generation()
        || first.get(&key) != last.get(&key) { return Err(REFUSED); }
    if let Some(route) = route {
        let (schema, _) = profiles.collection_schema(identity.installation_incarnation_id, route.collection_generation_id)?;
        if *schema.identity().map_err(|_| REFUSED)?.as_digest() != route.schema_identity_digest { return Err(REFUSED); }
        root_identity(native, identity)?;
        return Ok(NativeIndexBootstrap { schema, disposition: IndexBootstrapDisposition::Indexed,
            row: first.get(&key).cloned(), identity, pending: None });
    }
    if adoption.is_some() { return Err(REFUSED); }
    let (row, schema, value) = if let Some(value) = first.get(&key) {
        let row = Row::decode(value)?;
        let (schema, _) = profiles.collection_schema(identity.installation_incarnation_id, row.generation)?;
        if *schema.identity().map_err(|_| REFUSED)?.as_digest() != row.schema
            || row.operation != operation(identity, row.generation, row.schema) { return Err(REFUSED); }
        (row, schema, value.clone())
    } else {
        if !first.missing_keys().contains(&key) { return Err(REFUSED); }
        let ids = SourceIdentityIdsV2::<FreshRootCancellation>::bind(native.original_deadline(), &work);
        let generation = ids.fresh_collection_generation_id(native.context()).map_err(|_| REFUSED)?;
        let (schema, _) = profiles.collection_schema(identity.installation_incarnation_id, generation)?;
        let digest = *schema.identity().map_err(|_| REFUSED)?.as_digest();
        let row = Row { generation, schema: digest, operation: operation(identity, generation, digest),
            prepare_before: first.generation(), dispatch_before: 0, dispatched: false };
        let value = row.value()?;
        let command = command(&row, None, first.generation())?;
        commit_once(&mut port, native, &command)?;
        (row, schema, value)
    };
    let check = port.read_control_keys_with_original_context(std::slice::from_ref(&key), native.original_deadline(), &work).map_err(|_| UNKNOWN)?;
    if check.identity() != identity || check.get(&key) != Some(&value) { return Err(UNKNOWN); }
    root_identity(native, identity)?;
    Ok(NativeIndexBootstrap { schema, row: Some(value), identity, pending: None,
        disposition: if row.dispatched { IndexBootstrapDisposition::Dispatched } else { IndexBootstrapDisposition::Prepared } })
}

/// One CAS/readback before the parent may attempt actual C15 create. A retained
/// dispatched/unknown marker requires independent plane readback, never redispatch.
pub(super) fn mark_index_create_dispatched(native: &FreshRootContext<'_, '_>,
    journal: &mut db::PersistentControlJournal, publisher: &mut db::ControlSnapshotPublisher,
    intended: &mut NativeIndexBootstrap) -> Result<(), &'static str> {
    if journal.identity() != intended.identity { return Err(REFUSED); }
    root_identity(native, intended.identity)?;
    let work = |unit| native.work(unit);
    let mut port = journal.bind_control_port_with_original_context::<FreshRootCancellation, OwnerError>(
        publisher, native.original_deadline(), &work).map_err(|_| REFUSED)?;
    if let Some(command) = &intended.pending {
        recover(&mut port, native, command)?;
        // Even recovered metadata is not permission to repeat an external effect.
        return Err(READBACK);
    }
    if intended.disposition != IndexBootstrapDisposition::Prepared { return Err(READBACK); }
    if port.read_operating_profile_with_original_context(native.original_deadline(), &work).map_err(|_| UNKNOWN)?
        != JournalOperatingProfile::Direct { return Err(REFUSED); }
    let key = key()?;
    let read = port.read_control_keys_with_original_context(std::slice::from_ref(&key), native.original_deadline(), &work).map_err(|_| UNKNOWN)?;
    let old = intended.row.as_ref().ok_or(REFUSED)?;
    if read.identity() != intended.identity || read.get(&key) != Some(old) { return Err(REFUSED); }
    let mut row = Row::decode(old)?; row.dispatched = true; row.dispatch_before = read.generation();
    let command = command(&row, Some(old), read.generation())?;
    intended.pending = Some(command.clone());
    commit_once(&mut port, native, &command)?;
    let value = row.value()?;
    let after = port.read_control_keys_with_original_context(std::slice::from_ref(&key), native.original_deadline(), &work).map_err(|_| UNKNOWN)?;
    if after.identity() != intended.identity || after.get(&key) != Some(&value) { return Err(UNKNOWN); }
    root_identity(native, intended.identity)?;
    intended.row = Some(value); intended.disposition = IndexBootstrapDisposition::Dispatched; intended.pending = None;
    Ok(())
}
