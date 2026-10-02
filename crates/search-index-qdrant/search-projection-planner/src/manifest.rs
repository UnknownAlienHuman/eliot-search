use std::collections::BTreeMap;

use search_contracts::{ProfileId, ProjectionProfileSetId};
use search_point_identity::{
    DEFAULT_POINT_IDENTITY_LIMITS, PointIdentityLimits, derive_point_identity,
    encode_canonical_key,
};

use crate::digest::canonical_manifest_bytes;
use crate::{
    ManifestDiff, PointSpec, ProjectionBudget, ProjectionError,
    ProjectionManifest, ProjectionManifestEntry, ProjectionProfiles,
};

const MANIFEST_BODY_DOMAIN: &[u8] = b"eliot-search/projection-manifest/v1\0";

/// Produces one CAS-ready exact S11.3 manifest from point specifications.
pub fn canonicalize_manifest(
    points: &[PointSpec],
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    identity_limits: PointIdentityLimits,
) -> Result<ProjectionManifest, ProjectionError> {
    let budget = budget.validate()?;
    if points.len() > budget.max_points {
        return Err(ProjectionError::BudgetExceeded);
    }
    let mut entries = points.iter().map(entry_from_point).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.point_id);
    validate_manifest_entries(&entries)?;
    let canonical_bytes =
        canonical_manifest_bytes(&entries, profiles, budget, identity_limits)?;
    let manifest = ProjectionManifest {
        entries,
        canonical_bytes,
    };
    verify_manifest_integrity(&manifest, budget, identity_limits)?;
    Ok(manifest)
}

/// Recomputes every manifest entry and canonical byte from exact point specs.
pub fn verify_manifest_reconstruction(
    manifest: &ProjectionManifest,
    points: &[PointSpec],
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    identity_limits: PointIdentityLimits,
) -> Result<(), ProjectionError> {
    verify_manifest_integrity(manifest, budget, identity_limits)?;
    let rebuilt = canonicalize_manifest(points, profiles, budget, identity_limits)?;
    if rebuilt != *manifest {
        return Err(ProjectionError::ManifestMismatch);
    }
    Ok(())
}

/// Verifies that frozen manifest bytes encode exactly the supplied typed entries.
///
/// The check is bounded by the same point/byte/profile limits used by planning.
/// It rejects altered canonical bytes, mismatched entry counts, reordered or
/// duplicated entries, foreign S11 identities, wrong vector maps and trailing
/// bytes. Publication owners can therefore validate a manifest without source
/// units or Qdrant access before consuming an epoch.
pub fn verify_manifest_integrity(
    manifest: &ProjectionManifest,
    budget: ProjectionBudget,
    identity_limits: PointIdentityLimits,
) -> Result<(), ProjectionError> {
    let budget = budget.validate()?;
    identity_limits.validate()?;
    if manifest.entries.len() > budget.max_points
        || manifest.canonical_bytes.is_empty()
        || manifest.canonical_bytes.len() > budget.max_manifest_bytes
    {
        return Err(ProjectionError::ManifestMismatch);
    }
    validate_manifest_entries(&manifest.entries)?;

    let mut reader = ManifestReader::new(&manifest.canonical_bytes);
    reader.expect_bytes(MANIFEST_BODY_DOMAIN, budget.max_manifest_bytes)?;
    let profile_set = reader.text(identity_limits.max_identifier_bytes)?;
    let projection_schema = reader.text(identity_limits.max_identifier_bytes)?;
    ProjectionProfileSetId::new(profile_set.to_owned())
        .map_err(|_| ProjectionError::ManifestMismatch)?;
    ProfileId::new(projection_schema.to_owned())
        .map_err(|_| ProjectionError::ManifestMismatch)?;
    if manifest.entries.iter().any(|entry| {
        entry.identity_key.projection_profile_set_id.as_str() != profile_set
    }) {
        return Err(ProjectionError::ManifestMismatch);
    }

    if reader.usize()? != manifest.entries.len() {
        return Err(ProjectionError::ManifestMismatch);
    }
    for entry in &manifest.entries {
        reader.expect_bytes(entry.point_id.as_bytes(), 16)?;
        let canonical_key = encode_canonical_key(&entry.identity_key, identity_limits)?;
        reader.expect_bytes(canonical_key.as_slice(), identity_limits.max_canonical_bytes)?;
        reader.expect_bytes(entry.point_identity_digest_256.as_bytes(), 32)?;
        reader.expect_bytes(entry.source_membership_id.as_bytes(), 16)?;
        reader.expect_bytes(entry.unit_id.as_bytes(), 16)?;
        reader.expect_bytes(entry.payload_digest.as_bytes(), 32)?;
        if reader.usize()? != entry.vector_digests.len()
            || entry.vector_digests.len() > budget.max_vectors_per_point
        {
            return Err(ProjectionError::ManifestMismatch);
        }
        for (name, digest) in &entry.vector_digests {
            if reader.text(budget.max_vector_name_bytes)? != name {
                return Err(ProjectionError::ManifestMismatch);
            }
            reader.expect_bytes(digest.as_bytes(), 32)?;
        }
        reader.expect_bytes(entry.unit_digest.as_bytes(), 32)?;
        reader.expect_bytes(entry.reference_digest.as_bytes(), 32)?;
    }
    if !reader.finished() {
        return Err(ProjectionError::ManifestMismatch);
    }
    Ok(())
}

/// Returns exact create, retain and retire entries.
///
/// A physical point ID with changed immutable manifest content is rejected.
/// Point replacement must derive a distinct S11 identity; the planner never
/// emits an upsert and close for the same UUID.
pub fn diff_manifests(
    old: &ProjectionManifest,
    new: &ProjectionManifest,
) -> Result<ManifestDiff, ProjectionError> {
    validate_manifest_entries(&old.entries)?;
    validate_manifest_entries(&new.entries)?;
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
            Some(old_entry) if *old_entry == *new_entry => {
                retain.push((*new_entry).clone());
            }
            Some(_) => return Err(ProjectionError::ManifestMismatch),
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

/// Validates sorted exact S11 identities for every typed manifest entry.
pub fn validate_manifest_entries(
    entries: &[ProjectionManifestEntry],
) -> Result<(), ProjectionError> {
    if entries
        .windows(2)
        .any(|pair| pair[0].point_id >= pair[1].point_id)
    {
        return Err(ProjectionError::ManifestMismatch);
    }
    for entry in entries {
        let identity = derive_point_identity(
            entry.identity_key.clone(),
            DEFAULT_POINT_IDENTITY_LIMITS,
        )?;
        if identity.point_id != entry.point_id
            || identity.full_digest.as_contract_digest()
                != entry.point_identity_digest_256
            || entry.identity_key.unit_id != entry.unit_id
            || entry.vector_digests.is_empty()
        {
            return Err(ProjectionError::ManifestMismatch);
        }
    }
    Ok(())
}

fn entry_from_point(point: &PointSpec) -> ProjectionManifestEntry {
    ProjectionManifestEntry {
        point_id: point.point_id,
        identity_key: point.identity.key.clone(),
        point_identity_digest_256: point.identity.full_digest.as_contract_digest(),
        source_membership_id: point.source_membership_id,
        unit_id: point.payload.unit_id,
        payload_digest: point.expected_readback.payload_digest,
        vector_digests: point.expected_readback.vector_digests.clone(),
        unit_digest: point.expected_readback.unit_digest,
        reference_digest: point.expected_readback.reference_digest,
    }
}

struct ManifestReader<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> ManifestReader<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    fn usize(&mut self) -> Result<usize, ProjectionError> {
        let bytes = self.take(8)?;
        let mut encoded = [0_u8; 8];
        encoded.copy_from_slice(bytes);
        usize::try_from(u64::from_be_bytes(encoded))
            .map_err(|_| ProjectionError::ManifestMismatch)
    }

    fn bytes(&mut self, maximum: usize) -> Result<&'a [u8], ProjectionError> {
        let length = self.usize()?;
        if length > maximum {
            return Err(ProjectionError::ManifestMismatch);
        }
        self.take(length)
    }

    fn text(&mut self, maximum: usize) -> Result<&'a str, ProjectionError> {
        core::str::from_utf8(self.bytes(maximum)?)
            .map_err(|_| ProjectionError::ManifestMismatch)
    }

    fn expect_bytes(
        &mut self,
        expected: &[u8],
        maximum: usize,
    ) -> Result<(), ProjectionError> {
        if self.bytes(maximum)? == expected {
            Ok(())
        } else {
            Err(ProjectionError::ManifestMismatch)
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProjectionError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ProjectionError::ManifestMismatch)?;
        let value = self
            .input
            .get(self.position..end)
            .ok_or(ProjectionError::ManifestMismatch)?;
        self.position = end;
        Ok(value)
    }

    const fn finished(&self) -> bool {
        self.position == self.input.len()
    }
}
