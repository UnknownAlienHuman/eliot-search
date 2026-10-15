//! Profile-preserving Root composition over the existing retained-policy owner.
use super::{
    admission, array, bytes, c, hash, text, FreshRootContext, FreshRootError,
    PreparedFreshRoot, RequalifiedFreshRoot, SourceRegistry, Work, V,
};
use search_source_provenance::RetainedPolicyV2;
use std::{path::Path, sync::Arc};

pub(super) enum RootPolicy {
    V1(admission::CanonicalAdmissionPolicy),
    V2 {
        retained: Arc<RetainedPolicyV2>,
        reference: Arc<[u8]>,
    },
}

impl RootPolicy {
    pub(super) fn revision(&self) -> c::NonZeroRevision {
        match self {
            Self::V1(policy) => policy.revision(),
            Self::V2 { retained, .. } => retained.compiled().revision(),
        }
    }
    pub(super) fn fingerprint(&self) -> c::Blake3Digest32 {
        c::Blake3Digest32::from_stored_bytes(match self {
            Self::V1(policy) => *admission::policy_fingerprint(policy).as_bytes(),
            Self::V2 { retained, .. } => *retained.fingerprint().as_bytes(),
        })
    }
    pub(super) fn receipt_digests(
        &self,
        owner: &V,
        request: &V,
        native: V,
        intent: V,
        config: V,
        policy: V,
    ) -> Result<(c::Blake3Digest32, c::Blake3Digest32), FreshRootError> {
        if matches!(self, Self::V1(_)) {
            return super::receipt_digests(owner, request, native, intent, config, policy);
        }
        let fence = array(vec![
            V::U64(2),
            owner.clone(),
            request.clone(),
            native.clone(),
            intent.clone(),
        ])?;
        let proof = array(vec![
            V::U64(2),
            owner.clone(),
            request.clone(),
            native,
            intent,
            config,
            policy,
        ])?;
        Ok((
            hash("eliot/cbor/source-registry/root-owner-fence/v2", &fence)?,
            hash(
                "eliot/cbor/source-registry/root-permanent-policy/v2",
                &proof,
            )?,
        ))
    }
}

impl<'operation, 'owner> FreshRootContext<'operation, 'owner> {
    pub(crate) fn prepare_v2<'view>(
        &'view self,
        path: &Path,
        id: c::RootBindingId,
        operation: c::OpaqueId,
        expected: u64,
        retained: &Arc<RetainedPolicyV2>,
        reference: &Arc<[u8]>,
    ) -> Result<PreparedFreshRoot<'view, 'operation, 'owner>, FreshRootError> {
        self.checkpoint()?;
        self.charge(Work::Allocate(4096))?;
        let section = retained.compiled().section();
        let config = array(vec![
            V::U64(admission::SECTION_SCHEMA_REVISION),
            text(section.bootstrap_policy_profile())?,
            V::U64(section.max_file_bytes()),
            V::Bool(section.allow_generated()),
            V::Bool(section.allow_vendor()),
            V::Bool(section.allow_binary()),
        ])?;
        let policy = RootPolicy::V2 {
            retained: Arc::clone(retained),
            reference: Arc::clone(reference),
        };
        let proof = array(vec![
            V::U64(2),
            text(admission::policy_v2::POLICY_PROFILE)?,
            V::U64(policy.revision().get()),
            bytes(policy.fingerprint().as_bytes())?,
            bytes(reference)?,
        ])?;
        self.checkpoint()?;
        self.prepare_policy(path, id, operation, expected, policy, config, proof)
    }

    pub(crate) fn requalify_existing_v2<'view>(
        &'view self,
        path: &Path,
        id: c::RootBindingId,
        registry: &SourceRegistry,
        retained: &Arc<RetainedPolicyV2>,
        reference: &Arc<[u8]>,
    ) -> Result<RequalifiedFreshRoot<'view, 'operation, 'owner>, FreshRootError> {
        self.requalify_policy(
            path,
            id,
            registry,
            RootPolicy::V2 {
                retained: Arc::clone(retained),
                reference: Arc::clone(reference),
            },
        )
    }
}
