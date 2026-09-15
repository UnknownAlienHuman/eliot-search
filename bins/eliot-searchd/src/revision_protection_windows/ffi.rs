//! Raw Windows APIs and zeroizing native allocation owners.

use core::ffi::c_void;
use core::ptr::{self, null_mut};
use std::slice;

pub(super) const CRED_TYPE_GENERIC: u32 = 1;
pub(super) const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
pub(super) const ERROR_NOT_FOUND: u32 = 1_168;
pub(super) const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x0000_0001;
pub(super) const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
pub(super) const ROOT_SECRET_BYTES: usize = 32;
pub(super) const MAX_CREDENTIAL_BLOB_BYTES: usize = 5 * 512;

const VAULT_MUTEX_NAME: &str = "ELIOT-Search-RevisionVault-v1";
const VAULT_LOCK_WAIT_MILLIS: u32 = 5_000;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_ABANDONED: u32 = 0x0000_0080;

#[allow(dead_code)]
#[repr(C)]
pub(super) struct FileTime {
    pub(super) low_date_time: u32,
    pub(super) high_date_time: u32,
}

#[allow(dead_code)]
#[repr(C)]
pub(super) struct CredentialW {
    pub(super) flags: u32,
    pub(super) credential_type: u32,
    pub(super) target_name: *mut u16,
    pub(super) comment: *mut u16,
    pub(super) last_written: FileTime,
    pub(super) credential_blob_size: u32,
    pub(super) credential_blob: *mut u8,
    pub(super) persist: u32,
    pub(super) attribute_count: u32,
    pub(super) attributes: *mut c_void,
    pub(super) target_alias: *mut u16,
    pub(super) user_name: *mut u16,
}

#[repr(C)]
pub(super) struct DataBlob {
    pub(super) byte_length: u32,
    pub(super) bytes: *mut u8,
}

#[link(name = "Advapi32")]
unsafe extern "system" {
    #[link_name = "CredReadW"]
    pub(super) fn cred_read_w(
        target_name: *const u16,
        credential_type: u32,
        flags: u32,
        credential: *mut *mut CredentialW,
    ) -> i32;
    #[link_name = "CredWriteW"]
    pub(super) fn cred_write_w(
        credential: *const CredentialW,
        flags: u32,
    ) -> i32;
    #[cfg(test)]
    #[link_name = "CredDeleteW"]
    pub(super) fn cred_delete_w(
        target_name: *const u16,
        credential_type: u32,
        flags: u32,
    ) -> i32;
    #[link_name = "CredFree"]
    fn cred_free(buffer: *mut c_void);
}

#[link(name = "Crypt32")]
unsafe extern "system" {
    #[link_name = "CryptProtectData"]
    pub(super) fn crypt_protect_data(
        data_in: *const DataBlob,
        description: *const u16,
        optional_entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
    #[link_name = "CryptUnprotectData"]
    pub(super) fn crypt_unprotect_data(
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
    pub(super) fn get_last_error() -> u32;
    #[link_name = "LocalFree"]
    fn local_free(memory: *mut c_void) -> *mut c_void;
    #[link_name = "CreateMutexW"]
    fn create_mutex_w(
        security_attributes: *mut c_void,
        initial_owner: i32,
        name: *const u16,
    ) -> *mut c_void;
    #[link_name = "WaitForSingleObject"]
    fn wait_for_single_object(handle: *mut c_void, milliseconds: u32) -> u32;
    #[link_name = "ReleaseMutex"]
    fn release_mutex(handle: *mut c_void) -> i32;
    #[link_name = "CloseHandle"]
    fn close_handle(handle: *mut c_void) -> i32;
}

#[link(name = "Bcrypt")]
unsafe extern "system" {
    #[link_name = "BCryptGenRandom"]
    pub(super) fn bcrypt_gen_random(
        algorithm: *mut c_void,
        buffer: *mut u8,
        buffer_bytes: u32,
        flags: u32,
    ) -> i32;
}

pub(super) struct CredentialAllocation(pub(super) *mut CredentialW);

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

pub(super) struct LocalAllocation(pub(super) DataBlob);

impl LocalAllocation {
    pub(super) fn into_vec(
        mut self,
        max_bytes: usize,
    ) -> Result<Vec<u8>, String> {
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

/// Held cross-process vault mutex. Released and closed on drop.
pub(super) struct VaultLock(*mut c_void);

impl Drop for VaultLock {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            release_mutex(self.0);
            close_handle(self.0);
        }
    }
}

pub(super) fn acquire_vault_lock() -> Result<VaultLock, String> {
    let name = wide(VAULT_MUTEX_NAME);
    let handle = unsafe { create_mutex_w(null_mut(), 0, name.as_ptr()) };
    if handle.is_null() {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_REVISION_VAULT_LOCK_FAILED:{error}"));
    }
    let status = unsafe { wait_for_single_object(handle, VAULT_LOCK_WAIT_MILLIS) };
    if status == WAIT_OBJECT_0 || status == WAIT_ABANDONED {
        return Ok(VaultLock(handle));
    }
    unsafe {
        close_handle(handle);
    }
    Err(format!("DIRECT_REVISION_VAULT_BUSY:{status}"))
}

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(core::iter::once(0)).collect()
}
