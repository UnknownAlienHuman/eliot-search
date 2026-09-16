//! Package-owned lifecycle for immutable source-import record artifacts.
//!
//! Mapping plans and source-content manifests are newline-delimited,
//! content-free metadata artifacts authenticated by [`SourceImportRecordChain`].
//! This module owns temporary-file creation, exact second-pass comparison,
//! no-clobber hard-link publication, final record-chain readback and safe
//! temporary cleanup. The injected platform owns only native identity,
//! locator admission and directory durability observations.

#![allow(clippy::module_name_repetitions)]

use core::fmt;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use super::output_artifact::SourceImportOutputArtifactPlatform;
use super::{
    MAX_SOURCE_IMPORT_RECORD_BYTES, MAX_SOURCE_IMPORT_ROW_BYTES,
    SourceImportRecordChain, SourceImportRecordChainError,
};

/// Closed immutable-record-artifact failure.
#[derive(Debug)]
pub enum SourceImportRecordArtifactError<E> {
    /// Injected platform observation failed.
    Platform(E),
    /// The cooperative operation deadline elapsed.
    DeadlineExceeded,
    /// A supplied temporary basename was not the bounded local staging form.
    TemporaryNameInvalid,
    /// A supplied final basename was not one plain local filename.
    FinalNameInvalid,
    /// The temporary artifact could not be created without clobbering.
    CreateFailed,
    /// The temporary writer had already been consumed.
    Closed,
    /// Writing or flushing one record failed.
    WriteFailed,
    /// Flushing the completed temporary file to storage failed.
    SyncFailed,
    /// A temporary/final locator was not an admitted regular file of exact size.
    ObjectInvalid,
    /// Full record-chain fingerprint readback failed.
    ReadFailed,
    /// Exact second-pass row comparison could not read the expected bytes.
    ComparisonReadFailed,
    /// Exact second-pass rows, length, identity or final EOF did not match.
    ReadbackMismatch,
    /// Hard-link publication may have produced an externally visible effect.
    PublishOutcomeUnknown,
    /// An existing final artifact has a different exact record chain.
    ImmutableConflict,
    /// The opened temporary/final object changed native identity.
    IdentityChanged,
    /// The verified temporary locator could not be removed.
    CleanupFailed,
    /// The canonical record-chain rejected a row or aggregate bound.
    RecordChain(SourceImportRecordChainError),
}

impl<E: fmt::Display> fmt::Display for SourceImportRecordArtifactError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform(error) => error.fmt(formatter),
            Self::DeadlineExceeded => {
                formatter.write_str("record artifact deadline exceeded")
            }
            Self::TemporaryNameInvalid => {
                formatter.write_str("record artifact temporary name invalid")
            }
            Self::FinalNameInvalid => {
                formatter.write_str("record artifact final name invalid")
            }
            Self::CreateFailed => {
                formatter.write_str("record artifact create failed")
            }
            Self::Closed => formatter.write_str("record artifact closed"),
            Self::WriteFailed => {
                formatter.write_str("record artifact write failed")
            }
            Self::SyncFailed => {
                formatter.write_str("record artifact sync failed")
            }
            Self::ObjectInvalid => {
                formatter.write_str("record artifact object invalid")
            }
            Self::ReadFailed => {
                formatter.write_str("record artifact read failed")
            }
            Self::ComparisonReadFailed => {
                formatter.write_str("record artifact comparison read failed")
            }
            Self::ReadbackMismatch => {
                formatter.write_str("record artifact readback mismatch")
            }
            Self::PublishOutcomeUnknown => {
                formatter.write_str("record artifact publish outcome unknown")
            }
            Self::ImmutableConflict => {
                formatter.write_str("record artifact immutable conflict")
            }
            Self::IdentityChanged => {
                formatter.write_str("record artifact identity changed")
            }
            Self::CleanupFailed => {
                formatter.write_str("record artifact cleanup failed")
            }
            Self::RecordChain(error) => error.fmt(formatter),
        }
    }
}

impl<E> From<SourceImportRecordChainError> for SourceImportRecordArtifactError<E> {
    fn from(error: SourceImportRecordChainError) -> Self {
        Self::RecordChain(error)
    }
}

/// Stable observation of one complete record-chain artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceImportRecordArtifactObservation<I> {
    chain: [u8; 32],
    encoded_bytes: u64,
    identity: I,
}

impl<I> SourceImportRecordArtifactObservation<I> {
    /// Exact frozen record-chain digest.
    #[must_use]
    pub const fn chain(&self) -> &[u8; 32] {
        &self.chain
    }

    /// Exact encoded byte count.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    /// Platform-native identity observed after complete readback.
    #[must_use]
    pub const fn identity(&self) -> &I {
        &self.identity
    }
}

/// Receipt for one verified no-clobber publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceImportPublishedRecordArtifact {
    name: String,
    chain: [u8; 32],
    encoded_bytes: u64,
    reused: bool,
}

impl SourceImportPublishedRecordArtifact {
    /// Final local artifact basename.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact record-chain digest.
    #[must_use]
    pub const fn chain(&self) -> &[u8; 32] {
        &self.chain
    }

    /// Exact encoded byte count.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    /// Whether an existing matching final artifact was reused.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.reused
    }
}

struct OwnedStaging<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    platform: P,
    directory: PathBuf,
    path: PathBuf,
    identity: P::Identity,
    cleanup_armed: bool,
}

impl<P> OwnedStaging<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    fn cleanup(
        &mut self,
    ) -> Result<(), SourceImportRecordArtifactError<P::Error>> {
        if !core::mem::replace(&mut self.cleanup_armed, false) {
            return Ok(());
        }
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(_) => return Err(SourceImportRecordArtifactError::IdentityChanged),
        };
        if !regular(&metadata) {
            return Err(SourceImportRecordArtifactError::IdentityChanged);
        }
        let file = File::open(&self.path)
            .map_err(|_| SourceImportRecordArtifactError::IdentityChanged)?;
        self.platform
            .verify_locator(&file, &self.path)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        let identity = self
            .platform
            .identity(&file)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        if identity != self.identity {
            return Err(SourceImportRecordArtifactError::IdentityChanged);
        }
        drop(file);
        fs::remove_file(&self.path)
            .map_err(|_| SourceImportRecordArtifactError::CleanupFailed)
    }
}

impl<P> Drop for OwnedStaging<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Package-owned writer for one temporary immutable record artifact.
pub struct SourceImportRecordArtifact<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    writer: Option<BufWriter<File>>,
    owned: Option<OwnedStaging<P>>,
    chain: SourceImportRecordChain,
}

impl<P> SourceImportRecordArtifact<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    /// Creates one no-clobber temporary artifact under an admitted directory.
    ///
    /// `temporary_name` must be one local `.source-map.*.tmp` basename. Its
    /// uniqueness token is supplied by the integration adapter and is not
    /// evidence, authority or part of the final artifact identity.
    ///
    /// # Errors
    ///
    /// Returns a typed failure for an invalid name, unavailable directory,
    /// existing temporary locator, platform observation or elapsed deadline.
    pub fn create(
        directory: &Path,
        temporary_name: &str,
        platform: P,
        deadline: Instant,
    ) -> Result<Self, SourceImportRecordArtifactError<P::Error>> {
        check_deadline(deadline)?;
        if !valid_temporary_name(temporary_name) {
            return Err(SourceImportRecordArtifactError::TemporaryNameInvalid);
        }
        platform
            .validate_directory(directory)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        let path = directory.join(temporary_name);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| SourceImportRecordArtifactError::CreateFailed)?;
        platform
            .verify_locator(&file, &path)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        let identity = platform
            .identity(&file)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        Ok(Self {
            writer: Some(BufWriter::new(file)),
            owned: Some(OwnedStaging {
                platform,
                directory: directory.to_owned(),
                path,
                identity,
                cleanup_armed: true,
            }),
            chain: SourceImportRecordChain::new(),
        })
    }

    /// Appends one canonical newline-terminated record.
    ///
    /// # Errors
    ///
    /// Returns a record-chain, write, closed-state or deadline failure.
    pub fn push(
        &mut self,
        row: &[u8],
        deadline: Instant,
    ) -> Result<(), SourceImportRecordArtifactError<P::Error>> {
        check_deadline(deadline)?;
        self.chain.push(row)?;
        self.writer
            .as_mut()
            .ok_or(SourceImportRecordArtifactError::Closed)?
            .write_all(row)
            .map_err(|_| SourceImportRecordArtifactError::WriteFailed)?;
        check_deadline(deadline)
    }

    /// Flushes, syncs and closes the temporary writer before exact readback.
    ///
    /// # Errors
    ///
    /// Returns a write/sync/platform/identity/deadline failure.
    pub fn freeze(
        mut self,
        deadline: Instant,
    ) -> Result<SourceImportFrozenRecordArtifact<P>, SourceImportRecordArtifactError<P::Error>> {
        check_deadline(deadline)?;
        let mut writer = self
            .writer
            .take()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        writer
            .flush()
            .map_err(|_| SourceImportRecordArtifactError::WriteFailed)?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|_| SourceImportRecordArtifactError::SyncFailed)?;
        let owned = self
            .owned
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        owned
            .platform
            .verify_locator(writer.get_ref(), &owned.path)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        let identity = owned
            .platform
            .identity(writer.get_ref())
            .map_err(SourceImportRecordArtifactError::Platform)?;
        if identity != owned.identity {
            return Err(SourceImportRecordArtifactError::IdentityChanged);
        }
        drop(writer);
        check_deadline(deadline)?;
        let chain = core::mem::take(&mut self.chain);
        let encoded_bytes = chain.encoded_bytes();
        let digest = chain.finish();
        let owned = self
            .owned
            .take()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        Ok(SourceImportFrozenRecordArtifact {
            owned: Some(owned),
            chain: digest,
            encoded_bytes,
        })
    }
}

/// Closed temporary artifact awaiting an exact second source replay.
pub struct SourceImportFrozenRecordArtifact<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    owned: Option<OwnedStaging<P>>,
    chain: [u8; 32],
    encoded_bytes: u64,
}

impl<P> SourceImportFrozenRecordArtifact<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    /// Exact first-pass record-chain digest.
    #[must_use]
    pub const fn chain(&self) -> &[u8; 32] {
        &self.chain
    }

    /// Exact first-pass encoded byte count.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    /// Opens a second-pass comparator pinned to the original temporary object.
    ///
    /// # Errors
    ///
    /// Returns an object/platform/identity/deadline failure.
    pub fn begin_readback(
        self,
        deadline: Instant,
    ) -> Result<SourceImportRecordReadback<P>, SourceImportRecordArtifactError<P::Error>> {
        let owned = self
            .owned
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let opened = open_record_file(
            &owned.platform,
            &owned.path,
            self.encoded_bytes,
            Some(&owned.identity),
            deadline,
        )?;
        Ok(SourceImportRecordReadback {
            reader: Some(BufReader::new(opened.file)),
            frozen: Some(self),
            before_modified: opened.modified,
            observed: SourceImportRecordChain::new(),
            buffer: [0_u8; MAX_SOURCE_IMPORT_ROW_BYTES],
        })
    }
}

/// Exact second-pass comparator over the original temporary artifact.
pub struct SourceImportRecordReadback<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    reader: Option<BufReader<File>>,
    frozen: Option<SourceImportFrozenRecordArtifact<P>>,
    before_modified: Option<SystemTime>,
    observed: SourceImportRecordChain,
    buffer: [u8; MAX_SOURCE_IMPORT_ROW_BYTES],
}

impl<P> SourceImportRecordReadback<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    /// Compares one regenerated record with the next exact persisted bytes.
    ///
    /// # Errors
    ///
    /// Returns a record-chain, comparison-read, mismatch, closed-state or
    /// deadline failure.
    pub fn compare(
        &mut self,
        row: &[u8],
        deadline: Instant,
    ) -> Result<(), SourceImportRecordArtifactError<P::Error>> {
        check_deadline(deadline)?;
        self.observed.push(row)?;
        let reader = self
            .reader
            .as_mut()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        reader
            .read_exact(&mut self.buffer[..row.len()])
            .map_err(|_| SourceImportRecordArtifactError::ComparisonReadFailed)?;
        if self.buffer[..row.len()] != *row {
            return Err(SourceImportRecordArtifactError::ReadbackMismatch);
        }
        check_deadline(deadline)
    }

    /// Requires exact EOF, chain, byte count, metadata and native identity.
    ///
    /// # Errors
    ///
    /// Returns a comparison-read, mismatch, platform, identity, closed-state or
    /// deadline failure.
    pub fn finish(
        mut self,
        deadline: Instant,
    ) -> Result<SourceImportVerifiedRecordArtifact<P>, SourceImportRecordArtifactError<P::Error>> {
        check_deadline(deadline)?;
        let mut reader = self
            .reader
            .take()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let mut extra = [0_u8; 1];
        if reader
            .read(&mut extra)
            .map_err(|_| SourceImportRecordArtifactError::ComparisonReadFailed)?
            != 0
        {
            return Err(SourceImportRecordArtifactError::ReadbackMismatch);
        }
        let frozen = self
            .frozen
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let observed = core::mem::take(&mut self.observed);
        if observed.encoded_bytes() != frozen.encoded_bytes
            || observed.finish() != frozen.chain
        {
            return Err(SourceImportRecordArtifactError::ReadbackMismatch);
        }
        let owned = frozen
            .owned
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let after = reader
            .get_ref()
            .metadata()
            .map_err(|_| SourceImportRecordArtifactError::ReadbackMismatch)?;
        if after.len() != frozen.encoded_bytes
            || after.modified().ok() != self.before_modified
        {
            return Err(SourceImportRecordArtifactError::ReadbackMismatch);
        }
        owned
            .platform
            .verify_locator(reader.get_ref(), &owned.path)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        let identity = owned
            .platform
            .identity(reader.get_ref())
            .map_err(SourceImportRecordArtifactError::Platform)?;
        if identity != owned.identity {
            return Err(SourceImportRecordArtifactError::IdentityChanged);
        }
        check_deadline(deadline)?;
        drop(reader);
        let frozen = self
            .frozen
            .take()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        Ok(SourceImportVerifiedRecordArtifact {
            frozen: Some(frozen),
        })
    }
}

/// Temporary artifact proven equal to a complete second source replay.
pub struct SourceImportVerifiedRecordArtifact<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    frozen: Option<SourceImportFrozenRecordArtifact<P>>,
}

impl<P> SourceImportVerifiedRecordArtifact<P>
where
    P: SourceImportOutputArtifactPlatform,
{
    /// Publishes by no-clobber hard link, verifies the final record chain and
    /// removes only the original temporary object.
    ///
    /// # Errors
    ///
    /// Returns a name, object, read, record-chain, immutable-conflict,
    /// identity, cleanup, platform or deadline failure. An unclassified
    /// hard-link error remains outcome-unknown.
    pub fn publish(
        mut self,
        final_name: &str,
        deadline: Instant,
    ) -> Result<SourceImportPublishedRecordArtifact, SourceImportRecordArtifactError<P::Error>> {
        check_deadline(deadline)?;
        if !valid_local_name(final_name) {
            return Err(SourceImportRecordArtifactError::FinalNameInvalid);
        }
        let frozen = self
            .frozen
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let owned = frozen
            .owned
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let staging = inspect_record_file(
            &owned.platform,
            &owned.path,
            frozen.encoded_bytes,
            Some(&owned.identity),
            deadline,
        )?;
        if staging.chain != frozen.chain {
            return Err(SourceImportRecordArtifactError::ReadbackMismatch);
        }

        let final_path = owned.directory.join(final_name);
        let reused = match fs::hard_link(&owned.path, &final_path) {
            Ok(()) => false,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
            Err(_) => {
                return Err(SourceImportRecordArtifactError::PublishOutcomeUnknown);
            }
        };
        owned
            .platform
            .sync_directory(&owned.directory)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        let published = inspect_record_file(
            &owned.platform,
            &final_path,
            frozen.encoded_bytes,
            None,
            deadline,
        )?;
        if published.chain != frozen.chain {
            return Err(SourceImportRecordArtifactError::ImmutableConflict);
        }
        if !reused && published.identity != owned.identity {
            return Err(SourceImportRecordArtifactError::IdentityChanged);
        }

        let chain = frozen.chain;
        let encoded_bytes = frozen.encoded_bytes;
        let mut frozen = self
            .frozen
            .take()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let mut owned = frozen
            .owned
            .take()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        owned.cleanup()?;
        owned
            .platform
            .sync_directory(&owned.directory)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        check_deadline(deadline)?;
        Ok(SourceImportPublishedRecordArtifact {
            name: final_name.to_owned(),
            chain,
            encoded_bytes,
            reused,
        })
    }
}

/// Reads one complete immutable source-import record artifact and returns its
/// exact chain, byte count and stable native identity.
///
/// # Errors
///
/// Returns a typed failure for invalid size/object shape, read or record-chain
/// errors, platform/identity instability or an elapsed deadline.
pub fn inspect_source_import_record_artifact<P>(
    platform: &P,
    path: &Path,
    expected_bytes: u64,
    deadline: Instant,
) -> Result<SourceImportRecordArtifactObservation<P::Identity>, SourceImportRecordArtifactError<P::Error>>
where
    P: SourceImportOutputArtifactPlatform,
{
    inspect_record_file(platform, path, expected_bytes, None, deadline)
}

struct OpenedRecord {
    file: File,
    modified: Option<SystemTime>,
}

fn open_record_file<P>(
    platform: &P,
    path: &Path,
    expected_bytes: u64,
    expected_identity: Option<&P::Identity>,
    deadline: Instant,
) -> Result<OpenedRecord, SourceImportRecordArtifactError<P::Error>>
where
    P: SourceImportOutputArtifactPlatform,
{
    check_deadline(deadline)?;
    if expected_bytes > MAX_SOURCE_IMPORT_RECORD_BYTES {
        return Err(SourceImportRecordArtifactError::ObjectInvalid);
    }
    let parent = path
        .parent()
        .ok_or(SourceImportRecordArtifactError::ObjectInvalid)?;
    platform
        .validate_directory(parent)
        .map_err(SourceImportRecordArtifactError::Platform)?;
    let before = fs::symlink_metadata(path)
        .map_err(|_| SourceImportRecordArtifactError::ObjectInvalid)?;
    if !regular(&before) || before.len() != expected_bytes {
        return Err(SourceImportRecordArtifactError::ObjectInvalid);
    }
    let file = File::open(path)
        .map_err(|_| SourceImportRecordArtifactError::ObjectInvalid)?;
    let opened = file
        .metadata()
        .map_err(|_| SourceImportRecordArtifactError::ObjectInvalid)?;
    if !opened.is_file()
        || opened.len() != expected_bytes
        || opened.modified().ok() != before.modified().ok()
    {
        return Err(SourceImportRecordArtifactError::ObjectInvalid);
    }
    platform
        .verify_locator(&file, path)
        .map_err(SourceImportRecordArtifactError::Platform)?;
    let identity = platform
        .identity(&file)
        .map_err(SourceImportRecordArtifactError::Platform)?;
    if expected_identity.is_some_and(|expected| expected != &identity) {
        return Err(SourceImportRecordArtifactError::IdentityChanged);
    }
    check_deadline(deadline)?;
    Ok(OpenedRecord {
        file,
        modified: opened.modified().ok(),
    })
}

fn inspect_record_file<P>(
    platform: &P,
    path: &Path,
    expected_bytes: u64,
    expected_identity: Option<&P::Identity>,
    deadline: Instant,
) -> Result<SourceImportRecordArtifactObservation<P::Identity>, SourceImportRecordArtifactError<P::Error>>
where
    P: SourceImportOutputArtifactPlatform,
{
    let opened = open_record_file(
        platform,
        path,
        expected_bytes,
        expected_identity,
        deadline,
    )?;
    let before_modified = opened.modified;
    let mut reader = BufReader::new(opened.file);
    let mut row = Vec::new();
    let mut chain = SourceImportRecordChain::new();
    loop {
        check_deadline(deadline)?;
        row.clear();
        let read = Read::take(
            &mut reader,
            u64::try_from(MAX_SOURCE_IMPORT_ROW_BYTES)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_until(b'\n', &mut row)
        .map_err(|_| SourceImportRecordArtifactError::ReadFailed)?;
        if read == 0 {
            break;
        }
        chain.push(&row)?;
    }
    let after = reader
        .get_ref()
        .metadata()
        .map_err(|_| SourceImportRecordArtifactError::ReadFailed)?;
    if chain.encoded_bytes() != expected_bytes
        || after.len() != expected_bytes
        || after.modified().ok() != before_modified
    {
        return Err(SourceImportRecordArtifactError::ReadbackMismatch);
    }
    platform
        .verify_locator(reader.get_ref(), path)
        .map_err(SourceImportRecordArtifactError::Platform)?;
    let identity = platform
        .identity(reader.get_ref())
        .map_err(SourceImportRecordArtifactError::Platform)?;
    if expected_identity.is_some_and(|expected| expected != &identity) {
        return Err(SourceImportRecordArtifactError::IdentityChanged);
    }
    check_deadline(deadline)?;
    Ok(SourceImportRecordArtifactObservation {
        chain: chain.finish(),
        encoded_bytes: expected_bytes,
        identity,
    })
}

fn valid_temporary_name(name: &str) -> bool {
    valid_local_name(name)
        && name.starts_with(".source-map.")
        && name.ends_with(".tmp")
}

fn valid_local_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && Path::new(name).file_name() == Some(OsStr::new(name))
}

fn regular(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink()
}

fn check_deadline<E>(
    deadline: Instant,
) -> Result<(), SourceImportRecordArtifactError<E>> {
    if Instant::now() >= deadline {
        Err(SourceImportRecordArtifactError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
