//! ELIOT Search technical control state: pure reference semantics and a concrete
//! disk-backed journal. Neither implementation owns a searchable corpus.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

// Keep the original model and its existing public API intact. The persistent
// implementation does real I/O and must never be substituted by this model.
mod reference;
mod persistent;

pub use reference::*;
pub use persistent::PersistentControlJournal;
