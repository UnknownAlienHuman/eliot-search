//! Rebuild verification over the exact materialization, never digest equality alone.
use super::build::{assemble_manifest, verified};
use super::input::{UnitSetInput, UnitizationBudget};
use super::model::{UnitManifest, UnitManifestDiff, VerifiedUnitSet};
use super::v3_profile::ValidatedV3UnitizerProfile;
use crate::UnitizationError;
use search_contracts::Blake3Digest32;
use std::collections::BTreeMap;

/// The computed commitment of an immutable proposed v3 manifest.
#[must_use]
pub const fn manifest_digest(manifest: &UnitManifest) -> Blake3Digest32 {
    manifest.manifest_digest
}

/// Recompute every binding, occurrence, anchor, commitment and complete accounting.
/// A decoded manifest becomes a `VerifiedUnitSet` only after this exact-source rebuild.
pub fn verify_unit_manifest(
    manifest: &UnitManifest,
    input: &UnitSetInput<'_>,
    profile: &ValidatedV3UnitizerProfile,
    budget: &UnitizationBudget<'_>,
) -> Result<VerifiedUnitSet, UnitizationError> {
    let expected = assemble_manifest(input, profile, budget)?;
    if *manifest != expected {
        return Err(UnitizationError::UnitManifestDigestMismatch);
    }
    Ok(verified(expected))
}

/// Compare verified sets. Retention requires identical full occurrence descriptors.
pub fn diff_unit_manifests(
    old: &VerifiedUnitSet,
    new: &VerifiedUnitSet,
) -> Result<UnitManifestDiff, UnitizationError> {
    let old_units: BTreeMap<_, _> = old.units().iter().map(|u| (u.unit_id(), u)).collect();
    let new_units: BTreeMap<_, _> = new.units().iter().map(|u| (u.unit_id(), u)).collect();
    let mut created = Vec::new();
    let mut retained = Vec::new();
    let mut retired = Vec::new();
    for unit in new.units() {
        match old_units.get(&unit.unit_id()) {
            Some(old) if **old == *unit => retained.push(unit.unit_id()),
            Some(_) => return Err(UnitizationError::UnitizationNondeterministic),
            None => created.push(unit.unit_id()),
        }
    }
    for unit in old.units() {
        if !new_units.contains_key(&unit.unit_id()) {
            retired.push(unit.unit_id());
        }
    }
    Ok(UnitManifestDiff {
        old_digest: old.manifest_digest(),
        new_digest: new.manifest_digest(),
        created,
        retained,
        retired,
    })
}
