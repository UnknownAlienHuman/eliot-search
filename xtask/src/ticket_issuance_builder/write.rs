//! Ordinary local publication for advisory ticket-issuance plans.

use crate::context_artifact_io::write_exact_idempotent;
use crate::ticket_planner::PLAN_BYTE_CEILING;

use super::model::{TicketIssuanceBuild, TicketIssuanceBuildError};

/// Writes the plan only when an ordinary local output path was selected.
///
/// Stdout selection performs no filesystem mutation. Equal replay succeeds;
/// conflicting or unreadable output fails closed. This function cannot write
/// any swarm control-record root because the target was fenced during build.
///
/// # Errors
///
/// Returns `OUTPUT_WRITE_FAILED`, `OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT` or
/// `OUTPUT_PATH_SYMLINK` for bounded output failures.
pub fn write_plan(
    build: &TicketIssuanceBuild,
) -> Result<(), TicketIssuanceBuildError> {
    if build.plan_bytes().len() > PLAN_BYTE_CEILING {
        return Err(TicketIssuanceBuildError::new(
            "OUTPUT_WRITE_FAILED",
            format!(
                "canonical plan exceeds {PLAN_BYTE_CEILING}-byte ceiling"
            ),
        ));
    }
    let Some(target) = build.output_target() else {
        return Ok(());
    };
    write_exact_idempotent(build.root(), target, build.plan_bytes())
        .map_err(|error| {
            let reason = match error.reason() {
                "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT" => {
                    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT"
                }
                "OUTPUT_PATH_SYMLINK" => "OUTPUT_PATH_SYMLINK",
                _ => "OUTPUT_WRITE_FAILED",
            };
            TicketIssuanceBuildError::new(reason, error.message())
        })?;
    let observed = std::fs::read(target).map_err(|error| {
        TicketIssuanceBuildError::new(
            "OUTPUT_WRITE_FAILED",
            format!("unable to read advisory plan output: {error}"),
        )
    })?;
    if observed != build.plan_bytes() {
        return Err(TicketIssuanceBuildError::new(
            "OUTPUT_WRITE_FAILED",
            "advisory plan output differs after readback",
        ));
    }
    Ok(())
}
