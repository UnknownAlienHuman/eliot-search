//! Data-root opening and referenced-plaintext migration.

use std::fs;
use std::path::Path;

use zeroize::Zeroizing;

use super::super::{
    legacy_path, read_plaintext_path, remove_plaintext_after_readback, verify_revision_identity,
};
use super::{DirectStore, REVISION_DIRECTORY};
use crate::development::{DataRootGuard, InspectedDataRoot};
use crate::owner_composition::{InitializationRecovery, InitializingDataRoot};
use crate::plaintext_direct_store as plaintext;
use crate::plaintext_direct_store::RevisionMetadata;
use crate::revision_protection::RevisionProtector;
use crate::sha256;

enum InitializationBorrow<'a> {
    New(&'a InitializingDataRoot),
    Recovery(&'a InitializationRecovery<'a>),
}

/// Initialization child exposes only an empty-layout proof, never source
/// mutation/search. Its credential and store close before owner finalization.
pub(crate) struct InitializationStore<'a> {
    inner: plaintext::DirectStore,
    _protector: RevisionProtector,
    cap: InitializationBorrow<'a>,
}

impl InitializationStore<'_> {
    pub(crate) fn verify_empty(&self) -> Result<(), String> {
        let (root, namespace) = match self.cap {
            InitializationBorrow::New(cap) => {
                cap.verify().map_err(|error| error.code().to_owned())?;
                (cap.canonical_root(), cap.namespace_id())
            }
            InitializationBorrow::Recovery(cap) => {
                cap.verify().map_err(|error| error.code().to_owned())?;
                (cap.canonical_root(), cap.namespace_id())
            }
        };
        self.inner.verify_control()?;
        if self.inner.namespace_id() != sha256::hex(&namespace)
            || self.inner.source_event_count() != 0
            || !self.inner.list_sources().is_empty()
            || self.inner.retained_revisions().next().is_some()
        {
            return Err("DATA_ROOT_INITIALIZATION_LAYOUT_CHANGED".to_owned());
        }
        // No content or protected-object conversion belongs to initialization.
        if fs::read_dir(root.join(REVISION_DIRECTORY))
            .map_err(|_| "DATA_ROOT_INITIALIZATION_LAYOUT_CHANGED".to_owned())?
            .next()
            .transpose()
            .map_err(|_| "DATA_ROOT_INITIALIZATION_LAYOUT_CHANGED".to_owned())?
            .is_some()
        {
            return Err("DATA_ROOT_INITIALIZATION_LAYOUT_CHANGED".to_owned());
        }
        Ok(())
    }
}

/// Existing metadata/read child, tied to the held inspection capability.
/// Construction never resolves a secret. Explicit source reads use only the
/// existing protector, with no public access to a mutable store.
pub(crate) struct ReadOnlyStore<'a> {
    cap: &'a InspectedDataRoot,
    inner: plaintext::DirectStore,
}

impl ReadOnlyStore<'_> {
    pub(crate) fn namespace_id(&self) -> String {
        self.inner.namespace_id()
    }
    pub(crate) fn list_sources(&self) -> Vec<plaintext::SourceSummary> {
        self.inner.list_sources()
    }

    /// Catalog metadata only; no source contents or credential resolution.
    pub(crate) fn verify_catalog(&self) -> Result<plaintext::StoreVerification, String> {
        self.cap.verify_existing()?;
        self.inner.check_operation()?;
        self.inner.verify_control()?;
        let sources = self.inner.list_sources();
        let verification = plaintext::StoreVerification {
            source_events: self.inner.source_event_count(),
            registered_sources: sources.len(),
            active_sources: sources.iter().filter(|source| source.active).count(),
            referenced_revisions: self.inner.retained_revisions().len(),
            verified_revisions: 0,
            total_revision_bytes: 0,
        };
        self.cap.verify_existing()?;
        Ok(verification)
    }

    pub(crate) fn verify(&self) -> Result<plaintext::StoreVerification, String> {
        self.source_view()?.verify()
    }
    pub(crate) fn search(
        &self,
        query: &str,
        ascii_insensitive: bool,
    ) -> Result<plaintext::StoreSearchResult, String> {
        self.source_view()?.search(query, ascii_insensitive)
    }
    pub(crate) fn read_revision_range(
        &self,
        revision: &str,
        start: u64,
        end: u64,
    ) -> Result<plaintext::RevisionSlice, String> {
        self.source_view()?
            .read_revision_range(revision, start, end)
    }

    fn source_view(&self) -> Result<DirectStore, String> {
        self.cap.verify_existing()?;
        let inner = plaintext::DirectStore::open_existing_legacy_read_only(self.cap)?;
        if inner.namespace_id() != self.inner.namespace_id()
            || inner.source_event_count() != self.inner.source_event_count()
            || inner.list_sources() != self.inner.list_sources()
        {
            return Err("DIRECT_EXISTING_CATALOG_CHANGED".to_owned());
        }
        let store = assemble_existing(self.cap.canonical_root(), inner)?;
        self.cap.verify_existing()?;
        Ok(store)
    }
}

/// Mutating child cannot outlive the native owner it borrows.
pub(crate) struct MutatingStore<'a> {
    store: DirectStore,
    _owner: &'a DataRootGuard,
}

impl core::ops::Deref for MutatingStore<'_> {
    type Target = DirectStore;
    fn deref(&self) -> &Self::Target {
        &self.store
    }
}
impl core::ops::DerefMut for MutatingStore<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.store
    }
}

impl MutatingStore<'_> {
    /// Per-command context borrowing the exact same live service owner.
    pub(crate) fn bind_service_command(
        &mut self,
        command: &crate::development::DataRootCommand<'_>,
    ) -> Result<(), String> {
        command.verify_owner(self._owner)?;
        self.store.inner.bind_operation(command.request())?;
        command.verify_owner(self._owner)
    }
}

fn assemble_existing(root: &Path, inner: plaintext::DirectStore) -> Result<DirectStore, String> {
    let namespace = sha256::decode_digest(&inner.namespace_id())
        .ok_or_else(|| "DIRECT_NAMESPACE_INVALID".to_owned())?;
    let protector = RevisionProtector::open_existing(namespace)?
        .ok_or_else(|| "DIRECT_REVISION_KEY_MISSING".to_owned())?;
    Ok(DirectStore {
        root: root.to_owned(),
        inner,
        protector,
    })
}

impl DirectStore {
    pub(crate) fn check_operation(&self) -> Result<(), String> {
        self.inner.check_operation()
    }

    pub(crate) fn operation_deadline(&self) -> Result<Option<std::time::Instant>, String> {
        self.check_operation()?;
        match self.inner.operation_request() {
            Ok(request) => Ok(Some(request.deadline())),
            #[cfg(test)]
            Err(_) => Ok(None),
            #[cfg(not(test))]
            Err(error) => Err(error),
        }
    }

    /// The only production layout/credential creation arm; no migration occurs.
    pub(crate) fn initialize_legacy_layout(
        cap: &InitializingDataRoot,
    ) -> Result<InitializationStore<'_>, String> {
        cap.verify().map_err(|error| error.code().to_owned())?;
        let inner = plaintext::DirectStore::initialize_legacy_layout(cap)?;
        let protector = RevisionProtector::open(
            cap.namespace_id(),
            &cap.canonical_root().join(REVISION_DIRECTORY),
        )?;
        let store = InitializationStore {
            inner,
            _protector: protector,
            cap: InitializationBorrow::New(cap),
        };
        store.verify_empty()?;
        Ok(store)
    }

    /// Named recovery never retries layout or credential creation.
    pub(crate) fn open_initialization_recovery<'a>(
        cap: &'a InitializationRecovery<'a>,
    ) -> Result<InitializationStore<'a>, String> {
        cap.verify().map_err(|error| error.code().to_owned())?;
        let inner = plaintext::DirectStore::open_initialization_recovery(cap)?;
        let protector = RevisionProtector::open_existing(cap.namespace_id())?
            .ok_or_else(|| "DIRECT_REVISION_KEY_MISSING".to_owned())?;
        let store = InitializationStore {
            inner,
            _protector: protector,
            cap: InitializationBorrow::Recovery(cap),
        };
        store.verify_empty()?;
        Ok(store)
    }

    /// Creates no state and resolves no secret; source reads are explicit.
    pub(crate) fn open_existing_read_only(
        cap: &InspectedDataRoot,
    ) -> Result<ReadOnlyStore<'_>, String> {
        cap.verify_existing()?;
        let inner = plaintext::DirectStore::open_existing_legacy_read_only(cap)?;
        Ok(ReadOnlyStore { cap, inner })
    }

    /// Borrows the one live native owner; never initializes or migrates.
    pub(crate) fn open_existing_mutating(
        owner: &DataRootGuard,
    ) -> Result<MutatingStore<'_>, String> {
        owner.verify_existing()?;
        let inner = plaintext::DirectStore::open_existing_legacy_mutating(owner)?;
        let store = assemble_existing(owner.canonical_root(), inner)?;
        owner.verify_existing()?;
        Ok(MutatingStore {
            store,
            _owner: owner,
        })
    }

    /// Opens the source catalog and recovers every referenced protected object.
    #[cfg(test)]
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

    #[cfg(test)]
    fn migrate_referenced_plaintext(&self) -> Result<(), String> {
        for metadata in self.inner.retained_revisions() {
            self.seal_revision(&metadata)?;
        }
        Ok(())
    }

    #[cfg(test)]
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
