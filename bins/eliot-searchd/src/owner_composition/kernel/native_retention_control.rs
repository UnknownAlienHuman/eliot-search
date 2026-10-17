//! Source-history and retained-object records in the existing DIRECT journal.

use std::{fmt, time::Instant};
use search_contracts::{self as c, CanonicalValue as V};
use search_control_redb::{self as db, BoundControlJournal, ConditionalControlMutation,
    ControlError, ControlKey, ControlMutation, ControlRecordClass as Class,
    ControlRecordCondition as Condition, ControlValue, ControlWrite, JournalLimits,
    MigrationBootstrapFailure, MigrationBootstrapWork as Work, MutationId,
    ScopedControlRecordsReadback};
#[cfg(feature = "wave3-index")]
use search_control_redb::{source_retention::{
    DerivedArtifactObjectIntentReadback, DerivedArtifactReservationReadback,
    DerivedArtifactScopeGuardKeysV1, RevisionUseUsageReadback,
}};
#[cfg(feature = "wave3-index")]
use search_revision_store::{CipherSuite, EncryptionBinding, derived_artifact::{
    DerivedArtifactObjectRefV1, DerivedArtifactStorageV1,
}};
use search_ports::{CancellationProbe, DisclosureClass, IdempotencyClass, MutationIdentity,
    OperationContext, Port, PortErrorKind, PortFailure, PortRetryability};
use search_runtime_owner::OwnerError;
use search_source_history::{prepare::{SourceRevisionPreparePort, SourceRevisionPresealProfile,
    decode_source_revision_preseal_profile, encode_source_revision_preseal_profile,
    late_bind_object_residency}, revision::{SourceRevisionControlPort, SourceRevisionHead,
    SourceRevisionMutation, SourceRevisionMutationReadback, SourceRevisionRegistrationRequest,
    SourceRevisionSourceState, decode_source_revision_mutation_readback, decode_source_revision_record,
    encode_source_revision_mutation_readback, encode_source_revision_record}};
use search_source_registry::{api::NamespaceCutoverState, recovery::RegistryStateRecord,
    root::RootRecord, source::{RegisteredSource, SourceLifecycle}};
#[cfg(feature = "wave3-index")]
use crate::config_composition::EffectiveDaemonConfig;
use super::super::read_child::fresh_root::RetentionSecurity;

const LIMITS: JournalLimits = JournalLimits::BASELINE;
const PREFIX: &[u8] = b"eliot.control.native-retention.v1/";
const MAX_METADATA: usize = 8192;

pub(super) struct RetentionOperationReadback {
    pub(super) profile: Option<SourceRevisionPresealProfile>,
    pub(super) result: Option<SourceRevisionMutationReadback>,
    pub(super) reservation: Option<db::source_retention::SealReservationReadback>,
    pub(super) object: Option<db::source_retention::ObjectPublicationIntentReadback>,
    pub(super) reservation_id: MutationId,
}

fn address(phase: &str, parts: &[&[u8]], work: &dyn Fn(Work) -> Result<(), OwnerError>)
    -> Result<ControlKey, RetentionControlError> {
    let bytes = parts.iter().try_fold(PREFIX.len() + phase.len() + 64,
        |sum, part| sum.checked_add(part.len()).ok_or(ControlError::BudgetExceeded))?;
    let bytes = bytes.checked_mul(4).ok_or(ControlError::BudgetExceeded)?;
    for unit in [Work::Checkpoint, Work::Allocate(bytes), Work::Process(bytes)] {
        work(unit).map_err(|first| RetentionControlError { control: ControlError::BudgetExceeded, first: Some(first) })?;
    }
    let mut values = vec![V::Text(c::CanonicalText::new(phase).map_err(|_| ControlError::InvalidKey)?)];
    for part in parts { values.push(V::Bytes(c::BoundedBytes::new(part.to_vec()).map_err(|_| ControlError::InvalidKey)?)); }
    let encoded = c::to_canonical_cbor(&V::Array(c::BoundedList::new(values).map_err(|_| ControlError::InvalidKey)?))
        .map_err(|_| ControlError::InvalidKey)?;
    let mut key = PREFIX.to_vec(); key.extend_from_slice(encoded.as_slice());
    Ok(ControlKey::new(key, LIMITS)?)
}

/// Existing exact C02 source-revision row address for a derived-artifact guard.
pub(super) fn source_revision_guard_key(
    namespace: c::SourceNamespaceId,
    source: c::SourceId,
    revision: c::SourceRevisionId,
    work: &dyn Fn(Work) -> Result<(), OwnerError>,
) -> Result<ControlKey, RetentionControlError> {
    address(
        "revision",
        &[namespace.as_bytes(), source.as_bytes(), revision.as_bytes()],
        work,
    )
}

/// Technical phase ID, never a source/revision/domain identity.
pub(super) fn phase_id(operation: &c::OpaqueId, phase: &str,
    work: &dyn Fn(Work) -> Result<(), OwnerError>) -> Result<(MutationId, c::Blake3Digest32), RetentionControlError> {
    let key = address(phase, &[operation.as_str().as_bytes()], work)?;
    work(Work::Process(key.as_bytes().len())).map_err(|first| RetentionControlError {
        control: ControlError::BudgetExceeded, first: Some(first) })?;
    let digest = c::blake3_raw(&c::CanonicalDigestDomain::parse("eliot/raw/native-retention-phase/v1")
        .map_err(|_| ControlError::InvalidValue)?, key.as_bytes(),
        c::DigestInputLimit::new(LIMITS.max_key_bytes).map_err(|_| ControlError::BudgetExceeded)?)
        .map_err(|_| ControlError::InvalidValue)?;
    Ok((MutationId(*digest.as_bytes()), digest))
}

/// Resolves all durable phases before source reads, clocks or identity draws.
/// Returned metadata is historical; Native/source/policy qualification follows.
pub(super) fn lookup_before_source<C: CancellationProbe>(journal: &mut db::PersistentControlJournal,
    publisher: &mut db::ControlSnapshotPublisher, operation: &c::OpaqueId,
    context: &OperationContext<C>, deadline: Instant, work: &dyn Fn(Work) -> Result<(), OwnerError>,
) -> Result<RetentionOperationReadback, RetentionControlError> {
    context.preflight().map_err(|_| ControlError::ReadCancelled)?;
    let profile_key = address("preseal", &[operation.as_str().as_bytes()], work)?;
    let result_key = address("result", &[operation.as_str().as_bytes()], work)?;
    let (reservation_id, _) = phase_id(operation, "seal", work)?;
    let reservation_key = db::source_retention::revision_seal_reservation_key(reservation_id, LIMITS)?;
    let object_key = db::source_retention::object_publication_intent_key(reservation_id, LIMITS)?;
    let keys = [profile_key.clone(), result_key.clone(), reservation_key.clone(), object_key.clone()];
    let port = journal.bind_control_port::<C>(publisher)?;
    let callback = |unit| work(unit);
    let read = port.read_control_keys_with_original_context(&keys, deadline, &callback)?;
    for unit in [Work::Allocate(16 * MAX_METADATA), Work::Process(16 * MAX_METADATA)] {
        work(unit).map_err(|first| RetentionControlError { control: ControlError::BudgetExceeded, first: Some(first) })?;
    }
    let get = |key: &ControlKey, class| -> Result<Option<&ControlValue>, RetentionControlError> {
        match read.get(key) {
            Some(value) if value.class() == class => Ok(Some(value)),
            None if read.missing_keys().contains(key) => Ok(None),
            _ => Err(ControlError::StoreCorrupt.into()),
        }
    };
    let profile = get(&profile_key, Class::Operation)?.map(|value|
        decode_source_revision_preseal_profile(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)).transpose()?;
    let result = get(&result_key, Class::Operation)?.map(|value|
        decode_source_revision_mutation_readback(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)).transpose()?;
    let reservation = get(&reservation_key, Class::State)?.map(|_| db::source_retention::SealReservationReadback::read(&read, reservation_id, LIMITS)).transpose()?;
    let object = get(&object_key, Class::State)?.map(|_| db::source_retention::ObjectPublicationIntentReadback::read(&read, reservation_id, LIMITS)).transpose()?;
    if profile.as_ref().is_some_and(|profile| &profile.operation_id != operation)
        || result.as_ref().is_some_and(|result| &result.operation_id != operation)
        || reservation.as_ref().is_some_and(|reservation| reservation.operation_id() != operation)
        || object.as_ref().is_some_and(|object| object.operation_id() != operation)
        || ((result.is_some() || reservation.is_some()) && profile.is_none())
        || (object.is_some() && reservation.is_none()) || (result.is_some() && object.is_none())
        || object.as_ref().zip(reservation.as_ref()).is_some_and(|(object, reservation)| !object.binds(reservation))
    { return Err(ControlError::StoreCorrupt.into()); }
    Ok(RetentionOperationReadback { profile, result, reservation, object, reservation_id })
}

#[derive(Clone, Copy)]
pub(super) struct RetentionControlError { pub(super) control: ControlError, pub(super) first: Option<OwnerError> }
impl From<ControlError> for RetentionControlError {
    fn from(control: ControlError) -> Self { Self { control, first: None } }
}
impl From<MigrationBootstrapFailure<OwnerError>> for RetentionControlError {
    fn from(error: MigrationBootstrapFailure<OwnerError>) -> Self {
        Self { control: error.control, first: error.first_context }
    }
}
impl fmt::Debug for RetentionControlError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Display::fmt(self, out) }
}
impl fmt::Display for RetentionControlError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Display::fmt(&self.control, out) }
}
impl PortFailure for RetentionControlError {
    fn kind(&self) -> PortErrorKind {
        match self.control {
            ControlError::CommitOutcomeUnknown => PortErrorKind::OutcomeUnknown,
            ControlError::OperationConflict => PortErrorKind::Conflict,
            ControlError::BudgetExceeded => PortErrorKind::ResourceExhausted,
            _ => PortErrorKind::Quarantined,
        }
    }
    fn retryability(&self) -> PortRetryability { PortRetryability::AfterReadback }
    fn disclosure(&self) -> DisclosureClass { DisclosureClass::Redacted }
    fn operation_id(&self) -> Option<&c::OpaqueId> { None }
}

/// Borrowed server composition; no database, grant, ID allocator or query store.
pub(super) struct NativeRetentionControl<'journal, 'work, C: CancellationProbe> {
    port: BoundControlJournal<'journal, C>,
    source_namespace_id: c::SourceNamespaceId,
    source_id: c::SourceId,
    source_state: SourceRevisionSourceState,
    content_digest: c::Blake3Digest32,
    byte_length: u64,
    stability_receipt_ref: c::ReceiptRef,
    guards: Vec<Condition>,
    deadline: Instant,
    work: &'work dyn Fn(Work) -> Result<(), OwnerError>,
    object: Option<FinalObject>,
}

#[cfg(feature = "wave3-index")]
pub(super) struct DerivedArtifactReadback {
    pub(super) reservation: DerivedArtifactReservationReadback,
    pub(super) intent: DerivedArtifactObjectIntentReadback,
    pub(super) selected: super::retention::Selected,
}

struct FinalObject {
    id: c::OpaqueId,
    physical: c::ObjectResidencyKeyDigest,
    metadata: Vec<u8>,
    reservation: MutationId,
}

impl<C: CancellationProbe> Port for NativeRetentionControl<'_, '_, C> {
    type Error = RetentionControlError;
    type Cancellation = C;
}

impl<'journal, 'work, C: CancellationProbe> NativeRetentionControl<'journal, 'work, C> {
    pub(super) const fn deadline(&self) -> Instant { self.deadline }

    #[cfg(feature = "wave3-index")]
    pub(super) fn verify_current_source(&self) -> Result<(), RetentionControlError> {
        let keys = self.scope(&[])?;
        let read = self.read(&keys)?;
        self.require_source(&read)
    }

    /// The actual journal identity/generation with this source's complete guard
    /// set, using bounded key reads rather than a full projection-population scan.
    #[cfg(feature = "wave3-index")]
    pub(super) fn projection_metadata_checkpoint(
        &self,
    ) -> Result<(db::JournalIdentity, u64), RetentionControlError> {
        let keys = self.scope(&[])?;
        let read = self.read(&keys)?;
        self.require_source(&read)?;
        Ok((read.identity(), read.generation()))
    }

    /// Reads C07's exact late intent/reservation and actual selected Current
    /// in one guarded C02 transaction. This grants no write or seal capability.
    #[cfg(feature = "wave3-index")]
    pub(super) fn read_derived_artifact_evidence(
        &self,
        reference: &DerivedArtifactObjectRefV1,
        source_record: &c::SourceRevision,
        expected_residency: &[u8],
        expected_encryption: &EncryptionBinding,
        config: &EffectiveDaemonConfig,
    ) -> Result<DerivedArtifactReadback, RetentionControlError> {
        let descriptor = reference.descriptor();
        let source = descriptor.source();
        if source.source_namespace_id != self.source_namespace_id
            || source.source_id != self.source_id
            || source_record.source_id != source.source_id
            || source_record.revision_id != source.revision_id
            || source_record.content_digest != source.content_digest
            || source_record.byte_length != source.byte_length
        {
            return Err(ControlError::OperationConflict.into());
        }
        let (reservation_operation, _) = phase_id(
            descriptor.operation_id(),
            &format!("derived-seal-{}", descriptor.kind().as_str()),
            self.work,
        )?;
        let guards = self.derived_artifact_scope_guard_keys(source)?;
        let keys = db::source_retention::derived_artifact_object_intent_keys(
            reservation_operation,
            &guards,
            LIMITS,
        )?;
        let keys = self.scope(&keys)?;
        let read = self.read(&keys)?;
        self.require_source(&read)?;
        self.verify_derived_artifact_guards(&read, source_record, &guards)?;
        let revision_keys = db::source_retention::revision_use_keys(LIMITS)?;
        let selected = self.selected_from_read(&read, &revision_keys, config)?;
        let reservation = DerivedArtifactReservationReadback::read(
            &read,
            reservation_operation,
            LIMITS,
        )?;
        let intent = DerivedArtifactObjectIntentReadback::read(
            &read,
            reservation_operation,
            LIMITS,
        )?;
        verify_derived_artifact_readback(
            reference,
            &reservation,
            &intent,
            &guards,
            expected_residency,
            expected_encryption,
            &selected,
        )?;
        Ok(DerivedArtifactReadback { reservation, intent, selected })
    }

    #[cfg(feature = "wave3-index")]
    fn derived_artifact_scope_guard_keys(
        &self,
        source: &c::SourceRevisionRef,
    ) -> Result<DerivedArtifactScopeGuardKeysV1, RetentionControlError> {
        // bind_for_serving fixes order to Source, Namespace, Root, Membership,
        // then the current residency-policy and purge-fence predicates.
        if self.guards.len() != 6 {
            return Err(ControlError::OperationConflict.into());
        }
        let source_revision = source_revision_guard_key(
            source.source_namespace_id,
            source.source_id,
            source.revision_id,
            self.work,
        )?;
        DerivedArtifactScopeGuardKeysV1::new(
            self.guards[2].key().clone(),
            source_revision,
            self.guards[1].key().clone(),
            self.guards[4].key().clone(),
            self.guards[5].key().clone(),
        )
        .map_err(Into::into)
    }

    #[cfg(feature = "wave3-index")]
    fn verify_derived_artifact_guards(
        &self,
        read: &ScopedControlRecordsReadback,
        source_record: &c::SourceRevision,
        keys: &DerivedArtifactScopeGuardKeysV1,
    ) -> Result<(), RetentionControlError> {
        let source_revision = source_revision_guard_key(
            self.source_namespace_id,
            self.source_id,
            source_record.revision_id,
            self.work,
        )?;
        let value = ControlValue::new(
            Class::Revision,
            encode_source_revision_record(source_record)
                .map_err(|_| ControlError::StoreCorrupt)?,
            LIMITS,
        )?;
        let conditions = [
            self.guards[2].clone(),
            Condition::exact(source_revision, value),
            self.guards[1].clone(),
            self.guards[4].clone(),
            self.guards[5].clone(),
        ];
        if [
            keys.root_owner(),
            keys.source_revision(),
            keys.namespace_owner(),
            keys.residency_policy(),
            keys.purge_fence(),
        ] != conditions.each_ref().map(Condition::key)
        {
            return Err(ControlError::OperationConflict.into());
        }
        super::artifact_store::verify_guard_values(&conditions, read)
            .map_err(|_| ControlError::OperationConflict.into())
    }

    #[cfg(feature = "wave3-index")]
    fn selected_from_read(
        &self,
        read: &ScopedControlRecordsReadback,
        keys: &[ControlKey],
        config: &EffectiveDaemonConfig,
    ) -> Result<super::retention::Selected, RetentionControlError> {
        self.charge(16 * 32768)?;
        let state = |index: usize| -> Result<&ControlValue, RetentionControlError> {
            read.get(&keys[index])
                .filter(|value| value.class() == Class::State)
                .ok_or_else(|| ControlError::StoreCorrupt.into())
        };
        let current = state(3)?.as_bytes().to_vec();
        let search_os_secrets::RevisionDekSelectionRecord::Current(current_record) =
            search_os_secrets::decode_revision_dek_selection_record(&current)
                .map_err(|_| ControlError::StoreCorrupt)?
        else {
            return Err(ControlError::OperationConflict.into());
        };
        let search_os_secrets::RevisionDekRecord::Intent(intent) =
            search_os_secrets::decode_revision_dek_record(state(1)?.as_bytes())
                .map_err(|_| ControlError::StoreCorrupt)?
        else {
            return Err(ControlError::StoreCorrupt.into());
        };
        if current_record.intent().candidate().intent() != &intent
            || current_record.intent().policy().backend() != config.secrets_settings().backend()
            || current_record.intent().policy().secrets_section_digest()
                != *search_os_secrets::section_digest(config.secrets_settings()).as_bytes()
        {
            return Err(ControlError::OperationConflict.into());
        }
        let preparation = intent.binding().protection_entropy();
        let usage = RevisionUseUsageReadback::read(read, &preparation, LIMITS)?;
        let encryption = EncryptionBinding {
            key_reference: intent
                .binding()
                .key_reference()
                .map_err(|_| ControlError::StoreCorrupt)?,
            key_version: c::NonZeroRevision::new(intent.binding().key_version())
                .map_err(|_| ControlError::StoreCorrupt)?,
            cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
        };
        Ok(super::retention::Selected {
            current,
            preparation,
            claim: usage.residency_canonical().map(<[u8]>::to_vec),
            encryption,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind(journal: &'journal mut db::PersistentControlJournal,
        publisher: &'journal mut db::ControlSnapshotPublisher, source: &RegisteredSource,
        namespace: &NamespaceCutoverState, root: &RootRecord, security: &RetentionSecurity,
        deadline: Instant, work: &'work dyn Fn(Work) -> Result<(), OwnerError>,
    ) -> Result<Self, RetentionControlError> {
        security.checkpoint().map_err(|_| ControlError::OperationConflict)?;
        Self::bind_conditions(journal, publisher, source, namespace, root, security.conditions(),
            None, deadline, work)
    }

    /// Bind an exact active member to the existing serving domain's head.
    /// The caller retains that domain's grant/lock and obtains these predicates
    /// from `NativeSecurityDomain::retained_source_read_conditions`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind_for_serving(journal: &'journal mut db::PersistentControlJournal,
        publisher: &'journal mut db::ControlSnapshotPublisher, source: &RegisteredSource,
        namespace: &NamespaceCutoverState, root: &RootRecord,
        member: &search_source_registry::membership::MembershipRecord,
        conditions: &[Condition; 2], deadline: Instant,
        work: &'work dyn Fn(Work) -> Result<(), OwnerError>,
    ) -> Result<Self, RetentionControlError> {
        Self::bind_active_member(journal, publisher, source, namespace, root, member,
            conditions, deadline, work)
    }

    /// Internal Root bootstrap reads only C07 graph headers. The verified native
    /// owner and current namespace predicates authorize this technical metadata
    /// read; this does not issue a query grant or source-body read permission.
    #[cfg(feature = "wave4-query")]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind_for_projection_metadata(
        native: &super::super::read_child::fresh_root::FreshRootContext<'_, '_>,
        journal: &'journal mut db::PersistentControlJournal,
        publisher: &'journal mut db::ControlSnapshotPublisher, source: &RegisteredSource,
        namespace: &NamespaceCutoverState, root: &RootRecord,
        member: &search_source_registry::membership::MembershipRecord,
        conditions: &[Condition; 2], deadline: Instant,
        work: &'work dyn Fn(Work) -> Result<(), OwnerError>,
    ) -> Result<Self, RetentionControlError> {
        native.source_full_checkpoint().map_err(|_| ControlError::OperationConflict)?;
        Self::bind_active_member(journal, publisher, source, namespace, root, member,
            conditions, deadline, work)
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_active_member(journal: &'journal mut db::PersistentControlJournal,
        publisher: &'journal mut db::ControlSnapshotPublisher, source: &RegisteredSource,
        namespace: &NamespaceCutoverState, root: &RootRecord,
        member: &search_source_registry::membership::MembershipRecord,
        conditions: &[Condition; 2], deadline: Instant,
        work: &'work dyn Fn(Work) -> Result<(), OwnerError>,
    ) -> Result<Self, RetentionControlError> {
        if source.lifecycle() != SourceLifecycle::Active
            || member.lifecycle() != search_source_registry::membership::MembershipLifecycle::Active
            || member.key().source_identity != *source.identity()
            || member.permanent_v2().is_none()
        { return Err(ControlError::OperationConflict.into()); }
        Self::bind_conditions(journal, publisher, source, namespace, root, conditions,
            Some(member), deadline, work)
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_conditions(journal: &'journal mut db::PersistentControlJournal,
        publisher: &'journal mut db::ControlSnapshotPublisher, source: &RegisteredSource,
        namespace: &NamespaceCutoverState, root: &RootRecord, conditions: &[Condition; 2],
        member: Option<&search_source_registry::membership::MembershipRecord>,
        deadline: Instant, work: &'work dyn Fn(Work) -> Result<(), OwnerError>,
    ) -> Result<Self, RetentionControlError> {
        let source_state = match source.lifecycle() {
            SourceLifecycle::Active => SourceRevisionSourceState::Active,
            SourceLifecycle::Retired => SourceRevisionSourceState::Retired,
        };
        if source.permanent_admission().is_none()
            || source.identity().source_namespace_id != namespace.ownership.source_namespace_id
            || source.binding().root_binding_id() != root.root_binding_id
            || namespace.ownership.status != c::NamespaceOwnershipStatus::Active
            || namespace.fenced || namespace.preparation.is_some() || namespace.activation.is_some()
            || root.status != search_source_registry::root::RootStatus::Active
        { return Err(ControlError::OperationConflict.into()); }
        // Complete source/namespace owner encodings are finite technical metadata.
        // Pay before cloning or encoding any of them.
        for unit in [Work::Checkpoint, Work::Allocate(4 * LIMITS.max_value_bytes),
            Work::Process(4 * LIMITS.max_value_bytes)] {
            work(unit).map_err(|first| RetentionControlError { control: ControlError::BudgetExceeded, first: Some(first) })?;
        }
        let mut records = vec![RegistryStateRecord::Source(source.clone()),
            RegistryStateRecord::Namespace(namespace.clone()), RegistryStateRecord::Root(root.clone())];
        if let Some(member) = member { records.push(RegistryStateRecord::Membership(member.clone())); }
        let mut guards = Vec::with_capacity(records.len() + conditions.len());
        for record in records {
            let family = match &record {
                RegistryStateRecord::Source(_) => db::RegistryStateFamily::Source,
                RegistryStateRecord::Namespace(_) => db::RegistryStateFamily::Namespace,
                RegistryStateRecord::Root(_) => db::RegistryStateFamily::Root,
                RegistryStateRecord::Membership(_) => db::RegistryStateFamily::Membership,
                _ => return Err(ControlError::InvalidValue.into()),
            };
            let key = db::RegistryStateKey::new(family,
                record.key().encode().map_err(|_| ControlError::InvalidValue)?, LIMITS)?;
            let encoded = match &record {
                RegistryStateRecord::Root(_) => record.encode(),
                RegistryStateRecord::Membership(_) => record.encode_with_profile("eliot.registry-state.v5"),
                _ => record.encode_with_profile("eliot.registry-state.v4"),
            }
                .map_err(|_| ControlError::InvalidValue)?;
            guards.push(key.current_condition(&encoded, LIMITS)?);
        }
        guards.extend_from_slice(conditions);
        let source_namespace_id = source.identity().source_namespace_id;
        let content_digest = source.binding().content_digest();
        let byte_length = source.binding().content_bytes();
        let stability_receipt_ref = source.binding().observation_receipt().clone();
        let source_id = source.identity().source_id;
        let port = journal.bind_control_port(publisher)?;
        let adapter = Self { port, source_namespace_id, source_id, source_state, content_digest, byte_length, stability_receipt_ref, guards, deadline, work, object: None };
        let keys = adapter.guards.iter().map(|guard| guard.key().clone()).collect::<Vec<_>>();
        let read = adapter.read(&keys)?;
        adapter.require_source(&read)?;
        Ok(adapter)
    }

    fn charge(&self, bytes: usize) -> Result<(), RetentionControlError> {
        for unit in [Work::Checkpoint, Work::Allocate(bytes), Work::Process(bytes)] {
            (self.work)(unit).map_err(|first| RetentionControlError {
                control: ControlError::BudgetExceeded, first: Some(first),
            })?;
        }
        Ok(())
    }

    fn key(&self, phase: &str, parts: &[&[u8]]) -> Result<ControlKey, RetentionControlError> {
        address(phase, parts, self.work)
    }
    fn scoped_key(&self, phase: &str, revision: Option<c::SourceRevisionId>) -> Result<ControlKey, RetentionControlError> {
        match revision {
            Some(id) => self.key(phase, &[self.source_namespace_id.as_bytes(), self.source_id.as_bytes(), id.as_bytes()]),
            None => self.key(phase, &[self.source_namespace_id.as_bytes(), self.source_id.as_bytes()]),
        }
    }
    fn operation_key(&self, phase: &str, operation: &c::OpaqueId) -> Result<ControlKey, RetentionControlError> {
        self.key(phase, &[operation.as_str().as_bytes()])
    }
    fn read(&self, keys: &[ControlKey]) -> Result<ScopedControlRecordsReadback, RetentionControlError> {
        let work = |unit| (self.work)(unit);
        self.port.read_control_keys_with_original_context(keys, self.deadline, &work).map_err(Into::into)
    }
    fn value<'read>(read: &'read ScopedControlRecordsReadback, key: &ControlKey,
        class: Class) -> Result<Option<&'read ControlValue>, RetentionControlError> {
        if let Some(value) = read.get(key) {
            if value.class() != class { return Err(ControlError::StoreCorrupt.into()); }
            Ok(Some(value))
        } else if read.missing_keys().contains(key) { Ok(None) }
        else { Err(ControlError::OperationConflict.into()) }
    }
    fn require_source(&self, read: &ScopedControlRecordsReadback) -> Result<(), RetentionControlError> {
        let bytes = self.guards.iter().try_fold(0_usize, |sum, guard| sum
            .checked_add(guard.key().as_bytes().len())
            .and_then(|value| value.checked_add(guard.expected().map_or(0, ControlValue::len)))
            .ok_or(ControlError::BudgetExceeded))?;
        (self.work)(Work::Process(bytes)).map_err(|first| RetentionControlError {
            control: ControlError::BudgetExceeded, first: Some(first),
        })?;
        if self.guards.iter().any(|guard| read.get(guard.key()) != guard.expected()) {
            return Err(ControlError::OperationConflict.into());
        }
        Ok(())
    }
    fn scope(&self, extra: &[ControlKey]) -> Result<Vec<ControlKey>, RetentionControlError> {
        self.charge(self.guards.len().checked_add(extra.len()).and_then(|n| n.checked_mul(LIMITS.max_key_bytes))
            .ok_or(ControlError::BudgetExceeded)?)?;
        let mut keys = self.guards.iter().map(|guard| guard.key().clone()).collect::<Vec<_>>();
        keys.extend_from_slice(extra);
        keys.sort(); keys.dedup();
        Ok(keys)
    }
    fn contextual(&self, context: &OperationContext<C>) -> Result<(), RetentionControlError> {
        context.preflight().map_err(|_| ControlError::ReadCancelled)?;
        (self.work)(Work::Checkpoint).map_err(|first| RetentionControlError { control: ControlError::ReadCancelled, first: Some(first) })
    }
    fn encode_value(class: Class, bytes: Vec<u8>) -> Result<ControlValue, RetentionControlError> {
        if bytes.len() > MAX_METADATA { return Err(ControlError::BudgetExceeded.into()); }
        Ok(ControlValue::new(class, bytes, LIMITS)?)
    }
    fn claim_value(&self, operation: &c::OpaqueId) -> Result<ControlValue, RetentionControlError> {
        self.charge(operation.as_str().len().checked_mul(4).and_then(|n| n.checked_add(64))
            .ok_or(ControlError::BudgetExceeded)?)?;
        Self::encode_value(Class::Operation, c::to_canonical_cbor(&V::Text(
            c::CanonicalText::new(operation.as_str()).map_err(|_| ControlError::InvalidValue)?))
            .map_err(|_| ControlError::InvalidValue)?.into_vec())
    }
    fn commit(&mut self, phase: &str, operation: &c::OpaqueId, read: &ScopedControlRecordsReadback,
        writes: Vec<ControlWrite>, mut conditions: Vec<Condition>,
    ) -> Result<(), RetentionControlError> {
        self.charge(8 * LIMITS.max_value_bytes)?;
        conditions.extend_from_slice(&self.guards);
        let material = V::Array(c::BoundedList::new(vec![
            V::Text(c::CanonicalText::new(phase).map_err(|_| ControlError::InvalidValue)?),
            V::Text(c::CanonicalText::new(operation.as_str()).map_err(|_| ControlError::InvalidValue)?),
            V::U64(read.generation()),
        ]).map_err(|_| ControlError::InvalidValue)?);
        let digest = c::blake3_canonical(&c::CanonicalDigestDomain::parse("eliot/cbor/native-retention-control/v1")
            .map_err(|_| ControlError::InvalidValue)?, &material,
            c::DigestInputLimit::new(MAX_METADATA).map_err(|_| ControlError::InvalidValue)?)
            .map_err(|_| ControlError::InvalidValue)?;
        let id = MutationId(*digest.as_bytes());
        let identity = db::control_mutation_identity(id)?;
        let command = ConditionalControlMutation::new(ControlMutation::new(id, digest, read.generation(), writes, Vec::new()), conditions);
        let work = |unit| (self.work)(unit);
        match self.port.transact_records_with_original_context(&command, &identity, self.deadline, &work) {
            Ok(_) => Ok(()),
            Err(first) => {
                // Resolve exactly this command while its original generation,
                // bytes and allowance are still retained. Never replay it.
                match self.port.recover_records_with_original_context(&command, &identity, self.deadline, &work) {
                    Ok(db::CommitRecoveryDecision::Committed(_)) => Ok(()),
                    _ => Err(first.into()),
                }
            }
        }
    }

    /// Joins one historical occurrence to its exact durable object and cause.
    /// This reads metadata only; it grants neither plaintext nor client access.
    pub(super) fn load_committed_object(&self, profile: &SourceRevisionPresealProfile,
        committed: &SourceRevisionMutationReadback, context: &OperationContext<C>,
    ) -> Result<search_revision_store::PersistedSelectedObjectMetadata, RetentionControlError> {
        self.load_object(profile, committed, context, false).map(|(metadata, _, _)| metadata)
    }

    /// Resolve the complete durable join from an exact source reference. This
    /// observes only technical metadata; the serving owner must already hold
    /// the matching membership grant and live source-read checkpoint.
    pub(super) fn load_retained_revision(&self, expected: &c::SourceRevisionRef,
        context: &OperationContext<C>,
    ) -> Result<(search_revision_store::PersistedSelectedObjectMetadata,
        db::source_retention::SealReservationReadback), RetentionControlError> {
        self.load_member_setup_source(expected, context).map(|(metadata, seal, _)| (metadata, seal))
    }

    /// Exact already-verified C07/C28 link plus its complete current C02 guards.
    /// This reads technical metadata only; it never opens a Source locator.
    pub(super) fn load_member_setup_source(&self, expected: &c::SourceRevisionRef,
        context: &OperationContext<C>,
    ) -> Result<(search_revision_store::PersistedSelectedObjectMetadata,
        db::source_retention::SealReservationReadback, ScopedControlRecordsReadback), RetentionControlError> {
        self.contextual(context)?;
        if expected.source_namespace_id != self.source_namespace_id || expected.source_id != self.source_id {
            return Err(ControlError::OperationConflict.into());
        }
        let link = self.scoped_key("object-link", Some(expected.revision_id))?;
        let read = self.read(&self.scope(std::slice::from_ref(&link))?)?;
        self.require_source(&read)?;
        self.charge(4 * MAX_METADATA)?;
        let linked = Self::value(&read, &link, Class::State)?.ok_or(ControlError::StoreUnavailable)?;
        let metadata = search_revision_store::PersistedSelectedObjectMetadata::decode(linked.as_bytes())
            .map_err(|_| ControlError::StoreCorrupt)?;
        if metadata.source().namespace_id != expected.source_namespace_id
            || metadata.source().source_id != expected.source_id
            || metadata.source().revision_id != expected.revision_id
            || metadata.content_digest() != expected.content_digest
            || metadata.content_bytes() != expected.byte_length
        { return Err(ControlError::OperationConflict.into()); }
        let operation = metadata.operation_id();
        let profile = self.load_source_revision_preseal_profile(operation, context)?
            .ok_or(ControlError::StoreCorrupt)?;
        let committed = self.load_source_revision_operation(operation, context)?
            .ok_or(ControlError::StoreCorrupt)?;
        let (verified, seal, guards) = self.load_object(&profile, &committed, context, true)?;
        if verified != metadata { return Err(ControlError::StoreCorrupt.into()); }
        Ok((verified, seal, guards))
    }

    fn load_object(&self, profile: &SourceRevisionPresealProfile,
        committed: &SourceRevisionMutationReadback, context: &OperationContext<C>, require_active: bool,
    ) -> Result<(search_revision_store::PersistedSelectedObjectMetadata,
        db::source_retention::SealReservationReadback, ScopedControlRecordsReadback), RetentionControlError> {
        self.contextual(context)?;
        self.charge(32 * MAX_METADATA)?;
        let object_id = search_revision_store::selected_object_storage_id(committed.revision.object_residency_key_digest)
            .map_err(|_| ControlError::StoreCorrupt)?;
        let revision = self.scoped_key("revision", Some(profile.revision_id))?;
        let link = self.scoped_key("object-link", Some(profile.revision_id))?;
        let preseal = self.operation_key("preseal", &profile.operation_id)?;
        let result = self.operation_key("result", &profile.operation_id)?;
        let (reservation_id, _) = phase_id(&profile.operation_id, "seal", self.work)?;
        let reservation_key = db::source_retention::revision_seal_reservation_key(reservation_id, LIMITS)?;
        let intent_key = db::source_retention::object_publication_intent_key(reservation_id, LIMITS)?;
        let retention_key = search_retention::source_retention::source_retention_record_key(&object_id, &profile.operation_id);
        let retention = ControlKey::new(retention_key.into_bytes(), LIMITS)?;
        let read = self.read(&self.scope(&[revision.clone(), link.clone(), preseal.clone(), result.clone(),
            reservation_key, intent_key, retention.clone()])?)?;
        self.require_source(&read)?;
        let required = |key: &ControlKey, class| Self::value(&read, key, class)?
            .ok_or_else(|| RetentionControlError::from(ControlError::StoreCorrupt));
        if profile.source_namespace_id != self.source_namespace_id || profile.source_id != self.source_id
            || committed.source_namespace_id != self.source_namespace_id || committed.source_id != self.source_id
            || committed.operation_id != profile.operation_id || committed.expected_head != profile.expected_head
            || late_bind_object_residency(profile, committed.revision.object_residency_key_digest)
                .map_err(|_| ControlError::StoreCorrupt)? != committed.revision
            || required(&revision, Class::Revision)?.as_bytes()
                != encode_source_revision_record(&committed.revision).map_err(|_| ControlError::StoreCorrupt)?
            || required(&preseal, Class::Operation)?.as_bytes()
                != encode_source_revision_preseal_profile(profile).map_err(|_| ControlError::StoreCorrupt)?
            || required(&result, Class::Operation)?.as_bytes()
                != encode_source_revision_mutation_readback(committed).map_err(|_| ControlError::StoreCorrupt)?
        { return Err(ControlError::StoreCorrupt.into()); }
        let linked = required(&link, Class::State)?.as_bytes();
        let metadata = search_revision_store::PersistedSelectedObjectMetadata::decode(linked)
            .map_err(|_| ControlError::StoreCorrupt)?;
        let seal = db::source_retention::SealReservationReadback::read(&read, reservation_id, LIMITS)?;
        let intent = db::source_retention::ObjectPublicationIntentReadback::read(&read, reservation_id, LIMITS)?;
        let cause = search_retention::source_retention::decode_source_retention_record(required(&retention, Class::State)?.as_bytes())
            .map_err(|_| ControlError::StoreCorrupt)?;
        if metadata.storage_object_id() != &object_id
            || metadata.object_residency_key_digest() != committed.revision.object_residency_key_digest
            || metadata.source().namespace_id != self.source_namespace_id || metadata.source().source_id != self.source_id
            || metadata.source().revision_id != profile.revision_id || metadata.source().sequence.get() != profile.occurrence_sequence
            || metadata.operation_id() != &profile.operation_id || metadata.content_digest() != profile.content_digest
            || metadata.content_bytes() != profile.byte_length
            || search_revision_store::residency_canonical_bytes(metadata.domain_bindings())
                .map_err(|_| ControlError::StoreCorrupt)?.as_slice() != profile.residency_closure.as_slice()
            || !intent.binds(&seal) || intent.object_metadata_canonical() != linked
            || seal.operation_id() != &profile.operation_id
            || seal.preseal_occurrence_canonical() != required(&preseal, Class::Operation)?.as_bytes()
            || seal.residency_canonical() != profile.residency_closure.as_slice()
            || seal.content_digest() != profile.content_digest || seal.plaintext_length() != profile.byte_length
            || cause.object_id != object_id || cause.residency_digest != committed.revision.object_residency_key_digest
            || cause.namespace_id != self.source_namespace_id || cause.source_id != self.source_id || cause.source_revision_id != profile.revision_id
            || cause.operation.operation_id != profile.operation_id || cause.operation.request_digest != committed.request_digest
            || !matches!(cause.cause, search_retention::source_retention::SourceRetentionCause::UntilExplicitRelease)
            || (require_active && cause.state != search_retention::source_retention::SourceRetentionState::Active)
        { return Err(ControlError::StoreCorrupt.into()); }
        Ok((metadata, seal, read))
    }

    /// Called only after the Native C07 publication/decrypt/source checks.
    /// The journal additionally verifies the original reservation and late intent.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn commit_verified_object(&mut self, profile: &SourceRevisionPresealProfile,
        object_id: c::OpaqueId, physical: c::ObjectResidencyKeyDigest, metadata: &[u8],
        reservation: MutationId, context: &OperationContext<C>,
    ) -> Result<SourceRevisionMutationReadback, RetentionControlError> {
        self.contextual(context)?;
        self.charge(16 * MAX_METADATA)?;
        if metadata.is_empty() || metadata.len() > MAX_METADATA
            || c::parse_canonical_cbor(metadata).is_err()
        { return Err(ControlError::InvalidValue.into()); }
        let decoded = search_revision_store::PersistedSelectedObjectMetadata::decode(metadata)
            .map_err(|_| ControlError::InvalidValue)?;
        if decoded.storage_object_id() != &object_id || decoded.object_residency_key_digest() != physical
            || decoded.source().namespace_id != profile.source_namespace_id
            || decoded.source().source_id != profile.source_id || decoded.source().revision_id != profile.revision_id
            || decoded.source().sequence.get() != profile.occurrence_sequence
            || decoded.operation_id() != &profile.operation_id || decoded.content_digest() != profile.content_digest
            || decoded.content_bytes() != profile.byte_length
            || search_revision_store::residency_canonical_bytes(decoded.domain_bindings())
                .map_err(|_| ControlError::InvalidValue)?.as_slice() != profile.residency_closure.as_slice()
        { return Err(ControlError::OperationConflict.into()); }
        let revision = late_bind_object_residency(profile, physical).map_err(|_| ControlError::InvalidValue)?;
        let request = SourceRevisionRegistrationRequest::new(profile.operation_id.clone(),
            profile.source_namespace_id, profile.source_id, profile.expected_head,
            profile.content_digest, profile.byte_length, profile.observed_at.clone(),
            profile.acquisition_kind, profile.stability_receipt_ref.clone(), physical);
        let mutation = SourceRevisionMutation { operation_id: profile.operation_id.clone(),
            request_digest: request.request_digest(), source_namespace_id: profile.source_namespace_id,
            source_id: profile.source_id, expected_head: profile.expected_head, revision };
        self.object = Some(FinalObject { id: object_id, physical, metadata: metadata.to_vec(), reservation });
        let identity = MutationIdentity::new(profile.operation_id.clone(), IdempotencyClass::RetrySameIdentity);
        let outcome = self.compare_and_append_source_revision(&mutation, context, &identity);
        self.object = None;
        let observed = self.load_source_revision_operation(&profile.operation_id, context);
        match observed {
            Ok(Some(readback)) if readback.operation_id == mutation.operation_id
                && readback.request_digest == mutation.request_digest && readback.revision == mutation.revision
                && readback.source_namespace_id == self.source_namespace_id && readback.source_id == self.source_id
                && readback.expected_head == mutation.expected_head => Ok(readback),
            _ => match outcome { Err(first) => Err(first), Ok(()) => Err(ControlError::CommitOutcomeUnknown.into()) },
        }
    }

    fn commit_final(&mut self, mutation: &SourceRevisionMutation) -> Result<(), RetentionControlError> {
        self.charge(32 * MAX_METADATA)?;
        let object = self.object.as_ref().ok_or(ControlError::OperationConflict)?;
        if mutation.revision.source_id != self.source_id
            || mutation.revision.object_residency_key_digest != object.physical
        { return Err(ControlError::OperationConflict.into()); }
        let preseal = self.operation_key("preseal", &mutation.operation_id)?;
        let claim = self.scoped_key("occurrence-reservation", Some(mutation.revision.revision_id))?;
        let revision = self.scoped_key("revision", Some(mutation.revision.revision_id))?;
        let result = self.operation_key("result", &mutation.operation_id)?;
        let head = self.scoped_key("head", None)?;
        let link = self.scoped_key("object-link", Some(mutation.revision.revision_id))?;
        let retention_key = search_retention::source_retention::source_retention_record_key(&object.id, &mutation.operation_id);
        let retention = ControlKey::new(retention_key.into_bytes(), LIMITS)?;
        let publication_keys = db::source_retention::object_publication_intent_keys(object.reservation, LIMITS)?;
        let mut extra = vec![preseal.clone(), claim.clone(), revision.clone(), result.clone(), head.clone(), link.clone(), retention.clone()];
        extra.extend_from_slice(&publication_keys);
        let read = self.read(&self.scope(&extra)?)?;
        self.require_source(&read)?;
        if Self::value(&read, &result, Class::Operation)?.is_some()
            || Self::value(&read, &revision, Class::Revision)?.is_some()
            || Self::value(&read, &link, Class::State)?.is_some()
            || Self::value(&read, &retention, Class::State)?.is_some()
        { return Err(ControlError::OperationConflict.into()); }
        let preseal_value = Self::value(&read, &preseal, Class::Operation)?.ok_or(ControlError::OperationConflict)?;
        let profile = decode_source_revision_preseal_profile(preseal_value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?;
        if profile.operation_id != mutation.operation_id || profile.source_namespace_id != self.source_namespace_id
            || profile.source_id != self.source_id || profile.expected_head != mutation.expected_head
            || profile.content_digest != self.content_digest || profile.byte_length != self.byte_length
            || profile.stability_receipt_ref != self.stability_receipt_ref
            || late_bind_object_residency(&profile, object.physical).map_err(|_| ControlError::StoreCorrupt)? != mutation.revision
            || Self::value(&read, &claim, Class::Operation)? != Some(&self.claim_value(&mutation.operation_id)?)
        { return Err(ControlError::OperationConflict.into()); }
        let reservation = db::source_retention::SealReservationReadback::read(&read, object.reservation, LIMITS)?;
        let intent = db::source_retention::ObjectPublicationIntentReadback::read(&read, object.reservation, LIMITS)?;
        if !intent.binds(&reservation) || intent.operation_id() != &mutation.operation_id
            || reservation.preseal_occurrence_canonical() != preseal_value.as_bytes()
            || reservation.residency_canonical() != profile.residency_closure.as_slice()
            || reservation.content_digest() != profile.content_digest || reservation.plaintext_length() != profile.byte_length
            || intent.object_metadata_canonical() != object.metadata
            || Self::value(&read, &publication_keys[2], Class::State)?.map(ControlValue::as_bytes)
                != Some(reservation.selected_current_canonical())
        { return Err(ControlError::OperationConflict.into()); }
        let expected_head = Self::value(&read, &head, Class::Revision)?;
        let observed = expected_head.map(|value| decode_source_revision_record(value.as_bytes())
            .map_err(|_| ControlError::StoreCorrupt)).transpose()?;
        if observed.as_ref().map(|value| value.revision_id) != mutation.expected_head
            || observed.as_ref().map_or(Some(1), |value| value.occurrence_sequence.checked_add(1))
                != Some(mutation.revision.occurrence_sequence)
        { return Err(ControlError::OperationConflict.into()); }
        let request = SourceRevisionRegistrationRequest::new(mutation.operation_id.clone(), self.source_namespace_id,
            self.source_id, mutation.expected_head, profile.content_digest, profile.byte_length,
            profile.observed_at.clone(), profile.acquisition_kind, profile.stability_receipt_ref, object.physical);
        if request.request_digest() != mutation.request_digest { return Err(ControlError::OperationConflict.into()); }
        let receipt = c::ReceiptRef::new(format!("native-retained-v1:{}:{}:{}",
            super::super::super::codec::hex(self.source_namespace_id.as_bytes()),
            super::super::super::codec::hex(self.source_id.as_bytes()),
            super::super::super::codec::hex(mutation.revision.revision_id.as_bytes())))
            .map_err(|_| ControlError::InvalidValue)?;
        let readback = SourceRevisionMutationReadback { operation_id: mutation.operation_id.clone(),
            request_digest: mutation.request_digest, source_namespace_id: self.source_namespace_id, source_id: self.source_id,
            expected_head: mutation.expected_head, revision: mutation.revision.clone(), receipt };
        let cause = search_retention::source_retention::SourceRetentionRecord {
            object_id: object.id.clone(), residency_digest: object.physical, namespace_id: self.source_namespace_id,
            source_id: self.source_id, source_revision_id: mutation.revision.revision_id,
            cause: search_retention::source_retention::SourceRetentionCause::UntilExplicitRelease,
            operation: search_retention::RetentionOperation { operation_id: mutation.operation_id.clone(),
                request_digest: mutation.request_digest },
            state: search_retention::source_retention::SourceRetentionState::Active,
        };
        let revision_value = Self::encode_value(Class::Revision,
            encode_source_revision_record(&mutation.revision).map_err(|_| ControlError::InvalidValue)?)?;
        let result_value = Self::encode_value(Class::Operation,
            encode_source_revision_mutation_readback(&readback).map_err(|_| ControlError::InvalidValue)?)?;
        let cause_value = Self::encode_value(Class::State,
            search_retention::source_retention::encode_source_retention_record(&cause).map_err(|_| ControlError::InvalidValue)?)?;
        let link_value = Self::encode_value(Class::State, object.metadata.clone())?;
        let mut conditions = vec![Condition::absent(revision.clone()), Condition::absent(result.clone()),
            Condition::absent(link.clone()), Condition::absent(retention.clone()),
            Condition::exact(preseal, preseal_value.clone()), Condition::exact(claim, self.claim_value(&mutation.operation_id)?)];
        conditions.push(expected_head.map_or_else(|| Condition::absent(head.clone()), |value| Condition::exact(head.clone(), value.clone())));
        for index in [2, 3, 4] {
            conditions.push(Condition::exact(publication_keys[index].clone(),
                read.get(&publication_keys[index]).ok_or(ControlError::OperationConflict)?.clone()));
        }
        let writes = vec![ControlWrite { key: revision, value: revision_value.clone() },
            ControlWrite { key: head, value: revision_value }, ControlWrite { key: result, value: result_value },
            ControlWrite { key: link, value: link_value }, ControlWrite { key: retention, value: cause_value }];
        self.commit("complete", &mutation.operation_id, &read, writes, conditions)
    }
}

impl<C: CancellationProbe> SourceRevisionPreparePort for NativeRetentionControl<'_, '_, C> {
    fn load_source_revision_preseal_profile(&self, operation: &c::OpaqueId, context: &OperationContext<C>)
        -> Result<Option<SourceRevisionPresealProfile>, Self::Error> {
        self.contextual(context)?;
        let key = self.operation_key("preseal", operation)?;
        let read = self.read(std::slice::from_ref(&key))?;
        self.charge(4 * MAX_METADATA)?;
        Self::value(&read, &key, Class::Operation)?.map(|value| {
            let profile = decode_source_revision_preseal_profile(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?;
            if &profile.operation_id != operation || profile.source_namespace_id != self.source_namespace_id
                || profile.source_id != self.source_id { return Err(ControlError::OperationConflict.into()); }
            Ok(profile)
        }).transpose()
    }
    fn retain_source_revision_preseal_profile(&mut self, profile: &SourceRevisionPresealProfile,
        context: &OperationContext<C>) -> Result<(), Self::Error> {
        self.contextual(context)?;
        if profile.source_namespace_id != self.source_namespace_id || profile.source_id != self.source_id
            || self.source_state != SourceRevisionSourceState::Active
            || profile.content_digest != self.content_digest || profile.byte_length != self.byte_length
            || profile.stability_receipt_ref != self.stability_receipt_ref {
            return Err(ControlError::OperationConflict.into());
        }
        self.charge(4 * MAX_METADATA)?;
        let encoded = encode_source_revision_preseal_profile(profile).map_err(|_| ControlError::InvalidValue)?;
        let preseal = self.operation_key("preseal", &profile.operation_id)?;
        let claim = self.scoped_key("occurrence-reservation", Some(profile.revision_id))?;
        let head = self.scoped_key("head", None)?;
        let revision = self.scoped_key("revision", Some(profile.revision_id))?;
        let read = self.read(&self.scope(&[preseal.clone(), claim.clone(), head.clone(), revision.clone()])?)?;
        self.require_source(&read)?;
        let value = Self::encode_value(Class::Operation, encoded)?;
        let claim_value = self.claim_value(&profile.operation_id)?;
        if let Some(existing) = Self::value(&read, &preseal, Class::Operation)? {
            if existing == &value && Self::value(&read, &claim, Class::Operation)? == Some(&claim_value) { return Ok(()); }
            return Err(ControlError::OperationConflict.into());
        }
        if Self::value(&read, &claim, Class::Operation)?.is_some() || Self::value(&read, &revision, Class::Revision)?.is_some() {
            return Err(ControlError::OperationConflict.into());
        }
        let observed_head = Self::value(&read, &head, Class::Revision)?;
        let current = observed_head.map(|value| decode_source_revision_record(value.as_bytes())
            .map_err(|_| ControlError::StoreCorrupt)).transpose()?;
        if current.as_ref().map(|value| value.revision_id) != profile.expected_head {
            return Err(ControlError::OperationConflict.into());
        }
        let expected = observed_head.map_or_else(|| Condition::absent(head.clone()), |value| Condition::exact(head.clone(), value.clone()));
        self.commit("preseal", &profile.operation_id, &read,
            vec![ControlWrite { key: preseal.clone(), value }, ControlWrite { key: claim.clone(), value: claim_value }],
            vec![Condition::absent(preseal), Condition::absent(claim), Condition::absent(revision), expected])
    }
}

impl<C: CancellationProbe> SourceRevisionControlPort for NativeRetentionControl<'_, '_, C> {
    fn load_source_revision_operation(&self, operation: &c::OpaqueId, context: &OperationContext<C>)
        -> Result<Option<SourceRevisionMutationReadback>, Self::Error> {
        self.contextual(context)?;
        let key = self.operation_key("result", operation)?;
        let read = self.read(std::slice::from_ref(&key))?;
        self.charge(4 * MAX_METADATA)?;
        Self::value(&read, &key, Class::Operation)?.map(|value| {
            let result = decode_source_revision_mutation_readback(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?;
            if &result.operation_id != operation || result.source_namespace_id != self.source_namespace_id
                || result.source_id != self.source_id { return Err(ControlError::OperationConflict.into()); }
            Ok(result)
        }).transpose()
    }
    fn load_source_revision_head(&self, namespace: &c::SourceNamespaceId, source: &c::SourceId,
        context: &OperationContext<C>) -> Result<Option<SourceRevisionHead>, Self::Error> {
        self.contextual(context)?;
        if *namespace != self.source_namespace_id || *source != self.source_id { return Err(ControlError::OperationConflict.into()); }
        let head = self.scoped_key("head", None)?;
        let read = self.read(&self.scope(std::slice::from_ref(&head))?)?;
        self.require_source(&read)?;
        self.charge(4 * MAX_METADATA)?;
        let revision = Self::value(&read, &head, Class::Revision)?.map(|value|
            decode_source_revision_record(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)).transpose()?;
        if revision.as_ref().is_some_and(|revision| revision.source_id != *source) { return Err(ControlError::StoreCorrupt.into()); }
        Ok(Some(SourceRevisionHead { source_namespace_id: *namespace, source_id: *source,
            state: self.source_state, revision }))
    }
    fn load_source_revision(&self, namespace: &c::SourceNamespaceId, source: &c::SourceId,
        revision: &c::SourceRevisionId, context: &OperationContext<C>) -> Result<Option<c::SourceRevision>, Self::Error> {
        self.contextual(context)?;
        if *namespace != self.source_namespace_id || *source != self.source_id { return Err(ControlError::OperationConflict.into()); }
        let key = self.scoped_key("revision", Some(*revision))?;
        let read = self.read(std::slice::from_ref(&key))?;
        self.charge(4 * MAX_METADATA)?;
        Self::value(&read, &key, Class::Revision)?.map(|value| {
            let record = decode_source_revision_record(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?;
            if record.revision_id != *revision || record.source_id != *source { return Err(ControlError::StoreCorrupt.into()); }
            Ok(record)
        }).transpose()
    }
    fn compare_and_append_source_revision(&mut self, mutation: &SourceRevisionMutation,
        context: &OperationContext<C>, identity: &MutationIdentity) -> Result<(), Self::Error> {
        self.contextual(context)?;
        if identity.operation_id != mutation.operation_id || mutation.source_namespace_id != self.source_namespace_id
            || mutation.source_id != self.source_id || self.source_state != SourceRevisionSourceState::Active
        { return Err(ControlError::OperationConflict.into()); }
        self.commit_final(mutation)
    }
}

#[cfg(feature = "wave3-index")]
fn verify_derived_artifact_readback(
    reference: &DerivedArtifactObjectRefV1,
    reservation: &DerivedArtifactReservationReadback,
    intent: &DerivedArtifactObjectIntentReadback,
    guards: &DerivedArtifactScopeGuardKeysV1,
    expected_residency: &[u8],
    expected_encryption: &EncryptionBinding,
    selected: &super::retention::Selected,
) -> Result<(), RetentionControlError> {
    let descriptor = reference.descriptor();
    let descriptor_canonical = descriptor.encode().map_err(|_| ControlError::StoreCorrupt)?;
    let reference_canonical = reference.encode().map_err(|_| ControlError::StoreCorrupt)?;
    let address = reference.address().map_err(|_| ControlError::StoreCorrupt)?.relative_path();
    let expected_guards = [
        guards.root_owner(),
        guards.source_revision(),
        guards.namespace_owner(),
        guards.residency_policy(),
        guards.purge_fence(),
    ];
    if reference.storage() != DerivedArtifactStorageV1::Aes256GcmEnvelope
        || reservation.source_revision_ref() != descriptor.source()
        || reservation.operation_id() != descriptor.operation_id()
        || reservation.materialization_id() != descriptor.materialization_id()
        || reservation.representation_id() != descriptor.representation_id()
        || reservation.descriptor_canonical() != descriptor_canonical
        || reservation.residency_canonical() != expected_residency
        || reservation.selected_current_canonical() != selected.current
        || &selected.encryption != expected_encryption
        || selected.claim.as_deref().is_some_and(|claim| claim != expected_residency)
        || reservation.plaintext_length() != descriptor.encoded_body_bytes()
        || reservation.scope_guard_keys() != expected_guards
        || !intent.binds(reservation)
        || intent.operation_id() != descriptor.operation_id()
        || intent.object_reference_canonical() != reference_canonical
        || intent.address_bytes() != address.as_bytes()
        || intent.stored_object_digest() != reference.stored_object_digest()
        || intent.stored_object_length() != reference.stored_bytes()
        || intent.counter() != reservation.counter()
        || intent.authenticated_blocks() != reservation.authenticated_blocks()
        || intent.plaintext_length() != descriptor.encoded_body_bytes()
    {
        return Err(ControlError::OperationConflict.into());
    }
    Ok(())
}
