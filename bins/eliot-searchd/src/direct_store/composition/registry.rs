//! Coherent transient registry view and one-shot DIRECT ingestion plan.

use std::collections::BTreeMap;
use std::path::Path;

use search_source_admission::{AdmissionOutcome, AdmissionReceipt};
use search_source_identity::LegacyDigestPriorIdentity;

use super::admission::{
    AdmissionPolicy, build_observation, issue_receipt, outcome, verify_receipt,
};
use super::identity;
use crate::sha256;

const MAX_PRIOR_SOURCES: usize = 2_000_000;
const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PriorSourceView {
    pub(crate) source_id: String,
    pub(crate) file_identity_digest: String,
    pub(crate) path_digest: String,
    pub(crate) revision_id: String,
    pub(crate) record_digest: String,
    pub(crate) is_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegistryView {
    by_id: BTreeMap<String, PriorSourceView>,
    policy_revision: u64,
    policy_fingerprint: String,
}

impl RegistryView {
    pub(crate) fn build(
        prior: Vec<PriorSourceView>,
        policy: &AdmissionPolicy,
    ) -> Result<Self, String> {
        if prior.len() > MAX_PRIOR_SOURCES {
            return Err("DIRECT_SOURCE_EVENT_LIMIT_EXCEEDED".to_owned());
        }
        let mut by_id = BTreeMap::new();
        for entry in prior {
            for digest in [
                entry.source_id.as_str(),
                entry.file_identity_digest.as_str(),
                entry.path_digest.as_str(),
                entry.revision_id.as_str(),
                entry.record_digest.as_str(),
            ] {
                if sha256::decode_digest(digest).is_none() {
                    return Err("ADMISSION_RECEIPT_MISMATCH".to_owned());
                }
            }
            if by_id.insert(entry.source_id.clone(), entry).is_some() {
                return Err("SOURCE_IDENTITY_CONFLICT".to_owned());
            }
        }
        Ok(Self {
            by_id,
            policy_revision: policy.revision(),
            policy_fingerprint: policy.fingerprint().to_owned(),
        })
    }

    pub(crate) fn get(&self, source_id: &str) -> Option<PriorSourceView> {
        self.by_id.get(source_id).cloned()
    }

    fn identity_candidates(&self) -> Vec<LegacyDigestPriorIdentity> {
        self.by_id
            .values()
            .map(|source| LegacyDigestPriorIdentity {
                source_id: source.source_id.clone(),
                stable_identity_digest: source.file_identity_digest.clone(),
            })
            .collect()
    }

    fn admit(
        &self,
        receipt: &AdmissionReceipt,
        policy: &AdmissionPolicy,
        observation: &search_source_admission::AdmissionObservation,
    ) -> Result<(), String> {
        verify_receipt(receipt, policy, observation)?;
        if receipt.policy_revision().get() != self.policy_revision
            || receipt.policy_fingerprint().to_hex() != self.policy_fingerprint
        {
            return Err("ADMISSION_RECEIPT_STALE".to_owned());
        }
        match outcome(receipt) {
            AdmissionOutcome::Allow => Ok(()),
            AdmissionOutcome::Deny => Err("SOURCE_ADMISSION_DENIED".to_owned()),
            AdmissionOutcome::ReviewRequired => {
                Err("SOURCE_ADMISSION_REVIEW_REQUIRED".to_owned())
            }
            AdmissionOutcome::Unsupported => Err("SOURCE_KIND_UNSUPPORTED".to_owned()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CanonicalPlan {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_snapshot(
    original_path: &Path,
    file_identity_digest: &str,
    identity_strength: &str,
    content_digest: &str,
    snapshot_bytes: &[u8],
    namespace_digest: &str,
    policy: &AdmissionPolicy,
    view: &RegistryView,
) -> Result<CanonicalPlan, String> {
    if snapshot_bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err("SOURCE_TOO_LARGE".to_owned());
    }

    let observation = build_observation(original_path, snapshot_bytes, policy)?;
    let receipt = issue_receipt(policy, &observation)?;
    view.admit(&receipt, policy, &observation)?;

    let source_id = identity::resolve_source_id(
        namespace_digest,
        file_identity_digest,
        identity_strength,
        &view.identity_candidates(),
    )?;
    let byte_length = u64::try_from(snapshot_bytes.len())
        .map_err(|_| "SOURCE_TOO_LARGE".to_owned())?;
    let revision_id =
        identity::derive_revision_id(&source_id, content_digest, byte_length)?;

    Ok(CanonicalPlan {
        source_id,
        revision_id,
    })
}
