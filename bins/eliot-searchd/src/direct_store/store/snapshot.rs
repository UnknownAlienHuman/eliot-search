//! Final-handle source snapshots admitted into the legacy DIRECT store.

use std::path::{Path, PathBuf};

use search_safe_reader::SafeReadError;

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::safe_reader_adapter::{AdapterError, FullReadError};
use crate::{safe_reader_adapter, sha256};

use super::super::model::{FileSnapshot, IdentityStrength};
use super::filesystem::path_identity_bytes;

pub(in super::super) fn read_file_snapshot(
    path: &Path,
    data_root: &Path,
    remaining_batch_bytes: usize,
) -> Result<FileSnapshot, String> {
    let max_bytes = remaining_batch_bytes.min(MAX_SCAN_INPUT_BYTES);
    let limit_error = if remaining_batch_bytes < MAX_SCAN_INPUT_BYTES {
        "DIRECT_BATCH_BYTES_EXCEEDED"
    } else {
        "DIRECT_SOURCE_TOO_LARGE"
    };
    // Primary ingestion reads through the shared safe-reader kernel: the
    // platform adapter proves final-object/ancestor containment on the
    // opened handle and the kernel revalidates the same handle after the
    // read. No path-first byte product exists on this path.
    let absolute = absolutize_source(path)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| "DIRECT_SOURCE_PATH_DENIED".to_owned())?;
    let full = safe_reader_adapter::read_full_file_via_kernel(&absolute, parent, max_bytes)
        .map_err(|error| map_full_read_error(error, limit_error))?;
    if full.canonical_final.starts_with(data_root) {
        return Err("DIRECT_SOURCE_INSIDE_DATA_ROOT".to_owned());
    }
    if full.bytes.len() > max_bytes
        || u64::try_from(full.bytes.len()).unwrap_or(u64::MAX) != full.source_bytes
    {
        return Err("DIRECT_SOURCE_CHANGED_DURING_READ".to_owned());
    }
    let identity_strength = if full.identity_native {
        IdentityStrength::Native
    } else {
        IdentityStrength::PathBound
    };
    let path_digest = sha256::hex(&sha256::digest(&path_identity_bytes(&full.canonical_final)));
    let content_digest = sha256::hex(&sha256::digest(&full.bytes));
    Ok(FileSnapshot {
        path_digest,
        file_identity_digest: sha256::hex(&sha256::digest_parts(
            b"eliot-search/direct-file-identity/v1",
            &[&full.identity_material],
        )),
        identity_strength,
        content_digest,
        bytes: full.bytes,
    })
}

/// Resolves a caller-supplied source locator against the process directory.
///
/// Historical callers pass relative CLI paths; the kernel admits absolute
/// locators only, so relativize here instead of inside the adapter.
fn absolutize_source(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let current = std::env::current_dir().map_err(|_| "DIRECT_SOURCE_ACCESS_DENIED".to_owned())?;
    Ok(current.join(path))
}

/// Maps a kernel-verified read failure to the DIRECT namespace without
/// paths, bytes or raw OS error text.
fn map_full_read_error(error: FullReadError, limit_error: &str) -> String {
    match error {
        FullReadError::Adapter(adapter) => match adapter {
            AdapterError::PathDenied => "DIRECT_SOURCE_PATH_DENIED".to_owned(),
            AdapterError::LinkDenied | AdapterError::AncestorReparseDenied => {
                "DIRECT_SOURCE_LINK_DENIED".to_owned()
            }
            AdapterError::EscapeDenied => "DIRECT_SOURCE_ESCAPE_DENIED".to_owned(),
            AdapterError::RootRelocated => "DIRECT_SOURCE_ROOT_RELOCATED".to_owned(),
            AdapterError::NotRegular => "DIRECT_SOURCE_NOT_REGULAR".to_owned(),
            AdapterError::FinalObjectInvalid | AdapterError::DeviceDenied => {
                "DIRECT_SOURCE_FINAL_OBJECT_INVALID".to_owned()
            }
            AdapterError::HardlinkDenied => "DIRECT_SOURCE_HARDLINK_DENIED".to_owned(),
            AdapterError::AccessDenied => "DIRECT_SOURCE_ACCESS_DENIED".to_owned(),
            AdapterError::TooLarge => limit_error.to_owned(),
            AdapterError::ReceiptDenied => {
                "DIRECT_SOURCE_METADATA_ERROR:SAFE_ADAPTER_RECEIPT_DENIED".to_owned()
            }
        },
        FullReadError::Kernel(kernel) => match kernel {
            SafeReadError::RangeOutsideSource
            | SafeReadError::EofMismatch
            | SafeReadError::ReadLengthMismatch
            | SafeReadError::StableIdentityMismatch
            | SafeReadError::HandleChangedDuringRead
            | SafeReadError::BackendFailure => "DIRECT_SOURCE_CHANGED_DURING_READ".to_owned(),
            SafeReadError::RootIdentityMismatch => "DIRECT_SOURCE_ESCAPE_DENIED".to_owned(),
            SafeReadError::UnsupportedFileKind => {
                "DIRECT_SOURCE_FINAL_OBJECT_INVALID".to_owned()
            }
            SafeReadError::ReparseBoundaryDenied => "DIRECT_SOURCE_LINK_DENIED".to_owned(),
            SafeReadError::SecurityDenied | SafeReadError::SecurityRevisionMismatch => {
                "DIRECT_SOURCE_ACCESS_DENIED".to_owned()
            }
            SafeReadError::SourceSizeInvalid => limit_error.to_owned(),
            SafeReadError::Cancelled => "DIRECT_SOURCE_READ_CANCELLED".to_owned(),
            SafeReadError::InvalidLimits
            | SafeReadError::InvalidPathToken
            | SafeReadError::InvalidReadLength
            | SafeReadError::RangeOverflow
            | SafeReadError::InvalidRetryPolicy
            | SafeReadError::ReceiptMissing => "DIRECT_SOURCE_READ_INVALID".to_owned(),
        },
    }
}
