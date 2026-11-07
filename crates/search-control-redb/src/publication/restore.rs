//! Exact served-baseline installation into an owned incomplete destination.

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use redb_plain::{ReadableDatabase, TableHandle};

use super::{
    CapturedSource, ControlWriter, MAX_CAPTURE_BYTES, MAX_SOURCES, PublicationError as E, Result,
    check, read_corpus, source_error,
};
use crate::{current_preparation, current_source};

/// Validates a whole served baseline under source and publication metadata bounds.
/// This operation is read-only and performs no object I/O or preparation.
///
/// # Errors
/// Refuses malformed bindings, duplicate IDs/locators, owner bounds and operation failures.
pub fn validate_served_corpus(
    sources: &[CapturedSource],
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    check(deadline, cancel)?;
    if sources.len() > MAX_SOURCES {
        return Err(E::Limit);
    }
    let namespace = sources.first().map(|s| s.source.source_namespace_id);
    let mut previous = None;
    let mut locators = BTreeSet::new();
    let mut totals = current_source::SourceInventoryTotals::default();
    let mut charged = 0_usize;
    for captured in sources {
        check(deadline, cancel)?;
        let source = &captured.source;
        let preparation = &captured.preparation;
        let row = current_source::encode_record(source).map_err(source_error)?;
        current_source::decode_record(&row).map_err(source_error)?;
        let prepared = current_preparation::encode(preparation).map_err(|_| E::Corrupt)?;
        current_preparation::decode(&prepared).map_err(|_| E::Corrupt)?;
        if Some(source.source_namespace_id) != namespace
            || previous.is_some_and(|id| id >= source.source_id)
            || !locators.insert(source.locator.as_slice())
            || source.source_id != preparation.source_id
            || source.source_namespace_id != preparation.source_namespace_id
            || !preparation
                .outcome
                .as_ref()
                .is_some_and(|o| o.matches_input(source, preparation.settings))
        {
            return Err(E::Corrupt);
        }
        previous = Some(source.source_id);
        totals
            .add(row.len(), source.locator.len())
            .map_err(source_error)?;
        totals.check_limit().map_err(source_error)?;
        charged = charged
            .checked_add(row.len())
            .and_then(|n| n.checked_add(prepared.len()))
            .ok_or(E::Limit)?;
        if charged > MAX_CAPTURE_BYTES {
            return Err(E::Limit);
        }
    }
    check(deadline, cancel)
}

impl ControlWriter {
    /// Installs exact served source/preparation rows once into a pristine destination.
    /// An identical retry leaves all revisions and destination publication obligations intact.
    /// The caller owns the validated import marker and has verified every retained object.
    ///
    /// # Errors
    /// Refuses non-identical populated state, invalid accounting/bindings and ambiguous commits.
    pub fn install_served_corpus(
        &self,
        sources: &[CapturedSource],
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<()> {
        validate_served_corpus(sources, deadline, cancel)?;
        let read = self.database().begin_read().map_err(|_| E::Database)?;
        let pristine = {
            let mut names = read.list_tables().map_err(|_| E::Database)?;
            names
                .next()
                .is_some_and(|table| table.name() == "plain_control_meta")
                && names.next().is_none()
        };
        if !pristine {
            let capture = read_corpus(&read, deadline, cancel)?;
            let mut totals = current_source::SourceInventoryTotals::default();
            for source in sources {
                totals
                    .include(std::slice::from_ref(&source.source))
                    .map_err(source_error)?;
            }
            if capture.sources != sources
                || current_source::wanted_revision(&read).map_err(source_error)? == 0
                || current_source::read_totals(&read).map_err(source_error)? != Some(totals)
            {
                return Err(E::Stale);
            }
            return check(deadline, cancel);
        }
        drop(read);
        let tx = self.database().begin_write().map_err(|_| E::Database)?;
        let pristine = {
            let mut names = tx.list_tables().map_err(|_| E::Database)?;
            names
                .next()
                .is_some_and(|table| table.name() == "plain_control_meta")
                && names.next().is_none()
        };
        if !pristine
            || tx
                .list_multimap_tables()
                .map_err(|_| E::Database)?
                .next()
                .is_some()
        {
            return Err(E::Stale);
        }
        if tx
            .list_persistent_savepoints()
            .map_err(|_| E::Database)?
            .next()
            .is_some()
        {
            return Err(E::UnsupportedSchema);
        }
        current_source::install_import_sources(&tx, sources, deadline, cancel)
            .map_err(source_error)?;
        for source in sources {
            check(deadline, cancel)?;
            current_preparation::install_import_preparation(&tx, &source.preparation).map_err(
                |error| match error {
                    current_preparation::PreparationControlError::Database => E::Database,
                    _ => E::Corrupt,
                },
            )?;
        }
        check(deadline, cancel)?;
        tx.commit().map_err(|_| E::Database)
    }
}
