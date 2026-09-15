//! Exact fixed-width projection-reference codec.

use super::error::ProjectionCompositionError;
use super::model::ProjectionReference;
use super::spec::{REFERENCE_BYTES, REFERENCE_MAGIC};

impl ProjectionReference {
    /// Serializes the reference to its exact 88-byte record form.
    #[must_use]
    pub fn to_bytes(self) -> [u8; REFERENCE_BYTES] {
        let mut output = [0_u8; REFERENCE_BYTES];
        output[..8].copy_from_slice(REFERENCE_MAGIC);
        output[8..40].copy_from_slice(&self.scope_key);
        output[40..72].copy_from_slice(&self.manifest_digest);
        output[72..80].copy_from_slice(&self.manifest_bytes.to_be_bytes());
        output[80..88].copy_from_slice(&self.point_count.to_be_bytes());
        output
    }

    /// Parses one exact reference record; malformed input fails closed.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionCompositionError::ManifestInvalid`] when the magic
    /// prefix or fixed length is wrong.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProjectionCompositionError> {
        if bytes.len() != REFERENCE_BYTES || bytes[..8] != *REFERENCE_MAGIC {
            return Err(ProjectionCompositionError::ManifestInvalid);
        }
        let scope_key: [u8; 32] = bytes[8..40]
            .try_into()
            .map_err(|_| ProjectionCompositionError::ManifestInvalid)?;
        let manifest_digest: [u8; 32] = bytes[40..72]
            .try_into()
            .map_err(|_| ProjectionCompositionError::ManifestInvalid)?;
        let manifest_bytes = u64::from_be_bytes(
            bytes[72..80]
                .try_into()
                .map_err(|_| ProjectionCompositionError::ManifestInvalid)?,
        );
        let point_count = u64::from_be_bytes(
            bytes[80..88]
                .try_into()
                .map_err(|_| ProjectionCompositionError::ManifestInvalid)?,
        );
        Ok(Self {
            scope_key,
            manifest_digest,
            manifest_bytes,
            point_count,
        })
    }
}

pub(super) fn verify_reference_roundtrip(
    reference: &ProjectionReference,
) -> Result<(), ProjectionCompositionError> {
    let parsed = ProjectionReference::from_bytes(&reference.to_bytes())?;
    if &parsed != reference {
        return Err(ProjectionCompositionError::ManifestInvalid);
    }
    Ok(())
}
