//! Advisory candidate output-root grammar.

use super::error::ContextArtifactError;
use super::spec::ARTIFACT_ROOT;

/// Validates the pure grammar/containment prefix of output handling.
///
/// Symlink and parent-directory checks belong to the filesystem boundary.
///
/// # Errors
///
/// Returns `OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT` when the normalized path is
/// not `ARTIFACT_ROOT` or one of its descendants.
pub fn advisory_output_target(
    relative: &str,
) -> Result<String, ContextArtifactError> {
    let normalized = relative.replace('\\', "/");
    let normalized = normalized.trim_end_matches('/').to_owned();
    if crate::ticket_planner::safe_path(&normalized)
        && crate::ticket_planner::under(&normalized, ARTIFACT_ROOT)
        && normalized != format!("{ARTIFACT_ROOT}/.")
    {
        Ok(normalized)
    } else {
        Err(ContextArtifactError::new(
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            format!("output root must be {ARTIFACT_ROOT} or a descendant"),
        ))
    }
}
