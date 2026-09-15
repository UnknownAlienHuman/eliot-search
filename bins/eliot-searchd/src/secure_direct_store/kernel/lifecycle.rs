//! Data-root opening and referenced-plaintext migration.

use std::fs;
use std::path::Path;

use zeroize::Zeroizing;

use super::{DirectStore, REVISION_DIRECTORY};
use super::super::{
    legacy_path, read_plaintext_path, remove_plaintext_after_readback,
    verify_revision_identity,
};
use crate::plaintext_direct_store as plaintext;
use crate::plaintext_direct_store::RevisionMetadata;
use crate::revision_protection::RevisionProtector;
use crate::sha256;

impl DirectStore {
    /// Opens the source catalog and recovers every referenced protected object.
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        crate::catalog_presence::check_before_open(root)?;
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| format!("DIRECT_ROOT_CANONICALIZE_ERROR:{error}"))?;
        let inner = plaintext::DirectStore::open(&canonical_root)?;
        let namespace_id = sha256::decode_digest(&inner.namespace_id())
            .ok_or_else(|| "DIRECT_NAMESPACE_INVALID".to_owned())?;
        let revision_root = canonical_root.join(REVISION_DIRECTORY);
        let protector = RevisionProtector::open(namespace_id, &revision_root)?;
        let store = Self {
            root: canonical_root,
            inner,
            protector,
        };
        if store.protector.encrypts_new_objects() {
            store.migrate_referenced_plaintext()?;
        }
        Ok(store)
    }

    fn migrate_referenced_plaintext(&self) -> Result<(), String> {
        for metadata in self.inner.retained_revisions() {
            self.seal_revision(&metadata)?;
        }
        Ok(())
    }

    fn seal_revision(&self, metadata: &RevisionMetadata) -> Result<(), String> {
        verify_revision_identity(metadata)?;
        let path = legacy_path(&self.root, &metadata.revision_id)?;
        if path.exists() {
            let plaintext = Zeroizing::new(read_plaintext_path(&path, metadata)?);
            super::super::revision_writer::persist_verified(
                &self.root,
                &self.protector,
                metadata,
                &plaintext,
            )?;
            remove_plaintext_after_readback(&path)
        } else {
            // Opening a protected revision never consults current-path bytes.
            let _verified = Zeroizing::new(self.read_revision_detailed(metadata)?);
            Ok(())
        }
    }
}
