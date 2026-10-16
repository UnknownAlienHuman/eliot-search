//! One explicit Native Source admission over the same DIRECT journal.
use super::{DirectRegistryOwner, command::{DirectOutput, original_port, published_metadata}};
use crate::config_composition::RootLocatorHints;
use search_contracts::{self as c, CanonicalValue as V};
use search_source_registry::{api::{SourceAdmissionPreparationV2, SourceAdmissionRequestV2,
    SourceNamespaceSelectionV2, SourceOwnerScopeV2, SourceRegistry,
    commit_source_admission_v2, prepare_source_admission_v2},
    error::RegistryControlPort, recovery::{RegistryBatchLookup, RegistryStateRecord}};
use search_source_admission::{observation_v2::AdmissionRelativeLocator, policy_v2::CheckpointFailureV2};
use std::{cell::RefCell, ffi::OsString, path::Path};

pub(super) struct NativeSourceCommand {
    pub(super) relative: AdmissionRelativeLocator,
    pub(super) operation: c::OpaqueId,
    pub(super) predecessor: Option<c::NonZeroRevision>,
}
impl NativeSourceCommand {
    pub(super) fn parse(selected: &[OsString]) -> Result<Self, String> {
        if selected.len() != 7 { return Err("NATIVE_SOURCE_USAGE".into()); }
        let relative = selected[3].to_str().ok_or("NATIVE_SOURCE_LOCATOR_INVALID")?;
        let relative = AdmissionRelativeLocator::from_bytes(relative.as_bytes())
            .map_err(|_| "NATIVE_SOURCE_LOCATOR_INVALID")?;
        let id = selected[5].to_str().ok_or("NATIVE_SOURCE_OPERATION_INVALID")?;
        if id.len() != 32 || !id.bytes().all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
            || id.bytes().all(|v| v == b'0') { return Err("NATIVE_SOURCE_OPERATION_INVALID".into()); }
        let operation = c::OpaqueId::new(format!("native-source:{id}"))
            .map_err(|_| "NATIVE_SOURCE_OPERATION_INVALID")?;
        let predecessor = match selected[6].to_str().ok_or("NATIVE_SOURCE_PREDECESSOR_INVALID")? {
            "new" => None,
            value => Some(c::NonZeroRevision::new(value.parse().map_err(|_| "NATIVE_SOURCE_PREDECESSOR_INVALID")?)
                .map_err(|_| "NATIVE_SOURCE_PREDECESSOR_INVALID")?),
        };
        Ok(Self { relative, operation, predecessor })
    }
    fn intent(&self, root: c::RootBindingId, policy: &[u8]) -> Result<c::ReceiptRef, String> {
        let bytes = |v: &[u8]| c::BoundedBytes::new(v.to_vec()).map(V::Bytes).map_err(|_| "NATIVE_SOURCE_INTENT_INVALID");
        let value = c::BoundedList::new(vec![V::U64(1), bytes(root.as_bytes())?,
            bytes(self.relative.as_bytes())?, bytes(policy)?, self.predecessor.map_or(V::Null, |v| V::U64(v.get()))])
            .map(V::Array).map_err(|_| "NATIVE_SOURCE_INTENT_INVALID")?;
        let digest = c::blake3_canonical(&c::CanonicalDigestDomain::parse("eliot/cbor/native-source-intent/v1")
            .map_err(|_| "NATIVE_SOURCE_INTENT_INVALID")?, &value,
            c::DigestInputLimit::new(16 * 1024).map_err(|_| "NATIVE_SOURCE_INTENT_INVALID")?)
            .map_err(|_| "NATIVE_SOURCE_INTENT_INVALID")?;
        c::ReceiptRef::new(format!("native-source-intent-v1:{digest}")).map_err(|_| "NATIVE_SOURCE_INTENT_INVALID".into())
    }
}

pub(super) fn select_namespace(registry: &SourceRegistry, root: c::RootBindingId,
    profile: search_source_identity::FilesystemIdentityProfile, actual: SourceOwnerScopeV2,
    policy: &search_source_provenance::RetainedPolicyV2,
) -> Result<(SourceNamespaceSelectionV2, SourceOwnerScopeV2), String> {
    let mut selected = registry.namespaces().values().filter(|value| value.permanent_metadata()
        .is_some_and(|m| m.root_binding_id() == root && m.identity_profile() == profile));
    let Some(namespace) = selected.next() else { return Ok((SourceNamespaceSelectionV2::CreateNew, actual)); };
    if selected.next().is_some() { return Err("NATIVE_SOURCE_NAMESPACE_AMBIGUOUS".into()); }
    if namespace.fenced || namespace.preparation.is_some() || namespace.activation.is_some()
        || namespace.ownership.status != c::NamespaceOwnershipStatus::Active
        || namespace.ownership.owner_system_id != actual.system_id
        || namespace.ownership.owner_installation_incarnation_id != actual.installation_incarnation
        || namespace.permanent_metadata().is_none_or(|m| m.policy().fingerprint() != policy.fingerprint()) {
        return Err("NATIVE_SOURCE_NAMESPACE_FENCED".into());
    }
    let owner = SourceOwnerScopeV2 { epoch: namespace.ownership.owner_epoch, ..actual };
    Ok((SourceNamespaceSelectionV2::Existing { id: namespace.ownership.source_namespace_id,
        revision: namespace.ownership.ownership_record_revision,
        generation: namespace.ownership.source_owner_generation }, owner))
}

impl DirectRegistryOwner {
    pub(super) fn prepare_source_output(&mut self, hints: &RootLocatorHints,
        selected: Option<&Path>, policy: Option<&super::super::super::direct_policy_v2::CapturedPolicyV2>,
        command: &NativeSourceCommand,
    ) -> Result<DirectOutput, String> {
        let path = selected.ok_or("NATIVE_SOURCE_ROOT_REQUIRED")?;
        let policy = policy.ok_or("NATIVE_SOURCE_POLICY_REQUIRED")?;
        let root_id = hints.selected(path)?;
        let intent = command.intent(root_id, &policy.reference)?;
        let deadline = self.owner.request.credential_context().0;
        let mut read = super::super::read_child::direct_operation(&mut self.owner)
            .map_err(|e| e.code().to_owned())?;
        let native = read.fresh_root_context().map_err(|e| e.to_string())?;
        let callback = |work| native.work(work);
        let journal = self.control.journal.as_mut().ok_or("DIRECT_JOURNAL_UNAVAILABLE")?;
        let mut port = original_port(journal, &mut self.control.publisher, deadline, &callback)?;
        let mut registry = port.restore(native.context()).map_err(|_| "NATIVE_SOURCE_RESTORE_REFUSED")?.into_registry();
        let profile = search_source_identity::ntfs_source_profile_v1()
            .map_err(|_| "NATIVE_SOURCE_PROFILE_UNAVAILABLE")?;
        // Exact immutable operation comes before opening a fresh Source FD.
        if let RegistryBatchLookup::Committed(observed) = port.load_batch(&command.operation, native.context())
            .map_err(|_| "NATIVE_SOURCE_LOOKUP_REFUSED")? {
            observed.verify_operation().map_err(|_| "NATIVE_SOURCE_ORIGINAL_REFUSED")?;
            let record = observed.record();
            validate_replay(record, command, root_id, &intent, &policy.retained, &registry, profile)?;
            let receipt = search_source_registry::api::source_receipt(record, true)
                .map_err(|_| "NATIVE_SOURCE_ORIGINAL_REFUSED")?;
            return Ok(source_output(&receipt, true));
        }
        let (namespace, owner) = select_namespace(&registry, root_id, profile,
            native.source_owner_scope().map_err(|e| e.to_string())?, &policy.retained)?;
        let acquired = match native.acquire_source_v2(path, root_id, &registry, policy,
            command.relative.as_bytes(), profile) {
            Ok(value) => value,
            Err(error) => return Ok(DirectOutput { current: false, line: format!(
                "{{\"event\":\"native_source\",\"admitted\":false,\"reason\":\"{}\",\"ready\":false,\"source_body_serving\":false}}",
                error.source_refusal_code()) }),
        };
        let request = SourceAdmissionRequestV2 {
            expected_registry_revision: registry.revision(), root_binding_id: root_id,
            expected_root_revision: acquired.root().record_revision, namespace, owner,
            identity_profile: profile, identity_observation: acquired.identity_observation().clone(),
            binding_observation: acquired.binding_observation().clone(), original: acquired.original().clone(),
            expected_source_revision: command.predecessor, operation_id: command.operation.clone(), receipt: intent,
        };
        let receipt = commit_native_admission(&native, &mut registry, &request, &mut port,
            |view| acquired.check_current(view).map_err(|error| error.to_string()))?;
        // The enclosing command performs its full Native fence before output.
        // Only held-source validation and journal readback remain before it.
        acquired.check_current(&registry).map_err(|e| e.to_string())?;
        drop(port);
        let _published = published_metadata(journal, &mut self.control.publisher, deadline, &callback)?;
        Ok(source_output(&receipt, false))
    }
}

/// The shared admission boundary for metadata-only admission and retention.
/// The held Source is checked immediately before the same C03 commit.
pub(super) fn commit_native_admission(
    native: &super::super::read_child::fresh_root::FreshRootContext<'_, '_>,
    registry: &mut SourceRegistry, request: &SourceAdmissionRequestV2,
    port: &mut crate::registry_control::RegistryJournal<'_, super::super::read_child::fresh_root::FreshRootCancellation>,
    mut held_source: impl FnMut(&SourceRegistry) -> Result<(), String>,
) -> Result<search_source_registry::source::AdmittedSourceReceipt, String> {
    let first_refusal = RefCell::new(None);
    let mut probe = || native.source_checkpoint().map_err(|error| {
        let cancelled = error.is_cancelled();
        if first_refusal.borrow().is_none() { *first_refusal.borrow_mut() = Some(error); }
        if cancelled { CheckpointFailureV2::Cancelled } else { CheckpointFailureV2::DeadlineExceeded }
    });
    let preparation = prepare_source_admission_v2(registry, request, port, native.context(), &mut probe);
    if let Some(error) = first_refusal.borrow_mut().take() { return Err(error.to_string()); }
    match preparation.map_err(|error| format!("{error:?}"))? {
        SourceAdmissionPreparationV2::Replayed(receipt) => Ok(receipt),
        SourceAdmissionPreparationV2::Prepared(prepared) => {
            native.source_full_checkpoint().map_err(|error| error.to_string())?;
            held_source(registry)?;
            match commit_source_admission_v2(registry, &prepared, port, native.context(), &mut probe) {
                Ok(receipt) => Ok(receipt),
                Err(error) => {
                    if port.requires_recovery() { let _original = port.recover_pending(native.context()); }
                    if let Some(first) = first_refusal.borrow_mut().take() { return Err(first.to_string()); }
                    Err(format!("{error:?}"))
                }
            }
        }
    }
}

pub(super) fn validate_replay(record: &search_source_registry::recovery::DecodedBatch, command: &NativeSourceCommand,
    root: c::RootBindingId, intent: &c::ReceiptRef, policy: &search_source_provenance::RetainedPolicyV2,
    registry: &SourceRegistry, profile: search_source_identity::FilesystemIdentityProfile,
) -> Result<(), String> {
    let conflict = || "NATIVE_SOURCE_OPERATION_CONFLICT".to_owned();
    if record.encoding_profile() != "eliot.registry-batch.v4" || record.entry().operation_id != command.operation
        || &record.entry().receipt != intent { return Err(conflict()); }
    let mut sources = record.deltas().iter().filter(|v| matches!(v.after(), RegistryStateRecord::Source(_)));
    let delta = sources.next().ok_or_else(conflict)?;
    if sources.next().is_some() { return Err(conflict()); }
    let RegistryStateRecord::Source(source) = delta.after() else { return Err(conflict()); };
    let original = source.permanent_admission().ok_or_else(conflict)?.original();
    if registry.namespaces().get(&source.identity().source_namespace_id)
        .and_then(|namespace| namespace.permanent_metadata())
        .is_none_or(|metadata| metadata.root_binding_id() != root || metadata.identity_profile() != profile) {
        return Err(conflict());
    }
    if source.binding().root_binding_id() != root
        || source.lifecycle() != search_source_registry::source::SourceLifecycle::Active
        || original.policy().encoded_inputs() != policy.encoded_inputs()
        || original.observation().relative_locator().map(search_source_admission::observation_v2::AdmissionRelativeLocator::as_bytes)
            != Some(source.binding().relative_path().as_str().as_bytes()) { return Err(conflict()); }
    match (command.predecessor, delta.before()) {
        (None, None) if source.source_revision().get() == 1 => {}
        (Some(expected), Some(RegistryStateRecord::Source(before)))
            if before.identity() == source.identity() && before.source_revision() == expected
                && expected.get().checked_add(1) == Some(source.source_revision().get()) => {}
        _ => return Err(conflict()),
    }
    for other in record.deltas() {
        match other.after() {
            RegistryStateRecord::Source(_) => {}
            RegistryStateRecord::Namespace(namespace) if other.before().is_none()
                && command.predecessor.is_none()
                && namespace.ownership.source_namespace_id == source.identity().source_namespace_id
                && namespace.ownership.status == c::NamespaceOwnershipStatus::Active
                && !namespace.fenced && namespace.preparation.is_none() && namespace.activation.is_none()
                && namespace.permanent_metadata().is_some_and(|m| m.root_binding_id() == root
                    && m.identity_profile() == profile && m.policy().encoded_inputs() == policy.encoded_inputs()) => {}
            _ => return Err(conflict()),
        }
    }
    Ok(())
}

fn source_output(receipt: &search_source_registry::source::AdmittedSourceReceipt, historical: bool) -> DirectOutput {
    DirectOutput { current: true, line: format!(
        "{{\"event\":\"native_source\",\"admitted\":true,\"historical_commit\":{historical},\"namespace\":\"{}\",\"source\":\"{}\",\"source_record_revision\":{},\"registry_revision\":{},\"qualification_scope\":\"source_admission_only\",\"retained_revision\":false,\"source_body_serving\":false,\"ready\":false}}",
        super::super::super::codec::hex(receipt.source_identity.source_namespace_id.as_bytes()),
        super::super::super::codec::hex(receipt.source_identity.source_id.as_bytes()),
        receipt.source_revision.get(), receipt.registry_revision) }
}
