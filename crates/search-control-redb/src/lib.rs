//! ELIOT Search technical control state: pure reference semantics and a concrete
//! disk-backed journal. Neither implementation owns a searchable corpus.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

// Keep reference transitions available as the model, not as a disk fallback.
mod reference;
mod identity;
mod persistent;
mod snapshot_guard;
mod conditions;
pub mod revision_dek;
pub mod source_retention;
pub use revision_dek::{RevisionDekCommand, REVISION_DEK_INTENT_KEY, REVISION_DEK_CANDIDATE_KEY, REVISION_DEK_CURRENT_KEY};
pub use revision_dek::{RevisionDekSelectionCommand, REVISION_DEK_APPLICATION_INTENT_KEY};
const fn has_visibility_schema(version: u32) -> bool {
    matches!(version, 4 | identity::REGISTRY_JOURNAL_SCHEMA_VERSION)
}
const fn has_intent_schema(version: u32) -> bool {
    version == 2 || has_visibility_schema(version)
}
pub mod policy_codec;
pub mod access_policy;
pub mod provider_authority;
pub mod security_restriction;
pub mod publication_codec;
pub mod migration;

pub use reference::*;
pub use identity::*;
pub use persistent::PublicationSuccessor;
pub use persistent::{
    AbortResolutionMetadataV1, PublicationAbortFinalizationMetadataV1,
    PublicationAbortFinalizationRecordV1, PublicationIntentBindingMetadataV1,
    PublicationIntentBindingRecordV1,
};
pub use persistent::{IndexedProfileAdoption, JournalOperatingProfile, JournalProfileInitialization};
pub use persistent::{OwnerBoundSourceStage, OwnerSourceMappingReadback, OwnerSourceMappedReceipt};
pub use persistent::{OwnerInventoryStage, OwnerInventoryReadback, OwnerInventoryMappedReceipt};
pub use persistent::{OwnerContentStage, OwnerContentReadback, OwnerContentMappedReceipt};
pub use persistent::{MigrationBootstrap, MigrationBootstrapBinding, MigrationBootstrapFailure,
    MigrationBootstrapIdentity, MigrationBootstrapIdentityProfile, MigrationBootstrapOutput,
    MigrationBootstrapPlatform, MigrationBootstrapReceipt, MigrationBootstrapWork};
pub use persistent::{BoundControlJournal, ControlPortCommand, ControlPortError,
    ScopedControlRecordsReadback, control_mutation_identity};
pub use persistent::{PublicationProjectionSnapshot, PublicationRecoveryProjectionSnapshot};
pub use persistent::{PreparedArtifactSetMetadataV1, PreparedArtifactSetRecordV1,
    PreparedArtifactSetReadbackV1, MAX_PREPARED_ARTIFACT_SET_REFERENCE_BYTES};
pub use persistent::{PreparationInputMetadataV1, PreparationInputReadbackV1,
    PreparationInputRecordV1, MAX_PREPARATION_INPUT_POLICY_BYTES,
    MAX_PREPARATION_INPUT_RECORD_BYTES, MAX_PREPARATION_INPUT_RESIDENCY_BYTES};
pub use persistent::{registry_operation_value_len, RegistryStateFamily, RegistryStateKey, RegistryStateChange,
    RegistryMutationRequest, RegistryOperationEnvelope, RegistryStoredState,
    RegistryReadSnapshot, RegistryCommitReadback, RegistryTransactionReadback,
    RegistryOperationLookup, RegistryOperationObservation};
pub use conditions::{ConditionalControlMutation, ControlRecordCondition};
pub use persistent::{ControlCallError, ControlInterruption, PersistentControlJournal,
    ControlQuarantineReason, ControlQuarantineReceipt, ControlQuarantineRequest,
    ControlStoreHealth, JournalHealthState, JournalWriteCounters, SnapshotHealthState,
    PublicationIntentHead, PublicationIntentUpdate, PUBLICATION_INTENT_SCHEMA_VERSION,
    PublicationManifestChange, PublicationReadbackEvidence, PublicationSourceShadow,
    PublicationVisibilityState, VisibleEpochCommit, PUBLICATION_VISIBILITY_SCHEMA_VERSION};
// Explicit export supersedes the unfenced publisher from the reference glob.
// Existing callers, including PersistentControlJournal, use this public boundary.
pub use snapshot_guard::ControlSnapshotPublisher;

/// Coherent disk recovery inputs, not permission to serve or resume publication.
///
/// Construction is restricted to a verified schema-3 journal read. A retained
/// ABORTED intent consumes its epoch but does not prove external compensation.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationRecoveryCheckpoint {
    pub(crate) identity: JournalIdentity,
    pub(crate) generation: u64,
    pub(crate) visibility: PublicationVisibilityState,
    pub(crate) intent: Option<search_contracts::PublicationIntent>,
    pub(crate) original_binding: Option<PublicationIntentBindingRecordV1>,
    pub(crate) abort_finalization: Option<PublicationAbortFinalizationRecordV1>,
    pub(crate) publication_receipt: Option<search_contracts::PublicationReceipt>,
}

impl PublicationRecoveryCheckpoint {
    /// Exact installation/root/schema/owner identity observed on disk.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }
    /// Generation shared by every returned record and the checked receipt ledger.
    #[must_use]
    pub const fn generation(&self) -> u64 { self.generation }
    /// Actual committed visibility, route and guards from that same read.
    #[must_use]
    pub const fn visibility(&self) -> &PublicationVisibilityState { &self.visibility }
    /// Last retained intent, including unresolved and aborted work.
    #[must_use]
    pub const fn intent(&self) -> Option<&search_contracts::PublicationIntent> { self.intent.as_ref() }
    /// Original abort request metadata retained in the same verified journal read.
    /// Large source-backed bindings are intentionally absent and must be rehydrated.
    #[must_use]
    pub const fn abort_finalization(&self) -> Option<&PublicationAbortFinalizationRecordV1> {
        self.abort_finalization.as_ref()
    }
    /// Original operation/context coordinates retained atomically with the intent.
    #[must_use]
    pub const fn original_binding(&self) -> Option<&PublicationIntentBindingRecordV1> {
        self.original_binding.as_ref()
    }
    /// Actual latest publication receipt named by the same visibility record.
    #[must_use]
    pub const fn publication_receipt(&self) -> Option<&search_contracts::PublicationReceipt> {
        self.publication_receipt.as_ref()
    }
    /// Consumed floor from the retained intent, not guessed from `VisibleEpoch`.
    /// Intent absence is accepted only after checking its never-written history.
    #[must_use]
    pub fn last_reserved_epoch(&self) -> search_contracts::Epoch {
        self.intent.as_ref().map_or(self.visibility.visible_epoch, |intent| intent.target_epoch)
    }
}

impl std::fmt::Debug for PublicationRecoveryCheckpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicationRecoveryCheckpoint")
            .field("generation", &self.generation)
            .field("visible_epoch", &self.visibility.visible_epoch)
            .field("last_reserved_epoch", &self.last_reserved_epoch())
            .field("intent_state", &self.intent.as_ref().map(|intent| intent.state))
            .field("has_abort_finalization", &self.abort_finalization.is_some())
            .field("has_original_binding", &self.original_binding.is_some())
            .finish_non_exhaustive()
    }
}
