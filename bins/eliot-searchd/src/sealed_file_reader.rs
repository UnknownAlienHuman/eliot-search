//! Same-handle bounded Windows source reader for sealed ingest.
//!
//! The final object is opened with `FILE_FLAG_OPEN_REPARSE_POINT`; native file
//! identity and mutable metadata are compared before and after the bounded read.
//! No path is reopened after bytes are admitted.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;
use std::path::Path;

use crate::sealed_store::{MAX_PLAINTEXT_BYTES, SensitiveBytes};

/// Closed final-handle read failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FinalFileReadError {
    /// The current platform does not provide the Windows final-handle adapter.
    UnsupportedPlatform,
    /// The requested path could not be opened.
    OpenFailed,
    /// The final object is not a regular file.
    NotRegularFile,
    /// A symlink, junction, mount point, or another reparse object was observed.
    ReparsePointDenied,
    /// Source bytes exceed the finite ingest ceiling.
    SourceTooLarge,
    /// File bytes could not be read.
    ReadFailed,
    /// Native object identity or mutable metadata changed during the read.
    ChangedDuringRead,
    /// Empty source bytes cannot enter the sealed revision store.
    EmptySource,
}

impl FinalFileReadError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_FILE_READER_UNSUPPORTED_PLATFORM",
            Self::OpenFailed => "SEALED_FILE_READER_OPEN_FAILED",
            Self::NotRegularFile => "SEALED_FILE_READER_NOT_REGULAR",
            Self::ReparsePointDenied => "SEALED_FILE_READER_REPARSE_POINT_DENIED",
            Self::SourceTooLarge => "SEALED_FILE_READER_SOURCE_TOO_LARGE",
            Self::ReadFailed => "SEALED_FILE_READER_READ_FAILED",
            Self::ChangedDuringRead => "SEALED_FILE_READER_CHANGED_DURING_READ",
            Self::EmptySource => "SEALED_FILE_READER_EMPTY_SOURCE",
        }
    }
}

impl fmt::Display for FinalFileReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for FinalFileReadError {}

/// Content-free same-handle read evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalFileReadReceipt {
    /// Exact byte count read from the final handle.
    pub byte_length: u64,
    /// Native volume serial observed from the final handle.
    pub volume_serial: Option<u32>,
    /// Native stable file index observed from the final handle.
    pub file_index: Option<u64>,
    /// Last-write timestamp remained stable during the read.
    pub last_write_stable: bool,
    /// Creation timestamp remained stable during the read.
    pub creation_time_stable: bool,
    /// The same open final handle supplied both observations and all bytes.
    pub same_handle_verified: bool,
    /// Reparse attributes were absent before and after reading.
    pub reparse_free: bool,
}

/// Bounded plaintext and content-free final-handle evidence.
pub struct FinalFileRead {
    /// Exact source bytes. The allocation is overwritten on drop.
    pub plaintext: SensitiveBytes,
    /// Same-handle observation receipt.
    pub receipt: FinalFileReadReceipt,
}

impl fmt::Debug for FinalFileRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FinalFileRead")
            .field("plaintext", &"<redacted>")
            .field("receipt", &self.receipt)
            .finish()
    }
}

/// Reads one exact regular Windows file through one final handle.
pub fn read_final_file(path: &Path) -> Result<FinalFileRead, FinalFileReadError> {
    platform::read_final_file(path)
}

#[cfg(not(windows))]
mod platform {
    use super::{FinalFileRead, FinalFileReadError};
    use std::path::Path;

    pub(super) fn read_final_file(
        _path: &Path,
    ) -> Result<FinalFileRead, FinalFileReadError> {
        Err(FinalFileReadError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        FinalFileRead, FinalFileReadError, FinalFileReadReceipt,
        MAX_PLAINTEXT_BYTES, SensitiveBytes,
    };
    use std::fs::{self, File, OpenOptions};
    use std::io::Read;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::path::Path;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Observation {
        length: u64,
        volume_serial: Option<u32>,
        file_index: Option<u64>,
        last_write_time: u64,
        creation_time: u64,
        file_attributes: u32,
    }

    pub(super) fn read_final_file(
        path: &Path,
    ) -> Result<FinalFileRead, FinalFileReadError> {
        let path_metadata = fs::symlink_metadata(path)
            .map_err(|_| FinalFileReadError::OpenFailed)?;
        if path_metadata.file_type().is_symlink()
            || path_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(FinalFileReadError::ReparsePointDenied);
        }
        if !path_metadata.is_file() {
            return Err(FinalFileReadError::NotRegularFile);
        }

        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_SEQUENTIAL_SCAN)
            .open(path)
            .map_err(|_| FinalFileReadError::OpenFailed)?;
        let before = observe(&file)?;
        if before.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(FinalFileReadError::ReparsePointDenied);
        }
        if before.length == 0 {
            return Err(FinalFileReadError::EmptySource);
        }
        if before.length > u64::try_from(MAX_PLAINTEXT_BYTES).unwrap_or(u64::MAX) {
            return Err(FinalFileReadError::SourceTooLarge);
        }

        let capacity = usize::try_from(before.length)
            .map_err(|_| FinalFileReadError::SourceTooLarge)?;
        let mut bytes = Vec::with_capacity(capacity);
        (&mut file)
            .take(u64::try_from(MAX_PLAINTEXT_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|_| FinalFileReadError::ReadFailed)?;
        if bytes.len() > MAX_PLAINTEXT_BYTES {
            return Err(FinalFileReadError::SourceTooLarge);
        }

        let after = observe(&file)?;
        if before != after
            || bytes.len() != usize::try_from(before.length).unwrap_or(usize::MAX)
        {
            return Err(FinalFileReadError::ChangedDuringRead);
        }
        let plaintext = SensitiveBytes::new(bytes).map_err(|error| match error {
            crate::sealed_store::SealedStoreError::EmptyPlaintext => {
                FinalFileReadError::EmptySource
            }
            crate::sealed_store::SealedStoreError::PlaintextTooLarge => {
                FinalFileReadError::SourceTooLarge
            }
            _ => FinalFileReadError::ReadFailed,
        })?;
        Ok(FinalFileRead {
            plaintext,
            receipt: FinalFileReadReceipt {
                byte_length: before.length,
                volume_serial: before.volume_serial,
                file_index: before.file_index,
                last_write_stable: true,
                creation_time_stable: true,
                same_handle_verified: true,
                reparse_free: true,
            },
        })
    }

    fn observe(file: &File) -> Result<Observation, FinalFileReadError> {
        let metadata = file
            .metadata()
            .map_err(|_| FinalFileReadError::OpenFailed)?;
        if !metadata.is_file() {
            return Err(FinalFileReadError::NotRegularFile);
        }
        let native = eliot_searchd::native_file::observe(file).map_err(|error| match error {
            eliot_searchd::native_file::ObservationError::ReparsePointDenied => {
                FinalFileReadError::ReparsePointDenied
            }
            _ => FinalFileReadError::OpenFailed,
        })?;
        Ok(Observation {
            length: native.length,
            volume_serial: Some(native.volume_serial),
            file_index: Some(native.file_index),
            last_write_time: native.last_write_time,
            creation_time: native.creation_time,
            file_attributes: native.attributes,
        })
    }
}
