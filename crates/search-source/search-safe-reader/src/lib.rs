//! One bounded no-execute read from an explicitly selected local file.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_source_registry::{NativeFileKey, MAX_SOURCE_OBJECT_BYTES};

/// Maximum bytes accepted by the first whole-file source operation.
pub const MAX_SOURCE_FILE_BYTES: u64 = MAX_SOURCE_OBJECT_BYTES;

/// Closed failures from explicit local-file acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceReadError {
    /// The explicit path does not name an opened regular file.
    NotRegularFile,
    /// The file is missing or could not be opened.
    OpenFailed,
    /// The file exceeds the bounded whole-file read.
    TooLarge,
    /// Native handle identity is unavailable on this filesystem.
    IdentityUnavailable,
    /// Handle identity, length or native change metadata changed during the read.
    ChangedDuringRead,
    /// Allocation for the bounded body failed.
    AllocationFailed,
    /// The operation deadline expired.
    Deadline,
    /// Cancellation was observed.
    Cancelled,
    /// A file read or metadata operation failed.
    Io,
}

impl SourceReadError {
    /// Stable content-free machine error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotRegularFile => "SOURCE_NOT_REGULAR_FILE",
            Self::OpenFailed => "SOURCE_OPEN_FAILED",
            Self::TooLarge => "SOURCE_TOO_LARGE",
            Self::IdentityUnavailable => "SOURCE_IDENTITY_UNAVAILABLE",
            Self::ChangedDuringRead => "SOURCE_CHANGED_DURING_READ",
            Self::AllocationFailed => "SOURCE_ALLOCATION_FAILED",
            Self::Deadline => "SOURCE_DEADLINE",
            Self::Cancelled => "SOURCE_CANCELLED",
            Self::Io => "SOURCE_IO",
        }
    }
}

/// Exact bytes and native file identity returned by one stable handle read.
#[derive(Debug, Eq, PartialEq)]
pub struct AcquiredFile {
    bytes: Vec<u8>,
    file_key: NativeFileKey,
    byte_length: u64,
}

impl AcquiredFile {
    /// Consumes the result and transfers the original acquired allocation.
    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, NativeFileKey, u64) {
        (self.bytes, self.file_key, self.byte_length)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeObservation {
    file_key: NativeFileKey,
    byte_length: u64,
    version: NativeVersion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NativeVersion {
    #[cfg(unix)]
    Unix {
        modified: i64,
        modified_nanos: i64,
        changed: i64,
        changed_nanos: i64,
    },
    #[cfg(windows)]
    Windows {
        creation_time: u64,
        last_write_time: u64,
        attributes: u32,
    },
}

fn observe(file: &File) -> Result<NativeObservation, SourceReadError> {
    let metadata = file.metadata().map_err(read_error)?;
    if !metadata.is_file() {
        return Err(SourceReadError::NotRegularFile);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(NativeObservation {
            file_key: NativeFileKey::Unix {
                device: metadata.dev(),
                inode: metadata.ino(),
            },
            byte_length: metadata.len(),
            version: NativeVersion::Unix {
                modified: metadata.mtime(),
                modified_nanos: metadata.mtime_nsec(),
                changed: metadata.ctime(),
                changed_nanos: metadata.ctime_nsec(),
            },
        })
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(SourceReadError::NotRegularFile);
        }
        let information = winapi_util::file::information(file)
            .map_err(|_| SourceReadError::IdentityUnavailable)?;
        let volume_serial = u32::try_from(information.volume_serial_number())
            .map_err(|_| SourceReadError::IdentityUnavailable)?;
        let file_index = information.file_index();
        Ok(NativeObservation {
            file_key: NativeFileKey::Windows {
                volume_serial,
                file_index,
            },
            byte_length: metadata.len(),
            version: NativeVersion::Windows {
                creation_time: metadata.creation_time(),
                last_write_time: metadata.last_write_time(),
                attributes: metadata.file_attributes(),
            },
        })
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Err(SourceReadError::IdentityUnavailable)
    }
}

fn check(deadline: Instant, cancellation: &AtomicBool) -> Result<(), SourceReadError> {
    if cancellation.load(Ordering::Relaxed) {
        return Err(SourceReadError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(SourceReadError::Deadline);
    }
    Ok(())
}

fn read_error(error: &io::Error) -> SourceReadError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        SourceReadError::ChangedDuringRead
    } else {
        SourceReadError::Io
    }
}

#[cfg(any(unix, windows))]
fn source_open_error(error: io::Error) -> SourceReadError {
    #[cfg(windows)]
    if error
        .raw_os_error()
        .and_then(|code| u32::try_from(code).ok())
        == Some(windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION)
    {
        return SourceReadError::ChangedDuringRead;
    }
    #[cfg(unix)]
    if error.raw_os_error() == Some(libc::ELOOP) {
        return SourceReadError::NotRegularFile;
    }
    if error.kind() == io::ErrorKind::NotFound {
        SourceReadError::OpenFailed
    } else {
        read_error(&error)
    }
}

fn open_source_file(path: &Path) -> Result<File, SourceReadError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        };

        // Deny write and delete access for the lifetime of this read handle.
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(source_open_error)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(source_open_error)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(SourceReadError::IdentityUnavailable)
    }
}

fn same_handle_read_core(
    file: &mut File,
    maximum_bytes: u64,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<AcquiredFile, SourceReadError> {
    check(deadline, cancellation)?;
    let before = observe(file)?;
    if before.byte_length > maximum_bytes {
        return Err(SourceReadError::TooLarge);
    }
    let capacity = usize::try_from(before.byte_length).map_err(|_| SourceReadError::TooLarge)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| SourceReadError::AllocationFailed)?;
    bytes.resize(capacity, 0);

    check(deadline, cancellation)?;
    file.read_exact(&mut bytes).map_err(read_error)?;
    let mut extra = [0_u8; 1];
    if file.read(&mut extra).map_err(read_error)? != 0 {
        return Err(SourceReadError::ChangedDuringRead);
    }
    check(deadline, cancellation)?;

    let after = observe(file)?;
    if before != after {
        return Err(SourceReadError::ChangedDuringRead);
    }
    check(deadline, cancellation)?;
    Ok(AcquiredFile {
        bytes,
        file_key: before.file_key,
        byte_length: before.byte_length,
    })
}

/// Reads one explicitly selected file through a single opened handle.
///
/// The exact same handle supplies the before/after native identity, size and
/// change metadata. Windows denies concurrent write/delete access and refuses
/// opened reparse points; Unix refuses a symbolic link at the final component.
/// There is no path-parent admission rule, source execution, network access,
/// retry, or identity fallback.
///
/// # Errors
///
/// Returns a [`SourceReadError`] if the size bound is invalid, the selected file
/// cannot be opened or identified as a supported regular file, the file changes
/// during acquisition, a bounded read or allocation fails, or cancellation or
/// the deadline is observed.
pub fn acquire_file(
    path: &Path,
    maximum_bytes: u64,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<AcquiredFile, SourceReadError> {
    if maximum_bytes == 0 || maximum_bytes > MAX_SOURCE_FILE_BYTES {
        return Err(SourceReadError::TooLarge);
    }
    check(deadline, cancellation)?;
    let mut file = open_source_file(path)?;
    same_handle_read_core(&mut file, maximum_bytes, deadline, cancellation)
}
