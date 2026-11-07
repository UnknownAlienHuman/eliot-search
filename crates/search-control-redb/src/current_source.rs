//! Typed, short-transaction current-source rows in the root control database.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::{HashMap, HashSet};
#[cfg(windows)]
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use redb_plain::{
    Database, ReadOnlyDatabase, ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition,
    TableHandle, WriteTransaction,
};
use search_contracts::{Blake3Digest32, SourceId, SourceNamespaceId};
use search_source_registry::{
    CurrentSourceRecord, MAX_SOURCE_OBJECT_BYTES, NativeFileKey, ObjectKind, ObjectRef,
    SOURCE_OBJECT_FORMAT_VERSION,
};

const META_TABLE_NAME: &str = "plain_control_meta";
const SOURCE_STATE_NAME: &str = "source_state";
const SOURCE_ROWS_NAME: &str = "current_source_rows";
const SOURCE_LOCATORS_NAME: &str = "current_source_locators";
pub(super) const SOURCE_SCHEMA_VERSION: &[u8] = &[1];
pub(super) const SCHEMA_VERSION_KEY: &str = "schema_version";
pub(super) const NAMESPACE_KEY: &str = "source_namespace_id";
pub(super) const PREPARATION_TABLE_NAME: &str = "current_preparations";
const WANTED_REVISION_KEY: &str = "wanted_corpus_revision";
const MAX_LOCATOR_BYTES: usize = 32 * 1024;
/// Maximum current sources accepted by one bounded inventory or publication.
pub const MAX_CURRENT_SOURCES: usize = 4096;
const MAX_CURRENT_SOURCE_BYTES: usize = 16 * 1024 * 1024;

pub(super) const SOURCE_STATE: TableDefinition<&str, &[u8]> =
    TableDefinition::new(SOURCE_STATE_NAME);
pub(super) const SOURCE_ROWS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new(SOURCE_ROWS_NAME);
const SOURCE_LOCATORS: TableDefinition<&[u8], &[u8]> = TableDefinition::new(SOURCE_LOCATORS_NAME);

/// Content-free source-control failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceControlError {
    /// The selected locator is not absolute or exceeds the native path bound.
    InvalidLocator,
    /// The current platform has no lossless native locator encoding.
    UnsupportedPlatform,
    /// Current source tables have an unsupported or partial schema.
    UnsupportedSchema,
    /// A current source row or index is corrupt.
    Corrupt,
    /// A captured source registration changed before its row transaction.
    Stale,
    /// The control database failed to open or transact.
    Database,
    /// The operating-system random source failed.
    RandomUnavailable,
    /// A row revision cannot advance without resetting.
    RevisionExhausted,
    /// The bounded current-source inventory exceeded its row or byte ceiling.
    InventoryLimit,
    /// The operation deadline expired.
    Deadline,
    /// Cancellation was observed.
    Cancelled,
}

impl SourceControlError {
    /// Stable content-free machine error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLocator => "SOURCE_LOCATOR_INVALID",
            Self::UnsupportedPlatform => "SOURCE_PLATFORM_UNSUPPORTED",
            Self::UnsupportedSchema => "SOURCE_SCHEMA_UNSUPPORTED",
            Self::Corrupt => "SOURCE_CONTROL_CORRUPT",
            Self::Stale => "SOURCE_STALE",
            Self::Database => "SOURCE_CONTROL_IO",
            Self::RandomUnavailable => "SOURCE_ID_GENERATION_FAILED",
            Self::RevisionExhausted => "SOURCE_REVISION_EXHAUSTED",
            Self::InventoryLimit => "SOURCE_INVENTORY_LIMIT",
            Self::Deadline => "SOURCE_CONTROL_DEADLINE",
            Self::Cancelled => "SOURCE_CONTROL_CANCELLED",
        }
    }
}

/// Read-state guard captured before source acquisition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCapture {
    source_namespace_id: SourceNamespaceId,
    source_id: SourceId,
    locator: Vec<u8>,
    expected_namespace: Option<SourceNamespaceId>,
    expected_row_revision: Option<u64>,
    expected_registration_revision: Option<u64>,
}

fn check(deadline: Instant, cancellation: &AtomicBool) -> Result<(), SourceControlError> {
    if cancellation.load(Ordering::Relaxed) {
        return Err(SourceControlError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(SourceControlError::Deadline);
    }
    Ok(())
}

fn random_id() -> Result<[u8; 16], SourceControlError> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|_| SourceControlError::RandomUnavailable)?;
    Ok(bytes)
}

/// Encodes an absolute native locator using the current-source table format.
///
/// # Errors
/// Returns `InvalidLocator` for a relative, empty, or oversized locator and
/// `UnsupportedPlatform` when native locator encoding is unavailable.
pub fn source_locator_bytes(path: &Path) -> Result<Vec<u8>, SourceControlError> {
    if !path.is_absolute() {
        return Err(SourceControlError::InvalidLocator);
    }
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(windows)]
    let bytes = {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    #[cfg(not(any(unix, windows)))]
    let bytes = return Err(SourceControlError::UnsupportedPlatform);

    if bytes.is_empty() || bytes.len() > MAX_LOCATOR_BYTES {
        return Err(SourceControlError::InvalidLocator);
    }
    Ok(bytes)
}

/// Decodes a locator previously produced by [`source_locator_bytes`].
///
/// # Errors
/// Returns `InvalidLocator` for an empty or oversized encoding, `Corrupt` for
/// malformed or relative paths, and `UnsupportedPlatform` without native decoding.
pub fn source_path_from_locator(locator: &[u8]) -> Result<PathBuf, SourceControlError> {
    if locator.is_empty() || locator.len() > MAX_LOCATOR_BYTES {
        return Err(SourceControlError::InvalidLocator);
    }
    #[cfg(unix)]
    let value = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(locator.to_vec())
    };
    #[cfg(windows)]
    let value = {
        use std::os::windows::ffi::OsStringExt;
        let (units, remainder) = locator.as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(SourceControlError::Corrupt);
        }
        let wide = units
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect::<Vec<_>>();
        OsString::from_wide(&wide)
    };
    #[cfg(not(any(unix, windows)))]
    let value = return Err(SourceControlError::UnsupportedPlatform);

    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(SourceControlError::Corrupt);
    }
    Ok(path)
}

pub(super) fn catalogue_names(
    mut names: Vec<String>,
    has_multimap: bool,
) -> Result<bool, SourceControlError> {
    if has_multimap {
        return Err(SourceControlError::UnsupportedSchema);
    }
    let publication_count = names
        .iter()
        .filter(|name| crate::publication::TABLE_NAMES.contains(&name.as_str()))
        .count();
    if publication_count != 0 && publication_count != crate::publication::TABLE_NAMES.len() {
        return Err(SourceControlError::UnsupportedSchema);
    }
    names.retain(|name| !crate::publication::TABLE_NAMES.contains(&name.as_str()));
    names.sort_unstable();
    if names.len() == 1 && names[0] == META_TABLE_NAME {
        return Ok(false);
    }
    let mut expected = vec![
        META_TABLE_NAME.to_owned(),
        SOURCE_STATE_NAME.to_owned(),
        SOURCE_ROWS_NAME.to_owned(),
        SOURCE_LOCATORS_NAME.to_owned(),
    ];
    expected.sort_unstable();
    if names == expected {
        return Ok(true);
    }
    expected.push(PREPARATION_TABLE_NAME.to_owned());
    expected.sort_unstable();
    if names == expected {
        Ok(true)
    } else {
        Err(SourceControlError::UnsupportedSchema)
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], SourceControlError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(SourceControlError::Corrupt)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(SourceControlError::Corrupt)?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, SourceControlError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, SourceControlError> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| SourceControlError::Corrupt)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, SourceControlError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| SourceControlError::Corrupt)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, SourceControlError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| SourceControlError::Corrupt)?,
        ))
    }
}

pub(super) fn encode_record(record: &CurrentSourceRecord) -> Result<Vec<u8>, SourceControlError> {
    if record.row_revision == 0
        || record.registration_revision == 0
        || record.object_ref.kind != ObjectKind::Source
        || record.object_ref.format_version != SOURCE_OBJECT_FORMAT_VERSION
        || record.locator.is_empty()
        || record.locator.len() > MAX_LOCATOR_BYTES
    {
        return Err(SourceControlError::Corrupt);
    }
    let locator_length =
        u32::try_from(record.locator.len()).map_err(|_| SourceControlError::Corrupt)?;
    let mut bytes = Vec::with_capacity(113 + record.locator.len());
    bytes.push(1);
    bytes.extend_from_slice(record.source_namespace_id.as_bytes());
    bytes.extend_from_slice(record.source_id.as_bytes());
    bytes.extend_from_slice(&record.registration_revision.to_le_bytes());
    bytes.extend_from_slice(&record.row_revision.to_le_bytes());
    bytes.extend_from_slice(&locator_length.to_le_bytes());
    bytes.extend_from_slice(&record.locator);
    bytes.extend_from_slice(&record.file_key.to_bytes());
    bytes.extend_from_slice(&record.object_ref.format_version.to_le_bytes());
    bytes.push(record.object_ref.kind.tag());
    bytes.extend_from_slice(record.object_ref.digest.as_bytes());
    bytes.extend_from_slice(&record.object_ref.byte_length.to_le_bytes());
    Ok(bytes)
}

pub(super) fn decode_record(bytes: &[u8]) -> Result<CurrentSourceRecord, SourceControlError> {
    let mut decoder = Decoder { bytes, cursor: 0 };
    if decoder.u8()? != 1 {
        return Err(SourceControlError::UnsupportedSchema);
    }
    let source_namespace_id = SourceNamespaceId::from_bytes(
        decoder
            .take(16)?
            .try_into()
            .map_err(|_| SourceControlError::Corrupt)?,
    );
    let source_id = SourceId::from_bytes(
        decoder
            .take(16)?
            .try_into()
            .map_err(|_| SourceControlError::Corrupt)?,
    );
    let registration_revision = decoder.u64()?;
    let row_revision = decoder.u64()?;
    let locator_length =
        usize::try_from(decoder.u32()?).map_err(|_| SourceControlError::Corrupt)?;
    let locator = decoder.take(locator_length)?.to_vec();
    let file_key = NativeFileKey::from_bytes(
        decoder
            .take(17)?
            .try_into()
            .map_err(|_| SourceControlError::Corrupt)?,
    )
    .ok_or(SourceControlError::Corrupt)?;
    let format_version = decoder.u16()?;
    let kind = ObjectKind::from_tag(decoder.u8()?).ok_or(SourceControlError::UnsupportedSchema)?;
    let digest = Blake3Digest32::from_bytes(
        decoder
            .take(32)?
            .try_into()
            .map_err(|_| SourceControlError::Corrupt)?,
    );
    let byte_length = decoder.u64()?;
    if decoder.cursor != bytes.len()
        || row_revision == 0
        || registration_revision == 0
        || format_version != SOURCE_OBJECT_FORMAT_VERSION
        || kind != ObjectKind::Source
        || byte_length > MAX_SOURCE_OBJECT_BYTES
    {
        return Err(SourceControlError::Corrupt);
    }
    Ok(CurrentSourceRecord {
        source_namespace_id,
        source_id,
        row_revision,
        registration_revision,
        locator,
        file_key,
        object_ref: ObjectRef {
            format_version,
            kind,
            digest,
            byte_length,
        },
    })
}

pub(super) fn read_namespace(
    transaction: &ReadTransaction,
) -> Result<Option<SourceNamespaceId>, SourceControlError> {
    let table = transaction
        .open_table(SOURCE_STATE)
        .map_err(|_| SourceControlError::Corrupt)?;
    let version = table
        .get(SCHEMA_VERSION_KEY)
        .map_err(|_| SourceControlError::Database)?;
    let namespace = table
        .get(NAMESPACE_KEY)
        .map_err(|_| SourceControlError::Database)?;
    match (version, namespace) {
        (None, None) => Ok(None),
        (Some(version), Some(namespace)) if version.value() == SOURCE_SCHEMA_VERSION => {
            Ok(Some(SourceNamespaceId::from_bytes(
                namespace
                    .value()
                    .try_into()
                    .map_err(|_| SourceControlError::Corrupt)?,
            )))
        }
        _ => Err(SourceControlError::Corrupt),
    }
}

pub(super) fn read_source_by_id(
    transaction: &ReadTransaction,
    id: SourceId,
) -> Result<Option<CurrentSourceRecord>, SourceControlError> {
    let rows = transaction
        .open_table(SOURCE_ROWS)
        .map_err(|_| SourceControlError::Corrupt)?;
    let value = rows
        .get(id.as_bytes().as_slice())
        .map_err(|_| SourceControlError::Database)?;
    let Some(value) = value else { return Ok(None) };
    let source = decode_record(value.value())?;
    if source.source_id != id {
        return Err(SourceControlError::Corrupt);
    }
    Ok(Some(source))
}

pub(super) fn wanted_revision(transaction: &ReadTransaction) -> Result<u64, SourceControlError> {
    let table = transaction
        .open_table(SOURCE_STATE)
        .map_err(|_| SourceControlError::Corrupt)?;
    let value = table
        .get(WANTED_REVISION_KEY)
        .map_err(|_| SourceControlError::Database)?;
    value.map_or(Ok(0), |v| {
        Ok(u64::from_le_bytes(
            v.value()
                .try_into()
                .map_err(|_| SourceControlError::Corrupt)?,
        ))
    })
}

pub(super) fn write_wanted_revision(
    transaction: &WriteTransaction,
) -> Result<u64, SourceControlError> {
    let table = transaction
        .open_table(SOURCE_STATE)
        .map_err(|_| SourceControlError::Database)?;
    let value = table
        .get(WANTED_REVISION_KEY)
        .map_err(|_| SourceControlError::Database)?;
    value.map_or(Ok(0), |v| {
        Ok(u64::from_le_bytes(
            v.value()
                .try_into()
                .map_err(|_| SourceControlError::Corrupt)?,
        ))
    })
}

pub(super) fn advance_wanted_revision(
    transaction: &WriteTransaction,
) -> Result<(), SourceControlError> {
    let next = write_wanted_revision(transaction)?
        .checked_add(1)
        .ok_or(SourceControlError::RevisionExhausted)?;
    transaction
        .open_table(SOURCE_STATE)
        .map_err(|_| SourceControlError::Database)?
        .insert(WANTED_REVISION_KEY, next.to_le_bytes().as_slice())
        .map_err(|_| SourceControlError::Database)?;
    Ok(())
}

fn load_locator_row(
    transaction: &ReadTransaction,
    locator: &[u8],
) -> Result<Option<CurrentSourceRecord>, SourceControlError> {
    let locator_table = transaction
        .open_table(SOURCE_LOCATORS)
        .map_err(|_| SourceControlError::Corrupt)?;
    let Some(source_id) = locator_table
        .get(locator)
        .map_err(|_| SourceControlError::Database)?
    else {
        return Ok(None);
    };
    let source_id: [u8; 16] = source_id
        .value()
        .try_into()
        .map_err(|_| SourceControlError::Corrupt)?;
    let row_table = transaction
        .open_table(SOURCE_ROWS)
        .map_err(|_| SourceControlError::Corrupt)?;
    let row = row_table
        .get(source_id.as_slice())
        .map_err(|_| SourceControlError::Database)?
        .ok_or(SourceControlError::Corrupt)?;
    let record = decode_record(row.value())?;
    if record.source_id.as_bytes() != &source_id || record.locator != locator {
        return Err(SourceControlError::Corrupt);
    }
    Ok(Some(record))
}

/// Captures the current registration guard before external file I/O.
///
/// # Errors
/// Returns an error for invalid locators, unsupported/corrupt state, random-source failure or I/O.
pub fn capture_current_source(
    control_path: &Path,
    selected_file: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<SourceCapture, SourceControlError> {
    check(deadline, cancellation)?;
    let locator = source_locator_bytes(selected_file)?;
    let database =
        ReadOnlyDatabase::open(control_path).map_err(|_| SourceControlError::Database)?;
    let transaction = database
        .begin_read()
        .map_err(|_| SourceControlError::Database)?;
    check(deadline, cancellation)?;
    let listed_tables = transaction
        .list_tables()
        .map_err(|_| SourceControlError::Database)?;
    let names = listed_tables
        .map(|table| table.name().to_owned())
        .collect::<Vec<_>>();
    let listed_multimaps = transaction
        .list_multimap_tables()
        .map_err(|_| SourceControlError::Database)?;
    let is_current_schema = catalogue_names(names, listed_multimaps.into_iter().next().is_some())?;
    if !is_current_schema {
        let source_namespace_id = SourceNamespaceId::from_bytes(random_id()?);
        let source_id = SourceId::from_bytes(random_id()?);
        drop(transaction);
        drop(database);
        return Ok(SourceCapture {
            source_namespace_id,
            source_id,
            locator,
            expected_namespace: None,
            expected_row_revision: None,
            expected_registration_revision: None,
        });
    }
    let namespace_id = read_namespace(&transaction)?.ok_or(SourceControlError::Corrupt)?;
    let record = load_locator_row(&transaction, &locator)?;
    check(deadline, cancellation)?;
    let (source_id, expected_row_revision, expected_registration_revision) = match record {
        Some(record) => {
            if record.source_namespace_id != namespace_id {
                return Err(SourceControlError::Corrupt);
            }
            (
                record.source_id,
                Some(record.row_revision),
                Some(record.registration_revision),
            )
        }
        None => (SourceId::from_bytes(random_id()?), None, None),
    };
    drop(transaction);
    drop(database);
    Ok(SourceCapture {
        source_namespace_id: namespace_id,
        source_id,
        locator,
        expected_namespace: Some(namespace_id),
        expected_row_revision,
        expected_registration_revision,
    })
}

/// Captures every current source and validates its reverse locator binding in one read snapshot.
///
/// # Errors
/// Refuses unsupported schemas, corrupt bindings, inventory limits, cancellation,
/// deadline expiry, and database failures without writing.
pub fn capture_current_sources(
    control_path: &Path,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<Vec<CurrentSourceRecord>, SourceControlError> {
    check(deadline, cancellation)?;
    let database =
        ReadOnlyDatabase::open(control_path).map_err(|_| SourceControlError::Database)?;
    let transaction = database
        .begin_read()
        .map_err(|_| SourceControlError::Database)?;
    let names = transaction
        .list_tables()
        .map_err(|_| SourceControlError::Database)?
        .map(|table| table.name().to_owned())
        .collect::<Vec<_>>();
    let has_multimap = transaction
        .list_multimap_tables()
        .map_err(|_| SourceControlError::Database)?
        .next()
        .is_some();
    if !catalogue_names(names, has_multimap)? {
        return Ok(Vec::new());
    }

    let namespace = read_namespace(&transaction)?.ok_or(SourceControlError::Corrupt)?;
    let rows = transaction
        .open_table(SOURCE_ROWS)
        .map_err(|_| SourceControlError::Corrupt)?;
    let locators = transaction
        .open_table(SOURCE_LOCATORS)
        .map_err(|_| SourceControlError::Corrupt)?;
    let mut sources = Vec::new();
    let mut reverse = HashMap::new();
    let mut charged = 0_usize;
    for row in rows.iter().map_err(|_| SourceControlError::Database)? {
        check(deadline, cancellation)?;
        let (id, bytes) = row.map_err(|_| SourceControlError::Database)?;
        charged = charged
            .checked_add(bytes.value().len())
            .ok_or(SourceControlError::InventoryLimit)?;
        if sources.len() >= MAX_CURRENT_SOURCES || charged > MAX_CURRENT_SOURCE_BYTES {
            return Err(SourceControlError::InventoryLimit);
        }
        let source = decode_record(bytes.value())?;
        if id.value() != source.source_id.as_bytes() || source.source_namespace_id != namespace {
            return Err(SourceControlError::Corrupt);
        }
        let indexed = locators
            .get(source.locator.as_slice())
            .map_err(|_| SourceControlError::Database)?
            .ok_or(SourceControlError::Corrupt)?;
        if indexed.value() != source.source_id.as_bytes() {
            return Err(SourceControlError::Corrupt);
        }
        charged = charged
            .checked_add(source.locator.len())
            .ok_or(SourceControlError::InventoryLimit)?;
        if charged > MAX_CURRENT_SOURCE_BYTES {
            return Err(SourceControlError::InventoryLimit);
        }
        reverse.insert(source.source_id.as_bytes().to_vec(), source.locator.clone());
        sources
            .try_reserve(1)
            .map_err(|_| SourceControlError::InventoryLimit)?;
        sources.push(source);
    }

    let mut locator_count = 0_usize;
    for item in locators.iter().map_err(|_| SourceControlError::Database)? {
        check(deadline, cancellation)?;
        let (locator, id) = item.map_err(|_| SourceControlError::Database)?;
        locator_count = locator_count
            .checked_add(1)
            .ok_or(SourceControlError::InventoryLimit)?;
        if locator_count > MAX_CURRENT_SOURCES {
            return Err(SourceControlError::InventoryLimit);
        }
        let id = id.value();
        if reverse.remove(id).as_deref() != Some(locator.value()) {
            return Err(SourceControlError::Corrupt);
        }
    }
    if locator_count != sources.len() || !reverse.is_empty() {
        return Err(SourceControlError::Corrupt);
    }
    check(deadline, cancellation)?;
    Ok(sources)
}

fn source_revisions(
    transaction: &WriteTransaction,
    capture: &SourceCapture,
) -> Result<(u64, u64), SourceControlError> {
    let state = transaction
        .open_table(SOURCE_STATE)
        .map_err(|_| SourceControlError::Database)?;
    let rows = transaction
        .open_table(SOURCE_ROWS)
        .map_err(|_| SourceControlError::Database)?;
    let locators = transaction
        .open_table(SOURCE_LOCATORS)
        .map_err(|_| SourceControlError::Database)?;

    let stored_version = state
        .get(SCHEMA_VERSION_KEY)
        .map_err(|_| SourceControlError::Database)?;
    let stored_namespace = state
        .get(NAMESPACE_KEY)
        .map_err(|_| SourceControlError::Database)?;
    match (capture.expected_namespace, stored_version, stored_namespace) {
        (None, None, None) => {}
        (Some(expected), Some(version), Some(namespace))
            if version.value() == SOURCE_SCHEMA_VERSION
                && namespace.value() == expected.as_bytes() => {}
        _ => return Err(SourceControlError::Stale),
    }

    let existing_locator = locators
        .get(capture.locator.as_slice())
        .map_err(|_| SourceControlError::Database)?;
    match (
        capture.expected_row_revision,
        capture.expected_registration_revision,
        existing_locator,
    ) {
        (None, None, None) => {
            if rows
                .get(capture.source_id.as_bytes().as_slice())
                .map_err(|_| SourceControlError::Database)?
                .is_some()
            {
                return Err(SourceControlError::Stale);
            }
            Ok((1, 1))
        }
        (Some(expected_row), Some(expected_registration), Some(source_id))
            if source_id.value() == capture.source_id.as_bytes() =>
        {
            let stored = rows
                .get(capture.source_id.as_bytes().as_slice())
                .map_err(|_| SourceControlError::Database)?
                .ok_or(SourceControlError::Stale)?;
            let current = decode_record(stored.value())?;
            if current.source_namespace_id != capture.source_namespace_id
                || current.source_id != capture.source_id
                || current.row_revision != expected_row
                || current.registration_revision != expected_registration
                || current.locator != capture.locator
            {
                return Err(SourceControlError::Stale);
            }
            Ok((
                expected_row
                    .checked_add(1)
                    .ok_or(SourceControlError::RevisionExhausted)?,
                expected_registration,
            ))
        }
        _ => Err(SourceControlError::Stale),
    }
}

/// Commits one exact, already-retained source head with an atomic row/registration CAS.
///
/// # Errors
/// Returns Stale for changed guards, or an explicit encoding, revision, cancellation or I/O error.
pub fn commit_current_source(
    control_path: &Path,
    capture: SourceCapture,
    file_key: NativeFileKey,
    object_ref: ObjectRef,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<CurrentSourceRecord, SourceControlError> {
    check(deadline, cancellation)?;
    let database = Database::open(control_path).map_err(|_| SourceControlError::Database)?;
    let transaction = database
        .begin_write()
        .map_err(|_| SourceControlError::Database)?;
    check(deadline, cancellation)?;
    let (row_revision, registration_revision) = source_revisions(&transaction, &capture)?;
    let mut state = transaction
        .open_table(SOURCE_STATE)
        .map_err(|_| SourceControlError::Database)?;
    let mut rows = transaction
        .open_table(SOURCE_ROWS)
        .map_err(|_| SourceControlError::Database)?;
    let mut locators = transaction
        .open_table(SOURCE_LOCATORS)
        .map_err(|_| SourceControlError::Database)?;

    if capture.expected_row_revision.is_some() {
        let current = rows
            .get(capture.source_id.as_bytes().as_slice())
            .map_err(|_| SourceControlError::Database)?
            .ok_or(SourceControlError::Stale)?;
        let current = decode_record(current.value())?;
        if current.object_ref == object_ref && current.file_key == file_key {
            return Ok(current);
        }
    }

    let record = CurrentSourceRecord {
        source_namespace_id: capture.source_namespace_id,
        source_id: capture.source_id,
        row_revision,
        registration_revision,
        locator: capture.locator,
        file_key,
        object_ref,
    };
    let encoded = encode_record(&record)?;
    if capture.expected_namespace.is_none() {
        state
            .insert(SCHEMA_VERSION_KEY, SOURCE_SCHEMA_VERSION)
            .map_err(|_| SourceControlError::Database)?;
        state
            .insert(
                NAMESPACE_KEY,
                capture.source_namespace_id.as_bytes().as_slice(),
            )
            .map_err(|_| SourceControlError::Database)?;
    }
    rows.insert(capture.source_id.as_bytes().as_slice(), encoded.as_slice())
        .map_err(|_| SourceControlError::Database)?;
    locators
        .insert(
            record.locator.as_slice(),
            capture.source_id.as_bytes().as_slice(),
        )
        .map_err(|_| SourceControlError::Database)?;
    drop(locators);
    drop(rows);
    drop(state);
    advance_wanted_revision(&transaction)?;
    check(deadline, cancellation)?;
    if transaction.commit().is_ok() {
        Ok(record)
    } else {
        drop(database);
        match load_current_source(control_path, &record.locator, deadline, cancellation) {
            Ok(Some(observed)) if observed == record => Ok(record),
            Ok(Some(_)) => Err(SourceControlError::Stale),
            Ok(None) | Err(_) => Err(SourceControlError::Database),
        }
    }
}

/// Removes captured source rows, locator bindings, and preparation outcomes atomically.
///
/// # Errors
/// Returns `Stale` for changed registrations or duplicate selections, or a limit,
/// revision, corruption, cancellation, deadline, or database error.
pub fn remove_current_sources(
    control_path: &Path,
    expected: &[CurrentSourceRecord],
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), SourceControlError> {
    if expected.is_empty() {
        return Ok(());
    }
    if expected.len() > MAX_CURRENT_SOURCES {
        return Err(SourceControlError::InventoryLimit);
    }
    check(deadline, cancellation)?;
    let database = Database::open(control_path).map_err(|_| SourceControlError::Database)?;
    let transaction = database
        .begin_write()
        .map_err(|_| SourceControlError::Database)?;
    check(deadline, cancellation)?;

    let state = transaction
        .open_table(SOURCE_STATE)
        .map_err(|_| SourceControlError::Database)?;
    let namespace = {
        let version = state
            .get(SCHEMA_VERSION_KEY)
            .map_err(|_| SourceControlError::Database)?
            .ok_or(SourceControlError::Stale)?;
        let namespace = state
            .get(NAMESPACE_KEY)
            .map_err(|_| SourceControlError::Database)?
            .ok_or(SourceControlError::Stale)?;
        if version.value() != SOURCE_SCHEMA_VERSION {
            return Err(SourceControlError::Stale);
        }
        SourceNamespaceId::from_bytes(
            namespace
                .value()
                .try_into()
                .map_err(|_| SourceControlError::Corrupt)?,
        )
    };
    let mut rows = transaction
        .open_table(SOURCE_ROWS)
        .map_err(|_| SourceControlError::Database)?;
    let mut locators = transaction
        .open_table(SOURCE_LOCATORS)
        .map_err(|_| SourceControlError::Database)?;
    let mut ids = HashSet::new();
    let mut locator_keys = HashSet::new();
    for source in expected {
        check(deadline, cancellation)?;
        if source.source_namespace_id != namespace
            || !ids.insert(source.source_id.as_bytes().to_vec())
            || !locator_keys.insert(source.locator.clone())
        {
            return Err(SourceControlError::Stale);
        }
        let row = rows
            .get(source.source_id.as_bytes().as_slice())
            .map_err(|_| SourceControlError::Database)?
            .ok_or(SourceControlError::Stale)?;
        if decode_record(row.value())? != *source {
            return Err(SourceControlError::Stale);
        }
        let locator = locators
            .get(source.locator.as_slice())
            .map_err(|_| SourceControlError::Database)?
            .ok_or(SourceControlError::Stale)?;
        if locator.value() != source.source_id.as_bytes() {
            return Err(SourceControlError::Stale);
        }
    }

    for source in expected {
        check(deadline, cancellation)?;
        if rows
            .remove(source.source_id.as_bytes().as_slice())
            .map_err(|_| SourceControlError::Database)?
            .is_none()
            || locators
                .remove(source.locator.as_slice())
                .map_err(|_| SourceControlError::Database)?
                .is_none()
        {
            return Err(SourceControlError::Stale);
        }
        crate::current_preparation::remove_source_preparation(&transaction, source.source_id)?;
    }
    drop(locators);
    drop(rows);
    drop(state);
    advance_wanted_revision(&transaction)?;
    check(deadline, cancellation)?;
    if transaction.commit().is_ok() {
        return Ok(());
    }

    drop(database);
    let observed = capture_current_sources(control_path, deadline, cancellation)?;
    if expected.iter().any(|removed| {
        observed.iter().any(|current| {
            current.source_id == removed.source_id || current.locator == removed.locator
        })
    }) {
        Err(SourceControlError::Stale)
    } else {
        Ok(())
    }
}

fn load_current_source(
    control_path: &Path,
    locator: &[u8],
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<Option<CurrentSourceRecord>, SourceControlError> {
    check(deadline, cancellation)?;
    let database =
        ReadOnlyDatabase::open(control_path).map_err(|_| SourceControlError::Database)?;
    let transaction = database
        .begin_read()
        .map_err(|_| SourceControlError::Database)?;
    let listed_tables = transaction
        .list_tables()
        .map_err(|_| SourceControlError::Database)?;
    let names = listed_tables
        .map(|table| table.name().to_owned())
        .collect::<Vec<_>>();
    let listed_multimaps = transaction
        .list_multimap_tables()
        .map_err(|_| SourceControlError::Database)?;
    if !catalogue_names(names, listed_multimaps.into_iter().next().is_some())? {
        return Ok(None);
    }
    let record = load_locator_row(&transaction, locator)?;
    check(deadline, cancellation)?;
    Ok(record)
}
