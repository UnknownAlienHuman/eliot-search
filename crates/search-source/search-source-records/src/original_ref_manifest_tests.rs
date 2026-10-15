use super::{original, receipt, registered};
use crate::batch::{
    BatchCodecV4Error, DecodedBatch, OriginalInputResolver, RegistryStateDelta,
    RegistryStateRecord, required_original_refs_from_batch_v4,
    required_original_refs_from_state_v4,
};
use crate::error::{DEFAULT_REGISTRY_LIMITS, JournalEntryKind, RegistryJournalEntry};
use search_contracts::{Blake3Digest32, OpaqueId};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_admission::{
    AcceptedCapabilities, AdmissionError, BASELINE_PROFILE, SectionPlatform,
};
use search_source_provenance::{OriginalAdmissionV2, RetainedPolicyV2};
use std::collections::BTreeSet;
use std::sync::Arc;

struct Resolver {
    policy_bytes: Vec<u8>,
    admission_bytes: Vec<u8>,
    policy_reference: Vec<u8>,
    admission_reference: Vec<u8>,
}

impl Resolver {
    fn new(
        original: &OriginalAdmissionV2,
        permanent: &crate::provenance::PermanentSourceAdmissionV2,
    ) -> Self {
        Self {
            policy_bytes: original.policy().encoded_inputs().to_vec(),
            admission_bytes: original.encoded_source_evidence().to_vec(),
            policy_reference: permanent.policy_reference().to_vec(),
            admission_reference: permanent.admission_reference().to_vec(),
        }
    }
}

impl OriginalInputResolver for Resolver {
    fn resolve_policy(
        &mut self,
        reference: &[u8],
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Arc<RetainedPolicyV2>, BatchCodecV4Error> {
        if reference != self.policy_reference {
            return Err(BatchCodecV4Error::Admission(
                AdmissionError::ReceiptMismatch,
            ));
        }
        let mut forward = || probe();
        RetainedPolicyV2::restore(
            &self.policy_bytes,
            &SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            &AcceptedCapabilities {
                profiles: vec![BASELINE_PROFILE.into()],
            },
            &mut forward,
        )
        .map(Arc::new)
        .map_err(BatchCodecV4Error::Admission)
    }

    fn resolve_admission(
        &mut self,
        reference: &[u8],
        policy: Arc<RetainedPolicyV2>,
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<OriginalAdmissionV2, BatchCodecV4Error> {
        if reference != self.admission_reference {
            return Err(BatchCodecV4Error::Admission(
                AdmissionError::ReceiptMismatch,
            ));
        }
        let mut forward = || probe();
        OriginalAdmissionV2::restore(policy, &self.admission_bytes, &mut forward)
            .map_err(BatchCodecV4Error::Admission)
    }
}

fn expected_refs(permanent: &crate::provenance::PermanentSourceAdmissionV2) -> BTreeSet<Vec<u8>> {
    [
        permanent.policy_reference().to_vec(),
        permanent.admission_reference().to_vec(),
    ]
    .into_iter()
    .collect()
}

#[test]
fn state_manifest_selects_and_deduplicates_only_permanent_refs() {
    let original = original();
    let source = registered(original.clone());
    let permanent = source.permanent_admission().expect("permanent").clone();
    let state = RegistryStateRecord::Source(source);
    let bytes = state
        .encode_with_profile("eliot.registry-state.v4")
        .expect("v4 state");
    let refs = required_original_refs_from_state_v4(&bytes, &mut || Ok(())).expect("refs");
    assert_eq!(
        refs.into_iter().collect::<BTreeSet<_>>(),
        expected_refs(&permanent)
    );
}

#[test]
fn batch_manifest_reads_before_and_after_through_real_archived_refs() {
    let original = original();
    let state = RegistryStateRecord::Source(registered(original.clone()));
    let permanent = match &state {
        RegistryStateRecord::Source(source) => {
            source.permanent_admission().expect("permanent").clone()
        }
        _ => unreachable!(),
    };
    let delta = RegistryStateDelta::new(state.key(), Some(state.clone()), state).expect("delta");
    let entry = RegistryJournalEntry::new(
        OpaqueId::new("manifest:v4").expect("operation"),
        Blake3Digest32::from_bytes([7; 32]),
        0,
        1,
        JournalEntryKind::BatchApply,
        receipt("manifest:v4"),
    );
    let mut resolver = Resolver::new(&original, &permanent);
    let batch = DecodedBatch::from_observed_parts_v4(
        entry,
        Blake3Digest32::from_bytes([8; 32]),
        1,
        vec![delta],
        DEFAULT_REGISTRY_LIMITS,
        &mut resolver,
        &mut || Ok(()),
    )
    .expect("batch");
    let refs =
        required_original_refs_from_batch_v4(&batch.encode().expect("encode"), &mut || Ok(()))
            .expect("refs");
    assert_eq!(
        refs.into_iter().collect::<BTreeSet<_>>(),
        expected_refs(&permanent)
    );
}
