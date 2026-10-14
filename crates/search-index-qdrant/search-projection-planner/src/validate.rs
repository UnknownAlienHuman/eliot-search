//! Private descriptor lookup and exact coverage over the owner-verified set.
//!
//! The descriptor map is built once. Each planned unit performs one ordered
//! map lookup; no planned unit scans the full verified set. With N units,
//! building and admitting the set costs O(N log N), then coverage costs O(N).

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::UnitId;
use search_unitizer::{UnitDescriptor, VerifiedUnitSet};

use crate::error::ProjectionError;

struct IndexedUnit<'a> {
    descriptor: &'a UnitDescriptor,
    covered: bool,
}

/// Borrowed descriptors and coverage, constructed only from the verified owner.
// Preserve sibling-module access without widening this helper's visibility.
#[allow(clippy::redundant_pub_crate)]
pub(super) struct VerifiedUnitIndex<'a> {
    by_id: BTreeMap<UnitId, IndexedUnit<'a>>,
    #[cfg(test)]
    lookup_count: usize,
}

impl<'a> VerifiedUnitIndex<'a> {
    /// Index each verified descriptor once, refusing duplicate IDs or ordinals.
    pub(super) fn build(unit_set: &'a VerifiedUnitSet) -> Result<Self, ProjectionError> {
        let mut by_id = BTreeMap::new();
        let mut ordinals = BTreeSet::new();
        for descriptor in unit_set.units() {
            if !ordinals.insert(descriptor.occurrence().ordinal)
                || by_id
                    .insert(
                        descriptor.unit_id(),
                        IndexedUnit {
                            descriptor,
                            covered: false,
                        },
                    )
                    .is_some()
            {
                return Err(ProjectionError::DuplicateUnit);
            }
        }
        Ok(Self {
            by_id,
            #[cfg(test)]
            lookup_count: 0,
        })
    }

    /// Find and cover one exact ID, returning the owner's descriptor.
    pub(super) fn admit(&mut self, unit_id: UnitId) -> Result<&'a UnitDescriptor, ProjectionError> {
        #[cfg(test)]
        {
            self.lookup_count += 1;
        }
        let unit = self
            .by_id
            .get_mut(&unit_id)
            .ok_or(ProjectionError::ScopeMismatch)?;
        if unit.covered {
            return Err(ProjectionError::DuplicateUnit);
        }
        unit.covered = true;
        Ok(unit.descriptor)
    }

    /// Require coverage of every verified ID, including its original occurrence.
    pub(super) fn prove_complete(&self) -> Result<(), ProjectionError> {
        if self.by_id.values().any(|unit| !unit.covered) {
            return Err(ProjectionError::MissingUnitReceipt);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_uses_one_descriptor_lookup_per_unit_in_both_orders() {
        let set = fixture::verified_unit_set(&"abcde\n".repeat(512));
        assert_eq!(set.units().len(), 512);
        for reversed in [false, true] {
            let mut index = VerifiedUnitIndex::build(&set).expect("verified index");
            let mut descriptors: Vec<_> = set.units().iter().collect();
            if reversed {
                descriptors.reverse();
            }
            for descriptor in descriptors {
                let admitted = index.admit(descriptor.unit_id()).expect("owner descriptor");
                assert!(std::ptr::eq(admitted, descriptor));
            }
            index.prove_complete().expect("complete coverage");
            assert_eq!(index.lookup_count, set.units().len());
        }
    }

    #[test]
    fn missing_duplicate_and_foreign_ids_cannot_cover_the_set() {
        let set = fixture::verified_unit_set("alpha\nbeta\ngamma\n");
        assert!(set.units().len() > 1);
        let mut index = VerifiedUnitIndex::build(&set).expect("verified index");
        let unit_id = set.units()[0].unit_id();
        index.admit(unit_id).expect("first occurrence");
        assert_eq!(
            index.prove_complete(),
            Err(ProjectionError::MissingUnitReceipt)
        );
        assert_eq!(index.admit(unit_id), Err(ProjectionError::DuplicateUnit));
        assert_eq!(
            index.admit(UnitId::from_bytes([0xab; 16])),
            Err(ProjectionError::ScopeMismatch)
        );
        assert_eq!(
            index.prove_complete(),
            Err(ProjectionError::MissingUnitReceipt)
        );
    }
}
