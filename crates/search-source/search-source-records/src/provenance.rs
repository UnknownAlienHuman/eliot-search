//! Explicit historical-v1 and permanent-v2 Source evidence, without current authority.

use search_contracts::{NonZeroRevision, SourceOwnerGeneration};
use search_source_admission::original_envelope::OriginalAdmissionEnvelope;
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_admission::{AdmissionError, AdmissionReceipt};
use search_source_identity::SourceBinding;
use search_source_provenance::{OriginalAdmissionV2, OriginalInputsLayoutV2};

use crate::error::RegistryError;
use crate::source::AdmissionBindingProof;

/// Closed provenance alternatives. A version never establishes current admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceProvenance {
    /// Frozen v1 models; absent original inputs explicitly remain unavailable.
    HistoricalV1 {
        /// Original legacy receipt, never a permanent-v2 receipt.
        admission: AdmissionReceipt,
        /// Complete retained legacy envelope when available.
        original_admission: Option<OriginalAdmissionEnvelope>,
        /// Frozen legacy assignment metadata, including its historical flags.
        assignment: AdmissionBindingProof,
    },
    /// Complete v2 evidence and the exact fences observed at admission.
    PermanentV2(PermanentSourceAdmissionV2),
}

/// Retained permanent evidence and immutable admission-fence observations.
///
/// A semantic producer must independently validate the actual current Root,
/// namespace and identity assignment. These values provide no live grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermanentSourceAdmissionV2 {
    original: OriginalAdmissionV2,
    policy_reference: Vec<u8>,
    admission_reference: Vec<u8>,
    root_record_revision: NonZeroRevision,
    namespace_record_revision: NonZeroRevision,
    owner_generation: SourceOwnerGeneration,
}

impl PermanentSourceAdmissionV2 {
    /// Retains real owner-produced originals and computes their exact storage references.
    ///
    /// # Errors
    /// Preserves the original owner/checkpoint error without a partial result.
    pub fn new(
        original: OriginalAdmissionV2,
        root_record_revision: NonZeroRevision,
        namespace_record_revision: NonZeroRevision,
        owner_generation: SourceOwnerGeneration,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Self, AdmissionError> {
        let policy_reference = OriginalInputsLayoutV2::policy_reference(original.policy(), probe)?;
        let admission_reference = OriginalInputsLayoutV2::admission_reference(&original, probe)?;
        probe().map_err(|error| match error {
            CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
            CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
        })?;
        Ok(Self {
            original,
            policy_reference,
            admission_reference,
            root_record_revision,
            namespace_record_revision,
            owner_generation,
        })
    }

    /// Checks exact locator and byte-size binding; it grants no current access.
    pub fn validate_binding(&self, binding: &SourceBinding) -> Result<(), RegistryError> {
        let observation = self.original.observation();
        if observation.metadata().byte_size() != Some(binding.content_bytes())
            || observation.relative_locator().map(search_source_admission::observation_v2::AdmissionRelativeLocator::as_bytes)
                != Some(binding.relative_path().as_str().as_bytes())
            || self
                .owner_generation
                .as_bytes()
                .iter()
                .all(|value| *value == 0)
        {
            return Err(RegistryError::AdmissionBindingMismatch);
        }
        Ok(())
    }

    /// Complete original normalized policy, observation, decision and receipt.
    pub const fn original(&self) -> &OriginalAdmissionV2 {
        &self.original
    }
    /// Exact immutable policy archive reference; no source body is stored here.
    pub fn policy_reference(&self) -> &[u8] {
        &self.policy_reference
    }
    /// Exact immutable original-admission archive reference.
    pub fn admission_reference(&self) -> &[u8] {
        &self.admission_reference
    }
    /// Root record revision observed by the semantic admission operation.
    pub const fn root_record_revision(&self) -> NonZeroRevision {
        self.root_record_revision
    }
    /// Namespace record revision observed by the semantic admission operation.
    pub const fn namespace_record_revision(&self) -> NonZeroRevision {
        self.namespace_record_revision
    }
    /// Exact observed owner generation, never reconstructed from a fixed epoch.
    pub const fn owner_generation(&self) -> SourceOwnerGeneration {
        self.owner_generation
    }
}
