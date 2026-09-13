//! Fail-closed profile transition classification.

use super::model::ValidatedMaterializerProfile;

/// Profile change classification. Existing representations are never
/// reinterpreted under a changed profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MaterializerProfileChange {
    /// Identical canonical identity: nothing to redo.
    Noop,
    /// Behavior or bounds changed at a monotone revision: reprepare from the
    /// exact retained revision and reproject coordinates.
    RePreparationAndReprojection,
    /// Reserved for a future qualified optional document provider (P17).
    /// Baseline code never produces this variant.
    OptionalProviderGateRequired,
    /// Non-monotone revision or incompatible coordinate basis: the change
    /// must not be applied.
    Reject,
}

/// Classifies a profile transition without touching stored representations.
#[must_use]
pub fn classify_profile_change(
    old: &ValidatedMaterializerProfile,
    new: &ValidatedMaterializerProfile,
) -> MaterializerProfileChange {
    if old.id() == new.id() {
        return MaterializerProfileChange::Noop;
    }
    if new.revision() <= old.revision() {
        return MaterializerProfileChange::Reject;
    }
    let shared = old
        .coordinate_spaces()
        .iter()
        .filter(|space| new.coordinate_spaces().contains(space))
        .count();
    if shared == 0 {
        return MaterializerProfileChange::Reject;
    }
    MaterializerProfileChange::RePreparationAndReprojection
}
