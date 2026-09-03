//! Deterministic composite reconfiguration planning.

use std::collections::BTreeSet;

use crate::{
    ConfigDelta, ConfigError, ConfigFingerprint, ConfigOwner, ReceiptKind, ReconfigurationAction,
};

/// One deterministic action step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconfigurationStep {
    /// Zero-based deterministic step order.
    pub ordinal: u32,
    /// Independent action performed by this step.
    pub action: ReconfigurationAction,
    /// Capability owners affected by this action.
    pub affected_capabilities: BTreeSet<ConfigOwner>,
    /// Receipt required before the candidate may publish.
    pub required_receipt: Option<ReceiptKind>,
}

/// Complete fail-closed plan for one candidate fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconfigurationPlan {
    /// Currently authoritative fingerprint.
    pub old_fingerprint: ConfigFingerprint,
    /// Candidate fingerprint, unpublished until all receipts succeed.
    pub candidate_fingerprint: ConfigFingerprint,
    /// Every independent obligation; no scalar dominance collapse occurs.
    pub required_actions: BTreeSet<ReconfigurationAction>,
    /// Deterministically ordered execution steps.
    pub ordered_steps: Vec<ReconfigurationStep>,
    /// Every affected capability owner.
    pub affected_capabilities: BTreeSet<ConfigOwner>,
    /// Receipt classes required before publication.
    pub required_receipts: BTreeSet<ReceiptKind>,
    /// Whether activation remains blocked on an external gate receipt.
    pub activation_blocked: bool,
}

impl ReconfigurationPlan {
    /// Returns whether no action is required.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.required_actions.is_empty()
    }
}

/// Preserves all independent obligations and orders their prerequisites.
///
/// # Errors
///
/// Returns [`ConfigError::ReconfigurationRejected`] when any field contributes
/// the explicit reject action. No executable steps are returned in that case.
pub fn plan_reconfiguration(delta: &ConfigDelta) -> Result<ReconfigurationPlan, ConfigError> {
    let required_actions = delta.required_actions();
    if required_actions.contains(&ReconfigurationAction::Reject) {
        return Err(ConfigError::ReconfigurationRejected);
    }
    let affected_capabilities = delta.affected_capabilities();
    let mut actions = required_actions.iter().copied().collect::<Vec<_>>();
    actions.sort_by_key(|action| (topological_rank(*action), action.order()));

    let mut ordered_steps = Vec::with_capacity(actions.len());
    let mut required_receipts = BTreeSet::new();
    for (index, action) in actions.into_iter().enumerate() {
        let required_receipt = action.receipt();
        if let Some(receipt) = required_receipt {
            required_receipts.insert(receipt);
        }
        let ordinal = u32::try_from(index).map_err(|_| ConfigError::LengthOverflow)?;
        let affected = delta
            .sections
            .values()
            .filter(|section| section.required_actions.contains(&action))
            .map(|section| section.owner.clone())
            .collect::<BTreeSet<_>>();
        ordered_steps.push(ReconfigurationStep {
            ordinal,
            action,
            affected_capabilities: affected,
            required_receipt,
        });
    }

    Ok(ReconfigurationPlan {
        old_fingerprint: delta.old_fingerprint,
        candidate_fingerprint: delta.candidate_fingerprint,
        activation_blocked: required_actions.contains(&ReconfigurationAction::GateRequired),
        required_actions,
        ordered_steps,
        affected_capabilities,
        required_receipts,
    })
}

const fn topological_rank(action: ReconfigurationAction) -> u8 {
    match action {
        ReconfigurationAction::SecurityBarrier => 0,
        ReconfigurationAction::ApplyLive => 1,
        ReconfigurationAction::RestartDependency => 2,
        ReconfigurationAction::DrainAndRestart => 3,
        ReconfigurationAction::MigrateControlSchema => 4,
        ReconfigurationAction::NewCollectionGeneration => 5,
        ReconfigurationAction::RebuildProjection => 6,
        ReconfigurationAction::GateRequired => 7,
        ReconfigurationAction::Reject => 8,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::plan_reconfiguration;
    use crate::{
        ConfigDelta, ConfigFingerprint, ConfigOwner, ConfigSectionName, ReconfigurationAction,
        SectionDelta,
    };

    fn fingerprint(value: u8) -> ConfigFingerprint {
        ConfigFingerprint::from_bytes([value; 32])
    }

    #[test]
    fn mixed_obligations_are_preserved() {
        let owner = ConfigOwner::new("search-source-admission", 64).expect("owner");
        let section_name = ConfigSectionName::new("source_admission", 64).expect("section");
        let delta = ConfigDelta {
            old_fingerprint: fingerprint(1),
            candidate_fingerprint: fingerprint(2),
            profile_changed: false,
            global_actions: BTreeSet::new(),
            sections: BTreeMap::from([(
                section_name.clone(),
                SectionDelta {
                    section_name,
                    owner,
                    changed_key_paths: BTreeSet::new(),
                    required_actions: BTreeSet::from([
                        ReconfigurationAction::SecurityBarrier,
                        ReconfigurationAction::RestartDependency,
                        ReconfigurationAction::RebuildProjection,
                    ]),
                    restrictive: true,
                },
            )]),
        };
        let plan = plan_reconfiguration(&delta).expect("plan");
        assert_eq!(plan.required_actions.len(), 3);
        assert_eq!(
            plan.ordered_steps
                .iter()
                .map(|step| step.action)
                .collect::<Vec<_>>(),
            [
                ReconfigurationAction::SecurityBarrier,
                ReconfigurationAction::RestartDependency,
                ReconfigurationAction::RebuildProjection,
            ]
        );
    }

    #[test]
    fn reject_returns_no_executable_plan() {
        let delta = ConfigDelta {
            old_fingerprint: fingerprint(1),
            candidate_fingerprint: fingerprint(2),
            profile_changed: false,
            global_actions: BTreeSet::from([ReconfigurationAction::Reject]),
            sections: BTreeMap::new(),
        };
        assert_eq!(
            plan_reconfiguration(&delta),
            Err(crate::ConfigError::ReconfigurationRejected)
        );
    }
}
