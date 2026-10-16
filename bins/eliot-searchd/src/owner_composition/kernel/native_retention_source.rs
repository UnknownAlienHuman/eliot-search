//! One held-source ingestion path through the existing Native, C03 and C18 owners.

use std::{cell::RefCell, path::Path, sync::Arc, time::Instant};
use search_contracts as c;
use search_control_redb::{ControlSnapshotPublisher, MigrationBootstrapWork as Work, PersistentControlJournal};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_registry::{api::{NamespaceCutoverState, NativeNamespacePreparationV2,
    NativeNamespaceRequestV2, SourceAdmissionRequestV2, SourceNamespaceSelectionV2, SourceRegistry,
    commit_native_namespace_v2, prepare_native_namespace_v2},
    error::RegistryControlPort, recovery::RegistryBatchLookup,
    root::RootRecord, source::{AdmittedSourceReceipt, RegisteredSource}};
use crate::config_composition::RootLocatorHints;
use super::{DirectRegistryOwner, command::{DirectOutput, original_port},
    retention_command::NativeRetentionCommand, retention_control::{self, RetentionOperationReadback},
    source_command::{commit_native_admission, select_namespace, validate_replay}};
use super::super::super::direct_policy_v2::CapturedPolicyV2;
use super::super::read_child::fresh_root::{FreshRootContext, RetentionSecurity};

pub(super) struct RetentionSourceInput<'view, 'operation, 'owner> {
    pub(super) native: &'view FreshRootContext<'operation, 'owner>,
    pub(super) root: &'view RootRecord,
    pub(super) namespace: &'view NamespaceCutoverState,
    pub(super) source: &'view RegisteredSource,
    pub(super) security: &'view RetentionSecurity,
    pub(super) command: &'view NativeRetentionCommand,
    pub(super) existing: &'view RetentionOperationReadback,
    pub(super) body: Option<&'view [u8]>,
    pub(super) current_source: Option<&'view dyn Fn() -> Result<(), String>>,
    pub(super) deadline: Instant,
}

impl DirectRegistryOwner {
    /// The consumer first resolves retained state without a source body. It may
    /// request one fresh acquisition only when that resolution permits it.
    pub(super) fn with_retention_source(&mut self, hints: &RootLocatorHints, path: &Path,
        policy: &CapturedPolicyV2, command: &NativeRetentionCommand,
        mut consume: impl FnMut(RetentionSourceInput<'_, '_, '_>, &mut PersistentControlJournal,
            &mut ControlSnapshotPublisher) -> Result<Option<DirectOutput>, String>,
    ) -> Result<DirectOutput, String> {
        let root_id = hints.selected(path)?;
        let admission = command.admission()?;
        let intent = admission.intent(root_id, &policy.reference)?;
        let deadline = self.owner.request.credential_context().0;
        let mut read = super::super::read_child::direct_operation(&mut self.owner).map_err(|error| error.code().to_owned())?;
        let native = read.fresh_root_context().map_err(|error| error.to_string())?;
        let work = |unit| native.work(unit);
        let journal = self.control.journal.as_mut().ok_or("DIRECT_JOURNAL_UNAVAILABLE")?;
        let existing = retention_control::lookup_before_source(journal, &mut self.control.publisher,
            &command.operation, native.context(), deadline, &work).map_err(|error| error.to_string())?;
        let mut port = original_port(journal, &mut self.control.publisher, deadline, &work)?;
        let mut registry = port.restore(native.context()).map_err(|_| "NATIVE_SOURCE_RESTORE_REFUSED")?.into_registry();
        let profile = search_source_identity::ntfs_source_profile_v1().map_err(|_| "NATIVE_SOURCE_PROFILE_UNAVAILABLE")?;
        let historical = match port.load_batch(&admission.operation, native.context()).map_err(|_| "NATIVE_SOURCE_LOOKUP_REFUSED")? {
            RegistryBatchLookup::Committed(observed) => {
                observed.verify_operation().map_err(|_| "NATIVE_SOURCE_ORIGINAL_REFUSED")?;
                validate_replay(observed.record(), &admission, root_id, &intent, &policy.retained, &registry, profile)?;
                Some(search_source_registry::api::source_receipt(observed.record(), true).map_err(|_| "NATIVE_SOURCE_ORIGINAL_REFUSED")?)
            }
            RegistryBatchLookup::Absent { .. } => None,
        };
        if historical.is_none() && (existing.profile.is_some() || existing.result.is_some()) {
            return Err("NATIVE_RETENTION_ADMISSION_MISSING".into());
        }
        let qualified = native.requalify_existing_v2(path, root_id, &registry, &policy.retained,
            &policy.reference).map_err(|error| error.to_string())?;
        let root = qualified.root().clone();
        let (mut namespace, owner) = select_namespace(&registry, root_id, profile,
            native.source_owner_scope().map_err(|error| error.to_string())?, &policy.retained)?;
        let refusal = RefCell::new(None);
        let mut probe = || native.source_checkpoint().map_err(|error| {
            let cancelled = error.is_cancelled();
            if refusal.borrow().is_none() { *refusal.borrow_mut() = Some(error); }
            if cancelled { CheckpointFailureV2::Cancelled } else { CheckpointFailureV2::DeadlineExceeded }
        });
        let preparation_bytes = policy.retained.encoded_inputs().len().checked_mul(16)
            .and_then(|value| value.checked_add(32768)).ok_or("NATIVE_RETENTION_METADATA_BOUNDS")?;
        for unit in [Work::Allocate(preparation_bytes), Work::Process(preparation_bytes)] {
            work(unit).map_err(|error| error.code().to_owned())?;
        }
        let creation = if matches!(namespace, SourceNamespaceSelectionV2::CreateNew) {
            let request = NativeNamespaceRequestV2 { expected_registry_revision: registry.revision(),
                root_binding_id: root_id, expected_root_revision: root.record_revision, owner: owner.clone(),
                identity_profile: profile, policy: Arc::clone(&policy.retained),
                operation_id: command.namespace_operation.clone(), receipt: intent.clone() };
            let prepared = prepare_native_namespace_v2(&mut registry, &request, &mut port, native.context(), &mut probe);
            if let Some(error) = refusal.borrow_mut().take() { return Err(error.to_string()); }
            let created = match prepared.map_err(|_| "NATIVE_RETENTION_NAMESPACE_REFUSED")? {
                NativeNamespacePreparationV2::Prepared(prepared) => commit_native_namespace_v2(&mut registry,
                    &prepared, &mut port, native.context(), &mut probe).map_err(|_| "NATIVE_RETENTION_NAMESPACE_COMMIT_REFUSED")?,
                NativeNamespacePreparationV2::Replayed(created) => created,
            };
            let state = created.namespace();
            namespace = SourceNamespaceSelectionV2::Existing { id: state.ownership.source_namespace_id,
                revision: state.ownership.ownership_record_revision, generation: state.ownership.source_owner_generation };
            Some(created)
        } else { None };
        let SourceNamespaceSelectionV2::Existing { id: namespace_id, .. } = namespace else {
            return Err("NATIVE_RETENTION_NAMESPACE_REFUSED".into());
        };
        let state = registry.namespaces().get(&namespace_id).ok_or("NATIVE_RETENTION_NAMESPACE_MISSING")?;
        drop(port);
        let security_operation = c::OpaqueId::new(format!("native-retention-security:{}",
            super::super::super::codec::hex(namespace_id.as_bytes()))).map_err(|_| "NATIVE_RETENTION_OPERATION_INVALID")?;
        let security = native.retention_security(&registry, &root, state, creation.as_ref(), security_operation,
            journal, &mut self.control.publisher, deadline).map_err(|error| error.to_string())?;
        if let Some(receipt) = &historical {
            let source = registry.sources().get(&receipt.source_identity).ok_or("NATIVE_RETENTION_SOURCE_MISSING")?;
            let input = RetentionSourceInput { native: &native, root: &root, namespace: state, source,
                security: &security, command, existing: &existing, body: None, current_source: None, deadline };
            if let Some(output) = consume(input, journal, &mut self.control.publisher)? { return Ok(output); }
        }
        // The same C06 allocation and its held descriptor remain live through
        // encryption, C07 publication, exact readback and the final transaction.
        security.checkpoint().map_err(|error| error.to_string())?;
        let acquired = native.acquire_qualified_source_v2(qualified, &registry, command.relative.as_bytes(), profile)
            .map_err(|error| error.source_refusal_code().to_owned())?;
        let receipt: AdmittedSourceReceipt = if let Some(receipt) = historical {
            let source = registry.sources().get(&receipt.source_identity).ok_or("NATIVE_RETENTION_SOURCE_MISSING")?;
            if source.binding().content_digest() != acquired.binding_observation().content_digest
                || source.binding().content_bytes() != acquired.binding_observation().content_bytes
                || source.binding().stable_file_identity_digest() != acquired.binding_observation().stable_file_identity_digest
            { return Err("NATIVE_RETENTION_ORIGINAL_SOURCE_CHANGED".into()); }
            receipt
        } else {
            let mut port = original_port(journal, &mut self.control.publisher, deadline, &work)?;
            let request = SourceAdmissionRequestV2 { expected_registry_revision: registry.revision(), root_binding_id: root_id,
                expected_root_revision: root.record_revision, namespace, owner, identity_profile: profile,
                identity_observation: acquired.identity_observation().clone(), binding_observation: acquired.binding_observation().clone(),
                original: acquired.original().clone(), expected_source_revision: command.predecessor,
                operation_id: admission.operation, receipt: intent };
            commit_native_admission(&native, &mut registry, &request, &mut port,
                |view| acquired.check_current(view).map_err(|error| error.to_string()))?
        };
        let source = registry.sources().get(&receipt.source_identity).ok_or("NATIVE_RETENTION_SOURCE_MISSING")?;
        let namespace = registry.namespaces().get(&namespace_id).ok_or("NATIVE_RETENTION_NAMESPACE_MISSING")?;
        acquired.with_retained_body(&registry, |body| {
            security.checkpoint().map_err(|error| error.to_string())?;
            let current_source = || acquired.check_current(&registry).map_err(|error| error.to_string());
            consume(RetentionSourceInput { native: &native, root: &root, namespace, source,
                security: &security, command, existing: &existing, body: Some(body),
                current_source: Some(&current_source), deadline }, journal,
                &mut self.control.publisher)?.ok_or_else(|| "NATIVE_RETENTION_BODY_NOT_CONSUMED".to_owned())
        }).map_err(|error| match error {
            super::super::read_child::fresh_root::SourceBodyUseError::Before(error)
                | super::super::read_child::fresh_root::SourceBodyUseError::After(error) => error.to_string(),
            super::super::read_child::fresh_root::SourceBodyUseError::Consumer(error) => error,
        })
    }
}
