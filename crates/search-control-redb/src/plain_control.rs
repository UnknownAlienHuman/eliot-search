//! Plain-root control metadata stored in the root's control.redb file.
//!
//! Fresh plain roots use redb 4.5. The legacy redb 2.6 owner remains separate;
//! this module never upgrades an existing file or writes a populated database.
//! An owned preformat empty database can finish initialization after a read-only
//! proof. The caller holds root exclusion for initialization and discard.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use redb_plain::backends::FileBackend;
use redb_plain::{ReadableDatabase, ReadableTableMetadata, TableDefinition, TableHandle};

const META_TABLE: TableDefinition<&str, u64> = TableDefinition::new("plain_control_meta");
const FORMAT_VERSION_KEY: &str = "format_version";
const FORMAT_VERSION: u64 = 1;
const EMPTY_PROBE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmptyControlState {
    Preformat,
    Formatted,
    NotEmpty,
}

struct ProvenPreformatFile {
    file: File,
    length: u64,
}

/// Bounded failures from plain-root control access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainControlError {
    /// The caller's cancellation flag was set.
    Cancelled,
    /// The caller's deadline expired.
    Deadline,
    /// The file or storage operation is unavailable.
    Unavailable,
    /// The database is corrupt or lacks required format metadata.
    Corrupt,
    /// The database format or metadata schema is unsupported.
    Unsupported,
    /// The database is open or otherwise busy.
    Busy,
    /// The database exceeds the bounded semantic-empty probe size.
    ProbeLimit,
}

impl PlainControlError {
    /// Return a bounded static code suitable for logs and status mapping.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Deadline => "deadline",
            Self::Unavailable => "unavailable",
            Self::Corrupt => "corrupt",
            Self::Unsupported => "unsupported",
            Self::Busy => "busy",
            Self::ProbeLimit => "probe_limit",
        }
    }
}

impl fmt::Display for PlainControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for PlainControlError {}

fn check(deadline: Instant, cancellation: &AtomicBool) -> Result<(), PlainControlError> {
    if cancellation.load(Ordering::Relaxed) {
        return Err(PlainControlError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(PlainControlError::Deadline);
    }
    Ok(())
}

fn map_io_error(error: &io::Error) -> PlainControlError {
    match error.kind() {
        io::ErrorKind::AlreadyExists | io::ErrorKind::WouldBlock => PlainControlError::Busy,
        _ => PlainControlError::Unavailable,
    }
}

fn map_storage_error(error: &redb_plain::StorageError) -> PlainControlError {
    match error {
        redb_plain::StorageError::Corrupted(_) => PlainControlError::Corrupt,
        redb_plain::StorageError::ValueTooLarge(_) | redb_plain::StorageError::Unsupported => {
            PlainControlError::Unsupported
        }
        // I/O, closed/poisoned state and future storage failures remain unavailable.
        _ => PlainControlError::Unavailable,
    }
}

fn map_database_error(error: redb_plain::DatabaseError) -> PlainControlError {
    match error {
        redb_plain::DatabaseError::DatabaseAlreadyOpen
        | redb_plain::DatabaseError::LockProtocolMismatch
        | redb_plain::DatabaseError::TransactionInProgress => PlainControlError::Busy,
        redb_plain::DatabaseError::RepairAborted => PlainControlError::Corrupt,
        redb_plain::DatabaseError::UpgradeRequired(_) => PlainControlError::Unsupported,
        redb_plain::DatabaseError::Storage(error) => map_storage_error(&error),
        _ => PlainControlError::Unavailable,
    }
}

fn map_transaction_error(error: redb_plain::TransactionError) -> PlainControlError {
    match error {
        redb_plain::TransactionError::Storage(error) => map_storage_error(&error),
        redb_plain::TransactionError::ReadTransactionStillInUse(_)
        | redb_plain::TransactionError::WouldBlock => PlainControlError::Busy,
        _ => PlainControlError::Unavailable,
    }
}

fn map_table_error(error: redb_plain::TableError) -> PlainControlError {
    match error {
        redb_plain::TableError::TableDoesNotExist(_) | redb_plain::TableError::TableExists(_) => {
            PlainControlError::Corrupt
        }
        redb_plain::TableError::TableAlreadyOpen(_, _) => PlainControlError::Busy,
        redb_plain::TableError::Storage(error) => map_storage_error(&error),
        // Type/schema mismatches and future table kinds are unsupported.
        _ => PlainControlError::Unsupported,
    }
}

fn map_commit_error(error: redb_plain::CommitError) -> PlainControlError {
    match error {
        redb_plain::CommitError::Storage(error) => map_storage_error(&error),
        // Poisoned transactions and future commit failures remain unavailable.
        _ => PlainControlError::Unavailable,
    }
}

fn opened_file_len(
    file: &File,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<u64, PlainControlError> {
    check(deadline, cancellation)?;
    let metadata = file.metadata();
    check(deadline, cancellation)?;
    let metadata = metadata.map_err(|error| map_io_error(&error))?;
    if !metadata.is_file() {
        return Err(PlainControlError::Unsupported);
    }
    Ok(metadata.len())
}

fn open_read_only_file(
    file: File,
    byte_limit: Option<u64>,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<redb_plain::ReadOnlyDatabase, PlainControlError> {
    let file_len = opened_file_len(&file, deadline, cancellation)?;
    if byte_limit.is_some_and(|limit| file_len > limit) {
        return Err(PlainControlError::ProbeLimit);
    }

    check(deadline, cancellation)?;
    let backend = FileBackend::new(file);
    check(deadline, cancellation)?;
    let backend = backend.map_err(map_database_error)?;

    check(deadline, cancellation)?;
    let opened = redb_plain::Builder::new().open_read_only_with_backend(backend);
    let after_open = check(deadline, cancellation);
    match opened {
        Ok(database) => {
            if let Err(error) = after_open {
                let _ = close_read_only(database, Ok(()), deadline, cancellation);
                Err(error)
            } else {
                Ok(database)
            }
        }
        Err(error) => {
            after_open?;
            Err(map_database_error(error))
        }
    }
}

fn close_read_only<T>(
    database: redb_plain::ReadOnlyDatabase,
    operation: Result<T, PlainControlError>,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<T, PlainControlError> {
    let before_close = check(deadline, cancellation);
    let closed = database.close().map_err(map_database_error);
    let after_close = check(deadline, cancellation);
    before_close?;
    after_close?;
    let value = operation?;
    closed?;
    Ok(value)
}

fn read_format_metadata(
    database: &redb_plain::ReadOnlyDatabase,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    check(deadline, cancellation)?;
    let transaction = database.begin_read();
    check(deadline, cancellation)?;
    let transaction = transaction.map_err(map_transaction_error)?;

    let result = (|| {
        check(deadline, cancellation)?;
        let opened_table = transaction.open_table(META_TABLE);
        check(deadline, cancellation)?;
        let table = opened_table.map_err(map_table_error)?;

        check(deadline, cancellation)?;
        let stored_version = table.get(FORMAT_VERSION_KEY);
        check(deadline, cancellation)?;
        let stored_version = stored_version.map_err(|error| map_storage_error(&error))?;
        let Some(stored_version) = stored_version else {
            return Err(PlainControlError::Corrupt);
        };
        if stored_version.value() != FORMAT_VERSION {
            return Err(PlainControlError::Unsupported);
        }
        Ok(())
    })();

    drop(transaction);
    result
}

fn read_empty_state(
    database: &redb_plain::ReadOnlyDatabase,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<EmptyControlState, PlainControlError> {
    check(deadline, cancellation)?;
    let transaction = database.begin_read();
    check(deadline, cancellation)?;
    let transaction = transaction.map_err(map_transaction_error)?;

    let result = (|| {
        check(deadline, cancellation)?;
        let listed_tables = transaction.list_tables();
        check(deadline, cancellation)?;
        let listed_tables = listed_tables.map_err(|error| map_storage_error(&error))?;
        let mut regular_table_count = 0_usize;
        let mut only_expected_table = true;
        for handle in listed_tables {
            check(deadline, cancellation)?;
            regular_table_count += 1;
            only_expected_table &= handle.name() == META_TABLE.name();
        }
        check(deadline, cancellation)?;

        check(deadline, cancellation)?;
        let listed_multimaps = transaction.list_multimap_tables();
        check(deadline, cancellation)?;
        let listed_multimaps = listed_multimaps.map_err(|error| map_storage_error(&error))?;
        let has_multimap = listed_multimaps.into_iter().next().is_some();
        check(deadline, cancellation)?;

        if regular_table_count == 0 && !has_multimap {
            return Ok(EmptyControlState::Preformat);
        }
        if regular_table_count != 1 || !only_expected_table || has_multimap {
            return Ok(EmptyControlState::NotEmpty);
        }

        check(deadline, cancellation)?;
        let opened_table = transaction.open_table(META_TABLE);
        check(deadline, cancellation)?;
        let table = opened_table.map_err(map_table_error)?;

        check(deadline, cancellation)?;
        let stored_version = table.get(FORMAT_VERSION_KEY);
        check(deadline, cancellation)?;
        let stored_version = stored_version.map_err(|error| map_storage_error(&error))?;
        let Some(stored_version) = stored_version else {
            return Err(PlainControlError::Corrupt);
        };
        if stored_version.value() != FORMAT_VERSION {
            return Err(PlainControlError::Unsupported);
        }

        check(deadline, cancellation)?;
        let metadata_rows = table.len();
        check(deadline, cancellation)?;
        let metadata_rows = metadata_rows.map_err(|error| map_storage_error(&error))?;

        Ok(if metadata_rows == 1 {
            EmptyControlState::Formatted
        } else {
            EmptyControlState::NotEmpty
        })
    })();

    drop(transaction);
    result
}

fn inspect_open_file(
    file: File,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    let database = open_read_only_file(file, None, deadline, cancellation)?;
    let result = read_format_metadata(&database, deadline, cancellation);
    close_read_only(database, result, deadline, cancellation)
}

fn empty_state_open_file(
    file: File,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<EmptyControlState, PlainControlError> {
    let database = open_read_only_file(file, Some(EMPTY_PROBE_BYTES), deadline, cancellation)?;
    let result = read_empty_state(&database, deadline, cancellation);
    close_read_only(database, result, deadline, cancellation)
}

fn accept_existing_initialization(
    file: File,
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    match empty_state_open_file(file, deadline, cancellation)? {
        EmptyControlState::Formatted => Ok(()),
        EmptyControlState::NotEmpty => Err(PlainControlError::Unsupported),
        EmptyControlState::Preformat => {
            complete_preformat_initialization(path, deadline, cancellation)
        }
    }
}

fn complete_preformat_initialization(
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    check(deadline, cancellation)?;
    let opened = open_initialization_file(path);
    check(deadline, cancellation)?;
    let file = opened?;
    let length = opened_file_len(&file, deadline, cancellation)?;
    if length == 0 {
        return initialize_fresh(file, path, deadline, cancellation);
    }

    // Retain the exact OS file object for the later writer. Its duplicate is
    // used by one read-only database, which is explicitly closed before any
    // writable database is opened. No live database handles share file locks.
    check(deadline, cancellation)?;
    let reader = file.try_clone();
    check(deadline, cancellation)?;
    let reader = reader.map_err(|error| map_io_error(&error))?;
    match empty_state_open_file(reader, deadline, cancellation)? {
        EmptyControlState::Formatted => Ok(()),
        EmptyControlState::NotEmpty => Err(PlainControlError::Unsupported),
        EmptyControlState::Preformat => {
            let proven = ProvenPreformatFile { file, length };
            if opened_file_len(&proven.file, deadline, cancellation)? != proven.length {
                return Err(PlainControlError::Busy);
            }
            initialize_empty_file(proven.file, path, deadline, cancellation)
        }
    }
}

fn create_new_file(path: &Path) -> Result<File, PlainControlError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| map_io_error(&error))
}

fn open_initialization_file(path: &Path) -> Result<File, PlainControlError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                PlainControlError::Busy
            } else {
                map_io_error(&error)
            }
        })
}

fn write_format_metadata(
    database: &redb_plain::Database,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    check(deadline, cancellation)?;
    let transaction = database.begin_write();
    check(deadline, cancellation)?;
    let transaction = transaction.map_err(map_transaction_error)?;

    let has_regular_table = {
        check(deadline, cancellation)?;
        let tables = transaction.list_tables();
        check(deadline, cancellation)?;
        let mut tables = tables.map_err(|error| map_storage_error(&error))?;
        let has_table = tables.next().is_some();
        check(deadline, cancellation)?;
        has_table
    };
    let has_multimap_table = {
        check(deadline, cancellation)?;
        let tables = transaction.list_multimap_tables();
        check(deadline, cancellation)?;
        let mut tables = tables.map_err(|error| map_storage_error(&error))?;
        let has_table = tables.next().is_some();
        check(deadline, cancellation)?;
        has_table
    };
    if has_regular_table || has_multimap_table {
        return Err(PlainControlError::Unsupported);
    }

    check(deadline, cancellation)?;
    let opened_table = transaction.open_table(META_TABLE);
    check(deadline, cancellation)?;
    let mut table = opened_table.map_err(map_table_error)?;

    check(deadline, cancellation)?;
    let inserted = table.insert(FORMAT_VERSION_KEY, FORMAT_VERSION);
    check(deadline, cancellation)?;
    inserted.map_err(|error| map_storage_error(&error))?;
    drop(table);

    check(deadline, cancellation)?;
    let committed = transaction.commit();
    check(deadline, cancellation)?;
    committed.map_err(map_commit_error)
}

fn close_after_initialization(
    database: redb_plain::Database,
    operation: Result<(), PlainControlError>,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    let before_close = check(deadline, cancellation);
    let closed = database.close().map_err(map_database_error);
    let after_close = check(deadline, cancellation);
    before_close?;
    after_close?;
    operation?;
    closed
}

fn initialize_fresh(
    file: File,
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    if opened_file_len(&file, deadline, cancellation)? != 0 {
        return Err(PlainControlError::Busy);
    }
    initialize_empty_file(file, path, deadline, cancellation)
}

fn initialize_empty_file(
    file: File,
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    check(deadline, cancellation)?;
    let backend = FileBackend::new(file);
    check(deadline, cancellation)?;
    let backend = backend.map_err(map_database_error)?;

    check(deadline, cancellation)?;
    let mut builder = redb_plain::Builder::new();
    builder.set_repair_callback(redb_plain::RepairSession::abort);
    let opened_database = builder.create_with_backend(backend);
    let after_open = check(deadline, cancellation);
    let database = match opened_database {
        Ok(database) => database,
        Err(error) => {
            after_open?;
            return Err(map_database_error(error));
        }
    };
    if let Err(error) = after_open {
        let _ = close_after_initialization(database, Ok(()), deadline, cancellation);
        return Err(error);
    }

    let operation = write_format_metadata(&database, deadline, cancellation);
    close_after_initialization(database, operation, deadline, cancellation)?;

    // Reopen only after the writer has closed; this is a read-only format readback.
    inspect_plain_control(path, deadline, cancellation)
}

/// Initialize a fresh plain control database or accept a validated empty retry.
///
/// The caller must hold root exclusion and have published the exact init marker.
/// Missing files are created without overwrite. Existing non-zero files are
/// first classified through redb's bounded read-only probe. A clean preformat
/// database with no regular or multimap tables may finish its format transaction;
/// an already formatted empty database is accepted without rewriting it.
///
/// # Errors
///
/// Returns an error on cancellation, deadline expiry, unavailable or busy storage,
/// corrupt or unsupported metadata, or a retry containing additional data.
/// A retry exceeding the empty-probe size limit returns
/// [`PlainControlError::ProbeLimit`].
pub fn initialize_plain_control(
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    check(deadline, cancellation)?;
    let existing = File::open(path);
    check(deadline, cancellation)?;

    match existing {
        Ok(file) => {
            let file_len = opened_file_len(&file, deadline, cancellation)?;
            if file_len > 0 {
                return accept_existing_initialization(file, path, deadline, cancellation);
            }

            drop(file);
            check(deadline, cancellation)?;
            let reopened = open_initialization_file(path);
            check(deadline, cancellation)?;
            let file = reopened?;
            let file_len = opened_file_len(&file, deadline, cancellation)?;
            if file_len > 0 {
                return accept_existing_initialization(file, path, deadline, cancellation);
            }
            initialize_fresh(file, path, deadline, cancellation)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let created = create_new_file(path);
            check(deadline, cancellation)?;
            let file = created?;
            let file_len = opened_file_len(&file, deadline, cancellation)?;
            if file_len > 0 {
                return accept_existing_initialization(file, path, deadline, cancellation);
            }
            initialize_fresh(file, path, deadline, cancellation)
        }
        Err(error) => Err(map_io_error(&error)),
    }
}

/// Validate the current format marker through a read-only database handle.
///
/// This ordinary current-root check reads only the fixed format table/key. It
/// does not enumerate the table catalogue or impose a whole-file size limit.
///
/// # Errors
///
/// Returns an error on cancellation, deadline expiry, unavailable or busy storage,
/// or corrupt or unsupported format metadata.
pub fn inspect_plain_control(
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), PlainControlError> {
    check(deadline, cancellation)?;
    let opened = File::open(path);
    check(deadline, cancellation)?;
    let file = opened.map_err(|error| map_io_error(&error))?;
    inspect_open_file(file, deadline, cancellation)
}

/// Return whether an owned unfinished database contains no source or user data.
///
/// This bounded probe is for unfinished initialization and discard while the
/// caller holds root exclusion. It limits the actual opened file before redb
/// enumerates its table catalogue; oversized files return
/// [`PlainControlError::ProbeLimit`].
/// A clean database with no regular or multimap tables is preformat empty;
/// a formatted empty database contains only the single current format row.
///
/// # Errors
///
/// Returns an error on cancellation, deadline expiry, unavailable or busy storage,
/// corrupt or unsupported format metadata, or a file exceeding the probe size
/// limit. Additional tables or metadata rows return `Ok(false)`.
pub fn plain_control_is_empty(
    path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<bool, PlainControlError> {
    check(deadline, cancellation)?;
    let opened = File::open(path);
    check(deadline, cancellation)?;
    let file = opened.map_err(|error| map_io_error(&error))?;
    empty_state_open_file(file, deadline, cancellation)
        .map(|state| state != EmptyControlState::NotEmpty)
}
