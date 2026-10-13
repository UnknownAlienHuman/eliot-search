//! Typed read-only surface; no mutation method, inner escape or Deref.

use std::cell::{Cell, RefCell};

use search_runtime_owner::OwnerError;

use super::{DirectStore, RevisionSlice, SourceSummary, StoreSearchResult, StoreVerification, plaintext};
use crate::owner_composition::{NativeReadOperation, NativeSyncPreflightOperation};
use crate::revision_protection::RevisionProtector;

mod sealed {
    pub trait Sealed {}
}

/// The one owner-neutral replay of already-existing namespace and source-log
/// bytes. It parses namespace identity and replays the registry exactly as the
/// read path does; neither caller may initialize, migrate or repair anything.
fn replay_existing(
    root: &std::path::Path,
    namespace: &[u8],
    log: &[u8],
) -> Result<(plaintext::DirectStore, [u8; 32]), OwnerError> {
    replay_existing_checked(root, namespace, log, || Ok(()))
}

fn replay_existing_checked(
    root: &std::path::Path,
    namespace: &[u8],
    log: &[u8],
    checkpoint: impl FnMut() -> Result<(), String>,
) -> Result<(plaintext::DirectStore, [u8; 32]), OwnerError> {
    let store = plaintext::DirectStore::from_existing_bytes_checked(root, namespace, log, checkpoint)
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    let namespace_id =
        crate::sha256::decode_digest(&store.namespace_id())
            .ok_or(OwnerError::OwnerGuardMismatch)?;
    Ok((store, namespace_id))
}

/// Existing-only migration child. It exposes only staging into the caller's
/// qualified external output, never the store, source traversal or mutation.
pub struct ExistingMigrationStore {
    store: DirectStore,
    namespace: Vec<u8>,
    log: Vec<u8>,
}

impl ExistingMigrationStore {
    pub(crate) fn stage_mapping(
        &self,
        target: search_contracts::SourceNamespaceId,
        directory: &std::path::Path,
        operation: &mut NativeReadOperation<'_>,
        registration: Option<&[u8]>,
    ) -> Result<String, String> {
        operation.verify().map_err(owner_error)?;
        let binding = self.store.protector.initialization_key_binding().map_err(owner_error)?;
        if self.store.root != operation.root()
            || operation.selected_binding().is_some_and(|selected| binding != selected)
        {
            return Err(OwnerError::OwnerGuardMismatch.code().to_owned());
        }
        let result = {
            let inventory_owner = operation.migration_inventory_owner().map_err(owner_error)?;
            let context = super::migration_context::MigrationContext::new(operation);
            let input = super::migration_context::NativeMigrationInput {
                namespace: &self.namespace,
                log: &self.log,
                protector: &self.store.protector,
                context: &context,
                inventory_owner,
                registration,
            };
            let result = DirectStore::stage_mapping_artifact_existing(
                &self.store.inner, &self.store.root, target, directory, &input,
            )?;
            context.checkpoint()?;
            result
        };
        operation.verify().map_err(owner_error)?;
        Ok(result)
    }
}

impl core::fmt::Debug for ExistingMigrationStore {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ExistingMigrationStore(<opaque>)")
    }
}

pub fn open_existing_migration(
    operation: &mut NativeReadOperation<'_>,
) -> Result<ExistingMigrationStore, OwnerError> {
    operation.verify()?;
    let namespace = operation.namespace_bytes()?;
    let log = operation.catalog_bytes()?;
    let root = operation.root().to_owned();
    let (inner, namespace_id) = replay_existing_checked(&root, &namespace, &log, || {
        operation.migration_checkpoint().map_err(owner_error)
    })?;
    let selected = operation.selected_binding();
    let protector = operation.credential_call(|deadline, cancellation| {
        RevisionProtector::for_existing_credential(namespace_id, selected, deadline, cancellation)
    })?;
    operation.verify()?;
    Ok(ExistingMigrationStore {
        store: DirectStore { root, inner, protector },
        namespace,
        log,
    })
}

/// Private daemon read surface shared by compatibility callers and the new
/// owner-borrowed child. Implementations are closed inside this owner.
pub trait DirectRead: sealed::Sealed {
    fn namespace_id(&self) -> Result<String, String>;
    fn list_sources(&self) -> Result<Vec<SourceSummary>, String>;
    fn verify(&self) -> Result<StoreVerification, String>;
    fn search(&self, query: &str, insensitive: bool) -> Result<StoreSearchResult, String>;
    fn read_revision_range(
        &self,
        revision: &str,
        start: u64,
        end: u64,
    ) -> Result<RevisionSlice, String>;
}

impl sealed::Sealed for DirectStore {}
impl DirectRead for DirectStore {
    fn namespace_id(&self) -> Result<String, String> {
        Ok(self.namespace_id())
    }
    fn list_sources(&self) -> Result<Vec<SourceSummary>, String> {
        Ok(self.list_sources())
    }
    fn verify(&self) -> Result<StoreVerification, String> {
        self.verify()
    }
    fn search(&self, query: &str, insensitive: bool) -> Result<StoreSearchResult, String> {
        self.search(query, insensitive)
    }
    fn read_revision_range(
        &self,
        revision: &str,
        start: u64,
        end: u64,
    ) -> Result<RevisionSlice, String> {
        self.read_revision_range(revision, start, end)
    }
}

pub struct ReadOnlyStore<'owner> {
    store: &'owner ExistingReadStore,
    operation: RefCell<NativeReadOperation<'owner>>,
    native_refusal: Cell<Option<OwnerError>>,
}

/// Closed child owned by native sync custody while the durable owner stays
/// RELEASED. It owns the replayed store and the selected existing protector and
/// exposes only an immutable binding check in this slice: no mutation,
/// traversal, publication, read surface, escape or inner accessor.
pub struct ExistingSyncStore {
    store: DirectStore,
}

impl ExistingSyncStore {
    /// Checks immutable child bindings, then the retained native/registration pins.
    pub(crate) fn verify_binding(
        &self,
        operation: &mut NativeSyncPreflightOperation<'_>,
    ) -> Result<(), OwnerError> {
        let selected = self.store.protector.initialization_key_binding()?;
        if self.store.root != operation.root()
            || operation.selected_binding().is_some_and(|expected| expected != selected)
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        operation.verify()
    }
}

impl core::fmt::Debug for ExistingSyncStore {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ExistingSyncStore(<opaque>)")
    }
}

// The kernel never escapes this adapter. Only verified borrowed operations
// receive a read surface, while root custody owns this child's resource lifetime.
pub struct ExistingReadStore {
    store: DirectStore,
}

impl core::fmt::Debug for ReadOnlyStore<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ReadOnlyStore(<opaque>)")
    }
}

impl<'owner> ReadOnlyStore<'owner> {
    pub(crate) const fn new(
        store: &'owner ExistingReadStore,
        operation: NativeReadOperation<'owner>,
    ) -> Self {
        Self {
            store,
            operation: RefCell::new(operation),
            native_refusal: Cell::new(None),
        }
    }

    pub(crate) fn root(&self) -> std::path::PathBuf {
        self.operation.borrow().root().to_owned()
    }

    pub(crate) fn checkpoint(&self) -> Result<(), String> {
        self.native_checkpoint().map_err(owner_error)
    }

    pub(crate) fn native_checkpoint(&self) -> Result<(), OwnerError> {
        if let Some(error) = self.native_refusal.get() {
            return Err(error);
        }
        let result = self.operation.borrow_mut().checkpoint();
        if let Err(error) = result {
            self.native_refusal.set(Some(error));
        }
        result
    }

    pub(crate) const fn native_refusal(&self) -> Option<OwnerError> {
        self.native_refusal.get()
    }

    pub(crate) fn output_attempted(&self) -> bool {
        self.operation.borrow().output_attempted()
    }

    pub(crate) fn cancel_output(&self) {
        self.operation.borrow().cancel_output();
    }

    fn output_result<T>(&self, result: Result<T, OwnerError>) -> std::io::Result<T> {
        if let Err(error) = result.as_ref() {
            self.native_refusal.set(Some(*error));
        }
        result.map_err(|error| std::io::Error::other(error.code()))
    }

    pub(crate) fn write_output(
        &self,
        writer: &mut impl std::io::Write,
        bytes: &[u8],
    ) -> std::io::Result<usize> {
        self.native_checkpoint()
            .map_err(|error| std::io::Error::other(error.code()))?;
        let result = self.operation.borrow_mut().write_output(writer, bytes);
        self.output_result(result)
    }

    pub(crate) fn flush_output(&self, writer: &mut impl std::io::Write) -> std::io::Result<()> {
        self.native_checkpoint()
            .map_err(|error| std::io::Error::other(error.code()))?;
        let result = self.operation.borrow_mut().flush_output(writer);
        self.output_result(result)
    }

    pub(crate) fn into_operation(self) -> NativeReadOperation<'owner> {
        self.operation.into_inner()
    }

    pub(crate) fn retain_shutdown(
        self,
    ) -> Result<crate::owner_composition::RetainedShutdown, OwnerError> {
        self.operation.into_inner().retain_shutdown()
    }

    fn checked<T>(&self, read: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.checkpoint()?;
        let result = read();
        self.checkpoint()?;
        result.map_err(|_| "DIRECT_EXISTING_READ_FAILED".to_owned())
    }
}

impl sealed::Sealed for ReadOnlyStore<'_> {}
impl DirectRead for ReadOnlyStore<'_> {
    fn namespace_id(&self) -> Result<String, String> {
        self.checked(|| Ok(self.store.store.namespace_id()))
    }
    fn list_sources(&self) -> Result<Vec<SourceSummary>, String> {
        self.checked(|| Ok(self.store.store.list_sources()))
    }
    fn verify(&self) -> Result<StoreVerification, String> {
        self.checked(|| self.store.store.verify())
    }
    fn search(&self, query: &str, insensitive: bool) -> Result<StoreSearchResult, String> {
        self.checked(|| self.store.store.search(query, insensitive))
    }
    fn read_revision_range(
        &self,
        revision: &str,
        start: u64,
        end: u64,
    ) -> Result<RevisionSlice, String> {
        self.checked(|| self.store.store.read_revision_range(revision, start, end))
    }
}

pub fn open_existing_read(
    operation: &mut NativeReadOperation<'_>,
) -> Result<ExistingReadStore, OwnerError> {
    operation.verify()?;
    let namespace = operation.namespace_bytes()?;
    let log = operation.catalog_bytes()?;
    let (inner, namespace_id) = replay_existing(operation.root(), &namespace, &log)?;
    drop(log);
    let selected = operation.selected_binding();
    let protector = operation.credential_call(|deadline, cancellation| {
        RevisionProtector::for_existing_credential(namespace_id, selected, deadline, cancellation)
    })?;
    let store = DirectStore {
        root: operation.root().to_owned(),
        inner,
        protector,
    };
    operation.verify()?;
    Ok(ExistingReadStore { store })
}

/// Composes the closed sync child from the same already-existing bytes and the
/// same selected credential. The operation is borrow-only native custody: this
/// function performs no traversal, publication, lifecycle or currentness
/// effect and never creates or replaces a credential.
pub fn open_existing_sync(
    operation: &mut NativeSyncPreflightOperation<'_>,
) -> Result<ExistingSyncStore, OwnerError> {
    operation.verify()?;
    let namespace = operation.namespace_bytes()?;
    let log = operation.catalog_bytes()?;
    let (inner, namespace_id) = replay_existing(operation.root(), &namespace, &log)?;
    drop(log);
    let selected = operation.selected_binding();
    let protector = operation.credential_call(|deadline, cancellation| {
        RevisionProtector::for_existing_credential(namespace_id, selected, deadline, cancellation)
    })?;
    let store = DirectStore {
        root: operation.root().to_owned(),
        inner,
        protector,
    };
    operation.verify()?;
    Ok(ExistingSyncStore { store })
}

fn owner_error(error: OwnerError) -> String {
    error.code().to_owned()
}
