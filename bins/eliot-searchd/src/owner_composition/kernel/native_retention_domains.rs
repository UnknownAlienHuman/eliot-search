//! Stable semantic residency bodies; all UUIDs come from the existing Native port.

use std::time::Instant;
use search_contracts::{self as c, CanonicalValue as V};
use search_control_redb::MigrationBootstrapWork as Work;
use search_revision_store::{DomainBinding, DomainKind, decode_residency_canonical, residency_canonical_bytes};
use search_source_registry::{api::NamespaceCutoverState, root::RootRecord};
use crate::registry_control::{SourceIdentityIdError, SourceIdentityIdsV2};
use super::{FreshRootContext, FreshRootError, array, bytes, text};
use super::native_retention_security::RetentionSecurity;

fn mismatch() -> FreshRootError { FreshRootError::Source("NATIVE_RETENTION_DOMAIN_MISMATCH") }

impl FreshRootContext<'_, '_> {
    pub(crate) fn retention_domains(&self, root: &RootRecord, namespace: &NamespaceCutoverState,
        security: &RetentionSecurity, current_canonical: &[u8], existing_claim: Option<&[u8]>,
        deadline: Instant,
    ) -> Result<[DomainBinding; 6], FreshRootError> {
        self.source_full_checkpoint()?;
        security.checkpoint().map_err(|_| mismatch())?;
        let facts = self.retained_root_facts()?;
        let actual = self.source_owner_scope()?;
        let owner = &namespace.ownership;
        if namespace.fenced || namespace.preparation.is_some() || namespace.activation.is_some()
            || owner.status != c::NamespaceOwnershipStatus::Active
            || owner.owner_system_id != actual.system_id
            || owner.owner_installation_incarnation_id != actual.installation_incarnation
            || namespace.permanent_metadata().is_none_or(|metadata| metadata.root_binding_id() != root.root_binding_id)
            || root.status != search_source_registry::root::RootStatus::Active
        { return Err(mismatch()); }
        let allocation = current_canonical.len().checked_add(security.policy_body().len())
            .and_then(|value| value.checked_add(8192)).and_then(|value| value.checked_mul(16))
            .ok_or_else(mismatch)?;
        self.work(Work::Allocate(allocation)).map_err(FreshRootError::Native)?;
        self.work(Work::Process(allocation)).map_err(FreshRootError::Native)?;
        let search_os_secrets::RevisionDekSelectionRecord::Current(current) =
            search_os_secrets::decode_revision_dek_selection_record(current_canonical).map_err(|_| mismatch())?
        else { return Err(mismatch()); };
        let key = current.intent().candidate().intent().binding();
        if key.root_id() != facts.data_root || key.installation_id() != facts.installation
            || key.installation_incarnation_id() != facts.incarnation
            || key.user_scope_digest() != security.user_scope()
        { return Err(mismatch()); }
        let bodies = [
            array(vec![text("native-retention-scope/v1")?, bytes(root.root_binding_id.as_bytes())?,
                bytes(root.canonical_root_digest.as_bytes())?, bytes(facts.data_root.as_bytes())?,
                bytes(facts.installation.as_bytes())?, bytes(facts.incarnation.as_bytes())?,
                bytes(owner.source_namespace_id.as_bytes())?, text(owner.owner_system_id.as_str())?,
                bytes(owner.owner_installation_incarnation_id.as_bytes())?, V::U64(owner.owner_epoch.get()),
                V::U64(owner.ownership_record_revision.get()), bytes(owner.source_owner_generation.as_bytes())?])?,
            array(vec![text("native-retention-access/v1")?, bytes(&security.user_scope())?, bytes(security.policy_body())?])?,
            array(vec![text("native-retention-confidentiality/v1")?, text("Confidential")?, text("local-owner-only;no-export")?])?,
            array(vec![text("native-retention-encryption/v1")?, bytes(current_canonical)?])?,
            array(vec![text("native-retention-protection/v1")?, text("until-explicit-release")?,
                text("one-exact-cause;no-expiry;release-does-not-delete;purge-dominates")?])?,
            array(vec![text("native-retention-erasure/v1")?, bytes(facts.data_root.as_bytes())?,
                bytes(facts.installation.as_bytes())?, bytes(owner.source_namespace_id.as_bytes())?,
                text("whole-namespace;monotonic-deny;survives-key-owner-epoch-domain-change")?])?,
        ];
        let mut encoded = Vec::with_capacity(6);
        for body in bodies {
            let body = c::to_canonical_cbor(&body).map_err(|_| mismatch())?.into_vec();
            if body.len() > DomainBinding::MAX_BODY_BYTES { return Err(mismatch()); }
            encoded.push(body);
        }
        if let Some(claim) = existing_claim {
            let bindings = decode_residency_canonical(claim).map_err(|_| mismatch())?;
            for ((binding, kind), body) in bindings.iter().zip(DomainKind::ALL).zip(&encoded) {
                if binding.kind() != kind || binding.canonical_body() != body { return Err(mismatch()); }
            }
            // Preserve the original actual identities; equal meanings do not
            // justify drawing another six UUIDs on a later Source or restart.
            return Ok(bindings);
        }
        let work = |unit| self.work(unit);
        let mut ids = SourceIdentityIdsV2::bind(deadline, &work);
        let mut bindings = Vec::with_capacity(6);
        for (kind, body) in DomainKind::ALL.into_iter().zip(encoded) {
            let uuid = ids.fresh_residency_domain_uuid(self.context()).map_err(|error| match error {
                SourceIdentityIdError::Owner(first) => FreshRootError::Native(first),
                _ => FreshRootError::Source("NATIVE_RETENTION_DOMAIN_ID_UNAVAILABLE"),
            })?;
            bindings.push(DomainBinding::new(kind, uuid, body).map_err(|_| mismatch())?);
        }
        let bindings: [DomainBinding; 6] = bindings.try_into().map_err(|_| mismatch())?;
        residency_canonical_bytes(&bindings).map_err(|_| mismatch())?;
        self.source_full_checkpoint()?;
        Ok(bindings)
    }
}
