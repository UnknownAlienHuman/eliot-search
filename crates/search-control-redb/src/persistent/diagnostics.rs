//! Bounded read-only diagnostics. Metadata observations are not recovery,
//! content-integrity verification, publisher admission or owner authority.

use search_ports::{CancellationProbe, OperationContext};

use super::operation::{Budget, Check, Point};
use super::{ControlCallError, ControlError, ControlSnapshotPublisher, Header,
    JournalIdentity, PersistentControlJournal, is_corruption};

/// Coherent application counters, never operating-system I/O measurements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalWriteCounters {
    /// Committed application data generation from the validated disk header.
    pub data_generation: u64,
    /// Live record cardinality, compared with actual table metadata.
    pub live_records: u64,
    /// Declared live value bytes. Exact byte accounting is checked by `verify`.
    pub live_value_bytes: u64,
    /// Durable data-operation receipt cardinality, compared with its table.
    pub operation_records: u64,
    /// Declared encoded receipt bytes. This call does not scan receipt bodies.
    pub operation_record_bytes: u64,
    /// Successful mutating calls reported by this handle since creation/open.
    /// Includes initialization/handoff/hold writes, but not lost acknowledgements.
    /// Saturates at `u64::MAX` and resets on reopen; it is not a physical-write count.
    pub acknowledged_mutating_calls: u64,
}

/// What a bounded metadata inspection actually established.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalHealthState {
    /// Identity/schema/count metadata matched; record bodies were not reverified.
    MetadataReadable,
    /// An earlier possibly committed data mutation still requires exact recovery.
    RecoveryRequired,
    /// This handle is held locally, whether or not a durable marker was written.
    LocallyQuarantined,
    /// A durable hold marker exists. Even malformed marker presence blocks serving.
    DurablyQuarantined,
    /// Contradictory metadata was observed; no repair or durable hold is fabricated.
    QuarantineRequired,
}

/// Diagnostic relation between a supplied publisher and observed journal metadata.
/// None of these variants grants admission or authenticates a snapshot's contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotHealthState {
    /// The caller did not supply its publisher.
    NotObserved,
    /// The journal itself is blocked; publisher availability is irrelevant.
    BlockedByJournal,
    /// This publisher has never been bound to verified disk publication.
    Unbound,
    /// Publisher identity/owner differs from this exact journal binding.
    DifferentBinding,
    /// A previous publication attempt still requires exact disk recovery.
    RecoveryRequired,
    /// No snapshot is published, including initialized generation zero.
    NotPublished,
    /// The supplied historical snapshot is behind the observed disk generation.
    BehindJournal,
    /// The snapshot is ahead of the observed journal; do not infer disk recovery.
    AheadOfJournal,
    /// Binding and generation align. This is NOT a fresh content-integrity check.
    GenerationAligned,
}

/// Fixed-size diagnostic observation, not a usable journal/snapshot or receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlStoreHealth {
    /// The caller-verified binding held by this handle. If counters are absent,
    /// metadata inspection did not establish that disk currently matches it.
    pub expected_identity: JournalIdentity,
    /// Observed metadata/local-fence status.
    pub state: JournalHealthState,
    /// Present only after coherent header/schema/table-cardinality validation.
    pub counters: Option<JournalWriteCounters>,
    /// Whether an exact data-mutation recovery fence remains in this handle.
    pub pending_mutation: bool,
    /// Whether this handle's independent local quarantine fence remains set.
    pub local_quarantine: bool,
    /// Relation to the optional supplied publisher; never a readiness grant.
    pub snapshot: SnapshotHealthState,
    /// Closed reason, without paths, keys, values or arbitrary vendor strings.
    pub reason: Option<ControlError>,
}

impl PersistentControlJournal {
    /// Reads current bounded counters without scanning values or writing state.
    ///
    /// # Errors
    /// Blocked journals, invalid metadata, cancellation and deadline expiration
    /// fail without returning partial counters. Calls share one cooperative budget;
    /// synchronous redb/OS operations cannot be preempted.
    pub fn write_counters_with_context<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<JournalWriteCounters, ControlCallError> {
        let budget = Budget::new(context);
        self.write_counters_checked(&budget).map_err(|error| budget.failure(error, None))
    }

    /// Observes metadata and local fences, even while normal operations are held.
    ///
    /// Uses one read transaction and no record/receipt scan, repair, mutation,
    /// recovery or publisher update. A healthy-looking header does not resolve a
    /// pending operation, remove quarantine or prove all record bodies intact.
    /// The optional publisher is only inspected; a clone remains a historical view.
    ///
    /// # Errors
    /// Cancellation/deadline or transient storage failure returns an error, not
    /// invented corruption or a partial healthy result. Actual metadata conflicts
    /// are reported as quarantine-required data with absent counters. This method
    /// cannot inspect a database that native open could not return in the first place.
    pub fn journal_health_with_context<C: CancellationProbe>(
        &self,
        publisher: Option<&ControlSnapshotPublisher>,
        context: &OperationContext<C>,
    ) -> Result<ControlStoreHealth, ControlCallError> {
        let budget = Budget::new(context);
        self.journal_health_checked(publisher, &budget)
            .map_err(|error| budget.failure(error, None))
    }

    fn write_counters_checked(&self, check: &dyn Check) -> Result<JournalWriteCounters, ControlError> {
        self.ensure_available()?;
        let counters = self.observe_counters(check)?;
        check.check(Point::ReadComplete)?;
        Ok(counters)
    }

    fn observe_counters(&self, check: &dyn Check) -> Result<JournalWriteCounters, ControlError> {
        check.check(Point::Start)?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        check.check(Point::ReadHeader)?;
        let header = self.header_from(&read)?;
        check.check(Point::ReadHeader)?;
        Ok(counters_from(&header, self.committed_writes))
    }

    fn journal_health_checked(
        &self,
        publisher: Option<&ControlSnapshotPublisher>,
        check: &dyn Check,
    ) -> Result<ControlStoreHealth, ControlError> {
        let (state, counters, reason) = match self.observe_counters(check) {
            Ok(counters) if self.quarantined => (
                JournalHealthState::LocallyQuarantined, Some(counters), Some(ControlError::StoreQuarantined),
            ),
            Ok(counters) if self.pending.is_some() => (
                JournalHealthState::RecoveryRequired, Some(counters), Some(ControlError::CommitOutcomeUnknown),
            ),
            Ok(counters) => (JournalHealthState::MetadataReadable, Some(counters), None),
            Err(ControlError::StoreQuarantined) => (
                JournalHealthState::DurablyQuarantined, None, Some(ControlError::StoreQuarantined),
            ),
            Err(error) if is_corruption(error) => (
                JournalHealthState::QuarantineRequired, None, Some(error),
            ),
            Err(error) => return Err(error),
        };
        let snapshot = match counters {
            Some(counters) if state == JournalHealthState::MetadataReadable => {
                snapshot_health(publisher, self.identity, counters.data_generation)
            }
            _ => SnapshotHealthState::BlockedByJournal,
        };
        check.check(Point::ReadComplete)?;
        Ok(ControlStoreHealth {
            expected_identity: self.identity, state, counters,
            pending_mutation: self.pending.is_some(), local_quarantine: self.quarantined,
            snapshot, reason,
        })
    }
}

const fn counters_from(header: &Header, acknowledged_mutating_calls: u64) -> JournalWriteCounters {
    JournalWriteCounters {
        data_generation: header.generation, live_records: header.records,
        live_value_bytes: header.value_bytes, operation_records: header.operations,
        operation_record_bytes: header.operation_bytes, acknowledged_mutating_calls,
    }
}

fn snapshot_health(
    publisher: Option<&ControlSnapshotPublisher>,
    identity: JournalIdentity,
    generation: u64,
) -> SnapshotHealthState {
    let Some(publisher) = publisher else { return SnapshotHealthState::NotObserved; };
    let Some(bound_identity) = publisher.diagnostic_disk_identity() else {
        return SnapshotHealthState::Unbound;
    };
    if bound_identity != identity { return SnapshotHealthState::DifferentBinding; }
    if publisher.requires_recovery() { return SnapshotHealthState::RecoveryRequired; }
    let Some(current) = publisher.current() else { return SnapshotHealthState::NotPublished; };
    if current.identity != identity { return SnapshotHealthState::DifferentBinding; }
    match current.generation.cmp(&generation) {
        std::cmp::Ordering::Less => SnapshotHealthState::BehindJournal,
        std::cmp::Ordering::Equal => SnapshotHealthState::GenerationAligned,
        std::cmp::Ordering::Greater => SnapshotHealthState::AheadOfJournal,
    }
}

#[cfg(test)]
mod tests;
