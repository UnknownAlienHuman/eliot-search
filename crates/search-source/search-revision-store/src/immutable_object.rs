//! Qualified filesystem lifecycle for legacy immutable revision objects.
//!
//! This adapter owns bounded exact reads and no-clobber immutable publication
//! for the legacy DIRECT `.bin` / `.dpapi` object layout while Phase 3 moves
//! daemon storage behavior to its package owner. Bytes are opaque here: secret
//! protection, plaintext verification, source identity and catalog authority
//! remain with their existing owners.

#![allow(clippy::module_name_repetitions)]

use core::fmt;
use core::mem;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

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
    bytes: Zeroizing<Vec<u8>>,
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
        // `identity` is moved out before the buffer, so the guard is the only
        // thing this call still owns when it wipes and then hands over the
        // still-charged reserve. Nothing fallible happens between the take and
        // the return.
        let LegacyRevisionObjectRead { mut bytes, identity } = self;
        drop(identity);
        mem::take(&mut *bytes)
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
            Self::DirectoryCreate(error) => {
                write!(formatter, "revision directory create failed: {error}")
            }
            Self::ObjectInspect(error) => {
                write!(formatter, "revision object inspect failed: {error}")
            }
            Self::ObjectInvalid => formatter.write_str("revision object invalid"),
            Self::ObjectRead(error) => write!(formatter, "revision object read failed: {error}"),
            Self::ReadbackMismatch => formatter.write_str("revision object readback mismatch"),
            Self::Create(error) => write!(formatter, "revision object create failed: {error}"),
            Self::Write(error) => write!(formatter, "revision object write failed: {error}"),
            Self::PublishOutcomeUnknown(error) => write!(
                formatter,
                "revision object publish outcome unknown: {error}"
            ),
            Self::PublishPlatformOutcomeUnknown(error) => write!(
                formatter,
                "revision object publish durability unknown: {error}"
            ),
            Self::ImmutableConflict => formatter.write_str("revision object immutable conflict"),
            Self::IdentityChanged => formatter.write_str("revision object identity changed"),
            Self::Cleanup(error) => write!(formatter, "revision object cleanup failed: {error}"),
        }
    }
}

impl<E> std::error::Error for LegacyRevisionObjectError<E> where E: fmt::Debug + fmt::Display {}

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
    context::legacy_read(platform, path, maximum_bytes)
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
    context::legacy_publish(
        platform,
        directory,
        final_name,
        temporary_name,
        bytes,
        maximum_bytes,
    )
}

struct TemporaryObjectGuard<'a, P>
where
    P: LegacyRevisionObjectPlatform,
{
    platform: &'a P,
    path: PathBuf,
    identity: P::Identity,
    cleanup_armed: bool,
}

impl<P> TemporaryObjectGuard<'_, P>
where
    P: LegacyRevisionObjectPlatform,
{
    fn cleanup(&mut self) -> Result<(), LegacyRevisionObjectError<P::Error>> {
        if !core::mem::replace(&mut self.cleanup_armed, false) {
            return Ok(());
        }
        cleanup_exact(self.platform, &self.path, &self.identity)
    }
}

impl<P> Drop for TemporaryObjectGuard<'_, P>
where
    P: LegacyRevisionObjectPlatform,
{
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
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

#[path = "immutable_object/context.rs"]
mod context;
pub use context::*;

#[cfg(test)]
mod tests;
