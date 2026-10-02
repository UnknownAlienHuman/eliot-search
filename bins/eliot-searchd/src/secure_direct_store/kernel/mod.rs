//! Revision-protected DIRECT store composition.

mod catalog;
mod lifecycle;
mod read;
mod search;

use core::fmt;
use std::path::PathBuf;

use crate::plaintext_direct_store as plaintext;
use crate::revision_protection::RevisionProtector;

pub(super) use search_revision_store::{
    LEGACY_REVISION_DIRECTORY as REVISION_DIRECTORY,
    LEGACY_REVISION_MAX_OBJECT_BYTES as MAX_REVISION_OBJECT_BYTES,
};

/// DIRECT catalog with a platform-specific prepublication revision writer.
pub struct DirectStore {
    pub(super) root: PathBuf,
    pub(super) inner: plaintext::DirectStore,
    pub(super) protector: RevisionProtector,
}

impl fmt::Debug for DirectStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectStore")
            .field("root", &self.root)
            .field("namespace_id", &self.inner.namespace_id())
            .field("protector", &self.protector)
            .field("revision_count", &self.inner.retained_revisions().len())
            .finish()
    }
}

pub(super) use read::verify_plaintext;

#[cfg(test)]
mod tests;
