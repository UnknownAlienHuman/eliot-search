//! Conservative assurance aggregation over canonical contract classes.

use search_contracts::AssuranceClass;

use crate::{Decision, ReasonSet};

/// Minimum assurance required by an operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssuranceRequirement {
    /// Lowest acceptable canonical assurance class.
    pub minimum: AssuranceClass,
    /// Whether every contributing leg must independently satisfy the floor.
    pub require_every_contributor: bool,
}

/// Typed reason for an assurance denial.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AssuranceReason {
    /// No assurance-bearing contributor was supplied.
    NoContributors,
    /// The aggregate assurance is below the operation floor.
    BelowMinimum,
    /// At least one contributor is below the operation floor.
    ContributorBelowMinimum,
}

/// Result of conservative assurance evaluation.
pub type AssuranceDecision = Decision<AssuranceClass, AssuranceReason>;

/// Returns the weakest canonical assurance class in a non-empty set.
#[must_use]
pub fn minimum_assurance(contributors: &[AssuranceClass]) -> Option<AssuranceClass> {
    contributors
        .iter()
        .copied()
        .min_by_key(|value| assurance_strength(*value))
}

/// Combines assurance without upgrading any contributor.
#[must_use]
pub fn evaluate_assurance(
    contributors: &[AssuranceClass],
    requirement: AssuranceRequirement,
) -> AssuranceDecision {
    let Some(aggregate) = minimum_assurance(contributors) else {
        return Decision::Deny(ReasonSet::one(AssuranceReason::NoContributors));
    };

    if assurance_strength(aggregate) < assurance_strength(requirement.minimum) {
        let reason = if requirement.require_every_contributor {
            AssuranceReason::ContributorBelowMinimum
        } else {
            AssuranceReason::BelowMinimum
        };
        Decision::Deny(ReasonSet::one(reason))
    } else {
        Decision::Allow(aggregate)
    }
}

const fn assurance_strength(value: AssuranceClass) -> u8 {
    match value {
        AssuranceClass::ExactBytes => 4,
        AssuranceClass::MappedText => 3,
        AssuranceClass::LossyText => 2,
        AssuranceClass::DescriptiveOnly => 1,
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::AssuranceClass;

    use super::{AssuranceReason, AssuranceRequirement, evaluate_assurance};
    use crate::Decision;

    #[test]
    fn aggregate_never_exceeds_weakest_contributor() {
        let Decision::Allow(level) = evaluate_assurance(
            &[AssuranceClass::ExactBytes, AssuranceClass::LossyText],
            AssuranceRequirement {
                minimum: AssuranceClass::DescriptiveOnly,
                require_every_contributor: false,
            },
        ) else {
            panic!("minimum is satisfied");
        };
        assert_eq!(level, AssuranceClass::LossyText);
    }

    #[test]
    fn weak_contributor_fails_a_stronger_floor() {
        let Decision::Deny(reasons) = evaluate_assurance(
            &[AssuranceClass::ExactBytes, AssuranceClass::LossyText],
            AssuranceRequirement {
                minimum: AssuranceClass::MappedText,
                require_every_contributor: true,
            },
        ) else {
            panic!("lossy text is below mapped text");
        };
        assert_eq!(
            reasons.iter().copied().collect::<Vec<_>>(),
            [AssuranceReason::ContributorBelowMinimum]
        );
    }
}
