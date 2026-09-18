//! Test-only serialization and credential cleanup around the native vault.

/// Process-wide serializer for tests that touch revision-key credentials.
///
/// Credential Manager mutations from parallel test threads are serialized so
/// readback and deletion observations cannot race inside one harness process.
static UNIT_VAULT_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Hold the unit-vault serializer for a whole test body.
///
/// A poisoned lock is recovered because failure-path tests may intentionally
/// panic while holding it.
pub fn lock_unit_vault_for_test() -> std::sync::MutexGuard<'static, ()> {
    UNIT_VAULT_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Drop-based best-effort cleanup for one test data root.
pub struct TestCredentialGuard {
    data_root: std::path::PathBuf,
}

impl TestCredentialGuard {
    /// Creates a guard without reading or mutating the credential store.
    pub(crate) fn for_data_root(data_root: &std::path::Path) -> Self {
        Self {
            data_root: data_root.to_owned(),
        }
    }

    /// Deletes the test credential when the platform adapter supports it.
    pub(crate) fn cleanup(&self) {
        #[cfg(windows)]
        {
            super::windows::delete_test_credential_for_data_root(
                &self.data_root,
            );
        }
        #[cfg(not(windows))]
        {
            let _ = &self.data_root;
        }
    }
}

impl Drop for TestCredentialGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}
