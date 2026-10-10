//! Existing-only store access for secure one-shot commands.
//!
//! Root admission owns quarantine and durable owner validation. These helpers
//! borrow its canonical root and never acquire a second owner.
//!
//! Integration requirement: read-only construction and strict status inspection
//! must not resolve revision secrets. The secure reader needs lazy protected
//! reads; status callers need catalog-only verification because the current
//! `DirectStore::verify` reads retained revision content. No plaintext fallback
//! or mutating opener is permitted to fill that seam.

use std::path::Path;

use crate::catalog_quarantine;
use crate::development::{DataRootGuard, InspectedDataRoot};
use crate::direct_store::{DirectStore, ReadOnlyStore};
use crate::owner_composition::DataRootRequest;
use crate::storage_security::StorageSecurityStatus;

/// Executes an existing-store read under the caller's original request.
///
/// The opener must neither create nor migrate storage. Admission and final
/// identity/barrier checks are performed by the inspection owner. Preflight
/// checks reuse the original absolute deadline and cancellation state.
pub(super) fn with_store_request<T>(
    root: &Path,
    request: &DataRootRequest,
    operation: impl FnOnce(&Path, &ReadOnlyStore<'_>, &StorageSecurityStatus) -> Result<T, String>,
) -> Result<T, String> {
    request.preflight()?;
    DataRootGuard::with_inspection_request(root, request, |cap: &InspectedDataRoot| {
        request.preflight()?;
        let store = DirectStore::open_existing_read_only(cap)?;
        let storage = StorageSecurityStatus::inspect(cap.canonical_root())?;
        request.preflight()?;
        let value = operation(cap.canonical_root(), &store, &storage)?;
        request.preflight()?;
        Ok(value)
    })
}

/// Executes one existing-store mutation under the caller's original request.
///
/// The marker is armed before dispatch. Closure errors, including output errors,
/// and failed verification do not clear it or reach clean release. Only exact
/// readback permits clearing. A subsequent clear, drain or release error remains
/// an error; dropping the guard is never reported as clean durable release.
/// Cancellation or deadline refusal before marker clearing retains recovery
/// evidence. No helper starts another request or resets the deadline.
pub(super) fn with_store_mut_request<T>(
    root: &Path,
    request: &DataRootRequest,
    operation: impl FnOnce(&Path, &mut DirectStore) -> Result<T, String>,
) -> Result<T, String> {
    request.preflight()?;
    let mut guard = DataRootGuard::open_existing_request(root, request)?;
    request.preflight()?;
    let mut store = DirectStore::open_existing_mutating(&guard)?;
    guard.verify_existing()?;
    request.preflight()?;
    catalog_quarantine::arm(guard.canonical_root())?;

    // Early errors close the store before the guard and retain recovery evidence.
    // Output belongs to the closure, so a failed acknowledgement cannot clear
    // the marker or produce a successful release through this helper.
    let value = operation(guard.canonical_root(), &mut store)?;
    request.preflight()?;
    store.verify()?;
    guard.verify_existing()?;
    request.preflight()?;
    guard.clear_mutation_marker()?;

    drop(store);
    guard.begin_drain(search_runtime_owner::DrainReason::Shutdown)?;
    guard.release_cleanly()?;
    Ok(value)
}
