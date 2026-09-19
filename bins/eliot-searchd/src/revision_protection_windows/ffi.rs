//! Test-only Windows credential deletion and vault serialization.

use core::ffi::c_void;
use core::ptr::null_mut;

pub(super) const CRED_TYPE_GENERIC: u32 = 1;

const VAULT_MUTEX_NAME: &str = "ELIOT-Search-RevisionVault-v1";
const VAULT_LOCK_WAIT_MILLIS: u32 = 5_000;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_ABANDONED: u32 = 0x0000_0080;

#[link(name = "Advapi32")]
unsafe extern "system" {
    #[link_name = "CredDeleteW"]
    pub(super) fn cred_delete_w(
        target_name: *const u16,
        credential_type: u32,
        flags: u32,
    ) -> i32;
}

#[link(name = "Kernel32")]
unsafe extern "system" {
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

pub(super) struct VaultLock(*mut c_void);

impl Drop for VaultLock {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: the handle is a live mutex handle owned by this test guard.
        unsafe {
            release_mutex(self.0);
            close_handle(self.0);
        }
        self.0 = null_mut();
    }
}

pub(super) fn acquire_vault_lock() -> Result<VaultLock, String> {
    let name = wide(VAULT_MUTEX_NAME);
    // SAFETY: null security attributes and a terminated name are valid inputs.
    let handle = unsafe { create_mutex_w(null_mut(), 0, name.as_ptr()) };
    if handle.is_null() {
        return Err("TEST_REVISION_VAULT_LOCK_FAILED".to_owned());
    }
    // SAFETY: `handle` is a live mutex handle.
    let status = unsafe { wait_for_single_object(handle, VAULT_LOCK_WAIT_MILLIS) };
    if status == WAIT_OBJECT_0 || status == WAIT_ABANDONED {
        return Ok(VaultLock(handle));
    }
    // SAFETY: unsuccessful acquisition leaves one owned handle to close.
    unsafe {
        close_handle(handle);
    }
    Err(format!("TEST_REVISION_VAULT_BUSY:{status}"))
}

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(core::iter::once(0)).collect()
}
