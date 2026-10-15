//! Closed Native DIRECT composition over the existing lifecycle engines/codecs.
use super::{Boundary, CACHE_BYTES, PersistentControlJournal};
use super::{
    bootstrap::OriginalCheck,
    operation::{Check, Point},
};
use crate::{
    ControlCommitReceipt, ControlError, ControlSnapshotPublisher, JournalIdentity, JournalLimits,
    JournalOperatingProfile, JournalProfileInitialization, MigrationBootstrapFailure,
    MigrationBootstrapWork, MutationId,
};
use std::{fs::File, time::Instant};

impl JournalIdentity {
    /// Complete identity as a canonical byte value using the existing disk codec.
    ///
    /// This encodes all six coordinates, including owner epoch and schema version.
    /// It is metadata, not evidence that an owner or native file is currently held.
    /// The caller must reserve preparation work before constructing the value.
    /// # Errors
    /// Returns the shared canonical byte-bound error if the owner codec exceeds it.
    pub fn canonical_binding_value(
        self,
    ) -> Result<search_contracts::CanonicalValue, search_contracts::ContractError> {
        search_contracts::BoundedBytes::new(super::codec::identity_bytes(self))
            .map(search_contracts::CanonicalValue::Bytes)
    }
}

struct DirectCheck<'a, F, E> {
    original: OriginalCheck<'a, F, E>,
    value: usize,
}
impl<F: Fn(MigrationBootstrapWork) -> Result<(), E>, E> Check for DirectCheck<'_, F, E> {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        use MigrationBootstrapWork as W;
        let work = match point {
            Point::BeforeOpen => {
                self.reserve(W::Allocate(CACHE_BYTES))?;
                W::Open
            }
            Point::BeforeWrite | Point::BeforeCommit => W::Write(0),
            Point::StageRecord => W::Write(self.value),
            Point::ReadHeader | Point::ReadRecord | Point::ReadOperation | Point::Readback => {
                W::Read(self.value)
            }
            _ => W::Checkpoint,
        };
        self.reserve(work)
    }
    fn reserve(&self, work: MigrationBootstrapWork) -> Result<(), ControlError> {
        Check::reserve(&self.original, work)
    }
}
impl PersistentControlJournal {
    /// Create only an exclusively caller-created empty file, using its original work/deadline.
    /// # Errors
    /// Uses the existing lifecycle classifications; possible initialization stays unknown.
    pub fn create_direct_with_original_context<
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
        E,
    >(
        file: File,
        identity: JournalIdentity,
        limits: JournalLimits,
        deadline: Instant,
        work: &F,
    ) -> Result<Self, MigrationBootstrapFailure<E>> {
        let check = DirectCheck {
            original: OriginalCheck::registry(deadline, work),
            value: limits.max_value_bytes,
        };
        Self::create_checked(file, identity, limits, &check).map_err(|e| check.original.failure(e))
    }
    /// Existing-only open; no missing tables, identity or profile is invented.
    /// # Errors
    /// Preserves corruption, identity and possible native recovery failures.
    pub fn open_direct_with_original_context<F: Fn(MigrationBootstrapWork) -> Result<(), E>, E>(
        file: File,
        identity: JournalIdentity,
        limits: JournalLimits,
        deadline: Instant,
        work: &F,
    ) -> Result<Self, MigrationBootstrapFailure<E>> {
        let check = DirectCheck {
            original: OriginalCheck::registry(deadline, work),
            value: limits.max_value_bytes,
        };
        Self::open_checked(file, identity, limits, &check).map_err(|e| check.original.failure(e))
    }
    /// Initialize the existing immutable Direct profile under the original invocation.
    /// # Errors
    /// Conflicting/non-genesis/unknown state refuses without adoption or write replay.
    pub fn initialize_direct_with_original_context<
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
        E,
    >(
        &mut self,
        operation: MutationId,
        digest: search_contracts::Blake3Digest32,
        deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>> {
        let check = DirectCheck {
            original: OriginalCheck::registry(deadline, work),
            value: self.limits.max_value_bytes,
        };
        self.initialize_profile_checked(
            JournalProfileInitialization::Direct,
            operation,
            digest,
            Boundary::Normal,
            &check,
        )
        .map_err(|e| check.original.failure(e))
    }
    /// Consuming, exact next-owner handoff; never selects a stored epoch from untrusted bytes.
    /// # Errors
    /// Skips/identity changes refuse; possible writes retain the existing unknown outcome.
    pub fn advance_direct_with_original_context<
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
        E,
    >(
        self,
        next: JournalIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<Self, MigrationBootstrapFailure<E>> {
        let check = DirectCheck {
            original: OriginalCheck::registry(deadline, work),
            value: self.limits.max_value_bytes,
        };
        self.advance_owner_checked(next, &check)
            .map_err(|e| check.original.failure(e))
    }
    /// Verify the real Direct profile and exact original genesis receipt, never Inert/Indexed.
    /// # Errors
    /// Missing/foreign/pending/corrupt profile or exhausted original work refuses.
    pub fn verify_direct_with_original_context<
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
        E,
    >(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<(), MigrationBootstrapFailure<E>> {
        let check = DirectCheck {
            original: OriginalCheck::registry(deadline, work),
            value: self.limits.max_value_bytes,
        };
        let result = (|| {
            self.ensure_available()?;
            if self.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION
                || self.profile_checked(&check)? != JournalOperatingProfile::Direct
            {
                return Err(ControlError::SchemaUnsupported);
            }
            self.verify_checked(&check).map(|_| ())
        })();
        result.map_err(|e| check.original.failure(e))
    }
    /// Publish only verified current disk metadata through the same publisher.
    /// # Errors
    /// Failure leaves its admission suspended; this grants no native/current-policy authority.
    pub fn publish_direct_with_original_context<
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
        E,
    >(
        &self,
        publisher: &mut ControlSnapshotPublisher,
        deadline: Instant,
        work: &F,
    ) -> Result<(), MigrationBootstrapFailure<E>> {
        let check = DirectCheck {
            original: OriginalCheck::registry(deadline, work),
            value: self.limits.max_value_bytes,
        };
        self.recover_publication_checked(publisher, &check)
            .map(|_| ())
            .map_err(|e| check.original.failure(e))
    }
}
