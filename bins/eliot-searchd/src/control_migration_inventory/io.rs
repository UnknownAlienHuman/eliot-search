//! Fixed inventory observation seams over the original migration context.

use super::{MigrationSource, sha256};
use search_control_redb::migration::LegacyInventoryStatus as Status;
use search_revision_store::{
    LegacyRevisionObjectWork as Work, LegacyRevisionProtection,
    LEGACY_REVISION_DIRECTORY, LEGACY_REVISION_MAX_OBJECT_BYTES,
    legacy_revision_object_relative_locator,
};
use std::cell::RefCell;
use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use zeroize::Zeroizing;

pub(super) const PAGE_NAMES: usize = 32;
pub(super) const NAME_BYTES: usize = 2048;
pub(super) const CHUNK: usize = 64 * 1024;
pub(super) const MAX_PARENTS: usize = 8;
pub(super) const CONTROL_BYTES: usize = 128 * 1024 * 1024;

// One finite locator snapshot per directory, never a corpus/body cache.
// Excess names refuse before any visitor sees a partial denominator.
pub(super) const DIRECTORY_NAMES: usize = 2048;

pub(super) struct Name {
    pub(super) value: OsString,
    pub(super) key: Vec<u8>,
}

// The complete admitted locator view of one directory. Only basenames are
// admitted; no body, plaintext, digest or payload is retained, and no locator
// is copied into a second index, store or projection.
struct Enumerated {
    names: Vec<Name>,
    before: Metadata,
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

// V2 correlation only: root-relative native components, never a source identity
// or permission to delete. The original operation admits allocation and hashing.
pub(super) fn locator(
    source: &MigrationSource<'_, '_, '_>,
    path: &Path,
) -> Result<[u8; 32], String> {
    let bytes = locator_preimage(source.inventory_root(), path, |length| {
        source.charge(Work::Compare(length))?;
        source.charge(Work::Allocate(length))
    })?;
    fingerprint(source, &bytes)
}

pub(super) fn child_locator(
    source: &MigrationSource<'_, '_, '_>,
    directory: &Path,
    name: &std::ffi::OsStr,
) -> Result<[u8; 32], String> {
    source.charge(Work::Allocate(
        native_length(directory.as_os_str()).saturating_add(native_length(name)).saturating_add(2),
    ))?;
    locator(source, &directory.join(name))
}

fn locator_preimage(
    root: &Path,
    path: &Path,
    admit: impl FnOnce(usize) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    const DOMAIN: &[u8] = b"eliot-search/legacy-inventory/native-locator/v2\0";
    let relative = path.strip_prefix(root).map_err(|_| changed())?;
    let mut count = 0_u16;
    let mut length = DOMAIN.len() + 3;
    for part in relative.components() {
        let Component::Normal(name) = part else { return Err(changed()); };
        count = count.checked_add(1).ok_or_else(changed)?;
        let bytes = native_length(name);
        if usize::from(count) > MAX_PARENTS || bytes == 0 || bytes > NAME_BYTES {
            return Err(changed());
        }
        length = length.checked_add(4 + bytes).ok_or_else(changed)?;
    }
    admit(length)?;
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(DOMAIN);
    bytes.push(if cfg!(windows) { 1 } else if cfg!(unix) { 2 } else { 3 });
    bytes.extend_from_slice(&count.to_be_bytes());
    for part in relative.components() {
        let Component::Normal(name) = part else { return Err(changed()); };
        let size = u32::try_from(native_length(name)).map_err(|_| changed())?;
        bytes.extend_from_slice(&size.to_be_bytes());
        append_native(&mut bytes, name);
    }
    Ok(bytes)
}

fn native_length(name: &std::ffi::OsStr) -> usize {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        name.encode_wide().count().saturating_mul(2)
    }
    #[cfg(not(windows))]
    { name.as_encoded_bytes().len() }
}

fn append_native(bytes: &mut Vec<u8>, name: &std::ffi::OsStr) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in name.encode_wide() { bytes.extend_from_slice(&unit.to_be_bytes()); }
    }
    #[cfg(not(windows))]
    { bytes.extend_from_slice(name.as_encoded_bytes()); }
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

/// One held final representation. Its snapshot describes this original handle,
/// not a body digest or a replacement opened later through the locator.
pub(super) struct PinnedRevisionRepresentation {
    file: File,
    #[cfg(test)]
    protection: LegacyRevisionProtection,
    snapshot: eliot_searchd::native_file::Observation,
    metadata: Metadata,
}

impl PinnedRevisionRepresentation {
    #[cfg(test)]
    pub(super) const fn protection(&self) -> LegacyRevisionProtection {
        self.protection
    }

    pub(super) const fn native_identity(&self) -> (u64, u64) {
        (self.snapshot.volume_serial as u64, self.snapshot.file_index)
    }

    pub(super) const fn encoded_bytes(&self) -> u64 {
        self.snapshot.length
    }

    #[cfg(test)]
    pub(super) const fn snapshot(&self) -> &eliot_searchd::native_file::Observation {
        &self.snapshot
    }

    #[cfg(test)]
    pub(super) const fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

/// Read-only custody for one revision's at-most-two final representations.
/// Open before the source-owner callback and keep this value alive until its
/// observations and both explicit absences have been checked afterward.
pub(super) struct PinnedRevisionRepresentations<'a, 'view, 'operation, 'owner> {
    source: &'a MigrationSource<'view, 'operation, 'owner>,
    paths: [PathBuf; 2],
    originals: [Option<PinnedRevisionRepresentation>; 2],
    parents: Vec<File>,
    parent_snapshots: [Option<eliot_searchd::native_file::Observation>; MAX_PARENTS],
}

impl<'a, 'view, 'operation, 'owner>
    PinnedRevisionRepresentations<'a, 'view, 'operation, 'owner>
{
    /// Proven missing-both is `AbsentProven`; all other native refusals keep
    /// their inventory status. An invalid ID or observed drift is an error.
    pub(super) fn open(
        source: &'a MigrationSource<'view, 'operation, 'owner>,
        revision_id: &str,
    ) -> Result<Result<Self, Status>, String> {
        source.checkpoint()?;
        if revision_id.len() != 64 { return Err(changed()); }
        source.charge(Work::Compare(revision_id.len()))?;
        source.charge(Work::AllocateLocator(2 * (64 + 2 + 1 + 6)))?;
        let plain = legacy_revision_object_relative_locator(revision_id, LegacyRevisionProtection::Plaintext)
            .ok_or_else(changed)?;
        let protected = legacy_revision_object_relative_locator(revision_id, LegacyRevisionProtection::Protected)
            .ok_or_else(changed)?;
        if !cfg!(windows) { return Ok(Err(Status::Unsupported)); }
        source.charge(Work::Allocate(std::mem::size_of::<Self>()))?;
        source.charge(Work::AllocateLocator(2 * (native_length(source.inventory_root().as_os_str())
            + LEGACY_REVISION_DIRECTORY.len() * 2 + 80 * 2)))?;
        let directory = source.inventory_root().join(LEGACY_REVISION_DIRECTORY);
        let paths = [directory.join(plain), directory.join(protected)];
        let parents = match pin_parents(source, &paths[0], false)? {
            Ok(parents) => parents,
            Err(status) => return Ok(Err(status)),
        };
        let mut parent_snapshots = [None; MAX_PARENTS];
        for (index, file) in parents.iter().enumerate() {
            parent_snapshots[index] = match revision_pin_observation(source, file)? {
                Ok(snapshot) => Some(snapshot),
                Err(status) => return Ok(Err(status)),
            };
        }
        let mut originals = [None, None];
        for (index, protection) in [LegacyRevisionProtection::Plaintext, LegacyRevisionProtection::Protected]
            .into_iter().enumerate()
        {
            originals[index] = match pin_final_revision(source, &paths[index], protection)? {
                Ok(original) => original,
                Err(status) => return Ok(Err(status)),
            };
        }
        let held = Self { source, paths, originals, parents, parent_snapshots };
        if let Err(status) = held.verify_same_original()? { return Ok(Err(status)); }
        if held.originals.iter().all(Option::is_none) { return Ok(Err(Status::AbsentProven)); }
        Ok(Ok(held))
    }

    /// Each protection class has its own presence, identity and encoded length.
    pub(super) const fn representation(
        &self, protection: LegacyRevisionProtection,
    ) -> Option<&PinnedRevisionRepresentation> {
        self.originals[revision_protection_index(protection)].as_ref()
    }

    /// Outer absence means no raw representation; a present raw observation
    /// with no original native identity never matches a production pin.
    pub(super) fn verify_readback(
        &self, protection: LegacyRevisionProtection,
        raw: Option<(Option<(u64, u64)>, u64)>,
    ) -> Result<(), String> {
        self.source.checkpoint()?;
        match (self.representation(protection), raw) {
            (None, None) => Ok(()),
            (Some(held), Some((Some(identity), length)))
                if identity == held.native_identity() && length == held.encoded_bytes() => Ok(()),
            _ => Err(changed()),
        }
    }

    /// Reobserve held originals and pinned parents after the callback, without
    /// body reads. Locator metadata only rejects drift; it grants no authority.
    pub(super) fn verify_same_original(&self) -> Result<Result<(), Status>, String> {
        self.source.checkpoint()?;
        for (index, path) in self.paths.iter().enumerate() {
            match &self.originals[index] {
                None => match metadata(self.source, path)? {
                    Observed::Missing => {}
                    Observed::Refused(status) => return Ok(Err(status)),
                    Observed::Object(_) => return Err(changed()),
                },
                Some(original) => {
                    self.source.charge(Work::Observe)?;
                    let current = original.file.metadata();
                    self.source.checkpoint()?;
                    let Ok(current) = current else { return Ok(Err(Status::Unreadable)); };
                    if !current.is_file() || !unchanged(&original.metadata, &current) { return Err(changed()); }
                    let snapshot = match revision_pin_observation(self.source, &original.file)? {
                        Ok(snapshot) => snapshot,
                        Err(status) => return Ok(Err(status)),
                    };
                    if snapshot != original.snapshot { return Err(changed()); }
                    if let Err(status) = revision_pin_single_link(self.source, &original.file)? {
                        return Ok(Err(status));
                    }
                    match metadata(self.source, path)? {
                        Observed::Object(current) if current.is_file()
                            && unchanged(&original.metadata, &current) => {}
                        Observed::Refused(status) => return Ok(Err(status)),
                        _ => return Err(changed()),
                    }
                }
            }
        }
        for (index, file) in self.parents.iter().enumerate() {
            let snapshot = match revision_pin_observation(self.source, file)? {
                Ok(snapshot) => snapshot,
                Err(status) => return Ok(Err(status)),
            };
            if Some(snapshot) != self.parent_snapshots[index] { return Err(changed()); }
        }
        self.source.checkpoint()?;
        Ok(Ok(()))
    }
}

const fn revision_protection_index(protection: LegacyRevisionProtection) -> usize {
    match protection {
        LegacyRevisionProtection::Plaintext => 0,
        LegacyRevisionProtection::Protected => 1,
    }
}

fn pin_final_revision(
    source: &MigrationSource<'_, '_, '_>, path: &Path, _protection: LegacyRevisionProtection,
) -> Result<Result<Option<PinnedRevisionRepresentation>, Status>, String> {
    let before = match metadata(source, path)? {
        Observed::Missing => return Ok(Ok(None)),
        Observed::Refused(status) => return Ok(Err(status)),
        Observed::Object(meta) if meta.is_file() => meta,
        Observed::Object(_) => return Ok(Err(Status::Corrupt)),
    };
    if before.len() > LEGACY_REVISION_MAX_OBJECT_BYTES as u64 { return Err(limit()); }
    source.charge(Work::Open)?;
    let file = open_nofollow(path, false);
    source.checkpoint()?;
    let Ok(file) = file else { return Ok(Err(Status::Unreadable)); };
    source.charge(Work::Observe)?;
    let held = file.metadata();
    source.checkpoint()?;
    let Ok(held) = held else { return Ok(Err(Status::Unreadable)); };
    if !held.is_file() || !unchanged(&before, &held) { return Err(changed()); }
    let snapshot = match revision_pin_observation(source, &file)? {
        Ok(snapshot) => snapshot,
        Err(status) => return Ok(Err(status)),
    };
    if snapshot.length != held.len() { return Err(changed()); }
    if let Err(status) = revision_pin_single_link(source, &file)? { return Ok(Err(status)); }
    Ok(Ok(Some(PinnedRevisionRepresentation {
        file,
        #[cfg(test)]
        protection: _protection,
        snapshot,
        metadata: held,
    })))
}

fn revision_pin_observation(
    source: &MigrationSource<'_, '_, '_>, file: &File,
) -> Result<Result<eliot_searchd::native_file::Observation, Status>, String> {
    use eliot_searchd::native_file::ObservationError;
    let refusal = RefCell::new(None);
    let observed = eliot_searchd::native_file::observe_with_check(file, &|before| {
        let result = if before { source.charge(Work::Observe) } else { source.checkpoint() };
        result.map_err(|cause| {
            refusal.borrow_mut().get_or_insert(cause);
            ObservationError::ObservationFailed
        })
    });
    if let Some(cause) = refusal.into_inner() { return Err(cause); }
    Ok(observed.map_err(|error| match error {
        ObservationError::UnsupportedPlatform | ObservationError::UnsupportedHandle
        | ObservationError::UnsupportedFileSystem => Status::Unsupported,
        ObservationError::ReparsePointDenied => Status::Gap,
        ObservationError::ObservationFailed => Status::Unreadable,
    }))
}

fn revision_pin_single_link(
    source: &MigrationSource<'_, '_, '_>, file: &File,
) -> Result<Result<(), Status>, String> {
    source.checkpoint()?;
    // The existing hardlink observer performs these three fixed native calls.
    #[cfg(windows)]
    for _ in 0..3 { source.charge(Work::Observe)?; }
    let links = eliot_searchd::native_file::hardlink_count(file);
    source.checkpoint()?;
    Ok(match links {
        Ok(1) => Ok(()),
        _ => Err(Status::Gap),
    })
}

/// One pinned, deterministic enumeration of a complete directory.
///
/// The directory is read exactly once while every ancestor stays pinned and the
/// pinned handle retains its identity across the whole walk. Each admitted
/// basename locator is ordered by its native canonical key, and the complete
/// sequence is handed to the consumer in fixed 32-name windows. No page is
/// produced by rescanning the directory, so enumerating a directory of N names
/// costs one walk plus one canonical ordering rather than
/// N * (ceil(N / 32) + 1) whole-directory rescans. A directory that exceeds the
/// admitted ceilings refuses the whole enumeration instead of emitting a silent
/// prefix: the caller receives the limit reason, never a partial denominator.
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
    let Enumerated { names, .. } = match enumerate_pinned(source, directory, &pins, identity)? {
        Ok(enumerated) => enumerated,
        Err(status) => return Ok(Err(status)),
    };
    source.charge(Work::Allocate(PAGE_NAMES * std::mem::size_of::<Name>()))?;
    Ok(Ok(names
        .into_iter()
        .filter(|name| after.is_none_or(|cursor| name.key.as_slice() > cursor))
        .take(PAGE_NAMES)
        .collect()))
}

fn directory_identity(source: &MigrationSource<'_, '_, '_>, pins: &[File])
    -> Result<eliot_searchd::native_file::Observation, String> {
    source.charge(Work::Observe)?;
    let observed = pins.last().ok_or_else(changed).and_then(|file| identity(file).map_err(|_| changed()));
    source.checkpoint()?;
    observed
}

fn enumerate_pinned(source: &MigrationSource<'_, '_, '_>, directory: &Path,
    pins: &[File], expected: eliot_searchd::native_file::Observation)
    -> Result<Result<Enumerated, Status>, String> {
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
    let mut names: Vec<Name> = Vec::new();
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
        if names.len() == DIRECTORY_NAMES { return Err(limit()); }
        if names.len() == names.capacity() {
            let capacity = names.capacity().saturating_mul(2).clamp(PAGE_NAMES, DIRECTORY_NAMES);
            source.charge(Work::Allocate(capacity * std::mem::size_of::<Name>()))?;
            names.try_reserve_exact(capacity - names.len()).map_err(|_| limit())?;
        }
        // The admitted basename ceiling reserves the getter's native-name copy.
        source.charge(Work::Allocate(NAME_BYTES * 3))?;
        source.charge(Work::Compare(NAME_BYTES * 2))?;
        let value = entry.file_name();
        let key = native_name(&value);
        if key.len() > NAME_BYTES {
            return Err(limit());
        }
        names.push(Name { value, key });
    }
    order_names(source, &mut names)?;
    source.checkpoint()?;
    let result = match metadata(source, directory)? {
        Observed::Object(after)
            if unchanged(&before, &after) => Ok(Ok(Enumerated { names, before })),
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
    // One pinned enumeration of the whole directory. The admitted locator
    // collection is finite and already metered inside enumerate_pinned, so the
    // windows below retain no second window buffer and re-enumerate nothing.
    let Enumerated { names, before } = match enumerate_pinned(source, directory, &pins, identity)? {
        Ok(enumerated) => enumerated,
        Err(Status::AbsentProven) => return Ok(Some(Status::AbsentProven)),
        Err(status) => return Ok(Some(status)),
    };
    // Stream the canonically ordered sequence in fixed 32-name windows while
    // the ancestor pins stay held. The order is the canonical order the
    // artifact has always carried, so rows and terminals are unchanged.
    let mut offset = 0;
    while offset < names.len() {
        let end = (offset + PAGE_NAMES).min(names.len());
        source.checkpoint()?;
        for name in &names[offset..end] {
            visitor(name)?;
        }
        offset = end;
    }
    if directory_identity(source, &pins)? != identity { return Err(changed()); }
    match metadata(source, directory)? {
        Observed::Object(after) if unchanged(&before, &after) => Ok(None),
        _ => Err(changed()),
    }
}

// Bottom-up merge ordering has at most n * ceil(log2(n)) key comparisons.
// Two index buffers avoid cloning names or allocating sorting scratch inside
// an opaque comparator. Each merge pass is bounded and polls the same context.
fn order_names(source: &MigrationSource<'_, '_, '_>, names: &mut [Name]) -> Result<(), String> {
    let key_bytes = names.iter().map(|name| name.key.len()).max().unwrap_or(0);
    order_metadata(source, names, key_bytes, |left, right| left.key.cmp(&right.key))?;
    if names.windows(2).any(|pair| pair[0].key == pair[1].key) { return Err(changed()); }
    source.checkpoint()
}

pub(super) fn order_metadata<T>(source: &MigrationSource<'_, '_, '_>, values: &mut [T],
    key_bytes: usize, compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> Result<(), String> {
    let count = values.len();
    if count < 2 { return Ok(()); }
    let passes = usize::try_from((count - 1).ilog2() + 1).map_err(|_| limit())?;
    let index_bytes = std::mem::size_of::<usize>();
    let operations = count.checked_mul(passes + 1).ok_or_else(limit)?;
    let compare_bytes = operations.checked_mul(2 * key_bytes + 2 * index_bytes)
        .and_then(|bytes| bytes.checked_add(3 * count * std::mem::size_of::<T>()))
        .ok_or_else(limit)?;
    source.charge(Work::Allocate(2 * count * index_bytes))?;
    source.charge(Work::Compare(compare_bytes))?;
    let mut order = Vec::new();
    let mut scratch = Vec::new();
    order.try_reserve_exact(count).map_err(|_| limit())?;
    scratch.try_reserve_exact(count).map_err(|_| limit())?;
    order.extend(0..count);
    scratch.resize(count, 0);
    merge_order(values, &compare, &mut order, &mut scratch, || source.checkpoint())?;
    // Convert sorted-position -> original-position into the inverse permutation.
    for (position, original) in order.into_iter().enumerate() { scratch[original] = position; }
    for position in 0..count {
        while scratch[position] != position {
            let target = scratch[position];
            values.swap(position, target);
            scratch.swap(position, target);
        }
    }
    source.checkpoint()
}

fn merge_order<T>(values: &[T], compare: &impl Fn(&T, &T) -> std::cmp::Ordering,
    order: &mut Vec<usize>, scratch: &mut Vec<usize>,
    mut checkpoint: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let mut width = 1;
    while width < values.len() {
        let mut start = 0;
        while start < values.len() {
            let middle = (start + width).min(values.len());
            let end = (middle + width).min(values.len());
            let (mut left, mut right) = (start, middle);
            for destination in &mut scratch[start..end] {
                if right == end || (left < middle && compare(&values[order[left]], &values[order[right]]).is_le()) {
                    *destination = order[left]; left += 1;
                } else {
                    *destination = order[right]; right += 1;
                }
            }
            start = end;
        }
        std::mem::swap(order, scratch);
        checkpoint()?;
        width = width.saturating_mul(2);
    }
    Ok(())
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

#[cfg(test)]
mod locator_tests {
    use super::*;

    fn encoded(relative: &str) -> Vec<u8> {
        let root = Path::new("inventory-root");
        locator_preimage(root, &root.join(relative), |_| Ok(())).unwrap()
    }

    #[test]
    fn location_evidence_distinguishes_parent_tree_and_component_boundaries() {
        let original = encoded("revisions/aa/note.bin");
        for relocated in ["revisions/bb/note.bin", "preparation/aa/note.bin",
            "revisions/a/anote.bin", "revisions/aa-note/bin"] {
            assert_ne!(sha256::digest(&original), sha256::digest(&encoded(relocated)));
        }
        let other_root = Path::new("another-local-root");
        assert_eq!(original, locator_preimage(other_root,
            &other_root.join("revisions/aa/note.bin"), |_| Ok(())).unwrap());
    }

    #[test]
    fn invalid_locator_refuses_before_allocation_admission() {
        let root = Path::new("inventory-root");
        for path in [Path::new("elsewhere/item").to_path_buf(),
            root.join("../outside"), root.join("a/b/c/d/e/f/g/h/i"),
            root.join("x".repeat(NAME_BYTES + 1))] {
            assert!(locator_preimage(root, &path, |_| panic!("must not admit")).is_err());
        }
        assert!(locator_preimage(root, &root.join("valid"),
            |_| Err("ORIGINAL_REFUSAL".to_owned())).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn native_locator_preserves_nonunicode_utf16_without_lossy_conversion() {
        use std::os::windows::ffi::OsStringExt;
        let root = Path::new("inventory-root");
        let name = OsString::from_wide(&[0xd800, 0x0061]);
        let bytes = locator_preimage(root, &root.join(name), |_| Ok(())).unwrap();
        assert!(bytes.ends_with(&[0, 0, 0, 4, 0xd8, 0, 0, 0x61]));
        assert_ne!(bytes, encoded("\u{fffd}a"));
    }
}
