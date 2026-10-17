//! Reopen C07 projection metadata before Qdrant and full owner artifacts only after nomination.

#![allow(dead_code)] // The parent provider facade integrates these bounded read phases at cutover.

use std::path::Path;

use search_contracts::{
    self as c, ProjectionMembershipId, SourceIdentity, SourceOwnerGeneration, SourceRevisionRef,
};
use search_control_redb::{
    MigrationBootstrapWork as Work, PreparedArtifactSetRecordV1, PublicationProjectionSnapshot,
};
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiWork};
use search_revision_crypto::SelectedPlaintextV2;
use search_revision_store::{
    DomainBinding, decode_residency_canonical,
    derived_artifact::{
        DerivedArtifactDescriptorV1 as Descriptor, DerivedArtifactErrorV1,
        DerivedArtifactIoOperationV1, DerivedArtifactKindV1 as ArtifactKind,
        DerivedArtifactObjectRefV1 as ObjectRef, DerivedArtifactPlaintextV1,
        DerivedArtifactReadReceiptV1, DerivedArtifactSealBindingV1, DerivedArtifactSetDescriptorV1,
        DerivedArtifactStorageV1, DerivedArtifactVerificationPortV1, MAX_DERIVED_AAD_BYTES,
        MAX_DERIVED_SET_BYTES,
    },
    residency_canonical_bytes,
};
use search_source_history::revision::{SourceRevisionControlPort, SourceRevisionSourceState};
use search_source_registry::{
    api::{ProjectionMembershipRecord, read_projection_membership},
    membership::MembershipLifecycle,
    recovery::SourceRegistry,
    root::RootStatus,
    source::SourceLifecycle,
};

use crate::{
    config_composition::EffectiveDaemonConfig,
    preparation_composition::artifact_payloads::{
        PreparedRawCodeArtifacts, RawArtifactPreparationError, RawCodeArtifactProducerInput,
        prepare_raw_code_artifacts_from_native,
    },
};

use super::super::read_child::fresh_root::{FreshRootCancellation, FreshRootContext};
use super::super::read_child::{NativeDerivedArtifactFailure, OriginalObjectContext};
use super::{
    artifact_store::verify_derived_artifact_scope,
    dek_command::NativeContext,
    retention_control::{DerivedArtifactReadback, NativeRetentionControl},
};

const MAX_OBJECT_BYTES: usize = MAX_DERIVED_SET_BYTES + MAX_DERIVED_AAD_BYTES + 64;

#[derive(Debug)]
pub(super) enum NativeArtifactReadVerificationError {
    Refused,
}

pub(super) enum NativeArtifactReaderError {
    CurrentSetUnavailable,
    CurrentSetChanged,
    OwnerRefused,
    ControlRefused,
    Reference(DerivedArtifactErrorV1),
    Preparation(RawArtifactPreparationError),
    C07(NativeDerivedArtifactFailure<NativeArtifactReadVerificationError>),
}

impl NativeArtifactReaderError {
    pub(super) const fn code(&self) -> &'static str {
        match self {
            Self::CurrentSetUnavailable => "NATIVE_PREPARED_ARTIFACT_SET_UNAVAILABLE",
            Self::CurrentSetChanged => "NATIVE_PREPARED_ARTIFACT_SET_CHANGED",
            Self::OwnerRefused => "NATIVE_PREPARED_ARTIFACT_OWNER_REFUSED",
            Self::ControlRefused => "NATIVE_PREPARED_ARTIFACT_CONTROL_REFUSED",
            Self::Reference(_) => "NATIVE_PREPARED_ARTIFACT_REFERENCE_INVALID",
            Self::Preparation(_) => "NATIVE_PREPARED_ARTIFACT_RECONSTRUCTION_REFUSED",
            Self::C07(error) if error.outcome_unknown() => {
                "NATIVE_DERIVED_ARTIFACT_READ_OUTCOME_UNKNOWN"
            }
            Self::C07(_) => "NATIVE_DERIVED_ARTIFACT_READ_REFUSED",
        }
    }

    pub(super) const fn outcome_unknown(&self) -> bool {
        match self {
            Self::C07(error) => error.outcome_unknown(),
            Self::CurrentSetUnavailable
            | Self::CurrentSetChanged
            | Self::OwnerRefused
            | Self::ControlRefused
            | Self::Reference(_)
            | Self::Preparation(_) => false,
        }
    }
}

/// Full C08/C09/C13 reconstruction for a nominated source candidate or recovery check.
/// It is not the ordinary pre-Qdrant C18 metadata input. The C02 record and C03
/// membership are borrowed snapshots; this data is not reusable permission.
pub(super) struct NativeReopenedArtifactSet<'snapshot, 'registry> {
    snapshot: &'snapshot PublicationProjectionSnapshot,
    membership: ProjectionMembershipId,
    record: &'snapshot PreparedArtifactSetRecordV1,
    generation: u64,
    projection_membership: &'registry ProjectionMembershipRecord,
    source_identity: SourceIdentity,
    source_owner_generation: SourceOwnerGeneration,
    source_registry_generation: u64,
    source_revision: SourceRevisionRef,
    read: DerivedArtifactReadReceiptV1<Vec<u8>>,
    prepared: PreparedRawCodeArtifacts,
}

impl NativeReopenedArtifactSet<'_, '_> {
    pub(super) const fn snapshot(&self) -> &PublicationProjectionSnapshot {
        self.snapshot
    }

    pub(super) const fn membership(&self) -> ProjectionMembershipId {
        self.membership
    }

    pub(super) const fn record(&self) -> &PreparedArtifactSetRecordV1 {
        self.record
    }

    pub(super) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) const fn projection_membership(&self) -> &ProjectionMembershipRecord {
        self.projection_membership
    }

    pub(super) const fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }

    pub(super) const fn source_owner_generation(&self) -> SourceOwnerGeneration {
        self.source_owner_generation
    }

    pub(super) const fn source_registry_generation(&self) -> u64 {
        self.source_registry_generation
    }

    pub(super) const fn source_revision(&self) -> SourceRevisionRef {
        self.source_revision
    }

    /// Exact C07 stored-byte read observation; it contains no opened plaintext.
    pub(super) const fn c07_observation(&self) -> &DerivedArtifactReadReceiptV1<Vec<u8>> {
        &self.read
    }

    /// Owner outputs reconstructed and verified against all four stored members.
    pub(super) const fn prepared(&self) -> &PreparedRawCodeArtifacts {
        &self.prepared
    }
}

/// Current C02/C03 projection metadata plus the authenticated C07 graph header.
/// It retains the typed root reference and graph, not the encrypted root blob or
/// any member body. No member availability or preparation-validity is claimed.
///
/// It is read data, not reusable permission; the original request's live checkpoint
/// and current C18 authority remain required when consuming it.
#[allow(clippy::redundant_pub_crate)] // Expose only through the crate-private owner facade.
pub(crate) struct NativePublishedProjectionRead<'snapshot, 'registry> {
    snapshot: &'snapshot PublicationProjectionSnapshot,
    membership: ProjectionMembershipId,
    record: &'snapshot PreparedArtifactSetRecordV1,
    generation: u64,
    projection_membership: &'registry ProjectionMembershipRecord,
    source_identity: SourceIdentity,
    source_owner_generation: SourceOwnerGeneration,
    source_registry_generation: u64,
    source_revision: SourceRevisionRef,
    set_reference: ObjectRef,
    set_graph: DerivedArtifactSetDescriptorV1,
}

#[allow(clippy::redundant_pub_crate)] // Keep every metadata accessor inside the crate.
impl NativePublishedProjectionRead<'_, '_> {
    pub(crate) const fn snapshot(&self) -> &PublicationProjectionSnapshot {
        self.snapshot
    }

    pub(crate) const fn membership(&self) -> ProjectionMembershipId {
        self.membership
    }

    pub(crate) const fn record(&self) -> &PreparedArtifactSetRecordV1 {
        self.record
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) const fn projection_membership(&self) -> &ProjectionMembershipRecord {
        self.projection_membership
    }

    pub(crate) const fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }

    pub(crate) const fn source_owner_generation(&self) -> SourceOwnerGeneration {
        self.source_owner_generation
    }

    pub(crate) const fn source_registry_generation(&self) -> u64 {
        self.source_registry_generation
    }

    pub(crate) const fn source_revision(&self) -> SourceRevisionRef {
        self.source_revision
    }

    /// Actual C02-selected C07 `SetManifest` reference.
    pub(crate) const fn set_reference(&self) -> &ObjectRef {
        &self.set_reference
    }

    /// Authenticated C07 graph header. Member bodies were not opened.
    pub(crate) const fn set_graph(&self) -> &DerivedArtifactSetDescriptorV1 {
        &self.set_graph
    }

    pub(crate) const fn projection_manifest_reference(&self) -> &ObjectRef {
        &self.set_graph.members()[3]
    }

    pub(crate) const fn projection_manifest_descriptor(&self) -> &Descriptor {
        self.projection_manifest_reference().descriptor()
    }
}

/// Metadata-only C18 pre-Qdrant read. It opens only the C07 set header and never
/// opens source, representation, materialization, unit-manifest, or `ProjectionManifest`
/// bodies. Its typed child descriptor/profile comes from the authenticated graph header.
#[allow(clippy::too_many_arguments)] // Each live owner input remains explicit.
#[allow(clippy::too_many_lines)] // Keep current C02/C03/C07 checks and final readback ordered.
pub(super) fn read_published_projection_metadata_v1<'snapshot, 'registry>(
    native: &FreshRootContext<'_, '_>,
    control: &NativeRetentionControl<'_, '_, FreshRootCancellation>,
    snapshot: &'snapshot PublicationProjectionSnapshot,
    registry: &'registry SourceRegistry,
    membership: ProjectionMembershipId,
    expected_source: &SourceRevisionRef,
    retained_metadata: &search_revision_store::PersistedSelectedObjectMetadata,
    config: &EffectiveDaemonConfig,
) -> Result<NativePublishedProjectionRead<'snapshot, 'registry>, NativeArtifactReaderError> {
    native
        .source_full_checkpoint()
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    control
        .verify_current_source()
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    require_same_projection_snapshot(control, snapshot)?;

    let pointer = snapshot
        .manifests()
        .get(&membership)
        .ok_or(NativeArtifactReaderError::CurrentSetUnavailable)?;
    let record = snapshot
        .prepared_artifact_sets()
        .get(&membership)
        .ok_or(NativeArtifactReaderError::CurrentSetUnavailable)?;
    if record.reference() != pointer {
        return Err(NativeArtifactReaderError::CurrentSetUnavailable);
    }
    let (
        projection_membership,
        source_identity,
        source_owner_generation,
        source_registry_generation,
    ) = verify_current_projection_membership(registry, membership, expected_source, None)?;
    let metadata = record.metadata();
    let current_c03_binding = projection_membership.binding();
    if &metadata.source_revision != expected_source
        || metadata.source_revision.source_namespace_id != source_identity.source_namespace_id
        || metadata.source_revision.source_id != source_identity.source_id
        || metadata.representation_id != current_c03_binding.representation_id
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    let set_reference = ObjectRef::decode(&metadata.object_reference_canonical)
        .map_err(NativeArtifactReaderError::Reference)?;
    if set_reference
        .encode()
        .map_err(NativeArtifactReaderError::Reference)?
        != metadata.object_reference_canonical
        || set_reference.storage() != DerivedArtifactStorageV1::Aes256GcmEnvelope
        || set_reference.stored_object_digest() != metadata.stored_object_digest
        || set_reference.stored_bytes() != metadata.stored_object_length
        || set_reference.descriptor().kind()
            != search_revision_store::derived_artifact::DerivedArtifactKindV1::SetManifest
        || set_reference.descriptor().source() != expected_source
        || set_reference.descriptor().operation_id() != &metadata.operation_id
        || set_reference.descriptor().materialization_id() != metadata.materialization_id
        || set_reference.descriptor().representation_id() != metadata.representation_id
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    let domains: [DomainBinding; 6] = decode_residency_canonical(&metadata.residency_canonical)
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    if residency_canonical_bytes(&domains).map_err(|_| NativeArtifactReaderError::OwnerRefused)?
        != metadata.residency_canonical
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    verify_derived_artifact_scope(
        set_reference.descriptor(),
        expected_source,
        &metadata.operation_id,
        &domains,
        metadata.materialization_id,
        metadata.representation_id,
        &metadata.residency_canonical,
        &|unit| native.work(unit),
    )
    .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;

    let source_record = control
        .load_source_revision(
            &expected_source.source_namespace_id,
            &expected_source.source_id,
            &expected_source.revision_id,
            native.context(),
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?
        .ok_or(NativeArtifactReaderError::ControlRefused)?;
    let head = control
        .load_source_revision_head(
            &expected_source.source_namespace_id,
            &expected_source.source_id,
            native.context(),
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?
        .ok_or(NativeArtifactReaderError::ControlRefused)?;
    if source_ref(&source_record, expected_source.source_namespace_id) != *expected_source
        || head.state != SourceRevisionSourceState::Active
        || head.revision.as_ref() != Some(&source_record)
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }

    // Read C02's retained-object metadata only; no selected source object is opened.
    let (source_metadata, source_seal) = control
        .load_retained_revision(expected_source, native.context())
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    if &source_metadata != retained_metadata {
        return Err(NativeArtifactReaderError::CurrentSetChanged);
    }
    verify_source_metadata(
        &source_metadata,
        expected_source,
        &domains,
        &metadata.residency_canonical,
    )?;
    let initial_set_evidence = control
        .read_derived_artifact_evidence(
            &set_reference,
            &source_record,
            &metadata.residency_canonical,
            source_metadata.encryption(),
            config,
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;

    let mut verifier = NativeArtifactReadVerifier {
        native,
        control,
        source: *expected_source,
        source_record: source_record.clone(),
        operation: metadata.operation_id.clone(),
        materialization_id: metadata.materialization_id,
        representation_id: metadata.representation_id,
        domains,
        residency: metadata.residency_canonical.clone(),
        encryption: source_metadata.encryption().clone(),
        config,
        prepared: None,
        evidence: Vec::with_capacity(2),
    };
    let set_header = native
        .read_derived_artifact_set_header_v1(&mut verifier, &set_reference, MAX_OBJECT_BYTES)
        .map_err(NativeArtifactReaderError::C07)?;
    if set_header.read_observation().object_ref() != &set_reference {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    for member in set_header.set().members() {
        let descriptor = member.descriptor();
        if member.storage() != DerivedArtifactStorageV1::Aes256GcmEnvelope
            || descriptor.source() != expected_source
            || descriptor.operation_id() != &metadata.operation_id
            || descriptor.materialization_id() != metadata.materialization_id
            || descriptor.representation_id() != metadata.representation_id
            || descriptor.domains() != &verifier.domains
        {
            return Err(NativeArtifactReaderError::OwnerRefused);
        }
    }
    let projection_reference = &set_header.set().members()[3];
    if projection_reference.descriptor().kind() != ArtifactKind::ProjectionManifest {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    let set_graph = set_header.set().clone();
    drop(verifier);

    native
        .source_full_checkpoint()
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    control
        .verify_current_source()
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    require_same_projection_snapshot(control, snapshot)?;
    let (source_metadata_after, source_seal_after) = control
        .load_retained_revision(expected_source, native.context())
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    let source_record_after = control
        .load_source_revision(
            &expected_source.source_namespace_id,
            &expected_source.source_id,
            &expected_source.revision_id,
            native.context(),
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?
        .ok_or(NativeArtifactReaderError::ControlRefused)?;
    let head_after = control
        .load_source_revision_head(
            &expected_source.source_namespace_id,
            &expected_source.source_id,
            native.context(),
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?
        .ok_or(NativeArtifactReaderError::ControlRefused)?;
    let final_set_evidence = control
        .read_derived_artifact_evidence(
            &set_reference,
            &source_record_after,
            &metadata.residency_canonical,
            source_metadata_after.encryption(),
            config,
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    if source_metadata_after != source_metadata
        || source_seal_after != source_seal
        || source_record_after != source_record
        || head_after.state != SourceRevisionSourceState::Active
        || head_after.revision.as_ref() != Some(&source_record_after)
        || !same_derived_artifact_readback(&initial_set_evidence, &final_set_evidence)
    {
        return Err(NativeArtifactReaderError::CurrentSetChanged);
    }

    Ok(NativePublishedProjectionRead {
        snapshot,
        membership,
        record,
        generation: snapshot.generation(),
        projection_membership,
        source_identity,
        source_owner_generation,
        source_registry_generation,
        source_revision: *expected_source,
        set_reference,
        set_graph,
    })
}

struct StoredReadEvidence {
    reference: ObjectRef,
    readback: DerivedArtifactReadback,
}

/// Fully reopens a nominated source's committed C07 set for candidate/recovery validation.
///
/// The producer is supplied by the caller's qualified owner-profile selection. This
/// function derives source and materialization identities from C02/C07 bytes, reads
/// the exact retained source object under this Native root, and reconstructs C08/C09/C13
/// through their real producers. It creates no IDs, nonce reservations, journals or
/// query state, and returns no readiness/visibility claim. Ordinary C18 admission must
/// use `read_published_projection_metadata_v1` before Qdrant instead of this full path.
#[allow(clippy::too_many_arguments)] // Preserve the distinct original-owner inputs.
#[allow(clippy::too_many_lines)] // Keep the original-context, before/read/after proof ordered in one path.
pub(super) fn reopen_prepared_artifact_set_v1<'snapshot, 'registry>(
    native: &FreshRootContext<'_, '_>,
    control: &NativeRetentionControl<'_, '_, FreshRootCancellation>,
    snapshot: &'snapshot PublicationProjectionSnapshot,
    registry: &'registry SourceRegistry,
    membership: ProjectionMembershipId,
    expected_source: &SourceRevisionRef,
    retained_source: &SelectedPlaintextV2,
    retained_metadata: &search_revision_store::PersistedSelectedObjectMetadata,
    producer: &RawCodeArtifactProducerInput<'_>,
    config: &EffectiveDaemonConfig,
) -> Result<NativeReopenedArtifactSet<'snapshot, 'registry>, NativeArtifactReaderError> {
    native
        .source_full_checkpoint()
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    control
        .verify_current_source()
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    require_same_projection_snapshot(control, snapshot)?;

    let pointer = snapshot
        .manifests()
        .get(&membership)
        .ok_or(NativeArtifactReaderError::CurrentSetUnavailable)?;
    let record = snapshot
        .prepared_artifact_sets()
        .get(&membership)
        .ok_or(NativeArtifactReaderError::CurrentSetUnavailable)?;
    if record.reference() != pointer {
        return Err(NativeArtifactReaderError::CurrentSetUnavailable);
    }
    if producer.scope.membership.projection_membership_id != membership {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    let (
        projection_membership,
        source_identity,
        source_owner_generation,
        source_registry_generation,
    ) = verify_current_projection_membership(
        registry,
        membership,
        expected_source,
        Some(producer.scope),
    )?;
    let metadata = record.metadata();
    if &metadata.source_revision != expected_source {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    let reference = ObjectRef::decode(&metadata.object_reference_canonical)
        .map_err(NativeArtifactReaderError::Reference)?;
    let canonical_reference = reference
        .encode()
        .map_err(NativeArtifactReaderError::Reference)?;
    let descriptor = reference.descriptor();
    if canonical_reference != metadata.object_reference_canonical
        || reference.storage() != DerivedArtifactStorageV1::Aes256GcmEnvelope
        || reference.stored_object_digest() != metadata.stored_object_digest
        || reference.stored_bytes() != metadata.stored_object_length
        || descriptor.kind()
            != search_revision_store::derived_artifact::DerivedArtifactKindV1::SetManifest
        || descriptor.source() != expected_source
        || descriptor.operation_id() != &metadata.operation_id
        || descriptor.materialization_id() != metadata.materialization_id
        || descriptor.representation_id() != metadata.representation_id
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    let domains: [DomainBinding; 6] = decode_residency_canonical(&metadata.residency_canonical)
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    if residency_canonical_bytes(&domains).map_err(|_| NativeArtifactReaderError::OwnerRefused)?
        != metadata.residency_canonical
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    verify_derived_artifact_scope(
        descriptor,
        expected_source,
        &metadata.operation_id,
        &domains,
        metadata.materialization_id,
        metadata.representation_id,
        &metadata.residency_canonical,
        &|unit| native.work(unit),
    )
    .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;

    let source_record = control
        .load_source_revision(
            &expected_source.source_namespace_id,
            &expected_source.source_id,
            &expected_source.revision_id,
            native.context(),
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?
        .ok_or(NativeArtifactReaderError::ControlRefused)?;
    let head = control
        .load_source_revision_head(
            &expected_source.source_namespace_id,
            &expected_source.source_id,
            native.context(),
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?
        .ok_or(NativeArtifactReaderError::ControlRefused)?;
    if source_ref(&source_record, expected_source.source_namespace_id) != *expected_source
        || head.state != SourceRevisionSourceState::Active
        || head.revision.as_ref() != Some(&source_record)
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }

    // C02 verifies the active source link/cause before the Native object is opened.
    let (source_metadata, source_seal) = control
        .load_retained_revision(expected_source, native.context())
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    if &source_metadata != retained_metadata
        || u64::try_from(retained_source.as_bytes().len()).ok() != Some(expected_source.byte_length)
    {
        return Err(NativeArtifactReaderError::CurrentSetChanged);
    }
    verify_source_metadata(
        &source_metadata,
        expected_source,
        &domains,
        &metadata.residency_canonical,
    )?;
    let initial_set_evidence = control
        .read_derived_artifact_evidence(
            &reference,
            &source_record,
            &metadata.residency_canonical,
            source_metadata.encryption(),
            config,
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;

    validate_producer_input(
        native,
        producer,
        expected_source,
        source_record.occurrence_sequence,
        &source_metadata,
        &metadata.operation_id,
        metadata.materialization_id,
        metadata.representation_id,
    )?;
    let root_facts = native
        .revision_dek_root_facts()
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    if producer.schema.validate().is_err()
        || producer.schema.installation_incarnation_id() != &root_facts.incarnation
        || producer.scope.membership.installation_incarnation_id != root_facts.incarnation
        || producer.schema.collection_generation_id()
            != &producer.scope.membership.collection_generation_id
        || producer.schema.projection_profile_set_id()
            != &producer.scope.membership.projection_profile_set_id
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }

    // The caller supplies the source bytes already opened under its retained-read
    // budget and this same FreshRootContext/NativeRetentionControl operation.
    let work = |unit| native.work(unit);
    let prepared = prepare_raw_code_artifacts_from_native(
        producer,
        expected_source,
        source_record.occurrence_sequence,
        retained_source.as_bytes(),
        &domains,
        &metadata.operation_id,
        &work,
    )
    .map_err(NativeArtifactReaderError::Preparation)?;
    if prepared.scope() != producer.scope || prepared.schema() != producer.schema {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }

    let mut verifier = NativeArtifactReadVerifier {
        native,
        control,
        source: *expected_source,
        source_record: source_record.clone(),
        operation: metadata.operation_id.clone(),
        materialization_id: metadata.materialization_id,
        representation_id: metadata.representation_id,
        domains,
        residency: metadata.residency_canonical.clone(),
        encryption: source_metadata.encryption().clone(),
        config,
        prepared: Some(&prepared),
        evidence: Vec::with_capacity(5),
    };
    let read = native
        .read_derived_artifact_v1(&mut verifier, &reference, MAX_OBJECT_BYTES)
        .map_err(NativeArtifactReaderError::C07)?;
    if read.object_ref() != &reference {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    drop(verifier);

    native
        .source_full_checkpoint()
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    control
        .verify_current_source()
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    require_same_projection_snapshot(control, snapshot)?;
    let (source_metadata_after, source_seal_after) = control
        .load_retained_revision(expected_source, native.context())
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    let source_record_after = control
        .load_source_revision(
            &expected_source.source_namespace_id,
            &expected_source.source_id,
            &expected_source.revision_id,
            native.context(),
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?
        .ok_or(NativeArtifactReaderError::ControlRefused)?;
    let head_after = control
        .load_source_revision_head(
            &expected_source.source_namespace_id,
            &expected_source.source_id,
            native.context(),
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?
        .ok_or(NativeArtifactReaderError::ControlRefused)?;
    let final_set_evidence = control
        .read_derived_artifact_evidence(
            &reference,
            &source_record_after,
            &metadata.residency_canonical,
            source_metadata_after.encryption(),
            config,
        )
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    if source_metadata_after != source_metadata
        || source_seal_after != source_seal
        || source_record_after != source_record
        || head_after.state != SourceRevisionSourceState::Active
        || head_after.revision.as_ref() != Some(&source_record_after)
        || final_set_evidence.reservation != initial_set_evidence.reservation
        || final_set_evidence.intent != initial_set_evidence.intent
        || final_set_evidence.selected.current != initial_set_evidence.selected.current
        || final_set_evidence.selected.encryption != initial_set_evidence.selected.encryption
    {
        return Err(NativeArtifactReaderError::CurrentSetChanged);
    }

    Ok(NativeReopenedArtifactSet {
        snapshot,
        membership,
        record,
        generation: snapshot.generation(),
        projection_membership,
        source_identity,
        source_owner_generation,
        source_registry_generation,
        source_revision: *expected_source,
        read,
        prepared,
    })
}

/// Resolve the C03 row from the provider's current registry snapshot. Callers
/// supply the registry and stable membership selector, never a canonical row.
fn verify_current_projection_membership<'registry>(
    registry: &'registry SourceRegistry,
    expected_projection_membership_id: ProjectionMembershipId,
    expected_source: &SourceRevisionRef,
    scope: Option<&search_projection_planner::PlanningScope>,
) -> Result<
    (
        &'registry ProjectionMembershipRecord,
        SourceIdentity,
        SourceOwnerGeneration,
        u64,
    ),
    NativeArtifactReaderError,
> {
    let registry_generation = registry.revision();
    let source_membership_id = registry
        .projection_source_membership(expected_projection_membership_id)
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    let record = read_projection_membership(registry, source_membership_id, registry_generation)
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    let binding = record.binding();
    if binding.projection_membership_id != expected_projection_membership_id {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    if let Some(scope) = scope {
        let selected = &scope.membership;
        if binding.projection_membership_id != selected.projection_membership_id
            || binding.source_membership_id != selected.source_membership_id
            || binding.representation_id != selected.representation_id
            || binding.access_partition_id != selected.access_partition_id
            || binding.scoring_partition_id != selected.scoring_partition_id
            || binding.projection_schema_id != selected.projection_schema_id
        {
            return Err(NativeArtifactReaderError::OwnerRefused);
        }
    }

    let (member, identity) = registry
        .active_source_membership_v2(binding.source_membership_id)
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    let key = registry
        .membership_key_by_id(binding.source_membership_id)
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    let member_record = registry
        .membership(key)
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    let permanent = member_record
        .permanent_v2()
        .ok_or(NativeArtifactReaderError::OwnerRefused)?;
    let provenance = &record.request().expected_provenance;
    let source = registry
        .sources()
        .get(identity)
        .ok_or(NativeArtifactReaderError::OwnerRefused)?;
    let root = registry
        .root(source.binding().root_binding_id())
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    let namespace = registry
        .namespaces()
        .get(&identity.source_namespace_id)
        .ok_or(NativeArtifactReaderError::OwnerRefused)?;
    let owner_generation = namespace.ownership.source_owner_generation;
    if identity.source_namespace_id != expected_source.source_namespace_id
        || identity.source_id != expected_source.source_id
        || member.source_id != expected_source.source_id
        || member.source_membership_id != binding.source_membership_id
        || member_record.lifecycle() != MembershipLifecycle::Active
        || source.identity() != identity
        || source.lifecycle() != SourceLifecycle::Active
        || root.status != RootStatus::Active
        || source.binding().root_binding_id() != root.root_binding_id
        || namespace.ownership.source_namespace_id != expected_source.source_namespace_id
        || namespace.ownership.status != c::NamespaceOwnershipStatus::Active
        || namespace.fenced
        || namespace.preparation.is_some()
        || namespace.activation.is_some()
        || permanent.provenance() != provenance
        || provenance.source_identity() != identity
        || provenance.root_binding_id() != root.root_binding_id
        || provenance.root_record_revision() != root.record_revision
        || provenance.source_namespace_id() != identity.source_namespace_id
        || provenance.namespace_record_revision() != namespace.ownership.ownership_record_revision
        || provenance.owner_generation() != owner_generation
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    Ok((
        record,
        identity.clone(),
        owner_generation,
        registry_generation,
    ))
}

fn require_same_projection_snapshot(
    control: &NativeRetentionControl<'_, '_, FreshRootCancellation>,
    expected: &PublicationProjectionSnapshot,
) -> Result<(), NativeArtifactReaderError> {
    let (identity, generation) = control
        .projection_metadata_checkpoint()
        .map_err(|_| NativeArtifactReaderError::ControlRefused)?;
    // The expected view came from a complete verified read of this exact journal.
    // Its monotone generation plus the source guard set detects any intervening
    // transaction without re-reading every other source's projection metadata.
    if identity != expected.identity() || generation != expected.generation() {
        return Err(NativeArtifactReaderError::CurrentSetChanged);
    }
    Ok(())
}

const fn source_ref(
    record: &c::SourceRevision,
    namespace: c::SourceNamespaceId,
) -> SourceRevisionRef {
    SourceRevisionRef {
        source_namespace_id: namespace,
        source_id: record.source_id,
        revision_id: record.revision_id,
        content_digest: record.content_digest,
        byte_length: record.byte_length,
    }
}

fn verify_source_metadata(
    metadata: &search_revision_store::PersistedSelectedObjectMetadata,
    expected: &SourceRevisionRef,
    domains: &[DomainBinding; 6],
    expected_residency: &[u8],
) -> Result<(), NativeArtifactReaderError> {
    let residency = residency_canonical_bytes(metadata.domain_bindings())
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    if metadata.source().namespace_id != expected.source_namespace_id
        || metadata.source().source_id != expected.source_id
        || metadata.source().revision_id != expected.revision_id
        || metadata.content_digest() != expected.content_digest
        || metadata.content_bytes() != expected.byte_length
        || metadata.domain_bindings() != domains
        || residency != expected_residency
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    Ok(())
}

fn same_derived_artifact_readback(
    first: &DerivedArtifactReadback,
    second: &DerivedArtifactReadback,
) -> bool {
    first.reservation == second.reservation
        && first.intent == second.intent
        && first.selected.current == second.selected.current
        && first.selected.preparation == second.selected.preparation
        && first.selected.claim == second.selected.claim
        && first.selected.encryption == second.selected.encryption
}

#[allow(clippy::too_many_arguments)] // Each identity is independently sourced and checked.
fn validate_producer_input(
    native: &FreshRootContext<'_, '_>,
    producer: &RawCodeArtifactProducerInput<'_>,
    source: &SourceRevisionRef,
    occurrence_sequence: u64,
    source_metadata: &search_revision_store::PersistedSelectedObjectMetadata,
    operation: &c::OpaqueId,
    materialization_id: c::MaterializationId,
    representation_id: c::RepresentationId,
) -> Result<(), NativeArtifactReaderError> {
    if !native.matches_original_work(
        producer.unitization_budget.deadline,
        producer.unitization_budget.cancelled,
    ) {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    native
        .work(Work::Allocate(512))
        .and_then(|()| native.work(Work::Process(512)))
        .map_err(|_| NativeArtifactReaderError::OwnerRefused)?;
    let source_id = source.source_id.to_string();
    if producer.request.source_id().as_str() != source_id
        || producer.request.revision().get() != occurrence_sequence
        || producer.request.content_digest() != source.content_digest
        || producer.request.byte_count() != source.byte_length
        || producer.request.operation_id() != operation
        || producer.request.residency() != source_metadata.storage_object_id()
        || producer.binding.source_namespace_id != source.source_namespace_id
        || producer.binding.source_id != source.source_id
        || producer.binding.source_revision_id != source.revision_id
        || producer.binding.materialization_id != materialization_id
        || producer.binding.representation_id != representation_id
        || producer.scope.membership.source_id != source.source_id
        || producer.scope.membership.source_revision_id != source.revision_id
        || producer.scope.membership.representation_id != representation_id
    {
        return Err(NativeArtifactReaderError::OwnerRefused);
    }
    Ok(())
}

struct NativeArtifactReadVerifier<'a, 'operation, 'owner, 'journal, 'work> {
    native: &'a FreshRootContext<'operation, 'owner>,
    control: &'a NativeRetentionControl<'journal, 'work, FreshRootCancellation>,
    source: SourceRevisionRef,
    source_record: c::SourceRevision,
    operation: c::OpaqueId,
    materialization_id: c::MaterializationId,
    representation_id: c::RepresentationId,
    domains: [DomainBinding; 6],
    residency: Vec<u8>,
    encryption: search_revision_store::EncryptionBinding,
    config: &'a EffectiveDaemonConfig,
    prepared: Option<&'a PreparedRawCodeArtifacts>,
    evidence: Vec<StoredReadEvidence>,
}

impl<'operation, 'owner, 'context>
    DerivedArtifactVerificationPortV1<OriginalObjectContext<'context, 'operation, 'owner>>
    for NativeArtifactReadVerifier<'_, 'operation, 'owner, '_, '_>
{
    type Error = NativeArtifactReadVerificationError;

    fn verify_storage_scope(
        &mut self,
        context: &mut OriginalObjectContext<'context, 'operation, 'owner>,
        cas_root: &Path,
        reference: &ObjectRef,
        operation: DerivedArtifactIoOperationV1,
    ) -> Result<(), Self::Error> {
        if operation != DerivedArtifactIoOperationV1::Read
            || !std::ptr::eq(context.fresh_root(), self.native)
            || reference.storage() != DerivedArtifactStorageV1::Aes256GcmEnvelope
        {
            return Err(NativeArtifactReadVerificationError::Refused);
        }
        context
            .verify_cas_root(cas_root)
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        verify_derived_artifact_scope(
            reference.descriptor(),
            &self.source,
            &self.operation,
            &self.domains,
            self.materialization_id,
            self.representation_id,
            &self.residency,
            &|unit| self.native.work(unit),
        )
        .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        self.native
            .source_full_checkpoint()
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        self.control
            .verify_current_source()
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        let readback = self
            .control
            .read_derived_artifact_evidence(
                reference,
                &self.source_record,
                &self.residency,
                &self.encryption,
                self.config,
            )
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        self.cache_readback(reference, readback)
    }

    fn verify_owner_payload(
        &mut self,
        context: &mut OriginalObjectContext<'context, 'operation, 'owner>,
        descriptor: &Descriptor,
        encoded_body: &[u8],
    ) -> Result<(), Self::Error> {
        let native = context.fresh_root();
        native
            .work(Work::Allocate(encoded_body.len().saturating_mul(2)))
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        native
            .work(Work::Process(encoded_body.len()))
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        self.prepared
            .ok_or(NativeArtifactReadVerificationError::Refused)?
            .verify_owner_payload(descriptor, encoded_body)
            .map_err(|_| NativeArtifactReadVerificationError::Refused)
    }

    fn open_authenticated(
        &mut self,
        context: &mut OriginalObjectContext<'context, 'operation, 'owner>,
        binding: &DerivedArtifactSealBindingV1,
        ciphertext_and_tag: &[u8],
    ) -> Result<DerivedArtifactPlaintextV1, Self::Error> {
        let evidence = self
            .evidence
            .iter()
            .find(|item| item.reference.descriptor() == binding.descriptor())
            .ok_or(NativeArtifactReadVerificationError::Refused)?;
        let reference = &evidence.reference;
        let actual = self
            .control
            .read_derived_artifact_evidence(
                reference,
                &self.source_record,
                &self.residency,
                &self.encryption,
                self.config,
            )
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        if actual.reservation != evidence.readback.reservation
            || actual.intent != evidence.readback.intent
            || binding.selected_current_canonical() != actual.selected.current
            || binding.aad_bytes() != actual.reservation.aad_canonical()
            || binding.counter() != actual.reservation.counter()
            || binding.encryption() != &actual.selected.encryption
            || binding
                .authenticated_blocks()
                .map_err(|_| NativeArtifactReadVerificationError::Refused)?
                != actual.reservation.authenticated_blocks()
        {
            return Err(NativeArtifactReadVerificationError::Refused);
        }
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
            deadline: self.control.deadline(),
            poll: &poll,
            work: &native_work,
            open: &open,
            clear: &clear,
        };
        let plaintext = os::open_derived_artifact(
            &actual.reservation,
            binding,
            ciphertext_and_tag,
            self.config.secrets_settings(),
            &mut windows,
        )
        .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        native
            .work(Work::Allocate(plaintext.as_bytes().len().saturating_mul(2)))
            .and_then(|()| native.work(Work::Process(plaintext.as_bytes().len())))
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        Ok(DerivedArtifactPlaintextV1::new(
            plaintext.as_bytes().to_vec(),
        ))
    }
}

impl NativeArtifactReadVerifier<'_, '_, '_, '_, '_> {
    fn cache_readback(
        &mut self,
        reference: &ObjectRef,
        readback: DerivedArtifactReadback,
    ) -> Result<(), NativeArtifactReadVerificationError> {
        self.native
            .work(Work::Allocate(std::mem::size_of::<StoredReadEvidence>()))
            .map_err(|_| NativeArtifactReadVerificationError::Refused)?;
        if let Some(existing) = self
            .evidence
            .iter_mut()
            .find(|item| &item.reference == reference)
        {
            existing.readback = readback;
        } else {
            self.evidence.push(StoredReadEvidence {
                reference: reference.clone(),
                readback,
            });
        }
        Ok(())
    }
}
