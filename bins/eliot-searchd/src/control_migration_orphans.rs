//! Physical revision residue, kept separate from admitted source-revision evidence.
//! This read-only input never decrypts unknown objects, repairs state or authorizes GC.

use std::collections::BTreeMap;
use std::fs::{self, Metadata};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

use super::{DirectStore, DEADLINE, MAX_PAGE_BYTES, MAX_PAGE_STORED_BYTES,
    MAX_REVISION_OBJECT_BYTES, REVISION_DIRECTORY, check_deadline, ensure_directory,
    json_string, read_regular_file, sha256};

const MAX_FILES: usize = 65_536;
const PAGE_FILES: usize = 32;
const MAX_NAME_BYTES: usize = 192;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Kind { Referenced, Orphan, Temporary }
impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Self::Referenced => "catalog_referenced",
            Self::Orphan => "unreferenced_revision_object",
            Self::Temporary => "uncommitted_temporary_object",
        }
    }
}

#[derive(Eq, PartialEq)]
struct Entry {
    relative: String,
    size: u64,
    modified: SystemTime,
    kind: Kind,
}

#[derive(Eq, PartialEq)]
struct Inventory {
    shards: Vec<String>,
    files: Vec<Entry>,
    digest: [u8; 32],
    orphan_count: usize,
    temporary_count: usize,
    unreferenced_bytes: u64,
}

struct Cursor { checkpoint: [u8; 32], next: usize }
impl Cursor {
    fn parse(value: &str) -> Result<Self, String> {
        let invalid = || "DIRECT_MIGRATION_ORPHAN_CURSOR_INVALID".to_owned();
        if value.len() > 88 || !value.is_ascii() { return Err(invalid()); }
        let parts = value.splitn(4, '.').collect::<Vec<_>>();
        if parts.len() != 3 || parts[0] != "o1" { return Err(invalid()); }
        let checkpoint = sha256::decode_digest(parts[1]).ok_or_else(invalid)?;
        let next = parts[2].parse::<usize>().map_err(|_| invalid())?;
        if next > MAX_FILES || next.to_string() != parts[2] || sha256::hex(&checkpoint) != parts[1] {
            return Err(invalid());
        }
        Ok(Self { checkpoint, next })
    }
}

impl DirectStore {
    /// `control-migration-revisions<TAB>orphans` starts this mode; return its o1
    /// bookmark through the same argument for subsequent pages. No mode is inferred
    /// from a failed normal read. Temporary objects are distinct from final orphans.
    pub(super) fn inspect_migration_orphans(&self, cursor: Option<&str>) -> Result<String, String> {
        let deadline = Instant::now().checked_add(DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        let cursor = cursor.map(Cursor::parse).transpose()?;
        let catalog = self.inner.verify_migration_snapshot(deadline)?;
        let root = self.root.join(REVISION_DIRECTORY);
        let inventory = self.revision_inventory(&root, deadline)?;
        let checkpoint = sha256::digest_parts(b"eliot-search/control-migration-orphans/v1", &[
            &catalog, self.protector.backend_name().as_bytes(), &inventory.digest,
        ]);
        let first = cursor.as_ref().map_or(0, |value| value.next);
        let count = inventory.orphan_count + inventory.temporary_count;
        if first > count || cursor.as_ref().is_some_and(|value| value.checkpoint != checkpoint) {
            return Err("DIRECT_MIGRATION_ORPHAN_CURSOR_STALE".to_owned());
        }
        let mut rows = Vec::with_capacity(PAGE_FILES);
        let mut fingerprints = Vec::with_capacity(PAGE_FILES);
        let mut encoded_bytes = 0_u64;
        for entry in inventory.files.iter().filter(|entry| entry.kind != Kind::Referenced).skip(first) {
            check_deadline(Some(deadline))?;
            if rows.len() == PAGE_FILES || encoded_bytes.checked_add(entry.size)
                .is_none_or(|bytes| bytes > MAX_PAGE_STORED_BYTES)
            {
                break;
            }
            let digest = fingerprint(&root, entry, deadline)?;
            encoded_bytes += entry.size;
            rows.push(format!(concat!(
                "{{\"object_locator\":{},\"kind\":{},\"encoded_bytes\":{},\"encoded_sha256\":\"{}\",",
                "\"catalog_referenced\":false,\"source_binding_verified\":false,\"deletion_authorized\":false}}"
            ), json_string(&format!("revisions/{}", entry.relative)), json_string(entry.kind.tag()),
                entry.size, sha256::hex(&digest)));
            fingerprints.push((entry, digest));
        }
        if rows.is_empty() && first != count { return Err("DIRECT_MIGRATION_NO_PROGRESS".to_owned()); }
        // Any membership/size/mtime change invalidates page order. Fingerprints
        // of selected bytes are checked independently, not inferred from mtime.
        if self.revision_inventory(&root, deadline)? != inventory
            || self.inner.verify_migration_snapshot(deadline)? != catalog
        {
            return Err("DIRECT_MIGRATION_ORPHAN_INVENTORY_CHANGED".to_owned());
        }
        for (entry, digest) in fingerprints {
            if fingerprint(&root, entry, deadline)? != digest {
                return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
            }
        }
        let next = first + rows.len();
        let exhausted = next == count;
        let body = rows.join(",");
        let page_digest = sha256::digest_parts(b"eliot-search/control-migration-orphan-page/v1", &[
            &checkpoint, &(first as u64).to_be_bytes(), &(next as u64).to_be_bytes(), body.as_bytes(),
        ]);
        let next_cursor = if exhausted { "null".to_owned() }
            else { json_string(&format!("o1.{}.{next}", sha256::hex(&checkpoint))) };
        let output = format!(concat!(
            "{{\"event\":\"control_migration_orphans\",\"schema\":\"legacy-revision-orphans-v1\",",
            "\"scope\":\"revision_tree_only\",\"namespace_id\":{},\"catalog_snapshot_sha256\":\"{}\",",
            "\"inventory_sha256\":\"{}\",\"inventory_basis\":\"names_sizes_mtimes\",",
            "\"shards\":{},\"inventory_files\":{},\"catalog_referenced_files\":{},",
            "\"orphan_objects\":{},\"temporary_objects\":{},\"unreferenced_bytes\":{},",
            "\"after_object\":{},\"next_object\":{},\"page_objects\":{},\"page_encoded_bytes\":{},",
            "\"entries\":[{}],\"page_sha256\":\"{}\",\"next_cursor\":{},\"exhausted\":{},",
            "\"read_only\":true,\"page_encoded_bytes_hashed\":true,\"all_object_contents_hashed\":false,",
            "\"preparation_orphans_enumerated\":false,\"deletion_authorized\":false,",
            "\"cutover_revalidation_required\":true,\"canonical_mapping_complete\":false}}"
        ), json_string(&self.namespace_id()), sha256::hex(&catalog), sha256::hex(&inventory.digest),
            inventory.shards.len(), inventory.files.len(), inventory.files.len() - count,
            inventory.orphan_count, inventory.temporary_count, inventory.unreferenced_bytes,
            first, next, rows.len(), encoded_bytes, body, sha256::hex(&page_digest), next_cursor, exhausted);
        if output.len() > MAX_PAGE_BYTES { return Err("DIRECT_MIGRATION_PAGE_TOO_LARGE".to_owned()); }
        check_deadline(Some(deadline))?;
        Ok(output)
    }

    fn revision_inventory(&self, root: &Path, deadline: Instant) -> Result<Inventory, String> {
        ensure_directory(&self.root)?;
        ensure_directory(root)?;
        let mut shards = BTreeMap::new();
        for item in fs::read_dir(root).map_err(|_| inventory_error())? {
            check_deadline(Some(deadline))?;
            if shards.len() >= 256 { return Err("DIRECT_MIGRATION_SHARD_LIMIT".to_owned()); }
            let item = item.map_err(|_| inventory_error())?;
            let name = item.file_name().into_string().map_err(|_| inventory_error())?;
            if name.len() != 2 || !lower_hex(&name) { return Err(inventory_error()); }
            ensure_directory(&item.path())?;
            if shards.insert(name, item.path()).is_some() { return Err(inventory_error()); }
        }
        let mut files = BTreeMap::new();
        for (shard, path) in &shards {
            for item in fs::read_dir(path).map_err(|_| inventory_error())? {
                check_deadline(Some(deadline))?;
                // Enforce the global bound before allocating/sorting the inventory.
                if files.len() >= MAX_FILES { return Err("DIRECT_MIGRATION_OBJECT_LIMIT".to_owned()); }
                let item = item.map_err(|_| inventory_error())?;
                let name = item.file_name().into_string().map_err(|_| inventory_error())?;
                if name.len() > MAX_NAME_BYTES || !name.is_ascii() { return Err(inventory_error()); }
                let (id, temporary) = generated_name(&name).ok_or_else(inventory_error)?;
                if !id.starts_with(shard) { return Err(inventory_error()); }
                let metadata = fs::symlink_metadata(item.path()).map_err(|_| inventory_error())?;
                let (size, modified) = file_stamp(&metadata)?;
                let kind = if temporary { Kind::Temporary }
                    else if self.inner.retained_revision(id).is_some() { Kind::Referenced }
                    else { Kind::Orphan };
                let relative = format!("{shard}/{name}");
                if files.insert(relative.clone(), Entry { relative, size, modified, kind }).is_some() {
                    return Err(inventory_error());
                }
            }
        }
        let mut digest = sha256::digest_parts(b"eliot-search/revision-residue-inventory/v1", &[]);
        for name in shards.keys() {
            digest = sha256::digest_parts(b"eliot-search/revision-residue-shard/v1", &[&digest, name.as_bytes()]);
        }
        let (mut orphan_count, mut temporary_count, mut unreferenced_bytes) = (0, 0, 0_u64);
        for entry in files.values() {
            check_deadline(Some(deadline))?;
            let modified = entry.modified.duration_since(UNIX_EPOCH).map_err(|_| inventory_error())?;
            digest = sha256::digest_parts(b"eliot-search/revision-residue-file/v1", &[
                &digest, entry.relative.as_bytes(), &entry.size.to_be_bytes(),
                &modified.as_secs().to_be_bytes(), &modified.subsec_nanos().to_be_bytes(), entry.kind.tag().as_bytes(),
            ]);
            if entry.kind != Kind::Referenced {
                unreferenced_bytes = unreferenced_bytes.checked_add(entry.size)
                    .ok_or_else(|| "DIRECT_MIGRATION_BYTES_EXCEEDED".to_owned())?;
                orphan_count += usize::from(entry.kind == Kind::Orphan);
                temporary_count += usize::from(entry.kind == Kind::Temporary);
            }
        }
        Ok(Inventory {
            shards: shards.into_keys().collect(), files: files.into_values().collect(), digest,
            orphan_count, temporary_count, unreferenced_bytes,
        })
    }
}

fn inventory_error() -> String { "DIRECT_MIGRATION_UNEXPECTED_REVISION_OBJECT".to_owned() }

fn lower_hex(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Accept only canonical writer-generated names. This is stricter admission than
/// old GC's broad temporary-name recognizer; unknown names block rather than vanish.
fn generated_name(name: &str) -> Option<(&str, bool)> {
    for suffix in [".bin", ".dpapi"] {
        if let Some(id) = name.strip_suffix(suffix) {
            if id.len() == 64 && lower_hex(id) { return Some((id, false)); }
        }
    }
    let body = name.strip_prefix('.')?.strip_suffix(".tmp")?;
    let parts = body.splitn(5, '.').collect::<Vec<_>>();
    let id = *parts.first()?;
    if id.len() != 64 || !lower_hex(id) { return None; }
    let decimal = |value: &str| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    match parts.as_slice() {
        [_, pid] if decimal(pid) => Some((id, true)),
        [_, pid, stamp, "dpapi"] if decimal(pid) && decimal(stamp) => Some((id, true)),
        _ => None,
    }
}

fn file_stamp(metadata: &Metadata) -> Result<(u64, SystemTime), String> {
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    if !metadata.is_file() || metadata.file_type().is_symlink() || reparse
        || metadata.len() > MAX_REVISION_OBJECT_BYTES as u64
    {
        return Err(inventory_error());
    }
    Ok((metadata.len(), metadata.modified().map_err(|_| inventory_error())?))
}

fn fingerprint(root: &Path, entry: &Entry, deadline: Instant) -> Result<[u8; 32], String> {
    check_deadline(Some(deadline))?;
    let path = root.join(&entry.relative); // relative is generated-name validated, never user input
    ensure_directory(root)?;
    ensure_directory(path.parent().ok_or_else(inventory_error)?)?;
    let expected = (entry.size, entry.modified);
    if file_stamp(&fs::symlink_metadata(&path).map_err(|_| inventory_error())?)? != expected {
        return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
    }
    let bytes = Zeroizing::new(read_regular_file(&path, MAX_REVISION_OBJECT_BYTES, "DIRECT_MIGRATION_ORPHAN_READ_FAILED")?);
    let digest = sha256::digest(&bytes);
    if bytes.len() as u64 != entry.size
        || file_stamp(&fs::symlink_metadata(&path).map_err(|_| inventory_error())?)? != expected
    {
        return Err("DIRECT_MIGRATION_ORPHAN_OBJECT_CHANGED".to_owned());
    }
    check_deadline(Some(deadline))?;
    Ok(digest)
}
