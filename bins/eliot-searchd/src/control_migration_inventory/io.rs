//! Fixed inventory observation seams over the original migration context.

use std::ffi::OsString;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::Path;
use search_control_redb::migration::LegacyInventoryStatus as Status;
use search_revision_store::LegacyRevisionObjectWork as Work;
use zeroize::Zeroizing;
use super::{MigrationSource, sha256};

pub(super) const PAGE_NAMES: usize = 32;
const NAME_BYTES: usize = 2048;
const CHUNK: usize = 64 * 1024;
pub(super) const CONTROL_BYTES: usize = 128 * 1024 * 1024;

pub(super) struct Name {
    pub(super) value: OsString,
    pub(super) key: Vec<u8>,
}

pub(super) enum Observed {
    Missing,
    Object(Metadata),
    Refused(Status),
}

pub(super) fn metadata(source: &MigrationSource<'_, '_, '_>, path: &Path) -> Result<Observed, String> {
    source.charge(Work::Observe)?;
    let observed = fs::symlink_metadata(path);
    source.checkpoint()?;
    Ok(match observed {
        Ok(value) if is_reparse(&value) => Observed::Refused(Status::Gap),
        Ok(value) => Observed::Object(value),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Observed::Missing,
        Err(_) => Observed::Refused(Status::Unreadable),
    })
}

pub(super) fn native_name(name: &std::ffi::OsStr) -> Vec<u8> {
    #[cfg(windows)]
    { use std::os::windows::ffi::OsStrExt;
        name.encode_wide().flat_map(u16::to_be_bytes).collect() }
    #[cfg(unix)]
    { use std::os::unix::ffi::OsStrExt; name.as_bytes().to_vec() }
    #[cfg(not(any(windows, unix)))]
    { name.as_encoded_bytes().to_vec() }
}

pub(super) fn is_reparse(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0 }
    #[cfg(not(windows))]
    { metadata.file_type().is_symlink() }
}

fn unchanged(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.is_dir() == right.is_dir()
        && left.modified().ok() == right.modified().ok() && !is_reparse(right)
}

/// A complete rescan selects only the next 32 names in native canonical order.
/// A consumed page is never the whole directory denominator.
pub(super) fn page(source: &MigrationSource<'_, '_, '_>, directory: &Path,
    after: Option<&[u8]>) -> Result<Result<Vec<Name>, Status>, String> {
    let before = match metadata(source, directory)? {
        Observed::Missing => return Ok(Err(Status::AbsentProven)),
        Observed::Refused(status) => return Ok(Err(status)),
        Observed::Object(meta) if meta.is_dir() => meta,
        Observed::Object(_) => return Ok(Err(Status::Corrupt)),
    };
    source.charge(Work::Open)?;
    let entries = fs::read_dir(directory);
    source.checkpoint()?;
    let Ok(mut entries) = entries else { return Ok(Err(Status::Unreadable)); };
    source.charge(Work::Allocate(PAGE_NAMES * std::mem::size_of::<Name>()))?;
    let mut window = Vec::with_capacity(PAGE_NAMES);
    loop {
        source.charge(Work::Observe)?;
        let next = entries.next();
        source.checkpoint()?;
        let Some(next) = next else { break; };
        let Ok(entry) = next else { return Ok(Err(Status::Unreadable)); };
        // The admitted basename ceiling reserves the getter's native-name copy.
        source.charge(Work::Allocate(NAME_BYTES * 2))?;
        let value = entry.file_name();
        let key = native_name(&value);
        if key.len() > NAME_BYTES { return Err("DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned()); }
        source.charge(Work::Compare(key.len().saturating_mul(7)))?;
        if after.is_some_and(|cursor| key.as_slice() <= cursor) { continue; }
        let index = window.binary_search_by(|name: &Name| name.key.cmp(&key)).unwrap_or_else(|index| index);
        if index < PAGE_NAMES {
            if window.len() == PAGE_NAMES { window.pop(); }
            window.insert(index, Name { value, key });
        }
    }
    match metadata(source, directory)? {
        Observed::Object(after) if unchanged(&before, &after) => Ok(Ok(window)),
        _ => Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()),
    }
}

pub(super) fn visit(source: &MigrationSource<'_, '_, '_>, directory: &Path,
    mut visitor: impl FnMut(&Name) -> Result<(), String>) -> Result<Option<Status>, String> {
    let mut cursor: Option<Vec<u8>> = None;
    loop {
        let window = match page(source, directory, cursor.as_deref())? {
            Ok(window) => window,
            Err(status) => return Ok(Some(status)),
        };
        if window.is_empty() { return Ok(None); }
        for name in &window { visitor(name)?; }
        if let Some(last) = window.last() {
            source.charge(Work::Allocate(last.key.len()))?;
            cursor = Some(last.key.clone());
        }
    }
}

/// Stable same-handle finite reads. Buffers never enter the artifact encoder.
pub(super) fn read(source: &MigrationSource<'_, '_, '_>, path: &Path,
    maximum: usize) -> Result<Result<Zeroizing<Vec<u8>>, Status>, String> {
    let before = match metadata(source, path)? {
        Observed::Missing => return Ok(Err(Status::AbsentProven)),
        Observed::Refused(status) => return Ok(Err(status)),
        Observed::Object(meta) if meta.is_file() => meta,
        Observed::Object(_) => return Ok(Err(Status::Corrupt)),
    };
    let Ok(length) = usize::try_from(before.len()) else { return Ok(Err(Status::Unsupported)); };
    if length > maximum { return Ok(Err(Status::Unsupported)); }
    source.charge(Work::Open)?;
    let file = File::open(path);
    source.checkpoint()?;
    let Ok(mut file) = file else { return Ok(Err(Status::Unreadable)); };
    source.charge(Work::Observe)?;
    let held = file.metadata();
    source.checkpoint()?;
    let Ok(held) = held else { return Ok(Err(Status::Unreadable)); };
    if !unchanged(&before, &held) { return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()); }
    source.charge(Work::Observe)?;
    let identity = identity(&file);
    source.checkpoint()?;
    let Ok(identity) = identity else { return Ok(Err(Status::Unreadable)); };
    source.charge(Work::Allocate(length))?;
    // Reserve clear-on-drop processing before allocating the private buffer.
    source.charge(Work::Compare(length))?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes.try_reserve_exact(length).map_err(|_| "DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned())?;
    bytes.resize(length, 0);
    let mut offset = 0;
    while offset < length {
        let requested = (length - offset).min(CHUNK);
        source.charge(Work::Read(requested))?;
        let result = file.read(&mut bytes[offset..offset + requested]);
        source.checkpoint()?;
        match result { Ok(0) => return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()),
            Ok(count) => offset += count, Err(_) => return Ok(Err(Status::Unreadable)) }
    }
    source.charge(Work::Read(1))?;
    let extra = file.read(&mut [0]);
    source.checkpoint()?;
    if !matches!(extra, Ok(0)) { return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()); }
    source.charge(Work::Observe)?;
    let after = file.metadata();
    source.checkpoint()?;
    source.charge(Work::Observe)?;
    let final_identity = self::identity(&file);
    source.checkpoint()?;
    if !matches!(after, Ok(ref after) if unchanged(&held, after))
        || !matches!(final_identity, Ok(value) if value == identity) {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    match metadata(source, path)? {
        Observed::Object(current) if unchanged(&held, &current) => {}
        _ => return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()),
    }
    source.charge(Work::Open)?;
    let current = File::open(path).and_then(|file| self::identity(&file));
    source.checkpoint()?;
    if !matches!(current, Ok(value) if value == identity) {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    Ok(Ok(bytes))
}

fn identity(file: &File) -> std::io::Result<(u64, u64)> {
    #[cfg(windows)]
    { let value = eliot_searchd::native_file::observe(file)?;
        Ok((u64::from(value.volume_serial), value.file_index)) }
    #[cfg(unix)]
    { use std::os::unix::fs::MetadataExt; let value = file.metadata()?;
        Ok((value.dev(), value.ino())) }
    #[cfg(not(any(windows, unix)))]
    { let _ = file; Err(std::io::Error::other("unsupported identity")) }
}

pub(super) fn fingerprint(source: &MigrationSource<'_, '_, '_>, bytes: &[u8]) -> Result<[u8; 32], String> {
    source.charge(Work::Compare(bytes.len()))?;
    let digest = sha256::digest(bytes);
    source.checkpoint()?;
    Ok(digest)
}
