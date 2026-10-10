//! Guarded maintenance composition for destructive DIRECT operations.

use std::path::Path;

use crate::maintenance::{GarbageCollectionResult, collect_orphan_revisions_with_check};

/// Runs a verified preview before destructive GC under the caller's checkpoint.
///
/// Refuses deletion when the revision tree contains an unexpected object. The
/// callback checks the original admitted operation; it does not grant root
/// authority. Refusal propagates without rollback or recovery-marker cleanup.
pub fn guarded_collect_orphan_revisions_with_check(
    root: &Path,
    apply: bool,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<GarbageCollectionResult, String> {
    check()?;
    crate::catalog_presence::require_existing(root)?;
    check()?;
    let preview = collect_orphan_revisions_with_check(root, false, check)?;
    check()?;
    if !apply {
        return Ok(preview);
    }
    if preview.unexpected_objects != 0 {
        return Err("DIRECT_GC_UNEXPECTED_OBJECTS_PRESENT".to_owned());
    }
    check()?;
    let applied = collect_orphan_revisions_with_check(root, true, check)?;
    check()?;
    if applied.unexpected_objects != 0
        || applied.deleted_objects != preview.orphan_objects
        || applied.deleted_bytes != preview.orphan_bytes
    {
        return Err("DIRECT_GC_READBACK_MISMATCH".to_owned());
    }
    check()?;
    Ok(applied)
}

/// Context-free historical fixture wrapper; unavailable to production callers.
#[cfg(test)]
pub fn guarded_collect_orphan_revisions(
    root: &Path,
    apply: bool,
) -> Result<GarbageCollectionResult, String> {
    guarded_collect_orphan_revisions_with_check(root, apply, &|| Ok(()))
}
