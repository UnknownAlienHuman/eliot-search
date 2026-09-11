use crate::BridgeError;

/// Finite bridge limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeLimits {
    pub max_points_per_mutation: usize,
    pub max_query_candidates: usize,
    pub max_vector_values_per_point: usize,
    pub max_operation_receipts: usize,
}

impl BridgeLimits {
    pub const BASELINE: Self = Self {
        max_points_per_mutation: 1_024,
        max_query_candidates: 4_096,
        max_vector_values_per_point: 65_536,
        max_operation_receipts: 65_536,
    };

    pub const fn validate(self) -> Result<Self, BridgeError> {
        if self.max_points_per_mutation == 0
            || self.max_query_candidates == 0
            || self.max_vector_values_per_point == 0
            || self.max_operation_receipts == 0
        {
            Err(BridgeError::MutationTooLarge)
        } else {
            Ok(self)
        }
    }
}
