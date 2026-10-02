use std::collections::BTreeMap;

use crate::manifest::validate_manifest_entries;
use crate::{
    CollectionSchema, PayloadIndexKind, ProjectionError, ProjectionManifest,
    ProjectionProfiles,
};

/// Exact S9.5 baseline payload-index name/type set.
pub const REQUIRED_PAYLOAD_INDEXES: [(&str, PayloadIndexKind); 19] = [
    ("installation_incarnation_id", PayloadIndexKind::Uuid),
    ("collection_generation_id", PayloadIndexKind::Uuid),
    ("projection_membership_id", PayloadIndexKind::Uuid),
    ("access_partition_id", PayloadIndexKind::Uuid),
    ("scoring_partition_id", PayloadIndexKind::Uuid),
    ("source_id", PayloadIndexKind::Uuid),
    ("source_revision_id", PayloadIndexKind::Uuid),
    ("representation_id", PayloadIndexKind::Uuid),
    ("unit_id", PayloadIndexKind::Uuid),
    ("scoring_document_id", PayloadIndexKind::Uuid),
    ("projection_profile_set_id", PayloadIndexKind::Keyword),
    ("unit_kind", PayloadIndexKind::Keyword),
    ("modality", PayloadIndexKind::Keyword),
    ("language_or_format", PayloadIndexKind::Keyword),
    ("entity_kind", PayloadIndexKind::Keyword),
    ("normalized_symbol_key", PayloadIndexKind::Keyword),
    ("repository_lineage_id", PayloadIndexKind::Uuid),
    ("valid_from_epoch", PayloadIndexKind::Integer),
    ("valid_until_epoch_exclusive", PayloadIndexKind::Integer),
];

/// Materializes the exact qualified S9.5 payload-index map.
#[must_use]
pub fn expected_payload_indexes() -> BTreeMap<String, PayloadIndexKind> {
    REQUIRED_PAYLOAD_INDEXES
        .into_iter()
        .map(|(name, kind)| (name.to_owned(), kind))
        .collect()
}

/// Proves collection vector and payload-index compatibility before publication.
pub fn validate_schema_requirements(
    manifest: &ProjectionManifest,
    profiles: &ProjectionProfiles,
    schema: &CollectionSchema,
) -> Result<(), ProjectionError> {
    validate_manifest_entries(&manifest.entries)?;
    let expected_indexes = expected_payload_indexes();
    for (name, kind) in &expected_indexes {
        match schema.payload_indexes.get(name) {
            Some(actual) if actual == kind => {}
            Some(_) => return Err(ProjectionError::PayloadIndexTypeMismatch),
            None => return Err(ProjectionError::PayloadIndexMissing),
        }
    }
    if schema.payload_indexes.len() != expected_indexes.len() {
        return Err(ProjectionError::CollectionSchemaMismatch);
    }

    for (name, requirement) in &profiles.vectors {
        match schema.named_vectors.get(name) {
            Some(actual) if actual == requirement => {}
            Some(_) => return Err(ProjectionError::CollectionVectorMismatch),
            None => return Err(ProjectionError::CollectionVectorMissing),
        }
    }
    for entry in &manifest.entries {
        for name in entry.vector_digests.keys() {
            if !profiles.vectors.contains_key(name)
                || !schema.named_vectors.contains_key(name)
            {
                return Err(ProjectionError::CollectionVectorMissing);
            }
        }
    }
    Ok(())
}
