//! Complete physical preparation inventory, separate from verified derivation.
//! Unknown-profile references are preserved as unresolved, never treated as GC authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, Metadata};
use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

use super::{binding, decode_reference, decode_reference_fields, extension, lookup_key,
    object_id, profile_digest, read_regular_file, ensure_directory, sha256,
    MAX_OBJECT_BYTES, REF_BYTES};
use super::super::DirectStore;
use crate::service_output::json_string;

const MAX_FILES: usize = 65_536;
const PAGE_FILES: usize = 32;
const PAGE_BYTES: u64 = 512 * 1024 * 1024;
const RESPONSE_BYTES: usize = 60 * 1024;
const MAX_NAME_BYTES: usize = 192;
const DEADLINE: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Eq, PartialEq)]
enum Kind { CurrentReference, UnmappedReference, CurrentTarget, UnmappedObject, Temporary }
impl Kind {
    const fn tag(self) -> &'static str {
        match self {
            Self::CurrentReference => "current_profile_reference",
            Self::UnmappedReference => "unmapped_profile_or_revision_reference",
            Self::CurrentTarget => "current_profile_target",
            Self::UnmappedObject => "not_linked_by_current_profile",
            Self::Temporary => "uncommitted_temporary_object",
        }
    }
}

#[derive(Eq, PartialEq)]
struct Artifact {
    relative: String,
    size: u64,
    modified: SystemTime,
    kind: Kind,
    reference: Option<[u8; REF_BYTES]>,
    target: Option<String>,
}

#[derive(Eq, PartialEq)]
struct Inventory {
    // Includes base/tree presence and empty shards, not just directories with files.
    directories: BTreeSet<String>,
    files: BTreeMap<String, Artifact>,
    missing_current_references: usize,
    digest: [u8; 32],
}

struct Cursor { checkpoint: [u8; 32], next: usize }
impl Cursor {
    fn parse(value: &str) -> Result<Self, String> {
        let invalid = || "DIRECT_MIGRATION_PREPARATION_CURSOR_INVALID".to_owned();
        if value.len() > 88 || !value.is_ascii() { return Err(invalid()); }
        let parts = value.splitn(4, '.').collect::<Vec<_>>();
        if parts.len() != 3 || parts[0] != "p1" { return Err(invalid()); }
        let checkpoint = sha256::decode_digest(parts[1]).ok_or_else(invalid)?;
        let next = parts[2].parse::<usize>().map_err(|_| invalid())?;
        if next > MAX_FILES || next.to_string() != parts[2] || sha256::hex(&checkpoint) != parts[1] {
            return Err(invalid());
        }
        Ok(Self { checkpoint, next })
    }
}

impl DirectStore {
    /// Explicit administrative mode through the existing migration command.
    /// Every physical file is accounted for, including other-profile/backend residue.
    /// Mapping a current reference does not authenticate its target or prove derivation;
    /// normal revision migration performs those checks. No bytes are deleted or repaired.
    pub(crate) fn inspect_migration_preparation_files(
        &self, cursor: Option<&str>,
    ) -> Result<String, String> {
        let deadline = Instant::now().checked_add(DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        let cursor = cursor.map(Cursor::parse).transpose()?;
        let catalog = self.inner.verify_migration_snapshot(deadline)?;
        let inventory = self.preparation_inventory(deadline)?;
        let profile = profile_digest();
        let checkpoint = sha256::digest_parts(b"eliot-search/preparation-inventory-cursor/v1", &[
            &catalog, &profile, self.protector.backend_name().as_bytes(), &inventory.digest,
        ]);
        let first = cursor.as_ref().map_or(0, |value| value.next);
        if first > inventory.files.len() || cursor.as_ref().is_some_and(|value| value.checkpoint != checkpoint) {
            return Err("DIRECT_MIGRATION_PREPARATION_CURSOR_STALE".to_owned());
        }
        let mut rows = Vec::with_capacity(PAGE_FILES);
        let mut fingerprints = Vec::with_capacity(PAGE_FILES);
        let mut encoded_bytes = 0_u64;
        for entry in inventory.files.values().skip(first) {
            check(deadline)?;
            if rows.len() == PAGE_FILES || encoded_bytes.checked_add(entry.size).is_none_or(|bytes| bytes > PAGE_BYTES) {
                break;
            }
            let digest = sha256::digest(&read_artifact(&self.root, entry, MAX_OBJECT_BYTES, deadline)?);
            encoded_bytes += entry.size;
            rows.push(format!(concat!(
                "{{\"object_locator\":{},\"kind\":{},\"encoded_bytes\":{},\"encoded_sha256\":\"{}\",",
                "\"current_profile_target_locator\":{},\"content_binding_verified\":false,\"deletion_authorized\":false}}"
            ), json_string(&format!("preparation/{}", entry.relative)), json_string(entry.kind.tag()),
                entry.size, sha256::hex(&digest), entry.target.as_ref().map_or_else(|| "null".to_owned(),
                    |target| json_string(&format!("preparation/{target}")))));
            fingerprints.push((entry, digest));
        }
        if rows.is_empty() && first != inventory.files.len() { return Err("DIRECT_MIGRATION_NO_PROGRESS".to_owned()); }
        // Hash every reference during both sweeps, including references not owned by
        // the current profile. A same-size/mtime reference replacement invalidates the cursor.
        if self.preparation_inventory(deadline)? != inventory
            || self.inner.verify_migration_snapshot(deadline)? != catalog
        {
            return Err("DIRECT_MIGRATION_PREPARATION_INVENTORY_CHANGED".to_owned());
        }
        for (entry, digest) in fingerprints {
            if sha256::digest(&read_artifact(&self.root, entry, MAX_OBJECT_BYTES, deadline)?) != digest {
                return Err("DIRECT_MIGRATION_PREPARATION_OBJECT_CHANGED".to_owned());
            }
        }
        let count = |kind| inventory.files.values().filter(|entry| entry.kind == kind).count();
        let next = first + rows.len();
        let exhausted = next == inventory.files.len();
        let body = rows.join(",");
        let page_digest = sha256::digest_parts(b"eliot-search/preparation-inventory-page/v1", &[
            &checkpoint, &(first as u64).to_be_bytes(), &(next as u64).to_be_bytes(), body.as_bytes(),
        ]);
        let next_cursor = if exhausted { "null".to_owned() }
            else { json_string(&format!("p1.{}.{next}", sha256::hex(&checkpoint))) };
        let output = format!(concat!(
            "{{\"event\":\"control_migration_preparation_files\",\"schema\":\"preparation-physical-inventory-v1\",",
            "\"scope\":\"preparation_tree_only\",\"namespace_id\":{},\"backend\":{},",
            "\"catalog_snapshot_sha256\":\"{}\",\"profile_sha256\":\"{}\",\"inventory_sha256\":\"{}\",",
            "\"inventory_basis\":\"names_sizes_mtimes_and_all_reference_bytes\",\"preparation_directory_present\":{},",
            "\"inventory_directories\":{},\"inventory_files\":{},\"current_profile_references\":{},",
            "\"current_profile_targets\":{},\"unmapped_references\":{},\"unmapped_objects\":{},\"temporary_files\":{},",
            "\"missing_current_profile_references\":{},\"after_file\":{},\"next_file\":{},",
            "\"page_files\":{},\"page_encoded_bytes\":{},\"entries\":[{}],\"page_sha256\":\"{}\",",
            "\"next_cursor\":{},\"exhausted\":{},\"read_only\":true,\"page_encoded_bytes_hashed\":true,",
            "\"all_object_contents_hashed\":false,\"unmapped_is_orphan\":false,\"deletion_authorized\":false,",
            "\"cutover_revalidation_required\":true,\"canonical_mapping_complete\":false}}"
        ), json_string(&self.namespace_id()), json_string(self.protector.backend_name()),
            sha256::hex(&catalog), sha256::hex(&profile), sha256::hex(&inventory.digest),
            inventory.directories.contains("."), inventory.directories.len(), inventory.files.len(),
            count(Kind::CurrentReference), count(Kind::CurrentTarget), count(Kind::UnmappedReference),
            count(Kind::UnmappedObject), count(Kind::Temporary), inventory.missing_current_references,
            first, next, rows.len(), encoded_bytes, body, sha256::hex(&page_digest), next_cursor, exhausted);
        if output.len() > RESPONSE_BYTES { return Err("DIRECT_MIGRATION_PAGE_TOO_LARGE".to_owned()); }
        check(deadline)?;
        Ok(output)
    }

    fn preparation_inventory(&self, deadline: Instant) -> Result<Inventory, String> {
        let mut inventory = physical_inventory(&self.root, deadline)?;
        let namespace = self.inner.namespace_id();
        // Borrow the sole catalog's bounded inventory; do not build another revision map.
        for metadata in self.inner.retained_revisions() {
            check(deadline)?;
            let binding = binding(&namespace, &metadata)?;
            let key = lookup_key(&binding, &self.protector);
            let hex = sha256::hex(&key);
            let relative = format!("refs/{}/{hex}.ref", &hex[..2]);
            let Some(entry) = inventory.files.get_mut(&relative) else {
                inventory.missing_current_references += 1;
                continue;
            };
            if entry.kind != Kind::UnmappedReference { return Err(invalid()); }
            let saved = entry.reference.as_ref().ok_or_else(invalid)?;
            let (digest, _) = decode_reference(saved, &key, &self.protector).map_err(str::to_owned)?;
            let id = object_id(&binding, &self.protector, &digest);
            let target = format!("objects/{}/{id}.{}", &id[..2], extension(&self.protector));
            entry.kind = Kind::CurrentReference;
            entry.target = Some(target.clone());
            let object = inventory.files.get_mut(&target)
                .ok_or_else(|| "DIRECT_PREPARATION_REFERENCED_OBJECT_MISSING".to_owned())?;
            if object.kind != Kind::UnmappedObject { return Err("DIRECT_PREPARATION_REFERENCE_CONFLICT".to_owned()); }
            object.kind = Kind::CurrentTarget;
        }
        let mut digest = sha256::digest_parts(b"eliot-search/preparation-inventory-seed/v1", &[]);
        for directory in &inventory.directories {
            check(deadline)?;
            digest = sha256::digest_parts(b"eliot-search/preparation-inventory-directory/v1", &[&digest, directory.as_bytes()]);
        }
        for entry in inventory.files.values() {
            check(deadline)?;
            let modified = entry.modified.duration_since(UNIX_EPOCH).map_err(|_| invalid())?;
            let reference = entry.reference.as_ref().map_or(&[][..], |bytes| bytes.as_slice());
            digest = sha256::digest_parts(b"eliot-search/preparation-inventory-file/v1", &[
                &digest, entry.relative.as_bytes(), &entry.size.to_be_bytes(), &modified.as_secs().to_be_bytes(),
                &modified.subsec_nanos().to_be_bytes(), entry.kind.tag().as_bytes(), reference,
                entry.target.as_deref().unwrap_or("").as_bytes(),
            ]);
        }
        inventory.digest = digest;
        Ok(inventory)
    }
}

fn physical_inventory(root: &Path, deadline: Instant) -> Result<Inventory, String> {
    check(deadline)?;
    ensure_directory(root)?;
    let base = root.join("preparation");
    let mut result = Inventory { directories: BTreeSet::new(), files: BTreeMap::new(),
        missing_current_references: 0, digest: [0; 32] };
    match fs::symlink_metadata(&base) {
        Ok(_) => ensure_directory(&base)?,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(result),
        Err(_) => return Err(invalid()),
    }
    result.directories.insert(".".to_owned());
    let mut trees = BTreeSet::new();
    for item in fs::read_dir(&base).map_err(|_| invalid())? {
        check(deadline)?;
        let item = item.map_err(|_| invalid())?;
        let tree = item.file_name().into_string().map_err(|_| invalid())?;
        if !matches!(tree.as_str(), "refs" | "objects") || !trees.insert(tree.clone()) { return Err(invalid()); }
        ensure_directory(&item.path())?;
        result.directories.insert(tree.clone());
        let mut shards = BTreeSet::new();
        for shard in fs::read_dir(item.path()).map_err(|_| invalid())? {
            check(deadline)?;
            if shards.len() >= 256 { return Err("DIRECT_MIGRATION_SHARD_LIMIT".to_owned()); }
            let shard = shard.map_err(|_| invalid())?;
            let shard_name = shard.file_name().into_string().map_err(|_| invalid())?;
            if !hex_len(&shard_name, 2) || !shards.insert(shard_name.clone()) { return Err(invalid()); }
            ensure_directory(&shard.path())?;
            result.directories.insert(format!("{tree}/{shard_name}"));
            for file in fs::read_dir(shard.path()).map_err(|_| invalid())? {
                check(deadline)?;
                if result.files.len() >= MAX_FILES { return Err("DIRECT_MIGRATION_OBJECT_LIMIT".to_owned()); }
                let file = file.map_err(|_| invalid())?;
                let name = file.file_name().into_string().map_err(|_| invalid())?;
                let (id, kind) = generated(&name, &tree).ok_or_else(invalid)?;
                if !id.starts_with(&shard_name) { return Err(invalid()); }
                let (size, modified) = stamp(&fs::symlink_metadata(file.path()).map_err(|_| invalid())?)?;
                let relative = format!("{tree}/{shard_name}/{name}");
                let mut artifact = Artifact { relative: relative.clone(), size, modified, kind, reference: None, target: None };
                if kind == Kind::UnmappedReference {
                    let bytes = read_artifact(root, &artifact, REF_BYTES, deadline)?;
                    let key = sha256::decode_digest(id).ok_or_else(invalid)?;
                    decode_reference_fields(&bytes, &key).map_err(str::to_owned)?;
                    artifact.reference = Some(bytes.as_slice().try_into().map_err(|_| invalid())?);
                }
                if result.files.insert(relative, artifact).is_some() { return Err(invalid()); }
            }
        }
    }
    Ok(result)
}

fn generated<'a>(name: &'a str, tree: &str) -> Option<(&'a str, Kind)> {
    if name.len() > MAX_NAME_BYTES || !name.is_ascii() { return None; }
    if tree == "refs" {
        if let Some(id) = name.strip_suffix(".ref").filter(|id| hex_len(id, 64)) {
            return Some((id, Kind::UnmappedReference));
        }
    } else {
        for suffix in [".bin", ".dpapi"] {
            if let Some(id) = name.strip_suffix(suffix).filter(|id| hex_len(id, 64)) {
                return Some((id, Kind::UnmappedObject));
            }
        }
    }
    // Both reference and object writes use persist_immutable_object's exact temporary grammar.
    let parts = name.strip_prefix('.')?.strip_suffix(".dpapi.tmp")?.splitn(4, '.').collect::<Vec<_>>();
    let [id, pid, time] = parts.as_slice() else { return None; };
    if !hex_len(id, 64) || !decimal(pid) || !decimal(time) { return None; }
    Some((*id, Kind::Temporary))
}
fn hex_len(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn decimal(value: &str) -> bool { !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) }
fn invalid() -> String { "DIRECT_MIGRATION_UNEXPECTED_PREPARATION_OBJECT".to_owned() }
fn check(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline { Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned()) } else { Ok(()) }
}
fn stamp(metadata: &Metadata) -> Result<(u64, SystemTime), String> {
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    if !metadata.is_file() || metadata.file_type().is_symlink() || reparse || metadata.len() > MAX_OBJECT_BYTES as u64 {
        return Err(invalid());
    }
    Ok((metadata.len(), metadata.modified().map_err(|_| invalid())?))
}
fn read_artifact(root: &Path, entry: &Artifact, maximum: usize, deadline: Instant) -> Result<Zeroizing<Vec<u8>>, String> {
    check(deadline)?;
    ensure_directory(root)?;
    let base = root.join("preparation");
    ensure_directory(&base)?;
    let mut directory = base.clone();
    // The exact three components were produced by generated-name validation, not user input.
    for component in entry.relative.split('/').take(2) {
        directory.push(component);
        ensure_directory(&directory)?;
    }
    let path = base.join(&entry.relative);
    let expected = (entry.size, entry.modified);
    if stamp(&fs::symlink_metadata(&path).map_err(|_| invalid())?)? != expected { return Err(invalid()); }
    let bytes = Zeroizing::new(read_regular_file(&path, maximum, "DIRECT_MIGRATION_PREPARATION_READ_FAILED")?);
    if bytes.len() as u64 != entry.size || stamp(&fs::symlink_metadata(&path).map_err(|_| invalid())?)? != expected
        || entry.reference.as_ref().is_some_and(|saved| saved.as_slice() != bytes.as_slice())
    {
        return Err("DIRECT_MIGRATION_PREPARATION_OBJECT_CHANGED".to_owned());
    }
    check(deadline)?;
    Ok(bytes)
}
