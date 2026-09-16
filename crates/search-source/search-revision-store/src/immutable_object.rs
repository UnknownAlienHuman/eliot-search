//! Qualified filesystem lifecycle for legacy immutable revision objects.
//!
//! This adapter owns bounded exact reads and no-clobber immutable publication
//! for the legacy DIRECT `.bin` / `.dpapi` object layout while Phase 3 moves
//! daemon storage behavior to its package owner. Bytes are opaque here: secret
//! protection, plaintext verification, source identity and catalog authority
//! remain with their existing owners.

#![allow(clippy::module_name_repetitions)]

use core::fmt;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

/// Native observations required by the package-owned immutable-object lifecycle.
pub trait LegacyRevisionObjectPlatform {
    /// Stable native object identity used to fence locator replacement.
    type Identity: Clone + fmt::Debug + Eq;
    /// Platform-specific validation failure.
    type Error;

    /// Requires one real admitted directory, excluding links/reparse points.
    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error>;

    /// Requires `path` to resolve to the exact already-open file.
    fn verify_locator(&self, expected: &File, path: &Path) -> Result<(), Self::Error>;

    /// Returns the stable native identity of an already-open regular file.
    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error>;

    /// Makes directory-entry changes durable where the platform supports it.
    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error>;
}

/// One exact immutable-object read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyRevisionObjectRead<I> {
    bytes: Vec<u8>,
    identity: I,
}

impl<I> LegacyRevisionObjectRead<I> {
    /// Exact object bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the observation and returns its exact bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Stable identity observed before and after the read.
    #[must_use]
    pub const fn identity(&self) -> &I {
        &self.identity
    }

    /// Exact encoded byte count.
    #[must_use]
    pub fn encoded_bytes(&self) -> u64 {
        u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
    }
}

/// Receipt for one exact immutable-object publication or reuse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyRevisionObjectReceipt<I> {
    identity: I,
    encoded_bytes: u64,
    reused: bool,
}

impl<I> LegacyRevisionObjectReceipt<I> {
    /// Stable identity of the verified final object.
    #[must_use]
    pub const fn identity(&self) -> &I {
        &self.identity
    }

    /// Exact verified object length.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    /// Whether a pre-existing byte-identical final object was reused.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.reused
    }
}

/// Closed immutable-object lifecycle failure.
#[derive(Debug)]
pub enum LegacyRevisionObjectError<E> {
    /// Injected platform validation failed before publication.
    Platform(E),
    /// A required parent directory was absent from the locator shape.
    ParentMissing,
    /// A final or temporary locator was not one plain local filename.
    LocalNameInvalid,
    /// Supplied or observed object bytes exceed the explicit ceiling.
    SizeInvalid,
    /// A required shard directory could not be created.
    DirectoryCreate(io::Error),
    /// Existing final-object state could not be inspected.
    ObjectInspect(io::Error),
    /// A locator is not one admitted regular file.
    ObjectInvalid,
    /// Exact bounded object read failed.
    ObjectRead(io::Error),
    /// Length, timestamp, locator or identity changed during readback.
    ReadbackMismatch,
    /// A temporary object could not be created without clobbering.
    Create(io::Error),
    /// Temporary write or file sync failed.
    Write(io::Error),
    /// Publication may have produced an externally visible final object.
    PublishOutcomeUnknown(io::Error),
    /// Publication succeeded but directory durability could not be proved.
    PublishPlatformOutcomeUnknown(E),
    /// Existing final bytes conflict with the proposed immutable object.
    ImmutableConflict,
    /// A temporary or final locator changed native identity.
    IdentityChanged,
    /// The exact temporary object could not be removed.
    Cleanup(io::Error),
}

impl<E: fmt::Display> fmt::Display for LegacyRevisionObjectError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform(error) => error.fmt(formatter),
            Self::ParentMissing => formatter.write_str("revision object parent missing"),
            Self::LocalNameInvalid => formatter.write_str("revision object local name invalid"),
            Self::SizeInvalid => formatter.write_str("revision object size invalid"),
            Self::DirectoryCreate(error) => write!(formatter, "revision directory create failed: {error}"),
            Self::ObjectInspect(error) => write!(formatter, "revision object inspect failed: {error}"),
            Self::ObjectInvalid => formatter.write_str("revision object invalid"),
            Self::ObjectRead(error) => write!(formatter, "revision object read failed: {error}"),
            Self::ReadbackMismatch => formatter.write_str("revision object readback mismatch"),
            Self::Create(error) => write!(formatter, "revision object create failed: {error}"),
            Self::Write(error) => write!(formatter, "revision object write failed: {error}"),
            Self::PublishOutcomeUnknown(error) => write!(formatter, "revision object publish outcome unknown: {error}"),
            Self::PublishPlatformOutcomeUnknown(error) => write!(formatter, "revision object publish durability unknown: {error}"),
            Self::ImmutableConflict => formatter.write_str("revision object immutable conflict"),
            Self::IdentityChanged => formatter.write_str("revision object identity changed"),
            Self::Cleanup(error) => write!(formatter, "revision object cleanup failed: {error}"),
        }
    }
}

impl<E> std::error::Error for LegacyRevisionObjectError<E>
where
    E: fmt::Debug + fmt::Display,
{
}

/// Reads one complete immutable revision object through one opened file.
///
/// The parent, locator, file length, modification time and native identity are
/// checked before and after the bounded read. Empty objects are valid.
///
/// # Errors
///
/// Returns a typed failure for an invalid locator, oversize object, unstable
/// readback, platform validation failure or I/O error.
pub fn read_legacy_revision_object<P>(
    platform: &P,
    path: &Path,
    maximum_bytes: usize,
) -> Result<LegacyRevisionObjectRead<P::Identity>, LegacyRevisionObjectError<P::Error>>
where
    P: LegacyRevisionObjectPlatform,
{
    read_object(platform, path, maximum_bytes, None)
}

/// Publishes one exact immutable legacy revision object without replacement.
///
/// `directory` is the already derived revision shard. The package creates it
/// when absent after validating its parent. `final_name` and `temporary_name`
/// must be local basenames. A racing final object is reused only after exact
/// byte readback; a different object is never overwritten.
///
/// # Errors
///
/// Returns a typed failure for invalid names/bounds, directory or I/O failure,
/// immutable conflict, identity replacement, uncertain publication or cleanup.
pub fn publish_legacy_revision_object<P>(
    platform: &P,
    directory: &Path,
    final_name: &str,
    temporary_name: &str,
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<LegacyRevisionObjectReceipt<P::Identity>, LegacyRevisionObjectError<P::Error>>
where
    P: LegacyRevisionObjectPlatform,
{
    if bytes.len() > maximum_bytes {
        return Err(LegacyRevisionObjectError::SizeInvalid);
    }
    if !valid_local_name(final_name)
        || !valid_local_name(temporary_name)
        || final_name == temporary_name
        || !temporary_name.starts_with('.')
        || !temporary_name.ends_with(".tmp")
    {
        return Err(LegacyRevisionObjectError::LocalNameInvalid);
    }

    ensure_child_directory(platform, directory)?;
    let final_path = directory.join(final_name);
    let temporary_path = directory.join(temporary_name);

    match fs::symlink_metadata(&final_path) {
        Ok(_) => {
            let existing = read_object(platform, &final_path, maximum_bytes, None)?;
            if existing.bytes() != bytes {
                return Err(LegacyRevisionObjectError::ImmutableConflict);
            }
            let encoded_bytes = existing.encoded_bytes();
            let identity = existing.identity;
            return Ok(LegacyRevisionObjectReceipt {
                identity,
                encoded_bytes,
                reused: true,
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(LegacyRevisionObjectError::ObjectInspect(error)),
    }

    let mut temporary = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(LegacyRevisionObjectError::Create)?;
    let metadata = temporary
        .metadata()
        .map_err(LegacyRevisionObjectError::Create)?;
    if !regular(&metadata) {
        return Err(LegacyRevisionObjectError::ObjectInvalid);
    }
    platform
        .verify_locator(&temporary, &temporary_path)
        .map_err(LegacyRevisionObjectError::Platform)?;
    let temporary_identity = platform
        .identity(&temporary)
        .map_err(LegacyRevisionObjectError::Platform)?;

    if let Err(error) = temporary.write_all(bytes).and_then(|()| temporary.sync_all()) {
        drop(temporary);
        let _ = cleanup_exact(platform, &temporary_path, &temporary_identity);
        return Err(LegacyRevisionObjectError::Write(error));
    }
    drop(temporary);

    let staged = read_object(
        platform,
        &temporary_path,
        maximum_bytes,
        Some(&temporary_identity),
    )?;
    if staged.bytes() != bytes {
        return Err(LegacyRevisionObjectError::ReadbackMismatch);
    }

    let reused = match fs::hard_link(&temporary_path, &final_path) {
        Ok(()) => false,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => true,
        Err(error) => {
            let _ = cleanup_exact(platform, &temporary_path, &temporary_identity);
            return Err(LegacyRevisionObjectError::PublishOutcomeUnknown(error));
        }
    };
    if let Err(error) = platform.sync_directory(directory) {
        return Err(LegacyRevisionObjectError::PublishPlatformOutcomeUnknown(error));
    }

    let final_object = read_object(platform, &final_path, maximum_bytes, None)?;
    if final_object.bytes() != bytes {
        return Err(LegacyRevisionObjectError::ImmutableConflict);
    }
    if !reused && final_object.identity() != &temporary_identity {
        return Err(LegacyRevisionObjectError::IdentityChanged);
    }
    let encoded_bytes = final_object.encoded_bytes();
    let identity = final_object.identity;

    cleanup_exact(platform, &temporary_path, &temporary_identity)?;
    platform
        .sync_directory(directory)
        .map_err(LegacyRevisionObjectError::PublishPlatformOutcomeUnknown)?;
    Ok(LegacyRevisionObjectReceipt {
        identity,
        encoded_bytes,
        reused,
    })
}

fn read_object<P>(
    platform: &P,
    path: &Path,
    maximum_bytes: usize,
    expected_identity: Option<&P::Identity>,
) -> Result<LegacyRevisionObjectRead<P::Identity>, LegacyRevisionObjectError<P::Error>>
where
    P: LegacyRevisionObjectPlatform,
{
    let parent = path
        .parent()
        .ok_or(LegacyRevisionObjectError::ParentMissing)?;
    platform
        .validate_directory(parent)
        .map_err(LegacyRevisionObjectError::Platform)?;
    let before = fs::symlink_metadata(path)
        .map_err(LegacyRevisionObjectError::ObjectInspect)?;
    let before_len = usize::try_from(before.len())
        .map_err(|_| LegacyRevisionObjectError::SizeInvalid)?;
    if !regular(&before) || before_len > maximum_bytes {
        return Err(if before_len > maximum_bytes {
            LegacyRevisionObjectError::SizeInvalid
        } else {
            LegacyRevisionObjectError::ObjectInvalid
        });
    }

    let mut file = File::open(path).map_err(LegacyRevisionObjectError::ObjectRead)?;
    let opened = file
        .metadata()
        .map_err(LegacyRevisionObjectError::ObjectRead)?;
    if !regular(&opened)
        || opened.len() != before.len()
        || opened.modified().ok() != before.modified().ok()
    {
        return Err(LegacyRevisionObjectError::ReadbackMismatch);
    }
    platform
        .verify_locator(&file, path)
        .map_err(LegacyRevisionObjectError::Platform)?;
    let identity = platform
        .identity(&file)
        .map_err(LegacyRevisionObjectError::Platform)?;
    if expected_identity.is_some_and(|expected| expected != &identity) {
        return Err(LegacyRevisionObjectError::IdentityChanged);
    }

    let mut bytes = Vec::with_capacity(before_len);
    let limit = u64::try_from(maximum_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    (&mut file)
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(LegacyRevisionObjectError::ObjectRead)?;
    let after = file
        .metadata()
        .map_err(LegacyRevisionObjectError::ObjectRead)?;
    if bytes.len() != before_len
        || bytes.len() > maximum_bytes
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err(LegacyRevisionObjectError::ReadbackMismatch);
    }
    platform
        .verify_locator(&file, path)
        .map_err(LegacyRevisionObjectError::Platform)?;
    let after_identity = platform
        .identity(&file)
        .map_err(LegacyRevisionObjectError::Platform)?;
    if after_identity != identity {
        return Err(LegacyRevisionObjectError::IdentityChanged);
    }
    Ok(LegacyRevisionObjectRead { bytes, identity })
}

fn ensure_child_directory<P>(
    platform: &P,
    directory: &Path,
) -> Result<(), LegacyRevisionObjectError<P::Error>>
where
    P: LegacyRevisionObjectPlatform,
{
    let parent = directory
        .parent()
        .ok_or(LegacyRevisionObjectError::ParentMissing)?;
    platform
        .validate_directory(parent)
        .map_err(LegacyRevisionObjectError::Platform)?;
    match fs::symlink_metadata(directory) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match fs::create_dir(directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(LegacyRevisionObjectError::DirectoryCreate(error)),
            }
            platform
                .sync_directory(parent)
                .map_err(LegacyRevisionObjectError::Platform)?;
        }
        Err(error) => return Err(LegacyRevisionObjectError::ObjectInspect(error)),
    }
    platform
        .validate_directory(directory)
        .map_err(LegacyRevisionObjectError::Platform)
}

fn cleanup_exact<P>(
    platform: &P,
    path: &Path,
    expected_identity: &P::Identity,
) -> Result<(), LegacyRevisionObjectError<P::Error>>
where
    P: LegacyRevisionObjectPlatform,
{
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(LegacyRevisionObjectError::Cleanup(error)),
        Ok(metadata) if !regular(&metadata) => {
            return Err(LegacyRevisionObjectError::IdentityChanged);
        }
        Ok(_) => {}
    }
    let file = File::open(path).map_err(LegacyRevisionObjectError::Cleanup)?;
    platform
        .verify_locator(&file, path)
        .map_err(LegacyRevisionObjectError::Platform)?;
    let identity = platform
        .identity(&file)
        .map_err(LegacyRevisionObjectError::Platform)?;
    if &identity != expected_identity {
        return Err(LegacyRevisionObjectError::IdentityChanged);
    }
    drop(file);
    fs::remove_file(path).map_err(LegacyRevisionObjectError::Cleanup)
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
