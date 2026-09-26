//! Native Credential Manager, CSPRNG and cross-process vault effects.

use core::ffi::c_void;
use core::ptr::{null_mut, write_volatile};
use core::time::Duration;

use super::{
    LEGACY_REVISION_ROOT_SECRET_BYTES,
    LegacyRevisionRootSecret, LegacyRevisionRootSecretError,
    RootSecretPlatform,
};
use crate::model::clear_bytes;

const CRED_TYPE_GENERIC: u32 = 1;
const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
const ERROR_NOT_FOUND: u32 = 1_168;
const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
const MAX_CREDENTIAL_BLOB_BYTES: usize = 5 * 512;
const VAULT_MUTEX_NAME: &str = "ELIOT-Search-RevisionVault-v1";
const VAULT_LOCK_WAIT_MILLIS: u32 = 5_000;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_ABANDONED: u32 = 0x0000_0080;
const WAIT_TIMEOUT: u32 = 0x0000_0102;

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

#[link(name = "Kernel32")]
unsafe extern "system" {
    #[link_name = "GetLastError"]
    fn get_last_error() -> u32;
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
    fn bcrypt_gen_random(
        algorithm: *mut c_void,
        buffer: *mut u8,
        buffer_bytes: u32,
        flags: u32,
    ) -> i32;
}

pub(super) struct WindowsCredentialPlatform;

impl RootSecretPlatform for WindowsCredentialPlatform {
    type VaultGuard = WindowsVaultLock;

    fn read_credential(
        &mut self,
        target: &[u16],
    ) -> Result<Option<LegacyRevisionRootSecret>, LegacyRevisionRootSecretError> {
        let mut pointer = null_mut::<CredentialW>();
        // SAFETY: `target` is NUL-terminated and `pointer` is initialized for
        // one CredReadW output allocation.
        let success = unsafe {
            cred_read_w(
                target.as_ptr(),
                CRED_TYPE_GENERIC,
                0,
                &raw mut pointer,
            )
        };
        if success == 0 {
            // SAFETY: GetLastError has no preconditions and is captured
            // immediately after the failed credential call.
            let error = unsafe { get_last_error() };
            return if error == ERROR_NOT_FOUND {
                Ok(None)
            } else {
                Err(LegacyRevisionRootSecretError::CredentialReadFailed(error))
            };
        }
        if pointer.is_null() {
            return Err(
                LegacyRevisionRootSecretError::CredentialReadbackInvalid,
            );
        }

        let allocation = CredentialAllocation(pointer);
        // SAFETY: successful CredReadW returned one live CredentialW owned by
        // `allocation` until this scope ends.
        let credential = unsafe { &*allocation.0 };
        let size = usize::try_from(credential.credential_blob_size).map_err(
            |_| LegacyRevisionRootSecretError::CredentialReadbackInvalid,
        )?;
        if credential.credential_type != CRED_TYPE_GENERIC
            || credential.persist != CRED_PERSIST_LOCAL_MACHINE
            || size != LEGACY_REVISION_ROOT_SECRET_BYTES
            || credential.credential_blob.is_null()
        {
            return Err(
                LegacyRevisionRootSecretError::CredentialReadbackInvalid,
            );
        }
        let mut secret = [0_u8; LEGACY_REVISION_ROOT_SECRET_BYTES];
        // SAFETY: the validated credential blob is live for exactly 32 bytes.
        unsafe {
            core::ptr::copy_nonoverlapping(
                credential.credential_blob,
                secret.as_mut_ptr(),
                LEGACY_REVISION_ROOT_SECRET_BYTES,
            );
        }
        drop(allocation);
        Ok(Some(LegacyRevisionRootSecret::from_bytes(secret)))
    }

    fn write_credential(
        &mut self,
        target: &[u16],
        secret: &mut LegacyRevisionRootSecret,
    ) -> Result<(), LegacyRevisionRootSecretError> {
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
            credential_blob_size: u32::try_from(
                LEGACY_REVISION_ROOT_SECRET_BYTES,
            )
            .map_err(|_| LegacyRevisionRootSecretError::CredentialTooLarge)?,
            credential_blob: secret.expose_secret_mut().as_mut_ptr(),
            persist: CRED_PERSIST_LOCAL_MACHINE,
            attribute_count: 0,
            attributes: null_mut(),
            target_alias: null_mut(),
            user_name: user_name.as_mut_ptr(),
        };
        // SAFETY: every pointer in `credential` remains live for this call.
        let success = unsafe { cred_write_w(&raw const credential, 0) };
        if success == 0 {
            // SAFETY: GetLastError has no preconditions and is captured
            // immediately after the failed credential call.
            let error = unsafe { get_last_error() };
            return Err(
                LegacyRevisionRootSecretError::CredentialWriteFailed(error),
            );
        }
        Ok(())
    }

    fn generate_root_secret(
        &mut self,
    ) -> Result<LegacyRevisionRootSecret, LegacyRevisionRootSecretError> {
        let mut secret = [0_u8; LEGACY_REVISION_ROOT_SECRET_BYTES];
        let length = u32::try_from(secret.len())
            .map_err(|_| LegacyRevisionRootSecretError::CredentialTooLarge)?;
        // SAFETY: the buffer is live and writable for `length` bytes; a null
        // algorithm with the system-preferred flag is the documented CSPRNG path.
        let status = unsafe {
            bcrypt_gen_random(
                null_mut(),
                secret.as_mut_ptr(),
                length,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status < 0 {
            clear_bytes(&mut secret);
            return Err(
                LegacyRevisionRootSecretError::RandomGenerationFailed(status),
            );
        }
        Ok(LegacyRevisionRootSecret::from_bytes(secret))
    }

    fn acquire_vault_lock(&mut self) -> Result<Self::VaultGuard, ()> {
        WindowsVaultLock::acquire()
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

struct CredentialAllocation(*mut CredentialW);

impl Drop for CredentialAllocation {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: `self.0` is returned by CredReadW and owned exactly once.
        unsafe {
            let credential = &mut *self.0;
            let size = usize::try_from(credential.credential_blob_size)
                .unwrap_or(0)
                .min(MAX_CREDENTIAL_BLOB_BYTES);
            if !credential.credential_blob.is_null() {
                for offset in 0..size {
                    write_volatile(credential.credential_blob.add(offset), 0);
                }
                core::sync::atomic::compiler_fence(
                    core::sync::atomic::Ordering::SeqCst,
                );
            }
            cred_free(self.0.cast());
        }
        self.0 = null_mut();
    }
}

pub(super) struct WindowsVaultLock(*mut c_void);

impl WindowsVaultLock {
    fn acquire() -> Result<Self, ()> {
        Self::acquire_named(&wide(VAULT_MUTEX_NAME), VAULT_LOCK_WAIT_MILLIS)?.ok_or(())
    }

    // Shared native handle owner. Pairing uses its own exact global name and
    // short caller-budgeted waits; legacy revision timing/name stay unchanged.
    pub(super) fn acquire_named(name: &[u16], wait_ms: u32) -> Result<Option<Self>, ()> {
        if name.is_empty() || name.len() > 260 || name.last() != Some(&0)
            || name[..name.len() - 1].contains(&0) || wait_ms == u32::MAX
        {
            return Err(());
        }
        // SAFETY: null security attributes and a terminated name are valid inputs.
        let handle = unsafe { create_mutex_w(null_mut(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(());
        }
        // SAFETY: `handle` is a live mutex handle.
        let status = unsafe { wait_for_single_object(handle, wait_ms) };
        if status == WAIT_OBJECT_0 || status == WAIT_ABANDONED {
            return Ok(Some(Self(handle)));
        }
        // SAFETY: unsuccessful acquisition leaves one owned live handle to close.
        unsafe {
            close_handle(handle);
        }
        if status == WAIT_TIMEOUT { Ok(None) } else { Err(()) }
    }
}

impl Drop for WindowsVaultLock {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: this guard owns one acquired mutex handle.
        unsafe {
            release_mutex(self.0);
            close_handle(self.0);
        }
        self.0 = null_mut();
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(core::iter::once(0)).collect()
}
