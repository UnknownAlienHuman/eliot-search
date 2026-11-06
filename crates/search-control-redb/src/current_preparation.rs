//! Current preparation settings and conditional outcomes in the existing control database.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use redb_plain::{
    Database, ReadOnlyDatabase, ReadableDatabase, ReadableTable, TableDefinition, TableError,
    TableHandle, WriteTransaction,
};
use search_contracts::{Blake3Digest32, SourceId, SourceNamespaceId};
use search_source_registry::{
    CurrentSourceRecord, MAX_SOURCE_OBJECT_BYTES, ObjectKind, ObjectRef, PreparationSettings,
    SOURCE_OBJECT_FORMAT_VERSION,
};

use crate::current_source::{
    NAMESPACE_KEY, PREPARATION_TABLE_NAME, SCHEMA_VERSION_KEY, SOURCE_ROWS, SOURCE_SCHEMA_VERSION,
    SOURCE_STATE, catalogue_names, decode_record, read_namespace, read_source_by_id,
};

const PREPARATIONS: TableDefinition<&[u8], &[u8]> = TableDefinition::new(PREPARATION_TABLE_NAME);

/// Bounded content or execution-limit reason for a preparation gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationGap {
    /// Retained bytes contain binary controls.
    BinaryContent,
    /// Retained bytes are not UTF-8.
    InvalidUtf8,
    /// Selected input byte ceiling was exceeded.
    InputTooLarge,
    /// Selected output byte ceiling was exceeded.
    OutputTooLarge,
    /// Selected line ceiling was exceeded.
    TooManyLines,
    /// Selected unit ceiling was exceeded.
    TooManyUnits,
    /// A unit cannot fit the selected hard chunk setting.
    UnitTooLarge,
    /// The complete encoded artifact exceeds its byte allowance.
    EncodedTooLarge,
}

impl PreparationGap {
    /// Closed machine-readable gap reason.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::BinaryContent => "binary_content",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::InputTooLarge => "input_too_large",
            Self::OutputTooLarge => "output_too_large",
            Self::TooManyLines => "too_many_lines",
            Self::TooManyUnits => "too_many_units",
            Self::UnitTooLarge => "unit_too_large",
            Self::EncodedTooLarge => "encoded_too_large",
        }
    }
    const fn tag(self) -> u8 {
        match self {
            Self::BinaryContent => 1,
            Self::InvalidUtf8 => 2,
            Self::InputTooLarge => 3,
            Self::OutputTooLarge => 4,
            Self::TooManyLines => 5,
            Self::TooManyUnits => 6,
            Self::UnitTooLarge => 7,
            Self::EncodedTooLarge => 8,
        }
    }
    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::BinaryContent),
            2 => Some(Self::InvalidUtf8),
            3 => Some(Self::InputTooLarge),
            4 => Some(Self::OutputTooLarge),
            5 => Some(Self::TooManyLines),
            6 => Some(Self::TooManyUnits),
            7 => Some(Self::UnitTooLarge),
            8 => Some(Self::EncodedTooLarge),
            _ => None,
        }
    }
}

/// Result installed for one exact wanted preparation input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationOutcome {
    /// A complete stored prepared artifact, including successful empty text.
    Ready {
        /// Verified immutable prepared object.
        artifact: ObjectRef,
    },
    /// An explicit bounded failure, never a successful empty artifact.
    Gap {
        /// Actual content or fail-only ceiling reason.
        reason: PreparationGap,
    },
}

/// Binding of one current outcome to its source observation and output settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundPreparationOutcome {
    /// Source row revision at installation.
    pub source_row_revision: u64,
    /// Source registration revision at installation.
    pub registration_revision: u64,
    /// Exact retained source input.
    pub input: ObjectRef,
    /// Output-affecting settings used for this result.
    pub settings: PreparationSettings,
    /// Stored artifact or explicit gap.
    pub result: PreparationOutcome,
}

impl BoundPreparationOutcome {
    /// Whether pure artifact input remains identical under a newly captured row guard.
    #[must_use]
    pub fn matches_input(
        &self,
        source: &CurrentSourceRecord,
        settings: PreparationSettings,
    ) -> bool {
        (self.registration_revision, self.input, self.settings)
            == (source.registration_revision, source.object_ref, settings)
    }
}

/// One persisted current settings selection and optional preparation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentPreparation {
    /// Existing root-local namespace.
    pub source_namespace_id: SourceNamespaceId,
    /// Existing source identity.
    pub source_id: SourceId,
    /// Non-reset settings-selection revision.
    pub settings_revision: u64,
    /// Currently selected output-affecting settings.
    pub settings: PreparationSettings,
    /// Non-reset outcome revision, including absence transitions.
    pub outcome_revision: u64,
    /// Last installed bound outcome; settings changes do not delete it.
    pub outcome: Option<BoundPreparationOutcome>,
}

/// Consistent read-only guard captured before retained-object or preparation I/O.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparationCapture {
    /// Exact existing source row, including namespace and non-reset guards.
    pub source: CurrentSourceRecord,
    /// Current output settings.
    pub settings: PreparationSettings,
    /// Captured settings revision.
    pub settings_revision: u64,
    /// Captured outcome revision, even when no outcome exists.
    pub expected_outcome_revision: u64,
    /// Exact expected outcome value, including absence.
    pub expected_outcome: Option<BoundPreparationOutcome>,
}

/// Closed preparation-control failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationControlError {
    /// Source is not retained in this root.
    NotRetained,
    /// Explicit settings selection has not been initialized.
    NotSelected,
    /// Unsupported output-affecting settings.
    InvalidSettings,
    /// Unsupported or partial control catalogue.
    UnsupportedSchema,
    /// Invalid current row data.
    Corrupt,
    /// Captured source/settings/outcome changed.
    Stale,
    /// Database I/O failed.
    Database,
    /// A non-reset revision cannot advance.
    RevisionExhausted,
    /// An outcome reference has the wrong format or kind.
    InvalidOutcome,
    /// Original deadline expired.
    Deadline,
    /// Cancellation observed.
    Cancelled,
}
impl PreparationControlError {
    /// Content-free machine-readable error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotRetained => "PREPARATION_SOURCE_NOT_RETAINED",
            Self::NotSelected => "PREPARATION_NOT_SELECTED",
            Self::InvalidSettings => "PREPARATION_SETTINGS_INVALID",
            Self::UnsupportedSchema => "PREPARATION_SCHEMA_UNSUPPORTED",
            Self::Corrupt => "PREPARATION_CONTROL_CORRUPT",
            Self::Stale => "PREPARATION_STALE",
            Self::Database => "PREPARATION_CONTROL_IO",
            Self::RevisionExhausted => "PREPARATION_REVISION_EXHAUSTED",
            Self::InvalidOutcome => "PREPARATION_OUTCOME_INVALID",
            Self::Deadline => "PREPARATION_DEADLINE",
            Self::Cancelled => "PREPARATION_CANCELLED",
        }
    }
}
type Result<T> = std::result::Result<T, PreparationControlError>;
fn check(deadline: Instant, cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(PreparationControlError::Cancelled)
    } else if Instant::now() >= deadline {
        Err(PreparationControlError::Deadline)
    } else {
        Ok(())
    }
}
const fn source_error(error: crate::current_source::SourceControlError) -> PreparationControlError {
    use crate::current_source::SourceControlError as E;
    match error {
        E::UnsupportedSchema => PreparationControlError::UnsupportedSchema,
        E::Corrupt => PreparationControlError::Corrupt,
        _ => PreparationControlError::Database,
    }
}
fn put_settings(out: &mut Vec<u8>, s: PreparationSettings) {
    out.extend_from_slice(&s.utf8_version.to_le_bytes());
    out.extend_from_slice(&s.line_unitizer_version.to_le_bytes());
    out.extend_from_slice(&s.preferred_unit_bytes.to_le_bytes());
    out.extend_from_slice(&s.max_unit_bytes.to_le_bytes());
}
fn put_object(out: &mut Vec<u8>, o: ObjectRef) {
    out.extend_from_slice(&o.format_version.to_le_bytes());
    out.push(o.kind.tag());
    out.extend_from_slice(o.digest.as_bytes());
    out.extend_from_slice(&o.byte_length.to_le_bytes());
}
fn valid_object(o: ObjectRef, kind: ObjectKind) -> bool {
    o.kind == kind
        && o.format_version == SOURCE_OBJECT_FORMAT_VERSION
        && (kind != ObjectKind::Source || o.byte_length <= MAX_SOURCE_OBJECT_BYTES)
        && (kind != ObjectKind::Prepared || o.byte_length > 0)
}
fn validate(row: &CurrentPreparation) -> Result<()> {
    if row.settings_revision == 0 || !row.settings.is_supported() {
        return Err(PreparationControlError::Corrupt);
    }
    if let Some(o) = &row.outcome {
        if row.outcome_revision == 0
            || o.source_row_revision == 0
            || o.registration_revision == 0
            || !o.settings.is_supported()
            || !valid_object(o.input, ObjectKind::Source)
        {
            return Err(PreparationControlError::Corrupt);
        }
        if let PreparationOutcome::Ready { artifact } = o.result
            && !valid_object(artifact, ObjectKind::Prepared)
        {
            return Err(PreparationControlError::InvalidOutcome);
        }
    }
    Ok(())
}
fn encode(row: &CurrentPreparation) -> Result<Vec<u8>> {
    validate(row)?;
    let mut out = Vec::new();
    out.push(1);
    out.extend_from_slice(row.source_namespace_id.as_bytes());
    out.extend_from_slice(row.source_id.as_bytes());
    out.extend_from_slice(&row.settings_revision.to_le_bytes());
    put_settings(&mut out, row.settings);
    out.extend_from_slice(&row.outcome_revision.to_le_bytes());
    let Some(o) = &row.outcome else {
        out.push(0);
        return Ok(out);
    };
    out.push(match o.result {
        PreparationOutcome::Ready { .. } => 1,
        PreparationOutcome::Gap { .. } => 2,
    });
    out.extend_from_slice(&o.source_row_revision.to_le_bytes());
    out.extend_from_slice(&o.registration_revision.to_le_bytes());
    put_object(&mut out, o.input);
    put_settings(&mut out, o.settings);
    match o.result {
        PreparationOutcome::Ready { artifact } => put_object(&mut out, artifact),
        PreparationOutcome::Gap { reason } => out.push(reason.tag()),
    }
    Ok(out)
}
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Cursor<'_> {
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self
            .at
            .checked_add(N)
            .ok_or(PreparationControlError::Corrupt)?;
        let v = self
            .bytes
            .get(self.at..end)
            .ok_or(PreparationControlError::Corrupt)?
            .try_into()
            .map_err(|_| PreparationControlError::Corrupt)?;
        self.at = end;
        Ok(v)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn settings(&mut self) -> Result<PreparationSettings> {
        Ok(PreparationSettings {
            utf8_version: u16::from_le_bytes(self.array()?),
            line_unitizer_version: u16::from_le_bytes(self.array()?),
            preferred_unit_bytes: u32::from_le_bytes(self.array()?),
            max_unit_bytes: u32::from_le_bytes(self.array()?),
        })
    }
    fn object(&mut self) -> Result<ObjectRef> {
        Ok(ObjectRef {
            format_version: u16::from_le_bytes(self.array()?),
            kind: ObjectKind::from_tag(self.byte()?).ok_or(PreparationControlError::Corrupt)?,
            digest: Blake3Digest32::from_bytes(self.array()?),
            byte_length: self.u64()?,
        })
    }
}
fn decode(bytes: &[u8]) -> Result<CurrentPreparation> {
    let mut c = Cursor { bytes, at: 0 };
    if c.byte()? != 1 {
        return Err(PreparationControlError::UnsupportedSchema);
    }
    let source_namespace_id = SourceNamespaceId::from_bytes(c.array()?);
    let source_id = SourceId::from_bytes(c.array()?);
    let settings_revision = c.u64()?;
    let settings = c.settings()?;
    let outcome_revision = c.u64()?;
    let tag = c.byte()?;
    let outcome = if tag == 0 {
        None
    } else {
        let source_row_revision = c.u64()?;
        let registration_revision = c.u64()?;
        let input = c.object()?;
        let settings = c.settings()?;
        let result = match tag {
            1 => PreparationOutcome::Ready {
                artifact: c.object()?,
            },
            2 => PreparationOutcome::Gap {
                reason: PreparationGap::from_tag(c.byte()?)
                    .ok_or(PreparationControlError::Corrupt)?,
            },
            _ => return Err(PreparationControlError::Corrupt),
        };
        Some(BoundPreparationOutcome {
            source_row_revision,
            registration_revision,
            input,
            settings,
            result,
        })
    };
    if c.at != bytes.len() {
        return Err(PreparationControlError::Corrupt);
    }
    let row = CurrentPreparation {
        source_namespace_id,
        source_id,
        settings_revision,
        settings,
        outcome_revision,
        outcome,
    };
    validate(&row)?;
    Ok(row)
}

fn read_view(
    path: &Path,
    id: SourceId,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(CurrentSourceRecord, Option<CurrentPreparation>)> {
    check(deadline, cancel)?;
    let db = ReadOnlyDatabase::open(path).map_err(|_| PreparationControlError::Database)?;
    let tx = db
        .begin_read()
        .map_err(|_| PreparationControlError::Database)?;
    let names = tx
        .list_tables()
        .map_err(|_| PreparationControlError::Database)?
        .map(|t| t.name().to_owned())
        .collect();
    let multi = tx
        .list_multimap_tables()
        .map_err(|_| PreparationControlError::Database)?
        .next()
        .is_some();
    if !catalogue_names(names, multi).map_err(source_error)? {
        return Err(PreparationControlError::NotRetained);
    }
    let source = read_source_by_id(&tx, id)
        .map_err(source_error)?
        .ok_or(PreparationControlError::NotRetained)?;
    let ns = read_namespace(&tx)
        .map_err(source_error)?
        .ok_or(PreparationControlError::Corrupt)?;
    if ns != source.source_namespace_id {
        return Err(PreparationControlError::Corrupt);
    }
    let row = match tx.open_table(PREPARATIONS) {
        Ok(table) => {
            let value = table
                .get(id.as_bytes().as_slice())
                .map_err(|_| PreparationControlError::Database)?;
            value.map(|v| decode(v.value())).transpose()?
        }
        Err(TableError::TableDoesNotExist(_)) => None,
        Err(_) => return Err(PreparationControlError::Database),
    };
    if let Some(r) = &row
        && (r.source_id != id || r.source_namespace_id != ns)
    {
        return Err(PreparationControlError::Corrupt);
    }
    check(deadline, cancel)?;
    drop(tx);
    db.close().map_err(|_| PreparationControlError::Database)?;
    Ok((source, row))
}
fn write_source(tx: &WriteTransaction, id: SourceId) -> Result<CurrentSourceRecord> {
    let state = tx
        .open_table(SOURCE_STATE)
        .map_err(|_| PreparationControlError::Database)?;
    let version = state
        .get(SCHEMA_VERSION_KEY)
        .map_err(|_| PreparationControlError::Database)?
        .ok_or(PreparationControlError::Corrupt)?;
    if version.value() != SOURCE_SCHEMA_VERSION {
        return Err(PreparationControlError::UnsupportedSchema);
    }
    let ns = state
        .get(NAMESPACE_KEY)
        .map_err(|_| PreparationControlError::Database)?
        .ok_or(PreparationControlError::Corrupt)?;
    let rows = tx
        .open_table(SOURCE_ROWS)
        .map_err(|_| PreparationControlError::Database)?;
    let value = rows
        .get(id.as_bytes().as_slice())
        .map_err(|_| PreparationControlError::Database)?
        .ok_or(PreparationControlError::NotRetained)?;
    let source = decode_record(value.value()).map_err(source_error)?;
    if source.source_id != id || source.source_namespace_id.as_bytes() != ns.value() {
        return Err(PreparationControlError::Corrupt);
    }
    Ok(source)
}
fn finish_commit(
    db: Database,
    tx: redb_plain::WriteTransaction,
    path: &Path,
    proposed: CurrentPreparation,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<CurrentPreparation> {
    check(deadline, cancel)?;
    let committed = tx.commit();
    drop(db);
    if committed.is_ok() {
        return Ok(proposed);
    }
    let (_, observed) = read_view(path, proposed.source_id, deadline, cancel)?;
    if observed.as_ref() == Some(&proposed) {
        Ok(proposed)
    } else {
        Err(PreparationControlError::Stale)
    }
}

/// Explicitly selects settings, advancing their revision only when their value changes.
///
/// # Errors
/// Returns a closed error for invalid settings, missing/corrupt state, revision exhaustion or I/O.
pub fn select_preparation_settings(
    path: &Path,
    id: SourceId,
    settings: PreparationSettings,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<CurrentPreparation> {
    select_settings(path, id, settings, false, deadline, cancel)
}
fn select_settings(
    path: &Path,
    id: SourceId,
    settings: PreparationSettings,
    initialize_only: bool,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<CurrentPreparation> {
    if !settings.is_supported() {
        return Err(PreparationControlError::InvalidSettings);
    }
    let (source, previous) = read_view(path, id, deadline, cancel)?;
    if let Some(row) = &previous
        && (initialize_only || row.settings == settings)
    {
        return Ok(row.clone());
    }
    let db = Database::open(path).map_err(|_| PreparationControlError::Database)?;
    let tx = db
        .begin_write()
        .map_err(|_| PreparationControlError::Database)?;
    let current = write_source(&tx, id)?;
    if current.source_namespace_id != source.source_namespace_id
        || current.registration_revision != source.registration_revision
    {
        return Err(PreparationControlError::Stale);
    }
    let mut table = tx
        .open_table(PREPARATIONS)
        .map_err(|_| PreparationControlError::Database)?;
    let found = table
        .get(id.as_bytes().as_slice())
        .map_err(|_| PreparationControlError::Database)?
        .map(|v| decode(v.value()))
        .transpose()?;
    if found != previous {
        return Err(PreparationControlError::Stale);
    }
    let row = match found {
        Some(mut row) => {
            row.settings_revision = row
                .settings_revision
                .checked_add(1)
                .ok_or(PreparationControlError::RevisionExhausted)?;
            row.settings = settings;
            row
        }
        None => CurrentPreparation {
            source_namespace_id: source.source_namespace_id,
            source_id: id,
            settings_revision: 1,
            settings,
            outcome_revision: 0,
            outcome: None,
        },
    };
    let bytes = encode(&row)?;
    table
        .insert(id.as_bytes().as_slice(), bytes.as_slice())
        .map_err(|_| PreparationControlError::Database)?;
    drop(table);
    finish_commit(db, tx, path, row, deadline, cancel)
}

/// Initializes the default only when no selection exists, preserving selected settings on every repeat.
///
/// # Errors
/// Returns a closed error for missing/corrupt state or failed explicit initialization.
pub fn ensure_preparation_settings(
    path: &Path,
    id: SourceId,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<CurrentPreparation> {
    let (_, row) = read_view(path, id, deadline, cancel)?;
    row.map_or_else(
        || {
            select_settings(
                path,
                id,
                PreparationSettings::default(),
                true,
                deadline,
                cancel,
            )
        },
        Ok,
    )
}

/// Captures source, settings and exact expected outcome from one native read-only snapshot.
///
/// # Errors
/// Returns a closed error without creating tables or repairing the database.
pub fn capture_preparation(
    path: &Path,
    id: SourceId,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PreparationCapture> {
    let (source, row) = read_view(path, id, deadline, cancel)?;
    let row = row.ok_or(PreparationControlError::NotSelected)?;
    Ok(PreparationCapture {
        source,
        settings: row.settings,
        settings_revision: row.settings_revision,
        expected_outcome_revision: row.outcome_revision,
        expected_outcome: row.outcome,
    })
}

/// Installs an outcome only while all captured source/settings/outcome guards remain exact.
///
/// # Errors
/// Returns Stale on any changed guard. Object I/O must have completed before this call.
pub fn commit_preparation(
    path: &Path,
    capture: &PreparationCapture,
    result: PreparationOutcome,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<CurrentPreparation> {
    check(deadline, cancel)?;
    if let PreparationOutcome::Ready { artifact } = result
        && !valid_object(artifact, ObjectKind::Prepared)
    {
        return Err(PreparationControlError::InvalidOutcome);
    }
    let db = Database::open(path).map_err(|_| PreparationControlError::Database)?;
    let tx = db
        .begin_write()
        .map_err(|_| PreparationControlError::Database)?;
    let source = write_source(&tx, capture.source.source_id)?;
    if source != capture.source {
        return Err(PreparationControlError::Stale);
    }
    let mut table = tx
        .open_table(PREPARATIONS)
        .map_err(|_| PreparationControlError::Database)?;
    let mut row = {
        let value = table
            .get(source.source_id.as_bytes().as_slice())
            .map_err(|_| PreparationControlError::Database)?
            .ok_or(PreparationControlError::Stale)?;
        decode(value.value())?
    };
    let outcome_changed = (row.outcome_revision, &row.outcome)
        != (capture.expected_outcome_revision, &capture.expected_outcome);
    if row.source_namespace_id != source.source_namespace_id
        || row.source_id != source.source_id
        || row.settings_revision != capture.settings_revision
        || row.settings != capture.settings
        || outcome_changed
    {
        return Err(PreparationControlError::Stale);
    }
    if matches!(result, PreparationOutcome::Gap { .. })
        && row.outcome.as_ref().is_some_and(|o| {
            o.matches_input(&source, capture.settings)
                && matches!(o.result, PreparationOutcome::Ready { .. })
        })
    {
        return Ok(row);
    }
    let outcome = BoundPreparationOutcome {
        source_row_revision: source.row_revision,
        registration_revision: source.registration_revision,
        input: source.object_ref,
        settings: capture.settings,
        result,
    };
    if row.outcome.as_ref() == Some(&outcome) {
        return Ok(row);
    }
    row.outcome_revision = row
        .outcome_revision
        .checked_add(1)
        .ok_or(PreparationControlError::RevisionExhausted)?;
    row.outcome = Some(outcome);
    let bytes = encode(&row)?;
    table
        .insert(source.source_id.as_bytes().as_slice(), bytes.as_slice())
        .map_err(|_| PreparationControlError::Database)?;
    drop(table);
    finish_commit(db, tx, path, row, deadline, cancel)
}
