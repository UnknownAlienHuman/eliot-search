//! Exact retained source reopen through the existing C02, C07 and Windows owners.

use search_contracts::SourceRevisionRef;
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiWork};
use search_revision_crypto::{SELECTED_REVISION_MAX_BYTES, SelectedPlaintextV2};
use search_revision_store::{
    DEFAULT_REVISION_STORE_LIMITS, EncryptedRevisionPayload, ObjectMetadataBinding,
    PersistedSelectedObjectMetadata, PreSealDescriptor,
};
use search_control_redb::MigrationBootstrapWork as Work;

use crate::config_composition::EffectiveDaemonConfig;
use super::dek_command::NativeContext;
use super::retention_control::NativeRetentionControl;
use super::super::read_child::fresh_root::{FreshRootCancellation, FreshRootContext};

/// Production composition must enter through the provider's issuer-verified
/// source-read callback in the same live authority turn. The source-owner
/// fixture exercises reopen separately; it creates no serving permission.
/// The native context and control adapter retain
/// the original request; this function allocates no deadline, source identity,
/// key, query state or mutable Current selection.
pub(super) fn open_retained_revision(
    native: &FreshRootContext<'_, '_>,
    control: &NativeRetentionControl<'_, '_, FreshRootCancellation>,
    expected: &SourceRevisionRef,
    config: &EffectiveDaemonConfig,
) -> Result<SelectedPlaintextV2, String> {
    native.source_full_checkpoint().map_err(|error| error.to_string())?;
    let (metadata, seal) = control.load_retained_revision(expected, native.context())
        .map_err(|error| error.to_string())?;
    let read = native.read_selected_object(metadata.object_residency_key_digest(), SELECTED_REVISION_MAX_BYTES)
        .map_err(|error| error.code().to_owned())?;
    let ciphertext = read.into_bytes();
    for unit in [Work::Allocate(8 * 8192), Work::Process(8 * 8192), Work::Process(ciphertext.len())] {
        native.work(unit).map_err(|error| error.code().to_owned())?;
    }
    // Reuse C07's exact payload/physical metadata derivation. A valid envelope
    // alone cannot excuse bytes different from the durable physical object link.
    let payload = EncryptedRevisionPayload::new_selected_v2(expected.content_digest,
        expected.byte_length, metadata.nonce().to_vec(), ciphertext,
        metadata.encryption().clone(), DEFAULT_REVISION_STORE_LIMITS)
        .map_err(|error| error.code().to_owned())?;
    let descriptor = PreSealDescriptor::new(*metadata.source(), metadata.domain_bindings().clone(),
        expected.content_digest, expected.byte_length, metadata.operation_id().clone())
        .map_err(|error| error.code().to_owned())?;
    let binding = ObjectMetadataBinding::new(descriptor, metadata.storage_object_id().clone(),
        payload, metadata.encryption().clone()).map_err(|error| error.code().to_owned())?;
    if PersistedSelectedObjectMetadata::from_binding(&binding)
        .map_err(|error| error.code().to_owned())? != metadata
    { return Err("NATIVE_RETAINED_READ_OBJECT_MISMATCH".into()); }
    let seal_metadata = super::retention::metadata_from_reservation(&seal);
    let poll = || native.work(Work::Checkpoint);
    let native_work = |unit| match unit {
        LegacyRevisionDpapiWork::NativeCall => native.revision_dek_native_attempt(),
        LegacyRevisionDpapiWork::Allocate(bytes) => native.work(Work::Allocate(bytes)),
        LegacyRevisionDpapiWork::Process(bytes) => native.work(Work::Process(bytes)),
    };
    let open = || native.work(Work::Open);
    let clear = |bytes| native.revision_dek_account_native_clear(bytes);
    let mut key_context = NativeContext { deadline: control.deadline(), poll: &poll,
        work: &native_work, open: &open, clear: &clear };
    let plaintext = os::open_revision(binding.payload().ciphertext(), &seal_metadata,
        seal.selected_current_canonical(), seal.counter(), config.secrets_settings(), &mut key_context)
        .map_err(|error| error.code().to_owned())?;
    native.source_full_checkpoint().map_err(|error| error.to_string())?;
    // Release/purge or any source/policy drift after the object/key read wins.
    let (after, after_seal) = control.load_retained_revision(expected, native.context())
        .map_err(|error| error.to_string())?;
    if after != metadata || after_seal.selected_current_canonical() != seal.selected_current_canonical()
        || after_seal.counter() != seal.counter()
    { return Err("NATIVE_RETAINED_READ_STATE_CHANGED".into()); }
    Ok(plaintext)
}
