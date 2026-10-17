//! Actual Native/C02/Windows composition for C07's derived-object kernel.

use std::{path::Path, time::Instant};

use search_contracts::{self as c, SourceRevisionRef};
use search_control_redb::{
    self as db, ControlRecordClass, ControlRecordCondition, ControlSnapshotPublisher, ControlValue,
    JournalLimits, MigrationBootstrapWork as Work, MutationId, PersistentControlJournal,
    source_retention::{
        CommittedDerivedArtifactSealUse, DerivedArtifactObjectIntentReadback,
        DerivedArtifactObjectPublicationV1, DerivedArtifactReservationReadback,
        DerivedArtifactScopeGuardKeysV1, DerivedArtifactUseCommand, DerivedArtifactUseMetadataV1,
        RevisionUseUsageReadback,
    },
};
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiWork};
use search_revision_crypto::SelectedRevisionBindingV2;
use search_revision_store::derived_artifact::{
    DerivedArtifactDescriptorV1 as Descriptor, DerivedArtifactIoOperationV1,
    DerivedArtifactKindV1 as Kind, DerivedArtifactObjectRefV1 as ObjectRef,
    DerivedArtifactPayloadCandidateV1 as Candidate, DerivedArtifactPlaintextV1,
    DerivedArtifactSealBindingV1, DerivedArtifactStorageV1, DerivedArtifactVerificationPortV1,
    DerivedArtifactWriteReceiptV1, MAX_DERIVED_AAD_BYTES, MAX_DERIVED_SET_BYTES,
};
use search_revision_store::{DomainBinding, residency_canonical_bytes};
use search_runtime_owner::OwnerError;
use search_source_history::{
    prepare::{SourceRevisionPresealProfile, encode_source_revision_preseal_profile},
    revision::{SourceRevisionControlPort, encode_source_revision_record},
};
use search_source_registry::{
    api::read_projection_membership, membership::MembershipLifecycle,
    recovery::RegistryStateRecord, root::RootStatus, source::SourceLifecycle,
};

use crate::{
    config_composition::EffectiveDaemonConfig,
    preparation_composition::artifact_payloads::{
        PreparedRawArtifact, PreparedRawCodeArtifacts, RawCodeArtifactProducerInput,
    },
};

use super::super::read_child::fresh_root::{FreshRootCancellation, FreshRootContext};
use super::super::read_child::{NativeDerivedArtifactFailure, OriginalObjectContext};
use super::{
    command::original_port,
    dek_command::NativeContext,
    retained_reader::open_retained_revision,
    retention::{self, Selected},
    retention_control::{self, NativeRetentionControl},
    retention_source::RetentionSourceInput,
};

const LIMITS: JournalLimits = JournalLimits::BASELINE;

/// Actual C07 shared-kernel failure remains distinguishable from a pre-effect refusal.
pub enum NativeArtifactVerificationError {
    Refused,
}

pub enum NativeArtifactStoreError {
    OwnerRefused,
    ControlRefused,
    ControlOutcomeUnknown,
    C07(NativeDerivedArtifactFailure<NativeArtifactVerificationError>),
}
impl NativeArtifactStoreError {
    pub(crate) const fn outcome_unknown(&self) -> bool {
        match self {
            Self::OwnerRefused | Self::ControlRefused => false,
            Self::ControlOutcomeUnknown => true,
            Self::C07(error) => error.outcome_unknown(),
        }
    }
}

pub type NativeArtifactSetWrite = DerivedArtifactWriteReceiptV1<Vec<u8>>;

struct StoredArtifactEvidence {
    reservation_operation: MutationId,
    reservation: DerivedArtifactReservationReadback,
    intent: DerivedArtifactObjectIntentReadback,
    reference: ObjectRef,
    binding: DerivedArtifactSealBindingV1,
}

/// Concrete adapter created only from the existing admitted Native source operation.
pub struct NativeArtifactStoreAdapter<'input, 'view, 'operation, 'owner, 'journal> {
    input: &'input RetentionSourceInput<'view, 'operation, 'owner>,
    journal: &'journal mut PersistentControlJournal,
    publisher: &'journal mut ControlSnapshotPublisher,
    config: &'input EffectiveDaemonConfig,
    source_revision: SourceRevisionRef,
    source_record: c::SourceRevision,
    source_profile: SourceRevisionPresealProfile,
    retained_source: search_revision_crypto::SelectedPlaintextV2,
    occurrence_sequence: u64,
    operation_id: c::OpaqueId,
    selected: Selected,
    domains: [DomainBinding; 6],
    residency: Vec<u8>,
    guard_keys: DerivedArtifactScopeGuardKeysV1,
    scope: search_projection_planner::PlanningScope,
    schema: c::indexed::CollectionSchema,
    binding: search_unitizer::V3SourceBinding,
    maximum_plaintext_bytes: usize,
    evidence: Vec<StoredArtifactEvidence>,
}

impl<'input, 'view, 'operation, 'owner, 'journal>
    NativeArtifactStoreAdapter<'input, 'view, 'operation, 'owner, 'journal>
{
    #[allow(clippy::too_many_lines)] // One source admission chain preserves the original Native context.
    fn open(
        input: &'input RetentionSourceInput<'view, 'operation, 'owner>,
        journal: &'journal mut PersistentControlJournal,
        publisher: &'journal mut ControlSnapshotPublisher,
        config: &'input EffectiveDaemonConfig,
        producer: &RawCodeArtifactProducerInput<'_>,
    ) -> Result<Self, NativeArtifactStoreError> {
        let fail = || NativeArtifactStoreError::OwnerRefused;
        input.security.checkpoint().map_err(|_| fail())?;
        input.native.source_full_checkpoint().map_err(|_| fail())?;

        let original = input.existing.result.as_ref().ok_or_else(fail)?;
        let profile = input.existing.profile.as_ref().ok_or_else(fail)?.clone();
        if original.operation_id != input.command.operation
            || profile.operation_id != input.command.operation
            || producer.unitization_budget.deadline != input.deadline
            || !input
                .native
                .matches_original_work(input.deadline, producer.unitization_budget.cancelled)
            || original.source_namespace_id != input.source.identity().source_namespace_id
            || original.source_id != input.source.identity().source_id
            || profile.source_namespace_id != original.source_namespace_id
            || profile.source_id != original.source_id
            || profile.revision_id != original.revision.revision_id
            || profile.occurrence_sequence != original.revision.occurrence_sequence
            || profile.content_digest != original.revision.content_digest
            || profile.byte_length != original.revision.byte_length
        {
            return Err(fail());
        }
        let source_revision = SourceRevisionRef {
            source_namespace_id: original.source_namespace_id,
            source_id: original.source_id,
            revision_id: original.revision.revision_id,
            content_digest: original.revision.content_digest,
            byte_length: original.revision.byte_length,
        };

        // Resolve the actual C03 source/root/namespace/member and caller-owned
        // profile/partition scope before reopening the retained source body.
        validate_scope_shape(
            input,
            producer.scope,
            producer.schema,
            producer.binding,
            &source_revision,
            journal,
            publisher,
        )?;

        let work = |unit| input.native.work(unit);
        let control = NativeRetentionControl::bind(
            journal,
            publisher,
            input.source,
            input.namespace,
            input.root,
            input.security,
            input.deadline,
            &work,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let revision = control
            .load_source_revision(
                &source_revision.source_namespace_id,
                &source_revision.source_id,
                &source_revision.revision_id,
                input.native.context(),
            )
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?
            .filter(|value| value == &original.revision)
            .ok_or_else(fail)?;
        let head = control
            .load_source_revision_head(
                &source_revision.source_namespace_id,
                &source_revision.source_id,
                input.native.context(),
            )
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?
            .ok_or_else(fail)?;
        if head.state != search_source_history::revision::SourceRevisionSourceState::Active {
            return Err(fail());
        }
        let (retained_source, source_metadata) =
            open_retained_revision(input.native, &control, &source_revision, config)
                .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        drop(control);

        input
            .native
            .work(Work::Allocate(512))
            .and_then(|()| input.native.work(Work::Process(512)))
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        let expected_source_id = source_revision.source_id.to_string();
        if source_metadata.source().namespace_id != source_revision.source_namespace_id
            || source_metadata.source().source_id != source_revision.source_id
            || source_metadata.source().revision_id != source_revision.revision_id
            || source_metadata.source().sequence.get() != revision.occurrence_sequence
            || source_metadata.content_digest() != source_revision.content_digest
            || source_metadata.content_bytes() != source_revision.byte_length
            || source_metadata.operation_id() != &profile.operation_id
            || producer.request.source_id().as_str() != expected_source_id
            || producer.request.revision().get() != revision.occurrence_sequence
            || producer.request.content_digest() != source_revision.content_digest
            || producer.request.byte_count() != source_revision.byte_length
            || producer.request.operation_id() != &input.command.operation
            || producer.request.residency() != source_metadata.storage_object_id()
            || producer.binding.source_namespace_id != source_revision.source_namespace_id
            || producer.binding.source_id != source_revision.source_id
            || producer.binding.source_revision_id != source_revision.revision_id
            || producer.binding.representation_id != producer.scope.membership.representation_id
        {
            return Err(fail());
        }

        let selected = retention::selected(input, journal, publisher, config)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        if selected.current.is_empty() || selected.preparation.is_empty() {
            return Err(fail());
        }
        let existing_claim = selected
            .claim
            .as_deref()
            .unwrap_or(profile.residency_closure.as_slice());
        let domains = input
            .native
            .retention_domains(
                input.root,
                input.namespace,
                input.security,
                &selected.current,
                Some(existing_claim),
                input.deadline,
            )
            .map_err(|_| fail())?;
        if source_metadata.domain_bindings() != &domains
            || source_metadata.encryption() != &selected.encryption
            || selected
                .claim
                .as_deref()
                .is_some_and(|claim| claim != existing_claim)
        {
            return Err(fail());
        }
        let residency = residency_canonical_bytes(&domains).map_err(|_| fail())?;
        if residency != profile.residency_closure.as_slice() {
            return Err(fail());
        }
        let guard_keys = derived_guard_keys(input, &source_revision, &work)?;

        Ok(Self {
            input,
            journal,
            publisher,
            config,
            source_revision,
            source_record: revision,
            source_profile: profile,
            retained_source,
            occurrence_sequence: original.revision.occurrence_sequence,
            operation_id: input.command.operation.clone(),
            selected,
            domains,
            residency,
            guard_keys,
            scope: producer.scope.clone(),
            schema: producer.schema.clone(),
            binding: producer.binding.clone(),
            maximum_plaintext_bytes: producer.max_artifact_bytes,
            evidence: Vec::with_capacity(5),
        })
    }

    pub(crate) const fn source_revision(&self) -> &SourceRevisionRef {
        &self.source_revision
    }
    pub(crate) const fn occurrence_sequence(&self) -> u64 {
        self.occurrence_sequence
    }
    pub(crate) fn retained_source_body(&self) -> &[u8] {
        self.retained_source.as_bytes()
    }
    pub(crate) const fn domains(&self) -> &[DomainBinding; 6] {
        &self.domains
    }
    pub(crate) const fn operation_id(&self) -> &c::OpaqueId {
        &self.operation_id
    }
    pub(crate) const fn materialization_id(&self) -> c::MaterializationId {
        self.binding.materialization_id
    }
    pub(crate) const fn representation_id(&self) -> c::RepresentationId {
        self.binding.representation_id
    }
    pub(crate) const fn scope(&self) -> &search_projection_planner::PlanningScope {
        &self.scope
    }
    pub(crate) const fn schema(&self) -> &c::indexed::CollectionSchema {
        &self.schema
    }
    pub(crate) const fn deadline(&self) -> Instant {
        self.input.deadline
    }
    pub(crate) fn work(&self, unit: Work) -> Result<(), OwnerError> {
        self.input.native.work(unit)
    }
    pub(crate) fn residency_canonical(&self) -> Result<Vec<u8>, NativeArtifactStoreError> {
        self.charge(Work::Allocate(8 * 8192))?;
        self.charge(Work::Process(8 * 8192))?;
        residency_canonical_bytes(&self.domains).map_err(|_| NativeArtifactStoreError::OwnerRefused)
    }

    pub(crate) fn verify_prepared(
        &mut self,
        prepared: &PreparedRawCodeArtifacts,
    ) -> Result<(), NativeArtifactStoreError> {
        self.charge(Work::Process(4096))?;
        if prepared.units().manifest().provenance().binding() != &self.binding {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        for artifact in prepared.artifacts() {
            let descriptor = artifact.descriptor();
            if descriptor.source() != &self.source_revision
                || descriptor.operation_id() != &self.operation_id
                || descriptor.materialization_id() != self.binding.materialization_id
                || descriptor.representation_id() != self.binding.representation_id
                || descriptor.domains() != &self.domains
            {
                return Err(NativeArtifactStoreError::OwnerRefused);
            }
        }
        self.verify_current_owner_state(self.input.native)?;
        Ok(())
    }

    pub(crate) fn publish_member(
        &mut self,
        prepared: &PreparedRawCodeArtifacts,
        artifact: &PreparedRawArtifact,
    ) -> Result<DerivedArtifactWriteReceiptV1<Vec<u8>>, NativeArtifactStoreError> {
        if prepared.scope() != &self.scope || prepared.schema() != &self.schema {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        let descriptor = artifact.descriptor();
        if descriptor.kind() == Kind::SetManifest {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        let maximum = self.maximum_plaintext_bytes;
        let maximum_object = maximum
            .checked_add(MAX_DERIVED_AAD_BYTES + 64)
            .ok_or(NativeArtifactStoreError::OwnerRefused)?;
        self.seal_and_publish(
            descriptor,
            artifact.body(),
            maximum,
            maximum_object,
            prepared,
        )
    }

    pub(crate) fn publish_set(
        &mut self,
        prepared: &PreparedRawCodeArtifacts,
        set: &search_revision_store::derived_artifact::DerivedArtifactSetDescriptorV1,
        set_candidate: &Candidate,
    ) -> Result<DerivedArtifactWriteReceiptV1<Vec<u8>>, NativeArtifactStoreError> {
        if prepared.scope() != &self.scope
            || prepared.schema() != &self.schema
            || set_candidate.object_ref().descriptor().kind() != Kind::SetManifest
            || set_candidate.object_ref().descriptor().operation_id() != &self.operation_id
        {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        let base_descriptor = set_candidate.object_ref().descriptor();
        let plaintext = set_candidate.encoded_object();
        let set_limit = MAX_DERIVED_SET_BYTES;
        let set_object_limit = set_limit
            .checked_add(MAX_DERIVED_AAD_BYTES + 64)
            .ok_or(NativeArtifactStoreError::OwnerRefused)?;
        let receipt = self.seal_and_publish(
            base_descriptor,
            plaintext,
            set_limit,
            set_object_limit,
            prepared,
        )?;
        if receipt.object_ref().descriptor().owner_semantic_digest()
            != set
                .semantic_digest()
                .map_err(|_| NativeArtifactStoreError::OwnerRefused)?
        {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        Ok(receipt)
    }

    pub(crate) fn final_checkpoint(&mut self) -> Result<(), NativeArtifactStoreError> {
        self.input
            .security
            .checkpoint()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        self.verify_current_owner_state(self.input.native)
    }

    #[allow(clippy::too_many_lines)] // Keep C02 reservation, Windows sealing, late intent and C07 order visible.
    fn seal_and_publish(
        &mut self,
        descriptor: &Descriptor,
        plaintext: &[u8],
        maximum_plaintext: usize,
        maximum_object: usize,
        prepared: &PreparedRawCodeArtifacts,
    ) -> Result<DerivedArtifactWriteReceiptV1<Vec<u8>>, NativeArtifactStoreError> {
        if plaintext.len() > maximum_plaintext || maximum_object < maximum_plaintext {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        self.verify_descriptor_scope(descriptor)?;
        self.verify_current_owner_state(self.input.native)?;

        let descriptor_bytes = descriptor
            .encode()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        let (reservation_operation, command_digest) = retention_control::phase_id(
            descriptor.operation_id(),
            &format!("derived-seal-{}", descriptor.kind().as_str()),
            &|unit| self.work(unit),
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let counter = self.next_usage_counter(reservation_operation)?;
        let source_profile_bytes = encode_source_revision_preseal_profile(&self.source_profile)
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        let nonce_binding = SelectedRevisionBindingV2::from_owner_records(
            &self.selected.current,
            &source_profile_bytes,
            &self.residency,
            descriptor.operation_id(),
            counter,
            descriptor.source().content_digest,
            descriptor.source().byte_length,
        )
        .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        if nonce_binding.counter() != counter {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        let seal_binding = DerivedArtifactSealBindingV1::new(
            descriptor.clone(),
            &self.selected.current,
            self.selected.encryption.clone(),
            counter,
            nonce_binding.nonce(),
        )
        .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        if u64::try_from(plaintext.len()).ok() != Some(descriptor.encoded_body_bytes())
            || seal_binding.authenticated_blocks().is_err()
        {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }

        let metadata = DerivedArtifactUseMetadataV1 {
            operation_id: descriptor.operation_id().clone(),
            source_revision: self.source_revision,
            materialization_id: descriptor.materialization_id(),
            representation_id: descriptor.representation_id(),
            descriptor_canonical: descriptor_bytes,
            residency_canonical: self.residency.clone(),
            selected_current_canonical: self.selected.current.clone(),
            aad_canonical: seal_binding.aad_bytes().to_vec(),
            plaintext_length: descriptor.encoded_body_bytes(),
            expected_counter: counter,
        };
        self.charge(Work::Allocate(
            metadata
                .descriptor_canonical
                .len()
                .saturating_add(metadata.residency_canonical.len())
                .saturating_add(metadata.selected_current_canonical.len())
                .saturating_add(metadata.aad_canonical.len())
                .saturating_add(64 * 1024),
        ))?;
        self.charge(Work::Process(
            metadata
                .descriptor_canonical
                .len()
                .saturating_add(metadata.residency_canonical.len())
                .saturating_add(metadata.selected_current_canonical.len())
                .saturating_add(metadata.aad_canonical.len())
                .saturating_add(64 * 1024),
        ))?;

        let guard_conditions = self.guard_conditions()?;
        let native = self.input.native;
        let work = |unit| native.work(unit);
        let mut port = self
            .journal
            .bind_control_port::<FreshRootCancellation>(self.publisher)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let keys = db::source_retention::derived_artifact_reservation_keys(
            reservation_operation,
            &self.guard_keys,
            LIMITS,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let read = port
            .read_control_keys_with_original_context(&keys, self.input.deadline, &work)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        verify_guard_values(&guard_conditions, &read)?;
        let usage = RevisionUseUsageReadback::read(&read, &self.selected.preparation, LIMITS)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        if usage.attempts() != counter
            || usage
                .residency_canonical()
                .is_some_and(|value| value != self.residency)
        {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        let command = DerivedArtifactUseCommand::reserve(
            &read,
            reservation_operation,
            command_digest,
            &self.selected.preparation,
            metadata,
            &self.guard_keys,
            LIMITS,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let identity = db::control_mutation_identity(reservation_operation)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let (commit, permit) = port
            .transact_derived_artifact_use_with_original_context(
                &command,
                &identity,
                self.input.deadline,
                &work,
            )
            .map_err(|error| {
                if error.control == db::ControlError::CommitOutcomeUnknown {
                    NativeArtifactStoreError::ControlOutcomeUnknown
                } else {
                    NativeArtifactStoreError::ControlRefused
                }
            })?;
        if commit.replayed {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        let permit = permit.ok_or(NativeArtifactStoreError::ControlRefused)?;
        self.verify_current_owner_state(self.input.native)?;
        let reservation = self.read_reservation(reservation_operation)?;
        self.verify_reservation(&reservation, &permit, descriptor, &seal_binding)?;

        let native = self.input.native;
        let poll = || native.work(Work::Checkpoint);
        let native_work = |unit| match unit {
            LegacyRevisionDpapiWork::NativeCall => native.revision_dek_native_attempt(),
            LegacyRevisionDpapiWork::Allocate(bytes) => native.work(Work::Allocate(bytes)),
            LegacyRevisionDpapiWork::Process(bytes) => native.work(Work::Process(bytes)),
        };
        let open = || native.work(Work::Open);
        let clear = |bytes| native.revision_dek_account_native_clear(bytes);
        let mut windows = NativeContext {
            deadline: self.input.deadline,
            poll: &poll,
            work: &native_work,
            open: &open,
            clear: &clear,
        };
        let ciphertext = os::seal_derived_artifact(
            permit,
            &seal_binding,
            plaintext,
            self.config.secrets_settings(),
            &mut windows,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        self.charge(Work::Allocate(ciphertext.len().saturating_mul(2)))?;
        self.charge(Work::Process(ciphertext.len()))?;
        let candidate = Candidate::sealed(&seal_binding, ciphertext, maximum_object)
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;

        self.verify_current_owner_state(self.input.native)?;
        let reference = candidate.object_ref();
        let address = reference
            .address()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        let object = DerivedArtifactObjectPublicationV1 {
            object_reference_canonical: reference
                .encode()
                .map_err(|_| NativeArtifactStoreError::OwnerRefused)?,
            address_bytes: address.relative_path().into_bytes(),
            stored_object_digest: reference.stored_object_digest(),
            stored_object_length: reference.stored_bytes(),
        };
        let (intent_operation, intent_digest) = retention_control::phase_id(
            descriptor.operation_id(),
            &format!("derived-object-{}", descriptor.kind().as_str()),
            &|unit| self.work(unit),
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let guard_conditions = self.guard_conditions()?;
        let native = self.input.native;
        let work = |unit| native.work(unit);
        let mut port = self
            .journal
            .bind_control_port::<FreshRootCancellation>(self.publisher)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let keys = db::source_retention::derived_artifact_object_intent_keys(
            reservation_operation,
            &self.guard_keys,
            LIMITS,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let read = port
            .read_control_keys_with_original_context(&keys, self.input.deadline, &work)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        verify_guard_values(&guard_conditions, &read)?;
        let current_reservation =
            DerivedArtifactReservationReadback::read(&read, reservation_operation, LIMITS)
                .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        if current_reservation != reservation {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        let intent = DerivedArtifactUseCommand::bind_object_publication(
            &read,
            reservation_operation,
            intent_operation,
            intent_digest,
            object,
            &self.guard_keys,
            LIMITS,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let identity = db::control_mutation_identity(intent_operation)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let commit = port
            .transact_derived_artifact_object_intent_with_original_context(
                &intent,
                &identity,
                self.input.deadline,
                &work,
            )
            .map_err(|error| {
                if error.control == db::ControlError::CommitOutcomeUnknown {
                    NativeArtifactStoreError::ControlOutcomeUnknown
                } else {
                    NativeArtifactStoreError::ControlRefused
                }
            })?;
        if commit.replayed {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        let (reservation, intent) = self.read_intent(reservation_operation)?;
        Self::verify_intent(&reservation, &intent, &candidate, descriptor, &seal_binding)?;
        let evidence = StoredArtifactEvidence {
            reservation_operation,
            reservation,
            intent,
            reference: candidate.object_ref().clone(),
            binding: seal_binding,
        };
        self.charge(Work::Allocate(std::mem::size_of::<StoredArtifactEvidence>()))?;
        self.evidence.push(evidence);

        let native = self.input.native;
        let temporary = format!(
            "derived-{}.tmp",
            super::super::super::codec::hex(&reservation_operation.0),
        );
        let mut verifier = NativeArtifactVerificationPort {
            adapter: self,
            prepared,
        };
        let receipt = native
            .publish_derived_artifact_v1(&mut verifier, &temporary, &candidate, maximum_object)
            .map_err(NativeArtifactStoreError::C07)?;
        if receipt.object_ref() != candidate.object_ref() {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        self.verify_current_owner_state(self.input.native)?;
        self.read_and_verify_intent(reservation_operation)?;
        Ok(receipt)
    }

    fn next_usage_counter(
        &mut self,
        reservation_operation: MutationId,
    ) -> Result<u64, NativeArtifactStoreError> {
        self.verify_current_owner_state(self.input.native)?;
        let guard_conditions = self.guard_conditions()?;
        let native = self.input.native;
        let work = |unit| native.work(unit);
        let port = self
            .journal
            .bind_control_port::<FreshRootCancellation>(self.publisher)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let keys = db::source_retention::derived_artifact_reservation_keys(
            reservation_operation,
            &self.guard_keys,
            LIMITS,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let read = port
            .read_control_keys_with_original_context(&keys, self.input.deadline, &work)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        verify_guard_values(&guard_conditions, &read)?;
        let usage = RevisionUseUsageReadback::read(&read, &self.selected.preparation, LIMITS)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        if usage
            .residency_canonical()
            .is_some_and(|value| value != self.residency)
        {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        let _ = reservation_operation;
        Ok(usage.attempts())
    }

    fn read_reservation(
        &mut self,
        reservation_operation: MutationId,
    ) -> Result<DerivedArtifactReservationReadback, NativeArtifactStoreError> {
        let guard_conditions = self.guard_conditions()?;
        let native = self.input.native;
        let work = |unit| native.work(unit);
        let port = self
            .journal
            .bind_control_port::<FreshRootCancellation>(self.publisher)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let keys = db::source_retention::derived_artifact_reservation_keys(
            reservation_operation,
            &self.guard_keys,
            LIMITS,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let read = port
            .read_control_keys_with_original_context(&keys, self.input.deadline, &work)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        verify_guard_values(&guard_conditions, &read)?;
        DerivedArtifactReservationReadback::read(&read, reservation_operation, LIMITS)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)
    }

    fn read_intent(
        &mut self,
        reservation_operation: MutationId,
    ) -> Result<
        (
            DerivedArtifactReservationReadback,
            DerivedArtifactObjectIntentReadback,
        ),
        NativeArtifactStoreError,
    > {
        let guard_conditions = self.guard_conditions()?;
        let native = self.input.native;
        let work = |unit| native.work(unit);
        let port = self
            .journal
            .bind_control_port::<FreshRootCancellation>(self.publisher)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let keys = db::source_retention::derived_artifact_object_intent_keys(
            reservation_operation,
            &self.guard_keys,
            LIMITS,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let read = port
            .read_control_keys_with_original_context(&keys, self.input.deadline, &work)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        verify_guard_values(&guard_conditions, &read)?;
        let reservation =
            DerivedArtifactReservationReadback::read(&read, reservation_operation, LIMITS)
                .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let intent =
            DerivedArtifactObjectIntentReadback::read(&read, reservation_operation, LIMITS)
                .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        Ok((reservation, intent))
    }

    fn read_and_verify_intent(
        &mut self,
        reservation_operation: MutationId,
    ) -> Result<(), NativeArtifactStoreError> {
        let (reservation, intent) = self.read_intent(reservation_operation)?;
        let evidence = self
            .evidence
            .iter()
            .find(|item| item.reservation_operation == reservation_operation)
            .ok_or(NativeArtifactStoreError::ControlRefused)?;
        if reservation != evidence.reservation || intent != evidence.intent {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        Ok(())
    }

    fn verify_reservation(
        &self,
        reservation: &DerivedArtifactReservationReadback,
        permit: &CommittedDerivedArtifactSealUse,
        descriptor: &Descriptor,
        binding: &DerivedArtifactSealBindingV1,
    ) -> Result<(), NativeArtifactStoreError> {
        if reservation.source_revision_ref() != &self.source_revision
            || reservation.operation_id() != &self.operation_id
            || reservation.materialization_id() != descriptor.materialization_id()
            || reservation.representation_id() != descriptor.representation_id()
            || reservation.descriptor_canonical()
                != descriptor
                    .encode()
                    .map_err(|_| NativeArtifactStoreError::OwnerRefused)?
            || reservation.residency_canonical() != self.residency
            || reservation.selected_current_canonical() != self.selected.current
            || reservation.aad_canonical() != binding.aad_bytes()
            || reservation.plaintext_length() != descriptor.encoded_body_bytes()
            || reservation.counter() != binding.counter()
            || reservation.authenticated_blocks()
                != binding
                    .authenticated_blocks()
                    .map_err(|_| NativeArtifactStoreError::OwnerRefused)?
            || reservation.scope_guard_keys()
                != [
                    self.guard_keys.root_owner(),
                    self.guard_keys.source_revision(),
                    self.guard_keys.namespace_owner(),
                    self.guard_keys.residency_policy(),
                    self.guard_keys.purge_fence(),
                ]
            || permit.source_revision_ref() != &self.source_revision
            || permit.metadata().operation_id != self.operation_id
            || permit.materialization_id() != descriptor.materialization_id()
            || permit.representation_id() != descriptor.representation_id()
            || permit.descriptor_canonical() != reservation.descriptor_canonical()
            || permit.residency_canonical() != reservation.residency_canonical()
            || permit.selected_current_canonical() != reservation.selected_current_canonical()
            || permit.aad_canonical() != reservation.aad_canonical()
            || permit.counter() != reservation.counter()
            || permit.plaintext_length() != reservation.plaintext_length()
            || permit.authenticated_blocks() != reservation.authenticated_blocks()
        {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        Ok(())
    }

    fn verify_intent(
        reservation: &DerivedArtifactReservationReadback,
        intent: &DerivedArtifactObjectIntentReadback,
        candidate: &Candidate,
        descriptor: &Descriptor,
        binding: &DerivedArtifactSealBindingV1,
    ) -> Result<(), NativeArtifactStoreError> {
        let reference = candidate.object_ref();
        let encoded = reference
            .encode()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        let address = reference
            .address()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?
            .relative_path();
        if !intent.binds(reservation)
            || intent.operation_id() != descriptor.operation_id()
            || intent.object_reference_canonical() != encoded
            || intent.address_bytes() != address.as_bytes()
            || intent.stored_object_digest() != reference.stored_object_digest()
            || intent.stored_object_length() != reference.stored_bytes()
            || intent.counter() != binding.counter()
            || intent.authenticated_blocks()
                != binding
                    .authenticated_blocks()
                    .map_err(|_| NativeArtifactStoreError::OwnerRefused)?
            || intent.plaintext_length() != descriptor.encoded_body_bytes()
        {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        Ok(())
    }

    fn verify_c02_evidence(
        &mut self,
        reference: &ObjectRef,
    ) -> Result<&StoredArtifactEvidence, NativeArtifactStoreError> {
        let index = self
            .evidence
            .iter()
            .position(|item| &item.reference == reference)
            .ok_or(NativeArtifactStoreError::ControlRefused)?;
        let operation = self.evidence[index].reservation_operation;
        let (reservation, intent) = self.read_intent(operation)?;
        if reservation != self.evidence[index].reservation
            || intent != self.evidence[index].intent
            || !intent.binds(&reservation)
            || reservation.scope_guard_keys()
                != [
                    self.guard_keys.root_owner(),
                    self.guard_keys.source_revision(),
                    self.guard_keys.namespace_owner(),
                    self.guard_keys.residency_policy(),
                    self.guard_keys.purge_fence(),
                ]
            || intent.object_reference_canonical()
                != reference
                    .encode()
                    .map_err(|_| NativeArtifactStoreError::OwnerRefused)?
            || intent.stored_object_digest() != reference.stored_object_digest()
            || intent.stored_object_length() != reference.stored_bytes()
        {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        Ok(&self.evidence[index])
    }

    fn verify_descriptor_scope(
        &self,
        descriptor: &Descriptor,
    ) -> Result<(), NativeArtifactStoreError> {
        verify_derived_artifact_scope(
            descriptor,
            &self.source_revision,
            &self.operation_id,
            &self.domains,
            self.binding.materialization_id,
            self.binding.representation_id,
            &self.residency,
            &|unit| self.input.native.work(unit),
        )
    }

    fn verify_current_owner_state(
        &mut self,
        native: &FreshRootContext<'operation, 'owner>,
    ) -> Result<(), NativeArtifactStoreError> {
        if !std::ptr::eq(native, self.input.native) {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        native
            .source_full_checkpoint()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        self.input
            .security
            .checkpoint()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        self.verify_current_c03()?;

        let work = |unit| native.work(unit);
        let control = NativeRetentionControl::bind(
            self.journal,
            self.publisher,
            self.input.source,
            self.input.namespace,
            self.input.root,
            self.input.security,
            self.input.deadline,
            &work,
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let current_revision = control
            .load_source_revision(
                &self.source_revision.source_namespace_id,
                &self.source_revision.source_id,
                &self.source_revision.revision_id,
                native.context(),
            )
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?
            .ok_or(NativeArtifactStoreError::ControlRefused)?;
        if current_revision != self.source_record {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        drop(control);

        let selected = retention::selected(self.input, self.journal, self.publisher, self.config)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        if selected.current != self.selected.current
            || selected.preparation != self.selected.preparation
            || selected.encryption != self.selected.encryption
            || selected
                .claim
                .as_deref()
                .is_some_and(|value| value != self.residency)
        {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        let claim = selected
            .claim
            .as_deref()
            .unwrap_or(self.source_profile.residency_closure.as_slice());
        let domains = native
            .retention_domains(
                self.input.root,
                self.input.namespace,
                self.input.security,
                &selected.current,
                Some(claim),
                self.input.deadline,
            )
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        if domains != self.domains
            || residency_canonical_bytes(&domains)
                .map_err(|_| NativeArtifactStoreError::OwnerRefused)?
                != self.residency
        {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        self.input
            .security
            .checkpoint()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)
    }

    fn verify_current_c03(&mut self) -> Result<(), NativeArtifactStoreError> {
        validate_scope_shape(
            self.input,
            &self.scope,
            &self.schema,
            &self.binding,
            &self.source_revision,
            self.journal,
            self.publisher,
        )
    }

    fn guard_conditions(&self) -> Result<[ControlRecordCondition; 5], NativeArtifactStoreError> {
        let root = registry_owner_condition(
            &RegistryStateRecord::Root(self.input.root.clone()),
            db::RegistryStateFamily::Root,
            None,
        )?;
        let namespace = registry_owner_condition(
            &RegistryStateRecord::Namespace(self.input.namespace.clone()),
            db::RegistryStateFamily::Namespace,
            Some("eliot.registry-state.v4"),
        )?;
        let source_revision = retention_control::source_revision_guard_key(
            self.source_revision.source_namespace_id,
            self.source_revision.source_id,
            self.source_revision.revision_id,
            &|unit| self.input.native.work(unit),
        )
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let bytes = encode_source_revision_record(&self.source_record)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let source_value = ControlValue::new(ControlRecordClass::Revision, bytes, LIMITS)
            .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
        let security = self.input.security.conditions();
        Ok([
            root,
            ControlRecordCondition::exact(source_revision, source_value),
            namespace,
            security[0].clone(),
            security[1].clone(),
        ])
    }

    fn charge(&self, unit: Work) -> Result<(), NativeArtifactStoreError> {
        self.input
            .native
            .work(unit)
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)
    }
}

struct NativeArtifactVerificationPort<'a, 'input, 'view, 'operation, 'owner, 'journal> {
    adapter: &'a mut NativeArtifactStoreAdapter<'input, 'view, 'operation, 'owner, 'journal>,
    prepared: &'a PreparedRawCodeArtifacts,
}

impl<'operation, 'owner, 'context>
    DerivedArtifactVerificationPortV1<OriginalObjectContext<'context, 'operation, 'owner>>
    for NativeArtifactVerificationPort<'_, '_, '_, 'operation, 'owner, '_>
{
    type Error = NativeArtifactVerificationError;

    fn verify_storage_scope(
        &mut self,
        context: &mut OriginalObjectContext<'context, 'operation, 'owner>,
        cas_root: &Path,
        reference: &ObjectRef,
        operation: DerivedArtifactIoOperationV1,
    ) -> Result<(), Self::Error> {
        self.adapter
            .verify_storage_scope(context, cas_root, reference, operation)
            .map_err(|_| NativeArtifactVerificationError::Refused)
    }

    fn verify_owner_payload(
        &mut self,
        context: &mut OriginalObjectContext<'context, 'operation, 'owner>,
        descriptor: &Descriptor,
        encoded_body: &[u8],
    ) -> Result<(), Self::Error> {
        context
            .fresh_root()
            .work(Work::Allocate(encoded_body.len().saturating_mul(2)))
            .map_err(|_| NativeArtifactVerificationError::Refused)?;
        context
            .fresh_root()
            .work(Work::Process(encoded_body.len()))
            .map_err(|_| NativeArtifactVerificationError::Refused)?;
        self.prepared
            .verify_owner_payload(descriptor, encoded_body)
            .map_err(|_| NativeArtifactVerificationError::Refused)
    }

    fn open_authenticated(
        &mut self,
        context: &mut OriginalObjectContext<'context, 'operation, 'owner>,
        binding: &DerivedArtifactSealBindingV1,
        ciphertext_and_tag: &[u8],
    ) -> Result<DerivedArtifactPlaintextV1, Self::Error> {
        let deadline = self.adapter.input.deadline;
        let config = self.adapter.config;
        let evidence = self
            .adapter
            .verify_c02_evidence_for_binding(binding)
            .map_err(|_| NativeArtifactVerificationError::Refused)?;
        let native = context.fresh_root();
        let poll = || native.work(Work::Checkpoint);
        let native_work = |unit| match unit {
            LegacyRevisionDpapiWork::NativeCall => native.revision_dek_native_attempt(),
            LegacyRevisionDpapiWork::Allocate(bytes) => native.work(Work::Allocate(bytes)),
            LegacyRevisionDpapiWork::Process(bytes) => native.work(Work::Process(bytes)),
        };
        let open = || native.work(Work::Open);
        let clear = |bytes| native.revision_dek_account_native_clear(bytes);
        let mut windows = NativeContext {
            deadline,
            poll: &poll,
            work: &native_work,
            open: &open,
            clear: &clear,
        };
        let plaintext = os::open_derived_artifact(
            &evidence.reservation,
            binding,
            ciphertext_and_tag,
            config.secrets_settings(),
            &mut windows,
        )
        .map_err(|_| NativeArtifactVerificationError::Refused)?;
        native
            .work(Work::Allocate(plaintext.as_bytes().len().saturating_mul(2)))
            .map_err(|_| NativeArtifactVerificationError::Refused)?;
        native
            .work(Work::Process(plaintext.as_bytes().len()))
            .map_err(|_| NativeArtifactVerificationError::Refused)?;
        Ok(DerivedArtifactPlaintextV1::new(
            plaintext.as_bytes().to_vec(),
        ))
    }
}

impl<'operation, 'owner> NativeArtifactStoreAdapter<'_, '_, 'operation, 'owner, '_> {
    #[allow(clippy::needless_pass_by_ref_mut)] // C07 requires the original mutable I/O context type.
    fn verify_storage_scope(
        &mut self,
        context: &mut OriginalObjectContext<'_, 'operation, 'owner>,
        cas_root: &Path,
        reference: &ObjectRef,
        _operation: DerivedArtifactIoOperationV1,
    ) -> Result<(), NativeArtifactStoreError> {
        if !std::ptr::eq(context.fresh_root(), self.input.native) {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        context
            .verify_cas_root(cas_root)
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
        verify_derived_artifact_scope(
            reference.descriptor(),
            &self.source_revision,
            &self.operation_id,
            &self.domains,
            self.binding.materialization_id,
            self.binding.representation_id,
            &self.residency,
            &|unit| self.input.native.work(unit),
        )?;
        if reference.storage() != DerivedArtifactStorageV1::Aes256GcmEnvelope {
            return Err(NativeArtifactStoreError::OwnerRefused);
        }
        self.verify_current_owner_state(context.fresh_root())?;
        self.verify_c02_evidence(reference)?;
        self.input
            .security
            .checkpoint()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)
    }

    fn verify_c02_evidence_for_binding(
        &self,
        binding: &DerivedArtifactSealBindingV1,
    ) -> Result<&StoredArtifactEvidence, NativeArtifactStoreError> {
        let descriptor = binding.descriptor();
        let evidence = self
            .evidence
            .iter()
            .find(|item| item.reference.descriptor() == descriptor)
            .ok_or(NativeArtifactStoreError::ControlRefused)?;
        if &evidence.binding != binding
            || binding.selected_current_canonical()
                != evidence.reservation.selected_current_canonical()
            || binding.aad_bytes() != evidence.reservation.aad_canonical()
            || binding.counter() != evidence.reservation.counter()
            || binding
                .authenticated_blocks()
                .map_err(|_| NativeArtifactStoreError::OwnerRefused)?
                != evidence.reservation.authenticated_blocks()
        {
            return Err(NativeArtifactStoreError::ControlRefused);
        }
        Ok(evidence)
    }
}

fn validate_scope_shape(
    input: &RetentionSourceInput<'_, '_, '_>,
    scope: &search_projection_planner::PlanningScope,
    schema: &c::indexed::CollectionSchema,
    binding: &search_unitizer::V3SourceBinding,
    source: &SourceRevisionRef,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
) -> Result<(), NativeArtifactStoreError> {
    let fail = || NativeArtifactStoreError::OwnerRefused;
    input.native.source_full_checkpoint().map_err(|_| fail())?;
    input.security.checkpoint().map_err(|_| fail())?;
    let member_id = scope.membership.source_membership_id;
    let work = |unit| input.native.work(unit);
    let port = original_port(journal, publisher, input.deadline, &work).map_err(|_| fail())?;
    let registry = port
        .restore(input.native.context())
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?
        .into_registry();
    drop(port);

    let identity = input.source.identity();
    if registry.roots().get(&input.root.root_binding_id) != Some(input.root)
        || registry
            .namespaces()
            .get(&input.namespace.ownership.source_namespace_id)
            != Some(input.namespace)
        || registry.sources().get(identity) != Some(input.source)
        || input.root.status != RootStatus::Active
        || input.source.lifecycle() != SourceLifecycle::Active
        || input.source.binding().root_binding_id() != input.root.root_binding_id
        || input.namespace.fenced
        || input.namespace.preparation.is_some()
        || input.namespace.activation.is_some()
        || input.namespace.ownership.status != c::NamespaceOwnershipStatus::Active
        || input.namespace.ownership.source_namespace_id != source.source_namespace_id
        || identity.source_namespace_id != source.source_namespace_id
        || identity.source_id != source.source_id
        || binding.source_namespace_id != source.source_namespace_id
        || binding.source_id != source.source_id
        || binding.source_revision_id != source.revision_id
        || binding.representation_id != scope.membership.representation_id
        || scope.membership.source_id != source.source_id
        || scope.membership.source_revision_id != source.revision_id
        || scope.membership.source_membership_id != member_id
    {
        return Err(fail());
    }

    let (member, member_identity) = registry
        .active_source_membership_v2(member_id)
        .map_err(|_| fail())?;
    let member_key = registry
        .membership_key_by_id(member_id)
        .map_err(|_| fail())?;
    let member_record = registry.membership(member_key).map_err(|_| fail())?;
    if member_identity != identity
        || member.source_membership_id != member_id
        || member.source_id != source.source_id
        || member_record.lifecycle() != MembershipLifecycle::Active
        || member_record.permanent_v2().is_none()
    {
        return Err(fail());
    }
    let projection = read_projection_membership(&registry, member_id, registry.revision())
        .map_err(|_| fail())?;
    let actual = projection.binding();
    if actual.projection_membership_id != scope.membership.projection_membership_id
        || actual.source_membership_id != scope.membership.source_membership_id
        || actual.representation_id != scope.membership.representation_id
        || actual.access_partition_id != scope.membership.access_partition_id
        || actual.scoring_partition_id != scope.membership.scoring_partition_id
        || actual.projection_schema_id != scope.membership.projection_schema_id
    {
        return Err(fail());
    }
    let facts = input.native.revision_dek_root_facts().map_err(|_| fail())?;
    schema.validate().map_err(|_| fail())?;
    if schema.installation_incarnation_id() != &facts.incarnation
        || scope.membership.installation_incarnation_id != facts.incarnation
        || schema.collection_generation_id() != &scope.membership.collection_generation_id
        || schema.projection_profile_set_id() != &scope.membership.projection_profile_set_id
    {
        return Err(fail());
    }
    input.security.checkpoint().map_err(|_| fail())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Every independent source/residency identity stays explicit.
pub(super) fn verify_derived_artifact_scope(
    descriptor: &Descriptor,
    source: &SourceRevisionRef,
    operation: &c::OpaqueId,
    domains: &[DomainBinding; 6],
    materialization_id: c::MaterializationId,
    representation_id: c::RepresentationId,
    expected_residency: &[u8],
    work: &dyn Fn(Work) -> Result<(), OwnerError>,
) -> Result<(), NativeArtifactStoreError> {
    if descriptor.source() != source
        || descriptor.operation_id() != operation
        || descriptor.domains() != domains
        || descriptor.representation_id() != representation_id
        || descriptor.materialization_id() != materialization_id
    {
        return Err(NativeArtifactStoreError::OwnerRefused);
    }
    for unit in [Work::Allocate(8 * 8192), Work::Process(8 * 8192)] {
        work(unit).map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
    }
    let residency = residency_canonical_bytes(descriptor.domains())
        .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
    if residency != expected_residency {
        return Err(NativeArtifactStoreError::OwnerRefused);
    }
    Ok(())
}

pub(super) fn registry_owner_condition(
    record: &RegistryStateRecord,
    family: db::RegistryStateFamily,
    profile: Option<&str>,
) -> Result<ControlRecordCondition, NativeArtifactStoreError> {
    let key_bytes = record
        .key()
        .encode()
        .map_err(|_| NativeArtifactStoreError::OwnerRefused)?;
    let value = match profile {
        Some(profile) => record
            .encode_with_profile(profile)
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?,
        None => record
            .encode()
            .map_err(|_| NativeArtifactStoreError::OwnerRefused)?,
    };
    let key = db::RegistryStateKey::new(family, key_bytes, LIMITS)
        .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
    key.current_condition(&value, LIMITS)
        .map_err(|_| NativeArtifactStoreError::ControlRefused)
}

pub(super) fn verify_guard_values(
    conditions: &[ControlRecordCondition; 5],
    read: &db::ScopedControlRecordsReadback,
) -> Result<(), NativeArtifactStoreError> {
    if conditions
        .iter()
        .any(|condition| read.get(condition.key()) != condition.expected())
    {
        return Err(NativeArtifactStoreError::ControlRefused);
    }
    Ok(())
}

fn derived_guard_keys(
    input: &RetentionSourceInput<'_, '_, '_>,
    source: &SourceRevisionRef,
    work: &dyn Fn(Work) -> Result<(), OwnerError>,
) -> Result<DerivedArtifactScopeGuardKeysV1, NativeArtifactStoreError> {
    let root = registry_owner_condition(
        &RegistryStateRecord::Root(input.root.clone()),
        db::RegistryStateFamily::Root,
        None,
    )?;
    let namespace = registry_owner_condition(
        &RegistryStateRecord::Namespace(input.namespace.clone()),
        db::RegistryStateFamily::Namespace,
        Some("eliot.registry-state.v4"),
    )?;
    let source_revision = retention_control::source_revision_guard_key(
        source.source_namespace_id,
        source.source_id,
        source.revision_id,
        work,
    )
    .map_err(|_| NativeArtifactStoreError::ControlRefused)?;
    let security = input.security.conditions();
    DerivedArtifactScopeGuardKeysV1::new(
        root.key().clone(),
        source_revision,
        namespace.key().clone(),
        security[0].key().clone(),
        security[1].key().clone(),
    )
    .map_err(|_| NativeArtifactStoreError::ControlRefused)
}

#[allow(dead_code)] // Parent-owned ordinary factory caller remains a separate cutover.
pub(super) fn store_native_raw_code_artifacts(
    input: &RetentionSourceInput<'_, '_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    config: &EffectiveDaemonConfig,
    producer: &RawCodeArtifactProducerInput<'_>,
) -> Result<
    crate::preparation_composition::native_artifact_store::NativeStoredArtifactSet,
    crate::preparation_composition::native_artifact_store::NativeArtifactCompositionError,
> {
    let mut adapter = NativeArtifactStoreAdapter::open(input, journal, publisher, config, producer)
        .map_err(crate::preparation_composition::native_artifact_store::NativeArtifactCompositionError::Native)?;
    crate::preparation_composition::native_artifact_store::prepare_and_store(&mut adapter, producer)
}
