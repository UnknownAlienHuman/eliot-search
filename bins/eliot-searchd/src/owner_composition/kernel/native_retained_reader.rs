//! Exact retained source reopen through the existing C02, C07 and Windows owners.

use search_contracts::SourceRevisionRef;
use search_control_redb::MigrationBootstrapWork as Work;
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiWork};
use search_revision_crypto::{SELECTED_REVISION_MAX_BYTES, SelectedPlaintextV2};
use search_revision_store::{
    DEFAULT_REVISION_STORE_LIMITS, EncryptedRevisionPayload, ObjectMetadataBinding,
    PersistedSelectedObjectMetadata, PreSealDescriptor,
};

use super::super::read_child::fresh_root::{FreshRootCancellation, FreshRootContext};
use super::dek_command::NativeContext;
use super::retention_control::NativeRetentionControl;
use crate::config_composition::EffectiveDaemonConfig;

/// The canonical request retains this cumulative native meter between reads.
#[cfg(feature = "wave4-query")]
#[derive(Default)]
pub(crate) struct RetainedRevisionReadState {
    native: Option<super::super::read_child::NativeRequestReadState>,
    remaining_source_bytes: u64,
}

#[cfg(feature = "wave4-query")]
impl RetainedRevisionReadState {
    /// Capture the canonical task's admitted whole-request source-read ceiling.
    pub(crate) const fn new(max_source_read_bytes: u64) -> Self {
        Self {
            native: None,
            remaining_source_bytes: max_source_read_bytes,
        }
    }

    fn charge_source_bytes(
        &mut self,
        bytes: u64,
    ) -> Result<(), crate::provider_composition::CanonicalServingError> {
        if self.remaining_source_bytes == 0 {
            return Err(crate::provider_composition::CanonicalServingError::Backend(
                "RESOURCE_EXHAUSTED",
            ));
        }
        self.remaining_source_bytes = self.remaining_source_bytes.checked_sub(bytes).ok_or(
            crate::provider_composition::CanonicalServingError::Backend("RESOURCE_EXHAUSTED"),
        )?;
        Ok(())
    }
}

#[cfg(feature = "wave4-query")]
impl super::DirectRegistryOwner {
    /// Reopen through the same issuer-verified live turn and retained root.
    /// The canonical recipe retains `state` across all source reads and yields.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn with_serving_retained_read<T>(
        &mut self,
        state: &mut RetainedRevisionReadState,
        registry: &search_source_registry::SourceRegistry,
        output: &mut crate::provider_composition::CanonicalWorkOutput<'_, '_, '_>,
        admission: &crate::access_composition::StandalonePreRetrievalAdmission,
        membership: search_contracts::SourceMembershipId,
        expected: &SourceRevisionRef,
        config: &EffectiveDaemonConfig,
        consume: impl FnOnce(
            &[u8],
            &PersistedSelectedObjectMetadata,
        ) -> Result<T, crate::provider_composition::CanonicalServingError>,
    ) -> Result<T, crate::provider_composition::CanonicalServingError> {
        self.with_serving_retained_control(
            state,
            registry,
            output,
            admission,
            membership,
            expected,
            Some(expected.byte_length),
            |native, control| {
                let (plaintext, metadata) =
                    open_retained_revision(native, control, expected, config).map_err(|_| {
                        crate::provider_composition::CanonicalServingError::Backend(
                            "SOURCE_REVISION_UNAVAILABLE",
                        )
                    })?;
                consume(plaintext.as_bytes(), &metadata)
            },
        )
    }

    /// Resolve the real committed C07 link and active C28 cause under the same
    /// current issuer/native read boundary, without decrypting source bytes.
    /// An owned pin may be created inside `consume`; a changed final link drops
    /// that result before it can escape the qualified operation.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn with_serving_retained_metadata<T>(
        &mut self,
        state: &mut RetainedRevisionReadState,
        registry: &search_source_registry::SourceRegistry,
        output: &mut crate::provider_composition::CanonicalWorkOutput<'_, '_, '_>,
        admission: &crate::access_composition::StandalonePreRetrievalAdmission,
        membership: search_contracts::SourceMembershipId,
        expected: &SourceRevisionRef,
        consume: impl FnOnce(
            &PersistedSelectedObjectMetadata,
        ) -> Result<T, crate::provider_composition::CanonicalServingError>,
    ) -> Result<T, crate::provider_composition::CanonicalServingError> {
        self.with_serving_retained_control(
            state,
            registry,
            output,
            admission,
            membership,
            expected,
            None,
            |native, control| {
                let unavailable = || {
                    crate::provider_composition::CanonicalServingError::Backend(
                        "SOURCE_REVISION_UNAVAILABLE",
                    )
                };
                let (metadata, reservation) = control
                    .load_retained_revision(expected, native.context())
                    .map_err(|_| unavailable())?;
                let result = consume(&metadata)?;
                let (after, after_reservation) = control
                    .load_retained_revision(expected, native.context())
                    .map_err(|_| unavailable())?;
                if metadata != after || reservation != after_reservation {
                    return Err(unavailable());
                }
                native.source_full_checkpoint().map_err(|_| unavailable())?;
                Ok(result)
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn with_serving_retained_control<T>(
        &mut self,
        state: &mut RetainedRevisionReadState,
        registry: &search_source_registry::SourceRegistry,
        output: &mut crate::provider_composition::CanonicalWorkOutput<'_, '_, '_>,
        admission: &crate::access_composition::StandalonePreRetrievalAdmission,
        membership: search_contracts::SourceMembershipId,
        expected: &SourceRevisionRef,
        source_bytes: Option<u64>,
        consume: impl FnOnce(
            &FreshRootContext<'_, '_>,
            &NativeRetentionControl<'_, '_, FreshRootCancellation>,
        ) -> Result<T, crate::provider_composition::CanonicalServingError>,
    ) -> Result<T, crate::provider_composition::CanonicalServingError> {
        use crate::provider_composition::CanonicalServingError as Error;
        use search_access::AccessError;
        let unavailable = || Error::Backend("SOURCE_REVISION_UNAVAILABLE");
        output.with_standalone_source_read(
            admission,
            membership,
            expected,
            |expected, admitted, budget, domain, fence| {
                if registry.revision() != admission.scope.source_catalog_generation {
                    return Err(Error::Access(AccessError::SnapshotStale));
                }
                let (_, identity) = registry
                    .active_source_membership_v2(membership)
                    .map_err(|_| Error::Access(AccessError::ScopeUnauthorized))?;
                if identity.source_namespace_id != expected.source_namespace_id
                    || identity.source_id != expected.source_id
                {
                    return Err(Error::Access(AccessError::ScopeUnauthorized));
                }
                // This encrypted profile reopens the whole revision, even for
                // one small unit. Charge that actual source work before I/O;
                // subsequent candidates/turns cannot renew the request budget.
                if let Some(bytes) = source_bytes {
                    state.charge_source_bytes(bytes)?;
                }
                let key = registry
                    .membership_key_by_id(membership)
                    .map_err(|_| unavailable())?;
                let member = registry.membership(key).map_err(|_| unavailable())?;
                let source = registry.sources().get(identity).ok_or_else(unavailable)?;
                let namespace = registry
                    .namespaces()
                    .get(&identity.source_namespace_id)
                    .ok_or_else(unavailable)?;
                let root = registry
                    .root(source.binding().root_binding_id())
                    .map_err(|_| unavailable())?;
                if state.native.is_none() {
                    state.native = Some(
                        super::super::read_child::NativeRequestReadState::new(
                            &self.owner,
                            admitted,
                            budget,
                        )
                        .map_err(|_| unavailable())?,
                    );
                }
                let native_state = state.native.as_mut().ok_or_else(unavailable)?;
                let journal = self.control.journal.as_mut().ok_or_else(unavailable)?;
                let publisher = &mut self.control.publisher;
                super::super::read_child::with_direct_read_operation(
                    &mut self.owner,
                    admitted,
                    native_state,
                    budget,
                    |operation| {
                        let deadline = operation.migration_deadline();
                        let native = operation.fresh_root_context().map_err(|_| unavailable())?;
                        native.source_full_checkpoint().map_err(|_| unavailable())?;
                        // Pay for the bounded current-head predicates before their encoding.
                        for unit in [
                            Work::Allocate(
                                4 * search_control_redb::JournalLimits::BASELINE.max_value_bytes,
                            ),
                            Work::Process(
                                4 * search_control_redb::JournalLimits::BASELINE.max_value_bytes,
                            ),
                        ] {
                            native.work(unit).map_err(|_| unavailable())?;
                        }
                        let conditions = domain
                            .retained_source_read_conditions(
                                identity.source_namespace_id,
                                namespace.ownership.source_owner_generation,
                                journal,
                                fence,
                            )
                            .map_err(|error| match error {
                                crate::access_composition::NativeSecurityError::Access(error) => {
                                    Error::Access(error)
                                }
                                _ => unavailable(),
                            })?;
                        let work = |unit| native.work(unit);
                        let control = NativeRetentionControl::bind_for_serving(
                            journal,
                            publisher,
                            source,
                            namespace,
                            root,
                            member,
                            &conditions,
                            deadline,
                            &work,
                        )
                        .map_err(|_| unavailable())?;
                        consume(&native, &control)
                    },
                )
                .map_err(|_| unavailable())?
            },
        )
    }
}

#[cfg(all(test, feature = "wave4-query"))]
mod source_budget_tests {
    use super::RetainedRevisionReadState;

    #[test]
    fn whole_revision_reads_share_one_source_budget() {
        let mut state = RetainedRevisionReadState::new(8);
        state.charge_source_bytes(3).unwrap();
        assert!(state.charge_source_bytes(6).is_err());
        state.charge_source_bytes(5).unwrap();
        assert!(state.charge_source_bytes(1).is_err());
    }

    #[test]
    fn missing_source_budget_refuses_even_an_empty_revision() {
        assert!(
            RetainedRevisionReadState::default()
                .charge_source_bytes(0)
                .is_err()
        );
    }
}

/// Production composition must enter through the provider's issuer-verified
/// source-read callback in the same live authority turn.
///
/// The source-owner
/// fixture exercises reopen separately; it creates no serving permission.
/// The native context and control adapter retain
/// the original request; this function allocates no deadline, source identity,
/// key, query state or mutable Current selection.
pub(super) fn open_retained_revision(
    native: &FreshRootContext<'_, '_>,
    control: &NativeRetentionControl<'_, '_, FreshRootCancellation>,
    expected: &SourceRevisionRef,
    config: &EffectiveDaemonConfig,
) -> Result<(SelectedPlaintextV2, PersistedSelectedObjectMetadata), String> {
    native
        .source_full_checkpoint()
        .map_err(|error| error.to_string())?;
    let (metadata, seal) = control
        .load_retained_revision(expected, native.context())
        .map_err(|error| error.to_string())?;
    let read = native
        .read_selected_object(
            metadata.object_residency_key_digest(),
            SELECTED_REVISION_MAX_BYTES,
        )
        .map_err(|error| error.code().to_owned())?;
    let ciphertext = read.into_bytes();
    for unit in [
        Work::Allocate(8 * 8192),
        Work::Process(8 * 8192),
        Work::Process(ciphertext.len()),
    ] {
        native.work(unit).map_err(|error| error.code().to_owned())?;
    }
    // Reuse C07's exact payload/physical metadata derivation. A valid envelope
    // alone cannot excuse bytes different from the durable physical object link.
    let payload = EncryptedRevisionPayload::new_selected_v2(
        expected.content_digest,
        expected.byte_length,
        metadata.nonce().to_vec(),
        ciphertext,
        metadata.encryption().clone(),
        DEFAULT_REVISION_STORE_LIMITS,
    )
    .map_err(|error| error.code().to_owned())?;
    let descriptor = PreSealDescriptor::new(
        *metadata.source(),
        metadata.domain_bindings().clone(),
        expected.content_digest,
        expected.byte_length,
        metadata.operation_id().clone(),
    )
    .map_err(|error| error.code().to_owned())?;
    let binding = ObjectMetadataBinding::new(
        descriptor,
        metadata.storage_object_id().clone(),
        payload,
        metadata.encryption().clone(),
    )
    .map_err(|error| error.code().to_owned())?;
    if PersistedSelectedObjectMetadata::from_binding(&binding)
        .map_err(|error| error.code().to_owned())?
        != metadata
    {
        return Err("NATIVE_RETAINED_READ_OBJECT_MISMATCH".into());
    }
    let seal_metadata = super::retention::metadata_from_reservation(&seal);
    let poll = || native.work(Work::Checkpoint);
    let native_work = |unit| match unit {
        LegacyRevisionDpapiWork::NativeCall => native.revision_dek_native_attempt(),
        LegacyRevisionDpapiWork::Allocate(bytes) => native.work(Work::Allocate(bytes)),
        LegacyRevisionDpapiWork::Process(bytes) => native.work(Work::Process(bytes)),
    };
    let open = || native.work(Work::Open);
    let clear = |bytes| native.revision_dek_account_native_clear(bytes);
    let mut key_context = NativeContext {
        deadline: control.deadline(),
        poll: &poll,
        work: &native_work,
        open: &open,
        clear: &clear,
    };
    let plaintext = os::open_revision(
        binding.payload().ciphertext(),
        &seal_metadata,
        seal.selected_current_canonical(),
        seal.counter(),
        config.secrets_settings(),
        &mut key_context,
    )
    .map_err(|error| error.code().to_owned())?;
    native
        .source_full_checkpoint()
        .map_err(|error| error.to_string())?;
    // Release/purge or any source/policy drift after the object/key read wins.
    let (after, after_seal) = control
        .load_retained_revision(expected, native.context())
        .map_err(|error| error.to_string())?;
    if after != metadata
        || after_seal.selected_current_canonical() != seal.selected_current_canonical()
        || after_seal.counter() != seal.counter()
    {
        return Err("NATIVE_RETAINED_READ_STATE_CHANGED".into());
    }
    Ok((plaintext, metadata))
}
