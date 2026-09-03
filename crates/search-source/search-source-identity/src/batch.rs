//! Finite identity batch resolution with complete cancellation/budget accounting.

use std::collections::BTreeMap;

use search_contracts::{BoundedList, OpaqueId};

use crate::{
    CreationPolicy, IdentityError, IdentityResolution, PriorIdentityCandidates,
    ResolutionPolicy, StableIdentityEvidence, ValidatedIdentityObservation, resolve_identity,
};

/// Maximum observations in one pure identity batch.
pub const MAX_IDENTITY_BATCH_ITEMS: usize = 512;

/// One finite batch item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityBatchItem {
    /// Caller-supplied correlation identity.
    pub item_id: OpaqueId,
    /// Validated source observation.
    pub observation: ValidatedIdentityObservation,
    /// Finite prior candidates for this item.
    pub prior: PriorIdentityCandidates,
    /// New identity creation policy for this item.
    pub creation: CreationPolicy,
}

/// Explicit bounded batch control.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityBatchControl {
    /// Maximum total candidate comparisons across the batch.
    pub comparison_budget: usize,
    /// Number of items that may complete before cancellation becomes visible.
    pub cancel_after_items: Option<usize>,
}

impl IdentityBatchControl {
    /// Creates valid batch control.
    ///
    /// # Errors
    ///
    /// Zero comparison budget is rejected.
    pub fn new(
        comparison_budget: usize,
        cancel_after_items: Option<usize>,
    ) -> Result<Self, IdentityError> {
        if comparison_budget == 0 {
            return Err(IdentityError::IdentityBudgetExhausted);
        }
        Ok(Self {
            comparison_budget,
            cancel_after_items,
        })
    }
}

/// One explicitly accounted batch outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityBatchOutcome {
    /// Resolution completed.
    Resolved(Box<IdentityResolution>),
    /// Same canonical path and stable evidence appeared earlier in the batch.
    DuplicateObservation,
    /// Same canonical path carried materially different stable evidence.
    ConflictingObservation,
    /// Item was not processed because cancellation was visible.
    Cancelled,
    /// Item was not processed because the finite comparison budget was exhausted.
    BudgetExhausted,
    /// Item-specific candidate data was invalid.
    Invalid(IdentityError),
}

/// One input-to-output accounting row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityBatchEntry {
    /// Caller-supplied input identity.
    pub item_id: OpaqueId,
    /// Exact terminal outcome for this item.
    pub outcome: IdentityBatchOutcome,
}

/// Complete bounded batch decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityBatchDecision {
    /// One row per input item in original order.
    pub entries: BoundedList<IdentityBatchEntry, MAX_IDENTITY_BATCH_ITEMS>,
    /// Candidate comparisons consumed.
    pub comparisons_used: usize,
    /// Whether cancellation affected at least one item.
    pub cancellation_observed: bool,
    /// Whether budget exhaustion affected at least one item.
    pub budget_exhausted: bool,
}

/// Resolves a finite batch and accounts for every item.
///
/// Duplicate/conflicting observations are detected before identity resolution.
/// Cancellation or budget exhaustion never silently creates or matches an
/// unprocessed identity.
///
/// # Errors
///
/// Batch size or result construction beyond the finite ceiling is rejected.
pub fn resolve_batch(
    items: BoundedList<IdentityBatchItem, MAX_IDENTITY_BATCH_ITEMS>,
    control: IdentityBatchControl,
) -> Result<IdentityBatchDecision, IdentityError> {
    let mut entries = Vec::with_capacity(items.len());
    let mut remaining_budget = control.comparison_budget;
    let mut comparisons_used = 0_usize;
    let mut cancellation_observed = false;
    let mut budget_exhausted = false;
    let mut seen_exact: BTreeMap<
        (crate::CanonicalPathKey, StableIdentityEvidence),
        usize,
    > = BTreeMap::new();
    let mut seen_by_path: BTreeMap<crate::CanonicalPathKey, StableIdentityEvidence> =
        BTreeMap::new();

    for (index, item) in items.into_vec().into_iter().enumerate() {
        let cancelled = control
            .cancel_after_items
            .is_some_and(|limit| index >= limit);
        let outcome = if cancelled {
            cancellation_observed = true;
            IdentityBatchOutcome::Cancelled
        } else {
            let observed = item.observation.as_inner();
            let exact_key = (observed.path_key.clone(), observed.stable_evidence);
            if seen_exact.contains_key(&exact_key) {
                IdentityBatchOutcome::DuplicateObservation
            } else if seen_by_path
                .get(&observed.path_key)
                .is_some_and(|prior_evidence| prior_evidence != &observed.stable_evidence)
            {
                IdentityBatchOutcome::ConflictingObservation
            } else if item.prior.len() > remaining_budget {
                budget_exhausted = true;
                IdentityBatchOutcome::BudgetExhausted
            } else {
                let comparisons = item.prior.len();
                remaining_budget -= comparisons;
                comparisons_used = comparisons_used.saturating_add(comparisons);
                seen_exact.insert(exact_key, index);
                seen_by_path.insert(observed.path_key.clone(), observed.stable_evidence);
                let per_item_budget = comparisons.max(1);
                match ResolutionPolicy::new(item.creation, per_item_budget, false)
                    .and_then(|policy| resolve_identity(&item.observation, &item.prior, policy))
                {
                    Ok(resolution) => IdentityBatchOutcome::Resolved(Box::new(resolution)),
                    Err(error) => IdentityBatchOutcome::Invalid(error),
                }
            }
        };
        entries.push(IdentityBatchEntry {
            item_id: item.item_id,
            outcome,
        });
    }

    let entries = BoundedList::new(entries)
        .map_err(|_| IdentityError::IdentityCapacityExceeded)?;
    Ok(IdentityBatchDecision {
        entries,
        comparisons_used,
        cancellation_observed,
        budget_exhausted,
    })
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        Blake3Digest32, BoundedList, CatalogRevision, NonZeroRevision, OpaqueId,
        RootBindingId,
    };

    use super::{
        IdentityBatchControl, IdentityBatchItem, IdentityBatchOutcome, resolve_batch,
    };
    use crate::{
        CreationPolicy, IdentityObservation, ObservationConfidence, PriorIdentityCandidates,
        StableIdentityEvidence, StableIdentityKey,
    };

    fn observation(path_name: &str, stable_byte: u8) -> crate::ValidatedIdentityObservation {
        let profile = crate::FilesystemIdentityProfile::new(
            NonZeroRevision::new(1).expect("revision"),
            Blake3Digest32::from_bytes([1; 32]),
            crate::CaseBehavior::Sensitive,
            crate::UnicodeBehavior::PreserveScalarValues,
            crate::StableFieldPolicy::Required,
            crate::StableFieldPolicy::Required,
            crate::LinkBehavior::StablePhysicalIdentity,
            crate::ReparseBehavior::FinalTargetIdentity,
        )
        .expect("profile");
        let path_key = crate::derive_canonical_path_key(
            &crate::PathObservation {
                root_binding_id: RootBindingId::from_bytes([2; 16]),
                root_relative_lookup_path: path_name.into(),
                profile_revision: profile.revision(),
                profile_schema_digest: profile.schema_digest(),
                normalization_attested: true,
            },
            profile,
        )
        .expect("path");
        crate::validate_identity_observation(
            IdentityObservation {
                path_key,
                stable_evidence: StableIdentityEvidence::Exact(
                    StableIdentityKey::Filesystem {
                        volume_identity: Blake3Digest32::from_bytes([stable_byte; 32]),
                        file_identity: Blake3Digest32::from_bytes([
                            stable_byte.wrapping_add(1);
                            32
                        ]),
                        generation: Some(1),
                    },
                ),
                content_digest_hint: None,
                metadata_generation: Some(1),
                byte_length_hint: Some(1),
                claimed_source_id: None,
                candidate_catalog_revision: CatalogRevision::new(1),
                profile_revision: profile.revision(),
                profile_schema_digest: profile.schema_digest(),
                confidence: ObservationConfidence::Exact,
                evidence_digest: Blake3Digest32::from_bytes([9; 32]),
            },
            profile,
        )
        .expect("observation")
    }

    fn item(id: &str, path: &str, stable: u8) -> IdentityBatchItem {
        IdentityBatchItem {
            item_id: OpaqueId::new(id).expect("id"),
            observation: observation(path, stable),
            prior: PriorIdentityCandidates::new(Vec::new()).expect("prior"),
            creation: CreationPolicy::PermitExactNewIdentity,
        }
    }

    #[test]
    fn cancellation_accounts_every_remaining_item() {
        let items = BoundedList::new(vec![
            item("item:1", "a", 1),
            item("item:2", "b", 2),
            item("item:3", "c", 3),
        ])
        .expect("batch");
        let result = resolve_batch(
            items,
            IdentityBatchControl::new(10, Some(1)).expect("control"),
        )
        .expect("decision");
        assert_eq!(result.entries.len(), 3);
        assert!(matches!(
            result.entries.as_slice()[1].outcome,
            IdentityBatchOutcome::Cancelled
        ));
        assert!(matches!(
            result.entries.as_slice()[2].outcome,
            IdentityBatchOutcome::Cancelled
        ));
    }

    #[test]
    fn conflicting_same_path_is_not_arbitrarily_resolved() {
        let items = BoundedList::new(vec![
            item("item:1", "same", 1),
            item("item:2", "same", 2),
        ])
        .expect("batch");
        let result = resolve_batch(
            items,
            IdentityBatchControl::new(10, None).expect("control"),
        )
        .expect("decision");
        assert!(matches!(
            result.entries.as_slice()[1].outcome,
            IdentityBatchOutcome::ConflictingObservation
        ));
    }
}
