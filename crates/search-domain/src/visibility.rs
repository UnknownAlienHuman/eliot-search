//! Truthful capability-publication semantics.

use std::collections::BTreeSet;

use crate::{Decision, ReasonSet};

/// Runtime lifecycle of a capability surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityState {
    /// Capability is absent.
    Absent,
    /// Capability exists but is disabled.
    Disabled,
    /// Authority or dependencies are missing.
    Blocked,
    /// Implementation is starting but not request-ready.
    Starting,
    /// Every publication requirement is satisfied.
    Ready,
    /// New work is denied while accepted work drains.
    Draining,
    /// Contradictory state is quarantined.
    Quarantined,
}

/// State of one readiness prerequisite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequirementState {
    /// The prerequisite is satisfied by current exact evidence.
    Satisfied,
    /// The prerequisite is absent, stale, or denied.
    Missing,
}

/// Inputs required before publishing a capability as ready.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibilityRequirement {
    /// Accepted selected feature/profile binding.
    pub binding: RequirementState,
    /// Accepted direct package and stage dependencies.
    pub dependencies: RequirementState,
    /// Exact runtime/artifact identity and readiness.
    pub runtime: RequirementState,
    /// Current access, security, and lifecycle permission.
    pub security: RequirementState,
}

/// Why a capability cannot be published as ready.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VisibilityReason {
    /// No accepted binding authorizes the capability.
    BindingNotAccepted,
    /// Dependency handoff is absent or stale.
    DependencyNotAccepted,
    /// Exact runtime readiness is absent.
    RuntimeNotReady,
    /// Live security/lifecycle state denies exposure.
    SecurityDenied,
}

/// Result of capability visibility evaluation.
pub type VisibilityDecision = Decision<CapabilityState, VisibilityReason>;

/// Determines whether a capability may be published as ready.
#[must_use]
pub fn evaluate_visibility(requirement: VisibilityRequirement) -> VisibilityDecision {
    let mut reasons = BTreeSet::new();
    if requirement.binding == RequirementState::Missing {
        reasons.insert(VisibilityReason::BindingNotAccepted);
    }
    if requirement.dependencies == RequirementState::Missing {
        reasons.insert(VisibilityReason::DependencyNotAccepted);
    }
    if requirement.runtime == RequirementState::Missing {
        reasons.insert(VisibilityReason::RuntimeNotReady);
    }
    if requirement.security == RequirementState::Missing {
        reasons.insert(VisibilityReason::SecurityDenied);
    }
    let Some(first) = reasons.pop_first() else {
        return Decision::Allow(CapabilityState::Ready);
    };
    let mut reason_set = ReasonSet::one(first);
    for reason in reasons {
        reason_set.extend(ReasonSet::one(reason));
    }
    Decision::Deny(reason_set)
}

#[cfg(test)]
mod tests {
    use super::{
        CapabilityState, RequirementState, VisibilityReason, VisibilityRequirement,
        evaluate_visibility,
    };
    use crate::Decision;

    #[test]
    fn ready_process_without_binding_is_not_visible() {
        let Decision::Deny(reasons) = evaluate_visibility(VisibilityRequirement {
            binding: RequirementState::Missing,
            dependencies: RequirementState::Satisfied,
            runtime: RequirementState::Satisfied,
            security: RequirementState::Satisfied,
        }) else {
            panic!("process health is not authority");
        };
        assert_eq!(
            reasons.iter().copied().collect::<Vec<_>>(),
            [VisibilityReason::BindingNotAccepted]
        );
    }

    #[test]
    fn every_precondition_is_required_for_ready() {
        assert_eq!(
            evaluate_visibility(VisibilityRequirement {
                binding: RequirementState::Satisfied,
                dependencies: RequirementState::Satisfied,
                runtime: RequirementState::Satisfied,
                security: RequirementState::Satisfied,
            }),
            Decision::Allow(CapabilityState::Ready)
        );
    }
}
