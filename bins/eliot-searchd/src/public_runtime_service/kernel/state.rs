//! Shared mutable state for one runtime-service command.

use std::io::Write;
use std::path::Path;

use crate::continuation::ContinuationCatalog;
use crate::direct_store::DirectStore;
use crate::result_handles::ResultHandleCatalog;
use crate::storage_security::StorageSecurityStatus;

use super::session::MutationAttempt;

pub(super) struct CommandState<'a, W: Write> {
    pub(super) writer: &'a mut W,
    pub(super) store: &'a mut DirectStore,
    pub(super) continuations: &'a mut ContinuationCatalog,
    pub(super) handles: &'a mut ResultHandleCatalog,
    pub(super) canonical_root: &'a Path,
    pub(super) storage: &'a mut StorageSecurityStatus,
    pub(super) attempt: &'a mut MutationAttempt,
}

pub(super) fn invalidate_search_state(
    continuations: &mut ContinuationCatalog,
    handles: &mut ResultHandleCatalog,
) -> (usize, usize) {
    (continuations.invalidate_all(), handles.invalidate_all())
}
