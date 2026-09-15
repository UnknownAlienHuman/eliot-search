use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::io::{OpenedArtifact, open_existing, regular_nonempty};
use super::model::{
    SourceImportOutputArtifactError, SourceImportOutputArtifactPlatform,
    SourceImportPendingArtifact, SourceImportPublishedArtifact,
};
use super::super::output_lock::{
    SourceImportOutputLock, SourceImportOutputLockPlatform,
};

/// Exclusive package owner for `.pending`, final and crash-alias lifecycle.
#[derive(Debug)]
pub struct SourceImportOutputArtifact<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    platform: P,
    lock: SourceImportOutputLock<P>,
    final_path: PathBuf,
    pending_path: PathBuf,
}

impl<P> SourceImportOutputArtifact<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    /// Acquires the exact output lock and derives bounded local locators.
    pub fn acquire(
        directory: &Path,
        artifact_name: &str,
        platform: P,
        deadline: Instant,
    ) -> Result<Self, SourceImportOutputArtifactError<P::Error>> {
        let lock = SourceImportOutputLock::acquire(
            directory,
            artifact_name,
            platform.clone(),
            deadline,
        )
        .map_err(SourceImportOutputArtifactError::Lock)?;
        let final_path = directory.join(artifact_name);
        let pending_path = directory.join(format!(".{artifact_name}.pending"));
        Ok(Self {
            platform,
            lock,
            final_path,
            pending_path,
        })
    }

    /// Opens the current final artifact, or returns `None` when it is absent.
    pub fn open_final(
        &self,
        deadline: Instant,
    ) -> Result<
        Option<SourceImportPublishedArtifact<P::Identity>>,
        SourceImportOutputArtifactError<P::Error>,
    > {
        self.verify_lock(deadline)?;
        match fs::symlink_metadata(&self.final_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.verify_lock(deadline)?;
                Ok(None)
            }
            Err(_) => Err(SourceImportOutputArtifactError::OpenFailed),
            Ok(metadata) if !regular_nonempty(&metadata) => {
                Err(SourceImportOutputArtifactError::ObjectInvalid)
            }
            Ok(_) => {
                let opened = open_existing(
                    self.platform(),
                    &self.lock,
                    &self.final_path,
                    deadline,
                )?;
                Ok(Some(SourceImportPublishedArtifact {
                    file: opened.file,
                    identity: opened.identity,
                    reused: true,
                }))
            }
        }
    }

    /// Creates one empty pending file or reopens the admitted nonempty pending file.
    pub fn open_or_create_pending(
        &self,
        deadline: Instant,
    ) -> Result<
        SourceImportPendingArtifact<P::Identity>,
        SourceImportOutputArtifactError<P::Error>,
    > {
        self.verify_lock(deadline)?;
        let (opened, created) = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&self.pending_path)
        {
            Ok(file) => {
                self.platform()
                    .verify_locator(&file, &self.pending_path)
                    .map_err(SourceImportOutputArtifactError::Platform)?;
                let identity = self
                    .platform()
                    .identity(&file)
                    .map_err(SourceImportOutputArtifactError::Platform)?;
                (OpenedArtifact { file, identity }, true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                (
                    open_existing(
                        self.platform(),
                        &self.lock,
                        &self.pending_path,
                        deadline,
                    )?,
                    false,
                )
            }
            Err(_) => return Err(SourceImportOutputArtifactError::CreateFailed),
        };
        self.verify_lock(deadline)?;
        Ok(SourceImportPendingArtifact {
            file: opened.file,
            identity: opened.identity,
            created,
        })
    }

    /// Makes a newly initialized pending directory entry durable.
    pub fn sync_pending_creation(
        &self,
        deadline: Instant,
    ) -> Result<(), SourceImportOutputArtifactError<P::Error>> {
        self.verify_lock(deadline)?;
        self.platform()
            .sync_directory(self.lock.directory())
            .map_err(SourceImportOutputArtifactError::Platform)?;
        self.verify_lock(deadline)
    }

    /// Reopens the pending object and requires its exact original identity.
    pub fn open_pending_matching(
        &self,
        expected: &P::Identity,
        deadline: Instant,
    ) -> Result<
        SourceImportPendingArtifact<P::Identity>,
        SourceImportOutputArtifactError<P::Error>,
    > {
        let opened = open_existing(
            self.platform(),
            &self.lock,
            &self.pending_path,
            deadline,
        )?;
        if &opened.identity != expected {
            return Err(SourceImportOutputArtifactError::IdentityChanged);
        }
        Ok(SourceImportPendingArtifact {
            file: opened.file,
            identity: opened.identity,
            created: false,
        })
    }

    /// Publishes the exact pending object under the final locator.
    ///
    /// Existing final state is never overwritten. A reused final object must
    /// still undergo caller-owned exact redb readback before it is trusted.
    pub fn publish_pending(
        &self,
        expected_pending: &P::Identity,
        deadline: Instant,
    ) -> Result<
        SourceImportPublishedArtifact<P::Identity>,
        SourceImportOutputArtifactError<P::Error>,
    > {
        let pending =
            self.open_pending_matching(expected_pending, deadline)?;
        self.verify_lock(deadline)?;
        let reused = match fs::hard_link(&self.pending_path, &self.final_path) {
            Ok(()) => false,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                true
            }
            Err(_) => {
                return Err(
                    SourceImportOutputArtifactError::PublishOutcomeUnknown,
                );
            }
        };
        self.platform()
            .sync_directory(self.lock.directory())
            .map_err(SourceImportOutputArtifactError::Platform)?;
        self.platform()
            .verify_locator(&pending.file, &self.pending_path)
            .map_err(SourceImportOutputArtifactError::Platform)?;

        let final_artifact = self
            .open_final(deadline)?
            .ok_or(SourceImportOutputArtifactError::OpenFailed)?;
        if !reused && &final_artifact.identity != &pending.identity {
            return Err(SourceImportOutputArtifactError::IdentityChanged);
        }
        self.verify_lock(deadline)?;
        Ok(SourceImportPublishedArtifact {
            file: final_artifact.file,
            identity: final_artifact.identity,
            reused,
        })
    }

    /// Removes `.pending` only when it is an alias of the exact verified final object.
    ///
    /// `verified_final` must be captured from the same opened final file whose
    /// complete redb contents were successfully checked by the caller.
    pub fn cleanup_verified_alias(
        &self,
        verified_final: &P::Identity,
        deadline: Instant,
    ) -> Result<bool, SourceImportOutputArtifactError<P::Error>> {
        self.verify_lock(deadline)?;
        let final_artifact = open_existing(
            self.platform(),
            &self.lock,
            &self.final_path,
            deadline,
        )?;
        if &final_artifact.identity != verified_final {
            return Err(SourceImportOutputArtifactError::IdentityChanged);
        }

        match fs::symlink_metadata(&self.pending_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.verify_lock(deadline)?;
                return Ok(false);
            }
            Err(_) => return Err(SourceImportOutputArtifactError::OpenFailed),
            Ok(metadata) if !regular_nonempty(&metadata) => {
                return Err(SourceImportOutputArtifactError::ObjectInvalid);
            }
            Ok(_) => {}
        }

        let pending = open_existing(
            self.platform(),
            &self.lock,
            &self.pending_path,
            deadline,
        )?;
        if &pending.identity != &final_artifact.identity {
            self.verify_lock(deadline)?;
            return Ok(false);
        }

        self.platform()
            .verify_locator(&pending.file, &self.pending_path)
            .map_err(SourceImportOutputArtifactError::Platform)?;
        self.platform()
            .verify_locator(&final_artifact.file, &self.final_path)
            .map_err(SourceImportOutputArtifactError::Platform)?;
        self.verify_lock(deadline)?;
        drop(pending.file);
        drop(final_artifact.file);
        fs::remove_file(&self.pending_path)
            .map_err(|_| SourceImportOutputArtifactError::CleanupFailed)?;
        self.platform()
            .sync_directory(self.lock.directory())
            .map_err(SourceImportOutputArtifactError::Platform)?;
        self.verify_lock(deadline)?;
        Ok(true)
    }

    fn platform(&self) -> &P {
        &self.platform
    }

    fn verify_lock(
        &self,
        deadline: Instant,
    ) -> Result<(), SourceImportOutputArtifactError<P::Error>> {
        self.lock
            .verify(deadline)
            .map_err(SourceImportOutputArtifactError::Lock)
    }
}
