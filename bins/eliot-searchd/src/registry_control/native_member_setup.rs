//! Root-selected raw-code membership metadata. This composes existing C03,
//! preparation and policy owners; registration here grants no query permission.

use super::{RegistryJournal, SourceIdentityIdsV2, owner_failure, storage_failure};
use super::{source_ids::map_source_identity_error, source_scope::RootSourceScope};
use search_contracts::{self as c, CanonicalValue as V};
use search_control_redb::{
    self as db, ControlRecordClass as Class, ControlRecordCondition as Condition,
    MigrationBootstrapWork as Work,
};
use search_lexical::AcceptedCodeIdentifiersProfile;
use search_materializer::api::{SourceEncoding, SourceKind, ValidatedMaterializerProfile};
use search_ports::{CancellationProbe, OperationContext};
use search_revision_store::{DomainBinding, residency_canonical_bytes};
use search_runtime_owner::OwnerError;
use search_source_admission::SourceClass;
use search_source_registry::{
    api::{
        NamespaceCutoverState, SourceMembershipReceiptV2, SourceNamespaceSelectionV2,
        SourceOwnerScopeV2, SourceRegistry, active_source_membership_v2, bind_source_membership_v2,
        prepare_source_membership_v2,
    },
    error::{RegistryControlPort, RegistryError, RegistryPortError},
    membership::{
        MembershipKey, MembershipLifecycle, MembershipRecord, PermanentMembershipProvenanceV2,
    },
    recovery::{RegistryBatchLookup, RegistryStateRecord, decode_registry_state},
    root::RootRecord,
    source::RegisteredSource,
};
use search_unitizer::{V3RepresentationKind, ValidatedV3UnitizerProfile};

const PROFILE: &str = "eliot.daemon.native-member-setup.v1";
const PREFIX: &[u8] = b"eliot.daemon.native-member-setup.v1/";
const MAX_BYTES: usize = 60 * 1024;
const MAX_GUARDS: usize = 24;

/// Actual selected owner objects, independent of membership/representation IDs.
pub struct NativeMemberProfiles<'a> {
    pub materializer: &'a ValidatedMaterializerProfile,
    pub unitizer: &'a ValidatedV3UnitizerProfile,
    pub lexical: &'a AcceptedCodeIdentifiersProfile,
}

/// Complete bounded selected meanings; no constructor accepts caller `ProfileId`s.
pub struct SelectedNativeMember {
    canonical: Vec<u8>,
    preparation: c::ProfileId,
    retention: c::ProfileId,
    role: c::MembershipRole,
    domain_ids: [[u8; 16]; 6],
}

impl SelectedNativeMember {
    pub(crate) const fn preparation_profile_id(&self) -> &c::ProfileId {
        &self.preparation
    }
    pub(crate) const fn retention_policy_id(&self) -> &c::ProfileId {
        &self.retention
    }
    pub(crate) fn from_owners(
        profiles: &NativeMemberProfiles<'_>,
        source: &RegisteredSource,
        namespace: &NamespaceCutoverState,
        policy: &[u8],
        domains: &[DomainBinding; 6],
    ) -> Result<Self, RegistryPortError> {
        if !profiles
            .materializer
            .source_kinds()
            .contains(&SourceKind::Code)
            || !profiles
                .materializer
                .encodings()
                .contains(&SourceEncoding::Utf8)
            || profiles.unitizer.descriptor().representation_kind != V3RepresentationKind::Code
        {
            return Err(invalid());
        }
        let original = source.permanent_admission().ok_or_else(invalid)?.original();
        let role = match original.observation().metadata().source_class() {
            SourceClass::Regular => c::MembershipRole::Source,
            SourceClass::Generated => c::MembershipRole::Generated,
            SourceClass::Vendor => c::MembershipRole::Vendor,
            _ => return Err(invalid()),
        };
        let lexical = profiles.lexical.profile();
        let qualification = profiles.lexical.qualification();
        let preparation = array(vec![
            text("native-raw-code-preparation/v1")?,
            bytes(profiles.materializer.id().as_bytes())?,
            V::U64(profiles.materializer.revision()),
            bytes(profiles.materializer.golden_fixture_digest().as_bytes())?,
            text(profiles.unitizer.id().as_str())?,
            V::U64(profiles.unitizer.revision()),
            bytes(profiles.unitizer.descriptor().fixture_digest.as_bytes())?,
            text(lexical.id().as_str())?,
            V::U64(lexical.revision().get()),
            bytes(lexical.fingerprint().as_bytes())?,
            bytes(lexical.artifact_set_identity().as_bytes())?,
            bytes(lexical.compatibility_fixture_identity().as_bytes())?,
            bytes(lexical.collision_fixture_identity().as_bytes())?,
            text(qualification.qualification_receipt.as_str())?,
        ])?;
        let metadata = namespace.permanent_metadata().ok_or_else(invalid)?;
        let access = array(vec![
            text("native-member-access-binding/v1")?,
            bytes(policy)?,
            bytes(metadata.policy().encoded_inputs())?,
            bytes(domains[1].canonical_body())?,
            bytes(domains[2].canonical_body())?,
        ])?;
        let retention = array(vec![
            text("native-member-retention-profile/v1")?,
            bytes(domains[4].canonical_body())?,
            bytes(domains[5].canonical_body())?,
        ])?;
        let residency = residency_canonical_bytes(domains).map_err(|_| invalid())?;
        let preparation_id = profile_id("native-raw-code-v1", &preparation)?;
        let retention_id = profile_id("native-retention-v1", &retention)?;
        let role_value = match role {
            c::MembershipRole::Source => 0,
            c::MembershipRole::Generated => 1,
            c::MembershipRole::Vendor => 2,
            _ => return Err(invalid()),
        };
        let canonical = encode(&array(vec![
            text(PROFILE)?,
            preparation,
            access,
            retention,
            bytes(&residency)?,
            V::U64(role_value),
        ])?)?;
        Ok(Self {
            canonical,
            preparation: preparation_id,
            retention: retention_id,
            role,
            domain_ids: std::array::from_fn(|index| domains[index].uuid()),
        })
    }
}

/// All observations belong to one original Native retention invocation.
pub struct NativeMemberSetupInput<'a> {
    pub root: &'a RootRecord,
    pub namespace: &'a NamespaceCutoverState,
    pub source: &'a RegisteredSource,
    pub scope: RootSourceScope,
    pub selected: &'a SelectedNativeMember,
    pub operation: &'a c::OpaqueId,
    pub source_read: &'a db::ScopedControlRecordsReadback,
    pub selected_current: Condition,
}

struct Setup {
    operation: c::OpaqueId,
    selected: Vec<u8>,
    // Prospective C03 request data encoded by C03's own v5 codec. Only the
    // separate real C03 batch can install this as authoritative active state.
    record: MembershipRecord,
    generation: u64,
    conditions: Vec<Condition>,
}

impl<C: CancellationProbe> RegistryJournal<'_, C> {
    #[cfg(test)]
    pub(crate) fn native_member_test_assignment(
        &mut self,
        registry: &mut SourceRegistry,
        input: &NativeMemberSetupInput<'_>,
        context: &OperationContext<C>,
        checkpoint: &mut impl FnMut() -> Result<(), OwnerError>,
        mint: impl FnOnce() -> (
            c::SourceMembershipId,
            c::AccessPolicyBindingId,
            c::ResidencyPolicyBindingId,
        ),
    ) -> Result<SourceMembershipReceiptV2, RegistryPortError> {
        self.native_member_with(registry, input, context, checkpoint, |_, _| Ok(mint()))
    }
    pub(crate) fn ensure_native_member(
        &mut self,
        registry: &mut SourceRegistry,
        input: &NativeMemberSetupInput<'_>,
        context: &OperationContext<C>,
        checkpoint: &mut impl FnMut() -> Result<(), OwnerError>,
    ) -> Result<SourceMembershipReceiptV2, RegistryPortError> {
        self.native_member_with(registry, input, context, checkpoint, |ids, context| {
            Ok((
                ids.fresh_source_membership_id(context)
                    .map_err(map_source_identity_error)?,
                ids.fresh_access_policy_binding_id(context)
                    .map_err(map_source_identity_error)?,
                ids.fresh_residency_policy_binding_id(context)
                    .map_err(map_source_identity_error)?,
            ))
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Original assignment, exact C02 recovery and C03 bind remain one ordered setup"
    )]
    fn native_member_with(
        &mut self,
        registry: &mut SourceRegistry,
        input: &NativeMemberSetupInput<'_>,
        context: &OperationContext<C>,
        checkpoint: &mut impl FnMut() -> Result<(), OwnerError>,
        mint: impl FnOnce(
            &SourceIdentityIdsV2<'_, C>,
            &OperationContext<C>,
        ) -> Result<
            (
                c::SourceMembershipId,
                c::AccessPolicyBindingId,
                c::ResidencyPolicyBindingId,
            ),
            RegistryPortError,
        >,
    ) -> Result<SourceMembershipReceiptV2, RegistryPortError> {
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
        if self.requires_recovery() {
            return Err(owner_failure(RegistryError::MutationOutcomeUnknown));
        }
        let original = self.original_v4.as_ref().ok_or_else(invalid)?;
        let (deadline, work) = (original.deadline, original.work);
        for unit in [
            Work::Checkpoint,
            Work::Allocate(16 * MAX_BYTES),
            Work::Process(16 * MAX_BYTES),
        ] {
            work(unit).map_err(|error| {
                map_source_identity_error(super::SourceIdentityIdError::Owner(error))
            })?;
        }
        checkpoint().map_err(|error| {
            map_source_identity_error(super::SourceIdentityIdError::Owner(error))
        })?;
        if registry.roots().get(&input.root.root_binding_id) != Some(input.root)
            || registry.sources().get(input.source.identity()) != Some(input.source)
            || registry
                .namespaces()
                .get(&input.namespace.ownership.source_namespace_id)
                != Some(input.namespace)
            || input.source_read.identity() != self.identity
            || input.source_read.records().is_empty()
            || input.source_read.records().len() >= MAX_GUARDS
        {
            return Err(invalid());
        }
        let limits = self.journal.limits();
        let key = member_key(input, limits)?;
        let requested_op_key = operation_key(input.operation, limits)?;
        // The C03 original operation is inspected before any replacement draw.
        let requested_bind = bind_operation(input.operation)?;
        let prior_bind = self.load_batch(&requested_bind, context)?;
        let read = self
            .journal
            .read_control_keys_with_original_context(
                &[key.clone(), requested_op_key.clone()],
                deadline,
                &work,
            )
            .map_err(|error| storage_failure(error.control, Some(input.operation)))?;
        if read.identity() != self.identity || read.generation() != input.source_read.generation() {
            return Err(owner_failure(RegistryError::OperationConflict));
        }
        let (setup, encoded) = if let Some(value) = read.get(&key) {
            if value.class() != Class::State {
                return Err(invalid());
            }
            let setup = decode_setup(value.as_bytes(), limits)?;
            if setup.generation >= read.generation()
                || (setup.operation != *input.operation
                    && !matches!(&prior_bind, RegistryBatchLookup::Absent { .. }))
            {
                return Err(invalid());
            }
            let original_key = operation_key(&setup.operation, limits)?;
            let claim = self
                .journal
                .read_control_keys_with_original_context(&[original_key], deadline, &work)
                .map_err(|error| storage_failure(error.control, Some(&setup.operation)))?;
            if claim.identity() != self.identity
                || claim.generation() != read.generation()
                || claim.records().len() != 1
                || claim.records()[0].1.class() != Class::Operation
                || claim.records()[0].1.as_bytes() != value.as_bytes()
                || read.get(&requested_op_key).is_some_and(|entry| {
                    entry.class() != Class::Operation || entry.as_bytes() != value.as_bytes()
                })
            {
                return Err(invalid());
            }
            (setup, value.as_bytes().to_vec())
        } else {
            if !read.missing_keys().contains(&key)
                || !read.missing_keys().contains(&requested_op_key)
                || !matches!(prior_bind, RegistryBatchLookup::Absent { revision } if revision == registry.revision())
            {
                return Err(owner_failure(RegistryError::OperationConflict));
            }
            let mut conditions = input
                .source_read
                .records()
                .iter()
                .map(|(key, value)| Condition::exact(key.clone(), value.clone()))
                .collect::<Vec<_>>();
            conditions.push(input.selected_current.clone());
            verify_conditions(self, &conditions, read.generation(), deadline, work)?;
            checkpoint().map_err(|error| {
                map_source_identity_error(super::SourceIdentityIdError::Owner(error))
            })?;
            let (id, access, residency) =
                mint(&SourceIdentityIdsV2::<C>::bind(deadline, work), context)?;
            validate_new_ids(input, id, access, residency)?;
            let member = c::SourceMembership {
                source_membership_id: id,
                corpus_id: input.scope.corpus_id,
                source_id: input.source.identity().source_id,
                workspace_id: input.scope.workspace_id,
                role: input.selected.role,
                preparation_profile_id: input.selected.preparation_profile_id().clone(),
                access_policy_binding_id: access,
                retention_policy_id: input.selected.retention_policy_id().clone(),
                residency_policy_binding_id: residency,
                membership_revision: c::MembershipRevision::new(1),
            };
            let admission = input.source.permanent_admission().ok_or_else(invalid)?;
            let record = MembershipRecord::new_permanent_v2(
                MembershipKey {
                    corpus_id: c::OpaqueId::new(member.corpus_id.to_string())
                        .map_err(|_| invalid())?,
                    source_identity: input.source.identity().clone(),
                },
                c::NonZeroRevision::new(1).map_err(|_| invalid())?,
                member,
                PermanentMembershipProvenanceV2::capture(
                    input.source.identity().clone(),
                    input.source.source_revision(),
                    input.root.root_binding_id,
                    admission,
                ),
                MembershipLifecycle::Active,
                registry.revision().checked_add(1).ok_or_else(invalid)?,
                member_receipt(input.operation)?,
            )
            .map_err(owner_failure)?;
            let setup = Setup {
                operation: input.operation.clone(),
                selected: input.selected.canonical.clone(),
                record,
                generation: read.generation(),
                conditions,
            };
            let encoded = encode_setup(&setup)?;
            let (namespace, owner) = owner_selection(input, &setup)?;
            let member = setup.record.permanent_v2().ok_or_else(invalid)?.member();
            prepare_source_membership_v2(
                registry,
                member,
                input.root.root_binding_id,
                admission.root_record_revision(),
                namespace,
                &owner,
                input.source.source_revision(),
                &requested_bind,
                digest(&encoded),
                &member_receipt(input.operation)?,
                context,
            )
            .map_err(owner_failure)?;
            (setup, encoded)
        };
        validate_setup(input, &setup)?;
        let command = setup_command(input, &setup, &encoded, limits)?;
        let identity = db::control_mutation_identity(command.mutation().id())
            .map_err(|error| storage_failure(error, Some(&setup.operation)))?;
        checkpoint().map_err(|error| {
            map_source_identity_error(super::SourceIdentityIdError::Owner(error))
        })?;
        self.original_uncertain = true;
        if setup.generation == read.generation() {
            if let Err(first) = self
                .journal
                .transact_records_with_original_context(&command, &identity, deadline, &work)
            {
                match self
                    .journal
                    .recover_records_with_original_context(&command, &identity, deadline, &work)
                {
                    Ok(db::CommitRecoveryDecision::Committed(_)) => {}
                    Ok(db::CommitRecoveryDecision::NotCommittedRetrySameOperation) => {
                        self.original_uncertain = false;
                        return Err(storage_failure(first.control, Some(&setup.operation)));
                    }
                    _ => return Err(storage_failure(first.control, Some(&setup.operation))),
                }
            }
        } else {
            match self
                .journal
                .recover_records_with_original_context(&command, &identity, deadline, &work)
            {
                Ok(db::CommitRecoveryDecision::Committed(_)) => {}
                _ => return Err(owner_failure(RegistryError::MutationOutcomeUnknown)),
            }
        }
        // C02 verifies the exact original command fingerprint on both paths.
        let keys = [key, operation_key(&setup.operation, limits)?];
        let committed = self
            .journal
            .read_control_keys_with_original_context(&keys, deadline, &work)
            .map_err(|error| storage_failure(error.control, Some(&setup.operation)))?;
        if committed.identity() != self.identity
            || committed.records().len() != 2
            || committed
                .records()
                .iter()
                .any(|(_, value)| value.as_bytes() != encoded)
        {
            return Err(owner_failure(RegistryError::MutationOutcomeUnknown));
        }
        self.original_uncertain = false;
        let mut current = input
            .source_read
            .records()
            .iter()
            .map(|(key, value)| Condition::exact(key.clone(), value.clone()))
            .collect::<Vec<_>>();
        current.push(input.selected_current.clone());
        verify_conditions(self, &current, committed.generation(), deadline, work)?;
        checkpoint().map_err(|error| {
            map_source_identity_error(super::SourceIdentityIdError::Owner(error))
        })?;
        let (namespace, owner) = owner_selection(input, &setup)?;
        let permanent = setup.record.permanent_v2().ok_or_else(invalid)?;
        let result = bind_source_membership_v2(
            registry,
            permanent.member(),
            input.root.root_binding_id,
            permanent.provenance().root_record_revision(),
            namespace,
            &owner,
            permanent.provenance().source_revision(),
            &bind_operation(&setup.operation)?,
            digest(&encoded),
            &member_receipt(&setup.operation)?,
            self,
            context,
        )
        .map_err(owner_failure)?;
        let (active, identity) =
            active_source_membership_v2(registry, result.member.source_membership_id)
                .map_err(owner_failure)?;
        if active != &result.member || identity != input.source.identity() {
            return Err(invalid());
        }
        checkpoint().map_err(|error| {
            map_source_identity_error(super::SourceIdentityIdError::Owner(error))
        })?;
        Ok(result)
    }
}

fn validate_setup(
    input: &NativeMemberSetupInput<'_>,
    setup: &Setup,
) -> Result<(), RegistryPortError> {
    let permanent = setup.record.permanent_v2().ok_or_else(invalid)?;
    let member = permanent.member();
    if setup.selected != input.selected.canonical
        || member.source_id != input.source.identity().source_id
        || member.workspace_id != input.scope.workspace_id
        || member.corpus_id != input.scope.corpus_id
        || member.role != input.selected.role
        || member.preparation_profile_id != input.selected.preparation
        || member.retention_policy_id != input.selected.retention
        || permanent.provenance().source_identity() != input.source.identity()
        || permanent.provenance().root_binding_id() != input.root.root_binding_id
    {
        return Err(owner_failure(RegistryError::OperationConflict));
    }
    validate_new_ids(
        input,
        member.source_membership_id,
        member.access_policy_binding_id,
        member.residency_policy_binding_id,
    )
}
fn validate_new_ids(
    input: &NativeMemberSetupInput<'_>,
    member: c::SourceMembershipId,
    access: c::AccessPolicyBindingId,
    residency: c::ResidencyPolicyBindingId,
) -> Result<(), RegistryPortError> {
    let ids = [member.as_bytes(), access.as_bytes(), residency.as_bytes()];
    for (index, id) in ids.iter().enumerate() {
        if id[6] & 0xf0 != 0x40
            || id[8] & 0xc0 != 0x80
            || ids[..index].contains(id)
            || [
                input.root.root_binding_id.as_bytes(),
                input.source.identity().source_id.as_bytes(),
                input.namespace.ownership.source_namespace_id.as_bytes(),
                input.scope.workspace_id.as_bytes(),
                input.scope.corpus_id.as_bytes(),
            ]
            .contains(id)
            || input.selected.domain_ids.iter().any(|domain| domain == *id)
        {
            return Err(invalid());
        }
    }
    Ok(())
}
fn owner_selection(
    input: &NativeMemberSetupInput<'_>,
    setup: &Setup,
) -> Result<(SourceNamespaceSelectionV2, SourceOwnerScopeV2), RegistryPortError> {
    let ownership = &input.namespace.ownership;
    let provenance = setup
        .record
        .permanent_v2()
        .ok_or_else(invalid)?
        .provenance();
    Ok((
        SourceNamespaceSelectionV2::Existing {
            id: provenance.source_namespace_id(),
            revision: provenance.namespace_record_revision(),
            generation: provenance.owner_generation(),
        },
        SourceOwnerScopeV2 {
            system_id: ownership.owner_system_id.clone(),
            installation_incarnation: ownership.owner_installation_incarnation_id,
            epoch: ownership.owner_epoch,
        },
    ))
}
fn verify_conditions<C: CancellationProbe>(
    port: &RegistryJournal<'_, C>,
    conditions: &[Condition],
    generation: u64,
    deadline: std::time::Instant,
    work: super::OriginalNativeWork<'_>,
) -> Result<(), RegistryPortError> {
    let keys = conditions
        .iter()
        .map(|condition| condition.key().clone())
        .collect::<Vec<_>>();
    let read = port
        .journal
        .read_control_keys_with_original_context(&keys, deadline, &work)
        .map_err(|error| storage_failure(error.control, None))?;
    if read.identity() != port.identity
        || read.generation() != generation
        || conditions
            .iter()
            .any(|condition| read.get(condition.key()) != condition.expected())
    {
        return Err(invalid());
    }
    Ok(())
}
fn member_key(
    input: &NativeMemberSetupInput<'_>,
    limits: db::JournalLimits,
) -> Result<db::ControlKey, RegistryPortError> {
    db::ControlKey::new(
        [
            PREFIX,
            b"source/",
            input.root.root_binding_id.as_bytes(),
            input.source.identity().source_namespace_id.as_bytes(),
            input.source.identity().source_id.as_bytes(),
            input.scope.corpus_id.as_bytes(),
        ]
        .concat(),
        limits,
    )
    .map_err(|error| storage_failure(error, None))
}
fn operation_key(
    operation: &c::OpaqueId,
    limits: db::JournalLimits,
) -> Result<db::ControlKey, RegistryPortError> {
    db::ControlKey::new(
        [PREFIX, b"operation/", operation.as_str().as_bytes()].concat(),
        limits,
    )
    .map_err(|error| storage_failure(error, None))
}
fn bind_operation(operation: &c::OpaqueId) -> Result<c::OpaqueId, RegistryPortError> {
    c::OpaqueId::new(format!(
        "native-member-bind:{}",
        digest(operation.as_str().as_bytes())
    ))
    .map_err(|_| invalid())
}
fn member_receipt(operation: &c::OpaqueId) -> Result<c::ReceiptRef, RegistryPortError> {
    c::ReceiptRef::new(format!(
        "native-member-setup:{}",
        digest(operation.as_str().as_bytes())
    ))
    .map_err(|_| invalid())
}
fn setup_command(
    input: &NativeMemberSetupInput<'_>,
    setup: &Setup,
    encoded: &[u8],
    limits: db::JournalLimits,
) -> Result<db::ConditionalControlMutation, RegistryPortError> {
    let key = member_key(input, limits)?;
    let op_key = operation_key(&setup.operation, limits)?;
    let mut conditions = setup.conditions.clone();
    conditions.extend([
        Condition::absent(key.clone()),
        Condition::absent(op_key.clone()),
    ]);
    let id = db::MutationId(*digest(op_key.as_bytes()).as_bytes());
    let writes = [(key, Class::State), (op_key, Class::Operation)]
        .into_iter()
        .map(|(key, class)| {
            Ok(db::ControlWrite {
                key,
                value: db::ControlValue::new(class, encoded.to_vec(), limits)
                    .map_err(|error| storage_failure(error, None))?,
            })
        })
        .collect::<Result<Vec<_>, RegistryPortError>>()?;
    Ok(db::ConditionalControlMutation::new(
        db::ControlMutation::new(id, digest(encoded), setup.generation, writes, Vec::new()),
        conditions,
    ))
}
fn encode_setup(setup: &Setup) -> Result<Vec<u8>, RegistryPortError> {
    let guards = setup
        .conditions
        .iter()
        .map(|condition| {
            let value = condition.expected().ok_or_else(invalid)?;
            array(vec![
                bytes(condition.key().as_bytes())?,
                V::U64(class_tag(value.class())),
                bytes(value.as_bytes())?,
            ])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let member = RegistryStateRecord::Membership(setup.record.clone())
        .encode_with_profile("eliot.registry-state.v5")
        .map_err(owner_failure)?;
    encode(&array(vec![
        text(PROFILE)?,
        text(setup.operation.as_str())?,
        bytes(&setup.selected)?,
        bytes(&member)?,
        V::U64(setup.generation),
        array(guards)?,
    ])?)
}
fn decode_setup(encoded: &[u8], limits: db::JournalLimits) -> Result<Setup, RegistryPortError> {
    if encoded.len() > MAX_BYTES {
        return Err(invalid());
    }
    let V::Array(fields) = c::parse_canonical_cbor(encoded).map_err(|_| invalid())? else {
        return Err(invalid());
    };
    let [
        V::Text(profile),
        V::Text(operation),
        V::Bytes(selected),
        V::Bytes(member),
        V::U64(generation),
        V::Array(guards),
    ] = fields.as_slice()
    else {
        return Err(invalid());
    };
    if profile.as_str() != PROFILE || guards.is_empty() || guards.len() > MAX_GUARDS {
        return Err(invalid());
    }
    let RegistryStateRecord::Membership(record) =
        decode_registry_state(member.as_slice()).map_err(owner_failure)?
    else {
        return Err(invalid());
    };
    let conditions = guards
        .iter()
        .map(|guard| {
            let V::Array(guard) = guard else {
                return Err(invalid());
            };
            let [V::Bytes(key), V::U64(class), V::Bytes(value)] = guard.as_slice() else {
                return Err(invalid());
            };
            Ok(Condition::exact(
                db::ControlKey::new(key.as_slice().to_vec(), limits)
                    .map_err(|error| storage_failure(error, None))?,
                db::ControlValue::new(read_class(*class)?, value.as_slice().to_vec(), limits)
                    .map_err(|error| storage_failure(error, None))?,
            ))
        })
        .collect::<Result<Vec<_>, RegistryPortError>>()?;
    Ok(Setup {
        operation: c::OpaqueId::new(operation.as_str()).map_err(|_| invalid())?,
        selected: selected.as_slice().to_vec(),
        record,
        generation: *generation,
        conditions,
    })
}
const fn class_tag(class: Class) -> u64 {
    match class {
        Class::Identity => 0,
        Class::Revision => 1,
        Class::State => 2,
        Class::Receipt => 3,
        Class::Operation => 4,
        Class::Snapshot => 5,
        Class::Migration => 6,
    }
}
fn read_class(tag: u64) -> Result<Class, RegistryPortError> {
    match tag {
        0 => Ok(Class::Identity),
        1 => Ok(Class::Revision),
        2 => Ok(Class::State),
        3 => Ok(Class::Receipt),
        4 => Ok(Class::Operation),
        5 => Ok(Class::Snapshot),
        6 => Ok(Class::Migration),
        _ => Err(invalid()),
    }
}
fn digest(bytes: &[u8]) -> c::Blake3Digest32 {
    c::Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes())
}
fn invalid() -> RegistryPortError {
    owner_failure(RegistryError::SnapshotInvalid)
}
fn text(value: &str) -> Result<V, RegistryPortError> {
    c::CanonicalText::new(value)
        .map(V::Text)
        .map_err(|_| invalid())
}
fn bytes(value: &[u8]) -> Result<V, RegistryPortError> {
    c::BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| invalid())
}
fn array(value: Vec<V>) -> Result<V, RegistryPortError> {
    c::BoundedList::new(value)
        .map(V::Array)
        .map_err(|_| invalid())
}
fn encode(value: &V) -> Result<Vec<u8>, RegistryPortError> {
    let encoded = c::to_canonical_cbor(value)
        .map_err(|_| invalid())?
        .into_vec();
    if encoded.len() > MAX_BYTES {
        return Err(owner_failure(RegistryError::DurabilityExhausted));
    }
    Ok(encoded)
}
fn profile_id(prefix: &str, value: &V) -> Result<c::ProfileId, RegistryPortError> {
    c::ProfileId::new(format!("{prefix}:{}", digest(&encode(value)?))).map_err(|_| invalid())
}
