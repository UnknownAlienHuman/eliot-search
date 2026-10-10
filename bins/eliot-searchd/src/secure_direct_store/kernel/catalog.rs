//! Source-catalog mutation and retained-revision preparation entrypoints.

use std::path::Path;

use zeroize::Zeroizing;

use super::super::preparation_store;
use super::DirectStore;
use crate::plaintext_direct_store::{IndexedSource, RevisionMetadata, SourceSummary};

impl DirectStore {
    /// Stable namespace identity retained with the data root.
    pub(crate) fn namespace_id(&self) -> String {
        self.inner.namespace_id()
    }

    /// Revision bytes and saved preparation both precede source publication.
    pub(crate) fn index_file(&mut self, path: &Path) -> Result<IndexedSource, String> {
        self.check_operation()?;
        let namespace = self.inner.namespace_id();
        let root = &self.root;
        let protector = &self.protector;
        let indexed = self
            .inner
            .index_file_with_writer(path, &mut |store, source, bytes| {
                store.check_operation()?;
                preparation_store::persist_source(root, protector, &namespace, source, bytes)?;
                store.check_operation()
            })?;
        self.check_operation()?;
        Ok(indexed)
    }

    /// Every batch member crosses the same revision/preparation barrier.
    pub(crate) fn index_directory(
        &mut self,
        directory: &Path,
    ) -> Result<Vec<IndexedSource>, String> {
        self.check_operation()?;
        let namespace = self.inner.namespace_id();
        let root = &self.root;
        let protector = &self.protector;
        let indexed =
            self.inner
                .index_directory_with_writer(directory, &mut |store, source, bytes| {
                    store.check_operation()?;
                    preparation_store::persist_source(root, protector, &namespace, source, bytes)?;
                    store.check_operation()
                })?;
        self.check_operation()?;
        Ok(indexed)
    }

    /// Prepares a retained revision without rereading a current source path.
    pub(crate) fn prepare_revision(
        &self,
        revision_id: &str,
    ) -> Result<Option<&'static str>, String> {
        self.check_operation()?;
        crate::catalog_presence::require_existing(&self.root)?;
        self.inner.verify_control()?;
        self.check_operation()?;
        let metadata: RevisionMetadata = self
            .inner
            .retained_revision(revision_id)
            .ok_or_else(|| "DIRECT_REVISION_NOT_FOUND".to_owned())?;
        self.check_operation()?;
        let bytes = Zeroizing::new(self.read_revision_detailed(&metadata)?);
        self.check_operation()?;
        let gap = preparation_store::persist(
            &self.root,
            &self.protector,
            &self.inner.namespace_id(),
            &metadata,
            &bytes,
        )?;
        self.check_operation()?;
        Ok(gap)
    }

    /// Retires one source without deleting retained revision objects.
    pub(crate) fn retire_source(&mut self, source_id: &str) -> Result<SourceSummary, String> {
        self.check_operation()?;
        let source = self.inner.retire_source(source_id)?;
        self.check_operation()?;
        Ok(source)
    }

    /// Returns deterministic source summaries.
    pub(crate) fn list_sources(&self) -> Vec<SourceSummary> {
        self.inner.list_sources()
    }
}
