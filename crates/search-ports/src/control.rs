//! Control-journal and immutable snapshot ports.

use search_contracts::PublicationIntent;

use crate::{MutationIdentity, OperationContext, Port};

/// Durable control-journal abstraction.
pub trait ControlJournalPort: Port {
    /// Immutable control snapshot returned by a read transaction.
    type ControlSnapshot: Send + Sync + 'static;
    /// Bounded typed control command.
    type Command: Send + Sync + 'static;
    /// Guarded visible-epoch compare-and-swap request.
    type VisibleEpochGuards: Send + Sync + 'static;
    /// Content-free durable commit receipt.
    type ControlCommit: Send + Sync + 'static;
    /// Typed quarantine reason with no source/query/secret content.
    type QuarantineReason: Send + Sync + 'static;
    /// Content-free quarantine receipt.
    type QuarantineReceipt: Send + Sync + 'static;
    /// Bounded journal write counters.
    type JournalWriteCounters: Send + Sync + 'static;

    /// Reads one side-effect-free authoritative control snapshot.
    fn read_control_snapshot(
        &self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ControlSnapshot, Self::Error>;

    /// Applies one guarded atomic control command.
    fn transact(
        &mut self,
        command: &Self::Command,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::ControlCommit, Self::Error>;

    /// Atomically advances the visible epoch under exact guards.
    fn compare_and_swap_visible_epoch(
        &mut self,
        guards: &Self::VisibleEpochGuards,
        prior_commit: &Self::ControlCommit,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::ControlCommit, Self::Error>;

    /// Loads the exact unresolved publication intent, when one exists.
    fn load_unresolved_publication(
        &self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<PublicationIntent>, Self::Error>;

    /// Durably quarantines contradictory control state.
    fn quarantine(
        &mut self,
        reason: &Self::QuarantineReason,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::QuarantineReceipt, Self::Error>;

    /// Reads bounded content-free write counters without creating query history.
    fn write_counters(
        &self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::JournalWriteCounters, Self::Error>;
}

/// Process-local immutable control-snapshot publisher.
pub trait ControlSnapshotPort: Port {
    /// Immutable snapshot type.
    type ControlSnapshot: Send + Sync + 'static;

    /// Returns the current process-local snapshot without a durable write.
    fn current_snapshot(&self) -> Result<Self::ControlSnapshot, Self::Error>;
}
