//! Deterministic source identity resolution over explicit observations.

use std::collections::BTreeSet;

use search_contracts::{
    BoundedList, BoundedSet, OpaqueCanonicalBytes, SourceId, SourceIdentity, SourceNamespaceId,
};

use crate::{
    CanonicalPathKey, IdentityError, MissingIdentityEvidence, StableIdentityEvidence,
    StableIdentityKey, ValidatedIdentityObservation,
};

/// Maximum prior candidates considered by one identity decision.
pub const MAX_IDENTITY_CANDIDATES: usize = 1_024;
/// Maximum active or closed path lookup keys retained per candidate.
pub const MAX_PATH_KEYS_PER_CANDIDATE: usize = 256;

/// Prior durable source identity plus stable and path lookup evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorIdentityCandidate {
    /// Shared durable source identity.
    pub identity: SourceIdentity,
    /// Exact stable identity key accepted for this source.
    pub stable_key: StableIdentityKey,
    /// Currently active path lookup keys.
    pub active_paths: BoundedSet<CanonicalPathKey, MAX_PATH_KEYS_PER_CANDIDATE>,
    /// Closed historical path lookup keys.
    pub closed_paths: BoundedSet<CanonicalPathKey, MAX_PATH_KEYS_PER_CANDIDATE>,
}

/// Finite prior candidate set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorIdentityCandidates(BoundedList<PriorIdentityCandidate, MAX_IDENTITY_CANDIDATES>);

impl PriorIdentityCandidates {
    /// Creates a finite candidate set and rejects duplicate durable source IDs.
    ///
    /// # Errors
    ///
    /// Capacity overflow or duplicate source IDs is rejected.
    pub fn new(candidates: Vec<PriorIdentityCandidate>) -> Result<Self, IdentityError> {
        let mut source_ids = BTreeSet::new();
        for candidate in &candidates {
            if !source_ids.insert(candidate.identity.source_id) {
                return Err(IdentityError::IdentityObservationInvalid);
            }
        }
        BoundedList::new(candidates)
            .map(Self)
            .map_err(|_| IdentityError::IdentityCapacityExceeded)
    }

    /// Candidates in deterministic input order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &PriorIdentityCandidate> {
        self.0.iter()
    }

    /// Number of candidates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether no prior candidate exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Whether exact new identity creation is permitted.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CreationPolicy {
    /// Exact previously unseen stable evidence may yield a draft.
    PermitExactNewIdentity,
    /// Resolution may match only an existing identity.
    ExistingOnly,
}

/// Finite comparison budget and cancellation input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionPolicy {
    /// New identity policy.
    pub creation: CreationPolicy,
    /// Maximum candidates this call may compare.
    pub comparison_budget: usize,
    /// Cancellation observed before resolution begins.
    pub cancelled: bool,
}

impl ResolutionPolicy {
    /// Creates a usable policy.
    ///
    /// # Errors
    ///
    /// A zero or excessive comparison budget is rejected.
    pub const fn new(
        creation: CreationPolicy,
        comparison_budget: usize,
        cancelled: bool,
    ) -> Result<Self, IdentityError> {
        if comparison_budget == 0 || comparison_budget > MAX_IDENTITY_CANDIDATES {
            return Err(IdentityError::IdentityBudgetExhausted);
        }
        Ok(Self {
            creation,
            comparison_budget,
            cancelled,
        })
    }
}

/// Exact evidence supporting one existing match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExistingIdentityEvidence {
    /// Exact stable key used for the match.
    pub stable_key: StableIdentityKey,
    /// Digest of the complete current observation.
    pub observation_evidence_digest: search_contracts::Blake3Digest32,
}

/// Caller-independent draft for one exact previously unseen identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityDraft {
    /// Exact stable key.
    pub stable_key: StableIdentityKey,
    /// Canonical domain-separated stable components.
    pub stable_identity_components: OpaqueCanonicalBytes,
    /// Digest of the complete observation that produced this draft.
    pub observation_evidence_digest: search_contracts::Blake3Digest32,
    /// Whether the current lookup path was previously closed for another source.
    pub path_was_reused: bool,
}

/// Why a deterministic exact resolution could not be produced.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResolutionGap {
    /// Stable identity evidence is unavailable.
    MissingStableEvidence(MissingIdentityEvidence),
    /// Creation is disabled and no exact existing match exists.
    CreationDisabled,
    /// The finite comparison budget was exhausted.
    BudgetExhausted,
}

/// Existing/new/conflict/collision/ambiguity resolution outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityResolution {
    /// One exact existing stable identity matched.
    MatchExisting {
        /// Durable source ID.
        source_id: SourceId,
        /// Exact supporting evidence.
        evidence: ExistingIdentityEvidence,
    },
    /// Exact stable evidence is unseen and may be assigned a caller-supplied ID.
    CreateNew(Box<IdentityDraft>),
    /// More than one candidate remains plausible or stable evidence is absent.
    Ambiguous {
        /// Distinct plausible source IDs in canonical order.
        candidates: BoundedList<SourceId, MAX_IDENTITY_CANDIDATES>,
        /// Material resolution gap.
        gap: ResolutionGap,
    },
    /// One stable identity key maps to multiple durable source IDs.
    Collision {
        /// Conflicting durable source IDs in canonical order.
        source_ids: BoundedList<SourceId, MAX_IDENTITY_CANDIDATES>,
    },
    /// A claimed source or active path conflicts with exact stable evidence.
    Conflict {
        /// Claimed source ID when supplied.
        claimed_source_id: Option<SourceId>,
        /// Existing conflicting source IDs in canonical order.
        existing_source_ids: BoundedList<SourceId, MAX_IDENTITY_CANDIDATES>,
    },
    /// Source kind or stable evidence is explicitly unsupported.
    Unsupported(MissingIdentityEvidence),
}

/// Expected-versus-observed identity comparison.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdentityMatchDecision {
    /// Stable identity keys match exactly.
    ExactMatch,
    /// Exact stable identity keys differ materially.
    MaterialMismatch,
    /// Stable evidence is absent.
    InsufficientEvidence(MissingIdentityEvidence),
    /// Source kind/profile is unsupported.
    Unsupported(MissingIdentityEvidence),
}

/// Resolves one validated observation against a finite prior index.
///
/// Stable identity dominates path similarity. Path or content equality alone
/// never matches or creates a durable identity. No identifier is generated.
///
/// # Errors
///
/// Cancellation, budget exhaustion, malformed candidate identity kind, and
/// bounded-result construction failures are returned explicitly.
pub fn resolve_identity(
    observation: &ValidatedIdentityObservation,
    prior: &PriorIdentityCandidates,
    policy: ResolutionPolicy,
) -> Result<IdentityResolution, IdentityError> {
    if policy.cancelled {
        return Err(IdentityError::IdentityCancelled);
    }
    if prior.len() > policy.comparison_budget {
        return Err(IdentityError::IdentityBudgetExhausted);
    }
    let observed = observation.as_inner();
    let stable_key = match observed.stable_evidence {
        StableIdentityEvidence::Exact(key) => key,
        StableIdentityEvidence::Unavailable(MissingIdentityEvidence::UnsupportedSourceKind) => {
            return Ok(IdentityResolution::Unsupported(
                MissingIdentityEvidence::UnsupportedSourceKind,
            ));
        }
        StableIdentityEvidence::Unavailable(gap) => {
            return Ok(IdentityResolution::Ambiguous {
                candidates: path_candidates(&observed.path_key, prior)?,
                gap: ResolutionGap::MissingStableEvidence(gap),
            });
        }
    };

    let mut exact_ids = BTreeSet::new();
    let mut claimed_conflicts = BTreeSet::new();
    let mut active_path_conflicts = BTreeSet::new();
    let mut path_was_reused = false;

    for candidate in prior.iter() {
        if candidate.identity.identity_kind != candidate.stable_key.identity_kind() {
            return Err(IdentityError::IdentityObservationInvalid);
        }
        if candidate.stable_key == stable_key {
            exact_ids.insert(candidate.identity.source_id);
        }
        if observed.claimed_source_id == Some(candidate.identity.source_id)
            && candidate.stable_key != stable_key
        {
            claimed_conflicts.insert(candidate.identity.source_id);
        }
        if candidate.active_paths.contains(&observed.path_key) && candidate.stable_key != stable_key
        {
            active_path_conflicts.insert(candidate.identity.source_id);
        }
        if candidate.closed_paths.contains(&observed.path_key) && candidate.stable_key != stable_key
        {
            path_was_reused = true;
        }
    }

    if exact_ids.len() > 1 {
        return Ok(IdentityResolution::Collision {
            source_ids: bounded_source_ids(exact_ids)?,
        });
    }
    if let Some(source_id) = exact_ids.iter().next().copied() {
        if observed
            .claimed_source_id
            .is_some_and(|claimed| claimed != source_id)
        {
            let existing_source_ids = bounded_source_ids([source_id])?;
            return Ok(IdentityResolution::Conflict {
                claimed_source_id: observed.claimed_source_id,
                existing_source_ids,
            });
        }
        return Ok(IdentityResolution::MatchExisting {
            source_id,
            evidence: ExistingIdentityEvidence {
                stable_key,
                observation_evidence_digest: observed.evidence_digest,
            },
        });
    }

    claimed_conflicts.extend(active_path_conflicts);
    if !claimed_conflicts.is_empty() {
        return Ok(IdentityResolution::Conflict {
            claimed_source_id: observed.claimed_source_id,
            existing_source_ids: bounded_source_ids(claimed_conflicts)?,
        });
    }

    if policy.creation == CreationPolicy::ExistingOnly {
        return Ok(IdentityResolution::Ambiguous {
            candidates: path_candidates(&observed.path_key, prior)?,
            gap: ResolutionGap::CreationDisabled,
        });
    }

    Ok(IdentityResolution::CreateNew(Box::new(IdentityDraft {
        stable_key,
        stable_identity_components: encode_stable_key(stable_key)?,
        observation_evidence_digest: observed.evidence_digest,
        path_was_reused,
    })))
}

/// Binds a validated new-identity draft to caller-supplied namespace and source IDs.
///
/// # Errors
///
/// Shared canonical component bounds are enforced by draft construction; the
/// function never generates IDs or adds policy/membership fields.
pub fn derive_source_identity(
    draft: IdentityDraft,
    source_namespace_id: SourceNamespaceId,
    source_id: SourceId,
) -> Result<SourceIdentity, IdentityError> {
    if source_namespace_id.as_bytes().iter().all(|byte| *byte == 0)
        || source_id.as_bytes().iter().all(|byte| *byte == 0)
    {
        return Err(IdentityError::SourceIdentityInsufficientEvidence);
    }
    Ok(SourceIdentity {
        source_namespace_id,
        source_id,
        identity_kind: draft.stable_key.identity_kind(),
        stable_identity_components: draft.stable_identity_components,
    })
}

/// Compares exact expected stable identity with an observed evidence value.
#[must_use]
pub fn compare_identity(
    expected: StableIdentityKey,
    observed: StableIdentityEvidence,
) -> IdentityMatchDecision {
    match observed {
        StableIdentityEvidence::Exact(actual) if actual == expected => {
            IdentityMatchDecision::ExactMatch
        }
        StableIdentityEvidence::Exact(_) => IdentityMatchDecision::MaterialMismatch,
        StableIdentityEvidence::Unavailable(MissingIdentityEvidence::UnsupportedSourceKind) => {
            IdentityMatchDecision::Unsupported(MissingIdentityEvidence::UnsupportedSourceKind)
        }
        StableIdentityEvidence::Unavailable(gap) => {
            IdentityMatchDecision::InsufficientEvidence(gap)
        }
    }
}

fn path_candidates(
    path_key: &CanonicalPathKey,
    prior: &PriorIdentityCandidates,
) -> Result<BoundedList<SourceId, MAX_IDENTITY_CANDIDATES>, IdentityError> {
    let ids = prior
        .iter()
        .filter(|candidate| {
            candidate.active_paths.contains(path_key) || candidate.closed_paths.contains(path_key)
        })
        .map(|candidate| candidate.identity.source_id)
        .collect::<BTreeSet<_>>();
    bounded_source_ids(ids)
}

fn bounded_source_ids(
    ids: impl IntoIterator<Item = SourceId>,
) -> Result<BoundedList<SourceId, MAX_IDENTITY_CANDIDATES>, IdentityError> {
    BoundedList::new(ids.into_iter().collect()).map_err(|_| IdentityError::IdentityCapacityExceeded)
}

fn encode_stable_key(key: StableIdentityKey) -> Result<OpaqueCanonicalBytes, IdentityError> {
    let mut bytes = Vec::with_capacity(96);
    bytes.extend_from_slice(b"eliot.search.source-identity.v1\0");
    match key {
        StableIdentityKey::Filesystem {
            volume_identity,
            file_identity,
            generation,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(volume_identity.as_bytes());
            bytes.extend_from_slice(file_identity.as_bytes());
            match generation {
                Some(generation) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&generation.to_be_bytes());
                }
                None => bytes.push(0),
            }
        }
        StableIdentityKey::GitObject {
            lineage_id,
            object_identity,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(lineage_id.as_bytes());
            bytes.extend_from_slice(object_identity.as_bytes());
        }
        StableIdentityKey::Imported { import_identity } => {
            bytes.push(3);
            bytes.extend_from_slice(import_identity.as_bytes());
        }
        StableIdentityKey::VirtualSnapshot { attestation_digest } => {
            bytes.push(4);
            bytes.extend_from_slice(attestation_digest.as_bytes());
        }
    }
    OpaqueCanonicalBytes::from_validated(bytes).map_err(|_| IdentityError::ContractExhausted)
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        Blake3Digest32, BoundedSet, CatalogRevision, NonZeroRevision, OpaqueCanonicalBytes,
        RootBindingId, SourceId, SourceIdentity, SourceIdentityKind, SourceNamespaceId,
    };

    use super::{
        CreationPolicy, IdentityResolution, PriorIdentityCandidate, PriorIdentityCandidates,
        ResolutionPolicy, resolve_identity,
    };
    use crate::{
        CanonicalPathKey, IdentityObservation, ObservationConfidence, StableIdentityEvidence,
        StableIdentityKey, ValidatedIdentityObservation,
    };

    fn key(byte: u8) -> StableIdentityKey {
        StableIdentityKey::Filesystem {
            volume_identity: Blake3Digest32::from_bytes([byte; 32]),
            file_identity: Blake3Digest32::from_bytes([byte.wrapping_add(1); 32]),
            generation: Some(1),
        }
    }

    fn path() -> CanonicalPathKey {
        crate::derive_canonical_path_key(
            &crate::PathObservation {
                root_binding_id: RootBindingId::from_bytes([1; 16]),
                root_relative_lookup_path: "src/lib.rs".into(),
                profile_revision: NonZeroRevision::new(1).expect("revision"),
                profile_schema_digest: Blake3Digest32::from_bytes([2; 32]),
                normalization_attested: true,
            },
            crate::FilesystemIdentityProfile::new(
                NonZeroRevision::new(1).expect("revision"),
                Blake3Digest32::from_bytes([2; 32]),
                crate::CaseBehavior::Sensitive,
                crate::UnicodeBehavior::PreserveScalarValues,
                crate::StableFieldPolicy::Required,
                crate::StableFieldPolicy::Required,
                crate::LinkBehavior::StablePhysicalIdentity,
                crate::ReparseBehavior::FinalTargetIdentity,
            )
            .expect("profile"),
        )
        .expect("path")
    }

    fn observation(stable_key: StableIdentityKey) -> ValidatedIdentityObservation {
        ValidatedIdentityObservation(IdentityObservation {
            path_key: path(),
            stable_evidence: StableIdentityEvidence::Exact(stable_key),
            content_digest_hint: Some(Blake3Digest32::from_bytes([9; 32])),
            metadata_generation: Some(1),
            byte_length_hint: Some(10),
            claimed_source_id: None,
            candidate_catalog_revision: CatalogRevision::new(1),
            profile_revision: NonZeroRevision::new(1).expect("revision"),
            profile_schema_digest: Blake3Digest32::from_bytes([2; 32]),
            confidence: ObservationConfidence::Exact,
            evidence_digest: Blake3Digest32::from_bytes([8; 32]),
        })
    }

    fn candidate(source_byte: u8, stable_key: StableIdentityKey) -> PriorIdentityCandidate {
        PriorIdentityCandidate {
            identity: SourceIdentity {
                source_namespace_id: SourceNamespaceId::from_bytes([3; 16]),
                source_id: SourceId::from_bytes([source_byte; 16]),
                identity_kind: SourceIdentityKind::NtfsFile,
                stable_identity_components: OpaqueCanonicalBytes::from_validated(vec![source_byte])
                    .expect("components"),
            },
            stable_key,
            active_paths: BoundedSet::empty(),
            closed_paths: BoundedSet::empty(),
        }
    }

    #[test]
    fn exact_stable_key_matches_existing() {
        let stable_key = key(1);
        let prior =
            PriorIdentityCandidates::new(vec![candidate(7, stable_key)]).expect("candidates");
        let result = resolve_identity(
            &observation(stable_key),
            &prior,
            ResolutionPolicy::new(CreationPolicy::PermitExactNewIdentity, 10, false)
                .expect("policy"),
        )
        .expect("resolution");
        assert!(matches!(result, IdentityResolution::MatchExisting { .. }));
    }

    #[test]
    fn same_stable_key_for_two_sources_is_collision() {
        let stable_key = key(1);
        let prior =
            PriorIdentityCandidates::new(vec![candidate(7, stable_key), candidate(8, stable_key)])
                .expect("candidates");
        let result = resolve_identity(
            &observation(stable_key),
            &prior,
            ResolutionPolicy::new(CreationPolicy::PermitExactNewIdentity, 10, false)
                .expect("policy"),
        )
        .expect("resolution");
        assert!(matches!(result, IdentityResolution::Collision { .. }));
    }

    #[test]
    fn unseen_exact_key_creates_draft_not_id() {
        let result = resolve_identity(
            &observation(key(1)),
            &PriorIdentityCandidates::new(Vec::new()).expect("candidates"),
            ResolutionPolicy::new(CreationPolicy::PermitExactNewIdentity, 10, false)
                .expect("policy"),
        )
        .expect("resolution");
        assert!(matches!(result, IdentityResolution::CreateNew(_)));
    }
}
