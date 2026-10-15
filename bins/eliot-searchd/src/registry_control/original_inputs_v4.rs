//! Pure v4 original-input resolution over one scoped record batch.

use super::probe_error;
use search_control_redb::{ControlKey, ControlRecordClass, ControlValue, JournalLimits,
    ScopedControlRecordsReadback};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_admission::{AcceptedCapabilities, AdmissionError, SectionPlatform};
use search_source_provenance::{ObservedOriginalInputPart, OriginalInputsLayoutV2,
    RetainedPolicyV2};
use search_source_registry::recovery::{BatchCodecV4Error,
    OriginalInputResolver as OriginalInputResolverTrait};
use std::{collections::BTreeMap, sync::Arc};

/// In-memory v4 resolver over records already read from one coherent journal view.
///
/// This type owns no journal, native handle, source body, current grant or I/O
/// path. The caller supplies the actual platform, capabilities and checkpoint.
pub(crate) struct OriginalInputResolver<'a> {
    records: &'a [(ControlKey, ControlValue)],
    limits: JournalLimits,
    platform: SectionPlatform,
    capabilities: AcceptedCapabilities,
    policy_cache: BTreeMap<Vec<u8>, Arc<RetainedPolicyV2>>,
}

impl<'a> OriginalInputResolver<'a> {
    pub(crate) fn new(
        readback: &'a ScopedControlRecordsReadback,
        limits: JournalLimits,
        platform: &SectionPlatform,
        capabilities: &AcceptedCapabilities,
    ) -> Self {
        Self {
            records: readback.records(),
            limits,
            platform: platform.clone(),
            capabilities: capabilities.clone(),
            policy_cache: BTreeMap::new(),
        }
    }

    fn parts(
        &self,
        reference: &[u8],
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Vec<ObservedOriginalInputPart<'_>>, BatchCodecV4Error> {
        let keys = OriginalInputsLayoutV2::lookup_keys(reference, probe)
            .map_err(BatchCodecV4Error::Admission)?;
        let mut parts = Vec::with_capacity(keys.len());
        for key in keys {
            probe().map_err(checkpoint_error)?;
            let key = ControlKey::new(key, self.limits)
                .map_err(|_| admission_error(AdmissionError::ReceiptMismatch))?;
            let index = self
                .records
                .binary_search_by(|(candidate, _)| candidate.cmp(&key))
                .map_err(|_| admission_error(AdmissionError::ReceiptMismatch))?;
            let (actual_key, value) = &self.records[index];
            if value.class() != ControlRecordClass::State {
                return Err(admission_error(AdmissionError::ReceiptMismatch));
            }
            parts.push(ObservedOriginalInputPart {
                key: actual_key.as_bytes(),
                value: value.as_bytes(),
            });
        }
        probe().map_err(checkpoint_error)?;
        Ok(parts)
    }
}

impl OriginalInputResolverTrait for OriginalInputResolver<'_> {
    fn resolve_policy(
        &mut self,
        policy_reference: &[u8],
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Arc<RetainedPolicyV2>, BatchCodecV4Error> {
        probe().map_err(checkpoint_error)?;
        if let Some(policy) = self.policy_cache.get(policy_reference) {
            return Ok(Arc::clone(policy));
        }
        let parts = self.parts(policy_reference, probe)?;
        let mut original_probe = || probe();
        let policy = Arc::new(
            OriginalInputsLayoutV2::restore_policy(
                policy_reference,
                &parts,
                &self.platform,
                &self.capabilities,
                &mut original_probe,
            )
            .map_err(BatchCodecV4Error::Admission)?,
        );
        self.policy_cache
            .insert(policy_reference.to_vec(), Arc::clone(&policy));
        Ok(policy)
    }

    fn resolve_admission(
        &mut self,
        admission_reference: &[u8],
        policy: Arc<RetainedPolicyV2>,
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<search_source_provenance::OriginalAdmissionV2, BatchCodecV4Error> {
        let parts = self.parts(admission_reference, probe)?;
        let mut original_probe = || probe();
        OriginalInputsLayoutV2::restore_admission(admission_reference, &parts, policy, &mut original_probe)
            .map_err(BatchCodecV4Error::Admission)
    }
}

const fn checkpoint_error(error: CheckpointFailureV2) -> BatchCodecV4Error {
    BatchCodecV4Error::Admission(probe_error(error))
}

const fn admission_error(error: AdmissionError) -> BatchCodecV4Error {
    BatchCodecV4Error::Admission(error)
}
