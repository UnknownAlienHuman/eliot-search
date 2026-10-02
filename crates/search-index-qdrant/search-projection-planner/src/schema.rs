use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ProjectionError, ProjectionManifest, ProjectionProfiles, VectorRequirement,
};

/// Closed Qdrant payload-index type required by the S9.5 schema.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PayloadIndexKind {
    /// UUID payload index.
    Uuid,
    /// Exact keyword payload index.
    Keyword,
    /// Signed-integer range payload index.
    Integer,
}

/// One exact mandatory S9.5 payload-index declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadIndexRequirement {
    /// Exact payload field name.
    pub field: &'static str,
    /// Required Qdrant payload-index type.
    pub kind: PayloadIndexKind,
}

/// Exact 19-field baseline payload-index schema.
pub const REQUIRED_PAYLOAD_INDEXES: [PayloadIndexRequirement; 19] = [
    PayloadIndexRequirement { field: "installation_incarnation_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "collection_generation_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "projection_membership_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "access_partition_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "scoring_partition_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "source_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "source_revision_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "representation_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "unit_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "scoring_document_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "projection_profile_set_id", kind: PayloadIndexKind::Keyword },
    PayloadIndexRequirement { field: "unit_kind", kind: PayloadIndexKind::Keyword },
    PayloadIndexRequirement { field: "modality", kind: PayloadIndexKind::Keyword },
    PayloadIndexRequirement { field: "language_or_format", kind: PayloadIndexKind::Keyword },
    PayloadIndexRequirement { field: "entity_kind", kind: PayloadIndexKind::Keyword },
    PayloadIndexRequirement { field: "normalized_symbol_key", kind: PayloadIndexKind::Keyword },
    PayloadIndexRequirement { field: "repository_lineage_id", kind: PayloadIndexKind::Uuid },
    PayloadIndexRequirement { field: "valid_from_epoch", kind: PayloadIndexKind::Integer },
    PayloadIndexRequirement { field: "valid_until_epoch_exclusive", kind: PayloadIndexKind::Integer },
];

/// Provider-neutral collection schema admitted before publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionSchema {
    /// Named-vector shape by exact vector name.
    pub named_vectors: BTreeMap<String, VectorRequirement>,
    /// Exact payload-index type by field name.
    pub indexed_payload_fields: BTreeMap<String, PayloadIndexKind>,
}

/// Returns the exact mandatory S9.5 payload-index plan.
#[must_use]
pub const fn expected_payload_indexes() -> &'static [PayloadIndexRequirement; 19] {
    &REQUIRED_PAYLOAD_INDEXES
}

/// Proves collection named vectors and exact S9.5 indexes can admit a manifest.
pub fn validate_schema_requirements(
    manifest: &ProjectionManifest,
    profiles: &ProjectionProfiles,
    schema: &CollectionSchema,
) -> Result<(), ProjectionError> {
    if schema.indexed_payload_fields.len() != REQUIRED_PAYLOAD_INDEXES.len() {
        return if schema.indexed_payload_fields.len() < REQUIRED_PAYLOAD_INDEXES.len() {
            Err(ProjectionError::PayloadIndexMissing)
        } else {
            Err(ProjectionError::PayloadIndexUnexpected)
        };
    }
    for requirement in REQUIRED_PAYLOAD_INDEXES {
        match schema.indexed_payload_fields.get(requirement.field) {
            Some(kind) if *kind == requirement.kind => {}
            Some(_) => return Err(ProjectionError::PayloadIndexTypeMismatch),
            None => return Err(ProjectionError::PayloadIndexMissing),
        }
    }
    let allowed_fields = REQUIRED_PAYLOAD_INDEXES
        .iter()
        .map(|requirement| requirement.field)
        .collect::<BTreeSet<_>>();
    if schema
        .indexed_payload_fields
        .keys()
        .any(|field| !allowed_fields.contains(field.as_str()))
    {
        return Err(ProjectionError::PayloadIndexUnexpected);
    }

    validate_schema_dimensions(profiles, schema)?;
    let expected_vectors = profiles.vectors.keys().cloned().collect::<BTreeSet<_>>();
    for entry in &manifest.entries {
        let actual = entry.vector_digests.keys().cloned().collect::<BTreeSet<_>>();
        if actual != expected_vectors {
            return Err(ProjectionError::VectorSetMismatch);
        }
    }
    Ok(())
}

/// Proves every required named-vector shape exists in the collection schema.
pub fn validate_schema_dimensions(
    profiles: &ProjectionProfiles,
    schema: &CollectionSchema,
) -> Result<(), ProjectionError> {
    for (name, requirement) in &profiles.vectors {
        match schema.named_vectors.get(name) {
            Some(actual) if actual == requirement => {}
            Some(_) => return Err(ProjectionError::CollectionVectorMismatch),
            None => return Err(ProjectionError::CollectionVectorMissing),
        }
    }
    Ok(())
}
