//! Qualified root/file identity and same-handle rebinding proofs.

use std::fs::{self, File};
use std::path::Path;

use search_contracts::Blake3Digest32;

use super::path::path_identity_bytes;
use super::spec::{AdapterError, qualified_profile};

/// Computes the stable logical-root identity digest for one canonical root.
pub fn root_identity_digest(canonical_root: &Path) -> Result<Blake3Digest32, AdapterError> {
    let material = root_identity_material(canonical_root)?;
    Ok(blake3_digest(
        b"eliot-search/safe-adapter-root/v1",
        &material,
    ))
}

/// Computes the stable file identity digest for already-gathered material.
#[must_use]
pub fn file_identity_digest(identity_material: &[u8]) -> Blake3Digest32 {
    blake3_digest(b"eliot-search/safe-adapter-file/v1", identity_material)
}

fn blake3_digest(domain: &[u8], material: &[u8]) -> Blake3Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(qualified_profile().as_bytes());
    hasher.update(&material.len().to_be_bytes());
    hasher.update(material);
    Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
}

/// Computes stable logical-root identity material for one canonical root.
#[allow(clippy::unnecessary_wraps)]
fn root_identity_material(canonical_root: &Path) -> Result<Vec<u8>, AdapterError> {
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(canonical_root).map_err(|_| AdapterError::AccessDenied)?;
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&metadata.dev().to_be_bytes());
        bytes.extend_from_slice(&metadata.ino().to_be_bytes());
        Ok(bytes)
    }
    #[cfg(windows)]
    {
        Ok(path_identity_bytes(canonical_root))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(path_identity_bytes(canonical_root))
    }
}

/// Stable identity material for one already-open handle.
///
/// Matches historical DIRECT identity encoding (device/inode on Unix, NTFS
/// volume/file-index on Windows). Returns material, multi-link state and
/// whether the material is a native stable identity.
#[allow(clippy::unnecessary_wraps)]
pub(super) fn handle_identity_material(
    file: &File,
    canonical_final: &Path,
) -> Result<(Vec<u8>, bool, bool), AdapterError> {
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::fs::MetadataExt;
        let _ = canonical_final;
        let metadata = file.metadata().map_err(|_| AdapterError::AccessDenied)?;
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&metadata.dev().to_be_bytes());
        bytes.extend_from_slice(&metadata.ino().to_be_bytes());
        Ok((bytes, metadata.nlink() != 1, true))
    }
    #[cfg(windows)]
    {
        Ok(eliot_searchd::native_file::observe(file).map_or_else(
            |_| (path_identity_bytes(canonical_final), false, false),
            |observed| {
                let links = eliot_searchd::native_file::hardlink_count(file).unwrap_or(u32::MAX);
                (observed.legacy_identity_bytes().to_vec(), links != 1, true)
            },
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Ok((path_identity_bytes(canonical_final), false, false))
    }
}

/// Rebinds the already-open handle to its canonical path identity.
pub(super) fn verify_handle_rebinding(
    file: &File,
    canonical_final: &Path,
    handle_material: &[u8],
) -> Result<(), AdapterError> {
    #[cfg(all(unix, not(windows)))]
    {
        use std::os::unix::fs::MetadataExt;
        let final_metadata =
            fs::metadata(canonical_final).map_err(|_| AdapterError::AccessDenied)?;
        let _ = file;
        let mut expected = Vec::with_capacity(16);
        expected.extend_from_slice(&final_metadata.dev().to_be_bytes());
        expected.extend_from_slice(&final_metadata.ino().to_be_bytes());
        if expected != handle_material {
            return Err(AdapterError::AccessDenied);
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        let second = File::open(canonical_final).map_err(|_| AdapterError::AccessDenied)?;
        let first =
            eliot_searchd::native_file::observe(file).map_err(|_| AdapterError::AccessDenied)?;
        let other =
            eliot_searchd::native_file::observe(&second).map_err(|_| AdapterError::AccessDenied)?;
        if first.volume_serial != other.volume_serial || first.file_index != other.file_index {
            return Err(AdapterError::AccessDenied);
        }
        let _ = handle_material;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, canonical_final, handle_material);
        Ok(())
    }
}

pub(super) fn modified_nanos(metadata: &std::fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

pub(super) fn attribute_bits(metadata: &std::fs::Metadata) -> u64 {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        u64::from(metadata.file_attributes())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        u64::from(metadata.mode())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        0
    }
}
