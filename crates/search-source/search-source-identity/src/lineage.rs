//! Repository lineage, worktree boundaries, and workspace-view identity.

use std::collections::BTreeSet;

use search_contracts::{
    Blake3Digest32, BoundedList, BoundedSet, GitObjectId, NonZeroRevision, ReceiptRef,
    RepositoryLineageId, RootBindingId, WorkspaceId, WorkspaceInstance,
    WorkspaceViewRevisionId,
};

use crate::IdentityError;

/// Maximum lineage candidates compared in one decision.
pub const MAX_LINEAGE_CANDIDATES: usize = 256;
/// Maximum remote fingerprints retained as non-authoritative hints.
pub const MAX_REMOTE_FINGERPRINTS: usize = 32;

/// Explicit local repository boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RepositoryBoundary {
    /// Primary checkout/worktree root.
    Worktree,
    /// Nested repository not owned by the parent workspace.
    NestedRepository,
    /// Explicit submodule boundary.
    Submodule,
    /// Bare repository boundary.
    BareRepository,
}

/// Exact local repository/worktree observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryLineageObservation {
    /// Admitted root binding containing the checkout/worktree.
    pub root_binding_id: RootBindingId,
    /// Stable local repository identity when available.
    pub repository_identity_digest: Option<Blake3Digest32>,
    /// Stable object-database identity when available.
    pub object_database_identity_digest: Option<Blake3Digest32>,
    /// Stable worktree/checkout identity when available.
    pub worktree_identity_digest: Option<Blake3Digest32>,
    /// Exact current HEAD object hint; not sufficient for lineage alone.
    pub head_object: Option<GitObjectId>,
    /// Bounded remote fingerprints; names/URLs alone never prove lineage.
    pub remote_fingerprints:
        BoundedSet<Blake3Digest32, MAX_REMOTE_FINGERPRINTS>,
    /// Explicit repository boundary.
    pub boundary: RepositoryBoundary,
    /// Observation profile revision.
    pub profile_revision: NonZeroRevision,
    /// Digest of exact observation evidence.
    pub evidence_digest: Blake3Digest32,
}

/// Validated repository observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedRepositoryObservation(RepositoryLineageObservation);

impl ValidatedRepositoryObservation {
    /// Original validated observation.
    #[must_use]
    pub const fn as_inner(&self) -> &RepositoryLineageObservation {
        &self.0
    }
}

/// Validates an already-captured repository observation.
///
/// # Errors
///
/// Missing both local repository and object-database identity, inconsistent
/// worktree evidence, or a submodule/nested boundary without stable local
/// identity is rejected as ambiguous rather than inferred from remotes.
pub fn validate_repository_observation(
    observation: RepositoryLineageObservation,
) -> Result<ValidatedRepositoryObservation, IdentityError> {
    if observation.repository_identity_digest.is_none()
        && observation.object_database_identity_digest.is_none()
    {
        return Err(IdentityError::LineageIdentityAmbiguous);
    }
    if observation.boundary == RepositoryBoundary::Worktree
        && observation.worktree_identity_digest.is_none()
    {
        return Err(IdentityError::WorkspaceIdentityInvalid);
    }
    if matches!(
        observation.boundary,
        RepositoryBoundary::NestedRepository | RepositoryBoundary::Submodule
    ) && observation.repository_identity_digest.is_none()
    {
        return Err(IdentityError::RepositoryBoundaryConflict);
    }
    Ok(ValidatedRepositoryObservation(observation))
}

/// Previously accepted repository lineage and local boundary evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorRepositoryLineage {
    /// Stable lineage identity.
    pub lineage_id: RepositoryLineageId,
    /// Stable local repository identity when available.
    pub repository_identity_digest: Option<Blake3Digest32>,
    /// Stable object-database identity when available.
    pub object_database_identity_digest: Option<Blake3Digest32>,
    /// Known local worktree identities.
    pub worktree_identity_digests:
        BoundedSet<Blake3Digest32, MAX_LINEAGE_CANDIDATES>,
    /// Explicit accepted repository boundary.
    pub boundary: RepositoryBoundary,
}

/// Explicit relationship proven outside this package.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProvenLineageRelation {
    /// Repositories are proven fork-related but remain separate lineages.
    Fork,
    /// Repositories are proven mirror-related but remain separate lineages.
    Mirror,
    /// Repository is a proven copy but remains a separate lineage until policy says otherwise.
    ProvenCopy,
}

/// Receipt-bound relationship proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageProof {
    /// Existing lineage related to the observation.
    pub prior_lineage_id: RepositoryLineageId,
    /// Explicit relationship.
    pub relation: ProvenLineageRelation,
    /// Exact proof digest.
    pub proof_digest: Blake3Digest32,
    /// External verification receipt.
    pub verification_receipt: ReceiptRef,
}

/// Caller-independent draft for an independent repository lineage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryLineageDraft {
    /// Local repository identity.
    pub repository_identity_digest: Option<Blake3Digest32>,
    /// Object-database identity.
    pub object_database_identity_digest: Option<Blake3Digest32>,
    /// Explicit boundary.
    pub boundary: RepositoryBoundary,
    /// Digest of exact observation evidence.
    pub observation_evidence_digest: Blake3Digest32,
}

/// Deterministic repository lineage decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryLineageDecision {
    /// Exact existing lineage and worktree matched.
    SameLineageSameWorktree {
        /// Existing lineage.
        lineage_id: RepositoryLineageId,
    },
    /// Exact existing lineage matched but worktree identity is new.
    SameLineageNewWorktree {
        /// Existing lineage.
        lineage_id: RepositoryLineageId,
    },
    /// An explicit fork/mirror/copy relationship was proven without collapsing identities.
    RelatedButIndependent {
        /// Existing related lineage.
        prior_lineage_id: RepositoryLineageId,
        /// Explicit relation.
        relation: ProvenLineageRelation,
        /// Proof receipt.
        verification_receipt: ReceiptRef,
    },
    /// Nested repository is an explicit independent boundary.
    NestedBoundary(Box<RepositoryLineageDraft>),
    /// Submodule is an explicit independent boundary.
    SubmoduleBoundary(Box<RepositoryLineageDraft>),
    /// No exact existing lineage matched; caller may assign a new ID.
    Independent(Box<RepositoryLineageDraft>),
    /// More than one existing lineage matches exact local evidence.
    Collision {
        /// Conflicting lineage IDs.
        lineage_ids: BoundedList<RepositoryLineageId, MAX_LINEAGE_CANDIDATES>,
    },
    /// Evidence contradicts a claimed or proven lineage.
    Conflict {
        /// Conflicting lineage ID.
        lineage_id: RepositoryLineageId,
    },
    /// Local evidence remains insufficient.
    Ambiguous,
}

/// Classifies repository lineage without issuing IDs or reading Git/network state.
///
/// # Errors
///
/// Candidate overflow, duplicate prior lineage IDs, and proof identity mismatch
/// are rejected explicitly.
pub fn classify_repository_lineage(
    observation: &ValidatedRepositoryObservation,
    prior: &[PriorRepositoryLineage],
    proof: Option<&LineageProof>,
) -> Result<RepositoryLineageDecision, IdentityError> {
    if prior.len() > MAX_LINEAGE_CANDIDATES {
        return Err(IdentityError::IdentityCapacityExceeded);
    }
    let observed = observation.as_inner();
    let mut prior_ids = BTreeSet::new();
    let mut exact = BTreeSet::new();
    let mut same_worktree = BTreeSet::new();

    for candidate in prior {
        if !prior_ids.insert(candidate.lineage_id) {
            return Err(IdentityError::IdentityObservationInvalid);
        }
        if candidate.boundary != observed.boundary
            && matches!(
                observed.boundary,
                RepositoryBoundary::NestedRepository | RepositoryBoundary::Submodule
            )
        {
            continue;
        }
        let repository_matches = observed.repository_identity_digest.is_some()
            && observed.repository_identity_digest == candidate.repository_identity_digest;
        let object_database_matches = observed.object_database_identity_digest.is_some()
            && observed.object_database_identity_digest
                == candidate.object_database_identity_digest;
        if repository_matches || object_database_matches {
            exact.insert(candidate.lineage_id);
            if observed.worktree_identity_digest.is_some_and(|worktree| {
                candidate.worktree_identity_digests.contains(&worktree)
            }) {
                same_worktree.insert(candidate.lineage_id);
            }
        }
    }

    if exact.len() > 1 {
        return Ok(RepositoryLineageDecision::Collision {
            lineage_ids: bounded_lineage_ids(exact)?,
        });
    }
    if let Some(lineage_id) = exact.iter().next().copied() {
        if proof.is_some_and(|proof| proof.prior_lineage_id != lineage_id) {
            return Ok(RepositoryLineageDecision::Conflict { lineage_id });
        }
        return if same_worktree.contains(&lineage_id) {
            Ok(RepositoryLineageDecision::SameLineageSameWorktree {
                lineage_id,
            })
        } else {
            Ok(RepositoryLineageDecision::SameLineageNewWorktree {
                lineage_id,
            })
        };
    }

    if let Some(proof) = proof {
        if !prior_ids.contains(&proof.prior_lineage_id) {
            return Err(IdentityError::LineageIdentityAmbiguous);
        }
        return Ok(RepositoryLineageDecision::RelatedButIndependent {
            prior_lineage_id: proof.prior_lineage_id,
            relation: proof.relation,
            verification_receipt: proof.verification_receipt.clone(),
        });
    }

    let draft = RepositoryLineageDraft {
        repository_identity_digest: observed.repository_identity_digest,
        object_database_identity_digest: observed.object_database_identity_digest,
        boundary: observed.boundary,
        observation_evidence_digest: observed.evidence_digest,
    };
    Ok(match observed.boundary {
        RepositoryBoundary::NestedRepository => {
            RepositoryLineageDecision::NestedBoundary(Box::new(draft))
        }
        RepositoryBoundary::Submodule => {
            RepositoryLineageDecision::SubmoduleBoundary(Box::new(draft))
        }
        RepositoryBoundary::Worktree | RepositoryBoundary::BareRepository => {
            RepositoryLineageDecision::Independent(Box::new(draft))
        }
    })
}

/// Inputs for a caller-supplied workspace identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceIdentityInput {
    /// Caller-supplied stable workspace identity.
    pub workspace_id: WorkspaceId,
    /// Accepted repository lineage.
    pub lineage_id: RepositoryLineageId,
    /// Admitted root binding.
    pub root_binding_id: RootBindingId,
    /// Stable worktree/checkout identity digest.
    pub worktree_identity_digest: Blake3Digest32,
    /// Exact repository observation evidence digest.
    pub repository_evidence_digest: Blake3Digest32,
}

/// Derives a shared workspace instance without generating identifiers.
///
/// # Errors
///
/// The accepted lineage decision must name the supplied lineage and must not be
/// ambiguous, conflicting, nested, or submodule state.
pub fn derive_workspace_identity(
    input: WorkspaceIdentityInput,
    decision: &RepositoryLineageDecision,
) -> Result<WorkspaceInstance, IdentityError> {
    let accepted_lineage = match decision {
        RepositoryLineageDecision::SameLineageSameWorktree { lineage_id }
        | RepositoryLineageDecision::SameLineageNewWorktree { lineage_id } => *lineage_id,
        RepositoryLineageDecision::RelatedButIndependent { .. }
        | RepositoryLineageDecision::NestedBoundary(_)
        | RepositoryLineageDecision::SubmoduleBoundary(_)
        | RepositoryLineageDecision::Independent(_)
        | RepositoryLineageDecision::Collision { .. }
        | RepositoryLineageDecision::Conflict { .. }
        | RepositoryLineageDecision::Ambiguous => {
            return Err(IdentityError::WorkspaceIdentityInvalid);
        }
    };
    if accepted_lineage != input.lineage_id {
        return Err(IdentityError::WorkspaceIdentityInvalid);
    }
    let worktree_or_checkout_identity = search_contracts::OpaqueId::new(format!(
        "worktree:v1:{}:{}",
        input.worktree_identity_digest, input.repository_evidence_digest
    ))
    .map_err(|_| IdentityError::ContractExhausted)?;
    Ok(WorkspaceInstance {
        workspace_id: input.workspace_id,
        lineage_id: input.lineage_id,
        root_binding_id: input.root_binding_id,
        worktree_or_checkout_identity,
    })
}

/// Version fence for one branch/index/worktree view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceViewFence {
    /// Stable workspace identity.
    pub workspace_id: WorkspaceId,
    /// Caller-supplied view revision identity.
    pub view_revision_id: WorkspaceViewRevisionId,
    /// Monotone local view revision.
    pub revision: NonZeroRevision,
    /// Branch/HEAD state digest.
    pub branch_state_digest: Blake3Digest32,
    /// Git index state digest.
    pub index_state_digest: Blake3Digest32,
    /// Worktree observation digest.
    pub worktree_state_digest: Blake3Digest32,
}

/// Advances a workspace view without changing workspace or lineage identity.
///
/// # Errors
///
/// Workspace mismatch or a non-contiguous revision is rejected.
pub fn advance_workspace_view(
    current: WorkspaceViewFence,
    next: WorkspaceViewFence,
) -> Result<WorkspaceViewFence, IdentityError> {
    if current.workspace_id != next.workspace_id {
        return Err(IdentityError::WorkspaceIdentityInvalid);
    }
    let expected = current
        .revision
        .checked_next()
        .map_err(|_| IdentityError::ContractExhausted)?;
    if next.revision != expected {
        return Err(IdentityError::IdentityRevisionInvalid);
    }
    Ok(next)
}

fn bounded_lineage_ids(
    ids: impl IntoIterator<Item = RepositoryLineageId>,
) -> Result<BoundedList<RepositoryLineageId, MAX_LINEAGE_CANDIDATES>, IdentityError> {
    BoundedList::new(ids.into_iter().collect())
        .map_err(|_| IdentityError::IdentityCapacityExceeded)
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        Blake3Digest32, BoundedSet, NonZeroRevision, RepositoryLineageId,
        RootBindingId,
    };

    use super::{
        PriorRepositoryLineage, RepositoryBoundary, RepositoryLineageDecision,
        RepositoryLineageObservation, classify_repository_lineage,
        validate_repository_observation,
    };

    fn observation(boundary: RepositoryBoundary) -> super::ValidatedRepositoryObservation {
        validate_repository_observation(RepositoryLineageObservation {
            root_binding_id: RootBindingId::from_bytes([1; 16]),
            repository_identity_digest: Some(Blake3Digest32::from_bytes([2; 32])),
            object_database_identity_digest: Some(Blake3Digest32::from_bytes([3; 32])),
            worktree_identity_digest: Some(Blake3Digest32::from_bytes([4; 32])),
            head_object: None,
            remote_fingerprints: BoundedSet::empty(),
            boundary,
            profile_revision: NonZeroRevision::new(1).expect("revision"),
            evidence_digest: Blake3Digest32::from_bytes([5; 32]),
        })
        .expect("observation")
    }

    #[test]
    fn exact_object_database_matches_existing_lineage() {
        let prior = PriorRepositoryLineage {
            lineage_id: RepositoryLineageId::from_bytes([6; 16]),
            repository_identity_digest: None,
            object_database_identity_digest: Some(Blake3Digest32::from_bytes([3; 32])),
            worktree_identity_digests: BoundedSet::empty(),
            boundary: RepositoryBoundary::Worktree,
        };
        let decision = classify_repository_lineage(
            &observation(RepositoryBoundary::Worktree),
            &[prior],
            None,
        )
        .expect("decision");
        assert!(matches!(
            decision,
            RepositoryLineageDecision::SameLineageNewWorktree { .. }
        ));
    }

    #[test]
    fn submodule_is_not_collapsed_into_parent() {
        let decision = classify_repository_lineage(
            &observation(RepositoryBoundary::Submodule),
            &[],
            None,
        )
        .expect("decision");
        assert!(matches!(
            decision,
            RepositoryLineageDecision::SubmoduleBoundary(_)
        ));
    }
}
