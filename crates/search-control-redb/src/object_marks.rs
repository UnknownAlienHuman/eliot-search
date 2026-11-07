//! Disposable disk-backed marks from one supported consistent CONTROL snapshot.

use std::fs::{self, OpenOptions};
use std::ops::Bound;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use redb_plain::backends::FileBackend;
use redb_plain::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition, TableHandle, WriteTransaction,
};
use search_contracts::Blake3Digest32;
use search_source_registry::{MAX_SOURCE_OBJECT_BYTES, ObjectKind, ObjectRef, PreparationSettings};

const MARKS: TableDefinition<&[u8], u64> = TableDefinition::new("object_marks");
const META: TableDefinition<&str, u64> = TableDefinition::new("plain_control_meta");
const HEX: &[u8; 16] = b"0123456789abcdef";
const PREPARED: TableDefinition<&[u8], &[u8]> = TableDefinition::new("prepared_inputs");
// Scratch bookkeeping only, matching the existing bounded publication-row batch
// granularity. This cadence never limits or omits marked objects.
const CHECKPOINT_COMMITS: u8 = 32;

/// Source binding known from typed CONTROL rows, or inspected in a rooted artifact.
/// Registration/settings may be absent only when cleanup removed their original source row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedExpectation {
    /// Original source namespace.
    pub source_namespace_id: [u8; 16],
    /// Original logical source identity.
    pub source_id: [u8; 16],
    /// Exact retained input, including stale/Gap inputs.
    pub input: ObjectRef,
    /// Original registration and output settings when their row survives.
    pub revision_settings: Option<(u64, PreparationSettings)>,
}

impl PreparedExpectation {
    fn bytes(self) -> Result<Vec<u8>, MarkError> {
        if self.input.kind != ObjectKind::Source {
            return Err(MarkError::Corrupt);
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(96).map_err(|_| MarkError::Limit)?;
        bytes.extend_from_slice(&self.source_namespace_id);
        bytes.extend_from_slice(&self.source_id);
        bytes.extend_from_slice(&identity(self.input)?);
        bytes.extend_from_slice(&self.input.byte_length.to_le_bytes());
        bytes.push(u8::from(self.revision_settings.is_some()));
        if let Some((revision, settings)) = self.revision_settings {
            if revision == 0 || !settings.is_supported() {
                return Err(MarkError::Corrupt);
            }
            bytes.extend_from_slice(&revision.to_le_bytes());
            bytes.extend_from_slice(&settings.utf8_version.to_le_bytes());
            bytes.extend_from_slice(&settings.line_unitizer_version.to_le_bytes());
            bytes.extend_from_slice(&settings.preferred_unit_bytes.to_le_bytes());
            bytes.extend_from_slice(&settings.max_unit_bytes.to_le_bytes());
        }
        Ok(bytes)
    }

    fn read(bytes: &[u8]) -> Result<Self, MarkError> {
        if bytes.len() != 76 && bytes.len() != 96 {
            return Err(MarkError::Corrupt);
        }
        let input = reference(
            &bytes[32..67],
            u64::from_le_bytes(bytes[67..75].try_into().map_err(|_| MarkError::Corrupt)?),
        )?;
        let revision_settings = match (bytes[75], bytes.len()) {
            (0, 76) => None,
            (1, 96) => Some((
                u64::from_le_bytes(bytes[76..84].try_into().map_err(|_| MarkError::Corrupt)?),
                PreparationSettings {
                    utf8_version: u16::from_le_bytes(
                        bytes[84..86].try_into().map_err(|_| MarkError::Corrupt)?,
                    ),
                    line_unitizer_version: u16::from_le_bytes(
                        bytes[86..88].try_into().map_err(|_| MarkError::Corrupt)?,
                    ),
                    preferred_unit_bytes: u32::from_le_bytes(
                        bytes[88..92].try_into().map_err(|_| MarkError::Corrupt)?,
                    ),
                    max_unit_bytes: u32::from_le_bytes(
                        bytes[92..96].try_into().map_err(|_| MarkError::Corrupt)?,
                    ),
                },
            )),
            _ => return Err(MarkError::Corrupt),
        };
        let value = Self {
            source_namespace_id: bytes[..16].try_into().map_err(|_| MarkError::Corrupt)?,
            source_id: bytes[16..32].try_into().map_err(|_| MarkError::Corrupt)?,
            input,
            revision_settings,
        };
        value.bytes()?;
        Ok(value)
    }

    fn compatible(self, other: Self) -> bool {
        self.source_namespace_id == other.source_namespace_id
            && self.source_id == other.source_id
            && self.input == other.input
            && (self.revision_settings.is_none()
                || other.revision_settings.is_none()
                || self.revision_settings == other.revision_settings)
    }
}

/// Closed failures which never authorize a partial mark's sweep.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkError {
    /// Unsupported table layout, format or retained savepoints.
    Unsupported,
    /// Malformed rows, bindings or graph progress.
    Corrupt,
    /// One integrity address has conflicting lengths.
    ConflictingReference,
    /// Existing typed owner bounds or checked arithmetic were exceeded.
    Limit,
    /// Database access or commit failed.
    Database,
    /// Owned scratch file I/O failed.
    Io,
    /// Scratch identity randomness was unavailable.
    Random,
    /// Original deadline expired.
    Deadline,
    /// Cancellation was requested.
    Cancelled,
}

impl MarkError {
    /// Stable content-free failure code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unsupported => "COLLECTION_CONTROL_UNSUPPORTED",
            Self::Corrupt => "COLLECTION_CONTROL_CORRUPT",
            Self::ConflictingReference => "COLLECTION_REFERENCE_CONFLICT",
            Self::Limit => "COLLECTION_INPUT_LIMIT",
            Self::Database => "COLLECTION_CONTROL_IO",
            Self::Io => "COLLECTION_SCRATCH_IO",
            Self::Random => "COLLECTION_SCRATCH_RANDOM",
            Self::Deadline => "COLLECTION_DEADLINE",
            Self::Cancelled => "COLLECTION_CANCELLED",
        }
    }
}

pub(super) fn check(deadline: Instant, cancel: &AtomicBool) -> Result<(), MarkError> {
    if cancel.load(Ordering::Relaxed) {
        Err(MarkError::Cancelled)
    } else if Instant::now() >= deadline {
        Err(MarkError::Deadline)
    } else {
        Ok(())
    }
}

fn identity(reference: ObjectRef) -> Result<[u8; 35], MarkError> {
    let maximum = match reference.kind {
        ObjectKind::Source => MAX_SOURCE_OBJECT_BYTES,
        ObjectKind::Prepared => 64 * 1024 * 1024,
    };
    if reference.format_version != 1
        || reference.byte_length > maximum
        || (reference.kind == ObjectKind::Prepared && reference.byte_length == 0)
    {
        return Err(MarkError::Corrupt);
    }
    let mut key = [0; 35];
    key[..2].copy_from_slice(&reference.format_version.to_le_bytes());
    key[2] = reference.kind.tag();
    key[3..].copy_from_slice(reference.digest.as_bytes());
    Ok(key)
}

fn reference(key: &[u8], length: u64) -> Result<ObjectRef, MarkError> {
    if key.len() != 35 {
        return Err(MarkError::Corrupt);
    }
    let value = ObjectRef {
        format_version: u16::from_le_bytes(key[..2].try_into().map_err(|_| MarkError::Corrupt)?),
        kind: ObjectKind::from_tag(key[2]).ok_or(MarkError::Corrupt)?,
        digest: Blake3Digest32::from_bytes(key[3..].try_into().map_err(|_| MarkError::Corrupt)?),
        byte_length: length,
    };
    identity(value)?;
    Ok(value)
}

/// An owned temporary typed index, complete only after all snapshot owners accepted their rows.
/// It stores no object bodies and is never an installation database or durable GC receipt.
pub struct ObjectMarks {
    database: Option<Database>,
    path: Option<PathBuf>,
    metadata_bytes: u64,
    marked: u64,
    pending_commits: u8,
    served: Option<crate::publication::Generation>,
}

impl Drop for ObjectMarks {
    fn drop(&mut self) {
        if let Some(database) = self.database.take() {
            let _ = database.close();
        }
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

impl ObjectMarks {
    /// Builds a complete mark using exactly one CONTROL read transaction.
    /// The caller retains exclusive stopped-root ownership throughout verification and sweep.
    ///
    /// # Errors
    /// Refuses unknown/corrupt/incomplete metadata, retained savepoints, conflicting refs,
    /// owner limits, scratch I/O, cancellation and the original deadline before returning a mark.
    pub fn build(
        control: &Path,
        scratch_dir: &Path,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Self, MarkError> {
        check(deadline, cancel)?;
        let mut marks = Self::scratch(control, scratch_dir, deadline, cancel)?;
        let database = ReadOnlyDatabase::open(control).map_err(|_| MarkError::Database)?;
        if database
            .has_persistent_savepoints()
            .map_err(|_| MarkError::Database)?
        {
            return Err(MarkError::Unsupported);
        }
        let tx = database.begin_read().map_err(|_| MarkError::Database)?;
        let result = (|| {
            let mut names = Vec::new();
            for table in tx.list_tables().map_err(|_| MarkError::Database)? {
                check(deadline, cancel)?;
                if names.len() >= 5 + crate::publication::TABLE_NAMES.len() {
                    return Err(MarkError::Unsupported);
                }
                names.push(table.name().to_owned());
            }
            let multimaps = tx
                .list_multimap_tables()
                .map_err(|_| MarkError::Database)?
                .next()
                .is_some();
            crate::current_source::catalogue_names(names, multimaps)
                .map_err(|_| MarkError::Unsupported)?;
            let meta = tx.open_table(META).map_err(|_| MarkError::Corrupt)?;
            if meta.len().map_err(|_| MarkError::Database)? != 1
                || meta
                    .get("format_version")
                    .map_err(|_| MarkError::Database)?
                    .is_none_or(|value| value.value() != 1)
            {
                return Err(MarkError::Unsupported);
            }
            drop(meta);
            crate::current_source::mark_objects(&tx, &mut marks, deadline, cancel)?;
            crate::current_preparation::mark_objects(&tx, &mut marks, deadline, cancel)?;
            crate::publication::mark_objects(&tx, &mut marks, deadline, cancel)?;
            check(deadline, cancel)
        })();
        drop(tx);
        let closed = database.close().map_err(|_| MarkError::Database);
        result?;
        closed?;
        Ok(marks)
    }

    fn scratch(
        control: &Path,
        directory: &Path,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Self, MarkError> {
        let root = control
            .parent()
            .and_then(Path::parent)
            .ok_or(MarkError::Unsupported)?
            .canonicalize()
            .map_err(|_| MarkError::Io)?;
        let directory = directory.canonicalize().map_err(|_| MarkError::Io)?;
        if directory.starts_with(&root) {
            return Err(MarkError::Unsupported);
        }
        let mut random = [0_u8; 16];
        getrandom::getrandom(&mut random).map_err(|_| MarkError::Random)?;
        let mut name = String::new();
        name.try_reserve_exact(32).map_err(|_| MarkError::Limit)?;
        for byte in random {
            name.push(char::from(HEX[usize::from(byte >> 4)]));
            name.push(char::from(HEX[usize::from(byte & 15)]));
        }
        let path = directory.join(format!(".eliot-collect-{name}.redb"));
        check(deadline, cancel)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| MarkError::Io)?;
        let mut marks = Self {
            database: None,
            path: Some(path),
            metadata_bytes: 0,
            marked: 0,
            pending_commits: 0,
            served: None,
        };
        let backend = FileBackend::new(file).map_err(|_| MarkError::Database)?;
        let mut builder = redb_plain::Builder::new();
        builder.set_cache_size(crate::current_source::MAX_CURRENT_SOURCE_BYTES);
        marks.database = Some(
            builder
                .create_with_backend(backend)
                .map_err(|_| MarkError::Database)?,
        );
        let tx = marks
            .database()?
            .begin_write()
            .map_err(|_| MarkError::Database)?;
        drop(tx.open_table(MARKS).map_err(|_| MarkError::Database)?);
        drop(tx.open_table(PREPARED).map_err(|_| MarkError::Database)?);
        marks.commit(tx, deadline, cancel)?;
        Ok(marks)
    }

    fn database(&self) -> Result<&Database, MarkError> {
        self.database.as_ref().ok_or(MarkError::Database)
    }

    fn commit(
        &mut self,
        mut tx: WriteTransaction,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<(), MarkError> {
        check(deadline, cancel)?;
        let checkpoint = self.pending_commits == CHECKPOINT_COMMITS - 1;
        tx.set_durability(if checkpoint {
            Durability::Immediate
        } else {
            Durability::None
        })
        .map_err(|_| MarkError::Database)?;
        tx.commit().map_err(|_| MarkError::Database)?;
        // Count every successful scratch commit, including binding rewrites and
        // schema creation. redb releases pending non-durable bookkeeping on the
        // Immediate commit; aborted/no-op transactions consume no batch slot.
        self.pending_commits = if checkpoint {
            0
        } else {
            self.pending_commits + 1
        };
        check(deadline, cancel)
    }

    pub(super) fn charge(&mut self, bytes: usize) -> Result<(), MarkError> {
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(u64::try_from(bytes).map_err(|_| MarkError::Limit)?)
            .ok_or(MarkError::Limit)?;
        Ok(())
    }

    pub(super) fn insert(
        &mut self,
        reference: ObjectRef,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<(), MarkError> {
        check(deadline, cancel)?;
        let key = identity(reference)?;
        let tx = self
            .database()?
            .begin_write()
            .map_err(|_| MarkError::Database)?;
        let mut table = tx.open_table(MARKS).map_err(|_| MarkError::Database)?;
        let existing = table
            .get(key.as_slice())
            .map_err(|_| MarkError::Database)?
            .map(|value| value.value());
        if let Some(length) = existing {
            return if length == reference.byte_length {
                Ok(())
            } else {
                Err(MarkError::ConflictingReference)
            };
        }
        let count = self.marked.checked_add(1).ok_or(MarkError::Limit)?;
        // Logical scratch size is bounded by validated reference-bearing input,
        // rather than an invented global number of objects.
        if count
            .checked_mul(43)
            .is_none_or(|bytes| bytes > self.metadata_bytes)
        {
            return Err(MarkError::Limit);
        }
        table
            .insert(key.as_slice(), reference.byte_length)
            .map_err(|_| MarkError::Database)?;
        drop(table);
        check(deadline, cancel)?;
        self.commit(tx, deadline, cancel)?;
        self.marked = count;
        Ok(())
    }

    pub(super) fn prepared(
        &mut self,
        artifact: ObjectRef,
        expected: PreparedExpectation,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<(), MarkError> {
        if artifact.kind != ObjectKind::Prepared {
            return Err(MarkError::Corrupt);
        }
        self.insert(expected.input, deadline, cancel)?;
        self.insert(artifact, deadline, cancel)?;
        let key = identity(artifact)?;
        let tx = self
            .database()?
            .begin_write()
            .map_err(|_| MarkError::Database)?;
        let mut table = tx.open_table(PREPARED).map_err(|_| MarkError::Database)?;
        let previous = table
            .get(key.as_slice())
            .map_err(|_| MarkError::Database)?
            .map(|value| PreparedExpectation::read(value.value()))
            .transpose()?;
        let expected = if let Some(previous) = previous {
            if !previous.compatible(expected) {
                return Err(MarkError::ConflictingReference);
            }
            if previous.revision_settings.is_some() {
                previous
            } else {
                expected
            }
        } else {
            expected
        };
        let bytes = expected.bytes()?;
        table
            .insert(key.as_slice(), bytes.as_slice())
            .map_err(|_| MarkError::Database)?;
        drop(table);
        check(deadline, cancel)?;
        self.commit(tx, deadline, cancel)
    }

    /// Includes a verified rooted artifact's Source leaf in the mark closure.
    ///
    /// # Errors
    /// Refuses an unmarked artifact or any contradiction with surviving CONTROL bindings,
    /// scratch corruption/I/O, limits, cancellation and the original deadline.
    pub fn include_prepared_input(
        &mut self,
        artifact: ObjectRef,
        actual: PreparedExpectation,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<(), MarkError> {
        if artifact.kind != ObjectKind::Prepared
            || actual.input.kind != ObjectKind::Source
            || actual.revision_settings.is_none()
            || !self.contains(artifact, deadline, cancel)?
        {
            return Err(MarkError::Corrupt);
        }
        let tx = self
            .database()?
            .begin_read()
            .map_err(|_| MarkError::Database)?;
        let table = tx.open_table(PREPARED).map_err(|_| MarkError::Database)?;
        let key = identity(artifact)?;
        let value = table
            .get(key.as_slice())
            .map_err(|_| MarkError::Database)?
            .ok_or(MarkError::Corrupt)?;
        let expected = PreparedExpectation::read(value.value())?;
        if !expected.compatible(actual) {
            return Err(MarkError::ConflictingReference);
        }
        drop(value);
        drop(table);
        drop(tx);
        // One fixed Source leaf per artifact. The verified artifact is bounded
        // input too; no generic graph traversal or unbounded name set is needed.
        self.charge(usize::try_from(artifact.byte_length).map_err(|_| MarkError::Limit)?)?;
        self.insert(actual.input, deadline, cancel)
    }

    /// Number of distinct exact refs in the mark and its verified artifact closure.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.marked
    }

    /// Original served pointer captured by the same consistent mark snapshot.
    #[must_use]
    pub const fn served_generation(&self) -> Option<&crate::publication::Generation> {
        self.served.as_ref()
    }

    pub(super) const fn set_served(&mut self, generation: Option<crate::publication::Generation>) {
        self.served = generation;
    }

    /// Whether the completed mark has no retained objects.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.marked == 0
    }

    /// Reads one next fixed-size ref, without collecting mark names in memory.
    ///
    /// # Errors
    /// Returns scratch corruption/I/O, cancellation or original-deadline failure.
    pub fn next(
        &self,
        after: Option<ObjectRef>,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Option<ObjectRef>, MarkError> {
        check(deadline, cancel)?;
        let tx = self
            .database()?
            .begin_read()
            .map_err(|_| MarkError::Database)?;
        let table = tx.open_table(MARKS).map_err(|_| MarkError::Database)?;
        let key = after.map(identity).transpose()?;
        let start = key
            .as_ref()
            .map_or(Bound::Unbounded, |key| Bound::Excluded(key.as_slice()));
        let mut range = table
            .range::<&[u8]>((start, Bound::Unbounded))
            .map_err(|_| MarkError::Database)?;
        range
            .next()
            .map(|row| {
                let (key, value) = row.map_err(|_| MarkError::Database)?;
                reference(key.value(), value.value())
            })
            .transpose()
    }

    /// Tests exact membership; a same-name length conflict is never unmarked garbage.
    ///
    /// # Errors
    /// Refuses conflicting refs, scratch I/O, cancellation and deadline expiry.
    pub fn contains(
        &self,
        reference: ObjectRef,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<bool, MarkError> {
        check(deadline, cancel)?;
        let key = identity(reference)?;
        let tx = self
            .database()?
            .begin_read()
            .map_err(|_| MarkError::Database)?;
        let table = tx.open_table(MARKS).map_err(|_| MarkError::Database)?;
        match table.get(key.as_slice()).map_err(|_| MarkError::Database)? {
            None => Ok(false),
            Some(value) if value.value() == reference.byte_length => Ok(true),
            Some(_) => Err(MarkError::ConflictingReference),
        }
    }

    /// Closes/removes only this owned scratch, even after cancellation or expiry.
    ///
    /// # Errors
    /// Returns actual engine/cleanup or caller-budget failures; no final objects are touched.
    pub fn close(mut self, deadline: Instant, cancel: &AtomicBool) -> Result<(), MarkError> {
        let before = check(deadline, cancel);
        let closed = self
            .database
            .take()
            .map_or(Ok(()), |db| db.close().map_err(|_| MarkError::Database));
        let removed = self.path.take().map_or(Ok(()), |path| {
            fs::remove_file(path).map_err(|_| MarkError::Io)
        });
        before?;
        closed?;
        removed?;
        check(deadline, cancel)
    }
}
