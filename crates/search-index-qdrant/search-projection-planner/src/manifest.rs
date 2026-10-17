//! Exact canonical manifest construction and reconstruction proof.
//!
//! Manifest bytes encode exactly the entries they claim. The bytes are
//! produced through the shared #237 canonical CBOR owner; there is no
//! package-local manifest writer and no direct BLAKE3 use.

use search_contracts::{
    BoundedBytes, BoundedList, CanonicalBytes, CanonicalKey, CanonicalText, CanonicalValue,
    MAX_CANONICAL_BYTES, MAX_COLLECTION_ITEMS, to_canonical_cbor,
};

use super::ProjectionBudget;
use super::ProjectionManifest;
use crate::error::ProjectionError;
use crate::model::ProjectionManifestEntry;

/// Frozen canonical value emitted inside the manifest preimage.
pub const MANIFEST_VALUE: &str = "eliot-search/projection-manifest/v1";

/// One canonical bounded key.
fn canonical_key(name: &str) -> Result<CanonicalKey, ProjectionError> {
    CanonicalKey::new_non_empty(name).map_err(|_| ProjectionError::InvalidManifest)
}

/// One canonical bounded text value.
fn canonical_text(value: &str) -> Result<CanonicalValue, ProjectionError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| ProjectionError::InvalidManifest)
}

/// One canonical bounded byte value.
fn canonical_bytes(value: &[u8]) -> Result<CanonicalValue, ProjectionError> {
    BoundedBytes::new(value.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| ProjectionError::InvalidManifest)
}

/// Encodes one UUID coordinate as exact canonical bytes.
fn uuid_value(bytes: &[u8; 16]) -> Result<CanonicalValue, ProjectionError> {
    canonical_bytes(bytes)
}

/// Encodes one manifest entry as an exact canonical object.
fn entry_value(entry: &ProjectionManifestEntry) -> Result<CanonicalValue, ProjectionError> {
    let fields: Vec<(CanonicalKey, CanonicalValue)> = vec![
        (canonical_key("point_id")?, uuid_value(entry.point_id.as_bytes())?),
        (
            canonical_key("point_identity_digest_256")?,
            canonical_bytes(entry.point_identity_digest_256.as_bytes())?,
        ),
        (
            canonical_key("source_membership_id")?,
            uuid_value(entry.source_membership_id.as_bytes())?,
        ),
        (
            canonical_key("projection_membership_id")?,
            uuid_value(entry.projection_membership_id.as_bytes())?,
        ),
        (canonical_key("unit_id")?, uuid_value(entry.unit_id.as_bytes())?),
        (
            canonical_key("collection_generation_id")?,
            uuid_value(entry.collection_generation_id.as_bytes())?,
        ),
        (
            canonical_key("installation_incarnation_id")?,
            uuid_value(entry.installation_incarnation_id.as_bytes())?,
        ),
        (
            canonical_key("access_partition_id")?,
            uuid_value(entry.access_partition_id.as_bytes())?,
        ),
        (
            canonical_key("scoring_partition_id")?,
            uuid_value(entry.scoring_partition_id.as_bytes())?,
        ),
        (
            canonical_key("scoring_document_id")?,
            uuid_value(entry.scoring_document_id.as_bytes())?,
        ),
        (
            canonical_key("projection_profile_set_id")?,
            canonical_text(entry.projection_profile_set_id.as_str())?,
        ),
        (canonical_key("unit_digest")?, canonical_bytes(entry.unit_digest.as_bytes())?),
        (
            canonical_key("reference_digest")?,
            canonical_bytes(entry.reference_digest.as_bytes())?,
        ),
        (
            canonical_key("payload_identity")?,
            canonical_bytes(entry.payload_identity.as_bytes())?,
        ),
        (
            canonical_key("vector_digests")?,
            CanonicalValue::Array(BoundedList::new(vector_pairs(entry)?)?),
        ),
    ];
    Ok(CanonicalValue::Object(
        search_contracts::BoundedMap::from_entries(fields)
            .map_err(|_| ProjectionError::InvalidManifest)?,
    ))
}

/// Encodes every named-vector digest as an exact canonical name/value pair.
fn vector_pairs(entry: &ProjectionManifestEntry) -> Result<Vec<CanonicalValue>, ProjectionError> {
    entry
        .vector_digests
        .iter()
        .map(|(name, digest)| {
            let pair: Vec<CanonicalValue> = vec![canonical_text(name)?, canonical_bytes(digest.as_bytes())?];
            Ok(CanonicalValue::Array(BoundedList::new(pair)?))
        })
        .collect()
}

/// Encodes one complete manifest as exact canonical bytes.
///
/// The declared row count is validated against `MAX_COLLECTION_ITEMS` before
/// any row is constructed, and the total is bounded by the finite manifest
/// ceiling before any controlled allocation.
pub fn manifest_canonical_bytes(
    manifest: &ProjectionManifest,
    budget: ProjectionBudget,
) -> Result<CanonicalBytes, ProjectionError> {
    let budget = budget.validate()?;
    if manifest.entries.len() > budget.max_points
        || manifest.entries.len() > MAX_COLLECTION_ITEMS
    {
        return Err(ProjectionError::ManifestTooLarge);
    }
    let rows = manifest
        .entries
        .iter()
        .map(entry_value)
        .collect::<Result<Vec<_>, ProjectionError>>()?;
    let preimage = CanonicalValue::Object(
        search_contracts::BoundedMap::from_entries(vec![
            (canonical_key("value")?, canonical_text(MANIFEST_VALUE)?),
            (
                canonical_key("entries")?,
                CanonicalValue::Array(
                    BoundedList::new(rows).map_err(|_| ProjectionError::ManifestTooLarge)?,
                ),
            ),
        ])
        .map_err(|_| ProjectionError::InvalidManifest)?,
    );
    let encoded =
        to_canonical_cbor(&preimage).map_err(|_| ProjectionError::ManifestTooLarge)?;
    if encoded.len() > budget.max_manifest_bytes || encoded.len() > MAX_CANONICAL_BYTES {
        return Err(ProjectionError::ManifestTooLarge);
    }
    Ok(encoded)
}
