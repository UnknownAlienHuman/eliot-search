//! Exclusive logical ownership for one inactive source-import output artifact.
//!
//! This module owns the lock-file lifecycle and verification order. Native
//! directory admission, opened-object identity and directory durability stay
//! behind an injected platform boundary; the control package does not depend
//! on daemon internals or reopen migration artifacts on its own.

use std::ffi::OsStr;
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Platform observations required by the package-owned output lock.
pub trait SourceImportOutputLockPlatform {
    /// Platform failure retained without rewriting its established reason.
    type Error;

    /// Verifies that the output directory is an admitted real directory.
    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error>;

    /// Verifies that `path` still resolves to the exact already-open lock file.
    fn verify_locator(&self, expected: &File, path: &Path) -> Result<(), Self::Error>;

    /// Makes a newly created lock-directory entry durable where supported.
    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error>;
}

/// Closed package failures plus an exact platform failure.
#[derive(Debug)]
pub enum SourceImportOutputLockError<E> {
    /// Injected platform verification failed.
    Platform(E),
    /// The artifact name is not one plain local filename.
    InvalidArtifactName,
    /// The finite operation deadline elapsed.
    DeadlineExceeded,
    /// Existing lock metadata is not a zero-length regular file.
    LockInvalid,
    /// The lock file could not be created or opened.
    LockOpenFailed,
    /// Another process already owns the logical output.
    AlreadyOwned,
    /// Native exclusive locking failed for a reason other than contention.
    LockFailed,
    /// A newly created lock file could not be flushed.
    LockSyncFailed,
}

impl<E: fmt::Display> SourceImportOutputLockError<E> {
    /// Converts the package failure to the preserved stable reason string.
    #[must_use]
    pub fn into_reason(self) -> String {
        match self {
            Self::Platform(error) => error.to_string(),
            Self::InvalidArtifactName => {
                "DIRECT_MIGRATION_OUTPUT_NAME_INVALID".to_owned()
            }
            Self::DeadlineExceeded => "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned(),
            Self::LockInvalid => "DIRECT_MIGRATION_OUTPUT_LOCK_INVALID".to_owned(),
            Self::LockOpenFailed => "DIRECT_MIGRATION_OUTPUT_LOCK_OPEN_FAILED".to_owned(),
            Self::AlreadyOwned => "DIRECT_MIGRATION_OUTPUT_ALREADY_OWNED".to_owned(),
            Self::LockFailed => "DIRECT_MIGRATION_OUTPUT_LOCK_FAILED".to_owned(),
            Self::LockSyncFailed => "DIRECT_MIGRATION_OUTPUT_LOCK_SYNC_FAILED".to_owned(),
        }
    }
}

/// One non-clone exclusive lock for an inactive import output name.
///
/// The empty lock file is deliberately retained after release. Unlinking it
/// could allow another inode with the same locator to be locked while an older
/// process still owns the first inode.
#[derive(Debug)]
pub struct SourceImportOutputLock<P> {
    directory: PathBuf,
    path: PathBuf,
    file: File,
    platform: P,
}

impl<P> SourceImportOutputLock<P>
where
    P: SourceImportOutputLockPlatform,
{
    /// Acquires and verifies the exact logical output lock.
    pub fn acquire(
        directory: &Path,
        artifact_name: &str,
        platform: P,
        deadline: Instant,
    ) -> Result<Self, SourceImportOutputLockError<P::Error>> {
        check_deadline::<P::Error>(deadline)?;
        validate_artifact_name::<P::Error>(artifact_name)?;
        platform
            .validate_directory(directory)
            .map_err(SourceImportOutputLockError::Platform)?;
        let path = directory.join(format!(".{artifact_name}.lock"));
        let (file, created) = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => (file, true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata = std::fs::symlink_metadata(&path)
                    .map_err(|_| SourceImportOutputLockError::LockInvalid)?;
                if metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() != 0
                {
                    return Err(SourceImportOutputLockError::LockInvalid);
                }
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .map_err(|_| SourceImportOutputLockError::LockOpenFailed)?;
                (file, false)
            }
            Err(_) => return Err(SourceImportOutputLockError::LockOpenFailed),
        };

        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(SourceImportOutputLockError::AlreadyOwned);
            }
            Err(TryLockError::Error(_)) => {
                return Err(SourceImportOutputLockError::LockFailed);
            }
        }

        let guard = Self {
            directory: directory.to_owned(),
            path,
            file,
            platform,
        };
        guard.verify(deadline)?;
        if created {
            guard
                .file
                .sync_all()
                .map_err(|_| SourceImportOutputLockError::LockSyncFailed)?;
            guard
                .platform
                .sync_directory(directory)
                .map_err(SourceImportOutputLockError::Platform)?;
        }
        guard.verify(deadline)?;
        Ok(guard)
    }

    /// Revalidates directory admission, exact locator binding and zero length.
    pub fn verify(
        &self,
        deadline: Instant,
    ) -> Result<(), SourceImportOutputLockError<P::Error>> {
        check_deadline::<P::Error>(deadline)?;
        self.platform
            .validate_directory(&self.directory)
            .map_err(SourceImportOutputLockError::Platform)?;
        self.platform
            .verify_locator(&self.file, &self.path)
            .map_err(SourceImportOutputLockError::Platform)?;
        if self
            .file
            .metadata()
            .map_err(|_| SourceImportOutputLockError::LockInvalid)?
            .len()
            != 0
        {
            return Err(SourceImportOutputLockError::LockInvalid);
        }
        check_deadline::<P::Error>(deadline)
    }

    /// Exact admitted parent directory retained by this lock.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

fn validate_artifact_name<E>(
    artifact_name: &str,
) -> Result<(), SourceImportOutputLockError<E>> {
    let path = Path::new(artifact_name);
    if artifact_name.is_empty()
        || matches!(artifact_name, "." | "..")
        || path.file_name() != Some(OsStr::new(artifact_name))
    {
        Err(SourceImportOutputLockError::InvalidArtifactName)
    } else {
        Ok(())
    }
}

fn check_deadline<E>(
    deadline: Instant,
) -> Result<(), SourceImportOutputLockError<E>> {
    if Instant::now() >= deadline {
        Err(SourceImportOutputLockError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use super::{
        SourceImportOutputLock, SourceImportOutputLockError,
        SourceImportOutputLockPlatform,
    };

    #[derive(Clone, Copy, Debug)]
    struct TestPlatform;

    impl SourceImportOutputLockPlatform for TestPlatform {
        type Error = io::Error;

        fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(io::Error::other("invalid directory"));
            }
            Ok(())
        }

        fn verify_locator(
            &self,
            _expected: &fs::File,
            path: &Path,
        ) -> Result<(), Self::Error> {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(io::Error::other("invalid locator"));
            }
            Ok(())
        }

        fn sync_directory(&self, _path: &Path) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "eliot-source-import-lock-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).expect("create fixture");
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn one_logical_output_has_one_live_owner_and_retains_its_lock_file() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        let first = SourceImportOutputLock::acquire(
            &fixture.0,
            "artifact.redb",
            TestPlatform,
            deadline,
        )
        .expect("first owner");

        assert!(matches!(
            SourceImportOutputLock::acquire(
                &fixture.0,
                "artifact.redb",
                TestPlatform,
                deadline,
            ),
            Err(SourceImportOutputLockError::AlreadyOwned)
        ));
        drop(first);

        let reopened = SourceImportOutputLock::acquire(
            &fixture.0,
            "artifact.redb",
            TestPlatform,
            deadline,
        )
        .expect("reopen retained lock");
        reopened.verify(deadline).expect("verified");
        assert!(fixture.0.join(".artifact.redb.lock").is_file());
    }

    #[test]
    fn invalid_name_lock_state_and_elapsed_deadline_fail_closed() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        fs::write(fixture.0.join(".artifact.redb.lock"), b"x")
            .expect("precreate invalid lock");
        assert!(matches!(
            SourceImportOutputLock::acquire(
                &fixture.0,
                "artifact.redb",
                TestPlatform,
                deadline,
            ),
            Err(SourceImportOutputLockError::LockInvalid)
        ));
        assert!(matches!(
            SourceImportOutputLock::acquire(
                &fixture.0,
                "../escape.redb",
                TestPlatform,
                deadline,
            ),
            Err(SourceImportOutputLockError::InvalidArtifactName)
        ));
        assert!(matches!(
            SourceImportOutputLock::acquire(
                &fixture.0,
                "other.redb",
                TestPlatform,
                Instant::now() - Duration::from_millis(1),
            ),
            Err(SourceImportOutputLockError::DeadlineExceeded)
        ));
    }
}
