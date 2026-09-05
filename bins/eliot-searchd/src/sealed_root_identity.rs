//! Exact native identity for one Windows sealed data root.
//!
//! The identity binds the native volume/file index and canonical UTF-16 final
//! path. It is used to prove that an [`OwnerEpochGuard`] is being applied to the
//! same physical root that issued it.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;
use std::path::Path;

use crate::sealed_digest::{DigestError, Sha256Digest};
use crate::sealed_owner_epoch::OwnerEpochGuard;

/// Closed root-identity failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RootIdentityError {
    /// Windows native root identity is unavailable on this platform.
    UnsupportedPlatform,
    /// Root is absent or not a directory.
    InvalidRoot,
    /// Root is a symlink, junction, mount point, or another reparse object.
    ReparsePointDenied,
    /// Native metadata or canonical final path could not be read.
    ObservationFailed,
    /// Root identity differs from the current owner guard.
    OwnerBindingMismatch,
    /// Windows CNG SHA-256 failed.
    Digest(DigestError),
}

impl RootIdentityError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_ROOT_IDENTITY_UNSUPPORTED_PLATFORM",
            Self::InvalidRoot => "SEALED_ROOT_IDENTITY_INVALID_ROOT",
            Self::ReparsePointDenied => "SEALED_ROOT_IDENTITY_REPARSE_POINT_DENIED",
            Self::ObservationFailed => "SEALED_ROOT_IDENTITY_OBSERVATION_FAILED",
            Self::OwnerBindingMismatch => "SEALED_ROOT_IDENTITY_OWNER_MISMATCH",
            Self::Digest(error) => error.code(),
        }
    }
}

impl fmt::Display for RootIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RootIdentityError {}

impl From<DigestError> for RootIdentityError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

/// Computes the exact native/canonical root-binding digest.
pub fn root_binding_sha256(data_root: &Path) -> Result<Sha256Digest, RootIdentityError> {
    platform::root_binding_sha256(data_root)
}

/// Verifies that an owner guard covers this exact physical data root.
pub fn verify_owner_root(
    data_root: &Path,
    owner: &OwnerEpochGuard,
) -> Result<(), RootIdentityError> {
    if !owner.root_lock_held()
        || owner.epoch() == 0
        || root_binding_sha256(data_root)? != owner.root_binding_sha256()
    {
        return Err(RootIdentityError::OwnerBindingMismatch);
    }
    Ok(())
}

#[cfg(not(windows))]
mod platform {
    use super::{RootIdentityError, Sha256Digest};
    use std::path::Path;

    pub(super) fn root_binding_sha256(
        _data_root: &Path,
    ) -> Result<Sha256Digest, RootIdentityError> {
        Err(RootIdentityError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{RootIdentityError, Sha256Digest};
    use crate::sealed_digest::sha256;
    use std::fs::{self, OpenOptions};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::path::Path;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    pub(super) fn root_binding_sha256(
        data_root: &Path,
    ) -> Result<Sha256Digest, RootIdentityError> {
        let path_metadata = fs::symlink_metadata(data_root)
            .map_err(|_| RootIdentityError::InvalidRoot)?;
        if !path_metadata.is_dir() {
            return Err(RootIdentityError::InvalidRoot);
        }
        if path_metadata.file_type().is_symlink()
            || path_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(RootIdentityError::ReparsePointDenied);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(data_root)
            .map_err(|_| RootIdentityError::ObservationFailed)?;
        let metadata = file
            .metadata()
            .map_err(|_| RootIdentityError::ObservationFailed)?;
        if !metadata.is_dir() {
            return Err(RootIdentityError::InvalidRoot);
        }
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(RootIdentityError::ReparsePointDenied);
        }
        let canonical = fs::canonicalize(data_root)
            .map_err(|_| RootIdentityError::ObservationFailed)?;
        let observed = eliot_searchd::native_file::observe(&file)
            .map_err(|_| RootIdentityError::ObservationFailed)?;
        let mut binding = b"eliot-search/sealed-root-binding/v1\0".to_vec();
        binding.extend_from_slice(&observed.legacy_identity_bytes());
        for unit in canonical.as_os_str().encode_wide() {
            binding.extend_from_slice(&unit.to_le_bytes());
        }
        sha256(&binding).map_err(RootIdentityError::from)
    }
}
