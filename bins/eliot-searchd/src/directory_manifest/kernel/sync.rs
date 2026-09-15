//! Complete directory inventory, reconciliation and next-generation publication.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::codec::{build_manifest, manifest_path, validate_entry};
use super::load::{load_latest_manifest, load_manifest_file};
use super::model::{
    DirectoryEntry, DirectorySyncResult,
};
use super::paths::{ensure_directory, manifest_root, path_identity_bytes};
use super::persist::persist_manifest;
use super::spec::MAX_MANIFEST_ENTRIES;
use crate::direct_store::{DirectStore, IndexedSource};
use crate::sha256;

/// Completes one inventory, indexes current files, retires proven missing
/// bindings, and publishes the next immutable manifest generation.
pub fn sync_directory(
    store: &mut DirectStore,
    data_root: &Path,
    directory: &Path,
) -> Result<DirectorySyncResult, String> {
    let canonical_root = fs::canonicalize(data_root)
        .map_err(|error| format!("DIRECT_SYNC_ROOT_ERROR:{error}"))?;
    let canonical_directory = fs::canonicalize(directory)
        .map_err(|error| format!("DIRECT_SYNC_DIRECTORY_ERROR:{error}"))?;
    ensure_directory(&canonical_root)?;
    ensure_directory(&canonical_directory)?;
    if canonical_directory == canonical_root
        || canonical_directory.starts_with(&canonical_root)
    {
        return Err("DIRECT_SYNC_DIRECTORY_INSIDE_DATA_ROOT".to_owned());
    }

    let namespace_id = store.namespace_id();
    let directory_digest = sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-directory/v1",
        &[&path_identity_bytes(&canonical_directory)],
    ));
    let manifest_root = manifest_root(&canonical_root)?;
    let previous = load_latest_manifest(
        &manifest_root,
        &namespace_id,
        &directory_digest,
    )?;

    let indexed = store.index_directory(&canonical_directory)?;
    let next_entries = entries_from_indexed(&indexed)?;
    let current_sources = store
        .list_sources()
        .into_iter()
        .map(|source| (source.source_id.clone(), source))
        .collect::<BTreeMap<_, _>>();

    let mut missing_sources = 0_usize;
    let mut retired_sources = 0_usize;
    let mut moved_or_rebound_sources = 0_usize;
    if let Some(previous) = &previous {
        for (source_id, old_entry) in &previous.entries {
            if next_entries.contains_key(source_id) {
                continue;
            }
            missing_sources = missing_sources.saturating_add(1);
            let current = current_sources
                .get(source_id)
                .ok_or_else(|| "DIRECT_SYNC_SOURCE_STATE_MISSING".to_owned())?;
            if !current.active {
                continue;
            }
            if current.path_digest != old_entry.path_digest {
                moved_or_rebound_sources =
                    moved_or_rebound_sources.saturating_add(1);
                continue;
            }
            store.retire_source(source_id)?;
            retired_sources = retired_sources.saturating_add(1);
        }
    }

    let generation = previous.as_ref().map_or(Ok(1_u64), |manifest| {
        manifest
            .generation
            .checked_add(1)
            .ok_or_else(|| "DIRECT_SYNC_GENERATION_EXHAUSTED".to_owned())
    })?;
    let manifest = build_manifest(
        namespace_id.clone(),
        directory_digest.clone(),
        generation,
        next_entries,
    )?;
    persist_manifest(&manifest_root, &manifest)?;
    let readback = load_manifest_file(&manifest_path(&manifest_root, &manifest))?;
    if readback != manifest {
        return Err("DIRECT_SYNC_MANIFEST_READBACK_MISMATCH".to_owned());
    }

    Ok(DirectorySyncResult {
        namespace_id,
        directory_digest,
        previous_generation: previous
            .as_ref()
            .map(|manifest| manifest.generation),
        generation,
        previous_sources: previous
            .as_ref()
            .map_or(0, |manifest| manifest.entries.len()),
        indexed_sources: indexed.len(),
        changed_sources: indexed
            .iter()
            .filter(|source| source.changed)
            .count(),
        missing_sources,
        retired_sources,
        moved_or_rebound_sources,
        manifest_digest: manifest.manifest_digest,
    })
}

fn entries_from_indexed(
    indexed: &[IndexedSource],
) -> Result<BTreeMap<String, DirectoryEntry>, String> {
    if indexed.len() > MAX_MANIFEST_ENTRIES {
        return Err("DIRECT_MANIFEST_ENTRY_LIMIT_EXCEEDED".to_owned());
    }
    let mut entries = BTreeMap::new();
    for source in indexed {
        let entry = DirectoryEntry {
            source_id: source.source_id.clone(),
            path_digest: source.path_digest.clone(),
            revision_id: source.revision_id.clone(),
        };
        validate_entry(&entry)?;
        if entries.insert(entry.source_id.clone(), entry).is_some() {
            return Err("DIRECT_MANIFEST_SOURCE_DUPLICATE".to_owned());
        }
    }
    Ok(entries)
}
