//! Qualified filesystem lifecycle for legacy DIRECT preparation artifacts.
//!
//! This compatibility adapter owns bounded exact reads and no-clobber
//! immutable publication for preparation objects and reference records while
//! T02 moves daemon mechanics to `search-materializer`. Bytes remain opaque:
//! profile semantics, revision protection and preparation decoding are owned by
//! their existing package modules and daemon composition.

#![allow(clippy::module_name_repetitions)]

use core::fmt;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Native observations required by the package-owned preparation-artifact lifecycle.
pub trait LegacyPreparationArtifactPlatform {
    /// Stable native identity used to fence locator replacement.
    type Identity: Clone + fmt::Debug + Eq;
    /// Platform-specific validation failure.
    type Error;

    /// Requires one real admitted directory, excluding links/reparse points.
    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error>;

    /// Requires `path` to resolve to the exact already-open file.
    fn verify_locator(&self, expected: &File, path: &Path) -> Result<(), Self::Error>;

    /// Returns stable identity for one already-open regular file.
    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error>;

    /// Makes directory-entry changes durable where supported.
    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error>;
}

/// One exact bounded preparation-artifact read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyPreparationArtifactRead<I> {
    bytes: Vec<u8>,
    identity: I,
}

impl<I> LegacyPreparationArtifactRead<I> {
    /// Exact artifact bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the observation and returns exact bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Stable identity observed before and after readback.
    #[must_use]
    pub const fn identity(&self) -> &I {
        &self.identity
    }

    /// Exact byte count.
    #[must_use]
    pub fn encoded_bytes(&self) -> u64 {
        u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
    }
}

/// Receipt for one exact immutable preparation publication or reuse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyPreparationArtifactReceipt<I> {
    identity: I,
    encoded_bytes: u64,
    reused: bool,
}

impl<I> LegacyPreparationArtifactReceipt<I> {
    /// Stable identity of the verified final artifact.
    #[must_use]
    pub const fn identity(&self) -> &I {
        &self.identity
    }

    /// Exact verified byte count.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    /// Whether a byte-identical existing final artifact was reused.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.reused
    }
}

/// Closed preparation-artifact lifecycle failure.
#[derive(Debug)]
pub enum LegacyPreparationArtifactError<E> {
    /// Injected platform observation failed.
    Platform(E),
    /// A required parent directory is absent from the locator shape.
    ParentMissing,
    /// Final or temporary locator is not one local basename.
    LocalNameInvalid,
    /// Supplied or observed bytes exceed the explicit bound.
    SizeInvalid,
    /// Required child directory could not be created.
    DirectoryCreate(io::Error),
    /// Existing artifact state could not be inspected.
    ObjectInspect(io::Error),
    /// Locator is not one admitted regular file.
    ObjectInvalid,
    /// Exact bounded read failed.
    ObjectRead(io::Error),
    /// Length, timestamp, locator or identity changed during readback.
    ReadbackMismatch,
    /// Temporary artifact could not be created without clobbering.
    Create(io::Error),
    /// Temporary write or file sync failed.
    Write(io::Error),
    /// Publication may have produced an externally visible final artifact.
    PublishOutcomeUnknown(io::Error),
    /// Publication succeeded but directory durability could not be proved.
    PublishPlatformOutcomeUnknown(E),
    /// Existing final bytes conflict with the proposed immutable artifact.
    ImmutableConflict,
    /// Temporary or final locator changed native identity.
    IdentityChanged,
    /// Exact temporary artifact could not be removed.
    Cleanup(io::Error),
}

impl<E: fmt::Display> fmt::Display for LegacyPreparationArtifactError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform(error) => error.fmt(formatter),
            Self::ParentMissing => formatter.write_str("preparation artifact parent missing"),
            Self::LocalNameInvalid => formatter.write_str("preparation artifact local name invalid"),
            Self::SizeInvalid => formatter.write_str("preparation artifact size invalid"),
            Self::DirectoryCreate(error) => write!(formatter, "preparation directory create failed: {error}"),
            Self::ObjectInspect(error) => write!(formatter, "preparation artifact inspect failed: {error}"),
            Self::ObjectInvalid => formatter.write_str("preparation artifact invalid"),
            Self::ObjectRead(error) => write!(formatter, "preparation artifact read failed: {error}"),
            Self::ReadbackMismatch => formatter.write_str("preparation artifact readback mismatch"),
            Self::Create(error) => write!(formatter, "preparation artifact create failed: {error}"),
            Self::Write(error) => write!(formatter, "preparation artifact write failed: {error}"),
            Self::PublishOutcomeUnknown(error) => write!(formatter, "preparation artifact publish outcome unknown: {error}"),
            Self::PublishPlatformOutcomeUnknown(error) => write!(formatter, "preparation artifact publish durability unknown: {error}"),
            Self::ImmutableConflict => formatter.write_str("preparation artifact immutable conflict"),
            Self::IdentityChanged => formatter.write_str("preparation artifact identity changed"),
            Self::Cleanup(error) => write!(formatter, "preparation artifact cleanup failed: {error}"),
        }
    }
}

impl<E> std::error::Error for LegacyPreparationArtifactError<E>
where
    E: fmt::Debug + fmt::Display,
{
}

/// Reads one complete immutable preparation artifact through one opened file.
///
/// # Errors
///
/// Returns a typed failure for invalid or unstable state, an oversize object,
/// platform validation failure or bounded read error.
pub fn read_legacy_preparation_artifact<P>(
    platform: &P,
    path: &Path,
    maximum_bytes: usize,
) -> Result<LegacyPreparationArtifactRead<P::Identity>, LegacyPreparationArtifactError<P::Error>>
where
    P: LegacyPreparationArtifactPlatform,
{
    read_artifact(platform, path, maximum_bytes, None)
}

/// Publishes one exact immutable preparation artifact without replacement.
///
/// # Errors
///
/// Returns a typed failure for invalid names/bounds, I/O or platform failure,
/// immutable conflict, changed identity, uncertain publication or cleanup.
pub fn publish_legacy_preparation_artifact<P>(
    platform: &P,
    directory: &Path,
    final_name: &str,
    temporary_name: &str,
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<LegacyPreparationArtifactReceipt<P::Identity>, LegacyPreparationArtifactError<P::Error>>
where
    P: LegacyPreparationArtifactPlatform,
{
    if bytes.len() > maximum_bytes {
        return Err(LegacyPreparationArtifactError::SizeInvalid);
    }
    if !valid_local_name(final_name)
        || !valid_local_name(temporary_name)
        || final_name == temporary_name
        || !temporary_name.starts_with('.')
        || !temporary_name.ends_with(".tmp")
    {
        return Err(LegacyPreparationArtifactError::LocalNameInvalid);
    }

    ensure_child_directory(platform, directory)?;
    let final_path = directory.join(final_name);
    let temporary_path = directory.join(temporary_name);
    match fs::symlink_metadata(&final_path) {
        Ok(_) => {
            let existing = read_artifact(platform, &final_path, maximum_bytes, None)?;
            if existing.bytes() != bytes {
                return Err(LegacyPreparationArtifactError::ImmutableConflict);
            }
            let encoded_bytes = existing.encoded_bytes();
            let identity = existing.identity;
            return Ok(LegacyPreparationArtifactReceipt {
                identity,
                encoded_bytes,
                reused: true,
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(LegacyPreparationArtifactError::ObjectInspect(error)),
    }

    let mut temporary = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(LegacyPreparationArtifactError::Create)?;
    let metadata = temporary
        .metadata()
        .map_err(LegacyPreparationArtifactError::Create)?;
    if !regular(&metadata) {
        return Err(LegacyPreparationArtifactError::ObjectInvalid);
    }
    let temporary_identity = platform
        .identity(&temporary)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    let mut staging = TemporaryArtifactGuard {
        platform,
        path: temporary_path,
        identity: temporary_identity.clone(),
        cleanup_armed: true,
    };
    platform
        .verify_locator(&temporary, &staging.path)
        .map_err(LegacyPreparationArtifactError::Platform)?;

    if let Err(error) = temporary.write_all(bytes).and_then(|()| temporary.sync_all()) {
        drop(temporary);
        return Err(LegacyPreparationArtifactError::Write(error));
    }
    drop(temporary);

    let staged = read_artifact(
        platform,
        &staging.path,
        maximum_bytes,
        Some(&temporary_identity),
    )?;
    if staged.bytes() != bytes {
        return Err(LegacyPreparationArtifactError::ReadbackMismatch);
    }

    let reused = match fs::hard_link(&staging.path, &final_path) {
        Ok(()) => false,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => true,
        Err(error) => return Err(LegacyPreparationArtifactError::PublishOutcomeUnknown(error)),
    };
    if let Err(error) = platform.sync_directory(directory) {
        return Err(LegacyPreparationArtifactError::PublishPlatformOutcomeUnknown(error));
    }
    let final_artifact = read_artifact(platform, &final_path, maximum_bytes, None)?;
    if final_artifact.bytes() != bytes {
        return Err(LegacyPreparationArtifactError::ImmutableConflict);
    }
    if !reused && final_artifact.identity() != &temporary_identity {
        return Err(LegacyPreparationArtifactError::IdentityChanged);
    }
    let encoded_bytes = final_artifact.encoded_bytes();
    let identity = final_artifact.identity;

    staging.cleanup()?;
    platform
        .sync_directory(directory)
        .map_err(LegacyPreparationArtifactError::PublishPlatformOutcomeUnknown)?;
    Ok(LegacyPreparationArtifactReceipt {
        identity,
        encoded_bytes,
        reused,
    })
}

struct TemporaryArtifactGuard<'a, P>
where
    P: LegacyPreparationArtifactPlatform,
{
    platform: &'a P,
    path: PathBuf,
    identity: P::Identity,
    cleanup_armed: bool,
}

impl<P> TemporaryArtifactGuard<'_, P>
where
    P: LegacyPreparationArtifactPlatform,
{
    fn cleanup(&mut self) -> Result<(), LegacyPreparationArtifactError<P::Error>> {
        if !core::mem::replace(&mut self.cleanup_armed, false) {
            return Ok(());
        }
        cleanup_exact(self.platform, &self.path, &self.identity)
    }
}

impl<P> Drop for TemporaryArtifactGuard<'_, P>
where
    P: LegacyPreparationArtifactPlatform,
{
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn read_artifact<P>(
    platform: &P,
    path: &Path,
    maximum_bytes: usize,
    expected_identity: Option<&P::Identity>,
) -> Result<LegacyPreparationArtifactRead<P::Identity>, LegacyPreparationArtifactError<P::Error>>
where
    P: LegacyPreparationArtifactPlatform,
{
    let parent = path
        .parent()
        .ok_or(LegacyPreparationArtifactError::ParentMissing)?;
    platform
        .validate_directory(parent)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    let before = fs::symlink_metadata(path)
        .map_err(LegacyPreparationArtifactError::ObjectInspect)?;
    let before_len = usize::try_from(before.len())
        .map_err(|_| LegacyPreparationArtifactError::SizeInvalid)?;
    if !regular(&before) || before_len > maximum_bytes {
        return Err(if before_len > maximum_bytes {
            LegacyPreparationArtifactError::SizeInvalid
        } else {
            LegacyPreparationArtifactError::ObjectInvalid
        });
    }

    let mut file = File::open(path).map_err(LegacyPreparationArtifactError::ObjectRead)?;
    let opened = file
        .metadata()
        .map_err(LegacyPreparationArtifactError::ObjectRead)?;
    if !regular(&opened)
        || opened.len() != before.len()
        || opened.modified().ok() != before.modified().ok()
    {
        return Err(LegacyPreparationArtifactError::ReadbackMismatch);
    }
    platform
        .verify_locator(&file, path)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    let identity = platform
        .identity(&file)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    if expected_identity.is_some_and(|expected| expected != &identity) {
        return Err(LegacyPreparationArtifactError::IdentityChanged);
    }

    let mut artifact_bytes = Vec::with_capacity(before_len);
    let limit = u64::try_from(maximum_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    (&mut file)
        .take(limit)
        .read_to_end(&mut artifact_bytes)
        .map_err(LegacyPreparationArtifactError::ObjectRead)?;
    let after = file
        .metadata()
        .map_err(LegacyPreparationArtifactError::ObjectRead)?;
    if artifact_bytes.len() != before_len
        || artifact_bytes.len() > maximum_bytes
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err(LegacyPreparationArtifactError::ReadbackMismatch);
    }
    platform
        .verify_locator(&file, path)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    let after_identity = platform
        .identity(&file)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    if after_identity != identity {
        return Err(LegacyPreparationArtifactError::IdentityChanged);
    }
    Ok(LegacyPreparationArtifactRead {
        bytes: artifact_bytes,
        identity,
    })
}

fn ensure_child_directory<P>(
    platform: &P,
    directory: &Path,
) -> Result<(), LegacyPreparationArtifactError<P::Error>>
where
    P: LegacyPreparationArtifactPlatform,
{
    let parent = directory
        .parent()
        .ok_or(LegacyPreparationArtifactError::ParentMissing)?;
    platform
        .validate_directory(parent)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    match fs::symlink_metadata(directory) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match fs::create_dir(directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(LegacyPreparationArtifactError::DirectoryCreate(error)),
            }
            platform
                .sync_directory(parent)
                .map_err(LegacyPreparationArtifactError::Platform)?;
        }
        Err(error) => return Err(LegacyPreparationArtifactError::ObjectInspect(error)),
    }
    platform
        .validate_directory(directory)
        .map_err(LegacyPreparationArtifactError::Platform)
}

fn cleanup_exact<P>(
    platform: &P,
    path: &Path,
    expected_identity: &P::Identity,
) -> Result<(), LegacyPreparationArtifactError<P::Error>>
where
    P: LegacyPreparationArtifactPlatform,
{
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(LegacyPreparationArtifactError::Cleanup(error)),
        Ok(metadata) if !regular(&metadata) => {
            return Err(LegacyPreparationArtifactError::IdentityChanged);
        }
        Ok(_) => {}
    }
    let file = File::open(path).map_err(LegacyPreparationArtifactError::Cleanup)?;
    platform
        .verify_locator(&file, path)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    let identity = platform
        .identity(&file)
        .map_err(LegacyPreparationArtifactError::Platform)?;
    if &identity != expected_identity {
        return Err(LegacyPreparationArtifactError::IdentityChanged);
    }
    drop(file);
    fs::remove_file(path).map_err(LegacyPreparationArtifactError::Cleanup)
}

fn valid_local_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && Path::new(name).file_name() == Some(OsStr::new(name))
}

fn regular(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests;
