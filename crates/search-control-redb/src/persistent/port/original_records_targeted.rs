//! Bounded generic record subsets read under the retained original context.

use super::super::{bootstrap::OriginalCheck, operation::{Check, Point}};
use super::BoundControlJournal;
use crate::{ControlKey, ControlValue, JournalIdentity, MigrationBootstrapFailure,
    MigrationBootstrapWork};
use search_ports::CancellationProbe;
use std::time::Instant;

/// One coherent sorted subset of technical control records.
///
/// Its private fields and subset-only accessors prevent it from being mistaken
/// for a complete journal or serving snapshot; absent requested keys are explicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedControlRecordsReadback {
    identity: JournalIdentity,
    generation: u64,
    records: Vec<(ControlKey, ControlValue)>,
    missing_keys: Vec<ControlKey>,
}

impl ScopedControlRecordsReadback {
    /// Exact journal identity observed in the read transaction.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }
    /// Exact committed generation observed in the read transaction.
    #[must_use]
    pub const fn generation(&self) -> u64 { self.generation }
    /// Sorted records for requested keys only.
    #[must_use]
    pub fn records(&self) -> &[(ControlKey, ControlValue)] { &self.records }
    /// Sorted requested keys absent on disk.
    #[must_use]
    pub fn missing_keys(&self) -> &[ControlKey] { &self.missing_keys }
    /// Looks up one exact requested record without widening the read scope.
    #[must_use]
    pub fn get(&self, key: &ControlKey) -> Option<&ControlValue> {
        self.records.binary_search_by(|(candidate, _)| candidate.cmp(key))
            .ok().map(|index| &self.records[index].1)
    }
}

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Reads selected generic control keys with the caller's original context.
    ///
    /// One checked redb read transaction performs point reads only. No table
    /// scan, publisher/durable write or complete-snapshot conversion occurs.
    ///
    /// # Errors
    /// Returns no partial readback on callback/deadline/store/bound refusal.
    pub fn read_control_keys_with_original_context<F, E>(
        &self, keys: &[ControlKey], original_deadline: Instant, work: &F,
    ) -> Result<ScopedControlRecordsReadback, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        use MigrationBootstrapWork as Work;
        let check = OriginalCheck::registry(original_deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            let limits = self.journal.limits();
            if keys.is_empty() || keys.len() > limits.max_mutation_items {
                return Err(crate::ControlError::BudgetExceeded);
            }
            let slots = keys.len().checked_mul(std::mem::size_of::<&ControlKey>())
                .ok_or(crate::ControlError::BudgetExceeded)?;
            check.reserve(Work::Allocate(slots))?;
            let mut ordered = Vec::with_capacity(keys.len());
            ordered.extend(keys.iter());
            ordered.sort_unstable();
            let (identity, generation, values) =
                self.journal.read_current_records_checked(&ordered, &check)?;
            let mut records = Vec::new();
            let mut missing_keys = Vec::new();
            for (key, value) in ordered.into_iter().zip(values) {
                check.check(Point::ReadRecord)?;
                check.reserve(Work::Allocate(key.as_bytes().len()))?;
                check.reserve(Work::Process(key.as_bytes().len()))?;
                if let Some(value) = value {
                    records.push((key.clone(), value));
                } else {
                    missing_keys.push(key.clone());
                }
            }
            check.check(Point::ReadComplete)?;
            Ok(ScopedControlRecordsReadback { identity, generation, records, missing_keys })
        })();
        result.map_err(|error| check.failure(error))
    }
}
