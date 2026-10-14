//! Exact old/new manifest difference.
//!
//! A retained point requires full immutable content equality. The same
//! `PointId128` carrying changed immutable content fails as a collision
//! rather than being silently replaced, and a broad source filter is
//! structurally unavailable here.

use std::collections::BTreeMap;

use crate::error::ProjectionError;
use crate::model::{ManifestDiff, ProjectionManifest, ProjectionManifestEntry};

/// Validates strict canonical ordering and duplicate freedom.
pub fn validate_entries(entries: &[ProjectionManifestEntry]) -> Result<(), ProjectionError> {
    if entries
        .windows(2)
        .any(|pair| pair[0].point_id >= pair[1].point_id)
    {
        return Err(ProjectionError::InvalidManifest);
    }
    Ok(())
}

/// Returns exact create, retain and retire entries.
///
/// The same address with changed immutable content is refused as a collision
/// and manifest mismatch rather than reported as an ordinary replacement.
pub fn diff_manifests(
    old: &ProjectionManifest,
    new: &ProjectionManifest,
) -> Result<ManifestDiff, ProjectionError> {
    validate_entries(&old.entries)?;
    validate_entries(&new.entries)?;
    let old_by_id = old
        .entries
        .iter()
        .map(|entry| (entry.point_id, entry))
        .collect::<BTreeMap<_, _>>();
    let new_by_id = new
        .entries
        .iter()
        .map(|entry| (entry.point_id, entry))
        .collect::<BTreeMap<_, _>>();

    let mut create = Vec::new();
    let mut retain = Vec::new();
    let mut retire = Vec::new();

    for (point_id, new_entry) in &new_by_id {
        match old_by_id.get(point_id) {
            Some(old_entry) if *old_entry == *new_entry => retain.push((*new_entry).clone()),
            Some(_) => return Err(ProjectionError::ManifestMismatch),
            None => create.push((*new_entry).clone()),
        }
    }
    for (point_id, old_entry) in old_by_id {
        if !new_by_id.contains_key(&point_id) {
            retire.push(old_entry.clone());
        }
    }
    Ok(ManifestDiff {
        create,
        retain,
        retire,
    })
}
