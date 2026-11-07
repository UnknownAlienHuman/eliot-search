//! Conservative collection of current-format, unmarked immutable object leaves.

use std::ffi::OsStr;
use std::fs::{self, File, Metadata};
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_contracts::Blake3Digest32;
use search_runtime_owner::PlainRootGuard;
use search_source_registry::{NativeFileKey, ObjectKind, ObjectRef};

use super::{MAX_SOURCE_OBJECT_BYTES, ObjectStoreError, check, object_name, read_open_object};

const MAX_PREPARED_OBJECT_BYTES: u64 = 64 * 1024 * 1024;

const fn maximum_bytes(kind: ObjectKind) -> u64 {
    match kind {
        ObjectKind::Source => MAX_SOURCE_OBJECT_BYTES,
        ObjectKind::Prepared => MAX_PREPARED_OBJECT_BYTES,
    }
}

const fn allowed_length(kind: ObjectKind, length: u64) -> bool {
    length <= maximum_bytes(kind) && (matches!(kind, ObjectKind::Source) || length != 0)
}

fn parse_name(name: &OsStr) -> Option<(ObjectKind, Blake3Digest32)> {
    let name = name.to_str()?;
    let (kind, digest) = if let Some(digest) = name.strip_prefix("v1-source-") {
        (ObjectKind::Source, digest)
    } else {
        (ObjectKind::Prepared, name.strip_prefix("v1-prepared-")?)
    };
    if digest.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (byte, pair) in bytes
        .iter_mut()
        .zip(digest.as_bytes().as_chunks::<2>().0.iter())
    {
        *byte = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some((kind, Blake3Digest32::from_bytes(bytes)))
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub fn regular(metadata: &Metadata) -> bool {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

fn entry_metadata(path: &Path) -> Result<Option<Metadata>, ObjectStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ObjectStoreError::Io),
    }
}

/// Classifies one caller-enumerated name without scanning or reading object bodies.
///
/// Only canonical current-format regular objects within the Source/Prepared bounds
/// become candidates. Foreign names, links, reparses, absent entries and uncertain
/// ownership are preserved. A candidate is not proof that the object is unreferenced.
///
/// # Errors
///
/// Returns an I/O, deadline or cancellation error rather than continuing an
/// uncertain sweep after an actual filesystem failure.
pub fn collection_candidate(
    root: &PlainRootGuard,
    name: &OsStr,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Option<ObjectRef>, ObjectStoreError> {
    check(deadline, cancel)?;
    let Some((kind, digest)) = parse_name(name) else {
        return Ok(None);
    };
    let Some(metadata) = entry_metadata(&root.object_dir().join(name))? else {
        return Ok(None);
    };
    check(deadline, cancel)?;
    if !regular(&metadata) || !allowed_length(kind, metadata.len()) {
        return Ok(None);
    }
    let reference = match kind {
        ObjectKind::Source => ObjectRef::source(digest, metadata.len()),
        ObjectKind::Prepared => ObjectRef::prepared(digest, metadata.len()),
    };
    if OsStr::new(&object_name(reference, maximum_bytes(kind))?) != name {
        return Ok(None);
    }
    Ok(Some(reference))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeObservation {
    file_key: NativeFileKey,
    byte_length: u64,
    #[cfg(unix)]
    modified: (i64, i64),
    #[cfg(unix)]
    changed: (i64, i64),
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
    #[cfg(windows)]
    attributes: u32,
}

impl NativeObservation {
    fn matches_metadata(&self, metadata: &Metadata) -> bool {
        if !regular(metadata) || self.byte_length != metadata.len() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            self.file_key
                == (NativeFileKey::Unix {
                    device: metadata.dev(),
                    inode: metadata.ino(),
                })
                && self.modified == (metadata.mtime(), metadata.mtime_nsec())
                && self.changed == (metadata.ctime(), metadata.ctime_nsec())
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;

            self.creation_time == metadata.creation_time()
                && self.last_write_time == metadata.last_write_time()
                && self.attributes == metadata.file_attributes()
        }
        #[cfg(not(any(unix, windows)))]
        {
            false
        }
    }
}

fn observe(file: &File) -> Result<Option<NativeObservation>, ObjectStoreError> {
    let metadata = file.metadata().map_err(|_| ObjectStoreError::Io)?;
    if !regular(&metadata) {
        return Ok(None);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        Ok(Some(NativeObservation {
            file_key: NativeFileKey::Unix {
                device: metadata.dev(),
                inode: metadata.ino(),
            },
            byte_length: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }))
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        let information = match winapi_util::file::information(file) {
            Ok(information) => information,
            Err(error) if error.kind() == io::ErrorKind::Unsupported => return Ok(None),
            Err(_) => return Err(ObjectStoreError::Io),
        };
        let Ok(volume_serial) = u32::try_from(information.volume_serial_number()) else {
            return Ok(None);
        };
        Ok(Some(NativeObservation {
            file_key: NativeFileKey::Windows {
                volume_serial,
                file_index: information.file_index(),
            },
            byte_length: metadata.len(),
            creation_time: metadata.creation_time(),
            last_write_time: metadata.last_write_time(),
            attributes: metadata.file_attributes(),
        }))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(None)
    }
}

fn changed_open_error(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::NotFound {
        return true;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;

        if error
            .raw_os_error()
            .and_then(|code| u32::try_from(code).ok())
            == Some(ERROR_SHARING_VIOLATION)
        {
            return true;
        }
    }
    false
}

fn open_observed(path: &Path) -> Result<Option<(File, NativeObservation)>, ObjectStoreError> {
    let Some(before) = entry_metadata(path)? else {
        return Ok(None);
    };
    if !regular(&before) {
        return Ok(None);
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ,
        };

        // Deny writes while hashing, but allow our own pathname unlink below.
        options
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if changed_open_error(&error) => return Ok(None),
        Err(_) => return Err(ObjectStoreError::Io),
    };
    let Some(observation) = observe(&file)? else {
        return Ok(None);
    };
    let Some(after) = entry_metadata(path)? else {
        return Ok(None);
    };
    if !observation.matches_metadata(&before) || !observation.matches_metadata(&after) {
        return Ok(None);
    }
    Ok(Some((file, observation)))
}

/// Removes one verified object after the caller's complete mark proves it unreferenced.
///
/// The caller must finish marking before any sweep and retain the exclusive root
/// guard. Missing, changed, nonregular, unsupported-identity or corrupt leaves are
/// preserved with `Ok(false)`. Length/BLAKE3 verification uses the same opened file
/// whose native identity/version is checked before and after reading and against
/// a fresh pathname handle immediately before unlink.
///
/// The final unlink is pathname-based: the root guard excludes cooperating app
/// writers, but cannot make the last check/unlink atomic against a noncooperating
/// same-account rename or reparse replacement.
///
/// # Errors
///
/// Returns an error for an invalid reference, actual filesystem or allocation
/// failure, cancellation or an expired deadline. Integrity mismatch alone
/// preserves the unmarked file rather than claiming ownership of its bytes.
pub fn remove_unreferenced_object(
    root: &PlainRootGuard,
    expected: ObjectRef,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<bool, ObjectStoreError> {
    check(deadline, cancel)?;
    let name = object_name(expected, maximum_bytes(expected.kind))?;
    if !allowed_length(expected.kind, expected.byte_length) {
        return Err(ObjectStoreError::InvalidReference);
    }
    let path = root.object_dir().join(name);
    let Some((file, before)) = open_observed(&path)? else {
        return Ok(false);
    };
    if before.byte_length != expected.byte_length {
        return Ok(false);
    }
    match read_open_object(&file, expected, deadline, cancel) {
        Ok(_) => {}
        Err(ObjectStoreError::IntegrityFailure) => return Ok(false),
        Err(error) => return Err(error),
    }
    check(deadline, cancel)?;
    if observe(&file)?.as_ref() != Some(&before) {
        return Ok(false);
    }
    let Some((_current, current_observation)) = open_observed(&path)? else {
        return Ok(false);
    };
    if current_observation != before {
        return Ok(false);
    }
    check(deadline, cancel)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if changed_open_error(&error) => Ok(false),
        Err(_) => Err(ObjectStoreError::Io),
    }
}
