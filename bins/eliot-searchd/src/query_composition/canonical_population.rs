//! Root-owned C18 population compilation from the current C02/C03/C07 metadata image.
//!
//! The retained image is captured at native-root startup or a visibility update.
//! Query turns only filter these bounded immutable rows; they do not reopen C07
//! or source bodies. The image is data, never a read permit.

use std::collections::BTreeMap;

use search_access::{
    AccessCheckpoint, AccessError, AuthoritativeAccessSnapshot, AuthoritativePolicyState,
    CanonicalEligibilityPopulationRequest, CanonicalEligibilitySecuritySnapshot,
    CanonicalPopulationVerificationPort, CanonicalProjectionMembership,
    CompiledEligibilityPopulation, IndexedRouteFence, LiveSecurityState, NamespacePolicyFence,
    RequestedMembershipScope, compile_canonical_eligibility_population,
};
use search_contracts::{
    CollectionRoute, MAX_COLLECTION_ITEMS, ProjectionMembershipId, QuerySnapshotFence, ReceiptRef,
    RequestBody, SourceIdentity, SourceMembershipId, SourceOwnerGeneration, SourceRevisionRef,
    SourceView, indexed::CollectionSchema,
};
use search_control_redb::{
    PreparedArtifactSetRecordV1, PublicationProjectionSnapshot, PublicationSourceShadow,
    PublicationVisibilityState,
};
use search_revision_store::derived_artifact::{
    DerivedArtifactKindV1, DerivedArtifactObjectRefV1, DerivedArtifactSetDescriptorV1,
};
use search_source_registry::api::ProjectionMembershipRecord;

use crate::access_composition::{
    AuthoritativeGrantPolicy, GrantUseError, NativeSecurityDomain,
    StandaloneMembershipSecuritySnapshot, StandalonePreRetrievalAdmission, VerifiedStandaloneGrant,
};
use crate::owner_composition::NativePublishedProjectionRead;

/// Private, immutable query-time metadata copied from real native published reads.
/// Construction accepts the actual C02 snapshot and C07 header-only read results;
/// there is no constructor for supplied C00 rows or verification flags.
#[derive(Eq, PartialEq)]
pub struct CanonicalPopulationSnapshot {
    c02_generation: u64,
    visibility: PublicationVisibilityState,
    manifests: BTreeMap<ProjectionMembershipId, ReceiptRef>,
    shadows: BTreeMap<ProjectionMembershipId, PublicationSourceShadow>,
    schema: CollectionSchema,
    route: CollectionRoute,
    fence: QuerySnapshotFence,
    rows: BTreeMap<SourceMembershipId, CanonicalProjectionMetadataRow>,
}

#[derive(Eq, PartialEq)]
struct CanonicalProjectionMetadataRow {
    c02_record: PreparedArtifactSetRecordV1,
    c03: ProjectionMembershipRecord,
    source_identity: SourceIdentity,
    source_owner_generation: SourceOwnerGeneration,
    source_registry_generation: u64,
    source_revision: SourceRevisionRef,
    c07_set_reference: DerivedArtifactObjectRefV1,
    c07_set_graph: DerivedArtifactSetDescriptorV1,
}

/// Current values borrowed directly from `CanonicalQueryAuthority` while its
/// owner lock is held. Callers cannot replace these with request-derived copies.
#[allow(clippy::redundant_pub_crate)] // Used only by private Root/query composition modules.
pub(crate) struct CanonicalPopulationLiveAuthority<'a> {
    policy: &'a AuthoritativeGrantPolicy,
    access: &'a AuthoritativeAccessSnapshot,
    membership_security: &'a StandaloneMembershipSecuritySnapshot,
    sources: &'a BTreeMap<SourceMembershipId, SourceIdentity>,
    planned_policy: &'a NamespacePolicyFence,
    authoritative_policy: &'a AuthoritativePolicyState,
    route: IndexedRouteFence,
    visibility: &'a PublicationVisibilityState,
    live: &'a LiveSecurityState,
    domain: &'a NativeSecurityDomain,
}

impl<'a> CanonicalPopulationLiveAuthority<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn borrow_current(
        policy: &'a AuthoritativeGrantPolicy,
        access: &'a AuthoritativeAccessSnapshot,
        membership_security: &'a StandaloneMembershipSecuritySnapshot,
        sources: &'a BTreeMap<SourceMembershipId, SourceIdentity>,
        planned_policy: &'a NamespacePolicyFence,
        authoritative_policy: &'a AuthoritativePolicyState,
        route: IndexedRouteFence,
        visibility: &'a PublicationVisibilityState,
        live: &'a LiveSecurityState,
        domain: &'a NativeSecurityDomain,
    ) -> Self {
        Self {
            policy,
            access,
            membership_security,
            sources,
            planned_policy,
            authoritative_policy,
            route,
            visibility,
            live,
            domain,
        }
    }
}

impl CanonicalPopulationSnapshot {
    /// Capture only data from the same actual C02 snapshot used by every native
    /// metadata read. Reads contain authenticated C07 graph headers and no bodies.
    pub(crate) fn capture(
        c02: &PublicationProjectionSnapshot,
        schema: CollectionSchema,
        route: CollectionRoute,
        fence: QuerySnapshotFence,
        reads: &[NativePublishedProjectionRead<'_, '_>],
    ) -> Result<Self, AccessError> {
        if reads.is_empty() {
            return Err(AccessError::AuthorizedScopeEmpty);
        }
        if reads.len() > MAX_COLLECTION_ITEMS {
            return Err(AccessError::EligibilityPopulationTooLarge);
        }
        let initial_route_revision = c02.initial_route_revision()
            .ok_or(AccessError::RouteMismatch)?;
        if route.collection_route_revision != initial_route_revision
            || fence.collection_route_revision != initial_route_revision
        {
            return Err(AccessError::RouteMismatch);
        }
        let mut snapshot = Self {
            c02_generation: c02.generation(),
            visibility: *c02.visibility(),
            manifests: BTreeMap::new(),
            shadows: BTreeMap::new(),
            schema,
            route,
            fence,
            rows: BTreeMap::new(),
        };
        snapshot.validate_coordinates()?;

        for read in reads {
            if !std::ptr::eq(read.snapshot(), c02) || read.generation() != c02.generation() {
                return Err(AccessError::SnapshotStale);
            }
            let projection_id = read.membership();
            let c03 = read.projection_membership();
            let binding = c03.binding();
            let pointer = c02
                .manifests()
                .get(&projection_id)
                .ok_or(AccessError::SnapshotStale)?;
            let record = c02
                .prepared_artifact_sets()
                .get(&projection_id)
                .ok_or(AccessError::SnapshotStale)?;
            let metadata = record.metadata();
            let graph_member = read
                .set_graph()
                .members()
                .get(3)
                .ok_or(AccessError::SnapshotStale)?;
            let projection_descriptor = graph_member.descriptor();
            if binding.projection_membership_id != projection_id
                || record != read.record()
                || record.reference() != pointer
                || read.source_revision().source_namespace_id
                    != read.source_identity().source_namespace_id
                || read.source_revision().source_id != read.source_identity().source_id
                || metadata.source_revision != read.source_revision()
                || metadata.representation_id != binding.representation_id
                || read.set_reference().descriptor().kind() != DerivedArtifactKindV1::SetManifest
                || read.set_reference().descriptor().source() != &read.source_revision()
                || read.set_reference().descriptor().representation_id()
                    != binding.representation_id
                || projection_descriptor.kind() != DerivedArtifactKindV1::ProjectionManifest
                || projection_descriptor.source() != &read.source_revision()
                || projection_descriptor.representation_id() != binding.representation_id
            {
                return Err(AccessError::SnapshotStale);
            }
            let row = CanonicalProjectionMetadataRow {
                c02_record: record.clone(),
                c03: c03.clone(),
                source_identity: read.source_identity().clone(),
                source_owner_generation: read.source_owner_generation(),
                source_registry_generation: read.source_registry_generation(),
                source_revision: read.source_revision(),
                c07_set_reference: read.set_reference().clone(),
                c07_set_graph: read.set_graph().clone(),
            };
            if snapshot
                .manifests
                .insert(projection_id, pointer.clone())
                .is_some()
            {
                return Err(AccessError::SnapshotStale);
            }
            if let Some(shadow) = c02.shadows().get(&projection_id) {
                snapshot.shadows.insert(projection_id, *shadow);
            }
            if snapshot
                .rows
                .insert(binding.source_membership_id, row)
                .is_some()
            {
                return Err(AccessError::SnapshotStale);
            }
        }
        Ok(snapshot)
    }

    fn validate_coordinates(&self) -> Result<(), AccessError> {
        self.schema
            .validate()
            .map_err(|_| AccessError::RouteMismatch)?;
        self.fence
            .validate()
            .map_err(|_| AccessError::SnapshotStale)?;
        let visibility = &self.visibility;
        let schema_identity = self
            .schema
            .identity()
            .map_err(|_| AccessError::RouteMismatch)?;
        if schema_identity.as_digest() != &visibility.schema_identity_digest
            || self.route.schema_identity_digest != visibility.schema_identity_digest
            || self.route.collection_generation_id != visibility.collection_generation_id
            || self.route.committed_visible_epoch != visibility.visible_epoch
            || self.route.collection_route_revision != self.fence.collection_route_revision
            || self.fence.installation_incarnation_id != *self.schema.installation_incarnation_id()
            || self.fence.collection_generation_id != Some(*self.schema.collection_generation_id())
            || self.fence.visible_epoch != Some(visibility.visible_epoch)
        {
            return Err(AccessError::RouteMismatch);
        }
        Ok(())
    }

    pub(crate) fn validate_live(
        &self,
        current: &CanonicalPopulationLiveAuthority<'_>,
    ) -> Result<(), AccessError> {
        self.validate_coordinates()?;
        if &self.visibility != current.visibility
            || self.route.collection_generation_id != current.route.collection_generation_id
            || self.route.committed_visible_epoch != current.route.visible_epoch
            || self.route.collection_route_revision.get() != current.route.route_generation
        {
            return Err(AccessError::RouteMismatch);
        }
        if current.visibility.guards.owner_epoch != current.route.owner_epoch {
            return Err(AccessError::RouteMismatch);
        }
        if self.fence.installation_incarnation_id != current.policy.installation_incarnation_id {
            return Err(AccessError::InstallationMismatch);
        }
        if current.planned_policy.namespace_id != current.authoritative_policy.namespace_id {
            return Err(AccessError::NamespaceUnknown);
        }
        if current.planned_policy.owner_generation != current.authoritative_policy.owner_generation
        {
            return Err(AccessError::OwnerMismatch);
        }
        if current.planned_policy.policy_revision != current.authoritative_policy.policy_revision
            || self.fence.access_policy_revision != current.authoritative_policy.policy_revision
            || self.fence.shadow_fence_revision != current.authoritative_policy.shadow_revision
            || self.fence.purge_fence_revision != current.authoritative_policy.purge_revision
        {
            return Err(AccessError::PolicyRevisionStale);
        }
        if self.fence.catalog_revision.get() != current.access.source_catalog_generation
            || self.fence.membership_revision.get() != current.access.membership_generation
            || self.rows.len() != current.sources.len()
            || self.rows.keys().ne(current.sources.keys())
        {
            return Err(AccessError::SnapshotStale);
        }

        for (source_membership, row) in &self.rows {
            let binding = row.c03.binding();
            let identity = current
                .sources
                .get(source_membership)
                .ok_or(AccessError::ScopeUnknown)?;
            let access = current
                .access
                .bindings
                .get(source_membership)
                .ok_or(AccessError::SnapshotStale)?;
            let member_security = current
                .membership_security
                .memberships
                .get(source_membership)
                .ok_or(AccessError::SnapshotStale)?;
            let pointer = self
                .manifests
                .get(&binding.projection_membership_id)
                .ok_or(AccessError::SnapshotStale)?;
            let record = &row.c02_record;
            let projection_descriptor = row
                .c07_set_graph
                .members()
                .get(3)
                .ok_or(AccessError::SnapshotStale)?
                .descriptor();
            if identity.source_namespace_id != current.authoritative_policy.namespace_id {
                return Err(AccessError::NamespaceUnknown);
            }
            if row.source_owner_generation != current.authoritative_policy.owner_generation {
                return Err(AccessError::OwnerMismatch);
            }
            if binding.source_membership_id != *source_membership
                || &row.source_identity != identity
                || row.source_registry_generation != current.access.source_catalog_generation
                || record.reference() != pointer
                || row.source_revision.source_namespace_id != identity.source_namespace_id
                || row.source_revision.source_id != identity.source_id
                || access.membership_id != *source_membership
                || !access.active
                || member_security.access_partition_id != binding.access_partition_id
                || access.access_partition_digest != member_security.access_partition_digest
                || record.metadata().source_revision != row.source_revision
                || record.metadata().representation_id != binding.representation_id
                || row.c07_set_reference.descriptor().source() != &row.source_revision
                || row.c07_set_reference.descriptor().representation_id()
                    != binding.representation_id
                || projection_descriptor.source() != &row.source_revision
                || projection_descriptor.representation_id() != binding.representation_id
                || row.c07_set_reference.descriptor().kind() != DerivedArtifactKindV1::SetManifest
                || projection_descriptor.kind() != DerivedArtifactKindV1::ProjectionManifest
            {
                return Err(AccessError::SnapshotStale);
            }
        }
        Ok(())
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.c02_generation
    }

    pub(crate) const fn fence(&self) -> &QuerySnapshotFence {
        &self.fence
    }
}

/// Compile C18 using only current query authority and rows captured from real
/// C02/C03/C07 owners. The issuer token is checked by the compiler port and the
/// same live domain is checkpointed before and after compilation.
#[allow(clippy::too_many_arguments)]
pub(super) fn compile_current_population(
    metadata: &CanonicalPopulationSnapshot,
    current: &CanonicalPopulationLiveAuthority<'_>,
    request: &RequestBody,
    issued: &VerifiedStandaloneGrant<'_>,
    admission: &StandalonePreRetrievalAdmission,
    requested_scope: &RequestedMembershipScope,
) -> Result<CompiledEligibilityPopulation, AccessError> {
    metadata.validate_live(current)?;
    if request.recipe_request.source_view != metadata.fence.source_view {
        return Err(AccessError::SnapshotStale);
    }
    checkpoint_live(current, &admission.request_fence)?;

    let mut projection_memberships =
        BTreeMap::<ProjectionMembershipId, CanonicalProjectionMembership>::new();
    let mut modality = None;
    for source_membership in admission.scope.memberships.keys() {
        let row = metadata
            .rows
            .get(source_membership)
            .ok_or(AccessError::SnapshotStale)?;
        let member_security = current
            .membership_security
            .memberships
            .get(source_membership)
            .ok_or(AccessError::SnapshotStale)?;
        if modality.is_some_and(|selected| selected != member_security.modality) {
            return Err(AccessError::EligibilityPopulationIncoherent);
        }
        modality = Some(member_security.modality);
        let projection = canonical_projection_membership(row, &metadata.schema);
        if projection.source_membership_id != *source_membership
            || projection.projection_membership_id != row.c03.binding().projection_membership_id
            || projection_memberships
                .insert(projection.projection_membership_id, projection)
                .is_some()
        {
            return Err(AccessError::SnapshotStale);
        }
    }
    if projection_memberships.len() != admission.scope.memberships.len() {
        return Err(AccessError::SnapshotStale);
    }
    let modality = modality.ok_or(AccessError::AuthorizedScopeEmpty)?;
    let mut port = NativeCanonicalPopulationPort {
        metadata,
        request,
        issued,
    };
    let result = compile_canonical_eligibility_population(
        &mut port,
        issued,
        &CanonicalEligibilityPopulationRequest {
            grant: &request.grant,
            grant_policy: current.policy,
            recipe: request.recipe_request.recipe,
            modality,
            requested_scope,
            authoritative_access: current.access,
            namespace_policy_fence: current.planned_policy,
            authoritative_policy: current.authoritative_policy,
            snapshot_fence: &metadata.fence,
            collection_schema: &metadata.schema,
            collection_route: &metadata.route,
            publication_manifests: &metadata.manifests,
            publication_shadows: &metadata.shadows,
            projection_memberships: &projection_memberships,
            security: CanonicalEligibilitySecuritySnapshot {
                live: current.live,
                namespace_id: current.authoritative_policy.namespace_id,
                access_policy_revision: current.authoritative_policy.policy_revision,
                shadow_fence_revision: current.authoritative_policy.shadow_revision,
                purge_fence_revision: current.authoritative_policy.purge_revision,
                overlay_revision: metadata.fence.overlay_revision,
            },
        },
    )?;
    checkpoint_live(current, &admission.request_fence)?;
    Ok(result)
}

fn checkpoint_live(
    current: &CanonicalPopulationLiveAuthority<'_>,
    fence: &search_access::RequestSecurityFence,
) -> Result<(), AccessError> {
    let matches = current.domain.with_live_checkpoint(
        fence,
        AccessCheckpoint::RequestAdmission,
        |permit| {
            permit.live_generation == current.live.generation
                && permit.live_snapshot_digest == current.live.snapshot_digest
        },
    )?;
    if matches {
        Ok(())
    } else {
        Err(AccessError::SnapshotStale)
    }
}

fn canonical_projection_membership(
    row: &CanonicalProjectionMetadataRow,
    schema: &CollectionSchema,
) -> CanonicalProjectionMembership {
    let binding = row.c03.binding();
    CanonicalProjectionMembership {
        source_membership_id: binding.source_membership_id,
        installation_incarnation_id: *schema.installation_incarnation_id(),
        collection_generation_id: *schema.collection_generation_id(),
        projection_membership_id: binding.projection_membership_id,
        access_partition_id: binding.access_partition_id,
        scoring_partition_id: binding.scoring_partition_id,
        projection_profile_set_id: schema.projection_profile_set_id().clone(),
    }
}

struct NativeCanonicalPopulationPort<'a, 'request, 'borrow, 'grant> {
    metadata: &'a CanonicalPopulationSnapshot,
    request: &'request RequestBody,
    issued: &'borrow VerifiedStandaloneGrant<'grant>,
}

impl<'grant> CanonicalPopulationVerificationPort
    for NativeCanonicalPopulationPort<'_, '_, '_, 'grant>
{
    type IssuedGrant = VerifiedStandaloneGrant<'grant>;

    fn verified_issued_claims(
        &mut self,
        issued: &Self::IssuedGrant,
    ) -> Result<search_contracts::SearchReadGrantClaims, AccessError> {
        if !std::ptr::eq(issued, self.issued) {
            return Err(AccessError::GrantBindingMismatch);
        }
        issued
            .verify_current_claims(&self.request.grant)
            .map_err(map_grant_error)?;
        Ok(self.request.grant.clone())
    }

    fn current_projection_binding(
        &mut self,
        issued: &Self::IssuedGrant,
        membership_id: SourceMembershipId,
        fence: &QuerySnapshotFence,
    ) -> Result<CanonicalProjectionMembership, AccessError> {
        if !std::ptr::eq(issued, self.issued) {
            return Err(AccessError::GrantBindingMismatch);
        }
        if crate::provider_composition::monotonic_millis() >= issued.valid_until() {
            return Err(AccessError::GrantExpired);
        }
        if fence != self.metadata.fence() {
            return Err(AccessError::SnapshotStale);
        }
        if !issued
            .template()
            .allowed_membership_ids
            .contains(&membership_id)
        {
            return Err(AccessError::ScopeUnauthorized);
        }
        let row = self
            .metadata
            .rows
            .get(&membership_id)
            .ok_or(AccessError::ScopeUnknown)?;
        if let SourceView::RetainedRevision(revision_id) = &fence.source_view
            && row.source_revision.revision_id != *revision_id
        {
            return Err(AccessError::SnapshotStale);
        }
        let projection_id = row.c03.binding().projection_membership_id;
        let pointer = self
            .metadata
            .manifests
            .get(&projection_id)
            .ok_or(AccessError::SnapshotStale)?;
        let record = &row.c02_record;
        if record.reference() != pointer
            || record.metadata().source_revision != row.source_revision
            || record.metadata().representation_id != row.c03.binding().representation_id
        {
            return Err(AccessError::SnapshotStale);
        }
        Ok(canonical_projection_membership(row, &self.metadata.schema))
    }
}

const fn map_grant_error(error: GrantUseError) -> AccessError {
    match error {
        GrantUseError::Expired | GrantUseError::ClockUnavailable => AccessError::GrantExpired,
        GrantUseError::InvalidGrant | GrantUseError::BindingMismatch => {
            AccessError::GrantBindingMismatch
        }
        GrantUseError::PolicyUnavailable
        | GrantUseError::PolicyChanged
        | GrantUseError::RequestDenied
        | GrantUseError::RequestInactive => AccessError::ScopeUnauthorized,
    }
}
