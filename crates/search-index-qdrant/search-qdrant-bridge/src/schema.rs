use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, CollectionGenerationId, OpaqueId};

use crate::{BridgeError, EligibilityFilter};

/// Named vector schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorSchema {
    pub dimensions: u32,
    pub sparse: bool,
    pub idf_enabled: bool,
}

/// Must-be-true strictness floors for collection admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrictnessFloors {
    pub strict_mode: bool,
    pub wait_for_mutations: bool,
    pub strong_ordering: bool,
}

/// Exact collection schema and strict-mode correctness floors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionSchema {
    pub named_vectors: BTreeMap<String, VectorSchema>,
    pub indexed_payload_fields: BTreeSet<String>,
    pub one_shard: bool,
    pub floors: StrictnessFloors,
    pub schema_digest: Blake3Digest32,
}

impl CollectionSchema {
    pub fn validate(&self) -> Result<(), BridgeError> {
        if self.named_vectors.is_empty() {
            return Err(BridgeError::NamedVectorMissing);
        }
        if self
            .named_vectors
            .iter()
            .any(|(name, schema)| name.is_empty() || schema.dimensions == 0)
        {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        if !self.one_shard
            || !self.floors.strict_mode
            || !self.floors.wait_for_mutations
            || !self.floors.strong_ordering
        {
            return Err(BridgeError::StrictModeRequired);
        }
        for field in EligibilityFilter::INDEXED_FIELDS {
            if !self.indexed_payload_fields.contains(field) {
                return Err(BridgeError::PayloadIndexMissing);
            }
        }
        Ok(())
    }
}

/// Opaque physical collection route.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CollectionRoute {
    pub generation: CollectionGenerationId,
    pub physical_name: OpaqueId,
}
