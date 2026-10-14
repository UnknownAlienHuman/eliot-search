//! Live consuming source-mapping continuation in the original inactive target.
use std::time::Instant;
use crate::migration::{SourceImportBinding, SourceImportCounts, SourceImportRow,
    SourceImportRecordArtifactPublication, SourceImportPublishedRecordArtifact,
    SourceMappingWriteState, SourceMappingComparison};
use crate::{ControlError, ControlKey, MigrationBootstrapWork};
use super::{MigrationBootstrap, MigrationBootstrapFailure, MigrationBootstrapPlatform,
    OriginalCheck, OPERATION_KEY, ATTEMPT_KEY, LIMITS};
use crate::persistent::operation::{Check, Point};

#[path = "source_stage/codec.rs"]
mod codec;

const STAGE_KEY: &[u8] = b"migration/source-stage/v1/binding";
const MAP_ATTEMPT_KEY: &[u8] = b"migration/source-stage/v1/map-attempt";

enum Phase { Prepared, Published, Writing, Unknown }

/// Opaque source-only continuation retaining the very same database owner.
/// No raw journal/database, serving snapshot or verified candidate is exposed.
pub struct OwnerBoundSourceStage {
    mapping: Option<SourceMappingWriteState>,
    bootstrap: MigrationBootstrap,
    binding: SourceImportBinding,
    profile: Vec<u8>,
    phase: Phase,
    published: Option<SourceImportPublishedRecordArtifact>,
}

impl MigrationBootstrap {
    /// Consumes only the original in-process bootstrap whose inventory completed.
    /// Current file names, digests or caller Booleans cannot construct this state.
    /// # Errors
    /// A missing/unknown proof, changed original binding or refused context blocks
    /// the continuation without opening a second database or rewriting old bytes.
    pub fn into_source_stage<F, E>(mut self, binding: SourceImportBinding,
        deadline: Instant, checkpoint: F,
    ) -> Result<OwnerBoundSourceStage, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            check.check(Point::Start)?;
            check.reserve(MigrationBootstrapWork::Allocate(4096))?;
            let proof = self.completed_inventory.as_ref().ok_or(ControlError::CommitOutcomeUnknown)?;
            if binding.target_namespace != self.binding.target
                || binding.legacy_namespace != self.binding.legacy_namespace
                || binding.catalog_snapshot != self.binding.catalog_snapshot
                || binding.mapping_profile.as_bytes() != &crate::migration::source_mapping_profile_digest() {
                return Err(ControlError::IdentityMismatch);
            }
            let snapshot = self.journal.verify_checked(&check)?;
            let original = snapshot.get(&ControlKey::new(OPERATION_KEY.to_vec(), LIMITS)?)
                .ok_or(ControlError::MigrationUnverified)?;
            let attempt = snapshot.get(&ControlKey::new(ATTEMPT_KEY.to_vec(), LIMITS)?)
                .ok_or(ControlError::MigrationUnverified)?;
            if original.as_bytes() != self.original || attempt.as_bytes() != proof.attempt {
                return Err(ControlError::IdentityMismatch);
            }
            let profile = codec::marker(binding, proof);
            self.write(STAGE_KEY, profile.clone(), None, OPERATION_KEY, &check)?;
            check.check(Point::MutationComplete)?;
            Ok(OwnerBoundSourceStage { mapping: None, bootstrap: self, binding,
                profile, phase: Phase::Prepared, published: None })
        })();
        result.map_err(|control| check.failure(control))
    }
}

impl OwnerBoundSourceStage {
    /// Records and dispatches the exact original source-map artifact once.
    /// # Errors
    /// Pre-dispatch unknown forbids publication; post-effect refusal is unknown
    /// and permanently prevents further stage mutations or cleanup.
    pub fn publish_source_map<P, F, E>(&mut self,
        publication: &mut SourceImportRecordArtifactPublication<P>, platform: &P,
        deadline: Instant, checkpoint: F,
    ) -> Result<(), MigrationBootstrapFailure<E>>
    where P: MigrationBootstrapPlatform, F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let mut dispatched = false;
        let result = (|| {
            if !matches!(self.phase, Phase::Prepared) { return Err(ControlError::CommitOutcomeUnknown); }
            self.phase = Phase::Unknown;
            check.check(Point::Start)?;
            check.reserve(MigrationBootstrapWork::Allocate(8192))?;
            let identity = platform.bootstrap_identity(publication.original_identity()
                .map_err(|_| ControlError::MigrationUnverified)?);
            let pending = codec::attempt(&self.profile, self.binding, publication, identity, 0)?;
            self.bootstrap.write_guarded(MAP_ATTEMPT_KEY, pending.clone(), None,
                STAGE_KEY, self.profile.clone(), &check)?;
            check.check(Point::BeforeWrite)?;
            dispatched = true;
            let published = publication.publish_once(deadline).map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check.check(Point::AfterCommit)?;
            if published.reused() { return Err(ControlError::CommitOutcomeUnknown); }
            let completed = codec::attempt(&self.profile, self.binding, publication, identity, 1)?;
            self.bootstrap.write_guarded(MAP_ATTEMPT_KEY, completed, Some(pending),
                STAGE_KEY, self.profile.clone(), &check)?;
            check.check(Point::MutationComplete)?;
            self.published = Some(published);
            self.phase = Phase::Published;
            Ok(())
        })();
        result.map_err(|control| check.failure(if dispatched { ControlError::CommitOutcomeUnknown } else { control }))
    }

    /// Initializes mapping tables in the existing private database, without open/create.
    /// # Errors
    /// Any uncertain table initialization retains this target and blocks use.
    pub fn initialize_mapping<F, E>(&mut self, deadline: Instant, checkpoint: F)
        -> Result<(), MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            if !matches!(self.phase, Phase::Published) { return Err(ControlError::CommitOutcomeUnknown); }
            self.phase = Phase::Unknown;
            check.check(Point::Start)?;
            check.reserve(MigrationBootstrapWork::Allocate(8192))?;
            // A capability obtained from the live original transition, never a
            // reader guessing a table set or adopting a caller's current identity.
            self.bootstrap.journal.table_profile = crate::persistent::TableProfile::SourceStage(self.profile.clone());
            let reserve = |work| check.reserve(work);
            let state = SourceMappingWriteState::initialize_in(&self.bootstrap.journal.database,
                self.binding, None, Some(&self.profile), deadline, &reserve)?;
            self.bootstrap.journal.verify_checked(&check)?;
            self.mapping = Some(state);
            self.phase = Phase::Writing;
            Ok(())
        })();
        result.map_err(|_| check.failure(ControlError::CommitOutcomeUnknown))
    }

    /// Imports one typed row through the existing bounded mapping/successor engine.
    /// # Errors
    /// Interrupted or inconsistent mapping blocks the entire stage.
    pub fn push<F, E>(&mut self, row: SourceImportRow, deadline: Instant, checkpoint: F)
        -> Result<(), MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            if !matches!(self.phase, Phase::Writing) { return Err(ControlError::CommitOutcomeUnknown); }
            let state = self.mapping.as_mut().ok_or(ControlError::MigrationUnverified)?;
            state.push(&self.bootstrap.journal.database, row, deadline, &|work| check.reserve(work))
        })();
        if result.is_err() { self.phase = Phase::Unknown; }
        result.map_err(|control| check.failure(control))
    }

    /// Seals exact counts and starts full readback on this very same database.
    /// # Errors
    /// No partial/unknown state or count equality can mint a readback result.
    pub fn begin_readback<F, E>(mut self, expected: SourceImportCounts,
        deadline: Instant, checkpoint: F,
    ) -> Result<OwnerSourceMappingReadback, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            if !matches!(self.phase, Phase::Writing) { return Err(ControlError::CommitOutcomeUnknown); }
            self.phase = Phase::Unknown;
            let reserve = |work| check.reserve(work);
            self.mapping.as_mut().ok_or(ControlError::MigrationUnverified)?
                .seal(&self.bootstrap.journal.database, expected, deadline, &reserve)?;
            self.bootstrap.journal.verify_checked(&check)?;
            let comparison = SourceMappingComparison::begin_in(&self.bootstrap.journal.database,
                self.binding, None, expected, Some(&self.profile), deadline, &reserve)?;
            Ok(OwnerSourceMappingReadback { comparison, owner: self, expected, latched: None })
        })();
        result.map_err(|control| check.failure(control))
    }
}

/// Full row comparison pins a read transaction before its retained database owner.
pub struct OwnerSourceMappingReadback {
    comparison: SourceMappingComparison,
    owner: OwnerBoundSourceStage,
    expected: SourceImportCounts,
    latched: Option<ControlError>,
}
impl OwnerSourceMappingReadback {
    /// Compares one independently recompiled exact source row and its indices.
    /// # Errors
    /// Refusal or mismatch prevents a receipt and cannot replay writes.
    pub fn compare<F, E>(&mut self, row: &SourceImportRow, deadline: Instant, checkpoint: F)
        -> Result<(), MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        if let Some(control) = self.latched { return Err(check.failure(control)); }
        match self.comparison.compare(row, deadline, &|work| check.reserve(work)) {
            Ok(()) => Ok(()),
            Err(control) => {
                let first = *self.latched.get_or_insert(control);
                Err(check.failure(first))
            }
        }
    }
    /// Completes the full denominator; the result still lacks complete candidate authority.
    /// # Errors
    /// Every incomplete or refused comparison produces no receipt.
    pub fn finish<F, E>(self, deadline: Instant, checkpoint: F)
        -> Result<OwnerSourceMappedReceipt, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let Self { comparison, owner, expected, latched } = self;
        if let Some(control) = latched { return Err(check.failure(control)); }
        let result = (|| {
            let verified = comparison.finish(deadline, &|work| check.reserve(work))?;
            if verified != expected { return Err(ControlError::MigrationUnverified); }
            owner.bootstrap.journal.verify_checked(&check)?;
            check.check(Point::ReadComplete)?;
            let artifact = owner.published.clone().ok_or(ControlError::MigrationUnverified)?;
            Ok(OwnerSourceMappedReceipt { owner, artifact, expected })
        })();
        result.map_err(|control| check.failure(control))
    }
}

/// Package-owned exact source-mapping result, never a full migration candidate.
pub struct OwnerSourceMappedReceipt {
    owner: OwnerBoundSourceStage,
    artifact: SourceImportPublishedRecordArtifact,
    expected: SourceImportCounts,
}
impl OwnerSourceMappedReceipt {
    /// Bounded metadata-only receipt with the remaining full-inventory gap explicit.
    #[must_use]
    pub fn render_json(&self) -> String {
        format!("{{\"profile\":\"eliot.owner-source-stage.v1\",\"target\":\"{}\",\"source_map\":\"{}\",\"source_map_chain_sha256\":\"{}\",\"events\":{},\"sources\":{},\"occurrences\":{},\"gap\":\"complete_inventory_candidate_not_performed\"}}",
            self.owner.bootstrap.target_name, self.artifact.name(), super::codec::hex(self.artifact.chain()),
            self.expected.events, self.expected.sources, self.expected.occurrences)
    }
}

#[cfg(test)]
#[path = "source_stage/tests.rs"]
mod tests;
