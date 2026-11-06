//! Plain local root ownership.
//!
//! The owner keeps one stable lock file open for the lifetime of a root
//! guard. Initialization is explicit, creates the initialization marker
//! before stores, and publishes the final manifest without clobbering.
//! Control metadata is validated by search-control-redb; this module never
//! parses a database file or scans retained objects.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_control_redb::plain_control::{
    PlainControlError, initialize_plain_control, inspect_plain_control, plain_control_is_empty,
};
use tempfile::Builder;

const MANIFEST_BYTES: &[u8] = b"format = \"eliot-plain-root\"\nversion = 1\n";
const INIT_MARKER_BYTES: &[u8] = b"eliot-plain-root-initializing-v1\n";
const MAX_ROOT_ENTRIES: usize = 32;
const MAX_CONTROL_ENTRIES: usize = 16;
const MAX_MANIFEST_BYTES: u64 = 4096;
const MAX_MARKER_BYTES: u64 = 256;
const ARCHIVE_COMPLETE_MARKER: &str = "archive.complete";
const INIT_MARKER_STAGE: &str = "root.initializing.stage";
const MANIFEST_STAGE: &str = "root.toml.stage";

/// Stable lock filename inside the selected root.
pub const LOCK_FILE: &str = "root.lock";
/// Final plain-root manifest filename.
pub const MANIFEST_FILE: &str = "root.toml";
/// Explicit initialization marker filename.
pub const INIT_MARKER: &str = "root.initializing";
/// Reserved import marker filename. Its presence only refuses normal open;
/// import content and destination binding belong to the import owner.
pub const INCOMPLETE_IMPORT_MARKER: &str = "root.importing";
/// Empty local-object directory created by plain-root initialization.
pub const OBJECT_DIR: &str = "objects";
/// Control-store directory.
pub const CONTROL_DIR: &str = "control";
/// Control database filename validated by search-control-redb.
pub const CONTROL_DB_FILE: &str = "control.redb";

/// Result type for plain-root operations.
pub type PlainRootResult<T> = Result<T, PlainRootError>;

/// Read-only classification of a selected root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainRootStatus {
    /// The selected directory does not exist.
    Absent,
    /// The selected directory exists and has no entries.
    Empty,
    /// A recognized, empty initialization was interrupted before commit.
    IncompleteInitialization,
    /// The root manifest and required control format are valid.
    Current,
    /// A known plain layout is missing required committed state or is damaged.
    Corrupt,
    /// The path contains an unknown, legacy, or unsupported format.
    Unsupported,
    /// A cooperating process holds the stable root lock or control database.
    Busy,
    /// A recognized archive marker makes this path non-runnable.
    Archive,
    /// The reserved import marker makes this path unavailable for normal use.
    IncompleteImport,
}

/// Path and factual status returned by read-only inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlainRootInspection {
    /// The path supplied by the caller.
    pub path: PathBuf,
    /// The status observed during this inspection.
    pub status: PlainRootStatus,
}

impl PlainRootInspection {
    /// Whether inspection found a committed current-format root.
    #[must_use]
    pub const fn is_current(&self) -> bool {
        matches!(self.status, PlainRootStatus::Current)
    }

    /// Whether inspection found unfinished initialization or import state.
    #[must_use]
    pub const fn is_incomplete(&self) -> bool {
        matches!(
            self.status,
            PlainRootStatus::IncompleteInitialization | PlainRootStatus::IncompleteImport
        )
    }
}

/// A process-lifetime exclusive root lock.
///
/// Drop this only after all stores and tasks using the root have closed.
#[derive(Debug)]
pub struct PlainRootGuard {
    path: PathBuf,
    // The open handle, not file existence or file contents, owns exclusion.
    _lock: File,
}

impl PlainRootGuard {
    /// Selected root path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Path to the control database opened by search-control-redb.
    #[must_use]
    pub fn control_path(&self) -> PathBuf {
        self.path.join(CONTROL_DIR).join(CONTROL_DB_FILE)
    }

    /// Path to the local-object directory.
    #[must_use]
    pub fn object_dir(&self) -> PathBuf {
        self.path.join(OBJECT_DIR)
    }

    /// Publishes one bounded object basename through the root's shared
    /// same-parent, no-clobber file publisher.
    ///
    /// `final_name` and `stage_name` must be local basenames. The object
    /// publisher passes the exact acquired bytes; an existing final file is
    /// reused only after byte-for-byte readback.
    pub fn publish_object_noclobber(
        &self,
        final_name: &str,
        stage_name: &str,
        bytes: &[u8],
        maximum_bytes: u64,
        deadline: Instant,
        cancellation: &AtomicBool,
    ) -> PlainRootResult<()> {
        if !valid_local_name(final_name) || !valid_local_name(stage_name) {
            return Err(PlainRootError::InvalidPath);
        }
        let destination = self.object_dir().join(final_name);
        publish_noclobber(
            &destination,
            stage_name,
            bytes,
            maximum_bytes,
            OperationContext::new(deadline, cancellation),
        )
    }

    /// A returned guard always owns a validated current root.
    #[must_use]
    pub const fn is_current(&self) -> bool {
        true
    }
}

fn valid_local_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && Path::new(name).file_name() == Some(OsStr::new(name))
}

/// Bounded, content-free errors for root operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainRootError {
    /// The selected root path is empty or not absolute.
    InvalidPath,
    /// Another process owns the stable root or control lock.
    Busy,
    /// No root exists at the selected path.
    Absent,
    /// The path contains an unsupported or unknown format.
    Unsupported,
    /// Recognized root metadata or control state is damaged.
    Corrupt,
    /// The selected path is a recognized archive, not a runnable root.
    Archive,
    /// The selected path has an incomplete import marker.
    IncompleteImport,
    /// The requested operation does not match the observed root state.
    WrongState,
    /// A no-clobber destination already contains different bytes.
    AlreadyExists,
    /// A filesystem operation failed.
    Io,
    /// A bounded root file or directory contains more entries than allowed.
    TooLarge,
    /// The caller's cancellation flag was set.
    Cancelled,
    /// The caller's absolute deadline expired.
    Deadline,
    /// The control owner reported an unavailable store.
    Unavailable,
    /// The control owner's bounded emptiness probe reached its byte limit.
    ProbeLimit,
}

impl PlainRootError {
    /// Stable, bounded error code for logs and command output.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid_path",
            Self::Busy => "busy",
            Self::Absent => "absent",
            Self::Unsupported => "unsupported",
            Self::Corrupt => "corrupt",
            Self::Archive => "archive",
            Self::IncompleteImport => "incomplete_import",
            Self::WrongState => "wrong_state",
            Self::AlreadyExists => "already_exists",
            Self::Io => "io",
            Self::TooLarge => "too_large",
            Self::Cancelled => "cancelled",
            Self::Deadline => "deadline",
            Self::Unavailable => "unavailable",
            Self::ProbeLimit => "probe_limit",
        }
    }
}

impl fmt::Display for PlainRootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for PlainRootError {}

/// One invocation's original absolute deadline and cancellation flag.
///
/// The value is private and copied unchanged to the control owner. Helpers
/// check it before and after bounded I/O; no helper creates a new deadline.
#[derive(Clone, Copy)]
struct OperationContext<'a> {
    deadline: Instant,
    cancellation: &'a AtomicBool,
}

impl<'a> OperationContext<'a> {
    const fn new(deadline: Instant, cancellation: &'a AtomicBool) -> Self {
        Self {
            deadline,
            cancellation,
        }
    }

    fn check(self) -> PlainRootResult<()> {
        if self.cancellation.load(Ordering::Relaxed) {
            return Err(PlainRootError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(PlainRootError::Deadline);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    File,
    Directory,
    Other,
}

fn check_absolute(path: &Path) -> PlainRootResult<()> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(PlainRootError::InvalidPath);
    }
    Ok(())
}

fn io_error(error: &std::io::Error) -> PlainRootError {
    match error.kind() {
        std::io::ErrorKind::AlreadyExists => PlainRootError::AlreadyExists,
        std::io::ErrorKind::WouldBlock => PlainRootError::Busy,
        _ => PlainRootError::Io,
    }
}

fn after_io<T>(result: std::io::Result<T>, operation: OperationContext<'_>) -> PlainRootResult<T> {
    operation.check()?;
    result.map_err(|error| io_error(&error))
}

const fn control_error(error: PlainControlError) -> PlainRootError {
    match error {
        PlainControlError::Cancelled => PlainRootError::Cancelled,
        PlainControlError::Deadline => PlainRootError::Deadline,
        PlainControlError::Unavailable => PlainRootError::Unavailable,
        PlainControlError::Corrupt => PlainRootError::Corrupt,
        PlainControlError::Unsupported => PlainRootError::Unsupported,
        PlainControlError::Busy => PlainRootError::Busy,
        PlainControlError::ProbeLimit => PlainRootError::ProbeLimit,
    }
}

fn path_kind(path: &Path, operation: OperationContext<'_>) -> PlainRootResult<Option<EntryKind>> {
    operation.check()?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            operation.check()?;
            return Ok(None);
        }
        Err(error) => {
            operation.check()?;
            return Err(io_error(&error));
        }
    };
    operation.check()?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Ok(Some(EntryKind::Other));
    }
    if metadata.is_file() {
        Ok(Some(EntryKind::File))
    } else if metadata.is_dir() {
        Ok(Some(EntryKind::Directory))
    } else {
        Ok(Some(EntryKind::Other))
    }
}

fn read_directory_names(
    path: &Path,
    maximum: usize,
    operation: OperationContext<'_>,
) -> PlainRootResult<Vec<OsString>> {
    operation.check()?;
    let entries = after_io(fs::read_dir(path), operation)?;
    let mut entries = entries;
    let mut names = Vec::with_capacity(maximum.min(32));
    loop {
        operation.check()?;
        operation.check()?;
        let next = entries.next();
        operation.check()?;
        let Some(entry) = next else {
            break;
        };
        let entry = entry.map_err(|error| io_error(&error))?;
        if names.len() == maximum {
            return Err(PlainRootError::TooLarge);
        }
        names.push(entry.file_name());
    }
    Ok(names)
}

fn directory_is_empty(path: &Path, operation: OperationContext<'_>) -> PlainRootResult<bool> {
    operation.check()?;
    let entries = after_io(fs::read_dir(path), operation)?;
    let mut entries = entries;
    operation.check()?;
    let next = entries.next();
    operation.check()?;
    match next {
        None => Ok(true),
        Some(Ok(_)) => Ok(false),
        Some(Err(error)) => Err(io_error(&error)),
    }
}

fn read_optional_bounded(
    path: &Path,
    maximum: u64,
    operation: OperationContext<'_>,
) -> PlainRootResult<Option<Vec<u8>>> {
    match path_kind(path, operation)? {
        None => return Ok(None),
        Some(EntryKind::File) => {}
        Some(EntryKind::Directory | EntryKind::Other) => {
            return Err(PlainRootError::Unsupported);
        }
    }

    operation.check()?;
    let file = after_io(File::open(path), operation)?;
    operation.check()?;
    let metadata = after_io(file.metadata(), operation)?;
    let length = metadata.len();
    if length > maximum {
        return Err(PlainRootError::TooLarge);
    }

    let limit = maximum.checked_add(1).ok_or(PlainRootError::TooLarge)?;
    let capacity = usize::try_from(length).map_err(|_| PlainRootError::TooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut reader = file.take(limit);
    operation.check()?;
    after_io(reader.read_to_end(&mut bytes), operation)?;
    if u64::try_from(bytes.len()).map_err(|_| PlainRootError::TooLarge)? > maximum {
        return Err(PlainRootError::TooLarge);
    }
    Ok(Some(bytes))
}

fn file_length(path: &Path, operation: OperationContext<'_>) -> PlainRootResult<Option<u64>> {
    match path_kind(path, operation)? {
        None => Ok(None),
        Some(EntryKind::File) => {
            operation.check()?;
            let metadata = after_io(fs::metadata(path), operation)?;
            Ok(Some(metadata.len()))
        }
        Some(EntryKind::Directory | EntryKind::Other) => Err(PlainRootError::Unsupported),
    }
}

fn contains_name(names: &[OsString], name: &str) -> bool {
    names.iter().any(|candidate| candidate == OsStr::new(name))
}

fn metadata_stage_matches(
    path: &Path,
    expected: &[u8],
    allow_partial: bool,
    operation: OperationContext<'_>,
) -> PlainRootResult<bool> {
    match path_kind(path, operation)? {
        None => return Ok(true),
        Some(EntryKind::File) => {}
        Some(EntryKind::Directory | EntryKind::Other) => return Ok(false),
    }
    let maximum = u64::try_from(expected.len()).map_err(|_| PlainRootError::TooLarge)?;
    Ok(
        read_optional_bounded(path, maximum, operation)?.is_none_or(|contents| {
            if allow_partial {
                expected.starts_with(&contents)
            } else {
                contents == expected
            }
        }),
    )
}

fn root_entries_are_supported(
    root: &Path,
    names: &[OsString],
    operation: OperationContext<'_>,
) -> PlainRootResult<bool> {
    for name in names {
        match path_kind(&root.join(name), operation)? {
            Some(EntryKind::File | EntryKind::Directory) => {}
            None | Some(EntryKind::Other) => return Ok(false),
        }
    }
    Ok(true)
}

fn required_kind(
    path: &Path,
    expected: EntryKind,
    operation: OperationContext<'_>,
) -> PlainRootResult<bool> {
    Ok(path_kind(path, operation)? == Some(expected))
}

fn inspect_control_format(path: &Path, operation: OperationContext<'_>) -> PlainRootResult<()> {
    operation.check()?;
    let result = inspect_plain_control(path, operation.deadline, operation.cancellation);
    operation.check()?;
    result.map_err(control_error)
}

fn control_is_empty(path: &Path, operation: OperationContext<'_>) -> PlainRootResult<bool> {
    operation.check()?;
    let result = plain_control_is_empty(path, operation.deadline, operation.cancellation);
    operation.check()?;
    result.map_err(control_error)
}

const fn classify_control_failure(error: PlainRootError) -> Option<PlainRootStatus> {
    match error {
        PlainRootError::Busy => Some(PlainRootStatus::Busy),
        PlainRootError::Corrupt => Some(PlainRootStatus::Corrupt),
        PlainRootError::Unsupported => Some(PlainRootStatus::Unsupported),
        _ => None,
    }
}

fn classify_root(
    root: &Path,
    operation: OperationContext<'_>,
    probe_incomplete: bool,
) -> PlainRootResult<PlainRootStatus> {
    operation.check()?;
    match path_kind(root, operation)? {
        None => return Ok(PlainRootStatus::Absent),
        Some(EntryKind::Directory) => {}
        Some(EntryKind::File | EntryKind::Other) => return Ok(PlainRootStatus::Unsupported),
    }

    let names = read_directory_names(root, MAX_ROOT_ENTRIES, operation)?;
    if names.is_empty() {
        return Ok(PlainRootStatus::Empty);
    }
    if !root_entries_are_supported(root, &names, operation)? {
        return Ok(PlainRootStatus::Unsupported);
    }

    if contains_name(&names, INCOMPLETE_IMPORT_MARKER) {
        return if required_kind(
            &root.join(INCOMPLETE_IMPORT_MARKER),
            EntryKind::File,
            operation,
        )? {
            Ok(PlainRootStatus::IncompleteImport)
        } else {
            Ok(PlainRootStatus::Unsupported)
        };
    }
    if contains_name(&names, ARCHIVE_COMPLETE_MARKER) {
        return if required_kind(
            &root.join(ARCHIVE_COMPLETE_MARKER),
            EntryKind::File,
            operation,
        )? {
            Ok(PlainRootStatus::Archive)
        } else {
            Ok(PlainRootStatus::Unsupported)
        };
    }

    if contains_name(&names, MANIFEST_FILE) {
        classify_committed_root(root, &names, operation)
    } else {
        classify_incomplete_root(root, &names, operation, probe_incomplete)
    }
}

fn classify_committed_root(
    root: &Path,
    names: &[OsString],
    operation: OperationContext<'_>,
) -> PlainRootResult<PlainRootStatus> {
    const ALLOWED: [&str; 7] = [
        LOCK_FILE,
        MANIFEST_FILE,
        INIT_MARKER,
        OBJECT_DIR,
        CONTROL_DIR,
        INIT_MARKER_STAGE,
        MANIFEST_STAGE,
    ];
    if names
        .iter()
        .any(|name| !ALLOWED.iter().any(|allowed| name == OsStr::new(allowed)))
    {
        return Ok(PlainRootStatus::Unsupported);
    }

    let manifest =
        match read_optional_bounded(&root.join(MANIFEST_FILE), MAX_MANIFEST_BYTES, operation) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(PlainRootStatus::Corrupt),
            Err(PlainRootError::Unsupported) => return Ok(PlainRootStatus::Unsupported),
            Err(error) => return Err(error),
        };
    if manifest != MANIFEST_BYTES {
        return Ok(PlainRootStatus::Unsupported);
    }

    if contains_name(names, INIT_MARKER) {
        let marker =
            match read_optional_bounded(&root.join(INIT_MARKER), MAX_MARKER_BYTES, operation) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => return Ok(PlainRootStatus::Corrupt),
                Err(PlainRootError::Unsupported) => return Ok(PlainRootStatus::Unsupported),
                Err(error) => return Err(error),
            };
        if marker != INIT_MARKER_BYTES {
            return Ok(PlainRootStatus::Unsupported);
        }
    }
    if (contains_name(names, INIT_MARKER_STAGE) || contains_name(names, MANIFEST_STAGE))
        && !contains_name(names, INIT_MARKER)
    {
        return Ok(PlainRootStatus::Unsupported);
    }
    if !metadata_stage_matches(
        &root.join(INIT_MARKER_STAGE),
        INIT_MARKER_BYTES,
        false,
        operation,
    )? || !metadata_stage_matches(&root.join(MANIFEST_STAGE), MANIFEST_BYTES, false, operation)?
    {
        return Ok(PlainRootStatus::Unsupported);
    }

    if !contains_name(names, LOCK_FILE) {
        return Ok(PlainRootStatus::Corrupt);
    }
    if !required_kind(&root.join(LOCK_FILE), EntryKind::File, operation)? {
        return Ok(PlainRootStatus::Corrupt);
    }
    if !contains_name(names, OBJECT_DIR)
        || !required_kind(&root.join(OBJECT_DIR), EntryKind::Directory, operation)?
    {
        return Ok(PlainRootStatus::Corrupt);
    }
    if !contains_name(names, CONTROL_DIR)
        || !required_kind(&root.join(CONTROL_DIR), EntryKind::Directory, operation)?
    {
        return Ok(PlainRootStatus::Corrupt);
    }

    let control_dir = root.join(CONTROL_DIR);
    let control_names = read_directory_names(&control_dir, MAX_CONTROL_ENTRIES, operation)?;
    if control_names.len() != 1 {
        return if control_names
            .iter()
            .any(|name| name != OsStr::new(CONTROL_DB_FILE))
        {
            Ok(PlainRootStatus::Unsupported)
        } else {
            Ok(PlainRootStatus::Corrupt)
        };
    }
    if !contains_name(&control_names, CONTROL_DB_FILE) {
        return Ok(PlainRootStatus::Unsupported);
    }
    if !root_entries_are_supported(&control_dir, &control_names, operation)? {
        return Ok(PlainRootStatus::Unsupported);
    }

    let control_path = control_dir.join(CONTROL_DB_FILE);
    if !required_kind(&control_path, EntryKind::File, operation)? {
        return Ok(PlainRootStatus::Corrupt);
    }
    if file_length(&control_path, operation)?.unwrap_or_default() == 0 {
        return Ok(PlainRootStatus::Corrupt);
    }
    match inspect_control_format(&control_path, operation) {
        Ok(()) => Ok(PlainRootStatus::Current),
        Err(error) => classify_control_failure(error).ok_or(error),
    }
}

fn classify_incomplete_root(
    root: &Path,
    names: &[OsString],
    operation: OperationContext<'_>,
    probe_incomplete: bool,
) -> PlainRootResult<PlainRootStatus> {
    const ALLOWED: [&str; 6] = [
        LOCK_FILE,
        INIT_MARKER,
        OBJECT_DIR,
        CONTROL_DIR,
        INIT_MARKER_STAGE,
        MANIFEST_STAGE,
    ];

    if names.len() == 1 && contains_name(names, LOCK_FILE) {
        return if required_kind(&root.join(LOCK_FILE), EntryKind::File, operation)? {
            Ok(PlainRootStatus::IncompleteInitialization)
        } else {
            Ok(PlainRootStatus::Unsupported)
        };
    }
    if !contains_name(names, INIT_MARKER) {
        // Before marker publication, only the stable lock and the fixed
        // marker stage may exist. Its bytes must be an exact prefix of the
        // real marker, including the zero-byte create-before-write case.
        return if names.len() == 2
            && contains_name(names, LOCK_FILE)
            && contains_name(names, INIT_MARKER_STAGE)
            && required_kind(&root.join(LOCK_FILE), EntryKind::File, operation)?
            && metadata_stage_matches(
                &root.join(INIT_MARKER_STAGE),
                INIT_MARKER_BYTES,
                true,
                operation,
            )? {
            Ok(PlainRootStatus::IncompleteInitialization)
        } else {
            Ok(PlainRootStatus::Unsupported)
        };
    }

    if names
        .iter()
        .any(|name| !ALLOWED.iter().any(|allowed| name == OsStr::new(allowed)))
    {
        return Ok(PlainRootStatus::Unsupported);
    }

    let marker = match read_optional_bounded(&root.join(INIT_MARKER), MAX_MARKER_BYTES, operation) {
        Ok(Some(bytes)) => bytes,
        Ok(None) | Err(PlainRootError::Unsupported) => return Ok(PlainRootStatus::Unsupported),
        Err(error) => return Err(error),
    };
    if marker != INIT_MARKER_BYTES {
        return Ok(PlainRootStatus::Unsupported);
    }
    if !metadata_stage_matches(
        &root.join(INIT_MARKER_STAGE),
        INIT_MARKER_BYTES,
        false,
        operation,
    )? || !metadata_stage_matches(&root.join(MANIFEST_STAGE), MANIFEST_BYTES, true, operation)?
    {
        return Ok(PlainRootStatus::Unsupported);
    }
    if !contains_name(names, LOCK_FILE) {
        return Ok(PlainRootStatus::Corrupt);
    }
    if !required_kind(&root.join(LOCK_FILE), EntryKind::File, operation)? {
        return Ok(PlainRootStatus::Corrupt);
    }

    let objects_path = root.join(OBJECT_DIR);
    if contains_name(names, OBJECT_DIR) {
        if !required_kind(&objects_path, EntryKind::Directory, operation)? {
            return Ok(PlainRootStatus::Unsupported);
        }
        if !directory_is_empty(&objects_path, operation)? {
            return Ok(PlainRootStatus::Corrupt);
        }
    }

    let control_dir = root.join(CONTROL_DIR);
    if contains_name(names, CONTROL_DIR) {
        if !required_kind(&control_dir, EntryKind::Directory, operation)? {
            return Ok(PlainRootStatus::Unsupported);
        }
        let control_names = read_directory_names(&control_dir, MAX_CONTROL_ENTRIES, operation)?;
        if control_names
            .iter()
            .any(|name| name != OsStr::new(CONTROL_DB_FILE))
        {
            return Ok(PlainRootStatus::Unsupported);
        }
        if contains_name(&control_names, CONTROL_DB_FILE) {
            if !required_kind(
                &control_dir.join(CONTROL_DB_FILE),
                EntryKind::File,
                operation,
            )? {
                return Ok(PlainRootStatus::Unsupported);
            }
            let control_path = control_dir.join(CONTROL_DB_FILE);
            if probe_incomplete && file_length(&control_path, operation)?.unwrap_or_default() != 0 {
                match control_is_empty(&control_path, operation) {
                    Ok(true) => {}
                    Ok(false) => return Ok(PlainRootStatus::Corrupt),
                    Err(error) => match classify_control_failure(error) {
                        Some(status) => return Ok(status),
                        None => return Err(error),
                    },
                }
            }
        }
    }

    Ok(PlainRootStatus::IncompleteInitialization)
}

const fn status_error(status: PlainRootStatus) -> PlainRootError {
    match status {
        PlainRootStatus::Absent => PlainRootError::Absent,
        PlainRootStatus::Empty | PlainRootStatus::IncompleteInitialization => {
            PlainRootError::WrongState
        }
        PlainRootStatus::Current => PlainRootError::AlreadyExists,
        PlainRootStatus::Corrupt => PlainRootError::Corrupt,
        PlainRootStatus::Unsupported => PlainRootError::Unsupported,
        PlainRootStatus::Busy => PlainRootError::Busy,
        PlainRootStatus::Archive => PlainRootError::Archive,
        PlainRootStatus::IncompleteImport => PlainRootError::IncompleteImport,
    }
}

fn open_lock_file(
    root: &Path,
    create_if_missing: bool,
    operation: OperationContext<'_>,
) -> PlainRootResult<File> {
    let lock_path = root.join(LOCK_FILE);
    match path_kind(&lock_path, operation)? {
        Some(EntryKind::File) => {
            operation.check()?;
            let opened = OpenOptions::new()
                .read(true)
                .write(true)
                .truncate(false)
                .open(&lock_path);
            after_io(opened, operation)
        }
        Some(EntryKind::Directory | EntryKind::Other) => Err(PlainRootError::Unsupported),
        None if !create_if_missing => Err(PlainRootError::Corrupt),
        None => {
            operation.check()?;
            let created = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&lock_path);
            operation.check()?;
            let file = match created {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    operation.check()?;
                    if path_kind(&lock_path, operation)? != Some(EntryKind::File) {
                        return Err(PlainRootError::Unsupported);
                    }
                    let opened = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .truncate(false)
                        .open(&lock_path);
                    after_io(opened, operation)?
                }
                Err(error) => return Err(io_error(&error)),
            };
            operation.check()?;
            Ok(file)
        }
    }
}

fn acquire_lock(
    root: &Path,
    create_if_missing: bool,
    operation: OperationContext<'_>,
) -> PlainRootResult<File> {
    let file = open_lock_file(root, create_if_missing, operation)?;
    operation.check()?;
    let result = file.try_lock();
    operation.check()?;
    match result {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(PlainRootError::Busy),
        Err(TryLockError::Error(error)) => Err(io_error(&error)),
    }
}

fn observed_status(
    root: &Path,
    operation: OperationContext<'_>,
) -> PlainRootResult<PlainRootStatus> {
    let status = classify_root(root, operation, false)?;
    if !matches!(
        status,
        PlainRootStatus::Current | PlainRootStatus::IncompleteInitialization
    ) {
        return Ok(status);
    }
    let lock = match acquire_lock(root, false, operation) {
        Ok(lock) => lock,
        Err(PlainRootError::Busy) => return Ok(PlainRootStatus::Busy),
        Err(PlainRootError::Corrupt) => return Ok(PlainRootStatus::Corrupt),
        Err(PlainRootError::Unsupported) => return Ok(PlainRootStatus::Unsupported),
        Err(error) => return Err(error),
    };
    let checked = classify_root(
        root,
        operation,
        status == PlainRootStatus::IncompleteInitialization,
    );
    drop(lock);
    checked
}

/// Inspect a selected root without creating or repairing files.
///
/// The caller's absolute deadline and cancellation flag are honored through
/// filesystem and control-format reads.
pub fn inspect_plain(
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> PlainRootResult<PlainRootInspection> {
    let operation = OperationContext::new(deadline, cancellation);
    operation.check()?;
    check_absolute(path)?;
    let status = observed_status(path, operation)?;
    operation.check()?;
    Ok(PlainRootInspection {
        path: path.to_path_buf(),
        status,
    })
}

// Callers hold root.lock and have reclassified the exact initialization
// layout. A fixed basename alone never authorizes removing a stage.
fn remove_metadata_stage(
    path: &Path,
    expected: &[u8],
    allow_partial: bool,
    operation: OperationContext<'_>,
) -> PlainRootResult<()> {
    if !metadata_stage_matches(path, expected, allow_partial, operation)? {
        return Err(PlainRootError::Unsupported);
    }
    remove_known_file(path, operation)
}

fn metadata_stages_present(root: &Path, operation: OperationContext<'_>) -> PlainRootResult<bool> {
    Ok(
        path_kind(&root.join(INIT_MARKER_STAGE), operation)?.is_some()
            || path_kind(&root.join(MANIFEST_STAGE), operation)?.is_some(),
    )
}

fn cleanup_complete_metadata_stages(
    root: &Path,
    operation: OperationContext<'_>,
) -> PlainRootResult<()> {
    if !metadata_stages_present(root, operation)? {
        return Ok(());
    }
    let marker = read_optional_bounded(&root.join(INIT_MARKER), MAX_MARKER_BYTES, operation)?;
    let manifest = read_optional_bounded(&root.join(MANIFEST_FILE), MAX_MANIFEST_BYTES, operation)?;
    if marker.as_deref() != Some(INIT_MARKER_BYTES)
        || manifest.as_deref() != Some(MANIFEST_BYTES)
        || !metadata_stage_matches(
            &root.join(INIT_MARKER_STAGE),
            INIT_MARKER_BYTES,
            false,
            operation,
        )?
        || !metadata_stage_matches(&root.join(MANIFEST_STAGE), MANIFEST_BYTES, false, operation)?
    {
        return Err(PlainRootError::Unsupported);
    }
    // Validate both stages before deleting either one.
    remove_known_file(&root.join(INIT_MARKER_STAGE), operation)?;
    remove_known_file(&root.join(MANIFEST_STAGE), operation)?;
    sync_parent_directory(&root.join(MANIFEST_FILE), operation)
}

fn publish_noclobber(
    destination: &Path,
    stage_name: &str,
    bytes: &[u8],
    maximum: u64,
    operation: OperationContext<'_>,
) -> PlainRootResult<()> {
    if u64::try_from(bytes.len()).map_err(|_| PlainRootError::TooLarge)? > maximum {
        return Err(PlainRootError::TooLarge);
    }
    let parent = destination.parent().ok_or(PlainRootError::InvalidPath)?;
    let stage_path = parent.join(stage_name);
    if let Some(existing) = read_optional_bounded(destination, maximum, operation)? {
        if existing != bytes {
            return Err(PlainRootError::AlreadyExists);
        }
        remove_metadata_stage(&stage_path, bytes, false, operation)?;
        return sync_parent_directory(destination, operation);
    }

    remove_metadata_stage(&stage_path, bytes, true, operation)?;
    operation.check()?;
    let mut staged = after_io(
        Builder::new()
            .prefix(stage_name)
            .suffix("")
            .rand_bytes(0)
            .tempfile_in(parent),
        operation,
    )?;
    operation.check()?;
    after_io(staged.write_all(bytes), operation)?;
    operation.check()?;
    after_io(staged.flush(), operation)?;
    operation.check()?;
    let staged_metadata = after_io(staged.as_file().metadata(), operation)?;
    let staged_length = staged_metadata.len();
    if staged_length != u64::try_from(bytes.len()).map_err(|_| PlainRootError::TooLarge)? {
        return Err(PlainRootError::Io);
    }
    operation.check()?;
    after_io(staged.as_file().sync_all(), operation)?;

    operation.check()?;
    let seek_result = staged.as_file_mut().seek(SeekFrom::Start(0));
    after_io(seek_result, operation)?;
    let mut staged_readback = Vec::with_capacity(bytes.len());
    {
        let mut reader = staged.as_file_mut().take(
            u64::try_from(bytes.len())
                .map_err(|_| PlainRootError::TooLarge)?
                .checked_add(1)
                .ok_or(PlainRootError::TooLarge)?,
        );
        operation.check()?;
        let read_result = reader.read_to_end(&mut staged_readback);
        operation.check()?;
        read_result.map_err(|error| io_error(&error))?;
    }
    if staged_readback != bytes {
        return Err(PlainRootError::Corrupt);
    }

    operation.check()?;
    let persist_result = staged.persist_noclobber(destination);
    operation.check()?;
    match persist_result {
        Ok(file) => drop(file),
        Err(error) => {
            let persistence_error = error.error;
            drop(error.file);
            operation.check()?;
            match read_optional_bounded(destination, maximum, operation)? {
                Some(existing) if existing == bytes => {}
                Some(_) => return Err(PlainRootError::AlreadyExists),
                None => return Err(io_error(&persistence_error)),
            }
        }
    }

    operation.check()?;
    match read_optional_bounded(destination, maximum, operation)? {
        Some(existing) if existing == bytes => {}
        Some(_) => return Err(PlainRootError::Corrupt),
        None => return Err(PlainRootError::Io),
    }
    // persist_noclobber may succeed while leaving the original hard link.
    // Its fixed name and exact completed metadata make that link removable.
    remove_metadata_stage(&stage_path, bytes, false, operation)?;
    sync_parent_directory(destination, operation)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path, operation: OperationContext<'_>) -> PlainRootResult<()> {
    let parent = path.parent().ok_or(PlainRootError::InvalidPath)?;
    operation.check()?;
    let directory = after_io(File::open(parent), operation)?;
    operation.check()?;
    after_io(directory.sync_all(), operation).map(|()| ())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path, operation: OperationContext<'_>) -> PlainRootResult<()> {
    operation.check()
}

fn ensure_store_directory(
    root: &Path,
    name: &str,
    operation: OperationContext<'_>,
) -> PlainRootResult<()> {
    let path = root.join(name);
    match path_kind(&path, operation)? {
        Some(EntryKind::Directory) => Ok(()),
        Some(EntryKind::File | EntryKind::Other) => Err(PlainRootError::Unsupported),
        None => {
            operation.check()?;
            after_io(fs::create_dir(&path), operation)?;
            if required_kind(&path, EntryKind::Directory, operation)? {
                Ok(())
            } else {
                Err(PlainRootError::Unsupported)
            }
        }
    }
}

fn ensure_initialization_marker(
    root: &Path,
    operation: OperationContext<'_>,
) -> PlainRootResult<()> {
    let marker_path = root.join(INIT_MARKER);
    publish_noclobber(
        &marker_path,
        INIT_MARKER_STAGE,
        INIT_MARKER_BYTES,
        MAX_MARKER_BYTES,
        operation,
    )
}

fn initialize_control(path: &Path, operation: OperationContext<'_>) -> PlainRootResult<()> {
    operation.check()?;
    let result = initialize_plain_control(path, operation.deadline, operation.cancellation);
    operation.check()?;
    result.map_err(control_error)
}

/// Explicitly initialize an absent, empty, or recognized empty incomplete root.
///
/// On success the returned guard retains the stable root lock. Existing
/// committed roots are never overwritten or repaired.
pub fn initialize_plain(
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> PlainRootResult<PlainRootGuard> {
    let operation = OperationContext::new(deadline, cancellation);
    operation.check()?;
    check_absolute(path)?;

    let mut status = classify_root(path, operation, false)?;
    if status == PlainRootStatus::Absent {
        operation.check()?;
        after_io(fs::create_dir_all(path), operation)?;
        status = classify_root(path, operation, false)?;
    }
    match status {
        PlainRootStatus::Empty | PlainRootStatus::IncompleteInitialization => {}
        PlainRootStatus::Current => {
            if metadata_stages_present(path, operation)? {
                let _lock = acquire_lock(path, false, operation)?;
                match classify_root(path, operation, false)? {
                    PlainRootStatus::Current => {
                        cleanup_complete_metadata_stages(path, operation)?;
                    }
                    other => return Err(status_error(other)),
                }
            }
            return Err(PlainRootError::AlreadyExists);
        }
        other => return Err(status_error(other)),
    }

    let lock = acquire_lock(path, true, operation)?;
    let locked_status = classify_root(path, operation, true)?;
    match locked_status {
        PlainRootStatus::IncompleteInitialization => {}
        other => return Err(status_error(other)),
    }

    ensure_initialization_marker(path, operation)?;
    ensure_store_directory(path, OBJECT_DIR, operation)?;
    ensure_store_directory(path, CONTROL_DIR, operation)?;

    let object_dir = path.join(OBJECT_DIR);
    if !directory_is_empty(&object_dir, operation)? {
        return Err(PlainRootError::WrongState);
    }
    initialize_control(&path.join(CONTROL_DIR).join(CONTROL_DB_FILE), operation)?;

    match classify_root(path, operation, true)? {
        PlainRootStatus::IncompleteInitialization => {}
        other => return Err(status_error(other)),
    }

    publish_noclobber(
        &path.join(MANIFEST_FILE),
        MANIFEST_STAGE,
        MANIFEST_BYTES,
        MAX_MANIFEST_BYTES,
        operation,
    )?;
    match classify_root(path, operation, false)? {
        PlainRootStatus::Current => Ok(PlainRootGuard {
            path: path.to_path_buf(),
            _lock: lock,
        }),
        other => Err(status_error(other)),
    }
}

/// Open an existing current root without creating or repairing its files.
///
/// The returned guard owns the stable lock until it is dropped.
pub fn open_plain(
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> PlainRootResult<PlainRootGuard> {
    let operation = OperationContext::new(deadline, cancellation);
    operation.check()?;
    check_absolute(path)?;
    match classify_root(path, operation, false)? {
        PlainRootStatus::Current => {
            let lock = acquire_lock(path, false, operation)?;
            match classify_root(path, operation, false)? {
                PlainRootStatus::Current => Ok(PlainRootGuard {
                    path: path.to_path_buf(),
                    _lock: lock,
                }),
                other => Err(status_error(other)),
            }
        }
        PlainRootStatus::IncompleteInitialization => {
            let _lock = acquire_lock(path, false, operation)?;
            let status = classify_root(path, operation, true)?;
            Err(status_error(status))
        }
        other => Err(status_error(other)),
    }
}

fn remove_known_file(path: &Path, operation: OperationContext<'_>) -> PlainRootResult<()> {
    match path_kind(path, operation)? {
        None => Ok(()),
        Some(EntryKind::File) => {
            operation.check()?;
            let result = fs::remove_file(path);
            operation.check()?;
            match result {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(&error)),
            }
            Ok(())
        }
        Some(EntryKind::Directory | EntryKind::Other) => Err(PlainRootError::Unsupported),
    }
}

fn remove_known_empty_directory(
    path: &Path,
    operation: OperationContext<'_>,
) -> PlainRootResult<()> {
    match path_kind(path, operation)? {
        None => Ok(()),
        Some(EntryKind::Directory) => {
            if !directory_is_empty(path, operation)? {
                return Err(PlainRootError::WrongState);
            }
            operation.check()?;
            let result = fs::remove_dir(path);
            operation.check()?;
            match result {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(&error)),
            }
            Ok(())
        }
        Some(EntryKind::File | EntryKind::Other) => Err(PlainRootError::Unsupported),
    }
}

/// Discard only a marker-owned, empty unfinished initialization.
///
/// The stable root lock is retained in place. A lock-only root remains a
/// recognized retry point and is never deleted by this operation.
pub fn discard_incomplete_plain(
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> PlainRootResult<()> {
    let operation = OperationContext::new(deadline, cancellation);
    operation.check()?;
    check_absolute(path)?;

    match classify_root(path, operation, false)? {
        PlainRootStatus::IncompleteInitialization => {}
        other => return Err(status_error(other)),
    }
    let marker_path = path.join(INIT_MARKER);
    match read_optional_bounded(&marker_path, MAX_MARKER_BYTES, operation)? {
        Some(bytes) if bytes == INIT_MARKER_BYTES => {}
        _ => return Err(PlainRootError::WrongState),
    }

    let _lock = acquire_lock(path, false, operation)?;
    match classify_root(path, operation, true)? {
        PlainRootStatus::IncompleteInitialization => {}
        other => return Err(status_error(other)),
    }
    match read_optional_bounded(&marker_path, MAX_MARKER_BYTES, operation)? {
        Some(bytes) if bytes == INIT_MARKER_BYTES => {}
        _ => return Err(PlainRootError::WrongState),
    }
    if path_kind(&path.join(MANIFEST_FILE), operation)?.is_some() {
        return Err(PlainRootError::WrongState);
    }

    remove_metadata_stage(
        &path.join(INIT_MARKER_STAGE),
        INIT_MARKER_BYTES,
        false,
        operation,
    )?;
    remove_metadata_stage(&path.join(MANIFEST_STAGE), MANIFEST_BYTES, true, operation)?;
    let control_dir = path.join(CONTROL_DIR);
    if path_kind(&control_dir, operation)? == Some(EntryKind::Directory) {
        remove_known_file(&control_dir.join(CONTROL_DB_FILE), operation)?;
        remove_known_empty_directory(&control_dir, operation)?;
    }
    remove_known_empty_directory(&path.join(OBJECT_DIR), operation)?;
    remove_known_file(&marker_path, operation)?;
    sync_parent_directory(&marker_path, operation)
}
