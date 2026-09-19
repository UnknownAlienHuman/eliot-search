//! Feature-gated exact Credential Manager cleanup for native test harnesses.

use core::fmt;

#[cfg(any(windows, test))]
const CREDENTIAL_TARGET_PREFIX: &str = "ELIOT Search/revision-key/";

/// Closed failure from bounded test credential cleanup.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyRevisionRootSecretCleanupError {
    /// The adapter was called on a non-Windows target.
    UnsupportedPlatform,
    /// Bounded delete/readback retries ended without proving absence.
    OutcomeUnknown,
}

impl LegacyRevisionRootSecretCleanupError {
    /// Stable package-local reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => {
                "WINDOWS_REVISION_KEY_CLEANUP_UNSUPPORTED_PLATFORM"
            }
            Self::OutcomeUnknown => {
                "WINDOWS_REVISION_KEY_CLEANUP_OUTCOME_UNKNOWN"
            }
        }
    }
}

impl fmt::Display for LegacyRevisionRootSecretCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyRevisionRootSecretCleanupError {}

/// Deletes one exact namespace root secret for bounded native test cleanup.
///
/// The operation serializes with production root-secret creation, verifies
/// absence through the package-owned credential read API after every delete
/// attempt, and never enumerates or deletes another target.
#[cfg(windows)]
pub fn delete_legacy_revision_root_secret_for_test(
    namespace_id: &[u8; 32],
) -> Result<(), LegacyRevisionRootSecretCleanupError> {
    windows::delete_credential_for_test(namespace_id)
}

/// Deletes one exact namespace root secret for bounded native test cleanup.
#[cfg(not(windows))]
pub fn delete_legacy_revision_root_secret_for_test(
    _namespace_id: &[u8; 32],
) -> Result<(), LegacyRevisionRootSecretCleanupError> {
    Err(LegacyRevisionRootSecretCleanupError::UnsupportedPlatform)
}

#[cfg(any(windows, test))]
fn credential_target(namespace_id: &[u8; 32]) -> Vec<u16> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut target = String::with_capacity(CREDENTIAL_TARGET_PREFIX.len() + 64);
    target.push_str(CREDENTIAL_TARGET_PREFIX);
    for byte in namespace_id {
        let byte = *byte;
        target.push(char::from(HEX[usize::from(byte >> 4)]));
        target.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    target.encode_utf16().chain(core::iter::once(0)).collect()
}

#[cfg(windows)]
mod windows {
    use core::ffi::c_void;
    use core::ptr::null_mut;
    use core::time::Duration;

    use super::{LegacyRevisionRootSecretCleanupError, credential_target};
    use crate::load_existing_legacy_revision_root_secret;

    const CRED_TYPE_GENERIC: u32 = 1;
    const VAULT_MUTEX_NAME: &str = "ELIOT-Search-RevisionVault-v1";
    const VAULT_LOCK_WAIT_MILLIS: u32 = 5_000;
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_ABANDONED: u32 = 0x0000_0080;
    const CLEANUP_ATTEMPTS: u32 = 16;
    const RETRY_BASE_MILLIS: u64 = 10;

    #[link(name = "Advapi32")]
    unsafe extern "system" {
        #[link_name = "CredDeleteW"]
        fn cred_delete_w(
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

    pub(super) fn delete_credential_for_test(
        namespace_id: &[u8; 32],
    ) -> Result<(), LegacyRevisionRootSecretCleanupError> {
        let target = credential_target(namespace_id);
        for attempt in 0..CLEANUP_ATTEMPTS {
            let Ok(guard) = VaultLock::acquire() else {
                std::thread::sleep(retry_delay(attempt));
                continue;
            };

            // SAFETY: `target` is the exact package-generated NUL-terminated
            // namespace credential target. The return value is intentionally
            // not authoritative; absence is verified by the normal read owner.
            unsafe {
                let _ = cred_delete_w(target.as_ptr(), CRED_TYPE_GENERIC, 0);
            }
            let absent = matches!(
                load_existing_legacy_revision_root_secret(namespace_id),
                Ok(None)
            );
            drop(guard);
            if absent {
                return Ok(());
            }
            std::thread::sleep(retry_delay(attempt));
        }
        Err(LegacyRevisionRootSecretCleanupError::OutcomeUnknown)
    }

    struct VaultLock(*mut c_void);

    impl VaultLock {
        fn acquire() -> Result<Self, ()> {
            let name = wide(VAULT_MUTEX_NAME);
            // SAFETY: null security attributes and a terminated name are valid inputs.
            let handle = unsafe { create_mutex_w(null_mut(), 0, name.as_ptr()) };
            if handle.is_null() {
                return Err(());
            }
            // SAFETY: `handle` is a live mutex handle.
            let status = unsafe {
                wait_for_single_object(handle, VAULT_LOCK_WAIT_MILLIS)
            };
            if status == WAIT_OBJECT_0 || status == WAIT_ABANDONED {
                return Ok(Self(handle));
            }
            // SAFETY: unsuccessful acquisition leaves one owned handle to close.
            unsafe {
                close_handle(handle);
            }
            Err(())
        }
    }

    impl Drop for VaultLock {
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

    fn retry_delay(attempt: u32) -> Duration {
        Duration::from_millis(RETRY_BASE_MILLIS << attempt.min(7))
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(core::iter::once(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_target_is_exact_lower_hex_and_terminated() {
        let target = credential_target(&[0xab; 32]);
        let text = String::from_utf16(&target[..target.len() - 1]).expect("target");
        assert_eq!(
            text,
            "ELIOT Search/revision-key/abababababababababababababababababababababababababababababababab"
        );
        assert_eq!(target.last(), Some(&0));
    }

    #[test]
    fn cleanup_reason_codes_are_stable() {
        assert_eq!(
            LegacyRevisionRootSecretCleanupError::OutcomeUnknown.code(),
            "WINDOWS_REVISION_KEY_CLEANUP_OUTCOME_UNKNOWN"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_cleanup_fails_closed() {
        assert_eq!(
            delete_legacy_revision_root_secret_for_test(&[0x55; 32]),
            Err(LegacyRevisionRootSecretCleanupError::UnsupportedPlatform)
        );
    }
}
