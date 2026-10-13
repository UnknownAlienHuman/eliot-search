//! Fixed inventory observation seams over the original migration context.

use super::{MigrationSource, sha256};
use search_control_redb::migration::LegacyInventoryStatus as Status;
use search_revision_store::LegacyRevisionObjectWork as Work;
use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::Read;
use std::path::{Component, Path};
use zeroize::Zeroizing;

pub(super) const PAGE_NAMES: usize = 32;
pub(super) const NAME_BYTES: usize = 2048;
pub(super) const CHUNK: usize = 64 * 1024;
pub(super) const MAX_PARENTS: usize = 8;
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

pub(super) fn metadata(
    source: &MigrationSource<'_, '_, '_>,
    path: &Path,
) -> Result<Observed, String> {
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
    {
        use std::os::windows::ffi::OsStrExt;
        name.encode_wide().flat_map(u16::to_be_bytes).collect()
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        name.as_bytes().to_vec()
    }
    #[cfg(not(any(windows, unix)))]
    {
        name.as_encoded_bytes().to_vec()
    }
}

pub(super) fn is_reparse(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn unchanged(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.is_dir() == right.is_dir()
        && matches!((left.modified(), right.modified()), (Ok(left), Ok(right)) if left == right)
        && !is_reparse(right)
}

// Use the existing native-owner open profile: OPEN_REPARSE_POINT and read-only
// sharing. All ancestors stay pinned until the enumeration/read completes.
// Unsupported native platforms refuse rather than follow an unchecked locator.
fn open_nofollow(path: &Path, directory: bool) -> std::io::Result<File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .share_mode(1)
            .custom_flags(if directory { 0x0220_0000 } else { 0x0020_0000 })
            .open(path)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, directory);
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

fn pin_parents(
    source: &MigrationSource<'_, '_, '_>,
    path: &Path,
    directory: bool,
) -> Result<Result<Vec<File>, Status>, String> {
    let root = source.inventory_root();
    let parent = if directory {
        path
    } else {
        path.parent().ok_or_else(changed)?
    };
    let relative = parent.strip_prefix(root).map_err(|_| changed())?;
    let mut components = relative.components();
    source.charge(Work::Allocate(MAX_PARENTS * std::mem::size_of::<File>()))?;
    source.charge(Work::AllocateLocator(
        root.as_os_str()
            .len()
            .saturating_add(MAX_PARENTS * NAME_BYTES)
            .saturating_mul(2),
    ))?;
    let mut pins = Vec::with_capacity(MAX_PARENTS);
    let mut current = root.to_path_buf();
    loop {
        if pins.len() == MAX_PARENTS {
            return Err(limit());
        }
        match metadata(source, &current)? {
            Observed::Missing => return Ok(Err(Status::AbsentProven)),
            Observed::Refused(status) => return Ok(Err(status)),
            Observed::Object(meta) if meta.is_dir() => {}
            Observed::Object(_) => return Ok(Err(Status::Corrupt)),
        }
        source.charge(Work::Open)?;
        let file = open_nofollow(&current, true);
        source.checkpoint()?;
        let Ok(file) = file else {
            return Ok(Err(Status::Unreadable));
        };
        source.charge(Work::Observe)?;
        let observation = file.metadata();
        let native = identity(&file);
        source.checkpoint()?;
        if !matches!(observation, Ok(ref meta) if meta.is_dir() && !is_reparse(meta))
            || native.is_err()
        {
            return Ok(Err(Status::Gap));
        }
        pins.push(file);
        match components.next() {
            None => return Ok(Ok(pins)),
            Some(Component::Normal(name)) => current.push(name),
            Some(_) => return Err(changed()),
        }
    }
}

fn changed() -> String {
    "DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()
}
fn limit() -> String {
    "DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned()
}

/// A complete rescan selects only the next 32 names in native canonical order.
/// A consumed page is never the whole directory denominator.
#[cfg(test)]
pub(super) fn page(
    source: &MigrationSource<'_, '_, '_>,
    directory: &Path,
    after: Option<&[u8]>,
) -> Result<Result<Vec<Name>, Status>, String> {
    let pins = match pin_parents(source, directory, true)? {
        Ok(pins) => pins,
        Err(status) => return Ok(Err(status)),
    };
    let identity = directory_identity(source, &pins)?;
    page_pinned(source, directory, after, &pins, identity)
}

fn directory_identity(source: &MigrationSource<'_, '_, '_>, pins: &[File])
    -> Result<eliot_searchd::native_file::Observation, String> {
    source.charge(Work::Observe)?;
    let observed = pins.last().ok_or_else(changed).and_then(|file| identity(file).map_err(|_| changed()));
    source.checkpoint()?;
    observed
}

fn page_pinned(source: &MigrationSource<'_, '_, '_>, directory: &Path,
    after: Option<&[u8]>, pins: &[File],
    expected: eliot_searchd::native_file::Observation) -> Result<Result<Vec<Name>, Status>, String> {
    if directory_identity(source, pins)? != expected { return Err(changed()); }
    let before = match metadata(source, directory)? {
        Observed::Missing => return Ok(Err(Status::AbsentProven)),
        Observed::Refused(status) => return Ok(Err(status)),
        Observed::Object(meta) if meta.is_dir() => meta,
        Observed::Object(_) => return Ok(Err(Status::Corrupt)),
    };
    source.charge(Work::Open)?;
    let entries = fs::read_dir(directory);
    source.checkpoint()?;
    let Ok(mut entries) = entries else {
        return Ok(Err(Status::Unreadable));
    };
    source.charge(Work::Allocate(PAGE_NAMES * std::mem::size_of::<Name>()))?;
    let mut window = Vec::with_capacity(PAGE_NAMES);
    loop {
        source.charge(Work::Observe)?;
        let next = entries.next();
        source.checkpoint()?;
        let Some(next) = next else {
            break;
        };
        let Ok(entry) = next else {
            return Ok(Err(Status::Unreadable));
        };
        // The admitted basename ceiling reserves the getter's native-name copy.
        source.charge(Work::Allocate(NAME_BYTES * 2))?;
        let value = entry.file_name();
        let key = native_name(&value);
        if key.len() > NAME_BYTES {
            return Err("DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned());
        }
        source.charge(Work::Compare(key.len().saturating_mul(7)))?;
        if after.is_some_and(|cursor| key.as_slice() <= cursor) {
            continue;
        }
        let index = window
            .binary_search_by(|name: &Name| name.key.cmp(&key))
            .unwrap_or_else(|index| index);
        if index < PAGE_NAMES {
            if window.len() == PAGE_NAMES {
                window.pop();
            }
            window.insert(index, Name { value, key });
        }
    }
    let result = match metadata(source, directory)? {
        Observed::Object(after) if unchanged(&before, &after) => Ok(Ok(window)),
        _ => Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()),
    };
    if directory_identity(source, pins)? != expected { return Err(changed()); }
    result
}

pub(super) fn visit(
    source: &MigrationSource<'_, '_, '_>,
    directory: &Path,
    mut visitor: impl FnMut(&Name) -> Result<(), String>,
) -> Result<Option<Status>, String> {
    let pins = match pin_parents(source, directory, true)? {
        Ok(pins) => pins,
        Err(status) => return Ok(Some(status)),
    };
    let identity = directory_identity(source, &pins)?;
    let mut cursor: Option<Vec<u8>> = None;
    loop {
        let window = match page_pinned(source, directory, cursor.as_deref(), &pins, identity)? {
            Ok(window) => window,
            Err(Status::AbsentProven) if cursor.is_some() => return Err(changed()),
            Err(status) => return Ok(Some(status)),
        };
        if window.is_empty() {
            return Ok(None);
        }
        for name in &window {
            visitor(name)?;
        }
        if let Some(last) = window.last() {
            source.charge(Work::Allocate(last.key.len()))?;
            cursor = Some(last.key.clone());
        }
    }
}

/// Stable same-handle finite reads. Buffers never enter the artifact encoder.
pub(super) fn read(
    source: &MigrationSource<'_, '_, '_>,
    path: &Path,
    maximum: usize,
) -> Result<Result<Zeroizing<Vec<u8>>, Status>, String> {
    let _pins = match pin_parents(source, path, false)? {
        Ok(pins) => pins,
        Err(status) => return Ok(Err(status)),
    };
    let before = match metadata(source, path)? {
        Observed::Missing => return Ok(Err(Status::AbsentProven)),
        Observed::Refused(status) => return Ok(Err(status)),
        Observed::Object(meta) if meta.is_file() => meta,
        Observed::Object(_) => return Ok(Err(Status::Corrupt)),
    };
    let Ok(length) = usize::try_from(before.len()) else {
        return Ok(Err(Status::Unsupported));
    };
    if length > maximum {
        return Err(limit());
    }
    source.charge(Work::Open)?;
    let file = open_nofollow(path, false);
    source.checkpoint()?;
    let Ok(mut file) = file else {
        return Ok(Err(Status::Unreadable));
    };
    source.charge(Work::Observe)?;
    let held = file.metadata();
    source.checkpoint()?;
    let Ok(held) = held else {
        return Ok(Err(Status::Unreadable));
    };
    if !unchanged(&before, &held) {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    source.charge(Work::Observe)?;
    let identity = identity(&file);
    source.checkpoint()?;
    let Ok(identity) = identity else {
        return Ok(Err(Status::Unreadable));
    };
    source.charge(Work::Observe)?;
    let links = eliot_searchd::native_file::hardlink_count(&file);
    source.checkpoint()?;
    if !matches!(links, Ok(1)) { return Ok(Err(Status::Gap)); }
    source.charge(Work::Allocate(length))?;
    // Reserve clear-on-drop processing before allocating the private buffer.
    source.charge(Work::Compare(length))?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(length)
        .map_err(|_| "DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned())?;
    bytes.resize(length, 0);
    let mut offset = 0;
    while offset < length {
        let requested = (length - offset).min(CHUNK);
        source.charge(Work::Read(requested))?;
        let result = file.read(&mut bytes[offset..offset + requested]);
        source.checkpoint()?;
        match result {
            Ok(0) => return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()),
            Ok(count) => offset += count,
            Err(_) => return Ok(Err(Status::Unreadable)),
        }
    }
    source.charge(Work::Read(1))?;
    let extra = file.read(&mut [0]);
    source.checkpoint()?;
    if !matches!(extra, Ok(0)) {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    source.charge(Work::Observe)?;
    let after = file.metadata();
    source.checkpoint()?;
    source.charge(Work::Observe)?;
    let final_identity = self::identity(&file);
    source.checkpoint()?;
    if !matches!(after, Ok(ref after) if unchanged(&held, after))
        || !matches!(final_identity, Ok(value) if value == identity)
    {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    match metadata(source, path)? {
        Observed::Object(current) if unchanged(&held, &current) => {}
        _ => return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()),
    }
    source.charge(Work::Open)?;
    let current = open_nofollow(path, false).and_then(|file| {
        if eliot_searchd::native_file::hardlink_count(&file).map_err(std::io::Error::other)? != 1 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        self::identity(&file)
    });
    source.checkpoint()?;
    if !matches!(current, Ok(value) if value == identity) {
        return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
    }
    Ok(Ok(bytes))
}

fn identity(file: &File) -> std::io::Result<eliot_searchd::native_file::Observation> {
    eliot_searchd::native_file::observe(file).map_err(std::io::Error::other)
}

pub(super) fn fingerprint(
    source: &MigrationSource<'_, '_, '_>,
    bytes: &[u8],
) -> Result<[u8; 32], String> {
    source.charge(Work::Compare(bytes.len()))?;
    let digest = sha256::digest(bytes);
    source.checkpoint()?;
    Ok(digest)
}
