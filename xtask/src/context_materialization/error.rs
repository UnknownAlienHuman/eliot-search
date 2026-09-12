//! Closed context-materialization helper error.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// Typed failure carrying one stable machine reason code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationPlanError {
    reason: &'static str,
    message: String,
}

impl MaterializationPlanError {
    pub(crate) fn new(
        reason: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    /// Machine reason code from the closed registry.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }

    /// Content-free diagnostic detail.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for MaterializationPlanError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "{}: {}", self.reason, self.message)
    }
}

impl Error for MaterializationPlanError {}
