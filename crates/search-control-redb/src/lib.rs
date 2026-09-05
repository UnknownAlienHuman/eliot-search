//! ELIOT Search technical control state: pure reference semantics and a concrete
//! disk-backed journal. Neither implementation owns a searchable corpus.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

// Keep reference transitions available as the model, not as a disk fallback.
mod reference;
mod persistent;
mod snapshot_guard;

pub use reference::*;
pub use persistent::PersistentControlJournal;
// Explicit export supersedes the unfenced publisher from the reference glob.
// Existing callers, including PersistentControlJournal, use this public boundary.
pub use snapshot_guard::ControlSnapshotPublisher;
