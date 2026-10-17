//! Typed reference population from the existing journal, never source authority.

use super::{
    Check, ControlError, MANIFESTS, PersistentControlJournal, Point, PublicationSourceShadow,
    PublicationVisibilityState, SHADOWS, codec,
};
use crate::{JournalIdentity, MigrationBootstrapWork as Work, PreparedArtifactSetRecordV1};
use search_contracts::{Epoch, ProjectionMembershipId, PublicationIntent, ReceiptRef};
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
    initial_route_revision: Option<search_contracts::CollectionRouteRevision>,
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
    /// Initial revision of this snapshot's matching schema-5 genesis/adoption route.
    ///
    /// Revision one applies only while the immutable initial generation and
    /// schema still match this current snapshot.
    /// Legacy or replaced routes return None and require their own route owner.
    /// This technical coordinate does not qualify the physical Qdrant route.
    #[must_use]
    pub const fn initial_route_revision(
        &self,
    ) -> Option<search_contracts::CollectionRouteRevision> {
        self.initial_route_revision
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
            .field(
                "prepared_artifact_set_count",
                &self.prepared_artifact_sets.len(),
            )
            .field("shadow_count", &self.shadows.len())
            .finish_non_exhaustive()
    }
}

/// Coherent technical references for restoring an unresolved publication.
///
/// This is recovery input only. It does not make the route query-ready, grant
/// publication permission, or prove that referenced C07 objects remain valid.
/// Private construction binds every field to one verified C02 read transaction.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationRecoveryProjectionSnapshot {
    identity: JournalIdentity,
    generation: u64,
    visibility: PublicationVisibilityState,
    initial_route_revision: Option<search_contracts::CollectionRouteRevision>,
    intent: Option<PublicationIntent>,
    last_reserved_epoch: Epoch,
    manifests: BTreeMap<ProjectionMembershipId, ReceiptRef>,
    prepared_artifact_sets: BTreeMap<ProjectionMembershipId, PreparedArtifactSetRecordV1>,
    shadows: BTreeMap<ProjectionMembershipId, PublicationSourceShadow>,
}

impl PublicationRecoveryProjectionSnapshot {
    /// Exact installation/root/schema/owner identity observed on disk.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity {
        self.identity
    }
    /// Generation shared by visibility, intent, reservation floor and references.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Actual committed visibility and owner guards; unresolved intent does not alter it.
    #[must_use]
    pub const fn visibility(&self) -> &PublicationVisibilityState {
        &self.visibility
    }
    /// Initial route coordinate of this verified immutable genesis/adoption route, if any.
    #[must_use]
    pub const fn initial_route_revision(
        &self,
    ) -> Option<search_contracts::CollectionRouteRevision> {
        self.initial_route_revision
    }
    /// Last actual durable intent from the same read, including unresolved terminal states.
    #[must_use]
    pub const fn intent(&self) -> Option<&PublicationIntent> {
        self.intent.as_ref()
    }
    /// Consumed epoch floor from the retained intent, or committed visibility when no intent exists.
    #[must_use]
    pub const fn last_reserved_epoch(&self) -> Epoch {
        self.last_reserved_epoch
    }
    /// Current membership-to-manifest references; no point data is stored here.
    #[must_use]
    pub const fn manifests(&self) -> &BTreeMap<ProjectionMembershipId, ReceiptRef> {
        &self.manifests
    }
    /// Existing bounded v1 metadata selected by current manifest pointers.
    #[must_use]
    pub const fn prepared_artifact_sets(
        &self,
    ) -> &BTreeMap<ProjectionMembershipId, PreparedArtifactSetRecordV1> {
        &self.prepared_artifact_sets
    }
    /// Source-specific shadow records from the same verified read.
    #[must_use]
    pub const fn shadows(&self) -> &BTreeMap<ProjectionMembershipId, PublicationSourceShadow> {
        &self.shadows
    }
}

impl core::fmt::Debug for PublicationRecoveryProjectionSnapshot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PublicationRecoveryProjectionSnapshot")
            .field("generation", &self.generation)
            .field("visible_epoch", &self.visibility.visible_epoch)
            .field("has_intent", &self.intent.is_some())
            .field("manifest_count", &self.manifests.len())
            .field(
                "prepared_artifact_set_count",
                &self.prepared_artifact_sets.len(),
            )
            .field("shadow_count", &self.shadows.len())
            .finish_non_exhaustive()
    }
}

struct VerifiedProjectionRead {
    snapshot: super::super::super::JournalReadSnapshot,
    visibility: PublicationVisibilityState,
}

struct ProjectionReferences {
    initial_route_revision: Option<search_contracts::CollectionRouteRevision>,
    manifests: BTreeMap<ProjectionMembershipId, ReceiptRef>,
    prepared_artifact_sets: BTreeMap<ProjectionMembershipId, PreparedArtifactSetRecordV1>,
    shadows: BTreeMap<ProjectionMembershipId, PublicationSourceShadow>,
}

impl PersistentControlJournal {
    pub(in crate::persistent) fn read_projection_snapshot_checked(
        &self,
        check: &dyn Check,
    ) -> Result<Option<PublicationProjectionSnapshot>, ControlError> {
        let Some(read) = self.read_verified_projection_state_checked(check)? else {
            return Ok(None);
        };
        // Recovery checkpoints intentionally retain unresolved work. This serving
        // input must not turn it into a ready reference population.
        super::super::require_resolved(&read.snapshot.records)?;
        validate_projection_owner(&read)?;
        let references = self.read_projection_references_checked(&read, check)?;
        check.check(Point::ReadComplete)?;
        Ok(Some(PublicationProjectionSnapshot {
            identity: read.snapshot.identity,
            generation: read.snapshot.generation,
            visibility: read.visibility,
            initial_route_revision: references.initial_route_revision,
            manifests: references.manifests,
            prepared_artifact_sets: references.prepared_artifact_sets,
            shadows: references.shadows,
        }))
    }

    pub(in crate::persistent) fn read_recovery_projection_snapshot_checked(
        &self,
        check: &dyn Check,
    ) -> Result<Option<PublicationRecoveryProjectionSnapshot>, ControlError> {
        let Some(read) = self.read_verified_projection_state_checked(check)? else {
            return Ok(None);
        };
        // A verified owner handoff advances only the journal header. Recovery
        // checkpoint semantics retain the prior visibility/intent owner epoch
        // so the caller can classify that immutable state without serving it.
        let intent = read_publication_intent(&read, check)?;
        let last_reserved_epoch = intent
            .as_ref()
            .map_or(read.visibility.visible_epoch, |intent| intent.target_epoch);
        let references = self.read_projection_references_checked(&read, check)?;
        check.check(Point::ReadComplete)?;
        Ok(Some(PublicationRecoveryProjectionSnapshot {
            identity: read.snapshot.identity,
            generation: read.snapshot.generation,
            visibility: read.visibility,
            initial_route_revision: references.initial_route_revision,
            intent,
            last_reserved_epoch,
            manifests: references.manifests,
            prepared_artifact_sets: references.prepared_artifact_sets,
            shadows: references.shadows,
        }))
    }

    fn read_verified_projection_state_checked(
        &self,
        check: &dyn Check,
    ) -> Result<Option<VerifiedProjectionRead>, ControlError> {
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
        let visibility = if let Ok(index) = snapshot
            .records
            .binary_search_by(|(key, _)| key.as_bytes().cmp(super::STATE))
        {
            codec::read_state(&snapshot.records[index].1)?
        } else {
            check.check(Point::ReadComplete)?;
            return Ok(None);
        };
        Ok(Some(VerifiedProjectionRead {
            snapshot,
            visibility,
        }))
    }

    fn read_projection_references_checked(
        &self,
        read: &VerifiedProjectionRead,
        check: &dyn Check,
    ) -> Result<ProjectionReferences, ControlError> {
        let initial_route_revision = crate::persistent::operating_profile::initial_route_revision(
            &read.snapshot,
            &read.visibility,
            self.limits,
        )?;
        let mut manifests = BTreeMap::new();
        let mut prepared_artifact_sets = BTreeMap::new();
        let mut shadows = BTreeMap::new();
        for (key, value) in &read.snapshot.records {
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
                    &read.snapshot,
                    &reference,
                    self.limits,
                    check,
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
        Ok(ProjectionReferences {
            initial_route_revision,
            manifests,
            prepared_artifact_sets,
            shadows,
        })
    }
}

fn validate_projection_owner(read: &VerifiedProjectionRead) -> Result<(), ControlError> {
    if read.visibility.guards.owner_epoch != read.snapshot.identity.owner_epoch {
        return Err(ControlError::GenerationMismatch);
    }
    Ok(())
}

fn read_publication_intent(
    read: &VerifiedProjectionRead,
    check: &dyn Check,
) -> Result<Option<PublicationIntent>, ControlError> {
    check.check(Point::ReadRecord)?;
    let Ok(index) = read
        .snapshot
        .records
        .binary_search_by(|(key, _)| key.as_bytes().cmp(super::super::KEY))
    else {
        // verify_from_checked verified never-written history in this same read.
        return Ok(None);
    };
    let value = &read.snapshot.records[index].1;
    let work = value
        .len()
        .checked_add(core::mem::size_of::<PublicationIntent>())
        .ok_or(ControlError::BudgetExceeded)?;
    check.reserve(Work::Allocate(work))?;
    check.reserve(Work::Process(value.len()))?;
    super::super::codec::decode(value).map(Some)
}
