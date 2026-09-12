//! Advisory materialization-plan output-root grammar.

use super::error::MaterializationPlanError;
use super::spec::PLAN_ROOT;

/// Validates the pure path-grammar and containment prefix of output handling.
///
/// Filesystem creation and symlink-component checks belong to the I/O layer.
///
/// # Errors
///
/// Returns `MATERIALIZATION_OUTPUT_PATH_INVALID` when the normalized path is
/// not `PLAN_ROOT` or one of its descendants.
pub fn advisory_output_target(
    relative: &str,
) -> Result<String, MaterializationPlanError> {
    let normalized = relative.replace('\\', "/");
    let normalized = normalized.trim_end_matches('/').to_owned();
    if crate::ticket_planner::safe_path(&normalized)
        && crate::ticket_planner::under(&normalized, PLAN_ROOT)
    {
        Ok(normalized)
    } else {
        Err(MaterializationPlanError::new(
            "MATERIALIZATION_OUTPUT_PATH_INVALID",
            format!("output root must be {PLAN_ROOT} or a descendant"),
        ))
    }
}
