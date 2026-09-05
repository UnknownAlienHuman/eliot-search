//! Guarded maintenance composition for destructive DIRECT operations.

use std::path::Path;

use crate::maintenance::{GarbageCollectionResult, collect_orphan_revisions};

/// Runs a verified preview before destructive GC and refuses deletion when the
/// revision tree contains any object outside the generated-object grammar.
pub(crate) fn guarded_collect_orphan_revisions(
    root: &Path,
    apply: bool,
) -> Result<GarbageCollectionResult, String> {
    crate::catalog_presence::require_existing(root)?;
    let preview = collect_orphan_revisions(root, false)?;
    if !apply {
        return Ok(preview);
    }
    if preview.unexpected_objects != 0 {
        return Err("DIRECT_GC_UNEXPECTED_OBJECTS_PRESENT".to_owned());
    }
    let applied = collect_orphan_revisions(root, true)?;
    if applied.unexpected_objects != 0
        || applied.deleted_objects != preview.orphan_objects
        || applied.deleted_bytes != preview.orphan_bytes
    {
        return Err("DIRECT_GC_READBACK_MISMATCH".to_owned());
    }
    Ok(applied)
}
