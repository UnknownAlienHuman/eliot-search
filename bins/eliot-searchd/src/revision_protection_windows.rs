#![allow(unsafe_code)]

use core::ffi::c_void;
use core::ptr::{self, null, null_mut};
use std::fs;
use std::path::Path;
use std::slice;

use crate::sha256;

const CRED_TYPE_GENERIC: u32 = 1;
const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
const ERROR_NOT_FOUND: u32 = 1_168;
const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x0000_0001;
const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
const ROOT_SECRET_BYTES: usize = 32;
const MAX_CREDENTIAL_BLOB_BYTES: usize = 5 * 512;
const MAX_OBJECT_SCAN: usize = 2_000_000;

#[allow(dead_code)]
#[repr(C)]
struct FileTime {
    low_date_time: u32,
    high_date_time: u32,
}

#[allow(dead_code)]
#[repr(C)]
struct CredentialW {
    flags: u32,
    credential_type: u32,
    target_name: *mut u16,
    comment: *mut u16,
    last_written: FileTime,
    credential_blob_size: u32,
    credential_blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut c_void,
    target_alias: *mut u16,
    user_name: *mut u16,
}

#[repr(C)]
struct DataBlob {
    byte_length: u32,
    bytes: *mut u8,
}

#[link(name = "Advapi32")]
unsafe extern "system" {
    #[link_name = "CredReadW"]
    fn cred_read_w(
        target_name: *const u16,
        credential_type: u32,
        flags: u32,
        credential: *mut *mut CredentialW,
    ) -> i32;
    #[link_name = "CredWriteW"]
    fn cred_write_w(credential: *const CredentialW, flags: u32) -> i32;
    #[link_name = "CredFree"]
    fn cred_free(buffer: *mut c_void);
}

#[link(name = "Crypt32")]
unsafe extern "system" {
    #[link_name = "CryptProtectData"]
    fn crypt_protect_data(
        data_in: *const DataBlob,
        description: *const u16,
        optional_entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
    #[link_name = "CryptUnprotectData"]
    fn crypt_unprotect_data(
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
    #[link_name = "GetLastError"]
    fn get_last_error() -> u32;
    #[link_name = "LocalFree"]
    fn local_free(memory: *mut c_void) -> *mut c_void;
}

#[link(name = "Bcrypt")]
unsafe extern "system" {
    #[link_name = "BCryptGenRandom"]
    fn bcrypt_gen_random(
        algorithm: *mut c_void,
        buffer: *mut u8,
        buffer_bytes: u32,
        flags: u32,
    ) -> i32;
}

struct CredentialAllocation(*mut CredentialW);

impl Drop for CredentialAllocation {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            let credential = &mut *self.0;
            let size = usize::try_from(credential.credential_blob_size)
                .unwrap_or(0)
                .min(MAX_CREDENTIAL_BLOB_BYTES);
            if !credential.credential_blob.is_null() && size > 0 {
                ptr::write_bytes(credential.credential_blob, 0, size);
            }
            cred_free(self.0.cast());
        }
    }
}

struct LocalAllocation(DataBlob);

impl LocalAllocation {
    fn into_vec(mut self, max_bytes: usize) -> Result<Vec<u8>, String> {
        let length = usize::try_from(self.0.byte_length)
            .map_err(|_| "DIRECT_DPAPI_OUTPUT_TOO_LARGE".to_owned())?;
        if self.0.bytes.is_null() || length == 0 || length > max_bytes {
            return Err("DIRECT_DPAPI_OUTPUT_INVALID".to_owned());
        }
        let output = unsafe { slice::from_raw_parts(self.0.bytes, length) }.to_vec();
        unsafe {
            ptr::write_bytes(self.0.bytes, 0, length);
            let _ = local_free(self.0.bytes.cast());
        }
        self.0.bytes = null_mut();
        self.0.byte_length = 0;
        Ok(output)
    }
}

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if self.0.bytes.is_null() {
            return;
        }
        let length = usize::try_from(self.0.byte_length).unwrap_or(0);
        unsafe {
            if length > 0 {
                ptr::write_bytes(self.0.bytes, 0, length);
            }
            let _ = local_free(self.0.bytes.cast());
        }
    }
}

pub(super) fn load_or_create_root_secret(
    namespace_id: [u8; 32],
    revision_root: &Path,
) -> Result<[u8; 32], String> {
    let target_text = format!(
        "ELIOT Search/revision-key/{}",
        sha256::hex(&namespace_id),
    );
    let target = wide(&target_text);
    if let Some(secret) = read_credential(&target)? {
        return Ok(secret);
    }
    if contains_protected_objects(revision_root)? {
        return Err("DIRECT_REVISION_KEY_MISSING".to_owned());
    }
    let mut secret = [0_u8; ROOT_SECRET_BYTES];
    let status = unsafe {
        bcrypt_gen_random(
            null_mut(),
            secret.as_mut_ptr(),
            u32::try_from(secret.len())
                .map_err(|_| "DIRECT_REVISION_RNG_FAILED".to_owned())?,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        super::zeroize(&mut secret);
        return Err(format!("DIRECT_REVISION_RNG_FAILED:{status}"));
    }
    if let Err(error) = write_credential(&target, &mut secret) {
        super::zeroize(&mut secret);
        return Err(error);
    }
    let observed = match read_credential(&target) {
        Ok(Some(observed)) => observed,
        Ok(None) => {
            super::zeroize(&mut secret);
            return Err("DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN".to_owned());
        }
        Err(error) => {
            super::zeroize(&mut secret);
            return Err(error);
        }
    };
    if !constant_time_equal(&secret, &observed) {
        super::zeroize(&mut secret);
        let mut observed = observed;
        super::zeroize(&mut observed);
        return Err("DIRECT_REVISION_KEY_READBACK_MISMATCH".to_owned());
    }
    let mut observed = observed;
    super::zeroize(&mut observed);
    Ok(secret)
}

fn read_credential(target: &[u16]) -> Result<Option<[u8; 32]>, String> {
    let mut pointer = null_mut::<CredentialW>();
    let success = unsafe {
        cred_read_w(
            target.as_ptr(),
            CRED_TYPE_GENERIC,
            0,
            &mut pointer,
        )
    };
    if success == 0 {
        let error = unsafe { get_last_error() };
        return if error == ERROR_NOT_FOUND {
            Ok(None)
        } else {
            Err(format!("DIRECT_REVISION_KEY_READ_FAILED:{error}"))
        };
    }
    if pointer.is_null() {
        return Err("DIRECT_REVISION_KEY_READBACK_INVALID".to_owned());
    }
    let allocation = CredentialAllocation(pointer);
    let credential = unsafe { &*allocation.0 };
    let size = usize::try_from(credential.credential_blob_size)
        .map_err(|_| "DIRECT_REVISION_KEY_READBACK_INVALID".to_owned())?;
    if credential.credential_type != CRED_TYPE_GENERIC
        || credential.persist != CRED_PERSIST_LOCAL_MACHINE
        || size != ROOT_SECRET_BYTES
        || credential.credential_blob.is_null()
    {
        return Err("DIRECT_REVISION_KEY_READBACK_INVALID".to_owned());
    }
    let mut secret = [0_u8; ROOT_SECRET_BYTES];
    unsafe {
        ptr::copy_nonoverlapping(
            credential.credential_blob,
            secret.as_mut_ptr(),
            ROOT_SECRET_BYTES,
        );
    }
    drop(allocation);
    Ok(Some(secret))
}

fn write_credential(
    target: &[u16],
    secret: &mut [u8; 32],
) -> Result<(), String> {
    let mut target = target.to_vec();
    let mut user_name = wide("ELIOT Search");
    let credential = CredentialW {
        flags: 0,
        credential_type: CRED_TYPE_GENERIC,
        target_name: target.as_mut_ptr(),
        comment: null_mut(),
        last_written: FileTime {
            low_date_time: 0,
            high_date_time: 0,
        },
        credential_blob_size: u32::try_from(secret.len())
            .map_err(|_| "DIRECT_REVISION_KEY_TOO_LARGE".to_owned())?,
        credential_blob: secret.as_mut_ptr(),
        persist: CRED_PERSIST_LOCAL_MACHINE,
        attribute_count: 0,
        attributes: null_mut(),
        target_alias: null_mut(),
        user_name: user_name.as_mut_ptr(),
    };
    let success = unsafe { cred_write_w(&credential, 0) };
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_REVISION_KEY_WRITE_FAILED:{error}"));
    }
    Ok(())
}

pub(super) fn protect_data(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let input_len = u32::try_from(input.len())
        .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?;
    let mut entropy_copy = *entropy;
    let input_blob = DataBlob {
        byte_length: input_len,
        bytes: input.as_mut_ptr(),
    };
    let entropy_blob = DataBlob {
        byte_length: u32::try_from(entropy_copy.len())
            .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?,
        bytes: entropy_copy.as_mut_ptr(),
    };
    let mut output = DataBlob {
        byte_length: 0,
        bytes: null_mut(),
    };
    let success = unsafe {
        crypt_protect_data(
            &input_blob,
            null(),
            &entropy_blob,
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    super::zeroize(&mut entropy_copy);
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_DPAPI_PROTECT_FAILED:{error}"));
    }
    LocalAllocation(output).into_vec(super::MAX_PROTECTED_OBJECT_BYTES)
}

pub(super) fn unprotect_data(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let input_len = u32::try_from(input.len())
        .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?;
    let mut entropy_copy = *entropy;
    let input_blob = DataBlob {
        byte_length: input_len,
        bytes: input.as_mut_ptr(),
    };
    let entropy_blob = DataBlob {
        byte_length: u32::try_from(entropy_copy.len())
            .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?,
        bytes: entropy_copy.as_mut_ptr(),
    };
    let mut output = DataBlob {
        byte_length: 0,
        bytes: null_mut(),
    };
    let success = unsafe {
        crypt_unprotect_data(
            &input_blob,
            null_mut(),
            &entropy_blob,
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    super::zeroize(&mut entropy_copy);
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_DPAPI_UNPROTECT_FAILED:{error}"));
    }
    LocalAllocation(output).into_vec(super::MAX_PROTECTED_OBJECT_BYTES)
}

fn contains_protected_objects(root: &Path) -> Result<bool, String> {
    if !root.exists() {
        return Ok(false);
    }
    let mut observed = 0_usize;
    for shard in fs::read_dir(root)
        .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?
    {
        let shard = shard
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
        let metadata = fs::symlink_metadata(shard.path())
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DIRECT_REVISION_DIRECTORY_LINK_DENIED".to_owned());
        }
        if !metadata.is_dir() {
            continue;
        }
        for entry in fs::read_dir(shard.path())
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?
        {
            let entry = entry
                .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
            observed = observed.saturating_add(1);
            if observed > MAX_OBJECT_SCAN {
                return Err("DIRECT_REVISION_OBJECT_LIMIT_EXCEEDED".to_owned());
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
            if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                return Err("DIRECT_REVISION_OBJECT_LINK_DENIED".to_owned());
            }
            if path
                .extension()
                .is_some_and(|value| value == std::ffi::OsStr::new(super::PROTECTED_OBJECT_EXTENSION))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right.iter()) {
        difference |= left ^ right;
    }
    difference == 0
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(core::iter::once(0)).collect()
}
