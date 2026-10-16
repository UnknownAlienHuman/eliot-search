//! Explicit local-owner retention policy under the held DIRECT root.

use std::collections::BTreeSet;
use std::time::Instant;

use crate::access_composition::{HANDLE_SECURITY_DEPENDENT, NativeSecurityBinding,
    NativeSecurityDomain, NativeSecurityError, SecurityInvalidationRegistry};
use search_access::{AccessCheckpoint, AccessError, RequestSecurityFence};
use search_contracts::{self as c, CanonicalValue as V};
use search_control_redb::{ControlError, ControlRecordCondition, ControlSnapshotPublisher,
    MigrationBootstrapWork as W, PersistentControlJournal, policy_codec::AccessPolicyRecord,
    security_restriction::SecurityPolicyState};
use search_handles::{HandlePolicy, HandleStore};
use search_source_registry::api::{CommittedNativeNamespaceV2, NamespaceCutoverState, SourceRegistry};

use super::{FreshRootContext, FreshRootError, RootRecord, array, bytes, text};

fn refused() -> NativeSecurityError { AccessError::SecurityFailClosed.into() }
fn native_error(error: FreshRootError) -> NativeSecurityError {
    match error {
        FreshRootError::Native(first) => NativeSecurityError::OriginalContext {
            control: ControlError::ReadCancelled, first: Some(first),
        },
        _ => refused(),
    }
}

/// Operation-local owner, not an access grant or an independently mutable store.
pub(crate) struct RetentionSecurity {
    domain: NativeSecurityDomain,
    fence: RequestSecurityFence,
    conditions: [ControlRecordCondition; 2],
    policy_body: Vec<u8>,
    user: [u8; 32],
}

impl RetentionSecurity {
    pub(crate) fn checkpoint(&self) -> Result<(), NativeSecurityError> {
        self.domain.with_live_checkpoint(&self.fence, AccessCheckpoint::BeforeSourceReadback, |_| ())?;
        Ok(())
    }
    pub(crate) fn conditions(&self) -> &[ControlRecordCondition; 2] { &self.conditions }
    pub(crate) fn policy_body(&self) -> &[u8] { &self.policy_body }
    pub(crate) const fn user_scope(&self) -> [u8; 32] { self.user }
}

impl FreshRootContext<'_, '_> {
    fn retention_user_scope(&self) -> Result<[u8; 32], NativeSecurityError> {
        // The complete retention-security phase has full Native checks on
        // both sides. Keep the original poll and the real current-user probe.
        self.source_checkpoint().map_err(native_error)?;
        self.operation.borrow_mut().with_binding(|binding, request, work, _| {
            binding.storage_acl.as_ref().ok_or(search_runtime_owner::OwnerError::OwnerStorageAclUnqualified)?
                .current_user_scope(request, work)
        }).map_err(|first| NativeSecurityError::OriginalContext {
            control: ControlError::ReadCancelled, first: Some(first),
        })
    }

    /// Establishes real C18 state before any Source body or revision key is used.
    /// Only a fresh C03 namespace creation may initialize empty restrictions.
    /// Existing namespaces must have a coherent durable C18 state already.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn retention_security(&self, registry: &SourceRegistry, root: &RootRecord,
        namespace: &NamespaceCutoverState, creation: Option<&CommittedNativeNamespaceV2>,
        operation: c::OpaqueId, journal: &mut PersistentControlJournal,
        publisher: &mut ControlSnapshotPublisher, deadline: Instant,
    ) -> Result<RetentionSecurity, NativeSecurityError> {
        let work = |unit| self.work(unit);
        self.source_full_checkpoint().map_err(native_error)?;
        let actual = self.source_owner_scope().map_err(native_error)?;
        let facts = self.retained_root_facts().map_err(native_error)?;
        let user = self.retention_user_scope()?;
        let ownership = &namespace.ownership;
        let metadata = namespace.permanent_metadata().ok_or_else(refused)?;
        if ownership.status != c::NamespaceOwnershipStatus::Active || namespace.fenced
            || namespace.preparation.is_some() || namespace.activation.is_some()
            || ownership.owner_system_id != actual.system_id
            || ownership.owner_installation_incarnation_id != actual.installation_incarnation
            || metadata.root_binding_id() != root.root_binding_id
            || root.status != search_source_registry::root::RootStatus::Active
            || root.policy_revision != metadata.policy().compiled().revision()
            || root.policy_fingerprint.as_bytes() != metadata.policy().fingerprint().as_bytes()
        { return Err(refused()); }
        let retained = metadata.policy().encoded_inputs();
        let preparation = retained.len().checked_mul(16).and_then(|n| n.checked_add(8192))
            .ok_or(ControlError::BudgetExceeded)?;
        work(W::Allocate(preparation)).map_err(|first| NativeSecurityError::OriginalContext {
            control: ControlError::BudgetExceeded, first: Some(first),
        })?;
        work(W::Process(preparation)).map_err(|first| NativeSecurityError::OriginalContext {
            control: ControlError::BudgetExceeded, first: Some(first),
        })?;
        let policy_value = array(vec![text("native-owner-confidential-retention/v1").map_err(native_error)?,
            bytes(facts.data_root.as_bytes()).map_err(native_error)?,
            bytes(facts.installation.as_bytes()).map_err(native_error)?,
            bytes(facts.incarnation.as_bytes()).map_err(native_error)?,
            bytes(root.root_binding_id.as_bytes()).map_err(native_error)?,
            bytes(ownership.source_namespace_id.as_bytes()).map_err(native_error)?,
            bytes(ownership.source_owner_generation.as_bytes()).map_err(native_error)?,
            bytes(&user).map_err(native_error)?, bytes(retained).map_err(native_error)?,
            array(vec![text("retain").map_err(native_error)?, text("reopen").map_err(native_error)?,
                text("release").map_err(native_error)?, text("whole-namespace-purge").map_err(native_error)?])
                .map_err(native_error)?,
            text("Confidential").map_err(native_error)?,
            text("owner-SYSTEM-Administrators-private-storage/v1").map_err(native_error)?,
            text("until-explicit-release;purge-dominates;no-cross-domain-key-reuse/v1").map_err(native_error)?,
        ]).map_err(native_error)?;
        let policy_body = c::to_canonical_cbor(&policy_value).map_err(|_| refused())?.into_vec();
        if policy_body.len() > 8192 { return Err(refused()); }
        let policy_digest = c::blake3_raw(&c::CanonicalDigestDomain::parse(
            "eliot/raw/native-retention-policy/v1").map_err(|_| refused())?, &policy_body,
            c::DigestInputLimit::new(8192).map_err(|_| refused())?).map_err(|_| refused())?;
        let domain = c::OpaqueRef::new(format!("native-retention-policy-v1:{policy_digest}"))
            .map_err(|_| refused())?;
        let dependents = c::BoundedSet::from_items([c::OpaqueId::new(HANDLE_SECURITY_DEPENDENT)
            .map_err(|_| refused())?]).map_err(|_| refused())?;
        let binding = NativeSecurityBinding { namespace: ownership.source_namespace_id,
            source_owner: ownership.source_owner_generation, domain: domain.clone(), dependents };
        let (read, _) = journal.observe_security_restriction_with_original_context(binding.namespace, deadline, &work)
            .map_err(|error| NativeSecurityError::OriginalContext {
                control: error.control, first: error.first_context,
            })?;
        if read.mutation().is_none() {
            let proof = creation.ok_or_else(refused)?;
            // A historical acknowledgment cannot turn missing policy into empty
            // restrictions. An uncertain prior initialization stays fenced.
            if proof.replayed() || proof.namespace() != namespace
                || registry.sources().keys().any(|identity| identity.source_namespace_id == binding.namespace)
            { return Err(refused()); }
            let snapshot = array(vec![text("native-retention-initial-live/v1").map_err(native_error)?,
                bytes(binding.namespace.as_bytes()).map_err(native_error)?,
                bytes(policy_digest.as_bytes()).map_err(native_error)?, V::U64(1),
                array(Vec::new()).map_err(native_error)?, array(Vec::new()).map_err(native_error)?, V::Bool(false)])
                .map_err(native_error)?;
            let snapshot_digest = c::blake3_canonical(&c::CanonicalDigestDomain::parse(
                "eliot/cbor/native-retention-initial-live/v1").map_err(|_| refused())?, &snapshot,
                c::DigestInputLimit::new(8192).map_err(|_| refused())?).map_err(|_| refused())?;
            let state = SecurityPolicyState { policy: AccessPolicyRecord {
                namespace_id: binding.namespace, owner_generation: binding.source_owner,
                policy_revision: c::AccessPolicyRevision::new(root.policy_revision.get()),
                live_deny_generation: 1, shadow_fence_revision: c::ShadowFenceRevision::new(1),
                purge_fence_revision: c::PurgeFenceRevision::new(1), policy_digest,
            }, security_domain_ref: domain, snapshot_digest,
                denied_memberships: c::BoundedSet::empty(), purged_memberships: c::BoundedSet::empty(), fail_closed: false };
            let mutation = read.initialize(operation, snapshot_digest, state, binding.dependents.clone())?;
            journal.commit_security_restriction_with_original_context(&mutation, deadline, &work)
                .map_err(|error| NativeSecurityError::OriginalContext {
                    control: error.control, first: error.first_context,
                })?;
        }
        let (current, committed) = journal.observe_security_restriction_with_original_context(binding.namespace, deadline, &work)
            .map_err(|error| NativeSecurityError::OriginalContext { control: error.control, first: error.first_context })?;
        let stored = current.mutation().ok_or_else(refused)?;
        let state = stored.replacement();
        if state.policy.policy_digest != policy_digest || state.security_domain_ref != binding.domain
            || state.policy.namespace_id != binding.namespace || state.policy.owner_generation != binding.source_owner
            || state.fail_closed || !state.denied_memberships.is_empty() || !state.purged_memberships.is_empty()
            || state.policy.shadow_fence_revision.get() != 1 || state.policy.purge_fence_revision.get() != 1
        { return Err(refused()); }
        let generation = state.policy.live_deny_generation;
        let committed = committed.ok_or_else(refused)?;
        current.confirm_current(&committed)?;
        work(W::Allocate(preparation)).map_err(|first| NativeSecurityError::OriginalContext {
            control: ControlError::BudgetExceeded, first: Some(first) })?;
        work(W::Process(preparation)).map_err(|first| NativeSecurityError::OriginalContext {
            control: ControlError::BudgetExceeded, first: Some(first) })?;
        let conditions = committed.current_conditions()?;
        // This one-shot owns the exclusive DIRECT root and has no serving
        // connection or surviving process-local handles. It still dispatches
        // the canonical owner pass; an acknowledging callback is not used.
        let mut handles = HandleStore::new(HandlePolicy::BASELINE).map_err(|_| refused())?;
        let mut owners = SecurityInvalidationRegistry::new(&binding)?;
        owners.register_handles(&mut handles)?;
        let domain = NativeSecurityDomain::restore_direct_with_original_context(binding.clone(),
            journal, publisher, &mut owners, self.context(), deadline, &work)?;
        self.source_full_checkpoint().map_err(native_error)?;
        let security = RetentionSecurity { domain, conditions, policy_body, user,
            fence: RequestSecurityFence { planned_generation: generation, memberships: BTreeSet::new() } };
        security.checkpoint()?;
        Ok(security)
    }
}
