use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::time::Instant;

use super::model::{
    SourceImportOutputArtifactError, SourceImportOutputArtifactPlatform,
};
use super::super::output_lock::{
    SourceImportOutputLock, SourceImportOutputLockPlatform,
};

#[derive(Debug)]
pub(super) struct OpenedArtifact<I> {
    pub(super) file: File,
    pub(super) identity: I,
}

pub(super) fn open_existing<P>(
    platform: &P,
    lock: &SourceImportOutputLock<P>,
    path: &Path,
    deadline: Instant,
) -> Result<OpenedArtifact<P::Identity>, SourceImportOutputArtifactError<P::Error>>
where
    P: SourceImportOutputArtifactPlatform,
{
    verify_lock(lock, deadline)?;
    let before = fs::symlink_metadata(path)
        .map_err(|_| SourceImportOutputArtifactError::ObjectInvalid)?;
    if !regular_nonempty(&before) {
        return Err(SourceImportOutputArtifactError::ObjectInvalid);
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| SourceImportOutputArtifactError::ObjectInvalid)?;
    let opened = file
        .metadata()
        .map_err(|_| SourceImportOutputArtifactError::ObjectInvalid)?;
    if !opened.is_file()
        || opened.len() != before.len()
        || opened.modified().ok() != before.modified().ok()
    {
        return Err(SourceImportOutputArtifactError::ObjectInvalid);
    }
    platform
        .verify_locator(&file, path)
        .map_err(SourceImportOutputArtifactError::Platform)?;
    let identity = platform
        .identity(&file)
        .map_err(SourceImportOutputArtifactError::Platform)?;
    verify_lock(lock, deadline)?;
    Ok(OpenedArtifact { file, identity })
}

pub(super) fn regular_nonempty(metadata: &fs::Metadata) -> bool {
    !metadata.file_type().is_symlink()
        && metadata.is_file()
        && metadata.len() > 0
}

fn verify_lock<P>(
    lock: &SourceImportOutputLock<P>,
    deadline: Instant,
) -> Result<(), SourceImportOutputArtifactError<P::Error>>
where
    P: SourceImportOutputArtifactPlatform,
{
    lock.verify(deadline)
        .map_err(SourceImportOutputArtifactError::Lock)
}
