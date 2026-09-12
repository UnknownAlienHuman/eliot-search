//! Deterministic ticket-issuance plan orchestration.

mod plan;

use std::path::Path;

use crate::ticket_planner::{canonical_json_bytes, choose_decision};

use super::context::validate_context;
use super::control::validate_handoffs;
use super::drafts::load_draft_pair;
use super::model::{
    Checks, TicketIssuanceBuild, TicketIssuanceBuildError,
    TicketIssuanceBuildOptions,
};
use super::repository::{
    open_view, validate_control_schema, validate_control_state,
    validate_output, validate_registries, validate_workflows,
};
use plan::{assemble_plan, launch_class};

/// Builds one non-authoritative advisory plan from one immutable Git tree.
///
/// Invalid repository state is represented in the plan decision whenever a
/// truthful plan can still be assembled. Only failures that prevent immutable
/// repository inspection or canonical output construction return `Err`.
///
/// # Errors
///
/// Returns a closed fatal error when the repository cannot be opened as Git or
/// an immutable fallback view cannot be resolved.
pub fn build_plan(
    root: &Path,
    options: &TicketIssuanceBuildOptions,
) -> Result<TicketIssuanceBuild, TicketIssuanceBuildError> {
    let mut checks = Checks::new();
    let view = open_view(root, options, &mut checks)?;
    let registries =
        validate_registries(&view.tree, &options.package, &mut checks);
    let pair = load_draft_pair(
        &view.tree,
        &options.package,
        &registries,
        &mut checks,
    );
    let sources = pair.as_ref().map_or_else(Vec::new, |pair| {
        validate_context(&view.tree, pair, &options.package, &mut checks)
    });
    validate_control_schema(&view.tree, &registries.launch, &mut checks);
    validate_control_state(&view.tree, &options.package, &mut checks)?;
    validate_workflows(&view.tree, &mut checks)?;
    let accepted_handoffs = pair.as_ref().map_or_else(Vec::new, |pair| {
        validate_handoffs(
            &view.tree,
            pair,
            &options.accepted_handoffs,
            &mut checks,
        )
    });

    let classification = launch_class(&registries.launch, &options.package);
    if pair.as_ref().is_some_and(|pair| {
        super::util::text(&pair.ticket, "launch_class")
            == Some(classification)
            && matches!(classification, "AUTHORIZED" | "CONDITIONAL")
    }) {
        checks.pass(
            "launch-class",
            format!(
                "draft and launch classification agree: {classification}"
            ),
        );
    } else {
        checks.fail(
            "launch-class",
            "PACKAGE_STAGE_MISMATCH",
            "draft and launch classification differ",
        );
    }

    let repository_root = view.tree.root().to_owned();
    let output_target =
        validate_output(&repository_root, &options.output, &mut checks);
    let reason_refs: Vec<&str> =
        checks.reasons().iter().map(String::as_str).collect();
    let decision = choose_decision(view.selection_state, &reason_refs);
    let plan = assemble_plan(
        options,
        &view.tree,
        &registries,
        pair.as_ref(),
        sources,
        accepted_handoffs,
        classification,
        decision,
        &checks,
    );
    let plan_bytes = canonical_json_bytes(&plan);
    Ok(TicketIssuanceBuild::new(
        repository_root,
        plan,
        plan_bytes,
        output_target,
    ))
}
