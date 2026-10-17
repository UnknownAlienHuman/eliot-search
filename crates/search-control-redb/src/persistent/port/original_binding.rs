//! Bind the existing control owner without performing unscoped profile reads.

use core::marker::PhantomData;
use std::time::Instant;

use super::super::{
    bootstrap::OriginalCheck,
    operation::{Check, Point},
};
use super::BoundControlJournal;
use crate::{
    ControlError, ControlSnapshotPublisher, MigrationBootstrapFailure, MigrationBootstrapWork,
    PersistentControlJournal,
};
use search_ports::CancellationProbe;

impl PersistentControlJournal {
    /// Bind the same journal/publisher while charging profile validation to the original call.
    ///
    /// No file, store, deadline or operation ledger is created. Context refusal before profile
    /// validation leaves publisher admission untouched. Later refusal remains fail closed.
    ///
    /// # Errors
    /// Refuses expired/cancelled/over-budget calls, unsupported profiles and foreign publishers.
    pub fn bind_control_port_with_original_context<'a, C: CancellationProbe, E>(
        &'a mut self,
        publisher: &'a mut ControlSnapshotPublisher,
        original_deadline: Instant,
        work: &dyn Fn(MigrationBootstrapWork) -> Result<(), E>,
    ) -> Result<BoundControlJournal<'a, C>, MigrationBootstrapFailure<E>> {
        let callback = |unit| work(unit);
        let check = OriginalCheck::registry(original_deadline, &callback);
        let result = (|| {
            check.check(Point::Start)?;
            if !crate::has_visibility_schema(self.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            self.ensure_available()?;
            self.require_normal_profile(&check)?;
            check.check(Point::ReadComplete)?;
            publisher.begin_disk_publication(self.identity)
        })();
        result.map_err(|error| check.failure(error))?;
        Ok(BoundControlJournal {
            journal: self,
            publisher,
            cancellation: PhantomData,
        })
    }
}
