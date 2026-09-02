//! Non-widening scope algebra and canonical base-eligibility predicates.

use std::collections::BTreeSet;

use search_contracts::{
    AccessPartitionId, AccessPolicyRevision, AuthorizedScopeRef, CollectionGenerationId, Epoch,
    InstallationIncarnationId, LegKind, ProfileId, ProjectionMembershipId, PurgeFenceRevision,
    QuerySnapshotFence, SafeLeg, ScoringPartitionId, ShadowFenceRevision,
};

use crate::{Decision, DomainError, DomainErrorKind, ReasonSet};

/// Relationship between a requested finite set and server-authoritative scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeRelation {
    /// Sets are equal.
    Equal,
    /// The request is a strict subset.
    Narrower,
    /// The request contains values outside authority.
    AttemptsWidening,
}

/// Finite deterministic set used by non-widening scope decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EligibilitySet<T>
where
    T: Ord,
{
    values: BTreeSet<T>,
}

impl<T> EligibilitySet<T>
where
    T: Ord,
{
    /// Creates a finite canonical set.
    #[must_use]
    pub fn new<I>(values: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        Self {
            values: values.into_iter().collect(),
        }
    }

    /// Returns an empty set.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            values: BTreeSet::new(),
        }
    }

    /// Canonically ordered values.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        self.values.iter()
    }

    /// Number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no value remains eligible.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Mathematical intersection.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self
    where
        T: Clone,
    {
        Self {
            values: self.values.intersection(&other.values).cloned().collect(),
        }
    }

    /// Relation to server-authoritative scope.
    #[must_use]
    pub fn relation_to(&self, authoritative: &Self) -> ScopeRelation {
        if self == authoritative {
            ScopeRelation::Equal
        } else if self.values.is_subset(&authoritative.values) {
            ScopeRelation::Narrower
        } else {
            ScopeRelation::AttemptsWidening
        }
    }

    /// Consumes the wrapper.
    #[must_use]
    pub fn into_inner(self) -> BTreeSet<T> {
        self.values
    }
}

impl<T> Default for EligibilitySet<T>
where
    T: Ord,
{
    fn default() -> Self {
        Self::empty()
    }
}

/// Typed scope-decision denial reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EligibilityReason {
    /// Requested values exceed server authority.
    ScopeWidening,
    /// Intersections produced no eligible value.
    EmptyIntersection,
}

/// Result of non-widening scope intersection.
pub type EligibilityDecision<T> = Decision<EligibilitySet<T>, EligibilityReason>;

/// Intersects requested scope with every authoritative constraint.
#[must_use]
pub fn decide_eligibility<T>(
    requested: &EligibilitySet<T>,
    authoritative: &EligibilitySet<T>,
    constraints: &[EligibilitySet<T>],
) -> EligibilityDecision<T>
where
    T: Clone + Ord,
{
    if requested.relation_to(authoritative) == ScopeRelation::AttemptsWidening {
        return Decision::Deny(ReasonSet::one(EligibilityReason::ScopeWidening));
    }
    let eligible = constraints.iter().fold(requested.clone(), |current, next| {
        current.intersection(next)
    });
    if eligible.is_empty() {
        Decision::Deny(ReasonSet::one(EligibilityReason::EmptyIntersection))
    } else {
        Decision::Allow(eligible)
    }
}

/// One closed atom in the base retrieval/IDF eligibility predicate.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EligibilityAtom {
    /// Current installation incarnation.
    InstallationIncarnation(InstallationIncarnationId),
    /// Current collection generation.
    CollectionGeneration(CollectionGenerationId),
    /// Selected projection membership.
    ProjectionMembership(ProjectionMembershipId),
    /// Authorized access partition.
    AccessPartition(AccessPartitionId),
    /// Coherent scoring partition.
    ScoringPartition(ScoringPartitionId),
    /// Current access policy revision.
    AccessPolicyRevision(AccessPolicyRevision),
    /// Visible epoch used by validity intervals.
    VisibleEpoch(Epoch),
    /// Current shadow fence revision.
    ShadowFenceRevision(ShadowFenceRevision),
    /// Current purge fence revision.
    PurgeFenceRevision(PurgeFenceRevision),
    /// Exact vector/analyzer/profile identity.
    Profile(ProfileId),
    /// Server-authoritative scope digest.
    AuthorizedScope(AuthorizedScopeRef),
}

/// Canonically ordered base predicate shared by retrieval and IDF.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseEligibilityPredicate {
    atoms: BTreeSet<EligibilityAtom>,
}

impl BaseEligibilityPredicate {
    /// Canonically ordered atoms.
    #[must_use]
    pub fn atoms(&self) -> impl ExactSizeIterator<Item = &EligibilityAtom> {
        self.atoms.iter()
    }

    /// Number of exact atoms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    /// Whether no atom exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }
}

/// Builds the one canonical predicate used by both retrieval and filtered IDF.
///
/// # Errors
///
/// Rejects invalid snapshots, indexed legs without a collection/epoch pair,
/// and indexed legs without a projection membership.
pub fn build_base_eligibility_predicate(
    snapshot: &QuerySnapshotFence,
    leg: &SafeLeg,
) -> Result<BaseEligibilityPredicate, DomainError> {
    snapshot.validate().map_err(DomainError::from)?;
    let indexed = matches!(
        leg.leg_kind,
        LegKind::Structural | LegKind::Lexical | LegKind::Semantic | LegKind::Rerank
    );
    if indexed
        && (snapshot.collection_generation_id.is_none()
            || snapshot.visible_epoch.is_none()
            || leg.projection_membership_ids.is_empty())
    {
        return Err(DomainError::new(
            DomainErrorKind::InvariantViolation,
            "base_eligibility.indexed_leg",
        ));
    }

    let mut atoms = BTreeSet::new();
    atoms.insert(EligibilityAtom::InstallationIncarnation(
        snapshot.installation_incarnation_id,
    ));
    if let Some(generation) = snapshot.collection_generation_id {
        atoms.insert(EligibilityAtom::CollectionGeneration(generation));
    }
    if let Some(epoch) = snapshot.visible_epoch {
        atoms.insert(EligibilityAtom::VisibleEpoch(epoch));
    }
    atoms.insert(EligibilityAtom::AccessPolicyRevision(
        snapshot.access_policy_revision,
    ));
    atoms.insert(EligibilityAtom::ShadowFenceRevision(
        snapshot.shadow_fence_revision,
    ));
    atoms.insert(EligibilityAtom::PurgeFenceRevision(
        snapshot.purge_fence_revision,
    ));
    atoms.insert(EligibilityAtom::Profile(leg.profile_id.clone()));
    atoms.insert(EligibilityAtom::AuthorizedScope(leg.authorized_scope_ref));
    if let Some(partition) = leg.access_partition_id {
        atoms.insert(EligibilityAtom::AccessPartition(partition));
    }
    if let Some(partition) = leg.scoring_partition_id {
        atoms.insert(EligibilityAtom::ScoringPartition(partition));
    }
    atoms.extend(
        leg.projection_membership_ids
            .iter()
            .copied()
            .map(EligibilityAtom::ProjectionMembership),
    );

    Ok(BaseEligibilityPredicate { atoms })
}

/// Proves exact AST equivalence between retrieval and IDF base filters.
///
/// # Errors
///
/// Returns [`DomainErrorKind::EligibilityFilterMismatch`] when any atom differs.
pub fn prove_retrieval_idf_filter_equivalence(
    retrieval: &BaseEligibilityPredicate,
    idf: &BaseEligibilityPredicate,
) -> Result<(), DomainError> {
    if retrieval == idf {
        Ok(())
    } else {
        Err(DomainError::new(
            DomainErrorKind::EligibilityFilterMismatch,
            "base_eligibility.retrieval_idf",
        ))
    }
}

/// Verifies the digests embedded in a safe leg cannot represent divergent base filters.
///
/// # Errors
///
/// Lexical and semantic legs require an IDF digest exactly equal to the retrieval digest.
/// Other legs may omit IDF; when present it must still be equal.
pub fn verify_safe_leg_filter_digests(leg: &SafeLeg) -> Result<(), DomainError> {
    match (leg.leg_kind, leg.idf_predicate_digest) {
        (LegKind::Lexical | LegKind::Semantic, Some(idf))
            if idf == leg.eligibility_predicate_digest =>
        {
            Ok(())
        }
        (LegKind::Lexical | LegKind::Semantic, _) => Err(DomainError::new(
            DomainErrorKind::EligibilityFilterMismatch,
            "safe_leg.idf_predicate_digest",
        )),
        (_, Some(idf)) if idf != leg.eligibility_predicate_digest => Err(DomainError::new(
            DomainErrorKind::EligibilityFilterMismatch,
            "safe_leg.idf_predicate_digest",
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BaseEligibilityPredicate, EligibilityAtom, EligibilityReason, EligibilitySet,
        ScopeRelation, decide_eligibility, prove_retrieval_idf_filter_equivalence,
    };
    use crate::{Decision, DomainErrorKind};

    #[test]
    fn widening_is_rejected_not_clamped() {
        let authoritative = EligibilitySet::new([1, 2]);
        let requested = EligibilitySet::new([1, 3]);
        let Decision::Deny(reasons) = decide_eligibility(&requested, &authoritative, &[]) else {
            panic!("widening must be denied");
        };
        assert_eq!(
            reasons.iter().copied().collect::<Vec<_>>(),
            [EligibilityReason::ScopeWidening]
        );
    }

    #[test]
    fn every_constraint_only_narrows() {
        let authoritative = EligibilitySet::new([1, 2, 3]);
        let requested = EligibilitySet::new([1, 2]);
        let Decision::Allow(result) =
            decide_eligibility(&requested, &authoritative, &[EligibilitySet::new([2, 3])])
        else {
            panic!("one item remains");
        };
        assert_eq!(result.into_inner(), std::iter::once(2).collect());
        assert_eq!(
            EligibilitySet::new([1]).relation_to(&authoritative),
            ScopeRelation::Narrower
        );
    }

    #[test]
    fn retrieval_and_idf_must_be_exactly_equal() {
        let left = BaseEligibilityPredicate {
            atoms: BTreeSet::new(),
        };
        let mut right_atoms = BTreeSet::new();
        right_atoms.insert(EligibilityAtom::VisibleEpoch(
            search_contracts::Epoch::new(1).expect("epoch"),
        ));
        let right = BaseEligibilityPredicate { atoms: right_atoms };
        let error = prove_retrieval_idf_filter_equivalence(&left, &right)
            .expect_err("different ASTs must fail");
        assert_eq!(error.kind(), DomainErrorKind::EligibilityFilterMismatch);
    }

    use std::collections::BTreeSet;
}
