//! ELIOT Search technical control state: pure reference semantics and a concrete
//! disk-backed journal. Neither implementation owns a searchable corpus.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

// Keep reference transitions available as the model, not as a disk fallback.
mod reference;
mod persistent;
mod snapshot_guard;
mod conditions;
pub mod migration;

pub use reference::*;
pub use persistent::PublicationSuccessor;
pub use persistent::{BoundControlJournal, ControlPortCommand, ControlPortError, control_mutation_identity};
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
            .finish_non_exhaustive()
    }
}
