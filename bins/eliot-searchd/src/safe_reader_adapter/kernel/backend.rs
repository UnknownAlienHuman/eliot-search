//! Final-handle backend and shared-kernel backend contract.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};
use search_safe_reader::{
    AdapterRead, FinalHandleMetadata, FinalHandleOpenRequest, ReadSecurityDisposition,
    SafeReadBackend, SafeReadError,
};

use super::identity::{
    attribute_bits, file_identity_digest, handle_identity_material,
    modified_nanos, root_identity_digest, verify_handle_rebinding,
};
use super::path::{
    canonicalize_admitted_root, is_reparse, validate_relative_token,
    verify_ancestor_containment,
};
use super::spec::AdapterError;

/// Final opened handle: an owned file plus its proven identities.
///
/// `Debug` is redacted to digests and lengths; it never prints paths or raw
/// identity material.
pub struct FinalHandle {
    file: File,
    root_digest: Blake3Digest32,
    file_digest: Blake3Digest32,
    pub(super) identity_material: Vec<u8>,
    pub(super) identity_native: bool,
    pub(super) canonical_final: PathBuf,
    pub(super) source_bytes: u64,
}

impl core::fmt::Debug for FinalHandle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FinalHandle")
            .field("root_digest", &self.root_digest)
            .field("file_digest", &self.file_digest)
            .field("source_bytes", &self.source_bytes)
            .finish_non_exhaustive()
    }
}

/// Platform backend serving the shared kernel from one final handle.
pub struct FinalHandleBackend {
    canonical_root: PathBuf,
    root_digest: Blake3Digest32,
    token_text: String,
    barrier: NonZeroRevision,
    max_single_read_bytes: usize,
}

impl core::fmt::Debug for FinalHandleBackend {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FinalHandleBackend")
            .field("root_digest", &self.root_digest)
            .field("token_bytes", &self.token_text.len())
            .field("barrier", &self.barrier.get())
            .finish_non_exhaustive()
    }
}

impl FinalHandleBackend {
    /// Binds one admitted root and one validated relative token.
    ///
    /// No file is opened here; [`SafeReadBackend::open_final`] opens exactly
    /// once per attempt under this binding.
    pub fn bind(
        admitted_root: &Path,
        relative_token: &str,
        barrier: NonZeroRevision,
        max_single_read_bytes: usize,
    ) -> Result<Self, AdapterError> {
        if max_single_read_bytes == 0 {
            return Err(AdapterError::PathDenied);
        }
        validate_relative_token(relative_token, 32_768)?;
        let canonical_root = canonicalize_admitted_root(admitted_root)?;
        let root_digest = root_identity_digest(&canonical_root)?;
        Ok(Self {
            canonical_root,
            root_digest,
            token_text: relative_token.to_owned(),
            barrier,
            max_single_read_bytes,
        })
    }

    /// Stable logical-root digest proven at bind time.
    #[must_use]
    pub const fn root_digest(&self) -> Blake3Digest32 {
        self.root_digest
    }

    fn open_handle(&self) -> Result<FinalHandle, AdapterError> {
        let mut joined = self.canonical_root.clone();
        for component in self.token_text.split('/') {
            joined.push(component);
        }
        let precheck = fs::symlink_metadata(&joined).map_err(|_| AdapterError::AccessDenied)?;
        if precheck.file_type().is_symlink() || is_reparse(&precheck) {
            return Err(AdapterError::LinkDenied);
        }
        if precheck.is_dir() {
            return Err(AdapterError::NotRegular);
        }
        if !precheck.is_file() {
            return Err(AdapterError::FinalObjectInvalid);
        }
        deny_special_precheck(&precheck)?;

        let file = File::open(&joined).map_err(|_| AdapterError::AccessDenied)?;
        let handle_metadata = file.metadata().map_err(|_| AdapterError::AccessDenied)?;
        if !handle_metadata.is_file() {
            return Err(AdapterError::FinalObjectInvalid);
        }
        if is_reparse(&handle_metadata) {
            return Err(AdapterError::LinkDenied);
        }
        deny_special_handle(&file, &handle_metadata)?;

        let canonical_final = fs::canonicalize(&joined).map_err(|_| AdapterError::AccessDenied)?;
        verify_ancestor_containment(&canonical_final, &self.canonical_root)?;

        let (identity_material, is_hardlink, identity_native) =
            handle_identity_material(&file, &canonical_final)?;
        if is_hardlink {
            return Err(AdapterError::HardlinkDenied);
        }
        verify_handle_rebinding(&file, &canonical_final, &identity_material)?;

        let source_bytes = handle_metadata.len();
        let file_digest = file_identity_digest(&identity_material);
        let _ = change_text(
            &file_digest,
            source_bytes,
            modified_nanos(&handle_metadata),
            attribute_bits(&handle_metadata),
        )?;
        Ok(FinalHandle {
            file,
            root_digest: self.root_digest,
            file_digest,
            identity_material,
            identity_native,
            canonical_final,
            source_bytes,
        })
    }
}

#[cfg(unix)]
fn deny_special_precheck(metadata: &std::fs::Metadata) -> Result<(), AdapterError> {
    use std::os::unix::fs::FileTypeExt;
    let file_type = metadata.file_type();
    if file_type.is_fifo()
        || file_type.is_socket()
        || file_type.is_block_device()
        || file_type.is_char_device()
    {
        return Err(AdapterError::DeviceDenied);
    }
    Ok(())
}

#[cfg(not(unix))]
fn deny_special_precheck(metadata: &std::fs::Metadata) -> Result<(), AdapterError> {
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(AdapterError::DeviceDenied);
    }
    Ok(())
}

#[cfg(unix)]
fn deny_special_handle(_file: &File, metadata: &std::fs::Metadata) -> Result<(), AdapterError> {
    deny_special_precheck(metadata)
}

#[cfg(not(unix))]
fn deny_special_handle(file: &File, metadata: &std::fs::Metadata) -> Result<(), AdapterError> {
    deny_special_precheck(metadata)?;
    #[cfg(windows)]
    {
        match eliot_searchd::native_file::observe(file) {
            Ok(_) => Ok(()),
            Err(error) => {
                if error.code() == "NATIVE_FILE_REPARSE_POINT_DENIED" {
                    return Err(AdapterError::LinkDenied);
                }
                Err(AdapterError::FinalObjectInvalid)
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Ok(())
    }
}

fn change_text(
    file_digest: &Blake3Digest32,
    source_bytes: u64,
    modified_nanos: u128,
    attributes: u64,
) -> Result<String, AdapterError> {
    let text = format!("v1:{file_digest}:{source_bytes}:{modified_nanos}:{attributes}");
    if text.len() > 256 {
        return Err(AdapterError::ReceiptDenied);
    }
    Ok(text)
}

fn metadata_receipt(source_bytes: u64, change_text: &str) -> Result<ReceiptRef, AdapterError> {
    ReceiptRef::new(format!("safe-adapter-meta-v1:{source_bytes}:{change_text}"))
        .map_err(|_| AdapterError::ReceiptDenied)
}

fn read_receipt(offset: u64, length: usize) -> Result<ReceiptRef, AdapterError> {
    ReceiptRef::new(format!("safe-adapter-read-v1:{offset}:{length}"))
        .map_err(|_| AdapterError::ReceiptDenied)
}

fn change_token(change_text: &str) -> Result<OpaqueId, AdapterError> {
    OpaqueId::new(format!("safe-adapter-change-v1:{change_text}"))
        .map_err(|_| AdapterError::ReceiptDenied)
}

impl SafeReadBackend for FinalHandleBackend {
    type Handle = FinalHandle;
    type BackendError = AdapterError;

    fn open_final(
        &mut self,
        request: &FinalHandleOpenRequest,
    ) -> Result<Self::Handle, Self::BackendError> {
        if request.relative_path.as_str() != self.token_text {
            return Err(AdapterError::PathDenied);
        }
        let live_root = canonicalize_admitted_root(&self.canonical_root)
            .map_err(|_| AdapterError::RootRelocated)?;
        if live_root != self.canonical_root {
            return Err(AdapterError::RootRelocated);
        }
        self.open_handle()
    }

    fn inspect(
        &mut self,
        handle: &Self::Handle,
    ) -> Result<FinalHandleMetadata, Self::BackendError> {
        let metadata = handle
            .file
            .metadata()
            .map_err(|_| AdapterError::AccessDenied)?;
        let kind = if metadata.is_file() {
            search_safe_reader::FinalHandleKind::RegularFile
        } else if metadata.is_dir() {
            search_safe_reader::FinalHandleKind::Directory
        } else {
            search_safe_reader::FinalHandleKind::Other
        };
        let final_is_reparse = is_reparse(&metadata);
        let (live_material, live_hardlink, _) =
            handle_identity_material(&handle.file, &handle.canonical_final)
                .map_err(|_| AdapterError::AccessDenied)?;
        if live_hardlink {
            return Err(AdapterError::HardlinkDenied);
        }
        let live_digest = file_identity_digest(&live_material);
        let source_bytes = metadata.len();
        let change = change_text(
            &live_digest,
            source_bytes,
            modified_nanos(&metadata),
            attribute_bits(&metadata),
        )?;
        Ok(FinalHandleMetadata {
            root_identity_digest: handle.root_digest,
            stable_file_identity_digest: live_digest,
            kind,
            source_bytes,
            final_object_is_reparse: final_is_reparse,
            ancestor_reparse_observed: false,
            security_disposition: ReadSecurityDisposition::Permitted,
            security_barrier_revision: self.barrier,
            change_token: change_token(&change)?,
            metadata_receipt: Some(metadata_receipt(source_bytes, &change)?),
        })
    }

    fn read_exact_at(
        &mut self,
        handle: &Self::Handle,
        offset: u64,
        length: usize,
    ) -> Result<AdapterRead, Self::BackendError> {
        if length == 0 || length > self.max_single_read_bytes {
            return Err(AdapterError::AccessDenied);
        }
        let mut file = handle
            .file
            .try_clone()
            .map_err(|_| AdapterError::AccessDenied)?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| AdapterError::AccessDenied)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| AdapterError::TooLarge)?;
        file.take(length as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| AdapterError::AccessDenied)?;
        Ok(AdapterRead {
            bytes,
            read_receipt: Some(read_receipt(offset, length)?),
        })
    }

    fn map_backend_error(error: &Self::BackendError) -> SafeReadError {
        match error {
            AdapterError::TooLarge => SafeReadError::SourceSizeInvalid,
            _ => SafeReadError::BackendFailure,
        }
    }
}
