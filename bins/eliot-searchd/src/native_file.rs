//! Same-handle observations for the existing Windows NTFS identity encoding.
//!
//! No path is opened, no bytes are read and no state is persisted here. Native
//! handles/structs never leave the ABI module. A successful observation is not
//! a source-admission, containment, owner-authority or currentness receipt.

use core::fmt;
use std::fs::File;

/// Closed, content-free native observation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationError {
    /// Windows native observation is unavailable on this target.
    UnsupportedPlatform,
    /// The handle is not a disk file/directory or its type could not be read.
    UnsupportedHandle,
    /// This legacy 64-bit identity representation is only supported on NTFS.
    UnsupportedFileSystem,
    /// The operating system did not return all requested information.
    ObservationFailed,
    /// The exact opened object is a reparse object.
    ReparsePointDenied,
}

impl ObservationError {
    /// Stable diagnostic without paths, credentials or raw OS error text.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "NATIVE_FILE_UNSUPPORTED_PLATFORM",
            Self::UnsupportedHandle => "NATIVE_FILE_UNSUPPORTED_HANDLE",
            Self::UnsupportedFileSystem => "NATIVE_FILE_UNSUPPORTED_FILESYSTEM",
            Self::ObservationFailed => "NATIVE_FILE_OBSERVATION_FAILED",
            Self::ReparsePointDenied => "NATIVE_FILE_REPARSE_POINT_DENIED",
        }
    }
}

impl fmt::Display for ObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ObservationError {}

/// Information measured on one already-open object, excluding access time.
///
/// Access time can change because of the read being verified. Identity alone
/// is not a timeless source identity: deleted file IDs can eventually be reused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    /// Observed volume serial; never a replacement for an unavailable value.
    pub volume_serial: u32,
    /// Observed NTFS file index, with both 32-bit words preserved.
    pub file_index: u64,
    /// Exact native attributes at this observation.
    pub attributes: u32,
    /// File byte length (not meaningful for a directory).
    pub length: u64,
    /// Native creation time in Windows FILETIME units.
    pub creation_time: u64,
    /// Native last-write time in Windows FILETIME units.
    pub last_write_time: u64,
}

impl Observation {
    /// Existing persisted NTFS identity layout: volume u32 BE, file index u64 BE.
    #[must_use]
    pub fn legacy_identity_bytes(self) -> [u8; 12] {
        let mut bytes = [0_u8; 12];
        bytes[..4].copy_from_slice(&self.volume_serial.to_be_bytes());
        bytes[4..].copy_from_slice(&self.file_index.to_be_bytes());
        bytes
    }
}

/// Observes a borrowed final handle using stable Win32 APIs.
///
/// Does not reopen a locator, take ownership of the handle or fabricate a
/// missing identity. The old 64-bit encoding is retained only on NTFS: ReFS
/// requires a separately versioned 128-bit identity and migration, not truncation.
///
/// # Errors
/// Returns a closed failure for unsupported platform/handle/filesystem, reparse
/// objects or unsuccessful native observation. There is no path/zero fallback.
pub fn observe(file: &File) -> Result<Observation, ObservationError> {
    platform::observe(file)
}

#[cfg(not(windows))]
mod platform {
    use super::{File, Observation, ObservationError};

    pub(super) fn observe(_file: &File) -> Result<Observation, ObservationError> {
        Err(ObservationError::UnsupportedPlatform)
    }
}

// The exception covers only the checked ABI calls, not callers or serializers.
#[cfg(windows)]
#[allow(unsafe_code)]
#[path = "native_file/windows.rs"]
mod platform;

#[cfg(test)]
#[path = "native_file/tests.rs"]
mod tests;
