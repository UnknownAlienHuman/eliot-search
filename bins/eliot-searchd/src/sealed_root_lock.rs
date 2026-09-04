//! Exclusive process-local lease for the Windows sealed data root.
//!
//! This is an OS exclusion primitive, not a monotone `OwnerEpoch`. It prevents
//! concurrent mutation by two cooperating processes and emits no success until
//! the exact lock file has been acquired and its bounded owner record synced.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;
use std::path::Path;

/// Closed sealed-root lease failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedRootLockError {
    /// The current platform does not provide the Windows adapter.
    UnsupportedPlatform,
    /// Data root is absent or not a directory.
    InvalidDataRoot,
    /// Data root or lock object is a reparse point.
    ReparsePointDenied,
    /// Another cooperating process owns the sealed root.
    AlreadyOwned,
    /// Lock file could not be opened or written.
    IoFailure,
    /// OS file-lock acquisition failed for another reason.
    LockFailure,
}

impl SealedRootLockError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_ROOT_LOCK_UNSUPPORTED_PLATFORM",
            Self::InvalidDataRoot => "SEALED_ROOT_LOCK_DATA_ROOT_INVALID",
            Self::ReparsePointDenied => "SEALED_ROOT_LOCK_REPARSE_POINT_DENIED",
            Self::AlreadyOwned => "SEALED_ROOT_LOCK_ALREADY_OWNED",
            Self::IoFailure => "SEALED_ROOT_LOCK_IO_FAILURE",
            Self::LockFailure => "SEALED_ROOT_LOCK_FAILED",
        }
    }
}

impl fmt::Display for SealedRootLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedRootLockError {}

/// Held exclusive data-root lease.
///
/// The OS lock is released only when this non-cloneable guard is dropped.
pub struct SealedRootLease {
    inner: platform::PlatformLease,
}

impl SealedRootLease {
    /// Acquires the exact sealed data-root OS exclusion primitive.
    pub fn acquire(data_root: &Path) -> Result<Self, SealedRootLockError> {
        Ok(Self {
            inner: platform::PlatformLease::acquire(data_root)?,
        })
    }

    /// Whether the OS exclusion primitive remains held by this process.
    #[must_use]
    pub fn is_held(&self) -> bool {
        self.inner.is_held()
    }
}

impl fmt::Debug for SealedRootLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SealedRootLease")
            .field("held", &self.is_held())
            .finish_non_exhaustive()
    }
}

#[cfg(not(windows))]
mod platform {
    use super::SealedRootLockError;
    use std::path::Path;

    pub(super) struct PlatformLease;

    impl PlatformLease {
        pub(super) fn acquire(_data_root: &Path) -> Result<Self, SealedRootLockError> {
            Err(SealedRootLockError::UnsupportedPlatform)
        }

        pub(super) const fn is_held(&self) -> bool {
            false
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::SealedRootLockError;
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Seek, SeekFrom, Write};
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const LOCK_FILE: &str = ".eliot-search-sealed-owner.lock";
    const MAX_OWNER_RECORD_BYTES: usize = 4 * 1024;

    pub(super) struct PlatformLease {
        file: File,
        held: bool,
    }

    impl PlatformLease {
        pub(super) fn acquire(data_root: &Path) -> Result<Self, SealedRootLockError> {
            let root_metadata = fs::symlink_metadata(data_root)
                .map_err(|_| SealedRootLockError::InvalidDataRoot)?;
            if !root_metadata.is_dir() {
                return Err(SealedRootLockError::InvalidDataRoot);
            }
            if root_metadata.file_type().is_symlink()
                || root_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(SealedRootLockError::ReparsePointDenied);
            }

            let lock_path = data_root.join(LOCK_FILE);
            if let Ok(metadata) = fs::symlink_metadata(&lock_path) {
                if metadata.file_type().is_symlink()
                    || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
                    || !metadata.is_file()
                {
                    return Err(SealedRootLockError::ReparsePointDenied);
                }
            }
            let mut file = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .open(&lock_path)
                .map_err(|_| SealedRootLockError::IoFailure)?;
            let metadata = file
                .metadata()
                .map_err(|_| SealedRootLockError::IoFailure)?;
            if !metadata.is_file()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(SealedRootLockError::ReparsePointDenied);
            }
            file.try_lock().map_err(|error| {
                if error.kind() == io::ErrorKind::WouldBlock {
                    SealedRootLockError::AlreadyOwned
                } else {
                    SealedRootLockError::LockFailure
                }
            })?;

            let started_at_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| SealedRootLockError::IoFailure)?
                .as_millis();
            let record = format!(
                concat!(
                    "ELIOT-SEALED-OWNER-V1\n",
                    "pid={}\n",
                    "started_at_unix_ms={}\n",
                    "state=ACTIVE\n"
                ),
                std::process::id(),
                started_at_ms,
            );
            if record.len() > MAX_OWNER_RECORD_BYTES {
                let _ = file.unlock();
                return Err(SealedRootLockError::IoFailure);
            }
            if file
                .set_len(0)
                .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
                .and_then(|()| file.write_all(record.as_bytes()))
                .and_then(|()| file.sync_all())
                .is_err()
            {
                let _ = file.unlock();
                return Err(SealedRootLockError::IoFailure);
            }
            Ok(Self { file, held: true })
        }

        pub(super) const fn is_held(&self) -> bool {
            self.held
        }
    }

    impl Drop for PlatformLease {
        fn drop(&mut self) {
            if self.held {
                let _ = self.file.set_len(0);
                let _ = self.file.sync_all();
                let _ = self.file.unlock();
                self.held = false;
            }
        }
    }
}
