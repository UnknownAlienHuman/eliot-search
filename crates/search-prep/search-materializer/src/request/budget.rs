//! Finite per-operation materialization budgets.

use crate::MaterializationError;

/// Finite per-operation budgets. All dimensions are non-zero after validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationBudget {
    /// Maximum exact retained input bytes for this operation.
    pub max_input_bytes: u64,
    /// Maximum canonical output bytes for this operation.
    pub max_output_bytes: u64,
    /// Maximum logical lines for this operation.
    pub max_lines: u64,
    /// Maximum coordinate map segments for this operation.
    pub max_map_segments: u64,
    /// Maximum loss map records for this operation.
    pub max_loss_records: u64,
    /// Maximum elementary decode/normalize/map steps for this operation.
    pub max_steps: u64,
}

/// Conservative default operation budget matching the baseline profile limits.
pub const DEFAULT_MATERIALIZATION_BUDGET: MaterializationBudget = MaterializationBudget {
    max_input_bytes: 8 * 1024 * 1024,
    max_output_bytes: 8 * 1024 * 1024,
    max_lines: 1_000_000,
    max_map_segments: 1_000_032,
    max_loss_records: 1_000_032,
    max_steps: 64 * 1024 * 1024,
};

impl MaterializationBudget {
    /// Validates all finite dimensions as non-zero.
    pub const fn validate(self) -> Result<Self, MaterializationError> {
        if self.max_input_bytes == 0
            || self.max_output_bytes == 0
            || self.max_lines == 0
            || self.max_map_segments == 0
            || self.max_loss_records == 0
            || self.max_steps == 0
        {
            return Err(MaterializationError::InvalidLimits);
        }
        Ok(self)
    }

    pub(crate) const fn effective_input(&self, profile_max: u64) -> u64 {
        if self.max_input_bytes < profile_max {
            self.max_input_bytes
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_output(&self, profile_max: u64) -> u64 {
        if self.max_output_bytes < profile_max {
            self.max_output_bytes
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_lines(&self, profile_max: u64) -> u64 {
        if self.max_lines < profile_max {
            self.max_lines
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_segments(&self, profile_max: u64) -> u64 {
        if self.max_map_segments < profile_max {
            self.max_map_segments
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_loss(&self, profile_max: u64) -> u64 {
        if self.max_loss_records < profile_max {
            self.max_loss_records
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_steps(&self, profile_max: u64) -> u64 {
        if self.max_steps < profile_max {
            self.max_steps
        } else {
            profile_max
        }
    }
}
