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
    #[cfg(windows)]
    captured_namespace: std::cell::Cell<Option<[u8; 32]>>,
    #[cfg(windows)]
    native_fixture: bool,
    #[cfg(windows)]
    captured_outcome: std::sync::OnceLock<bool>,
}

impl TestCredentialGuard {
    /// Creates a guard without reading or mutating the credential store.
    pub(crate) fn for_data_root(data_root: &std::path::Path) -> Self {
        Self {
            data_root: data_root.to_owned(),
            #[cfg(windows)]
            captured_namespace: std::cell::Cell::new(None),
            #[cfg(windows)]
            native_fixture: false,
            #[cfg(windows)]
            captured_outcome: std::sync::OnceLock::new(),
        }
    }

    /// Register a newly created, owned Native fixture before initialization.
    #[cfg(windows)]
    pub(crate) fn for_native_data_root(data_root: &std::path::Path) -> Self {
        let root = std::fs::canonicalize(data_root).expect("owned Native fixture root");
        let temp = std::fs::canonicalize(std::env::temp_dir()).expect("fixture Temp root");
        assert!(root != temp && root.starts_with(temp));
        let mut guard = Self::for_data_root(&root);
        guard.native_fixture = true;
        guard
    }

    /// The immutable original wins before a fixture corrupts journal/namespace files.
    #[cfg(windows)]
    pub(crate) fn capture_native_namespace(&self) -> Result<Option<[u8; 32]>, &'static str> {
        if let Some(namespace) = self.captured_namespace.get() { return Ok(Some(namespace)); }
        if !self.native_fixture { return Err("fixture-original-not-registered"); }
        let namespace = crate::owner_composition::fixture_initialization_namespace(&self.data_root)?;
        self.captured_namespace.set(namespace);
        Ok(namespace)
    }

    /// Attempts exact cleanup once, including on unwind; false retains artifacts.
    #[cfg(windows)]
    pub(crate) fn cleanup_captured(&self) -> bool {
        *self.captured_outcome.get_or_init(|| {
            let outcome = matches!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let namespace = self.capture_native_namespace()?;
                    if let Some(namespace) = namespace {
                        search_os_secrets_windows::delete_legacy_revision_root_secret_for_test(&namespace)
                            .map_err(|error| error.code())?;
                    }
                    Ok::<_, &'static str>(())
                })),
                Ok(Ok(()))
            );
            if !outcome {
                let _ = std::io::Write::write_all(
                    &mut std::io::stderr(),
                    b"ELIOT_TEST_CLEANUP: exact fixture credential unresolved; artifacts retained\n",
                );
            }
            outcome
        })
    }

    /// Deletes the test credential when the platform adapter supports it.
    pub(crate) fn cleanup(&self) {
        #[cfg(windows)]
        {
            if self.native_fixture {
                self.cleanup_captured();
                return;
            }
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
        #[cfg(windows)]
        if self.native_fixture {
            let cleaned = self.cleanup_captured();
            assert!(cleaned || std::thread::panicking(), "exact fixture credential unresolved; disk retained");
            return;
        }
        self.cleanup();
    }
}
