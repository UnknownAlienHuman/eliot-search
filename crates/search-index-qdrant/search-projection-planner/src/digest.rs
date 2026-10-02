use search_contracts::{
    Blake3Digest32, ProjectionProfileSetId, RepresentationId,
    ScoringDocumentId, SourceRevisionId, UnitId,
};
use search_point_identity::{PointIdentityLimits, encode_canonical_key};

use crate::{
    MinimalPointPayload, NamedVector, PreparedVector, ProjectionBudget,
    ProjectionError, ProjectionManifest, ProjectionManifestEntry,
    ProjectionProfiles, VectorKind, VectorRequirement, VectorValue,
};

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"eliot-search/qdrant-payload/v1\0";
const VECTOR_DIGEST_DOMAIN: &[u8] = b"eliot-search/named-vector/v1\0";
const SCORING_DOCUMENT_DOMAIN: &[u8] = b"eliot-search/scoring-document/v1\0";
const MANIFEST_BODY_DOMAIN: &[u8] = b"eliot-search/projection-manifest/v1\0";
const MANIFEST_DIGEST_DOMAIN: &[u8] = b"eliot-search/projection-manifest-digest/v1\0";

/// Computes the canonical S9.5 payload digest owned by the planner.
#[must_use]
pub fn payload_digest(payload: &MinimalPointPayload) -> Blake3Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PAYLOAD_DIGEST_DOMAIN);
    hasher.update(payload.installation_incarnation_id.as_bytes());
    hasher.update(payload.collection_generation_id.as_bytes());
    hasher.update(payload.projection_membership_id.as_bytes());
    hasher.update(payload.access_partition_id.as_bytes());
    hasher.update(payload.scoring_partition_id.as_bytes());
    hasher.update(payload.source_id.as_bytes());
    hasher.update(payload.source_revision_id.as_bytes());
    hasher.update(payload.representation_id.as_bytes());
    hasher.update(payload.unit_id.as_bytes());
    hasher.update(payload.point_identity_digest_256.as_bytes());
    hasher.update(payload.scoring_document_id.as_bytes());
    hash_text(&mut hasher, payload.projection_profile_set_id.as_str());
    hash_text(&mut hasher, payload.unit_kind.as_str());
    hash_text(&mut hasher, payload.modality.as_str());
    hash_text(&mut hasher, payload.language_or_format.as_str());
    hash_optional_text(
        &mut hasher,
        payload.entity_kind.as_ref().map(|value| value.as_str()),
    );
    hash_optional_text(
        &mut hasher,
        payload
            .normalized_symbol_key
            .as_ref()
            .map(search_contracts::BoundedSymbolKey::as_str),
    );
    match payload.repository_lineage_id {
        Some(lineage) => {
            hasher.update(&[1]);
            hasher.update(lineage.as_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.update(&payload.valid_from_epoch.get().to_be_bytes());
    match payload.valid_until_epoch_exclusive {
        Some(until) => {
            hasher.update(&[1]);
            hasher.update(&until.get().to_be_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
}

/// Computes a named-vector digest from its name, immutable shape and values.
#[must_use]
pub fn vector_digest(
    name: &str,
    requirement: VectorRequirement,
    value: &VectorValue,
) -> Blake3Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(VECTOR_DIGEST_DOMAIN);
    hash_text(&mut hasher, name);
    hasher.update(&requirement.dimensions.to_be_bytes());
    hasher.update(&[match requirement.kind {
        VectorKind::Dense => 0,
        VectorKind::Sparse => 1,
    }]);
    hasher.update(&[u8::from(requirement.idf_enabled)]);
    match value {
        VectorValue::Dense(values) => {
            hasher.update(&[0]);
            hash_length(&mut hasher, values.len());
            for value in values {
                hasher.update(&value.to_bits().to_be_bytes());
            }
        }
        VectorValue::Sparse { indices, values } => {
            hasher.update(&[1]);
            hash_length(&mut hasher, values.len());
            for (index, value) in indices.iter().zip(values) {
                hasher.update(&index.to_be_bytes());
                hasher.update(&value.to_bits().to_be_bytes());
            }
        }
    }
    Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
}

/// Digest of the exact immutable manifest body.
#[must_use]
pub fn manifest_digest(manifest: &ProjectionManifest) -> Blake3Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MANIFEST_DIGEST_DOMAIN);
    hasher.update(&manifest.canonical_bytes);
    Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
}

pub fn derive_scoring_document_id(
    source_revision_id: SourceRevisionId,
    representation_id: RepresentationId,
    unit_id: UnitId,
    projection_profile_set_id: &ProjectionProfileSetId,
) -> ScoringDocumentId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SCORING_DOCUMENT_DOMAIN);
    hasher.update(source_revision_id.as_bytes());
    hasher.update(representation_id.as_bytes());
    hasher.update(unit_id.as_bytes());
    hash_text(&mut hasher, projection_profile_set_id.as_str());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    ScoringDocumentId::from_bytes(bytes)
}

pub fn canonical_manifest_bytes(
    entries: &[ProjectionManifestEntry],
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    identity_limits: PointIdentityLimits,
) -> Result<Vec<u8>, ProjectionError> {
    let budget = budget.validate()?;
    let mut writer = ManifestWriter::new(budget.max_manifest_bytes);
    writer.bytes(MANIFEST_BODY_DOMAIN)?;
    writer.text(profiles.projection_profile_set_id.as_str())?;
    writer.text(profiles.projection_schema_id.as_str())?;
    writer.usize(entries.len())?;
    for entry in entries {
        writer.bytes(entry.point_id.as_bytes())?;
        let key = encode_canonical_key(&entry.identity_key, identity_limits)?;
        writer.bytes(key.as_slice())?;
        writer.bytes(entry.point_identity_digest_256.as_bytes())?;
        writer.bytes(entry.source_membership_id.as_bytes())?;
        writer.bytes(entry.unit_id.as_bytes())?;
        writer.bytes(entry.payload_digest.as_bytes())?;
        writer.usize(entry.vector_digests.len())?;
        for (name, digest) in &entry.vector_digests {
            writer.text(name)?;
            writer.bytes(digest.as_bytes())?;
        }
        writer.bytes(entry.unit_digest.as_bytes())?;
        writer.bytes(entry.reference_digest.as_bytes())?;
    }
    Ok(writer.finish())
}

pub fn planned_vector(
    prepared: &PreparedVector,
    requirement: VectorRequirement,
) -> NamedVector {
    NamedVector {
        name: prepared.name.clone(),
        requirement,
        value: prepared.value.clone(),
        digest: vector_digest(&prepared.name, requirement, &prepared.value),
    }
}

fn hash_length(hasher: &mut blake3::Hasher, length: usize) {
    let value = u64::try_from(length).unwrap_or(u64::MAX);
    hasher.update(&value.to_be_bytes());
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hash_length(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn hash_optional_text(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_text(hasher, value);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

struct ManifestWriter {
    bytes: Vec<u8>,
    maximum: usize,
}

impl ManifestWriter {
    const fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    fn usize(&mut self, value: usize) -> Result<(), ProjectionError> {
        let value = u64::try_from(value).map_err(|_| ProjectionError::ManifestTooLarge)?;
        self.raw(&value.to_be_bytes())
    }

    fn text(&mut self, value: &str) -> Result<(), ProjectionError> {
        self.bytes(value.as_bytes())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), ProjectionError> {
        self.usize(value.len())?;
        self.raw(value)
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), ProjectionError> {
        let next = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or(ProjectionError::ManifestTooLarge)?;
        if next > self.maximum {
            return Err(ProjectionError::ManifestTooLarge);
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
