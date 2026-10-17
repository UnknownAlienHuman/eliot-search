//! Bounded reads of stored projection metadata, without source preparation.
//! Decode and scope consistency do not qualify source bytes, completeness,
//! access, publication or Qdrant contents. Those checks retain their owners.

use std::collections::BTreeMap;

use search_contracts::indexed::{CollectionSchema, VectorName};
use search_contracts::{
    self as c, BoundedMap, CanonicalKey, CanonicalValue as Value, MAX_MAP_ENTRIES,
};
use search_point_identity::s11::{
    POINT_KEY_SCHEMA_VERSION, PointId128, PointRole, ProjectionPointKey, derive_point_identity,
};

use crate::{
    PlanningScope, ProjectionBudget, ProjectionError as Error, ProjectionManifest,
    ProjectionManifestEntry, manifest_canonical_bytes,
};

type Fields = BoundedMap<CanonicalKey, Value, MAX_MAP_ENTRIES>;

/// Decodes the existing canonical C13 format into ordinary metadata.
///
/// This needs no `UnitSet`, source body, vectors, I/O or preparation. Exact
/// field sets, finite bounds, IDs/digests, vector names and ordering are checked,
/// and re-encoding must equal every original byte. Empty metadata is decoded
/// without inferring a scope. Publication still requires canonical admission;
/// citations still require exact source verification.
pub fn decode_projection_manifest_data(
    encoded: &[u8],
    budget: ProjectionBudget,
) -> Result<ProjectionManifest, Error> {
    let budget = budget.validate()?;
    if encoded.len() > budget.max_manifest_bytes || encoded.len() > c::MAX_CANONICAL_BYTES {
        return Err(Error::ManifestTooLarge);
    }
    let root = c::parse_canonical_cbor(encoded).map_err(|_| Error::InvalidManifest)?;
    let Value::Object(fields) = root else {
        return Err(Error::InvalidManifest);
    };
    if fields.len() != 2 || text(&fields, "value")? != super::manifest::MANIFEST_VALUE {
        return Err(Error::InvalidManifest);
    }
    let Value::Array(rows) = field(&fields, "entries")? else {
        return Err(Error::InvalidManifest);
    };
    if rows.len() > budget.max_points || rows.len() > c::MAX_COLLECTION_ITEMS {
        return Err(Error::ManifestTooLarge);
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(rows.len())
        .map_err(|_| Error::ManifestTooLarge)?;
    for row in rows {
        entries.push(decode_entry(row)?);
    }
    super::diff::validate_entries(&entries)?;
    let manifest = ProjectionManifest {
        entries,
        canonical_bytes: encoded.to_vec(),
    };
    if manifest_canonical_bytes(&manifest, budget)?.as_slice() != encoded {
        return Err(Error::ManifestMismatch);
    }
    Ok(manifest)
}

/// Checks decoded metadata against one explicit scope and qualified schema.
///
/// Recomputes S11 addresses/full identity digests and scoring-document IDs
/// through their existing owners. Payload/vector digests remain observations;
/// this returns no source proof or publication preparation. An empty manifest
/// is not a query population; explicit retirement is admitted separately.
pub fn validate_projection_manifest_metadata(
    manifest: &ProjectionManifest,
    scope: &PlanningScope,
    schema: &CollectionSchema,
    budget: ProjectionBudget,
) -> Result<(), Error> {
    if manifest.entries.is_empty() {
        return Err(Error::EmptyUnitSet);
    }
    if manifest_canonical_bytes(manifest, budget)?.as_slice() != manifest.canonical_bytes {
        return Err(Error::ManifestMismatch);
    }
    super::diff::validate_entries(&manifest.entries)?;
    schema
        .validate()
        .map_err(|_| Error::CollectionSchemaMismatch)?;
    let binding = &scope.membership;
    if schema.installation_incarnation_id() != &binding.installation_incarnation_id
        || schema.collection_generation_id() != &binding.collection_generation_id
        || schema.projection_profile_set_id() != &binding.projection_profile_set_id
    {
        return Err(Error::CollectionSchemaMismatch);
    }
    for entry in &manifest.entries {
        if entry.source_membership_id != binding.source_membership_id
            || entry.projection_membership_id != binding.projection_membership_id
            || entry.collection_generation_id != binding.collection_generation_id
            || entry.installation_incarnation_id != binding.installation_incarnation_id
            || entry.access_partition_id != binding.access_partition_id
            || entry.scoring_partition_id != binding.scoring_partition_id
            || entry.projection_profile_set_id != binding.projection_profile_set_id
        {
            return Err(Error::ScopeMismatch);
        }
        let key = ProjectionPointKey {
            schema_version: POINT_KEY_SCHEMA_VERSION,
            installation_incarnation_id: binding.installation_incarnation_id,
            collection_generation_id: binding.collection_generation_id,
            projection_membership_id: binding.projection_membership_id,
            representation_id: binding.representation_id,
            unit_id: entry.unit_id,
            point_role: PointRole::Unit,
            projection_profile_set_id: binding.projection_profile_set_id.clone(),
        };
        let identity = derive_point_identity(&key, scope.identity_limits)?;
        if identity.point_id() != entry.point_id
            || identity.full_digest() != entry.point_identity_digest_256
            || super::plan::derive_scoring_document_id(
                binding.source_revision_id,
                binding.representation_id,
                entry.unit_id,
                &binding.projection_profile_set_id,
            )? != entry.scoring_document_id
        {
            return Err(Error::ManifestMismatch);
        }
    }
    super::point::validate_schema_requirements(manifest, schema)
}

fn field<'a>(fields: &'a Fields, name: &str) -> Result<&'a Value, Error> {
    let key = CanonicalKey::new_non_empty(name).map_err(|_| Error::InvalidManifest)?;
    fields.get(&key).ok_or(Error::InvalidManifest)
}

fn text<'a>(fields: &'a Fields, name: &str) -> Result<&'a str, Error> {
    let Value::Text(value) = field(fields, name)? else {
        return Err(Error::InvalidManifest);
    };
    Ok(value.as_str())
}

fn fixed<const N: usize>(fields: &Fields, name: &str) -> Result<[u8; N], Error> {
    let Value::Bytes(value) = field(fields, name)? else {
        return Err(Error::InvalidManifest);
    };
    value
        .as_slice()
        .try_into()
        .map_err(|_| Error::InvalidManifest)
}

fn digest(fields: &Fields, name: &str) -> Result<c::Blake3Digest32, Error> {
    fixed(fields, name).map(c::Blake3Digest32::from_stored_bytes)
}

fn decode_entry(row: &Value) -> Result<ProjectionManifestEntry, Error> {
    let Value::Object(fields) = row else {
        return Err(Error::InvalidManifest);
    };
    if fields.len() != 15 {
        return Err(Error::InvalidManifest);
    }
    let Value::Array(pairs) = field(fields, "vector_digests")? else {
        return Err(Error::InvalidManifest);
    };
    let mut vector_digests = BTreeMap::new();
    for pair in pairs {
        let Value::Array(pair) = pair else {
            return Err(Error::InvalidManifest);
        };
        let [Value::Text(name), Value::Bytes(hash)] = pair.as_slice() else {
            return Err(Error::InvalidManifest);
        };
        let name = VectorName::new(name.as_str()).map_err(|_| Error::VectorNameMismatch)?;
        let hash = c::Blake3Digest32::from_stored_bytes(
            hash.as_slice()
                .try_into()
                .map_err(|_| Error::InvalidManifest)?,
        );
        if vector_digests
            .insert(name.as_str().to_owned(), hash)
            .is_some()
        {
            return Err(Error::VectorNameMismatch);
        }
    }
    Ok(ProjectionManifestEntry {
        point_id: PointId128::from_stored_bytes(fixed(fields, "point_id")?),
        point_identity_digest_256: digest(fields, "point_identity_digest_256")?,
        source_membership_id: c::SourceMembershipId::from_bytes(fixed(
            fields,
            "source_membership_id",
        )?),
        projection_membership_id: c::ProjectionMembershipId::from_bytes(fixed(
            fields,
            "projection_membership_id",
        )?),
        unit_id: c::UnitId::from_bytes(fixed(fields, "unit_id")?),
        collection_generation_id: c::CollectionGenerationId::from_bytes(fixed(
            fields,
            "collection_generation_id",
        )?),
        installation_incarnation_id: c::InstallationIncarnationId::from_bytes(fixed(
            fields,
            "installation_incarnation_id",
        )?),
        access_partition_id: c::AccessPartitionId::from_bytes(fixed(
            fields,
            "access_partition_id",
        )?),
        scoring_partition_id: c::ScoringPartitionId::from_bytes(fixed(
            fields,
            "scoring_partition_id",
        )?),
        scoring_document_id: c::ScoringDocumentId::from_bytes(fixed(
            fields,
            "scoring_document_id",
        )?),
        projection_profile_set_id: c::ProjectionProfileSetId::new(text(
            fields,
            "projection_profile_set_id",
        )?)?,
        unit_digest: digest(fields, "unit_digest")?,
        reference_digest: digest(fields, "reference_digest")?,
        payload_identity: digest(fields, "payload_identity")?,
        vector_digests,
    })
}
