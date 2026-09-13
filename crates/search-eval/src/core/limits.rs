//! Finite evaluation ceilings shared by corpus, policy, metric, and run owners.

use crate::EvalError;

/// Conservative finite evaluation limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvalLimits {
    /// Maximum cases in one control corpus.
    pub max_cases: usize,
    /// Maximum independent lineages.
    pub max_lineages: usize,
    /// Maximum registered metrics.
    pub max_metrics: usize,
    /// Maximum policy rules.
    pub max_policy_rules: usize,
    /// Maximum artifacts in one frozen run.
    pub max_artifacts: usize,
    /// Maximum repetitions per case/baseline.
    pub max_repetitions: u32,
    /// Maximum warm-up attempts per case/baseline.
    pub max_warmups: u32,
    /// Maximum UTF-8 bytes in one bounded identifier-like label.
    pub max_text_bytes: usize,
    /// Maximum immutable receipt references retained by one object.
    pub max_receipts: usize,
    /// Maximum resource samples retained by one attempt.
    pub max_resource_samples: usize,
    /// Maximum audit events or fault cells.
    pub max_audit_items: usize,
}

impl EvalLimits {
    /// Conservative baseline suitable for local Product Pulse runs.
    pub const BASELINE: Self = Self {
        max_cases: 100_000,
        max_lineages: 10_000,
        max_metrics: 4_096,
        max_policy_rules: 4_096,
        max_artifacts: 1_024,
        max_repetitions: 10_000,
        max_warmups: 1_000,
        max_text_bytes: 4_096,
        max_receipts: 100_000,
        max_resource_samples: 1_000_000,
        max_audit_items: 1_000_000,
    };

    /// Rejects zero or contradictory ceilings.
    pub const fn validate(self) -> Result<Self, EvalError> {
        if self.max_cases == 0
            || self.max_lineages < 8
            || self.max_metrics == 0
            || self.max_policy_rules == 0
            || self.max_artifacts == 0
            || self.max_repetitions == 0
            || self.max_warmups == 0
            || self.max_text_bytes == 0
            || self.max_receipts == 0
            || self.max_resource_samples == 0
            || self.max_audit_items == 0
        {
            Err(EvalError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}
