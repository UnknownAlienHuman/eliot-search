use crate::{ContractError, ContractErrorKind};

use super::codec::error;
use super::{
    CollectionSchema, EligibilityPopulation, MAX_STORED_VECTOR_VALUES, PointPayload, VectorName,
    VectorValue,
};

/// Exact named values returned by a provider. These remain untrusted hints
/// until collection, eligibility and authoritative source checks succeed.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedVectorValue {
    pub name: VectorName,
    pub value: VectorValue,
}

/// Typed point readback without a vendor point-id or physical route.
/// The collision-safe address is owned separately by point identity.
#[derive(Clone, Debug, PartialEq)]
pub struct PointReadback {
    pub payload: PointPayload,
    pub vectors: Vec<NamedVectorValue>,
}

impl PointReadback {
    /// Checks exact payload binding and complete named-vector shape, then the
    /// same eligibility population used by retrieval and filtered IDF.
    /// This does not establish authoritative source bytes or citation validity.
    pub fn validate(
        &self,
        schema: &CollectionSchema,
        population: &EligibilityPopulation,
    ) -> Result<(), ContractError> {
        self.payload.validate()?;
        population.validate_for_schema(schema)?;
        if !population.matches(&self.payload) {
            return Err(error(
                ContractErrorKind::ContradictoryState,
                "point_readback_population",
            ));
        }
        let requirements = schema.vectors();
        if self.vectors.len() != requirements.len() {
            return Err(error(
                ContractErrorKind::MalformedPayload,
                "point_readback_vectors",
            ));
        }
        let value_limit = usize::try_from(MAX_STORED_VECTOR_VALUES)
            .map_err(|_| error(ContractErrorKind::TooManyItems, "point_readback_values"))?;
        let mut total_values = 0_usize;
        for vector in &self.vectors {
            let values = match &vector.value {
                VectorValue::Dense(values) | VectorValue::Sparse { values, .. } => values.len(),
            };
            total_values = total_values
                .checked_add(values)
                .ok_or_else(|| error(ContractErrorKind::TooManyItems, "point_readback_values"))?;
            if total_values > value_limit {
                return Err(error(
                    ContractErrorKind::TooManyItems,
                    "point_readback_values",
                ));
            }
        }
        for (position, vector) in self.vectors.iter().enumerate() {
            if self.vectors[..position]
                .iter()
                .any(|other| other.name == vector.name)
            {
                return Err(error(ContractErrorKind::Duplicate, "point_readback_vector"));
            }
            let requirement = requirements
                .iter()
                .find(|required| required.name == vector.name)
                .ok_or_else(|| error(ContractErrorKind::UnknownField, "point_readback_vector"))?;
            vector.value.validate(&requirement.requirement)?;
        }
        Ok(())
    }
}
