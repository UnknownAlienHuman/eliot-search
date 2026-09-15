//! Windows CurrentUser DPAPI and immutable filesystem boundary.

#![allow(unsafe_code)]

use core::ffi::c_void;
use core::ptr::null_mut;
use core::slice;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::envelope::Envelope;
use super::super::model::{
    DeleteReceipt, SealReceipt, SensitiveBytes, VerifyReceipt, wipe,
};
use super::super::spec::{
    FORMAT_VERSION, MAX_ENVELOPE_BYTES, MAX_PLAINTEXT_BYTES,
    SealedStoreError, entropy_for, validate_object_id,
};

const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const STORE_DIRECTORY: &str = "sealed-revisions";
const OBJECT_SUFFIX: &str = ".els-dpapi";

#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

#[link(name = "Crypt32")]
unsafe extern "system" {
    fn CryptProtectData(
        data_in: *const DataBlob,
        description: *const u16,
        optional_entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;

    fn CryptUnprotectData(
        data_in: *const DataBlob,
        description: *mut *mut u16,
        optional_entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
}

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}

struct LocalAllocation {
    pointer: *mut u8,
    length: usize,
    wipe_before_free: bool,
}

impl LocalAllocation {
    const unsafe fn from_blob(
        blob: &DataBlob,
        wipe_before_free: bool,
    ) -> Result<Self, SealedStoreError> {
        if blob.pb_data.is_null() || blob.cb_data == 0 {
            return Err(SealedStoreError::DpapiFailure);
        }
        Ok(Self {
            pointer: blob.pb_data,
            length: blob.cb_data as usize,
            wipe_before_free,
        })
    }

    fn copy(&self) -> Vec<u8> {
        // SAFETY: DPAPI returned a live allocation of exactly `length` bytes.
        unsafe { slice::from_raw_parts(self.pointer, self.length).to_vec() }
    }
}

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if self.pointer.is_null() {
            return;
        }
        if self.wipe_before_free {
            // SAFETY: the DPAPI allocation is live and uniquely owned here.
            let bytes =
                unsafe { slice::from_raw_parts_mut(self.pointer, self.length) };
            wipe(bytes);
        }
        // SAFETY: DPAPI returned LocalAlloc memory and this guard owns it.
        unsafe {
            let _ = LocalFree(self.pointer.cast());
        }
        self.pointer = null_mut();
        self.length = 0;
    }
}

pub(crate) fn seal_immutable(
    data_root: &Path,
    object_id: &str,
    plaintext: &SensitiveBytes,
) -> Result<SealReceipt, SealedStoreError> {
    validate_object_id(object_id)?;
    let directory = ensure_store_directory(data_root, true)?;
    let target = object_path(&directory, object_id);
    if target.exists() {
        return Err(SealedStoreError::ObjectAlreadyExists);
    }
    let ciphertext = protect_current_user(object_id, plaintext.expose())?;
    let envelope = Envelope {
        object_id: object_id.to_owned(),
        plaintext_bytes: u64::try_from(plaintext.len())
            .map_err(|_| SealedStoreError::PlaintextTooLarge)?,
        ciphertext,
    };
    let ciphertext_bytes = u64::try_from(envelope.ciphertext.len())
        .map_err(|_| SealedStoreError::EnvelopeTooLarge)?;
    let encoded = envelope.encode()?;
    write_immutable(&directory, &target, &encoded)?;
    let readback = read_envelope(&target)?;
    if readback != envelope {
        return Err(SealedStoreError::ReadbackMismatch);
    }
    Ok(SealReceipt {
        object_id: object_id.to_owned(),
        plaintext_bytes: envelope.plaintext_bytes,
        ciphertext_bytes,
        format_version: FORMAT_VERSION,
        protection_scope: "windows_current_user_dpapi",
        readback_verified: true,
    })
}

pub(crate) fn open_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<SensitiveBytes, SealedStoreError> {
    validate_object_id(object_id)?;
    let directory = ensure_store_directory(data_root, false)?;
    let envelope = read_envelope(&object_path(&directory, object_id))?;
    if envelope.object_id != object_id {
        return Err(SealedStoreError::ObjectBindingMismatch);
    }
    let plaintext = SensitiveBytes::new(unprotect_current_user(
        object_id,
        &envelope.ciphertext,
    )?)?;
    if u64::try_from(plaintext.len())
        .map_err(|_| SealedStoreError::PlaintextTooLarge)?
        != envelope.plaintext_bytes
    {
        return Err(SealedStoreError::ReadbackMismatch);
    }
    Ok(plaintext)
}

pub(crate) fn verify_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<VerifyReceipt, SealedStoreError> {
    validate_object_id(object_id)?;
    let directory = ensure_store_directory(data_root, false)?;
    let envelope = read_envelope(&object_path(&directory, object_id))?;
    if envelope.object_id != object_id {
        return Err(SealedStoreError::ObjectBindingMismatch);
    }
    let plaintext = SensitiveBytes::new(unprotect_current_user(
        object_id,
        &envelope.ciphertext,
    )?)?;
    let plaintext_bytes = u64::try_from(plaintext.len())
        .map_err(|_| SealedStoreError::PlaintextTooLarge)?;
    if plaintext_bytes != envelope.plaintext_bytes {
        return Err(SealedStoreError::ReadbackMismatch);
    }
    Ok(VerifyReceipt {
        object_id: object_id.to_owned(),
        plaintext_bytes,
        ciphertext_bytes: u64::try_from(envelope.ciphertext.len())
            .map_err(|_| SealedStoreError::EnvelopeTooLarge)?,
        format_version: FORMAT_VERSION,
        protection_scope: "windows_current_user_dpapi",
        authenticated: true,
    })
}

pub(crate) fn delete_sealed(
    data_root: &Path,
    object_id: &str,
) -> Result<DeleteReceipt, SealedStoreError> {
    validate_object_id(object_id)?;
    let directory = ensure_store_directory(data_root, false)?;
    let target = object_path(&directory, object_id);
    validate_regular_non_reparse(&target, false)?;
    fs::remove_file(&target).map_err(|error| map_not_found(&error))?;
    if target.exists() {
        return Err(SealedStoreError::ReadbackMismatch);
    }
    Ok(DeleteReceipt {
        object_id: object_id.to_owned(),
        logical_delete_complete: true,
        physical_erasure_guaranteed: false,
    })
}

fn ensure_store_directory(
    data_root: &Path,
    create: bool,
) -> Result<PathBuf, SealedStoreError> {
    validate_directory_non_reparse(data_root)?;
    let directory = data_root.join(STORE_DIRECTORY);
    if !directory.exists() {
        if !create {
            return Err(SealedStoreError::ObjectNotFound);
        }
        fs::create_dir(&directory).map_err(|_| SealedStoreError::IoFailure)?;
    }
    validate_directory_non_reparse(&directory)?;
    Ok(directory)
}

fn validate_directory_non_reparse(path: &Path) -> Result<(), SealedStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| SealedStoreError::InvalidDataRoot)?;
    if !metadata.is_dir() {
        return Err(SealedStoreError::InvalidDataRoot);
    }
    if metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(SealedStoreError::ReparsePointDenied);
    }
    Ok(())
}

fn validate_regular_non_reparse(
    path: &Path,
    allow_absent: bool,
) -> Result<(), SealedStoreError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if allow_absent && error.kind() == io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(SealedStoreError::ObjectNotFound);
        }
        Err(_) => return Err(SealedStoreError::IoFailure),
    };
    if !metadata.is_file() {
        return Err(SealedStoreError::IoFailure);
    }
    if metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(SealedStoreError::ReparsePointDenied);
    }
    Ok(())
}

fn object_path(directory: &Path, object_id: &str) -> PathBuf {
    directory.join(format!("{object_id}{OBJECT_SUFFIX}"))
}

fn read_envelope(path: &Path) -> Result<Envelope, SealedStoreError> {
    validate_regular_non_reparse(path, false)?;
    let mut file = File::open(path).map_err(|error| map_not_found(&error))?;
    let before = file.metadata().map_err(|_| SealedStoreError::IoFailure)?;
    let before_identity = eliot_searchd::native_file::observe(&file)
        .map_err(|_| SealedStoreError::IoFailure)?;
    if before.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(SealedStoreError::ReparsePointDenied);
    }
    if before.len() > u64::try_from(MAX_ENVELOPE_BYTES).unwrap_or(u64::MAX) {
        return Err(SealedStoreError::EnvelopeTooLarge);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(before.len())
            .map_err(|_| SealedStoreError::EnvelopeTooLarge)?,
    );
    (&mut file)
        .take(u64::try_from(MAX_ENVELOPE_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| SealedStoreError::IoFailure)?;
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(SealedStoreError::EnvelopeTooLarge);
    }
    let after = file.metadata().map_err(|_| SealedStoreError::IoFailure)?;
    let after_identity = eliot_searchd::native_file::observe(&file)
        .map_err(|_| SealedStoreError::IoFailure)?;
    if before.len() != after.len()
        || before.last_write_time() != after.last_write_time()
        || before.creation_time() != after.creation_time()
        || before_identity != after_identity
    {
        return Err(SealedStoreError::ObjectChangedDuringRead);
    }
    Envelope::decode(&bytes)
}

fn write_immutable(
    directory: &Path,
    target: &Path,
    bytes: &[u8],
) -> Result<(), SealedStoreError> {
    validate_regular_non_reparse(target, true)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SealedStoreError::IoFailure)?
        .as_nanos();
    for attempt in 0_u8..32 {
        let temporary = directory.join(format!(
            ".sealed-{}-{timestamp}-{attempt}.tmp",
            std::process::id()
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                continue;
            }
            Err(_) => return Err(SealedStoreError::IoFailure),
        };
        let write_result = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(SealedStoreError::IoFailure);
        }
        match fs::hard_link(&temporary, target) {
            Ok(()) => {
                let _ = fs::remove_file(&temporary);
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temporary);
                return Err(SealedStoreError::ObjectAlreadyExists);
            }
            Err(_) => {
                let _ = fs::remove_file(&temporary);
                return Err(SealedStoreError::IoFailure);
            }
        }
    }
    Err(SealedStoreError::IoFailure)
}

fn protect_current_user(
    object_id: &str,
    plaintext: &[u8],
) -> Result<Vec<u8>, SealedStoreError> {
    if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT_BYTES {
        return Err(if plaintext.is_empty() {
            SealedStoreError::EmptyPlaintext
        } else {
            SealedStoreError::PlaintextTooLarge
        });
    }
    let entropy = entropy_for(object_id);
    let mut input = blob(plaintext)?;
    let mut entropy_blob = blob(&entropy)?;
    let description = format!("ELIOT Search sealed object {object_id}\0")
        .encode_utf16()
        .collect::<Vec<_>>();
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: null_mut(),
    };
    // SAFETY: inputs are bounded live slices; output ownership transfers below.
    let succeeded = unsafe {
        CryptProtectData(
            (&raw mut input).cast_const(),
            description.as_ptr(),
            (&raw mut entropy_blob).cast_const(),
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    };
    if succeeded == 0 {
        return Err(SealedStoreError::DpapiFailure);
    }
    // SAFETY: successful DPAPI output is a LocalAlloc allocation.
    let allocation = unsafe { LocalAllocation::from_blob(&output, false)? };
    if allocation.length > MAX_ENVELOPE_BYTES {
        return Err(SealedStoreError::EnvelopeTooLarge);
    }
    Ok(allocation.copy())
}

fn unprotect_current_user(
    object_id: &str,
    ciphertext: &[u8],
) -> Result<Vec<u8>, SealedStoreError> {
    if ciphertext.is_empty() || ciphertext.len() > MAX_ENVELOPE_BYTES {
        return Err(SealedStoreError::EnvelopeInvalid);
    }
    let entropy = entropy_for(object_id);
    let mut input = blob(ciphertext)?;
    let mut entropy_blob = blob(&entropy)?;
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: null_mut(),
    };
    let mut description: *mut u16 = null_mut();
    // SAFETY: inputs are live; DPAPI output allocations are released below.
    let succeeded = unsafe {
        CryptUnprotectData(
            (&raw mut input).cast_const(),
            &raw mut description,
            (&raw mut entropy_blob).cast_const(),
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    };
    if !description.is_null() {
        // SAFETY: DPAPI documents this output as LocalAlloc memory.
        unsafe {
            let _ = LocalFree(description.cast());
        }
    }
    if succeeded == 0 {
        return Err(SealedStoreError::DpapiFailure);
    }
    // SAFETY: plaintext output is wiped before LocalFree.
    let allocation = unsafe { LocalAllocation::from_blob(&output, true)? };
    if allocation.length > MAX_PLAINTEXT_BYTES {
        return Err(SealedStoreError::PlaintextTooLarge);
    }
    Ok(allocation.copy())
}

fn blob(bytes: &[u8]) -> Result<DataBlob, SealedStoreError> {
    Ok(DataBlob {
        cb_data: u32::try_from(bytes.len())
            .map_err(|_| SealedStoreError::PlaintextTooLarge)?,
        pb_data: if bytes.is_empty() {
            null_mut()
        } else {
            bytes.as_ptr().cast_mut()
        },
    })
}

fn map_not_found(error: &io::Error) -> SealedStoreError {
    if error.kind() == io::ErrorKind::NotFound {
        SealedStoreError::ObjectNotFound
    } else {
        SealedStoreError::IoFailure
    }
}
