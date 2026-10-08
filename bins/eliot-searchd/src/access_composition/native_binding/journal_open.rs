//! Verified final-handle opening of the persistent standalone control journal.
//!
//! This adapter consumes the already-held data-root owner, opens only the fixed
//! `control/control.redb` locator, rejects link/reparse/multi-link substitution,
//! binds the final native file identity and package-owned schema material, then
//! hands the exact file and independently derived [`JournalIdentity`] to the
//! root-owning standalone process composition.

use core::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io;

use search_contracts::{BindingId, Blake3Digest32, OwnerEpoch};
use search_continuation::{ContinuationCleanup, ContinuationLimits};
use search_control_redb::{
    ControlError, JournalIdentity, JournalLimits, JournalOwnerBinding,
    JournalPathIdentity, JournalSchemaIdentity, MutationId,
    current_journal_schema_descriptor, derive_journal_identity,
};
use search_handles::HandlePolicy;
use search_ports::{CancellationProbe, OperationContext};

use crate::development::DataRootGuard;
use eliot_searchd::native_file::{self, Observation, ObservationError};

use super::{
    NativeBindingExpectation, StandaloneProcessError, StandaloneProcessOwner,
};

const CONTROL_DIRECTORY: &str = "control";
const CONTROL_DATABASE: &str = "control.redb";
const CONTROL_RELATIVE_LOCATOR: &[u8] = b"control/control.redb";
const LOCATOR_DOMAIN: &[u8] = b"ELIOT-CONTROL-JOURNAL-CANONICAL-LOCATOR-v1\0";
const OBJECT_DOMAIN: &[u8] = b"ELIOT-CONTROL-JOURNAL-NATIVE-OBJECT-v1\0";
const PATH_IDENTITY_DOMAIN: &[u8] = b"ELIOT-CONTROL-JOURNAL-PATH-IDENTITY-v1\0";

/// Failure before or during root-owned persistent standalone restoration.
///
/// Paths, native identifiers, record bytes, credentials and source metadata are
/// never formatted. A process failure may follow durable recovery/publication;
/// its existing unknown-outcome semantics are preserved unchanged.
#[derive(Debug)]
pub enum StandaloneProcessOpenError<E> {
    /// Fixed control directory/database shape or containment was unsafe.
    InvalidControlObject,
    /// Final-handle native identity could not be established.
    NativeObservation(ObservationError),
    /// Owner/path/schema identity construction failed.
    Identity(ControlError),
    /// Bounded local file opening or metadata read failed.
    Io(io::Error),
    /// Existing process/bootstrap restoration failed.
    Process(StandaloneProcessError<E>),
}

impl<E: fmt::Display> fmt::Display for StandaloneProcessOpenError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidControlObject => {
                formatter.write_str("STANDALONE_CONTROL_OBJECT_INVALID")
            }
            Self::NativeObservation(error) => fmt::Display::fmt(error, formatter),
            Self::Identity(error) => fmt::Display::fmt(error, formatter),
            Self::Io(_) => formatter.write_str("STANDALONE_CONTROL_OPEN_IO_ERROR"),
            Self::Process(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<E: fmt::Debug + fmt::Display + 'static> std::error::Error
    for StandaloneProcessOpenError<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NativeObservation(error) => Some(error),
            Self::Identity(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Process(error) => Some(error),
            Self::InvalidControlObject => None,
        }
    }
}

impl<E> From<ObservationError> for StandaloneProcessOpenError<E> {
    fn from(error: ObservationError) -> Self {
        Self::NativeObservation(error)
    }
}

impl<E> From<ControlError> for StandaloneProcessOpenError<E> {
    fn from(error: ControlError) -> Self {
        Self::Identity(error)
    }
}

impl<E> From<io::Error> for StandaloneProcessOpenError<E> {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl<E> From<StandaloneProcessError<E>> for StandaloneProcessOpenError<E> {
    fn from(error: StandaloneProcessError<E>) -> Self {
        Self::Process(error)
    }
}

/// Restore one existing native standalone process from the fixed verified journal.
///
/// The acquired root owner is consumed and retained by the returned process. The
/// journal file must already exist and be nonempty; this path never creates an
/// empty replacement, follows a symlink/reparse object, accepts another relative
/// locator or derives expected identity from the database header.
///
/// # Errors
///
/// Returns a closed [`StandaloneProcessOpenError`] when the fixed control path,
/// final native object, owner/schema identity or existing persistent bootstrap
/// cannot be verified. A failure after possible durable recovery retains the
/// underlying unknown-outcome semantics and never authorizes a changed retry.
#[allow(clippy::too_many_arguments)]
pub fn restore_existing_standalone_process<C, E, F>(
    root: DataRootGuard,
    journal_limits: JournalLimits,
    connection_capacity: usize,
    handle_policy: HandlePolicy,
    continuation_limits: ContinuationLimits,
    binding_id: BindingId,
    operation_id: MutationId,
    expected: NativeBindingExpectation,
    cleanup: &mut F,
    context: &OperationContext<C>,
) -> Result<StandaloneProcessOwner, StandaloneProcessOpenError<E>>
where
    C: CancellationProbe + Clone,
    F: FnMut(&ContinuationCleanup, &OperationContext<C>) -> Result<(), E>,
{
    let (file, identity) = open_verified_journal(&root)?;
    StandaloneProcessOwner::restore_existing(
        root,
        file,
        identity,
        journal_limits,
        connection_capacity,
        handle_policy,
        continuation_limits,
        binding_id,
        operation_id,
        expected,
        cleanup,
        context,
    )
    .map_err(Into::into)
}

fn open_verified_journal<E>(
    root: &DataRootGuard,
) -> Result<(File, JournalIdentity), StandaloneProcessOpenError<E>> {
    let canonical_root = root.canonical_root();
    let control = canonical_root.join(CONTROL_DIRECTORY);
    let control_metadata = fs::symlink_metadata(&control)?;
    if control_metadata.file_type().is_symlink()
        || is_reparse(&control_metadata)
        || !control_metadata.is_dir()
    {
        return Err(StandaloneProcessOpenError::InvalidControlObject);
    }
    let canonical_control = fs::canonicalize(&control)?;
    if canonical_control.parent() != Some(canonical_root)
        || !canonical_control.starts_with(canonical_root)
    {
        return Err(StandaloneProcessOpenError::InvalidControlObject);
    }

    let path = canonical_control.join(CONTROL_DATABASE);
    let path_metadata = fs::symlink_metadata(&path)?;
    if path_metadata.file_type().is_symlink()
        || is_reparse(&path_metadata)
        || !path_metadata.is_file()
        || path_metadata.len() == 0
    {
        return Err(StandaloneProcessOpenError::InvalidControlObject);
    }
    let canonical_path = fs::canonicalize(&path)?;
    if canonical_path.parent() != Some(canonical_control.as_path())
        || canonical_path.file_name().and_then(|name| name.to_str())
            != Some(CONTROL_DATABASE)
    {
        return Err(StandaloneProcessOpenError::InvalidControlObject);
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&canonical_path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || is_reparse(&opened) || opened.len() == 0 {
        return Err(StandaloneProcessOpenError::InvalidControlObject);
    }
    if native_file::hardlink_count(&file)? != 1 {
        return Err(StandaloneProcessOpenError::InvalidControlObject);
    }
    let observation = native_file::observe(&file)?;

    // Reopen the final canonical locator and require it still names the same
    // object. The retained first handle remains authoritative for redb opening.
    let check = File::open(&canonical_path)?;
    if native_file::hardlink_count(&check)? != 1
        || native_file::observe(&check)? != observation
    {
        return Err(StandaloneProcessOpenError::InvalidControlObject);
    }

    let (incarnation, data_root_id, current_owner_epoch) = root.journal_owner_inputs();
    let stored_owner_epoch = current_owner_epoch
        .get()
        .checked_sub(1)
        .filter(|epoch| *epoch > 0)
        .and_then(|epoch| OwnerEpoch::new(epoch).ok())
        .ok_or(StandaloneProcessOpenError::InvalidControlObject)?;

    let locator_digest = canonical_locator_digest(data_root_id);
    let object_digest = native_object_digest(observation);
    let path_digest = combined_path_digest(locator_digest, object_digest);
    let path_identity = JournalPathIdentity::new(path_digest)?;

    let schema = current_journal_schema_descriptor();
    let schema_digest =
        Blake3Digest32::from_bytes(*blake3::hash(schema.material()).as_bytes());
    let schema_identity = JournalSchemaIdentity::new(schema_digest, schema.version())?;
    let owner = JournalOwnerBinding::new(incarnation, data_root_id, stored_owner_epoch);
    let identity = derive_journal_identity(owner, path_identity, schema_identity)?;
    Ok((file, identity))
}

fn canonical_locator_digest(data_root_id: search_contracts::DataRootId) -> Blake3Digest32 {
    let mut hash = blake3::Hasher::new();
    hash.update(LOCATOR_DOMAIN);
    hash.update(data_root_id.as_bytes());
    hash.update(CONTROL_RELATIVE_LOCATOR);
    Blake3Digest32::from_bytes(*hash.finalize().as_bytes())
}

fn native_object_digest(observation: Observation) -> Blake3Digest32 {
    let mut hash = blake3::Hasher::new();
    hash.update(OBJECT_DOMAIN);
    hash.update(&observation.volume_serial.to_be_bytes());
    hash.update(&observation.file_index.to_be_bytes());
    hash.update(&observation.creation_time.to_be_bytes());
    Blake3Digest32::from_bytes(*hash.finalize().as_bytes())
}

fn combined_path_digest(
    locator: Blake3Digest32,
    object: Blake3Digest32,
) -> Blake3Digest32 {
    let mut hash = blake3::Hasher::new();
    hash.update(PATH_IDENTITY_DOMAIN);
    hash.update(locator.as_bytes());
    hash.update(object.as_bytes());
    Blake3Digest32::from_bytes(*hash.finalize().as_bytes())
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &Metadata) -> bool {
    false
}
