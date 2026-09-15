use std::fmt;
use std::fs::File;

use super::super::output_lock::{
    SourceImportOutputLockError, SourceImportOutputLockPlatform,
};

/// Platform identity observation required by the package-owned artifact lifecycle.
pub trait SourceImportOutputArtifactPlatform:
    SourceImportOutputLockPlatform + Clone
{
    /// Stable native identity for one opened regular file.
    type Identity: Clone + Eq;

    /// Observes the stable identity of the already-open file.
    fn identity(
        &self,
        file: &File,
    ) -> Result<Self::Identity, Self::Error>;
}

/// Closed artifact-lifecycle failures plus exact platform and lock failures.
#[derive(Debug)]
pub enum SourceImportOutputArtifactError<E> {
    /// The exact output lock failed or expired.
    Lock(SourceImportOutputLockError<E>),
    /// The injected platform observer failed.
    Platform(E),
    /// An existing pending/final locator is not an admitted nonempty regular file.
    ObjectInvalid,
    /// An existing pending/final artifact could not be inspected or opened.
    OpenFailed,
    /// A new pending artifact could not be created.
    CreateFailed,
    /// An opened locator no longer names the expected native object.
    IdentityChanged,
    /// Hard-link publication may have had an externally visible effect.
    PublishOutcomeUnknown,
    /// A verified pending alias could not be removed.
    CleanupFailed,
}

impl<E: fmt::Display> SourceImportOutputArtifactError<E> {
    /// Converts the package failure to the preserved stable daemon reason.
    #[must_use]
    pub fn into_reason(self) -> String {
        match self {
            Self::Lock(error) => error.into_reason(),
            Self::Platform(error) => error.to_string(),
            Self::ObjectInvalid => {
                "DIRECT_MIGRATION_IMPORT_OBJECT_INVALID".to_owned()
            }
            Self::OpenFailed => {
                "DIRECT_MIGRATION_IMPORT_OPEN_FAILED".to_owned()
            }
            Self::CreateFailed => {
                "DIRECT_MIGRATION_IMPORT_CREATE_FAILED".to_owned()
            }
            Self::IdentityChanged => {
                "DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned()
            }
            Self::PublishOutcomeUnknown => {
                "DIRECT_MIGRATION_IMPORT_PUBLISH_OUTCOME_UNKNOWN".to_owned()
            }
            Self::CleanupFailed => {
                "DIRECT_MIGRATION_IMPORT_CLEANUP_FAILED".to_owned()
            }
        }
    }
}

/// One opened pending artifact and the identity observed before redb owns it.
#[derive(Debug)]
pub struct SourceImportPendingArtifact<I> {
    pub(super) file: File,
    pub(super) identity: I,
    pub(super) created: bool,
}

impl<I> SourceImportPendingArtifact<I> {
    /// Native identity of the opened pending object.
    #[must_use]
    pub const fn identity(&self) -> &I {
        &self.identity
    }

    /// Whether this invocation created the empty pending locator.
    #[must_use]
    pub const fn created(&self) -> bool {
        self.created
    }

    /// Transfers the opened file to the redb import writer.
    #[must_use]
    pub fn into_file(self) -> File {
        self.file
    }
}

/// One opened final artifact after package-owned publication checks.
#[derive(Debug)]
pub struct SourceImportPublishedArtifact<I> {
    pub(super) file: File,
    pub(super) identity: I,
    pub(super) reused: bool,
}

impl<I> SourceImportPublishedArtifact<I> {
    /// Native identity of the opened final object.
    #[must_use]
    pub const fn identity(&self) -> &I {
        &self.identity
    }

    /// Whether the final locator already existed before the hard-link attempt.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.reused
    }

    /// Transfers the opened final file to exact redb readback.
    #[must_use]
    pub fn into_file(self) -> File {
        self.file
    }
}
