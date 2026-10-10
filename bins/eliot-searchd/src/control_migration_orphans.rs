//! Physical revision residue, kept separate from admitted source-revision evidence.
//! This read-only input never decrypts unknown objects, repairs state or authorizes GC.

use std::collections::BTreeMap;
use std::fs::{self, Metadata};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use search_revision_store::{
    LEGACY_REVISION_MAX_INVENTORY_OBJECTS, LegacyRevisionInventory, LegacyRevisionInventoryCursor,
    LegacyRevisionInventoryDigest, LegacyRevisionInventoryEntry, LegacyRevisionInventoryError,
    LegacyRevisionObjectEvidence, build_legacy_revision_inventory,
    classify_legacy_revision_inventory_name, is_legacy_revision_inventory_shard,
    legacy_revision_inventory_checkpoint, legacy_revision_inventory_relative_locator,
    plan_legacy_revision_inventory_page, render_legacy_revision_inventory_report,
};
use zeroize::Zeroizing;

use super::{
    DEADLINE, DirectStore, MAX_REVISION_OBJECT_BYTES, REVISION_DIRECTORY, check_deadline,
    ensure_directory, read_regular_file, sha256,
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
    pub(super) fn inspect_migration_orphans(&self, cursor: Option<&str>) -> Result<String, String> {
        self.check_operation()?;
        let page_deadline = Instant::now()
            .checked_add(DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        let deadline = self
            .operation_deadline()?
            .map_or(page_deadline, |original| original.min(page_deadline));
        let check = || {
            self.check_operation()?;
            check_deadline(Some(deadline))
        };
        check()?;
        // Reject malformed cursors before catalog or filesystem observation.
        let cursor = cursor
            .map(LegacyRevisionInventoryCursor::parse)
            .transpose()
            .map_err(inventory_model_error)?;
        check()?;
        let catalog = self.inner.verify_migration_snapshot(deadline);
        check()?;
        let catalog = catalog?;
        let root = self.root.join(REVISION_DIRECTORY);
        let inventory = self.revision_inventory(&root, deadline)?;
        check()?;
        let checkpoint = legacy_revision_inventory_checkpoint::<DirectRevisionInventoryDigest>(
            &catalog,
            self.protector.backend_name(),
            &inventory,
        );
        check()?;
        let page = plan_legacy_revision_inventory_page(&inventory, checkpoint, cursor.as_ref())
            .map_err(inventory_model_error)?;
        check()?;

        let mut evidence = Vec::with_capacity(page.entries().len());
        for entry in page.entries() {
            check()?;
            let digest = fingerprint(&root, entry, deadline, &check)?;
            check()?;
            evidence.push(LegacyRevisionObjectEvidence::new(entry, digest));
            check()?;
        }
        check()?;
        // Any membership/size/mtime change invalidates page order. Fingerprints
        // of selected bytes are checked independently, not inferred from mtime.
        let observed_inventory = self.revision_inventory(&root, deadline)?;
        check()?;
        if observed_inventory != inventory {
            return Err("DIRECT_MIGRATION_ORPHAN_INVENTORY_CHANGED".to_owned());
        }
        let observed_catalog = self.inner.verify_migration_snapshot(deadline);
        check()?;
        if observed_catalog? != catalog {
            return Err("DIRECT_MIGRATION_ORPHAN_INVENTORY_CHANGED".to_owned());
        }
        for (entry, observed) in page.entries().iter().zip(&evidence) {
            check()?;
            let digest = fingerprint(&root, entry, deadline, &check)?;
            check()?;
            if digest != *observed.encoded_sha256() {
                return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
            }
            check()?;
        }
        check()?;

        let namespace_id = self.namespace_id();
        check()?;
        let output = render_legacy_revision_inventory_report::<DirectRevisionInventoryDigest>(
            &namespace_id,
            &catalog,
            &inventory,
            &page,
            &evidence,
        )
        .map_err(inventory_model_error)?;
        check()?;
        Ok(output)
    }

    fn revision_inventory(
        &self,
        root: &Path,
        deadline: Instant,
    ) -> Result<LegacyRevisionInventory, String> {
        let check = || {
            self.check_operation()?;
            check_deadline(Some(deadline))
        };
        check()?;
        let root_directory = ensure_directory(&self.root);
        check()?;
        root_directory?;
        let revision_directory = ensure_directory(root);
        check()?;
        revision_directory?;
        let mut shards = BTreeMap::new();
        let directory = fs::read_dir(root);
        check()?;
        for item in directory.map_err(|_| inventory_error())? {
            check()?;
            if shards.len() >= search_revision_store::LEGACY_REVISION_MAX_INVENTORY_SHARDS {
                return Err("DIRECT_MIGRATION_SHARD_LIMIT".to_owned());
            }
            let item = item.map_err(|_| inventory_error())?;
            check()?;
            let name = item
                .file_name()
                .into_string()
                .map_err(|_| inventory_error())?;
            if !is_legacy_revision_inventory_shard(&name) {
                return Err(inventory_error());
            }
            check()?;
            let shard_directory = ensure_directory(&item.path());
            check()?;
            shard_directory?;
            if shards.insert(name, item.path()).is_some() {
                return Err(inventory_error());
            }
            check()?;
        }
        check()?;

        let mut observations = BTreeMap::new();
        for (shard, path) in &shards {
            check()?;
            let directory = fs::read_dir(path);
            check()?;
            for item in directory.map_err(|_| inventory_error())? {
                check()?;
                // Enforce the global bound before allocating/sorting the model.
                if observations.len() >= LEGACY_REVISION_MAX_INVENTORY_OBJECTS {
                    return Err("DIRECT_MIGRATION_OBJECT_LIMIT".to_owned());
                }
                let item = item.map_err(|_| inventory_error())?;
                check()?;
                let name = item
                    .file_name()
                    .into_string()
                    .map_err(|_| inventory_error())?;
                let classified =
                    classify_legacy_revision_inventory_name(&name).ok_or_else(inventory_error)?;
                let relative = legacy_revision_inventory_relative_locator(shard, &name)
                    .ok_or_else(inventory_error)?;
                check()?;
                let metadata = fs::symlink_metadata(item.path());
                check()?;
                let stamp = file_stamp(&metadata.map_err(|_| inventory_error())?);
                check()?;
                let (size, modified) = stamp?;
                let kind = classified
                    .inventory_kind(self.inner.retained_revision(classified.id()).is_some());
                if observations
                    .insert(relative, (size, modified, kind))
                    .is_some()
                {
                    return Err(inventory_error());
                }
                check()?;
            }
            check()?;
        }
        check()?;
        let mut entries = Vec::with_capacity(observations.len());
        for (relative, (size, modified, kind)) in observations {
            check()?;
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
            check()?;
        }
        check()?;
        let mut shard_names = Vec::with_capacity(shards.len());
        for name in shards.into_keys() {
            check()?;
            shard_names.push(name);
            check()?;
        }
        check()?;
        let inventory =
            build_legacy_revision_inventory::<DirectRevisionInventoryDigest>(shard_names, entries)
                .map_err(inventory_model_error)?;
        check()?;
        Ok(inventory)
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
    check: &dyn Fn() -> Result<(), String>,
) -> Result<[u8; 32], String> {
    let checkpoint = || {
        check()?;
        check_deadline(Some(deadline))
    };
    checkpoint()?;
    let path = root.join(entry.relative_locator());
    let root_directory = ensure_directory(root);
    checkpoint()?;
    root_directory?;
    let parent_directory = ensure_directory(path.parent().ok_or_else(inventory_error)?);
    checkpoint()?;
    parent_directory?;
    let expected = (
        entry.encoded_bytes(),
        entry.modified_seconds(),
        entry.modified_nanos(),
    );
    let metadata = fs::symlink_metadata(&path);
    checkpoint()?;
    let before = normalized_file_stamp(&metadata.map_err(|_| inventory_error())?);
    checkpoint()?;
    if before? != expected {
        return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
    }
    checkpoint()?;
    let readback = read_regular_file(
        &path,
        MAX_REVISION_OBJECT_BYTES,
        "DIRECT_MIGRATION_ORPHAN_READ_FAILED",
    );
    checkpoint()?;
    let bytes = Zeroizing::new(readback?);
    let digest = sha256::digest(&bytes);
    checkpoint()?;
    if bytes.len() as u64 != entry.encoded_bytes() {
        return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
    }
    checkpoint()?;
    let metadata = fs::symlink_metadata(&path);
    checkpoint()?;
    let after = normalized_file_stamp(&metadata.map_err(|_| inventory_error())?);
    checkpoint()?;
    if after? != expected {
        return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
    }
    checkpoint()?;
    Ok(digest)
}
