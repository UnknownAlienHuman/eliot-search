use std::collections::BTreeMap;

use search_contracts::{
    AccessPartitionId, Blake3Digest32, CollectionGenerationId, Epoch,
    InstallationIncarnationId, ProfileId, ProjectionMembershipId,
    ProjectionProfileSetId, RepresentationId, ScoringPartitionId, SourceId,
    SourceMembershipId, SourceRevisionId, UnitId,
};
use search_point_identity::{PointId128, PointRole};

use crate::canonical::{CanonicalBuffer, MANIFEST_DOMAIN};
use crate::{
    PointSpec, ProjectionBudget, ProjectionDigestPort, ProjectionError,
    ProjectionProfiles, ProjectionScope,
};

/// Exact immutable manifest scope for one projection membership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifestScope {
    /// Installation incarnation bound to every point.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Exact physical collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Control-side source membership; never copied into Qdrant payload.
    pub source_membership_id: SourceMembershipId,
    /// Exact projection membership carried by every point.
    pub projection_membership_id: ProjectionMembershipId,
    /// Immutable access partition.
    pub access_partition_id: AccessPartitionId,
    /// Immutable scoring partition.
    pub scoring_partition_id: ScoringPartitionId,
    /// Stable source identity.
    pub source_id: SourceId,
    /// Exact retained source revision.
    pub source_revision_id: SourceRevisionId,
    /// Exact canonical representation.
    pub representation_id: RepresentationId,
    /// Projection schema bound by the membership.
    pub projection_schema_id: ProfileId,
    /// Inclusive validity epoch for staged points.
    pub valid_from_epoch: Epoch,
    /// Exact projection profile-set identity.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Digest of the complete profile-set behavior.
    pub profile_set_digest: Blake3Digest32,
}

impl ProjectionManifestScope {
    pub(crate) fn from_scope(
        scope: &ProjectionScope,
        profiles: &ProjectionProfiles,
    ) -> Self {
        Self {
            installation_incarnation_id: scope.installation_incarnation_id,
            collection_generation_id: scope.collection_generation_id,
            source_membership_id: scope.membership.source_membership_id,
            projection_membership_id: scope.membership.projection_membership_id,
            access_partition_id: scope.membership.access_partition_id,
            scoring_partition_id: scope.membership.scoring_partition_id,
            source_id: scope.source_id,
            source_revision_id: scope.source_revision_id,
            representation_id: scope.membership.representation_id,
            projection_schema_id: scope.membership.projection_schema_id.clone(),
            valid_from_epoch: scope.valid_from_epoch,
            projection_profile_set_id: profiles.profile_set_id.clone(),
            profile_set_digest: profiles.profile_set_digest,
        }
    }
}

/// One exact immutable projection-manifest entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifestEntry {
    /// Compact Qdrant point address.
    pub point_id: PointId128,
    /// Full BLAKE3-256 canonical point-identity digest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Exact projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Exact canonical representation.
    pub representation_id: RepresentationId,
    /// Exact occurrence unit.
    pub unit_id: UnitId,
    /// Exact projection profile-set identity.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Logical point role.
    pub point_role: PointRole,
    /// Digest of the canonical S9.5 payload.
    pub payload_digest: Blake3Digest32,
    /// Exact expected named-vector digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
}

impl ProjectionManifestEntry {
    pub(crate) fn from_point(point: &PointSpec) -> Self {
        Self {
            point_id: point.point_id,
            point_identity_digest_256: Blake3Digest32::from_bytes(
                *point.identity.full_digest.as_bytes(),
            ),
            projection_membership_id: point.identity.key.projection_membership_id,
            representation_id: point.identity.key.representation_id,
            unit_id: point.identity.key.unit_id,
            projection_profile_set_id: point
                .identity
                .key
                .projection_profile_set_id
                .clone(),
            point_role: point.identity.key.point_role,
            payload_digest: point.expected_readback.payload_digest,
            vector_digests: point.expected_readback.vector_digests.clone(),
        }
    }
}

/// Immutable CAS-ready exact projection manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifest {
    /// Exact membership-scoped control identity.
    pub scope: ProjectionManifestScope,
    /// Canonically point-ID-ordered entries.
    pub entries: Vec<ProjectionManifestEntry>,
    /// Frozen deterministic canonical bytes.
    pub canonical_bytes: Vec<u8>,
    /// BLAKE3-256 digest of `canonical_bytes`.
    pub manifest_digest: Blake3Digest32,
}

/// Exact old/new manifest difference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDiff {
    /// New or changed points to create.
    pub create: Vec<ProjectionManifestEntry>,
    /// Unchanged exact points to retain.
    pub retain: Vec<ProjectionManifestEntry>,
    /// Old or changed points to retire by exact ID.
    pub retire: Vec<ProjectionManifestEntry>,
}

/// Produces one CAS-ready exact manifest from canonical point specifications.
pub fn canonicalize_manifest<D: ProjectionDigestPort>(
    scope: &ProjectionScope,
    profiles: &ProjectionProfiles,
    points: &[PointSpec],
    budget: ProjectionBudget,
    digest_port: &mut D,
) -> Result<ProjectionManifest, ProjectionError> {
    let budget = budget.validate()?;
    if points.is_empty() || points.len() > budget.max_points {
        return Err(ProjectionError::BudgetExceeded);
    }
    let manifest_scope = ProjectionManifestScope::from_scope(scope, profiles);
    let mut entries = points
        .iter()
        .map(ProjectionManifestEntry::from_point)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.point_id);
    validate_manifest_entries(&entries)?;

    let canonical_bytes = encode_manifest(&manifest_scope, &entries, budget)?;
    let manifest_digest = digest_port.blake3_256(&canonical_bytes)?;
    Ok(ProjectionManifest {
        scope: manifest_scope,
        entries,
        canonical_bytes,
        manifest_digest,
    })
}

/// Proves an immutable manifest reconstructs exactly the supplied point set.
pub fn verify_manifest_reconstruction(
    manifest: &ProjectionManifest,
    points: &[PointSpec],
) -> Result<(), ProjectionError> {
    validate_manifest_entries(&manifest.entries)?;
    let mut derived = points
        .iter()
        .map(ProjectionManifestEntry::from_point)
        .collect::<Vec<_>>();
    derived.sort_by_key(|entry| entry.point_id);
    validate_manifest_entries(&derived)?;
    if manifest.entries == derived {
        Ok(())
    } else {
        Err(ProjectionError::InvalidManifest)
    }
}

/// Returns exact create, retain, and retire sets.
///
/// A scope/profile change is a complete replacement. Within one unchanged
/// scope, an entry is retained only when identity, payload digest, and every
/// named-vector digest remain byte-for-byte equal.
pub fn diff_manifests(
    old: &ProjectionManifest,
    new: &ProjectionManifest,
) -> Result<ManifestDiff, ProjectionError> {
    validate_manifest_entries(&old.entries)?;
    validate_manifest_entries(&new.entries)?;
    if old.scope != new.scope {
        return Ok(ManifestDiff {
            create: new.entries.clone(),
            retain: Vec::new(),
            retire: old.entries.clone(),
        });
    }

    let old_by_id = old
        .entries
        .iter()
        .map(|entry| (entry.point_id, entry))
        .collect::<BTreeMap<_, _>>();
    let new_by_id = new
        .entries
        .iter()
        .map(|entry| (entry.point_id, entry))
        .collect::<BTreeMap<_, _>>();
    let mut create = Vec::new();
    let mut retain = Vec::new();
    let mut retire = Vec::new();

    for (point_id, new_entry) in &new_by_id {
        match old_by_id.get(point_id) {
            Some(old_entry) if *old_entry == *new_entry => retain.push((*new_entry).clone()),
            Some(old_entry) => {
                retire.push((*old_entry).clone());
                create.push((*new_entry).clone());
            }
            None => create.push((*new_entry).clone()),
        }
    }
    for (point_id, old_entry) in old_by_id {
        if !new_by_id.contains_key(&point_id) {
            retire.push(old_entry.clone());
        }
    }
    Ok(ManifestDiff {
        create,
        retain,
        retire,
    })
}

fn encode_manifest(
    scope: &ProjectionManifestScope,
    entries: &[ProjectionManifestEntry],
    budget: ProjectionBudget,
) -> Result<Vec<u8>, ProjectionError> {
    let mut output = CanonicalBuffer::new(
        MANIFEST_DOMAIN,
        budget.max_manifest_bytes,
        ProjectionError::ManifestTooLarge,
    )?;
    output.append_bytes(scope.installation_incarnation_id.as_bytes())?;
    output.append_bytes(scope.collection_generation_id.as_bytes())?;
    output.append_bytes(scope.source_membership_id.as_bytes())?;
    output.append_bytes(scope.projection_membership_id.as_bytes())?;
    output.append_bytes(scope.access_partition_id.as_bytes())?;
    output.append_bytes(scope.scoring_partition_id.as_bytes())?;
    output.append_bytes(scope.source_id.as_bytes())?;
    output.append_bytes(scope.source_revision_id.as_bytes())?;
    output.append_bytes(scope.representation_id.as_bytes())?;
    output.append_text(scope.projection_schema_id.as_str())?;
    output.append_i64(scope.valid_from_epoch.get())?;
    output.append_text(scope.projection_profile_set_id.as_str())?;
    output.append_digest(scope.profile_set_digest)?;
    output.append_u64(
        u64::try_from(entries.len()).map_err(|_| ProjectionError::ManifestTooLarge)?,
    )?;
    for entry in entries {
        output.append_bytes(entry.point_id.as_bytes())?;
        output.append_digest(entry.point_identity_digest_256)?;
        output.append_bytes(entry.projection_membership_id.as_bytes())?;
        output.append_bytes(entry.representation_id.as_bytes())?;
        output.append_bytes(entry.unit_id.as_bytes())?;
        output.append_text(entry.projection_profile_set_id.as_str())?;
        output.append_text(entry.point_role.as_str())?;
        output.append_digest(entry.payload_digest)?;
        output.append_u64(
            u64::try_from(entry.vector_digests.len())
                .map_err(|_| ProjectionError::ManifestTooLarge)?,
        )?;
        for (name, digest) in &entry.vector_digests {
            output.append_text(name)?;
            output.append_digest(*digest)?;
        }
    }
    Ok(output.into_vec())
}

fn validate_manifest_entries(
    entries: &[ProjectionManifestEntry],
) -> Result<(), ProjectionError> {
    if entries.is_empty()
        || entries
            .windows(2)
            .any(|pair| pair[0].point_id >= pair[1].point_id)
    {
        return Err(ProjectionError::InvalidManifest);
    }
    Ok(())
}
