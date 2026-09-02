//! Shared pure decisions and canonical fingerprint verification.

use std::collections::BTreeSet;
use std::fmt;

use search_contracts::{
    PlanFingerprint, QuerySnapshotFence, QuerySnapshotFingerprint, SearchTaskPlan,
};

use crate::{DomainError, DomainErrorKind};

/// A non-empty, deterministically ordered set of machine-readable reasons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasonSet<R>
where
    R: Ord,
{
    reasons: BTreeSet<R>,
}

impl<R> ReasonSet<R>
where
    R: Ord,
{
    /// Creates a set containing one reason.
    #[must_use]
    pub fn one(reason: R) -> Self {
        Self {
            reasons: BTreeSet::from([reason]),
        }
    }

    /// Builds a non-empty reason set.
    ///
    /// # Errors
    ///
    /// Empty input is rejected.
    pub fn from_non_empty_iter<I>(reasons: I) -> Result<Self, DecisionError>
    where
        I: IntoIterator<Item = R>,
    {
        let reasons = reasons.into_iter().collect::<BTreeSet<_>>();
        if reasons.is_empty() {
            return Err(DecisionError::EmptyReasonSet);
        }
        Ok(Self { reasons })
    }

    /// Canonical reason order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &R> {
        self.reasons.iter()
    }

    /// Number of distinct reasons.
    #[must_use]
    pub fn len(&self) -> usize {
        self.reasons.len()
    }

    /// Whether the set is empty. A valid value always returns false.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reasons.is_empty()
    }

    /// Merges another non-empty set.
    pub fn extend(&mut self, other: Self) {
        self.reasons.extend(other.reasons);
    }
}

/// A pure allow-or-deny decision with typed reasons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision<T, R>
where
    R: Ord,
{
    /// Operation is permitted and carries a deterministic value.
    Allow(T),
    /// Operation is denied with one or more reasons.
    Deny(ReasonSet<R>),
}

impl<T, R> Decision<T, R>
where
    R: Ord,
{
    /// Maps the allowed value while preserving denial reasons.
    #[must_use]
    pub fn map<U, F>(self, function: F) -> Decision<U, R>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Allow(value) => Decision::Allow(function(value)),
            Self::Deny(reasons) => Decision::Deny(reasons),
        }
    }

    /// Whether the decision allows the operation.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow(_))
    }

    /// Combines two decisions without widening either one.
    #[must_use]
    pub fn and<U>(self, other: Decision<U, R>) -> Decision<(T, U), R> {
        match (self, other) {
            (Self::Allow(left), Decision::Allow(right)) => Decision::Allow((left, right)),
            (Self::Deny(mut left), Decision::Deny(right)) => {
                left.extend(right);
                Decision::Deny(left)
            }
            (Self::Deny(reasons), Decision::Allow(_))
            | (Self::Allow(_), Decision::Deny(reasons)) => Decision::Deny(reasons),
        }
    }
}

/// Invalid construction of a pure decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionError {
    /// A denial was requested without a reason.
    EmptyReasonSet,
}

impl fmt::Display for DecisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a denial reason set must not be empty")
    }
}

impl std::error::Error for DecisionError {}

/// Computes the canonical query-snapshot fingerprint using an injected
/// verified BLAKE3-256 function.
///
/// # Errors
///
/// Propagates closed snapshot/canonicalization failures.
pub fn compute_query_snapshot_fingerprint(
    snapshot: &QuerySnapshotFence,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<QuerySnapshotFingerprint, DomainError> {
    let input = snapshot
        .canonical_fingerprint_input()
        .map_err(DomainError::from)?;
    Ok(QuerySnapshotFingerprint::from_bytes(blake3_256(
        input.as_slice(),
    )))
}

/// Computes and verifies the snapshot's embedded fingerprint.
///
/// # Errors
///
/// Returns [`DomainErrorKind::FingerprintMismatch`] when canonical bytes hash
/// to another value.
pub fn compute_and_verify_query_snapshot_fingerprint(
    snapshot: &QuerySnapshotFence,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<QuerySnapshotFingerprint, DomainError> {
    let computed = compute_query_snapshot_fingerprint(snapshot, blake3_256)?;
    if computed != snapshot.snapshot_fingerprint {
        return Err(DomainError::new(
            DomainErrorKind::FingerprintMismatch,
            "query_snapshot.snapshot_fingerprint",
        ));
    }
    Ok(computed)
}

/// Computes the canonical task-plan fingerprint using an injected verified
/// BLAKE3-256 function.
///
/// # Errors
///
/// Propagates closed plan/canonicalization failures.
pub fn compute_plan_fingerprint(
    plan: &SearchTaskPlan,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<PlanFingerprint, DomainError> {
    let input = plan
        .canonical_fingerprint_input()
        .map_err(DomainError::from)?;
    Ok(PlanFingerprint::from_bytes(blake3_256(input.as_slice())))
}

/// Verifies both the embedded snapshot fingerprint and task-plan fingerprint.
///
/// # Errors
///
/// Returns a typed mismatch and never rewrites either immutable plan field.
pub fn compute_and_verify_plan_fingerprint(
    plan: &SearchTaskPlan,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<PlanFingerprint, DomainError> {
    compute_and_verify_query_snapshot_fingerprint(&plan.query_snapshot_fence, &blake3_256)?;
    let computed = compute_plan_fingerprint(plan, &blake3_256)?;
    if computed != plan.plan_fingerprint {
        return Err(DomainError::new(
            DomainErrorKind::FingerprintMismatch,
            "task_plan.plan_fingerprint",
        ));
    }
    Ok(computed)
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        AccessPolicyRevision, Blake3Digest32, BoundedList, CatalogRevision, ClientScopeFence,
        CollectionRouteRevision, ExactnessRequirements, FusionProfileId, GrantFence, GrantId,
        InstallationIncarnationId, MembershipRevision, ObservationCursorRevision,
        ObservationFreshness, ObservationFreshnessState, OpaqueRef, OverlayRevision,
        PlanFingerprint, PlanId, PortfolioRevision, PriorityClass, ProfileFence, ProtocolVersion,
        PurgeFenceRevision, QueryExecutionBudget, QuerySnapshotFence, QuerySnapshotFingerprint,
        RequestId, RequiredDenominator, ScopeDomainId, SearchTaskPlan, ShadowFenceRevision,
        SourceRevisionId, SourceView, UtcTimestamp,
    };

    use super::{
        DecisionError, ReasonSet, compute_and_verify_plan_fingerprint,
        compute_and_verify_query_snapshot_fingerprint, compute_plan_fingerprint,
        compute_query_snapshot_fingerprint,
    };
    use crate::DomainErrorKind;

    fn fake_blake3(bytes: &[u8]) -> [u8; 32] {
        let mut output = [0_u8; 32];
        let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes();
        output[..8].copy_from_slice(&len);
        for (index, byte) in bytes.iter().enumerate() {
            output[8 + index % 24] ^= *byte;
        }
        output
    }

    fn snapshot() -> QuerySnapshotFence {
        QuerySnapshotFence {
            installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
            collection_generation_id: None,
            visible_epoch: None,
            collection_route_revision: CollectionRouteRevision::new(1),
            catalog_revision: CatalogRevision::new(2),
            membership_revision: MembershipRevision::new(3),
            reference_portfolio_revision: Some(PortfolioRevision::new(4)),
            access_policy_revision: AccessPolicyRevision::new(5),
            shadow_fence_revision: ShadowFenceRevision::new(6),
            purge_fence_revision: PurgeFenceRevision::new(7),
            overlay_revision: OverlayRevision::new(8),
            observation_cursor_revision: ObservationCursorRevision::new(9),
            observation_freshness: ObservationFreshness {
                state: ObservationFreshnessState::CurrentConfirmed,
                observation_cursor_revision: ObservationCursorRevision::new(9),
                observed_age_ms: None,
            },
            source_view: SourceView::RetainedRevision(SourceRevisionId::from_bytes([2; 16])),
            workspace_view_revision_ref: None,
            lexical_profile_ids: BoundedList::empty(),
            snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([0; 32]),
        }
    }

    fn plan() -> SearchTaskPlan {
        let mut snapshot = snapshot();
        snapshot.snapshot_fingerprint =
            compute_query_snapshot_fingerprint(&snapshot, fake_blake3).expect("snapshot hash");
        SearchTaskPlan {
            plan_id: PlanId::from_bytes([3; 16]),
            provider_protocol_version: ProtocolVersion { major: 1, minor: 0 },
            request_id: RequestId::from_bytes([4; 16]),
            recipe_request_digest: Blake3Digest32::from_bytes([5; 32]),
            grant_fence: GrantFence {
                grant_id: GrantId::from_bytes([6; 16]),
                revocation_generation: 1,
            },
            client_scope_fence: ClientScopeFence {
                client_scope_ref: OpaqueRef::new("scope:client").expect("scope"),
                scope_domain_id: ScopeDomainId::from_bytes([7; 16]),
            },
            query_snapshot_fence: snapshot,
            source_owner_fences: BoundedList::empty(),
            selected_membership_ids: BoundedList::empty(),
            profile_fence: ProfileFence {
                fusion_profile_id: FusionProfileId::new("fusion-v1").expect("profile"),
                projection_profile_set_ids: BoundedList::empty(),
                optional_provider_profile_ids: BoundedList::empty(),
            },
            overlay_snapshot_refs: BoundedList::empty(),
            query_execution_budget: QueryExecutionBudget {
                priority_class: PriorityClass::Interactive,
                deadline_ms: 1_000,
                max_scoring_legs: 1,
                max_prefetch_candidates_per_leg: 10,
                max_validated_candidates: 5,
                max_source_read_bytes: 1024,
                max_exact_scan_items: 0,
                max_exact_scan_bytes: 0,
                max_materialized_result_bytes: 1024,
                max_cpu_ms: 500,
                max_memory_bytes: 1024 * 1024,
            },
            exactness_requirements: ExactnessRequirements {
                required_denominator: RequiredDenominator::CandidateScope,
                require_current_observation: true,
                allow_truthful_partial: true,
            },
            additional_state_dependencies: BoundedList::empty(),
            plan_fingerprint: PlanFingerprint::from_bytes([0; 32]),
            created_at: UtcTimestamp::parse("2026-09-02T00:00:00.000000Z").expect("time"),
            expires_at: UtcTimestamp::parse("2026-09-02T00:01:00.000000Z").expect("time"),
        }
    }

    #[test]
    fn reason_set_rejects_empty_input() {
        assert_eq!(
            ReasonSet::<u8>::from_non_empty_iter([]),
            Err(DecisionError::EmptyReasonSet)
        );
    }

    #[test]
    fn equal_canonical_snapshot_inputs_produce_equal_fingerprints() {
        let left = compute_query_snapshot_fingerprint(&snapshot(), fake_blake3).expect("hash");
        let right = compute_query_snapshot_fingerprint(&snapshot(), fake_blake3).expect("hash");
        assert_eq!(left, right);
    }

    #[test]
    fn plan_fingerprint_is_deterministic_and_verified() {
        let mut value = plan();
        let computed = compute_plan_fingerprint(&value, fake_blake3).expect("plan hash");
        value.plan_fingerprint = computed;
        assert_eq!(
            compute_and_verify_plan_fingerprint(&value, fake_blake3).expect("verify"),
            computed
        );
    }

    #[test]
    fn embedded_snapshot_mismatch_is_explicit() {
        let error = compute_and_verify_query_snapshot_fingerprint(&snapshot(), fake_blake3)
            .expect_err("embedded zero digest must differ");
        assert_eq!(error.kind(), DomainErrorKind::FingerprintMismatch);
    }
}
