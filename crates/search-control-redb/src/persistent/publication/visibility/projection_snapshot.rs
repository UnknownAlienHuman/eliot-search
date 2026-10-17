//! Typed reference population from the existing journal, never source authority.

use super::{
    Check, ControlError, MANIFESTS, PersistentControlJournal, Point, PublicationSourceShadow,
    PublicationVisibilityState, SHADOWS, codec,
};
use crate::{JournalIdentity, MigrationBootstrapWork as Work, PreparedArtifactSetRecordV1};
use search_contracts::{ProjectionMembershipId, ReceiptRef};
use std::collections::BTreeMap;

/// Exact committed projection references and shadow records from one disk read.
///
/// Private construction binds all fields to the verified journal transaction.
/// This is immutable technical data, not access permission, source qualification
/// or proof that a referenced C07 object is currently available.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationProjectionSnapshot {
    identity: JournalIdentity,
    generation: u64,
    visibility: PublicationVisibilityState,
    manifests: BTreeMap<ProjectionMembershipId, ReceiptRef>,
    prepared_artifact_sets: BTreeMap<ProjectionMembershipId, PreparedArtifactSetRecordV1>,
    shadows: BTreeMap<ProjectionMembershipId, PublicationSourceShadow>,
}
impl PublicationProjectionSnapshot {
    /// Exact installation/root/schema/owner identity observed on disk.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity {
        self.identity
    }
    /// Control generation shared by every returned reference and fence.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Committed collection generation, schema, visible epoch and owner guards.
    #[must_use]
    pub const fn visibility(&self) -> &PublicationVisibilityState {
        &self.visibility
    }
    /// Current exact membership-to-manifest references; no point data is stored here.
    #[must_use]
    pub const fn manifests(&self) -> &BTreeMap<ProjectionMembershipId, ReceiptRef> {
        &self.manifests
    }
    /// Full existing v1 records selected by the current manifest pointers in
    /// this same disk transaction. Legacy references have no v1 entry. Root
    /// must still verify each exact C07 object and its source/current policies.
    #[must_use]
    pub const fn prepared_artifact_sets(
        &self,
    ) -> &BTreeMap<ProjectionMembershipId, PreparedArtifactSetRecordV1> {
        &self.prepared_artifact_sets
    }
    /// Source-specific shadows retained by the same committed snapshot.
    #[must_use]
    pub const fn shadows(&self) -> &BTreeMap<ProjectionMembershipId, PublicationSourceShadow> {
        &self.shadows
    }
}
impl core::fmt::Debug for PublicationProjectionSnapshot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PublicationProjectionSnapshot")
            .field("generation", &self.generation)
            .field("visible_epoch", &self.visibility.visible_epoch)
            .field("manifest_count", &self.manifests.len())
            .field("prepared_artifact_set_count", &self.prepared_artifact_sets.len())
            .field("shadow_count", &self.shadows.len())
            .finish_non_exhaustive()
    }
}

impl PersistentControlJournal {
    pub(in crate::persistent) fn read_projection_snapshot_checked(
        &self,
        check: &dyn Check,
    ) -> Result<Option<PublicationProjectionSnapshot>, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        if !crate::has_visibility_schema(self.identity.schema_version) {
            return Err(ControlError::SchemaUnsupported);
        }
        let read = self
            .database
            .begin_read()
            .map_err(|_| ControlError::StoreUnavailable)?;
        let snapshot = self.verify_from_checked(&read, check)?;
        if self.identity.schema_version == crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            // Decode the profile from this verified snapshot. The legacy
            // require_indexed helper opens an unscoped second transaction.
            check.reserve(Work::Allocate(self.limits.max_value_bytes))?;
            check.reserve(Work::Process(self.limits.max_value_bytes))?;
            if crate::persistent::operating_profile::snapshot_mode(&snapshot, self.limits)?
                != crate::JournalOperatingProfile::Indexed
            {
                return Err(ControlError::MigrationUnverified);
            }
        }
        // verify_from_checked already validates visibility and its receipt ledger
        // in this transaction. Decode that same value without a second scan.
        let visibility = if let Ok(index) = snapshot
            .records
            .binary_search_by(|(key, _)| key.as_bytes().cmp(super::STATE))
        {
            codec::read_state(&snapshot.records[index].1)?
        } else {
            check.check(Point::ReadComplete)?;
            return Ok(None);
        };
        // Recovery checkpoints intentionally retain unresolved work. This serving
        // input must not turn it into a ready reference population.
        super::super::require_resolved(&snapshot.records)?;
        if visibility.guards.owner_epoch != snapshot.identity.owner_epoch {
            return Err(ControlError::GenerationMismatch);
        }
        let mut manifests = BTreeMap::new();
        let mut prepared_artifact_sets = BTreeMap::new();
        let mut shadows = BTreeMap::new();
        for (key, value) in &snapshot.records {
            check.check(Point::ReadRecord)?;
            let (suffix, manifest) = if let Some(id) = key.as_bytes().strip_prefix(MANIFESTS) {
                (id, true)
            } else if let Some(id) = key.as_bytes().strip_prefix(SHADOWS) {
                (id, false)
            } else {
                continue;
            };
            let id = ProjectionMembershipId::from_bytes(
                suffix.try_into().map_err(|_| ControlError::StoreCorrupt)?,
            );
            let allocation = value
                .len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(256))
                .ok_or(ControlError::BudgetExceeded)?;
            check.reserve(Work::Allocate(allocation))?;
            check.reserve(Work::Process(
                value
                    .len()
                    .checked_add(key.as_bytes().len())
                    .ok_or(ControlError::BudgetExceeded)?,
            ))?;
            let duplicate = if manifest {
                let reference = codec::read_manifest(value)?;
                if let Some(record) = super::super::prepared_artifacts::read_reference(
                    &snapshot, &reference, self.limits, check,
                )? {
                    check.reserve(Work::Allocate(256))?;
                    if prepared_artifact_sets.insert(id, record).is_some() {
                        return Err(ControlError::StoreCorrupt);
                    }
                }
                manifests.insert(id, reference).is_some()
            } else {
                shadows.insert(id, codec::read_shadow(value)?).is_some()
            };
            if duplicate {
                return Err(ControlError::StoreCorrupt);
            }
        }
        check.check(Point::ReadComplete)?;
        Ok(Some(PublicationProjectionSnapshot {
            identity: snapshot.identity,
            generation: snapshot.generation,
            visibility,
            manifests,
            prepared_artifact_sets,
            shadows,
        }))
    }
}
