//! Exact verified-membership coverage for one membership-scoped plan.
//!
//! Completeness is proven against the owner-verified unit set through an
//! indexed membership pass, never through a nested rescan of every expected
//! unit against every planned unit. Coverage is tracked on the same pass that
//! admits the planned units, so a large plan costs one hash lookup per unit
//! rather than `expected_units x planned_units` comparisons.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::UnitId;
use search_unitizer::VerifiedUnitSet;

use crate::error::ProjectionError;

/// One ordered membership index over the complete verified unit set.
///
/// The index is derived from the authoritative set during the same pass that
/// admits the planned units, so coverage and membership can never disagree.
/// Duplicate ordinals and duplicate identities are both refused rather than
/// collapsed, because a repeated unit is not a complete set. Fields are private
/// and the constructor is private: a caller can neither mint nor substitute an
/// index, so the verified set remains the single completeness authority.
pub struct VerifiedUnitIndex {
    /// Exact unit identities keyed by their canonical ordinal.
    by_ordinal: BTreeMap<u64, UnitId>,
    /// Exact unit identities retained for duplicate detection.
    identities: BTreeSet<UnitId>,
}

impl VerifiedUnitIndex {
    /// Builds the ordered index from the owner-verified set.
    pub(crate) fn build(unit_set: &VerifiedUnitSet) -> Result<Self, ProjectionError> {
        let mut by_ordinal: BTreeMap<u64, UnitId> = BTreeMap::new();
        let mut identities: BTreeSet<UnitId> = BTreeSet::new();
        for descriptor in unit_set.units() {
            let occurrence = descriptor.occurrence();
            if !identities.insert(occurrence.unit_id) {
                return Err(ProjectionError::DuplicateUnit);
            }
            if by_ordinal
                .insert(occurrence.ordinal, occurrence.unit_id)
                .is_some()
            {
                return Err(ProjectionError::DuplicateUnit);
            }
        }
        Ok(Self {
            by_ordinal,
            identities,
        })
    }

    /// Records one planned unit against the verified set.
    ///
    /// Every planned unit must belong to the exact set: an absent ordinal, an
    /// absent identity or a mismatched identity fails closed. A repeated
    /// planned unit fails as a duplicate rather than being collapsed.
    pub(crate) fn admit(&self,
        ordinal: u64,
        unit_id: UnitId,
        covered: &mut BTreeSet<u64>,
    ) -> Result<(), ProjectionError> {
        let expected = self
            .by_ordinal
            .get(&ordinal)
            .ok_or(ProjectionError::ScopeMismatch)?;
        if *expected != unit_id || !self.identities.contains(&unit_id) {
            return Err(ProjectionError::ScopeMismatch);
        }
        if !covered.insert(ordinal) {
            return Err(ProjectionError::DuplicateUnit);
        }
        Ok(())
    }

    /// Proves the covered ordinals equal the complete verified set exactly.
    ///
    /// Genuine coverage, not a count comparison: any uncovered verified unit
    /// fails closed as an incomplete plan.
    pub(crate) fn prove_complete(&self, covered: &BTreeSet<u64>) -> Result<(), ProjectionError> {
        if covered.len() != self.by_ordinal.len() {
            return Err(ProjectionError::MissingUnitReceipt);
        }
        for ordinal in self.by_ordinal.keys() {
            if !covered.contains(ordinal) {
                return Err(ProjectionError::MissingUnitReceipt);
            }
        }
        Ok(())
    }
}
