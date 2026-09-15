use std::path::Path;

use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::storage_security::StorageSecurityStatus;

pub(super) fn with_store<T>(
    root: &Path,
    operation: impl FnOnce(
        &Path,
        &DirectStore,
        &StorageSecurityStatus,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let guard = DataRootGuard::acquire(root)?;
    let store = DirectStore::open(guard.canonical_root())?;
    let storage = StorageSecurityStatus::inspect(guard.canonical_root())?;
    operation(guard.canonical_root(), &store, &storage)
}

pub(super) fn with_store_mut<T>(
    root: &Path,
    operation: impl FnOnce(&Path, &mut DirectStore) -> Result<T, String>,
) -> Result<T, String> {
    let guard = DataRootGuard::acquire(root)?;
    let mut store = DirectStore::open(guard.canonical_root())?;
    operation(guard.canonical_root(), &mut store)
}
