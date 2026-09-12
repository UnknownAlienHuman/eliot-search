//! Idempotent publication of ordinary materialization plan artifacts.

use std::path::{Path, PathBuf};

use crate::context_artifact_io::write_exact_idempotent;
use crate::context_materialization::MaterializationPlanError;

use super::model::MaterializationBuild;

/// Writes `plan.json` and any prospective payload/manifest files below the
/// already validated advisory output directory. Equal replay succeeds;
/// conflicting bytes fail closed.
///
/// # Errors
///
/// Returns a closed output path, symlink, conflict, write or readback failure.
pub fn write_plan(
    root: &Path,
    build: &MaterializationBuild,
) -> Result<Vec<PathBuf>, MaterializationPlanError> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        MaterializationPlanError::new(
            "MATERIALIZATION_OUTPUT_WRITE_FAILED",
            format!("unable to canonicalize repository root: {error}"),
        )
    })?;
    let directory = root.join(
        build
            .output_directory()
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    );
    let mut outputs = Vec::new();
    let plan = directory.join("plan.json");
    write_exact_idempotent(&root, &plan, build.plan_bytes()).map_err(map_output)?;
    outputs.push(plan);
    if let Some(payload) = build.payload_bytes() {
        let path = directory.join("context-manifest.payload.toml");
        write_exact_idempotent(&root, &path, payload).map_err(map_output)?;
        outputs.push(path);
    }
    if let Some(manifest) = build.manifest_bytes() {
        let path = directory.join("context-manifest.prospective.toml");
        write_exact_idempotent(&root, &path, manifest).map_err(map_output)?;
        outputs.push(path);
    }
    for path in &outputs {
        let expected = if path.file_name().and_then(|value| value.to_str()) == Some("plan.json") {
            build.plan_bytes()
        } else if path
            .file_name()
            .and_then(|value| value.to_str())
            == Some("context-manifest.payload.toml")
        {
            build.payload_bytes().unwrap_or_default()
        } else {
            build.manifest_bytes().unwrap_or_default()
        };
        let observed = std::fs::read(path).map_err(|error| {
            MaterializationPlanError::new(
                "MATERIALIZATION_OUTPUT_WRITE_FAILED",
                format!("unable to read plan output: {}: {error}", path.display()),
            )
        })?;
        if observed != expected {
            return Err(MaterializationPlanError::new(
                "MATERIALIZATION_OUTPUT_WRITE_FAILED",
                format!("plan output readback differs: {}", path.display()),
            ));
        }
    }
    Ok(outputs)
}

fn map_output(
    error: crate::context_artifact_io::CandidateOutputError,
) -> MaterializationPlanError {
    let reason = match error.reason() {
        "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT" => "MATERIALIZATION_OUTPUT_PATH_INVALID",
        "OUTPUT_PATH_SYMLINK" => "MATERIALIZATION_OUTPUT_PATH_SYMLINK",
        "CANDIDATE_OUTPUT_CONFLICT" => "MATERIALIZATION_OUTPUT_CONFLICT",
        _ => "MATERIALIZATION_OUTPUT_WRITE_FAILED",
    };
    MaterializationPlanError::new(reason, error.message())
}
