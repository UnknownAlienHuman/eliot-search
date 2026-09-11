mod program;
mod quality;
mod stages;

use std::path::Path;

use super::model::{
    EXPECTED_INTEGRATION_ORDER, EXPECTED_NEXT_ORDER, ProgramReport,
};
use super::parse::{child, get, incomplete, index_all, load_all};
use program::{
    check_current_state, check_discipline, check_identity, check_paths,
};
use quality::{
    check_blockers, check_cases, check_coverage_link, check_cross_cutting,
    check_sequence, check_slo, check_workflow,
};
use stages::{
    check_release_boundary, check_stage_order, check_stages, check_targets,
};

/// Read-only implementation-program validation against `root` (repository root).
#[must_use]
pub fn validate_implementation_program(root: &Path) -> ProgramReport {
    let mut errors: Vec<String> = Vec::new();
    let docs = match load_all(root) {
        Ok(docs) => docs,
        Err(message) => return incomplete(message),
    };
    let indexes = match index_all(&docs) {
        Ok(indexes) => indexes,
        Err(message) => return incomplete(message),
    };
    check_paths(&docs.program, root, &mut errors);
    check_identity(&docs.program, &mut errors);
    check_discipline(&docs.program, &docs.packages_doc, &mut errors);
    let current = check_current_state(
        &docs.program,
        &docs.launch,
        &docs.coverage,
        root,
        &mut errors,
    );
    check_stage_order(&indexes.program_stages, &indexes.stages, &mut errors);
    check_stages(
        &indexes.program_stages,
        &indexes.stages,
        &indexes.packages,
        &docs.stages_doc,
        &indexes.gates,
        &mut errors,
    );
    check_release_boundary(&docs.program, &mut errors);
    check_targets(&indexes.targets, &mut errors);
    check_slo(&docs.program, &docs.metrics, &mut errors);
    check_blockers(&docs.program, &mut errors);
    check_sequence(
        &docs.program,
        "integration_step",
        &EXPECTED_INTEGRATION_ORDER,
        EXPECTED_INTEGRATION_ORDER.len(),
        "integration bootstrap order mismatch",
        "integration step ordinals mismatch",
        &mut errors,
    );
    check_sequence(
        &docs.program,
        "next_step",
        &EXPECTED_NEXT_ORDER,
        EXPECTED_NEXT_ORDER.len(),
        "first implementation sequence mismatch",
        "next-step ordinals mismatch",
        &mut errors,
    );
    check_coverage_link(&docs.coverage, &mut errors);
    check_cross_cutting(&docs.program, &mut errors);
    check_cases(&docs.cases, &mut errors);
    check_workflow(root, &mut errors);
    let boundary = child(Some(&docs.program), "release_boundary", &mut errors);
    let baseline_requirements = get(boundary, "baseline_release_requires")
        .and_then(toml::Value::as_array)
        .map_or(0, Vec::len);
    ProgramReport {
        complete: true,
        passed: errors.is_empty(),
        stages: indexes.program_stages.len(),
        packages: indexes.packages.len(),
        targets: indexes.targets.len(),
        integration_steps: indexes.integration_steps.len(),
        next_steps: indexes.next_steps.len(),
        baseline_requirements,
        current_stage: current.stage,
        current_wave: current.wave,
        cargo_lock_present: current.lock_present,
        errors,
    }
}
