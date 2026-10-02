use search_contracts::Blake3Digest32;

use crate::{
    MinimalPointPayload, ProjectionBudget, ProjectionError, VectorValue,
};

pub const PAYLOAD_DOMAIN: &[u8] = b"eliot-search/qdrant-point-payload/v1";
pub const VECTOR_DOMAIN: &[u8] = b"eliot-search/projection-vector/v1";
pub const MANIFEST_DOMAIN: &[u8] = b"eliot-search/projection-manifest/v2";

pub struct CanonicalBuffer {
    bytes: Vec<u8>,
    limit: usize,
    overflow: ProjectionError,
}

impl CanonicalBuffer {
    pub fn new(
        domain: &[u8],
        limit: usize,
        overflow: ProjectionError,
    ) -> Result<Self, ProjectionError> {
        if limit == 0 {
            return Err(ProjectionError::InvalidLimits);
        }
        let mut buffer = Self {
            bytes: Vec::new(),
            limit,
            overflow,
        };
        buffer.append_bytes(domain)?;
        Ok(buffer)
    }

    pub fn append_bool(&mut self, value: bool) -> Result<(), ProjectionError> {
        self.extend(&[u8::from(value)])
    }

    pub fn append_u32(&mut self, value: u32) -> Result<(), ProjectionError> {
        self.extend(&value.to_be_bytes())
    }

    pub fn append_u64(&mut self, value: u64) -> Result<(), ProjectionError> {
        self.extend(&value.to_be_bytes())
    }

    pub fn append_i64(&mut self, value: i64) -> Result<(), ProjectionError> {
        self.extend(&value.to_be_bytes())
    }

    pub fn append_f32(&mut self, value: f32) -> Result<(), ProjectionError> {
        if !value.is_finite() {
            return Err(ProjectionError::InvalidVector);
        }
        self.extend(&value.to_bits().to_be_bytes())
    }

    pub fn append_text(&mut self, value: &str) -> Result<(), ProjectionError> {
        self.append_bytes(value.as_bytes())
    }

    pub fn append_bytes(&mut self, value: &[u8]) -> Result<(), ProjectionError> {
        let length = u64::try_from(value.len()).map_err(|_| self.overflow)?;
        self.append_u64(length)?;
        self.extend(value)
    }

    pub fn append_digest(
        &mut self,
        value: Blake3Digest32,
    ) -> Result<(), ProjectionError> {
        self.extend(value.as_bytes())
    }

    pub fn append_optional_text(
        &mut self,
        value: Option<&str>,
    ) -> Result<(), ProjectionError> {
        self.append_bool(value.is_some())?;
        if let Some(value) = value {
            self.append_text(value)?;
        }
        Ok(())
    }

    pub fn append_optional_bytes(
        &mut self,
        value: Option<&[u8]>,
    ) -> Result<(), ProjectionError> {
        self.append_bool(value.is_some())?;
        if let Some(value) = value {
            self.append_bytes(value)?;
        }
        Ok(())
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }

    fn extend(&mut self, value: &[u8]) -> Result<(), ProjectionError> {
        let length = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or(self.overflow)?;
        if length > self.limit {
            return Err(self.overflow);
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }
}

pub fn canonical_payload_bytes(
    payload: &MinimalPointPayload,
    budget: ProjectionBudget,
) -> Result<Vec<u8>, ProjectionError> {
    payload.validate()?;
    let mut output = CanonicalBuffer::new(
        PAYLOAD_DOMAIN,
        budget.max_payload_bytes,
        ProjectionError::BudgetExceeded,
    )?;
    output.append_bytes(payload.installation_incarnation_id.as_bytes())?;
    output.append_bytes(payload.collection_generation_id.as_bytes())?;
    output.append_bytes(payload.projection_membership_id.as_bytes())?;
    output.append_bytes(payload.access_partition_id.as_bytes())?;
    output.append_bytes(payload.scoring_partition_id.as_bytes())?;
    output.append_bytes(payload.source_id.as_bytes())?;
    output.append_bytes(payload.source_revision_id.as_bytes())?;
    output.append_bytes(payload.representation_id.as_bytes())?;
    output.append_bytes(payload.unit_id.as_bytes())?;
    output.append_digest(payload.point_identity_digest_256)?;
    output.append_bytes(payload.scoring_document_id.as_bytes())?;
    output.append_text(payload.projection_profile_set_id.as_str())?;
    output.append_text(payload.unit_kind.as_str())?;
    output.append_text(payload.modality.as_str())?;
    output.append_text(payload.language_or_format.as_str())?;
    output.append_optional_text(
        payload
            .entity_kind
            .map(search_contracts::EntityKind::as_str),
    )?;
    output.append_optional_text(
        payload
            .normalized_symbol_key
            .as_ref()
            .map(search_contracts::BoundedSymbolKey::as_str),
    )?;
    output.append_optional_bytes(
        payload
            .repository_lineage_id
            .as_ref()
            .map(|value| &value.as_bytes()[..]),
    )?;
    output.append_i64(payload.valid_from_epoch.get())?;
    output.append_bool(payload.valid_until_epoch_exclusive.is_some())?;
    if let Some(until) = payload.valid_until_epoch_exclusive {
        output.append_i64(until.get())?;
    }
    Ok(output.into_vec())
}

pub fn canonical_vector_bytes(
    name: &str,
    dimensions: u32,
    value: &VectorValue,
    budget: ProjectionBudget,
) -> Result<Vec<u8>, ProjectionError> {
    value.validate(dimensions)?;
    let mut output = CanonicalBuffer::new(
        VECTOR_DOMAIN,
        budget.max_vector_bytes,
        ProjectionError::BudgetExceeded,
    )?;
    output.append_text(name)?;
    output.append_u32(dimensions)?;
    output.append_bool(value.is_sparse())?;
    output.append_u64(
        u64::try_from(value.stored_values()).map_err(|_| ProjectionError::BudgetExceeded)?,
    )?;
    match value {
        VectorValue::Dense(values) => {
            for value in values {
                output.append_f32(*value)?;
            }
        }
        VectorValue::Sparse { indices, values } => {
            for (index, value) in indices.iter().zip(values) {
                output.append_u32(*index)?;
                output.append_f32(*value)?;
            }
        }
    }
    Ok(output.into_vec())
}
