//! Actual selected-key retention through the existing owners; no parallel store.

use std::{io::Write, path::Path};
use search_contracts as c;
use search_control_redb::{self as db, ControlRecordClass, ControlSnapshotPublisher, JournalLimits,
    MigrationBootstrapWork as Work, PersistentControlJournal, source_retention::{RevisionSealMetadata,
    RevisionUseCommand, RevisionUseUsageReadback, SealReservationReadback}};
use search_os_secrets::{RevisionDekRecord, RevisionDekSelectionRecord};
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiWork};
use search_revision_crypto::{SelectedRevisionBindingV2, SELECTED_REVISION_MAX_BYTES};
use search_revision_store::{CipherSuite, DomainBinding, EncryptedRevisionPayload, EncryptionBinding,
    ObjectMetadataBinding, PersistedSelectedObjectMetadata, PreSealDescriptor, TypedSourceIdentity,
    DEFAULT_REVISION_STORE_LIMITS, derive_object_residency_key_digest, residency_canonical_bytes};
use search_source_history::{prepare::{SourceRevisionPresealProfile, SourceRevisionPresealRequest,
    SourceRevisionPreparation, encode_source_revision_preseal_profile, prepare_source_revision},
};
use crate::{config_composition::{EffectiveDaemonConfig, RootLocatorHints}, registry_control::SourceIdentityIdsV2};
use super::{DirectRegistryOwner, command::{DirectOutput, published_metadata}, dek_command as shared,
    retention_command::NativeRetentionCommand, retention_control::{self, NativeRetentionControl},
    retention_source::RetentionSourceInput};
use super::super::super::direct_policy_v2::CapturedPolicyV2;

const LIMITS: JournalLimits = JournalLimits::BASELINE;

struct Selected {
    current: Vec<u8>,
    preparation: Vec<u8>,
    claim: Option<Vec<u8>>,
    encryption: EncryptionBinding,
}

fn reserve(input: &RetentionSourceInput<'_, '_, '_>, bytes: usize) -> Result<(), String> {
    for unit in [Work::Checkpoint, Work::Allocate(bytes), Work::Process(bytes)] {
        input.native.work(unit).map_err(|error| error.code().to_owned())?;
    }
    Ok(())
}

fn selected(input: &RetentionSourceInput<'_, '_, '_>, journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher, config: &EffectiveDaemonConfig,
) -> Result<Selected, String> {
    reserve(input, 16 * 32768)?;
    let keys = db::source_retention::revision_use_keys(LIMITS).map_err(|error| error.code().to_owned())?;
    let work = |unit| input.native.work(unit);
    let port = shared::bind(journal, publisher, input.native.context())?;
    let read = port.read_control_keys_with_original_context(&keys, input.deadline, &work)
        .map_err(|error| error.control.code().to_owned())?;
    let get = |index| read.get(&keys[index]).filter(|value| value.class() == ControlRecordClass::State)
        .ok_or("NATIVE_RETENTION_KEY_STATE_MISSING");
    let current = get(3)?.as_bytes().to_vec();
    let RevisionDekSelectionRecord::Current(record) = search_os_secrets::decode_revision_dek_selection_record(&current)
        .map_err(|_| "NATIVE_RETENTION_CURRENT_INVALID")? else { return Err("NATIVE_RETENTION_CURRENT_REQUIRED".into()); };
    let RevisionDekRecord::Intent(intent) = search_os_secrets::decode_revision_dek_record(get(1)?.as_bytes())
        .map_err(|_| "NATIVE_RETENTION_KEY_INTENT_INVALID")? else { return Err("NATIVE_RETENTION_KEY_INTENT_INVALID".into()); };
    if record.intent().candidate().intent() != &intent
        || record.intent().policy().backend() != config.secrets_settings().backend()
        || record.intent().policy().secrets_section_digest() != *search_os_secrets::section_digest(config.secrets_settings()).as_bytes()
    { return Err("NATIVE_RETENTION_KEY_POLICY_MISMATCH".into()); }
    let preparation = intent.binding().protection_entropy();
    let usage = RevisionUseUsageReadback::read(&read, &preparation, LIMITS)
        .map_err(|error| error.code().to_owned())?;
    let encryption = EncryptionBinding {
        key_reference: c::OpaqueId::new(format!("revision-dek:{}",
            super::super::super::codec::hex(&intent.binding().reference_digest())))
            .map_err(|_| "NATIVE_RETENTION_KEY_REFERENCE_INVALID")?,
        key_version: c::NonZeroRevision::new(intent.binding().key_version())
            .map_err(|_| "NATIVE_RETENTION_KEY_REFERENCE_INVALID")?,
        cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
    };
    Ok(Selected { current, preparation, claim: usage.residency_canonical().map(<[u8]>::to_vec), encryption })
}

fn object_metadata(input: &RetentionSourceInput<'_, '_, '_>, profile: &SourceRevisionPresealProfile,
    domains: [DomainBinding; 6], bytes: Vec<u8>, nonce: Vec<u8>, encryption: EncryptionBinding,
) -> Result<(ObjectMetadataBinding, PersistedSelectedObjectMetadata), String> {
    reserve(input, bytes.len().checked_mul(2).and_then(|value| value.checked_add(65536))
        .ok_or("NATIVE_RETENTION_OBJECT_BOUNDS")?)?;
    let payload = EncryptedRevisionPayload::new_selected_v2(profile.content_digest, profile.byte_length,
        nonce, bytes, encryption.clone(), DEFAULT_REVISION_STORE_LIMITS).map_err(|_| "NATIVE_RETENTION_OBJECT_INVALID")?;
    let key = c::SearchObjectResidencyKey {
        scope_domain_id: c::ScopeDomainId::from_bytes(domains[0].uuid()),
        access_domain_id: c::AccessDomainId::from_bytes(domains[1].uuid()),
        confidentiality_domain_id: c::ConfidentialityDomainId::from_bytes(domains[2].uuid()),
        encryption_key_domain_id: c::EncryptionKeyDomainId::from_bytes(domains[3].uuid()),
        retention_domain_id: c::RetentionDomainId::from_bytes(domains[4].uuid()),
        erasure_domain_id: c::ErasureDomainId::from_bytes(domains[5].uuid()),
        versioned_content_digest: c::VersionedContentDigest::from_stored_bytes(c::DigestAlgorithm::Blake3_256,
            *payload.ciphertext_digest().as_bytes()),
    };
    let physical = derive_object_residency_key_digest(&key);
    let object = c::OpaqueId::new(format!("selected-source-v2:{physical}")).map_err(|_| "NATIVE_RETENTION_OBJECT_INVALID")?;
    let source = TypedSourceIdentity::new(profile.source_namespace_id, profile.source_id, profile.revision_id,
        c::NonZeroRevision::new(profile.occurrence_sequence).map_err(|_| "NATIVE_RETENTION_OCCURRENCE_INVALID")?);
    let descriptor = PreSealDescriptor::new(source, domains, profile.content_digest, profile.byte_length,
        profile.operation_id.clone()).map_err(|_| "NATIVE_RETENTION_DESCRIPTOR_INVALID")?;
    let binding = ObjectMetadataBinding::new(descriptor, object, payload, encryption)
        .map_err(|_| "NATIVE_RETENTION_OBJECT_INVALID")?;
    let metadata = PersistedSelectedObjectMetadata::from_binding(&binding)
        .map_err(|_| "NATIVE_RETENTION_OBJECT_METADATA_INVALID")?;
    Ok((binding, metadata))
}

pub(super) fn metadata_from_reservation(reservation: &SealReservationReadback) -> RevisionSealMetadata {
    RevisionSealMetadata { operation_id: reservation.operation_id().clone(),
        preseal_occurrence_canonical: reservation.preseal_occurrence_canonical().to_vec(),
        residency_canonical: reservation.residency_canonical().to_vec(),
        aad_canonical: reservation.aad_canonical().to_vec(), plaintext_length: reservation.plaintext_length(),
        content_digest: reservation.content_digest() }
}

fn output(record: &search_source_history::revision::SourceRevisionMutationReadback,
    historical: bool, object_verified: bool) -> DirectOutput {
    DirectOutput { current: true, line: format!(
        "{{\"event\":\"native_source_retained\",\"namespace\":\"{}\",\"source\":\"{}\",\"revision\":\"{}\",\"occurrence_sequence\":{},\"historical_commit\":{historical},\"retained_revision\":true,\"object_readback_verified\":{object_verified},\"workspace_current\":false,\"source_body_serving\":false,\"ready\":false}}",
        super::super::super::codec::hex(record.source_namespace_id.as_bytes()),
        super::super::super::codec::hex(record.source_id.as_bytes()),
        super::super::super::codec::hex(record.revision.revision_id.as_bytes()), record.revision.occurrence_sequence) }
}

pub(super) fn consume_source(input: RetentionSourceInput<'_, '_, '_>, journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher, config: &EffectiveDaemonConfig,
) -> Result<Option<DirectOutput>, String> {
    input.security.checkpoint().map_err(|error| error.to_string())?;
    // Resolve a completed operation from its immutable original before reading
    // mutable Current/key policy. This is historical metadata, not key use or
    // source serving; current source/namespace/security guards still apply.
    if input.existing.result.is_some() {
        let profile = input.existing.profile.as_ref().ok_or("NATIVE_RETENTION_ORIGINAL_MISSING")?;
        reserve(&input, 16 * 32768)?;
        let replay = prepare_occurrence(&input, journal, publisher,
            profile.residency_closure.as_slice().to_vec(), profile.observed_at.clone())?;
        return match replay {
            SourceRevisionPreparation::Committed(committed) => {
                let work = |unit| input.native.work(unit);
                let control = NativeRetentionControl::bind(journal, publisher, input.source, input.namespace,
                    input.root, input.security, input.deadline, &work).map_err(|error| error.to_string())?;
                control.load_committed_object(&committed.profile, &committed.readback, input.native.context())
                    .map_err(|error| error.to_string())?;
                Ok(Some(output(&committed.readback, true, false)))
            },
            SourceRevisionPreparation::Pending(_) => Err("NATIVE_RETENTION_COMMIT_UNRESOLVED".into()),
        };
    }
    let selection = selected(&input, journal, publisher, config)?;
    if input.body.is_none() && input.existing.profile.is_none() { return Ok(None); }
    let previous = input.existing.profile.as_ref();
    let claim = selection.claim.as_deref().or_else(|| previous.map(|profile| profile.residency_closure.as_slice()));
    let domains = input.native.retention_domains(input.root, input.namespace, input.security,
        &selection.current, claim, input.deadline).map_err(|error| error.to_string())?;
    reserve(&input, 16 * 32768)?;
    let residency = residency_canonical_bytes(&domains).map_err(|_| "NATIVE_RETENTION_RESIDENCY_INVALID")?;
    let observed_at = match previous { Some(profile) => profile.observed_at.clone(),
        None => input.native.capture_retention_observed_at().map_err(|error| error.to_string())? };
    let work = |unit| input.native.work(unit);
    let prepared = prepare_occurrence(&input, journal, publisher, residency.clone(), observed_at)?;
    if let SourceRevisionPreparation::Committed(committed) = prepared {
        return Ok(Some(output(&committed.readback, true, false)));
    }
    let SourceRevisionPreparation::Pending(prepared) = prepared else { unreachable!() };
    let profile = prepared.profile;
    if profile.content_digest != input.source.binding().content_digest()
        || profile.byte_length != input.source.binding().content_bytes()
        || profile.stability_receipt_ref != *input.source.binding().observation_receipt()
        || input.source.lifecycle() != search_source_registry::source::SourceLifecycle::Active
    { return Err("NATIVE_RETENTION_ORIGINAL_SOURCE_CHANGED".into()); }
    let profile_bytes = encode_source_revision_preseal_profile(&profile).map_err(|_| "NATIVE_RETENTION_OCCURRENCE_INVALID")?;
    let poll = || input.native.work(Work::Checkpoint);
    let native_work = |unit| match unit {
        LegacyRevisionDpapiWork::NativeCall => input.native.revision_dek_native_attempt(),
        LegacyRevisionDpapiWork::Allocate(bytes) => input.native.work(Work::Allocate(bytes)),
        LegacyRevisionDpapiWork::Process(bytes) => input.native.work(Work::Process(bytes)),
    };
    let open = || input.native.work(Work::Open);
    let clear = |bytes| input.native.revision_dek_account_native_clear(bytes);
    let mut native_key = shared::NativeContext { deadline: input.deadline, poll: &poll,
        work: &native_work, open: &open, clear: &clear };
    let (binding, metadata, seal_metadata, counter) = if let Some(reservation) = &input.existing.reservation {
        let intent = input.existing.object.as_ref().ok_or("NATIVE_RETENTION_SEAL_USE_UNRESOLVED")?;
        if !intent.binds(reservation) || reservation.selected_current_canonical() != selection.current
            || reservation.preseal_occurrence_canonical() != profile_bytes
            || reservation.residency_canonical() != residency
        { return Err("NATIVE_RETENTION_RESERVATION_CONFLICT".into()); }
        let metadata = PersistedSelectedObjectMetadata::decode(intent.object_metadata_canonical())
            .map_err(|_| "NATIVE_RETENTION_OBJECT_METADATA_INVALID")?;
        let read = input.native.read_selected_object(metadata.object_residency_key_digest(), SELECTED_REVISION_MAX_BYTES)
            .map_err(|error| error.code().to_owned())?;
        let (binding, actual) = object_metadata(&input, &profile, domains, read.into_bytes(),
            metadata.nonce().to_vec(), selection.encryption.clone())?;
        if actual != metadata { return Err("NATIVE_RETENTION_OBJECT_READBACK_MISMATCH".into()); }
        (binding, metadata, metadata_from_reservation(reservation), reservation.counter())
    } else {
        let Some(body) = input.body else { return Ok(None); };
        if body.len() as u64 != profile.byte_length { return Err("NATIVE_RETENTION_SOURCE_LENGTH_CHANGED".into()); }
        if let Some(check) = input.current_source { check()?; }
        let mut port = shared::bind(journal, publisher, input.native.context())?;
        let keys = db::source_retention::revision_use_reservation_keys(input.existing.reservation_id, LIMITS)
            .map_err(|error| error.code().to_owned())?;
        let read = port.read_control_keys_with_original_context(&keys, input.deadline, &work)
            .map_err(|error| error.control.code().to_owned())?;
        reserve(&input, 16 * 32768)?;
        let counter = RevisionUseCommand::next_counter(&read, &selection.preparation, LIMITS)
            .map_err(|error| error.code().to_owned())?;
        let crypto = SelectedRevisionBindingV2::from_owner_records(&selection.current, &profile_bytes,
            &residency, &profile.operation_id, counter, profile.content_digest, profile.byte_length)
            .map_err(|_| "NATIVE_RETENTION_CRYPTO_BINDING_INVALID")?;
        let seal_metadata = RevisionSealMetadata { operation_id: profile.operation_id.clone(),
            preseal_occurrence_canonical: profile_bytes, residency_canonical: residency,
            aad_canonical: crypto.aad_bytes().to_vec(), plaintext_length: profile.byte_length, content_digest: profile.content_digest };
        let (_, digest) = retention_control::phase_id(&profile.operation_id, "seal", &work).map_err(|error| error.to_string())?;
        let command = RevisionUseCommand::reserve(&read, input.existing.reservation_id, digest,
            &selection.preparation, counter, seal_metadata.clone(), LIMITS).map_err(|error| error.code().to_owned())?;
        let identity = db::control_mutation_identity(command.command().mutation().id()).map_err(|error| error.code().to_owned())?;
        let (_, permit) = port.transact_revision_use_with_original_context(&command, &identity, input.deadline, &work)
            .map_err(|error| error.control.code().to_owned())?;
        let permit = permit.ok_or("NATIVE_RETENTION_SEAL_USE_ALREADY_BURNED")?;
        let encrypted = os::seal_revision(permit, body, config.secrets_settings(), &mut native_key)
            .map_err(|error| error.code().to_owned())?;
        let (binding, metadata) = object_metadata(&input, &profile, domains, encrypted,
            crypto.nonce().to_vec(), selection.encryption.clone())?;
        let encoded_metadata = metadata.encode().map_err(|_| "NATIVE_RETENTION_OBJECT_METADATA_INVALID")?;
        let mut port = shared::bind(journal, publisher, input.native.context())?;
        let keys = db::source_retention::object_publication_intent_keys(input.existing.reservation_id, LIMITS)
            .map_err(|error| error.code().to_owned())?;
        let read = port.read_control_keys_with_original_context(&keys, input.deadline, &work)
            .map_err(|error| error.control.code().to_owned())?;
        let (id, digest) = retention_control::phase_id(&profile.operation_id, "object", &work).map_err(|error| error.to_string())?;
        let command = RevisionUseCommand::bind_object_publication(&read, input.existing.reservation_id, id, digest,
            &selection.preparation, &encoded_metadata, LIMITS).map_err(|error| error.code().to_owned())?;
        let identity = db::control_mutation_identity(command.command().mutation().id()).map_err(|error| error.code().to_owned())?;
        if port.transact_revision_use_with_original_context(&command, &identity, input.deadline, &work).is_err()
            && !matches!(port.recover_revision_use_with_original_context(&command, &identity, input.deadline, &work),
                Ok(db::CommitRecoveryDecision::Committed(_)))
        { return Err("NATIVE_RETENTION_OBJECT_INTENT_UNKNOWN".into()); }
        input.security.checkpoint().map_err(|error| error.to_string())?;
        if let Some(check) = input.current_source { check()?; }
        let publication = input.native.publish_selected_object(metadata.object_residency_key_digest(), id,
            binding.payload().ciphertext(), SELECTED_REVISION_MAX_BYTES).map_err(|error| error.code().to_owned())?;
        if publication.encoded_bytes() != metadata.ciphertext_bytes() {
            return Err("NATIVE_RETENTION_OBJECT_PUBLICATION_MISMATCH".into());
        }
        (binding, metadata, seal_metadata, counter)
    };
    // Fresh publication returns only after C07 reads the durable final object
    // and compares every byte with this owned payload. Recovery likewise built
    // this binding from an exact Native read and checked the complete metadata.
    // Reuse those verified bytes; a second identical filesystem read adds no
    // evidence and repeats the whole native object-admission sequence.
    let plaintext = os::open_revision(binding.payload().ciphertext(), &seal_metadata, &selection.current, counter,
        config.secrets_settings(), &mut native_key).map_err(|error| error.code().to_owned())?;
    if input.body.is_some_and(|body| body != plaintext.as_bytes()) { return Err("NATIVE_RETENTION_PLAINTEXT_MISMATCH".into()); }
    drop(plaintext);
    input.native.source_full_checkpoint().map_err(|error| error.to_string())?;
    input.security.checkpoint().map_err(|error| error.to_string())?;
    if let Some(check) = input.current_source { check()?; }
    let mut control = NativeRetentionControl::bind(journal, publisher, input.source, input.namespace,
        input.root, input.security, input.deadline, &work).map_err(|error| error.to_string())?;
    let committed = control.commit_verified_object(&profile, metadata.storage_object_id().clone(),
        metadata.object_residency_key_digest(), &metadata.encode().map_err(|_| "NATIVE_RETENTION_OBJECT_METADATA_INVALID")?,
        input.existing.reservation_id, input.native.context()).map_err(|error| error.to_string())?;
    drop(control);
    published_metadata(journal, publisher, input.deadline, &work)?;
    Ok(Some(output(&committed, false, true)))
}

fn prepare_occurrence(input: &RetentionSourceInput<'_, '_, '_>, journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher, residency: Vec<u8>, observed_at: c::UtcTimestamp,
) -> Result<SourceRevisionPreparation, String> {
    let previous = input.existing.profile.as_ref();
    let request = SourceRevisionPresealRequest::new(input.command.operation.clone(),
        input.source.identity().source_namespace_id, input.source.identity().source_id, input.command.expected_revision,
        previous.map_or_else(|| input.source.binding().content_digest(), |profile| profile.content_digest),
        previous.map_or_else(|| input.source.binding().content_bytes(), |profile| profile.byte_length), observed_at,
        c::AcquisitionKind::Filesystem,
        previous.map_or_else(|| input.source.binding().observation_receipt().clone(), |profile| profile.stability_receipt_ref.clone()),
        residency);
    let work = |unit| input.native.work(unit);
    let mut ids = SourceIdentityIdsV2::bind(input.deadline, &work);
    let mut control = NativeRetentionControl::bind(journal, publisher, input.source, input.namespace,
        input.root, input.security, input.deadline, &work).map_err(|error| error.to_string())?;
    prepare_source_revision(&request, &mut ids, &mut control, input.native.context())
        .map_err(|_| "NATIVE_RETENTION_OCCURRENCE_REFUSED".into())
}

impl DirectRegistryOwner {
    pub(super) fn execute_retention(mut self, hints: &RootLocatorHints, path: &Path,
        policy: &CapturedPolicyV2, command: &NativeRetentionCommand, config: &EffectiveDaemonConfig,
        writer: &mut impl Write,
    ) -> Result<(), String> {
        let result = self.with_retention_source(hints, path, policy, command,
            |input, journal, publisher| consume_source(input, journal, publisher, config));
        self.complete_output(result, writer)
    }
}
