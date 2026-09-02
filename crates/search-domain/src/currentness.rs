//! Independent currentness axes, snapshot drift, and emission revalidation.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    ObservationFreshnessState, QuerySnapshotFence, ResultFence, SearchTaskPlan,
};

use crate::{Decision, DomainError, DomainErrorKind, ReasonSet};

/// A load-bearing currentness axis.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CurrentnessAxis {
    /// Authoritative source observation continuity.
    SourceObservation,
    /// Saved immutable source-revision identity.
    SavedRevision,
    /// Authenticated unsaved buffer-snapshot identity.
    BufferSnapshot,
    /// Published projection generation and route.
    Projection,
    /// Live access, deny, shadow, and purge state.
    Security,
}

/// Currentness state of one axis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentnessState {
    /// The axis is verified current for this operation.
    Current,
    /// The axis is known stale.
    Stale,
    /// The axis could not be established.
    Unknown,
    /// The axis does not apply to this operation.
    NotApplicable,
}

/// Currentness axes required by one operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CurrentnessRequirement {
    mandatory: BTreeSet<CurrentnessAxis>,
}

impl CurrentnessRequirement {
    /// Creates a requirement from mandatory axes.
    #[must_use]
    pub fn new<I>(mandatory: I) -> Self
    where
        I: IntoIterator<Item = CurrentnessAxis>,
    {
        Self {
            mandatory: mandatory.into_iter().collect(),
        }
    }

    /// Returns mandatory axes in canonical order.
    #[must_use]
    pub fn mandatory_axes(&self) -> impl ExactSizeIterator<Item = CurrentnessAxis> + '_ {
        self.mandatory.iter().copied()
    }
}

/// Why an operation cannot claim currentness.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CurrentnessReason {
    /// A required axis is absent.
    MissingAxis(CurrentnessAxis),
    /// A required axis is stale.
    StaleAxis(CurrentnessAxis),
    /// A required axis is unknown.
    UnknownAxis(CurrentnessAxis),
    /// A required axis was incorrectly marked not applicable.
    RequiredAxisNotApplicable(CurrentnessAxis),
}

/// Result of evaluating all required axes.
pub type CurrentnessDecision = Decision<(), CurrentnessReason>;

/// Evaluates every mandatory currentness axis independently.
#[must_use]
pub fn evaluate_currentness(
    requirement: &CurrentnessRequirement,
    states: &BTreeMap<CurrentnessAxis, CurrentnessState>,
) -> CurrentnessDecision {
    let mut reasons = BTreeSet::new();
    for axis in requirement.mandatory_axes() {
        match states.get(&axis) {
            Some(CurrentnessState::Current) => {}
            Some(CurrentnessState::Stale) => {
                reasons.insert(CurrentnessReason::StaleAxis(axis));
            }
            Some(CurrentnessState::Unknown) => {
                reasons.insert(CurrentnessReason::UnknownAxis(axis));
            }
            Some(CurrentnessState::NotApplicable) => {
                reasons.insert(CurrentnessReason::RequiredAxisNotApplicable(axis));
            }
            None => {
                reasons.insert(CurrentnessReason::MissingAxis(axis));
            }
        }
    }
    let Some(first) = reasons.pop_first() else {
        return Decision::Allow(());
    };
    let mut reason_set = ReasonSet::one(first);
    for reason in reasons {
        reason_set.extend(ReasonSet::one(reason));
    }
    Decision::Deny(reason_set)
}

/// Exact load-bearing snapshot axis that drifted.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SnapshotDriftAxis {
    /// Installation incarnation changed.
    InstallationIncarnation,
    /// Collection generation changed.
    CollectionGeneration,
    /// Visible epoch changed.
    VisibleEpoch,
    /// Collection route revision changed.
    CollectionRoute,
    /// Catalog revision changed.
    Catalog,
    /// Membership revision changed.
    Membership,
    /// Reference portfolio revision changed.
    Portfolio,
    /// Access-policy revision changed.
    AccessPolicy,
    /// Shadow fence changed.
    ShadowFence,
    /// Purge fence changed.
    PurgeFence,
    /// Overlay revision changed.
    Overlay,
    /// Observation cursor changed.
    ObservationCursor,
    /// Observation freshness changed.
    ObservationFreshness,
    /// Source view changed.
    SourceView,
    /// Workspace-view revision changed.
    WorkspaceView,
    /// Lexical profile set changed.
    LexicalProfiles,
}

/// Conservative classification of drift between planned and observed snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotDrift {
    axes: BTreeSet<SnapshotDriftAxis>,
    requires_replan: bool,
    requires_emission_revalidation: bool,
}

impl SnapshotDrift {
    /// Changed axes in canonical order.
    #[must_use]
    pub fn axes(&self) -> impl ExactSizeIterator<Item = SnapshotDriftAxis> + '_ {
        self.axes.iter().copied()
    }

    /// Whether load-bearing planning inputs changed.
    #[must_use]
    pub const fn requires_replan(&self) -> bool {
        self.requires_replan
    }

    /// Whether restrictive access/shadow/purge state requires final revalidation.
    #[must_use]
    pub const fn requires_emission_revalidation(&self) -> bool {
        self.requires_emission_revalidation
    }

    /// Whether no load-bearing axis changed.
    #[must_use]
    pub fn is_unchanged(&self) -> bool {
        self.axes.is_empty()
    }
}

/// Classifies every explicit S14 snapshot axis independently.
///
/// # Errors
///
/// Rejects invalid snapshots and a fingerprint-only difference that cannot be
/// explained by any load-bearing field.
pub fn classify_snapshot_drift(
    planned: &QuerySnapshotFence,
    observed: &QuerySnapshotFence,
) -> Result<SnapshotDrift, DomainError> {
    planned.validate().map_err(DomainError::from)?;
    observed.validate().map_err(DomainError::from)?;

    let mut axes = BTreeSet::new();
    macro_rules! changed {
        ($axis:ident, $field:ident) => {
            if planned.$field != observed.$field {
                axes.insert(SnapshotDriftAxis::$axis);
            }
        };
    }
    changed!(InstallationIncarnation, installation_incarnation_id);
    changed!(CollectionGeneration, collection_generation_id);
    changed!(VisibleEpoch, visible_epoch);
    changed!(CollectionRoute, collection_route_revision);
    changed!(Catalog, catalog_revision);
    changed!(Membership, membership_revision);
    changed!(Portfolio, reference_portfolio_revision);
    changed!(AccessPolicy, access_policy_revision);
    changed!(ShadowFence, shadow_fence_revision);
    changed!(PurgeFence, purge_fence_revision);
    changed!(Overlay, overlay_revision);
    changed!(ObservationCursor, observation_cursor_revision);
    changed!(ObservationFreshness, observation_freshness);
    changed!(SourceView, source_view);
    changed!(WorkspaceView, workspace_view_revision_ref);
    changed!(LexicalProfiles, lexical_profile_ids);

    if axes.is_empty() && planned.snapshot_fingerprint != observed.snapshot_fingerprint {
        return Err(DomainError::new(
            DomainErrorKind::FingerprintMismatch,
            "query_snapshot.snapshot_fingerprint",
        ));
    }

    let requires_emission_revalidation = axes.contains(&SnapshotDriftAxis::AccessPolicy)
        || axes.contains(&SnapshotDriftAxis::ShadowFence)
        || axes.contains(&SnapshotDriftAxis::PurgeFence)
        || observed.observation_freshness.state == ObservationFreshnessState::GapDetected
        || observed.observation_freshness.state == ObservationFreshnessState::Unknown;

    Ok(SnapshotDrift {
        requires_replan: !axes.is_empty(),
        axes,
        requires_emission_revalidation,
    })
}

/// Verifies that an emission fence preserves the exact planning snapshot and
/// reports whether source-owner or restrictive-security state changed.
///
/// # Errors
///
/// Rejects a result that rewrites its planned snapshot.
pub fn emission_requires_revalidation(
    plan: &SearchTaskPlan,
    result: &ResultFence,
) -> Result<bool, DomainError> {
    plan.validate().map_err(DomainError::from)?;
    result
        .planned_snapshot
        .validate()
        .map_err(DomainError::from)?;
    if result.planned_snapshot != plan.query_snapshot_fence {
        return Err(DomainError::new(
            DomainErrorKind::InvariantViolation,
            "result_fence.planned_snapshot",
        ));
    }

    Ok(
        result.emission_source_owner_fences != plan.source_owner_fences
            || result.emission_security_fence.access_policy_revision
                != plan.query_snapshot_fence.access_policy_revision
            || result.emission_security_fence.shadow_fence_revision
                != plan.query_snapshot_fence.shadow_fence_revision
            || result.emission_security_fence.purge_fence_revision
                != plan.query_snapshot_fence.purge_fence_revision,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        CurrentnessAxis, CurrentnessReason, CurrentnessRequirement, CurrentnessState,
        evaluate_currentness,
    };
    use crate::Decision;

    #[test]
    fn current_projection_cannot_hide_stale_source_observation() {
        let requirement = CurrentnessRequirement::new([
            CurrentnessAxis::SourceObservation,
            CurrentnessAxis::Projection,
        ]);
        let states = BTreeMap::from([
            (CurrentnessAxis::SourceObservation, CurrentnessState::Stale),
            (CurrentnessAxis::Projection, CurrentnessState::Current),
        ]);
        let Decision::Deny(reasons) = evaluate_currentness(&requirement, &states) else {
            panic!("stale observation must deny currentness");
        };
        assert_eq!(
            reasons.iter().copied().collect::<Vec<_>>(),
            [CurrentnessReason::StaleAxis(
                CurrentnessAxis::SourceObservation
            )]
        );
    }
}
