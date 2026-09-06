//! ELIOT Search technical control state: pure reference semantics and a concrete
//! disk-backed journal. Neither implementation owns a searchable corpus.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

// Keep reference transitions available as the model, not as a disk fallback.
mod reference;
mod persistent;
mod snapshot_guard;
mod conditions;

pub use reference::*;
pub use persistent::PublicationSuccessor;
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
