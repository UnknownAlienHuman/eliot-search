//! Immutable scoped manifest CAS and content-free control references.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use search_projection_planner::{
    ProjectionBudget, ProjectionError, ProjectionPlan, ScopeExpectation,
    verify_manifest_reconstruction,
};

use super::digest::{compute_scope_key, hex};
use super::error::ProjectionCompositionError;
use super::model::{ProjectionReference, StoredProjection};
use super::reference::verify_reference_roundtrip;
use super::spec::{MANIFEST_EXTENSION, REFERENCE_BYTES};

/// Persists one composed manifest in the scoped CAS with a control reference.
///
/// Publication is no-clobber with exact readback. Existing identical bytes
/// replay idempotently; divergence fails closed.
pub fn store_projection_manifest(
    root: &Path,
    plan: &ProjectionPlan,
    scope: &ScopeExpectation,
    budget: ProjectionBudget,
) -> Result<StoredProjection, ProjectionCompositionError> {
    let budget = budget
        .validate()
        .map_err(|_| ProjectionCompositionError::from(ProjectionError::InvalidLimits))?;
    verify_manifest_reconstruction(&plan.manifest, &plan.points)
        .map_err(ProjectionCompositionError::from)?;
    if plan.manifest.canonical_bytes.len() > budget.max_manifest_bytes {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }
    let manifest_digest = *blake3::hash(&plan.manifest.canonical_bytes).as_bytes();
    let Some(first) = plan.points.first() else {
        return Err(ProjectionCompositionError::ManifestInvalid);
    };
    if first.identity.key.namespace_id != scope.namespace_id
        || first.identity.key.source_id != scope.source_id
        || first.identity.key.source_revision != scope.source_revision
        || first.identity.key.source_membership_id != scope.source_membership_id
        || first.identity.key.projection_membership_id != scope.projection_membership_id
        || first.identity.key.projection_fingerprint != scope.projection_fingerprint
        || first.identity.key.projection_schema_revision != scope.projection_schema_revision
        || first.identity.key.representation_digest != scope.representation_digest
        || first.identity.key.scoring_partition_digest != scope.scoring_partition_digest
        || first.identity.key.collection_generation_digest != scope.collection_generation_digest
    {
        return Err(ProjectionCompositionError::ScopeMismatch);
    }
    let scope_key = compute_scope_key(scope);
    let (reference_path, objects_dir) = cas_directories(root, &scope_key)?;
    let object_id_hex = hex(&manifest_digest);
    let object_path = objects_dir
        .join(&object_id_hex[..2])
        .join(format!("{object_id_hex}.{MANIFEST_EXTENSION}"));
    if let Some(parent) = object_path.parent() {
        fs::create_dir_all(parent).map_err(|_| ProjectionCompositionError::CasUnavailable)?;
    }
    write_new_or_replay(
        &object_path,
        &plan.manifest.canonical_bytes,
        ProjectionCompositionError::CasConflict,
    )?;
    let point_count =
        u64::try_from(plan.points.len()).map_err(|_| ProjectionCompositionError::BudgetExceeded)?;
    let manifest_bytes = u64::try_from(plan.manifest.canonical_bytes.len())
        .map_err(|_| ProjectionCompositionError::BudgetExceeded)?;
    let reference = ProjectionReference {
        scope_key,
        manifest_digest,
        manifest_bytes,
        point_count,
    };
    let record = reference.to_bytes();
    write_new_or_replay(
        &reference_path,
        &record,
        ProjectionCompositionError::ReferenceConflict,
    )?;
    let reread_object = read_bounded(&object_path, budget.max_manifest_bytes)?;
    if reread_object != plan.manifest.canonical_bytes
        || blake3::hash(&reread_object).as_bytes() != &manifest_digest
    {
        return Err(ProjectionCompositionError::CasConflict);
    }
    let reread_reference = read_bounded(&reference_path, REFERENCE_BYTES)?;
    if reread_reference != record {
        return Err(ProjectionCompositionError::ReferenceConflict);
    }
    verify_reference_roundtrip(&reference)?;
    Ok(StoredProjection {
        reference,
        object_id_hex,
        scope_key_hex: hex(&scope_key),
    })
}

/// Loads exact canonical manifest bytes for one control reference.
pub fn load_projection_manifest_bytes(
    root: &Path,
    reference: &ProjectionReference,
    budget: ProjectionBudget,
) -> Result<Vec<u8>, ProjectionCompositionError> {
    let budget = budget
        .validate()
        .map_err(|_| ProjectionCompositionError::from(ProjectionError::InvalidLimits))?;
    if reference.manifest_bytes
        > u64::try_from(budget.max_manifest_bytes)
            .map_err(|_| ProjectionCompositionError::BudgetExceeded)?
    {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }
    let (reference_path, objects_dir) = cas_directories(root, &reference.scope_key)?;
    let reread_reference = read_bounded(&reference_path, REFERENCE_BYTES)?;
    if reread_reference != reference.to_bytes() {
        return Err(ProjectionCompositionError::ReferenceConflict);
    }
    let object_id_hex = hex(&reference.manifest_digest);
    let object_path = objects_dir
        .join(&object_id_hex[..2])
        .join(format!("{object_id_hex}.{MANIFEST_EXTENSION}"));
    let bytes = read_bounded(&object_path, budget.max_manifest_bytes)?;
    if bytes.len() as u64 != reference.manifest_bytes
        || blake3::hash(&bytes).as_bytes() != &reference.manifest_digest
    {
        return Err(ProjectionCompositionError::CasConflict);
    }
    Ok(bytes)
}

/// Verifies stored bytes equal a freshly composed plan's canonical bytes.
pub fn verify_stored_projection(
    root: &Path,
    stored: &StoredProjection,
    plan: &ProjectionPlan,
    budget: ProjectionBudget,
) -> Result<(), ProjectionCompositionError> {
    let bytes = load_projection_manifest_bytes(root, &stored.reference, budget)?;
    if bytes != plan.manifest.canonical_bytes {
        return Err(ProjectionCompositionError::CasConflict);
    }
    verify_manifest_reconstruction(&plan.manifest, &plan.points)
        .map_err(ProjectionCompositionError::from)?;
    Ok(())
}

fn cas_directories(
    root: &Path,
    scope_key: &[u8; 32],
) -> Result<(PathBuf, PathBuf), ProjectionCompositionError> {
    if !root.is_dir() {
        return Err(ProjectionCompositionError::CasUnavailable);
    }
    let base = root.join("projection");
    let refs = base.join("refs");
    let objects = base.join("objects");
    for directory in [&base, &refs, &objects] {
        fs::create_dir_all(directory).map_err(|_| ProjectionCompositionError::CasUnavailable)?;
    }
    Ok((refs.join(format!("{}.ref", hex(scope_key))), objects))
}

fn write_new_or_replay(
    path: &Path,
    bytes: &[u8],
    conflict: ProjectionCompositionError,
) -> Result<(), ProjectionCompositionError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(bytes)
                .map_err(|_| ProjectionCompositionError::CasUnavailable)?;
            file.sync_all()
                .map_err(|_| ProjectionCompositionError::CasUnavailable)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = read_bounded(path, bytes.len().saturating_add(1))?;
            if existing == bytes {
                Ok(())
            } else {
                Err(conflict)
            }
        }
        Err(_) => Err(ProjectionCompositionError::CasUnavailable),
    }
}

fn read_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>, ProjectionCompositionError> {
    let file = fs::File::open(path).map_err(|_| ProjectionCompositionError::CasUnavailable)?;
    let mut output = Vec::new();
    file.take(u64::try_from(max_bytes).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut output)
        .map_err(|_| ProjectionCompositionError::CasUnavailable)?;
    if output.len() > max_bytes {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }
    Ok(output)
}
