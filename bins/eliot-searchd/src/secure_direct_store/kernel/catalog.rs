//! Source-catalog mutation and retained-revision preparation entrypoints.

#[cfg(test)]
use std::path::Path;

#[cfg(test)]
use zeroize::Zeroizing;

#[cfg(test)]
use super::super::preparation_store;
use super::DirectStore;
#[cfg(test)]
use crate::plaintext_direct_store::IndexedSource;
#[cfg(test)]
use crate::plaintext_direct_store::RevisionMetadata;
use crate::plaintext_direct_store::SourceSummary;

impl DirectStore {
    /// Stable namespace identity retained with the data root.
    pub(crate) fn namespace_id(&self) -> String {
        self.inner.namespace_id()
    }

    /// Revision bytes and saved preparation both precede source publication.
    #[cfg(test)]
    pub(crate) fn index_file(&mut self, path: &Path) -> Result<IndexedSource, String> {
        let namespace = self.inner.namespace_id();
        let root = &self.root;
        let protector = &self.protector;
        self.inner
            .index_file_with_writer(path, &mut |_, source, bytes| {
                preparation_store::persist_source(root, protector, &namespace, source, bytes)
            })
    }

    /// Every batch member crosses the same revision/preparation barrier.
    #[cfg(test)]
    pub(crate) fn index_directory(
        &mut self,
        directory: &Path,
    ) -> Result<Vec<IndexedSource>, String> {
        let namespace = self.inner.namespace_id();
        let root = &self.root;
        let protector = &self.protector;
        self.inner
            .index_directory_with_writer(directory, &mut |_, source, bytes| {
                preparation_store::persist_source(root, protector, &namespace, source, bytes)
            })
    }

    /// Prepares a retained revision without rereading a current source path.
    #[cfg(test)]
    pub(crate) fn prepare_revision(
        &self,
        revision_id: &str,
    ) -> Result<Option<&'static str>, String> {
        crate::catalog_presence::require_existing(&self.root)?;
        self.inner.verify_control()?;
        let metadata: RevisionMetadata = self
            .inner
            .retained_revision(revision_id)
            .ok_or_else(|| "DIRECT_REVISION_NOT_FOUND".to_owned())?;
        let bytes = Zeroizing::new(self.read_revision_detailed(&metadata)?);
        preparation_store::persist(
            &self.root,
            &self.protector,
            &self.inner.namespace_id(),
            &metadata,
            &bytes,
        )
    }

    /// Retires one source without deleting retained revision objects.
    #[cfg(test)]
    pub(crate) fn retire_source(&mut self, source_id: &str) -> Result<SourceSummary, String> {
        self.inner.retire_source(source_id)
    }

    /// Returns deterministic source summaries.
    pub(crate) fn list_sources(&self) -> Vec<SourceSummary> {
        self.inner.list_sources()
    }
}
