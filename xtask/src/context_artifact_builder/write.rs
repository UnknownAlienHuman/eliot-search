//! Idempotent local publication of ordinary context-artifact candidate files.

use std::path::{Path, PathBuf};

use crate::context_artifact_io::write_exact_idempotent;

use super::model::{CandidateBuild, ContextArtifactBuildError};

/// Writes the exact bundle and candidate metadata once and verifies local
/// readback. Byte-identical replay succeeds; conflicting output fails closed.
///
/// # Errors
///
/// Returns one closed output-path/write/readback failure.
pub fn write_candidate(
    root: &Path,
    build: &CandidateBuild,
) -> Result<(PathBuf, PathBuf), ContextArtifactBuildError> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        ContextArtifactBuildError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            format!("unable to canonicalize repository root: {error}"),
        )
    })?;
    let bundle = root.join(
        build
            .bundle_relative_path()
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    );
    let candidate = root.join(
        build
            .candidate_relative_path()
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    );
    write_exact_idempotent(&root, &bundle, build.bundle_bytes())
        .map_err(map_output)?;
    write_exact_idempotent(&root, &candidate, build.candidate_bytes())
        .map_err(map_output)?;
    let bundle_readback = std::fs::read(&bundle).map_err(|error| {
        ContextArtifactBuildError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            format!("unable to read bundle output: {error}"),
        )
    })?;
    let candidate_readback = std::fs::read(&candidate).map_err(|error| {
        ContextArtifactBuildError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            format!("unable to read candidate output: {error}"),
        )
    })?;
    if bundle_readback != build.bundle_bytes()
        || candidate_readback != build.candidate_bytes()
    {
        return Err(ContextArtifactBuildError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            "candidate local readback failed",
        ));
    }
    Ok((bundle, candidate))
}

fn map_output(
    error: crate::context_artifact_io::CandidateOutputError,
) -> ContextArtifactBuildError {
    ContextArtifactBuildError::new(error.reason(), error.message())
}
