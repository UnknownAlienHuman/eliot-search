//! Coverage graph manifest, authority and workflow policy.

use std::path::Path;

use super::content::ContentCounts;
use super::load::{
    CoverageInputs, boolean, integer, read_text, string, strings,
};

const REQUIRED_PATHS: [(&str, &str); 4] = [
    (
        "operation_module_registry",
        "swarm/coverage/operation-modules.toml",
    ),
    (
        "documentation_node_registry",
        "swarm/coverage/documentation-nodes.toml",
    ),
    (
        "dependency_edge_registry",
        "swarm/coverage/dependency-edges.toml",
    ),
    (
        "module_coverage_registry",
        "swarm/coverage/module-coverage.toml",
    ),
];

pub(super) fn validate(
    root: &Path,
    inputs: &CoverageInputs,
    counts: &ContentCounts,
    errors: &mut Vec<String>,
) {
    validate_manifest(inputs, counts, errors);
    validate_launch_state(inputs, errors);
    validate_workflow(root, errors);
}

fn validate_manifest(
    inputs: &CoverageInputs,
    counts: &ContentCounts,
    errors: &mut Vec<String>,
) {
    let manifest = &inputs.manifest;
    if integer(manifest, "schema_version") != Some(2) {
        errors.push("coverage manifest must be schema v2".to_owned());
    }
    if string(manifest, "status")
        != Some("STATIC_OWNERSHIP_AND_RELATION_COVERAGE_CLOSED_NOT_IMPLEMENTED")
    {
        errors.push("coverage manifest status changed".to_owned());
    }
    if string(manifest, "route_assignment_policy")
        != Some("reviewed_registry_only")
        || boolean(manifest, "heuristic_route_generation_allowed") != Some(false)
    {
        errors.push(
            "coverage routes must be reviewed registry inputs; heuristic assignment is forbidden"
                .to_owned(),
        );
    }
    for (key, expected) in REQUIRED_PATHS {
        if string(manifest, key) != Some(expected) {
            errors.push(format!("coverage manifest path mismatch: {key}"));
        }
    }

    for (key, expected) in [
        ("package_count", inputs.package_rows.len()),
        ("exact_operation_module_count", inputs.operation_rows.len()),
        ("documentation_source_file_count", counts.documentation_files),
        ("documentation_node_count", inputs.documentation_rows.len()),
        ("dependency_edge_count", inputs.dependency_rows.len()),
        ("logical_module_count", inputs.module_rows.len()),
        ("weak_logical_module_count", counts.weak_modules.len()),
        ("package_map_count", inputs.package_rows.len()),
        (
            "package_map_file_count",
            inputs.package_rows.len().saturating_mul(4),
        ),
        ("integration_documentation_node_count", counts.governance),
    ] {
        if integer(manifest, key) != i64::try_from(expected).ok() {
            errors.push(format!("coverage manifest count mismatch: {key}"));
        }
    }

    for key in [
        "implementation_authorized_by_this_manifest",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ] {
        if boolean(manifest, key) != Some(false) {
            errors.push(format!("coverage manifest authority flag changed: {key}"));
        }
    }

    let Some(state) = manifest.get("current_state") else {
        errors.push("coverage manifest current_state missing".to_owned());
        return;
    };
    if integer(state, "implemented_packages") != Some(0)
        || integer(state, "accepted_package_handoffs") != Some(0)
        || integer(state, "accepted_gates") != Some(0)
        || integer(state, "accepted_wave_receipts") != Some(0)
        || string(state, "active_stage") != Some("P00")
        || integer(state, "active_wave") != Some(0)
    {
        errors.push("coverage manifest current state changed".to_owned());
    }
}

fn validate_launch_state(inputs: &CoverageInputs, errors: &mut Vec<String>) {
    if string(&inputs.launch_state, "active_stage") != Some("P00")
        || integer(&inputs.launch_state, "active_wave") != Some(0)
        || strings(&inputs.launch_state, "authorized_packages")
            != vec!["search-contracts".to_owned()]
    {
        errors.push("launch authority moved from P00/W0".to_owned());
    }
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let path = ".github/workflows/package-map-coverage-v2.yml";
    let workflow = match read_text(root, path) {
        Ok(workflow) => workflow,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    for token in [
        "workflow_dispatch:",
        "contents: read",
        "persist-credentials: false",
    ] {
        if !workflow.contains(token) {
            errors.push(format!("coverage workflow missing {token}"));
        }
    }
    for token in [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
        "\n  repository_dispatch:",
    ] {
        if workflow.contains(token) {
            errors.push(format!(
                "coverage workflow has automatic trigger {}",
                token.trim()
            ));
        }
    }
    let lower = workflow.to_ascii_lowercase();
    if lower.contains("python")
        || lower.contains("generate-coverage-graph-v2.py")
        || lower.contains("validate-coverage-graph-v2.py")
    {
        errors.push("coverage workflow invokes retired Python tooling".to_owned());
    }
    if !workflow.contains("generate coverage-graph --check --json") {
        errors.push("coverage workflow does not invoke Rust graph reconciliation".to_owned());
    }
    if !workflow.contains("validate coverage-graph --json") {
        errors.push("coverage workflow does not invoke Rust coverage validator".to_owned());
    }
}
