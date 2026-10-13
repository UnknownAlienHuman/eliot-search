//! Package-owned lifecycle for immutable source-import record artifacts.
//!
//! Mapping plans and source-content manifests are newline-delimited,
//! content-free metadata artifacts authenticated by [`SourceImportRecordChain`].
//! This module owns temporary-file creation, exact second-pass comparison,
//! one-shot no-clobber publication, retained unknown evidence and readback.
//! Original-object cleanup is deferred to #347. The platform owns native identity,
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
    MAX_SOURCE_IMPORT_RECORD_BYTES, MAX_SOURCE_IMPORT_ROW_BYTES, SourceImportRecordChain,
    SourceImportRecordChainError,
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
            Self::DeadlineExceeded => formatter.write_str("record artifact deadline exceeded"),
            Self::TemporaryNameInvalid => {
                formatter.write_str("record artifact temporary name invalid")
            }
            Self::FinalNameInvalid => formatter.write_str("record artifact final name invalid"),
            Self::CreateFailed => formatter.write_str("record artifact create failed"),
            Self::Closed => formatter.write_str("record artifact closed"),
            Self::WriteFailed => formatter.write_str("record artifact write failed"),
            Self::SyncFailed => formatter.write_str("record artifact sync failed"),
            Self::ObjectInvalid => formatter.write_str("record artifact object invalid"),
            Self::ReadFailed => formatter.write_str("record artifact read failed"),
            Self::ComparisonReadFailed => {
                formatter.write_str("record artifact comparison read failed")
            }
            Self::ReadbackMismatch => formatter.write_str("record artifact readback mismatch"),
            Self::PublishOutcomeUnknown => {
                formatter.write_str("record artifact publish outcome unknown")
            }
            Self::ImmutableConflict => formatter.write_str("record artifact immutable conflict"),
            Self::IdentityChanged => formatter.write_str("record artifact identity changed"),
            Self::CleanupFailed => formatter.write_str("record artifact cleanup failed"),
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
    rows: u64,
    encoded_bytes: u64,
    identity: I,
}

impl<I> SourceImportRecordArtifactObservation<I> {
    /// Exact number of complete records.
    #[must_use]
    pub const fn rows(&self) -> u64 {
        self.rows
    }

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
    /// Original-object deletion is not qualified by the current native owner.
    #[must_use]
    pub const fn cleanup_disposition(&self) -> SourceImportRecordArtifactCleanupDisposition {
        SourceImportRecordArtifactCleanupDisposition::DeferredTo347
    }

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
    original_file: File,
}

/// Required record-specific durability observation, without a default no-op.
pub trait SourceImportRecordArtifactPlatform: SourceImportOutputArtifactPlatform {
    /// Flushes the admitted directory after a possible record publication.
    /// # Errors
    /// Unsupported or failed synchronization must fail closed.
    fn sync_record_artifact_directory(&self, directory: &Path) -> Result<(), Self::Error>;
}

/// No path-based deletion is accepted as original-object cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceImportRecordArtifactCleanupDisposition {
    /// Retain the original staging locator until native cleanup is qualified.
    DeferredTo347,
}

/// Process-local publication disposition; no variant grants restart authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceImportRecordArtifactPublicationDisposition {
    /// No publication dispatch has occurred.
    Prepared,
    /// Possible effects retain the original object and all locators.
    RetainedUnknown,
    /// Same-process durability and readback completed; staging remains retained.
    VerifiedCleanupDeferred,
}

/// Bounded current evidence, never a publication or cleanup decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceImportRecordArtifactCurrentState {
    /// Neither locator is present.
    Missing,
    /// Only staging has the expected complete records.
    StageExact,
    /// Only final has the expected complete records.
    FinalExact,
    /// Both exact locators currently name one native object.
    BothExactSameObject,
    /// Both exact locators currently name distinct native objects.
    BothExactDistinctObjects,
    /// A present locator has conflicting records or cardinality.
    Conflict,
    /// A locator/object changed identity or has an inadmissible shape.
    Replaced,
    /// Admission, readback or the bounded deadline was unavailable.
    Unavailable,
}

/// Observations cannot replace #348's durable original-operation linkage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceImportRecordArtifactResolution {
    /// No retry, success or cleanup authority is established.
    Unresolved,
}

/// Content-free, read-only classification; deliberately has no completion API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceImportRecordArtifactInspection {
    state: SourceImportRecordArtifactCurrentState,
}

impl SourceImportRecordArtifactInspection {
    /// Current evidence only.
    #[must_use]
    pub const fn state(&self) -> SourceImportRecordArtifactCurrentState {
        self.state
    }

    /// Always unresolved without the accepted durable owner join in #348.
    #[must_use]
    pub const fn resolution(&self) -> SourceImportRecordArtifactResolution {
        SourceImportRecordArtifactResolution::Unresolved
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
        let original_file = file
            .try_clone()
            .map_err(|_| SourceImportRecordArtifactError::CreateFailed)?;
        platform
            .verify_locator(&original_file, &path)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        let identity = platform
            .identity(&original_file)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        Ok(Self {
            writer: Some(BufWriter::new(file)),
            owned: Some(OwnedStaging {
                platform,
                directory: directory.to_owned(),
                path,
                identity,
                original_file,
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
    ) -> Result<SourceImportFrozenRecordArtifact<P>, SourceImportRecordArtifactError<P::Error>>
    {
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
        let rows = chain.rows();
        let encoded_bytes = chain.encoded_bytes();
        let digest = chain.finish();
        let owned = self
            .owned
            .take()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        Ok(SourceImportFrozenRecordArtifact {
            owned: Some(owned),
            chain: digest,
            rows,
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
    rows: u64,
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
    ) -> Result<SourceImportVerifiedRecordArtifact<P>, SourceImportRecordArtifactError<P::Error>>
    {
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
        if observed.rows() != frozen.rows
            || observed.encoded_bytes() != frozen.encoded_bytes
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
        if after.len() != frozen.encoded_bytes || after.modified().ok() != self.before_modified {
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
    P: SourceImportRecordArtifactPlatform,
{
    /// Captures process-local original inputs before any publication dispatch.
    /// The original file has remained open since creation. Nothing is serialized.
    ///
    /// # Errors
    /// Invalid names, original readback or admission fail before dispatch.
    pub fn prepare_publication(
        mut self,
        final_name: &str,
        deadline: Instant,
    ) -> Result<SourceImportRecordArtifactPublication<P>, SourceImportRecordArtifactError<P::Error>>
    {
        check_deadline(deadline)?;
        if !valid_local_name(final_name) {
            return Err(SourceImportRecordArtifactError::FinalNameInvalid);
        }
        let frozen = self
            .frozen
            .take()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let owned = frozen
            .owned
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        let final_path = owned.directory.join(final_name);
        if final_path == owned.path {
            return Err(SourceImportRecordArtifactError::FinalNameInvalid);
        }
        let publication = SourceImportRecordArtifactPublication {
            frozen,
            final_path,
            final_name: final_name.to_owned(),
            disposition: SourceImportRecordArtifactPublicationDisposition::Prepared,
            reused: false,
        };
        publication.verify_original(deadline)?;
        Ok(publication)
    }

    /// Existing consuming adapter: one attempt, retained staging on every exit.
    /// Dropping this adapter loses process-local proof, never deletes evidence.
    /// # Errors
    /// Every uncertain post-dispatch outcome keeps `PublishOutcomeUnknown`.
    pub fn publish(
        self,
        final_name: &str,
        deadline: Instant,
    ) -> Result<SourceImportPublishedRecordArtifact, SourceImportRecordArtifactError<P::Error>>
    {
        self.prepare_publication(final_name, deadline)?
            .publish_once(deadline)
    }
}

/// One original in-process operation. Private fields cannot be reminted from
/// current paths/identities. Drop closes handles only; no locator is deleted.
pub struct SourceImportRecordArtifactPublication<P>
where
    P: SourceImportRecordArtifactPlatform,
{
    frozen: SourceImportFrozenRecordArtifact<P>,
    final_path: PathBuf,
    final_name: String,
    disposition: SourceImportRecordArtifactPublicationDisposition,
    reused: bool,
}

impl<P> SourceImportRecordArtifactPublication<P>
where
    P: SourceImportRecordArtifactPlatform,
{
    /// Content-free explicit publication disposition.
    #[must_use]
    pub const fn disposition(&self) -> SourceImportRecordArtifactPublicationDisposition {
        self.disposition
    }

    /// Original complete record chain.
    #[must_use]
    pub const fn chain(&self) -> &[u8; 32] {
        &self.frozen.chain
    }

    /// Original row count.
    #[must_use]
    pub const fn rows(&self) -> u64 {
        self.frozen.rows
    }

    /// Original encoded byte count.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.frozen.encoded_bytes
    }

    /// Original local staging basename.
    #[must_use]
    pub fn staging_name(&self) -> &OsStr {
        self.frozen.owned.as_ref().map_or_else(
            || OsStr::new(""),
            |owned| owned.path.file_name().unwrap_or_else(|| OsStr::new("")),
        )
    }

    /// Bound final basename.
    #[must_use]
    pub fn final_name(&self) -> &str {
        &self.final_name
    }

    /// Dispatches at most one link. Failures after dispatch retain original proof.
    /// # Errors
    /// Pre-dispatch failures stay typed; possible effects remain unknown.
    pub fn publish_once(
        &mut self,
        deadline: Instant,
    ) -> Result<SourceImportPublishedRecordArtifact, SourceImportRecordArtifactError<P::Error>>
    {
        self.publish_once_using(deadline, |stage, final_path| {
            fs::hard_link(stage, final_path)
        })
    }

    fn publish_once_using(
        &mut self,
        deadline: Instant,
        link: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<SourceImportPublishedRecordArtifact, SourceImportRecordArtifactError<P::Error>>
    {
        if self.disposition == SourceImportRecordArtifactPublicationDisposition::RetainedUnknown {
            return Err(SourceImportRecordArtifactError::PublishOutcomeUnknown);
        }
        if self.disposition != SourceImportRecordArtifactPublicationDisposition::Prepared {
            return Err(SourceImportRecordArtifactError::Closed);
        }
        self.verify_original(deadline)?;
        let owned = self
            .frozen
            .owned
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        // From here on every exit retains staging. No destructor or retry runs.
        self.disposition = SourceImportRecordArtifactPublicationDisposition::RetainedUnknown;
        self.reused = match link(&owned.path, &self.final_path) {
            Ok(()) => false,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
            Err(_) => return Err(SourceImportRecordArtifactError::PublishOutcomeUnknown),
        };
        self.resolve_same_process(deadline)
    }

    /// Synchronizes and revalidates the retained original object; never links.
    /// This requires the live object created before dispatch, not an inspection.
    /// # Errors
    /// Missing/replaced/conflicting objects, sync or late refusal remain unknown.
    pub fn resolve_same_process(
        &mut self,
        deadline: Instant,
    ) -> Result<SourceImportPublishedRecordArtifact, SourceImportRecordArtifactError<P::Error>>
    {
        if self.disposition != SourceImportRecordArtifactPublicationDisposition::RetainedUnknown {
            return Err(SourceImportRecordArtifactError::Closed);
        }
        self.sync_and_verify_final(deadline)
            .map_err(|_| SourceImportRecordArtifactError::PublishOutcomeUnknown)?;
        self.disposition =
            SourceImportRecordArtifactPublicationDisposition::VerifiedCleanupDeferred;
        Ok(SourceImportPublishedRecordArtifact {
            name: self.final_name.clone(),
            chain: self.frozen.chain,
            encoded_bytes: self.frozen.encoded_bytes,
            reused: self.reused,
        })
    }

    /// Read-only evidence relative to this retained original identity.
    #[must_use]
    pub fn classify_current(&self, deadline: Instant) -> SourceImportRecordArtifactInspection {
        let Some(owned) = self.frozen.owned.as_ref() else {
            return inspection(SourceImportRecordArtifactCurrentState::Unavailable);
        };
        classify_paths(
            &owned.platform,
            &owned.path,
            &self.final_path,
            (
                &self.frozen.chain,
                self.frozen.rows,
                self.frozen.encoded_bytes,
            ),
            Some(&owned.identity),
            deadline,
        )
    }

    fn verify_original(
        &self,
        deadline: Instant,
    ) -> Result<(), SourceImportRecordArtifactError<P::Error>> {
        check_deadline(deadline)?;
        let owned = self
            .frozen
            .owned
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        owned
            .platform
            .verify_locator(&owned.original_file, &owned.path)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        if owned
            .platform
            .identity(&owned.original_file)
            .map_err(SourceImportRecordArtifactError::Platform)?
            != owned.identity
        {
            return Err(SourceImportRecordArtifactError::IdentityChanged);
        }
        let stage = inspect_record_file(
            &owned.platform,
            &owned.path,
            self.frozen.encoded_bytes,
            Some(&owned.identity),
            deadline,
        )?;
        self.verify_records(&stage)
    }

    fn verify_records(
        &self,
        observation: &SourceImportRecordArtifactObservation<P::Identity>,
    ) -> Result<(), SourceImportRecordArtifactError<P::Error>> {
        if observation.chain != self.frozen.chain
            || observation.rows != self.frozen.rows
            || observation.encoded_bytes != self.frozen.encoded_bytes
        {
            return Err(SourceImportRecordArtifactError::ReadbackMismatch);
        }
        Ok(())
    }

    fn sync_and_verify_final(
        &self,
        deadline: Instant,
    ) -> Result<(), SourceImportRecordArtifactError<P::Error>> {
        self.verify_original(deadline)?;
        let owned = self
            .frozen
            .owned
            .as_ref()
            .ok_or(SourceImportRecordArtifactError::Closed)?;
        owned
            .original_file
            .sync_all()
            .map_err(|_| SourceImportRecordArtifactError::SyncFailed)?;
        owned
            .platform
            .sync_record_artifact_directory(&owned.directory)
            .map_err(SourceImportRecordArtifactError::Platform)?;
        self.verify_original(deadline)?;
        let final_artifact = inspect_record_file(
            &owned.platform,
            &self.final_path,
            self.frozen.encoded_bytes,
            Some(&owned.identity),
            deadline,
        )?;
        self.verify_records(&final_artifact)?;
        // Complete original readback again after final readback and late callbacks.
        self.verify_original(deadline)
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
) -> Result<
    SourceImportRecordArtifactObservation<P::Identity>,
    SourceImportRecordArtifactError<P::Error>,
>
where
    P: SourceImportOutputArtifactPlatform,
{
    inspect_record_file(platform, path, expected_bytes, None, deadline)
}

/// Observes current stage/final evidence without any mutation or completion.
///
/// `expected` is (complete chain, rows, encoded bytes), never an identity proof.
/// Every result remains unresolved after restart until #348 supplies its join.
#[must_use]
pub fn classify_source_import_record_artifact<P: SourceImportOutputArtifactPlatform>(
    platform: &P,
    directory: &Path,
    staging_name: &str,
    final_name: &str,
    expected: (&[u8; 32], u64, u64),
    deadline: Instant,
) -> SourceImportRecordArtifactInspection {
    if !valid_temporary_name(staging_name) {
        return inspection(SourceImportRecordArtifactCurrentState::Unavailable);
    }
    if !valid_local_name(final_name) || staging_name == final_name {
        return inspection(SourceImportRecordArtifactCurrentState::Unavailable);
    }
    classify_paths(
        platform,
        &directory.join(staging_name),
        &directory.join(final_name),
        expected,
        None,
        deadline,
    )
}

enum CurrentLocation<I> {
    Missing,
    Exact(I),
    Conflict,
    Replaced,
    Unavailable,
}

const fn inspection(
    state: SourceImportRecordArtifactCurrentState,
) -> SourceImportRecordArtifactInspection {
    SourceImportRecordArtifactInspection { state }
}

fn classify_paths<P: SourceImportOutputArtifactPlatform>(
    platform: &P,
    stage: &Path,
    final_path: &Path,
    expected: (&[u8; 32], u64, u64),
    original: Option<&P::Identity>,
    deadline: Instant,
) -> SourceImportRecordArtifactInspection {
    use CurrentLocation::{Conflict, Exact, Missing, Replaced, Unavailable};
    use SourceImportRecordArtifactCurrentState as State;
    if expected.2 > MAX_SOURCE_IMPORT_RECORD_BYTES
        || expected.1 > expected.2
        || (expected.1 == 0) != (expected.2 == 0)
        || check_deadline::<P::Error>(deadline).is_err()
        || stage
            .parent()
            .is_none_or(|parent| platform.validate_directory(parent).is_err())
    {
        return inspection(State::Unavailable);
    }
    let stage = current_location(platform, stage, expected, original, deadline);
    let final_artifact = current_location(platform, final_path, expected, original, deadline);
    let classification = match (stage, final_artifact) {
        (Unavailable, _) | (_, Unavailable) => State::Unavailable,
        (Replaced, _) | (_, Replaced) => State::Replaced,
        (Conflict, _) | (_, Conflict) => State::Conflict,
        (Missing, Missing) => State::Missing,
        (Exact(_), Missing) => State::StageExact,
        (Missing, Exact(_)) => State::FinalExact,
        (Exact(stage), Exact(final_artifact)) if stage == final_artifact => {
            State::BothExactSameObject
        }
        (Exact(_), Exact(_)) => State::BothExactDistinctObjects,
    };
    if check_deadline::<P::Error>(deadline).is_err() {
        inspection(State::Unavailable)
    } else {
        inspection(classification)
    }
}

fn current_location<P: SourceImportOutputArtifactPlatform>(
    platform: &P,
    path: &Path,
    expected: (&[u8; 32], u64, u64),
    original: Option<&P::Identity>,
    deadline: Instant,
) -> CurrentLocation<P::Identity> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return CurrentLocation::Missing;
        }
        Err(_) => return CurrentLocation::Unavailable,
    };
    if !regular(&metadata) {
        return CurrentLocation::Replaced;
    }
    if metadata.len() != expected.2 {
        return CurrentLocation::Conflict;
    }
    match inspect_record_file(platform, path, expected.2, original, deadline) {
        Ok(observed) if observed.chain == *expected.0 && observed.rows == expected.1 => {
            CurrentLocation::Exact(observed.identity)
        }
        Ok(_) | Err(SourceImportRecordArtifactError::RecordChain(_)) => CurrentLocation::Conflict,
        Err(
            SourceImportRecordArtifactError::IdentityChanged
            | SourceImportRecordArtifactError::ReadbackMismatch
            | SourceImportRecordArtifactError::ObjectInvalid,
        ) => CurrentLocation::Replaced,
        Err(_) => CurrentLocation::Unavailable,
    }
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
    let before =
        fs::symlink_metadata(path).map_err(|_| SourceImportRecordArtifactError::ObjectInvalid)?;
    if !regular(&before) || before.len() != expected_bytes {
        return Err(SourceImportRecordArtifactError::ObjectInvalid);
    }
    let file = File::open(path).map_err(|_| SourceImportRecordArtifactError::ReadFailed)?;
    let opened = file
        .metadata()
        .map_err(|_| SourceImportRecordArtifactError::ReadFailed)?;
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
) -> Result<
    SourceImportRecordArtifactObservation<P::Identity>,
    SourceImportRecordArtifactError<P::Error>,
>
where
    P: SourceImportOutputArtifactPlatform,
{
    let opened = open_record_file(platform, path, expected_bytes, expected_identity, deadline)?;
    let before_modified = opened.modified;
    let mut reader = BufReader::new(opened.file);
    let before_identity = platform
        .identity(reader.get_ref())
        .map_err(SourceImportRecordArtifactError::Platform)?;
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
    if identity != before_identity
        || expected_identity.is_some_and(|expected| expected != &identity)
    {
        return Err(SourceImportRecordArtifactError::IdentityChanged);
    }
    let verified = reader
        .get_ref()
        .metadata()
        .map_err(|_| SourceImportRecordArtifactError::ReadFailed)?;
    if verified.len() != expected_bytes || verified.modified().ok() != before_modified {
        return Err(SourceImportRecordArtifactError::ReadbackMismatch);
    }
    check_deadline(deadline)?;
    let rows = chain.rows();
    Ok(SourceImportRecordArtifactObservation {
        chain: chain.finish(),
        rows,
        encoded_bytes: expected_bytes,
        identity,
    })
}

fn valid_temporary_name(name: &str) -> bool {
    valid_local_name(name) && name.starts_with(".source-map.") && name.as_bytes().ends_with(b".tmp")
}

fn valid_local_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && Path::new(name).file_name() == Some(OsStr::new(name))
}

fn regular(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink()
}

fn check_deadline<E>(deadline: Instant) -> Result<(), SourceImportRecordArtifactError<E>> {
    if Instant::now() >= deadline {
        Err(SourceImportRecordArtifactError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
