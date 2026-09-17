//! Physical revision residue, kept separate from admitted source-revision evidence.
//! This read-only input never decrypts unknown objects, repairs state or authorizes GC.

use std::collections::BTreeMap;
use std::fs::{self, Metadata};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use search_revision_store::{
    LEGACY_REVISION_MAX_INVENTORY_OBJECTS,
    LegacyRevisionInventory, LegacyRevisionInventoryCursor,
    LegacyRevisionInventoryDigest, LegacyRevisionInventoryEntry,
    LegacyRevisionInventoryError,
    LegacyRevisionObjectEvidence, build_legacy_revision_inventory,
    classify_legacy_revision_inventory_name,
    is_legacy_revision_inventory_shard,
    legacy_revision_inventory_checkpoint,
    legacy_revision_inventory_relative_locator,
    plan_legacy_revision_inventory_page,
    render_legacy_revision_inventory_report,
};
use zeroize::Zeroizing;

use super::{
    DirectStore, DEADLINE, MAX_REVISION_OBJECT_BYTES, REVISION_DIRECTORY,
    check_deadline, ensure_directory, read_regular_file, sha256,
};

struct DirectRevisionInventoryDigest;

impl LegacyRevisionInventoryDigest for DirectRevisionInventoryDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        sha256::digest_parts(domain, parts)
    }
}

impl DirectStore {
    /// `control-migration-revisions<TAB>orphans` starts this mode; return its o1
    /// bookmark through the same argument for subsequent pages. No mode is inferred
    /// from a failed normal read. Temporary objects are distinct from final orphans.
    pub(super) fn inspect_migration_orphans(
        &self,
        cursor: Option<&str>,
    ) -> Result<String, String> {
        let deadline = Instant::now()
            .checked_add(DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        // Reject malformed cursors before catalog or filesystem observation.
        let cursor = cursor
            .map(LegacyRevisionInventoryCursor::parse)
            .transpose()
            .map_err(inventory_model_error)?;
        let catalog = self.inner.verify_migration_snapshot(deadline)?;
        let root = self.root.join(REVISION_DIRECTORY);
        let inventory = self.revision_inventory(&root, deadline)?;
        let checkpoint = legacy_revision_inventory_checkpoint::<
            DirectRevisionInventoryDigest,
        >(&catalog, self.protector.backend_name(), &inventory);
        let page = plan_legacy_revision_inventory_page(
            &inventory,
            checkpoint,
            cursor.as_ref(),
        )
        .map_err(inventory_model_error)?;

        let mut evidence = Vec::with_capacity(page.entries().len());
        for entry in page.entries() {
            check_deadline(Some(deadline))?;
            let digest = fingerprint(&root, entry, deadline)?;
            evidence.push(LegacyRevisionObjectEvidence::new(entry, digest));
        }
        // Any membership/size/mtime change invalidates page order. Fingerprints
        // of selected bytes are checked independently, not inferred from mtime.
        if self.revision_inventory(&root, deadline)? != inventory
            || self.inner.verify_migration_snapshot(deadline)? != catalog
        {
            return Err("DIRECT_MIGRATION_ORPHAN_INVENTORY_CHANGED".to_owned());
        }
        for (entry, observed) in page.entries().iter().zip(&evidence) {
            if fingerprint(&root, entry, deadline)?
                != *observed.encoded_sha256()
            {
                return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
            }
        }

        let namespace_id = self.namespace_id();
        let output = render_legacy_revision_inventory_report::<
            DirectRevisionInventoryDigest,
        >(
            &namespace_id,
            &catalog,
            &inventory,
            &page,
            &evidence,
        )
        .map_err(inventory_model_error)?;
        check_deadline(Some(deadline))?;
        Ok(output)
    }

    fn revision_inventory(
        &self,
        root: &Path,
        deadline: Instant,
    ) -> Result<LegacyRevisionInventory, String> {
        ensure_directory(&self.root)?;
        ensure_directory(root)?;
        let mut shards = BTreeMap::new();
        for item in fs::read_dir(root).map_err(|_| inventory_error())? {
            check_deadline(Some(deadline))?;
            if shards.len()
                >= search_revision_store::LEGACY_REVISION_MAX_INVENTORY_SHARDS
            {
                return Err("DIRECT_MIGRATION_SHARD_LIMIT".to_owned());
            }
            let item = item.map_err(|_| inventory_error())?;
            let name = item
                .file_name()
                .into_string()
                .map_err(|_| inventory_error())?;
            if !is_legacy_revision_inventory_shard(&name) {
                return Err(inventory_error());
            }
            ensure_directory(&item.path())?;
            if shards.insert(name, item.path()).is_some() {
                return Err(inventory_error());
            }
        }

        let mut observations = BTreeMap::new();
        for (shard, path) in &shards {
            for item in fs::read_dir(path).map_err(|_| inventory_error())? {
                check_deadline(Some(deadline))?;
                // Enforce the global bound before allocating/sorting the model.
                if observations.len() >= LEGACY_REVISION_MAX_INVENTORY_OBJECTS {
                    return Err("DIRECT_MIGRATION_OBJECT_LIMIT".to_owned());
                }
                let item = item.map_err(|_| inventory_error())?;
                let name = item
                    .file_name()
                    .into_string()
                    .map_err(|_| inventory_error())?;
                let classified = classify_legacy_revision_inventory_name(&name)
                    .ok_or_else(inventory_error)?;
                let relative = legacy_revision_inventory_relative_locator(
                    shard,
                    &name,
                )
                .ok_or_else(inventory_error)?;
                let metadata = fs::symlink_metadata(item.path())
                    .map_err(|_| inventory_error())?;
                let (size, modified) = file_stamp(&metadata)?;
                let kind = classified.inventory_kind(
                    self.inner.retained_revision(classified.id()).is_some(),
                );
                if observations
                    .insert(relative, (size, modified, kind))
                    .is_some()
                {
                    return Err(inventory_error());
                }
            }
        }
        let mut entries = Vec::with_capacity(observations.len());
        for (relative, (size, modified, kind)) in observations {
            let modified = modified
                .duration_since(UNIX_EPOCH)
                .map_err(|_| inventory_error())?;
            entries.push(
                LegacyRevisionInventoryEntry::new(
                    relative,
                    size,
                    modified.as_secs(),
                    modified.subsec_nanos(),
                    kind,
                )
                .map_err(inventory_model_error)?,
            );
        }
        build_legacy_revision_inventory::<DirectRevisionInventoryDigest>(
            shards.into_keys().collect(),
            entries,
        )
        .map_err(inventory_model_error)
    }
}

fn inventory_model_error(error: LegacyRevisionInventoryError) -> String {
    error.code().to_owned()
}

fn inventory_error() -> String {
    "DIRECT_MIGRATION_UNEXPECTED_REVISION_OBJECT".to_owned()
}

fn file_stamp(metadata: &Metadata) -> Result<(u64, SystemTime), String> {
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || reparse
        || metadata.len() > MAX_REVISION_OBJECT_BYTES as u64
    {
        return Err(inventory_error());
    }
    Ok((
        metadata.len(),
        metadata.modified().map_err(|_| inventory_error())?,
    ))
}

fn normalized_file_stamp(metadata: &Metadata) -> Result<(u64, u64, u32), String> {
    let (size, modified) = file_stamp(metadata)?;
    let modified = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|_| inventory_error())?;
    Ok((size, modified.as_secs(), modified.subsec_nanos()))
}

fn fingerprint(
    root: &Path,
    entry: &LegacyRevisionInventoryEntry,
    deadline: Instant,
) -> Result<[u8; 32], String> {
    check_deadline(Some(deadline))?;
    let path = root.join(entry.relative_locator());
    ensure_directory(root)?;
    ensure_directory(path.parent().ok_or_else(inventory_error)?)?;
    let expected = (
        entry.encoded_bytes(),
        entry.modified_seconds(),
        entry.modified_nanos(),
    );
    if normalized_file_stamp(
        &fs::symlink_metadata(&path).map_err(|_| inventory_error())?,
    )? != expected
    {
        return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
    }
    let bytes = Zeroizing::new(read_regular_file(
        &path,
        MAX_REVISION_OBJECT_BYTES,
        "DIRECT_MIGRATION_ORPHAN_READ_FAILED",
    )?);
    let digest = sha256::digest(&bytes);
    if bytes.len() as u64 != entry.encoded_bytes()
        || normalized_file_stamp(
            &fs::symlink_metadata(&path).map_err(|_| inventory_error())?,
        )? != expected
    {
        return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
    }
    check_deadline(Some(deadline))?;
    Ok(digest)
}
