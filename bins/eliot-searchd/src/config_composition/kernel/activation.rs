//! Atomic configuration activation planning and receipt gating.

use std::collections::BTreeSet;

use search_config::{
    ConfigError, ReceiptKind, ReconfigurationPlan, diff, plan_reconfiguration,
};

use super::snapshot::EffectiveDaemonConfig;
use super::spec::{
    ACTIVATION_BLOCKED, ACTIVATION_PARTIAL_REFUSED, ACTIVATION_REJECTED,
    config_code,
};

/// Plans activation of `candidate` over `current`, preserving every
/// independent obligation without scalar collapse.
pub fn plan_activation(
    current: &EffectiveDaemonConfig,
    candidate: &EffectiveDaemonConfig,
) -> Result<ReconfigurationPlan, ConfigError> {
    let delta = diff(
        current.snapshot(),
        candidate.snapshot(),
        current.registry(),
    )?;
    plan_reconfiguration(&delta)
}

/// Atomically gates publication of `candidate`.
///
/// The candidate becomes authoritative only when every receipt required by
/// its plan is present in `proven`. Otherwise the caller retains `current`.
pub fn try_activate(
    current: &EffectiveDaemonConfig,
    candidate: EffectiveDaemonConfig,
    proven: &BTreeSet<ReceiptKind>,
) -> Result<EffectiveDaemonConfig, String> {
    if current.fingerprint() == candidate.fingerprint() {
        return Ok(candidate);
    }
    let plan = plan_activation(current, &candidate).map_err(|error| {
        let code = config_code(error);
        if matches!(error, ConfigError::ReconfigurationRejected) {
            ACTIVATION_REJECTED.to_owned()
        } else {
            format!("{ACTIVATION_BLOCKED}:{code}:{error}")
        }
    })?;
    if plan.is_noop() {
        return Ok(candidate);
    }
    if plan
        .required_receipts
        .iter()
        .copied()
        .any(|receipt| !proven.contains(&receipt))
    {
        Err(ACTIVATION_PARTIAL_REFUSED.to_owned())
    } else {
        Ok(candidate)
    }
}
